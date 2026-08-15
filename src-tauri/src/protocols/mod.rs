mod compose;
mod css;
mod html;
mod imap;
mod links;
mod tls;

pub use compose::*;
pub use html::*;
pub use imap::*;
pub use links::*;
pub(crate) use tls::native_tls_connector;

use crate::core::{CommandError, CommandResult};
use mail_parser::{parsers::MessageStream, MessagePart, MimeHeaders};

pub(crate) fn attachment_file_name(part: &MessagePart<'_>, fallback: &str) -> String {
    normalize_attachment_file_name(part.attachment_name(), fallback)
}

/// Sends RFC 2971 IMAP ID when the server advertises the capability. Some
/// providers (NetEase 163/126/188) refuse mailbox operations for clients
/// that never identify themselves ("Unsafe Login"). Failures stay non-fatal:
/// servers without the gate keep working, and a gated server surfaces its own
/// rejection at mailbox open if the ID command itself was rejected.
pub(crate) async fn send_imap_id_if_supported<T>(session: &mut async_imap::Session<T>)
where
    T: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + std::fmt::Debug + Send,
{
    let capabilities = match session.capabilities().await {
        Ok(capabilities) => capabilities,
        Err(error) => {
            tracing::warn!(%error, "imap capability refresh failed; skipping IMAP ID");
            return;
        }
    };
    if !capabilities.has_str("ID") {
        return;
    }
    if let Err(error) = session
        .id([
            ("name", Some("NextMail")),
            ("version", Some(env!("CARGO_PKG_VERSION"))),
            ("vendor", Some("NextMail")),
        ])
        .await
    {
        tracing::warn!(
            %error,
            "imap ID command failed; servers gating mailboxes on client identity will reject further operations"
        );
    }
}

pub(crate) fn normalize_attachment_file_name(value: Option<&str>, fallback: &str) -> String {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return fallback.to_owned();
    };
    decode_complete_rfc2047_word(value).unwrap_or_else(|| value.to_owned())
}

fn decode_complete_rfc2047_word(value: &str) -> Option<String> {
    let encoded = value.strip_prefix('=')?;
    let mut stream = MessageStream::new(encoded.as_bytes());
    let decoded = stream.decode_rfc2047()?;
    (stream.remaining() == 0 && !decoded.trim().is_empty()).then_some(decoded)
}

pub fn extract_attachment(raw: &[u8], part_index: u32) -> CommandResult<Vec<u8>> {
    let message = mail_parser::MessageParser::default()
        .parse(raw)
        .ok_or_else(|| CommandError::new("message.mime_parse_failed"))?;
    message
        .attachment(part_index)
        .map(|attachment| attachment.contents().to_vec())
        .ok_or_else(|| CommandError::new("attachment.not_found"))
}

#[cfg(test)]
mod tests {
    use super::{decode_complete_rfc2047_word, send_imap_id_if_supported};
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

    #[test]
    fn attachment_name_compatibility_requires_one_complete_encoded_word() {
        assert_eq!(
            decode_complete_rfc2047_word("=?UTF-8?Q?report=2Exlsx?=").as_deref(),
            Some("report.xlsx")
        );
        assert_eq!(
            decode_complete_rfc2047_word("=?UTF-8?Q?report=2Exlsx"),
            None
        );
        assert_eq!(
            decode_complete_rfc2047_word("=?UTF-8?Q?report=2Exlsx?= trailing"),
            None
        );
    }

    #[tokio::test]
    async fn sends_imap_id_when_the_server_advertises_the_capability() {
        let (client_stream, server) = tokio::io::duplex(1 << 16);
        let server_task = tokio::spawn(async move {
            let mut lines = BufReader::new(server);
            let mut line = String::new();
            lines.read_line(&mut line).await.unwrap();
            let tag = line.split_whitespace().next().unwrap().to_owned();
            lines
                .get_mut()
                .write_all(format!("{tag} OK LOGIN completed\r\n").as_bytes())
                .await
                .unwrap();
            line.clear();
            lines.read_line(&mut line).await.unwrap();
            let tag = line.split_whitespace().next().unwrap().to_owned();
            assert!(line.starts_with(&format!("{tag} CAPABILITY")));
            lines
                .get_mut()
                .write_all(
                    format!("* CAPABILITY IMAP4rev1 ID MOVE\r\n{tag} OK CAPABILITY done\r\n")
                        .as_bytes(),
                )
                .await
                .unwrap();
            line.clear();
            lines.read_line(&mut line).await.unwrap();
            assert!(
                line.contains(r#"ID ("name" "NextMail""#),
                "expected IMAP ID command, got: {line}"
            );
            assert!(line.contains(concat!(r#""version" ""#, env!("CARGO_PKG_VERSION"), r#"""#)));
            let tag = line.split_whitespace().next().unwrap().to_owned();
            lines
                .get_mut()
                .write_all(format!("* ID (\"name\" \"server\")\r\n{tag} OK ID done\r\n").as_bytes())
                .await
                .unwrap();
        });

        let mut session = async_imap::Client::new(client_stream)
            .login("user", "pass")
            .await
            .unwrap();
        send_imap_id_if_supported(&mut session).await;
        server_task.await.unwrap();
    }

    #[tokio::test]
    async fn skips_imap_id_without_the_capability() {
        let (client_stream, server) = tokio::io::duplex(1 << 16);
        let server_task = tokio::spawn(async move {
            let mut lines = BufReader::new(server);
            let mut line = String::new();
            lines.read_line(&mut line).await.unwrap();
            let tag = line.split_whitespace().next().unwrap().to_owned();
            lines
                .get_mut()
                .write_all(format!("{tag} OK LOGIN completed\r\n").as_bytes())
                .await
                .unwrap();
            line.clear();
            lines.read_line(&mut line).await.unwrap();
            let tag = line.split_whitespace().next().unwrap().to_owned();
            lines
                .get_mut()
                .write_all(
                    format!("* CAPABILITY IMAP4rev1 MOVE\r\n{tag} OK CAPABILITY done\r\n")
                        .as_bytes(),
                )
                .await
                .unwrap();
            line.clear();
            lines.read_line(&mut line).await.unwrap();
            assert!(
                !line.contains("ID"),
                "no ID command expected without the capability, got: {line}"
            );
            let tag = line.split_whitespace().next().unwrap().to_owned();
            lines
                .get_mut()
                .write_all(format!("{tag} OK LOGOUT done\r\n").as_bytes())
                .await
                .unwrap();
        });

        let mut session = async_imap::Client::new(client_stream)
            .login("user", "pass")
            .await
            .unwrap();
        send_imap_id_if_supported(&mut session).await;
        let _ = session.logout().await;
        server_task.await.unwrap();
    }
}
