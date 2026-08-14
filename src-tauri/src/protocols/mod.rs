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
    use super::decode_complete_rfc2047_word;

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
}
