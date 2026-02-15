//! Autolink 파서
//!
//! `<URI>` 또는 `<email>` 형태의 autolink를 파싱합니다.
//! CommonMark 명세 Section 6.5: https://spec.commonmark.org/0.31.2/#autolinks

/// Autolink 파싱 결과
#[derive(Debug, Clone, PartialEq)]
pub enum AutolinkResult {
    /// URI autolink: `<scheme:content>`
    Uri {
        /// 원본 URI (scheme:... 전체)
        uri: String,
        /// 소비한 바이트 수 (< 부터 > 까지)
        bytes_consumed: usize,
    },
    /// Email autolink: `<user@domain>`
    Email {
        /// 이메일 주소
        email: String,
        /// 소비한 바이트 수
        bytes_consumed: usize,
    },
}

/// `<`로 시작하는 위치에서 autolink을 파싱한다
///
/// 성공 시 `Some(AutolinkResult)`, 실패 시 `None`
pub fn parse_autolink(input: &str) -> Option<AutolinkResult> {
    // < 로 시작해야 함
    if !input.starts_with('<') {
        return None;
    }

    // > 찾기
    let close = input.find('>')?;
    let content = &input[1..close];

    // 빈 content는 autolink 아님
    if content.is_empty() {
        return None;
    }

    // 공백이 있으면 autolink 아님
    if content.contains(|c: char| c == ' ' || c == '\t' || c == '\n') {
        return None;
    }

    let bytes_consumed = close + 1; // < + content + >

    // URI autolink 시도
    if let Some(result) = try_uri_autolink(content, bytes_consumed) {
        return Some(result);
    }

    // Email autolink 시도
    if let Some(result) = try_email_autolink(content, bytes_consumed) {
        return Some(result);
    }

    None
}

/// URI autolink: scheme이 2~32글자, letter로 시작, [letter, digit, +, -, .] 뒤에 `:`
fn try_uri_autolink(content: &str, bytes_consumed: usize) -> Option<AutolinkResult> {
    let colon_pos = content.find(':')?;
    let scheme = &content[..colon_pos];

    // scheme 길이 2~32
    if scheme.len() < 2 || scheme.len() > 32 {
        return None;
    }

    // 첫 문자는 letter
    let first = scheme.chars().next()?;
    if !first.is_ascii_alphabetic() {
        return None;
    }

    // 나머지는 letter, digit, +, -, .
    if !scheme[1..].chars().all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '-' || c == '.') {
        return None;
    }

    Some(AutolinkResult::Uri {
        uri: content.to_string(),
        bytes_consumed,
    })
}

/// Email autolink: 간단한 이메일 패턴 검증
///
/// 명세 정규식:
/// [a-zA-Z0-9.!#$%&'*+/=?^_`{|}~-]+@[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?
/// (\.[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*
fn try_email_autolink(content: &str, bytes_consumed: usize) -> Option<AutolinkResult> {
    let at_pos = content.find('@')?;
    let local = &content[..at_pos];
    let domain = &content[at_pos + 1..];

    // local part 검증
    if local.is_empty() || !local.chars().all(is_email_local_char) {
        return None;
    }

    // domain 검증
    if !is_valid_email_domain(domain) {
        return None;
    }

    Some(AutolinkResult::Email {
        email: content.to_string(),
        bytes_consumed,
    })
}

/// 이메일 local part 허용 문자
fn is_email_local_char(c: char) -> bool {
    c.is_ascii_alphanumeric()
        || matches!(c, '.' | '!' | '#' | '$' | '%' | '&' | '\'' | '*' | '+'
            | '/' | '=' | '?' | '^' | '_' | '`' | '{' | '|' | '}' | '~' | '-')
}

