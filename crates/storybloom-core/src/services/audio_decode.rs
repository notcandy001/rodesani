//! Decodes an audio file into the mono, 16kHz, `f32` PCM format
//! whisper.cpp expects, using `symphonia` (pure Rust, no ffmpeg/libav
//! dependency).
//!
//! This is intentionally minimal: whisper.cpp only needs speech-quality
//! audio, so a linear-interpolation resampler is sufficient and keeps the
//! dependency tree small. It is not intended for high-fidelity audio work.

use std::path::Path;

use symphonia::core::audio::SignalSpec;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;

use crate::error::CoreError;

/// The sample rate whisper.cpp models are trained on and require as input.
const WHISPER_SAMPLE_RATE: u32 = 16_000;

/// Decodes `path` (mp3, wav, or anything else `symphonia` supports) into
/// mono `f32` PCM samples at 16kHz.
pub fn decode_to_16k_mono_pcm(path: &Path) -> Result<Vec<f32>, CoreError> {
    let file = std::fs::File::open(path).map_err(CoreError::Io)?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = path.extension().and_then(|ext| ext.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .map_err(|err| CoreError::AudioDecode(format!("failed to probe audio format: {err}")))?;

    let mut format = probed.format;

    let track = format
        .tracks()
        .iter()
        .find(|track| track.codec_params.channels.is_some())
        .ok_or_else(|| CoreError::AudioDecode("no decodable audio track found".to_string()))?
        .clone();

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .map_err(|err| CoreError::AudioDecode(format!("failed to create audio decoder: {err}")))?;

    let track_id = track.id;
    let mut mono_samples: Vec<f32> = Vec::new();
    let mut source_rate = track.codec_params.sample_rate.unwrap_or(WHISPER_SAMPLE_RATE);

    loop {
        let packet = match format.next_packet() {
            Ok(packet) => packet,
            // End of stream and mid-stream format resets both surface as
            // errors in symphonia's API; both simply mean "stop reading"
            // here since we don't support gapless multi-track switching.
            Err(SymphoniaError::IoError(ref err)) if err.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(SymphoniaError::ResetRequired) => break,
            Err(err) => return Err(CoreError::AudioDecode(format!("failed to read packet: {err}"))),
        };

        if packet.track_id() != track_id {
            continue;
        }

        let decoded = match decoder.decode(&packet) {
            Ok(decoded) => decoded,
            // A single corrupt packet shouldn't abort the whole
            // transcription - skip it and keep going.
            Err(SymphoniaError::DecodeError(_)) => continue,
            Err(err) => return Err(CoreError::AudioDecode(format!("failed to decode packet: {err}"))),
        };

        let spec: SignalSpec = *decoded.spec();
        source_rate = spec.rate;
        let channels = spec.channels.count().max(1);

        let mut sample_buf =
            symphonia::core::audio::SampleBuffer::<f32>::new(decoded.capacity() as u64, spec);
        sample_buf.copy_interleaved_ref(decoded);

        for frame in sample_buf.samples().chunks(channels) {
            let sum: f32 = frame.iter().sum();
            mono_samples.push(sum / channels as f32);
        }
    }

    if mono_samples.is_empty() {
        return Err(CoreError::AudioDecode(
            "decoded audio contained no samples".to_string(),
        ));
    }

    Ok(resample_linear(&mono_samples, source_rate, WHISPER_SAMPLE_RATE))
}

/// Linear-interpolation resampler from `from_rate` to `to_rate`. A no-op if
/// the rates already match.
fn resample_linear(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    if from_rate == to_rate || samples.is_empty() {
        return samples.to_vec();
    }

    let ratio = from_rate as f64 / to_rate as f64;
    let out_len = ((samples.len() as f64) / ratio).round().max(0.0) as usize;
    let mut out = Vec::with_capacity(out_len);

    for i in 0..out_len {
        let src_pos = i as f64 * ratio;
        let idx = src_pos.floor() as usize;
        let frac = (src_pos - idx as f64) as f32;

        let a = samples.get(idx).copied().unwrap_or(0.0);
        let b = samples.get(idx + 1).copied().unwrap_or(a);
        out.push(a + (b - a) * frac);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resample_linear_is_noop_when_rates_match() {
        let samples = vec![0.1, 0.2, 0.3];
        assert_eq!(resample_linear(&samples, 16_000, 16_000), samples);
    }

    #[test]
    fn resample_linear_handles_empty_input() {
        assert_eq!(resample_linear(&[], 44_100, 16_000), Vec::<f32>::new());
    }

    #[test]
    fn resample_linear_downsamples_to_roughly_the_expected_length() {
        let samples = vec![0.0f32; 44_100];
        let resampled = resample_linear(&samples, 44_100, 16_000);
        // Allow a couple of samples of rounding slack rather than pinning
        // an exact length.
        assert!((resampled.len() as i64 - 16_000).abs() <= 2);
    }

    #[test]
    fn resample_linear_interpolates_between_samples() {
        // 2 samples at 2Hz resampled to 4Hz should insert a midpoint.
        let samples = vec![0.0f32, 1.0f32];
        let resampled = resample_linear(&samples, 2, 4);
        assert_eq!(resampled.len(), 4);
        assert!((resampled[0] - 0.0).abs() < 1e-6);
    }
}
