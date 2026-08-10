//! Services - business logic and external integrations.
//!
//! Each service is a plain struct constructed with the resources it needs
//! and exposes `async fn` methods returning `Result<T, crate::CoreError>`.
//! Services never reach back up into the view or Tauri layers.

pub mod audio_decode;
pub mod elevenlabs_client;
pub mod openai_client;
pub mod story_engine;
pub mod subtitle_engine;
pub mod voice_engine;

pub use story_engine::{StoryEngine, StoryEngineConfig};
pub use subtitle_engine::{SubtitleEngine, SubtitleEngineConfig, SubtitleSegment};
pub use voice_engine::{VoiceEngine, VoiceEngineConfig};
