use ammonia::Url;

const MAX_LINK_TARGET_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ValidatedMailLink {
    pub target: String,
}

pub fn validate_mail_link_target(candidate: &str) -> Option<ValidatedMailLink> {
    let trimmed = candidate.trim();
    if trimmed.is_empty()
        || trimmed.len() > MAX_LINK_TARGET_BYTES
        || trimmed.contains('\\')
        || contains_confusing_characters(trimmed)
        || contains_percent_encoded_confusing_characters(trimmed)
    {
        return None;
    }

    let candidate = if trimmed.starts_with("//") {
        format!("https:{trimmed}")
    } else {
        trimmed.to_owned()
    };
    let url = Url::parse(&candidate).ok()?;
    match url.scheme() {
        "http" | "https" => {
            if url.host_str().is_none() || !url.username().is_empty() || url.password().is_some() {
                return None;
            }
        }
        "mailto" => {
            if url.path().trim().is_empty() || url.host_str().is_some() {
                return None;
            }
        }
        _ => return None,
    }

    let target = url.to_string();
    if target.len() > MAX_LINK_TARGET_BYTES || contains_confusing_characters(&target) {
        return None;
    }
    Some(ValidatedMailLink { target })
}

fn contains_confusing_characters(value: &str) -> bool {
    value.chars().any(|character| {
        character.is_control()
            || matches!(
                character,
                '\u{061c}'
                    | '\u{200e}'
                    | '\u{200f}'
                    | '\u{202a}'..='\u{202e}'
                    | '\u{2066}'..='\u{2069}'
            )
    })
}

fn contains_percent_encoded_confusing_characters(value: &str) -> bool {
    let bytes = value.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        if bytes[index] == b'%' && index + 2 < bytes.len() {
            if let (Some(high), Some(low)) =
                (hex_value(bytes[index + 1]), hex_value(bytes[index + 2]))
            {
                decoded.push((high << 4) | low);
                index += 3;
                continue;
            }
        }
        decoded.push(bytes[index]);
        index += 1;
    }
    decoded.iter().any(|byte| byte.is_ascii_control())
        || std::str::from_utf8(&decoded).is_ok_and(contains_confusing_characters)
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_and_normalizes_explicit_supported_links() {
        assert_eq!(
            validate_mail_link_target(" HTTPS://Example.COM:443/path?q=1#part "),
            Some(ValidatedMailLink {
                target: "https://example.com/path?q=1#part".to_owned(),
            })
        );
        assert_eq!(
            validate_mail_link_target("//example.com/news")
                .unwrap()
                .target,
            "https://example.com/news"
        );
        assert_eq!(
            validate_mail_link_target("mailto:reader@example.com?subject=Hello")
                .unwrap()
                .target,
            "mailto:reader@example.com?subject=Hello"
        );
    }

    #[test]
    fn rejects_active_local_credential_and_confusing_targets() {
        for target in [
            "javascript:alert(1)",
            "data:text/html,hello",
            "file:///C:/secret.txt",
            "C:\\secret.txt",
            "/relative/path",
            "https://user:secret@example.com/",
            "https://example.com\\@attacker.invalid/",
            "https://example.com/%0d%0aHeader:value",
            "https://example.com/\u{202e}moc.live",
            "mailto:",
            "mailto:reader@example.com?body=hello%0aworld",
        ] {
            assert!(
                validate_mail_link_target(target).is_none(),
                "accepted unsafe target {target}"
            );
        }
    }
}
