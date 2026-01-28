use std::{
    io::IsTerminal,
    path::{Path, PathBuf},
    time::Duration,
};

use anyhow::Context;
use clap::{Args, ValueEnum};
use indicatif::{ProgressBar, ProgressStyle};
use tokio::io::AsyncWriteExt;

const DOWNLOAD_BAR_COLOR_HEX: &str = "#FFB5E8";

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum WhisperModel {
    /// Tiny Whisper model - finetuned for English.
    TinyEn,
    /// Tiny Whisper model.
    Tiny,
    /// Base Whisper model - finetuned for English.
    BaseEn,
    /// Base Whisper model.
    Base,
    /// Small Whisper model - finetuned for English.
    SmallEn,
    /// Small Whisper model.
    Small,
    /// Medium Whisper model - finetuned for English.
    MediumEn,
    /// Medium Whisper model.
    Medium,
    /// Large Whisper model - old version.
    LargeV1,
    /// Large Whisper model - V2.
    LargeV2,
    /// Large Whisper model - V3.
    LargeV3,
}

impl WhisperModel {
    fn filename(self) -> &'static str {
        match self {
            Self::TinyEn => "ggml-tiny.en.bin",
            Self::Tiny => "ggml-tiny.bin",
            Self::BaseEn => "ggml-base.en.bin",
            Self::Base => "ggml-base.bin",
            Self::SmallEn => "ggml-small.en.bin",
            Self::Small => "ggml-small.bin",
            Self::MediumEn => "ggml-medium.en.bin",
            Self::Medium => "ggml-medium.bin",
            Self::LargeV1 => "ggml-large-v1.bin",
            Self::LargeV2 => "ggml-large-v2.bin",
            Self::LargeV3 => "ggml-large-v3.bin",
        }
    }

    fn url(self) -> String {
        format!(
            "https://huggingface.co/ggerganov/whisper.cpp/resolve/main/{}",
            self.filename()
        )
    }
}

#[derive(Clone, Debug, Args)]
pub struct DownloadWhisperArgs {
    #[arg(
        value_enum,
        value_name = "MODEL",
        help = "Whisper model to download (tiny, base, small, medium, large-v1/v2/v3)"
    )]
    model: WhisperModel,

    #[arg(short = 'o', long = "out", help = "Output file path")]
    output: PathBuf,

    #[arg(long = "overwrite", help = "Overwrite existing file")]
    overwrite: bool,
}

pub async fn handle(args: &DownloadWhisperArgs) -> anyhow::Result<()> {
    let url = args.model.url();
    let filename = args.model.filename();
    let output_path = resolve_output_path(&args.output, filename);

    // Check to overwrite or not
    if output_path.exists() && !args.overwrite {
        return Err(anyhow::anyhow!(
            "output file already exists: {output_path:?} (use --overwrite to replace)"
        ));
    }

    // Create parent directory if not exists
    if let Some(parent) = output_path.parent()
        && !parent.as_os_str().is_empty()
    {
        tokio::fs::create_dir_all(parent)
            .await
            .context("failed to create output directory")?;
    }

    let response = reqwest::Client::new()
        .get(&url)
        .send()
        .await
        .context("failed to start download")?
        .error_for_status()
        .context("download request failed")?;

    let total_size = response.content_length();
    let progress = create_progress_bar(total_size);
    progress.set_message(format!("Downloading {}", filename));

    let mut file = tokio::fs::File::create(&output_path)
        .await
        .context("failed to create output file")?;

    let mut downloaded: u64 = 0;
    let mut response = response;
    while let Some(chunk) = response
        .chunk()
        .await
        .context("failed to read download stream")?
    {
        file.write_all(&chunk)
            .await
            .context("failed to write output file")?;
        downloaded = downloaded.saturating_add(chunk.len() as u64);
        if total_size.is_some() {
            progress.set_position(downloaded);
        } else {
            progress.inc(chunk.len() as u64);
        }
    }

    file.flush().await.context("failed to flush output file")?;

    if let Some(expected) = total_size
        && downloaded != expected
    {
        progress.finish_and_clear();
        return Err(anyhow::anyhow!(
            "download size mismatch: expected {expected} bytes, got {downloaded} bytes"
        ));
    }

    progress.finish_with_message(format!("Saved to {}", output_path.display()));
    print_whisper_model_path_hint(&output_path);
    Ok(())
}

/// Resolve the output path based on user input
fn resolve_output_path(path: &Path, filename: &str) -> PathBuf {
    let is_bin = path.extension().and_then(|ext| ext.to_str()) == Some("bin");
    if path.exists() {
        if path.is_dir() {
            path.join(filename)
        } else {
            path.to_path_buf()
        }
    } else if is_bin {
        // treat as .bin file path
        path.to_path_buf()
    } else {
        // treat as directory path
        path.join(filename)
    }
}

/// Create a progress bar on downloading whisper models
fn create_progress_bar(total_size: Option<u64>) -> ProgressBar {
    if !std::io::stdout().is_terminal() {
        return ProgressBar::hidden();
    }

    let bar = if let Some(total) = total_size {
        let style = ProgressStyle::with_template(&format!(
            "{{spinner:.{}}} {{msg}} [{{elapsed_precise}}] {{bytes}}/{{total_bytes}} ({{eta}})",
            DOWNLOAD_BAR_COLOR_HEX
        ))
        .unwrap_or_else(|_| ProgressStyle::default_bar());
        ProgressBar::new(total).with_style(style)
    } else {
        let style = ProgressStyle::with_template(&format!(
            "{{spinner:.{}}} {{msg}} [{{elapsed_precise}}] {{bytes}} ({{eta}})",
            DOWNLOAD_BAR_COLOR_HEX
        ))
        .unwrap_or_else(|_| ProgressStyle::default_spinner());
        ProgressBar::new_spinner().with_style(style)
    };

    bar.enable_steady_tick(Duration::from_millis(60));
    bar
}

/// Print hint to set WHISPER_MODEL_PATH environment variable
fn print_whisper_model_path_hint(path: &Path) {
    let path_display = path.display();
    println!(
        concat!(
            "To use it with extract/summarize, we recommend running:\n",
            "  export WHISPER_MODEL_PATH=\"{}\""
        ),
        path_display
    );
}
