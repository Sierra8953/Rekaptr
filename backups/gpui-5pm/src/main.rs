#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod audio;
mod config;
mod engine;
mod game_detector;
mod state;
mod ui;
mod utils;
mod video_player;

use crate::state::AppState;
use crate::ui::LumaWorkspace;
use anyhow::Result;
use gpui::*;
use parking_lot::Mutex;
use std::sync::Arc;

fn main() -> Result<()> {
    gstreamer::init()?;
    let app = gpui::Application::new();

    app.run(move |cx: &mut gpui::App| {
        adabraka_ui::init(cx);
        adabraka_ui::theme::install_theme(cx, adabraka_ui::theme::Theme::dark());
        
        let app_state = Arc::new(Mutex::new(AppState::new()));
        
        let bounds = Bounds::centered(None, size(px(1400.0), px(900.0)), cx);
        let options = WindowOptions {
            window_bounds: Some(WindowBounds::Windowed(bounds)),
            window_min_size: Some(size(px(800.0), px(600.0))),
            ..Default::default()
        };

        cx.open_window(options, |_window, cx| {
            cx.new(|cx| LumaWorkspace::new(app_state.clone(), cx))
        }).unwrap();
    });

    Ok(())
}
