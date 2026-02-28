pub mod app;
pub mod components;
pub mod csv_loader;
pub mod gui;
pub mod logging;
pub mod models;
pub mod themes;
pub mod utils;

use gpui::{App, actions};
pub use gui::setup_app;
use tracing::info;

actions!(gpui_demo, [Quit]);

// Takes a reference to the action (often unused) and mutable app context
pub fn quit(
    _: &Quit,
    cx: &mut App,
) {
    info!("Executing quit handler");
    cx.quit();
}
