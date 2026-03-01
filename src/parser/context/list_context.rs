//! ListContext: List 파싱 중 상태

use super::{ItemLine, LineResult, ListItemStart, NoneContext, ParsingContext};
use crate::node::{
    BlockNode, ListItemNode, ListNode, ParagraphNode,
};
use crate::parser::helpers::{calculate_indent, consume_indent, consume_indent_and_expand};
use crate::parser::list_item;
use crate::parser::thematic_break;

pub struct ListContext {
    /// 첫 아이템의 시작 정보 (리스트 타입 결정용)
    pub first_item_start: ListItemStart,
    /// 완성된 아이템들의 내용
    pub items: Vec<Vec<ItemLine>>,
    /// 현재 아이템의 줄들
    pub current_item_lines: Vec<ItemLine>,
    /// 현재 아이템의 content_indent (continuation line 판단용)
    pub current_content_indent: usize,
    /// tight 리스트 여부 (아이템 간 빈 줄 없음)
    pub tight: bool,
    /// 대기 중인 빈 줄 개수 (continuation 시 내용에 추가)
    pub pending_blank_count: usize,
}

impl ListContext {
    /// List 상태에서 줄 처리
    /// 반환: (새로 완성된 노드들, 새 컨텍스트)
    pub fn parse(self, line: &str) -> LineResult {
        // 빈 줄 처리 (항상 계속, 개수 추적)
        if line.trim().is_empty() {
            let context = ParsingContext::List(ListContext {
                pending_blank_count: self.pending_blank_count + 1,
                ..self
            });
            return (vec![], context);
        }

        let indent = calculate_indent(line);
        let first_content_indent = self.first_item_start.content_indent;

        // 빈 아이템 뒤 빈 줄 + continuation (새 아이템이 아닌 줄) → list 종료
        let current_is_empty = self.current_item_lines.iter().all(|l| l.content.trim().is_empty());
        if current_is_empty && self.pending_blank_count > 0 {
            // 같은 마커의 새 아이템이면 list 계속, 그 외는 종료
            if let Ok(list_item::ListItemOk::Started(new_start)) = list_item::parse(line) {
                if self.first_item_start.marker.is_same_type(&new_start.marker) {
                    // 새 아이템 → list 계속 (아래 로직으로 fall through)
                } else {
                    return self.end_and_reprocess(line);
                }
            } else {
                return self.end_and_reprocess(line);
            }
        }

        // 1. Thematic break 우선 체크 (Example 60: thematic break가 list item보다 우선)
        if indent < self.current_content_indent {
            if thematic_break::parse(line).is_ok() {
                return self.end_and_reprocess(line);
            }
            if let Ok(list_item::ListItemOk::Started(new_start)) = list_item::parse(line) {
                if self.first_item_start.marker.is_same_type(&new_start.marker) {
                    // 같은 마커 타입 → 새 아이템으로 계속
                    let items = push_item(self.items, self.current_item_lines);
                    let tight = self.tight && self.pending_blank_count == 0;
                    let new_content_indent = new_start.content_indent;
                    let context = ParsingContext::List(ListContext {
                        first_item_start: self.first_item_start,
                        items,
                        current_item_lines: vec![ItemLine::text(new_start.content)],
                        current_content_indent: new_content_indent,
                        tight,
                        pending_blank_count: 0,
                    });
                    return (vec![], context);
                }
                // 다른 마커 타입 → 리스트 종료
                return self.end_and_reprocess(line);
            }
            // 빈 줄 이후 under-indented 줄은 리스트 종료 (같은 타입 아이템만 예외)
            // Example 313: "    3. c"는 content_indent 미만이고 빈 줄 이후이므로 코드 블록
            if self.pending_blank_count > 0 {
                return self.end_and_reprocess(line);
            }
            // 4칸 이상 들여쓰기 + first_content_indent 이상이면 text_only continuation
            // Example 303: 4칸 들여쓰기된 마커는 텍스트 전용
            if indent > 3 && indent >= first_content_indent {
                let strip_amount = indent.min(self.current_content_indent);
                let content = consume_indent_and_expand(line, strip_amount);
                return self.continue_with(ItemLine::text_only(content));
            }
        }

        // 2. Continuation line (Example 303: first_content_indent 기준)
        if indent >= first_content_indent {
            let strip_amount = indent.min(self.current_content_indent);
            let content = consume_indent_and_expand(line, strip_amount);
            return self.continue_with(ItemLine::text(content));
        }

        // 3. Lazy continuation: 빈 줄 전이고 block-level 구조 시작이 아니면
        //    현재 아이템의 paragraph continuation으로 처리 (CommonMark §5.3)
        if self.pending_blank_count == 0 && !is_block_start_for_lazy(line) {
            let content = line.to_string();
            return self.continue_with(ItemLine::text(content));
        }

        // 4. first_content_indent 미만 + 새 아이템 아님 → 종료
        self.end_and_reprocess(line)
    }

