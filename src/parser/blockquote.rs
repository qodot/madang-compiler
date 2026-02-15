//! https://spec.commonmark.org/0.31.2/#block-quotes

use super::helpers::calculate_indent;
use crate::node::{BlockNode, BlockquoteNode};

#[derive(Debug, Clone, PartialEq)]
pub enum BlockquoteErr {
    /// 4칸 이상 들여쓰기 (코드 블록으로 해석됨)
    CodeBlockIndented,
    /// >로 시작하지 않음
    NotBlockquoteMarker,
}

pub fn parse(line: &str) -> Result<String, BlockquoteErr> {
    let indent = calculate_indent(line);
    let trimmed = line.trim();

    // 들여쓰기 3칸 초과면 Blockquote 아님
    if indent > 3 {
        return Err(BlockquoteErr::CodeBlockIndented);
    }

    // >로 시작하지 않으면 Blockquote 아님
    if !trimmed.starts_with('>') {
        return Err(BlockquoteErr::NotBlockquoteMarker);
    }

    // > 마커 제거 후 내용 반환
    let rest = &trimmed[1..];
    let content = if rest.starts_with(' ') || rest.starts_with('\t') {
        &rest[1..]
    } else {
        rest
    };

    Ok(content.to_string())
}

pub fn finalize<F>(contents: Vec<String>, parse_block: F) -> BlockNode
where
    F: Fn(&str) -> BlockNode,
{
    let text = contents.join("\n");

    // \n\n으로 분리하여 각 블록 파싱
    let children: Vec<BlockNode> = text
        .split("\n\n")
        .filter(|s| !s.is_empty())
        .map(parse_block)
        .collect();

    BlockNode::Blockquote(BlockquoteNode::new(children))
}

pub fn parse_text<F>(text: &str, parse_block: F) -> Option<BlockNode>
where
    F: Fn(&str) -> BlockNode,
{
    let mut contents: Vec<String> = Vec::new();

    for line in text.lines() {
        match parse(line) {
            Ok(content) => contents.push(content),
            Err(BlockquoteErr::NotBlockquoteMarker) => {
                // Lazy continuation: > 없는 줄은 그대로 유지
                if contents.is_empty() {
                    // 첫 줄이 blockquote가 아니면 None
                    return None;
                }
                contents.push(line.trim().to_string());
            }
            Err(BlockquoteErr::CodeBlockIndented) => {
                // 4칸 이상 들여쓰기면 blockquote 아님
                if contents.is_empty() {
                    return None;
                }
                // 이미 시작된 blockquote 안에서는 lazy continuation으로 처리
                contents.push(line.trim().to_string());
            }
        }
    }

    if contents.is_empty() {
        return None;
    }

    Some(finalize(contents, parse_block))
}

#[cfg(test)]
mod tests {
    use crate::node::{BlockNode, InlineNode, ListItemNode};
    use crate::parser::parse;
    use rstest::rstest;

    fn bq(depth: usize, inner: BlockNode) -> BlockNode {
        let mut result = inner;
        for _ in 0..depth {
            result = BlockNode::blockquote(vec![result]);
        }
        result
    }

