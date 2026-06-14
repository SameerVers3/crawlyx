use tokio::sync::mpsc::{self, Sender, Receiver};
use tokio::sync::Mutex;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use async_trait::async_trait;
use super::{Queue, WorkUnit};

pub struct InProcessQueue {
    tx: Sender<WorkUnit>,
    rx: Mutex<Receiver<WorkUnit>>,
    count: AtomicUsize,
}

impl InProcessQueue {
    pub fn new(capacity: usize) -> Arc<Self> {
        let (tx, rx) = mpsc::channel(capacity);
        Arc::new(Self {
            tx,
            rx: Mutex::new(rx),
            count: AtomicUsize::new(0),
        })
    }
}

#[async_trait]
impl Queue for InProcessQueue {
    async fn push(&self, work: WorkUnit) {
        self.count.fetch_add(1, Ordering::SeqCst);
        let _ = self.tx.send(work).await;
    }

    async fn pop(&self) -> WorkUnit {
        let mut rx = self.rx.lock().await;
        if let Some(work) = rx.recv().await {
            self.count.fetch_sub(1, Ordering::SeqCst);
            work
        } else {
            WorkUnit::shutdown()
        }
    }

    fn len(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }

    fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn work(url: &str) -> WorkUnit {
        WorkUnit {
            url: url.to_string(),
            current_depth: 0,
            target_depth: 3,
            parent_url: None,
            shutdown: false,
        }
    }

    #[tokio::test]
    async fn push_then_pop_returns_same_item() {
        let q = InProcessQueue::new(4);
        q.push(work("https://a.com")).await;
        assert_eq!(q.pop().await.url, "https://a.com");
    }

    #[tokio::test]
    async fn fifo_order_is_preserved() {
        let q = InProcessQueue::new(4);
        q.push(work("https://a.com")).await;
        q.push(work("https://b.com")).await;
        q.push(work("https://c.com")).await;
        assert_eq!(q.pop().await.url, "https://a.com");
        assert_eq!(q.pop().await.url, "https://b.com");
        assert_eq!(q.pop().await.url, "https://c.com");
    }

    #[tokio::test]
    async fn len_tracks_count() {
        let q = InProcessQueue::new(4);
        assert_eq!(q.len(), 0);
        q.push(work("https://a.com")).await;
        q.push(work("https://b.com")).await;
        assert_eq!(q.len(), 2);
        q.pop().await;
        assert_eq!(q.len(), 1);
    }

    #[tokio::test]
    async fn ring_wraps_around_correctly() {
        let q = InProcessQueue::new(3);
        q.push(work("https://a.com")).await;
        q.push(work("https://b.com")).await;
        q.push(work("https://c.com")).await;
        q.pop().await;
        q.pop().await;
        q.pop().await;
        q.push(work("https://d.com")).await;
        q.push(work("https://e.com")).await;
        assert_eq!(q.pop().await.url, "https://d.com");
        assert_eq!(q.pop().await.url, "https://e.com");
    }

    #[tokio::test]
    async fn pop_blocks_until_item_pushed() {
        let q = InProcessQueue::new(4);
        let q2 = Arc::clone(&q);

        let popper = tokio::spawn(async move { q2.pop().await.url });

        tokio::time::sleep(Duration::from_millis(50)).await;

        q.push(work("https://woke.com")).await;

        let url = popper.await.unwrap();
        assert_eq!(url, "https://woke.com");
    }

    #[tokio::test]
    async fn push_blocks_when_full_unblocks_on_pop() {
        let q = InProcessQueue::new(2);
        q.push(work("https://a.com")).await;
        q.push(work("https://b.com")).await; 

        let q2 = Arc::clone(&q);

        let pusher = tokio::spawn(async move {
            q2.push(work("https://c.com")).await; 
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        q.pop().await; 
        pusher.await.unwrap();

        assert_eq!(q.len(), 2);
    }

    #[tokio::test]
    async fn concurrent_producers_consumers_no_items_lost() {
        let q = InProcessQueue::new(64);
        let total = 1000usize;

        let producers: Vec<_> = (0..4).map(|i| {
            let q2 = Arc::clone(&q);
            tokio::spawn(async move {
                for j in 0..250 {
                    q2.push(work(&format!("https://producer-{}-item-{}.com", i, j))).await;
                }
            })
        }).collect();

        let results: Vec<_> = (0..4).map(|_| {
            let q2 = Arc::clone(&q);
            tokio::spawn(async move {
                let mut urls = Vec::new();
                for _ in 0..250 {
                    urls.push(q2.pop().await.url);
                }
                urls
            })
        }).collect();

        for p in producers { p.await.unwrap(); }

        let mut all_urls = Vec::new();
        for r in results {
            all_urls.extend(r.await.unwrap());
        }

        assert_eq!(all_urls.len(), total);
        assert_eq!(q.len(), 0);
    }
}
