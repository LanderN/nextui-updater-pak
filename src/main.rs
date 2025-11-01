#![deny(clippy::all)]
#![warn(clippy::pedantic)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::cast_possible_truncation)]
#![allow(dead_code)]

use app_state::AppStateManager;
use std::thread;
use ui::run_ui;
use update::{do_nextui_release_check, do_self_update};

mod app_state;
mod github;
mod ui;
mod update;

// Constants
pub const SDCARD_ROOT: &str = "/mnt/SDCARD/";

// Error type for the application
pub type Result<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

fn main() -> Result<()> {
    // Initialize application state
    let app_state: &'static AppStateManager = Box::leak(Box::new(AppStateManager::new()));

    // Get current NextUI version
    let version_file =
        std::fs::read_to_string(SDCARD_ROOT.to_owned() + ".system/version.txt").unwrap_or_default();
    let current_sha = version_file
        .lines()
        .nth(1)
        .map(std::borrow::ToOwned::to_owned);
    app_state.set_current_version(current_sha);

    // Self-update
    let app_state_clone = app_state.clone();
    thread::spawn(move || {
        do_self_update(&app_state_clone);
        do_nextui_release_check(&app_state_clone);
    });

    run_ui(app_state)?;

    Ok(())
}
