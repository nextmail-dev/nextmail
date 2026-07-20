use async_imap::types::NameAttribute;
use base64::{engine::general_purpose::STANDARD, Engine as _};

use crate::core::MailboxRole;

pub(super) fn mailbox_role(name: &str, attributes: &[NameAttribute<'_>]) -> MailboxRole {
    if name.eq_ignore_ascii_case("INBOX") {
        return MailboxRole::Inbox;
    }
    for attribute in attributes {
        match attribute {
            NameAttribute::Archive => return MailboxRole::Archive,
            NameAttribute::Drafts => return MailboxRole::Drafts,
            NameAttribute::Junk => return MailboxRole::Junk,
            NameAttribute::Sent => return MailboxRole::Sent,
            NameAttribute::Trash => return MailboxRole::Trash,
            _ => {}
        }
    }
    match name.trim().to_ascii_lowercase().as_str() {
        "sent" | "sent items" | "sent messages" => MailboxRole::Sent,
        "draft" | "drafts" => MailboxRole::Drafts,
        "trash" | "deleted" | "deleted items" => MailboxRole::Trash,
        "junk" | "junk e-mail" | "junk email" | "spam" => MailboxRole::Junk,
        "archive" | "archives" => MailboxRole::Archive,
        _ => MailboxRole::Other,
    }
}

pub fn decode_modified_utf7(input: &str) -> String {
    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some(relative_start) = input[cursor..].find('&') {
        let start = cursor + relative_start;
        output.push_str(&input[cursor..start]);
        let Some(relative_end) = input[start + 1..].find('-') else {
            output.push_str(&input[start..]);
            return output;
        };
        let end = start + 1 + relative_end;
        let encoded = &input[start + 1..end];
        if encoded.is_empty() {
            output.push('&');
        } else if let Some(decoded) = decode_modified_utf7_segment(encoded) {
            output.push_str(&decoded);
        } else {
            output.push_str(&input[start..=end]);
        }
        cursor = end + 1;
    }
    output.push_str(&input[cursor..]);
    output
}

fn decode_modified_utf7_segment(encoded: &str) -> Option<String> {
    let mut standard = encoded.replace(',', "/");
    while !standard.len().is_multiple_of(4) {
        standard.push('=');
    }
    let bytes = STANDARD.decode(standard).ok()?;
    if bytes.len() % 2 != 0 {
        return None;
    }
    let utf16 = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
        .collect::<Vec<_>>();
    String::from_utf16(&utf16).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_mailbox_names_and_recognizes_roles() {
        assert_eq!(decode_modified_utf7("INBOX"), "INBOX");
        assert_eq!(decode_modified_utf7("A&-B"), "A&B");
        assert_eq!(decode_modified_utf7("&U,BTFw-"), "台北");
        assert_eq!(decode_modified_utf7("&ZeVnLIqe-"), "日本語");
        assert_eq!(mailbox_role("Drafts", &[]), MailboxRole::Drafts);
        assert_eq!(mailbox_role("Sent Items", &[]), MailboxRole::Sent);
    }
}
