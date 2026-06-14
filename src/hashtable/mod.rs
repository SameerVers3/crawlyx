use std::collections::HashMap;
use std::sync::RwLock;
use crossbeam_utils::CachePadded;
use std::sync::Arc;
pub mod utils;
use std::collections::hash_map::Entry;
use utils::{shard_for, NUM_SHARDS};
use async_trait::async_trait;
use crate::storage::RedisClient;

#[async_trait]
pub trait VisitedStore: Send + Sync {
    async fn insert(&self, url: &str) -> bool;
    async fn mark_visited(&self, url: &str);
    async fn is_visited(&self, url: &str) -> bool;
}

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

#[async_trait]
impl VisitedStore for VisitedTable {
    async fn insert(&self, url: &str) -> bool {
        self.insert(url)
    }

    async fn mark_visited(&self, url: &str) {
        self.mark_visited(url);
    }

    async fn is_visited(&self, url: &str) -> bool {
        self.is_visited(url)
    }
}

pub struct RedisVisitedTable {
    client: RedisClient,
    key: String,
}

impl RedisVisitedTable {
    pub fn new(client: RedisClient, key: String) -> Self {
        Self { client, key }
    }
}

#[async_trait]
impl VisitedStore for RedisVisitedTable {
    async fn insert(&self, url: &str) -> bool {
        let cmd = vec!["SADD".to_string(), self.key.clone(), url.to_string()];
        match self.client.run_command(&cmd).await {
            Ok(val) => {
                val.as_i64() == Some(1) || val.as_bool() == Some(true)
            }
            Err(e) => {
                eprintln!("RedisVisitedTable insert error: {}", e);
                false
            }
        }
    }

    async fn mark_visited(&self, _url: &str) {
        // No-op for Redis since `insert` already added the URL to the visited set
    }

    async fn is_visited(&self, url: &str) -> bool {
        let cmd = vec!["SISMEMBER".to_string(), self.key.clone(), url.to_string()];
        match self.client.run_command(&cmd).await {
            Ok(val) => {
                val.as_i64() == Some(1) || val.as_bool() == Some(true)
            }
            Err(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::HttpRedisClient;
    use tokio::net::TcpListener;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

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
        for i in 0..10_000u64 {
            let url = format!("http://site.com/{}", i);
            let idx = shard_for(&url);
            assert!(idx < NUM_SHARDS, "shard index {} out of bounds", idx);
        }
    }

    #[test]
    fn bitmask_equals_modulo() {
        use std::hash::{Hash, Hasher};
        use ahash::AHasher;
        use crate::hashtable::utils::SHARD_MASK;
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
        assert!(!table.insert("http://site.com"));
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

        let result = std::panic::catch_unwind(|| {
            t.insert("http://site.com");
        });
        assert!(result.is_ok());
        assert!(table.get("http://site.com").is_some());
    }

    #[tokio::test]
    async fn test_redis_visited_table_integration() {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();

        let server_handle = tokio::spawn(async move {
            // SADD insert (return 1 => true)
            {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buf = [0; 1024];
                let n = socket.read(&mut buf).await.unwrap();
                let req_str = std::str::from_utf8(&buf[..n]).unwrap();
                assert!(req_str.contains(r#"[["SADD","visited","https://url1.com"]]"#));
                let resp = "HTTP/1.1 200 OK\r\nContent-Length: 15\r\n\r\n[{\"result\":1}]\n";
                socket.write_all(resp.as_bytes()).await.unwrap();
            }
            // SISMEMBER check (return 1 => true)
            {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buf = [0; 1024];
                let n = socket.read(&mut buf).await.unwrap();
                let req_str = std::str::from_utf8(&buf[..n]).unwrap();
                assert!(req_str.contains(r#"[["SISMEMBER","visited","https://url1.com"]]"#));
                let resp = "HTTP/1.1 200 OK\r\nContent-Length: 15\r\n\r\n[{\"result\":1}]\n";
                socket.write_all(resp.as_bytes()).await.unwrap();
            }
        });

        let client = RedisClient::Http(Arc::new(HttpRedisClient::new(format!("http://{}", addr), "t".to_string())));
        let store = RedisVisitedTable::new(client, "visited".to_string());

        assert!(store.insert("https://url1.com").await);
        assert!(store.is_visited("https://url1.com").await);
        server_handle.await.unwrap();
    }
}
