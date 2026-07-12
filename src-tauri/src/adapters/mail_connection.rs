use std::{sync::Arc, time::Duration};

use async_trait::async_trait;
use lettre::{
    address::Envelope,
    transport::smtp::{
        authentication::Credentials,
        client::{Tls, TlsParameters},
    },
    AsyncSmtpTransport, AsyncTransport, Tokio1Executor,
};
use rustls::{pki_types::ServerName, ClientConfig, RootCertStore};
use tokio::{net::TcpStream, time::timeout};
use tokio_rustls::TlsConnector;

use crate::{
    domain::{AccountDraft, ConnectionSecurity, ConnectionTestResult, ServerConfig},
    error::{CommandError, CommandResult},
};

const CONNECTION_TIMEOUT: Duration = Duration::from_secs(20);

#[async_trait]
pub trait ConnectionTester: Send + Sync {
    async fn test(&self, draft: &AccountDraft) -> CommandResult<ConnectionTestResult>;
}

pub async fn send_raw_smtp(
    config: &ServerConfig,
    password: &str,
    envelope: &Envelope,
    raw: &[u8],
) -> CommandResult<()> {
    let tls = match config.security {
        ConnectionSecurity::None => Tls::None,
        ConnectionSecurity::StartTls => Tls::Required(smtp_tls_parameters(&config.host)?),
        ConnectionSecurity::Tls => Tls::Wrapper(smtp_tls_parameters(&config.host)?),
    };
    let transport = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host)
        .port(config.port)
        .tls(tls)
        .credentials(Credentials::new(
            config.username.clone(),
            password.to_owned(),
        ))
        .build();
    let result = timeout(CONNECTION_TIMEOUT, transport.send_raw(envelope, raw))
        .await
        .map_err(|_| CommandError::retryable("send.smtp_timeout"))?
        .map_err(|error| {
            if error.is_transient() || error.is_timeout() {
                CommandError::retryable("send.smtp_temporary_failure")
            } else if error.is_client() {
                CommandError::new("send.smtp_authentication_failed")
            } else {
                CommandError::new("send.smtp_rejected")
            }
        });
    transport.shutdown().await;
    result.map(|_| ())
}

#[derive(Default)]
pub struct MailConnectionTester;

#[async_trait]
impl ConnectionTester for MailConnectionTester {
    async fn test(&self, draft: &AccountDraft) -> CommandResult<ConnectionTestResult> {
        validate_connection_request(draft)?;

        let password = draft.password.clone();
        let imap = timeout(CONNECTION_TIMEOUT, test_imap(&draft.incoming, &password))
            .await
            .map_err(|_| CommandError::retryable("account.imap_timeout"))??;

        timeout(CONNECTION_TIMEOUT, test_smtp(&draft.outgoing, &password))
            .await
            .map_err(|_| CommandError::retryable("account.smtp_timeout"))??;

        Ok(ConnectionTestResult {
            imap_capabilities: imap,
            smtp_authenticated: true,
        })
    }
}

fn validate_connection_request(draft: &AccountDraft) -> CommandResult<()> {
    if draft.password.is_empty() {
        return Err(CommandError::new("account.password_required"));
    }
    for server in [&draft.incoming, &draft.outgoing] {
        if server.host.trim().is_empty() || server.username.trim().is_empty() || server.port == 0 {
            return Err(CommandError::new("account.server_config_invalid"));
        }
    }
    let uses_plaintext = matches!(draft.incoming.security, ConnectionSecurity::None)
        || matches!(draft.outgoing.security, ConnectionSecurity::None);
    if uses_plaintext && !draft.insecure_acknowledged {
        return Err(CommandError::new(
            "account.insecure_acknowledgement_required",
        ));
    }
    Ok(())
}

async fn test_imap(config: &ServerConfig, password: &str) -> CommandResult<Vec<String>> {
    match config.security {
        ConnectionSecurity::None => {
            let stream = connect_tcp(config, "account.imap_connection_failed").await?;
            let mut client = async_imap::Client::new(stream);
            read_imap_greeting(&mut client).await?;
            authenticate_imap(client, config, password).await
        }
        ConnectionSecurity::Tls => {
            let stream = connect_tcp(config, "account.imap_connection_failed").await?;
            let tls = connect_tls(&config.host, stream, "account.imap_tls_failed").await?;
            let mut client = async_imap::Client::new(tls);
            read_imap_greeting(&mut client).await?;
            authenticate_imap(client, config, password).await
        }
        ConnectionSecurity::StartTls => {
            let stream = connect_tcp(config, "account.imap_connection_failed").await?;
            let mut client = async_imap::Client::new(stream);
            read_imap_greeting(&mut client).await?;
            client
                .run_command_and_check_ok("STARTTLS", None)
                .await
                .map_err(|_| CommandError::new("account.imap_starttls_failed"))?;
            let stream = client.into_inner();
            let tls = connect_tls(&config.host, stream, "account.imap_tls_failed").await?;
            authenticate_imap(async_imap::Client::new(tls), config, password).await
        }
    }
}

