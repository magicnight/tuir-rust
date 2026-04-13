//! HTML and Markdown content rendering
//!
//! Converts Reddit's HTML/Markdown content to terminal-friendly text.

use anyhow::Result;
use scraper::{Html, Selector};

/// Rendered content block
#[derive(Debug, Clone)]
pub struct RenderedBlock {
    pub lines: Vec<String>,
}

/// Convert HTML content to terminal-friendly text
pub fn render_html(html: &str) -> Result<String> {
    let document = Html::parse_document(html);
    let selector =
        Selector::parse("p, a, code, pre, blockquote, ul, ol, li, h1, h2, h3, h4, h5, h6")
            .map_err(|e| anyhow::anyhow!("{e}"))?;

    let mut output = Vec::new();
    for element in document.select(&selector) {
        let tag_name = element.value().name();
        let text: String = element.text().collect();
        match tag_name {
            "a" => {
                let href = element.value().attr("href");
                if let Some(href) = href {
                    output.push(format!("\x1b[36m{text}\x1b[0m ({href})"));
                } else {
                    output.push(text);
                }
            }
            "p" => output.push(format!("\n{text}\n")),
            "code" => output.push(format!("\x1b[33m{text}\x1b[0m")),
            "pre" => output.push(format!("\n```\n{text}\n```\n")),
            "blockquote" => output.push(format!("\n│ {text}\n")),
            "ul" | "ol" => output.push("\n".to_string()),
            "li" => output.push(format!("  • {text}\n")),
            "h1" | "h2" | "h3" | "h4" | "h5" | "h6" => output.push(format!("\n## {text}\n")),
            _ => output.push(text),
        }
    }
    Ok(output.join(""))
}

/// Strip all HTML tags and return plain text
pub fn strip_html(html: &str) -> String {
    let document = Html::parse_document(html);
    let body_selector = Selector::parse("body").ok();
    let text: String = if let Some(sel) = body_selector {
        document
            .select(&sel)
            .map(|e| e.text().collect::<String>())
            .collect()
    } else {
        document.root_element().text().collect()
    };
    text.trim().to_string()
}

/// Resolve Reddit short links to full URLs
pub fn resolve_permalink(short: &str) -> String {
    if short.starts_with('/') {
        format!("https://www.reddit.com{short}")
    } else if short.starts_with("http") {
        short.to_string()
    } else {
        format!("https://www.reddit.com{short}")
    }
}
