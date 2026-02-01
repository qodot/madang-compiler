//! ListContext: List 파싱 중 상태

use super::{ItemLine, LineResult, ListItemStart, NoneContext, ParsingContext};
use crate::node::{
    BlockNode, InlineNode, ListItemNode, ListNode, ParagraphNode, TextNode,
};
use crate::parser::helpers::count_leading_char;
use crate::parser::list_item;

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

        let indent = count_leading_char(line, ' ');
        let first_content_indent = self.first_item_start.content_indent;

        // 1. 새 아이템 체크 (Example 301: current_content_indent 기준)
        if indent < self.current_content_indent {
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
            // 4칸 이상 들여쓰기 + first_content_indent 이상이면 text_only continuation
            // Example 303: 4칸 들여쓰기된 마커는 텍스트 전용
            if indent > 3 && indent >= first_content_indent {
                let strip_amount = indent.min(self.current_content_indent);
                let content = line[strip_amount..].to_string();
                return self.continue_with(ItemLine::text_only(content));
            }
        }

        // 2. Continuation line (Example 303: first_content_indent 기준)
        if indent >= first_content_indent {
            let strip_amount = indent.min(self.current_content_indent);
            let content = line[strip_amount..].to_string();
            return self.continue_with(ItemLine::text(content));
        }

        // 3. first_content_indent 미만 + 새 아이템 아님 → 종료
        self.end_and_reprocess(line)
    }

    /// 현재 아이템에 줄 추가하여 계속
    fn continue_with(self, item_line: ItemLine) -> LineResult {
        let mut lines = self.current_item_lines;
        for _ in 0..self.pending_blank_count {
            lines.push(ItemLine::blank());
        }
        lines.push(item_line);
        let tight = self.tight && self.pending_blank_count == 0;
        let context = ParsingContext::List(ListContext {
            first_item_start: self.first_item_start,
            items: self.items,
            current_item_lines: lines,
            current_content_indent: self.current_content_indent,
            tight,
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
        let list_children: Vec<ListItemNode> = all_items
            .iter()
            .map(|item_lines| {
                let parsed_blocks = parse_item_lines(item_lines);
                ListItemNode::new(parsed_blocks)
            })
            .collect();

        BlockNode::List(ListNode::new(list_type, start, self.tight, list_children))
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
    // text_only가 있는지 확인
    let has_any_text_only = lines.iter().any(|l| l.text_only);

    if has_any_text_only {
        // text_only가 있는 경우: 청크 단위로 처리
        // 빈 줄로 분리하되, 빈 줄 후 들여쓰기된 내용은 이전 청크에 포함
        parse_item_lines_with_text_only(lines)
    } else {
        // text_only가 없는 경우: 전체를 한 번에 재파싱
        // 빈 줄이 있어도 리스트 continuation으로 처리됨
        let content: String = lines
            .iter()
            .map(|l| l.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");
        let doc = crate::parser::parse(&content);
        doc.children
    }
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
            result.push(BlockNode::Paragraph(ParagraphNode::new(vec![
                InlineNode::Text(TextNode::new(&content)),
            ])));
        } else {
            // 일반 청크는 전체 파서로 파싱
            let doc = crate::parser::parse(&content);
            result.extend(doc.children);
        }
    }

    result
}
