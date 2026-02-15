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

/// 매칭된 emphasis/strong 쌍
#[derive(Debug)]
struct EmphasisMatch {
    opener_idx: usize,
    closer_idx: usize,
    consume: usize, // 1 = emphasis, 2 = strong
}

/// Delimiter 스택을 처리하여 emphasis/strong을 생성한다
pub fn process_emphasis(
    nodes: Vec<InlineNode>,
    delimiters: Vec<DelimiterRun>,
) -> Vec<InlineNode> {
    let mut delims = delimiters;

    // 매칭 찾기
    let matches = find_matches(&mut delims);

    if matches.is_empty() {
        return nodes;
    }

    // 매칭을 기반으로 트리 구성
    build_tree(nodes, &delims, &matches)
}

/// 모든 delimiter 매칭을 찾는다
fn find_matches(delims: &mut Vec<DelimiterRun>) -> Vec<EmphasisMatch> {
    let mut matches = Vec::new();
    let mut closer_idx = 0;

    while closer_idx < delims.len() {
        if !delims[closer_idx].can_close || delims[closer_idx].length == 0 {
            closer_idx += 1;
            continue;
        }

        let closer_char = delims[closer_idx].char;

        // 여는 delimiter 검색 (가장 가까운 것)
        let mut found = false;
        for opener_idx in (0..closer_idx).rev() {
            let opener = &delims[opener_idx];
            if opener.char != closer_char || !opener.can_open || opener.length == 0 {
                continue;
            }

            // Rule 9/10: 합이 3의 배수 규칙
            if (opener.can_open && opener.can_close)
                || (delims[closer_idx].can_open && delims[closer_idx].can_close)
            {
                if (opener.original_length + delims[closer_idx].original_length) % 3 == 0
                    && opener.original_length % 3 != 0
                    && delims[closer_idx].original_length % 3 != 0
                {
                    continue;
                }
            }

            // strong (2개) vs emphasis (1개)
            let consume = if opener.length >= 2 && delims[closer_idx].length >= 2 {
                2
            } else {
                1
            };

            matches.push(EmphasisMatch {
                opener_idx,
                closer_idx,
                consume,
            });

            delims[opener_idx].length -= consume;
            delims[closer_idx].length -= consume;

            // opener~closer 사이의 delimiter들 무효화
            for i in (opener_idx + 1)..closer_idx {
                delims[i].length = 0;
            }

            found = true;
            break;
        }

        if !found {
            closer_idx += 1;
        }
        // found이면 같은 closer에서 재시도 (남은 길이가 있을 수 있음)
        else if delims[closer_idx].length == 0 {
            closer_idx += 1;
        }
    }

    matches
}

/// 매칭 결과를 기반으로 노드 트리를 구성한다
fn build_tree(
    nodes: Vec<InlineNode>,
    delims: &[DelimiterRun],
    matches: &[EmphasisMatch],
) -> Vec<InlineNode> {
    // 각 노드가 어떤 매칭의 opener/closer인지 매핑
    // opener_pos → (match_idx, consume, is_opener)
    // 재귀적으로 처리: 가장 바깥 매칭부터

    if matches.is_empty() {
        return nodes;
    }

    // 매칭을 opener position 기준으로 정렬 (바깥→안쪽 순서)
    let mut sorted_matches: Vec<&EmphasisMatch> = matches.iter().collect();
    sorted_matches.sort_by_key(|m| delims[m.opener_idx].position);

    // 재귀적으로 구성: 가장 바깥 매칭부터 처리
    build_from_matches(&nodes, delims, &sorted_matches, 0, nodes.len())
}

