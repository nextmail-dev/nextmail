use super::*;
use crate::core::ContactAddressRole;
use crate::protocols::imap::parse::parse_message;
use mail_parser::MessageParser;

#[test]
fn formats_a_batch_as_one_uid_set() {
    assert_eq!(format_uid_set(&[3, 7, 9]), "3,7,9");
    assert_eq!(format_uid_set(&[]), "");
}

#[test]
fn caps_header_fetch_commands_at_twenty_uids() {
    let uids = (1..=45).collect::<Vec<_>>();
    assert_eq!(
        uids.chunks(FETCH_BATCH_SIZE)
            .map(<[u32]>::len)
            .collect::<Vec<_>>(),
        [20, 20, 5]
    );
}

#[test]
fn classifies_message_unavailable_errors() {
    assert!(is_message_unavailable_error("sync.message_not_found"));
    assert!(is_message_unavailable_error("sync.message_body_missing"));
    assert!(!is_message_unavailable_error(
        "sync.message_body_fetch_failed"
    ));
    assert!(!is_message_unavailable_error("sync.uid_validity_changed"));
    assert!(!is_message_unavailable_error(""));
}

#[test]
fn split_uids_partitions_disjointly_and_covers_all_workers() {
    // Contiguous, disjoint, every UID present exactly once.
    let chunks = split_uids(&(1..=10).collect::<Vec<_>>(), 3);
    assert_eq!(chunks.len(), 3);
    let mut all: Vec<u32> = chunks.iter().flatten().copied().collect();
    all.sort_unstable();
    assert_eq!(all, (1..=10).collect::<Vec<_>>());
    // Fewer messages than workers -> some chunks empty, count still matches.
    let sparse = split_uids(&[7u32], 3);
    assert_eq!(sparse.len(), 3);
    assert_eq!(sparse.iter().flatten().copied().sum::<u32>(), 7);
    // No messages at all -> no panic, n empty chunks (workers no-op).
    let empty = split_uids(&[], 3);
    assert_eq!(empty.len(), 3);
    assert!(empty.iter().all(Vec::is_empty));
}

#[test]
fn full_message_sync_fetches_only_missing_bodies_for_current_uid_validity() {
    let locations = pending_body_locations(
        vec![
            StoredMessageLocation {
                message_id: "nine".to_owned(),
                uid: 9,
                uid_validity: 2,
            },
            StoredMessageLocation {
                message_id: "three".to_owned(),
                uid: 3,
                uid_validity: 2,
            },
            StoredMessageLocation {
                message_id: "one".to_owned(),
                uid: 1,
                uid_validity: 1,
            },
        ],
        2,
    );
    assert_eq!(
        locations
            .into_iter()
            .map(|location| location.uid)
            .collect::<Vec<_>>(),
        vec![3, 9]
    );
}

#[test]
fn parses_and_sanitizes_html_message() {
    let raw = b"From: Alice <alice@example.com>\r\nTo: Bob <bob@example.com>\r\nSubject: Hello\r\nMessage-ID: <1@example.com>\r\nContent-Type: text/html; charset=utf-8\r\n\r\n<!doctype html><html><head><title>Hidden title</title></head><body><p onclick=\"bad()\">Hello<script>bad()</script></p></body></html>";
    let message = parse_message(
        1,
        1,
        raw.len() as u64,
        1,
        [Flag::Seen].into_iter(),
        raw,
        Some(raw.to_vec()),
    )
    .unwrap();
    assert_eq!(message.subject, "Hello");
    assert!(!message.unread);
    assert_eq!(message.plain_text.as_deref(), Some("Hello\n"));
    assert_eq!(message.preview, "Hello\n");
    let safe_html = message.safe_html.unwrap();
    assert!(!safe_html.contains("<script"));
    assert!(!safe_html.contains("Hidden title"));
}

#[test]
fn recognizes_common_high_priority_headers() {
    for header in [
        "X-Priority: 1 (Highest)",
        "X-Priority: 2",
        "Importance: high",
        "Priority: urgent",
        "X-MSMail-Priority: High",
    ] {
        let raw = format!("{header}\r\nSubject: Important\r\n\r\nbody");
        let message = parse_message(
            1,
            1,
            raw.len() as u64,
            1,
            [Flag::Seen].into_iter(),
            raw.as_bytes(),
            Some(raw.as_bytes().to_vec()),
        )
        .unwrap();
        assert!(message.high_priority, "header was not recognized: {header}");
    }

    let normal = b"X-Priority: 3 (Normal)\r\nImportance: normal\r\n\r\nbody";
    let message = parse_message(
        1,
        1,
        normal.len() as u64,
        1,
        [Flag::Seen].into_iter(),
        normal,
        Some(normal.to_vec()),
    )
    .unwrap();
    assert!(!message.high_priority);
}

