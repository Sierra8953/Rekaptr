use sysinfo::System;
use windows::Win32::Foundation::{HWND, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowTextW, GetWindowThreadProcessId, IsWindowVisible,
};

#[derive(Clone, Debug)]
pub struct WindowInfo {
    pub title: String,
    pub hwnd: u64,
    pub process_name: String,
}

pub struct GameDetector {
    sys: System,
}

impl GameDetector {
    pub fn new() -> Self {
        Self { sys: System::new() }
    }

    pub fn get_process_name_from_hwnd(&mut self, hwnd: u64) -> Option<String> {
        let mut pid = 0u32;
        unsafe {
            GetWindowThreadProcessId(HWND(hwnd as *mut core::ffi::c_void), Some(&mut pid));
        }
        if pid != 0 {
            self.sys
                .refresh_processes();
            if let Some(process) = self.sys.process(sysinfo::Pid::from_u32(pid)) {
                return Some(process.name().to_string());
            }
        }
        None
    }

    pub fn enumerate_windows(&mut self) -> Vec<WindowInfo> {
        let mut windows = Vec::new();

        self.sys
            .refresh_processes();

        let blacklist = [
            "explorer.exe",
            "discord.exe",
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

        unsafe {
            let _ = EnumWindows(
                Some(enumerate_windows_callback),
                LPARAM(&mut state as *mut WindowEnumState as isize),
            );
        }

        windows
    }
}

struct WindowEnumState<'a> {
    windows: &'a mut Vec<WindowInfo>,
    sys: &'a mut System,
    blacklist: &'a [&'a str],
}

unsafe extern "system" fn enumerate_windows_callback(hwnd: HWND, lparam: LPARAM) -> windows::core::BOOL {
    let state = &mut *(lparam.0 as *mut WindowEnumState);

    if IsWindowVisible(hwnd).as_bool() {
        let mut pid = 0u32;
        GetWindowThreadProcessId(hwnd, Some(&mut pid));

        if pid != 0 {
            if let Some(process) = state.sys.process(sysinfo::Pid::from_u32(pid)) {
                let proc_name = process.name().to_string().to_lowercase();
                if state.blacklist.contains(&proc_name.as_str()) {
                    return true.into();
                }
            }
        }

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

            let mut pid = 0u32;
            GetWindowThreadProcessId(hwnd, Some(&mut pid));
            let proc_name = if pid != 0 {
                state.sys.process(sysinfo::Pid::from_u32(pid))
                    .map(|p| p.name().to_string())
                    .unwrap_or_else(|| "Unknown".to_string())
            } else {
                "Unknown".to_string()
            };

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
