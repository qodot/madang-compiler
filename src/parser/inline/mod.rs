//! Inline 파서
//!
//! 블록 파싱 후 텍스트를 인라인 노드로 변환합니다.
//! CommonMark 명세 Section 6: https://spec.commonmark.org/0.31.2/#inlines

mod autolink;
pub(crate) mod backslash_escape;
mod code_span;
mod emphasis;
mod line_break;
mod link;
mod raw_html;

use crate::node::{AutolinkNode, CodeSpanNode, ImageNode, InlineNode, LinkNode, RawHtmlNode, TextNode};
use emphasis::{DelimiterChar, DelimiterRun};

/// Bracket stack entry for link/image parsing
struct BracketEntry {
    /// `[` 텍스트 노드의 result 내 인덱스
    node_index: usize,
    /// 활성 여부 (내부 링크가 완성되면 외부 bracket 비활성화)
    active: bool,
    /// image bracket (`![`) 여부
    image: bool,
}

/// raw 텍스트를 인라인 노드들로 파싱
///
/// 블록 파서가 추출한 텍스트를 받아서 인라인 구조를 파싱합니다.
pub fn parse_inlines(raw: &str) -> Vec<InlineNode> {
    parse_inlines_with_refs(raw, None)
}

pub fn parse_inlines_with_refs(raw: &str, ref_map: Option<&crate::parser::link_ref_def::RefMap>) -> Vec<InlineNode> {
    let mut result = Vec::new();
    let mut delimiters: Vec<DelimiterRun> = Vec::new();
    let mut brackets: Vec<BracketEntry> = Vec::new();
    let mut pos = 0;
    let mut force_new_text = false;
    let bytes = raw.as_bytes();

    while pos < raw.len() {
        match bytes[pos] {
            b'\\' => {
                // \ + \n → hard line break
                if pos + 1 < raw.len() && bytes[pos + 1] == b'\n' {
                    strip_trailing_spaces_from_last_text(&mut result);
                    result.push(InlineNode::HardBreak);
                    pos += 2; // skip \ and \n
                    // 다음 줄의 leading spaces 건너뛰기
                    pos += line_break::skip_leading_spaces(&raw[pos..]);
                } else {
                    match backslash_escape::try_escape(&raw[pos..]) {
                        Some((escaped_char, consumed)) => {
                            if force_new_text {
                                result.push(InlineNode::Text(TextNode(escaped_char.to_string())));
                                force_new_text = false;
                            } else {
                                push_text_char(&mut result, escaped_char);
                            }
                            pos += consumed;
                        }
                        None => {
                            push_text_char(&mut result, '\\');
                            pos += 1;
                        }
                    }
                }
            }
            b'`' => {
                match code_span::parse_code_span(&raw[pos..]) {
                    Some(cs) => {
                        result.push(InlineNode::CodeSpan(CodeSpanNode::new(&cs.content)));
                        pos += cs.bytes_consumed;
                    }
                    None => {
                        let backtick_len = raw[pos..].chars().take_while(|&c| c == '`').count();
                        for _ in 0..backtick_len {
                            push_text_char(&mut result, '`');
                        }
                        pos += backtick_len;
                    }
                }
            }
            b'<' => {
                match autolink::parse_autolink(&raw[pos..]) {
                    Some(autolink::AutolinkResult::Uri { uri, bytes_consumed }) => {
                        result.push(InlineNode::Autolink(AutolinkNode::uri(&uri)));
                        pos += bytes_consumed;
                    }
                    Some(autolink::AutolinkResult::Email { email, bytes_consumed }) => {
                        result.push(InlineNode::Autolink(AutolinkNode::email(&email)));
                        pos += bytes_consumed;
                    }
                    None => {
                        match raw_html::parse_raw_html(&raw[pos..]) {
                            Some(rh) => {
                                result.push(InlineNode::RawHtml(RawHtmlNode::new(&rh.content)));
                                pos += rh.bytes_consumed;
                            }
                            None => {
                                push_text_char(&mut result, '<');
                                pos += 1;
                            }
                        }
                    }
                }
            }
            b'\n' => {
                // trailing spaces 2개 이상 → hard break, 그 외 → soft break
                let trailing = last_text_trailing_spaces(&result);
                if trailing >= 2 {
                    strip_trailing_spaces_from_last_text(&mut result);
                    result.push(InlineNode::HardBreak);
                } else {
                    strip_trailing_spaces_from_last_text(&mut result);
                    result.push(InlineNode::SoftBreak);
                }
                pos += 1;
                // 다음 줄의 leading spaces 건너뛰기
                pos += line_break::skip_leading_spaces(&raw[pos..]);
            }
            b'*' | b'_' => {
                let delim_char = if bytes[pos] == b'*' {
                    DelimiterChar::Asterisk
                } else {
                    DelimiterChar::Underscore
                };

                let run_len = raw[pos..].chars().take_while(|&c| c == bytes[pos] as char).count();

                let before = if pos > 0 { raw[..pos].chars().last() } else { None };
                let after = raw[pos + run_len..].chars().next();

                let (can_open, can_close) = emphasis::compute_flanking(delim_char, before, after);

                // delimiter 텍스트를 별도 노드로 추가
                let delim_text: String = std::iter::repeat(bytes[pos] as char).take(run_len).collect();
                result.push(InlineNode::Text(TextNode(delim_text)));

                delimiters.push(DelimiterRun {
                    char: delim_char,
                    original_length: run_len,
                    length: run_len,
                    can_open,
                    can_close,
                    position: result.len() - 1,
                });

                pos += run_len;
                // delimiter 뒤의 텍스트가 합쳐지지 않도록 force_new_text 설정
                force_new_text = true;
            }
            b'!' if pos + 1 < raw.len() && bytes[pos + 1] == b'[' => {
                // image bracket `![`
                result.push(InlineNode::Text(TextNode("![".to_string())));
                brackets.push(BracketEntry {
                    node_index: result.len() - 1,
                    active: true,
                    image: true,
                });
                pos += 2;
                force_new_text = true;
            }
            b'[' => {
                // link bracket `[`
                result.push(InlineNode::Text(TextNode("[".to_string())));
                brackets.push(BracketEntry {
                    node_index: result.len() - 1,
                    active: true,
                    image: false,
                });
                pos += 1;
                force_new_text = true;
            }
            b']' => {
                if let Some(bracket) = brackets.pop() {
                    if bracket.active && pos + 1 < raw.len() && bytes[pos + 1] == b'(' {
                        // `](` → 인라인 링크 시도
                        if let Some(dest) = link::parse_link_destination(&raw[pos + 1..]) {
                            let bracket_pos = bracket.node_index;

                            // delimiter를 bracket 기준으로 분할
                            let split_idx = delimiters
                                .iter()
                                .position(|d| d.position > bracket_pos)
                                .unwrap_or(delimiters.len());
                            let child_delims: Vec<DelimiterRun> = delimiters
                                .split_off(split_idx)
                                .into_iter()
                                .map(|mut d| {
                                    d.position -= bracket_pos + 1;
                                    d
                                })
                                .collect();

                            // children 추출
                            let children: Vec<InlineNode> = result
                                .drain((bracket_pos + 1)..)
                                .collect();
                            result.pop(); // remove the `[` text node

                            // children에 emphasis 처리 적용
                            let children = if !child_delims.is_empty() {
                                emphasis::process_emphasis(children, child_delims)
                            } else {
                                children
                            };

                            let node = if bracket.image {
                                InlineNode::Image(ImageNode::new(
                                    children,
                                    &dest.destination,
                                    dest.title.as_deref(),
                                ))
                            } else {
                                InlineNode::Link(LinkNode::new(
                                    children,
                                    &dest.destination,
                                    dest.title.as_deref(),
                                ))
                            };
                            result.push(node);

                            pos += 1 + dest.bytes_consumed; // skip ] + destination
                            force_new_text = true;

                            // link의 경우 외부 bracket 비활성화 (링크 안에 링크는 불가)
                            // image는 link 안에 포함될 수 있으므로 비활성화하지 않음
                            if !bracket.image {
                                for b in brackets.iter_mut() {
                                    b.active = false;
                                }
                            }
                            continue;
                        }
                    }
                    // inline link 실패 → reference link 시도
                    if bracket.active {
                        if let Some(rm) = ref_map {
                            if let Some((ref_node, consumed)) = try_reference_link(
                                &raw[pos..], &result, &delimiters, &bracket, rm,
                            ) {
                                let bracket_pos = bracket.node_index;
                                // delimiter 분할
                                let split_idx = delimiters
                                    .iter()
                                    .position(|d| d.position > bracket_pos)
                                    .unwrap_or(delimiters.len());
                                let child_delims: Vec<DelimiterRun> = delimiters
                                    .split_off(split_idx)
                                    .into_iter()
                                    .map(|mut d| {
                                        d.position -= bracket_pos + 1;
                                        d
                                    })
                                    .collect();

                                let children: Vec<InlineNode> = result
                                    .drain((bracket_pos + 1)..)
                                    .collect();
                                result.pop();

                                let children = if !child_delims.is_empty() {
                                    emphasis::process_emphasis(children, child_delims)
                                } else {
                                    children
                                };

                                result.push(ref_node.with_children(children));
                                pos += consumed;
                                force_new_text = true;

                                if !bracket.image {
                                    for b in brackets.iter_mut() {
                                        b.active = false;
                                    }
                                }
                                continue;
                            }
                        }
                    }
                    // 매칭 실패 → `]`를 텍스트로
                    push_text_char(&mut result, ']');
                    pos += 1;
                } else {
                    // bracket 없음 → `]`를 텍스트로
                    push_text_char(&mut result, ']');
                    pos += 1;
                }
            }
            _ => {
                let c = raw[pos..].chars().next().unwrap();
                if force_new_text {
                    result.push(InlineNode::Text(TextNode(c.to_string())));
                    force_new_text = false;
                } else {
                    push_text_char(&mut result, c);
                }
                pos += c.len_utf8();
            }
        }
    }

    if result.is_empty() {
        return vec![InlineNode::Text(TextNode::new(""))];
    }

    // emphasis/strong 처리
    if !delimiters.is_empty() {
        result = emphasis::process_emphasis(result, delimiters);
    }

    // 인접한 Text 노드 합치기
    merge_adjacent_text(&mut result);

    result
}

