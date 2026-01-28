use std::ffi::OsStr;
use std::path::{Path, PathBuf};

use anyhow::Context;
use indicatif::ProgressBar;
use log::debug;

use crate::cli_command::CliCommand;
use crate::validate::validate_command_executable;
use crate::{
    extractor::{EXTRACTED_CONTENT_FILENAME, Extractor},
    file_type::{Extension, Mime},
};

pub const PANDOC_INSTALLATION_HINT: &str = "Please install pandoc by following https://pandoc.org/installing.html and ensure it is included in your PATH.";

/// Extractor implementation using pandoc CLI
/// Requires pandoc to be installed and available in PATH
///
/// References:
/// - What ext/mime types does pandoc support?
///   - https://pandoc.org/MANUAL.html#general-options
/// - Pandoc adapter implementation in ripgrep-all
///   - https://github.com/phiresky/ripgrep-all/blob/v0.10.10/src/adapters/custom.rs#L116-L134
#[derive(Debug, Default)]
pub struct PandocExtractor;

/// Map ExtractorKindDetection to pandoc --from argument
fn pandoc_input_format_with(mime: &Option<Mime>, extension: &Option<Extension>) -> Option<String> {
    let format = if let Some(ext) = extension.as_ref().map(|e| e.as_str()) {
        match ext {
            "docx" => "docx",
            "doc" => "doc",
            "odt" => "odt",
            "rtf" => "rtf",
            "epub" => "epub",
            "md" => "markdown",
            "html" | "htm" | "xhtml" => "html",
            "txt" | "text" => "plain",
            "tex" | "latex" => "latex",
            _ => return None,
        }
    } else {
        let mime = mime.as_ref().map(|m| m.as_str())?;
        match mime {
            "application/vnd.openxmlformats-officedocument.wordprocessingml.document" => "docx",
            "application/msword" => "doc",
            "application/vnd.oasis.opendocument.text" => "odt",
            "application/rtf" | "text/rtf" => "rtf",
            "application/epub+zip" => "epub",
            "text/markdown" => "markdown",
            "text/html" | "application/xhtml+xml" => "html",
            "text/plain" => "plain",
            "text/latex" | "application/x-tex" | "text/x-tex" => "latex",
            _ => return None,
        }
    };
    Some(format!("--from={format}"))
}

#[async_trait::async_trait]
impl Extractor for PandocExtractor {
    async fn extract(
        &self,
        content_path: &Path,
        extension: &Option<Extension>,
        mime: &Option<Mime>,
        workdir: &Path,
        bar: &ProgressBar,
    ) -> anyhow::Result<PathBuf> {
        // Generate `--from` argument if possible
        let maybe_from = pandoc_input_format_with(mime, extension);

        // Validate pandoc command availability
        validate_command_executable(OsStr::new("pandoc"))
            .map_err(|_| anyhow::anyhow!(PANDOC_INSTALLATION_HINT))?;

        let command = CliCommand::new("pandoc")
            .arg_opt(maybe_from.as_deref())
            .args(["--to", "plain", "--wrap=none", "--markdown-headings=atx"])
            .arg(content_path.as_os_str());
        debug!("Running external CLI: {command:?}");

        bar.set_message("Extracting content with pandoc...");
        let output = command
            .into_tokio_command()
            .output()
            .await
            .map_err(|err| {
                if err.kind() == std::io::ErrorKind::NotFound {
                    anyhow::anyhow!("pandoc is not installed or not in PATH")
                } else {
                    err.into()
                }
            })
            .context("failed to run pandoc")?;

        bar.set_message("Processing pandoc output...");
        // Check `pandoc` execution result
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("pandoc failed: {stderr}"));
        }

        // Normalize to UTF-8 text for downstream LLM input and empty-output checks.
        // Reject invalid UTF-8 early to avoid propagating corrupted bytes.
        // Consider: If we need to preserve raw bytes (e.g., for non-UTF-8 content),
        // handle output as bytes and define a separate validation/cleanup path.
        let text = String::from_utf8(output.stdout)
            .context("pandoc output is not valid UTF-8")?
            .trim()
            .to_string();

        if text.is_empty() {
            Err(anyhow::anyhow!("pandoc produced empty output"))
        } else {
            let extracted_path = workdir.join(EXTRACTED_CONTENT_FILENAME);
            tokio::fs::write(&extracted_path, text)
                .await
                .context("failed to write extracted text content")?;
            Ok(extracted_path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod pandoc_input_format_with {
        use super::*;

        #[test]
        fn test_maps_docx() {
            assert_eq!(
                pandoc_input_format_with(
                    &Some(Mime(
                        "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                            .to_string()
                    )),
                    &None,
                )
                .as_deref(),
                Some("--from=docx")
            );
        }

        #[test]
        fn test_maps_docx_by_extension() {
            assert_eq!(
                pandoc_input_format_with(&None, &Some(Extension("docx".to_string()))).as_deref(),
                Some("--from=docx")
            );
        }

        #[test]
        fn test_maps_unknown() {
            assert_eq!(
                pandoc_input_format_with(&Some(Mime("application/unknown".to_string())), &None),
                None
            );
        }
    }
}
