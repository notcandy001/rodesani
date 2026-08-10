//! Domain-level error type.
//!
//! Services return `Result<T, CoreError>` so callers (view-models) can
//! match on specific failure modes (e.g. to show a validation message)
//! rather than always falling back to a generic error string. Anything
//! unexpected is wrapped via `#[from]` and treated as opaque.

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("not found: {0}")]
    NotFound(String),

    #[error("validation failed: {0}")]
    Validation(String),

    #[error("database error")]
    Database(#[from] sqlx::Error),

    #[error("no OpenAI API key configured - set `ai.api_key`, STORYBLOOM__AI__API_KEY, or OPENAI_API_KEY")]
    MissingApiKey,

    #[error("failed to reach the OpenAI API")]
    OpenAiTransport(#[from] reqwest::Error),

    #[error("OpenAI API returned an error (HTTP {status}): {message}")]
    OpenAiApi { status: u16, message: String },

    #[error("failed to parse model response: {0}")]
    ResponseParse(String),

    #[error("no ElevenLabs API key configured - set `elevenlabs.api_key`, STORYBLOOM__ELEVENLABS__API_KEY, or ELEVENLABS_API_KEY")]
    MissingElevenLabsApiKey,

    #[error("failed to reach the ElevenLabs API")]
    ElevenLabsTransport(reqwest::Error),

    #[error("ElevenLabs API returned an error (HTTP {status}): {message}")]
    ElevenLabsApi { status: u16, message: String },

    #[error("filesystem error")]
    Io(#[from] std::io::Error),

    #[error("failed to initialize whisper: {0}")]
    WhisperInit(String),

    #[error("whisper transcription failed: {0}")]
    WhisperTranscribe(String),

    #[error("failed to decode audio: {0}")]
    AudioDecode(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
