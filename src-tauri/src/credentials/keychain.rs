use crate::error::{AppError, Result};
use keyring::Entry;

const SERVICE_NAME: &str = "nextvestment-transfer-credentials";
const VAULT_ERROR: &str = "The operating system credential vault is unavailable. Unlock your keychain or ask your VDI administrator to enable Credential Manager, or use an AWS SSO profile. No plaintext credential file was written.";

trait Vault: Send + Sync {
    fn store(&self, key: &str, secret: &str) -> keyring::Result<()>;
    fn get(&self, key: &str) -> keyring::Result<String>;
    fn delete(&self, key: &str) -> keyring::Result<()>;
}

struct NativeVault;
impl Vault for NativeVault {
    fn store(&self, key: &str, secret: &str) -> keyring::Result<()> {
        Entry::new(SERVICE_NAME, key)?.set_password(secret)
    }
    fn get(&self, key: &str) -> keyring::Result<String> {
        Entry::new(SERVICE_NAME, key)?.get_password()
    }
    fn delete(&self, key: &str) -> keyring::Result<()> {
        Entry::new(SERVICE_NAME, key)?.delete_credential()
    }
}

/// OS-vault storage in both installed and portable builds. There is deliberately
/// no filesystem fallback and no access to the upstream application's namespace.
pub struct KeychainStorage {
    vault: Box<dyn Vault>,
}
impl Default for KeychainStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl KeychainStorage {
    pub fn new() -> Self {
        Self {
            vault: Box::new(NativeVault),
        }
    }
    pub fn store(&self, key: &str, secret: &str) -> Result<()> {
        if !is_identity_session(key) {
            return self.vault.store(key, secret).map_err(|_| vault_error());
        }
        // References are copy-on-write: never partially overwrite a live token.
        match self.vault.get(key) {
            Ok(_) => {
                return Err(crate::error::AppError::KeychainError(
                    "The AWS session reference already exists. Start a new sign-in.".into(),
                ))
            }
            Err(keyring::Error::NoEntry) => {}
            Err(_) => return Err(vault_error()),
        }
        let chunks = secret_chunks(secret)?;
        for (index, chunk) in chunks.iter().enumerate() {
            if self.vault.store(&chunk_key(key, index), chunk).is_err() {
                for written in 0..=index {
                    let _ = self.vault.delete(&chunk_key(key, written));
                }
                return Err(vault_error());
            }
        }
        if self
            .vault
            .store(key, &format!("{CHUNK_MANIFEST}{}", chunks.len()))
            .is_err()
        {
            for index in 0..chunks.len() {
                let _ = self.vault.delete(&chunk_key(key, index));
            }
            return Err(vault_error());
        }
        Ok(())
    }
    pub fn get(&self, key: &str) -> Result<Option<String>> {
        let value = match self.vault.get(key) {
            Ok(value) => value,
            Err(keyring::Error::NoEntry) => return Ok(None),
            Err(_) => return Err(vault_error()),
        };
        if !is_identity_session(key) {
            return Ok(Some(value));
        }
        let count = chunk_count(&value)?;
        let mut secret = String::new();
        for index in 0..count {
            let part = self
                .vault
                .get(&chunk_key(key, index))
                .map_err(|_| vault_error())?;
            if part.encode_utf16().count() > CHUNK_UTF16_LIMIT {
                return Err(vault_error());
            }
            secret.push_str(&part);
        }
        Ok(Some(secret))
    }
    pub fn delete(&self, key: &str) -> Result<()> {
        if is_identity_session(key) {
            match self.vault.get(key) {
                Ok(manifest) => {
                    let count = chunk_count(&manifest)?;
                    // Keep the manifest if deletion fails so a later retry knows all keys.
                    for index in 0..count {
                        match self.vault.delete(&chunk_key(key, index)) {
                            Ok(()) | Err(keyring::Error::NoEntry) => {}
                            Err(_) => return Err(vault_error()),
                        }
                    }
                }
                Err(keyring::Error::NoEntry) => return Ok(()),
                Err(_) => return Err(vault_error()),
            }
        }
        match self.vault.delete(key) {
            Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
            Err(_) => Err(vault_error()),
        }
    }
    pub fn exists(&self, key: &str) -> bool {
        matches!(self.get(key), Ok(Some(_)))
    }
    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self {
            vault: Box::new(MemoryVault::default()),
        }
    }
}