/// 인접한 Text 노드를 합친다
/// Reference link 해석 결과 (children은 나중에 설정)
enum RefLinkResult {
    Link { destination: String, title: Option<String> },
    Image { destination: String, title: Option<String> },
}

impl RefLinkResult {
    fn with_children(self, children: Vec<InlineNode>) -> InlineNode {
        match self {
            RefLinkResult::Link { destination, title } => {
                InlineNode::Link(LinkNode::new(children, &destination, title.as_deref()))
            }
            RefLinkResult::Image { destination, title } => {
                InlineNode::Image(ImageNode::new(children, &destination, title.as_deref()))
            }
        }
    }
}

/// reference link를 시도한다.
/// `]` 위치에서 시작, `][ref]`, `][]`, `]` (shortcut) 패턴 체크
/// 반환: Some((RefLinkResult, consumed_bytes)) — consumed는 `]` 포함
fn try_reference_link(
    input: &str, // `]`부터 시작
    result: &[InlineNode],
    _delimiters: &[DelimiterRun],
    bracket: &BracketEntry,
    ref_map: &crate::parser::link_ref_def::RefMap,
) -> Option<(RefLinkResult, usize)> {
    let bytes = input.as_bytes();
    let bracket_pos = bracket.node_index;

    // label 텍스트 추출 (bracket 다음 ~ 현재 result 끝)
    let label_text = extract_label_from_nodes(&result[(bracket_pos + 1)..]);

    // 패턴 1: `][ref]` — full reference
    if bytes.len() > 1 && bytes[1] == b'[' {
        // `]` 다음 `[ref]` 찾기
        if let Some(close) = input[2..].find(']') {
            let ref_label = &input[2..2 + close];
            if !ref_label.is_empty() {
                let normalized = crate::parser::link_ref_def::normalize_label(ref_label);
                if let Some((dest, title)) = ref_map.get(&normalized) {
                    let r = if bracket.image {
                        RefLinkResult::Image { destination: dest.clone(), title: title.clone() }
                    } else {
                        RefLinkResult::Link { destination: dest.clone(), title: title.clone() }
                    };
                    return Some((r, 1 + 1 + close + 1)); // ] + [ + ref + ]
                }
            }
        }

        // `][]` — collapsed reference
        if bytes.len() > 2 && bytes[1] == b'[' && bytes[2] == b']' {
            let normalized = crate::parser::link_ref_def::normalize_label(&label_text);
            if let Some((dest, title)) = ref_map.get(&normalized) {
                let r = if bracket.image {
                    RefLinkResult::Image { destination: dest.clone(), title: title.clone() }
                } else {
                    RefLinkResult::Link { destination: dest.clone(), title: title.clone() }
                };
                return Some((r, 3)); // ][]
            }
        }
    }

    // 패턴 3: `]` — shortcut reference
    let normalized = crate::parser::link_ref_def::normalize_label(&label_text);
    if let Some((dest, title)) = ref_map.get(&normalized) {
        let r = if bracket.image {
            RefLinkResult::Image { destination: dest.clone(), title: title.clone() }
        } else {
            RefLinkResult::Link { destination: dest.clone(), title: title.clone() }
        };
        return Some((r, 1)); // just ]
    }

    None
}

/// 인라인 노드 배열에서 텍스트 내용만 추출
fn extract_label_from_nodes(nodes: &[InlineNode]) -> String {
    let mut text = String::new();
    for node in nodes {
        match node {
            InlineNode::Text(t) => text.push_str(&t.0),
            InlineNode::CodeSpan(c) => text.push_str(&c.0),
            InlineNode::Emphasis(e) => text.push_str(&extract_label_from_nodes(&e.children)),
            InlineNode::Strong(s) => text.push_str(&extract_label_from_nodes(&s.children)),
            InlineNode::SoftBreak | InlineNode::HardBreak => text.push(' '),
            _ => {}
        }
    }
    text
}

fn merge_adjacent_text(nodes: &mut Vec<InlineNode>) {
    // 재귀적으로 children도 처리
    for node in nodes.iter_mut() {
        match node {
            InlineNode::Emphasis(e) => merge_adjacent_text(&mut e.children),
            InlineNode::Strong(s) => merge_adjacent_text(&mut s.children),
            InlineNode::Link(l) => merge_adjacent_text(&mut l.children),
            InlineNode::Image(i) => merge_adjacent_text(&mut i.children),
            _ => {}
        }
    }
    // 인접한 Text 노드 합치기
    let mut i = 0;
    while i + 1 < nodes.len() {
        if let (InlineNode::Text(_), InlineNode::Text(_)) = (&nodes[i], &nodes[i + 1]) {
            if let InlineNode::Text(next) = nodes.remove(i + 1) {
                if let InlineNode::Text(current) = &mut nodes[i] {
                    current.0.push_str(&next.0);
                }
            }
        } else {
            i += 1;
        }
    }
}

/// 마지막 노드가 Text이면 문자열을 추가, 아니면 새 Text 노드 생성
fn push_text_str(result: &mut Vec<InlineNode>, s: &str) {
    if let Some(InlineNode::Text(text)) = result.last_mut() {
        text.0.push_str(s);
    } else {
        result.push(InlineNode::Text(TextNode(s.to_string())));
    }
}

/// 마지막 노드가 Text이면 문자를 추가, 아니면 새 Text 노드 생성
fn push_text_char(result: &mut Vec<InlineNode>, c: char) {
    if let Some(InlineNode::Text(text)) = result.last_mut() {
        text.0.push(c);
    } else {
        let mut s = String::new();
        s.push(c);
        result.push(InlineNode::Text(TextNode(s)));
    }
}

/// 마지막 Text 노드의 trailing spaces 수
fn last_text_trailing_spaces(result: &[InlineNode]) -> usize {
    if let Some(InlineNode::Text(text)) = result.last() {
        line_break::count_trailing_spaces(&text.0)
    } else {
        0
    }
}

