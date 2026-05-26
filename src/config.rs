use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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

    pub respect_robots_txt: bool,
    pub crawl_delay: Option<Duration>,
    pub user_agent: String,
}

impl CrawlConfig {
    /// Reasonable defaults for local benchmarking / development.
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
            respect_robots_txt: false,
            crawl_delay: None,
            user_agent: "crawlyx-rs/0.1".to_string(),
        }
    }
}