const CHUNK_MANIFEST: &str = "nextvestment-vault-chunks-v1:";
const CHUNK_UTF16_LIMIT: usize = 1000; // 2000 bytes, below Windows' 2560-byte limit.
const MAX_CHUNKS: usize = 64;
fn is_identity_session(key: &str) -> bool {
    key.strip_prefix("identity-center-")
        .is_some_and(|id| uuid::Uuid::parse_str(id).is_ok())
}
fn vault_error() -> AppError {
    AppError::KeychainError(VAULT_ERROR.into())
}
fn chunk_key(key: &str, index: usize) -> String {
    format!("{key}.part.{index}")
}
fn chunk_count(manifest: &str) -> Result<usize> {
    let count = manifest
        .strip_prefix(CHUNK_MANIFEST)
        .and_then(|n| n.parse::<usize>().ok())
        .filter(|n| *n > 0 && *n <= MAX_CHUNKS)
        .ok_or_else(vault_error)?;
    Ok(count)
}
fn secret_chunks(secret: &str) -> Result<Vec<String>> {
    let mut chunks = Vec::new();
    let mut part = String::new();
    let mut units = 0;
    for character in secret.chars() {
        let size = character.len_utf16();
        if units + size > CHUNK_UTF16_LIMIT {
            chunks.push(part);
            part = String::new();
            units = 0;
        }
        part.push(character);
        units += size;
        if chunks.len() >= MAX_CHUNKS {
            return Err(vault_error());
        }
    }
    chunks.push(part);
    Ok(chunks)
}

