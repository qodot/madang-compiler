//! Line Break 파서
//!
//! Hard line break와 soft line break를 처리합니다.
//! CommonMark 명세:
//! - Section 6.7: https://spec.commonmark.org/0.31.2/#hard-line-breaks
//! - Section 6.8: https://spec.commonmark.org/0.31.2/#soft-line-breaks

/// 텍스트 노드 끝의 trailing spaces 수를 센다
pub fn count_trailing_spaces(text: &str) -> usize {
    text.chars().rev().take_while(|&c| c == ' ').count()
}

/// 텍스트 노드 끝에서 trailing spaces를 제거한다
pub fn strip_trailing_spaces(text: &str) -> &str {
    text.trim_end_matches(' ')
}

/// 줄바꿈 뒤의 leading spaces를 건너뛴 바이트 수를 반환한다
pub fn skip_leading_spaces(text: &str) -> usize {
    text.chars().take_while(|&c| c == ' ').count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("foo", 0)]
    #[case("foo ", 1)]
    #[case("foo  ", 2)]
    #[case("foo   ", 3)]
    #[case("", 0)]
    #[case("  ", 2)]
    fn test_count_trailing_spaces(#[case] input: &str, #[case] expected: usize) {
        assert_eq!(count_trailing_spaces(input), expected);
    }

    #[rstest]
    #[case("foo  ", "foo")]
    #[case("foo", "foo")]
    #[case("  ", "")]
    fn test_strip_trailing_spaces(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(strip_trailing_spaces(input), expected);
    }

    #[rstest]
    #[case("bar", 0)]
    #[case("  bar", 2)]
    #[case("     bar", 5)]
    #[case("", 0)]
    fn test_skip_leading_spaces(#[case] input: &str, #[case] expected: usize) {
        assert_eq!(skip_leading_spaces(input), expected);
    }
}
