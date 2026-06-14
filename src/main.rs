
use std::sync::Arc;
use std::time::{Duration, Instant};
use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crawlyx_rs::{
    config::{CrawlConfig, OutputFormat, OutputMode, OutputPathMode},
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

#[derive(Debug, Clone, Copy, ValueEnum)]
enum OutputPathModeArg {
    Relative,
    Original,
}

impl From<OutputPathModeArg> for OutputPathMode {
    fn from(value: OutputPathModeArg) -> Self {
        match value {
            OutputPathModeArg::Relative => OutputPathMode::Relative,
            OutputPathModeArg::Original => OutputPathMode::Original,
        }
    }
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

#[derive(Debug, Parser)]
struct CloneArgs {
    #[command(flatten)]
    crawl: CrawlArgs,

    /// Output directory for the cloned site
    #[arg(long, short = 'o')]
    output: PathBuf,

    /// How URLs map to local paths
    #[arg(long, value_enum, default_value_t = OutputPathModeArg::Relative)]
    path_mode: OutputPathModeArg,

    /// Rewrite internal links to local mirrored file paths (recommended for offline browsing)
    #[arg(long, default_value_t = true)]
    rewrite_links: bool,

    /// Keep original extensions in output paths (e.g. keep `.html`), even when output is Markdown.
    #[arg(long, default_value_t = false)]
    keep_extension: bool,
}

#[derive(Debug, clap::Subcommand)]
enum Command {
    /// Crawl a site in memory
    Crawl(CrawlArgs),
    /// Crawl and mirror pages locally 
    Clone(CloneArgs),
}

#[derive(Debug, Parser)]
#[command(name = "crawlyx", version, about = "Concurrent web crawler in Rust")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[tokio::main]
async fn main() {
    //console_subscriber::init();
    let cli = Cli::parse();

    let start = Instant::now();

    let (crawl, mut config) = match cli.command {
        Command::Crawl(args) => {
            let mut cfg = CrawlConfig::new(args.url.clone());
            cfg.output_mode = OutputMode::Crawl;
            (args, cfg)
        }
        Command::Clone(args) => {
            let mut cfg = CrawlConfig::new(args.crawl.url.clone());
            cfg.output_mode = OutputMode::Clone;
            cfg.output_dir = Some(args.output);
            cfg.output_path_mode = args.path_mode.into();
            cfg.rewrite_links = args.rewrite_links;
            cfg.keep_extension = args.keep_extension;
            (args.crawl, cfg)
        }
    };

    config.max_depth = crawl.depth;
    config.max_pages = crawl.max_pages;
    config.same_domain_only = crawl.same_domain;
    config.allow_subdomains = crawl.allow_subdomains;
    config.output_format = crawl.format.into();
    config.respect_robots_txt = crawl.respect_robots;
    config.crawl_delay = crawl.crawl_delay_ms.map(Duration::from_millis);
    config.user_agent = crawl.user_agent;

    let config = Arc::new(config);

    // Bootstrap storage backends based on environment variables
    let redis_client = crawlyx_rs::storage::create_redis_client();

    let (queue, hashtable): (
        Arc<dyn crawlyx_rs::queue::Queue>,
        Arc<dyn crawlyx_rs::hashtable::VisitedStore>,
    ) = if let Some(client) = redis_client {
        println!("Redis/Upstash storage detected. Bootstrapping distributed backends.");
        let q = Arc::new(crawlyx_rs::queue::redis_queue::RedisQueue::new(client.clone(), "crawlyx_queue".to_string()));
        let h = Arc::new(crawlyx_rs::hashtable::RedisVisitedTable::new(client, "crawlyx_visited".to_string()));
        (q, h)
    } else {
        println!("No Redis environment variables set. Running in-memory crawler.");
        let q = InProcessQueue::new(512);
        let h = Arc::new(VisitedTable::new());
        (q, h)
    };

    let graph = Arc::new(Graph::new(config.start_url.clone()));

    let scheduler = Scheduler::new(queue, hashtable, graph, crawl.workers, crawl.depth, config);
    scheduler.run(crawl.url).await;

    let duration = start.elapsed();
    println!("Time elapsed: {} ms", duration.as_millis());
}

