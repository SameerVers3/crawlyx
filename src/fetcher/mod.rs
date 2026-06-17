use std::time::Duration;
use reqwest::Client;
use chromiumoxide::Browser;

#[derive(Debug)]
pub enum FetchError {
    Network(reqwest::Error),
    BadStatus(u16),
    Timeout,
    BrowserError(String),
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::Network(e) => write!(f, "network error: {}", e),
            FetchError::BadStatus(s) => write!(f, "bad status: {}", s),
            FetchError::Timeout => write!(f, "request timed out"),
            FetchError::BrowserError(e) => write!(f, "browser error: {}", e),
        }
    }
}

impl std::error::Error for FetchError {}

impl From<reqwest::Error> for FetchError {
    fn from(e: reqwest::Error) -> Self {
        if e.is_timeout() {
            FetchError::Timeout
        } else {
            FetchError::Network(e)
        }
    }
}

pub enum FetcherBackend {
    Reqwest(Client),
    Headless(Browser),
}

pub struct Fetcher {
    backend: FetcherBackend,
}

impl Fetcher {
    pub fn new_reqwest(user_agent: &str, timeout: Option<Duration>) -> Self {
        let mut builder = Client::builder()
            .user_agent(user_agent.to_string())
            .redirect(reqwest::redirect::Policy::limited(10))
            .pool_max_idle_per_host(100)
            .pool_idle_timeout(Duration::from_secs(90));

        if let Some(t) = timeout {
            builder = builder.timeout(t);
        } else {
            builder = builder.timeout(Duration::from_secs(10));
        }

        let client = builder.build().expect("failed to build HTTP client");
    
        Self {
            backend: FetcherBackend::Reqwest(client),
        }
    }

    pub fn new_headless(browser: Browser) -> Self {
        Self {
            backend: FetcherBackend::Headless(browser),
        }
    }

    pub async fn fetch(&self, url: &str) -> Result<(String, u16), FetchError> {
        match &self.backend {
            FetcherBackend::Reqwest(client) => {
                let response = client.get(url).send().await?;

                let status = response.status();
                let status_code = status.as_u16();
                if !status.is_success() {
                    return Err(FetchError::BadStatus(status_code));
                }

                let body = response.text().await?;

                Ok((body, status_code))
            }
            FetcherBackend::Headless(browser) => {
                // Open a blank page first so we can configure network domain settings before navigating
                let page = browser.new_page("about:blank").await.map_err(|e| FetchError::BrowserError(e.to_string()))?;
                
                // Enable network domain and set blocked URL patterns (CSS, images, webfonts, trackers)
                use chromiumoxide::cdp::browser_protocol::network::{EnableParams, SetBlockedUrLsParams, BlockPattern};
                page.execute(EnableParams::default()).await.map_err(|e| FetchError::BrowserError(e.to_string()))?;
                
                let block_patterns = vec![
                    BlockPattern { url_pattern: "*://*/*.css".to_string(), block: true },
                    BlockPattern { url_pattern: "*://*/*.png".to_string(), block: true },
                    BlockPattern { url_pattern: "*://*/*.jpg".to_string(), block: true },
                    BlockPattern { url_pattern: "*://*/*.jpeg".to_string(), block: true },
                    BlockPattern { url_pattern: "*://*/*.gif".to_string(), block: true },
                    BlockPattern { url_pattern: "*://*/*.svg".to_string(), block: true },
                    BlockPattern { url_pattern: "*://*/*.woff*".to_string(), block: true },
                    BlockPattern { url_pattern: "*://*/*.ttf".to_string(), block: true },
                    BlockPattern { url_pattern: "*://*/*.ico".to_string(), block: true },
                    BlockPattern { url_pattern: "*://*/*.mp4".to_string(), block: true },
                    BlockPattern { url_pattern: "*://*/*.webm".to_string(), block: true },
                    BlockPattern { url_pattern: "*://*/*.ogg".to_string(), block: true },
                    BlockPattern { url_pattern: "*://*doubleclick.net/*".to_string(), block: true },
                    BlockPattern { url_pattern: "*://*google-analytics.com/*".to_string(), block: true },
                    BlockPattern { url_pattern: "*://*googletagmanager.com/*".to_string(), block: true },
                ];

                let block_params = SetBlockedUrLsParams {
                    url_patterns: Some(block_patterns),
                };
                page.execute(block_params).await.map_err(|e| FetchError::BrowserError(e.to_string()))?;
                
                // Navigate to the target URL
                page.goto(url).await.map_err(|e| FetchError::BrowserError(e.to_string()))?;
                page.wait_for_navigation().await.map_err(|e| FetchError::BrowserError(e.to_string()))?;
                
                let body = page.content().await.map_err(|e| FetchError::BrowserError(e.to_string()))?;
                let _ = page.close().await;
                Ok((body, 200))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    #[tokio::test]
    async fn fetch_real_page_returns_html() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0; 1024];
            let _ = socket.read(&mut buf).await;
            let response = "HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 37\r\n\r\n<html><body>Hello World</body></html>";
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        let fetcher = Fetcher::new_reqwest("Crawlyx/1.0", None);
        let result = fetcher.fetch(&format!("http://{}", addr)).await;
        assert!(result.is_ok(), "fetch failed: {:?}", result.err());
        let (body, status) = result.unwrap();
        assert_eq!(status, 200);
        assert!(body.contains("<html") || body.contains("<HTML"));
    }

    #[tokio::test]
    async fn fetch_bad_status_returns_error() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut buf = [0; 1024];
            let _ = socket.read(&mut buf).await;
            let response = "HTTP/1.1 404 Not Found\r\nContent-Length: 0\r\n\r\n";
            socket.write_all(response.as_bytes()).await.unwrap();
        });

        let fetcher = Fetcher::new_reqwest("Crawlyx/1.0", None);
        let result = fetcher.fetch(&format!("http://{}", addr)).await;
        assert!(matches!(result, Err(FetchError::BadStatus(404))));
    }

    #[tokio::test]
    async fn fetch_follows_redirects() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let addr_str = addr.to_string();
        
        tokio::spawn(async move {
            // first request: redirect
            {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buf = [0; 1024];
                let _ = socket.read(&mut buf).await;
                let response = format!("HTTP/1.1 302 Found\r\nLocation: http://{}/target\r\nContent-Length: 0\r\n\r\n", addr_str);
                socket.write_all(response.as_bytes()).await.unwrap();
            }

            // second request: ok
            {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut buf = [0; 1024];
                let _ = socket.read(&mut buf).await;
                let response = "HTTP/1.1 200 OK\r\nContent-Length: 5\r\n\r\nhello";
                socket.write_all(response.as_bytes()).await.unwrap();
            }
        });

        let fetcher = Fetcher::new_reqwest("Crawlyx/1.0", None);
        let result = fetcher.fetch(&format!("http://{}", addr)).await;
        assert!(result.is_ok(), "redirect fetch failed: {:?}", result.err());
    }

    #[tokio::test]
    async fn fetch_invalid_url_returns_network_error() {
        let fetcher = Fetcher::new_reqwest("Crawlyx/1.0", None);
        let result = fetcher.fetch("https://this-domain-does-not-exist-xyz.com").await;
        assert!(matches!(result, Err(FetchError::Network(_)) | Err(FetchError::Timeout)));
    }
}
