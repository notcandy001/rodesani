// Suppresses the extra console window on Windows in release builds. Has no
// effect on other platforms.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod app;
mod commands;
mod logging;
mod paths;
mod state;
mod viewmodels;

fn main() {
    if let Err(err) = app::run() {
        // Logging may not be initialized yet if startup failed early (e.g.
        // bad config), so also print to stderr to guarantee visibility.
        eprintln!("fatal error: {err:?}");
        std::process::exit(1);
    }
}
