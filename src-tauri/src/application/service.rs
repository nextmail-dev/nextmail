use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{
    adapters::{
        cleanup_initialized_files, create_account_slot, delete_account_slot,
        discover_account_config, initialize_content_database, read_data_marker, write_data_marker,
        AppPaths, ConnectionTester, CredentialStore, CONTENT_DATABASE_FILENAME,
        DATA_MARKER_FILENAME,
    },
    core::{
        AccountsConfigStore, AppearancePreferencesStore, BootstrapConfigStore,
        NotificationPreferencesConfigStore, ReadingPreferencesConfigStore,
    },
    domain::{
        AccountConnectionDraft, AccountDraft, AccountRecord, AccountSummary, AccountsFile,
        AppearancePreferences, BootstrapConfig, BootstrapStage, BootstrapStatus,
        ConnectionTestResult, DataDirectoryMarker, DataDirectoryValidation,
        DiscoveredAccountConfig, NotificationPreferences, ReadingPreferences,
    },
    error::{CommandError, CommandResult},
};

pub struct AppService {
    paths: AppPaths,
    bootstrap: Arc<dyn BootstrapConfigStore>,
    accounts: Arc<dyn AccountsConfigStore>,
    preferences: Arc<dyn AppearancePreferencesStore>,
    reading_preferences: Arc<dyn ReadingPreferencesConfigStore>,
    notification_preferences: Arc<dyn NotificationPreferencesConfigStore>,
    credentials: Arc<dyn CredentialStore>,
    connection_tester: Arc<dyn ConnectionTester>,
    account_mutation: Mutex<()>,
}

pub struct AppConfigStores {
    bootstrap: Arc<dyn BootstrapConfigStore>,
    accounts: Arc<dyn AccountsConfigStore>,
    preferences: Arc<dyn AppearancePreferencesStore>,
    reading_preferences: Arc<dyn ReadingPreferencesConfigStore>,
    notification_preferences: Arc<dyn NotificationPreferencesConfigStore>,
}

impl AppConfigStores {
    pub fn new(
        bootstrap: Arc<dyn BootstrapConfigStore>,
        accounts: Arc<dyn AccountsConfigStore>,
        preferences: Arc<dyn AppearancePreferencesStore>,
        reading_preferences: Arc<dyn ReadingPreferencesConfigStore>,
        notification_preferences: Arc<dyn NotificationPreferencesConfigStore>,
    ) -> Self {
        Self {
            bootstrap,
            accounts,
            preferences,
            reading_preferences,
            notification_preferences,
        }
    }
}

impl AppService {
    pub fn new(
        paths: AppPaths,
        stores: AppConfigStores,
        credentials: Arc<dyn CredentialStore>,
        connection_tester: Arc<dyn ConnectionTester>,
    ) -> Self {
        Self {
            bootstrap: stores.bootstrap,
            accounts: stores.accounts,
            preferences: stores.preferences,
            reading_preferences: stores.reading_preferences,
            notification_preferences: stores.notification_preferences,
            paths,
            credentials,
            connection_tester,
            account_mutation: Mutex::new(()),
        }
    }

    pub fn get_bootstrap_status(&self) -> CommandResult<BootstrapStatus> {
        let accounts_file = self.accounts.load()?;
        let accounts = accounts_file
            .accounts
            .iter()
            .map(AccountSummary::from)
            .collect::<Vec<_>>();
        let Some(config) = self.bootstrap.load()? else {
            return Ok(BootstrapStatus {
                stage: BootstrapStage::NeedsDataDirectory,
                default_data_dir: self.paths.default_data_dir.clone(),
                configured_data_dir: None,
                accounts,
                last_selected_account_id: None,
            });
        };

        let directory_ready = is_compatible_data_directory(&config.data_dir);
        let stage = if !directory_ready {
            BootstrapStage::DataDirectoryMissing
        } else if !config.onboarding_completed {
            BootstrapStage::NeedsAccount
        } else {
            BootstrapStage::Ready
        };

        Ok(BootstrapStatus {
            stage,
            default_data_dir: self.paths.default_data_dir.clone(),
            configured_data_dir: Some(config.data_dir),
            accounts,
            last_selected_account_id: valid_selected_account_id(&accounts_file),
        })
    }

