pub mod scheme_host;
pub mod relative;
pub mod fragment;
pub mod port;
pub mod query;
pub mod trailing_slash;

pub fn normalize(input: &str, base: Option<&str>) -> String {
    let mut url = input.to_string();

    //lowercase scheme and host
    url = scheme_host::lowercase_scheme_host(&url);

    //resolve relative URLs
    if let Some(base_url) = base {
        url = relative::resolve_relative(&url, base_url);
    }

    //remove default port
    url = port::remove_default_port(&url);

    //remove fragment
    url = fragment::remove_fragment(&url);

    //sort query parameters
    url = query::sort_query_params(&url);

    //normalize trailing slash
    url = trailing_slash::normalize_trailing_slash(&url);

    url
}