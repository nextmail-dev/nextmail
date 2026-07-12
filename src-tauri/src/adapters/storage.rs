use std::{
    fs::{self, File},
    io::{BufReader, Write},
    path::{Path, PathBuf},
};

use serde::{de::DeserializeOwned, Serialize};
use tauri::{AppHandle, Manager};
use tempfile::NamedTempFile;

use crate::{
    domain::{
        AccountsFile, AppearancePreferences, BootstrapConfig, DataDirectoryMarker,
        LanguagePreference,
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
}
