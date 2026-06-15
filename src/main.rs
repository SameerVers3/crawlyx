use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, AtomicBool, Ordering};
use std::time::{Duration, Instant};

use clap::{Parser, ValueEnum};
use url::Url;

use crawlyx_rs::{
    state::CrawlState,
    work::WorkUnit,
    hashtable::VisitedTable,
    queue::inprocess::InProcessQueue,
    queue::Queue,
    graph::Graph,
    fetcher::Fetcher,
};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormatArg {
    Json,
    Markdown,
}

#[derive(Debug, Parser)]
struct CrawlArgs {
    /// Seed URL to start crawling from
    #[arg(long, short = 'u')]
    url: String,

    /// Number of worker threads
    #[arg(long, short = 'w', default_value_t = 32)]
    workers: usize,

    /// depth limit
    #[arg(long, short = 'd', default_value_t = 6)]
    depth: usize,

    /// Stop after visiting N pages
    #[arg(long, short = 'l')]
    page_limit: Option<usize>,

    /// Output format (json or markdown)
    #[arg(long, value_enum, default_value_t = OutputFormatArg::Markdown)]
    format: OutputFormatArg,

    /// Timeout in seconds for individual HTTP requests
    #[arg(long, default_value_t = 10)]
    timeout: u64,

    /// Total crawl timeout in seconds
    #[arg(long)]
    crawl_timeout: Option<u64>,
}

#[derive(Debug, clap::Subcommand)]
enum Command {
    /// Crawl a site in memory
    Crawl(CrawlArgs),
}

#[derive(Debug, Parser)]
#[command(name = "crawlyx", version, about = "Concurrent web crawler in Rust")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[tokio::main]
async fn main() {
    let cli = Cli::parse();

    let crawl = match cli.command {
        Command::Crawl(args) => args,
    };

    let start_url = match Url::parse(&crawl.url) {
        Ok(u) => u,
        Err(e) => {
            eprintln!("Invalid seed URL '{}': {}", crawl.url, e);
            std::process::exit(1);
        }
    };

    let visited = Arc::new(VisitedTable::new());
    let queue = InProcessQueue::new(2048);
    let graph = Arc::new(Graph::new(crawl.url.clone()));
    
    let fetcher = Arc::new(Fetcher::new(
        "crawlyx-rs/0.1",
        Some(Duration::from_secs(crawl.timeout)),
    ));

    let state = Arc::new(CrawlState {
        visited: Arc::clone(&visited),
        queue: Arc::clone(&queue),
        graph: Arc::clone(&graph),
        fetcher,
        pages_crawled: AtomicUsize::new(0),
        in_flight: AtomicUsize::new(0),
        page_limit: crawl.page_limit.unwrap_or(usize::MAX),
        max_depth: crawl.depth,
        shutdown: AtomicBool::new(false),
        notify: Arc::new(tokio::sync::Notify::new()),
    });

    // Push the seed WorkUnit onto the queue and mark it in visited
    let start_url_str = start_url.to_string();
    visited.insert(&start_url_str); // mark in visited hashmap

    queue.push(WorkUnit {
        url: start_url.clone(),
        depth: 0,
        parent_node_id: None,
    }).await;

    // Listen for Ctrl+C in a separate task
    let state_clone = Arc::clone(&state);
    tokio::spawn(async move {
        if let Ok(_) = tokio::signal::ctrl_c().await {
            eprintln!("\nCtrl+C received. Gracefully shutting down... Waiting for in-flight tasks to finish.");
            state_clone.shutdown.store(true, Ordering::SeqCst);
        }
    });

    // Run the dispatcher with optional crawl timeout
    let dispatcher_future = crawlyx_rs::dispatcher::run_dispatcher(Arc::clone(&state), crawl.workers);
    
    let start_time = Instant::now();
    if let Some(t_limit) = crawl.crawl_timeout {
        match tokio::time::timeout(Duration::from_secs(t_limit), dispatcher_future).await {
            Ok(_) => {}
            Err(_) => {
                eprintln!("\nCrawl timeout of {} seconds reached. Shutting down... Waiting for in-flight tasks to finish.", t_limit);
                state.shutdown.store(true, Ordering::SeqCst);
                // Wait for any remaining in-flight tasks
                while state.in_flight.load(Ordering::SeqCst) > 0 {
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
            }
        }
    } else {
        dispatcher_future.await;
    }

    let duration = start_time.elapsed();

    // Derive tree from graph
    if let Some(tree) = crawlyx_rs::tree::derive_tree(&graph, &start_url_str) {
        // Format and print output
        let output = match crawl.format {
            OutputFormatArg::Json => crawlyx_rs::output::format_json(&tree),
            OutputFormatArg::Markdown => crawlyx_rs::output::format_markdown(&graph, &tree),
        };
        println!("{}", output);
    } else {
        eprintln!("Failed to derive crawl tree.");
    }

    eprintln!("Time elapsed: {} ms", duration.as_millis());
}
