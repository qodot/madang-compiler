//! Setext Heading 파서
//!
//! 밑줄 스타일의 제목을 처리합니다.
//! - `=` 밑줄: 레벨 1
//! - `-` 밑줄: 레벨 2

// =============================================================================
// 타입 정의
// =============================================================================

/// Setext 밑줄 레벨
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SetextLevel {
    /// `=` 밑줄 → 레벨 1
    Level1,
    /// `-` 밑줄 → 레벨 2
    Level2,
}

impl SetextLevel {
    /// 숫자 레벨로 변환 (Heading의 level 필드용)
    pub fn to_level(self) -> u8 {
        match self {
            SetextLevel::Level1 => 1,
            SetextLevel::Level2 => 2,
        }
    }
}

/// Setext Heading 밑줄 시작 정보
#[derive(Debug, Clone, PartialEq)]
pub struct HeadingSetextStart {
    /// 밑줄 레벨 (1 또는 2)
    pub level: SetextLevel,
}

/// Setext 밑줄 감지 성공 사유
#[derive(Debug, Clone, PartialEq)]
pub enum HeadingSetextStartReason {
    /// 유효한 밑줄 발견
    Started(HeadingSetextStart),
}

/// Setext 밑줄 아님 사유
#[derive(Debug, Clone, PartialEq)]
pub enum HeadingSetextNotStartReason {
    /// 4칸 이상 들여쓰기 (코드 블록으로 해석됨)
    CodeBlockIndented,
    /// 빈 줄
    Empty,
    /// 밑줄 문자(=, -)가 아님
    NotUnderlineChar,
    /// 문자가 섞임 (예: "=-=")
    MixedChars,
}

// =============================================================================
// 함수
// =============================================================================

