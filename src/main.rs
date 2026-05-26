
use std::sync::Arc;
use std::time::{Duration, Instant};

use clap::{Parser, ValueEnum};

use crawlyx_rs::{
    config::{CrawlConfig, OutputFormat},
    graph::Graph,
    hashtable::VisitedTable,
    queue::inprocess::InProcessQueue,
    scheduler::Scheduler,
};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputFormatArg {
    Html,
    Markdown,
}

impl From<OutputFormatArg> for OutputFormat {
    fn from(value: OutputFormatArg) -> Self {
        match value {
            OutputFormatArg::Html => OutputFormat::Html,
            OutputFormatArg::Markdown => OutputFormat::Markdown,
        }
    }
}

#[derive(Debug, Parser)]
#[command(name = "crawlyx", version, about = "Concurrent web crawler in Rust")]
struct Cli {
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
    #[arg(long)]
    max_pages: Option<usize>,

    /// Output format to store in the graph node content (Html or Markdown)
    #[arg(long, value_enum, default_value_t = OutputFormatArg::Markdown)]
    format: OutputFormatArg,

    /// Only crawl URLs on the same domain as the seed
    #[arg(long, default_value_t = true)]
    same_domain: bool,

    /// Allow subdomains when same-domain is enabled
    #[arg(long, default_value_t = false)]
    allow_subdomains: bool,

    /// Respect robots.txt (todo)
    #[arg(long, default_value_t = false)]
    respect_robots: bool,

    /// Crawl delay in milliseconds (todo)
    #[arg(long)]
    crawl_delay_ms: Option<u64>,

    /// User agent string to send in requests (todo)
    #[arg(long, default_value = "crawlyx-rs/0.1")]
    user_agent: String,
}

fn main() {
    //console_subscriber::init();
    let cli = Cli::parse();

    let start = Instant::now();

    let mut config = CrawlConfig::new(cli.url.clone());
    config.max_depth = cli.depth;
    config.max_pages = cli.max_pages;
    config.same_domain_only = cli.same_domain;
    config.allow_subdomains = cli.allow_subdomains;
    config.output_format = cli.format.into();
    config.respect_robots_txt = cli.respect_robots;
    config.crawl_delay = cli.crawl_delay_ms.map(Duration::from_millis);
    config.user_agent = cli.user_agent;
    let config = Arc::new(config);

    let queue = InProcessQueue::new(512);
    let hashtable = Arc::new(VisitedTable::new());
    let graph = Arc::new(Graph::new(config.start_url.clone()));

    let scheduler = Scheduler::new(queue, hashtable, graph, cli.workers, cli.depth, config);
    scheduler.run(cli.url);

    let duration = start.elapsed();
    println!("Time elapsed: {} ms", duration.as_millis());
}

