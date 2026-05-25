use std::sync::{Arc, mpsc};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use crate::graph::{Graph, Node};
use crate::hashtable::VisitedTable;
use crate::queue::{Queue, WorkUnit};
use crate::worker::{Worker, WorkerResult};

pub struct Scheduler {
    queue: Arc<dyn Queue>,
    hashtable: Arc<VisitedTable>,
    graph: Arc<Graph>,
    num_workers: usize,
    target_depth: usize,
}

impl Scheduler {
    pub fn new(
        queue: Arc<dyn Queue>,
        hashtable: Arc<VisitedTable>,
        graph: Arc<Graph>,
        num_workers: usize,
        target_depth: usize,
    ) -> Self {
        Self { queue, hashtable, graph, num_workers, target_depth }
    }

    pub fn run(&self, seed_url: String) {
        let (result_tx, result_rx) = mpsc::channel::<WorkerResult>();
        let active = Arc::new(AtomicUsize::new(0));

        // spawn workers
        let handles: Vec<_> = (0..self.num_workers).map(|id| {
            let q  = Arc::clone(&self.queue);
            let tx = result_tx.clone();
            thread::spawn(move || Worker::new(id, q, tx).run())
        }).collect();

        drop(result_tx);
        

        println!("seed enqueued: {}", seed_url);
        // enqueue the seed
        self.enqueue(seed_url, 0, None, &active);
        

        // dispatcher loop
        for result in result_rx {
            // mark URL as fully visited
            self.hashtable.mark_visited(&result.url);

            // store fetched HTML in the graph node
            if let Some(ref node) = result.node {
                println!("setting content for result node: {}", result.url);
                self.graph.set_content(node, result.content);
            }

            // get depth of the page we just processed
            let current_depth = result.node
                .as_ref()
                .map(|n| n.lock().unwrap().depth)
                .unwrap_or(0);

            // enqueue discovered URLs if we haven't hit depth limit

            if current_depth < self.target_depth {
                for url in result.discovered_urls {
                    
                    println!("enqueued: {}", url);
                    self.enqueue(url, current_depth + 1, result.node.clone(), &active);


                }
            }


            if active.fetch_sub(1, Ordering::SeqCst) == 1 {
                break;
            }
        }

        // send one shutdown pill per worker so they exit queue.pop()
        for _ in 0..self.num_workers {
            self.queue.push(WorkUnit::shutdown());
        }

        // wait for all workers to finish
        for handle in handles {
            handle.join().unwrap();
        }
    }

    fn enqueue(&self, url: String, depth: usize, parent: Option<Node>, active: &Arc<AtomicUsize>) {
        // insert returns false if url is already visited or in-flight so skip it
        if !self.hashtable.insert(&url) {
            return;
        }

        let node = self.graph.add_node(url.clone(), None, depth);

        if let Some(ref p) = parent {
            self.graph.add_edge(p, &node);
        }


        active.fetch_add(1, Ordering::SeqCst);

        self.queue.push(WorkUnit::new(url, depth, self.target_depth, Some(node)));
    }
}


