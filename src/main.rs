
use std::sync::Arc;
use crawlyx_rs::{
    graph::Graph,
    hashtable::VisitedTable,
    queue::inprocess::InProcessQueue,
    scheduler::Scheduler,
};


fn main() {
    //console_subscriber::init();
    
    let seed     = "http://localhost:8080/site/0/1".to_string();
    let workers  = 16;
    let depth    = 5;

    println!("Started");

    let queue     = InProcessQueue::new(512);
    let hashtable = Arc::new(VisitedTable::new());
    let graph     = Arc::new(Graph::new(seed.clone()));

    let scheduler = Scheduler::new(queue, hashtable, graph, workers, depth);
    scheduler.run(seed);

    println!("done");
}

