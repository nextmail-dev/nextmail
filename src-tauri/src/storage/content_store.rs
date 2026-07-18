use std::path::{Path, PathBuf};

use crate::core::{CommandError, CommandResult};
use sha2::{Digest, Sha256};
use tokio::fs;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub struct PreparedAttachmentFile {
    pub path: PathBuf,
    pub file_name: String,
    pub high_risk: bool,
}

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

    pub async fn materialize_attachment(
        &self,
        attachment_id: &str,
        file_name: &str,
        hash: &str,
    ) -> CommandResult<PreparedAttachmentFile> {
        validate_hash(hash)?;
        let file_name = sanitize_attachment_file_name(file_name);
        let attachment_key = format!("{:x}", Sha256::digest(attachment_id.as_bytes()));
        let directory = self
            .data_dir
            .join("cache")
            .join("attachment-open")
            .join(&attachment_key[0..2])
            .join(attachment_key)
            .join(hash);
        let target = directory.join(&file_name);
        if !target.is_file() {
            fs::create_dir_all(&directory)
                .await
                .map_err(|_| CommandError::new("attachment.cache_directory_failed"))?;
            let source = self.content_path("attachments", hash, None);
            let temporary = directory.join(format!(".{}.tmp", Uuid::new_v4()));
            fs::copy(&source, &temporary)
                .await
                .map_err(|_| CommandError::new("attachment.content_read_failed"))?;
            match fs::rename(&temporary, &target).await {
                Ok(()) => {}
                Err(_) if target.is_file() => {
                    let _ = fs::remove_file(&temporary).await;
                }
                Err(_) => {
                    let _ = fs::remove_file(&temporary).await;
                    return Err(CommandError::new("attachment.cache_write_failed"));
                }
            }
        }
        Ok(PreparedAttachmentFile {
            high_risk: is_high_risk_attachment(&file_name),
            path: target,
            file_name,
        })
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

pub fn sanitize_attachment_file_name(file_name: &str) -> String {
    let leaf = file_name
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or_default()
        .trim();
    let mut sanitized = leaf
        .chars()
        .map(|character| {
            if character.is_control()
                || matches!(
                    character,
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'
                )
            {
                '_'
            } else {
                character
            }
        })
        .collect::<String>();
    sanitized = sanitized.trim_end_matches([' ', '.']).to_owned();
    if sanitized.is_empty() || sanitized == "." || sanitized == ".." {
        sanitized = "attachment".to_owned();
    }
    if sanitized.len() > 180 {
        sanitized = if let Some((stem, extension)) = sanitized.rsplit_once('.') {
            if !stem.is_empty() && !extension.is_empty() && extension.len() <= 16 {
                let stem = truncate_utf8(stem, 179usize.saturating_sub(extension.len()));
                format!("{stem}.{extension}")
            } else {
                truncate_utf8(&sanitized, 180)
            }
        } else {
            truncate_utf8(&sanitized, 180)
        };
    }
    let stem = sanitized
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0');
    if reserved {
        sanitized.insert(0, '_');
    }
    sanitized
}

fn truncate_utf8(value: &str, max_bytes: usize) -> String {
    if value.len() <= max_bytes {
        return value.to_owned();
    }
    let boundary = value
        .char_indices()
        .map(|(index, _)| index)
        .take_while(|index| *index <= max_bytes)
        .last()
        .unwrap_or(0);
    value[..boundary].to_owned()
}

pub fn is_high_risk_attachment(file_name: &str) -> bool {
    let extension = file_name
        .rsplit_once('.')
        .map(|(_, value)| value.to_ascii_lowercase())
        .unwrap_or_default();
    matches!(
        extension.as_str(),
        "app"
            | "bat"
            | "bash"
            | "cmd"
            | "com"
            | "command"
            | "cpl"
            | "desktop"
            | "exe"
            | "gadget"
            | "hta"
            | "inf"
            | "ins"
            | "isp"
            | "jar"
            | "js"
            | "jse"
            | "lnk"
            | "msc"
            | "msi"
            | "msp"
            | "mst"
            | "pif"
            | "ps1"
            | "ps1xml"
            | "ps2"
            | "ps2xml"
            | "psc1"
            | "psc2"
            | "psd1"
            | "psm1"
            | "reg"
            | "scf"
            | "scr"
            | "sh"
            | "url"
            | "vb"
            | "vbe"
            | "vbs"
            | "ws"
            | "wsc"
            | "wsf"
            | "wsh"
            | "zsh"
    )
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

    #[tokio::test]
    async fn attachment_cache_uses_safe_name_and_preserves_content() {
        let directory = tempfile::tempdir().unwrap();
        let store = ContentStore::new(directory.path());
        let hash = store.write_attachment(b"attachment").await.unwrap();
        let prepared = store
            .materialize_attachment("attachment-id", "../report:2026?.pdf", &hash)
            .await
            .unwrap();

        assert_eq!(prepared.file_name, "report_2026_.pdf");
        assert!(!prepared.high_risk);
        assert_eq!(fs::read(prepared.path).await.unwrap(), b"attachment");
        let long_name = format!("{}.pdf", "附件".repeat(100));
        let sanitized = sanitize_attachment_file_name(&long_name);
        assert!(sanitized.len() <= 180);
        assert!(sanitized.ends_with(".pdf"));
    }

    #[test]
    fn executable_and_script_extensions_are_high_risk() {
        assert!(is_high_risk_attachment("invoice.exe"));
        assert!(is_high_risk_attachment("setup.PS1"));
        assert!(is_high_risk_attachment("shortcut.lnk"));
        assert!(!is_high_risk_attachment("invoice.pdf"));
        assert_eq!(
            sanitize_attachment_file_name("C:\\temp\\CON.txt"),
            "_CON.txt"
        );
    }
}
