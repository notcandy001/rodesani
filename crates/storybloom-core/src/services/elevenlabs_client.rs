//! Minimal, strongly typed client for the ElevenLabs Text-to-Speech API.
//!
//! This module knows nothing about StoryBloom's domain types - it's a thin,
//! reusable wrapper around the wire format
//! (`POST /text-to-speech/{voice_id}`) plus error handling. Retry policy
//! and file output live in `crate::services::voice_engine`.

use std::time::Duration;

use serde::{Deserialize, Serialize};

use crate::error::CoreError;

/// Tunables for how expressive/stable the synthesized voice sounds. See
/// <https://elevenlabs.io/docs/api-reference/text-to-speech> for details.
#[derive(Debug, Clone, Serialize)]
pub struct VoiceSettings {
    pub stability: f32,
    pub similarity_boost: f32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub style: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub use_speaker_boost: Option<bool>,
}

impl Default for VoiceSettings {
    fn default() -> Self {
        Self {
            stability: 0.5,
            similarity_boost: 0.75,
            style: None,
            use_speaker_boost: Some(true),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TextToSpeechRequest {
    pub text: String,
    pub model_id: String,
    pub voice_settings: VoiceSettings,
}

/// Shape of an ElevenLabs API error response body, used to surface a
/// useful message instead of a raw HTTP status when a request fails.
/// ElevenLabs returns `{"detail": {"status": ..., "message": ...}}` for
/// most errors but occasionally a plain string, hence the untagged enum.
#[derive(Debug, Clone, Deserialize)]
struct ElevenLabsErrorResponse {
    detail: ElevenLabsErrorDetail,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum ElevenLabsErrorDetail {
    Structured {
        message: String,
        #[serde(default)]
        #[allow(dead_code)]
        status: Option<String>,
    },
    Message(String),
}

/// Thin async HTTP client for the ElevenLabs `/text-to-speech/{voice_id}`
/// endpoint.
pub struct ElevenLabsClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl ElevenLabsClient {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        timeout: Duration,
    ) -> Result<Self, CoreError> {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(CoreError::ElevenLabsTransport)?;

        Ok(Self {
            http,
            base_url: base_url.into(),
            api_key: api_key.into(),
        })
    }

    /// Synthesizes `request.text` with `voice_id` and returns the raw audio
    /// bytes (encoded per `output_format`, e.g. `mp3_44100_128`).
    pub async fn text_to_speech(
        &self,
        voice_id: &str,
        output_format: &str,
        request: &TextToSpeechRequest,
    ) -> Result<Vec<u8>, CoreError> {
        let url = format!(
            "{}/text-to-speech/{voice_id}?output_format={output_format}",
            self.base_url.trim_end_matches('/'),
        );

        let response = self
            .http
            .post(url)
            .header("xi-api-key", &self.api_key)
            .header("accept", "audio/mpeg")
            .json(request)
            .send()
            .await
            .map_err(CoreError::ElevenLabsTransport)?;

        let status = response.status();
        let bytes = response.bytes().await.map_err(CoreError::ElevenLabsTransport)?;

        if !status.is_success() {
            let message = serde_json::from_slice::<ElevenLabsErrorResponse>(&bytes)
                .map(|body| match body.detail {
                    ElevenLabsErrorDetail::Structured { message, .. } => message,
                    ElevenLabsErrorDetail::Message(message) => message,
                })
                .unwrap_or_else(|_| String::from_utf8_lossy(&bytes).into_owned());
            return Err(CoreError::ElevenLabsApi {
                status: status.as_u16(),
                message,
            });
        }

        Ok(bytes.to_vec())
    }
}
