//! App-scoped SMTP credential adapter.

use quotatide_core::CredentialVault;
use secrecy::{ExposeSecret, SecretString};

const SERVICE: &str = "dev.theblind.quotatide.smtp";

#[derive(Debug, Clone, Copy, thiserror::Error)]
pub enum CredentialVaultError {
    #[error("credential vault is unavailable")]
    Unavailable,
}

#[derive(Clone, Copy)]
pub struct SystemCredentialVault;

impl CredentialVault for SystemCredentialVault {
    type Error = CredentialVaultError;

    async fn get(&self, slot: &'static str) -> Result<Option<SecretString>, Self::Error> {
        tauri::async_runtime::spawn_blocking(move || {
            let entry = entry(slot)?;
            match entry.get_password() {
                Ok(secret) => Ok(Some(SecretString::from(secret))),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(_) => Err(CredentialVaultError::Unavailable),
            }
        })
        .await
        .map_err(|_| CredentialVaultError::Unavailable)?
    }

    async fn set(&self, slot: &'static str, secret: SecretString) -> Result<(), Self::Error> {
        tauri::async_runtime::spawn_blocking(move || {
            entry(slot)?
                .set_password(secret.expose_secret())
                .map_err(|_| CredentialVaultError::Unavailable)
        })
        .await
        .map_err(|_| CredentialVaultError::Unavailable)?
    }

    async fn delete(&self, slot: &'static str) -> Result<(), Self::Error> {
        tauri::async_runtime::spawn_blocking(move || match entry(slot)?.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(CredentialVaultError::Unavailable),
        })
        .await
        .map_err(|_| CredentialVaultError::Unavailable)?
    }
}

fn entry(slot: &str) -> Result<keyring::Entry, CredentialVaultError> {
    let username = match slot {
        "slot-a" => "sender-slot-a",
        "slot-b" => "sender-slot-b",
        _ => return Err(CredentialVaultError::Unavailable),
    };
    keyring::Entry::new(SERVICE, username).map_err(|_| CredentialVaultError::Unavailable)
}
