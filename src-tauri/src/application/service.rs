use std::{
    fs,
    path::{Path, PathBuf},
    sync::Arc,
    time::{SystemTime, UNIX_EPOCH},
};

use uuid::Uuid;

use crate::{
    adapters::{
        cleanup_initialized_files, create_account_slot, delete_account_slot,
        discover_account_config, initialize_content_database, read_data_marker, write_data_marker,
        AccountsStore, AppPaths, BootstrapStore, ConnectionTester, CredentialStore,
        PreferencesStore, CONTENT_DATABASE_FILENAME, DATA_MARKER_FILENAME,
    },
    domain::{
        AccountDraft, AccountRecord, AccountSummary, AppearancePreferences, BootstrapConfig,
        BootstrapStage, BootstrapStatus, ConnectionTestResult, DataDirectoryMarker,
        DataDirectoryValidation, DiscoveredAccountConfig,
    },
    error::{CommandError, CommandResult},
};

pub struct AppService {
    paths: AppPaths,
    bootstrap: BootstrapStore,
    accounts: AccountsStore,
    preferences: PreferencesStore,
    credentials: Arc<dyn CredentialStore>,
    connection_tester: Arc<dyn ConnectionTester>,
}

impl AppService {
    pub fn new(
        paths: AppPaths,
        credentials: Arc<dyn CredentialStore>,
        connection_tester: Arc<dyn ConnectionTester>,
    ) -> Self {
        Self {
            bootstrap: BootstrapStore::new(&paths),
            accounts: AccountsStore::new(&paths),
            preferences: PreferencesStore::new(&paths),
            paths,
            credentials,
            connection_tester,
        }
    }

    pub fn get_bootstrap_status(&self) -> CommandResult<BootstrapStatus> {
        let accounts = self
            .accounts
            .load()?
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
            });
        };

        let directory_ready = is_compatible_data_directory(&config.data_dir);
        let stage = if !directory_ready {
            BootstrapStage::DataDirectoryMissing
        } else if accounts.is_empty() || !config.onboarding_completed {
            BootstrapStage::NeedsAccount
        } else {
            BootstrapStage::Ready
        };

        Ok(BootstrapStatus {
            stage,
            default_data_dir: self.paths.default_data_dir.clone(),
            configured_data_dir: Some(config.data_dir),
            accounts,
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

    pub async fn save_password_account(
        &self,
        draft: AccountDraft,
    ) -> CommandResult<AccountSummary> {
        crate::adapters::parse_email(&draft.email)?;
        self.connection_tester.test(&draft).await?;

        let mut accounts_file = self.accounts.load()?;
        if !accounts_file.accounts.is_empty() {
            return Err(CommandError::new("account.first_iteration_limit"));
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

        if let Err(error) = self.accounts.save(&accounts_file) {
            self.credentials.delete_password(&credential_ref).await;
            delete_account_slot(&bootstrap.data_dir, &data_slot_id).await;
            return Err(error);
        }
        Ok(summary)
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

fn unix_timestamp() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
