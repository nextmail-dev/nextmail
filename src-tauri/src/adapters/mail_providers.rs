//! Built-in IMAP/SMTP endpoints. Extend this table, not discovery orchestration.
//! MX suffixes must be provider-owned DNS zones, never generic hosting/gateway zones.
//! Domain aliases disambiguate personal mail from business mail sharing MX infrastructure.
//! Endpoint references (reviewed 2026-10-29):
//! https://help.aliyun.com/zh/document_detail/36576.html
//! https://support.microsoft.com/en-us/outlook/pop-imap-and-smtp-settings-for-outlook-com
//! https://www.zoho.com/mail/help/imap-access.html
//! https://www.fastmail.help/hc/en-us/articles/360060591153-Manual-DNS-configuration
//! https://support.gmx.com/pop-imap/imap/server.html
use super::discovery::server;
use crate::domain::{ConnectionSecurity, DiscoveredAccountConfig};

struct Provider {
    domains: &'static [&'static str],
    mx: &'static [&'static str],
    imap: &'static str,
    smtp: &'static str,
    smtp_port: u16,
}

macro_rules! provider {
    ([$($domain:literal),*], [$($mx:literal),*], $imap:literal, $smtp:literal, $port:literal) => {
        Provider { domains: &[$($domain),*], mx: &[$($mx),*], imap: $imap, smtp: $smtp, smtp_port: $port }
    };
}

// Personal providers are listed separately where shared MX cannot identify a product.
const PROVIDERS: &[Provider] = &[
    provider!(
        ["qq.com", "foxmail.com"],
        ["mx1.qq.com", "mx2.qq.com", "mx3.qq.com"],
        "imap.qq.com",
        "smtp.qq.com",
        465
    ),
    provider!(
        [],
        ["exmail.qq.com", "mxbiz1.qq.com", "mxbiz2.qq.com"],
        "imap.exmail.qq.com",
        "smtp.exmail.qq.com",
        465
    ),
    provider!(
        ["163.com"],
        ["163.com"],
        "imap.163.com",
        "smtp.163.com",
        465
    ),
    provider!(
        ["126.com"],
        ["126.com"],
        "imap.126.com",
        "smtp.126.com",
        465
    ),
    provider!(
        ["yeah.net"],
        ["yeah.net"],
        "imap.yeah.net",
        "smtp.yeah.net",
        465
    ),
    provider!(
        [],
        ["qiye.163.com", "qiye163.com"],
        "imap.qiye.163.com",
        "smtp.qiye.163.com",
        465
    ),
    provider!(
        ["aliyun.com"],
        [],
        "imap.aliyun.com",
        "smtp.aliyun.com",
        465
    ),
    provider!(
        [],
        ["qiye.aliyun.com", "mxhichina.com"],
        "imap.qiye.aliyun.com",
        "smtp.qiye.aliyun.com",
        465
    ),
    provider!(
        ["gmail.com", "googlemail.com"],
        [
            "aspmx.l.google.com",
            "googlemail.com",
            "smtp.google.com",
            "gmail-smtp-in.l.google.com"
        ],
        "imap.gmail.com",
        "smtp.gmail.com",
        465
    ),
    provider!(
        [
            "outlook.com",
            "hotmail.com",
            "live.com",
            "msn.com",
            "outlook.cn",
            "hotmail.co.uk"
        ],
        [],
        "outlook.office365.com",
        "smtp-mail.outlook.com",
        587
    ),
    provider!(
        [],
        ["mail.protection.outlook.com"],
        "outlook.office365.com",
        "smtp.office365.com",
        587
    ),
    provider!(
        ["icloud.com", "me.com", "mac.com"],
        ["icloud.com"],
        "imap.mail.me.com",
        "smtp.mail.me.com",
        587
    ),
    provider!(
        [
            "yahoo.com",
            "ymail.com",
            "rocketmail.com",
            "yahoo.co.uk",
            "yahoo.fr",
            "yahoo.de"
        ],
        ["yahoodns.net"],
        "imap.mail.yahoo.com",
        "smtp.mail.yahoo.com",
        465
    ),
    provider!(
        ["aol.com", "aim.com"],
        [],
        "imap.aol.com",
        "smtp.aol.com",
        465
    ),
    provider!(
        ["fastmail.com", "fastmail.fm"],
        ["messagingengine.com"],
        "imap.fastmail.com",
        "smtp.fastmail.com",
        465
    ),
    provider!(
        ["zoho.com", "zohomail.com"],
        [],
        "imap.zoho.com",
        "smtp.zoho.com",
        465
    ),
    provider!(
        [],
        ["zoho.com"],
        "imappro.zoho.com",
        "smtppro.zoho.com",
        465
    ),
    provider!([], ["zoho.eu"], "imappro.zoho.eu", "smtppro.zoho.eu", 465),
    provider!(
        [],
        ["zoho.com.cn"],
        "imappro.zoho.com.cn",
        "smtppro.zoho.com.cn",
        465
    ),
    provider!(
        ["yandex.com", "yandex.ru", "ya.ru", "yandex.kz", "yandex.by"],
        ["mx.yandex.net", "mx.yandex.ru"],
        "imap.yandex.com",
        "smtp.yandex.com",
        465
    ),
    provider!(
        ["gmx.com", "gmx.us", "gmx.fr"],
        [],
        "imap.gmx.com",
        "mail.gmx.com",
        465
    ),
    provider!(
        ["gmx.de", "gmx.net", "gmx.at", "gmx.ch"],
        [],
        "imap.gmx.net",
        "mail.gmx.net",
        465
    ),
];

