//! HTML Block 파싱
//!
//! CommonMark 명세: https://spec.commonmark.org/0.31.2/#html-blocks

use crate::node::{BlockNode, HtmlBlockNode};

/// HTML block의 7가지 타입
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HtmlBlockType {
    /// Type 1: pre, script, style, textarea
    Type1,
    /// Type 2: <!-- comment -->
    Type2,
    /// Type 3: <?processing instruction?>
    Type3,
    /// Type 4: <!DECLARATION>
    Type4,
    /// Type 5: <![CDATA[...]]>
    Type5,
    /// Type 6: Block-level tags (div, table, etc.)
    Type6,
    /// Type 7: Complete open/close tag on a line by itself
    Type7,
}

/// Type 6에 해당하는 block-level tag 목록
const BLOCK_TAGS: &[&str] = &[
    "address", "article", "aside", "base", "basefont", "blockquote", "body",
    "caption", "center", "col", "colgroup", "dd", "details", "dialog",
    "dir", "div", "dl", "dt", "fieldset", "figcaption", "figure",
    "footer", "form", "frame", "frameset",
    "h1", "h2", "h3", "h4", "h5", "h6", "head", "header", "hr",
    "html", "iframe", "legend", "li", "link", "main", "menu", "menuitem",
    "nav", "noframes", "ol", "optgroup", "option", "p", "param",
    "search", "section", "summary", "table", "tbody", "td",
    "tfoot", "th", "thead", "title", "tr", "track", "ul",
];

