use std::{
    collections::HashMap,
    fmt,
    path::{Path, PathBuf},
    sync::LazyLock,
    time::Duration,
};

use anyhow::Context;
use clap::{Args, CommandFactory, Parser, Subcommand, ValueEnum, error::ErrorKind};
use indicatif::{ProgressBar, ProgressStyle};
use log::{debug, info};
use uuid::Uuid;
use validator::Validate;

mod auxiliary;
mod cli_command;
mod extractor;
mod fetcher;
mod file_type;
mod llm;
mod message;
mod scheme;
mod validate;

use crate::{
    auxiliary::{CompletionArgs, DownloadWhisperArgs},
    extractor::{
        Extractor, html::HtmlExtractor, pandoc::PandocExtractor, pdf::PdfExtractor,
        whisper::WhisperExtractor,
    },
    fetcher::{
        Fetcher,
        http::{HttpFetcher, HttpFetcherOptions},
    },
    llm::{LLMConstructionOptions, LLMRequestOptions},
    message::{ChatMessageOptions, SystemChatMessageVariables},
    scheme::InputScheme,
    validate::{validate_extractor_kind, validate_file_already_exists},
};

/// various contents summarization tool powered by artificial intelligence
#[derive(Parser, Validate)]
#[command(name = "nosy", version)]
pub struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    #[command(flatten)]
    summarize_args: SummarizeArgs,
}

#[derive(Clone, Debug, Args, Validate)]
/// Arguments shared by extract and summarize commands
pub struct ExtractSummarizeArgs {
    // NOTE: Keep these optional to avoid duplicating required args in subcommand usage.
    // We enforce required-ness via validator for summarize/extract.
    #[arg(help = "Input path or URL")]
    #[validate(required)]
    input: Option<String>,

    #[arg(short = 'o', long = "out", help = "Output file path")]
    #[validate(custom(function = "validate_file_already_exists"))]
    #[validate(required)]
    output: Option<PathBuf>,

    #[arg(
        short = 'w',
        long = "workdir",
        help = "Working directory for temporary files"
    )]
    workdir: Option<PathBuf>,

    #[arg(
        long = "log-level",
        default_value_t = LogLevel::Info,
        help = "Set log level"
    )]
    log_level: LogLevel,

    #[arg(long = "no-progress", help = "Disable progress bar")]
    no_progress: bool,
}

/// Thin wrapper around log levels for clap
#[derive(Clone, Copy, Debug, ValueEnum)]
enum LogLevel {
    /// No logging
    Off,
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl fmt::Display for LogLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            LogLevel::Off => "off",
            LogLevel::Error => "error",
            LogLevel::Warn => "warn",
            LogLevel::Info => "info",
            LogLevel::Debug => "debug",
            LogLevel::Trace => "trace",
        };
        f.write_str(text)
    }
}

#[derive(Subcommand)]
enum Command {
    /// Extract fetched content to text for LLM consumption (alias: ext)
    #[command(name = "extract", alias = "ext")]
    Extract(ExtractArgs),
    /// Summarize content using LLM (alias: recap)
    #[command(name = "summarize", alias = "recap")]
    Summarize(SummarizeArgs),
    /// Generate shell completion script for specified shell (alias: comp)
    #[command(name = "completion", alias = "comp")]
    Completion(CompletionArgs),
    /// Download Whisper model to a specified path
    #[command(name = "download-whisper")]
    DownloadWhisper(DownloadWhisperArgs),
}

#[derive(Clone, Debug, Args, Validate)]
struct FetchArgs {
    #[command(flatten)]
    http_opts: HttpFetcherOptions,
}

#[derive(Clone, Debug, Args, Validate)]
struct ExtractArgs {
    #[command(flatten)]
    extract_summarize_args: ExtractSummarizeArgs,

    #[command(flatten)]
    fetch_args: FetchArgs,

    #[arg(long = "ext-kind", help = "Force extractor kind for extraction")]
    #[validate(custom(function = "validate_extractor_kind"))]
    extractor_kind: Option<extractor::Kind>,
}

#[derive(Debug, Args, Validate)]
struct SummarizeArgs {
    #[command(flatten)]
    extract_args: ExtractArgs,

    #[command(flatten)]
    llm_construction_opts: LLMConstructionOptions,

    #[command(flatten)]
    llm_request_opts: LLMRequestOptions,

