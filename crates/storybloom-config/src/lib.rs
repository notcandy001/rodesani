//! Configuration system for StoryBloom Studio.
//!
//! Configuration is layered, in increasing priority order:
//!
//! 1. `config/default.toml`            - baked-in defaults, always loaded.
//! 2. `config/{environment}.toml`      - environment-specific overrides
//!    (`development`, `production`, ...), selected via `STORYBLOOM_ENV`.
//! 3. `config/local.toml`              - optional, developer-local, gitignored.
//! 4. Environment variables prefixed with `STORYBLOOM__`, double-underscore
//!    delimited (e.g. `STORYBLOOM__DATABASE__MAX_CONNECTIONS=10`).
//!
//! Nothing here is feature logic - this crate's only job is to produce a
//! validated, strongly typed [`Settings`] value for the rest of the
//! application to depend on.

use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Top level, strongly typed application configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub app: AppSettings,
    pub logging: LoggingSettings,
    pub database: DatabaseSettings,
    pub window: WindowSettings,
    pub ai: AiSettings,
    pub elevenlabs: ElevenLabsSettings,
    pub whisper: WhisperSettings,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// Human readable application name, used in window titles, logs, etc.
    pub name: String,
    /// Deployment environment: "development", "staging", "production".
    pub environment: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoggingSettings {
    /// Default `tracing` filter directive, e.g. "info" or "storybloom=debug,warn".
    pub level: String,
    /// Emit logs as newline-delimited JSON instead of human-readable text.
    pub json: bool,
    /// Whether to also write logs to a rotating file under the app data dir.
    pub file_enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseSettings {
    /// SQLite file name, resolved relative to the app data directory unless
    /// an absolute path is given.
    pub file_name: String,
    pub max_connections: u32,
    pub run_migrations_on_startup: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowSettings {
    pub title: String,
    pub width: f64,
    pub height: f64,
    pub resizable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiSettings {
    /// Which provider this config targets. Only "openai" is supported today,
    /// but keeping it explicit makes a future provider switch a config
    /// change rather than a code change.
    pub provider: String,
    /// API base URL, without a trailing slash or path suffix (e.g.
    /// `https://api.openai.com/v1`). Overridable per-environment for
    /// proxies or OpenAI-compatible endpoints.
    pub base_url: String,
    /// Model identifier, e.g. "gpt-4o-mini".
    pub model: String,
    /// API key. Left empty in committed TOML files on purpose - set it via
    /// the `STORYBLOOM__AI__API_KEY` env var, or `local.toml` (gitignored).
    /// `storybloom-core` also accepts a plain `OPENAI_API_KEY` env var as a
    /// fallback if this is left empty, since that's the convention most
    /// OpenAI tooling expects.
    #[serde(default)]
    pub api_key: String,
    pub temperature: f32,
    pub max_output_tokens: u32,
    pub request_timeout_seconds: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ElevenLabsSettings {
    /// API base URL, without a trailing slash (e.g.
    /// `https://api.elevenlabs.io/v1`).
    pub base_url: String,
    /// Which voice to synthesize with, e.g. a voice ID from the ElevenLabs
    /// voice library.
    pub voice_id: String,
    /// Model identifier, e.g. "eleven_multilingual_v2".
    pub model_id: String,
    /// Requested output encoding, e.g. "mp3_44100_128".
    pub output_format: String,
    /// API key. Left empty in committed TOML files on purpose - set it via
    /// the `STORYBLOOM__ELEVENLABS__API_KEY` env var, or `local.toml`
    /// (gitignored). `storybloom-core` also accepts a plain
    /// `ELEVENLABS_API_KEY` env var as a fallback if this is left empty.
    #[serde(default)]
    pub api_key: String,
    pub request_timeout_seconds: u64,
    /// Number of retry attempts after the initial request fails, for
    /// transient errors (timeouts, HTTP 429/5xx) only.
    pub max_retries: u32,
    /// Base delay for exponential backoff between retries, in
    /// milliseconds. Actual delay is `base * 2^attempt`, capped at 30s.
    pub retry_base_delay_ms: u64,
    /// Directory generated narration MP3s are saved to, relative to the
    /// application's working directory unless given as an absolute path.
    pub output_dir: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhisperSettings {
    /// Path to a whisper.cpp GGML/GGUF model file (e.g.
    /// `models/ggml-base.en.bin`), relative to the app's working directory
    /// unless given as an absolute path. Not distributed with this repo -
    /// download separately (see README) since model files run from tens to
    /// hundreds of megabytes.
    pub model_path: String,
    /// Language code whisper.cpp expects (e.g. "en"), or "auto" to
    /// auto-detect from the audio itself (slower, and less accurate for
    /// short clips).
    pub language: String,
    /// Threads whisper.cpp uses internally for inference. Roughly: set to
    /// your machine's physical core count for best throughput.
    pub threads: u32,
}

impl Settings {
    /// Load configuration from `config_dir`, layering environment-specific
    /// and local overrides on top of defaults, then environment variables.
    ///
    /// `config_dir` is typically the `config/` directory shipped alongside
    /// the application (bundled as a resource in release builds).
    pub fn load(config_dir: impl Into<PathBuf>) -> Result<Self> {
        let config_dir = config_dir.into();
        let environment = std::env::var("STORYBLOOM_ENV").unwrap_or_else(|_| "development".into());

        let default_path = config_dir.join("default.toml");
        let env_path = config_dir.join(format!("{environment}.toml"));
        let local_path = config_dir.join("local.toml");

        let mut builder = config::Config::builder()
            .add_source(config::File::from(default_path).required(true));

        if env_path.exists() {
            builder = builder.add_source(config::File::from(env_path).required(false));
        }
        if local_path.exists() {
            builder = builder.add_source(config::File::from(local_path).required(false));
        }

        builder = builder.add_source(
            config::Environment::with_prefix("STORYBLOOM")
                .separator("__")
                .try_parsing(true),
        );

        let raw = builder
            .build()
            .with_context(|| format!("failed to build configuration from {config_dir:?}"))?;

        let settings: Settings = raw
            .try_deserialize()
            .context("failed to deserialize configuration into Settings")?;

        settings.validate()?;
        Ok(settings)
    }

    /// Basic sanity checks so misconfiguration fails fast at startup rather
    /// than surfacing as a confusing error deep in the app.
    fn validate(&self) -> Result<()> {
        anyhow::ensure!(!self.app.name.trim().is_empty(), "app.name must not be empty");
        anyhow::ensure!(
            self.database.max_connections > 0,
            "database.max_connections must be greater than zero"
        );
        anyhow::ensure!(
            self.window.width > 0.0 && self.window.height > 0.0,
            "window dimensions must be positive"
        );
        anyhow::ensure!(
            (0.0..=2.0).contains(&self.ai.temperature),
            "ai.temperature must be between 0.0 and 2.0"
        );
        anyhow::ensure!(
            self.ai.max_output_tokens > 0,
            "ai.max_output_tokens must be greater than zero"
        );
        anyhow::ensure!(
            self.ai.request_timeout_seconds > 0,
            "ai.request_timeout_seconds must be greater than zero"
        );
        anyhow::ensure!(
            !self.elevenlabs.voice_id.trim().is_empty(),
            "elevenlabs.voice_id must not be empty"
        );
        anyhow::ensure!(
            !self.elevenlabs.output_format.trim().is_empty(),
            "elevenlabs.output_format must not be empty"
        );
        anyhow::ensure!(
            self.elevenlabs.request_timeout_seconds > 0,
            "elevenlabs.request_timeout_seconds must be greater than zero"
        );
        anyhow::ensure!(
            !self.elevenlabs.output_dir.trim().is_empty(),
            "elevenlabs.output_dir must not be empty"
        );
        anyhow::ensure!(
            !self.whisper.model_path.trim().is_empty(),
            "whisper.model_path must not be empty"
        );
        anyhow::ensure!(
            !self.whisper.language.trim().is_empty(),
            "whisper.language must not be empty (use \"auto\" to auto-detect)"
        );
        anyhow::ensure!(
            self.whisper.threads > 0,
            "whisper.threads must be greater than zero"
        );
        Ok(())
    }
}
