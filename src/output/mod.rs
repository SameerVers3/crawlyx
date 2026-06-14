use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};

use url::Url;

use crate::config::{CrawlConfig, OutputFormat, OutputPathMode};

pub mod manifest;

#[derive(Debug, thiserror::Error)]
pub enum OutputError {
    #[error("invalid url: {0}")]
    InvalidUrl(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Debug, Clone)]
pub struct CloneWriter {
    output_dir: PathBuf,
    output_format: OutputFormat,
    output_path_mode: OutputPathMode,
    keep_extension: bool,
    rewrite_links: bool,
}

impl CloneWriter {
    pub fn from_config(config: &CrawlConfig) -> Option<Self> {
        let output_dir = config.output_dir.clone()?;
        Some(Self {
            output_dir,
            output_format: config.output_format,
            output_path_mode: config.output_path_mode,
            keep_extension: config.keep_extension,
            rewrite_links: config.rewrite_links,
        })
    }

    /// Returns the relative file path written (relative to output_dir).
    pub fn write_page(&self, url: &str, content: &str) -> Result<PathBuf, OutputError> {
        let url = Url::parse(url).map_err(|_| OutputError::InvalidUrl(url.to_string()))?;

        let content = if self.rewrite_links {
            self.rewrite_content_links(&url, content)
        } else {
            content.to_string()
        };

        let rel_path = self.url_to_relative_path(&url);
        let out_path = self.output_dir.join(&rel_path);

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)?;
        }

        fs::write(&out_path, content.as_bytes())?;
        Ok(rel_path)
    }

    pub fn manifest_path(&self) -> PathBuf {
        self.output_dir.join("manifest.json")
    }

    pub fn output_dir(&self) -> &Path {
        &self.output_dir
    }

    fn url_to_relative_path(&self, url: &Url) -> PathBuf {
        let host = url.host_str().unwrap_or("unknown-host");

        let mut base = match self.output_path_mode {
            OutputPathMode::Relative => PathBuf::from(host),
            OutputPathMode::Original => {
                let scheme = url.scheme();
                PathBuf::from(scheme).join(host)
            }
        };

        // Use only the URL path; ignore query/fragment in filename.
        let mut path = url.path().to_string();
        if path.is_empty() {
            path = "/".to_string();
        }

        // Remove leading '/'
        if path.starts_with('/') {
            path.remove(0);
        }

        // If empty, it's the root.
        if path.is_empty() {
            path = "index".to_string();
        }

        // If ends with '/', treat as directory index.
        if path.ends_with('/') {
            path.pop(); // remove trailing '/'
            if path.is_empty() {
                path.push_str("index");
            } else {
                path.push_str("/index");
            }
        }

        // If it has no extension, treat as directory index (unless it's already `index`).
        let path_buf = Path::new(&path);
        let has_ext = path_buf.extension().is_some();
        let mut final_rel = if has_ext {
            PathBuf::from(path)
        } else if path == "index" || path.ends_with("/index") {
            PathBuf::from(path)
        } else {
            PathBuf::from(path).join("index")
        };

        // File extension logic.
        if !self.keep_extension {
            match self.output_format {
                OutputFormat::Markdown => {
                    // convert *.html -> *.md, or add .md
                    final_rel.set_extension("md");
                }
                OutputFormat::Html => {
                    // ensure .html if missing extension (already handled by index + set_extension)
                    if final_rel.extension().is_none() {
                        final_rel.set_extension("html");
                    }
                }
            }
        }

        // If keep_relative is true, still ensure we have SOME extension for directory indexes.
        if self.keep_extension {
            if final_rel.extension().is_none() {
                final_rel.set_extension(match self.output_format {
                    OutputFormat::Markdown => "md",
                    OutputFormat::Html => "html",
                });
            }
        }

        // Basic filename sanitization: avoid empty segments and Windows-reserved characters.
        base.push(sanitize_path(&final_rel));
        base
    }

    fn rewrite_content_links(&self, base_url: &Url, content: &str) -> String {
        match self.output_format {
            // In HTML mode, do a light regex-free rewrite on common attributes.
            // (We keep it simple for now; a full HTML serializer can come later.)
            OutputFormat::Html => rewrite_html_links(base_url, content, |u| self.to_local_href(base_url, u)),
            OutputFormat::Markdown => rewrite_markdown_links(base_url, content, |u| self.to_local_href(base_url, u)),
        }
    }

    fn to_local_href(&self, base: &Url, raw: &str) -> Option<String> {
        let raw_trim = raw.trim();
        if raw_trim.is_empty() {
            return None;
        }

        // Don’t rewrite special schemes.
        let lower = raw_trim.to_ascii_lowercase();
        if lower.starts_with("mailto:") || lower.starts_with("javascript:") || lower.starts_with("tel:") {
            return None;
        }

        // Resolve relative URL vs base
        let target = base.join(raw_trim).ok().or_else(|| Url::parse(raw_trim).ok())?;

        // Rewrite only same-host links (and same scheme/host if original mode matters)
        if target.host_str() != base.host_str() {
            return None;
        }

        // Compute current page path and target page path as written in the mirror.
        let base_rel = self.url_to_relative_path(base);
        let target_rel = self.url_to_relative_path(&target);

        let base_dir = base_rel.parent().unwrap_or(Path::new(""));
        let rel_from_base = pathdiff::diff_paths(&target_rel, base_dir).unwrap_or(target_rel);

        // Convert to POSIX-style path separators for href.
        let mut s = rel_from_base.to_string_lossy().to_string();
        s = s.replace('\\', "/");

        // Preserve URL fragment if present.
        if let Some(frag) = target.fragment() {
            if !frag.is_empty() {
                s.push('#');
                s.push_str(frag);
            }
        }

        Some(s)
    }
}

