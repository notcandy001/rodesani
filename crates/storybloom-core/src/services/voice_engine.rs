//! The Voice Engine - turns narration text into a saved MP3 file by calling
//! the ElevenLabs Text-to-Speech API, retrying transient failures with
//! exponential backoff before giving up.

use std::path::PathBuf;
use std::time::Duration;

use crate::error::CoreError;
use crate::services::elevenlabs_client::{ElevenLabsClient, TextToSpeechRequest, VoiceSettings};

/// Everything the engine needs to talk to ElevenLabs and persist the
/// result. Deliberately separate from `storybloom_config::ElevenLabsSettings`
/// so this crate doesn't depend on `storybloom-config` - callers (e.g.
/// `src-tauri`) map one to the other.
#[derive(Debug, Clone)]
pub struct VoiceEngineConfig {
    pub base_url: String,
    pub api_key: String,
    pub voice_id: String,
    pub model_id: String,
    pub output_format: String,
    pub request_timeout: Duration,
    /// Directory generated MP3s are written to (created if missing).
    pub output_dir: PathBuf,
    /// Number of retries after the initial attempt, for transient failures
    /// only (network errors, timeouts, HTTP 429/5xx).
    pub max_retries: u32,
    /// Base delay for exponential backoff: `base * 2^attempt`, capped at 30s.
    pub retry_base_delay: Duration,
}

/// Generates narration audio via the ElevenLabs Text-to-Speech API and
/// saves it as an MP3 file, retrying transient failures automatically.
///
/// Expected to be constructed once at startup and shared (e.g. via `Arc`)
/// across every caller.
pub struct VoiceEngine {
    client: ElevenLabsClient,
    config: VoiceEngineConfig,
}

impl VoiceEngine {
    /// Builds the engine, failing fast if no API key or voice is configured
    /// rather than deferring that failure to the first `generate_speech`
    /// call.
    pub fn new(config: VoiceEngineConfig) -> Result<Self, CoreError> {
        if config.api_key.trim().is_empty() {
            return Err(CoreError::MissingElevenLabsApiKey);
        }
        if config.voice_id.trim().is_empty() {
            return Err(CoreError::Validation(
                "elevenlabs voice_id must not be empty".to_string(),
            ));
        }

        let client = ElevenLabsClient::new(
            config.base_url.clone(),
            config.api_key.clone(),
            config.request_timeout,
        )?;

        Ok(Self { client, config })
    }

    /// Synthesizes `text` and saves it as `<file_stem>.mp3` under the
    /// configured output directory, creating the directory if needed.
    /// Returns the path written to.
    pub async fn generate_speech(&self, text: &str, file_stem: &str) -> Result<PathBuf, CoreError> {
        if text.trim().is_empty() {
            return Err(CoreError::Validation(
                "text to synthesize must not be empty".to_string(),
            ));
        }

        let request = TextToSpeechRequest {
            text: text.to_string(),
            model_id: self.config.model_id.clone(),
            voice_settings: VoiceSettings::default(),
        };

        tracing::info!(
            voice_id = %self.config.voice_id,
            model = %self.config.model_id,
            chars = text.len(),
            "generating speech"
        );

        let audio = self.fetch_with_retry(&request).await?;
        let path = self.write_audio(file_stem, &audio).await?;

        tracing::info!(path = %path.display(), bytes = audio.len(), "saved narration audio");

        Ok(path)
    }

    /// Calls the ElevenLabs API, retrying on transient failures (network
    /// errors, timeouts, HTTP 429/5xx) with exponential backoff.
    /// Non-transient failures (bad request, invalid key, invalid voice,
    /// etc.) fail immediately - retrying them wastes time and quota since
    /// they fail the same way every time.
    async fn fetch_with_retry(&self, request: &TextToSpeechRequest) -> Result<Vec<u8>, CoreError> {
        let mut attempt = 0u32;

        loop {
            let result = self
                .client
                .text_to_speech(&self.config.voice_id, &self.config.output_format, request)
                .await;

            match result {
                Ok(audio) => return Ok(audio),
                Err(err) if attempt < self.config.max_retries && is_retryable(&err) => {
                    let delay = backoff_delay(self.config.retry_base_delay, attempt);
                    tracing::warn!(
                        attempt = attempt + 1,
                        max_retries = self.config.max_retries,
                        delay_ms = delay.as_millis() as u64,
                        error = %err,
                        "ElevenLabs request failed, retrying"
                    );
                    tokio::time::sleep(delay).await;
                    attempt += 1;
                }
                Err(err) => return Err(err),
            }
        }
    }

