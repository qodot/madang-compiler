//! Indented Code Block 파서
//!
//! 4칸 들여쓰기로 작성된 코드 블록을 파싱합니다.

use super::helpers::{calculate_indent, count_leading_char};

// =============================================================================
// 타입 정의
// =============================================================================

/// Indented Code Block 시작 정보
#[derive(Debug, Clone, PartialEq)]
pub struct CodeBlockIndentedStart {
    /// 첫 줄 내용 (4칸 들여쓰기 제거 후)
    pub content: String,
}

/// Indented Code Block 시작 성공 사유
#[derive(Debug, Clone, PartialEq)]
pub enum CodeBlockIndentedStartReason {
    /// 정상적인 시작
    Started(CodeBlockIndentedStart),
}

/// Indented Code Block 시작 아님 사유
#[derive(Debug, Clone, PartialEq)]
pub enum CodeBlockIndentedNotStartReason {
    /// 빈 줄 (공백만 있는 줄 포함)
    Empty,
    /// 들여쓰기 부족 (4칸 미만)
    InsufficientIndent,
}

// =============================================================================
// 함수
// =============================================================================

/// Indented Code Block 시작 줄인지 확인
/// 성공 시 Ok(Started), 실패 시 Err(사유) 반환
pub(crate) fn try_start(
    line: &str,
) -> Result<CodeBlockIndentedStartReason, CodeBlockIndentedNotStartReason> {
    // 1. 들여쓰기 확인 (4칸 이상이면 코드 줄, 탭은 4칸 탭 스톱으로 계산)
    let indent = calculate_indent(line);
    if indent >= 4 {
        // 4칸 제거 후 내용 반환
        let content = remove_leading_indent(line, 4);
        return Ok(CodeBlockIndentedStartReason::Started(
            CodeBlockIndentedStart { content },
        ));
    }

    // 2. 4칸 미만 들여쓰기: 빈 줄이면 Empty, 아니면 InsufficientIndent
    if line.trim().is_empty() {
        return Err(CodeBlockIndentedNotStartReason::Empty);
    }

    Err(CodeBlockIndentedNotStartReason::InsufficientIndent)
}

