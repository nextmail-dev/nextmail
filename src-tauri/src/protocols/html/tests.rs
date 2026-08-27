use std::collections::HashSet;

use serde::Deserialize;

use super::*;

const RENDERING_CASES: [(&str, &str); 8] = [
    (
        "plain-unstyled.html",
        include_str!("../../../../testdata/mail-rendering/plain-unstyled.html"),
    ),
    (
        "transactional-table.html",
        include_str!("../../../../testdata/mail-rendering/transactional-table.html"),
    ),
    (
        "flex-invoice-table.html",
        include_str!("../../../../testdata/mail-rendering/flex-invoice-table.html"),
    ),
    (
        "marketing-responsive.html",
        include_str!("../../../../testdata/mail-rendering/marketing-responsive.html"),
    ),
    (
        "native-dark.html",
        include_str!("../../../../testdata/mail-rendering/native-dark.html"),
    ),
    (
        "mixed-background-table.html",
        include_str!("../../../../testdata/mail-rendering/mixed-background-table.html"),
    ),
    (
        "links-and-remote-resources.html",
        include_str!("../../../../testdata/mail-rendering/links-and-remote-resources.html"),
    ),
    (
        "malicious-active-content.html",
        include_str!("../../../../testdata/mail-rendering/malicious-active-content.html"),
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
        "../../../../testdata/mail-rendering/manifest.json"
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
fn preserves_only_valid_bounded_raster_data_images() {
    let sanitized = sanitize_mail_html(
        r#"<img id="png" src="data:image/png;base64,iVBORw0KGgo=">
           <img id="fake" src="data:image/png;base64,bm90LWEtcG5n">
           <img id="svg" src="data:image/svg+xml;base64,PHN2Zz48L3N2Zz4+">"#,
    );

    assert!(sanitized
        .document
        .contains("data:image/png;base64,iVBORw0KGgo="));
    assert!(!sanitized
        .document
        .contains("data:image/png;base64,bm90LWEtcG5n"));
    assert!(!sanitized.document.contains("data:image/svg+xml"));
    assert!(sanitized.document.contains("id=\"fake\""));
    assert!(sanitized.document.contains("id=\"svg\""));
}

#[test]
fn preserves_valid_bmp_data_images_and_rejects_magic_only_prefixes() {
    let valid = "Qk02AAAAAAAAADYAAAAoAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA";
    let sanitized = sanitize_mail_html(&format!(
        "<img id=\"bmp\" src=\"data:image/bmp;base64,{valid}\">\
         <img id=\"fake\" src=\"data:image/bmp;base64,Qk1ub3QtYS1ibXA=\">"
    ));

    assert!(sanitized
        .document
        .contains(&format!("data:image/bmp;base64,{valid}")));
    assert!(!sanitized
        .document
        .contains("data:image/bmp;base64,Qk1ub3QtYS1ibXA="));
}

#[test]
fn detects_octet_stream_bmp_cid_parts_from_a_bounded_file_header() {
    let raw = concat!(
        "MIME-Version: 1.0\r\n",
        "Content-Type: multipart/related; boundary=nextmail\r\n\r\n",
        "--nextmail\r\n",
        "Content-Type: text/html; charset=utf-8\r\n\r\n",
        "<img src=\"cid:legacy-bmp@example.test\">\r\n",
        "--nextmail\r\n",
        "Content-Type: application/octet-stream\r\n",
        "Content-Disposition: inline; filename=legacy.bmp\r\n",
        "Content-ID: <legacy-bmp@example.test>\r\n",
        "Content-Transfer-Encoding: base64\r\n\r\n",
        "Qk02AAAAAAAAADYAAAAoAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA\r\n",
        "--nextmail--\r\n"
    );
    let body = sanitize_raw_message_body(raw.as_bytes()).expect("BMP MIME body");
    let html = body.safe_html.expect("safe BMP HTML");

    assert!(html.contains("data:image/bmp;base64,"));
    assert_eq!(body.inline_content_ids, vec!["legacy-bmp@example.test"]);
}

#[test]
fn normalizes_percent_encoded_cid_references() {
    assert_eq!(
        normalize_cid_reference("CID:<Logo%40Example.Test>"),
        Some("logo@example.test".to_owned())
    );
    assert_eq!(
        normalize_cid_reference("cid:logo%25mark@example.test"),
        Some("logo%mark@example.test".to_owned())
    );
    assert_eq!(normalize_cid_reference("https://example.test"), None);
    assert_eq!(normalize_cid_reference("cid:broken%2"), None);
}

#[test]
fn indexes_percent_encoded_cid_references_across_all_mime_parts() {
    let raw = concat!(
        "MIME-Version: 1.0\r\n",
        "Content-Type: multipart/related; boundary=nextmail\r\n\r\n",
        "--nextmail\r\n",
        "Content-Type: text/html; charset=utf-8\r\n\r\n",
        "<img src=\"CID:logo%40example.test\">\r\n",
        "--nextmail\r\n",
        "Content-Type: image/png\r\n",
        "Content-Disposition: attachment; filename=logo.png\r\n",
        "Content-ID: <logo@example.test>\r\n",
        "Content-Transfer-Encoding: base64\r\n\r\n",
        "aW1hZ2U=\r\n",
        "--nextmail--\r\n"
    );
    let message = MessageParser::default()
        .parse(raw.as_bytes())
        .expect("parse MIME message");
    let html = message.body_html(0).expect("HTML body");
    let references = referenced_content_ids(&html);
    assert_eq!(references, HashSet::from(["logo@example.test".to_owned()]));
    let images = inline_image_data_urls(&message, &html);
    assert_eq!(
        images.get("logo@example.test").map(String::as_str),
        Some("data:image/png;base64,aW1hZ2U=")
    );
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
        r##"<hr color="#b5c4df"><table class="campaign" id="mail-shell" role="presentation" width="600" cellpadding="0" cellspacing="0" border="1" bordercolor="#000000" align="center" bgcolor="#ffffff"><tbody><tr valign="top" bgcolor="#eeeeee"><td width="420" height="80" valign="middle" nowrap><font face="Arial" size="3" color="#202124">Content</font></td></tr></tbody></table>"##,
    );

    for expected in [
        "class=\"campaign\"",
        "id=\"mail-shell\"",
        "role=\"presentation\"",
        "width=\"600\"",
        "cellpadding=\"0\"",
        "cellspacing=\"0\"",
        "border=\"1\"",
        "bordercolor=\"#000000\"",
        "color=\"#b5c4df\"",
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
        "../../../../testdata/mail-rendering/marketing-responsive.html"
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
        "../../../../testdata/mail-rendering/native-dark.html"
    ));
    assert!(native_dark
        .document
        .contains("data-nextmail-native-dark=\"\""));
    assert!(native_dark
        .document
        .contains("@media (prefers-color-scheme:dark)"));
    assert!(native_dark.document.contains("background-color:#252525"));

    let plain = sanitize_mail_html(include_str!(
        "../../../../testdata/mail-rendering/plain-unstyled.html"
    ));
    assert!(!plain.document.contains("data-nextmail-native-dark"));
}

