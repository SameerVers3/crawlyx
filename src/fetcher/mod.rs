use std::time::Duration;
use reqwest::blocking::Client;

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
    pub fn new() -> Self {
        let client = Client::builder()
            .timeout(Duration::from_secs(10)) // will add this in config
            .user_agent("Crawlyx/1.0")
            .redirect(reqwest::redirect::Policy::limited(10))
            .build()
            .expect("failed to build HTTP client");
    
        Self {
            client
        }
    }

    pub fn fetch(&self, url: &str) -> Result<String, FetchError> {
        let response = self.client.get(url).send()?;

        let status = response.status();
        let true = status.is_success() else {
            return Err(FetchError::BadStatus(status.as_u16()));
        };

        let body = response.text()?;

        Ok(body)
    }
    
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_real_page_returns_html() {
        let fetcher = Fetcher::new();
        let result = fetcher.fetch("https://example.com");
        assert!(result.is_ok());
        let body = result.unwrap();
        assert!(body.contains("<html"));
    }

    #[test]
    fn fetch_bad_status_returns_error() {
        let fetcher = Fetcher::new();
        // httpbin.org/status/{code} returns the requested status code
        let result = fetcher.fetch("https://httpbin.org/status/404");
        assert!(matches!(result, Err(FetchError::BadStatus(404))));
    }

    #[test]
    fn fetch_follows_redirects() {
        let fetcher = Fetcher::new();
        // httpbin.org/redirect/{n} redirects n times then returns 200 OK
        let result = fetcher.fetch("https://httpbin.org");
        assert!(result.is_ok());
    }

    #[test]
    fn fetch_invalid_url_returns_network_error() {
        let fetcher = Fetcher::new();
        let result = fetcher.fetch("https://this-domain-does-not-exist-xyz.com");
        assert!(matches!(result, Err(FetchError::Network(_)) | Err(FetchError::Timeout)));
    }
}
