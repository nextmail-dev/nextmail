use super::*;

#[test]
fn preserves_recipients_styles_and_every_attachment_without_exposing_unsafe_html() {
    let raw = concat!(
        "From: sender@example.com\r\nTo: Alice <alice@example.com>\r\nCc: cc@example.com\r\nBcc: secret@example.com\r\nSubject: Original topic\r\n",
        "MIME-Version: 1.0\r\nContent-Type: multipart/mixed; boundary=x\r\n\r\n",
        "--x\r\nContent-Type: text/html; charset=utf-8\r\n\r\n",
        "<style>p { color: #123456 } body { background: url(https://bad.example/x) }</style><script>alert(1)</script>",
        "<table><tr><td><p style=\"font-family: Georgia; font-size: 22px\">Body</p><img src=\"cid:picture\"></td></tr></table>\r\n",
        "--x\r\nContent-Type: image/png\r\nContent-ID: <picture>\r\nContent-Disposition: inline; filename=picture.png\r\nContent-Transfer-Encoding: base64\r\n\r\n",
        "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aZf0AAAAASUVORK5CYII=\r\n",
        "--x\r\nContent-Type: application/octet-stream\r\nContent-Disposition: attachment; filename=report.bin\r\nContent-Transfer-Encoding: base64\r\n\r\nAQIDBA==\r\n--x--\r\n"
    );
    let imported = parse_editable_message(raw.as_bytes()).unwrap();
    assert_eq!(imported.source.subject, "Original topic");
    assert_eq!(
        imported.source.recipients.to[0].name.as_deref(),
        Some("Alice")
    );
    assert_eq!(imported.source.recipients.cc[0].email, "cc@example.com");
    assert_eq!(
        imported.source.recipients.bcc[0].email,
        "secret@example.com"
    );
    let html = imported.source.safe_html.unwrap();
    assert!(html.contains("data-nextmail-pasted-html"));
    assert!(html.contains("[data-nextmail-pasted-html] p"));
    assert!(html.contains("Georgia") && html.contains("22px") && html.contains("<table>"));
    assert!(!html.contains("<script") && !html.contains("url("));
    assert!(html.contains("cid:picture"));
    assert_eq!(imported.attachments.len(), 2);
    assert_eq!(
        imported.attachments[0].content_id.as_deref(),
        Some("picture")
    );
    assert_eq!(imported.attachments[1].file_name, "report.bin");
    assert_eq!(imported.attachments[1].bytes, [1, 2, 3, 4]);
}

#[test]
fn keeps_plain_text_and_does_not_promote_unverified_cid_parts_to_inline_images() {
    let raw = concat!("To: to@example.com\r\nContent-Type: multipart/mixed; boundary=x\r\n\r\n",
        "--x\r\nContent-Type: text/plain\r\n\r\nFirst\nSecond\r\n",
        "--x\r\nContent-Type: image/png\r\nContent-ID: <fake>\r\nContent-Disposition: attachment; filename=fake.png\r\n\r\nNOT A PNG\r\n--x--\r\n");
    let imported = parse_editable_message(raw.as_bytes()).unwrap();
    assert!(imported.source.plain_text.contains("First\nSecond"));
    assert_eq!(imported.attachments.len(), 1);
    assert!(imported.attachments[0].content_id.is_none());
    assert!(imported.source.recipients.bcc.is_empty());
}
