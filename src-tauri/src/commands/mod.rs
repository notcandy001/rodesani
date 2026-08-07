//! Tauri commands - the View's entry points into the Rust backend.
//!
//! Each `#[tauri::command]` function should stay thin: pull what it needs
//! out of `AppState`, delegate to a view-model, and map errors into
//! whatever `Result<T, String>` (or a dedicated serializable error type)
//! the frontend expects.
//!
//! New commands: add the `#[tauri::command]` fn here (or in a new
//! submodule re-exported from this file), then add its name to the
//! `tauri::generate_handler![...]` list in `app.rs`.

/// A basic health-check command, useful for confirming the Rust <-> WebView
/// bridge is wired correctly before real commands exist.
#[tauri::command]
pub fn ping() -> &'static str {
    "pong"
}
