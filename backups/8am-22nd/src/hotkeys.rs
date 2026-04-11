use std::sync::mpsc;

/// Actions that can be triggered by global hotkeys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    ToggleRecording,
    SaveClip,
    ToggleMic,
}

const HOTKEY_ID_TOGGLE_RECORDING: i32 = 1;
const HOTKEY_ID_SAVE_CLIP: i32 = 2;
const HOTKEY_ID_TOGGLE_MIC: i32 = 3;

/// Starts a background thread that registers global hotkeys and sends
/// `HotkeyAction` messages when they are pressed.
///
/// Returns a `mpsc::Receiver` that the UI thread can poll for actions.
pub fn start_hotkey_listener() -> mpsc::Receiver<HotkeyAction> {
    let (tx, rx) = mpsc::channel();

    std::thread::Builder::new()
        .name("Luma Hotkeys".to_string())
        .spawn(move || {
            unsafe {
                use windows::Win32::UI::Input::KeyboardAndMouse::*;
                use windows::Win32::UI::WindowsAndMessaging::*;
                use windows::Win32::Foundation::*;

                let config = crate::config::AppConfig::load();
                let hk = &config.hotkeys;

                // RegisterHotKey requires HOT_KEY_MODIFIERS
                let reg = |id: i32, modifiers: u32, vk: u32| -> bool {
                    RegisterHotKey(
                        None,
                        id,
                        HOT_KEY_MODIFIERS(modifiers | 0x4000), // MOD_NOREPEAT = 0x4000
                        vk,
                    ).is_ok()
                };

                let ok1 = reg(HOTKEY_ID_TOGGLE_RECORDING, hk.toggle_recording_mod, hk.toggle_recording_vk);
                let ok2 = reg(HOTKEY_ID_SAVE_CLIP, hk.save_clip_mod, hk.save_clip_vk);
                let ok3 = reg(HOTKEY_ID_TOGGLE_MIC, hk.toggle_mic_mod, hk.toggle_mic_vk);

                if ok1 { log::info!("[Hotkeys] Registered: Toggle Recording (vk=0x{:02X})", hk.toggle_recording_vk); }
                else { log::warn!("[Hotkeys] Failed to register Toggle Recording hotkey"); }
                if ok2 { log::info!("[Hotkeys] Registered: Save Clip (vk=0x{:02X})", hk.save_clip_vk); }
                if ok3 { log::info!("[Hotkeys] Registered: Toggle Mic (vk=0x{:02X})", hk.toggle_mic_vk); }

                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    if msg.message == WM_HOTKEY {
                        let action = match msg.wParam.0 as i32 {
                            HOTKEY_ID_TOGGLE_RECORDING => Some(HotkeyAction::ToggleRecording),
                            HOTKEY_ID_SAVE_CLIP => Some(HotkeyAction::SaveClip),
                            HOTKEY_ID_TOGGLE_MIC => Some(HotkeyAction::ToggleMic),
                            _ => None,
                        };
                        if let Some(action) = action {
                            log::info!("[Hotkeys] Action triggered: {:?}", action);
                            let _ = tx.send(action);
                        }
                    }
                    DispatchMessageW(&msg);
                }

                // Cleanup
                let _ = UnregisterHotKey(None, HOTKEY_ID_TOGGLE_RECORDING);
                let _ = UnregisterHotKey(None, HOTKEY_ID_SAVE_CLIP);
                let _ = UnregisterHotKey(None, HOTKEY_ID_TOGGLE_MIC);
            }
        })
        .ok();

    rx
}