/// 이메일 domain 검증
///
/// 하나 이상의 label이 `.`으로 구분됨
/// 각 label: [a-zA-Z0-9]로 시작/끝, 중간에 `-` 허용, 최대 63글자
fn is_valid_email_domain(domain: &str) -> bool {
    if domain.is_empty() {
        return false;
    }

    let labels: Vec<&str> = domain.split('.').collect();
    if labels.is_empty() {
        return false;
    }

    for label in &labels {
        if label.is_empty() || label.len() > 63 {
            return false;
        }

        let chars: Vec<char> = label.chars().collect();

        // 시작과 끝은 alphanumeric
        if !chars.first().map_or(false, |c| c.is_ascii_alphanumeric()) {
            return false;
        }
        if !chars.last().map_or(false, |c| c.is_ascii_alphanumeric()) {
            return false;
        }

        // 중간은 alphanumeric 또는 -
        if !chars.iter().all(|c| c.is_ascii_alphanumeric() || *c == '-') {
            return false;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    // =========================================================================
    // parse_autolink 테스트 — URI autolinks
    // =========================================================================

    #[rstest]
    // Example 594: 기본 http
    #[case("<http://foo.bar.baz>", Some(AutolinkResult::Uri { uri: "http://foo.bar.baz".into(), bytes_consumed: 20 }))]
    // Example 595: https with query
    #[case("<https://foo.bar.baz/test?q=hello&id=22&boolean>", Some(AutolinkResult::Uri { uri: "https://foo.bar.baz/test?q=hello&id=22&boolean".into(), bytes_consumed: 48 }))]
    // Example 596: irc scheme
    #[case("<irc://foo.bar:2233/baz>", Some(AutolinkResult::Uri { uri: "irc://foo.bar:2233/baz".into(), bytes_consumed: 24 }))]
    // Example 597: MAILTO (대문자)
    #[case("<MAILTO:FOO@BAR.BAZ>", Some(AutolinkResult::Uri { uri: "MAILTO:FOO@BAR.BAZ".into(), bytes_consumed: 20 }))]
    // Example 598: a+b+c scheme
    #[case("<a+b+c:d>", Some(AutolinkResult::Uri { uri: "a+b+c:d".into(), bytes_consumed: 9 }))]
    // Example 599: made-up scheme
    #[case("<made-up-scheme://foo,bar>", Some(AutolinkResult::Uri { uri: "made-up-scheme://foo,bar".into(), bytes_consumed: 26 }))]
    // Example 600: relative path
    #[case("<https://../>", Some(AutolinkResult::Uri { uri: "https://../".into(), bytes_consumed: 13 }))]
    // Example 601: localhost
    #[case("<localhost:5001/foo>", Some(AutolinkResult::Uri { uri: "localhost:5001/foo".into(), bytes_consumed: 20 }))]
    // Example 602: 공백 포함 → 무효
    #[case("<https://foo.bar/baz bim>", None)]
    // Example 603: backslash in URI (여전히 유효)
    #[case("<https://example.com/\\[\\>", Some(AutolinkResult::Uri { uri: "https://example.com/\\[\\".into(), bytes_consumed: 25 }))]
    // Example 607: 빈 content → 무효
    #[case("<>", None)]
    // Example 608: 공백으로 시작 → 무효
    #[case("< https://foo.bar >", None)]
    // Example 609: scheme 1글자 → 무효
    #[case("<m:abc>", None)]
    // Example 610: colon 없음 → 무효 (email도 아님 — @ 없음)
    #[case("<foo.bar.baz>", None)]
    // Example 611: < > 없음 → parse_autolink에서 처리 안 함
    #[case("https://example.com", None)]
    fn test_parse_autolink_uri(#[case] input: &str, #[case] expected: Option<AutolinkResult>) {
        assert_eq!(parse_autolink(input), expected);
    }

    // =========================================================================
    // parse_autolink 테스트 — Email autolinks
    // =========================================================================

    #[rstest]
    // Example 604: 기본 이메일
    #[case("<foo@bar.example.com>", Some(AutolinkResult::Email { email: "foo@bar.example.com".into(), bytes_consumed: 21 }))]
    // Example 605: 특수문자 포함 이메일
    #[case("<foo+special@Bar.baz-bar0.com>", Some(AutolinkResult::Email { email: "foo+special@Bar.baz-bar0.com".into(), bytes_consumed: 30 }))]
    // Example 606: backslash escape된 이메일 → 무효 (\ 는 local part 허용 문자 아님)
    #[case("<foo\\+@bar.example.com>", None)]
    fn test_parse_autolink_email(#[case] input: &str, #[case] expected: Option<AutolinkResult>) {
        assert_eq!(parse_autolink(input), expected);
    }
}
