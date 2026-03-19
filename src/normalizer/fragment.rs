#[allow(dead_code)]
pub fn remove_fragment(input: &str) -> String {
    match input.find('#') {
        Some(idx) => input[..idx].to_string(),
        None => input.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_remove_fragment() {
        let cases = [
            ("http://site.com/page#section", "http://site.com/page"),
            ("http://site.com/page", "http://site.com/page"),
            ("http://site.com/page#foo#bar", "http://site.com/page"),
        ];
        for (input, expected) in &cases {
            assert_eq!(remove_fragment(input), *expected);
        }
    }
}