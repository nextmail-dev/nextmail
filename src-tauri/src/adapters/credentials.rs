use async_trait::async_trait;

use crate::error::CommandResult;

const KEYRING_SERVICE: &str = "com.taurusxin.nextmail";

#[async_trait]
pub trait CredentialStore: Send + Sync {
    async fn get_password(&self, credential_ref: &str) -> CommandResult<String>;
    async fn set_password(&self, credential_ref: &str, password: &str) -> CommandResult<()>;
    async fn delete_password(&self, credential_ref: &str) -> CommandResult<()>;
}

#[derive(Default)]
pub struct SystemCredentialStore;

#[async_trait]
impl CredentialStore for SystemCredentialStore {
    async fn get_password(&self, credential_ref: &str) -> CommandResult<String> {
        let credential_ref = credential_ref.to_owned();
        tokio::task::spawn_blocking(move || {
            let entry = keyring::Entry::new(KEYRING_SERVICE, &credential_ref).map_err(|error| {
                crate::diagnostics::command_error("credential.unavailable", false, &error)
            })?;
            entry.get_password().map_err(|error| {
                crate::diagnostics::command_error("credential.read_failed", false, &error)
            })
        })
        .await
        .map_err(|error| {
            crate::diagnostics::command_error("credential.read_failed", false, &error)
        })?
    }

    async fn set_password(&self, credential_ref: &str, password: &str) -> CommandResult<()> {
        let credential_ref = credential_ref.to_owned();
        let password = password.to_owned();
        tokio::task::spawn_blocking(move || {
            let entry = keyring::Entry::new(KEYRING_SERVICE, &credential_ref).map_err(|error| {
                crate::diagnostics::command_error("credential.unavailable", false, &error)
            })?;
            entry.set_password(&password).map_err(|error| {
                crate::diagnostics::command_error("credential.write_failed", false, &error)
            })
        })
        .await
        .map_err(|error| {
            crate::diagnostics::command_error("credential.write_failed", false, &error)
        })?
    }

    async fn delete_password(&self, credential_ref: &str) -> CommandResult<()> {
        let credential_ref = credential_ref.to_owned();
        tokio::task::spawn_blocking(move || {
            let entry = keyring::Entry::new(KEYRING_SERVICE, &credential_ref).map_err(|error| {
                crate::diagnostics::command_error("credential.unavailable", false, &error)
            })?;
            match entry.delete_credential() {
                Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
                Err(error) => Err(crate::diagnostics::command_error(
                    "credential.delete_failed",
                    false,
                    &error,
                )),
            }
        })
        .await
        .map_err(|error| {
            crate::diagnostics::command_error("credential.delete_failed", false, &error)
        })?
    }
}
