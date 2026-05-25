
use std::sync::Arc;
use crawlyx_rs::{
    graph::Graph,
    hashtable::VisitedTable,
    queue::inprocess::InProcessQueue,
    scheduler::Scheduler,
};

fn main() {
    let seed     = "https://www.wikipedia.org/".to_string();
    let workers  = 8;
    let depth    = 3;

    let queue     = InProcessQueue::new(1024);
    let hashtable = Arc::new(VisitedTable::new());
    let graph     = Arc::new(Graph::new(seed.clone()));

    let scheduler = Scheduler::new(queue, hashtable, graph, workers, depth);
    scheduler.run(seed);

    println!("done");
}

