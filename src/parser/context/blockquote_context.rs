//! BlockquoteContext: Blockquote 파싱 중 상태

use super::{CodeBlockFencedContext, HtmlBlockContext, LineResult, NoneContext, ParsingContext};
use crate::node::BlockNode;
use crate::parser::blockquote;
use crate::parser::code_block_fenced::{self, parse as parse_code_block_fenced, CodeBlockFencedOk};
use crate::parser::code_block_indented;
use crate::parser::heading;
use crate::parser::html_block;
use crate::parser::list_item;
use crate::parser::thematic_break;

pub struct BlockquoteContext {
    pub pending_lines: Vec<String>,
}

impl BlockquoteContext {
    pub fn new(pending_lines: Vec<String>) -> Self {
        Self { pending_lines }
    }

    /// Blockquote 상태에서 줄 처리
    pub fn parse(self, line: &str) -> LineResult {
        let trimmed = line.trim();

        // 빈 줄이면 Blockquote 종료
        if trimmed.is_empty() {
            let node = blockquote::finalize(self.pending_lines, crate::parser::parse_block_simple);
            return (vec![node], ParsingContext::None(NoneContext));
        }

        // Fenced Code Block 시작이면 Blockquote 종료
        if let Ok(CodeBlockFencedOk::Start(start)) = parse_code_block_fenced(line, None) {
            let node = blockquote::finalize(self.pending_lines, crate::parser::parse_block_simple);
            let context = ParsingContext::CodeBlockFenced(
                CodeBlockFencedContext::new(start, Vec::new()),
            );
            return (vec![node], context);
        }

        // HTML Block 시작이면 Blockquote 종료
        if let Ok(html_block::HtmlBlockOk::Start(block_type)) = html_block::parse(line, None) {
            if html_block::can_interrupt_paragraph(block_type) {
                let bq_node = blockquote::finalize(self.pending_lines, crate::parser::parse_block_simple);
                if let Ok(html_block::HtmlBlockOk::End) = html_block::parse(line, Some(block_type)) {
                    let html_node = html_block::finalize(vec![line.to_string()]);
                    return (vec![bq_node, html_node], ParsingContext::None(NoneContext));
                }
                let context = ParsingContext::HtmlBlock(HtmlBlockContext::new(
                    block_type,
                    vec![line.to_string()],
                ));
                return (vec![bq_node], context);
            }
        }

        // Thematic Break이면 Blockquote 종료
        if let Ok(node) = thematic_break::parse(line) {
            let bq_node = blockquote::finalize(self.pending_lines, crate::parser::parse_block_simple);
            return (vec![bq_node, node], ParsingContext::None(NoneContext));
        }

        // ATX Heading이면 Blockquote 종료
        if let Ok(node) = heading::parse(line) {
            let bq_node = blockquote::finalize(self.pending_lines, crate::parser::parse_block_simple);
            return (vec![bq_node, node], ParsingContext::None(NoneContext));
        }

        // > 로 시작하면 마커 제거 후 저장
        if let Ok(stripped) = blockquote::parse(line) {
            let mut pending_lines = self.pending_lines;
            pending_lines.push(stripped);
            return (vec![], ParsingContext::Blockquote(BlockquoteContext { pending_lines }));
        }

        // > 없는 줄: paragraph가 열려있고, 현재 줄이 block 시작이 아니면 lazy continuation
        if !Self::is_paragraph_open(&self.pending_lines) || is_block_structure(line) {
            let bq_node = blockquote::finalize(self.pending_lines, crate::parser::parse_block_simple);
            let (more_nodes, new_context) = NoneContext.parse(line);
            let mut nodes = vec![bq_node];
            nodes.extend(more_nodes);
            return (nodes, new_context);
        }

        // Lazy continuation: paragraph 열림, > 없는 줄을 마지막 줄에 이어붙이기
        let mut pending_lines = self.pending_lines;
        if let Some(last) = pending_lines.last_mut() {
            last.push('\n');
            last.push_str(trimmed);
        } else {
            pending_lines.push(trimmed.to_string());
        }
        (vec![], ParsingContext::Blockquote(BlockquoteContext { pending_lines }))
    }

    /// pending_lines에서 마지막이 paragraph인지 판단
    fn is_paragraph_open(pending_lines: &[String]) -> bool {
        let last = match pending_lines.last() {
            Some(line) => line.as_str(),
            None => return false,
        };
        if last.trim().is_empty() {
            return false;
        }
        if thematic_break::parse(last).is_ok() {
            return false;
        }
        if heading::parse(last).is_ok() {
            return false;
        }
        if let Ok(CodeBlockFencedOk::Start(_)) = parse_code_block_fenced(last, None) {
            return false;
        }
        if code_block_indented::try_start(last).is_ok() {
            return false;
        }
        true
    }
}

/// 줄이 paragraph를 interrupt 할 수 있는 block 구조인지 판단
fn is_block_structure(line: &str) -> bool {
    if thematic_break::parse(line).is_ok() {
        return true;
    }
    if heading::parse(line).is_ok() {
        return true;
    }
    if let Ok(CodeBlockFencedOk::Start(_)) = parse_code_block_fenced(line, None) {
        return true;
    }
    if let Ok(list_item::ListItemOk::Started(_)) = list_item::parse(line) {
        return true;
    }
    if blockquote::parse(line).is_ok() {
        return true;
    }
    if let Ok(html_block::HtmlBlockOk::Start(bt)) = html_block::parse(line, None) {
        if html_block::can_interrupt_paragraph(bt) {
            return true;
        }
    }
    false
}