async fn read_imap_greeting<T>(client: &mut async_imap::Client<T>) -> CommandResult<()>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    client
        .read_response()
        .await
        .map_err(|_| CommandError::new("account.imap_greeting_failed"))?
        .ok_or_else(|| CommandError::new("account.imap_greeting_failed"))?;
    Ok(())
}

async fn authenticate_imap<T>(
    client: async_imap::Client<T>,
    config: &ServerConfig,
    password: &str,
) -> CommandResult<Vec<String>>
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let mut session = client
        .login(&config.username, password)
        .await
        .map_err(|_| CommandError::new("account.imap_authentication_failed"))?;
    let capabilities = session
        .capabilities()
        .await
        .map_err(|_| CommandError::new("account.imap_capability_failed"))?;
    let values = capabilities
        .iter()
        .map(|capability| format!("{capability:?}"))
        .collect();
    let _ = session.logout().await;
    Ok(values)
}

async fn test_smtp(config: &ServerConfig, password: &str) -> CommandResult<()> {
    let tls = match config.security {
        ConnectionSecurity::None => Tls::None,
        ConnectionSecurity::StartTls => Tls::Required(smtp_tls_parameters(&config.host)?),
        ConnectionSecurity::Tls => Tls::Wrapper(smtp_tls_parameters(&config.host)?),
    };
    let transport = AsyncSmtpTransport::<Tokio1Executor>::builder_dangerous(&config.host)
        .port(config.port)
        .tls(tls)
        .credentials(Credentials::new(
            config.username.clone(),
            password.to_owned(),
        ))
        .build();
    let authenticated = transport
        .test_connection()
        .await
        .map_err(|_| CommandError::new("account.smtp_authentication_failed"))?;
    transport.shutdown().await;
    if !authenticated {
        return Err(CommandError::new("account.smtp_authentication_failed"));
    }
    Ok(())
}

fn smtp_tls_parameters(host: &str) -> CommandResult<TlsParameters> {
    TlsParameters::builder(host.to_owned())
        .build_rustls()
        .map_err(|_| CommandError::new("account.smtp_tls_configuration_failed"))
}

async fn connect_tcp(config: &ServerConfig, code: &str) -> CommandResult<TcpStream> {
    TcpStream::connect((config.host.as_str(), config.port))
        .await
        .map_err(|_| CommandError::retryable(code))
}

async fn connect_tls(
    host: &str,
    stream: TcpStream,
    code: &str,
) -> CommandResult<tokio_rustls::client::TlsStream<TcpStream>> {
    let native = rustls_native_certs::load_native_certs();
    if native.certs.is_empty() {
        return Err(CommandError::new("account.system_certificates_unavailable"));
    }
    let mut roots = RootCertStore::empty();
    for certificate in native.certs {
        let _ = roots.add(certificate);
    }
    if roots.is_empty() {
        return Err(CommandError::new("account.system_certificates_unavailable"));
    }
    let config = ClientConfig::builder()
        .with_root_certificates(roots)
        .with_no_client_auth();
    let server_name = ServerName::try_from(host.to_owned())
        .map_err(|_| CommandError::new("account.server_name_invalid"))?;
    TlsConnector::from(Arc::new(config))
        .connect(server_name, stream)
        .await
        .map_err(|_| CommandError::new(code))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(security: ConnectionSecurity, acknowledged: bool) -> AccountDraft {
        let server = ServerConfig {
            host: "mail.example.com".to_owned(),
            port: 143,
            security,
            username: "user@example.com".to_owned(),
        };
        AccountDraft {
            email: "user@example.com".to_owned(),
            display_name: "User".to_owned(),
            password: "secret".to_owned(),
            incoming: server.clone(),
            outgoing: server,
            insecure_acknowledged: acknowledged,
        }
    }

    #[test]
    fn plaintext_requires_explicit_acknowledgement() {
        let error = validate_connection_request(&draft(ConnectionSecurity::None, false))
            .expect_err("plaintext connection should require acknowledgement");
        assert_eq!(error.code, "account.insecure_acknowledgement_required");
    }

    #[test]
    fn tls_does_not_require_plaintext_acknowledgement() {
        validate_connection_request(&draft(ConnectionSecurity::Tls, false))
            .expect("TLS connection is safe by default");
    }
}
