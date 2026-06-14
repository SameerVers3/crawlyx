use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::config::{CrawlConfig, OutputFormat, OutputMode, OutputPathMode};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestConfigSnapshot {
    pub start_url: String,
    pub max_depth: usize,
    pub max_pages: Option<usize>,
    pub same_domain_only: bool,
    pub allow_subdomains: bool,
    pub output_format: OutputFormat,
    pub output_mode: OutputMode,
    pub output_path_mode: OutputPathMode,
    pub rewrite_links: bool,
    pub keep_extension: bool,
    pub respect_robots_txt: bool,
    pub crawl_delay_ms: Option<u64>,
    pub user_agent: String,
}

impl From<&CrawlConfig> for ManifestConfigSnapshot {
    fn from(c: &CrawlConfig) -> Self {
        Self {
            start_url: c.start_url.clone(),
            max_depth: c.max_depth,
            max_pages: c.max_pages,
            same_domain_only: c.same_domain_only,
            allow_subdomains: c.allow_subdomains,
            output_format: c.output_format,
            output_mode: c.output_mode,
            output_path_mode: c.output_path_mode,
            rewrite_links: c.rewrite_links,
            keep_extension: c.keep_extension,
            respect_robots_txt: c.respect_robots_txt,
            crawl_delay_ms: c.crawl_delay.map(|d| d.as_millis() as u64),
            user_agent: c.user_agent.clone(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PageStatus {
    Ok,
    WriteError,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PageEntry {
    pub url: String,
    pub depth: usize,
    pub crawled_at: DateTime<Utc>,
    pub status: PageStatus,
    pub output_path: Option<String>,
    pub bytes: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ManifestSummary {
    pub crawled_at: DateTime<Utc>,
    pub pages: usize,
    pub output_dir: String,
    pub pages_file: String,
    pub config: ManifestConfigSnapshot,
}

#[derive(Debug)]
pub struct ManifestWriter {
    output_dir: PathBuf,
    jsonl_path: PathBuf,
    writer: BufWriter<File>,
    pages: usize,
    config: ManifestConfigSnapshot,
    started_at: DateTime<Utc>,
}

impl ManifestWriter {
    pub fn new(output_dir: impl Into<PathBuf>, config: &CrawlConfig) -> std::io::Result<Self> {
        let output_dir = output_dir.into();
        fs::create_dir_all(&output_dir)?;

        let jsonl_path = output_dir.join("manifest.jsonl");
        let file = OpenOptions::new().create(true).write(true).truncate(true).open(&jsonl_path)?;

        Ok(Self {
            output_dir,
            jsonl_path,
            writer: BufWriter::new(file),
            pages: 0,
            config: ManifestConfigSnapshot::from(config),
            started_at: Utc::now(),
        })
    }

    pub fn append(&mut self, entry: &PageEntry) -> std::io::Result<()> {
        let line = serde_json::to_string(entry).expect("manifest serialization failed");
        self.writer.write_all(line.as_bytes())?;
        self.writer.write_all(b"\n")?;
        self.pages += 1;
        Ok(())
    }

    pub fn finish(mut self) -> std::io::Result<()> {
        self.writer.flush()?;

        let summary = ManifestSummary {
            crawled_at: self.started_at,
            pages: self.pages,
            output_dir: self.output_dir.to_string_lossy().to_string(),
            pages_file: self.jsonl_path.file_name().unwrap_or_else(|| Path::new("manifest.jsonl").as_os_str()).to_string_lossy().to_string(),
            config: self.config,
        };

        let manifest_json = self.output_dir.join("manifest.json");
        fs::write(manifest_json, serde_json::to_vec_pretty(&summary).unwrap())?;
        Ok(())
    }
}
