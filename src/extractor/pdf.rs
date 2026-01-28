use std::path::{Path, PathBuf};

use anyhow::Context;
use indicatif::ProgressBar;

use crate::{
    extractor::{EXTRACTED_CONTENT_FILENAME, Extractor},
    file_type::{Extension, Mime},
};

#[derive(Debug, Default)]
pub struct PdfExtractor;

#[async_trait::async_trait]
impl Extractor for PdfExtractor {
    async fn extract(
        &self,
        content_path: &Path,
        _extension: &Option<Extension>,
        _mime: &Option<Mime>,
        workdir: &Path,
        _: &ProgressBar,
    ) -> anyhow::Result<PathBuf> {
        // Extract text from PDF using pdf_extract crate
        let text = pdf_extract::extract_text(content_path)
            .context("failed to extract text from PDF content")?;

        // Write extracted text to output file
        let text = text.trim().to_string();
        if text.is_empty() {
            Err(anyhow::anyhow!("failed to extract text from PDF content"))
        } else {
            let extracted_path = workdir.join(EXTRACTED_CONTENT_FILENAME);
            tokio::fs::write(&extracted_path, text)
                .await
                .context("failed to write extracted text content")?;
            Ok(extracted_path)
        }
    }
}
