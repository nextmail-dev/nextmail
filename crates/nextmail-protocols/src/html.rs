use std::borrow::Cow;

use ammonia::Builder;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SanitizedHtml {
    pub document: String,
    pub remote_images_blocked: bool,
}

pub fn sanitize_mail_html(input: &str) -> SanitizedHtml {
    let lower = input.to_ascii_lowercase();
    let remote_images_blocked = lower.contains("<img")
        && (lower.contains("src=\"http://")
            || lower.contains("src='http://")
            || lower.contains("src=\"https://")
            || lower.contains("src='https://"));

    let mut builder = Builder::default();
    builder
        .add_clean_content_tags(["script", "style", "form", "iframe", "object", "svg", "math"])
        .rm_tags([
            "form", "iframe", "object", "embed", "svg", "math", "meta", "link",
        ])
        .strip_comments(true)
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
                let value_lower = value.trim().to_ascii_lowercase();
                return if value_lower.starts_with("http://")
                    || value_lower.starts_with("https://")
                    || value_lower.starts_with("data:image/")
                {
                    Some(Cow::Borrowed(value))
                } else {
                    None
                };
            }
            if attribute_lower == "style" {
                let value_lower = value.to_ascii_lowercase();
                if value_lower.contains("url(")
                    || value_lower.contains("position:")
                    || value_lower.contains("z-index")
                    || value_lower.contains("behavior:")
                    || value_lower.contains("expression(")
                {
                    return None;
                }
            }
            Some(Cow::Borrowed(value))
        });

    let fragment = builder.clean(input).to_string();
    SanitizedHtml {
        document: format!(
            "<!doctype html><html><head><meta charset=\"utf-8\"><meta http-equiv=\"Content-Security-Policy\" content=\"default-src 'none'; img-src data:; style-src 'unsafe-inline'\"><style>html{{color-scheme:light dark}}body{{margin:0;padding:16px;font:14px/1.55 system-ui,sans-serif;overflow-wrap:anywhere}}img{{max-width:100%;height:auto}}table{{max-width:100%;border-collapse:collapse}}a{{color:#2563eb}}</style></head><body>{fragment}</body></html>"
        ),
        remote_images_blocked,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
               <p style="background:url(https://tracker.example/pixel)">content</p>"#,
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
    }
}
