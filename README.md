<p align="center">
  <a href="https://ibb.co/FbJX1qZP"><img src="https://i.ibb.co/rGM3hf8W/snorlax-no-bg.png" alt="snorlax-no-bg" border="0" width="200" /></a>
</p>

<p align="center">
  <strong>Crawlyx: blazing fast web crawler</strong>
</p>

<p align="center">
  <em>Because LLMs deserve clean Markdown, not web junk.</em>
</p>

<p align="center">
  <a href="https://www.rust-lang.org/"><img src="https://img.shields.io/badge/rust-1.75+-93450a.svg?style=flat-square&logo=rust" alt="Rust"/></a>
</p>

<br/>

---

## Features

- **Concurrent Async Engine:** Driven by `tokio` and `reqwest` with tuned Keep-Alive connection pooling for high-throughput I/O.
- **Boilerplate-free Extraction:** Integrates Mozilla's Readability algorithm to instantly strip sidebars, navbars, and headers.
- **LLM-Ready Markdown:** Converts cleaned HTML to clean Markdown with metadata parsing (title, author, description, date).
- **Reactive Event Loop:** An event-driven dispatcher using `tokio::sync::Notify` that sleeps silently and wakes up instantly without busy-polling.
- **Off-Thread CPU Work:** Offloads CPU-intensive HTML parsing and Markdown rendering to a `spawn_blocking` pool to keep networking lanes unblocked.
- **Deadlock-Free Graph:** Tracks relationships in a thread-safe Graph structure using lock ordering to guarantee zero deadlocks.

---

## Demo

```bash
# Crawl 50 pages of the Rust Book and output clean Markdown trees in 3.5 seconds

cargo run --release -- crawl -u "https://doc.rust-lang.org/stable/book/" -l 50 -d 3
```

<details>
<summary>Example Output Snippet</summary>

```markdown
# Page: https://doc.rust-lang.org/stable/book/

This is the online version of "The Rust Programming Language" book...

---

# Page: https://doc.rust-lang.org/stable/book/ch01-00-getting-started.html

Getting started with Rust involves installing the toolchain and writing a hello world...

---
```
</details>

---

## Installation

### Prerequisites

> **Note:** Make sure you have [Rust](https://rustup.rs/) installed (1.75+)

### Build from Source

```bash
# Clone the repository
git clone https://github.com/SameerVers3/crawlyx.git
cd crawlyx

# Build optimized release binary
cargo build --release

# Run
./target/release/crawlyx --help
```

---

## Usage

### Crawl a Site and Format as Markdown (Default)

```bash
cargo run --release -- crawl -u "https://example.com" -d 2
```

### Crawl and Output as JSON

```bash
cargo run --release -- crawl -u "https://example.com" -d 2 --format json
```

### Advanced Crawl Constraints

```bash
# Cap crawling at 100 pages, limit requests to 30s timeout, and run 64 concurrent workers
cargo run --release -- crawl -u "https://example.com" -l 100 -w 64 --timeout 30
```

---

## Options

<details open>
<summary><b>Crawl Command Options</b></summary>

| Option | Description | Default |
|:-------|:------------|:--------|
| `-u, --url <URL>` | Seed URL to start crawling from | **Required** |
| `-w, --workers <COUNT>` | Number of concurrent worker threads | `32` |
| `-d, --depth <DEPTH>` | Max depth limit for recursively following links | `6` |
| `-l, --page-limit <LIMIT>` | Maximum number of pages to crawl | Unlimited |
| `--format <FORMAT>` | Output format: `json` \| `markdown` | `markdown` |
| `--timeout <SECONDS>` | Timeout for individual HTTP requests | `10` |
| `--crawl-timeout <SECONDS>` | Overall crawler timeout (graceful exit) | Unlimited |

</details>

---

## Project Structure

```
crawlyx/
├── Cargo.toml            # Project manifest
├── src/
│   ├── main.rs           # CLI Entrypoint & Wiring
│   ├── lib.rs            # Library interface
│   ├── state.rs          # Shared CrawlState struct
│   ├── work.rs           # WorkUnit definition
│   ├── dispatcher.rs     # Event-driven worker coordinator
│   ├── worker/
│   │   └── mod.rs        # HTML/Readability extraction worker
│   ├── fetcher/
│   │   └── mod.rs        # Async client with Keep-Alive connection pooling
│   ├── queue/
│   │   ├── mod.rs        # Queue trait definitions
│   │   └── inprocess.rs  # In-process tokio channel queue
│   ├── hashtable/
│   │   └── mod.rs        # Sharded AHash lock visited table
│   ├── graph/
│   │   └── mod.rs        # Thread-safe crawl graph implementation
│   ├── tree.rs           # Post-crawl tree derivation
│   ├── parser/
│   │   └── mod.rs        # HTML link extractor
│   ├── normalizer/
│   │   └── mod.rs        # URL normalizer rules
│   └── output/
│       └── mod.rs        # JSON and Markdown formatters
```

---



<p align="center">
  <sub>Star ⭐ this repo if you found it useful!</sub>
</p>
