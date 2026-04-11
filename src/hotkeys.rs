use std::sync::mpsc;

/// Actions that can be triggered by global hotkeys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotkeyAction {
    ToggleRecording,
    SaveClip,
    ToggleMic,
    PushToTalk,
    MarkerFlag,
    MarkerKill,
    MarkerDeath,
    MarkerHighlight,
}

const HOTKEY_ID_TOGGLE_RECORDING: i32 = 1;
const HOTKEY_ID_SAVE_CLIP: i32 = 2;
const HOTKEY_ID_TOGGLE_MIC: i32 = 3;
const HOTKEY_ID_PUSH_TO_TALK: i32 = 4;
const HOTKEY_ID_MARKER_FLAG: i32 = 5;
const HOTKEY_ID_MARKER_KILL: i32 = 6;
const HOTKEY_ID_MARKER_DEATH: i32 = 7;
const HOTKEY_ID_MARKER_HIGHLIGHT: i32 = 8;

pub fn vk_to_string(vk: u32, modifiers: u32) -> String {
    if vk == 0 {
        return "None".to_string();
    }

    let mut res = String::new();
    if modifiers & 0x0002 != 0 { res.push_str("Ctrl+"); }
    if modifiers & 0x0001 != 0 { res.push_str("Alt+"); }
    if modifiers & 0x0004 != 0 { res.push_str("Shift+"); }
    if modifiers & 0x0008 != 0 { res.push_str("Win+"); }

    let key = match vk {
        0x70..=0x7B => format!("F{}", vk - 0x6F),
        0x41..=0x5A => ((vk as u8) as char).to_string(),
        0x30..=0x39 => ((vk as u8) as char).to_string(),
        0x20 => "Space".to_string(),
        0x0D => "Enter".to_string(),
        0x09 => "Tab".to_string(),
        0x24 => "Home".to_string(),
        0x23 => "End".to_string(),
        0x21 => "PgUp".to_string(),
        0x22 => "PgDn".to_string(),
        0x2D => "Insert".to_string(),
        0x2E => "Delete".to_string(),
        0x25 => "Left".to_string(),
        0x27 => "Right".to_string(),
        0x26 => "Up".to_string(),
        0x28 => "Down".to_string(),
        _ => format!("0x{:02X}", vk),
    };

    res.push_str(&key);
    res
}

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
                    if vk == 0 { return false; } // unbound
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
                let ok4 = reg(HOTKEY_ID_PUSH_TO_TALK, hk.push_to_talk_mod, hk.push_to_talk_vk);
                let ok5 = reg(HOTKEY_ID_MARKER_FLAG, hk.marker_flag_mod, hk.marker_flag_vk);
                let ok6 = reg(HOTKEY_ID_MARKER_KILL, hk.marker_kill_mod, hk.marker_kill_vk);
                let ok7 = reg(HOTKEY_ID_MARKER_DEATH, hk.marker_death_mod, hk.marker_death_vk);
                let ok8 = reg(HOTKEY_ID_MARKER_HIGHLIGHT, hk.marker_highlight_mod, hk.marker_highlight_vk);

                if ok1 { log::info!("[Hotkeys] Registered: Toggle Recording (vk=0x{:02X})", hk.toggle_recording_vk); }
                else if hk.toggle_recording_vk != 0 { log::warn!("[Hotkeys] Failed to register Toggle Recording hotkey"); }
                if ok2 { log::info!("[Hotkeys] Registered: Save Clip (vk=0x{:02X})", hk.save_clip_vk); }
                if ok3 { log::info!("[Hotkeys] Registered: Toggle Mic (vk=0x{:02X})", hk.toggle_mic_vk); }
                if ok4 { log::info!("[Hotkeys] Registered: Push-to-Talk (vk=0x{:02X})", hk.push_to_talk_vk); }
                if ok5 { log::info!("[Hotkeys] Registered: Marker Flag (vk=0x{:02X})", hk.marker_flag_vk); }
                if ok6 { log::info!("[Hotkeys] Registered: Marker Kill (vk=0x{:02X})", hk.marker_kill_vk); }
                if ok7 { log::info!("[Hotkeys] Registered: Marker Death (vk=0x{:02X})", hk.marker_death_vk); }
                if ok8 { log::info!("[Hotkeys] Registered: Marker Highlight (vk=0x{:02X})", hk.marker_highlight_vk); }

                let mut msg = MSG::default();
                while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                    if msg.message == WM_HOTKEY {
                        let action = match msg.wParam.0 as i32 {
                            HOTKEY_ID_TOGGLE_RECORDING => Some(HotkeyAction::ToggleRecording),
                            HOTKEY_ID_SAVE_CLIP => Some(HotkeyAction::SaveClip),
                            HOTKEY_ID_TOGGLE_MIC => Some(HotkeyAction::ToggleMic),
                            HOTKEY_ID_PUSH_TO_TALK => Some(HotkeyAction::PushToTalk),
                            HOTKEY_ID_MARKER_FLAG => Some(HotkeyAction::MarkerFlag),
                            HOTKEY_ID_MARKER_KILL => Some(HotkeyAction::MarkerKill),
                            HOTKEY_ID_MARKER_DEATH => Some(HotkeyAction::MarkerDeath),
                            HOTKEY_ID_MARKER_HIGHLIGHT => Some(HotkeyAction::MarkerHighlight),
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
                let _ = UnregisterHotKey(None, HOTKEY_ID_PUSH_TO_TALK);
                let _ = UnregisterHotKey(None, HOTKEY_ID_MARKER_FLAG);
                let _ = UnregisterHotKey(None, HOTKEY_ID_MARKER_KILL);
                let _ = UnregisterHotKey(None, HOTKEY_ID_MARKER_DEATH);
                let _ = UnregisterHotKey(None, HOTKEY_ID_MARKER_HIGHLIGHT);
            }
        })
        .ok();

    rx
}