/// 노드 범위 [start, end) 내에서 매칭을 적용하여 트리 구성
fn build_from_matches(
    nodes: &[InlineNode],
    delims: &[DelimiterRun],
    matches: &[&EmphasisMatch],
    start: usize,
    end: usize,
) -> Vec<InlineNode> {
    // 이 범위에 해당하는 매칭 찾기
    let relevant: Vec<&&EmphasisMatch> = matches
        .iter()
        .filter(|m| {
            let op = delims[m.opener_idx].position;
            let cl = delims[m.closer_idx].position;
            op >= start && cl < end
        })
        .collect();

    if relevant.is_empty() {
        // 매칭 없으면 노드를 그대로 반환 (빈 텍스트 제거)
        return nodes[start..end]
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
    }

    // 가장 바깥 (겹치지 않는) 매칭들을 찾기
    let mut result = Vec::new();
    let mut pos = start;

    // 겹치지 않는 매칭들을 순서대로 처리
    let outermost = find_outermost_matches(&relevant, delims);

    for m in &outermost {
        let opener_pos = delims[m.opener_idx].position;
        let closer_pos = delims[m.closer_idx].position;

        // opener 앞의 노드들
        for i in pos..opener_pos {
            let node = &nodes[i];
            // delimiter 텍스트에서 소비된 부분 조정
            let adjusted = adjust_delimiter_node(node, delims, i, matches);
            if let InlineNode::Text(t) = &adjusted {
                if t.0.is_empty() {
                    continue;
                }
            }
            result.push(adjusted);
        }

        // opener 노드 (소비되지 않은 부분만)
        let opener_remaining = get_remaining_delimiter_text(delims, m.opener_idx, true);
        if !opener_remaining.is_empty() {
            result.push(InlineNode::Text(TextNode(opener_remaining)));
        }

        // children: opener+1 ~ closer-1 을 재귀적으로 처리
        let children = build_from_matches(nodes, delims, matches, opener_pos + 1, closer_pos);

        // emphasis or strong 노드 생성
        if m.consume == 2 {
            result.push(InlineNode::Strong(StrongNode::new(children)));
        } else {
            result.push(InlineNode::Emphasis(EmphasisNode::new(children)));
        }

        // closer 노드 (소비되지 않은 부분만)
        let closer_remaining = get_remaining_delimiter_text(delims, m.closer_idx, false);
        if !closer_remaining.is_empty() {
            result.push(InlineNode::Text(TextNode(closer_remaining)));
        }

        pos = closer_pos + 1;
    }

    // 나머지 노드들
    for i in pos..end {
        let node = &nodes[i];
        let adjusted = adjust_delimiter_node(node, delims, i, matches);
        if let InlineNode::Text(t) = &adjusted {
            if t.0.is_empty() {
                continue;
            }
        }
        result.push(adjusted);
    }

    result
}

/// 겹치지 않는 가장 바깥 매칭들을 찾는다
fn find_outermost_matches<'a>(
    matches: &[&&'a EmphasisMatch],
    delims: &[DelimiterRun],
) -> Vec<&'a EmphasisMatch> {
    let mut result: Vec<&EmphasisMatch> = Vec::new();
    let mut last_closer_pos = 0;

    for m in matches.iter() {
        let opener_pos = delims[m.opener_idx].position;
        let closer_pos = delims[m.closer_idx].position;

        // 이전 매칭과 겹치지 않는지 확인
        if opener_pos >= last_closer_pos {
            result.push(m);
            last_closer_pos = closer_pos + 1;
        }
        // 중첩된 경우는 재귀에서 처리됨
    }

    result
}

/// delimiter에서 소비되지 않은 텍스트 반환
/// is_opener=true면 앞부분(소비 안 된 것), false면 뒷부분
fn get_remaining_delimiter_text(
    delims: &[DelimiterRun],
    delim_idx: usize,
    is_opener: bool,
) -> String {
    let d = &delims[delim_idx];
    let remaining = d.length;
    let consumed = d.original_length - remaining;
    let c = match d.char {
        DelimiterChar::Asterisk => '*',
        DelimiterChar::Underscore => '_',
    };

    if is_opener {
        // opener: 앞부분은 소비 안 됨, 뒷부분이 소비됨
        std::iter::repeat(c).take(remaining).collect()
    } else {
        // closer: 앞부분이 소비됨, 뒷부분은 소비 안 됨
        std::iter::repeat(c).take(remaining).collect()
    }
}

/// delimiter가 아닌 노드이거나, 매칭에 참여하지 않는 delimiter 노드를 반환
fn adjust_delimiter_node(
    node: &InlineNode,
    delims: &[DelimiterRun],
    node_pos: usize,
    matches: &[&EmphasisMatch],
) -> InlineNode {
    // 이 위치가 매칭된 opener나 closer인지 확인
    for m in matches {
        let opener_pos = delims[m.opener_idx].position;
        let closer_pos = delims[m.closer_idx].position;
        if node_pos == opener_pos || node_pos == closer_pos {
            // 이건 outermost에서 처리하므로 여기선 빈 텍스트 반환
            return InlineNode::Text(TextNode("".to_string()));
        }
    }

    // 무효화된 delimiter (length=0, 매칭 사이에 있던 것)
    for d in delims {
        if d.position == node_pos && d.length == 0 && d.original_length > 0 {
            return InlineNode::Text(TextNode("".to_string()));
        }
    }

    node.clone()
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
