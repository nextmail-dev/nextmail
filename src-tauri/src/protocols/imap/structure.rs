use async_imap::imap_proto::types::{
    BodyContentCommon, BodyContentSinglePart, BodyParams, BodyStructure, ContentEncoding,
};
use mail_parser::parsers::preview::preview_text;
use mail_parser::{MessageParser, MimeHeaders};

use crate::core::RemoteAttachment;
use crate::protocols::html_body_text;
use crate::protocols::normalize_attachment_file_name;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TextSectionKind {
    Plain,
    Html,
}

#[derive(Clone, Debug)]
pub(super) struct MessageSectionDescriptor {
    pub section: String,
    pub content_type: String,
    pub content_id: Option<String>,
    pub encoded_size: u64,
    pub text_kind: Option<TextSectionKind>,
}

#[derive(Clone, Debug, Default)]
pub(super) struct MessageStructure {
    pub text_sections: Vec<MessageSectionDescriptor>,
    pub inline_sections: Vec<MessageSectionDescriptor>,
    pub attachments: Vec<RemoteAttachment>,
    pub requires_full_fetch: bool,
}

#[derive(Debug)]
pub(super) struct ParsedTextSection {
    pub text: String,
    pub plain_text: String,
    pub preview: String,
}

pub(super) fn analyze_bodystructure(body: &BodyStructure<'_>) -> MessageStructure {
    let mut structure = MessageStructure::default();
    let mut path = Vec::new();
    let mut attachment_index = 0u32;
    walk_bodystructure(body, &mut path, &mut attachment_index, &mut structure);
    if structure.requires_full_fetch {
        structure.text_sections.clear();
        structure.inline_sections.clear();
        structure.attachments.clear();
    }
    structure
}

fn walk_bodystructure(
    body: &BodyStructure<'_>,
    path: &mut Vec<u32>,
    attachment_index: &mut u32,
    structure: &mut MessageStructure,
) {
    match body {
        BodyStructure::Multipart { common, bodies, .. } => {
            if !path.is_empty() && is_attachment(common) {
                structure.requires_full_fetch = true;
                return;
            }
            for (index, child) in bodies.iter().enumerate() {
                path.push(index as u32 + 1);
                walk_bodystructure(child, path, attachment_index, structure);
                path.pop();
            }
        }
        BodyStructure::Text { common, other, .. } => {
            push_leaf(common, other, path, attachment_index, structure)
        }
        BodyStructure::Basic { common, other, .. }
        | BodyStructure::Message { common, other, .. } => {
            push_leaf(common, other, path, attachment_index, structure)
        }
    }
}

fn push_leaf(
    common: &BodyContentCommon<'_>,
    other: &BodyContentSinglePart<'_>,
    path: &[u32],
    attachment_index: &mut u32,
    structure: &mut MessageStructure,
) {
    let section = if path.is_empty() {
        "1".to_owned()
    } else {
        path.iter()
            .map(u32::to_string)
            .collect::<Vec<_>>()
            .join(".")
    };
    let content_type = format!("{}/{}", common.ty.ty, common.ty.subtype).to_ascii_lowercase();
    let content_id = other
        .id
        .as_deref()
        .map(|value| value.trim().trim_matches(['<', '>']).to_ascii_lowercase())
        .filter(|value| !value.is_empty());
    let filename = attachment_filename(common);
    let disposition_attachment = common
        .disposition
        .as_ref()
        .is_some_and(|value| value.ty.eq_ignore_ascii_case("attachment"));
    let text_kind = if filename.is_none() && !disposition_attachment {
        if content_type == "text/plain" {
            Some(TextSectionKind::Plain)
        } else if content_type == "text/html" {
            Some(TextSectionKind::Html)
        } else {
            None
        }
    } else {
        None
    };
    let descriptor = MessageSectionDescriptor {
        section: section.clone(),
        content_type: content_type.clone(),
        content_id: content_id.clone(),
        encoded_size: u64::from(other.octets),
        text_kind,
    };
    if let Some(kind) = text_kind {
        if !structure
            .text_sections
            .iter()
            .any(|candidate| candidate.text_kind == Some(kind))
        {
            structure.text_sections.push(descriptor);
        }
        return;
    }

    // Some providers label CID-backed images as application/octet-stream.
    // Keep declared and plausibly mislabeled images available to the HTML
    // reference pass; byte-signature and size checks still decide what may be
    // inlined.
    if content_id.is_some()
        && (content_type.starts_with("image/") || content_type == "application/octet-stream")
    {
        structure.inline_sections.push(descriptor);
    }
    structure.attachments.push(RemoteAttachment {
        part_index: *attachment_index,
        imap_section: Some(section),
        file_name: normalize_attachment_file_name(filename.as_deref(), "attachment"),
        content_type,
        size: estimated_attachment_size(other),
        content_id,
    });
    *attachment_index += 1;
}

fn estimated_attachment_size(part: &BodyContentSinglePart<'_>) -> u64 {
    let encoded_size = u64::from(part.octets);
    if matches!(&part.transfer_encoding, ContentEncoding::Base64) {
        (encoded_size * 19 + 13) / 26
    } else {
        encoded_size
    }
}

