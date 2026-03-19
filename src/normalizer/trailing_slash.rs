use url::Url;

#[allow(dead_code)]
pub fn normalize_trailing_slash(input: &str) -> String {
    if let Ok(mut url) = Url::parse(input) {
        let path = url.path().to_string();
        if path.is_empty() {
            url.set_path("/");
        } else if path.ends_with('/') {
            // keep as is
        } else if path.contains('.') {
            // file, do nothing
        } else {
            url.set_path(&path); // keeping current path
        }
        url.to_string()
    } else {
        input.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trailing_slash() {
        let cases = [
            ("http://site.com", "http://site.com/"),
            ("http://site.com/", "http://site.com/"),
            ("http://site.com/page", "http://site.com/page"),
            ("http://site.com/page/", "http://site.com/page/"),
            ("http://site.com/page.html", "http://site.com/page.html"),
        ];
        for (input, expected) in &cases {
            assert_eq!(normalize_trailing_slash(input), *expected);
        }
    }
}