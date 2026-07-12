use async_trait::async_trait;

use crate::error::{CommandError, CommandResult};

const KEYRING_SERVICE: &str = "com.taurusxin.nextmail";

#[async_trait]
pub trait CredentialStore: Send + Sync {
    async fn get_password(&self, credential_ref: &str) -> CommandResult<String>;
    async fn set_password(&self, credential_ref: &str, password: &str) -> CommandResult<()>;
    async fn delete_password(&self, credential_ref: &str);
}

#[derive(Default)]
pub struct SystemCredentialStore;

#[async_trait]
impl CredentialStore for SystemCredentialStore {
    async fn get_password(&self, credential_ref: &str) -> CommandResult<String> {
        let credential_ref = credential_ref.to_owned();
        tokio::task::spawn_blocking(move || {
            let entry = keyring::Entry::new(KEYRING_SERVICE, &credential_ref)
                .map_err(|_| CommandError::new("credential.unavailable"))?;
            entry
                .get_password()
                .map_err(|_| CommandError::new("credential.read_failed"))
        })
        .await
        .map_err(|_| CommandError::new("credential.read_failed"))?
    }

    async fn set_password(&self, credential_ref: &str, password: &str) -> CommandResult<()> {
        let credential_ref = credential_ref.to_owned();
        let password = password.to_owned();
        tokio::task::spawn_blocking(move || {
            let entry = keyring::Entry::new(KEYRING_SERVICE, &credential_ref)
                .map_err(|_| CommandError::new("credential.unavailable"))?;
            entry
                .set_password(&password)
                .map_err(|_| CommandError::new("credential.write_failed"))
        })
        .await
        .map_err(|_| CommandError::new("credential.write_failed"))?
    }

    async fn delete_password(&self, credential_ref: &str) {
        let credential_ref = credential_ref.to_owned();
        let _ = tokio::task::spawn_blocking(move || {
            if let Ok(entry) = keyring::Entry::new(KEYRING_SERVICE, &credential_ref) {
                let _ = entry.delete_credential();
            }
        })
        .await;
    }
}
