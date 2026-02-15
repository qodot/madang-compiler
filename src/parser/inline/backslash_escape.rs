//! Backslash Escape 파서
//!
//! ASCII 구두점 문자 앞의 백슬래시를 이스케이프 처리합니다.
//! CommonMark 명세 Section 2.4: https://spec.commonmark.org/0.31.2/#backslash-escapes

/// ASCII 구두점 문자인지 확인
///
/// CommonMark 명세:
/// !, ", #, $, %, &, ', (, ), *, +, ,, -, ., / (U+0021–2F)
/// :, ;, <, =, >, ?, @ (U+003A–0040)
/// [, \, ], ^, _, ` (U+005B–0060)
/// {, |, }, ~ (U+007B–007E)
pub fn is_ascii_punctuation(c: char) -> bool {
    matches!(c,
        '!'..='/' | ':'..='@' | '['..='`' | '{'..='~'
    )
}

/// 백슬래시 이스케이프를 시도한다
///
/// `input`은 `\`로 시작해야 한다.
/// 다음 문자가 ASCII 구두점이면 Some((이스케이프된 문자, 소비 바이트 수))를 반환한다.
pub fn try_escape(input: &str) -> Option<(char, usize)> {
    let mut chars = input.chars();

    // 첫 문자가 \ 인지 확인
    if chars.next() != Some('\\') {
        return None;
    }

    // 다음 문자가 ASCII 구두점인지 확인
    let next = chars.next()?;
    if is_ascii_punctuation(next) {
        Some((next, 1 + next.len_utf8()))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // =========================================================================
    // is_ascii_punctuation 테스트
    // =========================================================================

    #[rstest]
    // U+0021–002F
    #[case('!', true)]
    #[case('"', true)]
    #[case('#', true)]
    #[case('$', true)]
    #[case('%', true)]
    #[case('&', true)]
    #[case('\'', true)]
    #[case('(', true)]
    #[case(')', true)]
    #[case('*', true)]
    #[case('+', true)]
    #[case(',', true)]
    #[case('-', true)]
    #[case('.', true)]
    #[case('/', true)]
    // U+003A–0040
    #[case(':', true)]
    #[case(';', true)]
    #[case('<', true)]
    #[case('=', true)]
    #[case('>', true)]
    #[case('?', true)]
    #[case('@', true)]
    // U+005B–0060
    #[case('[', true)]
    #[case('\\', true)]
    #[case(']', true)]
    #[case('^', true)]
    #[case('_', true)]
    #[case('`', true)]
    // U+007B–007E
    #[case('{', true)]
    #[case('|', true)]
    #[case('}', true)]
    #[case('~', true)]
    // 구두점이 아닌 것
    #[case('a', false)]
    #[case('A', false)]
    #[case('0', false)]
    #[case(' ', false)]
    #[case('\t', false)]
    #[case('φ', false)]
    #[case('«', false)]
    fn test_is_ascii_punctuation(#[case] c: char, #[case] expected: bool) {
        assert_eq!(is_ascii_punctuation(c), expected);
    }

    // =========================================================================
    // try_escape 테스트
    // =========================================================================

    #[rstest]
    // ASCII 구두점 이스케이프
    #[case("\\!", Some(('!', 2)))]
    #[case("\\*", Some(('*', 2)))]
    #[case("\\[", Some(('[', 2)))]
    #[case("\\\\", Some(('\\', 2)))]
    #[case("\\`", Some(('`', 2)))]
    #[case("\\~", Some(('~', 2)))]
    // 구두점 아닌 문자 → None (리터럴 백슬래시)
    #[case("\\A", None)]
    #[case("\\a", None)]
    #[case("\\ ", None)]
    #[case("\\3", None)]
    #[case("\\φ", None)]
    #[case("\\«", None)]
    // \ 뒤에 아무것도 없음 → None
    #[case("\\", None)]
    // \ 로 시작하지 않음
    #[case("abc", None)]
    // 이스케이프 후 나머지 문자 (소비 바이트 확인)
    #[case("\\!rest", Some(('!', 2)))]
    fn test_try_escape(#[case] input: &str, #[case] expected: Option<(char, usize)>) {
        assert_eq!(try_escape(input), expected);
    }
}
