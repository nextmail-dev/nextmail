use super::*;
use crate::core::{DraftRecipientFields, MessageAddress};
use crate::storage::{create_account_slot, initialize_content_database, MailRepository};

fn content(value: &str) -> DraftContent {
    DraftContent {
        editor_json: format!(
            r#"{{"type":"doc","content":[{{"type":"paragraph","content":[{{"type":"text","text":"{value}"}}]}}]}}"#
        ),
        html: format!("<p>{value}</p>"),
        plain_text: value.to_owned(),
    }
}

#[tokio::test]
async fn keeps_global_and_account_definitions_in_explicit_scopes() {
    let directory = tempfile::tempdir().expect("temporary directory");
    initialize_content_database(directory.path())
        .await
        .expect("initialize database");
    let repository = MailRepository::open(directory.path())
        .await
        .expect("repository");
    create_account_slot(directory.path(), "slot-one", 1)
        .await
        .expect("first slot");
    create_account_slot(directory.path(), "slot-two", 2)
        .await
        .expect("second slot");
    let definitions = repository.composition_definitions();

    definitions
        .create_mail_template(
            None,
            None,
            &MailTemplateDraft {
                name: "Global".to_owned(),
                subject: "Hello".to_owned(),
                recipients: DraftRecipientFields {
                    to: vec![MessageAddress {
                        name: Some("Recipient".to_owned()),
                        email: "recipient@example.com".to_owned(),
                    }],
                    cc: Vec::new(),
                    bcc: Vec::new(),
                },
                content: content("Shared body"),
            },
        )
        .await
        .expect("global template");
    definitions
        .create_mail_template(
            Some("account-one"),
            Some("slot-one"),
            &MailTemplateDraft {
                name: "Account".to_owned(),
                subject: String::new(),
                recipients: Default::default(),
                content: content("Private body"),
            },
        )
        .await
        .expect("account template");

    let global = definitions
        .list_mail_templates(None, None)
        .await
        .expect("global list");
    let first = definitions
        .list_mail_templates(Some("account-one"), Some("slot-one"))
        .await
        .expect("first account list");
    let second = definitions
        .list_mail_templates(Some("account-two"), Some("slot-two"))
        .await
        .expect("second account list");

    assert_eq!(global.len(), 1);
    assert_eq!(global[0].scope, CompositionDefinitionScope::Global);
    assert_eq!(
        global[0].recipients.as_ref().unwrap().to[0].email,
        "recipient@example.com"
    );
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].account_id.as_deref(), Some("account-one"));
    assert!(second.is_empty());
}

#[tokio::test]
async fn persists_signature_updates_and_rejects_stale_revisions() {
    let directory = tempfile::tempdir().expect("temporary directory");
    initialize_content_database(directory.path())
        .await
        .expect("initialize database");
    let repository = MailRepository::open(directory.path())
        .await
        .expect("repository");
    let definitions = repository.composition_definitions();
    let signature = definitions
        .create_mail_signature(
            None,
            None,
            &MailSignatureDraft {
                name: "Default".to_owned(),
                content: content("Alice"),
            },
        )
        .await
        .expect("signature");
    let initial_preferences = definitions
        .signature_preferences(None)
        .await
        .expect("initial signature preferences");
    assert_eq!(
        initial_preferences.default_signature_id.as_deref(),
        Some(signature.id.as_str())
    );
    assert!(initial_preferences.auto_insert);
    let saved_preferences = definitions
        .save_signature_preferences(
            None,
            &SignaturePreferencesDraft {
                default_signature_id: Some(signature.id.clone()),
                auto_insert: false,
                inherit: false,
            },
            initial_preferences.revision,
        )
        .await
        .expect("disable automatic signature");
    assert!(!saved_preferences.auto_insert);
    let stale_preferences = definitions
        .save_signature_preferences(
            None,
            &SignaturePreferencesDraft {
                default_signature_id: Some(signature.id.clone()),
                auto_insert: true,
                inherit: false,
            },
            initial_preferences.revision,
        )
        .await
        .expect_err("stale signature preferences");
    assert_eq!(
        stale_preferences.code,
        "signature_preferences.revision_conflict"
    );
    let updated = definitions
        .update_mail_signature(
            None,
            None,
            &signature.id,
            &MailSignatureDraft {
                name: "Primary".to_owned(),
                content: content("Alice Example"),
            },
            signature.revision,
        )
        .await
        .expect("update signature");

    let stale = definitions
        .update_mail_signature(
            None,
            None,
            &signature.id,
            &MailSignatureDraft {
                name: "Stale".to_owned(),
                content: content("Old"),
            },
            signature.revision,
        )
        .await
        .expect_err("stale revision");
    assert_eq!(stale.code, "signature.revision_conflict");

    drop(repository);
    let reopened = MailRepository::open(directory.path())
        .await
        .expect("reopened repository");
    let stored = reopened
        .composition_definitions()
        .list_mail_signatures(None, None)
        .await
        .expect("stored signatures");
    assert_eq!(stored[0].name, "Primary");
    assert_eq!(stored[0].revision, updated.revision);
    let stored_preferences = reopened
        .composition_definitions()
        .signature_preferences(None)
        .await
        .expect("stored signature preferences");
    assert_eq!(
        stored_preferences.default_signature_id.as_deref(),
        Some(signature.id.as_str())
    );
    assert!(!stored_preferences.auto_insert);
}

