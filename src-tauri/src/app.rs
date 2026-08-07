//! Application bootstrap: load config, init logging, connect the database,
//! assemble `AppState`, and hand off to the Tauri event loop.
//!
//! Kept separate from `main.rs` so the wiring is unit-testable in
//! principle and so `main.rs` stays a one-line entry point.

use anyhow::{Context, Result};

use crate::paths::AppPaths;
use crate::state::AppState;

pub fn run() -> Result<()> {
    let paths = AppPaths::resolve().context("failed to resolve application directories")?;

    let settings = storybloom_config::Settings::load(&paths.config_dir)
        .with_context(|| format!("failed to load configuration from {:?}", paths.config_dir))?;

    // Held for the lifetime of the process - dropping it stops the file
    // log writer. Bound in `run`'s scope so it lives until `.run()` returns.
    let _logging_guard = crate::logging::init(&settings.logging, &paths.log_dir)
        .context("failed to initialize logging")?;

    tracing::info!(
        environment = %settings.app.environment,
        data_dir = %paths.data_dir.display(),
        "starting {}",
        settings.app.name
    );

    let db_path = paths.data_dir.join(&settings.database.file_name);
    let max_connections = settings.database.max_connections;
    let run_migrations_on_startup = settings.database.run_migrations_on_startup;

    // Config loading and logging init are synchronous, but connecting to
    // SQLite and running migrations are async (`sqlx`). We're not inside a
    // Tokio runtime yet at this point in startup, so drive this one future
    // to completion via Tauri's async runtime (backed by Tokio) rather
    // than pulling in `#[tokio::main]` for the whole binary - everything
    // after this point runs inside Tauri's own event loop.
    let db = tauri::async_runtime::block_on(async {
        let pool = storybloom_db::connect(&db_path, max_connections).await?;
        if run_migrations_on_startup {
            storybloom_db::run_migrations(&pool).await?;
        }
        anyhow::Ok(pool)
    })
    .context("failed to initialize the database")?;

    let state = AppState::new(settings, db).context("failed to assemble application state")?;

    tauri::Builder::default()
        .manage(state)
        .invoke_handler(tauri::generate_handler![crate::commands::ping])
        .setup(|_app| {
            tracing::info!("Tauri setup complete");
            Ok(())
        })
        .run(tauri::generate_context!())
        .context("error while running the Tauri application")?;

    Ok(())
}
