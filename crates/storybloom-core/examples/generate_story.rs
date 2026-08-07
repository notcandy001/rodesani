//! Standalone demonstration of the Story Engine - no Tauri, no UI.
//!
//! Run with:
//!
//!     OPENAI_API_KEY=sk-... cargo run -p storybloom-core --example generate_story
//!
//! Optionally override the model:
//!
//!     OPENAI_MODEL=gpt-4o cargo run -p storybloom-core --example generate_story

use std::time::Duration;

use storybloom_core::{StoryDuration, StoryEngine, StoryEngineConfig, StoryRequest, StoryType, Tone};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let api_key = std::env::var("OPENAI_API_KEY")
        .map_err(|_| anyhow::anyhow!("set OPENAI_API_KEY to run this example"))?;
    let model = std::env::var("OPENAI_MODEL").unwrap_or_else(|_| "gpt-4o-mini".to_string());

    let engine = StoryEngine::new(StoryEngineConfig {
        base_url: "https://api.openai.com/v1".to_string(),
        api_key,
        model,
        temperature: 0.9,
        max_output_tokens: 800,
        request_timeout: Duration::from_secs(30),
    })?;

    let request = StoryRequest::new(StoryType::Adventure, StoryDuration::Short, Tone::Lighthearted);

    let result = engine.generate(&request).await?;

    println!("Title:       {}", result.title);
    println!("Description: {}", result.description);
    println!(
        "Hashtags:    {}",
        result
            .hashtags
            .iter()
            .map(|tag| tag.to_string())
            .collect::<Vec<_>>()
            .join(" ")
    );
    println!("\n{}", result.story);

    Ok(())
}