#[cfg(test)]
#[derive(Default)]
struct MemoryVault(std::sync::Mutex<std::collections::HashMap<String, String>>);
#[cfg(test)]
impl Vault for MemoryVault {
    fn store(&self, key: &str, secret: &str) -> keyring::Result<()> {
        self.0.lock().unwrap().insert(key.into(), secret.into());
        Ok(())
    }
    fn get(&self, key: &str) -> keyring::Result<String> {
        self.0
            .lock()
            .unwrap()
            .get(key)
            .cloned()
            .ok_or(keyring::Error::NoEntry)
    }
    fn delete(&self, key: &str) -> keyring::Result<()> {
        self.0.lock().unwrap().remove(key);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    struct UnavailableVault;
    impl Vault for UnavailableVault {
        fn store(&self, _: &str, _: &str) -> keyring::Result<()> {
            Err(keyring::Error::Invalid(
                "synthetic-secret".into(),
                "private-diagnostic".into(),
            ))
        }
        fn get(&self, _: &str) -> keyring::Result<String> {
            Err(keyring::Error::Invalid(
                "synthetic-secret".into(),
                "private-diagnostic".into(),
            ))
        }
        fn delete(&self, _: &str) -> keyring::Result<()> {
            Err(keyring::Error::Invalid(
                "synthetic-secret".into(),
                "private-diagnostic".into(),
            ))
        }
    }
    #[test]
    fn vault_failure_is_closed_and_diagnostics_are_redacted() {
        let storage = KeychainStorage {
            vault: Box::new(UnavailableVault),
        };
        let failures = [
            storage.store("key", "synthetic-secret").unwrap_err(),
            storage.get("key").unwrap_err(),
            storage.delete("key").unwrap_err(),
        ];
        for error in failures {
            let text = error.to_string();
            assert!(text.contains("No plaintext credential file was written"));
            assert!(!text.contains("synthetic-secret"));
            assert!(!text.contains("private-diagnostic"));
        }
    }
    #[test]
    fn isolated_vault_round_trip_never_uses_a_plaintext_file() {
        let storage = KeychainStorage::for_test();
        assert_eq!(storage.get("key").unwrap(), None);
        storage.store("key", "synthetic-secret").unwrap();
        assert_eq!(
            storage.get("key").unwrap().as_deref(),
            Some("synthetic-secret")
        );
        storage.delete("key").unwrap();
        storage.delete("key").unwrap();
        assert_eq!(storage.get("key").unwrap(), None);
        assert_eq!(SERVICE_NAME, "nextvestment-transfer-credentials");
    }
    #[test]
    fn failed_private_file_replacement_preserves_the_destination() {
        let directory = tempfile::tempdir().unwrap();
        let destination = directory.path().join("existing");
        std::fs::create_dir(&destination).unwrap();
        std::fs::write(destination.join("keep"), "original").unwrap();
        assert!(crate::credentials::write_private_file(&destination, b"replacement").is_err());
        assert_eq!(
            std::fs::read_to_string(destination.join("keep")).unwrap(),
            "original"
        );
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 1);
    }
    #[derive(Clone, Default)]
    struct BoundedVault {
        entries: std::sync::Arc<std::sync::Mutex<std::collections::HashMap<String, String>>>,
        fail_part: Option<usize>,
    }
    impl Vault for BoundedVault {
        fn store(&self, key: &str, value: &str) -> keyring::Result<()> {
            if value.encode_utf16().count() * 2 > 2560
                || self
                    .fail_part
                    .is_some_and(|n| key.ends_with(&format!(".part.{n}")))
            {
                return Err(keyring::Error::Invalid(
                    "synthetic".into(),
                    "synthetic".into(),
                ));
            }
            self.entries
                .lock()
                .unwrap()
                .insert(key.into(), value.into());
            Ok(())
        }
        fn get(&self, key: &str) -> keyring::Result<String> {
            self.entries
                .lock()
                .unwrap()
                .get(key)
                .cloned()
                .ok_or(keyring::Error::NoEntry)
        }
        fn delete(&self, key: &str) -> keyring::Result<()> {
            self.entries.lock().unwrap().remove(key);
            Ok(())
        }
    }
    #[test]
    fn large_identity_token_roundtrips_with_windows_vault_blob_limit_and_cascading_delete() {
        let backend = BoundedVault::default();
        let inspect = backend.clone();
        let storage = KeychainStorage {
            vault: Box::new(backend),
        };
        let key = format!("identity-center-{}", uuid::Uuid::new_v4());
        let secret = format!("{}{}", "synthetic-token-".repeat(400), "🔒".repeat(700));
        assert!(secret.encode_utf16().count() * 2 > 2560);
        storage.store(&key, &secret).unwrap();
        assert_eq!(storage.get(&key).unwrap(), Some(secret));
        assert!(inspect
            .entries
            .lock()
            .unwrap()
            .values()
            .all(|value| value.encode_utf16().count() * 2 <= 2560));
        assert!(storage.store(&key, "overwrite").is_err());
        storage.delete(&key).unwrap();
        storage.delete(&key).unwrap();
        assert!(inspect.entries.lock().unwrap().is_empty());
    }
    #[test]
    fn partial_chunk_write_rolls_back_without_publishing_manifest() {
        let backend = BoundedVault {
            fail_part: Some(2),
            ..Default::default()
        };
        let inspect = backend.clone();
        let storage = KeychainStorage {
            vault: Box::new(backend),
        };
        let key = format!("identity-center-{}", uuid::Uuid::new_v4());
        assert!(storage.store(&key, &"x".repeat(5000)).is_err());
        assert!(inspect.entries.lock().unwrap().is_empty());
    }
    #[test]
    fn missing_chunk_and_invalid_manifest_fail_closed() {
        let backend = BoundedVault::default();
        let inspect = backend.clone();
        let storage = KeychainStorage {
            vault: Box::new(backend),
        };
        let key = format!("identity-center-{}", uuid::Uuid::new_v4());
        storage.store(&key, &"x".repeat(3000)).unwrap();
        inspect.entries.lock().unwrap().remove(&chunk_key(&key, 1));
        assert!(storage.get(&key).is_err());
        storage.delete(&key).unwrap();
        assert!(inspect.entries.lock().unwrap().is_empty());
        inspect
            .entries
            .lock()
            .unwrap()
            .insert(key.clone(), format!("{CHUNK_MANIFEST}999999999"));
        assert!(storage.get(&key).is_err());
    }
}
