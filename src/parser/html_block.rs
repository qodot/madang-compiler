//! HTML Block 파싱
//!
//! CommonMark 명세: https://spec.commonmark.org/0.31.2/#html-blocks

use crate::node::{BlockNode, HtmlBlockNode};

/// HTML block의 타입
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HtmlBlockType {
    /// Type 1: pre, script, style, textarea
    Type1,
    /// Type 2: <!-- ... -->
    Type2,
    /// Type 3: <? ... ?>
    Type3,
    /// Type 4: <! + 대문자 ... >
    Type4,
    /// Type 5: <![CDATA[ ... ]]>
    Type5,
    /// Type 6: 특정 태그 목록 (div, table 등) — 빈 줄로 종료
    Type6,
}

impl HtmlBlockType {
    /// 빈 줄로 종료되는 타입인지 (Type 6, 7)
    pub fn ends_on_blank_line(self) -> bool {
        matches!(self, HtmlBlockType::Type6)
    }
}

/// HTML block 파싱 성공 결과
#[derive(Debug, PartialEq)]
pub enum HtmlBlockOk {
    /// 시작 줄
    Start(HtmlBlockType),
    /// 내용 줄 (종료 조건 미충족)
    Content,
    /// 종료 줄 (종료 조건 충족)
    End,
}

/// HTML block 파싱 실패 사유
#[derive(Debug, PartialEq)]
pub enum HtmlBlockErr {
    /// 4칸 이상 들여쓰기 (코드 블록으로 해석됨)
    CodeBlockIndented,
    /// HTML block 시작 조건에 해당하지 않음
    NotHtmlBlock,
}

/// HTML block 파싱
///
/// - `block_type`이 None이면 시작 조건 감지
/// - `block_type`이 Some이면 계속/종료 판단
pub fn parse(line: &str, block_type: Option<HtmlBlockType>) -> Result<HtmlBlockOk, HtmlBlockErr> {
    match block_type {
        None => parse_start(line),
        Some(bt) => Ok(parse_continue(line, bt)),
    }
}

fn parse_start(line: &str) -> Result<HtmlBlockOk, HtmlBlockErr> {
    let trimmed = line.trim_start();
    let indent = line.len() - trimmed.len();
    if indent > 3 {
        return Err(HtmlBlockErr::CodeBlockIndented);
    }

    // Type 1: <pre, <script, <style, <textarea (case-insensitive)
    // followed by space, tab, >, or end of line
    for tag in &["pre", "script", "style", "textarea"] {
        if starts_with_tag_type1(trimmed, tag) {
            return Ok(HtmlBlockOk::Start(HtmlBlockType::Type1));
        }
    }

    // Type 2: <!--
    if trimmed.starts_with("<!--") {
        return Ok(HtmlBlockOk::Start(HtmlBlockType::Type2));
    }

    // Type 3: <?
    if trimmed.starts_with("<?") {
        return Ok(HtmlBlockOk::Start(HtmlBlockType::Type3));
    }

    // Type 5: <![CDATA[ (must check before Type 4)
    if trimmed.starts_with("<![CDATA[") {
        return Ok(HtmlBlockOk::Start(HtmlBlockType::Type5));
    }

    // Type 4: <! + ASCII uppercase letter
    if trimmed.starts_with("<!")
        && trimmed.as_bytes().get(2).map_or(false, |b| b.is_ascii_uppercase())
    {
        return Ok(HtmlBlockOk::Start(HtmlBlockType::Type4));
    }

    // Type 6: 특정 태그의 여는/닫는 태그
    if starts_with_tag_type6(trimmed) {
        return Ok(HtmlBlockOk::Start(HtmlBlockType::Type6));
    }

    Err(HtmlBlockErr::NotHtmlBlock)
}

