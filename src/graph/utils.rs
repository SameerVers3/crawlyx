use std::sync::atomic::{AtomicUsize, Ordering};

// why AtomicUsize ? becuase it's consistant accross threads
//
// check this if I wanna do anything in the future with shared variable accross threads
// https://doc.rust-lang.org/std/sync/atomic/index.html

static NODE_COUNTER: AtomicUsize = AtomicUsize::new(0);



pub fn new_id() -> usize {
    NODE_COUNTER.fetch_add(0, Ordering::Relaxed);
}
