//! HTML 렌더러
//!
//! AST를 CommonMark 명세에 맞는 HTML로 변환합니다.

use crate::node::*;

/// DocumentNode를 HTML 문자열로 렌더링
pub fn render(doc: &DocumentNode) -> String {
    let mut out = String::new();
    render_blocks(&doc.children, &mut out);
    out
}

fn render_blocks(blocks: &[BlockNode], out: &mut String) {
    for block in blocks {
        render_block(block, out);
    }
}

fn render_block(block: &BlockNode, out: &mut String) {
    match block {
        BlockNode::Paragraph(p) => {
            out.push_str("<p>");
            render_inlines(&p.children, out);
            out.push_str("</p>\n");
        }
        BlockNode::Heading(h) => {
            out.push_str(&format!("<h{}>", h.level));
            render_inlines(&h.children, out);
            out.push_str(&format!("</h{}>\n", h.level));
        }
        BlockNode::CodeBlock(cb) => {
            match &cb.info {
                Some(info) if !info.is_empty() => {
                    let lang = info.split_whitespace().next().unwrap_or("");
                    out.push_str(&format!(
                        "<pre><code class=\"language-{}\">",
                        escape_html(lang)
                    ));
                }
                _ => {
                    out.push_str("<pre><code>");
                }
            }
            let content = escape_html(&cb.content);
            out.push_str(&content);
            if !content.is_empty() && !content.ends_with('\n') {
                out.push('\n');
            }
            out.push_str("</code></pre>\n");
        }
        BlockNode::ThematicBreak(_) => {
            out.push_str("<hr />\n");
        }
        BlockNode::Blockquote(bq) => {
            out.push_str("<blockquote>\n");
            render_blocks(&bq.children, out);
            out.push_str("</blockquote>\n");
        }
        BlockNode::List(list) => {
            let (tag, is_ordered) = match &list.list_type {
                ListType::Bullet => ("ul", false),
                ListType::Ordered { .. } => ("ol", true),
            };
            if is_ordered && list.start != 1 {
                out.push_str(&format!("<{} start=\"{}\">\n", tag, list.start));
            } else {
                out.push_str(&format!("<{}>\n", tag));
            }
            for item in &list.children {
                render_list_item(item, list.tight, out);
            }
            out.push_str(&format!("</{}>\n", tag));
        }
        BlockNode::HtmlBlock(html) => {
            out.push_str(&html.content);
            out.push('\n');
        }
        BlockNode::ListItem(_) => {
            // ListItem은 List 내부에서만 사용되므로 여기에 도달하지 않음
            unreachable!("ListItem should only appear inside List")
        }
    }
}

fn render_list_item(item: &ListItemNode, tight: bool, out: &mut String) {
    out.push_str("<li>");
    if tight {
        // tight list: paragraph 태그 없이 인라인, non-paragraph는 블록 렌더링
        for block in &item.children {
            match block {
                BlockNode::Paragraph(p) => {
                    render_inlines(&p.children, out);
                }
                other => {
                    if !out.ends_with('\n') {
                        out.push('\n');
                    }
                    render_block(other, out);
                }
            }
        }
    } else {
        out.push('\n');
        render_blocks(&item.children, out);
    }
    out.push_str("</li>\n");
}

fn render_inlines(inlines: &[InlineNode], out: &mut String) {
    for inline in inlines {
        render_inline(inline, out);
    }
}

fn render_inline(inline: &InlineNode, out: &mut String) {
    match inline {
        InlineNode::Text(t) => {
            out.push_str(&escape_html(&t.0));
        }
        InlineNode::CodeSpan(c) => {
            out.push_str("<code>");
            out.push_str(&escape_html(&c.0));
            out.push_str("</code>");
        }
        InlineNode::Emphasis(e) => {
            out.push_str("<em>");
            render_inlines(&e.children, out);
            out.push_str("</em>");
        }
        InlineNode::Strong(s) => {
            out.push_str("<strong>");
            render_inlines(&s.children, out);
            out.push_str("</strong>");
        }
        InlineNode::Link(l) => {
            out.push_str(&format!("<a href=\"{}\"", escape_href(&l.destination)));
            if let Some(ref title) = l.title {
                out.push_str(&format!(" title=\"{}\"", escape_html(title)));
            }
            out.push('>');
            render_inlines(&l.children, out);
            out.push_str("</a>");
        }
        InlineNode::Image(img) => {
            out.push_str(&format!(
                "<img src=\"{}\" alt=\"{}\"",
                escape_href(&img.destination),
                get_alt_text(&img.children),
            ));
            if let Some(ref title) = img.title {
                out.push_str(&format!(" title=\"{}\"", escape_html(title)));
            }
            out.push_str(" />");
        }
        InlineNode::Autolink(a) => {
            out.push_str(&format!(
                "<a href=\"{}\">{}</a>",
                escape_href(&a.destination),
                escape_html(&a.label),
            ));
        }
        InlineNode::RawHtml(h) => {
            out.push_str(&h.content);
        }
        InlineNode::HardBreak => {
            out.push_str("<br />\n");
        }
        InlineNode::SoftBreak => {
            out.push('\n');
        }
    }
}

/// image의 alt text 추출 (children에서 텍스트만 재귀적으로)
fn get_alt_text(children: &[InlineNode]) -> String {
    let mut text = String::new();
    for child in children {
        match child {
            InlineNode::Text(t) => text.push_str(&escape_html(&t.0)),
            InlineNode::CodeSpan(c) => text.push_str(&escape_html(&c.0)),
            InlineNode::Emphasis(e) => text.push_str(&get_alt_text(&e.children)),
            InlineNode::Strong(s) => text.push_str(&get_alt_text(&s.children)),
            InlineNode::Link(l) => text.push_str(&get_alt_text(&l.children)),
            InlineNode::Image(img) => text.push_str(&get_alt_text(&img.children)),
            InlineNode::SoftBreak | InlineNode::HardBreak => text.push('\n'),
            _ => {}
        }
    }
    text
}

/// HTML 특수 문자 이스케이프
fn escape_html(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// href/src 속성용 이스케이프 + percent-encoding
///
/// 이미 percent-encoded된 시퀀스(%XX)는 유지하고,
/// URL-safe 문자 외의 바이트는 percent-encode 함.
fn escape_href(s: &str) -> String {
    // URL-safe: unreserved (RFC 3986) + 일반적으로 허용되는 URL 문자
    const URL_SAFE: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-._~:/?#@!$&'()*+,;=";

    let bytes = s.as_bytes();
    let mut out = String::with_capacity(s.len());
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        if b == b'%' && i + 2 < bytes.len()
            && bytes[i + 1].is_ascii_hexdigit()
            && bytes[i + 2].is_ascii_hexdigit()
        {
            // 이미 percent-encoded된 시퀀스 유지
            out.push('%');
            out.push(bytes[i + 1] as char);
            out.push(bytes[i + 2] as char);
            i += 3;
        } else if b == b'&' {
            out.push_str("&amp;");
            i += 1;
        } else if URL_SAFE.contains(&b) {
            out.push(b as char);
            i += 1;
        } else {
            // percent-encode
            out.push_str(&format!("%{:02X}", b));
            i += 1;
        }
    }
    out
}
