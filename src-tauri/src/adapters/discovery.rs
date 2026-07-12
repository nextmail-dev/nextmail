use std::time::Duration;

use hickory_resolver::{proto::rr::RData, Resolver};
use quick_xml::de::from_str;
use reqwest::{redirect::Policy, Client, Url};
use serde::Deserialize;

use crate::{
    domain::{ConnectionSecurity, DiscoveredAccountConfig, ServerConfig},
    error::{CommandError, CommandResult},
};

const AUTOCONFIG_MAX_BYTES: usize = 1_048_576;

pub async fn discover_account_config(email: &str) -> CommandResult<DiscoveredAccountConfig> {
    let (local_part, domain) = parse_email(email)?;

    if let Some(config) = built_in_config(email, domain) {
        return Ok(config);
    }
    if let Some(config) = discover_with_srv(email, domain).await {
        return Ok(config);
    }
    if let Some(config) = discover_with_autoconfig(email, local_part, domain).await {
        return Ok(config);
    }

    Err(CommandError::new("account.discovery_not_found"))
}

pub fn parse_email(email: &str) -> CommandResult<(&str, &str)> {
    let email = email.trim();
    let (local, domain) = email
        .rsplit_once('@')
        .ok_or_else(|| CommandError::new("account.email_invalid"))?;
    let domain_valid = !domain.is_empty()
        && domain.len() <= 253
        && domain.contains('.')
        && domain.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
    if local.is_empty() || local.len() > 64 || !domain_valid {
        return Err(CommandError::new("account.email_invalid"));
    }
    Ok((local, domain))
}

fn built_in_config(email: &str, domain: &str) -> Option<DiscoveredAccountConfig> {
    let domain = domain.to_ascii_lowercase();
    let (
        incoming_host,
        incoming_port,
        incoming_security,
        outgoing_host,
        outgoing_port,
        outgoing_security,
    ) = match domain.as_str() {
        "gmail.com" | "googlemail.com" => (
            "imap.gmail.com",
            993,
            ConnectionSecurity::Tls,
            "smtp.gmail.com",
            465,
            ConnectionSecurity::Tls,
        ),
        "outlook.com" | "hotmail.com" | "live.com" | "office365.com" => (
            "outlook.office365.com",
            993,
            ConnectionSecurity::Tls,
            "smtp.office365.com",
            587,
            ConnectionSecurity::StartTls,
        ),
        "icloud.com" | "me.com" | "mac.com" => (
            "imap.mail.me.com",
            993,
            ConnectionSecurity::Tls,
            "smtp.mail.me.com",
            587,
            ConnectionSecurity::StartTls,
        ),
        "qq.com" => (
            "imap.qq.com",
            993,
            ConnectionSecurity::Tls,
            "smtp.qq.com",
            465,
            ConnectionSecurity::Tls,
        ),
        "163.com" => (
            "imap.163.com",
            993,
            ConnectionSecurity::Tls,
            "smtp.163.com",
            465,
            ConnectionSecurity::Tls,
        ),
        _ => return None,
    };

    Some(DiscoveredAccountConfig {
        source: "built_in".to_owned(),
        incoming: server(incoming_host, incoming_port, incoming_security, email),
        outgoing: server(outgoing_host, outgoing_port, outgoing_security, email),
    })
}

async fn discover_with_srv(email: &str, domain: &str) -> Option<DiscoveredAccountConfig> {
    let resolver = Resolver::builder_tokio().ok()?.build().ok()?;
    let incoming = lookup_srv(
        &resolver,
        [
            (format!("_imaps._tcp.{domain}."), ConnectionSecurity::Tls),
            (
                format!("_imap._tcp.{domain}."),
                ConnectionSecurity::StartTls,
            ),
        ],
        email,
    )
    .await?;
    let outgoing = lookup_srv(
        &resolver,
        [
            (
                format!("_submissions._tcp.{domain}."),
                ConnectionSecurity::Tls,
            ),
            (
                format!("_submission._tcp.{domain}."),
                ConnectionSecurity::StartTls,
            ),
        ],
        email,
    )
    .await?;

    Some(DiscoveredAccountConfig {
        source: "dns_srv".to_owned(),
        incoming,
        outgoing,
    })
}

async fn lookup_srv<const N: usize>(
    resolver: &Resolver<hickory_resolver::net::runtime::TokioRuntimeProvider>,
    queries: [(String, ConnectionSecurity); N],
    username: &str,
) -> Option<ServerConfig> {
    for (query, security) in queries {
        let lookup = match resolver.srv_lookup(query).await {
            Ok(lookup) => lookup,
            Err(_) => continue,
        };
        let mut records = lookup
            .answers()
            .iter()
            .filter_map(|record| match &record.data {
                RData::SRV(value) if value.port > 0 => Some(value.clone()),
                _ => None,
            })
            .collect::<Vec<_>>();
        records.sort_by_key(|record| (record.priority, std::cmp::Reverse(record.weight)));
        if let Some(record) = records.first() {
            return Some(server(
                record.target.to_utf8().trim_end_matches('.'),
                record.port,
                security,
                username,
            ));
        }
    }
    None
}

