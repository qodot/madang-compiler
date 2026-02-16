//! Inline Link 파서
//!
//! `[link text](destination "title")` 형태의 인라인 링크를 파싱합니다.
//! CommonMark 명세 Section 6.3: https://spec.commonmark.org/0.31.2/#links

/// 인라인 링크의 destination + title 파싱 결과
#[derive(Debug, Clone, PartialEq)]
pub struct LinkDestination {
    pub destination: String,
    pub title: Option<String>,
    /// `(` 부터 `)` 까지 소비한 바이트 수 (괄호 포함)
    pub bytes_consumed: usize,
}

/// `(` 로 시작하는 인라인 링크 destination을 파싱한다.
/// 입력은 `(` 부터 시작해야 한다.
/// 반환: Some(LinkDestination) 또는 None
pub fn parse_link_destination(input: &str) -> Option<LinkDestination> {
    let bytes = input.as_bytes();
    if bytes.is_empty() || bytes[0] != b'(' {
        return None;
    }

    let mut pos = 1; // skip '('

    // skip optional whitespace
    pos += skip_whitespace(&input[pos..]);

    if pos >= input.len() {
        return None;
    }

    // 빈 destination: ()
    if bytes[pos] == b')' {
        return Some(LinkDestination {
            destination: String::new(),
            title: None,
            bytes_consumed: pos + 1,
        });
    }

    // destination 파싱
    let (destination, dest_consumed) = if bytes[pos] == b'<' {
        parse_angle_destination(&input[pos..])?
    } else {
        parse_bare_destination(&input[pos..])?
    };

    pos += dest_consumed;

    // skip whitespace
    let ws = skip_whitespace(&input[pos..]);
    pos += ws;

    if pos >= input.len() {
        return None;
    }

    // title 파싱 (optional)
    let title = if bytes[pos] == b'"' || bytes[pos] == b'\'' || bytes[pos] == b'(' {
        match parse_title(&input[pos..]) {
            Some((t, consumed)) => {
                pos += consumed;
                pos += skip_whitespace(&input[pos..]);
                Some(t)
            }
            None => return None,
        }
    } else {
        None
    };

    if pos >= input.len() || bytes[pos] != b')' {
        return None;
    }

    Some(LinkDestination {
        destination,
        title,
        bytes_consumed: pos + 1, // include ')'
    })
}

/// `<destination>` 형태 파싱
/// 반환: (destination, bytes_consumed)
fn parse_angle_destination(input: &str) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    if bytes.is_empty() || bytes[0] != b'<' {
        return None;
    }

    let mut pos = 1;
    let mut dest = String::new();

    while pos < input.len() {
        match bytes[pos] {
            b'>' => return Some((dest, pos + 1)),
            b'<' => return None, // no nested < in angle destination
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
            b'\n' => return None, // no newlines
            _ => {
                let c = input[pos..].chars().next().unwrap();
                dest.push(c);
                pos += c.len_utf8();
            }
        }
    }

    None // no closing >
}

/// 괄호 없는 bare destination 파싱
/// 반환: (destination, bytes_consumed)
fn parse_bare_destination(input: &str) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    let mut pos = 0;
    let mut dest = String::new();
    let mut paren_depth = 0;

    while pos < input.len() {
        match bytes[pos] {
            // ASCII control characters or space end destination
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

/// title 파싱: "title", 'title', (title)
/// 반환: (title, bytes_consumed)
fn parse_title(input: &str) -> Option<(String, usize)> {
    let bytes = input.as_bytes();
    if bytes.is_empty() {
        return None;
    }

    let (open, close) = match bytes[0] {
        b'"' => (b'"', b'"'),
        b'\'' => (b'\'', b'\''),
        b'(' => (b'(', b')'),
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

    None // no closing quote
}

/// whitespace (space, tab, newline) 건너뛰기
fn skip_whitespace(input: &str) -> usize {
    input
        .bytes()
        .take_while(|b| *b == b' ' || *b == b'\t' || *b == b'\n' || *b == b'\r')
        .count()
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    // Example 482: [link](/uri "title")
    #[case("(/uri \"title\")", Some(("/uri", Some("title"), 14)))]
    // Example 483: [link](/uri)
    #[case("(/uri)", Some(("/uri", None, 6)))]
    // Example 484: [](./target.md)
    #[case("(./target.md)", Some(("./target.md", None, 13)))]
    // Example 485: [link]()
    #[case("()", Some(("", None, 2)))]
    // Example 486: [link](<>)
    #[case("(<>)", Some(("", None, 4)))]
    // Example 487: []()
    #[case("()", Some(("", None, 2)))]
    // Example 488: [link](/my uri) — space in bare dest → fail
    #[case("(/my uri)", None)]
    // Example 489: [link](</my uri>) — angle brackets allow spaces
    #[case("(</my uri>)", Some(("/my uri", None, 11)))]
    // Example 490: [link](foo\nbar) — newline in bare dest → fail
    #[case("(foo\nbar)", None)]
    // Example 495: [link](\(foo\))
    #[case("(\\(foo\\))", Some(("(foo)", None, 9)))]
    // Example 496: [link](foo(and(bar)))
    #[case("(foo(and(bar)))", Some(("foo(and(bar))", None, 15)))]
    // Example 497: [link](foo(and(bar)) — unbalanced parens
    #[case("(foo(and(bar))", None)]
    // Example 505: title with single and double quotes
    #[case("(/url \"title\")", Some(("/url", Some("title"), 14)))]
    // Example 510: whitespace around destination and title
    #[case("(   /uri\n  \"title\"  )", Some(("/uri", Some("title"), 21)))]
    fn test_parse_link_destination(
        #[case] input: &str,
        #[case] expected: Option<(&str, Option<&str>, usize)>,
    ) {
        let result = parse_link_destination(input);
        match expected {
            Some((dest, title, consumed)) => {
                let r = result.unwrap();
                assert_eq!(r.destination, dest);
                assert_eq!(r.title.as_deref(), title);
                assert_eq!(r.bytes_consumed, consumed);
            }
            None => assert!(result.is_none(), "Expected None, got {:?}", result),
        }
    }
}
