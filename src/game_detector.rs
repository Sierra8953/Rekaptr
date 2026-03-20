//! Window enumeration and game process detection.
//!
//! Enumerates visible top-level windows via `EnumWindows` and resolves each to
//! its owning process name. A blacklist filters out OS shell windows, dev tools,
//! and Luma itself — the goal is to surface only windows that plausibly represent
//! games or applications the user might want to record.
//!
//! Process list refreshes are throttled to a 2-second cooldown because
//! `sysinfo::System::refresh_processes()` is expensive (~5ms+ with hundreds of
//! processes). Since this module is polled on every foreground change event,
//! caching prevents redundant work during rapid window switching.

use sysinfo::System;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};

/// Describes a visible top-level window and its owning process.
#[derive(Clone, Debug)]
pub struct WindowInfo {
    pub title: String,
    pub hwnd: u64,
    pub process_name: String,
}

/// Detects recordable game windows by enumerating the desktop and filtering
/// out known non-game processes. Caches the process list to avoid repeated
/// expensive system queries.
pub struct GameDetector {
    sys: System,
    last_refresh: std::time::Instant,
}

impl GameDetector {
    pub fn new() -> Self {
        Self {
            sys: System::new(),
            last_refresh: std::time::Instant::now() - std::time::Duration::from_secs(60),
        }
    }

    /// Refresh the process snapshot if the cache is stale (>2s old).
    fn refresh_if_needed(&mut self) {
        if self.last_refresh.elapsed() > std::time::Duration::from_secs(2) {
            self.sys.refresh_processes();
            self.last_refresh = std::time::Instant::now();
        }
    }

    pub fn get_process_name_from_hwnd(&mut self, hwnd: u64, app_state: &crate::state::AppState) -> Option<String> {
        let mut pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(HWND(hwnd as *mut core::ffi::c_void), Some(&mut pid));
        }
        if pid != 0 {
            // Check cache first
            let hwnd_key = format!("hwnd_{}", hwnd);
            if let Some(cached) = app_state.process_cache.get(&hwnd_key) {
                return Some(format!("PID:{}", cached.value()));
            }

            self.refresh_if_needed();
            if let Some(process) = self.sys.process(sysinfo::Pid::from_u32(pid)) {
                let name = process.name().to_string();
                app_state.insert_process(hwnd_key, pid);
                return Some(name);
            }
        }
        None
    }

    /// Enumerate all visible top-level windows, filtering out known non-game
    /// processes. Returns only windows with non-empty titles that aren't on the
    /// blacklist.
    pub fn enumerate_windows(&mut self) -> Vec<WindowInfo> {
        let mut windows = Vec::new();

        self.refresh_if_needed();

        // Blacklist strategy: exclude rather than include. Games are too diverse
        // to whitelist, but the set of OS/shell/dev processes is small and stable.
        let blacklist = [
            "explorer.exe",
            "steam.exe",
            "powershell.exe",
            "cmd.exe",
            "taskmgr.exe",
            "applicationframehost.exe",
            "shellexperiencehost.exe",
            "searchhost.exe",
            "startmenuexperiencehost.exe",
            "luma.exe",
            "nvidia share.exe",
            "devenv.exe",
            "rustrover.exe",
        ];

        let mut state = WindowEnumState {
            windows: &mut windows,
            sys: &mut self.sys,
            blacklist: &blacklist,
        };

        // Safety: EnumWindows requires an unsafe extern "system" callback. We pass
        // mutable state through LPARAM by casting a pointer to isize — this is the
        // standard Win32 pattern. The callback runs synchronously on this thread
        // before EnumWindows returns, so the borrow is valid for the entire call.
        unsafe {
            let _ = EnumWindows(
                Some(enumerate_windows_callback),
                LPARAM(&mut state as *mut WindowEnumState as isize),
            );
        }

        windows
    }
}

/// Mutable state passed through `LPARAM` to the `EnumWindows` callback.
struct WindowEnumState<'a> {
    windows: &'a mut Vec<WindowInfo>,
    sys: &'a mut System,
    blacklist: &'a [&'a str],
}

/// Win32 `EnumWindows` callback. Called once per top-level window.
/// Returns `true` to continue enumeration, per the Win32 convention.
///
/// Safety: `lparam` must be a valid pointer to `WindowEnumState`. This is
/// guaranteed because `enumerate_windows` passes a stack-local reference
/// and `EnumWindows` calls this synchronously.
unsafe extern "system" fn enumerate_windows_callback(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    let state = &mut *(lparam.0 as *mut WindowEnumState);

    if IsWindowVisible(hwnd).as_bool() {
        let mut text = [0u16; 512];
        let len = GetWindowTextW(hwnd, &mut text);

        if len > 0 {
            let title = String::from_utf16_lossy(&text[..len as usize]);

            if title == "Program Manager"
                || title == "Settings"
                || title == "Microsoft Text Input Application"
            {
                return true.into();
            }

            // ⚡ Bolt: Cache PID and sysinfo lookup to avoid redundant OS queries
            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            let mut proc_name = String::from("Unknown");

            if pid != 0 {
                if let Some(process) = state.sys.process(sysinfo::Pid::from_u32(pid)) {
                    let name = process.name().to_string();
                    let name_lower = name.to_lowercase();

                    if state.blacklist.contains(&name_lower.as_str()) {
                        return true.into();
                    }
                    proc_name = name;
                }
            }

            state.windows.push(WindowInfo {
                title,
                hwnd: hwnd.0 as u64,
                process_name: proc_name,
            });
        }
    }
    true.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enumerate_windows() {
        let mut detector = GameDetector::new();
        let windows = detector.enumerate_windows();
        assert!(!windows.is_empty());
        for win in windows {
            println!("Window: {} (HWND: {})", win.title, win.hwnd);
        }
    }
}