/// Setext 밑줄인지 확인하고 레벨 반환
///
/// - `Ok(HeadingSetextStartReason::Started(start))`: 유효한 밑줄
/// - `Err(reason)`: 유효하지 않음
pub fn try_start(line: &str, indent: usize) -> Result<HeadingSetextStartReason, HeadingSetextNotStartReason> {
    // 들여쓰기 4칸 이상이면 코드 블록
    if indent > 3 {
        return Err(HeadingSetextNotStartReason::CodeBlockIndented);
    }

    // 후행 공백 제거
    let trimmed = line.trim_end();

    // 빈 문자열은 밑줄이 아님
    if trimmed.is_empty() {
        return Err(HeadingSetextNotStartReason::Empty);
    }

    // 첫 문자로 레벨 결정
    let first = trimmed.chars().next().unwrap();
    let level = match first {
        '=' => SetextLevel::Level1,
        '-' => SetextLevel::Level2,
        _ => return Err(HeadingSetextNotStartReason::NotUnderlineChar),
    };

    // 모든 문자가 같은지 확인
    if trimmed.chars().all(|c| c == first) {
        Ok(HeadingSetextStartReason::Started(HeadingSetextStart { level }))
    } else {
        Err(HeadingSetextNotStartReason::MixedChars)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::{BlockNode, InlineNode};
    use rstest::rstest;

    /// try_start 유닛 테스트
    #[rstest]
    // 유효한 밑줄: 레벨 1 (=)
    #[case("=", 0, Ok(SetextLevel::Level1))]
    #[case("==", 0, Ok(SetextLevel::Level1))]
    #[case("===", 0, Ok(SetextLevel::Level1))]
    #[case("==========", 0, Ok(SetextLevel::Level1))]
    #[case("===   ", 0, Ok(SetextLevel::Level1))]
    // 유효한 밑줄: 레벨 2 (-)
    #[case("-", 0, Ok(SetextLevel::Level2))]
    #[case("--", 0, Ok(SetextLevel::Level2))]
    #[case("---", 0, Ok(SetextLevel::Level2))]
    #[case("----------", 0, Ok(SetextLevel::Level2))]
    #[case("---   ", 0, Ok(SetextLevel::Level2))]
    // 들여쓰기: 0-3칸 허용
    #[case("===", 1, Ok(SetextLevel::Level1))]
    #[case("===", 2, Ok(SetextLevel::Level1))]
    #[case("===", 3, Ok(SetextLevel::Level1))]
    // 무효: 4칸 이상 들여쓰기 → CodeBlockIndented
    #[case("===", 4, Err(HeadingSetextNotStartReason::CodeBlockIndented))]
    #[case("===", 5, Err(HeadingSetextNotStartReason::CodeBlockIndented))]
    // 무효: 빈 줄 → Empty
    #[case("", 0, Err(HeadingSetextNotStartReason::Empty))]
    #[case("   ", 0, Err(HeadingSetextNotStartReason::Empty))]
    // 무효: 밑줄 문자가 아님 → NotUnderlineChar
    #[case("abc", 0, Err(HeadingSetextNotStartReason::NotUnderlineChar))]
    #[case("###", 0, Err(HeadingSetextNotStartReason::NotUnderlineChar))]
    // 무효: 문자 섞임 → MixedChars
    #[case("= =", 0, Err(HeadingSetextNotStartReason::MixedChars))]
    #[case("- -", 0, Err(HeadingSetextNotStartReason::MixedChars))]
    #[case("== ==", 0, Err(HeadingSetextNotStartReason::MixedChars))]
    #[case("=-=", 0, Err(HeadingSetextNotStartReason::MixedChars))]
    #[case("==-", 0, Err(HeadingSetextNotStartReason::MixedChars))]
    // 무효: 밑줄 뒤 비공백 문자 → MixedChars
    #[case("=== bar", 0, Err(HeadingSetextNotStartReason::MixedChars))]
    #[case("--- foo", 0, Err(HeadingSetextNotStartReason::MixedChars))]
    fn test_try_start(
        #[case] line: &str,
        #[case] indent: usize,
        #[case] expected: Result<SetextLevel, HeadingSetextNotStartReason>,
    ) {
        let result = try_start(line, indent);
        match expected {
            Ok(level) => assert_eq!(
                result,
                Ok(HeadingSetextStartReason::Started(HeadingSetextStart { level }))
            ),
            Err(reason) => assert_eq!(result, Err(reason)),
        }
    }

    /// Setext Heading 통합 테스트
    #[rstest]
    // Example 80: 기본 setext heading
    #[case("Foo\n=========", vec![BlockNode::heading(1, vec![InlineNode::text("Foo")])])]
    #[case("Foo\n---------", vec![BlockNode::heading(2, vec![InlineNode::text("Foo")])])]
    // Example 81: 여러 줄 제목
    #[case("Foo\nbar\n===", vec![BlockNode::heading(1, vec![InlineNode::text("Foo\nbar")])])]
    #[case("Foo\nbar\nbaz\n---", vec![BlockNode::heading(2, vec![InlineNode::text("Foo\nbar\nbaz")])])]
    // Example 83: 다양한 밑줄 길이
    #[case("Foo\n-------------------------", vec![BlockNode::heading(2, vec![InlineNode::text("Foo")])])]
    #[case("Foo\n=", vec![BlockNode::heading(1, vec![InlineNode::text("Foo")])])]
    // Example 84: 제목 내용에 0-3칸 들여쓰기 허용, 밑줄과 정렬 불필요
    #[case("   Foo\n---", vec![BlockNode::heading(2, vec![InlineNode::text("Foo")])])]
    #[case("  Foo\n-----", vec![BlockNode::heading(2, vec![InlineNode::text("Foo")])])]
    #[case("  Foo\n  ===", vec![BlockNode::heading(1, vec![InlineNode::text("Foo")])])]
    // Example 85: 4칸 들여쓰기는 코드 블록
    #[case("    Foo\n    ---", vec![BlockNode::code_block(None, "Foo\n---")])]
    #[case("    Foo\n---", vec![BlockNode::code_block(None, "Foo"), BlockNode::thematic_break()])]
    // Example 86: 밑줄에 1-3칸 들여쓰기 허용
    #[case("Foo\n   ----", vec![BlockNode::heading(2, vec![InlineNode::text("Foo")])])]
    // Example 87: 밑줄에 4칸 들여쓰기는 너무 많음
    #[case("Foo\n    ---", vec![BlockNode::paragraph(vec![InlineNode::text("Foo\n---")])])]
    // Example 88: 밑줄에 내부 공백 불허
    #[case("Foo\n= =", vec![BlockNode::paragraph(vec![InlineNode::text("Foo\n= =")])])]
    #[case("Foo\n--- -", vec![BlockNode::paragraph(vec![InlineNode::text("Foo")]), BlockNode::thematic_break()])]
    // Example 89: 제목 내용 뒤 trailing spaces는 hard line break 아님
    #[case("Foo  \n-----", vec![BlockNode::heading(2, vec![InlineNode::text("Foo")])])]
    // Example 90: 제목 내용 뒤 backslash
    #[case("Foo\\\n----", vec![BlockNode::heading(2, vec![InlineNode::text("Foo\\")])])]
    // Example 92: setext 밑줄은 blockquote의 lazy continuation 불가
    #[case("> Foo\n---", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("Foo")])]), BlockNode::thematic_break()])]
    // Example 94: setext 밑줄은 list item의 lazy continuation 불가
    #[case("- Foo\n---", vec![BlockNode::bullet_list(true, vec![crate::node::ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("Foo")])])]), BlockNode::thematic_break()])]
    // Example 95: paragraph와 setext heading 사이에 빈 줄 불필요 (paragraph가 heading 내용이 됨)
    #[case("Foo\nBar\n---", vec![BlockNode::heading(2, vec![InlineNode::text("Foo\nBar")])])]
    // Example 96: setext heading 전후에 빈 줄 불필요
    #[case("---\nFoo\n---\nBar\n---\nBaz", vec![BlockNode::thematic_break(), BlockNode::heading(2, vec![InlineNode::text("Foo")]), BlockNode::heading(2, vec![InlineNode::text("Bar")]), BlockNode::paragraph(vec![InlineNode::text("Baz")])])]
    // Example 97: setext heading은 비어있을 수 없음
    #[case("====", vec![BlockNode::paragraph(vec![InlineNode::text("====")])])]
    // Example 98: setext 텍스트 줄이 다른 블록 구조로 해석되면 밑줄은 thematic break
    #[case("---\n---", vec![BlockNode::thematic_break(), BlockNode::thematic_break()])]
    #[case("- foo\n-----", vec![BlockNode::bullet_list(true, vec![crate::node::ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("foo")])])]), BlockNode::thematic_break()])]
    #[case("    foo\n---", vec![BlockNode::code_block(None, "foo"), BlockNode::thematic_break()])]
    #[case("> foo\n-----", vec![BlockNode::blockquote(vec![BlockNode::paragraph(vec![InlineNode::text("foo")])]), BlockNode::thematic_break()])]
    // Example 100: paragraph 뒤 빈 줄로 분리하면 별개
    #[case("Foo\n\nbar\n---\nbaz", vec![BlockNode::paragraph(vec![InlineNode::text("Foo")]), BlockNode::heading(2, vec![InlineNode::text("bar")]), BlockNode::paragraph(vec![InlineNode::text("baz")])])]
    // Example 101: 빈 줄로 thematic break 분리
    #[case("Foo\nbar\n\n---\n\nbaz", vec![BlockNode::paragraph(vec![InlineNode::text("Foo\nbar")]), BlockNode::thematic_break(), BlockNode::paragraph(vec![InlineNode::text("baz")])])]
    // Example 102: * * *는 setext 밑줄이 될 수 없음
    #[case("Foo\nbar\n* * *\nbaz", vec![BlockNode::paragraph(vec![InlineNode::text("Foo\nbar")]), BlockNode::thematic_break(), BlockNode::paragraph(vec![InlineNode::text("baz")])])]
    // 추가 케이스
    #[case("Foo\n-", vec![BlockNode::heading(2, vec![InlineNode::text("Foo")])])]
    #[case("Foo\n----------", vec![BlockNode::heading(2, vec![InlineNode::text("Foo")])])]
    #[case("Foo\n===   ", vec![BlockNode::heading(1, vec![InlineNode::text("Foo")])])]
    #[case("Foo\n---   ", vec![BlockNode::heading(2, vec![InlineNode::text("Foo")])])]
    #[case(" Foo\n===", vec![BlockNode::heading(1, vec![InlineNode::text("Foo")])])]
    #[case("  Foo\n===", vec![BlockNode::heading(1, vec![InlineNode::text("Foo")])])]
    // Setext Heading이 아닌 케이스
    // 밑줄에 4칸 이상 들여쓰기 → Paragraph continuation
    #[case("Foo\n    ===", vec![BlockNode::paragraph(vec![InlineNode::text("Foo\n===")])])]
    // 빈 줄 후 밑줄 → 두 개의 Paragraph
    #[case("Foo\n\n===", vec![BlockNode::paragraph(vec![InlineNode::text("Foo")]), BlockNode::paragraph(vec![InlineNode::text("===")])])]
    // 밑줄만 단독 (=) → Paragraph
    #[case("===", vec![BlockNode::paragraph(vec![InlineNode::text("===")])])]
    // 밑줄만 단독 (-) → Thematic Break
    #[case("---", vec![BlockNode::thematic_break()])]
    // 밑줄 뒤 비공백 문자 → Paragraph continuation
    #[case("Foo\n=== bar", vec![BlockNode::paragraph(vec![InlineNode::text("Foo\n=== bar")])])]
    fn test_setext_heading(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = crate::parse(input);
        assert_eq!(doc.children, expected);
    }

    /// Setext Heading - inline 파싱 필요한 Example (현재 미지원)
    #[rstest]
    // Example 80: emphasis in setext heading (inline 파싱 필요)
    // #[case("Foo *bar*\n=========", "<h1>Foo <em>bar</em></h1>")]
    // Example 81: multi-line emphasis (inline 파싱 필요)
    // #[case("Foo *bar\nbaz*\n====", "<h1>Foo <em>bar\nbaz</em></h1>")]
    // Example 82: indented content with tab (inline 파싱 + 탭 처리 필요)
    // #[case("  Foo *bar\nbaz*\t\n====", "<h1>Foo <em>bar\nbaz</em></h1>")]
    // Example 91: block structure takes precedence over inline (inline code 필요)
    // #[case("`Foo\n----\n`", "<h2>`Foo</h2><p>`</p>")]
    // Example 93: lazy continuation in blockquote (복잡한 blockquote 처리 필요)
    // #[case("> foo\nbar\n===", "<blockquote><p>foo\nbar\n===</p></blockquote>")]
    // Example 99: backslash escape (inline 파싱 필요)
    // #[case("\\> foo\n------", "<h2>&gt; foo</h2>")]
    // Example 103: backslash escape prevents setext (inline 파싱 필요)
    // #[case("Foo\nbar\n\\---\nbaz", "<p>Foo\nbar\n---\nbaz</p>")]
    #[case("placeholder", "placeholder")]
    #[ignore = "inline 파싱 미구현으로 보류"]
    fn test_setext_heading_pending(#[case] _input: &str, #[case] _expected: &str) {
        // inline 파싱 구현 후 활성화
    }

    // === SetextLevel::to_level 테스트 ===
    #[rstest]
    #[case(SetextLevel::Level1, 1)]
    #[case(SetextLevel::Level2, 2)]
    fn test_setext_level_to_level(#[case] level: SetextLevel, #[case] expected: u8) {
        assert_eq!(level.to_level(), expected);
    }
}
