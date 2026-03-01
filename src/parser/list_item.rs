//! List Item 파서 (CommonMark 5.2 List Items)
//!
//! https://spec.commonmark.org/0.31.2/#list-items
//!
//! - 마커 인식 규칙 (Example 261, 265-269)
//! - 들여쓰기 규칙 (Example 253-258, 270-277)
//! - 빈 줄로 시작하는 아이템 (Example 278-280)
//! - 빈 아이템 (Example 281-284)
//! - Paragraph 인터럽트 (Example 285)
//! - 복합 블록 (Example 254, 259-260, 262-264)

use crate::node::ListType;
use super::helpers::{consume_indent, consume_indent_from_col, count_leading_char};

// =============================================================================
// 타입 정의
// =============================================================================

/// 리스트 마커 타입
#[derive(Debug, Clone, PartialEq)]
pub enum ListMarker {
    /// Bullet 마커: '-', '+', '*'
    Bullet(char),
    /// Ordered 마커: 숫자 + '.' 또는 ')'
    Ordered {
        /// 시작 숫자
        start: usize,
        /// 구분자 ('.' 또는 ')')
        delimiter: char,
    },
}

impl ListMarker {
    /// ListType과 시작 번호로 변환
    pub fn to_list_type(&self) -> (ListType, usize) {
        match self {
            ListMarker::Bullet(_) => (ListType::Bullet, 1),
            ListMarker::Ordered { start, delimiter } => (
                ListType::Ordered {
                    delimiter: *delimiter,
                },
                *start,
            ),
        }
    }

    /// 같은 리스트 타입인지 확인 (같은 리스트에 속할 수 있는지)
    pub fn is_same_type(&self, other: &ListMarker) -> bool {
        match (self, other) {
            (ListMarker::Bullet(c1), ListMarker::Bullet(c2)) => c1 == c2,
            (
                ListMarker::Ordered { delimiter: d1, .. },
                ListMarker::Ordered { delimiter: d2, .. },
            ) => d1 == d2,
            _ => false,
        }
    }
}

/// List Item 시작 정보
/// parse에서 반환되며, 같은 리스트 소속 여부 판단에 사용
#[derive(Debug, Clone, PartialEq)]
pub struct ListItemStart {
    /// 마커 타입
    pub marker: ListMarker,
    /// 마커 앞 들여쓰기 (0-3칸)
    pub indent: usize,
    /// 내용 시작 위치 (마커 + 공백 이후)
    pub content_indent: usize,
    /// 첫 줄 내용 (마커 이후)
    pub content: String,
}

impl ListItemStart {
    /// 라인에서 content를 추출하여 새 인스턴스 반환
    /// content_indent columns만큼 소비하고 나머지를 content로 설정
    pub fn with_content_from(self, line: &str) -> Self {
        let content = consume_indent(line, self.content_indent);
        Self { content, ..self }
    }

    #[cfg(test)]
    pub fn bullet(marker_char: char, indent: usize, content_indent: usize, content: &str) -> Self {
        Self {
            marker: ListMarker::Bullet(marker_char),
            indent,
            content_indent,
            content: content.to_string(),
        }
    }

    #[cfg(test)]
    pub fn ordered(
        start: usize,
        delimiter: char,
        indent: usize,
        content_indent: usize,
        content: &str,
    ) -> Self {
        Self {
            marker: ListMarker::Ordered { start, delimiter },
            indent,
            content_indent,
            content: content.to_string(),
        }
    }
}


#[derive(Debug, Clone, PartialEq)]
pub enum ListItemOk {
    /// 정상적인 시작
    Started(ListItemStart),
}

#[derive(Debug, Clone, PartialEq)]
pub enum ListItemErr {
    /// 4칸 이상 들여쓰기 (indented code block으로 해석됨)
    CodeBlockIndented,
    /// 유효한 리스트 마커 아님
    NotListMarker,
}

/// 리스트 아이템 내용 줄
#[derive(Debug, Clone, PartialEq)]
pub struct ItemLine {
    /// 내용
    pub content: String,
    /// true면 텍스트 전용 (리스트 마커처럼 보여도 재파싱 시 리스트 아님)
    /// Example 303: 4칸 들여쓰기된 마커는 텍스트 전용
    pub text_only: bool,
}