fn rewrite_html_links<F>(_base_url: &Url, html: &str, mut map: F) -> String
where
    F: FnMut(&str) -> Option<String>,
{
    // Minimal attribute rewrites (string-based). This intentionally avoids a full HTML re-serializer.
    // Supported: href="...", src="..." (double quotes only for v1)
    let mut out = String::with_capacity(html.len());
    let mut i = 0;
    while i < html.len() {
        let rest = &html[i..];
        if let Some(pos) = rest.find("href=\"").or_else(|| rest.find("src=\"")) {
            // copy up to attribute
            out.push_str(&rest[..pos]);
            i += pos;

            let attr = if html[i..].starts_with("href=\"") { "href=\"" } else { "src=\"" };
            out.push_str(attr);
            i += attr.len();

            if let Some(end) = html[i..].find('"') {
                let val = &html[i..i + end];
                if let Some(new_val) = map(val) {
                    out.push_str(&new_val);
                } else {
                    out.push_str(val);
                }
                out.push('"');
                i += end + 1;
            } else {
                // malformed; copy rest
                out.push_str(&html[i..]);
                break;
            }
        } else {
            out.push_str(rest);
            break;
        }
    }
    out
}

fn rewrite_markdown_links<F>(_base_url: &Url, md: &str, mut map: F) -> String
where
    F: FnMut(&str) -> Option<String>,
{
    // Rewrite markdown links/images: ](url) and ![](url)
    // Simple parser: find "](" then read until ')'.
    let mut out = String::with_capacity(md.len());
    let mut i = 0;
    while let Some(pos) = md[i..].find("](") {
        let abs = i + pos;
        out.push_str(&md[i..abs + 2]); // include "]("
        let start = abs + 2;
        if let Some(end_rel) = md[start..].find(')') {
            let end = start + end_rel;
            let url = &md[start..end];
            if let Some(new_url) = map(url) {
                out.push_str(&new_url);
            } else {
                out.push_str(url);
            }
            out.push(')');
            i = end + 1;
        } else {
            // no closing, copy rest
            out.push_str(&md[start..]);
            return out;
        }
    }
    out.push_str(&md[i..]);
    out
}

