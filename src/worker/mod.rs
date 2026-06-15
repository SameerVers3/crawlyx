use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::collections::HashMap;
use crate::state::CrawlState;
use crate::work::WorkUnit;

#[derive(Debug, Clone)]
pub struct ExtractedPage {
    pub url: String,
    pub title: Option<String>,
    pub metadata: HashMap<String, String>,
    pub markdown: String,
    pub status_code: u16,
    pub extraction_time_ms: u64,
}

use crate::queue::Queue;

pub fn extract_metadata(html: &str) -> HashMap<String, String> {
    let mut metadata = HashMap::new();
    let document = scraper::Html::parse_document(html);
    
    if let Ok(meta_selector) = scraper::Selector::parse("meta") {
        for element in document.select(&meta_selector) {
            let name = element.value().attr("name")
                .or_else(|| element.value().attr("property"))
                .or_else(|| element.value().attr("http-equiv"));
            let content = element.value().attr("content");
            
            if let (Some(name), Some(content)) = (name, content) {
                let name_lower = name.to_lowercase();
                if name_lower.contains("author") {
                    metadata.insert("author".to_string(), content.to_string());
                } else if name_lower.contains("description") {
                    metadata.insert("description".to_string(), content.to_string());
                } else if name_lower.contains("date") || name_lower.contains("published_time") {
                    metadata.insert("date".to_string(), content.to_string());
                }
            }
        }
    }
    metadata
}

pub async fn worker(work: WorkUnit, state: Arc<CrawlState>) {
    let url = work.url.clone();
    let url_str = url.to_string();

    // 1. Reserve a page slot atomically before doing any work
    let slot = state.pages_crawled.fetch_add(1, Ordering::SeqCst);
    if slot >= state.page_limit {
        state.pages_crawled.fetch_sub(1, Ordering::SeqCst);
        state.in_flight.fetch_sub(1, Ordering::SeqCst);
        return; // page limit hit, drop this work unit
    }

    // 2. HTTP fetch the URL
    let fetch_res = state.fetcher.fetch(url.as_str()).await;

    // 3. Process result
    match fetch_res {
        Ok((fetched_html, _status_code)) => {
            let url_clone = url.clone();
            let url_str_clone = url_str.clone();
            
            let (markdown, discovered_urls) = tokio::task::spawn_blocking(move || {
                let _metadata = extract_metadata(&fetched_html);

                let mut cursor = std::io::Cursor::new(&fetched_html);
                let (distilled_html, _title) = match readability::extractor::extract(&mut cursor, &url_clone) {
                    Ok(product) => (product.content, Some(product.title)),
                    Err(_) => (String::new(), None)
                };

                let markdown = match htmd::convert(&distilled_html) {
                    Ok(md) => md,
                    Err(e) => {
                        eprintln!("htmd conversion error for {}: {:?}", url_str_clone, e);
                        distilled_html
                    }
                };

                let discovered_urls = crate::parser::extract_urls(&fetched_html, &url_str_clone);
                (markdown, discovered_urls)
            }).await.unwrap();

            // Add self as a node in the graph, link to parent if parent_node_id is Some
            let node = state.graph.add_node(url_str.clone(), Some(markdown), work.depth);
            if let Some(ref parent_id) = work.parent_node_id {
                if let Some(parent_node) = state.graph.get_node(parent_id) {
                    state.graph.add_edge(&parent_node, &node);
                }
            }

            // For each child URL
            for child_url_str in discovered_urls {
                if let Ok(child_url) = url::Url::parse(&child_url_str) {
                    if work.depth + 1 <= state.max_depth {
                        // Try inserting into visited hashmap - if it was already there, skip
                        if state.visited.insert(&child_url_str) {
                            state.queue.push(WorkUnit {
                                url: child_url,
                                depth: work.depth + 1,
                                parent_node_id: Some(url_str.clone()),
                            }).await;
                            state.notify.notify_one();
                        }
                    }
                }
            }
        }
        Err(e) => {
            eprintln!("Fetch failed for {}: {:?}", url_str, e);
            // Worker errors should decrement in_flight and not decrement pages_crawled (per Step 8)
        }
    }

    // Decrement in_flight when done (whether success or error)
    state.in_flight.fetch_sub(1, Ordering::SeqCst);
    state.notify.notify_one();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_metadata_parses_tags() {
        let html = r#"
        <html>
            <head>
                <meta name="author" content="Alice Smith">
                <meta property="og:description" content="My sweet home page">
                <meta name="pubdate" content="2026-06-15">
            </head>
        </html>
        "#;
        let meta = extract_metadata(html);
        assert_eq!(meta.get("author").unwrap(), "Alice Smith");
        assert_eq!(meta.get("description").unwrap(), "My sweet home page");
        assert_eq!(meta.get("date").unwrap(), "2026-06-15");
    }
}
