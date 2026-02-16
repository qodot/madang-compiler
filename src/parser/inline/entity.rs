//! Entity and numeric character references
//!
//! CommonMark 명세 Section 2.5: https://spec.commonmark.org/0.31.2/#entity-and-numeric-character-references
//!
//! 세 가지 종류:
//! - Named entity: `&amp;` → `&`
//! - Numeric decimal: `&#35;` → `#`
//! - Numeric hex: `&#X22;` or `&#x22;` → `"`

use entities::ENTITIES;

/// `&`로 시작하는 입력에서 entity reference를 파싱한다.
///
/// 성공 시 (치환된 문자열, 소비된 바이트 수)를 반환.
pub fn try_parse_entity(input: &str) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'&' {
        return None;
    }

    if bytes[1] == b'#' {
        try_parse_numeric(input)
    } else {
        try_parse_named(input)
    }
}

/// Named entity: `&name;`
/// name은 알파벳+숫자, 유효한 HTML5 entity만 매칭
fn try_parse_named(input: &str) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    // &name; — name은 [a-zA-Z][a-zA-Z0-9]* 형태
    if !bytes.get(1).map_or(false, |b| b.is_ascii_alphabetic()) {
        return None;
    }

    let semi_pos = input[1..].find(';')?;
    let name = &input[1..1 + semi_pos];

    // name은 알파벳+숫자만 허용
    if !name.chars().all(|c| c.is_ascii_alphanumeric()) {
        return None;
    }

    let entity_str = &input[..1 + semi_pos + 1]; // "&name;"

    // entities 크레이트에서 조회
    ENTITIES
        .iter()
        .find(|e| e.entity == entity_str)
        .map(|e| (e.characters.to_string(), entity_str.len()))
}

/// Numeric reference: `&#digits;` or `&#Xhex;` / `&#xhex;`
fn try_parse_numeric(input: &str) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    // bytes[0] = '&', bytes[1] = '#'
    if bytes.len() < 4 {
        return None;
    }

    let is_hex = bytes[2] == b'X' || bytes[2] == b'x';
    let digits_start = if is_hex { 3 } else { 2 };

    let semi_pos = input[digits_start..].find(';')?;
    let digits = &input[digits_start..digits_start + semi_pos];

    if digits.is_empty() {
        return None;
    }

    let codepoint = if is_hex {
        if !digits.chars().all(|c| c.is_ascii_hexdigit()) {
            return None;
        }
        u32::from_str_radix(digits, 16).ok()?
    } else {
        if !digits.chars().all(|c| c.is_ascii_digit()) {
            return None;
        }
        digits.parse::<u32>().ok()?
    };

    let ch = codepoint_to_char(codepoint)?;
    let consumed = digits_start + semi_pos + 1; // includes '&', '#', optional 'x'/'X', digits, ';'
    Some((ch.to_string(), consumed))
}

/// codepoint를 char에 매핑. 0이면 replacement character, 유효 범위 밖이면 None.
fn codepoint_to_char(cp: u32) -> Option<char> {
    if cp == 0 {
        return Some('\u{FFFD}');
    }
    char::from_u32(cp)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    // Named entities
    #[case("&amp;", Some(("&".to_string(), 5)))] // Example 25
    #[case("&copy;", Some(("©".to_string(), 6)))] // Example 25
    #[case("&AElig;", Some(("Æ".to_string(), 7)))] // Example 25
    #[case("&Dcaron;", Some(("Ď".to_string(), 8)))] // Example 25
    #[case("&frac34;", Some(("¾".to_string(), 8)))] // Example 25
    #[case("&HilbertSpace;", Some(("ℋ".to_string(), 14)))] // Example 25
    #[case("&nbsp;", Some(("\u{00A0}".to_string(), 6)))] // Example 25
    // Numeric decimal references
    #[case("&#35;", Some(("#".to_string(), 5)))] // Example 26
    #[case("&#1234;", Some(("Ӓ".to_string(), 7)))] // Example 26
    #[case("&#992;", Some(("Ϡ".to_string(), 6)))] // Example 26
    #[case("&#0;", Some(("\u{FFFD}".to_string(), 4)))] // Example 26: 0 → replacement char
    // Numeric hex references
    #[case("&#X22;", Some(("\"".to_string(), 6)))] // Example 27
    #[case("&#XD06;", Some(("ആ".to_string(), 7)))] // Example 27
    #[case("&#xcab;", Some(("ಫ".to_string(), 7)))] // Example 27
    // Invalid references — should return None
    #[case("&nbsp", None)] // Example 29: missing semicolon
    #[case("&x;", None)] // Example 28: not a valid entity
    #[case("&#;", None)] // Example 28: empty numeric
    #[case("&#x;", None)] // Example 28: empty hex
    #[case("&#87654321;", None)] // Example 28: out of range
    #[case("&#abcdef0;", None)] // Example 28: not valid decimal
    #[case("&ThisIsNotDefined;", None)] // Example 28: unknown entity
    #[case("&hi?;", None)] // Example 28: invalid chars in name
    #[case("&MadeUpEntity;", None)] // Example 30: unknown entity
    fn test_try_parse_entity(
        #[case] input: &str,
        #[case] expected: Option<(String, usize)>,
    ) {
        assert_eq!(try_parse_entity(input), expected);
    }
}
