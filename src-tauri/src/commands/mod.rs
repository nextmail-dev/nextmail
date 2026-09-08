use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State, WebviewUrl, WebviewWindow, WebviewWindowBuilder};

use crate::{
    domain::{
        AccountConnectionDraft, AccountDraft, AccountManagementDetail, AccountRemovalImpact,
        AccountRuntimeSummary, AccountSummary, AddressPresentation, AppAbout,
        AppearancePreferences, AttachmentSummary, BootstrapStatus, ComposerBootstrap,
        CompositionSceneRule, CompositionSceneRuleDraft, ConnectionTestResult, ContactDetail,
        ContactDraft, ContactGroupDetail, ContactGroupDraft, ContactGroupSummary, ContactListPage,
        ContactSuggestions, ContactSummary, DataDirectoryValidation, DesktopPreferences,
        DiscoveredAccountConfig, DraftAttachmentSummary, DraftContent, DraftDetail, DraftListItem,
        DraftRecipientFields, MailSignature, MailSignatureDraft, MailTemplate, MailTemplateDraft,
        MailboxRole, MailboxSummary, MainCloseAction, MessageAddress, MessageComposeAction,
        MessageDetail, MessageListPage, NewMailNotification, NotificationPreferences,
        PendingOperationSummary, PreparedInlineImage, ReadingPreferences, RenderedMailSignature,
        RenderedMailTemplate, SendJobSummary, SignaturePreferences, SignaturePreferencesDraft,
        SyncInterval, SyncProgress, UpdateCheckResult,
    },
    error::CommandResult,
    state::AppState,
    tray_runtime, updater_runtime,
    window_titles::{update_open_window_titles, window_title, WindowTitleKind},
};

mod accounts;
mod composer;
mod mail;
mod system;
mod windows;

pub use accounts::*;
pub use composer::*;
pub use mail::*;
pub use system::*;
pub use windows::*;

fn emit_accounts_changed(app: &AppHandle, revision: u64) {
    emit_or_log(app, "accounts-changed", AccountsChangedEvent { revision });
}

fn emit_composition_definitions_changed(
    app: &AppHandle,
    account_id: Option<String>,
    kind: &'static str,
) {
    emit_or_log(
        app,
        "composition-definitions-changed",
        CompositionDefinitionsChangedEvent { account_id, kind },
    );
}

fn emit_or_log<S: Serialize + Clone>(app: &AppHandle, event: &'static str, payload: S) {
    if let Err(error) = app.emit(event, payload) {
        tracing::warn!(%event, ?error, "application event emission failed");
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountsChangedEvent {
    revision: u64,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct AccountRemovingEvent {
    account_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompositionDefinitionsChangedEvent {
    account_id: Option<String>,
    kind: &'static str,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct RawMessageWindowLocation {
    account_id: String,
    message_id: String,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct MessagePreviewWindowLocation {
    account_id: String,
    mailbox_id: String,
    message_id: String,
}
