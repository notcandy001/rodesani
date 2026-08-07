//! Minimal, strongly typed client for the OpenAI Chat Completions API.
//!
//! This module knows nothing about StoryBloom's domain types - it's a thin,
//! reusable wrapper around the wire format (`POST /chat/completions`) plus
//! error handling. Domain-specific prompt construction lives in
//! `crate::services::story_engine`.

use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::CoreError;

/// A single message in a chat completion request.
#[derive(Debug, Clone, Serialize)]
pub struct ChatMessage {
    pub role: ChatRole,
    pub content: String,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

/// Requests structured JSON output that strictly conforms to `schema`.
/// See <https://platform.openai.com/docs/guides/structured-outputs>.
#[derive(Debug, Clone, Serialize)]
pub struct ResponseFormat {
    #[serde(rename = "type")]
    pub format_type: ResponseFormatType,
    pub json_schema: JsonSchemaSpec,
}

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResponseFormatType {
    JsonSchema,
}

#[derive(Debug, Clone, Serialize)]
pub struct JsonSchemaSpec {
    pub name: String,
    pub strict: bool,
    pub schema: Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChatCompletionRequest {
    pub model: String,
    pub messages: Vec<ChatMessage>,
    pub temperature: f32,
    pub max_tokens: u32,
    pub response_format: ResponseFormat,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionResponse {
    pub id: String,
    pub choices: Vec<ChatCompletionChoice>,
    #[serde(default)]
    pub usage: Option<ChatCompletionUsage>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionChoice {
    pub index: u32,
    pub message: ChatCompletionResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ChatCompletionResponseMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ChatCompletionUsage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// Shape of an OpenAI API error response body, used to surface a useful
/// message instead of a raw HTTP status when a request fails.
#[derive(Debug, Clone, Deserialize)]
struct OpenAiErrorResponse {
    error: OpenAiErrorDetail,
}

#[derive(Debug, Clone, Deserialize)]
struct OpenAiErrorDetail {
    message: String,
}

/// Thin async HTTP client for the OpenAI-compatible `/chat/completions`
/// endpoint.
pub struct OpenAiClient {
    http: reqwest::Client,
    base_url: String,
    api_key: String,
}

impl OpenAiClient {
    pub fn new(
        base_url: impl Into<String>,
        api_key: impl Into<String>,
        timeout: Duration,
    ) -> Result<Self, CoreError> {
        let http = reqwest::Client::builder()
            .timeout(timeout)
            .build()
            .map_err(CoreError::OpenAiTransport)?;

        Ok(Self {
            http,
            base_url: base_url.into(),
            api_key: api_key.into(),
        })
    }

    pub async fn chat_completion(
        &self,
        request: &ChatCompletionRequest,
    ) -> Result<ChatCompletionResponse, CoreError> {
        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        let response = self
            .http
            .post(url)
            .bearer_auth(&self.api_key)
            .json(request)
            .send()
            .await
            .map_err(CoreError::OpenAiTransport)?;

        let status = response.status();
        let bytes = response.bytes().await.map_err(CoreError::OpenAiTransport)?;

        if !status.is_success() {
            let message = serde_json::from_slice::<OpenAiErrorResponse>(&bytes)
                .map(|body| body.error.message)
                .unwrap_or_else(|_| String::from_utf8_lossy(&bytes).into_owned());
            return Err(CoreError::OpenAiApi {
                status: status.as_u16(),
                message,
            });
        }

        serde_json::from_slice(&bytes)
            .map_err(|err| CoreError::ResponseParse(format!("malformed chat completion response: {err}")))
    }
}
