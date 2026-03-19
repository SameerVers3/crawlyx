use url::Url;

#[allow(dead_code)]
pub fn resolve_relative(input: &str, base: &str) -> String {
    if input.is_empty() || input.starts_with('#') {
        // empty or fragment-only resolves to base
        return base.to_string();
    }

    if let Ok(url) = Url::parse(input) {
        // absolute URL
        url.to_string()
    } else if let Ok(base_url) = Url::parse(base) {
        // relative URL resolution
        if let Ok(resolved) = base_url.join(input) {
            resolved.to_string()
        } else {
            input.to_string()
        }
    } else {
        input.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_resolve_relative() {
        let base = "http://site.com/a/b/page.html";
        let cases = [
            ("http://other.com/page", Some(base), "http://other.com/page"),
            ("/page", Some(base), "http://site.com/page"),
            ("./page", Some(base), "http://site.com/a/b/page"),
            ("../page", Some(base), "http://site.com/a/page"),
            ("../../page", Some(base), "http://site.com/page"),
            ("../../../page", Some(base), "http://site.com/page"),
            ("page", Some(base), "http://site.com/a/b/page"),
            ("//cdn.site.com/asset.js", Some(base), "http://cdn.site.com/asset.js"),
            ("", Some(base), base),
            ("#section", Some(base), base),
        ];
        for (input, b, expected) in &cases {
            assert_eq!(resolve_relative(input, b.unwrap()), *expected);
        }
    }
}