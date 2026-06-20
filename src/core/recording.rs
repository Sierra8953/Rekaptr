//! Recording pipeline lifecycle, separated from the UI that triggers it.
//!
//! These are the `AppState`-only halves of the record feature: tearing the
//! GStreamer pipeline down (graceful stop and emergency/error stop), releasing
//! mic subscribers, and keeping the tray icon and in-game overlay in sync. The
//! UI in `ui/recording.rs` owns the parts that genuinely need the window/`cx`
//! (toasts, the bus monitor, the pipeline *launch*), and calls in here for the
//! rest.

use crate::overlay::{self, OverlayEvent};
use crate::state::{AppState, RecordingPhase, TrayCommand};
use gstreamer as gst;
use gstreamer::prelude::*;
use std::sync::Arc;

/// Keep the tray icon and in-game overlay in sync with the recording state.
///
/// Every stop/start path must do this so the tray doesn't get stuck showing
/// "Recording" (which would route the next stop press to start, since the phase
/// is already Idle, and never re-send the reset). `SetStopEnabled` tracks
/// `active` because the Stop control is only meaningful while recording.
pub fn notify_recording_state(app_state: &AppState, active: bool, title: Option<String>) {
    if let Some(tx) = app_state.tray_tx.lock().as_ref() {
        let _ = tx.send(TrayCommand::SetStopEnabled(active));
        let _ = tx.send(TrayCommand::SetRecording(active));
    }
    overlay::send(app_state, OverlayEvent::RecordingChanged { active, title });
}

/// Unsubscribe the mic-provider keys inserted for the active recording session
/// so the mic thread stops pushing into the now-dead AppSrcs.
pub fn clear_mic_subscribers(app_state: &AppState) {
    let keys: Vec<u64> = std::mem::take(&mut *app_state.recording.mic_subscriber_keys.lock());
    if keys.is_empty() {
        return;
    }
    if let Some(provider) = app_state.mic_provider.lock().as_ref() {
        for key in keys {
            provider.subscribers.remove(&key);
        }
    }
}

/// Emergency/error stop: tear the pipeline down immediately, reset phase to
/// Idle, and resync the tray/overlay. Used by the bus-monitor error path.
///
/// `game_dir` is derived from `source` for the deferred `fixup_eos_segments`.
pub fn emergency_stop(app_state: &Arc<AppState>, source: &str) {
    let game_dir = crate::utils::get_storage_root().join(crate::utils::clean_title(source));

    if let Some(pipeline) = app_state.recording.pipeline.lock().take() {
        // Even on an emergency stop, give splitmuxsink a brief window to flush its
        // current fragment via EOS so the final .m4s has a valid mfra/mdat. If EOS
        // doesn't arrive within 500ms, fall through to forced Null and let
        // fixup_eos_segments rename the partial file out of the playable set.
        pipeline.send_event(gst::event::Eos::new());
        if let Some(bus) = pipeline.bus() {
            let _ = bus.timed_pop_filtered(
                gst::ClockTime::from_mseconds(500),
                &[gst::MessageType::Eos, gst::MessageType::Error],
            );
        }
        let _ = pipeline.set_state(gst::State::Null);

        std::thread::spawn(move || {
            std::thread::sleep(std::time::Duration::from_millis(200));
            crate::utils::fixup_eos_segments(&game_dir);
        });
    }
    clear_mic_subscribers(app_state);
    *app_state.recording.phase.lock() = RecordingPhase::Idle;
    // Mirror stop_recording's tray reset: this emergency/error path also ends the
    // recording, so the tray icon and tooltip must revert.
    notify_recording_state(app_state, false, None);
}

/// Graceful stop: spawn the background teardown thread that flushes splitmuxsink
/// via EOS, drives the pipeline to Null, runs `fixup_eos_segments`, then flips
/// the phase back to Idle and clears the teardown guard.
///
/// The caller is expected to have already set the phase to `Stopping` and
/// cleared mic subscribers. If there is no live pipeline, the phase is reset to
/// Idle inline.
pub fn begin_graceful_teardown(app_state: &Arc<AppState>, game_dir: std::path::PathBuf) {
    if let Some(pipeline) = app_state.recording.pipeline.lock().take() {
        let phase_handle = Arc::clone(&app_state.recording.phase);
        let app_state_for_thread = Arc::clone(app_state);
        app_state
            .recording
            .teardown_in_progress
            .store(true, std::sync::atomic::Ordering::Release);
        std::thread::spawn(move || {
            pipeline.send_event(gst::event::Eos::new());
            if let Some(bus) = pipeline.bus() {
                let _ = bus.timed_pop_filtered(gst::ClockTime::from_seconds(5), &[gst::MessageType::Eos]);
            }
            let _ = pipeline.set_state(gst::State::Null);
            std::thread::sleep(std::time::Duration::from_millis(200));
            crate::utils::fixup_eos_segments(&game_dir);
            *phase_handle.lock() = RecordingPhase::Idle;
            app_state_for_thread
                .recording
                .teardown_in_progress
                .store(false, std::sync::atomic::Ordering::Release);
        });
    } else {
        *app_state.recording.phase.lock() = RecordingPhase::Idle;
    }
}
