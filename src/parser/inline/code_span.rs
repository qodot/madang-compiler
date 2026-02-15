//! Code Span 파서
//!
//! 백틱(`)으로 감싼 인라인 코드를 파싱합니다.
//! CommonMark 명세 Section 6.1: https://spec.commonmark.org/0.31.2/#code-spans

/// Code span 파싱 결과
#[derive(Debug, Clone, PartialEq)]
pub struct CodeSpanResult {
    /// code span 내용 (strip 완료)
    pub content: String,
    /// 원본 문자열에서 소비한 바이트 수 (여는 백틱부터 닫는 백틱 끝까지)
    pub bytes_consumed: usize,
}

/// 백틱 시퀀스의 길이를 센다
fn count_backticks(s: &str) -> usize {
    s.chars().take_while(|&c| c == '`').count()
}

/// code span 내용의 공백을 정규화한다
///
/// 명세 규칙:
/// - 줄바꿈은 공백으로 변환
/// - 내용이 공백만으로 이루어지지 않았고, 앞뒤가 모두 공백이면 공백 1개씩 제거
fn normalize_content(raw: &str) -> String {
    // 줄바꿈 → 공백
    let collapsed = raw.replace('\n', " ");

    // 앞뒤 공백 strip 규칙: 앞뒤 모두 공백이고, 내용이 공백만이 아니면 1개씩 제거
    if collapsed.len() >= 2
        && collapsed.starts_with(' ')
        && collapsed.ends_with(' ')
        && !collapsed.chars().all(|c| c == ' ')
    {
        collapsed[1..collapsed.len() - 1].to_string()
    } else {
        collapsed
    }
}

/// 주어진 위치에서 code span을 파싱한다
///
/// `input`은 백틱 시퀀스로 시작해야 한다.
/// 매칭되는 닫는 백틱 시퀀스를 찾으면 Some을 반환한다.
pub fn parse_code_span(input: &str) -> Option<CodeSpanResult> {
    let open_len = count_backticks(input);
    if open_len == 0 {
        return None;
    }

    let after_open = &input[open_len..];
    let mut pos = 0;

    while pos < after_open.len() {
        // 다음 백틱 찾기
        let remaining = &after_open[pos..];
        let next_backtick = remaining.find('`')?;
        pos += next_backtick;

        // 닫는 백틱 시퀀스 길이 세기
        let close_len = count_backticks(&after_open[pos..]);

        if close_len == open_len {
            // 매칭! 내용 추출
            let raw_content = &after_open[..pos];
            let content = normalize_content(raw_content);
            let bytes_consumed = open_len + pos + close_len;
            return Some(CodeSpanResult {
                content,
                bytes_consumed,
            });
        }

        // 매칭 안 되면 이 백틱 시퀀스를 건너뛰기
        pos += close_len;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // =========================================================================
    // count_backticks 테스트
    // =========================================================================

    #[rstest]
    #[case("`foo", 1)]
    #[case("``foo", 2)]
    #[case("```foo", 3)]
    #[case("foo", 0)]
    #[case("", 0)]
    fn test_count_backticks(#[case] input: &str, #[case] expected: usize) {
        assert_eq!(count_backticks(input), expected);
    }

    // =========================================================================
    // normalize_content 테스트
    // =========================================================================

    #[rstest]
    #[case("foo", "foo")]
    // 줄바꿈 → 공백
    #[case("foo\nbar", "foo bar")]
    // 앞뒤 공백 strip (내용이 공백만이 아닌 경우)
    #[case(" foo ", "foo")]
    #[case(" `` ", "``")]
    // 공백만인 경우 strip 안 함
    #[case(" ", " ")]
    #[case("  ", "  ")]
    // 앞만 공백 → strip 안 함
    #[case(" foo", " foo")]
    // 뒤만 공백 → strip 안 함
    #[case("foo ", "foo ")]
    // 줄바꿈 + strip
    #[case("\nfoo\n", "foo")]
    fn test_normalize_content(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(normalize_content(input), expected);
    }

    // =========================================================================
    // parse_code_span 테스트 — CommonMark Examples
    // =========================================================================

    #[rstest]
    // Example 328: 기본 code span
    #[case("`foo`", Some(("foo".to_string(), 5)))]
    // Example 329: 백틱 2개, 내부에 백틱 1개
    #[case("`` foo ` bar ``", Some(("foo ` bar".to_string(), 15)))]
    // Example 330: 백틱 1개, 내부에 백틱 2개
    #[case("` `` `", Some(("``".to_string(), 6)))]
    // Example 331: 내부 공백 보존 (앞뒤 strip 후에도 공백 남음)
    #[case("`  ``  `", Some((" `` ".to_string(), 8)))]
    // Example 332: 앞에만 공백 → strip 안 함
    #[case("` a`", Some((" a".to_string(), 4)))]
    // Example 335: 줄바꿈 → 공백으로 변환
    #[case("``\nfoo\nbar  \nbaz\n``", Some(("foo bar   baz".to_string(), 19)))]
    // Example 336: 줄바꿈 후 strip
    #[case("``\nfoo \n``", Some(("foo ".to_string(), 10)))]
    // Example 337: 내부 공백 유지 (줄바꿈 → 공백)
    #[case("`foo   bar \nbaz`", Some(("foo   bar  baz".to_string(), 16)))]
    // Example 338: 백슬래시는 이스케이프 안 됨 — `foo\` 와 bar` 중 첫 매칭
    #[case("`foo\\`bar`", Some(("foo\\".to_string(), 6)))]
    // Example 339: 백틱 2개, 내부에 백틱 1개
    #[case("``foo`bar``", Some(("foo`bar".to_string(), 11)))]
    // Example 340: 백틱 1개, 내부에 백틱 2개
    #[case("` foo `` bar `", Some(("foo `` bar".to_string(), 14)))]
    // Example 347: 닫는 백틱 시퀀스가 없음 (3개로 열었는데 2개만)
    #[case("```foo``", None)]
    // Example 348: 닫는 백틱 없음
    #[case("`foo", None)]
    // Example 349: `foo 매칭 안 됨, ``bar`` 매칭
    // 이 케이스는 parse_code_span이 첫 번째 ` 시퀀스만 보므로 None
    // (전체 인라인 파서에서 fallback으로 처리)
    #[case("`foo``bar``", None)]
    fn test_parse_code_span(#[case] input: &str, #[case] expected: Option<(String, usize)>) {
        let result = parse_code_span(input);
        match expected {
            Some((content, len)) => {
                let r = result.unwrap();
                assert_eq!(r.content, content);
                assert_eq!(r.bytes_consumed, len);
            }
            None => assert!(result.is_none()),
        }
    }
}
