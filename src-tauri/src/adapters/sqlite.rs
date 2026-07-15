use std::{fs, path::Path};

pub use crate::storage::{
    create_account_slot, delete_account_slot, initialize_content_database,
    CONTENT_DATABASE_FILENAME,
};

pub fn cleanup_initialized_files(data_dir: &Path) {
    for name in [
        CONTENT_DATABASE_FILENAME.to_owned(),
        format!("{CONTENT_DATABASE_FILENAME}-wal"),
        format!("{CONTENT_DATABASE_FILENAME}-shm"),
        ".nextmail-data.json".to_owned(),
    ] {
        let path = data_dir.join(name);
        if path.is_file() {
            let _ = fs::remove_file(path);
        }
    }
    for name in ["raw", "attachments", "cache"] {
        let path = data_dir.join(name);
        if path.is_dir() {
            let _ = fs::remove_dir(path);
        }
    }
    let _ = fs::remove_dir(data_dir);
}
