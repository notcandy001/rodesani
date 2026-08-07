//! Resolves the filesystem locations the app depends on: where to read
//! configuration from, and where to write persistent data (SQLite file,
//! logs). Centralized here so `app.rs` stays focused on wiring, not path
//! arithmetic, and so the platform-specific bits (macOS `.app` bundle
//! layout, XDG dirs on Linux, `%APPDATA%` on Windows) live in one place.

use std::path::PathBuf;

use anyhow::{Context, Result};
use directories::ProjectDirs;

pub struct AppPaths {
    /// Directory containing `default.toml` / `{env}.toml` / `local.toml`.
    pub config_dir: PathBuf,
    /// Directory for persistent app data, e.g. the SQLite database file.
    pub data_dir: PathBuf,
    /// Directory for rotating log files.
    pub log_dir: PathBuf,
}

impl AppPaths {
    pub fn resolve() -> Result<Self> {
        let project_dirs = ProjectDirs::from("com", "notcandy001", "StoryBloom Studio")
            .context("failed to determine platform application directories")?;

        let data_dir = project_dirs.data_dir().to_path_buf();
        let log_dir = data_dir.join("logs");

        Ok(Self {
            config_dir: resolve_config_dir(),
            data_dir,
            log_dir,
        })
    }
}

/// Config resolution order:
///
/// 1. `STORYBLOOM_CONFIG_DIR` env var, if set - explicit override, always
///    wins (useful for tests / CI / power users).
/// 2. A `config/` directory next to the running executable - the layout
///    produced by bundling `../config` as a Tauri resource in production
///    builds (see `bundle.resources` in `tauri.conf.json`).
/// 3. macOS `.app` bundle layout: `Contents/Resources/config`, since on
///    macOS the executable lives in `Contents/MacOS/`.
/// 4. The workspace `config/` directory, resolved at compile time relative
///    to this crate - the development fallback when running via
///    `cargo run` / `cargo tauri dev`.
fn resolve_config_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("STORYBLOOM_CONFIG_DIR") {
        return PathBuf::from(dir);
    }

    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            let beside_exe = exe_dir.join("config");
            if beside_exe.is_dir() {
                return beside_exe;
            }

            let macos_resources = exe_dir.join("../Resources/config");
            if macos_resources.is_dir() {
                return macos_resources;
            }
        }
    }

    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../config")
}