    pub fn validate_data_directory(&self, path: &str) -> DataDirectoryValidation {
        validate_data_directory(Path::new(path))
    }

    pub async fn initialize_data_directory(&self, path: &str) -> CommandResult<BootstrapStatus> {
        let data_dir = PathBuf::from(path.trim());
        let validation = validate_data_directory(&data_dir);
        if !validation.valid || !validation.can_initialize {
            return Err(CommandError::new(validation.message_code));
        }

        if validation.is_existing_dataset {
            let onboarding_completed = self
                .bootstrap
                .load()?
                .is_some_and(|config| config.onboarding_completed);
            self.bootstrap.save(&BootstrapConfig {
                data_dir,
                onboarding_completed,
            })?;
            return self.get_bootstrap_status();
        }

        fs::create_dir_all(&data_dir)
            .map_err(|_| CommandError::new("data_directory.create_failed"))?;
        let initialize_result = async {
            for name in ["raw", "attachments", "cache"] {
                fs::create_dir(data_dir.join(name))
                    .map_err(|_| CommandError::new("data_directory.create_failed"))?;
            }
            write_data_marker(
                &data_dir,
                &DataDirectoryMarker {
                    format_version: 1,
                    dataset_id: Uuid::new_v4().to_string(),
                },
            )?;
            initialize_content_database(&data_dir).await?;
            self.bootstrap.save(&BootstrapConfig {
                data_dir: data_dir.clone(),
                onboarding_completed: false,
            })?;
            Ok::<(), CommandError>(())
        }
        .await;

        if let Err(error) = initialize_result {
            cleanup_initialized_files(&data_dir);
            return Err(error);
        }
        self.get_bootstrap_status()
    }

    pub fn get_preferences(&self) -> CommandResult<AppearancePreferences> {
        self.preferences.load()
    }

    pub fn set_preferences(
        &self,
        preferences: AppearancePreferences,
    ) -> CommandResult<AppearancePreferences> {
        if !is_valid_accent_color(&preferences.accent_color) {
            return Err(CommandError::new("preferences.accent_invalid"));
        }
        self.preferences.save(&preferences)?;
        Ok(preferences)
    }

    pub fn get_reading_preferences(&self) -> CommandResult<ReadingPreferences> {
        self.reading_preferences.load()
    }

    pub fn set_reading_preferences(
        &self,
        preferences: ReadingPreferences,
    ) -> CommandResult<ReadingPreferences> {
        self.reading_preferences.save(&preferences)?;
        Ok(preferences)
    }

    pub fn get_notification_preferences(&self) -> CommandResult<NotificationPreferences> {
        let mut preferences = self.notification_preferences.load()?;
        let account_ids = self
            .accounts
            .load()?
            .accounts
            .into_iter()
            .map(|account| account.id)
            .collect::<std::collections::HashSet<_>>();
        preferences
            .accounts
            .retain(|setting| account_ids.contains(&setting.account_id));
        preferences
            .folders
            .retain(|setting| account_ids.contains(&setting.account_id));
        Ok(preferences)
    }

    pub fn set_notification_preferences(
        &self,
        preferences: NotificationPreferences,
    ) -> CommandResult<NotificationPreferences> {
        validate_notification_preferences(&preferences, &self.accounts.load()?)?;
        self.notification_preferences.save(&preferences)?;
        Ok(preferences)
    }

    pub async fn discover_account_config(
        &self,
        email: &str,
    ) -> CommandResult<DiscoveredAccountConfig> {
        discover_account_config(email).await
    }

    pub async fn test_account_connections(
        &self,
        draft: &AccountDraft,
    ) -> CommandResult<ConnectionTestResult> {
        self.connection_tester.test(draft).await
    }