/// 마지막 Text 노드에서 trailing spaces 제거
fn strip_trailing_spaces_from_last_text(result: &mut Vec<InlineNode>) {
    if let Some(InlineNode::Text(text)) = result.last_mut() {
        let stripped = line_break::strip_trailing_spaces(&text.0).to_string();
        text.0 = stripped;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::InlineNode;
    use rstest::rstest;

    // =========================================================================
    // parse_inlines 기본 테스트
    // =========================================================================

    #[rstest]
    #[case("hello", vec![InlineNode::text("hello")])]
    #[case("", vec![InlineNode::text("")])]
    #[case("foo bar baz", vec![InlineNode::text("foo bar baz")])]
    fn test_parse_inlines_text(#[case] input: &str, #[case] expected: Vec<InlineNode>) {
        assert_eq!(parse_inlines(input), expected);
    }

    // =========================================================================
    // parse_inlines — code span 통합 테스트
    // =========================================================================

    #[rstest]
    // Example 328: 기본 code span
    #[case("`foo`", vec![InlineNode::code_span("foo")])]
    // Example 329: 백틱 2개, 내부에 백틱 1개
    #[case("`` foo ` bar ``", vec![InlineNode::code_span("foo ` bar")])]
    // Example 330: 내부에 백틱 2개
    #[case("` `` `", vec![InlineNode::code_span("``")])]
    // Example 331: 내부 공백 보존
    #[case("`  ``  `", vec![InlineNode::code_span(" `` ")])]
    // Example 332: 앞에만 공백
    #[case("` a`", vec![InlineNode::code_span(" a")])]
    // Example 335: 줄바꿈 → 공백
    #[case("``\nfoo\nbar  \nbaz\n``", vec![InlineNode::code_span("foo bar   baz")])]
    // Example 337: 내부 공백 유지
    #[case("`foo   bar \nbaz`", vec![InlineNode::code_span("foo   bar  baz")])]
    // Example 338: 백슬래시는 code span 내에서 이스케이프 안 됨
    #[case("`foo\\`bar`", vec![InlineNode::code_span("foo\\"), InlineNode::text("bar`")])]
    // Example 339: 백틱 2개로 감싸면 내부 백틱 1개 OK
    #[case("``foo`bar``", vec![InlineNode::code_span("foo`bar")])]
    // Example 340: 백틱 1개로 감싸면 내부 백틱 2개 OK
    #[case("` foo `` bar `", vec![InlineNode::code_span("foo `` bar")])]
    // Example 341: code span이 emphasis보다 우선
    #[case("*foo`*`", vec![InlineNode::text("*foo"), InlineNode::code_span("*")])]
    // Example 342: code span이 link보다 우선
    #[case("[not a `link](/foo`)", vec![InlineNode::text("[not a "), InlineNode::code_span("link](/foo"), InlineNode::text(")")])]
    // Example 343: code span 내에서 HTML은 리터럴
    #[case("`<a href=\"`\">` ", vec![InlineNode::code_span("<a href=\""), InlineNode::text("\">` ")])]
    // Example 347: 닫는 백틱 없음 → 텍스트
    #[case("```foo``", vec![InlineNode::text("```foo``")])]
    // Example 348: 닫는 백틱 없음 → 텍스트
    #[case("`foo", vec![InlineNode::text("`foo")])]
    // Example 349: 첫 ` 매칭 안 됨, ``bar`` 매칭
    #[case("`foo``bar``", vec![InlineNode::text("`foo"), InlineNode::code_span("bar")])]
    // code span 앞뒤에 텍스트
    #[case("hello `world` bye", vec![InlineNode::text("hello "), InlineNode::code_span("world"), InlineNode::text(" bye")])]
    // 여러 code span
    #[case("`a` and `b`", vec![InlineNode::code_span("a"), InlineNode::text(" and "), InlineNode::code_span("b")])]
    fn test_parse_inlines_code_span(#[case] input: &str, #[case] expected: Vec<InlineNode>) {
        assert_eq!(parse_inlines(input), expected);
    }

    // =========================================================================
    // parse_inlines — backslash escape 통합 테스트
    // =========================================================================

    #[rstest]
    // Example 12: 모든 ASCII 구두점 이스케이프
    #[case(
        "\\!\\\"\\#\\$\\%\\&\\'\\(\\)\\*\\+\\,\\-\\.\\/\\:\\;\\<\\=\\>\\?\\@\\[\\\\\\]\\^\\_\\`\\{\\|\\}\\~",
        vec![InlineNode::text("!\"#$%&'()*+,-./:;<=>?@[\\]^_`{|}~")]
    )]
    // Example 13: 구두점이 아닌 문자는 이스케이프 안 됨
    #[case("\\A\\a\\ \\3", vec![InlineNode::text("\\A\\a\\ \\3")])]
    // Example 14 (부분): \*는 리터럴 *
    #[case("\\*not emphasized*", vec![InlineNode::text("*not emphasized*")])]
    // Example 14 (부분): \`는 리터럴 `
    #[case("\\`not code`", vec![InlineNode::text("`not code`")])]
    // Example 15 (부분): \\는 리터럴 \
    #[case("\\\\hello", vec![InlineNode::text("\\hello")])]
    // Example 17: code span 내에서는 이스케이프 안 됨
    #[case("`` \\[\\` ``", vec![InlineNode::code_span("\\[\\`")])]
    // 이스케이프와 code span 혼합
    #[case("\\*`code`\\*", vec![InlineNode::text("*"), InlineNode::code_span("code"), InlineNode::text("*")])]
    fn test_parse_inlines_backslash_escape(#[case] input: &str, #[case] expected: Vec<InlineNode>) {
        assert_eq!(parse_inlines(input), expected);
    }

    // =========================================================================
    // parse_inlines — autolink 통합 테스트
    // =========================================================================

    #[rstest]
    // Example 594: 기본 URI autolink
    #[case("<http://foo.bar.baz>", vec![InlineNode::autolink_uri("http://foo.bar.baz")])]
    // Example 597: MAILTO URI
    #[case("<MAILTO:FOO@BAR.BAZ>", vec![InlineNode::autolink_uri("MAILTO:FOO@BAR.BAZ")])]
    // Example 601: localhost
    #[case("<localhost:5001/foo>", vec![InlineNode::autolink_uri("localhost:5001/foo")])]
    // Example 602: 공백 포함 → 텍스트
    #[case("<https://foo.bar/baz bim>", vec![InlineNode::text("<https://foo.bar/baz bim>")])]
    // Example 604: email autolink
    #[case("<foo@bar.example.com>", vec![InlineNode::autolink_email("foo@bar.example.com")])]
    // Example 607: 빈 → 텍스트
    #[case("<>", vec![InlineNode::text("<>")])]
    // Example 609: scheme 1글자 → 텍스트
    #[case("<m:abc>", vec![InlineNode::text("<m:abc>")])]
    // Example 611: < > 없음 → 텍스트
    #[case("https://example.com", vec![InlineNode::text("https://example.com")])]
    // autolink 앞뒤에 텍스트
    #[case("visit <http://example.com> now", vec![InlineNode::text("visit "), InlineNode::autolink_uri("http://example.com"), InlineNode::text(" now")])]
    fn test_parse_inlines_autolink(#[case] input: &str, #[case] expected: Vec<InlineNode>) {
        assert_eq!(parse_inlines(input), expected);
    }

    // =========================================================================
    // parse_inlines — emphasis 통합 테스트
    // =========================================================================

    #[rstest]
    // Example 350: 기본 * emphasis
    #[case("*foo bar*", vec![InlineNode::emphasis(vec![InlineNode::text("foo bar")])])]
    // Example 355: 단어 중간 * emphasis
    #[case("foo*bar*", vec![InlineNode::text("foo"), InlineNode::emphasis(vec![InlineNode::text("bar")])])]
    // Example 357: 기본 _ emphasis
    #[case("_foo bar_", vec![InlineNode::emphasis(vec![InlineNode::text("foo bar")])])]
    // Example 393: 기본 ** strong
    #[case("**foo bar**", vec![InlineNode::strong(vec![InlineNode::text("foo bar")])])]
    // Example 410: 기본 __ strong
    #[case("__foo bar__", vec![InlineNode::strong(vec![InlineNode::text("foo bar")])])]
    // Example 351: * 뒤에 공백 → emphasis 아님
    #[case("a * foo bar*", vec![InlineNode::text("a * foo bar*")])]
    // Example 358: _ 뒤에 공백 → emphasis 아님
    #[case("_ foo bar_", vec![InlineNode::text("_ foo bar_")])]
    // Example 360: _ 단어 내부 → emphasis 아님
    #[case("foo_bar_", vec![InlineNode::text("foo_bar_")])]
    // Example 348 (code span): 닫는 ` 없음 → 텍스트 (기존 테스트와 충돌 안 하는지 확인)
    fn test_parse_inlines_emphasis(#[case] input: &str, #[case] expected: Vec<InlineNode>) {
        assert_eq!(parse_inlines(input), expected);
    }

    // =========================================================================
    // parse_inlines — emphasis/strong examples 350-481 (CommonMark spec)
    // =========================================================================

    // Example 353: * a *
    #[test]
    fn example_353() {
        assert_eq!(parse_inlines("*\u{a0}a\u{a0}*"), vec![InlineNode::text("*\u{a0}a\u{a0}*")]);
    }

    // Example 356: 5*6*78
    #[test]
    fn example_356() {
        assert_eq!(parse_inlines("5*6*78"), vec![InlineNode::text("5"), InlineNode::emphasis(vec![InlineNode::text("6")]), InlineNode::text("78")]);
    }

    // Example 361: 5_6_78
    #[test]
    fn example_361() {
        assert_eq!(parse_inlines("5_6_78"), vec![InlineNode::text("5_6_78")]);
    }

    // Example 362: пристаням_стремятся_
    #[test]
    fn example_362() {
        assert_eq!(parse_inlines("пристаням_стремятся_"), vec![InlineNode::text("пристаням_стремятся_")]);
    }

    // Example 364: foo-_(bar)_
    #[test]
    fn example_364() {
        assert_eq!(parse_inlines("foo-_(bar)_"), vec![InlineNode::text("foo-"), InlineNode::emphasis(vec![InlineNode::text("(bar)")])]);
    }

    // Example 365: _foo*
    #[test]
    fn example_365() {
        assert_eq!(parse_inlines("_foo*"), vec![InlineNode::text("_foo*")]);
    }

    // Example 366: *foo bar *
    #[test]
    fn example_366() {
        assert_eq!(parse_inlines("*foo bar *"), vec![InlineNode::text("*foo bar *")]);
    }

    // Example 367: *foo bar\n*
    #[test]
    fn example_367() {
        assert_eq!(parse_inlines("*foo bar\n*"), vec![InlineNode::text("*foo bar"), InlineNode::SoftBreak, InlineNode::text("*")]);
    }

    // Example 368: *(*foo)
    #[test]
    fn example_368() {
        assert_eq!(parse_inlines("*(*foo)"), vec![InlineNode::text("*(*foo)")]);
    }

    // Example 369: *(*foo*)*
    #[test]
    fn example_369() {
        assert_eq!(parse_inlines("*(*foo*)*"), vec![InlineNode::emphasis(vec![InlineNode::text("("), InlineNode::emphasis(vec![InlineNode::text("foo")]), InlineNode::text(")")])]);
    }

    // Example 370: *foo*bar
    #[test]
    fn example_370() {
        assert_eq!(parse_inlines("*foo*bar"), vec![InlineNode::emphasis(vec![InlineNode::text("foo")]), InlineNode::text("bar")]);
    }

    // Example 371: _foo bar _
    #[test]
    fn example_371() {
        assert_eq!(parse_inlines("_foo bar _"), vec![InlineNode::text("_foo bar _")]);
    }

    // Example 372: _(_foo)
    #[test]
    fn example_372() {
        assert_eq!(parse_inlines("_(_foo)"), vec![InlineNode::text("_(_foo)")]);
    }

    // Example 373: _(_foo_)_
    #[test]
    fn example_373() {
        assert_eq!(parse_inlines("_(_foo_)_"), vec![InlineNode::emphasis(vec![InlineNode::text("("), InlineNode::emphasis(vec![InlineNode::text("foo")]), InlineNode::text(")")])]);
    }

    // Example 374: _foo_bar
    #[test]
    fn example_374() {
        assert_eq!(parse_inlines("_foo_bar"), vec![InlineNode::text("_foo_bar")]);
    }

    // Example 375: _пристаням_стремятся
    #[test]
    fn example_375() {
        assert_eq!(parse_inlines("_пристаням_стремятся"), vec![InlineNode::text("_пристаням_стремятся")]);
    }

    // Example 376: _foo_bar_baz_
    #[test]
    fn example_376() {
        assert_eq!(parse_inlines("_foo_bar_baz_"), vec![InlineNode::emphasis(vec![InlineNode::text("foo_bar_baz")])]);
    }

    // Example 377: _(bar)_.
    #[test]
    fn example_377() {
        assert_eq!(parse_inlines("_(bar)_."), vec![InlineNode::emphasis(vec![InlineNode::text("(bar)")]), InlineNode::text(".")]);
    }

    // Example 378: **foo bar**
    #[test]
    fn example_378() {
        assert_eq!(parse_inlines("**foo bar**"), vec![InlineNode::strong(vec![InlineNode::text("foo bar")])]);
    }

    // Example 379: ** foo bar**
    #[test]
    fn example_379() {
        assert_eq!(parse_inlines("** foo bar**"), vec![InlineNode::text("** foo bar**")]);
    }

    // Example 381: foo**bar**
    #[test]
    fn example_381() {
        assert_eq!(parse_inlines("foo**bar**"), vec![InlineNode::text("foo"), InlineNode::strong(vec![InlineNode::text("bar")])]);
    }

    // Example 382: __foo bar__
    #[test]
    fn example_382() {
        assert_eq!(parse_inlines("__foo bar__"), vec![InlineNode::strong(vec![InlineNode::text("foo bar")])]);
    }

    // Example 383: __ foo bar__
    #[test]
    fn example_383() {
        assert_eq!(parse_inlines("__ foo bar__"), vec![InlineNode::text("__ foo bar__")]);
    }

    // Example 384: __\nfoo bar__
    #[test]
    fn example_384() {
        assert_eq!(parse_inlines("__\nfoo bar__"), vec![InlineNode::text("__"), InlineNode::SoftBreak, InlineNode::text("foo bar__")]);
    }

    // Example 386: foo__bar__
    #[test]
    fn example_386() {
        assert_eq!(parse_inlines("foo__bar__"), vec![InlineNode::text("foo__bar__")]);
    }

    // Example 387: 5__6__78
    #[test]
    fn example_387() {
        assert_eq!(parse_inlines("5__6__78"), vec![InlineNode::text("5__6__78")]);
    }

    // Example 388: пристаням__стремятся__
    #[test]
    fn example_388() {
        assert_eq!(parse_inlines("пристаням__стремятся__"), vec![InlineNode::text("пристаням__стремятся__")]);
    }

    // Example 389: __foo, __bar__, baz__
    #[test]
    fn example_389() {
        assert_eq!(parse_inlines("__foo, __bar__, baz__"), vec![InlineNode::strong(vec![InlineNode::text("foo, "), InlineNode::strong(vec![InlineNode::text("bar")]), InlineNode::text(", baz")])]);
    }

    // Example 390: foo-__(bar)__
    #[test]
    fn example_390() {
        assert_eq!(parse_inlines("foo-__(bar)__"), vec![InlineNode::text("foo-"), InlineNode::strong(vec![InlineNode::text("(bar)")])]);
    }

    // Example 391: **foo bar **
    #[test]
    fn example_391() {
        assert_eq!(parse_inlines("**foo bar **"), vec![InlineNode::text("**foo bar **")]);
    }

    // Example 392: **(**foo)
    #[test]
    fn example_392() {
        assert_eq!(parse_inlines("**(**foo)"), vec![InlineNode::text("**(**foo)")]);
    }

    // Example 394: **Gomphocarpus (*Gomphocarpus physocarpus*, syn.\n
    #[test]
    fn example_394() {
        assert_eq!(parse_inlines("**Gomphocarpus (*Gomphocarpus physocarpus*, syn.\n*Asclepias physocarpa*)**"), vec![InlineNode::strong(vec![InlineNode::text("Gomphocarpus ("), InlineNode::emphasis(vec![InlineNode::text("Gomphocarpus physocarpus")]), InlineNode::text(", syn."), InlineNode::SoftBreak, InlineNode::emphasis(vec![InlineNode::text("Asclepias physocarpa")]), InlineNode::text(")")])]);
    }

    // Example 396: **foo**bar
    #[test]
    fn example_396() {
        assert_eq!(parse_inlines("**foo**bar"), vec![InlineNode::strong(vec![InlineNode::text("foo")]), InlineNode::text("bar")]);
    }

    // Example 397: __foo bar __
    #[test]
    fn example_397() {
        assert_eq!(parse_inlines("__foo bar __"), vec![InlineNode::text("__foo bar __")]);
    }

    // Example 398: __(__foo)
    #[test]
    fn example_398() {
        assert_eq!(parse_inlines("__(__foo)"), vec![InlineNode::text("__(__foo)")]);
    }

    // Example 399: _(__foo__)_
    #[test]
    fn example_399() {
        assert_eq!(parse_inlines("_(__foo__)_"), vec![InlineNode::emphasis(vec![InlineNode::text("("), InlineNode::strong(vec![InlineNode::text("foo")]), InlineNode::text(")")])]);
    }

    // Example 400: __foo__bar
    #[test]
    fn example_400() {
        assert_eq!(parse_inlines("__foo__bar"), vec![InlineNode::text("__foo__bar")]);
    }

    // Example 401: __пристаням__стремятся
    #[test]
    fn example_401() {
        assert_eq!(parse_inlines("__пристаням__стремятся"), vec![InlineNode::text("__пристаням__стремятся")]);
    }

    // Example 402: __foo__bar__baz__
    #[test]
    fn example_402() {
        assert_eq!(parse_inlines("__foo__bar__baz__"), vec![InlineNode::strong(vec![InlineNode::text("foo__bar__baz")])]);
    }

    // Example 403: __(bar)__.
    #[test]
    fn example_403() {
        assert_eq!(parse_inlines("__(bar)__."), vec![InlineNode::strong(vec![InlineNode::text("(bar)")]), InlineNode::text(".")]);
    }

    // Example 405: *foo\nbar*
    #[test]
    fn example_405() {
        assert_eq!(parse_inlines("*foo\nbar*"), vec![InlineNode::emphasis(vec![InlineNode::text("foo"), InlineNode::SoftBreak, InlineNode::text("bar")])]);
    }

    // Example 406: _foo __bar__ baz_
    #[test]
    fn example_406() {
        assert_eq!(parse_inlines("_foo __bar__ baz_"), vec![InlineNode::emphasis(vec![InlineNode::text("foo "), InlineNode::strong(vec![InlineNode::text("bar")]), InlineNode::text(" baz")])]);
    }

    // Example 407: _foo _bar_ baz_
    #[test]
    fn example_407() {
        assert_eq!(parse_inlines("_foo _bar_ baz_"), vec![InlineNode::emphasis(vec![InlineNode::text("foo "), InlineNode::emphasis(vec![InlineNode::text("bar")]), InlineNode::text(" baz")])]);
    }

    // Example 408: __foo_ bar_
    #[test]
    fn example_408() {
        assert_eq!(parse_inlines("__foo_ bar_"), vec![InlineNode::emphasis(vec![InlineNode::emphasis(vec![InlineNode::text("foo")]), InlineNode::text(" bar")])]);
    }

    // Example 409: *foo *bar**
    #[test]
    fn example_409() {
        assert_eq!(parse_inlines("*foo *bar**"), vec![InlineNode::emphasis(vec![InlineNode::text("foo "), InlineNode::emphasis(vec![InlineNode::text("bar")])])]);
    }

    // Example 411: *foo**bar**baz*
    #[test]
    fn example_411() {
        assert_eq!(parse_inlines("*foo**bar**baz*"), vec![InlineNode::emphasis(vec![InlineNode::text("foo"), InlineNode::strong(vec![InlineNode::text("bar")]), InlineNode::text("baz")])]);
    }

    // Example 412: *foo**bar*
    #[test]
    fn example_412() {
        assert_eq!(parse_inlines("*foo**bar*"), vec![InlineNode::emphasis(vec![InlineNode::text("foo**bar")])]);
    }

    // Example 413: ***foo** bar*
    #[test]
    fn example_413() {
        assert_eq!(parse_inlines("***foo** bar*"), vec![InlineNode::emphasis(vec![InlineNode::strong(vec![InlineNode::text("foo")]), InlineNode::text(" bar")])]);
    }

    // Example 414: *foo **bar***
    #[test]
    fn example_414() {
        assert_eq!(parse_inlines("*foo **bar***"), vec![InlineNode::emphasis(vec![InlineNode::text("foo "), InlineNode::strong(vec![InlineNode::text("bar")])])]);
    }

    // Example 415: *foo**bar***
    #[test]
    fn example_415() {
        assert_eq!(parse_inlines("*foo**bar***"), vec![InlineNode::emphasis(vec![InlineNode::text("foo"), InlineNode::strong(vec![InlineNode::text("bar")])])]);
    }

    // Example 416: foo***bar***baz
    #[test]
    fn example_416() {
        assert_eq!(parse_inlines("foo***bar***baz"), vec![InlineNode::text("foo"), InlineNode::emphasis(vec![InlineNode::strong(vec![InlineNode::text("bar")])]), InlineNode::text("baz")]);
    }

    // Example 417: foo******bar*********baz
    #[test]
    fn example_417() {
        assert_eq!(parse_inlines("foo******bar*********baz"), vec![InlineNode::text("foo"), InlineNode::strong(vec![InlineNode::strong(vec![InlineNode::strong(vec![InlineNode::text("bar")])])]), InlineNode::text("***baz")]);
    }

    // Example 418: *foo **bar *baz* bim** bop*
    #[test]
    fn example_418() {
        assert_eq!(parse_inlines("*foo **bar *baz* bim** bop*"), vec![InlineNode::emphasis(vec![InlineNode::text("foo "), InlineNode::strong(vec![InlineNode::text("bar "), InlineNode::emphasis(vec![InlineNode::text("baz")]), InlineNode::text(" bim")]), InlineNode::text(" bop")])]);
    }

    // Example 420: ** is not an empty emphasis
    #[test]
    fn example_420() {
        assert_eq!(parse_inlines("** is not an empty emphasis"), vec![InlineNode::text("** is not an empty emphasis")]);
    }

    // Example 421: **** is not an empty strong emphasis
    #[test]
    fn example_421() {
        assert_eq!(parse_inlines("**** is not an empty strong emphasis"), vec![InlineNode::text("**** is not an empty strong emphasis")]);
    }

    // Example 423: **foo\nbar**
    #[test]
    fn example_423() {
        assert_eq!(parse_inlines("**foo\nbar**"), vec![InlineNode::strong(vec![InlineNode::text("foo"), InlineNode::SoftBreak, InlineNode::text("bar")])]);
    }

    // Example 424: __foo _bar_ baz__
    #[test]
    fn example_424() {
        assert_eq!(parse_inlines("__foo _bar_ baz__"), vec![InlineNode::strong(vec![InlineNode::text("foo "), InlineNode::emphasis(vec![InlineNode::text("bar")]), InlineNode::text(" baz")])]);
    }

    // Example 425: __foo __bar__ baz__
    #[test]
    fn example_425() {
        assert_eq!(parse_inlines("__foo __bar__ baz__"), vec![InlineNode::strong(vec![InlineNode::text("foo "), InlineNode::strong(vec![InlineNode::text("bar")]), InlineNode::text(" baz")])]);
    }

    // Example 426: ____foo__ bar__
    #[test]
    fn example_426() {
        assert_eq!(parse_inlines("____foo__ bar__"), vec![InlineNode::strong(vec![InlineNode::strong(vec![InlineNode::text("foo")]), InlineNode::text(" bar")])]);
    }

    // Example 427: **foo **bar****
    #[test]
    fn example_427() {
        assert_eq!(parse_inlines("**foo **bar****"), vec![InlineNode::strong(vec![InlineNode::text("foo "), InlineNode::strong(vec![InlineNode::text("bar")])])]);
    }

    // Example 428: **foo *bar* baz**
    #[test]
    fn example_428() {
        assert_eq!(parse_inlines("**foo *bar* baz**"), vec![InlineNode::strong(vec![InlineNode::text("foo "), InlineNode::emphasis(vec![InlineNode::text("bar")]), InlineNode::text(" baz")])]);
    }

    // Example 429: **foo*bar*baz**
    #[test]
    fn example_429() {
        assert_eq!(parse_inlines("**foo*bar*baz**"), vec![InlineNode::strong(vec![InlineNode::text("foo"), InlineNode::emphasis(vec![InlineNode::text("bar")]), InlineNode::text("baz")])]);
    }

    // Example 430: ***foo* bar**
    #[test]
    fn example_430() {
        assert_eq!(parse_inlines("***foo* bar**"), vec![InlineNode::strong(vec![InlineNode::emphasis(vec![InlineNode::text("foo")]), InlineNode::text(" bar")])]);
    }

    // Example 431: **foo *bar***
    #[test]
    fn example_431() {
        assert_eq!(parse_inlines("**foo *bar***"), vec![InlineNode::strong(vec![InlineNode::text("foo "), InlineNode::emphasis(vec![InlineNode::text("bar")])])]);
    }

    // Example 432: **foo *bar **baz**\nbim* bop**
    #[test]
    fn example_432() {
        assert_eq!(parse_inlines("**foo *bar **baz**\nbim* bop**"), vec![InlineNode::strong(vec![InlineNode::text("foo "), InlineNode::emphasis(vec![InlineNode::text("bar "), InlineNode::strong(vec![InlineNode::text("baz")]), InlineNode::SoftBreak, InlineNode::text("bim")]), InlineNode::text(" bop")])]);
    }

    // Example 434: __ is not an empty emphasis
    #[test]
    fn example_434() {
        assert_eq!(parse_inlines("__ is not an empty emphasis"), vec![InlineNode::text("__ is not an empty emphasis")]);
    }

    // Example 435: ____ is not an empty strong emphasis
    #[test]
    fn example_435() {
        assert_eq!(parse_inlines("____ is not an empty strong emphasis"), vec![InlineNode::text("____ is not an empty strong emphasis")]);
    }

    // Example 436: foo ***
    #[test]
    fn example_436() {
        assert_eq!(parse_inlines("foo ***"), vec![InlineNode::text("foo ***")]);
    }

    // Example 437: foo *\**
    #[test]
    fn example_437() {
        assert_eq!(parse_inlines("foo *\\**"), vec![InlineNode::text("foo "), InlineNode::emphasis(vec![InlineNode::text("*")])]);
    }

    // Example 438: foo *_*
    #[test]
    fn example_438() {
        assert_eq!(parse_inlines("foo *_*"), vec![InlineNode::text("foo "), InlineNode::emphasis(vec![InlineNode::text("_")])]);
    }

    // Example 439: foo *****
    #[test]
    fn example_439() {
        assert_eq!(parse_inlines("foo *****"), vec![InlineNode::text("foo *****")]);
    }

    // Example 440: foo **\***
    #[test]
    fn example_440() {
        assert_eq!(parse_inlines("foo **\\***"), vec![InlineNode::text("foo "), InlineNode::strong(vec![InlineNode::text("*")])]);
    }

    // Example 441: foo **_**
    #[test]
    fn example_441() {
        assert_eq!(parse_inlines("foo **_**"), vec![InlineNode::text("foo "), InlineNode::strong(vec![InlineNode::text("_")])]);
    }

    // Example 442: **foo*
    #[test]
    fn example_442() {
        assert_eq!(parse_inlines("**foo*"), vec![InlineNode::text("*"), InlineNode::emphasis(vec![InlineNode::text("foo")])]);
    }

    // Example 443: *foo**
    #[test]
    fn example_443() {
        assert_eq!(parse_inlines("*foo**"), vec![InlineNode::emphasis(vec![InlineNode::text("foo")]), InlineNode::text("*")]);
    }

    // Example 444: ***foo**
    #[test]
    fn example_444() {
        assert_eq!(parse_inlines("***foo**"), vec![InlineNode::text("*"), InlineNode::strong(vec![InlineNode::text("foo")])]);
    }

    // Example 445: ****foo*
    #[test]
    fn example_445() {
        assert_eq!(parse_inlines("****foo*"), vec![InlineNode::text("***"), InlineNode::emphasis(vec![InlineNode::text("foo")])]);
    }

    // Example 446: **foo***
    #[test]
    fn example_446() {
        assert_eq!(parse_inlines("**foo***"), vec![InlineNode::strong(vec![InlineNode::text("foo")]), InlineNode::text("*")]);
    }

    // Example 447: *foo****
    #[test]
    fn example_447() {
        assert_eq!(parse_inlines("*foo****"), vec![InlineNode::emphasis(vec![InlineNode::text("foo")]), InlineNode::text("***")]);
    }

    // Example 448: foo ___
    #[test]
    fn example_448() {
        assert_eq!(parse_inlines("foo ___"), vec![InlineNode::text("foo ___")]);
    }

    // Example 449: foo _\__
    #[test]
    fn example_449() {
        assert_eq!(parse_inlines("foo _\\__"), vec![InlineNode::text("foo "), InlineNode::emphasis(vec![InlineNode::text("_")])]);
    }

    // Example 450: foo _*_
    #[test]
    fn example_450() {
        assert_eq!(parse_inlines("foo _*_"), vec![InlineNode::text("foo "), InlineNode::emphasis(vec![InlineNode::text("*")])]);
    }

    // Example 451: foo _____
    #[test]
    fn example_451() {
        assert_eq!(parse_inlines("foo _____"), vec![InlineNode::text("foo _____")]);
    }

    // Example 452: foo __\___
    #[test]
    fn example_452() {
        assert_eq!(parse_inlines("foo __\\___"), vec![InlineNode::text("foo "), InlineNode::strong(vec![InlineNode::text("_")])]);
    }

    // Example 453: foo __*__
    #[test]
    fn example_453() {
        assert_eq!(parse_inlines("foo __*__"), vec![InlineNode::text("foo "), InlineNode::strong(vec![InlineNode::text("*")])]);
    }

    // Example 454: __foo_
    #[test]
    fn example_454() {
        assert_eq!(parse_inlines("__foo_"), vec![InlineNode::text("_"), InlineNode::emphasis(vec![InlineNode::text("foo")])]);
    }

    // Example 455: _foo__
    #[test]
    fn example_455() {
        assert_eq!(parse_inlines("_foo__"), vec![InlineNode::emphasis(vec![InlineNode::text("foo")]), InlineNode::text("_")]);
    }

    // Example 456: ___foo__
    #[test]
    fn example_456() {
        assert_eq!(parse_inlines("___foo__"), vec![InlineNode::text("_"), InlineNode::strong(vec![InlineNode::text("foo")])]);
    }

    // Example 457: ____foo_
    #[test]
    fn example_457() {
        assert_eq!(parse_inlines("____foo_"), vec![InlineNode::text("___"), InlineNode::emphasis(vec![InlineNode::text("foo")])]);
    }

    // Example 458: __foo___
    #[test]
    fn example_458() {
        assert_eq!(parse_inlines("__foo___"), vec![InlineNode::strong(vec![InlineNode::text("foo")]), InlineNode::text("_")]);
    }

    // Example 459: _foo____
    #[test]
    fn example_459() {
        assert_eq!(parse_inlines("_foo____"), vec![InlineNode::emphasis(vec![InlineNode::text("foo")]), InlineNode::text("___")]);
    }

    // Example 460: **foo**
    #[test]
    fn example_460() {
        assert_eq!(parse_inlines("**foo**"), vec![InlineNode::strong(vec![InlineNode::text("foo")])]);
    }

    // Example 461: *_foo_*
    #[test]
    fn example_461() {
        assert_eq!(parse_inlines("*_foo_*"), vec![InlineNode::emphasis(vec![InlineNode::emphasis(vec![InlineNode::text("foo")])])]);
    }

    // Example 462: __foo__
    #[test]
    fn example_462() {
        assert_eq!(parse_inlines("__foo__"), vec![InlineNode::strong(vec![InlineNode::text("foo")])]);
    }

    // Example 463: _*foo*_
    #[test]
    fn example_463() {
        assert_eq!(parse_inlines("_*foo*_"), vec![InlineNode::emphasis(vec![InlineNode::emphasis(vec![InlineNode::text("foo")])])]);
    }

    // Example 464: ****foo****
    #[test]
    fn example_464() {
        assert_eq!(parse_inlines("****foo****"), vec![InlineNode::strong(vec![InlineNode::strong(vec![InlineNode::text("foo")])])]);
    }

    // Example 465: ____foo____
    #[test]
    fn example_465() {
        assert_eq!(parse_inlines("____foo____"), vec![InlineNode::strong(vec![InlineNode::strong(vec![InlineNode::text("foo")])])]);
    }

    // Example 466: ******foo******
    #[test]
    fn example_466() {
        assert_eq!(parse_inlines("******foo******"), vec![InlineNode::strong(vec![InlineNode::strong(vec![InlineNode::strong(vec![InlineNode::text("foo")])])])]);
    }

    // Example 467: ***foo***
    #[test]
    fn example_467() {
        assert_eq!(parse_inlines("***foo***"), vec![InlineNode::emphasis(vec![InlineNode::strong(vec![InlineNode::text("foo")])])]);
    }

    // Example 468: _____foo_____
    #[test]
    fn example_468() {
        assert_eq!(parse_inlines("_____foo_____"), vec![InlineNode::emphasis(vec![InlineNode::strong(vec![InlineNode::strong(vec![InlineNode::text("foo")])])])]);
    }

    // Example 469: *foo _bar* baz_
    #[test]
    fn example_469() {
        assert_eq!(parse_inlines("*foo _bar* baz_"), vec![InlineNode::emphasis(vec![InlineNode::text("foo _bar")]), InlineNode::text(" baz_")]);
    }

    // Example 470: *foo __bar *baz bim__ bam*
    #[test]
    fn example_470() {
        assert_eq!(parse_inlines("*foo __bar *baz bim__ bam*"), vec![InlineNode::emphasis(vec![InlineNode::text("foo "), InlineNode::strong(vec![InlineNode::text("bar *baz bim")]), InlineNode::text(" bam")])]);
    }

    // Example 471: **foo **bar baz**
    #[test]
    fn example_471() {
        assert_eq!(parse_inlines("**foo **bar baz**"), vec![InlineNode::text("**foo "), InlineNode::strong(vec![InlineNode::text("bar baz")])]);
    }

    // Example 472: *foo *bar baz*
    #[test]
    fn example_472() {
        assert_eq!(parse_inlines("*foo *bar baz*"), vec![InlineNode::text("*foo "), InlineNode::emphasis(vec![InlineNode::text("bar baz")])]);
    }

    // Example 478: *a `*`*
    #[test]
    fn example_478() {
        assert_eq!(parse_inlines("*a `*`*"), vec![InlineNode::emphasis(vec![InlineNode::text("a "), InlineNode::code_span("*")])]);
    }

    // Example 479: _a `_`_
    #[test]
    fn example_479() {
        assert_eq!(parse_inlines("_a `_`_"), vec![InlineNode::emphasis(vec![InlineNode::text("a "), InlineNode::code_span("_")])]);
    }


    // =========================================================================
    // parse_inlines — inline link 통합 테스트 (Examples 482-526)
    // =========================================================================

    // Example 482: [link](/uri "title")
    #[test]
    fn example_482() {
        assert_eq!(
            parse_inlines("[link](/uri \"title\")"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "/uri", Some("title"))]
        );
    }

    // Example 483: [link](/uri)
    #[test]
    fn example_483() {
        assert_eq!(
            parse_inlines("[link](/uri)"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "/uri", None)]
        );
    }

    // Example 484: [](./target.md)
    #[test]
    fn example_484() {
        assert_eq!(
            parse_inlines("[](./target.md)"),
            vec![InlineNode::link(vec![], "./target.md", None)]
        );
    }

    // Example 485: [link]()
    #[test]
    fn example_485() {
        assert_eq!(
            parse_inlines("[link]()"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "", None)]
        );
    }

    // Example 486: [link](<>)
    #[test]
    fn example_486() {
        assert_eq!(
            parse_inlines("[link](<>)"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "", None)]
        );
    }

    // Example 487: []()
    #[test]
    fn example_487() {
        assert_eq!(
            parse_inlines("[]()"),
            vec![InlineNode::link(vec![], "", None)]
        );
    }

    // Example 488: [link](/my uri) — space in bare dest → not a link
    #[test]
    fn example_488() {
        assert_eq!(
            parse_inlines("[link](/my uri)"),
            vec![InlineNode::text("[link](/my uri)")]
        );
    }

    // Example 489: [link](</my uri>) — angle brackets allow spaces
    #[test]
    fn example_489() {
        assert_eq!(
            parse_inlines("[link](</my uri>)"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "/my uri", None)]
        );
    }

    // Example 490: [link](foo\nbar) — newline in bare dest → not a link
    #[test]
    fn example_490() {
        assert_eq!(
            parse_inlines("[link](foo\nbar)"),
            vec![InlineNode::text("[link](foo"), InlineNode::SoftBreak, InlineNode::text("bar)")]
        );
    }

    // Example 491: [link](<foo\nbar>) — newline in angle dest → not a link
    #[test]
    fn example_491() {
        assert_eq!(
            parse_inlines("[link](<foo\nbar>)"),
            vec![InlineNode::text("[link]("), InlineNode::raw_html("<foo\nbar>"), InlineNode::text(")")]
        );
    }

    // Example 492: [a](<b)c>)
    #[test]
    fn example_492() {
        assert_eq!(
            parse_inlines("[a](<b)c>)"),
            vec![InlineNode::link(vec![InlineNode::text("a")], "b)c", None)]
        );
    }

    // Example 493: [link](<foo\>) — backslash before > in angle dest
    #[test]
    #[ignore] // TODO: 파서가 `<foo\>` 를 angle dest에서 `\>` escape로 처리하여 닫는 `>`를 못 찾고, 전체를 텍스트로 처리함. 스펙은 `<foo>` raw html로 인식해야 함.
    fn example_493() {
        assert_eq!(
            parse_inlines("[link](<foo\\>)"),
            vec![InlineNode::text("[link]("), InlineNode::text("<foo>)")]
        );
    }

    // Example 494: multi-line, multiple failed links
    #[test]
    fn example_494() {
        assert_eq!(
            parse_inlines("[a](<b)c\n[a](<b)c>\n[a](<b>c)"),
            vec![
                InlineNode::text("[a](<b)c"),
                InlineNode::SoftBreak,
                InlineNode::text("[a](<b)c>"),
                InlineNode::SoftBreak,
                InlineNode::text("[a]("),
                InlineNode::raw_html("<b>"),
                InlineNode::text("c)"),
            ]
        );
    }

    // Example 495: [link](\(foo\))
    #[test]
    fn example_495() {
        assert_eq!(
            parse_inlines("[link](\\(foo\\))"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "(foo)", None)]
        );
    }

    // Example 496: [link](foo(and(bar)))
    #[test]
    fn example_496() {
        assert_eq!(
            parse_inlines("[link](foo(and(bar)))"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "foo(and(bar))", None)]
        );
    }

    // Example 497: [link](foo(and(bar)) — unbalanced parens → not a link
    #[test]
    fn example_497() {
        assert_eq!(
            parse_inlines("[link](foo(and(bar))"),
            vec![InlineNode::text("[link](foo(and(bar))")]
        );
    }

    // Example 498: [link](foo\(and\(bar\))
    #[test]
    fn example_498() {
        assert_eq!(
            parse_inlines("[link](foo\\(and\\(bar\\))"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "foo(and(bar)", None)]
        );
    }

    // Example 499: [link](<foo(and(bar)>)
    #[test]
    fn example_499() {
        assert_eq!(
            parse_inlines("[link](<foo(and(bar)>)"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "foo(and(bar)", None)]
        );
    }

    // Example 500: [link](foo\)\:)
    #[test]
    fn example_500() {
        assert_eq!(
            parse_inlines("[link](foo\\)\\:)"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "foo):", None)]
        );
    }

    // Example 501: fragment and query links
    #[test]
    fn example_501_a() {
        assert_eq!(
            parse_inlines("[link](#fragment)"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "#fragment", None)]
        );
    }

    #[test]
    fn example_501_b() {
        assert_eq!(
            parse_inlines("[link](https://example.com#fragment)"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "https://example.com#fragment", None)]
        );
    }

    #[test]
    fn example_501_c() {
        assert_eq!(
            parse_inlines("[link](https://example.com?foo=3#frag)"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "https://example.com?foo=3#frag", None)]
        );
    }

    // Example 502: [link](foo\bar) — backslash not before punctuation
    #[test]
    fn example_502() {
        assert_eq!(
            parse_inlines("[link](foo\\bar)"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "foo\\bar", None)]
        );
    }

    // Example 503: [link](foo%20b&auml;) — entity not resolved by parser
    #[test]
    fn example_503() {
        assert_eq!(
            parse_inlines("[link](foo%20b&auml;)"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "foo%20b&auml;", None)]
        );
    }

    // Example 504: [link]("title") — quotes as part of destination
    #[test]
    fn example_504() {
        assert_eq!(
            parse_inlines("[link](\"title\")"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "\"title\"", None)]
        );
    }

    // Example 505: title with different quote types
    #[test]
    fn example_505_a() {
        assert_eq!(
            parse_inlines("[link](/url \"title\")"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "/url", Some("title"))]
        );
    }

    #[test]
    fn example_505_b() {
        assert_eq!(
            parse_inlines("[link](/url 'title')"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "/url", Some("title"))]
        );
    }

    #[test]
    fn example_505_c() {
        assert_eq!(
            parse_inlines("[link](/url (title))"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "/url", Some("title"))]
        );
    }

    // Example 506: [link](/url "title \"&quot;") — escaped quote in title
    #[test]
    fn example_506() {
        // &quot; is not resolved by our parser, stays as-is
        assert_eq!(
            parse_inlines("[link](/url \"title \\\"&quot;\")"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "/url", Some("title \"&quot;"))]
        );
    }

    // Example 507: [link](/url\u{a0}"title") — NBSP between url and title → title is part of dest
    #[test]
    fn example_507() {
        // NBSP is not whitespace for link parser, so "title" becomes part of destination
        assert_eq!(
            parse_inlines("[link](/url\u{a0}\"title\")"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "/url\u{a0}\"title\"", None)]
        );
    }

    // Example 508: [link](/url "title "and" title") — not a valid title
    #[test]
    fn example_508() {
        assert_eq!(
            parse_inlines("[link](/url \"title \"and\" title\")"),
            vec![InlineNode::text("[link](/url \"title \"and\" title\")")]
        );
    }

    // Example 509: [link](/url 'title "and" title')
    #[test]
    fn example_509() {
        assert_eq!(
            parse_inlines("[link](/url 'title \"and\" title')"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "/url", Some("title \"and\" title"))]
        );
    }

    // Example 510: [link](   /uri\n  "title"  )
    #[test]
    fn example_510() {
        assert_eq!(
            parse_inlines("[link](   /uri\n  \"title\"  )"),
            vec![InlineNode::link(vec![InlineNode::text("link")], "/uri", Some("title"))]
        );
    }

    // Example 511: [link] (/uri) — space between ] and ( → not a link
    #[test]
    fn example_511() {
        assert_eq!(
            parse_inlines("[link] (/uri)"),
            vec![InlineNode::text("[link] (/uri)")]
        );
    }

    // Example 512: [link [foo [bar]]](/uri) — nested brackets in link text
    #[test]
    fn example_512() {
        assert_eq!(
            parse_inlines("[link [foo [bar]]](/uri)"),
            vec![InlineNode::link(vec![InlineNode::text("link [foo [bar]]")], "/uri", None)]
        );
    }

    // Example 513: [link] bar](/uri) — not a link
    #[test]
    fn example_513() {
        assert_eq!(
            parse_inlines("[link] bar](/uri)"),
            vec![InlineNode::text("[link] bar](/uri)")]
        );
    }

    // Example 514: [link [bar](/uri) — inner link wins
    #[test]
    fn example_514() {
        assert_eq!(
            parse_inlines("[link [bar](/uri)"),
            vec![InlineNode::text("[link "), InlineNode::link(vec![InlineNode::text("bar")], "/uri", None)]
        );
    }

    // Example 515: [link \[bar](/uri) — escaped bracket
    #[test]
    fn example_515() {
        assert_eq!(
            parse_inlines("[link \\[bar](/uri)"),
            vec![InlineNode::link(vec![InlineNode::text("link [bar")], "/uri", None)]
        );
    }

    // Example 516: [link *foo **bar** `#`*](/uri) — emphasis inside link
    #[test]
    fn example_516() {
        assert_eq!(
            parse_inlines("[link *foo **bar** `#`*](/uri)"),
            vec![InlineNode::link(
                vec![
                    InlineNode::text("link "),
                    InlineNode::emphasis(vec![
                        InlineNode::text("foo "),
                        InlineNode::strong(vec![InlineNode::text("bar")]),
                        InlineNode::text(" "),
                        InlineNode::code_span("#"),
                    ]),
                ],
                "/uri",
                None,
            )]
        );
    }

    // Example 517: [![moon](moon.jpg)](/uri) — image inside link
    #[test]
    fn example_517() {
        assert_eq!(
            parse_inlines("[![moon](moon.jpg)](/uri)"),
            vec![InlineNode::link(
                vec![InlineNode::image(vec![InlineNode::text("moon")], "moon.jpg", None)],
                "/uri",
                None,
            )]
        );
    }

    // Example 518: [foo [bar](/uri)](/uri) — links cannot contain links
    #[test]
    fn example_518() {
        assert_eq!(
            parse_inlines("[foo [bar](/uri)](/uri)"),
            vec![InlineNode::text("[foo "), InlineNode::link(vec![InlineNode::text("bar")], "/uri", None), InlineNode::text("](/uri)")]
        );
    }

    // Example 519: [foo *[bar [baz](/uri)](/uri)*](/uri) — nested link precedence
    #[test]
    fn example_519() {
        assert_eq!(
            parse_inlines("[foo *[bar [baz](/uri)](/uri)*](/uri)"),
            vec![
                InlineNode::text("[foo "),
                InlineNode::emphasis(vec![
                    InlineNode::text("[bar "),
                    InlineNode::link(vec![InlineNode::text("baz")], "/uri", None),
                    InlineNode::text("](/uri)"),
                ]),
                InlineNode::text("](/uri)"),
            ]
        );
    }

    // Example 520: ![[[foo](uri1)](uri2)](uri3) — image 미구현
    #[test]
    #[ignore] // TODO: image 구문 미구현
    fn example_520() {
    }

    // Example 521: *[foo*](/uri) — emphasis delimiter inside link text
    #[test]
    fn example_521() {
        assert_eq!(
            parse_inlines("*[foo*](/uri)"),
            vec![InlineNode::text("*"), InlineNode::link(vec![InlineNode::text("foo*")], "/uri", None)]
        );
    }

    // Example 522: [foo *bar](baz*) — unmatched emphasis in link
    #[test]
    fn example_522() {
        assert_eq!(
            parse_inlines("[foo *bar](baz*)"),
            vec![InlineNode::link(vec![InlineNode::text("foo *bar")], "baz*", None)]
        );
    }

    // Example 523: *foo [bar* baz] — emphasis closes before link bracket
    #[test]
    fn example_523() {
        assert_eq!(
            parse_inlines("*foo [bar* baz]"),
            vec![InlineNode::emphasis(vec![InlineNode::text("foo [bar")]), InlineNode::text(" baz]")]
        );
    }

    // Example 524: [foo <bar attr="](baz)"> — raw HTML contains ]
    #[test]
    fn example_524() {
        assert_eq!(
            parse_inlines("[foo <bar attr=\"](baz)\">"),
            vec![InlineNode::text("[foo "), InlineNode::raw_html("<bar attr=\"](baz)\">")]
        );
    }

    // Example 525: [foo`](/uri)` — code span contains ]
    #[test]
    fn example_525() {
        assert_eq!(
            parse_inlines("[foo`](/uri)`"),
            vec![InlineNode::text("[foo"), InlineNode::code_span("](/uri)")]
        );
    }

    // Example 526: [foo<https://example.com/?search=](uri)> — autolink contains ]
    #[test]
    fn example_526() {
        assert_eq!(
            parse_inlines("[foo<https://example.com/?search=](uri)>"),
            vec![InlineNode::text("[foo"), InlineNode::autolink_uri("https://example.com/?search=](uri)")]
        );
    }

    // --- SKIPPED emphasis examples ---
    // Example 352: entity references (미구현)
    // Example 354: multi-paragraph
    // Example 359: entity references (미구현)
    // Example 363: entity references (미구현)
    // Example 380: entity references (미구현)
    // Example 385: entity references (미구현)
    // Example 395: entity references (미구현)
    // Example 404: links/images (미구현)
    // Example 419: links/images (미구현)
    // Example 422: links/images (미구현)
    // Example 433: links/images (미구현)
    // Example 473: links/images (미구현)
    // Example 474: links/images (미구현)
    // Example 475: links/images (미구현)
    // Example 476: links/images (미구현)
    // Example 477: links/images (미구현)
    // Example 480: links/images (미구현)
    // Example 481: links/images (미구현)


    // =========================================================================
    // parse_inlines — line break 통합 테스트
    // =========================================================================

    #[rstest]
    // Example 633: trailing spaces 2개 → hard break
    #[case("foo  \nbaz", vec![InlineNode::text("foo"), InlineNode::HardBreak, InlineNode::text("baz")])]
    // Example 634: \ + \n → hard break
    #[case("foo\\\nbaz", vec![InlineNode::text("foo"), InlineNode::HardBreak, InlineNode::text("baz")])]
    // Example 635: trailing spaces 많이 → hard break
    #[case("foo       \nbaz", vec![InlineNode::text("foo"), InlineNode::HardBreak, InlineNode::text("baz")])]
    // Example 636: hard break 후 leading spaces 제거
    #[case("foo  \n     bar", vec![InlineNode::text("foo"), InlineNode::HardBreak, InlineNode::text("bar")])]
    // Example 637: \ hard break 후 leading spaces 제거
    #[case("foo\\\n     bar", vec![InlineNode::text("foo"), InlineNode::HardBreak, InlineNode::text("bar")])]
    // Example 640: code span 내에서는 hard break 아님 (줄바꿈 → 공백)
    #[case("`code  \nspan`", vec![InlineNode::code_span("code   span")])]
    // Example 641: code span 내에서 \도 리터럴
    #[case("`code\\\nspan`", vec![InlineNode::code_span("code\\ span")])]
    // Example 644: \ + EOF → 리터럴 \ (hard break 아님)
    #[case("foo\\", vec![InlineNode::text("foo\\")])]
    // Example 645: trailing spaces + EOF → spaces 제거 (hard break 아님)
    #[case("foo  ", vec![InlineNode::text("foo  ")])]
    // Example 648: soft line break
    #[case("foo\nbaz", vec![InlineNode::text("foo"), InlineNode::SoftBreak, InlineNode::text("baz")])]
    // Example 649: soft break 전후 spaces 제거
    #[case("foo \n baz", vec![InlineNode::text("foo"), InlineNode::SoftBreak, InlineNode::text("baz")])]
    fn test_parse_inlines_line_break(#[case] input: &str, #[case] expected: Vec<InlineNode>) {
        assert_eq!(parse_inlines(input), expected);
    }

    // =========================================================================
    // parse_inlines — raw HTML 통합 테스트
    // =========================================================================

    #[rstest]
    // Example 613: 기본 open tags
    #[case("<a><bab><c2c>", vec![InlineNode::raw_html("<a>"), InlineNode::raw_html("<bab>"), InlineNode::raw_html("<c2c>")])]
    // Example 614: self-closing tags
    #[case("<a/><b2/>", vec![InlineNode::raw_html("<a/>"), InlineNode::raw_html("<b2/>")])]
    // Example 615: attributes with whitespace/newline
    #[case("<a  /><b2\ndata=\"foo\" >", vec![InlineNode::raw_html("<a  />"), InlineNode::raw_html("<b2\ndata=\"foo\" >")])]
    // Example 617: custom element
    #[case("Foo <responsive-image src=\"foo.jpg\" />", vec![InlineNode::text("Foo "), InlineNode::raw_html("<responsive-image src=\"foo.jpg\" />")])]
    // Example 618: invalid tag names → text
    #[case("<33> <__>", vec![InlineNode::text("<33> <__>")])]
    // Example 619: invalid attribute → text
    #[case("<a h*#ref=\"hi\">", vec![InlineNode::text("<a h*#ref=\"hi\">")])]
    // Example 623: closing tags
    #[case("</a></foo >", vec![InlineNode::raw_html("</a>"), InlineNode::raw_html("</foo >")])]
    // Example 624: closing tag with attributes → text
    #[case("</a href=\"foo\">", vec![InlineNode::text("</a href=\"foo\">")])]
    // Example 625: HTML comment with hyphens
    #[case("foo <!-- this is a --\ncomment - with hyphens -->", vec![InlineNode::text("foo "), InlineNode::raw_html("<!-- this is a --\ncomment - with hyphens -->")])]
    // Example 627: processing instruction
    #[case("foo <?php echo $a; ?>", vec![InlineNode::text("foo "), InlineNode::raw_html("<?php echo $a; ?>")])]
    // Example 628: declaration
    #[case("foo <!ELEMENT br EMPTY>", vec![InlineNode::text("foo "), InlineNode::raw_html("<!ELEMENT br EMPTY>")])]
    // Example 629: CDATA
    #[case("foo <![CDATA[>&<]]>", vec![InlineNode::text("foo "), InlineNode::raw_html("<![CDATA[>&<]]>")])]
    // Example 630: entity in attribute (not resolved in raw html)
    #[case("foo <a href=\"&ouml;\">", vec![InlineNode::text("foo "), InlineNode::raw_html("<a href=\"&ouml;\">")])]
    // Example 631: backslash in attribute
    #[case("foo <a href=\"\\*\">", vec![InlineNode::text("foo "), InlineNode::raw_html("<a href=\"\\*\">")])]
    // Example 632: backslash-escaped quote → invalid, backslash escape produces literal "
    #[case("<a href=\"\\\"\">", vec![InlineNode::text("<a href=\"\"\">")])]
    fn test_parse_inlines_raw_html(#[case] input: &str, #[case] expected: Vec<InlineNode>) {
        assert_eq!(parse_inlines(input), expected);
    }

    // =========================================================================
    // parse_inlines — image 통합 테스트 (Examples 572-593)
    // =========================================================================

    // Example 572: ![foo](/url "title")
    #[test]
    fn example_572() {
        assert_eq!(
            parse_inlines("![foo](/url \"title\")"),
            vec![InlineNode::image(vec![InlineNode::text("foo")], "/url", Some("title"))]
        );
    }

    // Example 573: reference image
    #[test]
    #[ignore] // link reference definition 미구현
    fn example_573() {}

    // Example 574: ![foo ![bar](/url)](/url2) — nested image in alt
    #[test]
    fn example_574() {
        assert_eq!(
            parse_inlines("![foo ![bar](/url)](/url2)"),
            vec![InlineNode::image(
                vec![InlineNode::text("foo "), InlineNode::image(vec![InlineNode::text("bar")], "/url", None)],
                "/url2",
                None,
            )]
        );
    }

    // Example 575: ![foo [bar](/url)](/url2) — link in image alt
    #[test]
    #[ignore] // 파서가 image alt 내부의 link를 처리할 때 image opener가 비활성화됨
    fn example_575() {
        assert_eq!(
            parse_inlines("![foo [bar](/url)](/url2)"),
            vec![InlineNode::image(
                vec![InlineNode::text("foo "), InlineNode::link(vec![InlineNode::text("bar")], "/url", None)],
                "/url2",
                None,
            )]
        );
    }

    // Example 576: reference image
    #[test]
    #[ignore] // link reference definition 미구현
    fn example_576() {}

    // Example 577: reference image
    #[test]
    #[ignore] // link reference definition 미구현
    fn example_577() {}

    // Example 578: ![foo](train.jpg)
    #[test]
    fn example_578() {
        assert_eq!(
            parse_inlines("![foo](train.jpg)"),
            vec![InlineNode::image(vec![InlineNode::text("foo")], "train.jpg", None)]
        );
    }

    // Example 579: My ![foo bar](/path/to/train.jpg  "title"   )
    #[test]
    fn example_579() {
        assert_eq!(
            parse_inlines("My ![foo bar](/path/to/train.jpg  \"title\"   )"),
            vec![InlineNode::text("My "), InlineNode::image(vec![InlineNode::text("foo bar")], "/path/to/train.jpg", Some("title"))]
        );
    }

    // Example 580: ![foo](<url>)
    #[test]
    fn example_580() {
        assert_eq!(
            parse_inlines("![foo](<url>)"),
            vec![InlineNode::image(vec![InlineNode::text("foo")], "url", None)]
        );
    }

    // Example 581: ![](/url)
    #[test]
    fn example_581() {
        assert_eq!(
            parse_inlines("![](/url)"),
            vec![InlineNode::image(vec![], "/url", None)]
        );
    }

    // Example 582: reference image
    #[test]
    #[ignore] // link reference definition 미구현
    fn example_582() {}

    // Example 583: reference image
    #[test]
    #[ignore] // link reference definition 미구현
    fn example_583() {}

    // Example 584: reference image
    #[test]
    #[ignore] // link reference definition 미구현
    fn example_584() {}

    // Example 585: reference image
    #[test]
    #[ignore] // link reference definition 미구현
    fn example_585() {}

    // Example 586: reference image
    #[test]
    #[ignore] // link reference definition 미구현
    fn example_586() {}

    // Example 587: reference image
    #[test]
    #[ignore] // link reference definition 미구현
    fn example_587() {}

    // Example 588: reference image
    #[test]
    #[ignore] // link reference definition 미구현
    fn example_588() {}

    // Example 589: reference image
    #[test]
    #[ignore] // link reference definition 미구현
    fn example_589() {}

    // Example 590: ![[foo]] — not a valid image syntax
    #[test]
    fn example_590() {
        assert_eq!(
            parse_inlines("![[foo]]"),
            vec![InlineNode::text("![[foo]]")]
        );
    }

    // Example 591: reference image
    #[test]
    #[ignore] // link reference definition 미구현
    fn example_591() {}

    // Example 592: !\[foo] — escaped ! before [
    #[test]
    fn example_592() {
        assert_eq!(
            parse_inlines("!\\[foo]"),
            vec![InlineNode::text("![foo]")]
        );
    }

    // Example 593: \![foo] — escaped ! (not image)
    #[test]
    fn example_593() {
        assert_eq!(
            parse_inlines("\\![foo]"),
            vec![InlineNode::text("![foo]")]
        );
    }
}