    #[command(flatten)]
    chat_message_opts: ChatMessageOptions,

    #[command(flatten)]
    system_chat_message_vars: SystemChatMessageVariables,
}

const FETCH_COLOR_HEX: &str = "#FFEADB";
const EXTRACT_COLOR_HEX: &str = "#F7C5A8";
const SUMMARIZE_COLOR_HEX: &str = "#FF9A76";

static FETCH_SPINNER_TEMPLATE: LazyLock<String> =
    LazyLock::new(|| format!("{{spinner:.{FETCH_COLOR_HEX}}} {{msg}} [{{elapsed}}]"));
static EXTRACT_SPINNER_TEMPLATE: LazyLock<String> =
    LazyLock::new(|| format!("{{spinner:.{EXTRACT_COLOR_HEX}}} {{msg}} [{{elapsed}}]"));
static SUMMARIZE_SPINNER_TEMPLATE: LazyLock<String> =
    LazyLock::new(|| format!("{{spinner:.{SUMMARIZE_COLOR_HEX}}} {{msg}} [{{elapsed}}]"));

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let Cli {
        command,
        summarize_args,
    } = Cli::parse();

    // Handle auxiliary commands early (e.g., completion)
    // so main stays focused on extract/summarize.
    if auxiliary::handle_auxiliary_command(command.as_ref()).await? {
        return Ok(());
    }

    let extract_summarize_args = match &command {
        Some(Command::Extract(args)) => &args.extract_summarize_args,
        Some(Command::Summarize(args)) => &args.extract_args.extract_summarize_args,
        None => &summarize_args.extract_args.extract_summarize_args,
        Some(Command::Completion(_)) | Some(Command::DownloadWhisper(_)) => {
            unreachable!("auxiliary commands handled earlier")
        }
    };

    // Enable RUST_LOG environment variable support
    // For developpers, we can set RUST_LOG=debug to see debug logs from dependencies.
    env_logger::Builder::from_env(
        env_logger::Env::default()
            .default_filter_or(format!("nosy={}", extract_summarize_args.log_level)),
    )
    .init();

    debug!("Extract/summarize arguments: {:?}", extract_summarize_args);

    // 0. Validate arguments
    if let Err(err) = extract_summarize_args.validate() {
        let missing_input = err.field_errors().contains_key("input");
        let missing_output = err.field_errors().contains_key("output");
        let kind = if missing_input || missing_output {
            ErrorKind::MissingRequiredArgument
        } else {
            ErrorKind::ValueValidation
        };
        Cli::command().error(kind, err.to_string()).exit();
    }
    match command {
        Some(Command::Extract(ref args)) => {
            if let Err(err) = args.validate() {
                Cli::command()
                    .error(ErrorKind::ValueValidation, err.to_string())
                    .exit();
            }
        }
        Some(Command::Summarize(ref args)) => {
            if let Err(err) = args.validate() {
                Cli::command()
                    .error(ErrorKind::ValueValidation, err.to_string())
                    .exit();
            }
        }
        _ => {
            if let Err(err) = summarize_args.validate() {
                Cli::command()
                    .error(ErrorKind::ValueValidation, err.to_string())
                    .exit();
            }
        }
    }

    let input = extract_summarize_args
        .input
        .as_deref()
        .expect("input is required");
    let output = extract_summarize_args
        .output
        .as_deref()
        .expect("output is required");
    let workdir = extract_summarize_args.workdir.clone().unwrap_or_else(|| {
        let tmp_dir = std::env::temp_dir()
            .join("nosy")
            .join(Uuid::new_v4().to_string());
        info!("Using system temporary directory as workdir: {tmp_dir:?}");
        tmp_dir
    });

    // 1. Detect scheme
    let scheme = scheme::detect(input);
    debug!("Detected scheme: {scheme:?}");

    let extract_args = match &command {
        Some(Command::Extract(args)) => args,
        Some(Command::Summarize(args)) => &args.extract_args,
        None => &summarize_args.extract_args,
        _ => unreachable!("auxiliary commands handled earlier"),
    };

    // 2. Fetch content
    let raw_content_path = fetch(
        input,
        &scheme,
        &workdir,
        &extract_args.fetch_args,
        extract_summarize_args.no_progress,
    )
    .await?;
    debug!("Raw content path: {raw_content_path:?}");

    // 3. Detect extractor kind
    let (extractor_kind, maybe_file_ext, maybe_mime) = match &extract_args.extractor_kind {
        Some(forced_extractor_kind) => {
            debug!("Using forced extractor kind: {forced_extractor_kind:?}");
            (*forced_extractor_kind, None, None)
        }
        None => {
            let maybe_file_ext = file_type::file_extension_lowercase(&raw_content_path);
            let maybe_mime = Some(file_type::mime_type(&raw_content_path)?);

            let kind = file_type::match_kind_by_extension(&maybe_file_ext);
            debug!("Detected extractor kind by extension '{maybe_file_ext:?}': {kind:?}");
            if kind != extractor::Kind::Unsupported {
                (kind, maybe_file_ext, maybe_mime)
            } else {
                let kind = file_type::match_kind_by_mime(&maybe_mime);
                debug!("Detected extractor kind by mime '{maybe_mime:?}': {kind:?}");
                (kind, maybe_file_ext, maybe_mime)
            }
        }
    };
    info!(
        "Use '{extractor_kind:?}' extractor for file extension '{maybe_file_ext:?}' and mime '{maybe_mime:?}'"
    );

    // 4. Extract content
    let extracted_content_path = extract(
        &raw_content_path,
        &extractor_kind,
        &maybe_file_ext,
        &maybe_mime,
        &workdir,
        extract_summarize_args.no_progress,
    )
    .await?;
    debug!("Extracted content path: {extracted_content_path:?}");

    // Consider: Instead of copying file from workdir to output path here,
    // should we directly write to output path in extract function?
    if let Some(Command::Extract(_)) = &command {
        create_parent_dirs(output).await?;
        tokio::fs::copy(&extracted_content_path, output)
            .await
            .with_context(|| {
                format!("failed to write extracted content to output path '{output:?}'")
            })?;
        debug!("Wrote extracted content to output path: {output:?}");
        return Ok(());
    }

    // Note: `nosy` and `nosy summarize` commands share the same summarize_args
    let summarize_args = match &command {
        Some(Command::Summarize(args)) => args,
        None => &summarize_args,
        _ => unreachable!("auxiliary and extract commands handled earlier"),
    };

    // 5. Summarize content
    let summary = summarize(
        &extracted_content_path,
        summarize_args,
        extract_summarize_args.no_progress,
    )
    .await?;
    let summary_chars = summary.chars().count();
    debug!("Received summary from LLM: chars={summary_chars}");

    // 6. Write output
    create_parent_dirs(output).await?;
    tokio::fs::write(output, summary)
        .await
        .with_context(|| format!("failed to write summary to output path '{output:?}'"))?;
    debug!("Wrote summary to output path: {output:?}");

    Ok(())
}

