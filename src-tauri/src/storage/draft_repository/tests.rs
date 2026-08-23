use super::*;
use crate::application::{
    compose_imported_draft, compose_message_action_draft, MessageActionLabels,
};
use crate::storage::{create_account_slot, initialize_content_database, MailRepository};

#[tokio::test]
async fn imports_a_server_message_as_an_editable_local_draft() {
    let directory = tempfile::tempdir().unwrap();
    initialize_content_database(directory.path()).await.unwrap();
    create_account_slot(directory.path(), "slot", 1)
        .await
        .unwrap();
    let repository = MailRepository::open(directory.path()).await.unwrap();
    sqlx::query(
        "INSERT INTO messages(id, account_slot_id, subject, to_json, cc_json, received_at) \
         VALUES ('message', 'slot', 'Imported', '[{\"name\":null,\"email\":\"to@example.com\"}]', '[]', 1)",
    )
    .execute(&repository.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO message_bodies(message_id, plain_text, safe_html, updated_at) \
         VALUES ('message', 'First paragraph\n\nSecond paragraph', '<p>First paragraph</p><p>Second paragraph</p>', 1)",
    )
    .execute(&repository.pool)
    .await
    .unwrap();

    let source = repository
        .drafts()
        .imported_draft_source("slot", "message")
        .await
        .unwrap();
    let content = compose_imported_draft(&source).unwrap();
    let draft = repository
        .drafts()
        .persist_imported_draft(PersistImportedDraftRequest {
            account_id: "account",
            account_slot_id: "slot",
            message_id: "message",
            source: &source,
            content: &content,
        })
        .await
        .unwrap();
    assert_eq!(draft.subject, "Imported");
    assert_eq!(draft.recipients.to[0].email, "to@example.com");
    assert!(draft.content.editor_json.contains("Second paragraph"));
    assert_eq!(draft.status, DraftStatus::Editing);
}