/// HTML block 시작 조건 감지 (0~3칸 들여쓰기 허용)
/// 반환: Some(HtmlBlockType) if 시작 조건 충족
pub fn detect_start(line: &str) -> Option<HtmlBlockType> {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    if indent > 3 {
        return None;
    }

    if !trimmed.starts_with('<') && !trimmed.starts_with("<!") {
        // Type 3 처리: <? 로 시작
        if !trimmed.starts_with("<?") {
            return None;
        }
    }

    // Type 1: <pre, <script, <style, <textarea (case-insensitive)
    // followed by space, tab, >, or end of line
    for tag in &["pre", "script", "style", "textarea"] {
        if starts_with_tag_type1(trimmed, tag) {
            return Some(HtmlBlockType::Type1);
        }
    }

    // Type 2: <!--
    if trimmed.starts_with("<!--") {
        return Some(HtmlBlockType::Type2);
    }

    // Type 3: <?
    if trimmed.starts_with("<?") {
        return Some(HtmlBlockType::Type3);
    }

    // Type 4: <! followed by ASCII letter
    if trimmed.starts_with("<!") {
        let rest = &trimmed[2..];
        if let Some(ch) = rest.chars().next() {
            if ch.is_ascii_alphabetic() {
                return Some(HtmlBlockType::Type4);
            }
        }
    }

    // Type 5: <![CDATA[
    if trimmed.starts_with("<![CDATA[") {
        return Some(HtmlBlockType::Type5);
    }

    // Type 6: < or </ followed by block-level tag name
    if let Some(block_type) = detect_type6(trimmed) {
        return Some(block_type);
    }

    // Type 7: complete open tag or closing tag on a line by itself
    if let Some(block_type) = detect_type7(trimmed) {
        return Some(block_type);
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

/// Type 6 감지: < or </ followed by block tag name + space/tab/>//>/ end of line
fn detect_type6(trimmed: &str) -> Option<HtmlBlockType> {
    let lower = trimmed.to_ascii_lowercase();

    let (prefix, after_prefix) = if lower.starts_with("</") {
        ("</", &lower[2..])
    } else if lower.starts_with('<') {
        ("<", &lower[1..])
    } else {
        return None;
    };

    let _ = prefix; // used for matching

    for tag in BLOCK_TAGS {
        if after_prefix.starts_with(tag) {
            let rest = &after_prefix[tag.len()..];
            if rest.is_empty()
                || rest.starts_with(' ')
                || rest.starts_with('\t')
                || rest.starts_with('>')
                || rest.starts_with("/>")
            {
                return Some(HtmlBlockType::Type6);
            }
        }
    }

    None
}

/// Type 7 감지: complete open tag (not pre/script/style/textarea) or closing tag,
/// followed by optional spaces/tabs, then end of line
fn detect_type7(trimmed: &str) -> Option<HtmlBlockType> {
    // Try parsing a complete open tag or closing tag
    let s = trimmed;

    if s.starts_with("</") {
        // Closing tag: </tagname> followed by optional whitespace
        if let Some(rest) = parse_closing_tag(s) {
            if rest.trim().is_empty() {
                return Some(HtmlBlockType::Type7);
            }
        }
    } else if s.starts_with('<') {
        // Open tag: <tagname ...> followed by optional whitespace
        if let Some(rest) = parse_open_tag(s) {
            if rest.trim().is_empty() {
                return Some(HtmlBlockType::Type7);
            }
        }
    }

    None
}

/// Parse a closing tag, return remaining string after >
fn parse_closing_tag(s: &str) -> Option<&str> {
    // </tagname>
    if !s.starts_with("</") {
        return None;
    }
    let rest = &s[2..];
    // tag name: ASCII letter followed by ASCII letters, digits, or hyphens
    let tag_end = rest
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .unwrap_or(rest.len());
    if tag_end == 0 {
        return None;
    }
    let after_tag = &rest[tag_end..];
    // optional spaces/tabs then >
    let after_spaces = after_tag.trim_start_matches(|c: char| c == ' ' || c == '\t');
    if after_spaces.starts_with('>') {
        Some(&after_spaces[1..])
    } else {
        None
    }
}

/// Parse an open tag, return remaining string after >
/// Open tag: <tagname (attributes)* /?>
fn parse_open_tag(s: &str) -> Option<&str> {
    if !s.starts_with('<') {
        return None;
    }
    let rest = &s[1..];

    // tag name: ASCII letter followed by ASCII letters, digits, or hyphens
    let mut chars = rest.chars();
    let first = chars.next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }
    let tag_end = 1 + rest[1..]
        .find(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .unwrap_or(rest.len() - 1);

    let tag_name = &rest[..tag_end].to_ascii_lowercase();

    // Type 7 excludes pre, script, style, textarea
    if ["pre", "script", "style", "textarea"].contains(&tag_name.as_str()) {
        return None;
    }

    let mut remaining = &rest[tag_end..];

    // Parse attributes
    loop {
        // Skip spaces/tabs
        let before = remaining;
        remaining = remaining.trim_start_matches(|c: char| c == ' ' || c == '\t');

        if remaining.starts_with("/>") {
            return Some(&remaining[2..]);
        }
        if remaining.starts_with('>') {
            return Some(&remaining[1..]);
        }
        if remaining.is_empty() {
            return None;
        }

        // If we didn't skip any space and we're not at > or />, then it's not valid
        if remaining.len() == before.len() {
            return None;
        }

        // Try to parse an attribute
        // attribute name: ASCII letter, _, or : followed by letters, digits, _, ., :, -
        let attr_start = remaining
            .chars()
            .next()
            .filter(|c| c.is_ascii_alphabetic() || *c == '_' || *c == ':')?;
        let _ = attr_start;
        let attr_name_end = remaining
            .find(|c: char| {
                !c.is_ascii_alphanumeric() && c != '_' && c != '.' && c != ':' && c != '-'
            })
            .unwrap_or(remaining.len());
        remaining = &remaining[attr_name_end..];

        // Optional value specification
        let after_spaces = remaining.trim_start_matches(|c: char| c == ' ' || c == '\t');
        if after_spaces.starts_with('=') {
            remaining = after_spaces[1..].trim_start_matches(|c: char| c == ' ' || c == '\t');
            // attribute value: unquoted, single-quoted, or double-quoted
            if remaining.starts_with('"') {
                let end = remaining[1..].find('"')?;
                remaining = &remaining[end + 2..];
            } else if remaining.starts_with('\'') {
                let end = remaining[1..].find('\'')?;
                remaining = &remaining[end + 2..];
            } else {
                // unquoted: non-empty, no spaces, quotes, =, <, >, `
                let end = remaining
                    .find(|c: char| {
                        c == ' '
                            || c == '\t'
                            || c == '"'
                            || c == '\''
                            || c == '='
                            || c == '<'
                            || c == '>'
                            || c == '`'
                    })
                    .unwrap_or(remaining.len());
                if end == 0 {
                    return None;
                }
                remaining = &remaining[end..];
            }
        }
    }
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
        HtmlBlockType::Type2 => line.contains("-->"),
        HtmlBlockType::Type3 => line.contains("?>"),
        HtmlBlockType::Type4 => line.contains('>'),
        HtmlBlockType::Type5 => line.contains("]]>"),
        // Type 6 and 7 end when followed by a blank line (handled by caller)
        HtmlBlockType::Type6 | HtmlBlockType::Type7 => false,
    }
}

/// HTML block 완성
pub fn finalize(lines: Vec<String>) -> BlockNode {
    let content = lines.join("\n");
    BlockNode::HtmlBlock(HtmlBlockNode::new(&content))
}

/// HTML block이 paragraph를 interrupt할 수 있는지 (type 7은 불가)
pub fn can_interrupt_paragraph(block_type: HtmlBlockType) -> bool {
    block_type != HtmlBlockType::Type7
}

#[cfg(test)]
mod tests {
    use crate::node::{BlockNode, InlineNode};
    use crate::parser::parse;
    use rstest::rstest;

    // =============================================================================
    // HTML Block Examples (148-193)
    // https://spec.commonmark.org/0.31.2/#html-blocks
    // =============================================================================

    #[rstest]
    // Example 148: Type 6 (table) interrupted by blank line, then parsed as markdown
    #[case(
        "<table><tr><td>\n<pre>\n**Hello**,\n\n_world_.\n</pre>\n</td></tr></table>",
        vec![
            BlockNode::html_block("<table><tr><td>\n<pre>\n**Hello**,"),
            BlockNode::paragraph(vec![InlineNode::text("_world_.\n</pre>")]),
            BlockNode::html_block("</td></tr></table>"),
        ]
    )]
    // Example 149: Type 6 (table) basic
    #[case(
        "<table>\n  <tr>\n    <td>\n           hi\n    </td>\n  </tr>\n</table>\n\nokay.",
        vec![
            BlockNode::html_block("<table>\n  <tr>\n    <td>\n           hi\n    </td>\n  </tr>\n</table>"),
            BlockNode::paragraph(vec![InlineNode::text("okay.")]),
        ]
    )]
    // Example 150: Type 6 (div) with leading space
    #[case(
        " <div>\n  *hello*\n         <foo><a>",
        vec![BlockNode::html_block(" <div>\n  *hello*\n         <foo><a>")]
    )]
    // Example 151: Closing tag starts HTML block
    #[case(
        "</div>\n*foo*",
        vec![BlockNode::html_block("</div>\n*foo*")]
    )]
    // Example 152: Type 6 with blank line separation → markdown inside
    #[case(
        "<DIV CLASS=\"foo\">\n\n*Markdown*\n\n</DIV>",
        vec![
            BlockNode::html_block("<DIV CLASS=\"foo\">"),
            BlockNode::paragraph(vec![InlineNode::text("*Markdown*")]),
            BlockNode::html_block("</DIV>"),
        ]
    )]
    // Example 153: Type 6 partial tag split across lines
    #[case(
        "<div id=\"foo\"\n  class=\"bar\">\n</div>",
        vec![BlockNode::html_block("<div id=\"foo\"\n  class=\"bar\">\n</div>")]
    )]
    // Example 154: Type 6 attribute value split across lines
    #[case(
        "<div id=\"foo\" class=\"bar\n  baz\">\n</div>",
        vec![BlockNode::html_block("<div id=\"foo\" class=\"bar\n  baz\">\n</div>")]
    )]
    // Example 155: Type 6 open tag not closed, blank line ends block
    #[case(
        "<div>\n*foo*\n\n*bar*",
        vec![
            BlockNode::html_block("<div>\n*foo*"),
            BlockNode::paragraph(vec![InlineNode::text("*bar*")]),
        ]
    )]
    // Example 156: Type 6 partial tag
    #[case(
        "<div id=\"foo\"\n*hi*",
        vec![BlockNode::html_block("<div id=\"foo\"\n*hi*")]
    )]
    // Example 157: Type 6 partial tag (garbage in)
    #[case(
        "<div class\nfoo",
        vec![BlockNode::html_block("<div class\nfoo")]
    )]
    // Example 158: Type 6 invalid tag content
    #[case(
        "<div *???-&&&-<---\n*foo*",
        vec![BlockNode::html_block("<div *???-&&&-<---\n*foo*")]
    )]
    // Example 159: Type 6 tag with content on same line
    #[case(
        "<div><a href=\"bar\">*foo*</a></div>",
        vec![BlockNode::html_block("<div><a href=\"bar\">*foo*</a></div>")]
    )]
    // Example 160: Type 6 table with content
    #[case(
        "<table><tr><td>\nfoo\n</td></tr></table>",
        vec![BlockNode::html_block("<table><tr><td>\nfoo\n</td></tr></table>")]
    )]
    // Example 161: Type 6 continues past code fence
    #[case(
        "<div></div>\n``` c\nint x = 33;\n```",
        vec![BlockNode::html_block("<div></div>\n``` c\nint x = 33;\n```")]
    )]
    // Example 162: Type 6 continues past blockquote marker
    #[case(
        "<div\n> not quoted text",
        vec![BlockNode::html_block("<div\n> not quoted text")]
    )]
    // Example 163: Type 7 open tag (not block-level)
    #[case(
        "<a href=\"foo\">\n*bar*\n</a>",
        vec![BlockNode::html_block("<a href=\"foo\">\n*bar*\n</a>")]
    )]
    // Example 164: Type 7 custom tag
    #[case(
        "<Warning>\n*bar*\n</Warning>",
        vec![BlockNode::html_block("<Warning>\n*bar*\n</Warning>")]
    )]
    // Example 165: Type 7 inline tag
    #[case(
        "<i class=\"foo\">\n*bar*\n</i>",
        vec![BlockNode::html_block("<i class=\"foo\">\n*bar*\n</i>")]
    )]
    // Example 166: Type 7 closing tag
    #[case(
        "</ins>\n*bar*",
        vec![BlockNode::html_block("</ins>\n*bar*")]
    )]
    // Example 167: Type 7 del tag as HTML block
    #[case(
        "<del>\n*foo*\n</del>",
        vec![BlockNode::html_block("<del>\n*foo*\n</del>")]
    )]
    // Example 168: Type 7 del with blank lines → markdown inside
    #[case(
        "<del>\n\n*foo*\n\n</del>",
        vec![
            BlockNode::html_block("<del>"),
            BlockNode::paragraph(vec![InlineNode::text("*foo*")]),
            BlockNode::html_block("</del>"),
        ]
    )]
    // Example 169: Inline del (not HTML block, paragraph with inline HTML)
    #[case(
        "<del>*foo*</del>",
        vec![BlockNode::paragraph(vec![InlineNode::text("<del>*foo*</del>")])]
    )]
    // Example 170: del with attribute, not complete on one line → paragraph
    #[case(
        "<del\nclass=\"foo\">\n*foo*\n</del>",
        vec![BlockNode::paragraph(vec![InlineNode::text("<del\nclass=\"foo\">\n*foo*\n</del>")])]
    )]
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
    // Example 176: HTML block inside blockquote
    #[case::ignore_176(
        "> <div>\n> foo\n\nbar",
        vec![
            BlockNode::blockquote(vec![BlockNode::html_block("<div>\nfoo")]),
            BlockNode::paragraph(vec![InlineNode::text("bar")]),
        ]
    )]
    // Example 177: HTML block in list
    #[case::ignore_177(
        "- <div>\n- foo",
        vec![
            BlockNode::bullet_list(true, vec![
                crate::node::ListItemNode::new(vec![BlockNode::html_block("<div>")]),
                crate::node::ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("foo")])]),
            ]),
        ]
    )]
    // Example 178: Type 1 (style) end tag on same line
    #[case(
        "<style>p{color:red;}</style>\n*foo*",
        vec![
            BlockNode::html_block("<style>p{color:red;}</style>"),
            BlockNode::paragraph(vec![InlineNode::text("*foo*")]),
        ]
    )]
    // Example 179: Type 2 (comment) end on same line
    #[case(
        "<!-- foo -->*bar*\n*baz*",
        vec![
            BlockNode::html_block("<!-- foo -->*bar*"),
            BlockNode::paragraph(vec![InlineNode::text("*baz*")]),
        ]
    )]
    // Example 180: Type 1 (script) content after end tag stays in block
    #[case(
        "<script>\nfoo\n</script>1. *bar*",
        vec![BlockNode::html_block("<script>\nfoo\n</script>1. *bar*")]
    )]
    // Example 181: Type 2 (comment) with blank lines
    #[case(
        "<!-- Foo\n\nbar\n   baz -->\nokay",
        vec![
            BlockNode::html_block("<!-- Foo\n\nbar\n   baz -->"),
            BlockNode::paragraph(vec![InlineNode::text("okay")]),
        ]
    )]
    // Example 182: Type 3 (processing instruction) with blank lines
    #[case(
        "<?php\n\n  echo '>';\n\n?>\nokay",
        vec![
            BlockNode::html_block("<?php\n\n  echo '>';\n\n?>"),
            BlockNode::paragraph(vec![InlineNode::text("okay")]),
        ]
    )]
    // Example 183: Type 4 (declaration)
    #[case(
        "<!DOCTYPE html>",
        vec![BlockNode::html_block("<!DOCTYPE html>")]
    )]
    // Example 184: Type 5 (CDATA) with blank lines
    #[case(
        "<![CDATA[\nfunction matchwo(a,b)\n{\n  if (a < b && a < 0) then {\n    return 1;\n\n  } else {\n\n    return 0;\n  }\n}\n]]>\nokay",
        vec![
            BlockNode::html_block("<![CDATA[\nfunction matchwo(a,b)\n{\n  if (a < b && a < 0) then {\n    return 1;\n\n  } else {\n\n    return 0;\n  }\n}\n]]>"),
            BlockNode::paragraph(vec![InlineNode::text("okay")]),
        ]
    )]
    // Example 185: Indentation: 2 spaces OK, 4 spaces → code block
    #[case(
        "  <!-- foo -->\n\n    <!-- foo -->",
        vec![
            BlockNode::html_block("  <!-- foo -->"),
            BlockNode::code_block(None, "<!-- foo -->"),
        ]
    )]
    // Example 186: Indentation: 2 spaces OK, 4 spaces → code block (div)
    #[case(
        "  <div>\n\n    <div>",
        vec![
            BlockNode::html_block("  <div>"),
            BlockNode::code_block(None, "<div>"),
        ]
    )]
    // Example 187: Type 6 can interrupt paragraph
    #[case(
        "Foo\n<div>\nbar\n</div>",
        vec![
            BlockNode::paragraph(vec![InlineNode::text("Foo")]),
            BlockNode::html_block("<div>\nbar\n</div>"),
        ]
    )]
    // Example 188: Type 6 followed by content without blank line
    #[case(
        "<div>\nbar\n</div>\n*foo*",
        vec![BlockNode::html_block("<div>\nbar\n</div>\n*foo*")]
    )]
    // Example 189: Type 7 cannot interrupt paragraph
    #[case(
        "Foo\n<a href=\"bar\">\nbaz",
        vec![BlockNode::paragraph(vec![InlineNode::text("Foo\n<a href=\"bar\">\nbaz")])]
    )]
    // Example 190: Type 6 with blank line → markdown inside
    #[case(
        "<div>\n\n*Emphasized* text.\n\n</div>",
        vec![
            BlockNode::html_block("<div>"),
            BlockNode::paragraph(vec![InlineNode::text("*Emphasized* text.")]),
            BlockNode::html_block("</div>"),
        ]
    )]
    // Example 191: Type 6 without blank line → raw HTML
    #[case(
        "<div>\n*Emphasized* text.\n</div>",
        vec![BlockNode::html_block("<div>\n*Emphasized* text.\n</div>")]
    )]
    // Example 192: Table with blank lines → multiple HTML blocks
    #[case(
        "<table>\n\n<tr>\n\n<td>\nHi\n</td>\n\n</tr>\n\n</table>",
        vec![
            BlockNode::html_block("<table>"),
            BlockNode::html_block("<tr>"),
            BlockNode::html_block("<td>\nHi\n</td>"),
            BlockNode::html_block("</tr>"),
            BlockNode::html_block("</table>"),
        ]
    )]
    // Example 193: Table with indented inner tags → code block inside
    #[case::ignore_193(
        "<table>\n\n  <tr>\n\n    <td>\n      Hi\n    </td>\n\n  </tr>\n\n</table>",
        vec![
            BlockNode::html_block("<table>"),
            BlockNode::html_block("  <tr>"),
            BlockNode::code_block(None, "<td>\n  Hi\n</td>"),
            BlockNode::html_block("  </tr>"),
            BlockNode::html_block("</table>"),
        ]
    )]
    fn test_html_block(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse(input);
        assert_eq!(doc.children, expected);
    }
}