#[tokio::test]
async fn account_signature_preferences_inherit_until_the_first_local_signature() {
    let directory = tempfile::tempdir().expect("temporary directory");
    initialize_content_database(directory.path())
        .await
        .expect("initialize database");
    let repository = MailRepository::open(directory.path())
        .await
        .expect("repository");
    create_account_slot(directory.path(), "slot-one", 1)
        .await
        .expect("account slot");
    let definitions = repository.composition_definitions();
    let global = definitions
        .create_mail_signature(
            None,
            None,
            &MailSignatureDraft {
                name: "Shared".to_owned(),
                content: content("Shared signature"),
            },
        )
        .await
        .expect("global signature");

    let inherited = definitions
        .signature_preferences(Some("slot-one"))
        .await
        .expect("inherited preferences");
    assert!(inherited.inherited);
    assert_eq!(
        inherited.default_signature_id.as_deref(),
        Some(global.id.as_str())
    );

    let local = definitions
        .create_mail_signature(
            Some("account-one"),
            Some("slot-one"),
            &MailSignatureDraft {
                name: "Account".to_owned(),
                content: content("Account signature"),
            },
        )
        .await
        .expect("account signature");
    let account_preferences = definitions
        .signature_preferences(Some("slot-one"))
        .await
        .expect("account preferences");
    assert!(!account_preferences.inherited);
    assert_eq!(
        account_preferences.default_signature_id.as_deref(),
        Some(local.id.as_str())
    );
}

#[tokio::test]
async fn resolves_account_rules_over_global_rules_and_protects_references() {
    let directory = tempfile::tempdir().expect("temporary directory");
    initialize_content_database(directory.path())
        .await
        .expect("initialize database");
    let repository = MailRepository::open(directory.path())
        .await
        .expect("repository");
    create_account_slot(directory.path(), "slot-one", 1)
        .await
        .expect("account slot");
    let definitions = repository.composition_definitions();
    let template = definitions
        .create_mail_template(
            None,
            None,
            &MailTemplateDraft {
                name: "Shared".to_owned(),
                subject: String::new(),
                recipients: Default::default(),
                content: content("Shared"),
            },
        )
        .await
        .expect("template");
    let global = definitions
        .save_composition_scene_rule(
            None,
            None,
            &CompositionSceneRuleDraft {
                scene: CompositionScene::New,
                template_id: Some(template.id.clone()),
                signature_id: None,
                inherit: false,
            },
            0,
        )
        .await
        .expect("global rule");
    assert_eq!(global.revision, 1);
    for scene in [
        CompositionScene::Reply,
        CompositionScene::ReplyAll,
        CompositionScene::Forward,
    ] {
        definitions
            .save_composition_scene_rule(
                None,
                None,
                &CompositionSceneRuleDraft {
                    scene,
                    template_id: None,
                    signature_id: None,
                    inherit: false,
                },
                0,
            )
            .await
            .expect("global scene rule");
    }
    let global_rules = definitions
        .list_composition_scene_rules(None)
        .await
        .expect("four global rules");
    assert_eq!(global_rules.len(), 4);
    assert!(global_rules.iter().all(|value| value.revision == 1));

    let inherited = definitions
        .list_composition_scene_rules(Some("slot-one"))
        .await
        .expect("account rules");
    let inherited_new = inherited
        .iter()
        .find(|value| value.scene == CompositionScene::New)
        .expect("new rule");
    assert!(inherited_new.inherited);
    assert_eq!(inherited_new.revision, 0);
    assert_eq!(
        inherited_new.template_id.as_deref(),
        Some(template.id.as_str())
    );

    let account = definitions
        .save_composition_scene_rule(
            Some("account-one"),
            Some("slot-one"),
            &CompositionSceneRuleDraft {
                scene: CompositionScene::New,
                template_id: None,
                signature_id: None,
                inherit: false,
            },
            0,
        )
        .await
        .expect("account override");
    assert!(!account.inherited);
    let resolved = definitions
        .resolved_composition_scene_rule("slot-one", CompositionScene::New)
        .await
        .expect("resolved account rule");
    assert_eq!(resolved.template_id, None);

    let protected = definitions
        .delete_mail_template(None, None, &template.id, template.revision)
        .await
        .expect_err("referenced template");
    assert_eq!(protected.code, "template.in_use");
}