    pub async fn add_password_account(&self, draft: AccountDraft) -> CommandResult<AccountSummary> {
        crate::adapters::parse_email(&draft.email)?;
        self.connection_tester.test(&draft).await?;

        let _guard = self.account_mutation.lock().await;
        let mut accounts_file = self.accounts.load()?;
        if has_duplicate_account(&accounts_file, &draft, None) {
            return Err(CommandError::new("account.duplicate"));
        }

        let bootstrap = self
            .bootstrap
            .load()?
            .ok_or_else(|| CommandError::new("data_directory.not_configured"))?;
        if !is_compatible_data_directory(&bootstrap.data_dir) {
            return Err(CommandError::new("data_directory.missing"));
        }

        let now = unix_timestamp();
        let account_id = Uuid::new_v4().to_string();
        let data_slot_id = Uuid::new_v4().to_string();
        let credential_ref = format!("account:{account_id}:password");

        create_account_slot(&bootstrap.data_dir, &data_slot_id, now).await?;
        if let Err(error) = self
            .credentials
            .set_password(&credential_ref, &draft.password)
            .await
        {
            delete_account_slot(&bootstrap.data_dir, &data_slot_id).await;
            return Err(error);
        }

        let record = AccountRecord {
            id: account_id,
            data_slot_id: data_slot_id.clone(),
            email: draft.email.trim().to_owned(),
            display_name: draft.display_name.trim().to_owned(),
            incoming: draft.incoming,
            outgoing: draft.outgoing,
            credential_ref: credential_ref.clone(),
            created_at: now,
        };
        let summary = AccountSummary::from(&record);
        accounts_file.accounts.push(record);
        accounts_file.revision = accounts_file.revision.saturating_add(1);
        if accounts_file.last_selected_account_id.is_none() {
            accounts_file.last_selected_account_id = Some(summary.id.clone());
        }

        if let Err(error) = self.accounts.save(&accounts_file) {
            if self
                .credentials
                .delete_password(&credential_ref)
                .await
                .is_err()
            {
                let mut cleanup_file = self.accounts.load().unwrap_or_default();
                push_cleanup_reference(&mut cleanup_file, credential_ref.clone());
                let _ = self.accounts.save(&cleanup_file);
            }
            delete_account_slot(&bootstrap.data_dir, &data_slot_id).await;
            return Err(error);
        }
        Ok(summary)
    }

    pub fn get_account_connection_draft(
        &self,
        account_id: &str,
    ) -> CommandResult<AccountConnectionDraft> {
        let account = self.account_record(account_id)?;
        Ok(AccountConnectionDraft {
            email: account.email,
            display_name: account.display_name,
            incoming: account.incoming,
            outgoing: account.outgoing,
            insecure_acknowledged: false,
        })
    }

    pub async fn update_password_account(
        &self,
        account_id: &str,
        draft: AccountConnectionDraft,
        new_password: Option<String>,
    ) -> CommandResult<AccountSummary> {
        crate::adapters::parse_email(&draft.email)?;
        let _guard = self.account_mutation.lock().await;
        let mut accounts_file = self.accounts.load()?;
        let current = accounts_file
            .accounts
            .iter()
            .find(|account| account.id == account_id)
            .cloned()
            .ok_or_else(|| CommandError::new("account.not_found"))?;
        let password = match new_password.as_deref() {
            Some(value) if !value.is_empty() => value.to_owned(),
            Some(_) => return Err(CommandError::new("account.password_required")),
            None => {
                self.credentials
                    .get_password(&current.credential_ref)
                    .await?
            }
        };
        let candidate = draft.with_password(password.clone());
        self.connection_tester.test(&candidate).await?;
        if has_duplicate_account(&accounts_file, &candidate, Some(account_id)) {
            return Err(CommandError::new("account.duplicate"));
        }

        let replacement_ref = new_password
            .as_ref()
            .map(|_| format!("account:{account_id}:password:{}", Uuid::new_v4()));
        if let Some(reference) = replacement_ref.as_deref() {
            self.credentials.set_password(reference, &password).await?;
        }

        let updated = AccountRecord {
            id: current.id.clone(),
            data_slot_id: current.data_slot_id.clone(),
            email: draft.email.trim().to_owned(),
            display_name: draft.display_name.trim().to_owned(),
            incoming: draft.incoming,
            outgoing: draft.outgoing,
            credential_ref: replacement_ref
                .clone()
                .unwrap_or_else(|| current.credential_ref.clone()),
            created_at: current.created_at,
        };
        let Some(index) = accounts_file
            .accounts
            .iter()
            .position(|account| account.id == account_id)
        else {
            return Err(CommandError::new("account.not_found"));
        };
        accounts_file.accounts[index] = updated.clone();
        accounts_file.revision = accounts_file.revision.saturating_add(1);
        if replacement_ref.is_some() {
            push_cleanup_reference(&mut accounts_file, current.credential_ref.clone());
        }
        if let Err(error) = self.accounts.save(&accounts_file) {
            if let Some(reference) = replacement_ref.as_deref() {
                let _ = self.credentials.delete_password(reference).await;
            }
            return Err(error);
        }

        if replacement_ref.is_some()
            && self
                .credentials
                .delete_password(&current.credential_ref)
                .await
                .is_ok()
        {
            remove_cleanup_reference(&mut accounts_file, &current.credential_ref);
            // The configuration already points at the new credential. Keeping a stale,
            // idempotent cleanup reference is safer than reporting the committed update as failed.
            let _ = self.accounts.save(&accounts_file);
        }
        Ok(AccountSummary::from(&updated))
    }

