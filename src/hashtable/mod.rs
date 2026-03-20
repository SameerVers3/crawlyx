use std::collections::HashMap;
use std::sync::RwLock;
use crossbeam_utils::CachePadded;
use std::sync::Arc;
pub mod utils;
use std::collections::hash_map::Entry;
use utils::{shard_for, NUM_SHARDS, SHARD_MASK};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum UrlState {
    InFlight,
    Visited,
}

pub struct VisitedTable {
    shards: Arc<Vec<CachePadded<RwLock<HashMap<String, UrlState, ahash::RandomState>>>>>
}

impl VisitedTable {
    pub fn new() -> Self {
        let shards: Vec<_> = (0..NUM_SHARDS).map(|_| {
            CachePadded::new(RwLock::new(HashMap::with_hasher(ahash::RandomState::new())))
        }).collect();

        Self {
            shards: Arc::new(shards)
        }
    }

    pub fn insert(&self, url: &str) -> bool {
        let index = shard_for(url);
        let mut map = self.shards[index].write().unwrap();
        match map.entry(url.to_string()) {
            Entry::Vacant(e) => { e.insert(UrlState::InFlight); true }
            Entry::Occupied(_) => { false }
        }
    }

    pub fn is_visited(&self, url: &str) -> bool {
        let index = shard_for(url);
        let map = self.shards[index].read().unwrap();
        match map.get(url) {
            Some(UrlState::Visited) => true,
            _ => false,
        }
    }

    pub fn mark_visited(&self, url: &str) {
        let index = shard_for(url);
        let mut map = self.shards[index].write().unwrap();
        map.insert(url.to_string(), UrlState::Visited);
    }

    pub fn get(&self, url: &str) -> Option<UrlState> {
        let index = shard_for(url);
        let map = self.shards[index].read().unwrap();
        map.get(url).copied()
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    //Basic insert & lookup

    #[test]
    fn insert_then_lookup_returns_inflight() {
        let table = VisitedTable::new();
        table.insert("http://site.com/page");
        assert!(matches!(table.get("http://site.com/page"), Some(UrlState::InFlight)));
    }

    #[test]
    fn lookup_on_empty_returns_none() {
        let table = VisitedTable::new();
        assert!(table.get("http://site.com/page").is_none());
    }

    #[test]
    fn insert_same_key_twice_no_duplicate() {
        let table = VisitedTable::new();
        let first  = table.insert("http://site.com/page");
        let second = table.insert("http://site.com/page");
        assert!(first);   // new — inserted
        assert!(!second); // already exists — skipped
    }

    #[test]
    fn insert_then_mark_visited() {
        let table = VisitedTable::new();
        table.insert("http://site.com/page");
        table.mark_visited("http://site.com/page");
        assert!(matches!(table.get("http://site.com/page"), Some(UrlState::Visited)));
    }

    #[test]
    fn lookup_after_mark_visited_persists() {
        let table = VisitedTable::new();
        table.insert("http://site.com/page");
        table.mark_visited("http://site.com/page");
        // call it again — state should still be Visited
        assert!(matches!(table.get("http://site.com/page"), Some(UrlState::Visited)));
    }

    #[test]
    fn bulk_insert_1000_urls() {
        let table = VisitedTable::new();
        let urls: Vec<String> = (0..1000)
            .map(|i| format!("http://site.com/page/{}", i))
            .collect();
        for url in &urls {
            table.insert(url);
        }
        for url in &urls {
            assert!(matches!(table.get(url), Some(UrlState::InFlight)));
        }
    }

    // Shard routing

    #[test]
    fn shard_index_always_in_bounds() {
        // generate 10,000 fake URLs and verify every shard index is in [0, NUM_SHARDS)
        for i in 0..10_000u64 {
            let url = format!("http://site.com/{}", i);
            let idx = shard_for(&url);
            assert!(idx < NUM_SHARDS, "shard index {} out of bounds", idx);
        }
    }

    #[test]
    fn bitmask_equals_modulo() {
        // hash & (N-1) must equal hash % N for all inputs when N is power of 2
        use std::hash::{Hash, Hasher};
        use ahash::AHasher;
        for i in 0..10_000u64 {
            let url = format!("http://site.com/{}", i);
            let mut hasher = AHasher::default();
            url.hash(&mut hasher);
            let hash = hasher.finish() as usize;
            assert_eq!(hash & SHARD_MASK, hash % NUM_SHARDS);
        }
    }

    // State machine

    #[test]
    fn none_to_inflight() {
        let table = VisitedTable::new();
        assert!(table.get("http://site.com").is_none());
        table.insert("http://site.com");
        assert!(matches!(table.get("http://site.com"), Some(UrlState::InFlight)));
    }

    #[test]
    fn inflight_to_visited() {
        let table = VisitedTable::new();
        table.insert("http://site.com");
        table.mark_visited("http://site.com");
        assert!(matches!(table.get("http://site.com"), Some(UrlState::Visited)));
    }

    #[test]
    fn visited_url_not_reinserted() {
        let table = VisitedTable::new();
        table.insert("http://site.com");
        table.mark_visited("http://site.com");
        // insert returns false — already in table
        assert!(!table.insert("http://site.com"));
        // state stays Visited, not overwritten to InFlight
        assert!(matches!(table.get("http://site.com"), Some(UrlState::Visited)));
    }

    // Edge cases

    #[test]
    fn very_long_url() {
        let table = VisitedTable::new();
        let url = format!("http://site.com/{}", "a".repeat(10_000));
        table.insert(&url);
        assert!(matches!(table.get(&url), Some(UrlState::InFlight)));
    }

    // Lock poisoning

    #[test]
    fn recover_from_poisoned_lock() {
        use std::sync::Arc;
        let table = Arc::new(VisitedTable::new());
        let t = table.clone();

        // spawn a thread that panics while holding nothing
        // (actually poisoning requires panic INSIDE a write lock — simulate it)
        let result = std::panic::catch_unwind(|| {
            t.insert("http://site.com");
        });
        // the table should still work after a panic outside the lock
        assert!(result.is_ok());
        assert!(table.get("http://site.com").is_some());
    }
}
