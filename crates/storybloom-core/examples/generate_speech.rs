//! Standalone demonstration of the Voice Engine - no Tauri, no UI.
//!
//! Reads `elevenlabs.api_key` / `elevenlabs.voice_id` from the same layered
//! config the full app uses (`config/default.toml` + `config/local.toml`),
//! so once your key and voice ID are in `config/local.toml`, just run:
//!
//!     cargo run -p storybloom-core --example generate_speech
//!
//! Environment variables still override config if set, for one-off tests:
//!
//!     ELEVENLABS_API_KEY=... ELEVENLABS_VOICE_ID=... cargo run -p storybloom-core --example generate_speech

use std::path::PathBuf;
use std::time::Duration;

use storybloom_core::{VoiceEngine, VoiceEngineConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // This example runs from the workspace root (`cargo run -p ...`), so
    // `config/` sits one level up from nothing - it's already at the root.
    let settings = storybloom_config::Settings::load("config")
        .map_err(|err| anyhow::anyhow!("failed to load config/: {err}"))?;

    let api_key = std::env::var("ELEVENLABS_API_KEY")
        .ok()
        .filter(|key| !key.trim().is_empty())
        .or_else(|| Some(settings.elevenlabs.api_key.clone()).filter(|key| !key.trim().is_empty()))
        .ok_or_else(|| {
            anyhow::anyhow!(
                "no ElevenLabs API key found - set it in config/local.toml under \
                 [elevenlabs] api_key, or export ELEVENLABS_API_KEY"
            )
        })?;

    let voice_id = std::env::var("ELEVENLABS_VOICE_ID")
        .ok()
        .filter(|id| !id.trim().is_empty())
        .unwrap_or(settings.elevenlabs.voice_id.clone());

    let engine = VoiceEngine::new(VoiceEngineConfig {
        base_url: settings.elevenlabs.base_url.clone(),
        api_key,
        voice_id,
        model_id: settings.elevenlabs.model_id.clone(),
        output_format: settings.elevenlabs.output_format.clone(),
        request_timeout: Duration::from_secs(settings.elevenlabs.request_timeout_seconds),
        output_dir: PathBuf::from(&settings.elevenlabs.output_dir),
        max_retries: settings.elevenlabs.max_retries,
        retry_base_delay: Duration::from_millis(settings.elevenlabs.retry_base_delay_ms),
    })?;

    let text = "Once upon a time, in a kingdom powered by curiosity, a small fox learned that \
                the bravest thing it could do was ask why.";

    let path = engine.generate_speech(text, "demo-narration").await?;

    println!("Saved narration to {}", path.display());

    Ok(())
}
