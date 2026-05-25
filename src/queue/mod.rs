pub mod inprocess;

use crate::graph::Node;

#[derive(Clone)]
pub struct WorkUnit {
    pub url: String,
    pub current_depth: usize,
    pub target_depth: usize,
    pub parent_node: Option<Node>,
}

pub trait Queue: Send + Sync {
    fn push(&self, work: WorkUnit);
    fn pop(&self) -> WorkUnit;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}