fn is_attachment(common: &BodyContentCommon<'_>) -> bool {
    attachment_filename(common).is_some()
        || common
            .disposition
            .as_ref()
            .is_some_and(|value| value.ty.eq_ignore_ascii_case("attachment"))
}

fn attachment_filename(common: &BodyContentCommon<'_>) -> Option<String> {
    common
        .disposition
        .as_ref()
        .and_then(|value| body_param(&value.params, "filename"))
        .or_else(|| body_param(&common.ty.params, "name"))
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn body_param(params: &BodyParams<'_>, name: &str) -> Option<String> {
    let params = params.as_ref()?;
    let matches = params
        .iter()
        .filter(|(key, _)| {
            key.eq_ignore_ascii_case(name)
                || key
                    .get(..name.len())
                    .is_some_and(|prefix| prefix.eq_ignore_ascii_case(name))
                    && key.as_bytes().get(name.len()) == Some(&b'*')
        })
        .collect::<Vec<_>>();
    if matches.is_empty() {
        return None;
    }

    // Reuse mail-parser's RFC 2047/2231 implementation. BODYSTRUCTURE has
    // already split the parameter pairs, so rebuild only this local MIME field
    // and let the same parser used for full messages join and decode them.
    let mut header = if name.eq_ignore_ascii_case("filename") {
        "Content-Disposition: attachment".to_owned()
    } else {
        "Content-Type: application/octet-stream".to_owned()
    };
    for (key, value) in matches {
        let escaped = value
            .replace(['\r', '\n'], "")
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        header.push_str(&format!("; {key}=\"{escaped}\""));
    }
    header.push_str("\r\n\r\n");
    MessageParser::default()
        .parse_headers(header.as_bytes())?
        .attachment_name()
        .map(str::to_owned)
}

pub(super) fn canonical_section_path(section: &str) -> Option<Vec<u32>> {
    let path = section
        .split('.')
        .map(str::parse::<u32>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    if path.is_empty() || path.contains(&0) {
        return None;
    }
    let canonical = path
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(".");
    (canonical == section).then_some(path)
}

pub(super) fn parse_text_section(
    kind: TextSectionKind,
    mime: &[u8],
    body: &[u8],
) -> Option<ParsedTextSection> {
    let raw = standalone_part(mime, body);
    let message = MessageParser::default().parse(&raw)?;
    let text = match kind {
        TextSectionKind::Plain => message.body_text(0)?.into_owned(),
        TextSectionKind::Html => message.body_html(0)?.into_owned(),
    };
    let plain_text = match kind {
        TextSectionKind::Plain => text.clone(),
        TextSectionKind::Html => html_body_text(&text),
    };
    Some(ParsedTextSection {
        preview: preview_text(plain_text.as_str().into(), 180).into_owned(),
        text,
        plain_text,
    })
}

pub(super) fn parse_binary_section(mime: &[u8], body: &[u8]) -> Option<Vec<u8>> {
    let raw = standalone_part(mime, body);
    let message = MessageParser::default().parse(&raw)?;
    message.parts.first().map(|part| part.contents().to_vec())
}

fn standalone_part(mime: &[u8], body: &[u8]) -> Vec<u8> {
    let mut raw = Vec::with_capacity(mime.len() + body.len() + 4);
    raw.extend_from_slice(mime);
    if !(mime.ends_with(b"\r\n\r\n") || mime.ends_with(b"\n\n")) {
        if !mime.ends_with(b"\n") {
            raw.extend_from_slice(b"\r\n");
        }
        raw.extend_from_slice(b"\r\n");
    }
    raw.extend_from_slice(body);
    raw
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_imap::imap_proto::types::{ContentEncoding, ContentType};
    use std::borrow::Cow;

    fn common(content_type: (&str, &str), name: Option<&str>) -> BodyContentCommon<'static> {
        BodyContentCommon {
            ty: ContentType {
                ty: Cow::Owned(content_type.0.to_owned()),
                subtype: Cow::Owned(content_type.1.to_owned()),
                params: name
                    .map(|value| vec![(Cow::Borrowed("NAME"), Cow::Owned(value.to_owned()))]),
            },
            disposition: None,
            language: None,
            location: None,
        }
    }

    fn single() -> BodyContentSinglePart<'static> {
        BodyContentSinglePart {
            id: None,
            md5: None,
            description: None,
            transfer_encoding: ContentEncoding::SevenBit,
            octets: 128,
        }
    }

    #[test]
    fn maps_text_and_attachment_sections_without_confusing_ordinals() {
        let body = BodyStructure::Multipart {
            common: common(("MULTIPART", "MIXED"), None),
            bodies: vec![
                BodyStructure::Text {
                    common: common(("TEXT", "PLAIN"), None),
                    other: single(),
                    lines: 1,
                    extension: None,
                },
                BodyStructure::Basic {
                    common: common(("APPLICATION", "PDF"), Some("report.pdf")),
                    other: single(),
                    extension: None,
                },
            ],
            extension: None,
        };

        let structure = analyze_bodystructure(&body);

        assert_eq!(structure.text_sections[0].section, "1");
        assert_eq!(structure.attachments[0].part_index, 0);
        assert_eq!(structure.attachments[0].imap_section.as_deref(), Some("2"));
        assert_eq!(structure.attachments[0].file_name, "report.pdf");
    }

    #[test]
    fn estimates_base64_attachment_size_from_mime_octets() {
        let mut attachment = single();
        attachment.transfer_encoding = ContentEncoding::Base64;
        attachment.octets = 13_684;
        let body = BodyStructure::Basic {
            common: common(("APPLICATION", "PDF"), Some("report.pdf")),
            other: attachment,
            extension: None,
        };

        let structure = analyze_bodystructure(&body);

        assert_eq!(structure.attachments[0].size, 10_000);
    }

    #[test]
    fn keeps_mislabeled_content_id_parts_available_to_the_html_reference_pass() {
        let mut inline_part = single();
        inline_part.id = Some(Cow::Borrowed("<logo@example.test>"));
        let body = BodyStructure::Multipart {
            common: common(("MULTIPART", "RELATED"), None),
            bodies: vec![
                BodyStructure::Text {
                    common: common(("TEXT", "HTML"), None),
                    other: single(),
                    lines: 1,
                    extension: None,
                },
                BodyStructure::Basic {
                    common: common(("APPLICATION", "OCTET-STREAM"), Some("logo.png")),
                    other: inline_part,
                    extension: None,
                },
            ],
            extension: None,
        };

        let structure = analyze_bodystructure(&body);

        assert_eq!(structure.inline_sections.len(), 1);
        assert_eq!(structure.inline_sections[0].section, "2");
        assert_eq!(
            structure.inline_sections[0].content_id.as_deref(),
            Some("logo@example.test")
        );
        assert_eq!(structure.attachments.len(), 1);
    }

    #[test]
    fn decodes_rfc2231_filename_continuations_from_bodystructure() {
        let params = Some(vec![
            (
                Cow::Borrowed("filename*0*"),
                Cow::Borrowed("utf-8''%E6%B5%99%E6%B1%9F%E5%B9%BF%E7%94%B5%E6%97%A0%E7%BA%BF"),
            ),
            (
                Cow::Borrowed("filename*1*"),
                Cow::Borrowed("%E4%BF%A1%E5%8F%B7%E6%B5%8B%E8%AF%95%E8%AF%B4%E6%98%8E.docx"),
            ),
        ]);

        assert_eq!(
            body_param(&params, "filename").as_deref(),
            Some("浙江广电无线信号测试说明.docx")
        );

        let encoded_word = Some(vec![(
            Cow::Borrowed("filename"),
            Cow::Borrowed("=?utf-8?B?5rWZ5rGf5bm/55S15peg57q/5L+h5Y+35rWL6K+V6K+05piOLmRvY3g=?="),
        )]);
        assert_eq!(
            body_param(&encoded_word, "filename").as_deref(),
            Some("浙江广电无线信号测试说明.docx")
        );
    }

    #[test]
    fn validates_section_paths_before_building_imap_queries() {
        assert_eq!(canonical_section_path("2.1"), Some(vec![2, 1]));
        assert_eq!(canonical_section_path("2.01"), None);
        assert_eq!(canonical_section_path("2.MIME"), None);
        assert_eq!(canonical_section_path("1] BODY.PEEK[]"), None);
    }

    #[test]
    fn discards_partial_sections_when_a_multipart_attachment_requires_full_fetch() {
        let body = BodyStructure::Multipart {
            common: common(("MULTIPART", "MIXED"), None),
            bodies: vec![
                BodyStructure::Multipart {
                    common: common(("MULTIPART", "MIXED"), Some("bundle.mime")),
                    bodies: Vec::new(),
                    extension: None,
                },
                BodyStructure::Basic {
                    common: common(("APPLICATION", "PDF"), Some("report.pdf")),
                    other: single(),
                    extension: None,
                },
            ],
            extension: None,
        };

        let structure = analyze_bodystructure(&body);

        assert!(structure.requires_full_fetch);
        assert!(structure.text_sections.is_empty());
        assert!(structure.attachments.is_empty());
    }

    #[test]
    fn parses_transfer_encoded_standalone_parts() {
        let parsed = parse_binary_section(
            b"Content-Type: text/plain\r\nContent-Transfer-Encoding: base64\r\n\r\n",
            b"aGVsbG8=",
        )
        .unwrap();
        assert_eq!(parsed, b"hello");
    }

    #[test]
    fn extracts_html_only_text_and_preview_from_body() {
        let parsed = parse_text_section(
            TextSectionKind::Html,
            b"Content-Type: text/html; charset=utf-8\r\n\r\n",
            b"<!doctype html><html><head><title>Hidden title</title></head><body><p>Visible body</p></body></html>",
        )
        .unwrap();

        assert_eq!(parsed.plain_text, "Visible body\n");
        assert_eq!(parsed.preview, "Visible body\n");
    }
}
