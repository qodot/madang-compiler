//! Inline 파서
//!
//! 블록 파싱 후 텍스트를 인라인 노드로 변환합니다.
//! CommonMark 명세 Section 6: https://spec.commonmark.org/0.31.2/#inlines

mod code_span;

use crate::node::{InlineNode, TextNode};

/// raw 텍스트를 인라인 노드들로 파싱
///
/// 블록 파서가 추출한 텍스트를 받아서 인라인 구조를 파싱합니다.
/// 현재는 텍스트를 그대로 반환하며, 점진적으로 code span, emphasis 등을 추가합니다.
pub fn parse_inlines(raw: &str) -> Vec<InlineNode> {
    vec![InlineNode::Text(TextNode::new(raw))]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::InlineNode;
    use rstest::rstest;

    #[rstest]
    #[case("hello", vec![InlineNode::text("hello")])]
    #[case("", vec![InlineNode::text("")])]
    #[case("foo bar baz", vec![InlineNode::text("foo bar baz")])]
    fn test_parse_inlines(#[case] input: &str, #[case] expected: Vec<InlineNode>) {
        assert_eq!(parse_inlines(input), expected);
    }
}