#[test]
fn collects_every_observable_address_header_for_contacts() {
    let raw = concat!(
        "From: From Person <from@example.com>\r\n",
        "Sender: sender@example.com\r\n",
        "Reply-To: reply@example.com\r\n",
        "To: to@example.com\r\n",
        "Cc: cc@example.com\r\n",
        "Bcc: bcc@example.com\r\n",
        "Subject: Contact headers\r\n\r\n",
        "Body"
    );
    let message = parse_message(
        1,
        1,
        raw.len() as u64,
        1,
        std::iter::empty(),
        raw.as_bytes(),
        Some(raw.as_bytes().to_vec()),
    )
    .unwrap();
    let values = message
        .contact_addresses
        .into_iter()
        .map(|value| (value.role, value.address.email))
        .collect::<Vec<_>>();
    assert_eq!(
        values,
        vec![
            (ContactAddressRole::From, "from@example.com".to_owned()),
            (ContactAddressRole::Sender, "sender@example.com".to_owned()),
            (ContactAddressRole::ReplyTo, "reply@example.com".to_owned()),
            (ContactAddressRole::To, "to@example.com".to_owned()),
            (ContactAddressRole::Cc, "cc@example.com".to_owned()),
            (ContactAddressRole::Bcc, "bcc@example.com".to_owned()),
        ]
    );
}

#[test]
fn embeds_referenced_cid_images_without_listing_them_as_attachments() {
    let raw = concat!(
        "From: sender@example.com\r\n",
        "To: reader@example.com\r\n",
        "Subject: Inline image\r\n",
        "MIME-Version: 1.0\r\n",
        "Content-Type: multipart/related; boundary=nextmail\r\n\r\n",
        "--nextmail\r\n",
        "Content-Type: text/html; charset=utf-8\r\n\r\n",
        "<p>Logo <img src=\"CID:logo%40example.test\"></p>\r\n",
        "--nextmail\r\n",
        "Content-Type: image/png; name=logo.png\r\n",
        "Content-Disposition: attachment; filename=logo.png\r\n",
        "Content-ID: <logo@example.test>\r\n",
        "Content-Transfer-Encoding: base64\r\n\r\n",
        "aW1hZ2U=\r\n",
        "--nextmail--\r\n"
    );
    let message = parse_message(
        1,
        1,
        raw.len() as u64,
        1,
        [Flag::Seen].into_iter(),
        raw.as_bytes(),
        Some(raw.as_bytes().to_vec()),
    )
    .unwrap();

    let safe_html = message.safe_html.expect("safe HTML");
    assert!(
        safe_html.contains("data:image/png;base64,aW1hZ2U="),
        "unexpected safe HTML: {safe_html}"
    );
    assert!(message.attachments.is_empty());
}

#[test]
fn embeds_aliyun_style_content_ids_without_listing_them_as_attachments() {
    let raw = concat!(
        "From: sender@example.com\r\n",
        "To: reader@example.com\r\n",
        "Subject: Inline image\r\n",
        "MIME-Version: 1.0\r\n",
        "Content-Type: multipart/related; boundary=nextmail\r\n\r\n",
        "--nextmail\r\n",
        "Content-Type: text/html; charset=utf-8\r\n\r\n",
        "<img src=\"cid:__aliyun178512290634140581\">\r\n",
        "--nextmail\r\n",
        "Content-Type: application/octet-stream\r\n",
        "Content-Disposition: attachment; filename=image.png\r\n",
        "Content-ID: <__aliyun178512290634140581>\r\n",
        "Content-Transfer-Encoding: base64\r\n\r\n",
        "iVBORw0KGgo=\r\n",
        "--nextmail--\r\n"
    );
    let message = parse_message(
        1,
        1,
        raw.len() as u64,
        1,
        [Flag::Seen].into_iter(),
        raw.as_bytes(),
        Some(raw.as_bytes().to_vec()),
    )
    .unwrap();

    let safe_html = message.safe_html.expect("safe HTML");
    assert!(
        safe_html.contains("data:image/png;base64,iVBORw0KGgo="),
        "unexpected safe HTML: {safe_html}"
    );
    assert!(message.attachments.is_empty());
}

