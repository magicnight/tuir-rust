//! HTML content rendering for terminal display.
//!
//! Reddit returns comment/message bodies as HTML (`body_html`). This module
//! walks the DOM depth-first and produces plain text lines suitable for
//! rendering inside a ratatui `Paragraph`.
//!
//! Supported tags: `p`, `br`, `a`, `code`, `pre`, `blockquote`, `ul`, `ol`,
//! `li`, `h1`-`h6`, `em`/`i`, `strong`/`b`. Anything else is traversed
//! transparently (its text survives, markup is dropped).

use scraper::node::Node;
use scraper::{ElementRef, Html};

/// Render an HTML fragment to plain-text lines.
///
/// Each returned `String` represents one output line (no trailing newline).
/// Blank lines separating blocks are represented as empty strings.
pub fn render_plain(html: &str) -> Vec<String> {
    let document = Html::parse_fragment(html);
    let mut ctx = RenderCtx::default();
    ctx.walk_children(document.root_element());
    ctx.finish()
}

/// Convenience helper: join [`render_plain`] output with `\n` and trim the
/// trailing blank lines that Reddit's HTML often leaves behind.
pub fn render_plain_string(html: &str) -> String {
    render_plain(html).join("\n").trim_end().to_string()
}

/// Strip all tags, returning the concatenated text content only.
pub fn strip_html(html: &str) -> String {
    let document = Html::parse_fragment(html);
    document
        .root_element()
        .text()
        .collect::<String>()
        .trim()
        .to_string()
}

/// Resolve Reddit short links to full URLs.
pub fn resolve_permalink(short: &str) -> String {
    if short.starts_with("http://") || short.starts_with("https://") {
        short.to_string()
    } else if short.starts_with('/') {
        format!("https://www.reddit.com{short}")
    } else {
        format!("https://www.reddit.com/{short}")
    }
}

#[derive(Default)]
struct RenderCtx {
    lines: Vec<String>,
    current: String,
    list_depth: usize,
    in_pre: bool,
}

impl RenderCtx {
    fn finish(mut self) -> Vec<String> {
        self.flush_line();
        while self.lines.last().map(String::is_empty).unwrap_or(false) {
            self.lines.pop();
        }
        self.lines
    }

    fn flush_line(&mut self) {
        let line = std::mem::take(&mut self.current);
        self.lines.push(line);
    }

    fn ensure_blank_line(&mut self) {
        if !self.current.is_empty() {
            self.flush_line();
        }
        if self.lines.last().map(|l| !l.is_empty()).unwrap_or(false) {
            self.lines.push(String::new());
        }
    }

    fn push_text(&mut self, text: &str) {
        if self.in_pre {
            for (i, segment) in text.split('\n').enumerate() {
                if i > 0 {
                    self.flush_line();
                }
                self.current.push_str(segment);
            }
            return;
        }

        let collapsed = collapse_whitespace(text);
        if collapsed.is_empty() {
            return;
        }
        if self.current.is_empty() {
            self.current.push_str(collapsed.trim_start());
        } else {
            self.current.push_str(&collapsed);
        }
    }

    fn walk_children(&mut self, element: ElementRef<'_>) {
        for child in element.children() {
            match child.value() {
                Node::Text(text) => self.push_text(text),
                Node::Element(_) => {
                    if let Some(child_ref) = ElementRef::wrap(child) {
                        self.visit_element(child_ref);
                    }
                }
                _ => {}
            }
        }
    }

