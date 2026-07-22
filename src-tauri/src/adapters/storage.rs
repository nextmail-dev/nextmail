use std::{
    fs::{self, File},
    io::{BufReader, Write},
    path::{Path, PathBuf},
};

use serde::{de::DeserializeOwned, Serialize};
use tauri::{AppHandle, Manager};
use tempfile::NamedTempFile;

use crate::{
    core::{
        AccountsConfigStore, AppearancePreferencesStore, BootstrapConfigStore,
        NotificationPreferencesConfigStore, ReadingPreferencesConfigStore,
    },
    domain::{
        AccountsFile, AppearancePreferences, BootstrapConfig, DataDirectoryMarker,
        LanguagePreference, NotificationPreferences, ReadingPreferences,
    },
    error::{CommandError, CommandResult},
};

pub const DATA_MARKER_FILENAME: &str = ".nextmail-data.json";

#[derive(Clone, Debug)]
pub struct AppPaths {
    pub config_dir: PathBuf,
    pub default_data_dir: PathBuf,
}

impl AppPaths {
    pub fn from_handle(app: &AppHandle) -> CommandResult<Self> {
        let config_dir = app
            .path()
            .app_config_dir()
            .map_err(|_| CommandError::new("path.config_unavailable"))?
            .join("config");
        let default_data_dir = app
            .path()
            .app_local_data_dir()
            .map_err(|_| CommandError::new("path.local_data_unavailable"))?
            .join("mail-data");

        Ok(Self {
            config_dir,
            default_data_dir,
        })
    }

    pub fn bootstrap_file(&self) -> PathBuf {
        self.config_dir.join("bootstrap.json")
    }

    pub fn accounts_file(&self) -> PathBuf {
        self.config_dir.join("accounts.json")
    }

    pub fn preferences_file(&self) -> PathBuf {
        self.config_dir.join("preferences.json")
    }

    pub fn reading_preferences_file(&self) -> PathBuf {
        self.config_dir.join("reading-preferences.json")
    }

    pub fn notification_preferences_file(&self) -> PathBuf {
        self.config_dir.join("notification-preferences.json")
    }
}

#[derive(Clone)]
pub struct BootstrapStore {
    path: PathBuf,
}

impl BootstrapStore {
    pub fn new(paths: &AppPaths) -> Self {
        Self {
            path: paths.bootstrap_file(),
        }
    }

    pub fn load(&self) -> CommandResult<Option<BootstrapConfig>> {
        read_optional_json(&self.path, "storage.bootstrap_corrupt")
    }

    pub fn save(&self, value: &BootstrapConfig) -> CommandResult<()> {
        write_json_atomic(&self.path, value, "storage.bootstrap_write_failed")
    }
}

impl BootstrapConfigStore for BootstrapStore {
    fn load(&self) -> CommandResult<Option<BootstrapConfig>> {
        BootstrapStore::load(self)
    }

    fn save(&self, value: &BootstrapConfig) -> CommandResult<()> {
        BootstrapStore::save(self, value)
    }
}

#[derive(Clone)]
pub struct AccountsStore {
    path: PathBuf,
}

impl AccountsStore {
    pub fn new(paths: &AppPaths) -> Self {
        Self {
            path: paths.accounts_file(),
        }
    }

    pub fn load(&self) -> CommandResult<AccountsFile> {
        Ok(read_optional_json(&self.path, "storage.accounts_corrupt")?.unwrap_or_default())
    }

    pub fn save(&self, value: &AccountsFile) -> CommandResult<()> {
        write_json_atomic(&self.path, value, "storage.accounts_write_failed")
    }
}

impl AccountsConfigStore for AccountsStore {
    fn load(&self) -> CommandResult<AccountsFile> {
        AccountsStore::load(self)
    }

    fn save(&self, value: &AccountsFile) -> CommandResult<()> {
        AccountsStore::save(self, value)
    }
}

#[derive(Clone)]
pub struct PreferencesStore {
    path: PathBuf,
}