#[tokio::test]
async fn creates_reply_all_with_deduplicated_recipients_and_thread_headers() {
    let directory = tempfile::tempdir().unwrap();
    initialize_content_database(directory.path()).await.unwrap();
    create_account_slot(directory.path(), "slot", 1)
        .await
        .unwrap();
    let repository = MailRepository::open(directory.path()).await.unwrap();
    sqlx::query(
        "INSERT INTO messages(id, account_slot_id, subject, from_json, to_json, cc_json, \
         message_id, references_json, received_at) VALUES ( \
         'message', 'slot', 'Topic', \
         '[{\"name\":\"Sender\",\"email\":\"sender@example.com\"}]', \
         '[{\"name\":null,\"email\":\"me@example.com\"},{\"name\":null,\"email\":\"other@example.com\"}]', \
         '[{\"name\":null,\"email\":\"sender@example.com\"}]', \
         'child@example.com', '[\"root@example.com\"]', 1)",
    )
    .execute(&repository.pool)
    .await
    .unwrap();
    sqlx::query(
        "INSERT INTO message_bodies(message_id, plain_text, safe_html, updated_at) \
         VALUES ('message', 'Original body', '<p>Original body</p>', 1)",
    )
    .execute(&repository.pool)
    .await
    .unwrap();

    let source = repository
        .drafts()
        .message_action_source("slot", "message")
        .await
        .unwrap();
    let composed = compose_message_action_draft(
        &source,
        "me@example.com",
        MessageComposeAction::ReplyAll,
        MessageActionLabels {
            reply_original_message: "Original message — Reply",
            forward_original_message: "Original message — Forward",
            from: "From",
            date: "Sent",
            to: "To",
            subject: "Subject",
            reply_subject_prefix: "Re: ",
            forward_subject_prefix: "Fwd: ",
        },
        "1970-01-01 00:00",
    )
    .unwrap();
    let draft = repository
        .drafts()
        .persist_message_action_draft(PersistMessageActionDraftRequest {
            account_id: "account",
            account_slot_id: "slot",
            message_id: "message",
            action: MessageComposeAction::ReplyAll,
            draft: &composed,
        })
        .await
        .unwrap();
    assert_eq!(draft.subject, "Re: Topic");
    assert_eq!(draft.recipients.to.len(), 1);
    assert_eq!(draft.recipients.to[0].email, "sender@example.com");
    assert_eq!(draft.recipients.cc.len(), 1);
    assert_eq!(draft.recipients.cc[0].email, "other@example.com");
    assert!(draft.content.plain_text.contains("Original body"));
    assert!(!draft.content.plain_text.contains("> Original body"));
    assert!(draft
        .content
        .editor_json
        .contains("nextmailOriginalMessage"));
    assert!(draft.content.html.contains("<p>Original body</p>"));
    let threading = repository
        .drafts()
        .draft_threading_headers("slot", &draft.id)
        .await
        .unwrap();
    assert_eq!(threading.in_reply_to.as_deref(), Some("child@example.com"));
    assert_eq!(
        threading.references,
        vec!["root@example.com", "child@example.com"]
    );

    assert!(repository
        .drafts()
        .discard_empty_draft("slot", &draft.id)
        .await
        .unwrap());
    assert_eq!(
        repository
            .drafts()
            .get_draft("account", "slot", &draft.id)
            .await
            .unwrap_err()
            .code,
        "draft.not_found"
    );

    let edited = repository
        .drafts()
        .persist_message_action_draft(PersistMessageActionDraftRequest {
            account_id: "account",
            account_slot_id: "slot",
            message_id: "message",
            action: MessageComposeAction::ReplyAll,
            draft: &composed,
        })
        .await
        .unwrap();
    repository
        .drafts()
        .save_draft(SaveDraftRequest {
            account_id: "account",
            account_slot_id: "slot",
            draft_id: &edited.id,
            recipients: &edited.recipients,
            subject: &edited.subject,
            content: &edited.content,
            expected_revision: edited.revision,
        })
        .await
        .unwrap();
    assert!(!repository
        .drafts()
        .discard_empty_draft("slot", &edited.id)
        .await
        .unwrap());
}

#[tokio::test]
async fn discards_only_completely_empty_drafts() {
    let directory = tempfile::tempdir().unwrap();
    initialize_content_database(directory.path()).await.unwrap();
    create_account_slot(directory.path(), "slot", 1)
        .await
        .unwrap();
    let repository = MailRepository::open(directory.path()).await.unwrap();
    let empty = repository
        .drafts()
        .create_draft("account", "slot")
        .await
        .unwrap();
    let discarded = repository
        .drafts()
        .discard_empty_draft("slot", &empty.id)
        .await
        .unwrap();
    assert!(discarded);
    assert_eq!(
        repository
            .drafts()
            .get_draft("account", "slot", &empty.id)
            .await
            .unwrap_err()
            .code,
        "draft.not_found"
    );

    let retained = repository
        .drafts()
        .create_draft("account", "slot")
        .await
        .unwrap();
    repository
        .drafts()
        .save_draft(SaveDraftRequest {
            account_id: "account",
            account_slot_id: "slot",
            draft_id: &retained.id,
            recipients: &DraftRecipientFields::default(),
            subject: "",
            content: &DraftContent {
                editor_json: r#"{"type":"doc","content":[{"type":"paragraph","content":[{"type":"text","text":"body"}]}]}"#.into(),
                html: "<p>body</p>".into(),
                plain_text: "body".into(),
            },
            expected_revision: retained.revision,
        })
        .await
        .unwrap();
    let discarded = repository
        .drafts()
        .discard_empty_draft("slot", &retained.id)
        .await
        .unwrap();
    assert!(!discarded);
    assert!(repository
        .drafts()
        .get_draft("account", "slot", &retained.id)
        .await
        .is_ok());
}

