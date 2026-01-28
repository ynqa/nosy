use std::{
    fmt,
    path::{Path, PathBuf},
};

use anyhow::{Context, anyhow};
use clap::{Args, ValueEnum};
use headless_chrome::{Browser, LaunchOptions};
use indicatif::ProgressBar;

use crate::fetcher::{FETCHED_CONTENT_FILENAME, Fetcher};

/// HTTP fetch modes
#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum HttpFetchMode {
    #[value(name = "headless")]
    Headless,
    #[value(name = "get")]
    Get,
}

impl fmt::Display for HttpFetchMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            HttpFetchMode::Headless => "headless",
            HttpFetchMode::Get => "get",
        };
        f.write_str(s)
    }
}

/// Options for HttpFetcher
#[derive(Clone, Debug, Args)]
pub struct HttpFetcherOptions {
    #[arg(
        long = "http-fetch-mode",
        value_enum,
        default_value_t = HttpFetchMode::Get,
        help = "HTTP fetch mode (only if input scheme is HTTP or HTTPS)"
    )]
    pub mode: HttpFetchMode,
}

/// Fetcher for HTTP resources
pub struct HttpFetcher<'a> {
    options: &'a HttpFetcherOptions,
}

impl<'a> HttpFetcher<'a> {
    pub fn new(options: &'a HttpFetcherOptions) -> Self {
        Self { options }
    }

    /// Fetch content using headless Chrome
    async fn fetch_headless(&self, uri: &str) -> anyhow::Result<String> {
        let uri = uri.to_owned();
        tokio::task::spawn_blocking(move || -> anyhow::Result<String> {
            let browser = Browser::new(LaunchOptions::default_builder().headless(true).build()?)
                .context("failed to launch headless chrome")?;
            let tab = browser.new_tab().context("failed to open new tab")?;
            tab.navigate_to(&uri)
                .with_context(|| format!("failed to navigate to '{uri}'"))?;
            tab.wait_until_navigated()
                .context("failed while waiting for page navigation")?;
            tab.get_content().context("failed to extract page HTML")
        })
        .await
        .context("headless chrome task panicked or was cancelled")?
    }

    /// Fetch content using HTTP GET
    async fn fetch_reqwest(&self, uri: &str) -> anyhow::Result<String> {
        let client = reqwest::Client::builder()
            .build()
            .context("failed to build reqwest client")?;

        let res = client
            .get(uri)
            .send()
            .await
            .with_context(|| format!("failed to send GET '{uri}'"))?;

        let status = res.status();
        if !status.is_success() {
            return Err(anyhow!("GET '{uri}' failed with status {status}"));
        }

        res.text()
            .await
            .with_context(|| format!("failed to read response body from '{uri}'"))
    }
}

#[async_trait::async_trait]
impl<'a> Fetcher for HttpFetcher<'a> {
    async fn fetch(&self, uri: &str, workdir: &Path, bar: &ProgressBar) -> anyhow::Result<PathBuf> {
        bar.set_message(format!("Fetching HTTP content from {uri}"));
        let content = match self.options.mode {
            HttpFetchMode::Headless => self.fetch_headless(uri).await,
            HttpFetchMode::Get => self.fetch_reqwest(uri).await,
        }
        .with_context(|| format!("failed to fetch content from '{uri}'"))?;

        bar.set_message("Writing fetched content to disk");
        let temp_path = workdir.join(FETCHED_CONTENT_FILENAME);
        tokio::fs::write(&temp_path, content)
            .await
            .with_context(|| format!("failed to write fetched content to '{temp_path:?}'"))?;
        Ok(temp_path)
    }
}