    pub async fn reauthenticate_password_account(
        &self,
        account_id: &str,
        password: String,
    ) -> CommandResult<AccountSummary> {
        let draft = self.get_account_connection_draft(account_id)?;
        self.update_password_account(account_id, draft, Some(password))
            .await
    }

    pub async fn remove_account(&self, account_id: &str) -> CommandResult<u64> {
        let _guard = self.account_mutation.lock().await;
        let mut accounts_file = self.accounts.load()?;
        let index = accounts_file
            .accounts
            .iter()
            .position(|account| account.id == account_id)
            .ok_or_else(|| CommandError::new("account.not_found"))?;
        let removed = accounts_file.accounts.remove(index);
        accounts_file.revision = accounts_file.revision.saturating_add(1);
        if accounts_file.last_selected_account_id.as_deref() == Some(account_id) {
            accounts_file.last_selected_account_id = accounts_file
                .accounts
                .first()
                .map(|account| account.id.clone());
        }
        push_cleanup_reference(&mut accounts_file, removed.credential_ref.clone());
        self.accounts.save(&accounts_file)?;

        if self
            .credentials
            .delete_password(&removed.credential_ref)
            .await
            .is_ok()
        {
            remove_cleanup_reference(&mut accounts_file, &removed.credential_ref);
            // A stale cleanup reference is harmless because credential deletion is idempotent.
            let _ = self.accounts.save(&accounts_file);
        }
        Ok(accounts_file.revision)
    }

    pub async fn retry_pending_credential_cleanup(&self) -> CommandResult<()> {
        let _guard = self.account_mutation.lock().await;
        let mut accounts_file = self.accounts.load()?;
        if accounts_file.pending_credential_cleanup.is_empty() {
            return Ok(());
        }
        let mut remaining = Vec::new();
        for reference in &accounts_file.pending_credential_cleanup {
            if self.credentials.delete_password(reference).await.is_err() {
                remaining.push(reference.clone());
            }
        }
        if remaining != accounts_file.pending_credential_cleanup {
            accounts_file.pending_credential_cleanup = remaining;
            self.accounts.save(&accounts_file)?;
        }
        Ok(())
    }

    pub fn accounts_revision(&self) -> CommandResult<u64> {
        Ok(self.accounts.load()?.revision)
    }

    pub fn last_selected_account_id(&self) -> CommandResult<Option<String>> {
        let accounts_file = self.accounts.load()?;
        Ok(valid_selected_account_id(&accounts_file))
    }

    pub async fn set_last_selected_account(&self, account_id: &str) -> CommandResult<String> {
        let _guard = self.account_mutation.lock().await;
        let mut accounts_file = self.accounts.load()?;
        if !accounts_file
            .accounts
            .iter()
            .any(|account| account.id == account_id)
        {
            return Err(CommandError::new("account.not_found"));
        }
        if accounts_file.last_selected_account_id.as_deref() != Some(account_id) {
            accounts_file.last_selected_account_id = Some(account_id.to_owned());
            self.accounts.save(&accounts_file)?;
        }
        Ok(account_id.to_owned())
    }

    pub fn complete_onboarding(&self) -> CommandResult<BootstrapStatus> {
        let mut config = self
            .bootstrap
            .load()?
            .ok_or_else(|| CommandError::new("data_directory.not_configured"))?;
        if !is_compatible_data_directory(&config.data_dir) {
            return Err(CommandError::new("data_directory.missing"));
        }
        if self.accounts.load()?.accounts.is_empty() {
            return Err(CommandError::new("account.required"));
        }
        config.onboarding_completed = true;
        self.bootstrap.save(&config)?;
        self.get_bootstrap_status()
    }