async fn discover_with_autoconfig(
    email: &str,
    local_part: &str,
    domain: &str,
) -> Option<DiscoveredAccountConfig> {
    let client = Client::builder()
        .timeout(Duration::from_secs(7))
        .redirect(Policy::limited(2))
        .build()
        .ok()?;
    let endpoints = [
        format!("https://autoconfig.{domain}/mail/config-v1.1.xml"),
        format!("https://{domain}/.well-known/autoconfig/mail/config-v1.1.xml"),
    ];
    for endpoint in endpoints {
        let mut url = Url::parse(&endpoint).ok()?;
        url.query_pairs_mut().append_pair("emailaddress", email);
        let response = match client.get(url).send().await {
            Ok(response) if response.status().is_success() => response,
            _ => continue,
        };
        if response
            .content_length()
            .is_some_and(|length| length as usize > AUTOCONFIG_MAX_BYTES)
        {
            continue;
        }
        let bytes = match response.bytes().await {
            Ok(bytes) if bytes.len() <= AUTOCONFIG_MAX_BYTES => bytes,
            _ => continue,
        };
        let xml = match std::str::from_utf8(&bytes) {
            Ok(xml) => xml,
            Err(_) => continue,
        };
        if let Some(config) = parse_autoconfig(xml, email, local_part) {
            return Some(config);
        }
    }
    None
}

fn parse_autoconfig(xml: &str, email: &str, local_part: &str) -> Option<DiscoveredAccountConfig> {
    let document: ClientConfigXml = from_str(xml).ok()?;
    let incoming = document
        .provider
        .incoming
        .iter()
        .find(|server| server.kind.eq_ignore_ascii_case("imap"))?;
    let outgoing = document
        .provider
        .outgoing
        .iter()
        .find(|server| server.kind.eq_ignore_ascii_case("smtp"))?;

    Some(DiscoveredAccountConfig {
        source: "autoconfig".to_owned(),
        incoming: xml_server(incoming, email, local_part)?,
        outgoing: xml_server(outgoing, email, local_part)?,
    })
}

fn xml_server(config: &XmlServer, email: &str, local_part: &str) -> Option<ServerConfig> {
    let username = config
        .username
        .replace("%EMAILADDRESS%", email)
        .replace("%EMAILLOCALPART%", local_part);
    let security = match config.socket_type.to_ascii_uppercase().as_str() {
        "SSL" | "TLS" => ConnectionSecurity::Tls,
        "STARTTLS" => ConnectionSecurity::StartTls,
        "PLAIN" | "NONE" => ConnectionSecurity::None,
        _ => return None,
    };
    if config.hostname.trim().is_empty() || config.port == 0 || username.trim().is_empty() {
        return None;
    }
    Some(server(&config.hostname, config.port, security, &username))
}

fn server(host: &str, port: u16, security: ConnectionSecurity, username: &str) -> ServerConfig {
    ServerConfig {
        host: host.to_owned(),
        port,
        security,
        username: username.to_owned(),
    }
}

#[derive(Debug, Deserialize)]
struct ClientConfigXml {
    #[serde(rename = "emailProvider")]
    provider: EmailProviderXml,
}

#[derive(Debug, Deserialize)]
struct EmailProviderXml {
    #[serde(rename = "incomingServer", default)]
    incoming: Vec<XmlServer>,
    #[serde(rename = "outgoingServer", default)]
    outgoing: Vec<XmlServer>,
}

#[derive(Debug, Deserialize)]
struct XmlServer {
    #[serde(rename = "@type")]
    kind: String,
    hostname: String,
    port: u16,
    #[serde(rename = "socketType")]
    socket_type: String,
    username: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_provider_uses_full_email_as_username() {
        let config = built_in_config("person@gmail.com", "gmail.com").expect("known provider");
        assert_eq!(config.incoming.host, "imap.gmail.com");
        assert_eq!(config.incoming.username, "person@gmail.com");
        assert_eq!(config.outgoing.security, ConnectionSecurity::Tls);
    }

    #[test]
    fn invalid_email_is_rejected() {
        assert!(parse_email("person@localhost").is_err());
        assert!(parse_email("@example.com").is_err());
    }

    #[test]
    fn thunderbird_autoconfig_is_parsed() {
        let xml = r#"
          <clientConfig>
            <emailProvider id="example.com">
              <incomingServer type="imap">
                <hostname>imap.example.com</hostname><port>993</port>
                <socketType>SSL</socketType><username>%EMAILADDRESS%</username>
              </incomingServer>
              <outgoingServer type="smtp">
                <hostname>smtp.example.com</hostname><port>587</port>
                <socketType>STARTTLS</socketType><username>%EMAILLOCALPART%</username>
              </outgoingServer>
            </emailProvider>
          </clientConfig>
        "#;
        let config = parse_autoconfig(xml, "person@example.com", "person").expect("parse config");
        assert_eq!(config.incoming.username, "person@example.com");
        assert_eq!(config.outgoing.username, "person");
    }
}