#[test]
fn leaves_non_image_octet_stream_cid_parts_as_attachments() {
    let raw = concat!(
        "From: sender@example.com\r\n",
        "To: reader@example.com\r\n",
        "Subject: Invalid inline image\r\n",
        "MIME-Version: 1.0\r\n",
        "Content-Type: multipart/related; boundary=nextmail\r\n\r\n",
        "--nextmail\r\n",
        "Content-Type: text/html; charset=utf-8\r\n\r\n",
        "<img src=\"cid:__aliyun178512290634140581\">\r\n",
        "--nextmail\r\n",
        "Content-Type: application/octet-stream\r\n",
        "Content-Disposition: inline; filename=image.png\r\n",
        "Content-ID: <__aliyun178512290634140581>\r\n",
        "Content-Transfer-Encoding: base64\r\n\r\n",
        "bm90LWEtcG5n\r\n",
        "--nextmail--\r\n"
    );
    let message = parse_message(
        1,
        1,
        raw.len() as u64,
        1,
        [Flag::Seen].into_iter(),
        raw.as_bytes(),
        Some(raw.as_bytes().to_vec()),
    )
    .unwrap();

    assert!(!message.safe_html.expect("safe HTML").contains("data:image"));
    assert_eq!(message.attachments.len(), 1);
}

#[test]
fn leaves_unreferenced_content_id_parts_in_the_attachment_list() {
    let raw = concat!(
        "From: sender@example.com\r\n",
        "To: reader@example.com\r\n",
        "Subject: Unreferenced image\r\n",
        "MIME-Version: 1.0\r\n",
        "Content-Type: multipart/related; boundary=nextmail\r\n\r\n",
        "--nextmail\r\n",
        "Content-Type: text/html; charset=utf-8\r\n\r\n",
        "<p>No inline image reference</p>\r\n",
        "--nextmail\r\n",
        "Content-Type: image/png; name=logo.png\r\n",
        "Content-Disposition: attachment; filename=logo.png\r\n",
        "Content-ID: <logo@example.test>\r\n",
        "Content-Transfer-Encoding: base64\r\n\r\n",
        "aW1hZ2U=\r\n",
        "--nextmail--\r\n"
    );
    let message = parse_message(
        1,
        1,
        raw.len() as u64,
        1,
        [Flag::Seen].into_iter(),
        raw.as_bytes(),
        Some(raw.as_bytes().to_vec()),
    )
    .unwrap();

    assert_eq!(message.attachments.len(), 1);
    assert_eq!(message.attachments[0].file_name, "logo.png");
}

#[test]
fn decodes_rfc2047_attachment_names_split_across_continuation_parameters() {
    let raw =
        include_bytes!("../../../../testdata/mail-rendering/segmented-rfc2047-attachment-name.eml");
    let message = parse_message(
        1,
        1,
        raw.len() as u64,
        1,
        [Flag::Seen].into_iter(),
        raw,
        Some(raw.to_vec()),
    )
    .unwrap();

    assert_eq!(message.attachments.len(), 1);
    assert_eq!(
        message.attachments[0].file_name,
        "黄龙机房搬迁割接第三期1.xlsx"
    );
}

#[test]
fn decodes_split_rfc2047_name_from_content_type_when_filename_is_absent() {
    let raw = concat!(
        "From: sender@example.com\r\n",
        "To: reader@example.com\r\n",
        "Subject: Split content type name\r\n",
        "MIME-Version: 1.0\r\n",
        "Content-Type: multipart/mixed; boundary=nextmail\r\n\r\n",
        "--nextmail\r\n",
        "Content-Type: text/plain; charset=utf-8\r\n\r\n",
        "Body\r\n",
        "--nextmail\r\n",
        "Content-Type: application/octet-stream;\r\n",
        " name*0=\"=?UTF-8?B?6buE6b6Z5py65oi/5pCs6L+B5Ymy5o6l56ys5LiJ5pyfMS54bH\";\r\n",
        " name*1=\"N4?=\"\r\n",
        "Content-Transfer-Encoding: base64\r\n",
        "Content-Disposition: attachment\r\n\r\n",
        "YXR0YWNobWVudA==\r\n",
        "--nextmail--\r\n"
    );
    let message = parse_message(
        1,
        1,
        raw.len() as u64,
        1,
        [Flag::Seen].into_iter(),
        raw.as_bytes(),
        Some(raw.as_bytes().to_vec()),
    )
    .unwrap();

    assert_eq!(message.attachments.len(), 1);
    assert_eq!(
        message.attachments[0].file_name,
        "黄龙机房搬迁割接第三期1.xlsx"
    );
}