fn sanitize_path(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in p.components() {
        let s = comp.as_os_str().to_string_lossy();
        let cleaned: String = s
            .chars()
            .map(|c| match c {
                // very small cross-platform safe set
                ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
                _ => c,
            })
            .collect();

        if cleaned.is_empty() || cleaned == "." {
            continue;
        }
        out.push(OsStr::new(&cleaned));
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{OutputFormat, OutputPathMode};

    fn writer(tmp: &Path, format: OutputFormat, path_mode: OutputPathMode, keep_extension: bool) -> CloneWriter {
        CloneWriter {
            output_dir: tmp.to_path_buf(),
            output_format: format,
            output_path_mode: path_mode,
            keep_extension,
            rewrite_links: false,
        }
    }

    fn writer_with_rewrite(tmp: &Path, format: OutputFormat) -> CloneWriter {
        CloneWriter {
            output_dir: tmp.to_path_buf(),
            output_format: format,
            output_path_mode: OutputPathMode::Relative,
            keep_extension: false,
            rewrite_links: true,
        }
    }

    #[test]
    fn maps_root_to_index_md() {
        let dir = std::env::temp_dir().join("crawlyx_test_root");
        let _ = std::fs::remove_dir_all(&dir);
        let w = writer(&dir, OutputFormat::Markdown, OutputPathMode::Relative, false);
        let rel = w.write_page("https://docs.rust-lang.org/", "hi").unwrap();
        assert_eq!(rel.to_string_lossy(), "docs.rust-lang.org/index.md");
    }

    #[test]
    fn maps_directory_to_index_md() {
        let dir = std::env::temp_dir().join("crawlyx_test_dir");
        let _ = std::fs::remove_dir_all(&dir);
        let w = writer(&dir, OutputFormat::Markdown, OutputPathMode::Relative, false);
        let rel = w.write_page("https://docs.rust-lang.org/book/", "hi").unwrap();
        assert_eq!(rel.to_string_lossy(), "docs.rust-lang.org/book/index.md");
    }

    #[test]
    fn maps_html_to_md_when_keep_extension_false() {
        let dir = std::env::temp_dir().join("crawlyx_test_html_to_md");
        let _ = std::fs::remove_dir_all(&dir);
        let w = writer(&dir, OutputFormat::Markdown, OutputPathMode::Relative, false);
        let rel = w.write_page("https://docs.rust-lang.org/std/index.html?x=1#frag", "hi").unwrap();
        assert_eq!(rel.to_string_lossy(), "docs.rust-lang.org/std/index.md");
    }

    #[test]
    fn original_mode_includes_scheme() {
        let dir = std::env::temp_dir().join("crawlyx_test_scheme");
        let _ = std::fs::remove_dir_all(&dir);
        let w = writer(&dir, OutputFormat::Markdown, OutputPathMode::Original, false);
        let rel = w.write_page("https://example.com/a", "hi").unwrap();
        assert_eq!(rel.to_string_lossy(), "https/example.com/a/index.md");
    }

    #[test]
    fn keep_extension_keeps_html_extension() {
        let dir = std::env::temp_dir().join("crawlyx_test_keep_relative");
        let _ = std::fs::remove_dir_all(&dir);
        let w = writer(&dir, OutputFormat::Markdown, OutputPathMode::Relative, true);
        let rel = w.write_page("https://example.com/a.html", "hi").unwrap();
        assert_eq!(rel.to_string_lossy(), "example.com/a.html");
    }

    #[test]
    fn rewrites_markdown_internal_links_to_relative_paths() {
        let dir = std::env::temp_dir().join("crawlyx_test_rewrite_md");
        let _ = std::fs::remove_dir_all(&dir);
        let w = writer_with_rewrite(&dir, OutputFormat::Markdown);

        let input = "See [Book](/book/) and [Chapter](/book/ch1.html#intro).";
        let rel = w.write_page("https://docs.rust-lang.org/", input).unwrap();
        assert_eq!(rel.to_string_lossy(), "docs.rust-lang.org/index.md");

        let wrote = std::fs::read_to_string(dir.join("docs.rust-lang.org/index.md")).unwrap();
        assert!(wrote.contains("[Book](book/index.md)"));
        assert!(wrote.contains("[Chapter](book/ch1.md#intro)"));
    }

    #[test]
    fn rewrites_html_internal_links_to_relative_paths() {
        let dir = std::env::temp_dir().join("crawlyx_test_rewrite_html");
        let _ = std::fs::remove_dir_all(&dir);
        let w = writer_with_rewrite(&dir, OutputFormat::Html);

    let input = r#"<a href="/book/">Book</a><img src="/img/logo.png"/>"#;
        let rel = w.write_page("https://docs.rust-lang.org/", input).unwrap();
        assert_eq!(rel.to_string_lossy(), "docs.rust-lang.org/index.html");

        let wrote = std::fs::read_to_string(dir.join("docs.rust-lang.org/index.html")).unwrap();
        assert!(wrote.contains("href=\"book/index.html\""));
        assert!(wrote.contains("src=\"img/logo.png\""));
    }
}
