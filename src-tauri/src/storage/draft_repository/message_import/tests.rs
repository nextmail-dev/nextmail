use super::*;
use crate::{
    application::compose_imported_draft,
    storage::{create_account_slot, initialize_content_database, MailRepository},
};

async fn fixture(role: &str) -> (tempfile::TempDir, MailRepository, ImportedDraftSource) {
    let directory = tempfile::tempdir().unwrap();
    initialize_content_database(directory.path()).await.unwrap();
    create_account_slot(directory.path(), "slot", 1)
        .await
        .unwrap();
    create_account_slot(directory.path(), "other", 1)
        .await
        .unwrap();
    let repository = MailRepository::open(directory.path()).await.unwrap();
    sqlx::query("INSERT INTO messages(id, account_slot_id, subject, received_at) VALUES ('message', 'slot', 'Original', 1)").execute(&repository.pool).await.unwrap();
    sqlx::query("INSERT INTO mailboxes(id, account_slot_id, remote_name, display_name, role) VALUES ('box', 'slot', 'Box', 'Box', ?)").bind(role).execute(&repository.pool).await.unwrap();
    sqlx::query("INSERT INTO message_locations(id, message_id, mailbox_id, uid, uid_validity) VALUES ('location', 'message', 'box', 1, 1)").execute(&repository.pool).await.unwrap();
    let source = ImportedDraftSource {
        recipients: DraftRecipientFields {
            to: vec![MessageAddress {
                name: Some("Recipient".into()),
                email: "to@example.com".into(),
            }],
            cc: vec![MessageAddress {
                name: None,
                email: "cc@example.com".into(),
            }],
            bcc: vec![MessageAddress {
                name: None,
                email: "bcc@example.com".into(),
            }],
        },
        subject: "Original".into(),
        plain_text: "Body".into(),
        safe_html: Some(r#"<p><b>Body</b><img src="cid:image"></p>"#.into()),
    };
    (directory, repository, source)
}

#[tokio::test]
async fn both_import_modes_copy_all_content_but_only_server_drafts_retain_source_identity() {
    for role in [MailboxRole::Drafts, MailboxRole::Sent] {
        let (_dir, repository, source) = fixture(if role == MailboxRole::Drafts {
            "drafts"
        } else {
            "sent"
        })
        .await;
        let content = compose_imported_draft(&source).unwrap();
        let attachments = vec![
            ImportedDraftAttachment {
                file_name: "report.txt".into(),
                content_type: "text/plain".into(),
                bytes: b"report".to_vec(),
                content_id: None,
            },
            ImportedDraftAttachment {
                file_name: "image.png".into(),
                content_type: "image/png".into(),
                bytes: vec![1, 2, 3],
                content_id: Some("image".into()),
            },
        ];
        let drafts = repository.drafts();
        let draft = drafts
            .persist_imported_draft(PersistImportedDraftRequest {
                account_id: "account",
                account_slot_id: "slot",
                message_id: "message",
                mailbox_id: "box",
                source_role: role.clone(),
                source: &source,
                content: &content,
                attachments: &attachments,
            })
            .await
            .unwrap();
        assert_eq!(draft.subject, source.subject);
        assert_eq!(draft.recipients, source.recipients);
        assert_eq!(draft.content.html, content.html);
        assert_eq!(draft.attachments.len(), 2);
        for (stored, original) in drafts
            .draft_attachments("slot", &draft.id)
            .await
            .unwrap()
            .iter()
            .zip(&attachments)
        {
            assert_eq!(
                drafts.attachment_bytes(&stored.content_hash).await.unwrap(),
                original.bytes
            );
            assert_eq!(stored.summary.content_id, original.content_id);
        }
        let source_id: Option<String> =
            sqlx::query_scalar("SELECT source_message_id FROM drafts WHERE id = ?")
                .bind(&draft.id)
                .fetch_one(&repository.pool)
                .await
                .unwrap();
        assert_eq!(
            source_id.as_deref(),
            if role == MailboxRole::Drafts {
                Some("message")
            } else {
                None
            }
        );
        let thread = drafts
            .draft_threading_headers("slot", &draft.id)
            .await
            .unwrap();
        assert!(thread.in_reply_to.is_none() && thread.references.is_empty());
        let second = drafts
            .persist_imported_draft(PersistImportedDraftRequest {
                account_id: "account",
                account_slot_id: "slot",
                message_id: "message",
                mailbox_id: "box",
                source_role: role.clone(),
                source: &source,
                content: &content,
                attachments: &attachments,
            })
            .await
            .unwrap();
        assert_eq!(draft.id == second.id, role == MailboxRole::Drafts);
        drafts
            .delete_editing_draft("slot", &draft.id)
            .await
            .unwrap();
        let original: String =
            sqlx::query_scalar("SELECT subject FROM messages WHERE id = 'message'")
                .fetch_one(&repository.pool)
                .await
                .unwrap();
        assert_eq!(original, "Original");
    }
}

#[tokio::test]
async fn rejects_wrong_account_folder_role_and_rolls_back_incomplete_imports() {
    let (_dir, repository, source) = fixture("sent").await;
    let drafts = repository.drafts();
    for (slot, mailbox, role) in [
        ("other", "box", MailboxRole::Sent),
        ("slot", "missing", MailboxRole::Sent),
        ("slot", "box", MailboxRole::Drafts),
    ] {
        assert_eq!(
            drafts
                .ensure_import_source(slot, mailbox, "message", &role)
                .await
                .unwrap_err()
                .code,
            "draft.source_not_editable"
        );
    }
    let content = compose_imported_draft(&source).unwrap();
    let a = ImportedDraftAttachment {
        file_name: "image.png".into(),
        content_type: "image/png".into(),
        bytes: vec![1],
        content_id: Some("duplicate".into()),
    };
    let result = drafts
        .persist_imported_draft(PersistImportedDraftRequest {
            account_id: "account",
            account_slot_id: "slot",
            message_id: "message",
            mailbox_id: "box",
            source_role: MailboxRole::Sent,
            source: &source,
            content: &content,
            attachments: &[a.clone(), a],
        })
        .await;
    assert!(result.is_err());
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM drafts")
        .fetch_one(&repository.pool)
        .await
        .unwrap();
    assert_eq!(count, 0);
    sqlx::query("INSERT INTO mailboxes(id, account_slot_id, remote_name, display_name, role) VALUES ('mapped', 'slot', 'Mapped', 'Mapped', 'other')").execute(&repository.pool).await.unwrap();
    repository
        .mailbox_roles()
        .set_mailbox_role_mapping("slot", MailboxRole::Sent, Some("mapped"))
        .await
        .unwrap();
    assert!(drafts
        .ensure_import_source("slot", "box", "message", &MailboxRole::Sent)
        .await
        .is_err());
}

#[tokio::test]
async fn recovers_local_bcc_only_for_identical_mime_in_the_same_account() {
    let (_dir, repository, source) = fixture("sent").await;
    let drafts = repository.drafts();
    let local = drafts
        .create_initialized_draft_with_recipients(
            "account",
            "slot",
            &source.recipients,
            "Original",
            &compose_imported_draft(&source).unwrap(),
        )
        .await
        .unwrap();
    sqlx::query("INSERT INTO send_jobs(id, draft_id, account_slot_id, mime_hash, envelope_recipients_json, status, created_at, updated_at) VALUES ('job', ?, 'slot', 'hash', '[]', 'sent', 1, 1)").bind(&local.id).execute(&repository.pool).await.unwrap();
    sqlx::query("UPDATE messages SET raw_content_hash = 'hash' WHERE id = 'message'")
        .execute(&repository.pool)
        .await
        .unwrap();
    assert_eq!(
        drafts.locally_sent_bcc("slot", "message").await.unwrap(),
        Some(source.recipients.bcc)
    );
    assert!(drafts
        .locally_sent_bcc("other", "message")
        .await
        .unwrap()
        .is_none());
    sqlx::query("UPDATE messages SET raw_content_hash = 'different' WHERE id = 'message'")
        .execute(&repository.pool)
        .await
        .unwrap();
    assert!(drafts
        .locally_sent_bcc("slot", "message")
        .await
        .unwrap()
        .is_none());
}
