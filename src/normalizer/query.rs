use url::Url;

#[allow(dead_code)]
pub fn sort_query_params(input: &str) -> String {
    if let Ok(mut url) = Url::parse(input) {
        let mut pairs: Vec<_> = url.query_pairs().into_owned().collect();
        
        if pairs.is_empty() {
          return url.to_string()
        }
        
        pairs.sort_by(|a, b| {
            let ord = a.0.cmp(&b.0);
            if ord == std::cmp::Ordering::Equal {
                a.1.cmp(&b.1)
            } else {
                ord
            }
        });
        url.query_pairs_mut().clear().extend_pairs(pairs);
        url.to_string()
    } else {
        input.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sort_query_params() {
        let cases = [
            ("http://site.com/page?b=2&a=1", "http://site.com/page?a=1&b=2"),
            ("http://site.com/page?a=2&a=1", "http://site.com/page?a=1&a=2"),
            ("http://site.com/page", "http://site.com/page"),
        ];
        for (input, expected) in &cases {
            assert_eq!(sort_query_params(input), *expected);
        }
    }
}