use crate::{
    application::{
        normalize_mail_signature_draft, normalize_mail_template_draft, render_mail_signature,
        render_mail_template,
    },
    core::{
        CommandResult, CompositionSceneRule, CompositionSceneRuleDraft, DraftRecipientFields,
        MailSignature, MailSignatureDraft, MailTemplate, MailTemplateDraft, RenderedMailSignature,
        RenderedMailTemplate, SignaturePreferences, SignaturePreferencesDraft,
    },
};

use super::{sanitize_draft_content, ComposerRuntime};

impl ComposerRuntime {
    pub async fn list_mail_templates(
        &self,
        account_id: Option<&str>,
    ) -> CommandResult<Vec<MailTemplate>> {
        let account = self.definition_account(account_id)?;
        self.repository()
            .await?
            .composition_definitions()
            .list_mail_templates(
                account.as_ref().map(|value| value.id.as_str()),
                account.as_ref().map(|value| value.data_slot_id.as_str()),
            )
            .await
    }

    pub async fn create_mail_template(
        &self,
        account_id: Option<&str>,
        draft: MailTemplateDraft,
    ) -> CommandResult<MailTemplate> {
        let mut draft = draft;
        draft.content = sanitize_draft_content(draft.content)?;
        let draft = normalize_mail_template_draft(draft)?;
        let account = self.definition_account(account_id)?;
        self.repository()
            .await?
            .composition_definitions()
            .create_mail_template(
                account.as_ref().map(|value| value.id.as_str()),
                account.as_ref().map(|value| value.data_slot_id.as_str()),
                &draft,
            )
            .await
    }

    pub async fn update_mail_template(
        &self,
        account_id: Option<&str>,
        template_id: &str,
        draft: MailTemplateDraft,
        expected_revision: u64,
    ) -> CommandResult<MailTemplate> {
        let mut draft = draft;
        draft.content = sanitize_draft_content(draft.content)?;
        let draft = normalize_mail_template_draft(draft)?;
        let account = self.definition_account(account_id)?;
        self.repository()
            .await?
            .composition_definitions()
            .update_mail_template(
                account.as_ref().map(|value| value.id.as_str()),
                account.as_ref().map(|value| value.data_slot_id.as_str()),
                template_id,
                &draft,
                expected_revision,
            )
            .await
    }

    pub async fn delete_mail_template(
        &self,
        account_id: Option<&str>,
        template_id: &str,
        expected_revision: u64,
    ) -> CommandResult<()> {
        let account = self.definition_account(account_id)?;
        self.repository()
            .await?
            .composition_definitions()
            .delete_mail_template(
                account.as_ref().map(|value| value.id.as_str()),
                account.as_ref().map(|value| value.data_slot_id.as_str()),
                template_id,
                expected_revision,
            )
            .await
    }

    pub async fn list_mail_signatures(
        &self,
        account_id: Option<&str>,
    ) -> CommandResult<Vec<MailSignature>> {
        let account = self.definition_account(account_id)?;
        self.repository()
            .await?
            .composition_definitions()
            .list_mail_signatures(
                account.as_ref().map(|value| value.id.as_str()),
                account.as_ref().map(|value| value.data_slot_id.as_str()),
            )
            .await
    }

    pub async fn create_mail_signature(
        &self,
        account_id: Option<&str>,
        draft: MailSignatureDraft,
    ) -> CommandResult<MailSignature> {
        let mut draft = draft;
        draft.content = sanitize_draft_content(draft.content)?;
        let draft = normalize_mail_signature_draft(draft)?;
        let account = self.definition_account(account_id)?;
        self.repository()
            .await?
            .composition_definitions()
            .create_mail_signature(
                account.as_ref().map(|value| value.id.as_str()),
                account.as_ref().map(|value| value.data_slot_id.as_str()),
                &draft,
            )
            .await
    }

