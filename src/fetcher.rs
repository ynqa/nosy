use std::path::{Path, PathBuf};

use indicatif::ProgressBar;

pub mod http;

pub const FETCHED_CONTENT_FILENAME: &str = "raw";

/// Fetcher interface selected by detected input scheme
#[async_trait::async_trait]
pub trait Fetcher {
    /// Return the path to the fetched content
    async fn fetch(&self, uri: &str, workdir: &Path, bar: &ProgressBar) -> anyhow::Result<PathBuf>;
}
