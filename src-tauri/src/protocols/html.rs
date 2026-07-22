use std::{
    borrow::Cow,
    collections::{HashMap, HashSet},
    sync::Arc,
};

use ammonia::Builder;
use base64::{engine::general_purpose::STANDARD, Engine as _};
use mail_parser::{Message, MessageParser, MimeHeaders};

use super::{
    css::{
        sanitize_style_attribute, sanitize_stylesheet, sanitize_stylesheet_for_composer,
        sanitize_stylesheet_for_scope,
    },
    validate_mail_link_target,
};

const MAX_STYLE_ELEMENTS: usize = 32;
const MAX_TOTAL_STYLESHEET_BYTES: usize = 256 * 1024;
const MAX_TOTAL_STYLE_RULES: usize = 2_048;
const MAX_INLINE_READER_IMAGE_BYTES: usize = 25 * 1024 * 1024;
const MAX_TOTAL_INLINE_READER_IMAGE_BYTES: usize = 100 * 1024 * 1024;
const PASTED_HTML_SCOPE: &str = "[data-nextmail-pasted-html]";

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SanitizedHtml {
    pub document: String,
    pub remote_images_blocked: bool,
    pub inline_content_ids: HashSet<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SanitizedMessageBody {
    pub plain_text: Option<String>,
    pub safe_html: Option<String>,
    pub remote_images_blocked: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SanitizedComposerBody {
    pub plain_text: Option<String>,
    pub safe_html: Option<String>,
    pub inline_images: Vec<ComposerInlineImage>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ComposerInlineImage {
    pub content_id: String,
    pub file_name: String,
    pub content_type: String,
    pub bytes: Vec<u8>,
}

pub fn sanitize_raw_message_body(raw: &[u8]) -> Option<SanitizedMessageBody> {
    let message = MessageParser::default().parse(raw)?;
    let plain_text = message.body_text(0).map(|value| value.into_owned());
    let sanitized_html = message
        .body_html(0)
        .map(|value| sanitize_mail_html_with_cid_images(&value, &message));
    if plain_text.is_none() && sanitized_html.is_none() {
        return None;
    }
    let (safe_html, remote_images_blocked) = sanitized_html
        .map(|sanitized| (Some(sanitized.document), sanitized.remote_images_blocked))
        .unwrap_or_default();
    Some(SanitizedMessageBody {
        plain_text,
        safe_html,
        remote_images_blocked,
    })
}

pub fn sanitize_raw_message_for_composer(raw: &[u8]) -> Option<SanitizedComposerBody> {
    let message = MessageParser::default().parse(raw)?;
    let plain_text = message.body_text(0).map(|value| value.into_owned());
    let safe_html = message
        .body_html(0)
        .map(|value| sanitize_mail_html_for_composer(&value));
    if plain_text.is_none() && safe_html.is_none() {
        return None;
    }
    let normalized_html = safe_html
        .as_deref()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let inline_images = message
        .attachments()
        .filter_map(|attachment| {
            let content_id = attachment
                .content_id()?
                .trim()
                .trim_matches(['<', '>'])
                .to_owned();
            let content_type = attachment.content_type()?;
            let content_type = format!(
                "{}/{}",
                content_type.ctype(),
                content_type.subtype().unwrap_or("octet-stream")
            );
            if !matches!(
                content_type.to_ascii_lowercase().as_str(),
                "image/gif" | "image/jpeg" | "image/png" | "image/webp"
            ) || !normalized_html.contains(&format!("cid:{}", content_id.to_ascii_lowercase()))
            {
                return None;
            }
            Some(ComposerInlineImage {
                file_name: attachment
                    .attachment_name()
                    .filter(|value| !value.trim().is_empty())
                    .unwrap_or("inline-image")
                    .to_owned(),
                content_id,
                content_type,
                bytes: attachment.contents().to_vec(),
            })
        })
        .collect();
    Some(SanitizedComposerBody {
        plain_text,
        safe_html,
        inline_images,
    })
}

pub fn sanitize_mail_html(input: &str) -> SanitizedHtml {
    sanitize_mail_html_with_data_urls(input, HashMap::new())
}

pub fn sanitize_mail_html_with_cid_images(input: &str, message: &Message<'_>) -> SanitizedHtml {
    sanitize_mail_html_with_data_urls(input, inline_image_data_urls(message, input))
}

fn sanitize_mail_html_with_data_urls(
    input: &str,
    cid_images: HashMap<String, String>,
) -> SanitizedHtml {
    let cid_images = Arc::new(cid_images);
    let fragment = sanitize_mail_html_fragment_with_cid_images(
        input,
        true,
        false,
        false,
        Arc::clone(&cid_images),
    );
    let normalized = fragment.to_ascii_lowercase();
    let remote_images_blocked = normalized.contains("<img")
        && (normalized.contains("src=\"http://") || normalized.contains("src=\"https://"));
    SanitizedHtml {
        document: format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data:; style-src 'unsafe-inline'\"><style>html{{color-scheme:light}}body{{margin:0}}</style></head><body>{fragment}</body></html>"
        ),
        remote_images_blocked,
        inline_content_ids: cid_images
            .iter()
            .filter_map(|(content_id, data_url)| {
                fragment.contains(data_url).then_some(content_id.clone())
            })
            .collect(),
    }
}

fn inline_image_data_urls(message: &Message<'_>, input: &str) -> HashMap<String, String> {
    let normalized_html = input.to_ascii_lowercase();
    let mut total_bytes = 0usize;
    let mut images = HashMap::new();
    for attachment in message.attachments() {
        let Some(content_id) = attachment.content_id() else {
            continue;
        };
        let content_id = content_id.trim().trim_matches(['<', '>']).to_owned();
        let size = attachment.len();
        if content_id.is_empty()
            || size > MAX_INLINE_READER_IMAGE_BYTES
            || total_bytes.saturating_add(size) > MAX_TOTAL_INLINE_READER_IMAGE_BYTES
            || !normalized_html.contains(&format!("cid:{}", content_id.to_ascii_lowercase()))
        {
            continue;
        }
        let Some(content_type) = attachment.content_type() else {
            continue;
        };
        let content_type = format!(
            "{}/{}",
            content_type.ctype(),
            content_type.subtype().unwrap_or("octet-stream")
        )
        .to_ascii_lowercase();
        if !matches!(
            content_type.as_str(),
            "image/gif" | "image/jpeg" | "image/png" | "image/webp"
        ) {
            continue;
        }
        total_bytes += size;
        images.insert(
            content_id.to_ascii_lowercase(),
            format!(
                "data:{content_type};base64,{}",
                STANDARD.encode(attachment.contents())
            ),
        );
    }
    images
}

pub fn sanitize_mail_html_for_composer(input: &str) -> String {
    sanitize_mail_html_fragment(input, true, true, true)
}

pub fn sanitize_composer_document(input: &str) -> String {
    sanitize_mail_html_fragment(input, true, false, true)
}

pub fn sanitize_rich_text_paste(input: &str) -> String {
    let fragment = sanitize_mail_html_fragment_with_scope(
        input,
        true,
        Some(PASTED_HTML_SCOPE),
        true,
        Arc::new(HashMap::new()),
    );
    format!("<div data-nextmail-pasted-html=\"\">{fragment}</div>")
}

fn sanitize_mail_html_fragment(
    input: &str,
    preserve_stylesheets: bool,
    scope_stylesheets: bool,
    preserve_cid_images: bool,
) -> String {
    sanitize_mail_html_fragment_with_scope(
        input,
        preserve_stylesheets,
        scope_stylesheets.then_some("[data-nextmail-original-message]"),
        preserve_cid_images,
        Arc::new(HashMap::new()),
    )
}

fn sanitize_mail_html_fragment_with_cid_images(
    input: &str,
    preserve_stylesheets: bool,
    scope_stylesheets: bool,
    preserve_cid_images: bool,
    cid_images: Arc<HashMap<String, String>>,
) -> String {
    sanitize_mail_html_fragment_with_scope(
        input,
        preserve_stylesheets,
        scope_stylesheets.then_some("[data-nextmail-original-message]"),
        preserve_cid_images,
        cid_images,
    )
}

fn sanitize_mail_html_fragment_with_scope(
    input: &str,
    preserve_stylesheets: bool,
    stylesheet_scope: Option<&'static str>,
    preserve_cid_images: bool,
    cid_images: Arc<HashMap<String, String>>,
) -> String {
    let mut builder = Builder::default();
    if preserve_cid_images || !cid_images.is_empty() {
        builder.add_url_schemes(["cid", "data"]);
    }
    builder
        .add_clean_content_tags(["script", "form", "iframe", "object", "svg", "math"])
        .add_tags(["font", "tfoot"])
        .add_tag_attributes(
            "div",
            [
                "align",
                "data-nextmail-body",
                "data-nextmail-original-message",
                "data-nextmail-pasted-html",
                "data-nextmail-reply",
                "data-nextmail-signature-id",
                "data-nextmail-template-id",
            ],
        )
        .add_tag_attributes("p", ["align"])
        .add_tag_attributes(
            "table",
            [
                "border",
                "cellpadding",
                "cellspacing",
                "bgcolor",
                "height",
                "role",
                "valign",
                "width",
            ],
        )
        .add_tag_attributes("tbody", ["bgcolor", "height", "valign", "width"])
        .add_tag_attributes("thead", ["bgcolor", "height", "valign", "width"])
        .add_tag_attributes("tfoot", ["align", "bgcolor", "height", "valign", "width"])
        .add_tag_attributes("tr", ["bgcolor", "height", "valign", "width"])
        .add_tag_attributes("td", ["bgcolor", "height", "nowrap", "valign", "width"])
        .add_tag_attributes("th", ["bgcolor", "height", "nowrap", "valign", "width"])
        .add_tag_attributes("col", ["valign", "width"])
        .add_tag_attributes("colgroup", ["valign", "width"])
        .add_tag_attributes("img", ["border", "hspace", "vspace"])
        .add_tag_attributes("font", ["color", "face", "size"])
        .rm_tags([
            "form", "iframe", "object", "embed", "svg", "math", "meta", "link",
        ])
        .strip_comments(true)
        .add_generic_attributes(["class", "dir", "id", "role", "style"])
        .add_generic_attribute_prefixes(["aria-"])
        .set_tag_attribute_value("a", "target", "_blank")
        .attribute_filter(move |element, attribute, value| {
            let attribute_lower = attribute.to_ascii_lowercase();
            if attribute_lower.starts_with("on")
                || matches!(attribute_lower.as_str(), "srcset" | "formaction" | "action")
            {
                return None;
            }
            if attribute_lower == "style" {
                let sanitized = sanitize_style_attribute(value);
                return (!sanitized.is_empty()).then_some(Cow::Owned(sanitized));
            }
            if element == "a" && attribute_lower == "href" {
                let validated = validate_mail_link_target(value)?;
                return Some(Cow::Owned(validated.target));
            }
            if element == "a" && attribute_lower == "target" {
                return Some(Cow::Borrowed("_blank"));
            }
            if element == "img" && attribute_lower == "src" {
                let trimmed = value.trim();
                let value_lower = trimmed.to_ascii_lowercase();
                if let Some(content_id) = value_lower.strip_prefix("cid:") {
                    if let Some(data_url) = cid_images.get(content_id.trim_matches(['<', '>'])) {
                        return Some(Cow::Owned(data_url.clone()));
                    }
                }
                return if value_lower.starts_with("//") {
                    Some(Cow::Owned(format!("https:{trimmed}")))
                } else if value_lower.starts_with("http://")
                    || value_lower.starts_with("https://")
                    || value_lower.starts_with("data:image/")
                    || (preserve_cid_images && value_lower.starts_with("cid:"))
                {
                    Some(Cow::Borrowed(value))
                } else {
                    None
                };
            }
            Some(Cow::Borrowed(value))
        });

    if preserve_stylesheets {
        builder.rm_clean_content_tags(["style"]).add_tags(["style"]);
    } else {
        builder.add_clean_content_tags(["style"]);
    }

    let input = preserve_body_container(input);
    let fragment = builder.clean(input.as_ref()).to_string();
    if preserve_stylesheets {
        sanitize_style_elements(&fragment, stylesheet_scope)
    } else {
        fragment
    }
}

fn preserve_body_container(input: &str) -> Cow<'_, str> {
    let bytes = input.as_bytes();
    let mut output = String::new();
    let mut cursor = 0usize;
    let mut index = 0usize;

    while index < bytes.len() {
        let replacement = if starts_with_ascii_case_insensitive(bytes, index, b"<body")
            && is_html_tag_boundary(bytes.get(index + 5).copied())
        {
            Some((5, "<div data-nextmail-body=\"\""))
        } else if starts_with_ascii_case_insensitive(bytes, index, b"</body")
            && is_html_tag_boundary(bytes.get(index + 6).copied())
        {
            Some((6, "</div"))
        } else {
            None
        };

        if let Some((matched, value)) = replacement {
            output.push_str(&input[cursor..index]);
            output.push_str(value);
            index += matched;
            cursor = index;
        } else {
            index += 1;
        }
    }

    if output.is_empty() {
        Cow::Borrowed(input)
    } else {
        output.push_str(&input[cursor..]);
        Cow::Owned(output)
    }
}

fn starts_with_ascii_case_insensitive(input: &[u8], start: usize, expected: &[u8]) -> bool {
    input
        .get(start..start + expected.len())
        .is_some_and(|candidate| candidate.eq_ignore_ascii_case(expected))
}

fn is_html_tag_boundary(value: Option<u8>) -> bool {
    value.is_none_or(|value| value.is_ascii_whitespace() || matches!(value, b'>' | b'/'))
}

fn sanitize_style_elements(fragment: &str, stylesheet_scope: Option<&'static str>) -> String {
    let mut output = String::with_capacity(fragment.len());
    let mut cursor = 0usize;
    let mut style_count = 0usize;
    let mut total_stylesheet_bytes = 0usize;
    let mut total_style_rules = 0usize;

    while let Some(relative_start) = fragment[cursor..].find("<style>") {
        let start = cursor + relative_start;
        output.push_str(&fragment[cursor..start]);
        let content_start = start + "<style>".len();
        let Some(relative_end) = fragment[content_start..].find("</style>") else {
            return output;
        };
        let end = content_start + relative_end;

        if style_count < MAX_STYLE_ELEMENTS {
            let stylesheet = if let Some(scope) = stylesheet_scope {
                if scope == "[data-nextmail-original-message]" {
                    sanitize_stylesheet_for_composer(&fragment[content_start..end])
                } else {
                    sanitize_stylesheet_for_scope(&fragment[content_start..end], scope)
                }
            } else {
                sanitize_stylesheet(&fragment[content_start..end])
            };
            let stylesheet_rules = stylesheet.bytes().filter(|value| *value == b'{').count();
            if !stylesheet.is_empty()
                && total_stylesheet_bytes + stylesheet.len() <= MAX_TOTAL_STYLESHEET_BYTES
                && total_style_rules + stylesheet_rules <= MAX_TOTAL_STYLE_RULES
            {
                if stylesheet_scope.is_some() {
                    output.push_str("<style data-nextmail-compose-style=\"\">");
                } else {
                    output.push_str("<style>");
                }
                output.push_str(&stylesheet);
                output.push_str("</style>");
                total_stylesheet_bytes += stylesheet.len();
                total_style_rules += stylesheet_rules;
            }
        }

        style_count += 1;
        cursor = end + "</style>".len();
    }

    output.push_str(&fragment[cursor..]);
    output
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use serde::Deserialize;

    use super::*;

    const RENDERING_CASES: [(&str, &str); 8] = [
        (
            "plain-unstyled.html",
            include_str!("../../../testdata/mail-rendering/plain-unstyled.html"),
        ),
        (
            "transactional-table.html",
            include_str!("../../../testdata/mail-rendering/transactional-table.html"),
        ),
        (
            "flex-invoice-table.html",
            include_str!("../../../testdata/mail-rendering/flex-invoice-table.html"),
        ),
        (
            "marketing-responsive.html",
            include_str!("../../../testdata/mail-rendering/marketing-responsive.html"),
        ),
        (
            "native-dark.html",
            include_str!("../../../testdata/mail-rendering/native-dark.html"),
        ),
        (
            "mixed-background-table.html",
            include_str!("../../../testdata/mail-rendering/mixed-background-table.html"),
        ),
        (
            "links-and-remote-resources.html",
            include_str!("../../../testdata/mail-rendering/links-and-remote-resources.html"),
        ),
        (
            "malicious-active-content.html",
            include_str!("../../../testdata/mail-rendering/malicious-active-content.html"),
        ),
    ];

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RenderingManifest {
        schema_version: u32,
        cases: Vec<RenderingManifestCase>,
    }

    #[derive(Debug, Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct RenderingManifestCase {
        file: String,
        category: String,
        expected_safe_layout: String,
        contains_remote_resources: bool,
        contains_links: bool,
        dark_mode_case: String,
        active_threats: Vec<String>,
    }

    #[test]
    fn rendering_corpus_manifest_covers_every_shared_fixture() {
        let manifest: RenderingManifest = serde_json::from_str(include_str!(
            "../../../testdata/mail-rendering/manifest.json"
        ))
        .expect("rendering corpus manifest must be valid JSON");
        assert_eq!(manifest.schema_version, 1);

        let fixture_names = RENDERING_CASES
            .iter()
            .map(|(name, _)| *name)
            .collect::<HashSet<_>>();
        let manifest_names = manifest
            .cases
            .iter()
            .map(|case| case.file.as_str())
            .collect::<HashSet<_>>();
        assert_eq!(fixture_names, manifest_names);
        assert_eq!(manifest_names.len(), manifest.cases.len());

        for case in manifest.cases {
            assert!(!case.category.trim().is_empty());
            assert!(!case.expected_safe_layout.trim().is_empty());
            assert!(!case.dark_mode_case.trim().is_empty());
            if case.contains_remote_resources {
                assert!(case.file.contains("marketing") || !case.active_threats.is_empty());
            }
            if case.contains_links {
                assert!(case.file != "plain-unstyled.html");
            }
        }
    }

    #[test]
    fn shared_rendering_corpus_keeps_the_current_active_content_boundary() {
        for (name, source) in RENDERING_CASES {
            let sanitized = sanitize_mail_html(source);
            let normalized = sanitized.document.to_ascii_lowercase();

            assert!(normalized.starts_with("<!doctype html>"), "fixture {name}");
            assert!(
                normalized.contains("default-src 'none'"),
                "fixture {name} must keep the restrictive document CSP"
            );
            for forbidden in [
                "<script",
                "<form",
                "<iframe",
                "<object",
                "<embed",
                "<svg",
                "<math",
                "<link",
                "javascript:",
                "file:///",
                " onload=",
                " onclick=",
                " onerror=",
                "@import",
                "url(",
                "position:fixed",
            ] {
                assert!(
                    !normalized.contains(forbidden),
                    "fixture {name} retained forbidden token {forbidden}"
                );
            }
        }
    }

    #[test]
    fn removes_scripts_events_navigation_and_remote_images() {
        let sanitized = sanitize_mail_html(
            r#"<script>alert(1)</script><a href="javascript:alert(2)">link</a><img src="https://tracker.example/pixel" onerror="alert(3)">"#,
        );
        assert!(!sanitized.document.contains("<script"));
        assert!(!sanitized.document.contains("javascript:"));
        assert!(!sanitized.document.contains("onerror"));
        assert!(sanitized.document.contains("tracker.example"));
        assert!(sanitized.document.contains("img-src data:;"));
        assert!(!sanitized.document.contains("img-src data: http: https:"));
        assert!(sanitized.remote_images_blocked);
    }

    #[test]
    fn preserves_only_valid_external_targets_for_system_opening() {
        let sanitized = sanitize_mail_html(
            r#"<a href="HTTPS://Example.COM:443/account" target="_blank">web</a>
               <a href="//news.example.com/latest">news</a>
               <a href="mailto:reader@example.com?subject=Hello">mail</a>
               <a href="javascript:alert(1)">script</a>
               <a href="file:///C:/secret.txt">file</a>
               <a href="https://user:secret@example.com/">credentials</a>
               <a href="/relative/path">relative</a>"#,
        );

        for expected in [
            "href=\"https://example.com/account\"",
            "href=\"https://news.example.com/latest\"",
            "href=\"mailto:reader@example.com?subject=Hello\"",
        ] {
            assert!(sanitized.document.contains(expected), "missing {expected}");
        }
        assert_eq!(sanitized.document.matches("target=\"_blank\"").count(), 7);
        assert_eq!(
            sanitized
                .document
                .matches("rel=\"noopener noreferrer\"")
                .count(),
            7
        );
        for forbidden in [
            "javascript:",
            "file:///",
            "user:secret",
            "href=\"/relative/path\"",
        ] {
            assert!(!sanitized.document.contains(forbidden));
        }
    }

    #[test]
    fn removes_forms_embedded_documents_and_css_resource_urls() {
        let sanitized = sanitize_mail_html(
            r#"<form action="https://example.com"><input name="secret"></form>
               <iframe src="https://example.com"></iframe>
               <svg><script>alert(1)</script></svg>
               <p style="background-image:url(https://tracker.example/pixel);color:red;padding:12px;width:320px">content</p>"#,
        );
        for forbidden in [
            "<form",
            "<input",
            "<iframe",
            "<svg",
            "url(",
            "tracker.example",
        ] {
            assert!(!sanitized.document.contains(forbidden), "found {forbidden}");
        }
        assert!(sanitized.document.contains("content"));
        assert!(sanitized.document.contains("color:red"));
        assert!(sanitized.document.contains("padding:12px"));
        assert!(sanitized.document.contains("width:320px"));
    }

    #[test]
    fn preserves_safe_inline_email_layout_without_enabling_active_css() {
        let sanitized = sanitize_mail_html(
            r#"<table style="width:100%;border-collapse:separate;background-color:#fff"><tr><td style="font-size:16px;text-align:center;position:fixed;z-index:9999">Hello</td></tr></table><img src="//cdn.example/banner.png" style="width:240px;height:80px">"#,
        );

        for expected in [
            "width:100%",
            "border-collapse:separate",
            "background-color:#fff",
            "font-size:16px",
            "text-align:center",
        ] {
            assert!(sanitized.document.contains(expected), "missing {expected}");
        }
        assert!(!sanitized.document.contains("position:fixed"));
        assert!(!sanitized.document.contains("z-index"));
        assert!(sanitized
            .document
            .contains("src=\"https://cdn.example/banner.png\""));
        assert!(sanitized.remote_images_blocked);
    }

    #[test]
    fn preserves_legacy_email_table_layout_and_css_selector_hooks() {
        let sanitized = sanitize_mail_html(
            r##"<table class="campaign" id="mail-shell" role="presentation" width="600" cellpadding="0" cellspacing="0" border="0" align="center" bgcolor="#ffffff"><tbody><tr valign="top" bgcolor="#eeeeee"><td width="420" height="80" valign="middle" nowrap><font face="Arial" size="3" color="#202124">Content</font></td></tr></tbody></table>"##,
        );

        for expected in [
            "class=\"campaign\"",
            "id=\"mail-shell\"",
            "role=\"presentation\"",
            "width=\"600\"",
            "cellpadding=\"0\"",
            "cellspacing=\"0\"",
            "border=\"0\"",
            "align=\"center\"",
            "bgcolor=\"#ffffff\"",
            "valign=\"middle\"",
            "nowrap=\"\"",
            "face=\"Arial\"",
        ] {
            assert!(sanitized.document.contains(expected), "missing {expected}");
        }
        for unwanted in [
            "padding:16px",
            "font:14px/1.55",
            "overflow-wrap:anywhere",
            "img{max-width:100%}",
            "table{max-width:100%}",
        ] {
            assert!(
                !sanitized.document.contains(unwanted),
                "retained layout override {unwanted}"
            );
        }
    }

    #[test]
    fn preserves_authored_body_styles_in_a_safe_inner_container() {
        let sanitized = sanitize_mail_html(
            r#"<!doctype html><html><body style="background-color:#f4f5f7;color:#202124"><p>Body content</p></body></html>"#,
        );

        assert!(sanitized.document.contains("data-nextmail-body=\"\""));
        assert!(sanitized
            .document
            .contains("style=\"background-color:#f4f5f7;color:#202124\""));
        assert!(sanitized.document.contains("Body content"));
    }

    #[test]
    fn preserves_safe_embedded_email_styles_and_controlled_media_queries() {
        let marketing = sanitize_mail_html(include_str!(
            "../../../testdata/mail-rendering/marketing-responsive.html"
        ));
        for expected in [
            "<style>",
            ".campaign{",
            ".campaign-title{",
            "class=\"campaign\"",
            "class=\"campaign-title\"",
            "@media (max-width:640px)",
            "padding:18px",
        ] {
            assert!(marketing.document.contains(expected), "missing {expected}");
        }
        assert!(marketing.remote_images_blocked);

        let native_dark = sanitize_mail_html(include_str!(
            "../../../testdata/mail-rendering/native-dark.html"
        ));
        assert!(native_dark
            .document
            .contains("@media (prefers-color-scheme:dark)"));
        assert!(native_dark.document.contains("background-color:#252525"));
    }

    #[test]
    fn malicious_embedded_css_cannot_escape_or_request_resources() {
        let sanitized = sanitize_mail_html(include_str!(
            "../../../testdata/mail-rendering/malicious-active-content.html"
        ));
        assert!(sanitized.document.contains("Visible inert fixture text"));
        for forbidden in [
            "@import",
            "@font-face",
            "url(",
            "position:fixed",
            "z-index",
            "</style><script",
            "attacker.example.invalid/mail.css",
            "attacker.example.invalid/beacon.gif",
        ] {
            assert!(
                !sanitized.document.contains(forbidden),
                "retained {forbidden}"
            );
        }
        assert!(sanitized.remote_images_blocked);
    }

    #[test]
    fn rebuilds_a_safe_body_from_local_raw_mime() {
        let raw = concat!(
            "From: sender@example.com\r\n",
            "To: reader@example.com\r\n",
            "Subject: Cached HTML\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: text/html; charset=utf-8\r\n",
            "\r\n",
            "<style>.card { color: #123456; }</style>",
            "<div class=\"card\">Offline body</div>"
        );
        let body = sanitize_raw_message_body(raw.as_bytes()).expect("raw MIME body");
        assert!(body.plain_text.is_some());
        assert!(body
            .safe_html
            .expect("safe HTML")
            .contains(".card{color:#123456}"));
        assert!(!body.remote_images_blocked);
    }

    #[test]
    fn builds_an_inert_high_fidelity_fragment_for_composer_import() {
        let sanitized = sanitize_mail_html_for_composer(
            r##"<style>.campaign { width: 600px; }</style>
                <script>alert(1)</script>
                <table width="600" cellpadding="0" cellspacing="0"><tr>
                  <td style="color:#123456;background-color:#ffffff">
                    <a href="https://example.com/account">Account</a>
                    <img src="https://cdn.example/banner.png" alt="Banner" onerror="alert(2)">
                  </td>
                </tr></table>"##,
        );

        for expected in [
            "<style data-nextmail-compose-style=\"\">",
            "[data-nextmail-original-message] .campaign{width:600px}",
            "width=\"600\"",
            "cellpadding=\"0\"",
            "color:#123456",
            "background-color:#ffffff",
            "href=\"https://example.com/account\"",
            "src=\"https://cdn.example/banner.png\"",
        ] {
            assert!(sanitized.contains(expected), "missing {expected}");
        }
        for forbidden in ["<script", "onerror", "alert(1)", "<style>.campaign"] {
            assert!(!sanitized.contains(forbidden), "retained {forbidden}");
        }
    }

    #[test]
    fn scopes_rich_text_paste_styles_without_losing_safe_formatting() {
        let sanitized = sanitize_rich_text_paste(
            r##"<style>.copied { color:#123456; position:fixed; background:url(https://bad.test/a.png) }</style>
                <script>alert(1)</script>
                <div class="copied" id="copied-block" style="font-size:18px;position:fixed">
                  <span style="font-family:Arial;color:#654321">Copied</span>
                </div>"##,
        );

        for expected in [
            "data-nextmail-pasted-html",
            "[data-nextmail-pasted-html] .copied{color:#123456}",
            "class=\"copied\"",
            "id=\"copied-block\"",
            "font-size:18px",
            "font-family:Arial",
            "color:#654321",
        ] {
            assert!(
                sanitized.contains(expected),
                "missing {expected}: {sanitized}"
            );
        }
        for forbidden in ["<script", "alert(1)", "position:fixed", "bad.test"] {
            assert!(
                !sanitized.contains(forbidden),
                "retained {forbidden}: {sanitized}"
            );
        }
    }

    #[test]
    fn preserves_flex_invoice_column_ratios_for_reading_and_composer_import() {
        let fixture = include_str!("../../../testdata/mail-rendering/flex-invoice-table.html");
        let reading = sanitize_mail_html(fixture);
        for expected in [
            ".invoice-table tr{display:flex;width:100%}",
            ".invoice-table th:nth-child(1)",
            ".invoice-table td:nth-child(4)",
            "flex:2",
            "flex:3",
        ] {
            assert!(
                reading.document.contains(expected),
                "missing {expected}: {}",
                reading.document
            );
        }

        let sanitized = sanitize_mail_html_for_composer(fixture);

        for expected in [
            "[data-nextmail-original-message] .invoice-table tr{display:flex;width:100%}",
            "[data-nextmail-original-message] .invoice-table th:nth-child(1)",
            "[data-nextmail-original-message] .invoice-table td:nth-child(4)",
            "flex:2",
            "flex:3",
        ] {
            assert!(
                sanitized.contains(expected),
                "missing {expected}: {sanitized}"
            );
        }
    }

    #[test]
    fn composer_mime_import_prefers_html_without_returning_a_document_shell() {
        let raw = concat!(
            "From: sender@example.com\r\n",
            "To: reader@example.com\r\n",
            "Subject: Editable HTML\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: text/html; charset=utf-8\r\n",
            "\r\n",
            "<p style=\"font-size:16px\">Editable <strong>body</strong></p>"
        );
        let body = sanitize_raw_message_for_composer(raw.as_bytes()).expect("composer body");
        let html = body.safe_html.expect("safe HTML fragment");
        assert!(html.contains("font-size:16px"));
        assert!(html.contains("<strong>body</strong>"));
        assert!(!html.contains("<!doctype"));
        assert!(!html.contains("Content-Security-Policy"));
    }

    #[test]
    fn composer_mime_import_keeps_referenced_cid_images_with_decoded_bytes() {
        let raw = concat!(
            "From: sender@example.com\r\n",
            "To: reader@example.com\r\n",
            "Subject: Inline image\r\n",
            "MIME-Version: 1.0\r\n",
            "Content-Type: multipart/related; boundary=nextmail\r\n",
            "\r\n",
            "--nextmail\r\n",
            "Content-Type: text/html; charset=utf-8\r\n\r\n",
            "<p>Logo <img src=\"cid:logo@example.test\"></p>\r\n",
            "--nextmail\r\n",
            "Content-Type: image/png; name=logo.png\r\n",
            "Content-Disposition: inline; filename=logo.png\r\n",
            "Content-ID: <logo@example.test>\r\n",
            "Content-Transfer-Encoding: base64\r\n\r\n",
            "aW1hZ2U=\r\n",
            "--nextmail--\r\n"
        );
        let body = sanitize_raw_message_for_composer(raw.as_bytes()).expect("composer body");
        assert!(body
            .safe_html
            .expect("safe HTML")
            .contains("cid:logo@example.test"));
        assert_eq!(body.inline_images.len(), 1);
        assert_eq!(body.inline_images[0].content_id, "logo@example.test");
        assert_eq!(body.inline_images[0].content_type, "image/png");
        assert_eq!(body.inline_images[0].bytes, b"image");
    }
}
