//! The Subtitle Engine - transcribes narration audio into a timestamped
//! SRT subtitle file using a local whisper.cpp model (via `whisper-rs`).
//!
//! Unlike `StoryEngine`/`VoiceEngine`, this makes no network calls: model
//! loading and inference both run entirely on-device against a GGML/GGUF
//! model file supplied by the caller (see `SubtitleEngineConfig::model_path`).

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::error::CoreError;
use crate::services::audio_decode::decode_to_16k_mono_pcm;

/// Everything the engine needs to load a whisper.cpp model and run
/// inference. Deliberately separate from `storybloom_config::WhisperSettings`
/// so this crate doesn't depend on `storybloom-config` - callers (e.g.
/// `src-tauri`) map one to the other.
#[derive(Debug, Clone)]
pub struct SubtitleEngineConfig {
    /// Path to a whisper.cpp GGML/GGUF model file (e.g. `ggml-base.en.bin`).
    /// Not distributed with this repo - download separately.
    pub model_path: PathBuf,
    /// Language code whisper.cpp expects (e.g. "en"), or "auto" to
    /// auto-detect from the audio.
    pub language: String,
    /// Threads whisper.cpp uses internally for inference.
    pub threads: i32,
}

/// One transcribed line, with start/end offsets from the start of the audio.
#[derive(Debug, Clone, PartialEq)]
pub struct SubtitleSegment {
    pub index: usize,
    pub start: Duration,
    pub end: Duration,
    pub text: String,
}

/// Transcribes audio files into timestamped SRT subtitles using a local
/// whisper.cpp model.
///
/// The model is loaded once at construction and reused for every
/// `transcribe` call - loading a GGML model is expensive (reading and
/// parsing anywhere from tens to hundreds of megabytes), so this is
/// expected to be constructed once at startup and shared (e.g. via `Arc`)
/// across every caller, same as `StoryEngine`/`VoiceEngine`.
pub struct SubtitleEngine {
    context: Arc<WhisperContext>,
    config: SubtitleEngineConfig,
}

impl SubtitleEngine {
    /// Loads the whisper.cpp model from `config.model_path`.
    ///
    /// Model loading is a blocking, CPU/IO-bound operation, so even though
    /// this function is `async`, the actual load runs on `spawn_blocking`
    /// rather than on the async runtime's worker threads, to avoid
    /// stalling other tasks while a large model file is read and parsed.
    pub async fn new(config: SubtitleEngineConfig) -> Result<Self, CoreError> {
        if !config.model_path.exists() {
            return Err(CoreError::WhisperInit(format!(
                "model file not found at {} - download a whisper.cpp GGML model \
                 (e.g. ggml-base.en.bin) and point whisper.model_path at it",
                config.model_path.display()
            )));
        }
        if config.threads <= 0 {
            return Err(CoreError::Validation(
                "whisper threads must be greater than zero".to_string(),
            ));
        }
        if config.language.trim().is_empty() {
            return Err(CoreError::Validation(
                "whisper language must not be empty (use \"auto\" to auto-detect)".to_string(),
            ));
        }

        let model_path = config.model_path.clone();
        let context = tokio::task::spawn_blocking(move || {
            WhisperContext::new_with_params(
                &model_path.to_string_lossy(),
                WhisperContextParameters::default(),
            )
        })
        .await
        .map_err(|err| CoreError::WhisperInit(format!("model loading task panicked: {err}")))?
        .map_err(|err| CoreError::WhisperInit(format!("failed to load whisper model: {err}")))?;

        Ok(Self {
            context: Arc::new(context),
            config,
        })
    }

    /// Transcribes `input_audio` and writes an SRT file next to it with the
    /// same stem (e.g. `story.mp3` -> `story.srt`), returning the path
    /// written to.
    ///
    /// Decoding and inference are both CPU-bound, so the whole pipeline
    /// runs on `spawn_blocking` rather than the async runtime's worker
    /// threads - this is what makes the engine safe to call from an async
    /// context (e.g. a future Tauri command) without stalling other tasks
    /// for the seconds-to-minutes inference can take.
    pub async fn transcribe(&self, input_audio: impl AsRef<Path>) -> Result<PathBuf, CoreError> {
        let input_path = input_audio.as_ref().to_path_buf();
        if !input_path.exists() {
            return Err(CoreError::Validation(format!(
                "input audio file not found: {}",
                input_path.display()
            )));
        }

        let output_path = input_path.with_extension("srt");
        let context = Arc::clone(&self.context);
        let language = self.config.language.clone();
        let threads = self.config.threads;
        let output_path_for_task = output_path.clone();
        let input_path_for_log = input_path.clone();

        tracing::info!(
            input = %input_path_for_log.display(),
            output = %output_path.display(),
            language = %language,
            "transcribing audio"
        );

        tokio::task::spawn_blocking(move || {
            let pcm = decode_to_16k_mono_pcm(&input_path)?;
            let segments = run_whisper_inference(&context, &pcm, &language, threads)?;
            write_srt(&output_path_for_task, &segments)?;
            Ok::<(), CoreError>(())
        })
        .await
        .map_err(|err| CoreError::WhisperTranscribe(format!("transcription task panicked: {err}")))??;

        tracing::info!(output = %output_path.display(), "saved subtitles");

        Ok(output_path)
    }
}

