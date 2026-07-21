mod compose;
mod css;
mod html;
mod imap;
mod tls;

pub use compose::*;
pub use html::*;
pub use imap::*;
pub(crate) use tls::native_tls_connector;

use crate::core::{CommandError, CommandResult};

pub fn extract_attachment(raw: &[u8], part_index: u32) -> CommandResult<Vec<u8>> {
    let message = mail_parser::MessageParser::default()
        .parse(raw)
        .ok_or_else(|| CommandError::new("message.mime_parse_failed"))?;
    message
        .attachment(part_index)
        .map(|attachment| attachment.contents().to_vec())
        .ok_or_else(|| CommandError::new("attachment.not_found"))
}
