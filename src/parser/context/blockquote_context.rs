//! BlockquoteContext: Blockquote 파싱 중 상태

use super::{CodeBlockFencedContext, HtmlBlockContext, LineResult, NoneContext, ParsingContext};
use crate::parser::block_start::{self, BlockStart};
use crate::parser::blockquote;
use crate::parser::html_block;

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

        // > 로 시작하면 blockquote 계속
        if let Ok(stripped) = blockquote::parse(line) {
            let mut pending_lines = self.pending_lines;
            pending_lines.push(stripped);
            return (vec![], ParsingContext::Blockquote(BlockquoteContext { pending_lines }));
        }

        // > 없는 줄: 블록 시작 감지
        if let Some(start) = block_start::detect(line, false) {
            // paragraph interrupt 가능한 블록이면 blockquote 종료
            if block_start::can_interrupt_paragraph(&start) {
                return self.end_and_start_block(start, line);
            }
            // fenced code는 항상 blockquote 종료 (paragraph context가 아니므로)
            if matches!(start, BlockStart::FencedCode(_)) {
                return self.end_and_start_block(start, line);
            }
        }

        // Lazy continuation: paragraph가 열려있으면 이어붙이기
        if Self::is_paragraph_open(&self.pending_lines) {
            let mut pending_lines = self.pending_lines;
            if let Some(last) = pending_lines.last_mut() {
                last.push('\n');
                last.push_str(trimmed);
            } else {
                pending_lines.push(trimmed.to_string());
            }
            return (vec![], ParsingContext::Blockquote(BlockquoteContext { pending_lines }));
        }

        // paragraph가 안 열려있으면 blockquote 종료 후 재처리
        let bq_node = blockquote::finalize(self.pending_lines, crate::parser::parse_block_simple);
        let (more_nodes, new_context) = NoneContext.parse(line);
        let mut nodes = vec![bq_node];
        nodes.extend(more_nodes);
        (nodes, new_context)
    }

    /// blockquote 종료 후 새 블록 시작
    fn end_and_start_block(self, start: BlockStart, line: &str) -> LineResult {
        let bq_node = blockquote::finalize(self.pending_lines, crate::parser::parse_block_simple);

        match start {
            BlockStart::ThematicBreak(node) | BlockStart::AtxHeading(node) => {
                (vec![bq_node, node], ParsingContext::None(NoneContext))
            }
            BlockStart::FencedCode(fenced_start) => {
                let context = ParsingContext::CodeBlockFenced(
                    CodeBlockFencedContext::new(fenced_start, Vec::new()),
                );
                (vec![bq_node], context)
            }
            BlockStart::HtmlBlock(block_type) => {
                if let Ok(html_block::HtmlBlockOk::End) = html_block::parse(line, Some(block_type)) {
                    let html_node = html_block::finalize(vec![line.to_string()]);
                    return (vec![bq_node, html_node], ParsingContext::None(NoneContext));
                }
                let context = ParsingContext::HtmlBlock(HtmlBlockContext::new(
                    block_type,
                    vec![line.to_string()],
                ));
                (vec![bq_node], context)
            }
            BlockStart::Blockquote(content) => {
                // 새 blockquote 시작 (> 없는 줄에서 blockquote가 감지될 일은 없지만 안전장치)
                let context = ParsingContext::Blockquote(BlockquoteContext::new(vec![content]));
                (vec![bq_node], context)
            }
            BlockStart::ListItem(item_start) => {
                let context = ParsingContext::List(super::ListContext {
                    current_content_indent: item_start.content_indent,
                    current_item_lines: vec![super::ItemLine::text(item_start.content.clone())],
                    first_item_start: item_start,
                    items: Vec::new(),
                    tight: true,
                    pending_blank_count: 0,
                });
                (vec![bq_node], context)
            }
            _ => {
                // IndentedCode, SetextHeading 등은 blockquote를 종료하지 않음
                let (more_nodes, new_context) = NoneContext.parse(line);
                let mut nodes = vec![bq_node];
                nodes.extend(more_nodes);
                (nodes, new_context)
            }
        }
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
        // 블록 시작이 감지되면 paragraph가 아님
        if let Some(start) = block_start::detect(last, false) {
            match start {
                BlockStart::ThematicBreak(_)
                | BlockStart::AtxHeading(_)
                | BlockStart::FencedCode(_)
                | BlockStart::IndentedCode(_) => return false,
                // HTML block, blockquote, list item은 paragraph의 내용일 수 있음
                _ => {}
            }
        }
        true
    }
}
