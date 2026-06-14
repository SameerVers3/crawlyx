use std::time::Duration;
use reqwest::Client;

#[derive(Debug)]
pub enum FetchError {
    Network(reqwest::Error),
    BadStatus(u16),
    Timeout,
}

impl std::fmt::Display for FetchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FetchError::Network(e) => write!(f, "network error: {}", e),
            FetchError::BadStatus(s) => write!(f, "bad status: {}", s),
            FetchError::Timeout => write!(f, "request timed out"),
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

pub struct Fetcher {
    client: Client,
}

impl Fetcher {
    pub fn new(user_agent: &str) -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10))
            .user_agent(user_agent.to_string())
            .redirect(reqwest::redirect::Policy::limited(10))
            .pool_max_idle_per_host(100)
            .pool_idle_timeout(Duration::from_secs(90))
            .build()
            .expect("failed to build HTTP client");
    
        Self {
            client
        }
    }

    pub async fn fetch(&self, url: &str) -> Result<(String, u16), FetchError> {
        let response = self.client.get(url).send().await?;

        let status = response.status();
        let status_code = status.as_u16();
        if !status.is_success() {
            return Err(FetchError::BadStatus(status_code));
        }

        let body = response.text().await?;

        Ok((body, status_code))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn fetch_real_page_returns_html() {
        let fetcher = Fetcher::new("Crawlyx/1.0");
        let result = fetcher.fetch("https://example.com").await;
        if let Err(ref e) = result {
            eprintln!("FETCH ERROR: {:?}", e);
        }
        assert!(result.is_ok());
        let (body, status) = result.unwrap();
        assert_eq!(status, 200);
        assert!(body.contains("<html") || body.contains("<HTML"));
    }

    #[tokio::test]
    async fn fetch_bad_status_returns_error() {
        let fetcher = Fetcher::new("Crawlyx/1.0");
        let result = fetcher.fetch("https://httpbin.org/status/404").await;
        assert!(matches!(result, Err(FetchError::BadStatus(404))));
    }

    #[tokio::test]
    async fn fetch_follows_redirects() {
        let fetcher = Fetcher::new("Crawlyx/1.0");
        let result = fetcher.fetch("https://httpbin.org/redirect/2").await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn fetch_invalid_url_returns_network_error() {
        let fetcher = Fetcher::new("Crawlyx/1.0");
        let result = fetcher.fetch("https://this-domain-does-not-exist-xyz.com").await;
        assert!(matches!(result, Err(FetchError::Network(_)) | Err(FetchError::Timeout)));
    }
}