#[test]
fn preserves_standard_percent_encoded_rfc2231_continuations() {
    let raw = concat!(
        "From: sender@example.com\r\n",
        "To: reader@example.com\r\n",
        "Subject: RFC 2231 attachment name\r\n",
        "MIME-Version: 1.0\r\n",
        "Content-Type: multipart/mixed; boundary=nextmail\r\n\r\n",
        "--nextmail\r\n",
        "Content-Type: text/plain; charset=utf-8\r\n\r\n",
        "Body\r\n",
        "--nextmail\r\n",
        "Content-Type: application/octet-stream\r\n",
        "Content-Transfer-Encoding: base64\r\n",
        "Content-Disposition: attachment;\r\n",
        " filename*0*=UTF-8''%E9%BB%84%E9%BE%99%E6%9C%BA%E6%88%BF;\r\n",
        " filename*1*=%E6%90%AC%E8%BF%81.xlsx\r\n\r\n",
        "YXR0YWNobWVudA==\r\n",
        "--nextmail--\r\n"
    );
    let message = parse_message(
        1,
        1,
        raw.len() as u64,
        1,
        [Flag::Seen].into_iter(),
        raw.as_bytes(),
        Some(raw.as_bytes().to_vec()),
    )
    .unwrap();

    assert_eq!(message.attachments.len(), 1);
    assert_eq!(message.attachments[0].file_name, "黄龙机房搬迁.xlsx");
}

#[test]
fn decodes_gb2312_encoded_words_and_message_bodies() {
    let raw = b"From: =?GB2312?B?xOO6ww==?= <alice@example.com>\r\n\
To: Bob <bob@example.com>\r\n\
Subject: =?GB2312?B?xOO6ww==?=\r\n\
Content-Type: text/plain; charset=gb2312\r\n\
Content-Transfer-Encoding: base64\r\n\r\n\
xOO6ww==";
    let message = parse_message(
        1,
        1,
        raw.len() as u64,
        1,
        [Flag::Seen].into_iter(),
        raw,
        Some(raw.to_vec()),
    )
    .unwrap();

    assert_eq!(message.subject, "你好");
    assert_eq!(message.from[0].name.as_deref(), Some("你好"));
    assert_eq!(message.plain_text.as_deref(), Some("你好"));
}

#[test]
fn decodes_rfc2047_b_q_aliases_and_address_phrases() {
    let cases = [
        ("=?UTF-8?B?5L2g5aW9?=", "你好"),
        ("=?utf-8?q?Hello_=E4=B8=96=E7=95=8C?=", "Hello 世界"),
        ("=?ISO-8859-1?Q?caf=E9?=", "café"),
        ("=?windows-1252?Q?=80uro?=", "€uro"),
        ("=?UTF-7?B?K1plVm5MSXFlLQ==?=", "日本語"),
    ];

    for (encoded, expected) in cases {
        let raw = format!("From: {encoded} <alice@example.com>\r\nSubject: {encoded}\r\n\r\nbody");
        let message = parse_message(
            1,
            1,
            raw.len() as u64,
            1,
            [Flag::Seen].into_iter(),
            raw.as_bytes(),
            Some(raw.as_bytes().to_vec()),
        )
        .unwrap();
        assert_eq!(message.subject, expected);
        assert_eq!(message.from[0].name.as_deref(), Some(expected));
    }
}

#[test]
fn decodes_adjacent_folded_rfc2047_words_and_mixed_ascii() {
    let raw = concat!(
        "From: =?UTF-8?B?5L2g5aW9?=\r\n",
        " =?UTF-8?Q?_=E4=B8=96=E7=95=8C?= <alice@example.com>\r\n",
        "Subject: Status =?UTF-8?B?5L2g5aW9?=\r\n",
        " =?UTF-8?Q?_=E4=B8=96=E7=95=8C?= ready\r\n\r\n",
        "body"
    );
    let directly_parsed = MessageParser::default().parse(raw.as_bytes()).unwrap();
    assert_eq!(directly_parsed.subject(), Some("Status 你好 世界 ready"));
    assert_eq!(
        directly_parsed
            .from()
            .and_then(|address| address.first())
            .and_then(|address| address.name.as_deref()),
        Some("你好 世界")
    );
    let message = parse_message(
        1,
        1,
        raw.len() as u64,
        1,
        [Flag::Seen].into_iter(),
        raw.as_bytes(),
        Some(raw.as_bytes().to_vec()),
    )
    .unwrap();

    assert_eq!(message.subject, "Status 你好 世界 ready");
    assert_eq!(message.from[0].name.as_deref(), Some("你好 世界"));
}

#[test]
fn malformed_rfc2047_words_fail_safely_without_losing_following_headers() {
    let raw = "From: Alice <alice@example.com>\r\n\
Subject: prefix =?X-UNKNOWN?Q?abc=FF?= suffix\r\n\
Message-ID: <safe@example.com>\r\n\r\nbody";
    let message = parse_message(
        1,
        1,
        raw.len() as u64,
        1,
        [Flag::Seen].into_iter(),
        raw.as_bytes(),
        Some(raw.as_bytes().to_vec()),
    )
    .unwrap();

    assert!(message.subject.starts_with("prefix "));
    assert!(message.subject.ends_with(" suffix"));
    assert_eq!(message.message_id.as_deref(), Some("safe@example.com"));
}

