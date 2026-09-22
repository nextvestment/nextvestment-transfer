pub mod keychain;
pub mod manager;

pub use keychain::KeychainStorage;
pub use manager::{CredentialType, Profile, ProfileManager};

use crate::error::Result;
use std::path::PathBuf;
use std::sync::Arc;
use tauri::{AppHandle, Manager};
use tokio::sync::RwLock;

pub(crate) fn write_private_file(path: &std::path::Path, content: &[u8]) -> Result<()> {
    use std::io::Write;
    let parent = path
        .parent()
        .ok_or_else(|| crate::error::AppError::IoError("Missing storage directory".to_string()))?;
    std::fs::create_dir_all(parent)?;
    let mut replacement = tempfile::NamedTempFile::new_in(parent)?;
    replacement.write_all(content)?;
    replacement.as_file().sync_all()?;
    replacement
        .persist(path)
        .map_err(|error| crate::error::AppError::IoError(error.error.to_string()))?;
    Ok(())
}

fn portable_data_dir() -> Option<PathBuf> {
    let env_portable = std::env::var("NEXTVESTMENT_TRANSFER_PORTABLE")
        .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
        .unwrap_or(false);

    let exe_dir = std::env::current_exe()
        .ok()
        .and_then(|path| path.parent().map(|parent| parent.to_path_buf()))?;

    portable_data_dir_for(&exe_dir, env_portable)
}

fn portable_data_dir_for(exe_dir: &std::path::Path, env_portable: bool) -> Option<PathBuf> {
    if env_portable || exe_dir.join("nextvestment-transfer.portable").is_file() {
        Some(exe_dir.join("nextvestment-transfer-data"))
    } else {
        None
    }
}

/// Initialize the credentials manager and register it as app state.
/// This must happen during Tauri setup before frontend commands run.
pub fn init<R: tauri::Runtime>(app: &AppHandle<R>) -> Result<PathBuf> {
    // When both Linux backends are compiled, make the persistent desktop
    // Secret Service selection explicit. Upstream credential stores are never read.
    #[cfg(target_os = "linux")]
    keyring::set_default_credential_builder(keyring::secret_service::default_credential_builder());

    let portable_config_dir = portable_data_dir();
    let portable = portable_config_dir.is_some();
    let config_dir = match portable_config_dir {
        Some(path) => path,
        None => app
            .path()
            .app_config_dir()
            .map_err(|e: tauri::Error| crate::error::AppError::ConfigError(e.to_string()))?,
    };

    // Ensure config directory exists
    std::fs::create_dir_all(&config_dir)?;

    let manager = ProfileManager::new(config_dir.clone())?;
    let state = Arc::new(RwLock::new(manager));

    app.manage(state);

    log::info!(
        "Credentials manager initialized{}",
        if portable { " in portable mode" } else { "" }
    );
    Ok(config_dir)
}

#[cfg(test)]
mod tests {
    use super::portable_data_dir_for;

    #[test]
    fn portable_metadata_ignores_upstream_marker_and_uses_own_directory() {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path();
        std::fs::write(root.join("brows3.portable"), "").unwrap();
        assert_eq!(portable_data_dir_for(root, false), None);
        let expected = Some(root.join("nextvestment-transfer-data"));
        assert_eq!(portable_data_dir_for(root, true), expected);
        std::fs::write(root.join("nextvestment-transfer.portable"), "").unwrap();
        assert_eq!(portable_data_dir_for(root, false), expected);
        assert!(!root.join("nextvestment-transfer-data").exists());
    }
}

pub mod identity_center;