impl ItemLine {
    pub fn text(content: String) -> Self {
        Self {
            content,
            text_only: false,
        }
    }

    pub fn text_only(content: String) -> Self {
        Self {
            content,
            text_only: true,
        }
    }

    pub fn blank() -> Self {
        Self {
            content: String::new(),
            text_only: false,
        }
    }
}


// =============================================================================
// 함수
// =============================================================================

/// column 위치 start_col에서 시작하여 leading whitespace의 column 수를 계산
/// 탭 스톱을 올바르게 고려함
fn calculate_indent_from_col(s: &str, start_col: usize) -> usize {
    let mut col = start_col;
    for c in s.chars() {
        match c {
            ' ' => col += 1,
            '\t' => col += 4 - (col % 4),
            _ => break,
        }
    }
    col - start_col
}

/// List Item 시작 줄인지 확인
/// 성공 시 Ok(Started), 실패 시 Err(사유) 반환
pub(crate) fn parse(line: &str) -> Result<ListItemOk, ListItemErr> {
    let indent = count_leading_char(line, ' ');

    // 4칸 이상 들여쓰기는 코드 블록
    if indent > 3 {
        return Err(ListItemErr::CodeBlockIndented);
    }

    let after_indent = &line[indent..];

    // Bullet 또는 Ordered 마커 시도 (content는 마커 함수에서 직접 추출)
    try_bullet_marker(after_indent, indent)
        .or_else(|| try_ordered_marker(after_indent, indent))
        .map(|start| ListItemOk::Started(start))
        .ok_or(ListItemErr::NotListMarker)
}


/// Bullet 마커 감지 (-*+)
fn try_bullet_marker(s: &str, indent: usize) -> Option<ListItemStart> {
    let first_char = s.chars().next()?;

    // Bullet 마커 문자인지 확인
    if !matches!(first_char, '-' | '+' | '*') {
        return None;
    }

    // 마커 뒤 공백 확인 (최소 1칸)
    let rest = &s[1..];
    if rest.is_empty() {
        // 마커만 있고 끝 → 빈 아이템으로 허용
        // content_indent = indent + marker_width + 1 (spec Rule 3: blank-start item)
        return Some(ListItemStart {
            marker: ListMarker::Bullet(first_char),
            indent,
            content_indent: indent + 2,
            content: String::new(),
        });
    }

    // 마커 뒤 첫 문자가 공백이어야 함
    let after_marker = rest.chars().next()?;
    if after_marker != ' ' && after_marker != '\t' {
        return None;
    }

    // 내용 시작 위치 계산 (마커 + 공백), column 기반 (탭 고려)
    let marker_col = indent + 1;
    let ws_cols = calculate_indent_from_col(rest, marker_col);
    let effective_ws = if ws_cols >= 5 { 1 } else { ws_cols };
    let content_indent = marker_col + effective_ws;

    // 첫 줄 content 추출: marker_col 위치에서 effective_ws columns를 소비
    let content = consume_indent_from_col(rest, effective_ws, marker_col);

    Some(ListItemStart {
        marker: ListMarker::Bullet(first_char),
        indent,
        content_indent,
        content,
    })
}

