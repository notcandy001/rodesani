//! `storybloom-core` - the Model layer.
//!
//! Domain models, domain errors, and services (business logic that
//! operates over external resources such as a `storybloom_db::DbPool` or
//! the OpenAI API). This crate has no knowledge of Tauri, windows, or
//! view-models - it compiles and is testable as a plain Rust library.

pub mod error;
pub mod models;
pub mod services;

pub use error::CoreError;
pub use models::{Hashtag, HashtagError, StoryDuration, StoryRequest, StoryResult, StoryType, Tone};
pub use services::{
    StoryEngine, StoryEngineConfig, SubtitleEngine, SubtitleEngineConfig, SubtitleSegment,
    VoiceEngine, VoiceEngineConfig,
};