fn matches_suffix(host: &str, suffix: &str) -> bool {
    host == suffix
        || host
            .strip_suffix(suffix)
            .is_some_and(|prefix| prefix.ends_with('.'))
}

pub(super) fn match_provider(
    email: &str,
    domain: &str,
    mx_hosts: &[String],
) -> Option<DiscoveredAccountConfig> {
    let domain = domain.to_ascii_lowercase();
    let exact = PROVIDERS
        .iter()
        .find(|provider| provider.domains.contains(&domain.as_str()));
    let provider = if let Some(provider) = exact {
        provider
    } else {
        // Prefer the most specific suffix; reject unknown or conflicting primary MXs.
        let mut matched: Option<&Provider> = None;
        for host in mx_hosts {
            let host = host.trim_end_matches('.').to_ascii_lowercase();
            let provider = PROVIDERS
                .iter()
                .flat_map(|p| p.mx.iter().map(move |suffix| (p, *suffix)))
                .filter(|(_, suffix)| matches_suffix(&host, suffix))
                .max_by_key(|(_, suffix)| suffix.len())?
                .0;
            if matched.is_some_and(|previous| !std::ptr::eq(previous, provider)) {
                return None;
            }
            matched = Some(provider);
        }
        matched?
    };
    Some(DiscoveredAccountConfig {
        source: "built_in".into(),
        incoming: server(provider.imap, 993, ConnectionSecurity::Tls, email.trim()),
        outgoing: server(
            provider.smtp,
            provider.smtp_port,
            if provider.smtp_port == 587 {
                ConnectionSecurity::StartTls
            } else {
                ConnectionSecurity::Tls
            },
            email.trim(),
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn custom_domains_use_mx_and_longest_suffix() {
        for (mx, expected) in [
            ("MX1.EXMAIL.QQ.COM.", "imap.exmail.qq.com"),
            ("mxbiz1.qq.com", "imap.exmail.qq.com"),
            ("mx.qiye.163.com", "imap.qiye.163.com"),
            ("alt1.aspmx.l.google.com", "imap.gmail.com"),
            (
                "company.mail.protection.outlook.com",
                "outlook.office365.com",
            ),
            ("mx1.qiye.aliyun.com", "imap.qiye.aliyun.com"),
        ] {
            let result = match_provider("user@example.org", "example.org", &[mx.into()]).unwrap();
            assert_eq!(result.incoming.host, expected);
            assert_eq!(result.incoming.username, "user@example.org");
        }
    }
    #[test]
    fn personal_domains_disambiguate_shared_mx() {
        let result = match_provider(
            "user@outlook.com",
            "OUTLOOK.COM",
            &["outlook-com.mail.protection.outlook.com".into()],
        )
        .unwrap();
        assert_eq!(result.outgoing.host, "smtp-mail.outlook.com");
        assert_eq!(result.outgoing.security, ConnectionSecurity::StartTls);
        assert!(match_provider("user@126.com", "126.com", &[]).is_some());
    }
    #[test]
    fn unknown_null_conflicting_and_deceptive_mx_are_not_guessed() {
        for hosts in [
            vec![],
            vec!["."],
            vec!["mx.example.org"],
            vec!["evil-exmail.qq.com"],
            vec!["mx.exmail.qq.com.evil.org"],
            vec!["mx.exmail.qq.com", "smtp.google.com"],
        ] {
            assert!(match_provider(
                "user@example.org",
                "example.org",
                &hosts.into_iter().map(String::from).collect::<Vec<_>>()
            )
            .is_none());
        }
    }
}