impl PreferencesStore {
    pub fn new(paths: &AppPaths) -> Self {
        Self {
            path: paths.preferences_file(),
        }
    }

    pub fn load(&self) -> CommandResult<AppearancePreferences> {
        Ok(
            read_optional_json(&self.path, "storage.preferences_corrupt")?.unwrap_or_else(|| {
                AppearancePreferences {
                    language: if sys_locale::get_locale()
                        .is_some_and(|locale| locale.to_ascii_lowercase().starts_with("zh"))
                    {
                        LanguagePreference::ZhCn
                    } else {
                        LanguagePreference::EnUs
                    },
                    ..AppearancePreferences::default()
                }
            }),
        )
    }

    pub fn save(&self, value: &AppearancePreferences) -> CommandResult<()> {
        write_json_atomic(&self.path, value, "storage.preferences_write_failed")
    }
}

impl AppearancePreferencesStore for PreferencesStore {
    fn load(&self) -> CommandResult<AppearancePreferences> {
        PreferencesStore::load(self)
    }

    fn save(&self, value: &AppearancePreferences) -> CommandResult<()> {
        PreferencesStore::save(self, value)
    }
}

#[derive(Clone)]
pub struct ReadingPreferencesStore {
    path: PathBuf,
}

impl ReadingPreferencesStore {
    pub fn new(paths: &AppPaths) -> Self {
        Self {
            path: paths.reading_preferences_file(),
        }
    }

    pub fn load(&self) -> CommandResult<ReadingPreferences> {
        Ok(
            read_optional_json(&self.path, "storage.reading_preferences_corrupt")?
                .unwrap_or_default(),
        )
    }

    pub fn save(&self, value: &ReadingPreferences) -> CommandResult<()> {
        write_json_atomic(
            &self.path,
            value,
            "storage.reading_preferences_write_failed",
        )
    }
}

impl ReadingPreferencesConfigStore for ReadingPreferencesStore {
    fn load(&self) -> CommandResult<ReadingPreferences> {
        ReadingPreferencesStore::load(self)
    }

    fn save(&self, value: &ReadingPreferences) -> CommandResult<()> {
        ReadingPreferencesStore::save(self, value)
    }
}

#[derive(Clone)]
pub struct NotificationPreferencesStore {
    path: PathBuf,
}

impl NotificationPreferencesStore {
    pub fn new(paths: &AppPaths) -> Self {
        Self {
            path: paths.notification_preferences_file(),
        }
    }

    pub fn load(&self) -> CommandResult<NotificationPreferences> {
        Ok(
            read_optional_json(&self.path, "storage.notification_preferences_corrupt")?
                .unwrap_or_default(),
        )
    }

    pub fn save(&self, value: &NotificationPreferences) -> CommandResult<()> {
        write_json_atomic(
            &self.path,
            value,
            "storage.notification_preferences_write_failed",
        )
    }
}

impl NotificationPreferencesConfigStore for NotificationPreferencesStore {
    fn load(&self) -> CommandResult<NotificationPreferences> {
        NotificationPreferencesStore::load(self)
    }

    fn save(&self, value: &NotificationPreferences) -> CommandResult<()> {
        NotificationPreferencesStore::save(self, value)
    }
}

pub fn read_data_marker(data_dir: &Path) -> CommandResult<Option<DataDirectoryMarker>> {
    read_optional_json(
        &data_dir.join(DATA_MARKER_FILENAME),
        "data_directory.marker_corrupt",
    )
}

pub fn write_data_marker(data_dir: &Path, marker: &DataDirectoryMarker) -> CommandResult<()> {
    write_json_atomic(
        &data_dir.join(DATA_MARKER_FILENAME),
        marker,
        "data_directory.marker_write_failed",
    )
}

