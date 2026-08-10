//! Shared application state, managed by Tauri and injected into commands
//! via `tauri::State<AppState>`.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use storybloom_config::Settings;
use storybloom_core::{
    CoreError, StoryEngine, StoryEngineConfig, SubtitleEngine, SubtitleEngineConfig, VoiceEngine,
    VoiceEngineConfig,
};
use storybloom_db::DbPool;

use crate::viewmodels::AppViewModels;

/// Everything a Tauri command might need to do its job. Cheap to clone
/// (everything inside is an `Arc` or already cheaply cloneable), so it can
/// be handed to spawned tasks without lifetime headaches.
#[derive(Clone)]
pub struct AppState {
    pub settings: Arc<Settings>,
    pub db: DbPool,
    pub view_models: Arc<AppViewModels>,
}

impl AppState {
    /// Fallible only for genuinely unexpected failures (e.g. the HTTP
    /// client failing to build). A missing OpenAI/ElevenLabs API key, or a
    /// missing whisper model file, is *not* treated as fatal - the app
    /// still starts with the affected engine set to `None`, since there's
    /// no UI yet that depends on any of them being present.
    pub fn new(settings: Settings, db: DbPool) -> Result<Self> {
        let story_engine = build_story_engine(&settings)?.map(Arc::new);
        let voice_engine = build_voice_engine(&settings)?.map(Arc::new);

        // `SubtitleEngine::new` is async (model loading runs on
        // `spawn_blocking`), but `AppState::new` itself is synchronous and
        // called before Tauri's async runtime takes over - so drive it to
        // completion here the same way `app.rs` does for the database.
        let subtitle_engine = tauri::async_runtime::block_on(build_subtitle_engine(&settings))?
            .map(Arc::new);

        let view_models = Arc::new(AppViewModels::new(
            db.clone(),
            story_engine,
            voice_engine,
            subtitle_engine,
        ));

        Ok(Self {
            settings: Arc::new(settings),
            db,
            view_models,
        })
    }
}

/// Maps `storybloom_config::AiSettings` onto `StoryEngineConfig` and
/// constructs the engine, falling back to the `OPENAI_API_KEY` environment
/// variable when `ai.api_key` is left empty in config - the convention
/// most OpenAI tooling expects.
///
/// Returns `Ok(None)` (not an error) when no key is available anywhere,
/// so callers can start the app without AI configured.
fn build_story_engine(settings: &Settings) -> Result<Option<StoryEngine>> {
    let api_key = if settings.ai.api_key.trim().is_empty() {
        std::env::var("OPENAI_API_KEY").unwrap_or_default()
    } else {
        settings.ai.api_key.clone()
    };

    let config = StoryEngineConfig {
        base_url: settings.ai.base_url.clone(),
        api_key,
        model: settings.ai.model.clone(),
        temperature: settings.ai.temperature,
        max_output_tokens: settings.ai.max_output_tokens,
        request_timeout: Duration::from_secs(settings.ai.request_timeout_seconds),
    };

    match StoryEngine::new(config) {
        Ok(engine) => Ok(Some(engine)),
        Err(CoreError::MissingApiKey) => {
            tracing::warn!(
                "no OpenAI API key configured (ai.api_key / STORYBLOOM__AI__API_KEY / \
                 OPENAI_API_KEY) - Story Engine is unavailable until one is set"
            );
            Ok(None)
        }
        Err(err) => Err(err.into()),
    }
}

/// Maps `storybloom_config::ElevenLabsSettings` onto `VoiceEngineConfig` and
/// constructs the engine, falling back to the `ELEVENLABS_API_KEY`
/// environment variable when `elevenlabs.api_key` is left empty in config.
///
/// Returns `Ok(None)` (not an error) when no key is available anywhere, so
/// callers can start the app without narration configured.
fn build_voice_engine(settings: &Settings) -> Result<Option<VoiceEngine>> {
    let api_key = if settings.elevenlabs.api_key.trim().is_empty() {
        std::env::var("ELEVENLABS_API_KEY").unwrap_or_default()
    } else {
        settings.elevenlabs.api_key.clone()
    };

    let config = VoiceEngineConfig {
        base_url: settings.elevenlabs.base_url.clone(),
        api_key,
        voice_id: settings.elevenlabs.voice_id.clone(),
        model_id: settings.elevenlabs.model_id.clone(),
        output_format: settings.elevenlabs.output_format.clone(),
        request_timeout: Duration::from_secs(settings.elevenlabs.request_timeout_seconds),
        output_dir: PathBuf::from(&settings.elevenlabs.output_dir),
        max_retries: settings.elevenlabs.max_retries,
        retry_base_delay: Duration::from_millis(settings.elevenlabs.retry_base_delay_ms),
    };

        Err(err) => Err(err.into()),
    }
}

/// Maps `storybloom_config::WhisperSettings` onto `SubtitleEngineConfig`
/// and constructs the engine.
///
/// Returns `Ok(None)` (not an error) when the configured model file
/// doesn't exist, so callers can start the app without a whisper model
/// downloaded yet.
async fn build_subtitle_engine(settings: &Settings) -> Result<Option<SubtitleEngine>> {
    let config = SubtitleEngineConfig {
        model_path: PathBuf::from(&settings.whisper.model_path),
        language: settings.whisper.language.clone(),
        threads: settings.whisper.threads as i32,
    };

    match SubtitleEngine::new(config).await {
        Ok(engine) => Ok(Some(engine)),
        Err(CoreError::WhisperInit(reason)) => {
            tracing::warn!(
                reason = %reason,
                "Subtitle Engine is unavailable until a whisper model is configured \
                 (whisper.model_path / STORYBLOOM__WHISPER__MODEL_PATH)"
            );
            Ok(None)
        }
        Err(err) => Err(err.into()),
    }
}