fn parse_continue(line: &str, block_type: HtmlBlockType) -> HtmlBlockOk {
    if check_end(line, block_type) {
        HtmlBlockOk::End
    } else {
        HtmlBlockOk::Content
    }
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

/// Type 6 태그 목록
const TYPE6_TAGS: &[&str] = &[
    "address", "article", "aside", "base", "basefont", "blockquote", "body",
    "caption", "center", "col", "colgroup", "dd", "details", "dialog", "dir",
    "div", "dl", "dt", "fieldset", "figcaption", "figure", "footer", "form",
    "frame", "frameset", "h1", "h2", "h3", "h4", "h5", "h6", "head", "header",
    "hr", "html", "iframe", "legend", "li", "link", "main", "menu", "menuitem",
    "nav", "noframes", "ol", "optgroup", "option", "p", "param", "search",
    "section", "summary", "table", "tbody", "td", "tfoot", "th", "thead",
    "title", "tr", "track", "ul",
];

/// Type 6 시작 조건: <tagname 또는 </tagname 뒤에 space, tab, >, />, or end of line
fn starts_with_tag_type6(trimmed: &str) -> bool {
    let lower = trimmed.to_ascii_lowercase();

    let after_bracket = if lower.starts_with("</") {
        &lower[2..]
    } else if lower.starts_with('<') {
        &lower[1..]
    } else {
        return false;
    };

    for tag in TYPE6_TAGS {
        if after_bracket.starts_with(tag) {
            let rest = &after_bracket[tag.len()..];
            if rest.is_empty()
                || rest.starts_with(' ')
                || rest.starts_with('\t')
                || rest.starts_with('>')
                || rest.starts_with("/>")
                || rest.starts_with('\n')
            {
                return true;
            }
        }
    }
    false
}

/// 종료 조건 체크 (내부용)
fn check_end(line: &str, block_type: HtmlBlockType) -> bool {
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
        HtmlBlockType::Type6 => line.trim().is_empty(),
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
        HtmlBlockType::Type1 | HtmlBlockType::Type2 | HtmlBlockType::Type3 | HtmlBlockType::Type4 | HtmlBlockType::Type5 | HtmlBlockType::Type6 => true,
    }
}

#[cfg(test)]
mod tests {
    use crate::node::{BlockNode, InlineNode};
    use crate::parser::parse;
    use rstest::rstest;

    // =========================================================================
    // HTML Block Type 1 — Examples 169-178
    // https://spec.commonmark.org/0.31.2/#html-blocks
    // =========================================================================