/// Create parent directories for the given path
async fn create_parent_dirs(path: &Path) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .with_context(|| format!("failed to create parent directories for '{path:?}'"))?;
    }
    Ok(())
}

/// Create and configure a spinner progress bar
/// NOTE: After calling spinner.set_message, be aware that logging will cause a newline.
fn create_spinner(template: &str, no_progress: bool) -> anyhow::Result<ProgressBar> {
    if no_progress {
        return Ok(ProgressBar::hidden());
    }
    let spinner = ProgressBar::new_spinner();
    spinner.enable_steady_tick(Duration::from_millis(60));
    spinner.set_style(
        ProgressStyle::with_template(template).unwrap_or_else(|_| ProgressStyle::default_spinner()),
    );
    Ok(spinner)
}

/// Fetch content from given URI with given arguments
async fn fetch(
    uri: &str,
    scheme: &InputScheme,
    workdir: &PathBuf,
    args: &FetchArgs,
    no_progress: bool,
) -> anyhow::Result<PathBuf> {
    // Return the given path immediately if file scheme because no fetching is needed.
    // Therefore, no workdir creation is needed.
    // Consider: Whether to create workdir or not can be determined by concrete Fetcher side?
    if scheme != &InputScheme::File {
        tokio::fs::create_dir_all(workdir)
            .await
            .with_context(|| format!("failed to create workdir at '{workdir:?}'"))?;
    }

    let fetcher: Box<dyn Fetcher> = match scheme {
        InputScheme::File => return Ok(uri.into()),
        InputScheme::Http => Box::new(HttpFetcher::new(&args.http_opts)),
        _ => {
            return Err(anyhow::anyhow!("unsupported input scheme"));
        }
    };

    let bar = create_spinner(FETCH_SPINNER_TEMPLATE.as_str(), no_progress)?;

    bar.set_message(format!("Fetching content from '{uri}'..."));
    match fetcher.fetch(uri, workdir, &bar).await {
        Ok(path) => {
            bar.finish_with_message("Fetching completed.");
            Ok(path)
        }
        Err(err) => {
            bar.finish_and_clear();
            Err(err)
        }
    }
}

