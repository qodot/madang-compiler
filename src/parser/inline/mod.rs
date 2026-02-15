//! Inline 파서
//!
//! 블록 파싱 후 텍스트를 인라인 노드로 변환합니다.
//! CommonMark 명세 Section 6: https://spec.commonmark.org/0.31.2/#inlines

mod autolink;
mod backslash_escape;
mod code_span;
mod line_break;

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
            b'\\' => {
                // \ + \n → hard line break
                if pos + 1 < raw.len() && bytes[pos + 1] == b'\n' {
                    strip_trailing_spaces_from_last_text(&mut result);
                    result.push(InlineNode::HardBreak);
                    pos += 2; // skip \ and \n
                    // 다음 줄의 leading spaces 건너뛰기
                    pos += line_break::skip_leading_spaces(&raw[pos..]);
                } else {
                    match backslash_escape::try_escape(&raw[pos..]) {
                        Some((escaped_char, consumed)) => {
                            push_text_char(&mut result, escaped_char);
                            pos += consumed;
                        }
                        None => {
                            push_text_char(&mut result, '\\');
                            pos += 1;
                        }
                    }
                }
            }
            b'`' => {
                match code_span::parse_code_span(&raw[pos..]) {
                    Some(cs) => {
                        result.push(InlineNode::CodeSpan(CodeSpanNode::new(&cs.content)));
                        pos += cs.bytes_consumed;
                    }
                    None => {
                        let backtick_len = raw[pos..].chars().take_while(|&c| c == '`').count();
                        for _ in 0..backtick_len {
                            push_text_char(&mut result, '`');
                        }
                        pos += backtick_len;
                    }
                }
            }
            b'\n' => {
                // trailing spaces 2개 이상 → hard break, 그 외 → soft break
                let trailing = last_text_trailing_spaces(&result);
                if trailing >= 2 {
                    strip_trailing_spaces_from_last_text(&mut result);
                    result.push(InlineNode::HardBreak);
                } else {
                    strip_trailing_spaces_from_last_text(&mut result);
                    result.push(InlineNode::SoftBreak);
                }
                pos += 1;
                // 다음 줄의 leading spaces 건너뛰기
                pos += line_break::skip_leading_spaces(&raw[pos..]);
            }
            _ => {
                let c = raw[pos..].chars().next().unwrap();
                push_text_char(&mut result, c);
                pos += c.len_utf8();
            }
        }
    }

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

/// 마지막 Text 노드의 trailing spaces 수
fn last_text_trailing_spaces(result: &[InlineNode]) -> usize {
    if let Some(InlineNode::Text(text)) = result.last() {
        line_break::count_trailing_spaces(&text.0)
    } else {
        0
    }
}

/// 마지막 Text 노드에서 trailing spaces 제거
fn strip_trailing_spaces_from_last_text(result: &mut Vec<InlineNode>) {
    if let Some(InlineNode::Text(text)) = result.last_mut() {
        let stripped = line_break::strip_trailing_spaces(&text.0).to_string();
        text.0 = stripped;
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
    // Example 343: code span 내에서 HTML은 리터럴
    #[case("`<a href=\"`\">` ", vec![InlineNode::code_span("<a href=\""), InlineNode::text("\">` ")])]
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

    // =========================================================================
    // parse_inlines — backslash escape 통합 테스트
    // =========================================================================

    #[rstest]
    // Example 12: 모든 ASCII 구두점 이스케이프
    #[case(
        "\\!\\\"\\#\\$\\%\\&\\'\\(\\)\\*\\+\\,\\-\\.\\/\\:\\;\\<\\=\\>\\?\\@\\[\\\\\\]\\^\\_\\`\\{\\|\\}\\~",
        vec![InlineNode::text("!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~")]
    )]
    // Example 13: 구두점이 아닌 문자는 이스케이프 안 됨
    #[case("\\A\\a\\ \\3", vec![InlineNode::text("\\A\\a\\ \\3")])]
    // Example 14 (부분): \*는 리터럴 *
    #[case("\\*not emphasized*", vec![InlineNode::text("*not emphasized*")])]
    // Example 14 (부분): \`는 리터럴 `
    #[case("\\`not code`", vec![InlineNode::text("`not code`")])]
    // Example 15 (부분): \\는 리터럴 \
    #[case("\\\\hello", vec![InlineNode::text("\\hello")])]
    // Example 17: code span 내에서는 이스케이프 안 됨
    #[case("`` \\[\\` ``", vec![InlineNode::code_span("\\[\\`")])]
    // 이스케이프와 code span 혼합
    #[case("\\*`code`\\*", vec![InlineNode::text("*"), InlineNode::code_span("code"), InlineNode::text("*")])]
    fn test_parse_inlines_backslash_escape(#[case] input: &str, #[case] expected: Vec<InlineNode>) {
        assert_eq!(parse_inlines(input), expected);
    }

    // =========================================================================
    // parse_inlines — line break 통합 테스트
    // =========================================================================

    #[rstest]
    // Example 633: trailing spaces 2개 → hard break
    #[case("foo  \nbaz", vec![InlineNode::text("foo"), InlineNode::HardBreak, InlineNode::text("baz")])]
    // Example 634: \ + \n → hard break
    #[case("foo\\\nbaz", vec![InlineNode::text("foo"), InlineNode::HardBreak, InlineNode::text("baz")])]
    // Example 635: trailing spaces 많이 → hard break
    #[case("foo       \nbaz", vec![InlineNode::text("foo"), InlineNode::HardBreak, InlineNode::text("baz")])]
    // Example 636: hard break 후 leading spaces 제거
    #[case("foo  \n     bar", vec![InlineNode::text("foo"), InlineNode::HardBreak, InlineNode::text("bar")])]
    // Example 637: \ hard break 후 leading spaces 제거
    #[case("foo\\\n     bar", vec![InlineNode::text("foo"), InlineNode::HardBreak, InlineNode::text("bar")])]
    // Example 640: code span 내에서는 hard break 아님 (줄바꿈 → 공백)
    #[case("`code  \nspan`", vec![InlineNode::code_span("code   span")])]
    // Example 641: code span 내에서 \도 리터럴
    #[case("`code\\\nspan`", vec![InlineNode::code_span("code\\ span")])]
    // Example 644: \ + EOF → 리터럴 \ (hard break 아님)
    #[case("foo\\", vec![InlineNode::text("foo\\")])]
    // Example 645: trailing spaces + EOF → spaces 제거 (hard break 아님)
    #[case("foo  ", vec![InlineNode::text("foo  ")])]
    // Example 648: soft line break
    #[case("foo\nbaz", vec![InlineNode::text("foo"), InlineNode::SoftBreak, InlineNode::text("baz")])]
    // Example 649: soft break 전후 spaces 제거
    #[case("foo \n baz", vec![InlineNode::text("foo"), InlineNode::SoftBreak, InlineNode::text("baz")])]
    fn test_parse_inlines_line_break(#[case] input: &str, #[case] expected: Vec<InlineNode>) {
        assert_eq!(parse_inlines(input), expected);
    }
}
