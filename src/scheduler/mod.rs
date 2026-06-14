use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use crate::graph::Graph;
use crate::hashtable::VisitedStore;
use crate::queue::{Queue, WorkUnit};
use crate::worker::{WorkerResult, process_work};
use crate::config::{CrawlConfig, OutputMode};
use crate::output::{CloneWriter};
use crate::output::manifest::{ManifestWriter, PageEntry, PageStatus};
use chrono::Utc;

pub struct Scheduler {
    queue: Arc<dyn Queue>,
    hashtable: Arc<dyn VisitedStore>,
    graph: Arc<Graph>,
    num_workers: usize,
    target_depth: usize,
    config: Arc<CrawlConfig>,
}

impl Scheduler {
    pub fn new(
        queue: Arc<dyn Queue>,
        hashtable: Arc<dyn VisitedStore>,
        graph: Arc<Graph>,
        num_workers: usize,
        target_depth: usize,
        config: Arc<CrawlConfig>,
    ) -> Self {
        Self { queue, hashtable, graph, num_workers, target_depth, config }
    }

    pub async fn run(&self, seed_url: String) {
        let (result_tx, mut result_rx) = tokio::sync::mpsc::channel::<WorkerResult>(1024);
        let active = Arc::new(AtomicUsize::new(0));

        let clone_writer = if self.config.output_mode == OutputMode::Clone {
            CloneWriter::from_config(&self.config)
        } else {
            None
        };

        let mut manifest = if let Some(ref writer) = clone_writer {
            Some(
                ManifestWriter::new(writer.output_dir().to_path_buf(), &self.config)
                    .expect("failed to create manifest writer")
            )
        } else {
            None
        };

        let sem = Arc::new(tokio::sync::Semaphore::new(self.num_workers));

        self.enqueue(seed_url, 0, None, &active).await;
        if active.load(Ordering::SeqCst) == 0 {
            println!("Seed URL already visited or skipped. Nothing to crawl.");
            return;
        }

        let queue = self.queue.clone();
        let fetcher = Arc::new(crate::fetcher::Fetcher::new(&self.config.user_agent));
        let config = self.config.clone();
        let tx = result_tx.clone();
        let sem_clone = sem.clone();

        let queue_reader_handle = tokio::spawn(async move {
            loop {
                let work = queue.pop().await;
                if work.shutdown {
                    break;
                }
                
                let fetcher = fetcher.clone();
                let config = config.clone();
                let tx = tx.clone();
                
                let permit = sem_clone.clone().acquire_owned().await.unwrap();
                
                tokio::spawn(async move {
                    let _permit = permit;
                    let result = process_work(work, fetcher, config).await;
                    let _ = tx.send(result).await;
                });
            }
        });

        drop(result_tx);

        while let Some(result) = result_rx.recv().await {
            self.hashtable.mark_visited(&result.url).await;

            let depth = self.graph.get_node(&result.url)
                .map(|n| n.lock().unwrap().depth)
                .unwrap_or(0);

            if let (Some(writer), Some(manifest_writer)) = (clone_writer.as_ref(), manifest.as_mut()) {
                let page_bytes = result.page.markdown.len();
                match writer.write_page(&result.url, &result.page.markdown) {
                    Ok(rel_path) => {
                        let entry = PageEntry {
                            url: result.url.clone(),
                            depth,
                            crawled_at: Utc::now(),
                            status: PageStatus::Ok,
                            output_path: Some(rel_path.to_string_lossy().to_string()),
                            bytes: page_bytes,
                        };
                        let _ = manifest_writer.append(&entry);
                    }
                    Err(e) => {
                        eprintln!("[clone] failed to write {}: {}", result.url, e);
                        let entry = PageEntry {
                            url: result.url.clone(),
                            depth,
                            crawled_at: Utc::now(),
                            status: PageStatus::WriteError,
                            output_path: None,
                            bytes: page_bytes,
                        };
                        let _ = manifest_writer.append(&entry);
                    }
                }
            }

            if let Some(node) = self.graph.get_node(&result.url) {
                self.graph.set_content(&node, result.page.markdown.clone());
            }

            let current_depth = self.graph.get_node(&result.url)
                .map(|n| n.lock().unwrap().depth)
                .unwrap_or(0);

            if current_depth < self.target_depth {
                let mut max_pages_reached = false;
                if let Some(max_pages) = self.config.max_pages {
                    if self.graph.size() >= max_pages {
                        max_pages_reached = true;
                    }
                }

                if !max_pages_reached {
                    for url in result.discovered_urls {
                        if self.should_crawl(&url) {
                            self.enqueue(url, current_depth + 1, Some(result.url.clone()), &active).await;
                        }
                    }
                }
            }

            if active.fetch_sub(1, Ordering::SeqCst) == 1 {
                break;
            }
        }

        self.queue.push(WorkUnit::shutdown()).await;
        let _ = queue_reader_handle.await;

        if let Some(manifest_writer) = manifest {
            if let Err(e) = manifest_writer.finish() {
                eprintln!("[clone] failed to finalize manifest.json: {}", e);
            }
        }
    }

    async fn enqueue(&self, url: String, depth: usize, parent_url: Option<String>, active: &Arc<AtomicUsize>) {
        if !self.hashtable.insert(&url).await {
            return;
        }

        let node = self.graph.add_node(url.clone(), None, depth);

        if let Some(ref p_url) = parent_url {
            if let Some(p_node) = self.graph.get_node(p_url) {
                self.graph.add_edge(&p_node, &node);
            }
        }

        active.fetch_add(1, Ordering::SeqCst);
        self.queue.push(WorkUnit::new(url, depth, self.target_depth, parent_url)).await;
    }

    fn should_crawl(&self, url: &str) -> bool {
        let target_url = match url::Url::parse(url) {
            Ok(u) => u,
            Err(_) => return false,
        };

        if self.config.same_domain_only {
            let start_url = match url::Url::parse(&self.config.start_url) {
                Ok(u) => u,
                Err(_) => return false,
            };

            let target_host = match target_url.host_str() {
                Some(h) => h,
                None => return false,
            };

            let start_host = match start_url.host_str() {
                Some(h) => h,
                None => return false,
            };

            if self.config.allow_subdomains {
                if !target_host.ends_with(start_host) {
                    return false;
                }
            } else {
                if target_host != start_host {
                    return false;
                }
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hashtable::VisitedTable;
    use crate::queue::inprocess::InProcessQueue;

    #[tokio::test]
    async fn test_should_crawl_respects_domains() {
        let mut config = CrawlConfig::new("https://example.com/start");
        config.same_domain_only = true;
        config.allow_subdomains = false;
        let config = Arc::new(config);

        let queue = InProcessQueue::new(10);
        let hashtable = Arc::new(VisitedTable::new());
        let graph = Arc::new(Graph::new(config.start_url.clone()));
        
        let scheduler = Scheduler::new(queue, hashtable, graph, 2, 3, config.clone());

        assert!(scheduler.should_crawl("https://example.com/page1"));
        assert!(scheduler.should_crawl("https://example.com/sub/page2"));
        assert!(!scheduler.should_crawl("https://otherdomain.com/page"));
        assert!(!scheduler.should_crawl("https://sub.example.com/page"));
    }
}
