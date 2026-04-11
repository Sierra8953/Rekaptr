//! Event-driven automatic game recording.
//!
//! Uses a Win32 `WinEventHook` to detect foreground window changes, then checks
//! whether the new foreground process matches any game in the user's registry
//! that has auto-record enabled. If so, recording starts without user interaction.
//!
//! Architecture overview:
//!
//! 1. A dedicated OS thread installs `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)`
//!    and runs a Win32 message pump. This thread exists solely because
//!    `WinEventHook` with `WINEVENT_OUTOFCONTEXT` requires an active message loop
//!    on the thread that called `SetWinEventHook`.
//!
//! 2. The hook callback sends a notification through a tokio mpsc channel,
//!    bridging the synchronous Win32 world into the async GPUI context.
//!
//! 3. On the async side, a 500ms debounce delay prevents rapid-fire toggling
//!    when the user Alt-Tabs through windows quickly. After the debounce,
//!    the game detector checks if the current foreground window is a known game.

use crate::state::AppState;
use crate::ui::LumaWorkspace;
use gpui::*;
use std::sync::Arc;

/// Spawn the auto-record event loop as a detached GPUI async task.
///
/// This runs for the lifetime of the application. It idles with near-zero CPU
/// cost until a foreground change event arrives, then debounces and checks
/// the game registry. A 60-second fallback timer ensures we eventually poll
/// even if the Win32 hook misses an event (rare, but defensive).
pub fn spawn_auto_record_loop(
    cx: &mut App,
    app_state: Arc<AppState>,
    workspace_handle: Arc<std::sync::Mutex<Option<WeakEntity<LumaWorkspace>>>>,
) {
    cx.spawn(|cx: &mut AsyncApp| {
        let cx = cx.clone();
        async move {
            use windows::Win32::UI::Accessibility::*;
            use windows::Win32::UI::WindowsAndMessaging::*;
            use windows::Win32::Foundation::HWND;

            // Channel bridges the synchronous Win32 callback into the async world.
            // Capacity of 10 absorbs bursts of rapid window switches during debounce.
            let (tx, mut rx) = tokio::sync::mpsc::channel::<()>(10);

            // WinEventHook requires a thread with a running message pump (GetMessage loop).
            // We can't use the main UI thread because GPUI owns that message loop.
            std::thread::spawn(move || {
                unsafe {
                    extern "system" fn winevent_callback(
                        _h_win_event_hook: HWINEVENTHOOK,
                        _event: u32,
                        _hwnd: HWND,
                        _id_object: i32,
                        _id_child: i32,
                        _dw_event_thread: u32,
                        _dw_ms_event_time: u32,
                    ) {
                        unsafe {
                            if let Some(tx) = HOOK_TX.as_ref() {
                                let _ = tx.blocking_send(());
                            }
                        }
                    }

                    static mut HOOK_TX: Option<tokio::sync::mpsc::Sender<()>> = None;
                    HOOK_TX = Some(tx);

                    let hook = SetWinEventHook(
                        EVENT_SYSTEM_FOREGROUND,
                        EVENT_SYSTEM_FOREGROUND,
                        None,
                        Some(winevent_callback),
                        0,
                        0,
                        WINEVENT_OUTOFCONTEXT,
                    );

                    if !hook.is_invalid() {
                        let mut msg = MSG::default();
                        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                            DispatchMessageW(&msg);
                        }
                        UnhookWinEvent(hook);
                    }
                }
            });

            let mut detector = crate::game_detector::GameDetector::new();
            let _ = detector.enumerate_windows();

            loop {
                // Wait for either a foreground-change event or a 60s fallback poll.
                tokio::select! {
                    _ = rx.recv() => {
                        // Debounce: wait 500ms after the last foreground change to let
                        // rapid Alt-Tab sequences settle before we commit to a decision.
                        cx.background_executor().timer(std::time::Duration::from_millis(500)).await;
                    }
                    _ = cx.background_executor().timer(std::time::Duration::from_secs(60)) => {}
                }

                let is_recording = app_state.is_recording.load(std::sync::atomic::Ordering::SeqCst);
                if is_recording { continue; }

                let config = crate::config::AppConfig::load();
                let windows = detector.enumerate_windows();

                let mut target_match = None;

                for (game_title, settings) in &config.game_registry {
                    if !settings.auto_record { continue; }

                    if let Some(target_proc) = &settings.target_process {
                        if let Some(win) = windows.iter().find(|w| &w.process_name == target_proc) {
                            target_match = Some((game_title.clone(), win.hwnd));
                            break;
                        }
                    }
                }

                if let Some((game, hwnd)) = target_match {
                    let workspace_weak = match workspace_handle.lock() {
                        Ok(guard) => guard.clone(),
                        Err(_) => { continue; }
                    };
                    if let Some(workspace_weak) = workspace_weak {
                        let _ = cx.update(|cx| {
                            if let Some(workspace_entity) = workspace_weak.upgrade() {
                                if let Some(any_window) = cx.windows().first().cloned() {
                                    let _ = any_window.update(cx, |_, window, cx| {
                                        workspace_entity.update(cx, |workspace, cx| {
                                            println!("[Auto-Record] Event-Driven Match Found: {} (HWND: {})", game, hwnd);
                                            workspace.selected_source = Some(game.clone());
                                            workspace.refresh_available_windows(cx);
                                            workspace.toggle_recording_ext(Some(hwnd), window, cx);
                                        });
                                    });
                                }
                            }
                        });
                    }
                }
            }
        }
    }).detach();
}