    /// Ensures the output directory exists and writes `audio` to
    /// `<output_dir>/<sanitized file_stem>.mp3`.
    async fn write_audio(&self, file_stem: &str, audio: &[u8]) -> Result<PathBuf, CoreError> {
        tokio::fs::create_dir_all(&self.config.output_dir)
            .await
            .map_err(CoreError::Io)?;

        let file_name = format!("{}.mp3", sanitize_file_stem(file_stem));
        let path = self.config.output_dir.join(file_name);

        tokio::fs::write(&path, audio).await.map_err(CoreError::Io)?;

        Ok(path)
    }
}

/// Whether a failure is worth retrying. Network-level errors and
/// server-side/rate-limit HTTP statuses are transient; anything else (bad
/// request, invalid API key, invalid voice, etc.) will fail the same way
/// every time.
fn is_retryable(err: &CoreError) -> bool {
    match err {
        CoreError::ElevenLabsTransport(_) => true,
        CoreError::ElevenLabsApi { status, .. } => *status == 429 || *status >= 500,
        _ => false,
    }
}

/// Exponential backoff with a fixed cap, so a misbehaving API can't stall
/// the caller for minutes: `base * 2^attempt`, capped at 30s.
fn backoff_delay(base: Duration, attempt: u32) -> Duration {
    let multiplier = 1u32.checked_shl(attempt).unwrap_or(u32::MAX);
    base.saturating_mul(multiplier).min(Duration::from_secs(30))
}

/// Strips path separators and other characters that don't belong in a file
/// name, so `file_stem` (which may be derived from user/story input) can't
/// escape the output directory or produce an invalid path.
fn sanitize_file_stem(file_stem: &str) -> String {
    let cleaned: String = file_stem
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
        .collect();

    if cleaned.trim_matches('_').is_empty() {
        "narration".to_string()
    } else {
        cleaned
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_config() -> VoiceEngineConfig {
        VoiceEngineConfig {
            base_url: "https://api.elevenlabs.io/v1".to_string(),
            api_key: "test-key".to_string(),
            voice_id: "21m00Tcm4TlvDq8ikWAM".to_string(),
            model_id: "eleven_multilingual_v2".to_string(),
            output_format: "mp3_44100_128".to_string(),
            request_timeout: Duration::from_secs(30),
            output_dir: PathBuf::from("output/audio"),
            max_retries: 3,
            retry_base_delay: Duration::from_millis(250),
        }
    }

    #[test]
    fn new_rejects_empty_api_key() {
        let mut config = test_config();
        config.api_key = String::new();
        assert!(matches!(
            VoiceEngine::new(config),
            Err(CoreError::MissingElevenLabsApiKey)
        ));
    }

    #[test]
    fn new_rejects_empty_voice_id() {
        let mut config = test_config();
        config.voice_id = String::new();
        assert!(matches!(VoiceEngine::new(config), Err(CoreError::Validation(_))));
    }

    #[test]
    fn new_accepts_a_valid_config() {
        assert!(VoiceEngine::new(test_config()).is_ok());
    }

    #[tokio::test]
    async fn generate_speech_rejects_empty_text() {
        let engine = VoiceEngine::new(test_config()).unwrap();
        let result = engine.generate_speech("   ", "demo").await;
        assert!(matches!(result, Err(CoreError::Validation(_))));
    }

    #[test]
    fn sanitize_file_stem_strips_unsafe_characters() {
        assert_eq!(sanitize_file_stem("my story/../evil"), "my_story___evil");
        assert_eq!(sanitize_file_stem("../../etc/passwd"), "______etc_passwd");
        assert_eq!(sanitize_file_stem(""), "narration");
        assert_eq!(sanitize_file_stem("___"), "narration");
    }

    #[test]
    fn backoff_delay_grows_exponentially_and_caps() {
        let base = Duration::from_millis(100);
        assert_eq!(backoff_delay(base, 0), Duration::from_millis(100));
        assert_eq!(backoff_delay(base, 1), Duration::from_millis(200));
        assert_eq!(backoff_delay(base, 2), Duration::from_millis(400));
        assert_eq!(backoff_delay(Duration::from_secs(10), 5), Duration::from_secs(30));
    }

    #[test]
    fn is_retryable_classifies_errors() {
        assert!(is_retryable(&CoreError::ElevenLabsApi {
            status: 429,
            message: String::new()
        }));
        assert!(is_retryable(&CoreError::ElevenLabsApi {
            status: 503,
            message: String::new()
        }));
        assert!(!is_retryable(&CoreError::ElevenLabsApi {
            status: 400,
            message: String::new()
        }));
        assert!(!is_retryable(&CoreError::ElevenLabsApi {
            status: 401,
            message: String::new()
        }));
        assert!(!is_retryable(&CoreError::MissingElevenLabsApiKey));
    }
}
