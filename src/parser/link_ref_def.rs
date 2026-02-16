//! Link Reference Definition 파서
//!
//! `[label]: destination "title"` 형태의 링크 참조 정의를 파싱합니다.
//! CommonMark 명세 Section 4.7: https://spec.commonmark.org/0.31.2/#link-reference-definitions

use std::collections::HashMap;

/// 파싱된 link reference definition
#[derive(Debug, Clone, PartialEq)]
pub struct LinkRefDef {
    pub label: String,
    pub destination: String,
    pub title: Option<String>,
}

/// Reference map: normalized label → (destination, title)
pub type RefMap = HashMap<String, (String, Option<String>)>;

/// label을 정규화한다: 대소문자 무시, 연속 공백을 하나로, 앞뒤 공백 제거
pub fn normalize_label(label: &str) -> String {
    label
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

/// 텍스트에서 link reference definitions를 추출한다.
/// paragraph의 raw 텍스트를 받아서:
/// - 시작 부분의 link ref def들을 추출하여 ref_map에 추가
/// - 남은 텍스트를 반환 (빈 문자열이면 paragraph 자체가 사라짐)
pub fn extract_definitions(text: &str, ref_map: &mut RefMap) -> String {
    let mut remaining = text;

    loop {
        match try_parse_definition(remaining) {
            Some((def, consumed)) => {
                let normalized = normalize_label(&def.label);
                // 첫 번째 정의만 유효 (중복 무시)
                ref_map
                    .entry(normalized)
                    .or_insert((def.destination, def.title));
                remaining = &remaining[consumed..];
                // 선행 newline 건너뛰기
                if remaining.starts_with('\n') {
                    remaining = &remaining[1..];
                }
            }
            None => break,
        }
    }

    remaining.to_string()
}

/// 텍스트 시작 부분에서 link ref def를 파싱 시도한다.
/// 반환: Some((LinkRefDef, consumed_bytes)) 또는 None
fn try_parse_definition(input: &str) -> Option<(LinkRefDef, usize)> {
    let mut pos = 0;
    let bytes = input.as_bytes();

    // optional leading spaces (0-3)
    let spaces = count_leading(bytes, pos, b' ');
    if spaces > 3 {
        return None;
    }
    pos += spaces;

    // [label]
    let (label, label_end) = parse_label(&input[pos..])?;
    pos += label_end;

    // :
    if pos >= input.len() || bytes[pos] != b':' {
        return None;
    }
    pos += 1;

    // optional whitespace (including up to one newline)
    pos += skip_spaces_and_optional_newline(&input[pos..]);

    // destination
    let (destination, dest_consumed) = parse_destination(&input[pos..])?;
    pos += dest_consumed;

    // optional title (requires at least one whitespace before)
    let ws_before_title = skip_spaces_and_optional_newline(&input[pos..]);
    let title = if ws_before_title > 0 {
        pos += ws_before_title;
        match parse_title(&input[pos..]) {
            Some((t, consumed)) => {
                pos += consumed;
                Some(t)
            }
            None => {
                // title 파싱 실패 → whitespace를 되돌리기
                pos -= ws_before_title;
                None
            }
        }
    } else {
        None
    };

    // 줄 끝이어야 함 (trailing spaces + newline or EOF)
    let trailing = count_leading(bytes, pos, b' ');
    pos += trailing;
    if pos < input.len() && bytes[pos] != b'\n' {
        return None;
    }
    if pos < input.len() && bytes[pos] == b'\n' {
        // newline은 consumed에 포함하지 않음 (caller가 처리)
    }

    Some((
        LinkRefDef {
            label,
            destination: super::inline::entity::resolve_entities(&destination),
            title: title.map(|t| super::inline::entity::resolve_entities(&t)),
        },
        pos,
    ))
}

/// [label] 파싱: `[` ~ `]`, 999자 제한, `[` 불허
fn parse_label(input: &str) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    if bytes.is_empty() || bytes[0] != b'[' {
        return None;
    }

    let mut pos = 1;
    let mut label = String::new();

    while pos < input.len() {
        match bytes[pos] {
            b']' => {
                let trimmed = label.trim();
                if trimmed.is_empty() {
                    return None;
                }
                return Some((label, pos + 1));
            }
            b'[' => return None, // unescaped [ in label
            b'\\' if pos + 1 < input.len() => {
                let next = bytes[pos + 1] as char;
                if crate::parser::inline::backslash_escape::is_ascii_punctuation(next) {
                    label.push(next);
                    pos += 2;
                } else {
                    label.push('\\');
                    pos += 1;
                }
            }
            _ => {
                let c = input[pos..].chars().next().unwrap();
                label.push(c);
                pos += c.len_utf8();
            }
        }
        if label.len() > 999 {
            return None;
        }
    }

    None
}

