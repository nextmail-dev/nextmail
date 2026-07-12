use std::path::{Path, PathBuf};

use nextmail_core::{CommandError, CommandResult};
use sha2::{Digest, Sha256};
use tokio::fs;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct ContentStore {
    data_dir: PathBuf,
}

impl ContentStore {
    pub fn new(data_dir: impl Into<PathBuf>) -> Self {
        Self {
            data_dir: data_dir.into(),
        }
    }

    pub async fn write_raw(&self, content: &[u8]) -> CommandResult<String> {
        self.write_content("raw", content, Some("eml")).await
    }

    pub async fn write_attachment(&self, content: &[u8]) -> CommandResult<String> {
        self.write_content("attachments", content, None).await
    }

    pub async fn read_raw(&self, hash: &str) -> CommandResult<Vec<u8>> {
        validate_hash(hash)?;
        fs::read(self.content_path("raw", hash, Some("eml")))
            .await
            .map_err(|_| CommandError::new("message.raw_read_failed"))
    }

    pub async fn read_attachment(&self, hash: &str) -> CommandResult<Vec<u8>> {
        validate_hash(hash)?;
        fs::read(self.content_path("attachments", hash, None))
            .await
            .map_err(|_| CommandError::new("attachment.content_read_failed"))
    }

    async fn write_content(
        &self,
        root: &str,
        content: &[u8],
        extension: Option<&str>,
    ) -> CommandResult<String> {
        let hash = format!("{:x}", Sha256::digest(content));
        let target = self.content_path(root, &hash, extension);
        if target.is_file() {
            return Ok(hash);
        }

        let parent = target
            .parent()
            .ok_or_else(|| CommandError::new("storage.invalid_path"))?;
        fs::create_dir_all(parent)
            .await
            .map_err(|_| CommandError::new("storage.content_directory_failed"))?;

        let temporary = parent.join(format!(".{}.tmp", Uuid::new_v4()));
        fs::write(&temporary, content)
            .await
            .map_err(|_| CommandError::new("storage.content_write_failed"))?;
        match fs::rename(&temporary, &target).await {
            Ok(()) => Ok(hash),
            Err(_) if target.is_file() => {
                let _ = fs::remove_file(&temporary).await;
                Ok(hash)
            }
            Err(_) => {
                let _ = fs::remove_file(&temporary).await;
                Err(CommandError::new("storage.content_commit_failed"))
            }
        }
    }

    fn content_path(&self, root: &str, hash: &str, extension: Option<&str>) -> PathBuf {
        let file_name = extension
            .map(|value| format!("{hash}.{value}"))
            .unwrap_or_else(|| hash.to_owned());
        self.data_dir
            .join(root)
            .join(&hash[0..2])
            .join(&hash[2..4])
            .join(file_name)
    }
}

fn validate_hash(hash: &str) -> CommandResult<()> {
    if hash.len() == 64 && hash.chars().all(|character| character.is_ascii_hexdigit()) {
        Ok(())
    } else {
        Err(CommandError::new("storage.content_id_invalid"))
    }
}

pub fn is_within_data_dir(data_dir: &Path, path: &Path) -> bool {
    path.starts_with(data_dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn raw_content_is_addressed_by_hash_and_deduplicated() {
        let directory = tempfile::tempdir().unwrap();
        let store = ContentStore::new(directory.path());
        let first = store.write_raw(b"Subject: test\r\n\r\nbody").await.unwrap();
        let second = store.write_raw(b"Subject: test\r\n\r\nbody").await.unwrap();
        assert_eq!(first, second);
        assert_eq!(
            store.read_raw(&first).await.unwrap(),
            b"Subject: test\r\n\r\nbody"
        );
    }
}
