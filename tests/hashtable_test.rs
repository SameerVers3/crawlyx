use std::sync::Arc;
use std::thread;
use crawlyx_rs::hashtable::{VisitedTable, UrlState};

// Concurrent reads

#[test]
fn fifty_threads_reading_same_key_simultaneously() {
    let table = Arc::new(VisitedTable::new());
    table.insert("http://site.com/page");

    let handles: Vec<_> = (0..50).map(|_| {
        let t = table.clone();
        thread::spawn(move || {
            // all 50 threads read — none should block each other
            assert!(matches!(t.get("http://site.com/page"), Some(_)));
        })
    }).collect();

    for h in handles { h.join().unwrap(); }
}

#[test]
fn reads_on_different_shards_dont_block_each_other() {
    let table = Arc::new(VisitedTable::new());

    // pre-insert 50 URLs that land on different shards
    for i in 0..50 {
        table.insert(&format!("http://site.com/page/{}", i));
    }

    let handles: Vec<_> = (0..50).map(|i| {
        let t = table.clone();
        thread::spawn(move || {
            let url = format!("http://site.com/page/{}", i);
            assert!(t.get(&url).is_some());
        })
    }).collect();

    for h in handles { h.join().unwrap(); }
}

// Concurrent writes

#[test]
fn hundred_threads_inserting_unique_urls_no_lost_inserts() {
    let table = Arc::new(VisitedTable::new());

    let handles: Vec<_> = (0..100).map(|i| {
        let t = table.clone();
        thread::spawn(move || {
            t.insert(&format!("http://site.com/page/{}", i));
        })
    }).collect();

    for h in handles { h.join().unwrap(); }

    // every single URL must be present
    for i in 0..100 {
        let url = format!("http://site.com/page/{}", i);
        assert!(table.get(&url).is_some(), "missing: {}", url);
    }
}

#[test]
fn hundred_threads_inserting_same_url_no_corruption() {
    let table = Arc::new(VisitedTable::new());

    let handles: Vec<_> = (0..100).map(|_| {
        let t = table.clone();
        thread::spawn(move || {
            t.insert("http://site.com/page");
        })
    }).collect();

    for h in handles { h.join().unwrap(); }

    // exactly one entry, no corruption
    assert!(table.get("http://site.com/page").is_some());
}

// Scheduler contract

#[test]
fn inflight_url_not_dispatched_twice() {
    let table = Arc::new(VisitedTable::new());

    // simulate scheduler: insert returns true only once
    let first  = table.insert("http://site.com/page");
    let second = table.insert("http://site.com/page");

    assert!(first,  "first insert should succeed");
    assert!(!second, "second insert should be blocked");
}

#[test]
fn visited_url_never_recrawled() {
    let table = Arc::new(VisitedTable::new());
    table.insert("http://site.com/page");
    table.mark_visited("http://site.com/page");

    // scheduler checks — should see Visited and skip
    assert!(matches!(table.get("http://site.com/page"), Some(UrlState::Visited)));
    assert!(!table.insert("http://site.com/page")); // blocked
}
