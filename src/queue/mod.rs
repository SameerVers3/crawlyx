pub mod inprocess;

use crate::graph::Node;

#[derive(Clone)]
pub struct WorkUnit {
    pub url: String,
    pub current_depth: usize,
    pub target_depth: usize,
    pub parent_node: Option<Node>,
    pub shutdown: bool,
}

impl WorkUnit {
    pub fn new(url: String, current_depth: usize, target_depth: usize, parent_node: Option<Node>) -> Self {
        Self {
            url,
            current_depth,
            target_depth,
            parent_node, 
            shutdown: false
        }
    }

    pub fn shutdown() -> Self {
        Self {
            url: String::new(),
            current_depth: 0,
            target_depth: 0,
            parent_node: None,
            shutdown: true,
        }
    }
}

pub trait Queue: Send + Sync {
    fn push(&self, work: WorkUnit);
    fn pop(&self) -> WorkUnit;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