#[tokio::test]
async fn deletes_an_editing_draft_explicitly() {
    let directory = tempfile::tempdir().unwrap();
    initialize_content_database(directory.path()).await.unwrap();
    create_account_slot(directory.path(), "slot", 1)
        .await
        .unwrap();
    let repository = MailRepository::open(directory.path()).await.unwrap();
    let draft = repository
        .drafts()
        .create_draft("account", "slot")
        .await
        .unwrap();
    repository
        .drafts()
        .delete_editing_draft("slot", &draft.id)
        .await
        .unwrap();
    assert_eq!(
        repository
            .drafts()
            .get_draft("account", "slot", &draft.id)
            .await
            .unwrap_err()
            .code,
        "draft.not_found"
    );
}

#[tokio::test]
async fn draft_and_send_job_survive_repository_reopen() {
    let directory = tempfile::tempdir().unwrap();
    initialize_content_database(directory.path()).await.unwrap();
    create_account_slot(directory.path(), "slot", 1)
        .await
        .unwrap();
    let repository = MailRepository::open(directory.path()).await.unwrap();
    let draft = repository
        .drafts()
        .create_draft("account", "slot")
        .await
        .unwrap();
    let saved = repository
        .drafts()
        .save_draft(SaveDraftRequest {
            account_id: "account",
            account_slot_id: "slot",
            draft_id: &draft.id,
            recipients: &DraftRecipientFields {
                to: vec![MessageAddress {
                    name: Some("收件人".into()),
                    email: "to@example.com".into(),
                }],
                ..Default::default()
            },
            subject: "中文主题",
            content: &DraftContent {
                editor_json: "{}".into(),
                html: "<p>正文</p>".into(),
                plain_text: "正文".into(),
            },
            expected_revision: draft.revision,
        })
        .await
        .unwrap();
    assert_eq!(saved.subject, "中文主题");
    let drafts = repository
        .drafts()
        .list_editing_drafts("account", "slot")
        .await
        .unwrap();
    assert_eq!(drafts.len(), 1);
    assert_eq!(drafts[0].id, draft.id);
    repository
        .drafts()
        .add_draft_attachment("slot", &draft.id, "报告.txt", "text/plain", b"file")
        .await
        .unwrap();
    let mime_hash = repository
        .send_jobs()
        .write_send_mime(b"From: a@example.com\r\n\r\nbody")
        .await
        .unwrap();
    let job = repository
        .send_jobs()
        .queue_send_job(
            "account",
            "slot",
            &draft.id,
            &mime_hash,
            &["to@example.com".into()],
        )
        .await
        .unwrap();
    assert_eq!(job.status, SendJobStatus::Queued);
    drop(repository);

    let repository = MailRepository::open(directory.path()).await.unwrap();
    let claimed = repository
        .send_jobs()
        .claim_next_send_job()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(claimed.id, job.id);
    repository
        .send_jobs()
        .recover_interrupted_send_jobs()
        .await
        .unwrap();
    let reclaimed = repository
        .send_jobs()
        .claim_next_send_job()
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reclaimed.id, job.id);
    repository
        .send_jobs()
        .complete_send_job(&job.id)
        .await
        .unwrap();
    assert_eq!(
        repository
            .send_jobs()
            .get_send_job("account", "slot", &job.id)
            .await
            .unwrap()
            .status,
        SendJobStatus::Sent
    );
}

