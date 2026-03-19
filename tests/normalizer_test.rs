use my_crate::normalizer::normalize;

struct Case<'a> {
    input: &'a str,
    base: Option<&'a str>,
    expected: &'a str,
}

#[test]
fn test_full_pipeline() {
    let cases = [
        // Example: Scheme & Host + Query + Fragment
        Case { 
            input: "HTTP://SITE.COM:80/page?b=2&a=1#section", 
            base: None, 
            expected: "http://site.com/page?a=1&b=2" 
        },
        // Relative URL + query sort
        Case { 
            input: "../other?z=1&a=2", 
            base: Some("http://site.com/a/b/page.html"), 
            expected: "http://site.com/a/other?a=2&z=1" 
        },
        // Protocol-relative + port removal
        Case { 
            input: "//CDN.SITE.COM:443/asset", 
            base: Some("https://site.com/page"), 
            expected: "https://cdn.site.com/asset" 
        },
        // File with fragment
        Case { 
            input: "HTTP://Site.Com/page.html#hero", 
            base: None, 
            expected: "http://site.com/page.html" 
        },
        // Root-relative + query sorting
        Case { 
            input: "/search?q=hello&lang=en", 
            base: Some("https://SITE.COM:443/home"), 
            expected: "https://site.com/search?lang=en&q=hello" 
        },
        // Clean URL
        Case { 
            input: "http://site.com/page", 
            base: None, 
            expected: "http://site.com/page" 
        },
    ];

    for case in &cases {
        let result = normalize(case.input, case.base);
        assert_eq!(
            result, case.expected,
            "Failed: input={} base={:?} got={}", 
            case.input, case.base, result
        );
    }
}

#[test]
fn test_dedup_equivalence() {
    let pairs = [
        ("HTTP://site.com/page", "http://site.com/page"),
        ("http://site.com:80/page", "http://site.com/page"),
        ("http://site.com/page#section", "http://site.com/page"),
        ("http://site.com/page?b=2&a=1", "http://site.com/page?a=1&b=2"),
        ("http://SITE.COM/page", "http://site.com/page"),
        ("http://site.com/page?b=2&a=1#frag", "http://site.com/page?a=1&b=2"),
    ];

    for (a, b) in pairs {
        let na = normalize(a, None);
        let nb = normalize(b, None);
        assert_eq!(na, nb, "Dedup failed: A={} B={}", a, b);
    }
}