    pub async fn update_mail_signature(
        &self,
        account_id: Option<&str>,
        signature_id: &str,
        draft: MailSignatureDraft,
        expected_revision: u64,
    ) -> CommandResult<MailSignature> {
        let mut draft = draft;
        draft.content = sanitize_draft_content(draft.content)?;
        let draft = normalize_mail_signature_draft(draft)?;
        let account = self.definition_account(account_id)?;
        self.repository()
            .await?
            .composition_definitions()
            .update_mail_signature(
                account.as_ref().map(|value| value.id.as_str()),
                account.as_ref().map(|value| value.data_slot_id.as_str()),
                signature_id,
                &draft,
                expected_revision,
            )
            .await
    }

    pub async fn delete_mail_signature(
        &self,
        account_id: Option<&str>,
        signature_id: &str,
        expected_revision: u64,
    ) -> CommandResult<()> {
        let account = self.definition_account(account_id)?;
        self.repository()
            .await?
            .composition_definitions()
            .delete_mail_signature(
                account.as_ref().map(|value| value.id.as_str()),
                account.as_ref().map(|value| value.data_slot_id.as_str()),
                signature_id,
                expected_revision,
            )
            .await
    }

    pub async fn get_signature_preferences(
        &self,
        account_id: Option<&str>,
    ) -> CommandResult<SignaturePreferences> {
        let account = self.definition_account(account_id)?;
        self.repository()
            .await?
            .composition_definitions()
            .signature_preferences(account.as_ref().map(|value| value.data_slot_id.as_str()))
            .await
    }

    pub async fn save_signature_preferences(
        &self,
        account_id: Option<&str>,
        draft: SignaturePreferencesDraft,
        expected_revision: u64,
    ) -> CommandResult<SignaturePreferences> {
        let account = self.definition_account(account_id)?;
        self.repository()
            .await?
            .composition_definitions()
            .save_signature_preferences(
                account.as_ref().map(|value| value.data_slot_id.as_str()),
                &draft,
                expected_revision,
            )
            .await
    }

    pub async fn list_composition_scene_rules(
        &self,
        account_id: Option<&str>,
    ) -> CommandResult<Vec<CompositionSceneRule>> {
        let account = self.definition_account(account_id)?;
        self.repository()
            .await?
            .composition_definitions()
            .list_composition_scene_rules(account.as_ref().map(|value| value.data_slot_id.as_str()))
            .await
    }

    pub async fn save_composition_scene_rule(
        &self,
        account_id: Option<&str>,
        mut draft: CompositionSceneRuleDraft,
        expected_revision: u64,
    ) -> CommandResult<CompositionSceneRule> {
        draft.signature_id = None;
        let account = self.definition_account(account_id)?;
        self.repository()
            .await?
            .composition_definitions()
            .save_composition_scene_rule(
                account.as_ref().map(|value| value.id.as_str()),
                account.as_ref().map(|value| value.data_slot_id.as_str()),
                &draft,
                expected_revision,
            )
            .await
    }

    pub async fn render_mail_template(
        &self,
        account_id: &str,
        template_id: &str,
        recipients: DraftRecipientFields,
    ) -> CommandResult<RenderedMailTemplate> {
        let account = self.service.account_record(account_id)?;
        let template = self
            .repository()
            .await?
            .composition_definitions()
            .available_mail_template(account_id, &account.data_slot_id, template_id)
            .await?;
        let recipient = template
            .recipients
            .as_ref()
            .and_then(|recipients| recipients.to.first())
            .or_else(|| recipients.to.first());
        render_mail_template(&template, &self.render_context(&account, recipient)?)
    }

    pub async fn render_mail_signature(
        &self,
        account_id: &str,
        signature_id: &str,
        recipients: DraftRecipientFields,
    ) -> CommandResult<RenderedMailSignature> {
        let account = self.service.account_record(account_id)?;
        let signature = self
            .repository()
            .await?
            .composition_definitions()
            .available_mail_signature(account_id, &account.data_slot_id, signature_id)
            .await?;
        render_mail_signature(
            &signature,
            &self.render_context(&account, recipients.to.first())?,
        )
    }
}