#[tokio::test]
async fn draft_attachments_are_isolated_by_account_slot() {
    let directory = tempfile::tempdir().unwrap();
    initialize_content_database(directory.path()).await.unwrap();
    create_account_slot(directory.path(), "slot-a", 1)
        .await
        .unwrap();
    create_account_slot(directory.path(), "slot-b", 2)
        .await
        .unwrap();
    let repository = MailRepository::open(directory.path()).await.unwrap();
    let draft = repository
        .drafts()
        .create_draft("account-a", "slot-a")
        .await
        .unwrap();
    let attachment = repository
        .drafts()
        .add_draft_attachment("slot-a", &draft.id, "private.txt", "text/plain", b"secret")
        .await
        .unwrap();
    let inline = repository
        .drafts()
        .add_draft_inline_image(
            "slot-a",
            &draft.id,
            "logo.png",
            "image/png",
            Some("logo@example.test"),
            b"image",
        )
        .await
        .unwrap();
    assert!(inline.is_inline);
    assert_eq!(inline.content_id.as_deref(), Some("logo@example.test"));

    assert!(repository
        .drafts()
        .draft_attachments("slot-b", &draft.id)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        repository
            .drafts()
            .remove_draft_attachment("slot-b", &draft.id, &attachment.id)
            .await
            .unwrap_err()
            .code,
        "draft.attachment_not_found"
    );
    assert_eq!(
        repository
            .drafts()
            .draft_attachments("slot-a", &draft.id)
            .await
            .unwrap()
            .len(),
        2
    );
    repository
        .drafts()
        .remove_draft_attachment("slot-a", &draft.id, &attachment.id)
        .await
        .unwrap();
    let remaining = repository
        .drafts()
        .draft_attachments("slot-a", &draft.id)
        .await
        .unwrap();
    assert_eq!(remaining.len(), 1);
    assert_eq!(remaining[0].summary.id, inline.id);
}

#[tokio::test]
async fn per_account_send_claims_preserve_fifo_and_do_not_cross_slots() {
    let directory = tempfile::tempdir().unwrap();
    initialize_content_database(directory.path()).await.unwrap();
    create_account_slot(directory.path(), "slot-a", 1)
        .await
        .unwrap();
    create_account_slot(directory.path(), "slot-b", 2)
        .await
        .unwrap();
    let repository = MailRepository::open(directory.path()).await.unwrap();
    let draft_a1 = repository
        .drafts()
        .create_draft("account-a", "slot-a")
        .await
        .unwrap();
    let draft_a2 = repository
        .drafts()
        .create_draft("account-a", "slot-a")
        .await
        .unwrap();
    let draft_b = repository
        .drafts()
        .create_draft("account-b", "slot-b")
        .await
        .unwrap();
    let mime_hash = repository
        .send_jobs()
        .write_send_mime(b"From: a@example.com\r\n\r\nbody")
        .await
        .unwrap();
    let job_a1 = repository
        .send_jobs()
        .queue_send_job(
            "account-a",
            "slot-a",
            &draft_a1.id,
            &mime_hash,
            &["to@example.com".to_owned()],
        )
        .await
        .unwrap();
    let job_a2 = repository
        .send_jobs()
        .queue_send_job(
            "account-a",
            "slot-a",
            &draft_a2.id,
            &mime_hash,
            &["to@example.com".to_owned()],
        )
        .await
        .unwrap();
    let job_b = repository
        .send_jobs()
        .queue_send_job(
            "account-b",
            "slot-b",
            &draft_b.id,
            &mime_hash,
            &["to@example.com".to_owned()],
        )
        .await
        .unwrap();

    let slots = repository
        .send_jobs()
        .ready_send_account_slots()
        .await
        .unwrap();
    assert_eq!(slots, vec!["slot-a", "slot-b"]);
    assert_eq!(
        repository
            .send_jobs()
            .claim_next_send_job_for_account("slot-a")
            .await
            .unwrap()
            .unwrap()
            .id,
        job_a1.id
    );
    assert_eq!(
        repository
            .send_jobs()
            .claim_next_send_job_for_account("slot-a")
            .await
            .unwrap()
            .unwrap()
            .id,
        job_a2.id
    );
    assert_eq!(
        repository
            .send_jobs()
            .claim_next_send_job_for_account("slot-b")
            .await
            .unwrap()
            .unwrap()
            .id,
        job_b.id
    );
    assert!(repository
        .send_jobs()
        .claim_next_send_job_for_account("slot-b")
        .await
        .unwrap()
        .is_none());
}
