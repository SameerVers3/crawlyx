pub mod inprocess;
pub mod redis_queue;

use serde::{Deserialize, Serialize};
use async_trait::async_trait;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct WorkUnit {
    pub url: String,
    pub current_depth: usize,
    pub target_depth: usize,
    pub parent_url: Option<String>,
    pub shutdown: bool,
}

impl WorkUnit {
    pub fn new(url: String, current_depth: usize, target_depth: usize, parent_url: Option<String>) -> Self {
        Self {
            url,
            current_depth,
            target_depth,
            parent_url, 
            shutdown: false
        }
    }

    pub fn shutdown() -> Self {
        Self {
            url: String::new(),
            current_depth: 0,
            target_depth: 0,
            parent_url: None,
            shutdown: true,
        }
    }
}

#[async_trait]
pub trait Queue: Send + Sync {
    async fn push(&self, work: WorkUnit);
    async fn pop(&self) -> WorkUnit;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}
