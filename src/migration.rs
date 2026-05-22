// One-time migration from the Luma identity to the Rekaptr identity.
// Luma data is copied (not moved) so users retain a backup at the old paths.

use std::path::{Path, PathBuf};
use winreg::enums::*;
use winreg::RegKey;

const LEGACY_APPDATA_DIR: &str = "Luma";
const NEW_APPDATA_DIR: &str = "Rekaptr";
const LEGACY_DB_FILENAME: &str = "luma.db";
const LEGACY_STARTUP_REG_VALUE: &str = "Luma";
const NEW_STARTUP_REG_VALUE: &str = "Rekaptr";
const STARTUP_REG_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";

pub fn run() {
    migrate_appdata_dir();
    migrate_db_file();
    migrate_startup_registry();
}

fn migrate_appdata_dir() {
    let Some(local_app_data) = std::env::var_os("LOCALAPPDATA") else {
        return;
    };
    let base = PathBuf::from(local_app_data);
    let legacy = base.join(LEGACY_APPDATA_DIR);
    let new = base.join(NEW_APPDATA_DIR);

    if !legacy.exists() || new.exists() {
        return;
    }

    log::info!(
        "[Migration] Copying legacy AppData dir {} -> {}",
        legacy.display(),
        new.display()
    );
    if let Err(e) = copy_dir_recursive(&legacy, &new) {
        log::error!("[Migration] AppData copy failed: {}", e);
    } else {
        log::info!("[Migration] AppData migration complete. Legacy dir preserved as backup.");
    }
}

fn migrate_db_file() {
    let Ok(exe) = std::env::current_exe() else {
        return;
    };
    let Some(dir) = exe.parent() else {
        return;
    };
    let legacy = dir.join(LEGACY_DB_FILENAME);
    let new = crate::config::AppConfig::get_db_path();

    if !legacy.exists() || new.exists() {
        return;
    }
    if let Some(parent) = new.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    log::info!(
        "[Migration] Copying legacy database {} -> {}",
        legacy.display(),
        new.display()
    );
    if let Err(e) = std::fs::copy(&legacy, &new) {
        log::error!("[Migration] DB copy failed: {}", e);
    } else {
        log::info!("[Migration] DB migration complete. Legacy luma.db preserved as backup.");
    }
}

fn migrate_startup_registry() {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let Ok(key) = hkcu.open_subkey_with_flags(STARTUP_REG_KEY, KEY_READ | KEY_SET_VALUE) else {
        return;
    };

    let legacy_value: Result<String, _> = key.get_value(LEGACY_STARTUP_REG_VALUE);
    let Ok(_legacy_path) = legacy_value else {
        return;
    };

    // User had auto-start enabled under the old name. Rewrite it under the new
    // name using the current exe path, then drop the legacy entry.
    let exe_path = std::env::current_exe()
        .map(|p| format!("\"{}\"", p.to_string_lossy()))
        .unwrap_or_else(|_| "\"rekaptr.exe\"".to_string());

    if let Err(e) = key.set_value(NEW_STARTUP_REG_VALUE, &exe_path) {
        log::error!("[Migration] Failed to write new startup registry value: {}", e);
        return;
    }
    if let Err(e) = key.delete_value(LEGACY_STARTUP_REG_VALUE) {
        log::warn!("[Migration] Failed to delete legacy startup registry value: {}", e);
    }
    log::info!("[Migration] Startup registry entry migrated to Rekaptr.");
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else if ty.is_file() {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}
