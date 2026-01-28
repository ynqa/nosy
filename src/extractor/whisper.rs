use std::path::{Path, PathBuf};

use anyhow::Context;
use indicatif::ProgressBar;
use rodio::{Decoder, source::UniformSourceIterator};
use whisper_rs::{FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters};

use crate::{
    extractor::{EXTRACTED_CONTENT_FILENAME, Extractor},
    file_type::{Extension, Mime},
    validate::validate_whisper_model_path_from_env,
};

/// Extractor implementation using Rust bindings for whisper.cpp.
///
/// Input format expected by whisper-rs:
/// - raw PCM samples (f32), 16kHz, mono
/// - https://codeberg.org/tazz4843/whisper-rs
///
/// Steps:
/// 1. Decode audio and normalize to f32/16kHz/mono with rodio
/// 2. Transcribe audio samples with whisper-rs
#[derive(Debug, Default)]
pub struct WhisperExtractor;

pub const WHISPER_REQUIRED_SAMPLE_RATE: u32 = 16_000;
pub const WHISPER_REQUIRED_CHANNELS: u16 = 1;

/// Decode audio file to f32 samples with rodio
fn decode_audio_samples(path: &Path) -> anyhow::Result<Vec<f32>> {
    let file = std::fs::File::open(path).context("failed to open audio file")?;
    let len = file.metadata()?.len();

    let decoder = Decoder::builder()
        .with_data(file)
        .with_byte_len(len)
        .with_seekable(true)
        .build()?;

    let uniform = UniformSourceIterator::new(
        decoder,
        WHISPER_REQUIRED_CHANNELS,
        WHISPER_REQUIRED_SAMPLE_RATE,
    );

    let samples: Vec<f32> = uniform.collect();
    if samples.is_empty() {
        return Err(anyhow::anyhow!("decoded audio is empty"));
    }
    Ok(samples)
}

/// Transcribe audio samples with whisper-rs
fn transcribe_audio(audio: &[f32], model_path: &Path) -> anyhow::Result<String> {
    let ctx = WhisperContext::new_with_params(
        model_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("model path is not valid UTF-8"))?,
        WhisperContextParameters::default(),
    )
    .context("failed to load whisper model")?;

    let mut state = ctx
        .create_state()
        .context("failed to create whisper state")?;

    let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });

    // Set number of threads to available parallelism
    let threads = std::thread::available_parallelism()
        .map(|count| count.get())
        .unwrap_or(1) as i32;
    params.set_n_threads(threads);

    state
        .full(params, audio)
        .context("failed to run whisper transcription")?;

    let num_segments = state.full_n_segments();
    let mut text = String::new();
    for idx in 0..num_segments {
        let Some(segment) = state.get_segment(idx) else {
            continue;
        };
        let segment_text = segment
            .to_str_lossy()
            .context("failed to read whisper segment")?;
        let trimmed = segment_text.trim();
        if !trimmed.is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(trimmed);
        }
    }

    Ok(text.trim().to_string())
}

#[async_trait::async_trait]
impl Extractor for WhisperExtractor {
    async fn extract(
        &self,
        content_path: &Path,
        _extension: &Option<Extension>,
        _mime: &Option<Mime>,
        workdir: &Path,
        bar: &ProgressBar,
    ) -> anyhow::Result<PathBuf> {
        // Disable whisper.cpp/ggml stdout/stderr logging to keep spinner clean.
        whisper_rs::install_logging_hooks();

        // Validate and get whisper model path from environment variable.
        let valid_model_path = validate_whisper_model_path_from_env()?;

        bar.set_message("Decoding audio with rodio...");
        let samples = decode_audio_samples(content_path)?;

        bar.set_message("Transcribing audio with whisper...");
        let text = transcribe_audio(&samples, &valid_model_path)?;
        let text = text.trim().to_string();

        // Write extracted text content to file
        if text.is_empty() {
            Err(anyhow::anyhow!("whisper produced empty output"))
        } else {
            let extracted_path = workdir.join(EXTRACTED_CONTENT_FILENAME);
            tokio::fs::write(&extracted_path, text)
                .await
                .context("failed to write extracted text content")?;
            Ok(extracted_path)
        }
    }
}