    pub fn list_account_summaries(&self) -> CommandResult<Vec<AccountSummary>> {
        Ok(self
            .accounts
            .load()?
            .accounts
            .iter()
            .map(AccountSummary::from)
            .collect())
    }

    pub fn account_record(&self, account_id: &str) -> CommandResult<AccountRecord> {
        self.accounts
            .load()?
            .accounts
            .into_iter()
            .find(|account| account.id == account_id)
            .ok_or_else(|| CommandError::new("account.not_found"))
    }

    pub fn account_record_for_slot(&self, account_slot_id: &str) -> CommandResult<AccountRecord> {
        self.accounts
            .load()?
            .accounts
            .into_iter()
            .find(|account| account.data_slot_id == account_slot_id)
            .ok_or_else(|| CommandError::new("account.not_found"))
    }

    pub fn configured_data_dir(&self) -> CommandResult<PathBuf> {
        let config = self
            .bootstrap
            .load()?
            .ok_or_else(|| CommandError::new("data_directory.not_configured"))?;
        if !is_compatible_data_directory(&config.data_dir) {
            return Err(CommandError::new("data_directory.missing"));
        }
        Ok(config.data_dir)
    }

    pub async fn account_password(&self, credential_ref: &str) -> CommandResult<String> {
        self.credentials.get_password(credential_ref).await
    }
}

fn validate_data_directory(path: &Path) -> DataDirectoryValidation {
    if path.as_os_str().is_empty() || !path.is_absolute() {
        return invalid("data_directory.absolute_path_required");
    }
    if path.is_file() {
        return invalid("data_directory.path_is_file");
    }
    if !path.exists() {
        return valid_new();
    }

    if path.join(DATA_MARKER_FILENAME).is_file() {
        return match read_data_marker(path) {
            Ok(Some(marker))
                if marker.format_version == 1
                    && !marker.dataset_id.is_empty()
                    && path.join(CONTENT_DATABASE_FILENAME).is_file() =>
            {
                DataDirectoryValidation {
                    valid: true,
                    can_initialize: true,
                    is_existing_dataset: true,
                    message_code: "data_directory.existing_dataset".to_owned(),
                }
            }
            _ => invalid("data_directory.incompatible_dataset"),
        };
    }

    match fs::read_dir(path) {
        Ok(mut entries) => {
            if entries.next().is_none() {
                valid_new()
            } else {
                invalid("data_directory.not_empty")
            }
        }
        Err(_) => invalid("data_directory.unreadable"),
    }
}

fn valid_new() -> DataDirectoryValidation {
    DataDirectoryValidation {
        valid: true,
        can_initialize: true,
        is_existing_dataset: false,
        message_code: "data_directory.ready".to_owned(),
    }
}

fn invalid(code: &str) -> DataDirectoryValidation {
    DataDirectoryValidation {
        valid: false,
        can_initialize: false,
        is_existing_dataset: false,
        message_code: code.to_owned(),
    }
}

fn is_compatible_data_directory(path: &Path) -> bool {
    let validation = validate_data_directory(path);
    validation.valid && validation.is_existing_dataset
}

fn is_valid_accent_color(value: &str) -> bool {
    value.len() == 7
        && value.starts_with('#')
        && value[1..]
            .chars()
            .all(|character| character.is_ascii_hexdigit())
}

