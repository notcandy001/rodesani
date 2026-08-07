//! Shared application state, managed by Tauri and injected into commands
//! via `tauri::State<AppState>`.

use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use storybloom_config::Settings;
use storybloom_core::{CoreError, StoryEngine, StoryEngineConfig};
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
    /// client failing to build). A missing OpenAI API key is *not* treated
    /// as fatal - the app still starts with `story_engine` set to `None`,
    /// since there's no UI yet that depends on it being present.
    pub fn new(settings: Settings, db: DbPool) -> Result<Self> {
        let story_engine = build_story_engine(&settings)?.map(Arc::new);

        let view_models = Arc::new(AppViewModels::new(db.clone(), story_engine));

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