/// Extract content to LLM-friendly input format
async fn extract(
    content_path: &PathBuf,
    extractor_kind: &extractor::Kind,
    maybe_file_ext: &Option<file_type::Extension>,
    maybe_mime: &Option<file_type::Mime>,
    workdir: &PathBuf,
    no_progress: bool,
) -> anyhow::Result<PathBuf> {
    // Return the given path immediately if plain text because no extraction is needed.
    // Therefore, no workdir creation is needed.
    // Consider: Whether to create workdir or not can be determined by concrete Extractor side?
    if *extractor_kind != extractor::Kind::PlainText {
        tokio::fs::create_dir_all(workdir)
            .await
            .with_context(|| format!("failed to create workdir at '{workdir:?}'"))?;
    }

    let extractor: Box<dyn Extractor> = match *extractor_kind {
        extractor::Kind::PlainText => return Ok(content_path.into()),
        extractor::Kind::HtmlNative => Box::new(HtmlExtractor),
        extractor::Kind::PdfNative => Box::new(PdfExtractor),
        extractor::Kind::Pandoc => Box::new(PandocExtractor),
        extractor::Kind::Whisper => Box::new(WhisperExtractor),
        _ => {
            return Err(anyhow::anyhow!(
                concat!(
                    "unsupported extractor kind for ext/mime: {:?}/{:?}. ",
                    "Extractor detection is heuristic and may be wrong. ",
                    "Try specifying one explicitly via --ext-kind."
                ),
                maybe_file_ext,
                maybe_mime,
            ));
        }
    };

    let bar = create_spinner(EXTRACT_SPINNER_TEMPLATE.as_str(), no_progress)?;

    bar.set_message("Extracting content...");
    match extractor
        .extract(content_path, maybe_file_ext, maybe_mime, workdir, &bar)
        .await
    {
        Ok(path) => {
            bar.finish_with_message("Extraction completed.");
            Ok(path)
        }
        Err(err) => {
            bar.finish_and_clear();
            Err(err)
        }
    }
}

/// Summarize extracted content using LLM
async fn summarize(
    content_path: &Path,
    summarize_args: &SummarizeArgs,
    no_progress: bool,
) -> anyhow::Result<String> {
    // Read extracted content
    // Consider: If we want to handle non-text formats (e.g., images)in the future,
    // we need to change this part.
    let content = tokio::fs::read_to_string(content_path)
        .await
        .with_context(|| format!("failed to read extracted content from '{content_path:?}'"))?;

    // Log LLM request info
    let model = &summarize_args.llm_request_opts.model;
    let adapter_kind = llm::infer_adapter_kind(model)?;
    let provider_label = summarize_args
        .llm_construction_opts
        .provider
        .as_ref()
        .map(llm::Provider::as_str)
        .unwrap_or_else(|| adapter_kind.as_lower_str());
    info!(
        "LLM request: model='{model}', provider='{}'",
        provider_label
    );

    let bar = &create_spinner(SUMMARIZE_SPINNER_TEMPLATE.as_str(), no_progress)?;

    bar.set_message("Generating chat messages to summarize...");
    let chat_messages = message::create_chat_messages(
        &summarize_args.chat_message_opts,
        &summarize_args.system_chat_message_vars,
        &HashMap::from([("content".to_string(), content)]),
    )?;

    bar.set_message("Summarizing content with LLM...");
    let llm_client = llm::create_llm_client(&summarize_args.llm_construction_opts)?;
    match llm::chat_request(&llm_client, &summarize_args.llm_request_opts, chat_messages).await {
        Ok(response) => {
            bar.finish_with_message("Summarization completed.");
            Ok(response)
        }
        Err(err) => {
            bar.finish_and_clear();
            Err(err)
        }
    }
}
