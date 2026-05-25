use std::sync::{Arc, Condvar, Mutex};
use super::{Queue, WorkUnit};

struct Inner {
    buf: Vec<Option<WorkUnit>>,
    head: usize,
    tail: usize,
    count: usize,
    capacity: usize,
}

impl Inner {
    fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "Queue capacity must be > 0");

        Self {
            buf: vec![None; capacity],
            head: 0,
            tail: 0,
            count: 0,
            capacity,
        }
    }

    fn is_full(&self) -> bool {
        self.count == self.capacity
    }

    fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn push(&mut self, work: WorkUnit) {
        self.buf[self.head] = Some(work);
        self.head = (self.head + 1) % self.capacity;
        self.count += 1;
    }

    fn pop(&mut self) -> WorkUnit {
        let work = self.buf[self.tail].take().unwrap();
        self.tail = (self.tail + 1) % self.capacity;
        self.count -= 1;
        work
    }


}

pub struct InProcessQueue {
    inner: Mutex<Inner>,
    not_full: Condvar,
    not_empty: Condvar,
}

impl InProcessQueue {
    pub fn new(capacity: usize) -> Arc<Self> {
        Arc::new(Self {
            inner: Mutex::new(Inner::new(capacity)),
            not_full: Condvar::new(),
            not_empty: Condvar::new(),
        })
    }
}

impl Queue for InProcessQueue {
    fn push(&self, work: WorkUnit) {
        let mut inner = self.inner.lock().unwrap();

        while inner.is_full() {
            inner = self.not_full.wait(inner).unwrap();
        }

        inner.push(work);

        self.not_empty.notify_one();
    }

    fn pop(&self) -> WorkUnit {
        let mut inner = self.inner.lock().unwrap();

        while inner.is_empty() {
            inner = self.not_empty.wait(inner).unwrap();
        }

        let work = inner.pop();

        self.not_full.notify_one();
        work 
    }

    fn len(&self) -> usize {
        self.inner.lock().unwrap().count
    }

    fn is_empty(&self) -> bool {
        self.inner.lock().unwrap().is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    fn work(url: &str) -> WorkUnit {
        WorkUnit {
            url: url.to_string(),
            current_depth: 0,
            target_depth: 3,
            parent_node: None,
            shutdown: false,
        }
    }

    #[test]
    fn push_then_pop_returns_same_item() {
        let q = InProcessQueue::new(4);
        q.push(work("https://a.com"));
        assert_eq!(q.pop().url, "https://a.com");
    }

    #[test]
    fn fifo_order_is_preserved() {
        let q = InProcessQueue::new(4);
        q.push(work("https://a.com"));
        q.push(work("https://b.com"));
        q.push(work("https://c.com"));
        assert_eq!(q.pop().url, "https://a.com");
        assert_eq!(q.pop().url, "https://b.com");
        assert_eq!(q.pop().url, "https://c.com");
    }

    #[test]
    fn len_tracks_count() {
        let q = InProcessQueue::new(4);
        assert_eq!(q.len(), 0);
        q.push(work("https://a.com"));
        q.push(work("https://b.com"));
        assert_eq!(q.len(), 2);
        q.pop();
        assert_eq!(q.len(), 1);
    }

    #[test]
    fn ring_wraps_around_correctly() {
        let q = InProcessQueue::new(3);
        q.push(work("https://a.com"));
        q.push(work("https://b.com"));
        q.push(work("https://c.com"));
        q.pop();
        q.pop();
        q.pop();
        q.push(work("https://d.com"));
        q.push(work("https://e.com"));
        assert_eq!(q.pop().url, "https://d.com");
        assert_eq!(q.pop().url, "https://e.com");
    }

    #[test]
    fn pop_blocks_until_item_pushed() {
        let q = InProcessQueue::new(4);
        let q2 = Arc::clone(&q);

        let popper = thread::spawn(move || q2.pop().url);

        thread::sleep(Duration::from_millis(50));

        q.push(work("https://woke.com"));

        let url = popper.join().unwrap();
        assert_eq!(url, "https://woke.com");
    }

    #[test]
    fn push_blocks_when_full_unblocks_on_pop() {
        let q = InProcessQueue::new(2);
        q.push(work("https://a.com"));
        q.push(work("https://b.com")); 

        let q2 = Arc::clone(&q);

        let pusher = thread::spawn(move || {
            q2.push(work("https://c.com")); 
        });

        thread::sleep(Duration::from_millis(50));

        q.pop(); 
        pusher.join().unwrap();

        assert_eq!(q.len(), 2);
    }

    #[test]
    fn concurrent_producers_consumers_no_items_lost() {
        let q = InProcessQueue::new(64);
        let total = 1000usize;

        // 4 producers push 250 items each
        let producers: Vec<_> = (0..4).map(|i| {
            let q2 = Arc::clone(&q);
            thread::spawn(move || {
                for j in 0..250 {
                    q2.push(work(&format!("https://producer-{}-item-{}.com", i, j)));
                }
            })
        }).collect();

        // 4 consumers pop 250 items each, collect URLs
        let results: Vec<_> = (0..4).map(|_| {
            let q2 = Arc::clone(&q);
            thread::spawn(move || {
                let mut urls = Vec::new();
                for _ in 0..250 {
                    urls.push(q2.pop().url);
                }
                urls
            })
        }).collect();

        for p in producers { p.join().unwrap(); }

        let all_urls: Vec<String> = results
            .into_iter()
            .flat_map(|r| r.join().unwrap())
            .collect();

        assert_eq!(all_urls.len(), total);
        assert_eq!(q.len(), 0);
    }
}
