
use std::sync::Arc;
use crawlyx_rs::{
    graph::Graph,
    hashtable::VisitedTable,
    queue::inprocess::InProcessQueue,
    scheduler::Scheduler,
};

use std::time::Instant;


fn main() {
    //console_subscriber::init();
     
    let seed     = "http://localhost:8080/site/0/1".to_string();
    let workers  = 32;
    let depth    = 6;

    println!("Started");
    let start = Instant::now();

    let queue     = InProcessQueue::new(512);
    let hashtable = Arc::new(VisitedTable::new());
    let graph     = Arc::new(Graph::new(seed.clone()));

    let scheduler = Scheduler::new(queue, hashtable, graph, workers, depth);
    scheduler.run(seed);

    let duration = start.elapsed();
    println!("Time elapsed: {} ms", duration.as_millis());

    println!("done");
}

