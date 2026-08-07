//! Services - business logic and external integrations.
//!
//! Each service is a plain struct constructed with the resources it needs
//! and exposes `async fn` methods returning `Result<T, crate::CoreError>`.
//! Services never reach back up into the view or Tauri layers.

pub mod openai_client;
pub mod story_engine;

pub use story_engine::{StoryEngine, StoryEngineConfig};