mod worker_tests {
    use super::*;
    use crate::core::{ConnectionSecurity, MessageUpsertOutcome, RemoteMessageBody, SyncObserver};
    use async_trait::async_trait;
    use std::sync::Mutex as StdMutex;
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, DuplexStream};

    const TEST_HEADER: &str =
        "From: Alice <alice@example.com>\r\nTo: Bob <bob@example.com>\r\nSubject: test\r\nMessage-ID: <m@example.com>\r\n";

    struct RecordingSink {
        upserts: StdMutex<Vec<(u32, usize)>>,
        bodies: StdMutex<Vec<String>>,
        pending: StdMutex<Vec<StoredMessageLocation>>,
        mailbox_events: StdMutex<Vec<String>>,
    }

    impl RecordingSink {
        fn new() -> Self {
            Self {
                upserts: StdMutex::new(Vec::new()),
                bodies: StdMutex::new(Vec::new()),
                pending: StdMutex::new(Vec::new()),
                mailbox_events: StdMutex::new(Vec::new()),
            }
        }

        fn mailbox_event_snapshot(&self) -> Vec<String> {
            self.mailbox_events.lock().unwrap().clone()
        }

        fn upsert_snapshot(&self) -> Vec<(u32, usize)> {
            self.upserts.lock().unwrap().clone()
        }

        fn body_snapshot(&self) -> Vec<String> {
            self.bodies.lock().unwrap().clone()
        }

        fn set_pending(&self, locations: Vec<StoredMessageLocation>) {
            *self.pending.lock().unwrap() = locations;
        }
    }

    #[async_trait]
    impl MailSyncSink for RecordingSink {
        async fn ensure_mailbox(
            &self,
            _account_slot_id: &str,
            mailbox: &RemoteMailbox,
        ) -> CommandResult<Option<StoredMailbox>> {
            self.mailbox_events
                .lock()
                .unwrap()
                .push(format!("ensure:{}", mailbox.name));
            Ok(Some(StoredMailbox {
                id: format!("mb-{}", mailbox.name),
                last_uid: 0,
                highest_modseq: None,
                notification_baseline_required: true,
            }))
        }

        async fn upsert_mailbox(
            &self,
            _account_slot_id: &str,
            mailbox: &RemoteMailbox,
        ) -> CommandResult<StoredMailbox> {
            self.mailbox_events
                .lock()
                .unwrap()
                .push(format!("upsert:{}", mailbox.name));
            Ok(StoredMailbox {
                id: "mb".to_owned(),
                last_uid: 0,
                highest_modseq: None,
                notification_baseline_required: true,
            })
        }

        async fn upsert_message(
            &self,
            _account_slot_id: &str,
            _mailbox_id: &str,
            message: &RemoteMessage,
        ) -> CommandResult<MessageUpsertOutcome> {
            self.upserts
                .lock()
                .unwrap()
                .push((message.uid, message.attachments.len()));
            Ok(MessageUpsertOutcome {
                message_id: format!("id-{}", message.uid),
                is_new_location: true,
                contacts_changed: false,
            })
        }

        async fn complete_notification_baseline(
            &self,
            _account_slot_id: &str,
        ) -> CommandResult<()> {
            Ok(())
        }

        async fn complete_mailbox(&self, _mailbox_id: &str, _last_uid: u32) -> CommandResult<()> {
            Ok(())
        }

        async fn stored_uids(
            &self,
            _mailbox_id: &str,
            _uid_validity: u32,
        ) -> CommandResult<Vec<u32>> {
            Ok(Vec::new())
        }

        async fn pending_body_locations(
            &self,
            _mailbox_id: &str,
            _received_after: Option<i64>,
        ) -> CommandResult<Vec<StoredMessageLocation>> {
            Ok(self.pending.lock().unwrap().clone())
        }

        async fn replace_message_body(
            &self,
            _account_slot_id: &str,
            message_id: &str,
            _body: &RemoteMessageBody,
        ) -> CommandResult<()> {
            self.bodies.lock().unwrap().push(message_id.to_owned());
            Ok(())
        }

        async fn reconcile_mailbox(
            &self,
            _mailbox_id: &str,
            _uid_validity: u32,
            _highest_modseq: Option<u64>,
            _states: &[RemoteMessageState],
        ) -> CommandResult<()> {
            Ok(())
        }
    }

    struct RecordingObserver {
        mailbox_changes: StdMutex<Vec<String>>,
    }

    impl RecordingObserver {
        fn new() -> Self {
            Self {
                mailbox_changes: StdMutex::new(Vec::new()),
            }
        }

        fn mailbox_changes_snapshot(&self) -> Vec<String> {
            self.mailbox_changes.lock().unwrap().clone()
        }
    }

    impl SyncObserver for RecordingObserver {
        fn notify(&self, notice: SyncNotice) {
            if let SyncNotice::MailboxChanged { mailbox_id, .. } = notice {
                self.mailbox_changes.lock().unwrap().push(mailbox_id);
            }
        }
    }

    struct WorkerHarness {
        mailbox: StoredMailbox,
    }

    impl WorkerHarness {
        fn context(&self) -> FolderSyncContext<'_> {
            FolderSyncContext {
                uid_validity: 7,
                mailbox: &self.mailbox,
                mailbox_name: "Inbox",
                default_notification_enabled: false,
            }
        }
    }

    fn test_account() -> ImapAccountConfig {
        ImapAccountConfig {
            account_id: "acc".to_owned(),
            account_slot_id: "slot".to_owned(),
            download_full_messages: false,
            host: "imap.example.com".to_owned(),
            port: 993,
            security: ConnectionSecurity::Tls,
            username: "user".to_owned(),
            password: "pass".to_owned(),
        }
    }

    fn header_response(uid: u32) -> String {
        format!(
            "* {uid} FETCH (UID {uid} FLAGS (\\Seen) INTERNALDATE \"15-Aug-2026 14:01:04 +0800\" RFC822.SIZE 100 BODY[HEADER] {{{}}}\r\n{TEST_HEADER})\r\n",
            TEST_HEADER.len()
        )
    }

    fn tagged_ok(tag: &str) -> String {
        format!("{tag} OK UID FETCH Completed\r\n")
    }

    fn attachment_bodystructure(uid: u32) -> String {
        format!("* {uid} FETCH (UID {uid} BODYSTRUCTURE (\"APPLICATION\" \"OCTET-STREAM\" (\"name\" \"mail.eml\" \"charset\" \"utf-8\") NIL NIL \"BASE64\" 2536 NIL (\"attachment\" (\"filename\" \"mail.eml\")) NIL))\r\n")
    }

    fn plain_bodystructure(uid: u32) -> String {
        format!("* {uid} FETCH (UID {uid} BODYSTRUCTURE (\"TEXT\" \"PLAIN\" (\"charset\" \"utf-8\") NIL NIL \"7BIT\" 5 1 NIL NIL NIL))\r\n")
    }

    // Delivery-status report shape observed from QQ Mail: the HTML part's
    // body-fld-enc is NIL, which the strict RFC 3501 grammar in imap-proto
    // rejects and poisons the whole response stream.
    fn qq_poison_bodystructure(uid: u32) -> String {
        format!("* {uid} FETCH (UID {uid} BODYSTRUCTURE ((\"TEXT\" \"HTML\" (\"charset\" \"utf-8\") NIL NIL NIL 2715 34 NIL NIL NIL)(\"APPLICATION\" \"OCTET-STREAM\" (\"name\" \"mail.eml\" \"charset\" \"utf-8\") NIL NIL \"BASE64\" 2536 NIL (\"attachment\" (\"filename\" \"mail.eml\")) NIL) \"REPORT\" (\"BOUNDARY\" \"QQ_MAIL_RETURN\") NIL NIL))\r\n")
    }

    #[tokio::test]
    async fn bodystructure_parse_failure_commits_headers_and_degrades_gracefully() {
        let (client_stream, server) = tokio::io::duplex(1 << 16);
        let server_task = tokio::spawn(async move {
            let mut lines = BufReader::new(server);
            let mut line = String::new();
            // LOGIN
            lines.read_line(&mut line).await.unwrap();
            let tag = line.split_whitespace().next().unwrap().to_owned();
            write_all(
                lines.get_mut(),
                format!("{tag} OK LOGIN completed\r\n").as_bytes(),
            )
            .await;
            // header batch
            line.clear();
            lines.read_line(&mut line).await.unwrap();
            let tag = line.split_whitespace().next().unwrap().to_owned();
            let mut out = String::new();
            for uid in 1..=3u32 {
                out.push_str(&header_response(uid));
            }
            out.push_str(&tagged_ok(&tag));
            write_all(lines.get_mut(), out.as_bytes()).await;
            // bodystructure batch, UID 2 poisoned with the QQ shape
            line.clear();
            lines.read_line(&mut line).await.unwrap();
            let tag = line.split_whitespace().next().unwrap().to_owned();
            let mut out = String::new();
            out.push_str(&plain_bodystructure(1));
            out.push_str(&qq_poison_bodystructure(2));
            out.push_str(&plain_bodystructure(3));
            out.push_str(&tagged_ok(&tag));
            write_all(lines.get_mut(), out.as_bytes()).await;
        });

        let sink = RecordingSink::new();
        let observer = RecordingObserver::new();
        let harness = WorkerHarness {
            mailbox: StoredMailbox {
                id: "mb".to_owned(),
                last_uid: 0,
                highest_modseq: None,
                notification_baseline_required: true,
            },
        };
        let account = test_account();
        let context = harness.context();
        let completed = AtomicU64::new(0);
        let write_lock = Mutex::new(());
        let mut session = async_imap::Client::new(client_stream)
            .login("user", "pass")
            .await
            .unwrap();
        let result = fetch_summaries_worker(
            &mut session,
            &[1, 2, 3],
            &account,
            &sink,
            &observer,
            &context,
            false,
            &completed,
            3,
            &write_lock,
        )
        .await;

        server_task.await.unwrap();
        let (highest_uid, session_usable) = result.unwrap();
        assert_eq!(highest_uid, 3);
        assert!(!session_usable);
        let upserts = sink.upsert_snapshot();
        assert_eq!(upserts, vec![(1, 0), (2, 0), (3, 0)]);
    }

    #[tokio::test]
    async fn bodystructure_attachments_merge_into_committed_headers() {
        let (client_stream, server) = tokio::io::duplex(1 << 16);
        let server_task = tokio::spawn(async move {
            let mut lines = BufReader::new(server);
            let mut line = String::new();
            // LOGIN
            lines.read_line(&mut line).await.unwrap();
            let tag = line.split_whitespace().next().unwrap().to_owned();
            write_all(
                lines.get_mut(),
                format!("{tag} OK LOGIN completed\r\n").as_bytes(),
            )
            .await;
            // header batch
            line.clear();
            lines.read_line(&mut line).await.unwrap();
            let tag = line.split_whitespace().next().unwrap().to_owned();
            let mut out = String::new();
            for uid in 1..=3u32 {
                out.push_str(&header_response(uid));
            }
            out.push_str(&tagged_ok(&tag));
            write_all(lines.get_mut(), out.as_bytes()).await;
            // bodystructure batch, UID 2 carries one attachment
            line.clear();
            lines.read_line(&mut line).await.unwrap();
            let tag = line.split_whitespace().next().unwrap().to_owned();
            let mut out = String::new();
            out.push_str(&plain_bodystructure(1));
            out.push_str(&attachment_bodystructure(2));
            out.push_str(&plain_bodystructure(3));
            out.push_str(&tagged_ok(&tag));
            write_all(lines.get_mut(), out.as_bytes()).await;
        });

        let sink = RecordingSink::new();
        let observer = RecordingObserver::new();
        let harness = WorkerHarness {
            mailbox: StoredMailbox {
                id: "mb".to_owned(),
                last_uid: 0,
                highest_modseq: None,
                notification_baseline_required: true,
            },
        };
        let account = test_account();
        let context = harness.context();
        let completed = AtomicU64::new(0);
        let write_lock = Mutex::new(());
        let mut session = async_imap::Client::new(client_stream)
            .login("user", "pass")
            .await
            .unwrap();
        let result = fetch_summaries_worker(
            &mut session,
            &[1, 2, 3],
            &account,
            &sink,
            &observer,
            &context,
            false,
            &completed,
            3,
            &write_lock,
        )
        .await;

        server_task.await.unwrap();
        let (highest_uid, session_usable) = result.unwrap();
        assert_eq!(highest_uid, 3);
        assert!(session_usable);
        let upserts = sink.upsert_snapshot();
        assert_eq!(upserts.len(), 4);
        assert_eq!(&upserts[..3], &[(1, 0), (2, 0), (3, 0)]);
        assert_eq!(upserts[3], (2, 1));
    }

    async fn write_all(stream: &mut DuplexStream, bytes: &[u8]) {
        stream.write_all(bytes).await.unwrap();
    }

    #[tokio::test]
    async fn folder_tree_is_precreated_and_notified_before_message_sync() {
        let sink = RecordingSink::new();
        let observer = RecordingObserver::new();
        let account = test_account();
        let descriptors = vec![
            FolderDescriptor {
                name: "INBOX".to_owned(),
                display_name: "INBOX".to_owned(),
                progress_name: "INBOX".to_owned(),
                delimiter: None,
                role: MailboxRole::Inbox,
                selectable: true,
            },
            FolderDescriptor {
                name: "[Gmail]".to_owned(),
                display_name: "[Gmail]".to_owned(),
                progress_name: "[Gmail]".to_owned(),
                delimiter: Some("/".to_owned()),
                role: MailboxRole::Other,
                selectable: false,
            },
        ];
        precreate_folder_tree(&sink, &account.account_slot_id, &descriptors, &observer)
            .await
            .unwrap();
        assert_eq!(
            sink.mailbox_event_snapshot(),
            vec!["ensure:INBOX", "ensure:[Gmail]"]
        );
        assert_eq!(
            observer.mailbox_changes_snapshot(),
            vec!["mb-INBOX", "mb-[Gmail]"]
        );
    }

    const FULL_MESSAGE: &str = "From: a@example.com\r\nTo: b@example.com\r\nSubject: full\r\nContent-Type: text/plain; charset=utf-8\r\n\r\nhello body\r\n";

    fn no_structure_response(tag: &str, uid: u32) -> String {
        format!("* {uid} FETCH (UID {uid})\r\n{tag} OK UID FETCH Completed\r\n")
    }

    fn full_message_response(tag: &str, uid: u32) -> String {
        format!(
            "* {uid} FETCH (UID {uid} FLAGS (\\Seen) INTERNALDATE \"15-Aug-2026 14:01:04 +0800\" RFC822.SIZE {} BODY[] {{{}}}\r\n{FULL_MESSAGE})\r\n{tag} OK UID FETCH Completed\r\n",
            FULL_MESSAGE.len(),
            FULL_MESSAGE.len(),
        )
    }

    #[tokio::test]
    async fn prefetch_requeues_tail_when_bodystructure_kills_a_session() {
        // Session A serves UID 1 with a poison BODYSTRUCTURE (kills the
        // session), session B serves everyone else. UIDs 2-3 must be
        // re-dispatched to session B in the same run; UID 1 stays pending.
        let (client_a, server_a) = tokio::io::duplex(1 << 16);
        let (client_b, server_b) = tokio::io::duplex(1 << 16);
        let server_task_a = tokio::spawn(async move {
            let mut lines = BufReader::new(server_a);
            let mut line = String::new();
            lines.read_line(&mut line).await.unwrap();
            let tag = line.split_whitespace().next().unwrap().to_owned();
            write_all(
                lines.get_mut(),
                format!("{tag} OK LOGIN completed\r\n").as_bytes(),
            )
            .await;
            line.clear();
            lines.read_line(&mut line).await.unwrap();
            let tag = line.split_whitespace().next().unwrap().to_owned();
            let mut out = qq_poison_bodystructure(1);
            out.push_str(&tagged_ok(&tag));
            write_all(lines.get_mut(), out.as_bytes()).await;
            // The full-message fallback on the poisoned session is doomed;
            // read the command and then close the connection.
            line.clear();
            let _ = lines.read_line(&mut line).await;
        });
        let server_task_b = tokio::spawn(async move {
            let mut lines = BufReader::new(server_b);
            let mut line = String::new();
            lines.read_line(&mut line).await.unwrap();
            let tag = line.split_whitespace().next().unwrap().to_owned();
            write_all(
                lines.get_mut(),
                format!("{tag} OK LOGIN completed\r\n").as_bytes(),
            )
            .await;
            loop {
                line.clear();
                let read = lines.read_line(&mut line).await.unwrap();
                if read == 0 {
                    break;
                }
                let tokens = line.split_whitespace().collect::<Vec<_>>();
                let (tag, uid) = (tokens[0].to_owned(), tokens[3].to_owned());
                let response = if line.contains("BODYSTRUCTURE") {
                    no_structure_response(&tag, uid.parse().unwrap())
                } else {
                    full_message_response(&tag, uid.parse().unwrap())
                };
                write_all(lines.get_mut(), response.as_bytes()).await;
            }
        });

        let sink = RecordingSink::new();
        sink.set_pending(
            (1..=5u32)
                .map(|uid| StoredMessageLocation {
                    message_id: format!("m{uid}"),
                    uid,
                    uid_validity: 7,
                })
                .collect(),
        );
        let observer = RecordingObserver::new();
        let harness = WorkerHarness {
            mailbox: StoredMailbox {
                id: "mb".to_owned(),
                last_uid: 0,
                highest_modseq: None,
                notification_baseline_required: true,
            },
        };
        let account = test_account();
        let context = harness.context();
        let write_lock = Mutex::new(());
        let remote_uids = (1..=5u32).collect::<HashSet<_>>();
        let mut session_a = async_imap::Client::new(client_a)
            .login("user", "pass")
            .await
            .unwrap();
        let mut session_b = async_imap::Client::new(client_b)
            .login("user", "pass")
            .await
            .unwrap();
        let mut sessions = vec![&mut session_a, &mut session_b];
        fetch_missing_bodies(
            &mut sessions,
            &account,
            &sink,
            &observer,
            &context,
            &write_lock,
            &remote_uids,
        )
        .await
        .unwrap();

        server_task_a.await.unwrap();
        drop(sessions);
        drop(session_a);
        drop(session_b);
        server_task_b.await.unwrap();
        let mut upserts = sink.upsert_snapshot();
        upserts.sort_unstable();
        assert_eq!(upserts, vec![(2, 0), (3, 0), (4, 0), (5, 0)]);
        assert!(sink.body_snapshot().is_empty());
    }
}
