//! "Start Rekaptr with Windows" — read/write the per-user Run registry key.

use winreg::enums::*;
use winreg::RegKey;

const STARTUP_REG_KEY: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
const STARTUP_REG_VALUE: &str = "Rekaptr";

pub fn set_startup_with_windows(enable: bool) {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    match hkcu.open_subkey_with_flags(STARTUP_REG_KEY, KEY_SET_VALUE) {
        Ok(key) => {
            if enable {
                let exe_path = std::env::current_exe()
                    .unwrap_or_else(|_| std::path::PathBuf::from("rekaptr.exe"));
                let value = format!("\"{}\"", exe_path.to_string_lossy());
                if let Err(e) = key.set_value(STARTUP_REG_VALUE, &value) {
                    log::error!("[Startup] Failed to set registry value: {}", e);
                } else {
                    log::info!("[Startup] Registered startup with Windows");
                }
            } else {
                let _ = key.delete_value(STARTUP_REG_VALUE);
                log::info!("[Startup] Removed startup with Windows");
            }
        }
        Err(e) => log::error!("[Startup] Failed to open registry key: {}", e),
    }
}

pub fn is_startup_with_windows() -> bool {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    if let Ok(key) = hkcu.open_subkey(STARTUP_REG_KEY) {
        key.get_value::<String, _>(STARTUP_REG_VALUE).is_ok()
    } else {
        false
    }
}
