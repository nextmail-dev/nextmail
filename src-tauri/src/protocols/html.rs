use std::{borrow::Cow, collections::HashSet};

use ammonia::Builder;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SanitizedHtml {
    pub document: String,
    pub remote_images_blocked: bool,
}

pub fn sanitize_mail_html(input: &str) -> SanitizedHtml {
    let mut builder = Builder::default();
    builder
        .add_clean_content_tags(["script", "style", "form", "iframe", "object", "svg", "math"])
        .rm_tags([
            "form", "iframe", "object", "embed", "svg", "math", "meta", "link",
        ])
        .strip_comments(true)
        .add_generic_attributes(["style"])
        .filter_style_properties(safe_style_properties())
        .attribute_filter(|element, attribute, value| {
            let attribute_lower = attribute.to_ascii_lowercase();
            if attribute_lower.starts_with("on")
                || matches!(attribute_lower.as_str(), "srcset" | "formaction" | "action")
            {
                return None;
            }
            if element == "a" && attribute_lower == "href" {
                return None;
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

    let fragment = builder.clean(input).to_string();
    let normalized = fragment.to_ascii_lowercase();
    let remote_images_blocked = normalized.contains("<img")
        && (normalized.contains("src=\"http://") || normalized.contains("src=\"https://"));
    SanitizedHtml {
        document: format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data:; style-src 'unsafe-inline'\"><style>html{{color-scheme:light}}body{{margin:0;padding:16px;font:14px/1.55 system-ui,sans-serif;overflow-wrap:anywhere}}img{{max-width:100%}}table{{max-width:100%}}a{{color:#2563eb}}</style></head><body>{fragment}</body></html>"
        ),
        remote_images_blocked,
    }
}

fn safe_style_properties() -> HashSet<&'static str> {
    [
        "align-content",
        "align-items",
        "align-self",
        "background-color",
        "border",
        "border-bottom",
        "border-bottom-color",
        "border-bottom-left-radius",
        "border-bottom-right-radius",
        "border-bottom-style",
        "border-bottom-width",
        "border-collapse",
        "border-color",
        "border-left",
        "border-left-color",
        "border-left-style",
        "border-left-width",
        "border-radius",
        "border-right",
        "border-right-color",
        "border-right-style",
        "border-right-width",
        "border-spacing",
        "border-style",
        "border-top",
        "border-top-color",
        "border-top-left-radius",
        "border-top-right-radius",
        "border-top-style",
        "border-top-width",
        "border-width",
        "box-sizing",
        "clear",
        "color",
        "direction",
        "display",
        "flex",
        "flex-basis",
        "flex-direction",
        "flex-grow",
        "flex-shrink",
        "flex-wrap",
        "float",
        "font",
        "font-family",
        "font-size",
        "font-stretch",
        "font-style",
        "font-variant",
        "font-weight",
        "gap",
        "height",
        "justify-content",
        "letter-spacing",
        "line-height",
        "list-style-position",
        "list-style-type",
        "margin",
        "margin-bottom",
        "margin-left",
        "margin-right",
        "margin-top",
        "max-height",
        "max-width",
        "min-height",
        "min-width",
        "opacity",
        "overflow",
        "overflow-wrap",
        "overflow-x",
        "overflow-y",
        "padding",
        "padding-bottom",
        "padding-left",
        "padding-right",
        "padding-top",
        "table-layout",
        "text-align",
        "text-decoration",
        "text-indent",
        "text-transform",
        "unicode-bidi",
        "vertical-align",
        "white-space",
        "width",
        "word-break",
        "word-spacing",
    ]
    .into_iter()
    .collect()
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
                "background-image",
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
}
