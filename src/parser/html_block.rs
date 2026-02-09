//! HTML Block 파싱
//!
//! CommonMark 명세: https://spec.commonmark.org/0.31.2/#html-blocks

use crate::node::{BlockNode, HtmlBlockNode};

/// HTML block의 타입
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HtmlBlockType {
    /// Type 1: pre, script, style, textarea
    Type1,
}

/// HTML block 시작 조건 감지 (0~3칸 들여쓰기 허용)
/// 반환: Some(HtmlBlockType) if 시작 조건 충족
pub fn detect_start(line: &str) -> Option<HtmlBlockType> {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    if indent > 3 {
        return None;
    }

    // Type 1: <pre, <script, <style, <textarea (case-insensitive)
    // followed by space, tab, >, or end of line
    for tag in &["pre", "script", "style", "textarea"] {
        if starts_with_tag_type1(trimmed, tag) {
            return Some(HtmlBlockType::Type1);
        }
    }

    None
}

/// Type 1 시작 조건: <tagname 뒤에 space, tab, >, or end of line
fn starts_with_tag_type1(trimmed: &str, tag: &str) -> bool {
    let lower = trimmed.to_ascii_lowercase();
    if !lower.starts_with(&format!("<{}", tag)) {
        return false;
    }
    let rest = &trimmed[1 + tag.len()..];
    rest.is_empty()
        || rest.starts_with(' ')
        || rest.starts_with('\t')
        || rest.starts_with('>')
}

/// 종료 조건 체크
pub fn check_end(line: &str, block_type: HtmlBlockType) -> bool {
    let lower = line.to_ascii_lowercase();
    match block_type {
        HtmlBlockType::Type1 => {
            lower.contains("</pre>")
                || lower.contains("</script>")
                || lower.contains("</style>")
                || lower.contains("</textarea>")
        }
    }
}

/// HTML block 완성
pub fn finalize(lines: Vec<String>) -> BlockNode {
    let content = lines.join("\n");
    BlockNode::HtmlBlock(HtmlBlockNode::new(&content))
}

/// HTML block이 paragraph를 interrupt할 수 있는지
pub fn can_interrupt_paragraph(block_type: HtmlBlockType) -> bool {
    match block_type {
        HtmlBlockType::Type1 => true,
    }
}

#[cfg(test)]
mod tests {
    use crate::node::{BlockNode, InlineNode};
    use crate::parser::parse;
    use rstest::rstest;

    // =========================================================================
    // HTML Block Type 1 — Examples 171-180
    // https://spec.commonmark.org/0.31.2/#html-blocks
    // =========================================================================

    #[rstest]
    // Example 171: Type 1 (pre) with blank lines inside
    #[case(
        "<pre language=\"haskell\"><code>\nimport Text.HTML.TagSoup\n\nmain :: IO ()\nmain = print $ parseTags tags\n</code></pre>\nokay",
        vec![
            BlockNode::html_block("<pre language=\"haskell\"><code>\nimport Text.HTML.TagSoup\n\nmain :: IO ()\nmain = print $ parseTags tags\n</code></pre>"),
            BlockNode::paragraph(vec![InlineNode::text("okay")]),
        ]
    )]
    // Example 172: Type 1 (script) with blank lines inside
    #[case(
        "<script type=\"text/javascript\">\n// JavaScript example\n\ndocument.getElementById(\"demo\").innerHTML = \"Hello JavaScript!\";\n</script>\nokay",
        vec![
            BlockNode::html_block("<script type=\"text/javascript\">\n// JavaScript example\n\ndocument.getElementById(\"demo\").innerHTML = \"Hello JavaScript!\";\n</script>"),
            BlockNode::paragraph(vec![InlineNode::text("okay")]),
        ]
    )]
    // Example 173: Type 1 (textarea) with blank lines
    #[case(
        "<textarea>\n\n*foo*\n\n_bar_\n\n</textarea>",
        vec![BlockNode::html_block("<textarea>\n\n*foo*\n\n_bar_\n\n</textarea>")]
    )]
    // Example 174: Type 1 (style) with blank lines
    #[case(
        "<style\n  type=\"text/css\">\nh1 {color:red;}\n\np {color:blue;}\n</style>\nokay",
        vec![
            BlockNode::html_block("<style\n  type=\"text/css\">\nh1 {color:red;}\n\np {color:blue;}\n</style>"),
            BlockNode::paragraph(vec![InlineNode::text("okay")]),
        ]
    )]
    // Example 175: Type 1 (style) no end tag → ends at document end
    #[case(
        "<style\n  type=\"text/css\">\n\nfoo",
        vec![BlockNode::html_block("<style\n  type=\"text/css\">\n\nfoo")]
    )]
    // Example 178: Type 1 (style) end tag on same line
    #[case(
        "<style>p{color:red;}</style>\n*foo*",
        vec![
            BlockNode::html_block("<style>p{color:red;}</style>"),
            BlockNode::paragraph(vec![InlineNode::text("*foo*")]),
        ]
    )]
    // Example 179: Type 2 (comment) — skip, not Type 1
    // Example 180: Type 1 (script) content after end tag stays in block
    #[case(
        "<script>\nfoo\n</script>1. *bar*",
        vec![BlockNode::html_block("<script>\nfoo\n</script>1. *bar*")]
    )]
    fn test_html_block_type1(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse(input);
        assert_eq!(doc.children, expected);
    }
}
