//! ViewModel layer (the VM in MVVM).
//!
//! View-models sit between Tauri commands (the View's entry points) and
//! `storybloom_core` services (the Model). They translate domain types and
//! `CoreError`s into whatever shape the frontend expects, and hold no
//! business logic of their own - that belongs in `storybloom-core`.

use std::sync::Arc;

use storybloom_core::{StoryEngine, SubtitleEngine, VoiceEngine};
use storybloom_db::DbPool;

/// Single container for every view-model so `AppState` only needs one
/// field. As concrete view-models are added they're constructed here and
/// exposed as fields, e.g. a future `StoryViewModel` wrapping `story_engine`
/// plus persistence for generated stories.
pub struct AppViewModels {
    #[allow(dead_code)]
    db: DbPool,
    /// `None` when no OpenAI API key is configured - the rest of the app
    /// still starts, this just means Story Engine calls aren't available
    /// yet. `Arc` because it holds a pooled `reqwest::Client` that's cheap
    /// to share but not cheap to rebuild.
    pub story_engine: Option<Arc<StoryEngine>>,
    /// `None` when no ElevenLabs API key is configured - the rest of the
    /// app still starts, this just means narration calls aren't available
    /// yet. `Arc` for the same reason as `story_engine`.
    pub voice_engine: Option<Arc<VoiceEngine>>,
    /// `None` when no whisper model file is configured/found - the rest of
    /// the app still starts, this just means subtitle generation isn't
    /// available yet. `Arc` because it holds a loaded whisper.cpp model
    /// that's expensive to reload per call.
    pub subtitle_engine: Option<Arc<SubtitleEngine>>,
}

impl AppViewModels {
    pub fn new(
        db: DbPool,
        story_engine: Option<Arc<StoryEngine>>,
        voice_engine: Option<Arc<VoiceEngine>>,
        subtitle_engine: Option<Arc<SubtitleEngine>>,
    ) -> Self {
        Self {
            db,
            story_engine,
            voice_engine,
            subtitle_engine,
        }
    }
}
