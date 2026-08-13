//! Safe Markdown → HTML rendering for transcript message bodies.
//!
//! Uses pulldown-cmark in safe mode: raw HTML blocks and inline HTML in the
//! source text are dropped so the output contains only well-formed elements
//! produced by the parser. This prevents XSS from untrusted transcript content
//! while preserving headings, lists, code blocks, inline code, links, and
//! emphasis.

use pulldown_cmark::{Event, Options, Parser, html};

/// Renders `input` as sanitized HTML.
///
/// Raw `<script>`, `<style>`, and arbitrary inline/block HTML in the source
/// text are stripped. Code fences and backtick spans are preserved as
/// `<pre><code>` and `<code>` respectively.
pub fn render_markdown(input: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;

    // Walk the event stream, dropping any raw HTML events.
    // pulldown-cmark emits Event::Html for block HTML and Event::InlineHtml
    // for inline HTML; both are stripped here.
    let parser = Parser::new_ext(input, options).filter(|event| {
        !matches!(event, Event::Html(_) | Event::InlineHtml(_))
    });

    let mut output = String::with_capacity(input.len().saturating_add(64));
    html::push_html(&mut output, parser);
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_headings_and_paragraphs() {
        let html = render_markdown("# Hello\n\nWorld");
        assert!(html.contains("<h1>"), "expected h1, got: {html}");
        assert!(html.contains("Hello"), "expected Hello, got: {html}");
        assert!(html.contains("<p>"), "expected p, got: {html}");
        assert!(html.contains("World"), "expected World, got: {html}");
    }

    #[test]
    fn renders_code_fence_as_pre_code() {
        let html = render_markdown("```rust\nfn main() {}\n```");
        assert!(html.contains("<pre>"), "expected pre, got: {html}");
        assert!(html.contains("<code"), "expected code, got: {html}");
        assert!(html.contains("fn main()"), "expected code body, got: {html}");
    }

    #[test]
    fn renders_inline_code() {
        let html = render_markdown("Use `cargo check`.");
        assert!(html.contains("<code>"), "expected inline code, got: {html}");
        assert!(html.contains("cargo check"), "expected code body, got: {html}");
    }

    #[test]
    fn strips_raw_html_blocks() {
        let html = render_markdown("<script>alert(1)</script>\n\nSafe text");
        assert!(!html.contains("<script>"), "script must be stripped, got: {html}");
        assert!(html.contains("Safe text"), "expected safe text, got: {html}");
    }

    #[test]
    fn strips_inline_html() {
        let html = render_markdown("Click <a onclick=\"alert(1)\">here</a>.");
        assert!(!html.contains("onclick"), "onclick must be stripped, got: {html}");
    }

    #[test]
    fn renders_unordered_list() {
        let html = render_markdown("- alpha\n- beta\n- gamma");
        assert!(html.contains("<ul>"), "expected ul, got: {html}");
        assert!(html.contains("<li>"), "expected li, got: {html}");
    }

    #[test]
    fn renders_emphasis_and_strong() {
        let html = render_markdown("_italic_ and **bold**");
        assert!(html.contains("<em>"), "expected em, got: {html}");
        assert!(html.contains("<strong>"), "expected strong, got: {html}");
    }
}
