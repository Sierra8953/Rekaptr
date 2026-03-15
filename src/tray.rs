//! System tray icon, balloon notifications, and global hotkey for instant replay.
//!
//! Runs on a dedicated thread with its own Win32 message loop. The tray icon shows
//! Luma's recording state and provides a right-click context menu. Balloon notifications
//! are used for export-complete alerts that work even when the app is minimized.
//!
//! The global hotkey (Ctrl+Shift+F9 by default) triggers an instant replay export of
//! the last N seconds from the active recording buffer.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

// Message IDs
const WM_TRAY_ICON: u32 = 0x8001;
const WM_HOTKEY_INSTANT_REPLAY: i32 = 1;
const IDM_TOGGLE_RECORDING: u32 = 1001;
const IDM_OPEN_LUMA: u32 = 1002;
const IDM_QUIT: u32 = 1003;

static TRAY_CREATED: AtomicBool = AtomicBool::new(false);

/// Sends a balloon notification through the system tray icon.
/// Falls back silently if the tray hasn't been created yet.
pub fn show_notification(title: &str, message: &str) {
    if !TRAY_CREATED.load(Ordering::Relaxed) {
        return;
    }

    use windows::Win32::UI::Shell::*;

    unsafe {
        let hwnd = find_tray_window();
        if hwnd.0.is_null() { return; }

        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_INFO;
        nid.dwInfoFlags = NIIF_INFO;

        // Copy title
        let title_wide: Vec<u16> = title.encode_utf16().chain(std::iter::once(0)).collect();
        let title_len = title_wide.len().min(nid.szInfoTitle.len());
        nid.szInfoTitle[..title_len].copy_from_slice(&title_wide[..title_len]);

        // Copy message
        let msg_wide: Vec<u16> = message.encode_utf16().chain(std::iter::once(0)).collect();
        let msg_len = msg_wide.len().min(nid.szInfo.len());
        nid.szInfo[..msg_len].copy_from_slice(&msg_wide[..msg_len]);

        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

/// Updates the tray tooltip to reflect recording state.
pub fn update_recording_state(is_recording: bool) {
    if !TRAY_CREATED.load(Ordering::Relaxed) { return; }

    use windows::Win32::UI::Shell::*;

    unsafe {
        let hwnd = find_tray_window();
        if hwnd.0.is_null() { return; }

        let tooltip = if is_recording { "Luma — Recording" } else { "Luma — Idle" };
        let tip_wide: Vec<u16> = tooltip.encode_utf16().chain(std::iter::once(0)).collect();

        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = 1;
        nid.uFlags = NIF_TIP;
        let tip_len = tip_wide.len().min(nid.szTip.len());
        nid.szTip[..tip_len].copy_from_slice(&tip_wide[..tip_len]);

        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

unsafe fn find_tray_window() -> windows::Win32::Foundation::HWND {
    use windows::Win32::UI::WindowsAndMessaging::FindWindowW;
    use windows::core::w;
    FindWindowW(w!("LumaTrayWindow"), None).unwrap_or_default()
}

/// Starts the tray icon thread and registers the global hotkey.
///
/// `on_instant_replay` is called on the tray thread when the hotkey fires.
/// The caller should use a channel or atomic flag to communicate back to the main thread.
pub fn start_tray_thread(
    is_recording: Arc<AtomicBool>,
    instant_replay_tx: std::sync::mpsc::Sender<()>,
) {
    std::thread::Builder::new()
        .name("Luma Tray".to_string())
        .spawn(move || {
            unsafe { run_tray_loop(is_recording, instant_replay_tx) };
        })
        .ok();
}

unsafe fn run_tray_loop(
    _is_recording: Arc<AtomicBool>,
    instant_replay_tx: std::sync::mpsc::Sender<()>,
) {
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::UI::Shell::*;
    use windows::Win32::UI::Input::KeyboardAndMouse::*;
    use windows::Win32::Foundation::*;
    use windows::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows::core::w;

    let hinstance = GetModuleHandleW(None).unwrap_or_default();

    // Register a minimal window class for the hidden message-only window
    let class_name = w!("LumaTrayWindow");
    let wc = WNDCLASSW {
        lpfnWndProc: Some(tray_wnd_proc),
        hInstance: hinstance.into(),
        lpszClassName: class_name,
        ..Default::default()
    };
    RegisterClassW(&wc);

    let hwnd = CreateWindowExW(
        WINDOW_EX_STYLE::default(),
        class_name,
        w!("Luma Tray"),
        WS_OVERLAPPED,
        0, 0, 0, 0,
        Some(HWND_MESSAGE), // message-only window
        None,
        Some(hinstance.into()),
        None,
    ).unwrap_or_default();

    if hwnd.0.is_null() {
        log::error!("[Tray] Failed to create message window");
        return;
    }

    // Store the instant replay sender as window user data
    let tx_box = Box::new(instant_replay_tx);
    SetWindowLongPtrW(hwnd, GWLP_USERDATA, Box::into_raw(tx_box) as isize);

    // Create the tray icon
    let tooltip = "Luma — Idle";
    let tip_wide: Vec<u16> = tooltip.encode_utf16().chain(std::iter::once(0)).collect();

    let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
    nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
    nid.hWnd = hwnd;
    nid.uID = 1;
    nid.uFlags = NIF_MESSAGE | NIF_TIP | NIF_ICON;
    nid.uCallbackMessage = WM_TRAY_ICON;

    let tip_len = tip_wide.len().min(nid.szTip.len());
    nid.szTip[..tip_len].copy_from_slice(&tip_wide[..tip_len]);

    // Use a default application icon
    nid.hIcon = LoadIconW(None, IDI_APPLICATION).unwrap_or_default();

    let _ = Shell_NotifyIconW(NIM_ADD, &nid);
    TRAY_CREATED.store(true, Ordering::Relaxed);

    // Register global hotkey: Ctrl+Shift+F9 for instant replay
    let _ = RegisterHotKey(
        Some(hwnd),
        WM_HOTKEY_INSTANT_REPLAY,
        MOD_CONTROL | MOD_SHIFT | MOD_NOREPEAT,
        0x78, // VK_F9
    );

    log::info!("[Tray] System tray icon created, global hotkey Ctrl+Shift+F9 registered");

    // Message loop
    let mut msg = MSG::default();
    while GetMessageW(&mut msg, None, 0, 0).as_bool() {
        let _ = TranslateMessage(&msg);
        DispatchMessageW(&msg);
    }

    // Cleanup
    let _ = Shell_NotifyIconW(NIM_DELETE, &nid);
    let _ = UnregisterHotKey(Some(hwnd), WM_HOTKEY_INSTANT_REPLAY);
    TRAY_CREATED.store(false, Ordering::Relaxed);
}

unsafe extern "system" fn tray_wnd_proc(
    hwnd: windows::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> windows::Win32::Foundation::LRESULT {
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::Win32::Foundation::*;

    match msg {
        WM_TRAY_ICON => {
            let event = (lparam.0 & 0xFFFF) as u32;
            if event == WM_RBUTTONUP {
                // Show context menu
                let mut pt = windows::Win32::Foundation::POINT::default();
                let _ = windows::Win32::UI::WindowsAndMessaging::GetCursorPos(&mut pt);

                let menu = CreatePopupMenu().unwrap_or_default();
                let _ = AppendMenuW(menu, MENU_ITEM_FLAGS(0), IDM_OPEN_LUMA as usize, windows::core::w!("Open Luma"));
                let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
                let _ = AppendMenuW(menu, MENU_ITEM_FLAGS(0), IDM_QUIT as usize, windows::core::w!("Quit"));

                // Required for popup menus from tray icons
                SetForegroundWindow(hwnd);
                let _ = TrackPopupMenu(menu, TPM_LEFTALIGN | TPM_BOTTOMALIGN, pt.x, pt.y, None, hwnd, None);
                let _ = DestroyMenu(menu);
            } else if event == WM_LBUTTONDBLCLK {
                // Double-click: bring window to front
                bring_main_window_to_front();
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let cmd = (wparam.0 & 0xFFFF) as u32;
            match cmd {
                IDM_OPEN_LUMA => {
                    bring_main_window_to_front();
                }
                IDM_QUIT => {
                    std::process::exit(0);
                }
                _ => {}
            }
            LRESULT(0)
        }
        0x0312 /* WM_HOTKEY */ => {
            let id = wparam.0 as i32;
            if id == WM_HOTKEY_INSTANT_REPLAY {
                let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const std::sync::mpsc::Sender<()>;
                if !ptr.is_null() {
                    let _ = (*ptr).send(());
                }
            }
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wparam, lparam),
    }
}

fn bring_main_window_to_front() {
    use windows::Win32::UI::WindowsAndMessaging::*;
    use windows::core::w;

    unsafe {
        if let Ok(hwnd) = FindWindowW(None, w!("Luma")) {
            if !hwnd.0.is_null() {
                if IsIconic(hwnd).as_bool() {
                    let _ = ShowWindow(hwnd, SW_RESTORE);
                }
                SetForegroundWindow(hwnd);
            }
        }
    }
}
