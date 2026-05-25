use scraper::{Html, Selector};

pub fn extract_urls(html: &str, base_url: &str) -> Vec<String> {
    let document = Html::parse_document(html);
    let selector = Selector::parse("a[href], iframe[src], frame[src]").unwrap();

    let mut urls = Vec::new();

    for element in document.select(&selector) {
        let attr = if element.value().name() == "a" {
            element.value().attr("href")
        } else {
            element.value().attr("src")
        };

        if let Some(raw_url) = attr {
            if raw_url.starts_with('#')
                || raw_url.starts_with("javascript:")
                || raw_url.starts_with("mailto:")
            {
                continue;
            }

            let normalized = crate::normalizer::normalize(raw_url, Some(base_url));
            urls.push(normalized);
        }
    }

    urls.sort();
    urls.dedup();
    urls
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_absolute_links() {
        let html = r#"<html><body>
            <a href="https://example.com/page">link</a>
        </body></html>"#;
        let urls = extract_urls(html, "https://example.com");
        assert!(urls.contains(&"https://example.com/page".to_string()));
    }

    #[test]
    fn resolves_relative_links() {
        let html = r#"<html><body>
            <a href="/about">about</a>
        </body></html>"#;
        let urls = extract_urls(html, "https://example.com");
        assert!(urls.contains(&"https://example.com/about".to_string()));
    }

    #[test]
    fn skips_javascript_and_mailto() {
        let html = r#"<html><body>
            <a href="javascript:void(0)">js</a>
            <a href="mailto:a@b.com">mail</a>
        </body></html>"#;
        let urls = extract_urls(html, "https://example.com");
        assert!(urls.is_empty());
    }

    #[test]
    fn skips_fragments() {
        let html = r##"<html><body>
            <a href="#section">anchor</a>
        </body></html>"##;
        let urls = extract_urls(html, "https://example.com");
        assert!(urls.is_empty());
    }

    #[test]
    fn extracts_iframe_src() {
        let html = r#"<html><body>
            <iframe src="https://other.com/embed"></iframe>
        </body></html>"#;
        let urls = extract_urls(html, "https://example.com");
        assert!(urls.contains(&"https://other.com/embed".to_string()));
    }

    #[test]
    fn no_duplicates_after_normalization() {
        let html = r#"<html><body>
            <a href="/page">one</a>
            <a href="/page">two</a>
        </body></html>"#;
        let urls = extract_urls(html, "https://example.com");
        assert_eq!(urls.len(), 1);
    }
}
