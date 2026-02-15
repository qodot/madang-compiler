//! Raw HTML (inline) 파서
//!
//! CommonMark 0.31.2 Section 6.6: https://spec.commonmark.org/0.31.2/#raw-html
//! 6가지 패턴: open tag, closing tag, HTML comment, processing instruction, declaration, CDATA

/// Raw HTML 파싱 결과
pub struct RawHtmlResult {
    pub content: String,
    pub bytes_consumed: usize,
}

/// `<`로 시작하는 input에서 raw HTML 패턴을 매칭
///
/// 6가지 패턴 중 하나에 매칭되면 Some 반환
pub fn parse_raw_html(input: &str) -> Option<RawHtmlResult> {
    if !input.starts_with('<') {
        return None;
    }

    None.or_else(|| try_open_tag(input))
        .or_else(|| try_closing_tag(input))
        .or_else(|| try_html_comment(input))
        .or_else(|| try_processing_instruction(input))
        .or_else(|| try_declaration(input))
        .or_else(|| try_cdata(input))
}

/// Open tag: `<` tag_name (attributes)* `\s*` `/`? `>`
fn try_open_tag(input: &str) -> Option<RawHtmlResult> {
    let bytes = input.as_bytes();
    if bytes.len() < 2 || bytes[0] != b'<' {
        return None;
    }

    let mut pos = 1;

    // tag name: [a-zA-Z][a-zA-Z0-9-]*
    if !bytes.get(pos)?.is_ascii_alphabetic() {
        return None;
    }
    pos += 1;
    while pos < bytes.len() && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'-') {
        pos += 1;
    }

    // attributes
    loop {
        let ws_start = pos;
        // consume whitespace (at least one required before attribute)
        while pos < bytes.len() && is_whitespace(bytes[pos]) {
            pos += 1;
        }
        let ws_count = pos - ws_start;

        // check if this is the end: optional `/` + `>`
        if pos < bytes.len() && (bytes[pos] == b'>' || bytes[pos] == b'/') {
            // optional /
            if bytes[pos] == b'/' {
                pos += 1;
            }
            if pos < bytes.len() && bytes[pos] == b'>' {
                pos += 1;
                return Some(RawHtmlResult {
                    content: input[..pos].to_string(),
                    bytes_consumed: pos,
                });
            }
            return None;
        }

        // need at least one whitespace before attribute name
        if ws_count == 0 {
            return None;
        }

        // try to parse attribute name: [a-zA-Z_:][a-zA-Z0-9_.:-]*
        if pos >= bytes.len() || !(bytes[pos].is_ascii_alphabetic() || bytes[pos] == b'_' || bytes[pos] == b':') {
            return None;
        }
        pos += 1;
        while pos < bytes.len()
            && (bytes[pos].is_ascii_alphanumeric()
                || bytes[pos] == b'_'
                || bytes[pos] == b'.'
                || bytes[pos] == b':'
                || bytes[pos] == b'-')
        {
            pos += 1;
        }

        // optional value specification: \s*=\s* attr_value
        let saved = pos;
        let mut tmp = pos;
        while tmp < bytes.len() && is_whitespace(bytes[tmp]) {
            tmp += 1;
        }
        if tmp < bytes.len() && bytes[tmp] == b'=' {
            tmp += 1;
            while tmp < bytes.len() && is_whitespace(bytes[tmp]) {
                tmp += 1;
            }
            // attr_value
            if tmp >= bytes.len() {
                return None;
            }
            match bytes[tmp] {
                b'\'' => {
                    tmp += 1;
                    while tmp < bytes.len() && bytes[tmp] != b'\'' {
                        tmp += 1;
                    }
                    if tmp >= bytes.len() {
                        return None;
                    }
                    tmp += 1; // skip closing '
                }
                b'"' => {
                    tmp += 1;
                    while tmp < bytes.len() && bytes[tmp] != b'"' {
                        tmp += 1;
                    }
                    if tmp >= bytes.len() {
                        return None;
                    }
                    tmp += 1; // skip closing "
                }
                _ => {
                    // unquoted: [^\s"'=<>`]+
                    let start = tmp;
                    while tmp < bytes.len() && !is_unquoted_excluded(bytes[tmp]) {
                        tmp += 1;
                    }
                    if tmp == start {
                        return None;
                    }
                }
            }
            pos = tmp;
        } else {
            // no = found, pos stays at saved (attribute with no value)
            let _ = saved;
        }
    }
}

/// Closing tag: `</` tag_name `\s*>`
fn try_closing_tag(input: &str) -> Option<RawHtmlResult> {
    let bytes = input.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'<' || bytes[1] != b'/' {
        return None;
    }

    let mut pos = 2;
    if !bytes.get(pos)?.is_ascii_alphabetic() {
        return None;
    }
    pos += 1;
    while pos < bytes.len() && (bytes[pos].is_ascii_alphanumeric() || bytes[pos] == b'-') {
        pos += 1;
    }

    // optional whitespace
    while pos < bytes.len() && is_whitespace(bytes[pos]) {
        pos += 1;
    }

    if pos < bytes.len() && bytes[pos] == b'>' {
        pos += 1;
        return Some(RawHtmlResult {
            content: input[..pos].to_string(),
            bytes_consumed: pos,
        });
    }
    None
}

/// HTML comment: `<!--` ... `-->` (but not `<!-->` or `<!--->`)
fn try_html_comment(input: &str) -> Option<RawHtmlResult> {
    if !input.starts_with("<!--") {
        return None;
    }

    // `<!--` followed by text, not starting with `>` or `->`, ending with `-->`
    // But simpler: `<!-->` and `<!--->` are invalid
    // Per CommonMark: `<!--` + text + `-->` where text must not start with `>` or `->`
    let rest = &input[4..];

    // text must not start with `>` or `->`
    if rest.starts_with('>') || rest.starts_with("->") {
        return None;
    }

    // Find first `-->`
    let end = rest.find("-->")?;
    let total = 4 + end + 3;

    Some(RawHtmlResult {
        content: input[..total].to_string(),
        bytes_consumed: total,
    })
}