#[test]
fn carries_outlook_dark_hooks_through_the_safe_document() {
    let sanitized =
        sanitize_mail_html(r#"<div data-ogsc="" style="color:#eee">Outlook dark</div>"#);
    assert!(sanitized
        .document
        .contains("data-nextmail-native-dark=\"\""));
    assert!(sanitized.document.contains("data-ogsc=\"\""));
}

#[test]
fn malicious_embedded_css_cannot_escape_or_request_resources() {
    let sanitized = sanitize_mail_html(include_str!(
        "../../../../testdata/mail-rendering/malicious-active-content.html"
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
fn preserves_portable_composer_body_styles() {
    let sanitized = sanitize_composer_document(
        r#"<style>
            [data-nextmail-composer-body]{background:#fff;font-family:system-ui,"Segoe UI",Arial,sans-serif;font-size:14px;line-height:1.2;overflow-wrap:anywhere}
            [data-nextmail-composer-body] p{margin:0 0 .8em}
            [data-nextmail-composer-body] ul{list-style-type:none}
            [data-nextmail-composer-body] .nextmail-composition-signature>:last-child{margin-bottom:0}
        </style>
            <div data-nextmail-composer-body=""><p>Visible body</p></div>"#,
    );

    for expected in [
        "data-nextmail-composer-body",
        "font-size:14px",
        "line-height:1.2",
        "list-style-type:none",
        "nextmail-composition-signature>:last-child",
        "Visible body",
    ] {
        assert!(sanitized.contains(expected), "missing {expected}");
    }
}

#[test]
fn preserves_flex_invoice_column_ratios_for_reading_and_composer_import() {
    let fixture = include_str!("../../../../testdata/mail-rendering/flex-invoice-table.html");
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
        "Content-Type: application/octet-stream; name=logo.png\r\n",
        "Content-Disposition: inline; filename=logo.png\r\n",
        "Content-ID: <logo@example.test>\r\n",
        "Content-Transfer-Encoding: base64\r\n\r\n",
        "iVBORw0KGgo=\r\n",
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
    assert_eq!(body.inline_images[0].bytes, b"\x89PNG\r\n\x1a\n");
}
