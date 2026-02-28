//! 파서 공통 헬퍼 함수

/// 문자열 앞에서 특정 문자가 연속으로 몇 개 있는지 세기
pub(crate) fn count_leading_char(s: &str, c: char) -> usize {
    s.chars().take_while(|&ch| ch == c).count()
}

/// 들여쓰기 계산 (tab stop 고려, 4칸 단위)
pub(crate) fn calculate_indent(s: &str) -> usize {
    let mut col = 0;
    for c in s.chars() {
        match c {
            ' ' => col += 1,
            '\t' => col += 4 - (col % 4),
            _ => break,
        }
    }
    col
}

/// n columns만큼 leading whitespace를 소비하고 나머지를 반환
/// 탭이 partially consumed되면 남은 부분을 spaces로 채움
pub(crate) fn consume_indent(s: &str, n: usize) -> String {
    let mut col = 0;
    let mut chars = s.chars().peekable();

    while let Some(&c) = chars.peek() {
        if col >= n {
            break;
        }
        match c {
            ' ' => {
                col += 1;
                chars.next();
            }
            '\t' => {
                let tab_width = 4 - (col % 4);
                if col + tab_width > n {
                    // 탭이 partially consumed — 남은 cols를 spaces로
                    let remaining_spaces = (col + tab_width) - n;
                    chars.next();
                    let mut result = " ".repeat(remaining_spaces);
                    result.extend(chars);
                    return result;
                }
                col += tab_width;
                chars.next();
            }
            _ => break,
        }
    }

    chars.collect()
}

/// start_col 위치에서 시작하여 n columns만큼 leading whitespace를 소비하고 나머지를 반환
/// 탭이 partially consumed되면 남은 부분을 spaces로 채움
/// 소비 후 남은 leading whitespace도 spaces로 확장 (탭 스톱 정렬 보존)
pub(crate) fn consume_indent_from_col(s: &str, n: usize, start_col: usize) -> String {
    let mut col = start_col;
    let target = start_col + n;
    let mut chars = s.chars().peekable();

    // Phase 1: n columns 소비
    while let Some(&c) = chars.peek() {
        if col >= target {
            break;
        }
        match c {
            ' ' => {
                col += 1;
                chars.next();
            }
            '\t' => {
                let tab_width = 4 - (col % 4);
                if col + tab_width > target {
                    // 탭이 partially consumed
                    col = col + tab_width;
                    chars.next();
                    break;
                }
                col += tab_width;
                chars.next();
            }
            _ => break,
        }
    }

    // Phase 2: 남은 leading whitespace를 spaces로 확장 (탭 스톱 정렬 보존)
    let mut result = " ".repeat(col - target);
    while let Some(&c) = chars.peek() {
        match c {
            ' ' => {
                result.push(' ');
                col += 1;
                chars.next();
            }
            '\t' => {
                let tab_width = 4 - (col % 4);
                result.push_str(&" ".repeat(tab_width));
                col += tab_width;
                chars.next();
            }
            _ => break,
        }
    }

    // Phase 3: 나머지 non-whitespace 내용 추가
    result.extend(chars);
    result
}

/// n columns만큼 leading whitespace를 소비하고 나머지를 반환
/// 소비 후 남은 leading whitespace의 탭도 spaces로 확장 (탭 스톱 정렬 보존)
pub(crate) fn consume_indent_and_expand(s: &str, n: usize) -> String {
    consume_indent_from_col(s, n, 0)
}

/// 문자열에서 최대 n칸의 공백/탭 제거 (tab stop 고려)
pub(crate) fn remove_indent(s: &str, n: usize) -> &str {
    // spaces-only fast path (기존 호출 호환)
    let spaces = count_leading_char(s, ' ');
    let remove = spaces.min(n);
    &s[remove..]
}

