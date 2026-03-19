#[allow(dead_code)]
pub fn lowercase_scheme_host(url: &str) -> String {
    let parts: Vec<&str> = url.splitn(2, "://").collect();
    if parts.len() == 2 {
        let scheme = parts[0];
        let rest = parts[1];

        let host_path: Vec<&str> = rest.splitn(2,"/").collect();

        let host = host_path[0];

        let path = if host_path.len() == 2 {
          format!("/{}", host_path[1])
        } else {
          String::new()
        };

        format!("{}://{}{}", scheme.to_lowercase(), host.to_lowercase(), path)
    } else {
        url.to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lowercase_scheme_host() {
        let cases = [
            ("http://site.com/page", "http://site.com/page"),
            ("HTTP://site.com/page", "http://site.com/page"),
            ("http://SITE.COM/page", "http://site.com/page"),
            ("HTTP://SITE.COM/PAGE", "http://site.com/PAGE"),
            ("HTTPS://Site.Com/Path", "https://site.com/Path"),
            ("ftp://FILES.SITE.COM/", "ftp://files.site.com/"),
        ];
        for (input, expected) in &cases {
            assert_eq!(lowercase_scheme_host(input), *expected);
        }
    }
}