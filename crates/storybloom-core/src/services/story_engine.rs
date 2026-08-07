//! The Story Engine - turns a [`StoryRequest`] into a [`StoryResult`] by
//! calling an OpenAI-compatible chat completion endpoint with a strict JSON
//! schema, so the model's response deserializes directly into our domain
//! type with no manual text parsing.

use std::time::Duration;

use serde_json::json;

use crate::error::CoreError;
use crate::models::story::{StoryRequest, StoryResult};
use crate::services::openai_client::{
    ChatCompletionRequest, ChatMessage, ChatRole, JsonSchemaSpec, OpenAiClient, ResponseFormat,
    ResponseFormatType,
};

/// Everything the engine needs to talk to the model. Deliberately separate
/// from `storybloom_config::AiSettings` so this crate doesn't depend on
/// `storybloom-config` - callers (e.g. `src-tauri`) map one to the other.
#[derive(Debug, Clone)]
pub struct StoryEngineConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
    pub temperature: f32,
    pub max_output_tokens: u32,
    pub request_timeout: Duration,
}

/// Generates stories via an OpenAI-compatible chat completion API.
///
/// Expected to be constructed once at startup and shared (e.g. via `Arc`)
/// across every caller.
pub struct StoryEngine {
    client: OpenAiClient,
    config: StoryEngineConfig,
}

impl StoryEngine {
    /// Builds the engine, failing fast if no API key is configured rather
    /// than deferring that failure to the first `generate` call.
    pub fn new(config: StoryEngineConfig) -> Result<Self, CoreError> {
        if config.api_key.trim().is_empty() {
            return Err(CoreError::MissingApiKey);
        }

        let client = OpenAiClient::new(
            config.base_url.clone(),
            config.api_key.clone(),
            config.request_timeout,
        )?;

        Ok(Self { client, config })
    }

    /// Generates a story matching `request`.
    pub async fn generate(&self, request: &StoryRequest) -> Result<StoryResult, CoreError> {
        let chat_request = self.build_chat_request(request);

        tracing::info!(
            story_type = %request.story_type,
            tone = %request.tone,
            duration = %request.duration,
            model = %self.config.model,
            "generating story"
        );

        let response = self.client.chat_completion(&chat_request).await?;

        let content = response
            .choices
            .first()
            .map(|choice| choice.message.content.as_str())
            .ok_or_else(|| CoreError::ResponseParse("model returned no choices".to_string()))?;

        let result: StoryResult = serde_json::from_str(content).map_err(|err| {
            CoreError::ResponseParse(format!(
                "model output did not match the expected story schema: {err}"
            ))
        })?;

        Ok(result)
    }

    fn build_chat_request(&self, request: &StoryRequest) -> ChatCompletionRequest {
        let (min_words, max_words) = request.duration.target_word_count();

        let system_prompt = format!(
            "You are StoryBloom Studio's story generation engine. You write short-form \
             stories intended to be narrated over social media video. Respond ONLY with a \
             JSON object matching the provided schema - no markdown, no commentary outside \
             the JSON. The `story` field must be approximately {min_words}-{max_words} words, \
             matching a {duration} narration length. `hashtags` must each start with '#' and \
             contain no spaces.",
            duration = request.duration,
        );

        let user_prompt = format!(
            "Story type: {story_type}\n\
             Tone: {tone}\n\
             Target duration: {duration} ({min_words}-{max_words} words)\n\n\
             Write the story now.",
            story_type = request.story_type,
            tone = request.tone,
            duration = request.duration,
        );

        ChatCompletionRequest {
            model: self.config.model.clone(),
            messages: vec![
                ChatMessage {
                    role: ChatRole::System,
                    content: system_prompt,
                },
                ChatMessage {
                    role: ChatRole::User,
                    content: user_prompt,
                },
            ],
            temperature: self.config.temperature,
            max_tokens: self.config.max_output_tokens,
            response_format: ResponseFormat {
                format_type: ResponseFormatType::JsonSchema,
                json_schema: JsonSchemaSpec {
                    name: "story_output".to_string(),
                    strict: true,
                    schema: story_output_json_schema(),
                },
            },
        }
    }
}

/// JSON Schema describing [`StoryResult`], passed to OpenAI's structured
/// outputs feature so the model is constrained to produce exactly this
/// shape. Kept as a `serde_json::Value` (schema-describing-a-schema is
/// inherently structural metadata) rather than a Rust type - the Rust-side
/// contract is `StoryResult` itself, which the response is deserialized
/// into after the model returns.
fn story_output_json_schema() -> serde_json::Value {
    json!({
        "type": "object",
        "properties": {
            "title": {
                "type": "string",
                "description": "A short, catchy title for the story."
            },
            "story": {
                "type": "string",
                "description": "The full story text, written to be narrated aloud."
            },
            "description": {
                "type": "string",
                "description": "A one-to-two sentence social-media caption summarizing the story."
            },
            "hashtags": {
                "type": "array",
                "items": { "type": "string" },
                "description": "5-10 relevant hashtags, each starting with '#' and containing no spaces."
            }
        },
        "required": ["title", "story", "description", "hashtags"],
        "additionalProperties": false
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::story::{StoryDuration, StoryType, Tone};

    fn test_config() -> StoryEngineConfig {
        StoryEngineConfig {
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: "test-key".to_string(),
            model: "gpt-4o-mini".to_string(),
            temperature: 0.9,
            max_output_tokens: 800,
            request_timeout: Duration::from_secs(30),
        }
    }

    #[test]
    fn new_rejects_empty_api_key() {
        let mut config = test_config();
        config.api_key = String::new();
        let result = StoryEngine::new(config);
        assert!(matches!(result, Err(CoreError::MissingApiKey)));
    }

    #[test]
    fn new_accepts_a_valid_config() {
        let engine = StoryEngine::new(test_config());
        assert!(engine.is_ok());
    }

    #[test]
    fn prompt_reflects_the_request() {
        let engine = StoryEngine::new(test_config()).unwrap();
        let request = StoryRequest::new(StoryType::Horror, StoryDuration::Short, Tone::Dark);
        let chat_request = engine.build_chat_request(&request);

        assert_eq!(chat_request.messages.len(), 2);
        assert!(chat_request.messages[1].content.contains("Horror"));
        assert!(chat_request.messages[1].content.contains("Dark"));
    }
}