    /// 현재 아이템에 줄 추가하여 계속
    /// NOTE: 아이템 내 빈 줄은 tight 판정에 영향 없음
    /// tight는 아이템 **간** 빈 줄만 추적하고,
    /// 아이템 **내** 직접 블록 사이 빈 줄은 build_list_node에서 재파싱 후 판정
    fn continue_with(self, item_line: ItemLine) -> LineResult {
        let mut lines = self.current_item_lines;
        for _ in 0..self.pending_blank_count {
            lines.push(ItemLine::blank());
        }
        lines.push(item_line);
        // tight는 유지 (아이템 내 빈 줄은 tight 판정에 영향 없음)
        let context = ParsingContext::List(ListContext {
            first_item_start: self.first_item_start,
            items: self.items,
            current_item_lines: lines,
            current_content_indent: self.current_content_indent,
            tight: self.tight, // 변경: pending_blank_count 무시
            pending_blank_count: 0,
        });
        (vec![], context)
    }

    /// 리스트 종료 후 현재 줄을 재처리
    fn end_and_reprocess(self, line: &str) -> LineResult {
        let list_node = self.build_list_node();
        let (more_nodes, new_context) = NoneContext.parse(line);
        let mut nodes = vec![list_node];
        nodes.extend(more_nodes);
        (nodes, new_context)
    }

    /// List 노드 생성 (완성된 아이템들로부터)
    pub fn build_list_node(self) -> BlockNode {
        let (list_type, start) = self.first_item_start.marker.to_list_type();
        let all_items = push_item(self.items, self.current_item_lines);

        // 각 아이템을 파싱하여 ListItem 노드 생성
        // 동시에 아이템 내 직접 블록 사이 빈 줄 여부 확인
        let mut item_has_internal_blank = false;
        let list_children: Vec<ListItemNode> = all_items
            .iter()
            .map(|item_lines| {
                let (parsed_blocks, has_blank) = parse_item_lines_with_blank_info(item_lines);
                if has_blank {
                    item_has_internal_blank = true;
                }
                ListItemNode::new(parsed_blocks)
            })
            .collect();

        // tight 판정:
        // - 아이템 간 빈 줄이 있으면 loose (self.tight == false)
        // - 아이템 내 직접 블록 사이 빈 줄이 있으면 loose
        let tight = self.tight && !item_has_internal_blank;

        BlockNode::List(ListNode::new(list_type, start, tight, list_children))
    }
}

/// 아이템 리스트에 아이템 추가
fn push_item(mut items: Vec<Vec<ItemLine>>, item: Vec<ItemLine>) -> Vec<Vec<ItemLine>> {
    items.push(item);
    items
}

/// 리스트 아이템 내용 파싱
/// text_only 플래그를 고려하여 처리
fn parse_item_lines(lines: &[ItemLine]) -> Vec<BlockNode> {
    parse_item_lines_with_blank_info(lines).0
}