/// Ordered 마커 감지 (숫자 + . 또는 ))
fn try_ordered_marker(s: &str, indent: usize) -> Option<ListItemStart> {
    // 숫자 추출
    let num_str: String = s.chars().take_while(|c| c.is_ascii_digit()).collect();

    // 숫자가 없거나 9자리 초과면 실패 (Example 265-266)
    if num_str.is_empty() || num_str.len() > 9 {
        return None;
    }

    // 선행 0 허용: "003"은 3으로 파싱 (Example 267-268)
    let start_num: usize = num_str.parse().ok()?;

    // 숫자 뒤 구분자 확인
    let rest = &s[num_str.len()..];
    let delimiter = rest.chars().next()?;

    if delimiter != '.' && delimiter != ')' {
        return None;
    }

    // 구분자 뒤 공백 확인
    let after_delimiter = &rest[1..];
    if after_delimiter.is_empty() {
        // 구분자만 있고 끝 → 빈 아이템
        // content_indent = indent + marker_len + 1 (spec Rule 3: blank-start item)
        let marker_len = num_str.len() + 1; // 숫자 + 구분자
        return Some(ListItemStart {
            marker: ListMarker::Ordered {
                start: start_num,
                delimiter,
            },
            indent,
            content_indent: indent + marker_len + 1,
            content: String::new(),
        });
    }

    // 구분자 뒤 첫 문자가 공백이어야 함
    let after_char = after_delimiter.chars().next()?;
    if after_char != ' ' && after_char != '\t' {
        return None;
    }

    // content_indent 계산 (column 기반, 탭 고려)
    let marker_len = num_str.len() + 1; // 숫자 + 구분자
    let marker_col = indent + marker_len;
    let ws_cols = calculate_indent_from_col(after_delimiter, marker_col);
    let effective_ws = if ws_cols >= 5 { 1 } else { ws_cols };
    let content_indent = marker_col + effective_ws;

    // 첫 줄 content 추출: marker_col 위치에서 effective_ws columns를 소비
    let content = consume_indent_from_col(after_delimiter, effective_ws, marker_col);

    Some(ListItemStart {
        marker: ListMarker::Ordered {
            start: start_num,
            delimiter,
        },
        indent,
        content_indent,
        content,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{BlockNode, InlineNode, ListItemNode};
    use crate::parser::parse as parse_doc;
    use crate::Spec;
    use rstest::rstest;

    // 5.2 List Items - 마커 인식 (parse) 성공 케이스
    #[rstest]
    // 기본 Bullet 마커
    #[case("- item", ListItemStart::bullet('-', 0, 2, "item"))]
    #[case("+ item", ListItemStart::bullet('+', 0, 2, "item"))]
    #[case("* item", ListItemStart::bullet('*', 0, 2, "item"))]
    // Bullet 마커 앞 들여쓰기 (0-3칸)
    #[case(" - item", ListItemStart::bullet('-', 1, 3, "item"))]
    #[case("  - item", ListItemStart::bullet('-', 2, 4, "item"))]
    #[case("   - item", ListItemStart::bullet('-', 3, 5, "item"))]
    // Bullet 마커 뒤 여러 공백
    #[case("-  item", ListItemStart::bullet('-', 0, 3, "item"))]
    #[case("-   item", ListItemStart::bullet('-', 0, 4, "item"))]
    #[case("-    item", ListItemStart::bullet('-', 0, 5, "item"))]
    #[case("-     item", ListItemStart::bullet('-', 0, 2, "    item"))]  // 5+ spaces → N=1, 나머지는 content
    // Bullet 빈 아이템 (content_indent = indent + 2, spec Rule 3)
    #[case("-", ListItemStart::bullet('-', 0, 2, ""))]
    #[case("+", ListItemStart::bullet('+', 0, 2, ""))]
    #[case("*", ListItemStart::bullet('*', 0, 2, ""))]
    // 기본 Ordered 마커
    #[case("1. item", ListItemStart::ordered(1, '.', 0, 3, "item"))]
    #[case("2. item", ListItemStart::ordered(2, '.', 0, 3, "item"))]
    #[case("10. item", ListItemStart::ordered(10, '.', 0, 4, "item"))]
    #[case("123. item", ListItemStart::ordered(123, '.', 0, 5, "item"))]
    #[case("1) item", ListItemStart::ordered(1, ')', 0, 3, "item"))]
    #[case("2) item", ListItemStart::ordered(2, ')', 0, 3, "item"))]
    #[case("10) item", ListItemStart::ordered(10, ')', 0, 4, "item"))]
    // Ordered 마커 앞 들여쓰기
    #[case(" 1. item", ListItemStart::ordered(1, '.', 1, 4, "item"))]
    #[case("  1. item", ListItemStart::ordered(1, '.', 2, 5, "item"))]
    #[case("   1. item", ListItemStart::ordered(1, '.', 3, 6, "item"))]
    // Ordered 마커 뒤 여러 공백
    #[case("1.  item", ListItemStart::ordered(1, '.', 0, 4, "item"))]
    #[case("1.   item", ListItemStart::ordered(1, '.', 0, 5, "item"))]
    // Example 265: 9자리까지 허용
    #[case("123456789. ok", ListItemStart::ordered(123456789, '.', 0, 11, "ok"))]
    // Example 267: 0으로 시작 가능
    #[case("0. ok", ListItemStart::ordered(0, '.', 0, 3, "ok"))]
    // Example 268: 선행 0 허용 (값은 3)
    #[case("003. ok", ListItemStart::ordered(3, '.', 0, 5, "ok"))]
    // Ordered 빈 아이템 (content_indent = indent + marker_len + 1, spec Rule 3)
    #[case("1.", ListItemStart::ordered(1, '.', 0, 3, ""))]
    #[case("1)", ListItemStart::ordered(1, ')', 0, 3, ""))]
    fn test_parse_ok(
        #[case] input: &str,
        #[case] expected: ListItemStart,
    ) {
        assert_eq!(parse(input), Ok(ListItemOk::Started(expected)));
    }

    // 5.2 List Items - 마커 인식 (parse) 실패 케이스
    #[rstest]
    // Example 261: 마커 뒤 공백 필수
    #[case("-item", ListItemErr::NotListMarker)]
    #[case("--item", ListItemErr::NotListMarker)]
    #[case("1.item", ListItemErr::NotListMarker)]
    // 4칸 이상 들여쓰기는 코드 블록
    #[case("    - item", ListItemErr::CodeBlockIndented)]
    #[case("    1. item", ListItemErr::CodeBlockIndented)]
    // 유효한 리스트 마커 아님
    #[case("text", ListItemErr::NotListMarker)]
    #[case("", ListItemErr::NotListMarker)]
    // Example 266: 10자리 이상은 마커 아님
    #[case("1234567890. not ok", ListItemErr::NotListMarker)]
    // Example 269: 음수는 마커 아님
    #[case("-1. not ok", ListItemErr::NotListMarker)]
    #[case("a. item", ListItemErr::NotListMarker)]
    #[case("1: item", ListItemErr::NotListMarker)]
    fn test_parse_err(
        #[case] input: &str,
        #[case] expected: ListItemErr,
    ) {
        assert_eq!(parse(input), Err(expected));
    }

    // === ListMarker::to_list_type 테스트 ===
    #[rstest]
    #[case(ListMarker::Bullet('-'), ListType::Bullet, 1)]
    #[case(ListMarker::Bullet('+'), ListType::Bullet, 1)]
    #[case(ListMarker::Bullet('*'), ListType::Bullet, 1)]
    #[case(ListMarker::Ordered { start: 1, delimiter: '.' }, ListType::Ordered { delimiter: '.' }, 1)]
    #[case(ListMarker::Ordered { start: 5, delimiter: '.' }, ListType::Ordered { delimiter: '.' }, 5)]
    #[case(ListMarker::Ordered { start: 1, delimiter: ')' }, ListType::Ordered { delimiter: ')' }, 1)]
    fn test_to_list_type(
        #[case] marker: ListMarker,
        #[case] expected_type: ListType,
        #[case] expected_start: usize,
    ) {
        let (list_type, start) = marker.to_list_type();
        assert_eq!(list_type, expected_type);
        assert_eq!(start, expected_start);
    }

    // === ListMarker::is_same_type 테스트 ===
    #[rstest]
    // 같은 Bullet 마커
    #[case(ListMarker::Bullet('-'), ListMarker::Bullet('-'), true)]
    #[case(ListMarker::Bullet('+'), ListMarker::Bullet('+'), true)]
    #[case(ListMarker::Bullet('*'), ListMarker::Bullet('*'), true)]
    // 다른 Bullet 마커
    #[case(ListMarker::Bullet('-'), ListMarker::Bullet('+'), false)]
    #[case(ListMarker::Bullet('-'), ListMarker::Bullet('*'), false)]
    // 같은 Ordered 마커 (delimiter만 비교, start는 무관)
    #[case(ListMarker::Ordered { start: 1, delimiter: '.' }, ListMarker::Ordered { start: 1, delimiter: '.' }, true)]
    #[case(ListMarker::Ordered { start: 1, delimiter: '.' }, ListMarker::Ordered { start: 5, delimiter: '.' }, true)]
    #[case(ListMarker::Ordered { start: 1, delimiter: ')' }, ListMarker::Ordered { start: 1, delimiter: ')' }, true)]
    // 다른 Ordered 마커
    #[case(ListMarker::Ordered { start: 1, delimiter: '.' }, ListMarker::Ordered { start: 1, delimiter: ')' }, false)]
    // Bullet과 Ordered 혼합
    #[case(ListMarker::Bullet('-'), ListMarker::Ordered { start: 1, delimiter: '.' }, false)]
    fn test_is_same_type(#[case] a: ListMarker, #[case] b: ListMarker, #[case] expected: bool) {
        assert_eq!(a.is_same_type(&b), expected);
    }

    // =========================================================================
    // 5.2 List Items - 통합 테스트 (Example 253-285)
    // =========================================================================
    #[rstest]
    // Example 253: 리스트 아닌 일반 블록 (대조용)
    #[case("A paragraph\nwith two lines.\n\n    indented code\n\n> A block quote.", vec![
        BlockNode::paragraph(vec![InlineNode::text("A paragraph"), InlineNode::SoftBreak, InlineNode::text("with two lines.")]),
        BlockNode::code_block(None, "indented code"),
        BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("A block quote.")])]),
    ])]
    // Example 254: 아이템 내 paragraph + indented code + blockquote
    #[case("1.  A paragraph\n    with two lines.\n\n        indented code\n\n    > A block quote.", vec![
        BlockNode::ordered_list('.', 1, false, vec![
            ListItemNode::new(vec![
                BlockNode::paragraph(vec![InlineNode::text("A paragraph"), InlineNode::SoftBreak, InlineNode::text("with two lines.")]),
                BlockNode::code_block(None, "indented code"),
                BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("A block quote.")])]),
            ]),
        ])
    ])]
    // Example 255: 들여쓰기 부족 (1칸) → 리스트 종료
    #[case("- one\n\n two", vec![
        BlockNode::bullet_list(true, vec![ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("one")])])]),
        BlockNode::paragraph(vec![InlineNode::text("two")]),
    ])]
    // Example 256: 충분한 들여쓰기 (2칸) → 같은 아이템 두 번째 단락 (loose)
    #[case("- one\n\n  two", vec![
        BlockNode::bullet_list(false, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("one")]), BlockNode::paragraph(vec![InlineNode::text("two")])]),
        ])
    ])]
    // Example 257: content_indent 부족 → 리스트 종료 + indented code
    #[case(" -    one\n\n     two", vec![
        BlockNode::bullet_list(true, vec![ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("one")])])]),
        BlockNode::code_block(None, " two"),
    ])]
    // Example 258: content_indent 충족 → 같은 아이템 (loose)
    #[case(" -    one\n\n      two", vec![
        BlockNode::bullet_list(false, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("one")]), BlockNode::paragraph(vec![InlineNode::text("two")])]),
        ])
    ])]
    // Example 261: 마커 뒤 공백 없으면 paragraph
    #[case("-one", vec![BlockNode::paragraph(vec![InlineNode::text("-one")])])]
    #[case("2.two", vec![BlockNode::paragraph(vec![InlineNode::text("2.two")])])]
    // Example 262: 아이템 내 여러 빈 줄
    #[case("- foo\n\n\n  bar", vec![
        BlockNode::bullet_list(false, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("foo")]), BlockNode::paragraph(vec![InlineNode::text("bar")])]),
        ])
    ])]
    // Example 263: 아이템 내 code fence + paragraph + blockquote
    #[case("1.  foo\n\n    ```\n    bar\n    ```\n\n    baz\n\n    > bam", vec![
        BlockNode::ordered_list('.', 1, false, vec![
            ListItemNode::new(vec![
                BlockNode::paragraph(vec![InlineNode::text("foo")]),
                BlockNode::code_block(None, "bar"),
                BlockNode::paragraph(vec![InlineNode::text("baz")]),
                BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("bam")])]),
            ]),
        ])
    ])]
    // Example 264: 리스트 아이템 내 코드 블록 (빈 줄 보존)
    #[case("- Foo\n\n      bar\n\n\n      baz", vec![
        BlockNode::bullet_list(false, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("Foo")]), BlockNode::code_block(None, "bar\n\n\nbaz")]),
        ])
    ])]
    // Example 265: 9자리 숫자 허용
    #[case("123456789. ok", vec![BlockNode::ordered_list('.', 123456789, true, vec![ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("ok")])])])])]
    // Example 266: 10자리 숫자는 마커 아님 → paragraph
    #[case("1234567890. not ok", vec![BlockNode::paragraph(vec![InlineNode::text("1234567890. not ok")])])]
    // Example 267: 0 시작 허용
    #[case("0. ok", vec![BlockNode::ordered_list('.', 0, true, vec![ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("ok")])])])])]
    // Example 268: 선행 0 허용 (003 → start=3)
    #[case("003. ok", vec![BlockNode::ordered_list('.', 3, true, vec![ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("ok")])])])])]
    // Example 269: 음수는 마커 아님 → paragraph
    #[case("-1. not ok", vec![BlockNode::paragraph(vec![InlineNode::text("-1. not ok")])])]
    // Example 270: 아이템 내 indented code (content_indent=2, 6칸 들여쓰기)
    #[case("- foo\n\n      bar", vec![
        BlockNode::bullet_list(false, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("foo")]), BlockNode::code_block(None, "bar")]),
        ])
    ])]
    // Example 271: ordered 마커 + 아이템 내 indented code
    #[case("  10.  foo\n\n           bar", vec![
        BlockNode::ordered_list('.', 10, false, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("foo")]), BlockNode::code_block(None, "bar")]),
        ])
    ])]
    // Example 272: 리스트 아닌 indented code (대조용)
    #[case("    indented code\n\nparagraph\n\n    more code", vec![
        BlockNode::code_block(None, "indented code"),
        BlockNode::paragraph(vec![InlineNode::text("paragraph")]),
        BlockNode::code_block(None, "more code"),
    ])]
    // Example 275: 리스트 아닌 paragraph (대조용)
    #[case("   foo\n\nbar", vec![
        BlockNode::paragraph(vec![InlineNode::text("foo")]),
        BlockNode::paragraph(vec![InlineNode::text("bar")]),
    ])]
    // Example 276: content_indent 부족 → 리스트 종료
    #[case("-    foo\n\n  bar", vec![
        BlockNode::bullet_list(true, vec![ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("foo")])])]),
        BlockNode::paragraph(vec![InlineNode::text("bar")]),
    ])]
    // Example 277: content_indent 충족 → 같은 아이템 (loose)
    #[case("-  foo\n\n   bar", vec![
        BlockNode::bullet_list(false, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("foo")]), BlockNode::paragraph(vec![InlineNode::text("bar")])]),
        ])
    ])]
    // Example 278: 빈 줄로 시작하는 아이템
    #[case("-\n  foo\n-\n  ```\n  bar\n  ```\n-\n      baz", vec![
        BlockNode::bullet_list(true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("foo")])]),
            ListItemNode::new(vec![BlockNode::code_block(None, "bar")]),
            ListItemNode::new(vec![BlockNode::code_block(None, "baz")]),
        ])
    ])]
    // Example 281: 중간 빈 아이템 (bullet)
    #[case("- foo\n-\n- bar", vec![
        BlockNode::bullet_list(true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("foo")])]),
            ListItemNode::new(vec![]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("bar")])]),
        ])
    ])]
    // Example 282: trailing whitespace 빈 아이템
    #[case("- foo\n-   \n- bar", vec![
        BlockNode::bullet_list(true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("foo")])]),
            ListItemNode::new(vec![]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("bar")])]),
        ])
    ])]
    // Example 283: 중간 빈 아이템 (ordered)
    #[case("1. foo\n2.\n3. bar", vec![
        BlockNode::ordered_list('.', 1, true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("foo")])]),
            ListItemNode::new(vec![]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("bar")])]),
        ])
    ])]
    // Example 284: 단일 빈 아이템
    #[case("*", vec![BlockNode::bullet_list(true, vec![ListItemNode::new(vec![])])])]
    // Example 285: 빈 아이템은 paragraph 인터럽트 불가
    #[case("foo\n*", vec![BlockNode::paragraph(vec![InlineNode::text("foo"), InlineNode::SoftBreak, InlineNode::text("*")])])]
    #[case("foo\n1.", vec![BlockNode::paragraph(vec![InlineNode::text("foo"), InlineNode::SoftBreak, InlineNode::text("1.")])])]
    // 추가 케이스: 단일 아이템 (마커 종류별)
    #[case("- item", vec![BlockNode::bullet_list(true, vec![ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("item")])])])])]
    #[case("1. item", vec![BlockNode::ordered_list('.', 1, true, vec![ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("item")])])])])]
    #[case("1) item", vec![BlockNode::ordered_list(')', 1, true, vec![ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("item")])])])])]
    // 추가 케이스: Ordered 시작 번호
    #[case("5. item", vec![BlockNode::ordered_list('.', 5, true, vec![ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("item")])])])])]
    #[case("10. item", vec![BlockNode::ordered_list('.', 10, true, vec![ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("item")])])])])]
    // 추가 케이스: Continuation line
    #[case("- line1\n  line2\n  line3", vec![BlockNode::bullet_list(true, vec![ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("line1"), InlineNode::SoftBreak, InlineNode::text("line2"), InlineNode::SoftBreak, InlineNode::text("line3")])])])])]
    // 추가 케이스: 빈 아이템 연속 + 빈 줄 = loose
    #[case("-\n\n- foo", vec![
        BlockNode::bullet_list(false, vec![ListItemNode::new(vec![]), ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("foo")])])]),
    ])]
    fn test_list_item(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse_doc(input, Spec::CommonMark);
        assert_eq!(doc.children, expected);
    }

    // =========================================================================
    // 5.2 List Items - 미지원 케이스
    // =========================================================================
    #[rstest]
    // Example 279: 마커 뒤 공백만 있는 빈 줄 시작
    #[case("-   \n  foo", vec![
        BlockNode::bullet_list(true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("foo")])]),
        ])
    ])]
    // Example 280: 빈 줄 2개 → 빈 아이템 + paragraph
    #[case("-\n\n  foo", vec![
        BlockNode::bullet_list(true, vec![ListItemNode::new(vec![])]),
        BlockNode::paragraph(vec![InlineNode::text("foo")]),
    ])]
    // 통과하는 케이스들
    #[rstest]
    // Example 279: 마커 뒤 공백만 있는 빈 줄 시작
    #[case("-   \n  foo", vec![
        BlockNode::bullet_list(true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("foo")])]),
        ])
    ])]
    // Example 280: 빈 줄 2개 → 빈 아이템 + paragraph
    #[case("-\n\n  foo", vec![
        BlockNode::bullet_list(true, vec![ListItemNode::new(vec![])]),
        BlockNode::paragraph(vec![InlineNode::text("foo")]),
    ])]
    // Example 260: blockquote 내부 bullet list
    #[case(">>- one\n>>\n  >  > two", vec![
        BlockNode::blockquote(vec![BlockNode::blockquote(vec![
            BlockNode::bullet_list(true, vec![
                ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("one")])]),
            ]),
            BlockNode::paragraph(vec![InlineNode::text("two")]),
        ])]),
    ])]
    // Example 273: 아이템이 indented code로 시작 (마커 뒤 5칸)
    #[case("1.     indented code\n\n   paragraph\n\n       more code", vec![
        BlockNode::ordered_list('.', 1, false, vec![
            ListItemNode::new(vec![
                BlockNode::code_block(None, "indented code"),
                BlockNode::paragraph(vec![InlineNode::text("paragraph")]),
                BlockNode::code_block(None, "more code"),
            ]),
        ])
    ])]
    // Example 274: 아이템이 indented code로 시작 (마커 뒤 6칸, 여분 공백 1칸)
    #[case("1.      indented code\n\n   paragraph\n\n       more code", vec![
        BlockNode::ordered_list('.', 1, false, vec![
            ListItemNode::new(vec![
                BlockNode::code_block(None, " indented code"),
                BlockNode::paragraph(vec![InlineNode::text("paragraph")]),
                BlockNode::code_block(None, "more code"),
            ]),
        ])
    ])]
    fn test_list_item_resolved(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse_doc(input, Spec::CommonMark);
        assert_eq!(doc.children, expected);
    }

    // Example 259: nested blockquote 내 ordered list — 복잡한 중첩
    #[rstest]
    #[case("   > > 1.  one\n>>\n>>     two", vec![
        BlockNode::blockquote(vec![BlockNode::blockquote(vec![
            BlockNode::ordered_list('.', 1, false, vec![
                ListItemNode::new(vec![
                    BlockNode::paragraph(vec![InlineNode::text("one")]),
                    BlockNode::paragraph(vec![InlineNode::text("two")]),
                ]),
            ]),
        ])]),
    ])]
    fn test_list_item_nested_blockquote(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse_doc(input, Spec::CommonMark);
        assert_eq!(doc.children, expected);
    }
}
