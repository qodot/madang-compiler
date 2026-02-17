//! https://spec.commonmark.org/0.31.2/#thematic-breaks

use super::helpers::calculate_indent;
use crate::node::{BlockNode, ThematicBreakNode};

#[derive(Debug, Clone, PartialEq)]
pub enum ThematicBreakErr {
    /// 4칸 이상 들여쓰기 (코드 블록으로 해석됨)
    CodeBlockIndented,
    /// 빈 줄
    Empty,
    /// 유효하지 않은 마커 문자 (*, -, _ 아님)
    InvalidMarker,
    /// 마커 개수 부족 (3개 미만)
    InsufficientMarkers,
    /// 다른 문자 섞임 (공백/탭 외)
    MixedCharacters,
}

pub fn parse(line: &str) -> Result<BlockNode, ThematicBreakErr> {
    let indent = calculate_indent(line);
    let trimmed = line.trim();

    // 들여쓰기 3칸 초과면 코드 블록
    if indent > 3 {
        return Err(ThematicBreakErr::CodeBlockIndented);
    }

    // 빈 줄
    if trimmed.is_empty() {
        return Err(ThematicBreakErr::Empty);
    }

    // 첫 문자가 마커 문자인지 확인
    let first = trimmed.chars().next().unwrap();
    if first != '*' && first != '-' && first != '_' {
        return Err(ThematicBreakErr::InvalidMarker);
    }

    // 모든 문자가 같은 마커이거나 공백/탭인지 확인
    let mut marker_count = 0;
    for c in trimmed.chars() {
        if c == first {
            marker_count += 1;
        } else if c != ' ' && c != '\t' {
            return Err(ThematicBreakErr::MixedCharacters);
        }
    }

    // 마커가 3개 이상이어야 함
    if marker_count < 3 {
        return Err(ThematicBreakErr::InsufficientMarkers);
    }

    Ok(BlockNode::ThematicBreak(ThematicBreakNode))
}

#[cfg(test)]
mod tests {
    use crate::node::{BlockNode, InlineNode};
    use crate::parser::parse;
    use rstest::rstest;

    #[rstest]
    // Example 43: 기본 케이스
    #[case("***", vec![BlockNode::thematic_break()])]
    #[case("---", vec![BlockNode::thematic_break()])]
    #[case("___", vec![BlockNode::thematic_break()])]
    // Example 44: 유효하지 않은 마커 문자 → Paragraph
    #[case("+++", vec![BlockNode::paragraph(vec![InlineNode::text("+++")])])]
    // Example 45: 유효하지 않은 마커 문자 → Paragraph
    #[case("===", vec![BlockNode::paragraph(vec![InlineNode::text("===")])])]
    // Example 46: 2개는 부족 → Paragraph
    #[case("**", vec![BlockNode::paragraph(vec![InlineNode::text("**")])])]
    #[case("--", vec![BlockNode::paragraph(vec![InlineNode::text("--")])])]
    #[case("__", vec![BlockNode::paragraph(vec![InlineNode::text("__")])])]
    // Example 47: 1-3칸 들여쓰기 허용
    #[case(" ***", vec![BlockNode::thematic_break()])]
    #[case("  ***", vec![BlockNode::thematic_break()])]
    #[case("   ***", vec![BlockNode::thematic_break()])]
    // Example 48: 4칸 들여쓰기는 코드 블록
    #[case("    ***", vec![BlockNode::code_block(None, "***")])]
    // Example 49: Paragraph 내 4칸 들여쓰기는 continuation
    #[case("Foo\n    ***", vec![BlockNode::paragraph(vec![InlineNode::text("Foo"), InlineNode::SoftBreak, InlineNode::text("***")])])]
    // Example 50: 많은 문자
    #[case("_____________________________________", vec![BlockNode::thematic_break()])]
    // Example 51: 공백 사이
    #[case(" - - -", vec![BlockNode::thematic_break()])]
    // Example 52: 복잡한 공백 패턴
    #[case(" **  * ** * ** * **", vec![BlockNode::thematic_break()])]
    // Example 53: 많은 공백
    #[case("-     -      -      -", vec![BlockNode::thematic_break()])]
    // Example 54: 끝 공백
    #[case("- - - -    ", vec![BlockNode::thematic_break()])]
    // Example 55: 다른 문자 포함 시 무효 → Paragraph
    #[case("_ _ _ _ a", vec![BlockNode::paragraph(vec![InlineNode::text("_ _ _ _ a")])])]
    #[case("a------", vec![BlockNode::paragraph(vec![InlineNode::text("a------")])])]
    #[case("---a---", vec![BlockNode::paragraph(vec![InlineNode::text("---a---")])])]
    // Example 56: 혼합 문자는 무효 → Paragraph
    #[case("*-*", vec![BlockNode::paragraph(vec![InlineNode::emphasis(vec![InlineNode::text("-")])])])]
    // 추가 케이스
    #[case("*****", vec![BlockNode::thematic_break()])]
    #[case("----------", vec![BlockNode::thematic_break()])]
    #[case("* * *", vec![BlockNode::thematic_break()])]
    #[case("- - -", vec![BlockNode::thematic_break()])]
    #[case("_  _  _", vec![BlockNode::thematic_break()])]
    #[case("  ---", vec![BlockNode::thematic_break()])]
    #[case("   ___", vec![BlockNode::thematic_break()])]
    #[case("***   ", vec![BlockNode::thematic_break()])]
    #[case("***a", vec![BlockNode::paragraph(vec![InlineNode::text("***a")])])]
    fn test_thematic_break(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse(input);
        assert_eq!(doc.children, expected);
    }

    /// 탭 관련 thematic break 테스트 (CommonMark 명세 Section 2.2 Tabs)
    #[rstest]
    // Example 11: 탭으로 구분된 thematic break 마커
    #[case("*\t*\t*\t", vec![BlockNode::thematic_break()])]
    
    fn test_thematic_break_tabs(#[case] _input: &str, #[case] _expected: Vec<BlockNode>) {
    }
}
