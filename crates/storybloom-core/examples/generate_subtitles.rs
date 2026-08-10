//! Standalone demonstration of the Subtitle Engine - no Tauri, no UI.
//!
//! Reads `whisper.model_path` / `whisper.language` / `whisper.threads`
//! from the same layered config the full app uses (`config/default.toml` +
//! `config/local.toml`), so once your model path is set there, just run:
//!
//!     cargo run -p storybloom-core --example generate_subtitles -- output/audio/demo-narration.mp3
//!
//! Pass a different input path as the first CLI argument; it defaults to
//! `output/audio/demo-narration.mp3` (the file `generate_speech` produces).

use std::path::PathBuf;

use storybloom_core::{SubtitleEngine, SubtitleEngineConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let settings = storybloom_config::Settings::load("config")
        .map_err(|err| anyhow::anyhow!("failed to load config/: {err}"))?;

    let input_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("output/audio/demo-narration.mp3"));

    if !input_path.exists() {
        anyhow::bail!(
            "input audio file not found: {} - pass a path as the first argument, or run the \
             generate_speech example first to produce one",
            input_path.display()
        );
    }

    let engine = SubtitleEngine::new(SubtitleEngineConfig {
        model_path: PathBuf::from(&settings.whisper.model_path),
        language: settings.whisper.language.clone(),
        threads: settings.whisper.threads as i32,
    })
    .await?;

    println!("Transcribing {}...", input_path.display());

    let output_path = engine.transcribe(&input_path).await?;

    println!("Saved subtitles to {}", output_path.display());

    Ok(())
}
