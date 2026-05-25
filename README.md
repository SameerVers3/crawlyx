# Crawlyx

A fast, multi-threaded web crawler.

## Implementation Status

### Queue
- [x] In-process circular queue
- [x] Waiting list for when queue is full
- [ ] Redis queue option (multi-machine)

### Dispatcher / Scheduler
- [ ] Single dispatcher thread
- [ ] Check pool for available slot before scheduling
- [ ] Check hashtable before scheduling (deduplication)
- [ ] Mark URL as in-flight at dispatch time

### Thread Pool
- [ ] Worker pool
- [ ] Unit of work: (url, current_depth, target_depth, parent_node_reference)

### Visited Page Hashtable
- [x] Sharded RwLock
- [x] Power of 2 shards with bitmasking for bucket lookup
- [x] Fast non-cryptographic hash function (FxHash or AHash) [used AHash]

### URL Normalization
- [x] Lowercase scheme and host
- [x] Resolve relative URLs against base URL of the page
- [x] Remove default ports
- [x] Remove fragments
- [x] Sort query parameters
- [x] Trailing slash consistency

### HTTP Fetcher
- [x] Fetch page by URL
- [x] Handle redirects

### HTML Parser (cancelled => now just writing a wraper of scraper)
- [ ] Custom self-written parser (cancelled)
- [ ] Build DOM tree (cancelled)
- [x] Walk nodes with links (a, base, iframe, etc.)
- [x] Extract and return URLs

### Graph (Current Crawl State)
- [x] Graph data structure
- [x] Per-node locks
- [x] Lock ordering by node ID (ascending) to prevent deadlocks

### Tree (Post-Crawl)
- [ ] Derive tree from graph after full crawl
- [ ] Cycle detection during derivation
