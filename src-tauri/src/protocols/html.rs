use std::borrow::Cow;

use ammonia::Builder;
use mail_parser::MessageParser;

use super::{
    css::{sanitize_style_attribute, sanitize_stylesheet},
    validate_mail_link_target,
};

const MAX_STYLE_ELEMENTS: usize = 32;
const MAX_TOTAL_STYLESHEET_BYTES: usize = 256 * 1024;
const MAX_TOTAL_STYLE_RULES: usize = 2_048;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SanitizedHtml {
    pub document: String,
    pub remote_images_blocked: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SanitizedMessageBody {
    pub plain_text: Option<String>,
    pub safe_html: Option<String>,
    pub remote_images_blocked: bool,
}

pub fn sanitize_raw_message_body(raw: &[u8]) -> Option<SanitizedMessageBody> {
    let message = MessageParser::default().parse(raw)?;
    let plain_text = message.body_text(0).map(|value| value.into_owned());
    let sanitized_html = message.body_html(0).map(|value| sanitize_mail_html(&value));
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

pub fn sanitize_mail_html(input: &str) -> SanitizedHtml {
    let mut builder = Builder::default();
    builder
        .add_clean_content_tags(["script", "form", "iframe", "object", "svg", "math"])
        .rm_clean_content_tags(["style"])
        .add_tags(["style", "font", "tfoot"])
        .add_tag_attributes("div", ["align", "data-nextmail-body"])
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
        .attribute_filter(|element, attribute, value| {
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
                return if value_lower.starts_with("//") {
                    Some(Cow::Owned(format!("https:{trimmed}")))
                } else if value_lower.starts_with("http://")
                    || value_lower.starts_with("https://")
                    || value_lower.starts_with("data:image/")
                {
                    Some(Cow::Borrowed(value))
                } else {
                    None
                };
            }
            Some(Cow::Borrowed(value))
        });

    let input = preserve_body_container(input);
    let fragment = sanitize_style_elements(&builder.clean(input.as_ref()).to_string());
    let normalized = fragment.to_ascii_lowercase();
    let remote_images_blocked = normalized.contains("<img")
        && (normalized.contains("src=\"http://") || normalized.contains("src=\"https://"));
    SanitizedHtml {
        document: format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data:; style-src 'unsafe-inline'\"><style>html{{color-scheme:light}}body{{margin:0}}</style></head><body>{fragment}</body></html>"
        ),
        remote_images_blocked,
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

fn sanitize_style_elements(fragment: &str) -> String {
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
            let stylesheet = sanitize_stylesheet(&fragment[content_start..end]);
            let stylesheet_rules = stylesheet.bytes().filter(|value| *value == b'{').count();
            if !stylesheet.is_empty()
                && total_stylesheet_bytes + stylesheet.len() <= MAX_TOTAL_STYLESHEET_BYTES
                && total_style_rules + stylesheet_rules <= MAX_TOTAL_STYLE_RULES
            {
                output.push_str("<style>");
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

    const RENDERING_CASES: [(&str, &str); 7] = [
        (
            "plain-unstyled.html",
            include_str!("../../../testdata/mail-rendering/plain-unstyled.html"),
        ),
        (
            "transactional-table.html",
            include_str!("../../../testdata/mail-rendering/transactional-table.html"),
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
}