/// 리스트 아이템 내용 파싱 + 직접 블록 사이 빈 줄 여부 반환
/// 반환: (파싱된 블록들, 아이템 내 직접 블록 사이 빈 줄 여부)
fn parse_item_lines_with_blank_info(lines: &[ItemLine]) -> (Vec<BlockNode>, bool) {
    // text_only가 있는지 확인
    let has_any_text_only = lines.iter().any(|l| l.text_only);

    if has_any_text_only {
        // text_only가 있는 경우: 청크 단위로 처리
        let blocks = parse_item_lines_with_text_only(lines);
        // 빈 줄로 분리된 청크가 2개 이상이면 has_blank = true
        let blank_count = lines.iter().filter(|l| l.content.trim().is_empty() && !l.text_only).count();
        let has_blank = blank_count > 0 && blocks.len() > 1;
        (blocks, has_blank)
    } else {
        // text_only가 없는 경우: 전체를 한 번에 재파싱
        let content: String = lines
            .iter()
            .map(|l| l.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        // parse_blocks를 사용: ref def를 제거하지 않으므로 tight/loose 판정이 정확함
        // ref def 추출은 최상위 parse()에서 extract_link_ref_defs가 재귀적으로 수행
        let blocks = crate::parser::parse_blocks(&content);

        // 아이템 내 **직접** 블록 사이 빈 줄 확인
        // sublist 내의 빈 줄은 제외해야 함
        // 직접 블록: blocks의 최상위 블록들
        // 빈 줄로 분리된 직접 블록이 2개 이상인지 확인
        let has_blank = has_direct_blank_between_blocks(lines, &blocks);

        (blocks, has_blank)
    }
}

/// 아이템의 직접 자식 블록 사이에 빈 줄이 있는지 확인
/// sublist 내의 빈 줄은 제외
/// 
/// CommonMark 명세:
/// "A list is loose if any of its constituent list items directly contain
///  two block-level elements with a blank line between them"
/// 
/// 핵심: 직접(directly) 포함하는 블록 사이의 빈 줄만 카운트
/// sublist는 하나의 블록이므로, sublist 내부의 빈 줄은 무시
fn has_direct_blank_between_blocks(lines: &[ItemLine], blocks: &[BlockNode]) -> bool {
    // 빈 줄이 없으면 false
    let has_blank = lines.iter().any(|l| l.content.trim().is_empty());
    if !has_blank {
        return false;
    }

    // 직접 자식 중 List가 아닌 블록의 개수
    let non_list_count = blocks
        .iter()
        .filter(|b| !matches!(b, BlockNode::List(_)))
        .count();

    // 비-List 블록이 2개 이상이면, 그 사이에 빈 줄이 있을 수 있음
    // (빈 줄이 있다는 것은 이미 확인했으므로)
    if non_list_count >= 2 {
        return true;
    }

    // 비-List 블록이 1개 이하이고, List 블록이 있는 경우
    // → 빈 줄은 List 내부에 있거나 List와 다른 블록 사이에 있음
    // → List와 다른 블록 사이의 빈 줄은 outer를 loose로 만듦
    // 
    // 예: * foo
    //       * bar
    //     
    //       baz
    // foo → bar sublist → baz
    // foo와 sublist 사이에는 빈 줄 없음
    // sublist와 baz 사이에 빈 줄 있음 → loose
    
    // 직접 자식 블록 사이에 빈 줄이 있는지 판정
    // sublist 내부의 빈 줄은 outer를 loose로 만들지 않음
    //
    // 전략: lines를 순서대로 스캔하여
    // - non-empty content 뒤에 blank가 오고, 그 뒤에 또 non-empty content가 오면 loose
    // - 단, sublist로 흡수되는 blank는 제외해야 함
    //
    // 간단한 근사: 비-List 블록이 2개 이상이면 사이에 빈 줄이 있을 수 있음 (기존 로직)
    // 비-List 1개 + List 1개인 경우: 빈 줄이 비-List와 List 사이에 있는지 확인
    if non_list_count >= 2 {
        return true;
    }

    // 비-List 블록이 있고 List 블록도 있는 경우
    // sublist 시작(list marker) 전에 빈 줄이 있으면 loose
    // sublist 시작 후의 빈 줄은 sublist 내부에 속하므로 무시
    let has_list = blocks.iter().any(|b| matches!(b, BlockNode::List(_)));
    if non_list_count >= 1 && has_list {
        // sublist 시작 line 찾기: list marker로 시작하는 첫 번째 line
        let mut saw_content = false;
        for line in lines {
            let trimmed = line.content.trim();
            let is_blank = trimmed.is_empty();
            let is_list_marker = !is_blank && !line.text_only && (
                trimmed.starts_with("- ") || trimmed.starts_with("* ") || 
                trimmed.starts_with("+ ") || trimmed == "-" || trimmed == "*" || trimmed == "+" ||
                // ordered: digit(s) + . or ) + space
                trimmed.chars().next().map_or(false, |c| c.is_ascii_digit()) && (
                    trimmed.contains(". ") || trimmed.contains(") ")
                )
            );

            if !saw_content && !is_blank {
                saw_content = true;
                continue;
            }

            if saw_content && is_blank {
                // 빈 줄 발견 — sublist 시작 전이면 loose
                // 다음 non-blank line이 list marker면 loose (빈 줄이 paragraph와 sublist 사이)
                // 다음 non-blank line이 list marker가 아니면 이것도 loose (paragraph 사이 빈 줄)
                return true;
            }

            if saw_content && is_list_marker {
                // sublist 시작 — 이후의 빈 줄은 sublist 내부
                break;
            }
        }
    }

    false
}

/// text_only가 있는 아이템 내용 파싱
fn parse_item_lines_with_text_only(lines: &[ItemLine]) -> Vec<BlockNode> {
    // 빈 줄을 기준으로 청크로 분리
    let mut chunks: Vec<(Vec<&ItemLine>, bool)> = vec![]; // (lines, has_text_only)
    let mut current_chunk: Vec<&ItemLine> = vec![];
    let mut current_has_text_only = false;

    for line in lines {
        if line.content.trim().is_empty() && !line.text_only {
            if !current_chunk.is_empty() {
                chunks.push((current_chunk, current_has_text_only));
                current_chunk = vec![];
                current_has_text_only = false;
            }
        } else {
            if line.text_only {
                current_has_text_only = true;
            }
            current_chunk.push(line);
        }
    }
    if !current_chunk.is_empty() {
        chunks.push((current_chunk, current_has_text_only));
    }

    let mut result: Vec<BlockNode> = vec![];

    for (chunk, has_text_only) in chunks {
        let content: String = chunk
            .iter()
            .map(|l| l.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        if has_text_only {
            // text_only가 있는 청크는 무조건 paragraph로 처리
            result.push(BlockNode::Paragraph(ParagraphNode::with_raw_text(
                crate::parser::inline::parse_inlines(&content),
                &content,
            )));
        } else {
            // 일반 청크는 전체 파서로 파싱
            let doc = crate::parser::parse(&content, crate::Spec::default());
            result.extend(doc.children);
        }
    }

    result
}

/// 줄이 block-level 구조의 시작인지 확인 (lazy continuation 판단용)
/// paragraph를 interrupt할 수 없는 블록 (indented code, HTML type 7 등)은 제외
fn is_block_start_for_lazy(line: &str) -> bool {
    use crate::parser::block_start;
    match block_start::detect(line, false) {
        Some(ref start) => block_start::can_interrupt_paragraph(start),
        None => false,
    }
}
