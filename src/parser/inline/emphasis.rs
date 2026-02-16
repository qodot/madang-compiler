//! Emphasis / Strong Emphasis 파서
//!
//! Delimiter run 알고리즘으로 `*`/`_` emphasis를 처리합니다.
//! CommonMark 명세 Section 6.2: https://spec.commonmark.org/0.31.2/#emphasis-and-strong-emphasis

use crate::node::{EmphasisNode, InlineNode, StrongNode, TextNode};

/// Delimiter run의 종류
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DelimiterChar {
    Asterisk,   // *
    Underscore, // _
}

/// Delimiter run 정보
#[derive(Debug, Clone)]
pub struct DelimiterRun {
    /// 구분자 종류
    pub char: DelimiterChar,
    /// 구분자 개수 (원래 길이)
    pub original_length: usize,
    /// 남은 구분자 개수
    pub length: usize,
    /// 열림 가능 여부
    pub can_open: bool,
    /// 닫힘 가능 여부
    pub can_close: bool,
    /// 인라인 노드 리스트에서의 위치 인덱스
    pub position: usize,
}

/// 문자가 Unicode whitespace인지 확인
fn is_unicode_whitespace(c: char) -> bool {
    c.is_whitespace()
}

/// 문자가 Unicode punctuation인지 확인
fn is_unicode_punctuation(c: char) -> bool {
    if crate::parser::inline::backslash_escape::is_ascii_punctuation(c) {
        return true;
    }
    if !c.is_ascii() {
        return !c.is_alphanumeric() && !c.is_whitespace();
    }
    false
}

/// Delimiter run의 can_open / can_close를 결정한다
pub fn compute_flanking(
    char: DelimiterChar,
    before: Option<char>,
    after: Option<char>,
) -> (bool, bool) {
    let before = before.unwrap_or(' ');
    let after = after.unwrap_or(' ');

    let after_ws = is_unicode_whitespace(after);
    let after_punct = is_unicode_punctuation(after);
    let before_ws = is_unicode_whitespace(before);
    let before_punct = is_unicode_punctuation(before);

    let left_flanking = !after_ws && (!after_punct || before_ws || before_punct);
    let right_flanking = !before_ws && (!before_punct || after_ws || after_punct);

    match char {
        DelimiterChar::Asterisk => (left_flanking, right_flanking),
        DelimiterChar::Underscore => (
            left_flanking && (!right_flanking || before_punct),
            right_flanking && (!left_flanking || after_punct),
        ),
    }
}

/// Delimiter 스택을 처리하여 emphasis/strong을 생성한다
///
/// cmark 참조 구현의 process_emphasis 알고리즘을 따른다:
/// - closer를 앞에서 뒤로 순회
/// - 각 closer에 대해 가장 가까운 opener를 뒤에서 앞으로 검색
/// - 매칭 시 사이 노드들을 em/strong children으로 이동
/// - opener/closer 텍스트 길이를 줄이고, 0이면 삭제
pub fn process_emphasis(
    mut nodes: Vec<InlineNode>,
    mut delimiters: Vec<DelimiterRun>,
) -> Vec<InlineNode> {
    // openers_bottom: delimiter 종류별 검색 하한
    // [0..2] = underscore (can_open: false, length%3 = 0,1,2)
    // [3..5] = underscore (can_open: true, length%3 = 0,1,2)
    // [6..8] = asterisk (can_open: false, length%3 = 0,1,2)
    // [9..11] = asterisk (can_open: true, length%3 = 0,1,2)
    let mut openers_bottom: [usize; 12] = [0; 12];

    let mut closer_idx = 0;

    while closer_idx < delimiters.len() {
        if !delimiters[closer_idx].can_close || delimiters[closer_idx].length == 0 {
            closer_idx += 1;
            continue;
        }

        let closer_char = delimiters[closer_idx].char;

        // openers_bottom 인덱스 계산
        let bottom_idx = opener_bottom_index(
            closer_char,
            delimiters[closer_idx].can_open,
            delimiters[closer_idx].length,
        );

        // opener 검색 (가장 가까운 것, 뒤에서 앞으로)
        let mut opener_found = None;
        for oi in (0..closer_idx).rev() {
            if delimiters[oi].position < openers_bottom[bottom_idx] {
                break;
            }
            let opener = &delimiters[oi];
            if opener.char != closer_char || !opener.can_open || opener.length == 0 {
                continue;
            }

            // Rule 9/10: 합이 3의 배수 규칙
            if (opener.can_open && opener.can_close)
                || (delimiters[closer_idx].can_open && delimiters[closer_idx].can_close)
            {
                if (opener.length + delimiters[closer_idx].length) % 3 == 0
                    && opener.length % 3 != 0
                    && delimiters[closer_idx].length % 3 != 0
                {
                    continue;
                }
            }

            opener_found = Some(oi);
            break;
        }

        if let Some(opener_idx) = opener_found {
            // strong (2개) vs emphasis (1개)
            let use_delims = if delimiters[opener_idx].length >= 2
                && delimiters[closer_idx].length >= 2
            {
                2
            } else {
                1
            };

            let opener_pos = delimiters[opener_idx].position;
            let closer_pos = delimiters[closer_idx].position;

            // opener/closer 텍스트 길이 줄이기
            shrink_delimiter_text(&mut nodes, opener_pos, use_delims, true);
            shrink_delimiter_text(&mut nodes, closer_pos, use_delims, false);

            delimiters[opener_idx].length -= use_delims;
            delimiters[closer_idx].length -= use_delims;

            // opener~closer 사이의 delimiter 무효화
            for i in (opener_idx + 1)..closer_idx {
                delimiters[i].length = 0;
                delimiters[i].can_open = false;
                delimiters[i].can_close = false;
            }

            // opener+1 ~ closer-1 사이 노드들을 children으로 추출
            let children: Vec<InlineNode> = nodes[(opener_pos + 1)..closer_pos]
                .iter()
                .filter(|n| {
                    if let InlineNode::Text(t) = n {
                        !t.0.is_empty()
                    } else {
                        true
                    }
                })
                .cloned()
                .collect();

            // emph/strong 노드 생성
            let emph_node = if use_delims == 2 {
                InlineNode::Strong(StrongNode::new(children))
            } else {
                InlineNode::Emphasis(EmphasisNode::new(children))
            };

            // opener+1 ~ closer-1 을 emph_node 하나로 교체
            // 사이 노드들을 빈 텍스트로 만들고 opener+1에 emph_node 배치
            for i in (opener_pos + 1)..closer_pos {
                nodes[i] = InlineNode::Text(TextNode("".to_string()));
            }
            // opener+1 위치에 emph_node 배치
            if opener_pos + 1 < closer_pos {
                nodes[opener_pos + 1] = emph_node;
            } else {
                // opener와 closer가 인접한 경우 (빈 emphasis)
                // closer 위치에 emph_node + 빈 텍스트로 처리
                // 실제로 이 경우는 매우 드물지만 안전하게 처리
                nodes[closer_pos] = emph_node;
                // closer가 emph_node로 대체되므로 여기서 continue
                if delimiters[closer_idx].length == 0 {
                    closer_idx += 1;
                }
                continue;
            }

            // opener가 빈 텍스트가 되면 삭제 (position 조정 불필요, 나중에 필터링)
            if delimiters[opener_idx].length == 0 {
                // delimiter 제거는 하지 않음 (length=0으로 이미 무효화)
            }

            // closer length가 0이면 다음으로
            if delimiters[closer_idx].length == 0 {
                closer_idx += 1;
            }
            // 아직 남아있으면 같은 closer에서 재시도
        } else {
            // opener 못 찾음 → 하한 업데이트
            openers_bottom[bottom_idx] = delimiters[closer_idx].position;
            if !delimiters[closer_idx].can_open {
                delimiters[closer_idx].length = 0;
            }
            closer_idx += 1;
        }
    }

    // 빈 텍스트 노드 제거
    nodes
        .into_iter()
        .filter(|n| {
            if let InlineNode::Text(t) = n {
                !t.0.is_empty()
            } else {
                true
            }
        })
        .collect()
}