fn validate_notification_preferences(
    preferences: &NotificationPreferences,
    accounts_file: &AccountsFile,
) -> CommandResult<()> {
    if !(1..=10).contains(&preferences.max_stacked) {
        return Err(CommandError::new(
            "notification_preferences.max_stacked_invalid",
        ));
    }
    if !(1..=60).contains(&preferences.display_duration_seconds) {
        return Err(CommandError::new(
            "notification_preferences.duration_invalid",
        ));
    }
    let account_ids = accounts_file
        .accounts
        .iter()
        .map(|account| account.id.as_str())
        .collect::<std::collections::HashSet<_>>();
    let mut configured_accounts = std::collections::HashSet::new();
    for setting in &preferences.accounts {
        if !account_ids.contains(setting.account_id.as_str())
            || !configured_accounts.insert(setting.account_id.as_str())
        {
            return Err(CommandError::new(
                "notification_preferences.account_invalid",
            ));
        }
    }
    let mut configured_folders = std::collections::HashSet::new();
    for setting in &preferences.folders {
        if !account_ids.contains(setting.account_id.as_str())
            || setting.mailbox_id.trim().is_empty()
            || !configured_folders
                .insert((setting.account_id.as_str(), setting.mailbox_id.as_str()))
        {
            return Err(CommandError::new("notification_preferences.folder_invalid"));
        }
    }
    Ok(())
}

fn has_duplicate_account(
    accounts_file: &AccountsFile,
    draft: &AccountDraft,
    excluded_account_id: Option<&str>,
) -> bool {
    let email = draft.email.trim().to_ascii_lowercase();
    let host = draft.incoming.host.trim().to_ascii_lowercase();
    let username = draft.incoming.username.trim().to_ascii_lowercase();
    accounts_file.accounts.iter().any(|account| {
        excluded_account_id != Some(account.id.as_str())
            && account.email.trim().eq_ignore_ascii_case(&email)
            && account.incoming.host.trim().eq_ignore_ascii_case(&host)
            && account.incoming.port == draft.incoming.port
            && account
                .incoming
                .username
                .trim()
                .eq_ignore_ascii_case(&username)
    })
}

fn valid_selected_account_id(accounts_file: &AccountsFile) -> Option<String> {
    accounts_file
        .last_selected_account_id
        .as_ref()
        .filter(|selected| {
            accounts_file
                .accounts
                .iter()
                .any(|account| &account.id == *selected)
        })
        .cloned()
        .or_else(|| {
            accounts_file
                .accounts
                .first()
                .map(|account| account.id.clone())
        })
}

fn push_cleanup_reference(accounts_file: &mut AccountsFile, reference: String) {
    if !accounts_file
        .pending_credential_cleanup
        .contains(&reference)
    {
        accounts_file.pending_credential_cleanup.push(reference);
    }
}