/// Runs whisper.cpp inference over `pcm` (mono, 16kHz, `[-1.0, 1.0]`
/// samples) and returns timestamped, non-empty segments.
fn run_whisper_inference(
    context: &WhisperContext,
    pcm: &[f32],
    language: &str,
    threads: i32,
) -> Result<Vec<SubtitleSegment>, CoreError> {
    let mut state = context
        .create_state()
        .map_err(|err| CoreError::WhisperTranscribe(format!("failed to create whisper state: {err}")))?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
    params.set_n_threads(threads);
    params.set_translate(false);
    params.set_print_special(false);
    params.set_print_progress(false);
    params.set_print_realtime(false);
    params.set_print_timestamps(false);
    if language != "auto" {
        params.set_language(Some(language));
    }

    state
        .full(params, pcm)
        .map_err(|err| CoreError::WhisperTranscribe(format!("inference failed: {err}")))?;

    let num_segments = state
        .full_n_segments()
        .map_err(|err| CoreError::WhisperTranscribe(format!("failed to read segment count: {err}")))?;

    let mut segments = Vec::with_capacity(num_segments.max(0) as usize);

    for i in 0..num_segments {
        let text = state
            .full_get_segment_text(i)
            .map_err(|err| CoreError::WhisperTranscribe(format!("failed to read segment text: {err}")))?
            .trim()
            .to_string();

        if text.is_empty() {
            continue;
        }

        // whisper.cpp reports timestamps in centiseconds (units of 10ms).
        let t0_centis = state
            .full_get_segment_t0(i)
            .map_err(|err| CoreError::WhisperTranscribe(format!("failed to read segment start: {err}")))?;
        let t1_centis = state
            .full_get_segment_t1(i)
            .map_err(|err| CoreError::WhisperTranscribe(format!("failed to read segment end: {err}")))?;

        segments.push(SubtitleSegment {
            index: segments.len() + 1,
            start: Duration::from_millis((t0_centis.max(0) as u64) * 10),
            end: Duration::from_millis((t1_centis.max(0) as u64) * 10),
            text,
        });
    }

    Ok(segments)
}

/// Writes `segments` as a standard SRT file.
fn write_srt(path: &Path, segments: &[SubtitleSegment]) -> Result<(), CoreError> {
    let mut out = String::new();

    for segment in segments {
        out.push_str(&segment.index.to_string());
        out.push('\n');
        out.push_str(&format_srt_timestamp(segment.start));
        out.push_str(" --> ");
        out.push_str(&format_srt_timestamp(segment.end));
        out.push('\n');
        out.push_str(&segment.text);
        out.push_str("\n\n");
    }

    std::fs::write(path, out).map_err(CoreError::Io)
}

/// Formats a `Duration` as an SRT timestamp: `HH:MM:SS,mmm`.
fn format_srt_timestamp(duration: Duration) -> String {
    let total_millis = duration.as_millis();
    let hours = total_millis / 3_600_000;
    let minutes = (total_millis % 3_600_000) / 60_000;
    let seconds = (total_millis % 60_000) / 1_000;
    let millis = total_millis % 1_000;

    format!("{hours:02}:{minutes:02}:{seconds:02},{millis:03}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_srt_timestamp_formats_correctly() {
        assert_eq!(format_srt_timestamp(Duration::from_millis(0)), "00:00:00,000");
        assert_eq!(format_srt_timestamp(Duration::from_millis(1_234)), "00:00:01,234");
        assert_eq!(
            format_srt_timestamp(Duration::from_millis(3_661_001)),
            "01:01:01,001"
        );
    }

    #[test]
    fn write_srt_produces_expected_format() {
        let dir = std::env::temp_dir().join(format!("storybloom-srt-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.srt");

        let segments = vec![
            SubtitleSegment {
                index: 1,
                start: Duration::from_millis(0),
                end: Duration::from_millis(1_500),
                text: "Hello world".to_string(),
            },
            SubtitleSegment {
                index: 2,
                start: Duration::from_millis(1_500),
                end: Duration::from_millis(3_000),
                text: "Second line".to_string(),
            },
        ];

        write_srt(&path, &segments).unwrap();
        let contents = std::fs::read_to_string(&path).unwrap();

        assert_eq!(
            contents,
            "1\n00:00:00,000 --> 00:00:01,500\nHello world\n\n\
             2\n00:00:01,500 --> 00:00:03,000\nSecond line\n\n"
        );

        std::fs::remove_file(&path).ok();
        std::fs::remove_dir(&dir).ok();
    }

    #[tokio::test]
    async fn new_rejects_missing_model_file() {
        let config = SubtitleEngineConfig {
            model_path: PathBuf::from("/nonexistent/ggml-base.en.bin"),
            language: "en".to_string(),
            threads: 4,
        };

        let result = SubtitleEngine::new(config).await;
        assert!(matches!(result, Err(CoreError::WhisperInit(_))));
    }
}
