pub mod inprocess;
// pub mod redis_queue;

use async_trait::async_trait;
use crate::work::WorkUnit;

#[async_trait]
pub trait Queue: Send + Sync {
    async fn push(&self, work: WorkUnit);
    async fn pop(&self) -> WorkUnit;
    fn len(&self) -> usize;
    fn is_empty(&self) -> bool;
}
