use std::path::{Path, PathBuf};

use anyhow::Context;
use indicatif::ProgressBar;
use readabilityrs::Readability;

use crate::{
    extractor::{EXTRACTED_CONTENT_FILENAME, Extractor},
    file_type::{Extension, Mime},
};

#[derive(Debug, Default)]
pub struct HtmlExtractor;

#[async_trait::async_trait]
impl Extractor for HtmlExtractor {
    async fn extract(
        &self,
        content_path: &Path,
        _extension: &Option<Extension>,
        _mime: &Option<Mime>,
        workdir: &Path,
        _: &ProgressBar,
    ) -> anyhow::Result<PathBuf> {
        // Read HTML content from file
        let html = tokio::fs::read(content_path)
            .await
            .context("failed to read HTML content")?;

        // Parse and extract main content using readability
        let html =
            std::str::from_utf8(&html).context("content is not valid UTF-8 for HTML conversion")?;
        let readability = Readability::new(html, None, None)
            .context("failed to initialize readability parser")?;

        // Get extracted text content, and write to output file
        if let Some(article) = readability.parse()
            && let Some(text) = article.text_content
        {
            let extracted_path = workdir.join(EXTRACTED_CONTENT_FILENAME);
            tokio::fs::write(&extracted_path, text)
                .await
                .context("failed to write extracted text content")?;
            return Ok(extracted_path);
        }
        Err(anyhow::anyhow!("failed to extract text from HTML content"))
    }
}