/// destination 파싱 (angle bracket 또는 bare)
fn parse_destination(input: &str) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    if bytes.is_empty() {
        return None;
    }

    if bytes[0] == b'<' {
        // angle bracket destination
        let mut pos = 1;
        let mut dest = String::new();
        while pos < input.len() {
            match bytes[pos] {
                b'>' => return Some((dest, pos + 1)),
                b'<' | b'\n' => return None,
                b'\\' if pos + 1 < input.len() => {
                    let next = bytes[pos + 1] as char;
                    if crate::parser::inline::backslash_escape::is_ascii_punctuation(next) {
                        dest.push(next);
                        pos += 2;
                    } else {
                        dest.push('\\');
                        pos += 1;
                    }
                }
                _ => {
                    let c = input[pos..].chars().next().unwrap();
                    dest.push(c);
                    pos += c.len_utf8();
                }
            }
        }
        None
    } else {
        // bare destination
        let mut pos = 0;
        let mut dest = String::new();
        let mut paren_depth = 0;

        while pos < input.len() {
            match bytes[pos] {
                b if b <= 0x20 => break,
                b')' => {
                    if paren_depth == 0 {
                        break;
                    }
                    paren_depth -= 1;
                    dest.push(')');
                    pos += 1;
                }
                b'(' => {
                    paren_depth += 1;
                    dest.push('(');
                    pos += 1;
                }
                b'\\' if pos + 1 < input.len() => {
                    let next = bytes[pos + 1] as char;
                    if crate::parser::inline::backslash_escape::is_ascii_punctuation(next) {
                        dest.push(next);
                        pos += 2;
                    } else {
                        dest.push('\\');
                        pos += 1;
                    }
                }
                _ => {
                    let c = input[pos..].chars().next().unwrap();
                    dest.push(c);
                    pos += c.len_utf8();
                }
            }
        }

        if dest.is_empty() {
            return None;
        }
        Some((dest, pos))
    }
}

/// title 파싱: "title", 'title', (title)
fn parse_title(input: &str) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    if bytes.is_empty() {
        return None;
    }

    let close = match bytes[0] {
        b'"' => b'"',
        b'\'' => b'\'',
        b'(' => b')',
        _ => return None,
    };

    let mut pos = 1;
    let mut title = String::new();

    while pos < input.len() {
        if bytes[pos] == close {
            return Some((title, pos + 1));
        }
        if bytes[pos] == b'\\' && pos + 1 < input.len() {
            let next = bytes[pos + 1] as char;
            if crate::parser::inline::backslash_escape::is_ascii_punctuation(next) {
                title.push(next);
                pos += 2;
                continue;
            }
        }
        let c = input[pos..].chars().next().unwrap();
        title.push(c);
        pos += c.len_utf8();
    }

    None
}

fn count_leading(bytes: &[u8], start: usize, ch: u8) -> usize {
    bytes[start..].iter().take_while(|&&b| b == ch).count()
}

fn skip_spaces_and_optional_newline(input: &str) -> usize {
    let bytes = input.as_bytes();
    let mut pos = 0;
    // spaces/tabs
    while pos < bytes.len() && (bytes[pos] == b' ' || bytes[pos] == b'\t') {
        pos += 1;
    }
    // optional one newline
    if pos < bytes.len() && bytes[pos] == b'\n' {
        pos += 1;
        // spaces after newline
        while pos < bytes.len() && (bytes[pos] == b' ' || bytes[pos] == b'\t') {
            pos += 1;
        }
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    #[case("foo", "foo")]
    #[case("Foo BAR", "foo bar")]
    #[case("  foo  bar  ", "foo bar")]
    #[case("Foo\n  bar", "foo bar")]
    fn test_normalize_label(#[case] input: &str, #[case] expected: &str) {
        assert_eq!(normalize_label(input), expected);
    }

    #[rstest]
    // Example 192: basic link ref def
    #[case("[foo]: /url \"title\"\n\n[foo]", "foo", "/url", Some("title"))]
    // Example 198: destination on next line
    #[case("[foo]:\n/url\n\n[foo]", "foo", "/url", None)]
    // Example 200: empty angle dest
    #[case("[foo]: <>\n\n[foo]", "foo", "", None)]
    fn test_extract_definitions(
        #[case] input: &str,
        #[case] label: &str,
        #[case] dest: &str,
        #[case] title: Option<&str>,
    ) {
        let mut ref_map = RefMap::new();
        let paragraph_text = input.split("\n\n").next().unwrap();
        extract_definitions(paragraph_text, &mut ref_map);
        let normalized = normalize_label(label);
        let entry = ref_map.get(&normalized).expect("label not found in ref_map");
        assert_eq!(entry.0, dest);
        assert_eq!(entry.1.as_deref(), title);
    }
}