fn remove_cleanup_reference(accounts_file: &mut AccountsFile, reference: &str) {
    accounts_file
        .pending_credential_cleanup
        .retain(|candidate| candidate != reference);
}

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashMap,
        sync::{
            atomic::{AtomicBool, Ordering},
            Mutex as StdMutex,
        },
    };

    use async_trait::async_trait;

    use super::*;
    use crate::adapters::{
        AccountsStore, BootstrapStore, NotificationPreferencesStore, PreferencesStore,
        ReadingPreferencesStore,
    };

    #[derive(Default)]
    struct MemoryCredentialStore {
        values: StdMutex<HashMap<String, String>>,
        fail_set: AtomicBool,
        fail_delete: AtomicBool,
    }

    #[async_trait]
    impl CredentialStore for MemoryCredentialStore {
        async fn get_password(&self, credential_ref: &str) -> CommandResult<String> {
            self.values
                .lock()
                .expect("credential lock")
                .get(credential_ref)
                .cloned()
                .ok_or_else(|| CommandError::new("credential.read_failed"))
        }

        async fn set_password(&self, credential_ref: &str, password: &str) -> CommandResult<()> {
            if self.fail_set.load(Ordering::Acquire) {
                return Err(CommandError::new("credential.write_failed"));
            }
            self.values
                .lock()
                .expect("credential lock")
                .insert(credential_ref.to_owned(), password.to_owned());
            Ok(())
        }

        async fn delete_password(&self, credential_ref: &str) -> CommandResult<()> {
            if self.fail_delete.load(Ordering::Acquire) {
                return Err(CommandError::new("credential.delete_failed"));
            }
            self.values
                .lock()
                .expect("credential lock")
                .remove(credential_ref);
            Ok(())
        }
    }

    #[derive(Default)]
    struct PassingConnectionTester;

    #[async_trait]
    impl ConnectionTester for PassingConnectionTester {
        async fn test(&self, _draft: &AccountDraft) -> CommandResult<ConnectionTestResult> {
            Ok(ConnectionTestResult {
                imap_capabilities: vec!["IMAP4rev1".to_owned()],
                smtp_authenticated: true,
            })
        }
    }

    fn account_draft(email: &str, incoming_host: &str) -> AccountDraft {
        AccountDraft {
            email: email.to_owned(),
            display_name: email.split('@').next().unwrap_or(email).to_owned(),
            password: format!("password-{email}"),
            incoming: crate::domain::ServerConfig {
                host: incoming_host.to_owned(),
                port: 993,
                security: crate::domain::ConnectionSecurity::Tls,
                username: email.to_owned(),
            },
            outgoing: crate::domain::ServerConfig {
                host: format!("smtp.{incoming_host}"),
                port: 465,
                security: crate::domain::ConnectionSecurity::Tls,
                username: email.to_owned(),
            },
            insecure_acknowledged: false,
        }
    }

    async fn initialized_service() -> (tempfile::TempDir, Arc<MemoryCredentialStore>, AppService) {
        let directory = tempfile::tempdir().expect("temporary directory");
        let paths = AppPaths {
            config_dir: directory.path().join("config"),
            default_data_dir: directory.path().join("default-data"),
        };
        let credentials = Arc::new(MemoryCredentialStore::default());
        let bootstrap = Arc::new(BootstrapStore::new(&paths));
        let accounts = Arc::new(AccountsStore::new(&paths));
        let preferences = Arc::new(PreferencesStore::new(&paths));
        let reading_preferences = Arc::new(ReadingPreferencesStore::new(&paths));
        let notification_preferences = Arc::new(NotificationPreferencesStore::new(&paths));
        let stores = AppConfigStores::new(
            bootstrap,
            accounts,
            preferences,
            reading_preferences,
            notification_preferences,
        );
        let service = AppService::new(
            paths,
            stores,
            credentials.clone(),
            Arc::new(PassingConnectionTester),
        );
        let data_dir = directory.path().join("mail-data");
        service
            .initialize_data_directory(data_dir.to_str().expect("utf-8 path"))
            .await
            .expect("initialize data directory");
        (directory, credentials, service)
    }

    #[test]
    fn non_empty_unrelated_directory_is_rejected() {
        let directory = tempfile::tempdir().expect("temporary directory");
        fs::write(directory.path().join("unrelated.txt"), "content").expect("write fixture");
        let validation = validate_data_directory(directory.path());
        assert!(!validation.valid);
        assert_eq!(validation.message_code, "data_directory.not_empty");
    }

    #[test]
    fn empty_directory_can_be_initialized() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let validation = validate_data_directory(directory.path());
        assert!(validation.valid);
        assert!(validation.can_initialize);
        assert!(!validation.is_existing_dataset);
    }

    #[test]
    fn accent_color_requires_six_hex_digits() {
        assert!(is_valid_accent_color("#2563eb"));
        assert!(!is_valid_accent_color("blue"));
        assert!(!is_valid_accent_color("#12345"));
    }

    #[tokio::test]
    async fn adds_multiple_accounts_and_rejects_only_the_same_incoming_identity() {
        let (_directory, _credentials, service) = initialized_service().await;
        let first = service
            .add_password_account(account_draft("user@example.com", "imap.example.com"))
            .await
            .expect("add first account");
        let second = service
            .add_password_account(account_draft("user@example.com", "imap.other.example"))
            .await
            .expect("same address on another server is valid");

        assert_ne!(first.id, second.id);
        assert_eq!(service.list_account_summaries().unwrap().len(), 2);
        assert_eq!(service.accounts_revision().unwrap(), 2);
        assert_eq!(
            service.last_selected_account_id().unwrap().as_deref(),
            Some(first.id.as_str())
        );

        let duplicate = service
            .add_password_account(account_draft("USER@example.com", "IMAP.EXAMPLE.COM"))
            .await
            .expect_err("same identity must be rejected");
        assert_eq!(duplicate.code, "account.duplicate");
        assert_eq!(service.list_account_summaries().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn rotating_a_password_commits_new_configuration_and_cleans_the_old_credential() {
        let (_directory, credentials, service) = initialized_service().await;
        let account = service
            .add_password_account(account_draft("user@example.com", "imap.example.com"))
            .await
            .unwrap();
        let before = service.account_record(&account.id).unwrap();
        let mut draft = service.get_account_connection_draft(&account.id).unwrap();
        draft.display_name = "Updated User".to_owned();
        draft.incoming.host = "imap2.example.com".to_owned();

        service
            .update_password_account(&account.id, draft, Some("replacement".to_owned()))
            .await
            .expect("update account");

        let after = service.account_record(&account.id).unwrap();
        assert_eq!(after.data_slot_id, before.data_slot_id);
        assert_ne!(after.credential_ref, before.credential_ref);
        assert_eq!(after.display_name, "Updated User");
        assert_eq!(
            credentials
                .values
                .lock()
                .unwrap()
                .get(&after.credential_ref)
                .map(String::as_str),
            Some("replacement")
        );
        assert!(!credentials
            .values
            .lock()
            .unwrap()
            .contains_key(&before.credential_ref));
        assert!(service
            .accounts
            .load()
            .unwrap()
            .pending_credential_cleanup
            .is_empty());
    }

    #[tokio::test]
    async fn failed_credential_write_leaves_no_account_or_anonymous_slot() {
        let (_directory, credentials, service) = initialized_service().await;
        credentials.fail_set.store(true, Ordering::Release);

        let error = service
            .add_password_account(account_draft("user@example.com", "imap.example.com"))
            .await
            .expect_err("credential failure must abort account creation");
        assert_eq!(error.code, "credential.write_failed");
        assert!(service.list_account_summaries().unwrap().is_empty());

        let repository = crate::storage::MailRepository::open(
            &service
                .configured_data_dir()
                .expect("configured data directory"),
        )
        .await
        .unwrap();
        let slots: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM account_slots")
            .fetch_one(&repository.pool)
            .await
            .unwrap();
        assert_eq!(slots, 0);
    }

    #[tokio::test]
    async fn removal_keeps_the_data_slot_and_retries_failed_credential_cleanup() {
        let (_directory, credentials, service) = initialized_service().await;
        let account = service
            .add_password_account(account_draft("user@example.com", "imap.example.com"))
            .await
            .unwrap();
        let record = service.account_record(&account.id).unwrap();
        credentials.fail_delete.store(true, Ordering::Release);

        service.remove_account(&account.id).await.unwrap();
        assert!(service.list_account_summaries().unwrap().is_empty());
        assert_eq!(
            service.accounts.load().unwrap().pending_credential_cleanup,
            vec![record.credential_ref.clone()]
        );

        let repository = crate::storage::MailRepository::open(
            &service
                .configured_data_dir()
                .expect("configured data directory"),
        )
        .await
        .unwrap();
        let slot_exists: i64 =
            sqlx::query_scalar("SELECT COUNT(*) FROM account_slots WHERE id = ?")
                .bind(&record.data_slot_id)
                .fetch_one(&repository.pool)
                .await
                .unwrap();
        assert_eq!(slot_exists, 1);

        credentials.fail_delete.store(false, Ordering::Release);
        service.retry_pending_credential_cleanup().await.unwrap();
        assert!(service
            .accounts
            .load()
            .unwrap()
            .pending_credential_cleanup
            .is_empty());
        assert!(!credentials
            .values
            .lock()
            .unwrap()
            .contains_key(&record.credential_ref));
    }

    #[test]
    fn notification_preferences_reject_invalid_ranges_and_accounts() {
        let mut preferences = NotificationPreferences {
            max_stacked: 0,
            ..NotificationPreferences::default()
        };
        assert_eq!(
            validate_notification_preferences(&preferences, &AccountsFile::default())
                .unwrap_err()
                .code,
            "notification_preferences.max_stacked_invalid"
        );

        preferences.max_stacked = 3;
        preferences.display_duration_seconds = 61;
        assert_eq!(
            validate_notification_preferences(&preferences, &AccountsFile::default())
                .unwrap_err()
                .code,
            "notification_preferences.duration_invalid"
        );

        preferences.display_duration_seconds = 5;
        preferences
            .accounts
            .push(crate::core::NotificationAccountSetting {
                account_id: "missing".to_owned(),
                enabled: true,
            });
        assert_eq!(
            validate_notification_preferences(&preferences, &AccountsFile::default())
                .unwrap_err()
                .code,
            "notification_preferences.account_invalid"
        );
    }
}
