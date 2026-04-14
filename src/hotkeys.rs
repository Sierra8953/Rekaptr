use std::sync::mpsc;

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

struct HotkeyBinding {
    id: i32,
    action: HotkeyAction,
    label: &'static str,
}

const BINDINGS: &[HotkeyBinding] = &[
    HotkeyBinding {
        id: 1,
        action: HotkeyAction::ToggleRecording,
        label: "Toggle Recording",
    },
    HotkeyBinding {
        id: 2,
        action: HotkeyAction::SaveClip,
        label: "Save Clip",
    },
    HotkeyBinding {
        id: 3,
        action: HotkeyAction::ToggleMic,
        label: "Toggle Mic",
    },
    HotkeyBinding {
        id: 4,
        action: HotkeyAction::PushToTalk,
        label: "Push-to-Talk",
    },
    HotkeyBinding {
        id: 5,
        action: HotkeyAction::MarkerFlag,
        label: "Marker Flag",
    },
    HotkeyBinding {
        id: 6,
        action: HotkeyAction::MarkerKill,
        label: "Marker Kill",
    },
    HotkeyBinding {
        id: 7,
        action: HotkeyAction::MarkerDeath,
        label: "Marker Death",
    },
    HotkeyBinding {
        id: 8,
        action: HotkeyAction::MarkerHighlight,
        label: "Marker Highlight",
    },
];

pub fn vk_to_string(vk: u32, modifiers: u32) -> String {
    if vk == 0 {
        return "None".to_string();
    }

    let mut res = String::new();
    if modifiers & 0x0002 != 0 {
        res.push_str("Ctrl+");
    }
    if modifiers & 0x0001 != 0 {
        res.push_str("Alt+");
    }
    if modifiers & 0x0004 != 0 {
        res.push_str("Shift+");
    }
    if modifiers & 0x0008 != 0 {
        res.push_str("Win+");
    }

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

/// Returns (modifiers, vk) for a binding from the config, by hotkey ID.
fn get_binding_keys(hk: &crate::config::HotkeyConfig, id: i32) -> (u32, u32) {
    match id {
        1 => (hk.toggle_recording_mod, hk.toggle_recording_vk),
        2 => (hk.save_clip_mod, hk.save_clip_vk),
        3 => (hk.toggle_mic_mod, hk.toggle_mic_vk),
        4 => (hk.push_to_talk_mod, hk.push_to_talk_vk),
        5 => (hk.marker_flag_mod, hk.marker_flag_vk),
        6 => (hk.marker_kill_mod, hk.marker_kill_vk),
        7 => (hk.marker_death_mod, hk.marker_death_vk),
        8 => (hk.marker_highlight_mod, hk.marker_highlight_vk),
        _ => (0, 0),
    }
}

/// Starts a background thread that registers global hotkeys and sends
/// `HotkeyAction` messages when they are pressed.
pub fn start_hotkey_listener() -> mpsc::Receiver<HotkeyAction> {
    let (tx, rx) = mpsc::channel();

    std::thread::Builder::new()
        .name("Luma Hotkeys".to_string())
        .spawn(move || unsafe {
            use windows::Win32::UI::Input::KeyboardAndMouse::*;
            use windows::Win32::UI::WindowsAndMessaging::*;

            let config = crate::config::AppConfig::load();
            let hk = &config.hotkeys;

            for binding in BINDINGS {
                let (modifiers, vk) = get_binding_keys(hk, binding.id);
                if vk == 0 {
                    continue;
                }
                let ok =
                    RegisterHotKey(None, binding.id, HOT_KEY_MODIFIERS(modifiers | 0x4000), vk)
                        .is_ok();
                if ok {
                    log::info!("[Hotkeys] Registered: {} (vk=0x{:02X})", binding.label, vk);
                } else {
                    log::warn!("[Hotkeys] Failed to register {} hotkey", binding.label);
                }
            }

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                if msg.message == WM_HOTKEY {
                    let id = msg.wParam.0 as i32;
                    if let Some(binding) = BINDINGS.iter().find(|b| b.id == id) {
                        log::info!("[Hotkeys] Action triggered: {:?}", binding.action);
                        let _ = tx.send(binding.action);
                    }
                }
                DispatchMessageW(&msg);
            }

            for binding in BINDINGS {
                let _ = UnregisterHotKey(None, binding.id);
            }
        })
        .ok();

    rx
}