/// openers_bottom 인덱스 계산
fn opener_bottom_index(char: DelimiterChar, can_open: bool, length: usize) -> usize {
    let base = match char {
        DelimiterChar::Underscore => 0,
        DelimiterChar::Asterisk => 6,
    };
    let open_offset = if can_open { 3 } else { 0 };
    base + open_offset + (length % 3)
}

/// delimiter 텍스트 노드의 길이를 줄인다
/// is_opener: true면 뒷부분에서 제거 (opener는 뒤쪽이 소비됨)
///           false면 앞부분에서 제거 (closer는 앞쪽이 소비됨)
fn shrink_delimiter_text(nodes: &mut [InlineNode], pos: usize, amount: usize, is_opener: bool) {
    if let InlineNode::Text(ref mut t) = nodes[pos] {
        if is_opener {
            // opener: 뒷부분 제거
            let new_len = t.0.len().saturating_sub(amount);
            t.0.truncate(new_len);
        } else {
            // closer: 앞부분 제거
            if amount < t.0.len() {
                t.0 = t.0[amount..].to_string();
            } else {
                t.0.clear();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::InlineNode;
    use rstest::rstest;

    #[rstest]
    #[case(DelimiterChar::Asterisk, None, Some('f'), true, false)]
    #[case(DelimiterChar::Asterisk, Some('f'), None, false, true)]
    #[case(DelimiterChar::Asterisk, Some(' '), Some('f'), true, false)]
    #[case(DelimiterChar::Asterisk, Some('f'), Some(' '), false, true)]
    #[case(DelimiterChar::Asterisk, Some(' '), Some(' '), false, false)]
    #[case(DelimiterChar::Underscore, None, Some('f'), true, false)]
    #[case(DelimiterChar::Underscore, Some('f'), None, false, true)]
    #[case(DelimiterChar::Underscore, Some('a'), Some('b'), false, false)]
    #[case(DelimiterChar::Asterisk, Some('a'), Some('b'), true, true)]
    #[case(DelimiterChar::Asterisk, Some('.'), Some('f'), true, false)]
    #[case(DelimiterChar::Underscore, Some('.'), Some('f'), true, false)]
    fn test_compute_flanking(
        #[case] char: DelimiterChar,
        #[case] before: Option<char>,
        #[case] after: Option<char>,
        #[case] expected_open: bool,
        #[case] expected_close: bool,
    ) {
        let (can_open, can_close) = compute_flanking(char, before, after);
        assert_eq!((can_open, can_close), (expected_open, expected_close));
    }
}
