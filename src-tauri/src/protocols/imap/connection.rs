use std::time::Duration;

use async_imap::Session;
use rustls::pki_types::ServerName;
use tokio::{
    io::{AsyncRead, AsyncWrite},
    net::TcpStream,
};

use super::{map_imap_err, timeout::TimeoutStream};
use crate::{
    core::{CommandError, CommandResult, ConnectionSecurity, ImapAccountConfig},
    protocols::native_tls_connector,
};

const IMAP_IO_TIMEOUT: Duration = Duration::from_secs(60);
const IMAP_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

pub(super) trait ImapTransport:
    AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send
{
}

impl<T> ImapTransport for T where T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send {}

pub(super) type BoxedImapTransport = Box<dyn ImapTransport>;

pub(super) async fn connect_session(
    account: &ImapAccountConfig,
) -> CommandResult<Session<BoxedImapTransport>> {
    let stream = tokio::time::timeout(
        IMAP_CONNECT_TIMEOUT,
        TcpStream::connect((account.host.as_str(), account.port)),
    )
    .await
    .map_err(|_| CommandError::retryable("sync.imap_connection_failed"))?
    .map_err(map_imap_err("sync.imap_connection_failed", true))?;
    let transport: BoxedImapTransport = match account.security {
        ConnectionSecurity::None => Box::new(TimeoutStream::new(stream, IMAP_IO_TIMEOUT)),
        ConnectionSecurity::Tls => Box::new(TimeoutStream::new(
            connect_tls(&account.host, stream).await?,
            IMAP_IO_TIMEOUT,
        )),
        ConnectionSecurity::StartTls => {
            let mut client = async_imap::Client::new(stream);
            read_greeting(&mut client).await?;
            client
                .run_command_and_check_ok("STARTTLS", None)
                .await
                .map_err(map_imap_err("sync.imap_starttls_failed", false))?;
            Box::new(TimeoutStream::new(
                connect_tls(&account.host, client.into_inner()).await?,
                IMAP_IO_TIMEOUT,
            ))
        }
    };
    let mut client = async_imap::Client::new(transport);
    if account.security != ConnectionSecurity::StartTls {
        read_greeting(&mut client).await?;
    }
    login(client, account).await
}

async fn read_greeting<T>(client: &mut async_imap::Client<T>) -> CommandResult<()>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    client
        .read_response()
        .await
        .map_err(map_imap_err("sync.imap_greeting_failed", false))?
        .ok_or_else(|| CommandError::new("sync.imap_greeting_failed"))?;
    Ok(())
}

async fn login<T>(
    client: async_imap::Client<T>,
    account: &ImapAccountConfig,
) -> CommandResult<Session<T>>
where
    T: AsyncRead + AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    client
        .login(&account.username, &account.password)
        .await
        .map_err(map_imap_err("sync.imap_authentication_failed", false))
}

async fn connect_tls(
    host: &str,
    stream: TcpStream,
) -> CommandResult<tokio_rustls::client::TlsStream<TcpStream>> {
    let server_name = ServerName::try_from(host.to_owned())
        .map_err(map_imap_err("sync.server_name_invalid", false))?;
    native_tls_connector("sync.system_certificates_unavailable")?
        .connect(server_name, stream)
        .await
        .map_err(map_imap_err("sync.imap_tls_failed", true))
}