/// 앞뒤 빈 줄(빈 문자열) 제거 후 join
pub(crate) fn trim_blank_lines(lines: Vec<String>) -> String {
    let trimmed_lines: Vec<_> = lines
        .into_iter()
        .skip_while(|s| s.trim().is_empty())
        .collect();
    let mut trimmed_lines: Vec<_> = trimmed_lines
        .into_iter()
        .rev()
        .skip_while(|s| s.trim().is_empty())
        .collect();
    trimmed_lines.reverse();
    trimmed_lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    // 빈 문자열
    #[case("", ' ', 0)]
    #[case("", '`', 0)]
    // 해당 문자가 없는 경우
    #[case("abc", ' ', 0)]
    #[case("abc", '`', 0)]
    // 공백 세기
    #[case(" ", ' ', 1)]
    #[case("  ", ' ', 2)]
    #[case("  abc", ' ', 2)]
    #[case("     ", ' ', 5)]           // 전체가 공백
    // 백틱 세기
    #[case("```", '`', 3)]
    #[case("`````code", '`', 5)]
    #[case("```rust", '`', 3)]
    // 틸드 세기
    #[case("~~~", '~', 3)]
    #[case("~~~~~", '~', 5)]
    // 탭은 공백으로 안 셈
    #[case("\tabc", ' ', 0)]
    #[case(" \tabc", ' ', 1)]          // 공백 1개 후 탭
    // 중간에 다른 문자
    #[case("  a  ", ' ', 2)]           // 앞 공백만 셈
    fn test_count_leading_char(#[case] input: &str, #[case] c: char, #[case] expected: usize) {
        assert_eq!(count_leading_char(input, c), expected);
    }

    #[rstest]
    // 기본 케이스
    #[case("  code", 2, "code")]
    #[case("    code", 4, "code")]
    // 부분 제거 (n보다 공백이 많음)
    #[case("    code", 2, "  code")]
    #[case("      code", 3, "   code")]
    // 공백 없는 경우
    #[case("code", 2, "code")]
    #[case("code", 0, "code")]
    // n이 공백보다 큰 경우
    #[case("  code", 5, "code")]
    #[case("  code", 100, "code")]
    // n이 0인 경우
    #[case("  code", 0, "  code")]
    // 빈 문자열
    #[case("", 2, "")]
    #[case("", 0, "")]
    // 전체가 공백
    #[case("    ", 2, "  ")]
    #[case("    ", 4, "")]
    #[case("    ", 10, "")]
    // 탭은 제거 안 됨 (공백만 제거)
    #[case("\tcode", 2, "\tcode")]
    #[case("  \tcode", 2, "\tcode")]   // 공백 2개 제거, 탭은 남음
    #[case("  \tcode", 3, "\tcode")]   // 공백 2개만 있으므로 2개만 제거
    fn test_remove_indent(#[case] input: &str, #[case] n: usize, #[case] expected: &str) {
        assert_eq!(remove_indent(input, n), expected);
    }

    #[rstest]
    // 공백만
    #[case("", 0)]
    #[case(" ", 1)]
    #[case("  ", 2)]
    #[case("   ", 3)]
    #[case("    ", 4)]
    // 탭 (tab stop 기준)
    #[case("\t", 4)]
    #[case("\t\t", 8)]
    // 공백 + 탭 혼합 (tab stop 고려)
    #[case(" \t", 4)]           // col 1 → tab to col 4
    #[case("  \t", 4)]          // col 2 → tab to col 4
    #[case("   \t", 4)]         // col 3 → tab to col 4
    #[case("\t ", 5)]           // col 4 + 1
    #[case("    \t", 8)]        // col 4 → tab to col 8
    // 텍스트 포함
    #[case("code", 0)]
    #[case(" code", 1)]
    #[case("  code", 2)]
    #[case("\tcode", 4)]
    #[case("  \tcode", 4)]      // col 2 → tab to col 4
    fn test_calculate_indent(#[case] input: &str, #[case] expected: usize) {
        assert_eq!(calculate_indent(input), expected);
    }

    #[rstest]
    // 기본: spaces 소비
    #[case("    foo", 4, "foo")]
    #[case("  foo", 2, "foo")]
    #[case("  foo", 1, " foo")]
    #[case("foo", 0, "foo")]
    #[case("foo", 3, "foo")]           // non-whitespace는 소비 안 함
    // 탭 소비
    #[case("\tfoo", 4, "foo")]         // 탭 1개 = 4 cols, 정확히 소비
    #[case("\t\tfoo", 8, "foo")]
    #[case("\t\tfoo", 4, "\tfoo")]     // 첫 탭만 소비, 두번째 탭은 그대로 남음
    // 탭 partial consumption
    #[case("\tfoo", 1, "   foo")]      // 탭 4cols 중 1만 소비, 3 남음
    #[case("\tfoo", 2, "  foo")]       // 탭 4cols 중 2 소비, 2 남음
    #[case("\tfoo", 3, " foo")]        // 탭 4cols 중 3 소비, 1 남음
    // 혼합
    #[case(" \tfoo", 4, "foo")]        // space(1) + tab(3) = 4 cols
    #[case(" \tfoo", 2, "  foo")]      // space(1) 소비, tab 중 1col 소비, 2 남음
    #[case("  \tfoo", 4, "foo")]       // 2 spaces + tab(2) = 4 cols
    // Ex 7 시뮬레이션: list item content에서 탭 처리
    #[case("\t\tfoo", 5, "   foo")]    // 4+1 소비, 두번째 탭에서 3 남음
    // 빈 문자열
    #[case("", 0, "")]
    #[case("", 4, "")]
    fn test_consume_indent(#[case] input: &str, #[case] n: usize, #[case] expected: &str) {
        assert_eq!(consume_indent(input, n), expected);
    }

    #[rstest]
    // 빈 Vec
    #[case(vec![], "")]
    // 모두 빈 줄
    #[case(vec!["".to_string()], "")]
    #[case(vec!["".to_string(), "".to_string()], "")]
    #[case(vec!["  ".to_string()], "")]                    // 공백만 있는 줄도 빈 줄
    // 앞에만 빈 줄
    #[case(vec!["".to_string(), "code".to_string()], "code")]
    #[case(vec!["".to_string(), "".to_string(), "code".to_string()], "code")]
    #[case(vec!["  ".to_string(), "code".to_string()], "code")]  // 공백만 있는 앞줄
    // 뒤에만 빈 줄
    #[case(vec!["code".to_string(), "".to_string()], "code")]
    #[case(vec!["code".to_string(), "".to_string(), "".to_string()], "code")]
    #[case(vec!["code".to_string(), "  ".to_string()], "code")]  // 공백만 있는 뒷줄
    // 앞뒤 모두 빈 줄
    #[case(vec!["".to_string(), "code".to_string(), "".to_string()], "code")]
    #[case(vec!["".to_string(), "".to_string(), "code".to_string(), "".to_string(), "".to_string()], "code")]
    // 중간 빈 줄은 유지
    #[case(vec!["a".to_string(), "".to_string(), "b".to_string()], "a\n\nb")]
    #[case(vec!["".to_string(), "a".to_string(), "".to_string(), "b".to_string(), "".to_string()], "a\n\nb")]
    // 빈 줄 없음
    #[case(vec!["a".to_string()], "a")]
    #[case(vec!["a".to_string(), "b".to_string()], "a\nb")]
    #[case(vec!["a".to_string(), "b".to_string(), "c".to_string()], "a\nb\nc")]
    fn test_trim_blank_lines(#[case] input: Vec<String>, #[case] expected: &str) {
        assert_eq!(trim_blank_lines(input), expected);
    }
}
