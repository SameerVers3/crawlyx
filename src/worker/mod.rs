use std::sync::Arc;
use std::collections::HashMap;
use crate::fetcher::{Fetcher, FetchError};
use crate::queue::WorkUnit;
use crate::config::CrawlConfig;

#[derive(Debug, Clone)]
pub struct ExtractedPage {
    pub url: String,
    pub title: Option<String>,
    pub metadata: HashMap<String, String>, // Author, date, description
    pub markdown: String,                  // The clean, LLM-ready content
    pub status_code: u16,
    pub extraction_time_ms: u64,
}

impl ExtractedPage {
    pub fn error(url: String, status_code: u16, extraction_time_ms: u64) -> Self {
        Self {
            url,
            title: None,
            metadata: HashMap::new(),
            markdown: String::new(),
            status_code,
            extraction_time_ms,
        }
    }
}

pub struct WorkerResult {
    pub url: String,
    pub page: ExtractedPage,
    pub discovered_urls: Vec<String>,
}

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

pub async fn process_work(
    work: WorkUnit,
    fetcher: Arc<Fetcher>,
    _config: Arc<CrawlConfig>,
) -> WorkerResult {
    let start_time = std::time::Instant::now();
    let url = work.url.clone();
    
    let fetch_res = fetcher.fetch(&url).await;
    let extraction_time_ms = start_time.elapsed().as_millis() as u64;
    
    match fetch_res {
        Ok((fetched_html, status_code)) => {
            let metadata = extract_metadata(&fetched_html);
            let mut cursor = std::io::Cursor::new(&fetched_html);
            let parsed_url = match url::Url::parse(&url) {
                Ok(u) => u,
                Err(_) => {
                    return WorkerResult {
                        url: url.clone(),
                        page: ExtractedPage::error(url, status_code, extraction_time_ms),
                        discovered_urls: vec![],
                    };
                }
            };
            
            let (distilled_html, title) = match readability::extractor::extract(&mut cursor, &parsed_url) {
                Ok(product) => (product.content, Some(product.title)),
                Err(_) => (String::new(), None)
            };
            
            let markdown = match htmd::convert(&distilled_html) {
                Ok(md) => md,
                Err(e) => {
                    eprintln!("htmd conversion error for {}: {:?}", url, e);
                    distilled_html.clone()
                }
            };
            
            let discovered_urls = crate::parser::extract_urls(&fetched_html, &url);
            
            let page = ExtractedPage {
                url: url.clone(),
                title,
                metadata,
                markdown,
                status_code,
                extraction_time_ms,
            };
            
            WorkerResult {
                url,
                page,
                discovered_urls,
            }
        }
        Err(e) => {
            let status_code = match e {
                FetchError::BadStatus(code) => code,
                _ => 0,
            };
            
            WorkerResult {
                url: url.clone(),
                page: ExtractedPage::error(url, status_code, extraction_time_ms),
                discovered_urls: vec![],
            }
        }
    }
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
