use std::sync::{Arc, mpsc};

use crate::config::{CrawlConfig, OutputFormat};
use crate::fetcher::Fetcher;
use crate::graph::Node;
use crate::parser;
use crate::queue::{Queue, WorkUnit};

pub struct WorkerResult {
    pub url: String,
    pub node: Option<Node>,
    pub content: String,
    pub discovered_urls: Vec<String>,
}

pub struct Worker {
    id: usize,
    queue: Arc<dyn Queue>,
    fetcher: Fetcher,
    result_tx: mpsc::Sender<WorkerResult>,
    config: Arc<CrawlConfig>,
}

impl Worker {
    pub fn new(
        id: usize,
        queue: Arc<dyn Queue>,
        result_tx: mpsc::Sender<WorkerResult>,
        config: Arc<CrawlConfig>,
    ) -> Self {
        Self { id, queue, fetcher: Fetcher::new(), result_tx, config }
    }

    pub fn run(self) {
        loop {
            let work = self.queue.pop();

            if work.shutdown {
                break;
            }

            let fetched_html = match self.fetcher.fetch(&work.url) {
                Ok(html) => html,
                Err(e) => {
                    eprintln!("[worker {}] fetch error for {}: {:?}", self.id, work.url, e);

                    let _ = self.result_tx.send(WorkerResult {
                        url: work.url,
                        node: work.parent_node,
                        content: String::new(),
                        discovered_urls: vec![],
                    });
                    continue;
                }
            };

            // Output processing hook.
            let content = match self.config.output_format {
                OutputFormat::Html => fetched_html,
                OutputFormat::Markdown => {
                    match htmd::convert(&fetched_html) {
                        Ok(markdown) => markdown,
                        Err(e) => {
                            eprintln!(
                                "[worker {}] htmd conversion error for {}: {:?}",
                                self.id, work.url, e
                            );
                            fetched_html
                        }
                    }
                }
            };

            let discovered_urls = parser::extract_urls(&content, &work.url);

            let _ = self.result_tx.send(WorkerResult {
                url: work.url,
                node: work.parent_node,
                content,
                discovered_urls,
            });
        }
    }
}
