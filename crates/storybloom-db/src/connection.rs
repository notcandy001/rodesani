use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::SqlitePool;

/// Type alias so callers don't need to depend on `sqlx` directly.
pub type DbPool = SqlitePool;

/// Open (creating if necessary) a SQLite database at `db_path` and return a
/// connection pool sized by `max_connections`.
///
/// Applies a handful of pragmas that matter for a desktop app writing to a
/// local file: WAL mode for concurrent readers, a busy timeout so writers
/// don't fail fast under contention, and foreign key enforcement.
pub async fn connect(db_path: impl AsRef<Path>, max_connections: u32) -> Result<DbPool> {
    let db_path = db_path.as_ref();

    if let Some(parent) = db_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create database directory at {parent:?}"))?;
    }

    let options = SqliteConnectOptions::new()
        .filename(db_path)
        .create_if_missing(true)
        .foreign_keys(true)
        .busy_timeout(Duration::from_secs(5))
        .journal_mode(sqlx::sqlite::SqliteJournalMode::Wal)
        .synchronous(sqlx::sqlite::SqliteSynchronous::Normal);

    let pool = SqlitePoolOptions::new()
        .max_connections(max_connections)
        .connect_with(options)
        .await
        .with_context(|| format!("failed to connect to SQLite database at {db_path:?}"))?;

    tracing::info!(path = %db_path.display(), max_connections, "connected to SQLite database");

    Ok(pool)
}