fn read_optional_json<T: DeserializeOwned>(path: &Path, code: &str) -> CommandResult<Option<T>> {
    if !path.exists() {
        return Ok(None);
    }

    let file = File::open(path).map_err(|_| CommandError::new(code))?;
    serde_json::from_reader(BufReader::new(file))
        .map(Some)
        .map_err(|_| CommandError::new(code))
}

fn write_json_atomic<T: Serialize>(path: &Path, value: &T, code: &str) -> CommandResult<()> {
    let parent = path
        .parent()
        .ok_or_else(|| CommandError::new("storage.invalid_path"))?;
    fs::create_dir_all(parent).map_err(|_| CommandError::new(code))?;

    let mut temporary = NamedTempFile::new_in(parent).map_err(|_| CommandError::new(code))?;
    serde_json::to_writer_pretty(temporary.as_file_mut(), value)
        .map_err(|_| CommandError::new(code))?;
    temporary
        .as_file_mut()
        .write_all(b"\n")
        .map_err(|_| CommandError::new(code))?;
    temporary
        .as_file_mut()
        .sync_all()
        .map_err(|_| CommandError::new(code))?;
    temporary
        .persist(path)
        .map_err(|_| CommandError::new(code))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn json_store_round_trips_without_side_files() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let path = directory.path().join("preferences.json");
        let preferences = AppearancePreferences::default();

        write_json_atomic(&path, &preferences, "write_failed").expect("write preferences");
        let mut updated = preferences.clone();
        updated.accent_color = "#7c3aed".to_owned();
        write_json_atomic(&path, &updated, "write_failed").expect("replace preferences");
        let loaded: AppearancePreferences = read_optional_json(&path, "read_failed")
            .expect("read preferences")
            .expect("preferences exist");

        assert_eq!(loaded.accent_color, updated.accent_color);
        assert_eq!(fs::read_dir(directory.path()).unwrap().count(), 1);
    }

    #[test]
    fn reading_preferences_default_to_privacy_first_and_round_trip() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let paths = AppPaths {
            config_dir: directory.path().to_owned(),
            default_data_dir: directory.path().join("data"),
        };
        let store = ReadingPreferencesStore::new(&paths);

        assert_eq!(
            store.load().expect("default preferences"),
            ReadingPreferences::default()
        );
        let preferences = ReadingPreferences {
            auto_load_remote_images: true,
            auto_open_downloaded_attachments: false,
        };
        store.save(&preferences).expect("save reading preferences");
        assert_eq!(store.load().expect("load reading preferences"), preferences);
    }

    #[test]
    fn legacy_reading_preferences_enable_attachment_auto_open() {
        let preferences: ReadingPreferences =
            serde_json::from_str(r#"{"autoLoadRemoteImages":true}"#)
                .expect("deserialize legacy reading preferences");

        assert!(preferences.auto_load_remote_images);
        assert!(preferences.auto_open_downloaded_attachments);
    }

    #[test]
    fn notification_preferences_default_and_round_trip() {
        let directory = tempfile::tempdir().expect("temporary directory");
        let paths = AppPaths {
            config_dir: directory.path().to_owned(),
            default_data_dir: directory.path().join("data"),
        };
        let store = NotificationPreferencesStore::new(&paths);
        assert_eq!(
            store.load().expect("default notification preferences"),
            NotificationPreferences::default()
        );
        let preferences = NotificationPreferences {
            enabled: false,
            display_duration_seconds: 10,
            ..NotificationPreferences::default()
        };
        store
            .save(&preferences)
            .expect("save notification preferences");
        assert_eq!(
            store.load().expect("load notification preferences"),
            preferences
        );
    }

    #[test]
    fn legacy_accounts_file_defaults_new_multi_account_metadata() {
        let accounts: AccountsFile =
            serde_json::from_str(r#"{"accounts":[]}"#).expect("deserialize legacy accounts file");

        assert_eq!(accounts.revision, 0);
        assert!(accounts.accounts.is_empty());
        assert!(accounts.last_selected_account_id.is_none());
        assert!(accounts.pending_credential_cleanup.is_empty());
    }
}