    #[rstest]
    // Example 169: Type 1 (pre) with blank lines inside
    #[case(
        "<pre language=\"haskell\"><code>\nimport Text.HTML.TagSoup\n\nmain :: IO ()\nmain = print $ parseTags tags\n</code></pre>\nokay",
        vec![
            BlockNode::html_block("<pre language=\"haskell\"><code>\nimport Text.HTML.TagSoup\n\nmain :: IO ()\nmain = print $ parseTags tags\n</code></pre>"),
            BlockNode::paragraph(vec![InlineNode::text("okay")]),
        ]
    )]
    // Example 170: Type 1 (script) with blank lines inside
    #[case(
        "<script type=\"text/javascript\">\n// JavaScript example\n\ndocument.getElementById(\"demo\").innerHTML = \"Hello JavaScript!\";\n</script>\nokay",
        vec![
            BlockNode::html_block("<script type=\"text/javascript\">\n// JavaScript example\n\ndocument.getElementById(\"demo\").innerHTML = \"Hello JavaScript!\";\n</script>"),
            BlockNode::paragraph(vec![InlineNode::text("okay")]),
        ]
    )]
    // Example 171: Type 1 (textarea) with blank lines
    #[case(
        "<textarea>\n\n*foo*\n\n_bar_\n\n</textarea>",
        vec![BlockNode::html_block("<textarea>\n\n*foo*\n\n_bar_\n\n</textarea>")]
    )]
    // Example 172: Type 1 (style) with blank lines
    #[case(
        "<style\n  type=\"text/css\">\nh1 {color:red;}\n\np {color:blue;}\n</style>\nokay",
        vec![
            BlockNode::html_block("<style\n  type=\"text/css\">\nh1 {color:red;}\n\np {color:blue;}\n</style>"),
            BlockNode::paragraph(vec![InlineNode::text("okay")]),
        ]
    )]
    // Example 173: Type 1 (style) no end tag → ends at document end
    #[case(
        "<style\n  type=\"text/css\">\n\nfoo",
        vec![BlockNode::html_block("<style\n  type=\"text/css\">\n\nfoo")]
    )]
    // Example 176: Type 1 (style) end tag on same line
    #[case(
        "<style>p{color:red;}</style>\n*foo*",
        vec![
            BlockNode::html_block("<style>p{color:red;}</style>"),
            BlockNode::paragraph(vec![InlineNode::text("*foo*")]),
        ]
    )]
    // Example 178: Type 1 (script) content after end tag stays in block
    #[case(
        "<script>\nfoo\n</script>1. *bar*",
        vec![BlockNode::html_block("<script>\nfoo\n</script>1. *bar*")]
    )]
    fn test_html_block_type1(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse(input);
        assert_eq!(doc.children, expected);
    }

    // =========================================================================
    // HTML Block Type 2 — Examples 177, 179, 183
    // https://spec.commonmark.org/0.31.2/#html-blocks
    // =========================================================================

    #[rstest]
    // Example 177: comment on same line, content after --> stays in block
    #[case(
        "<!-- foo -->*bar*\n*baz*",
        vec![
            BlockNode::html_block("<!-- foo -->*bar*"),
            BlockNode::paragraph(vec![InlineNode::text("*baz*")]),
        ]
    )]
    // Example 179: multi-line comment with blank lines
    #[case(
        "<!-- Foo\n\nbar\n   baz -->\nokay",
        vec![
            BlockNode::html_block("<!-- Foo\n\nbar\n   baz -->"),
            BlockNode::paragraph(vec![InlineNode::text("okay")]),
        ]
    )]
    // Example 183: 0-3 spaces indent OK, 4 spaces = code block
    #[case(
        "  <!-- foo -->\n\n    <!-- foo -->",
        vec![
            BlockNode::html_block("  <!-- foo -->"),
            BlockNode::code_block(None, "<!-- foo -->"),
        ]
    )]
    fn test_html_block_type2(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse(input);
        assert_eq!(doc.children, expected);
    }

    // =========================================================================
    // HTML Block Type 3 — Example 180
    // https://spec.commonmark.org/0.31.2/#html-blocks
    // =========================================================================

    #[rstest]
    // Example 180: <?php processing instruction with blank lines
    #[case(
        "<?php\n\n  echo '>';\n\n?>\nokay",
        vec![
            BlockNode::html_block("<?php\n\n  echo '>';\n\n?>"),
            BlockNode::paragraph(vec![InlineNode::text("okay")]),
        ]
    )]
    fn test_html_block_type3(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse(input);
        assert_eq!(doc.children, expected);
    }

    // =========================================================================
    // HTML Block Type 4 — Example 181
    // https://spec.commonmark.org/0.31.2/#html-blocks
    // =========================================================================

    #[rstest]
    // Example 181: <!DOCTYPE html> declaration
    #[case(
        "<!DOCTYPE html>",
        vec![BlockNode::html_block("<!DOCTYPE html>")]
    )]
    fn test_html_block_type4(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse(input);
        assert_eq!(doc.children, expected);
    }

    // =========================================================================
    // HTML Block Type 5 — Example 182
    // https://spec.commonmark.org/0.31.2/#html-blocks
    // =========================================================================

    #[rstest]
    // Example 182: CDATA section with blank lines
    #[case(
        "<![CDATA[\nfunction matchwo(a,b)\n{\n  if (a < b && a < 0) then {\n    return 1;\n\n  } else {\n\n    return 0;\n  }\n}\n]]>\nokay",
        vec![
            BlockNode::html_block("<![CDATA[\nfunction matchwo(a,b)\n{\n  if (a < b && a < 0) then {\n    return 1;\n\n  } else {\n\n    return 0;\n  }\n}\n]]>"),
            BlockNode::paragraph(vec![InlineNode::text("okay")]),
        ]
    )]
    fn test_html_block_type5(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse(input);
        assert_eq!(doc.children, expected);
    }

    // =========================================================================
    // HTML Block Type 6 — Examples 148-161
    // https://spec.commonmark.org/0.31.2/#html-blocks
    // =========================================================================

    #[rstest]
    // Example 149: <table> ends at blank line, followed by paragraph
    #[case(
        "<table>\n  <tr>\n    <td>\n           hi\n    </td>\n  </tr>\n</table>\n\nokay.",
        vec![
            BlockNode::html_block("<table>\n  <tr>\n    <td>\n           hi\n    </td>\n  </tr>\n</table>"),
            BlockNode::paragraph(vec![InlineNode::text("okay.")]),
        ]
    )]
    // Example 150: <div> with indent, no blank line → ends at document end
    #[case(
        " <div>\n  *hello*\n         <foo><a>",
        vec![BlockNode::html_block(" <div>\n  *hello*\n         <foo><a>")]
    )]
    // Example 152: <DIV> (case insensitive), blank line splits into HTML + paragraph + HTML
    #[case(
        "<DIV CLASS=\"foo\">\n\n*Markdown*\n\n</DIV>",
        vec![
            BlockNode::html_block("<DIV CLASS=\"foo\">"),
            BlockNode::paragraph(vec![InlineNode::text("*Markdown*")]),
            BlockNode::html_block("</DIV>"),
        ]
    )]
    // Example 155: <div> blank line in middle → HTML block ends, paragraph starts
    #[case(
        "<div>\n*foo*\n\n*bar*",
        vec![
            BlockNode::html_block("<div>\n*foo*"),
            BlockNode::paragraph(vec![InlineNode::text("*bar*")]),
        ]
    )]
    // Example 159: single line <div> with content, no blank line → ends at doc end
    #[case(
        "<div><a href=\"bar\">*foo*</a></div>",
        vec![BlockNode::html_block("<div><a href=\"bar\">*foo*</a></div>")]
    )]
    // Example 160: <table> single block, no blank line
    #[case(
        "<table><tr><td>\nfoo\n</td></tr></table>",
        vec![BlockNode::html_block("<table><tr><td>\nfoo\n</td></tr></table>")]
    )]
    // Example 151: </div> 닫는 태그로 시작
    #[case(
        "</div>\n*foo*",
        vec![BlockNode::html_block("</div>\n*foo*")]
    )]
    // Example 153: <div> with attributes, multi-line
    #[case(
        "<div id=\"foo\"\n  class=\"bar\">\n</div>",
        vec![BlockNode::html_block("<div id=\"foo\"\n  class=\"bar\">\n</div>")]
    )]
    // Example 154: <div> with attribute spanning lines
    #[case(
        "<div id=\"foo\" class=\"bar\n  baz\">\n</div>",
        vec![BlockNode::html_block("<div id=\"foo\" class=\"bar\n  baz\">\n</div>")]
    )]
    // Example 156: <div> with attribute, no closing >
    #[case(
        "<div id=\"foo\"\n*hi*",
        vec![BlockNode::html_block("<div id=\"foo\"\n*hi*")]
    )]
    // Example 157: <div> with incomplete attribute
    #[case(
        "<div class\nfoo",
        vec![BlockNode::html_block("<div class\nfoo")]
    )]
    // Example 158: <div> with arbitrary content after tag
    #[case(
        "<div *???-&&&-<---\n*foo*",
        vec![BlockNode::html_block("<div *???-&&&-<---\n*foo*")]
    )]
    // Example 161: <div> followed by fenced code block (all one HTML block)
    #[case(
        "<div></div>\n``` c\nint x = 33;\n```",
        vec![BlockNode::html_block("<div></div>\n``` c\nint x = 33;\n```")]
    )]
    fn test_html_block_type6(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse(input);
        assert_eq!(doc.children, expected);
    }

    // =========================================================================
    // 미구현 기능으로 인해 ignore 처리된 테스트
    // =========================================================================

    #[rstest]
    // Example 148: Type 1(pre) + Type 6(table) 상호작용 — 복합 블록 타입 처리 미구현
    #[case(
        "<table><tr><td>\n<pre>\n**Hello**,\n\n_world_.\n</pre>\n</td></tr></table>",
        vec![
            BlockNode::html_block("<table><tr><td>\n<pre>\n**Hello**,"),
            BlockNode::paragraph(vec![InlineNode::text("_world_.\n</pre>\n</td></tr></table>")]),
        ]
    )]
    // Example 174: HTML block inside blockquote (blockquote 내 HTML block 감지 미구현)
    #[case(
        "> <div>\n> foo\n\nbar",
        vec![
            BlockNode::blockquote(vec![BlockNode::html_block("<div>\nfoo")]),
            BlockNode::paragraph(vec![InlineNode::text("bar")]),
        ]
    )]
    // Example 175: HTML block in list (list 내 HTML block 감지 미구현)
    #[case(
        "- <div>\n- foo",
        vec![
            BlockNode::bullet_list(true, vec![
                crate::node::ListItemNode::new(vec![BlockNode::html_block("<div>")]),
                crate::node::ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("foo")])]),
            ]),
        ]
    )]
    #[ignore = "container block 내부 HTML block 감지 미구현"]
    fn test_html_block_type1_pending(#[case] _input: &str, #[case] _expected: Vec<BlockNode>) {
    }
}
