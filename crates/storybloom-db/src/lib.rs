//! SQLite persistence layer.
//!
//! This crate owns the connection pool and migration runner. It
//! deliberately knows nothing about domain models (those live in
//! `storybloom-core`); its only job is "give me a healthy `SqlitePool`".

mod connection;

pub use connection::{connect, DbPool};

use anyhow::{Context, Result};

/// Run all pending migrations embedded from `src/migrations`.
///
/// Safe to call on every startup: `sqlx::migrate!` tracks applied
/// migrations in a bookkeeping table and is a no-op once up to date.
pub async fn run_migrations(pool: &DbPool) -> Result<()> {
    sqlx::migrate!("./src/migrations")
        .run(pool)
        .await
        .context("failed to run database migrations")?;
    Ok(())
}
