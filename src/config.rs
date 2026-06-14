use std::time::Duration;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputMode {
    /// keep results in memory
    Crawl,
    /// write to disk
    Clone,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputPathMode {
    Relative,
    Original,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OutputFormat {
    Html,
    Markdown,
}

#[derive(Debug, Clone)]
pub struct CrawlConfig {
    pub start_url: String,

    pub max_depth: usize,
    pub max_pages: Option<usize>,

    pub same_domain_only: bool,
    pub allow_subdomains: bool,
    pub allowed_domains: Vec<String>,
    pub blocked_paths: Vec<String>,

    pub output_format: OutputFormat,

    pub output_mode: OutputMode,
    pub output_dir: Option<PathBuf>,
    pub output_path_mode: OutputPathMode,
    /// If true (recommended for clone), rewrite internal links to point to local mirrored files.
    pub rewrite_links: bool,
    /// If true, keep original extensions in file paths (e.g. keep `.html`), instead of rewriting
    /// to `.md` when output is Markdown.
    pub keep_extension: bool,

    pub respect_robots_txt: bool,
    pub crawl_delay: Option<Duration>,
    pub user_agent: String,
}

impl CrawlConfig {
    pub fn new(start_url: impl Into<String>) -> Self {
        Self {
            start_url: start_url.into(),
            max_depth: 0,
            max_pages: None,
            same_domain_only: true,
            allow_subdomains: false,
            allowed_domains: vec![],
            blocked_paths: vec![],
            output_format: OutputFormat::Markdown,
            output_mode: OutputMode::Crawl,
            output_dir: None,
            output_path_mode: OutputPathMode::Relative,
            rewrite_links: true,
            keep_extension: false,
            respect_robots_txt: false,
            crawl_delay: None,
            user_agent: "crawlyx-rs/0.1".to_string(),
        }
    }
}
