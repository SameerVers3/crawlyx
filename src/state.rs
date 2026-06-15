use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicBool};
use crate::hashtable::VisitedTable;
use crate::queue::inprocess::InProcessQueue;
use crate::graph::Graph;
use crate::fetcher::Fetcher;

pub type VisitedHashmap = VisitedTable;
pub type InMemoryQueue = InProcessQueue;

pub struct CrawlState {
    pub visited: Arc<VisitedHashmap>,
    pub queue: Arc<InMemoryQueue>,
    pub graph: Arc<Graph>,
    pub fetcher: Arc<Fetcher>,
    pub pages_crawled: AtomicUsize,
    pub in_flight: AtomicUsize,
    pub page_limit: usize,
    pub max_depth: usize,
    pub shutdown: AtomicBool,
    pub notify: Arc<tokio::sync::Notify>,
}
