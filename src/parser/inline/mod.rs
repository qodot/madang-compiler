//! Inline 파서
//!
//! 블록 파싱 후 텍스트를 인라인 노드로 변환합니다.
//! CommonMark 명세 Section 6: https://spec.commonmark.org/0.31.2/#inlines

mod code_span;

use crate::node::{CodeSpanNode, InlineNode, TextNode};

/// raw 텍스트를 인라인 노드들로 파싱
///
/// 블록 파서가 추출한 텍스트를 받아서 인라인 구조를 파싱합니다.
pub fn parse_inlines(raw: &str) -> Vec<InlineNode> {
    let mut result = Vec::new();
    let mut pos = 0;
    let bytes = raw.as_bytes();

    while pos < raw.len() {
        match bytes[pos] {
            b'`' => {
                match code_span::parse_code_span(&raw[pos..]) {
                    Some(cs) => {
                        result.push(InlineNode::CodeSpan(CodeSpanNode::new(&cs.content)));
                        pos += cs.bytes_consumed;
                    }
                    None => {
                        // 매칭 안 되면 백틱 시퀀스 전체를 텍스트로 처리
                        let backtick_len = raw[pos..].chars().take_while(|&c| c == '`').count();
                        for _ in 0..backtick_len {
                            push_text_char(&mut result, '`');
                        }
                        pos += backtick_len;
                    }
                }
            }
            _ => {
                push_text_char(&mut result, raw[pos..].chars().next().unwrap());
                pos += raw[pos..].chars().next().unwrap().len_utf8();
            }
        }
    }

    // 빈 입력이면 빈 텍스트 노드 하나 반환 (기존 동작 유지)
    if result.is_empty() {
        return vec![InlineNode::Text(TextNode::new(""))];
    }

    result
}

/// 마지막 노드가 Text이면 문자를 추가, 아니면 새 Text 노드 생성
fn push_text_char(result: &mut Vec<InlineNode>, c: char) {
    if let Some(InlineNode::Text(text)) = result.last_mut() {
        text.0.push(c);
    } else {
        let mut s = String::new();
        s.push(c);
        result.push(InlineNode::Text(TextNode(s)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::InlineNode;
    use rstest::rstest;

    // =========================================================================
    // parse_inlines 기본 테스트
    // =========================================================================

    #[rstest]
    #[case("hello", vec![InlineNode::text("hello")])]
    #[case("", vec![InlineNode::text("")])]
    #[case("foo bar baz", vec![InlineNode::text("foo bar baz")])]
    fn test_parse_inlines_text(#[case] input: &str, #[case] expected: Vec<InlineNode>) {
        assert_eq!(parse_inlines(input), expected);
    }

    // =========================================================================
    // parse_inlines — code span 통합 테스트
    // =========================================================================

    #[rstest]
    // Example 328: 기본 code span
    #[case("`foo`", vec![InlineNode::code_span("foo")])]
    // Example 329: 백틱 2개, 내부에 백틱 1개
    #[case("`` foo ` bar ``", vec![InlineNode::code_span("foo ` bar")])]
    // Example 330: 내부에 백틱 2개
    #[case("` `` `", vec![InlineNode::code_span("``")])]
    // Example 331: 내부 공백 보존
    #[case("`  ``  `", vec![InlineNode::code_span(" `` ")])]
    // Example 332: 앞에만 공백
    #[case("` a`", vec![InlineNode::code_span(" a")])]
    // Example 335: 줄바꿈 → 공백
    #[case("``\nfoo\nbar  \nbaz\n``", vec![InlineNode::code_span("foo bar   baz")])]
    // Example 337: 내부 공백 유지
    #[case("`foo   bar \nbaz`", vec![InlineNode::code_span("foo   bar  baz")])]
    // Example 338: 백슬래시는 code span 내에서 이스케이프 안 됨
    #[case("`foo\\`bar`", vec![InlineNode::code_span("foo\\"), InlineNode::text("bar`")])]
    // Example 339: 백틱 2개로 감싸면 내부 백틱 1개 OK
    #[case("``foo`bar``", vec![InlineNode::code_span("foo`bar")])]
    // Example 340: 백틱 1개로 감싸면 내부 백틱 2개 OK
    #[case("` foo `` bar `", vec![InlineNode::code_span("foo `` bar")])]
    // Example 341: code span이 emphasis보다 우선
    #[case("*foo`*`", vec![InlineNode::text("*foo"), InlineNode::code_span("*")])]
    // Example 342: code span이 link보다 우선
    #[case("[not a `link](/foo`)", vec![InlineNode::text("[not a "), InlineNode::code_span("link](/foo"), InlineNode::text(")")])]
    // Example 347: 닫는 백틱 없음 → 텍스트
    #[case("```foo``", vec![InlineNode::text("```foo``")])]
    // Example 348: 닫는 백틱 없음 → 텍스트
    #[case("`foo", vec![InlineNode::text("`foo")])]
    // Example 349: 첫 ` 매칭 안 됨, ``bar`` 매칭
    #[case("`foo``bar``", vec![InlineNode::text("`foo"), InlineNode::code_span("bar")])]
    // code span 앞뒤에 텍스트
    #[case("hello `world` bye", vec![InlineNode::text("hello "), InlineNode::code_span("world"), InlineNode::text(" bye")])]
    // 여러 code span
    #[case("`a` and `b`", vec![InlineNode::code_span("a"), InlineNode::text(" and "), InlineNode::code_span("b")])]
    fn test_parse_inlines_code_span(#[case] input: &str, #[case] expected: Vec<InlineNode>) {
        assert_eq!(parse_inlines(input), expected);
    }
}