/// Processing instruction: `<?` ... `?>`
fn try_processing_instruction(input: &str) -> Option<RawHtmlResult> {
    if !input.starts_with("<?") {
        return None;
    }
    let rest = &input[2..];
    let end = rest.find("?>")?;
    let total = 2 + end + 2;
    Some(RawHtmlResult {
        content: input[..total].to_string(),
        bytes_consumed: total,
    })
}

/// Declaration: `<!` uppercase_letter ... `>`
fn try_declaration(input: &str) -> Option<RawHtmlResult> {
    let bytes = input.as_bytes();
    if bytes.len() < 3 || bytes[0] != b'<' || bytes[1] != b'!' {
        return None;
    }
    if !bytes[2].is_ascii_uppercase() {
        return None;
    }
    let rest = &input[2..];
    let end = rest.find('>')?;
    let total = 2 + end + 1;
    Some(RawHtmlResult {
        content: input[..total].to_string(),
        bytes_consumed: total,
    })
}

/// CDATA: `<![CDATA[` ... `]]>`
fn try_cdata(input: &str) -> Option<RawHtmlResult> {
    if !input.starts_with("<![CDATA[") {
        return None;
    }
    let rest = &input[9..];
    let end = rest.find("]]>")?;
    let total = 9 + end + 3;
    Some(RawHtmlResult {
        content: input[..total].to_string(),
        bytes_consumed: total,
    })
}

fn is_whitespace(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r')
}

fn is_unquoted_excluded(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\n' | b'\r' | b'"' | b'\'' | b'=' | b'<' | b'>' | b'`')
}

#[cfg(test)]
mod tests {
    use super::*;
    use rstest::rstest;

    #[rstest]
    // Example 613: basic open tags
    #[case("<a>", Some("<a>"))]
    #[case("<bab>", Some("<bab>"))]
    #[case("<c2c>", Some("<c2c>"))]
    // Example 614: self-closing tags
    #[case("<a/>", Some("<a/>"))]
    #[case("<b2/>", Some("<b2/>"))]
    // Example 615: attributes with whitespace/newline
    #[case("<a  />", Some("<a  />"))]
    #[case("<b2\ndata=\"foo\" >", Some("<b2\ndata=\"foo\" >"))]
    // Example 616: complex attributes
    #[case("<a foo=\"bar\" bam = 'baz <em>\"</em>'\n_boolean zoop:33=zoop:33 />", Some("<a foo=\"bar\" bam = 'baz <em>\"</em>'\n_boolean zoop:33=zoop:33 />"))]
    // Example 617: custom element with attribute
    #[case("<responsive-image src=\"foo.jpg\" />", Some("<responsive-image src=\"foo.jpg\" />"))]
    // Example 618: invalid tag names
    #[case("<33>", None)]
    #[case("<__>", None)]
    // Example 619: invalid attribute name
    #[case("<a h*#ref=\"hi\">", None)]
    // Example 620: unclosed quotes
    #[case("<a href=\"hi'>", None)]
    #[case("<a href=hi'>", None)]
    // Example 621: invalid tags
    #[case("< a>", None)]  // space before tag name
    #[case("<\nfoo>", None)]  // newline before tag name
    #[case("<bar/ >", None)]  // space between / and >
    #[case("<foo bar=baz\nbim!bop />", None)]  // invalid unquoted value char !
    // Example 622: missing whitespace between attributes
    #[case("<a href='bar'title=title>", None)]
    // Example 623: closing tags
    #[case("</a>", Some("</a>"))]
    #[case("</foo >", Some("</foo >"))]
    // Example 624: closing tag with attributes is invalid
    #[case("</a href=\"foo\">", None)]
    // Example 625: HTML comment
    #[case("<!-- this is a --\ncomment - with hyphens -->", Some("<!-- this is a --\ncomment - with hyphens -->"))]
    // Example 626: invalid comments
    #[case("<!--> foo -->", None)]
    #[case("<!---> foo -->", None)]
    // Example 627: processing instruction
    #[case("<?php echo $a; ?>", Some("<?php echo $a; ?>"))]
    // Example 628: declaration
    #[case("<!ELEMENT br EMPTY>", Some("<!ELEMENT br EMPTY>"))]
    // Example 629: CDATA
    #[case("<![CDATA[>&<]]>", Some("<![CDATA[>&<]]>"))]
    // Example 630: open tag with entity in attribute (parsed as raw html, entities not resolved)
    #[case("<a href=\"&ouml;\">", Some("<a href=\"&ouml;\">"))]
    // Example 631: backslash in attribute value
    #[case("<a href=\"\\*\">", Some("<a href=\"\\*\">"))]
    // Example 632: backslash-escaped quote in attribute value (unclosed)
    #[case("<a href=\"\\\"\">", None)]
    fn test_parse_raw_html(#[case] input: &str, #[case] expected: Option<&str>) {
        let result = parse_raw_html(input);
        match expected {
            Some(s) => {
                let r = result.expect(&format!("Expected Some for input: {:?}", input));
                assert_eq!(r.content, s, "content mismatch for input: {:?}", input);
                assert_eq!(r.bytes_consumed, s.len(), "bytes_consumed mismatch for input: {:?}", input);
            }
            None => {
                assert!(result.is_none(), "Expected None for input: {:?}", input);
            }
        }
    }
}
