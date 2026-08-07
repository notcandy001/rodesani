//! `tracing` initialization.
//!
//! Sets up a subscriber that writes human-readable (or JSON, per config)
//! logs to stdout, plus an optional daily-rotating file appender under the
//! platform-appropriate app data directory. Returns the file-appender
//! guard, which the caller must keep alive for the lifetime of the
//! process - dropping it flushes and stops background log writing.

use std::path::Path;

use anyhow::{Context, Result};
use storybloom_config::LoggingSettings;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::layer::{Layered, SubscriberExt};
use tracing_subscriber::{fmt, util::SubscriberInitExt, EnvFilter, Layer, Registry};

/// Must be held for the lifetime of `main` - see module docs.
pub struct LoggingGuard(#[allow(dead_code)] Option<WorkerGuard>);

/// The subscriber type immediately after `EnvFilter` has been layered onto
/// the base `Registry` - i.e. what any further layer added via `.with(...)`
/// must implement `Layer<_>` for. Named so the boxed stdout layer below can
/// be typed against the exact subscriber it will be attached to, rather
/// than the bare `Registry` (which is a different, incompatible type once
/// `env_filter` is in the chain).
type FilteredRegistry = Layered<EnvFilter, Registry>;

pub fn init(settings: &LoggingSettings, log_dir: &Path) -> Result<LoggingGuard> {
    let env_filter = EnvFilter::try_new(&settings.level)
        .with_context(|| format!("invalid logging.level directive: {}", settings.level))?;

    let stdout_layer: Box<dyn Layer<FilteredRegistry> + Send + Sync> = if settings.json {
        Box::new(fmt::layer().with_target(true).with_level(true).json())
    } else {
        Box::new(fmt::layer().with_target(true).with_level(true))
    };

    let (file_layer, guard) = if settings.file_enabled {
        std::fs::create_dir_all(log_dir)
            .with_context(|| format!("failed to create log directory at {log_dir:?}"))?;

        let file_appender = tracing_appender::rolling::daily(log_dir, "storybloom.log");
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        let layer = fmt::layer().with_ansi(false).with_writer(non_blocking);
        (Some(layer), Some(guard))
    } else {
        (None, None)
    };

    tracing_subscriber::registry()
        .with(env_filter)
        .with(stdout_layer)
        .with(file_layer)
        .try_init()
        .context("failed to install global tracing subscriber")?;

    tracing::info!(
        level = %settings.level,
        json = settings.json,
        file_enabled = settings.file_enabled,
        "logging initialized"
    );

    Ok(LoggingGuard(guard))
}
