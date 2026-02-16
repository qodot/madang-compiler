//! Paragraph 파싱
//!
//! CommonMark 명세: https://spec.commonmark.org/0.31.2/#paragraphs

use crate::node::{BlockNode, ParagraphNode};
use super::inline::parse_inlines;

/// Paragraph 파싱 (기본 fallback)
/// 다른 블록 요소가 아닌 경우 항상 Paragraph로 처리
pub fn parse(trimmed: &str) -> BlockNode {
    BlockNode::Paragraph(ParagraphNode::with_raw_text(parse_inlines(trimmed), trimmed))
}

#[cfg(test)]
mod tests {
    use crate::node::{BlockNode, InlineNode};
    use crate::parser::parse;
    use rstest::rstest;

    #[rstest]
    // Example 219: 빈 줄로 분리된 두 Paragraph
    #[case("aaa\n\nbbb", vec![BlockNode::paragraph(vec![InlineNode::text("aaa")]), BlockNode::paragraph(vec![InlineNode::text("bbb")])])]
    // Example 220: 여러 줄 Paragraph (soft line break)
    #[case("aaa\nbbb\n\nccc\nddd", vec![BlockNode::paragraph(vec![InlineNode::text("aaa"), InlineNode::SoftBreak, InlineNode::text("bbb")]), BlockNode::paragraph(vec![InlineNode::text("ccc"), InlineNode::SoftBreak, InlineNode::text("ddd")])])]
    // Example 221: 여러 빈 줄로 분리
    #[case("aaa\n\n\nbbb", vec![BlockNode::paragraph(vec![InlineNode::text("aaa")]), BlockNode::paragraph(vec![InlineNode::text("bbb")])])]
    // Example 222: 선행 공백 제거
    #[case("  aaa\n bbb", vec![BlockNode::paragraph(vec![InlineNode::text("aaa"), InlineNode::SoftBreak, InlineNode::text("bbb")])])]
    // Example 223: 들여쓰기된 continuation lines
    #[case("aaa\n         bbb\n                                       ccc", vec![BlockNode::paragraph(vec![InlineNode::text("aaa"), InlineNode::SoftBreak, InlineNode::text("bbb"), InlineNode::SoftBreak, InlineNode::text("ccc")])])]
    // Example 224: 3칸 들여쓰기 허용
    #[case("   aaa\nbbb", vec![BlockNode::paragraph(vec![InlineNode::text("aaa"), InlineNode::SoftBreak, InlineNode::text("bbb")])])]
    // Example 225: 4칸 들여쓰기 → code block + paragraph
    #[case("    aaa\nbbb", vec![BlockNode::code_block(None, "aaa"), BlockNode::paragraph(vec![InlineNode::text("bbb")])])]
    // Example 227: 빈 줄만 있는 문서 — paragraph + heading
    #[case("  \n\naaa\n  \n\n# aaa\n\n  ", vec![BlockNode::paragraph(vec![InlineNode::text("aaa")]), BlockNode::heading(1, vec![InlineNode::text("aaa")])])]
    // 추가 케이스
    #[case("hello", vec![BlockNode::paragraph(vec![InlineNode::text("hello")])])]
    #[case("\n\nparagraph", vec![BlockNode::paragraph(vec![InlineNode::text("paragraph")])])]
    #[case("paragraph\n\n", vec![BlockNode::paragraph(vec![InlineNode::text("paragraph")])])]
    fn test_paragraph(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse(input);
        assert_eq!(doc.children, expected);
    }
}
