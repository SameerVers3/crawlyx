use url::Url;

#[allow(dead_code)]
pub fn remove_default_port(input: &str) -> String {
    if let Ok(mut url) = Url::parse(input) {
        let _ = match url.scheme() {
            "http" if url.port_or_known_default() == Some(80) => url.set_port(None),
            "https" if url.port_or_known_default() == Some(443) => url.set_port(None),
            _ => Ok(())
        };
        url.to_string()
    } else {
        input.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_default_port() {
        let cases = [
            ("http://site.com:80/page", "http://site.com/page"),
            ("https://site.com:443/page", "https://site.com/page"),
            ("http://site.com:8080/page", "http://site.com:8080/page"),
        ];
        for (input, expected) in &cases {
            assert_eq!(remove_default_port(input), *expected);
        }
    }
}