    fn visit_element(&mut self, element: ElementRef<'_>) {
        let name = element.value().name();
        match name {
            "p" => {
                self.ensure_blank_line();
                self.walk_children(element);
                self.flush_line();
            }
            "br" => self.flush_line(),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => {
                self.ensure_blank_line();
                let level = name[1..].parse::<usize>().unwrap_or(1);
                self.current.push_str(&"#".repeat(level));
                self.current.push(' ');
                self.walk_children(element);
                self.flush_line();
            }
            "a" => {
                self.walk_children(element);
                if let Some(href) = element.value().attr("href") {
                    let text_only: String = element.text().collect();
                    if text_only.trim() != href.trim() {
                        if !self.current.ends_with(' ') {
                            self.current.push(' ');
                        }
                        self.current.push('(');
                        self.current.push_str(href);
                        self.current.push(')');
                    }
                }
            }
            "code" => {
                if self.in_pre {
                    self.walk_children(element);
                } else {
                    if !self.current.is_empty() && !self.current.ends_with(' ') {
                        self.current.push(' ');
                    }
                    self.current.push('`');
                    let text: String = element.text().collect();
                    self.current.push_str(text.trim());
                    self.current.push('`');
                }
            }
            "pre" => {
                self.ensure_blank_line();
                self.lines.push("```".to_string());
                let prev_pre = self.in_pre;
                self.in_pre = true;
                self.walk_children(element);
                self.flush_line();
                self.in_pre = prev_pre;
                while self.lines.last().map(String::is_empty).unwrap_or(false) {
                    self.lines.pop();
                }
                self.lines.push("```".to_string());
                self.lines.push(String::new());
            }
            "blockquote" => {
                self.ensure_blank_line();
                let mut inner = RenderCtx::default();
                inner.walk_children(element);
                for line in inner.finish() {
                    self.lines.push(if line.is_empty() {
                        "│".to_string()
                    } else {
                        format!("│ {line}")
                    });
                }
                self.lines.push(String::new());
            }
            "ul" | "ol" => {
                self.ensure_blank_line();
                self.list_depth += 1;
                let mut index = 1;
                for child in element.children() {
                    if let Some(child_ref) = ElementRef::wrap(child) {
                        if child_ref.value().name() == "li" {
                            let indent = "  ".repeat(self.list_depth - 1);
                            let marker = if name == "ol" {
                                format!("{indent}{index}. ")
                            } else {
                                format!("{indent}• ")
                            };
                            self.current.push_str(&marker);
                            self.walk_children(child_ref);
                            self.flush_line();
                            index += 1;
                        }
                    }
                }
                self.list_depth -= 1;
                self.lines.push(String::new());
            }
            "em" | "i" => {
                self.current.push('*');
                self.walk_children(element);
                self.current.push('*');
            }
            "strong" | "b" => {
                self.current.push_str("**");
                self.walk_children(element);
                self.current.push_str("**");
            }
            _ => self.walk_children(element),
        }
    }
}

fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut prev_space = false;
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !prev_space {
                out.push(' ');
                prev_space = true;
            }
        } else {
            out.push(ch);
            prev_space = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strip_html_returns_plain_text() {
        let html = "<p>Hello <b>world</b></p>";
        assert_eq!(strip_html(html), "Hello world");
    }

    #[test]
    fn render_plain_handles_paragraphs() {
        let html = "<p>First paragraph.</p><p>Second paragraph.</p>";
        let lines = render_plain(html);
        assert!(lines.contains(&"First paragraph.".to_string()));
        assert!(lines.contains(&"Second paragraph.".to_string()));
    }

    #[test]
    fn render_plain_annotates_links_with_href() {
        let html = r#"<p>See <a href="https://rust-lang.org">the site</a>.</p>"#;
        let out = render_plain_string(html);
        assert!(out.contains("the site"));
        assert!(out.contains("(https://rust-lang.org)"));
    }

    #[test]
    fn render_plain_skips_href_when_text_matches_url() {
        let html = r#"<a href="https://rust-lang.org">https://rust-lang.org</a>"#;
        let out = render_plain_string(html);
        assert_eq!(out.matches("https://rust-lang.org").count(), 1);
    }

    #[test]
    fn render_plain_renders_bullet_list() {
        let html = "<ul><li>One</li><li>Two</li></ul>";
        let out = render_plain_string(html);
        assert!(out.contains("• One"));
        assert!(out.contains("• Two"));
    }

    #[test]
    fn render_plain_numbers_ordered_list() {
        let html = "<ol><li>Alpha</li><li>Beta</li></ol>";
        let out = render_plain_string(html);
        assert!(out.contains("1. Alpha"));
        assert!(out.contains("2. Beta"));
    }

    #[test]
    fn render_plain_quotes_blockquote_with_bar() {
        let html = "<blockquote><p>quoted text</p></blockquote>";
        let out = render_plain_string(html);
        assert!(out.contains("│ quoted text"));
    }

    #[test]
    fn render_plain_wraps_code_block_with_fences() {
        let html = "<pre><code>let x = 1;\nlet y = 2;</code></pre>";
        let out = render_plain_string(html);
        let fence_count = out.matches("```").count();
        assert_eq!(fence_count, 2, "expected matching code fences in: {out}");
        assert!(out.contains("let x = 1;"));
        assert!(out.contains("let y = 2;"));
    }

    #[test]
    fn render_plain_inline_code_uses_backticks() {
        let html = "<p>Call <code>fn main()</code> to start.</p>";
        let out = render_plain_string(html);
        assert!(out.contains("`fn main()`"));
    }

    #[test]
    fn render_plain_emphasis_markers() {
        let html = "<p>Keep <em>calm</em> and <strong>carry on</strong>.</p>";
        let out = render_plain_string(html);
        assert!(out.contains("*calm*"));
        assert!(out.contains("**carry on**"));
    }

    #[test]
    fn resolve_permalink_handles_variants() {
        assert_eq!(
            resolve_permalink("/r/rust"),
            "https://www.reddit.com/r/rust"
        );
        assert_eq!(
            resolve_permalink("https://reddit.com/x"),
            "https://reddit.com/x"
        );
        assert_eq!(
            resolve_permalink("r/rust"),
            "https://www.reddit.com/r/rust"
        );
    }
}