    #[rstest]
    // Example 228: Blockquote 내 heading
    #[case("> # Foo", vec![BlockNode::blockquote(vec![BlockNode::heading(1, vec![InlineNode::text("Foo")])])])]
    #[case("> # Title", vec![BlockNode::blockquote(vec![BlockNode::heading(1, vec![InlineNode::text("Title")])])])]
    #[case("> ## Subtitle", vec![BlockNode::blockquote(vec![BlockNode::heading(2, vec![InlineNode::text("Subtitle")])])])]
    // Example 228: Blockquote 내 thematic break
    #[case("> ---", vec![BlockNode::blockquote(vec![BlockNode::thematic_break()])])]
    #[case("> ***", vec![BlockNode::blockquote(vec![BlockNode::thematic_break()])])]
    // Example 229: > 뒤 공백 없어도 OK
    #[case(">hello", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("hello")])])])]
    #[case(">bar", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("bar")])])])]
    // Example 230: 1-3칸 들여쓰기 허용
    #[case(" > hello", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("hello")])])])]
    #[case("  > hello", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("hello")])])])]
    #[case("   > hello", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("hello")])])])]
    // Example 231: 4칸 들여쓰기는 code block
    #[case("    > # Foo", vec![BlockNode::code_block(None, "> # Foo")])]
    // Example 232: Lazy continuation
    #[case("> bar\nbaz", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("bar"), InlineNode::SoftBreak, InlineNode::text("baz")])])])]
    // Example 233: Lazy continuation with > marker after lazy line
    #[case("> bar\nbaz\n> foo", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("bar"), InlineNode::SoftBreak, InlineNode::text("baz"), InlineNode::SoftBreak, InlineNode::text("foo")])])])]
    // Example 234: Laziness 한계 - thematic break
    #[case("> foo\n---", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("foo")])]), BlockNode::thematic_break()])]
    // Example 238: 4칸 들여쓰기 lazy continuation
    #[case("> foo\n    - bar", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("foo"), InlineNode::SoftBreak, InlineNode::text("- bar")])])])]
    // Example 239: 빈 blockquote
    #[case(">", vec![BlockNode::blockquote(vec![])])]
    // Example 240: 공백만 있는 빈 blockquote
    #[case(">\n>  \n> ", vec![BlockNode::blockquote(vec![])])]
    // Example 241: 앞뒤 빈 줄 있는 blockquote
    #[case(">\n> foo\n>  ", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("foo")])])])]
    // Example 242: 빈 줄로 분리된 두 blockquote
    #[case("> foo\n\n> bar", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("foo")])]), BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("bar")])])])]
    // Example 243: 여러 줄 하나의 paragraph
    #[case("> foo\n> bar", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("foo"), InlineNode::SoftBreak, InlineNode::text("bar")])])])]
    // Example 244: Blockquote 내 복수 단락
    #[case("> foo\n>\n> bar", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("foo")]), BlockNode::paragraph(vec![InlineNode::text("bar")])])])]
    #[case("> line1\n>\n> line2", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("line1")]), BlockNode::paragraph(vec![InlineNode::text("line2")])])])]
    #[case("> a\n>\n> b\n>\n> c", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("a")]), BlockNode::paragraph(vec![InlineNode::text("b")]), BlockNode::paragraph(vec![InlineNode::text("c")])])])]
    // Example 245: Paragraph 후 blockquote
    #[case("foo\n> bar", vec![BlockNode::paragraph(vec![InlineNode::text("foo")]), BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("bar")])])])]
    // Example 246: blockquote/thematic break/blockquote
    #[case("> aaa\n***\n> bbb", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("aaa")])]), BlockNode::thematic_break(), BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("bbb")])])])]
    // Example 247: Lazy continuation
    #[case("> bar\nbaz", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("bar"), InlineNode::SoftBreak, InlineNode::text("baz")])])])]
    // Example 248: Blockquote 후 빈 줄 + paragraph
    #[case("> bar\n\nbaz", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("bar")])]), BlockNode::paragraph(vec![InlineNode::text("baz")])])]
    // 추가 케이스
    #[case("> hello", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("hello")])])])]
    #[case(">  hello", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("hello")])])])]
    #[case("> 안녕하세요", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("안녕하세요")])])])]
    #[case("> a\n> b\n> c", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("a"), InlineNode::SoftBreak, InlineNode::text("b"), InlineNode::SoftBreak, InlineNode::text("c")])])])]
    #[case(">line1\n>line2", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("line1"), InlineNode::SoftBreak, InlineNode::text("line2")])])])]
    #[case("> a\nb\nc", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("a"), InlineNode::SoftBreak, InlineNode::text("b"), InlineNode::SoftBreak, InlineNode::text("c")])])])]
    #[case("> start\n> middle\nend", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("start"), InlineNode::SoftBreak, InlineNode::text("middle"), InlineNode::SoftBreak, InlineNode::text("end")])])])]
    fn test_blockquote(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse(input);
        assert_eq!(doc.children, expected);
    }

    #[rstest]
    // Example 250: 중첩 blockquote + lazy continuation
    #[case("> > > foo\nbar", 3, "foo\nbar")]
    // Example 251: 최소 마커 중첩
    #[case(">>> foo", 3, "foo")]
    // 추가 케이스
    #[case("> > nested", 2, "nested")]
    #[case("> > > deep", 3, "deep")]
    #[case("> > > > 4단계", 4, "4단계")]
    #[case("> > a\n> > b", 2, "a\nb")]
    fn test_nested_blockquote(#[case] input: &str, #[case] depth: usize, #[case] text: &str) {
        let doc = parse(input);
        let inlines: Vec<InlineNode> = text.split('\n').enumerate().fold(vec![], |mut acc, (i, part)| {
            if i > 0 { acc.push(InlineNode::SoftBreak); }
            acc.push(InlineNode::text(part));
            acc
        });
        let expected = vec![bq(depth, BlockNode::paragraph(inlines))];
        assert_eq!(doc.children, expected);
    }

    // TODO: 현재 파서 미지원 케이스
    #[rstest]
    // Example 235: Laziness 한계 - list가 blockquote 중단
    #[case("> - foo\n- bar", vec![BlockNode::blockquote(vec![BlockNode::bullet_list(true, vec![ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("foo")])])])]), BlockNode::bullet_list(true, vec![ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("bar")])])])])]
    // Example 236: Laziness 한계 - indented code block
    #[case(">     foo\n    bar", vec![BlockNode::blockquote(vec![BlockNode::code_block(None, "foo")]), BlockNode::code_block(None, "bar")])]
    // Example 237: Laziness 한계 - fenced code block
    #[case("> ```\nfoo\n```", vec![BlockNode::blockquote(vec![BlockNode::code_block(None, "")]), BlockNode::paragraph(vec![InlineNode::text("foo")]), BlockNode::code_block(None, "")])]
    // Example 249: 빈 blockquote 줄 후 paragraph
    #[case("> bar\n>\nbaz", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("bar")])]), BlockNode::paragraph(vec![InlineNode::text("baz")])])]
    // Example 252: blockquote 내 5칸 들여쓰기 → 코드블록, 4칸은 paragraph
    #[case(">     code\n\n>    not code", vec![BlockNode::blockquote(vec![BlockNode::code_block(None, "code")]), BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("not code")])])])]
    #[ignore = "blockquote 내 indented code block 미지원"]
    fn test_blockquote_pending(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse(input);
        assert_eq!(doc.children, expected);
    }

    /// 탭 관련 blockquote 테스트 (CommonMark 명세 Section 2.2 Tabs)
    #[rstest]
    // Example 6: > 뒤 탭 → 탭이 3칸 공백으로 확장, 1칸은 구분자 → 코드 블록 (2칸 들여쓰기)
    #[case(">\t\tfoo", vec![BlockNode::blockquote(vec![BlockNode::code_block(None, "  foo")])])]
    #[ignore = "탭 처리 미지원"]
    fn test_blockquote_tabs(#[case] _input: &str, #[case] _expected: Vec<BlockNode>) {
    }
}