/// 줄 앞에서 n칸의 indent를 제거 (탭은 4칸 탭 스톱으로 계산)
fn remove_leading_indent(line: &str, n: usize) -> String {
    let mut col = 0;
    let mut byte_pos = 0;

    for (i, c) in line.char_indices() {
        if col >= n {
            byte_pos = i;
            return line[byte_pos..].to_string();
        }
        match c {
            ' ' => col += 1,
            '\t' => {
                let tab_width = 4 - (col % 4);
                col += tab_width;
                if col > n {
                    // 탭이 n칸을 넘어서면 남은 공백을 spaces로 채움
                    let remaining_spaces = col - n;
                    return " ".repeat(remaining_spaces) + &line[i + 1..];
                }
            }
            _ => {
                byte_pos = i;
                return line[byte_pos..].to_string();
            }
        }
        byte_pos = i + c.len_utf8();
    }

    String::new()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{BlockNode, InlineNode, ListItemNode};
    use rstest::rstest;

    /// try_start 테스트: 성공/실패 케이스 통합
    /// expected: Ok(content) 또는 Err(reason)
    #[rstest]
    // 성공 케이스: 4칸 이상 들여쓰기
    #[case("    code", Ok("code"))]
    #[case("     code", Ok(" code"))]
    #[case("        code", Ok("    code"))]
    // 실패 케이스: 빈 줄
    #[case("", Err(CodeBlockIndentedNotStartReason::Empty))]
    #[case("   ", Err(CodeBlockIndentedNotStartReason::Empty))]
    // 실패 케이스: 들여쓰기 부족
    #[case("code", Err(CodeBlockIndentedNotStartReason::InsufficientIndent))]
    #[case(" code", Err(CodeBlockIndentedNotStartReason::InsufficientIndent))]
    #[case("  code", Err(CodeBlockIndentedNotStartReason::InsufficientIndent))]
    #[case("   code", Err(CodeBlockIndentedNotStartReason::InsufficientIndent))]
    fn test_try_start(
        #[case] input: &str,
        #[case] expected: Result<&str, CodeBlockIndentedNotStartReason>,
    ) {
        let result = try_start(input);
        match expected {
            Ok(content) => {
                let reason = result.expect("시작이어야 함");
                let CodeBlockIndentedStartReason::Started(start) = reason;
                assert_eq!(start.content, content, "입력: {:?}", input);
            }
            Err(expected_reason) => {
                let reason = result.expect_err("시작이 아니어야 함");
                assert_eq!(reason, expected_reason, "입력: {:?}", input);
            }
        }
    }

    /// Indented Code Block 통합 테스트 (CommonMark 명세 기반)
    #[rstest]
    // Example 107: 기본 코드 블록
    #[case("    a simple\n      indented code block", vec![BlockNode::code_block(None, "a simple\n  indented code block")])]
    // Example 110: HTML/마크다운은 그대로 코드로 처리
    #[case("    <a/>\n    *hi*\n\n    - one", vec![BlockNode::code_block(None, "<a/>\n*hi*\n\n- one")])]
    // Example 111: 빈 줄로 분리된 청크들은 하나의 블록
    #[case("    chunk1\n\n    chunk2\n  \n \n \n    chunk3", vec![BlockNode::code_block(None, "chunk1\n\nchunk2\n\n\n\nchunk3")])]
    // Example 112: 들여쓰기된 빈 줄 유지
    #[case("    chunk1\n      \n      chunk2", vec![BlockNode::code_block(None, "chunk1\n  \n  chunk2")])]
    // Example 113: Paragraph 인터럽트 불가 - 빈 줄 없이 4칸 들여쓰기는 Paragraph 일부
    #[case("Foo\n    bar", vec![BlockNode::paragraph(vec![InlineNode::text("Foo"), InlineNode::SoftBreak, InlineNode::text("bar")])])]
    // Example 114: 코드 블록 후 4칸 미만 줄은 새 Paragraph
    #[case("    foo\nbar", vec![BlockNode::code_block(None, "foo"), BlockNode::paragraph(vec![InlineNode::text("bar")])])]
    // Example 115: heading + code block + setext heading + code block + thematic break
    #[case("# Heading\n    foo\nHeading\n------\n    foo\n----", vec![
        BlockNode::heading(1, vec![InlineNode::text("Heading")]),
        BlockNode::code_block(None, "foo"),
        BlockNode::heading(2, vec![InlineNode::text("Heading")]),
        BlockNode::code_block(None, "foo"),
        BlockNode::thematic_break(),
    ])]
    // Example 116: 8칸 들여쓰기 (4칸 제거 후 4칸 유지)
    #[case("        foo\n    bar", vec![BlockNode::code_block(None, "    foo\nbar")])]
    // Example 117: 앞뒤 빈 줄은 제거됨
    #[case("\n    \n    foo\n    ", vec![BlockNode::code_block(None, "foo")])]
    // Example 118: 후행 공백은 유지됨
    #[case("    foo  ", vec![BlockNode::code_block(None, "foo  ")])]
    fn test_code_block_indented(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = crate::parse(input);
        assert_eq!(doc.children, expected);
    }

    /// list item 내 indented code 관련 — 현재 파서 미지원
    #[rstest]
    // Example 108: list item 내 continuation paragraph
    #[case("  - foo\n\n    bar", vec![BlockNode::bullet_list(false, vec![
        ListItemNode::new(vec![
            BlockNode::paragraph(vec![InlineNode::text("foo")]),
            BlockNode::paragraph(vec![InlineNode::text("bar")]),
        ]),
    ])])]
    // Example 109: ordered list + nested bullet list (loose due to blank line)
    #[case("1.  foo\n\n    - bar", vec![BlockNode::ordered_list('.', 1, false, vec![
        ListItemNode::new(vec![
            BlockNode::paragraph(vec![InlineNode::text("foo")]),
            BlockNode::bullet_list(true, vec![
                ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("bar")])]),
            ]),
        ]),
    ])])]
    #[ignore = "list item 내 loose list 판정 미지원"]
    fn test_code_block_indented_pending(#[case] _input: &str, #[case] _expected: Vec<BlockNode>) {
    }

    /// 탭 관련 indented code block 테스트 (CommonMark 명세 Section 2.2 Tabs)
    #[rstest]
    // Example 1: 탭으로 인덴트된 코드 블록
    #[case("\tfoo\tbaz\t\tbim", vec![BlockNode::code_block(None, "foo baz     bim")])]
    // Example 2: 2칸 공백 + 탭 (탭이 나머지 2칸을 채워 4칸 인덴트)
    #[case("  \tfoo\tbaz\t\tbim", vec![BlockNode::code_block(None, "foo baz     bim")])]
    // Example 3: 탭 위치에 따른 정렬 (탭이 spaces로 확장됨)
    #[case("    a\ta\n    ὐ\ta", vec![BlockNode::code_block(None, "a   a\nὐ   a")])]
    // Example 8: space + tab 혼용 (4칸 space 인덴트 후, 다음 줄은 탭으로 인덴트)
    #[case("    foo\n\tbar", vec![BlockNode::code_block(None, "foo\nbar")])]
    fn test_code_block_indented_tabs(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = crate::parse(input);
        assert_eq!(doc.children, expected);
    }
}
