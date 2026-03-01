//! ParagraphContext: Paragraph 파싱 중 상태

use super::{
    ItemLine, LineResult, ListContext, ParsingContext,
};
use crate::node::HeadingNode;
use crate::parser::block_start::{self, BlockStart};
use crate::parser::html_block;
use crate::parser::{inline, paragraph};
use super::NoneContext;

#[derive(Debug, Clone)]
pub struct ParagraphContext {
    pub pending_lines: Vec<String>,
}

impl ParagraphContext {
    pub fn new(pending_lines: Vec<String>) -> Self {
        Self { pending_lines }
    }

    pub fn parse(self, line: &str) -> LineResult {
        let trimmed = line.trim();

        // 빈 줄이면 Paragraph 종료
        if trimmed.is_empty() {
            let text = self.pending_lines.join("\n");
            return (vec![paragraph::parse(&text)], ParsingContext::None(NoneContext));
        }

        // 블록 시작 감지 (setext heading 포함)
        if let Some(start) = block_start::detect(line, true) {
            return self.handle_block_start(start, line);
        }

        // 줄 추가
        let mut pending_lines = self.pending_lines;
        pending_lines.push(line.trim_start().to_string());
        (vec![], ParsingContext::Paragraph(ParagraphContext::new(pending_lines)))
    }

    fn handle_block_start(self, start: BlockStart, line: &str) -> LineResult {
        // Setext heading: paragraph를 heading으로 변환 (interrupt가 아님)
        if let BlockStart::SetextHeading(setext) = start {
            let text = self.pending_lines.join("\n");
            let text_trimmed = text.trim_end();
            let node = crate::node::BlockNode::Heading(HeadingNode::setext(
                setext.level.to_level(),
                inline::parse_inlines(text_trimmed),
                &text,
                line.trim(),
            ));
            return (vec![node], ParsingContext::None(NoneContext));
        }

        // paragraph interrupt 가능한지 체크
        if !block_start::can_interrupt_paragraph(&start) {
            // interrupt 불가 → paragraph continuation
            let mut pending_lines = self.pending_lines;
            pending_lines.push(line.trim_start().to_string());
            return (vec![], ParsingContext::Paragraph(ParagraphContext::new(pending_lines)));
        }

        // paragraph 종료 후 새 블록 시작
        let text = self.pending_lines.join("\n");
        let para_node = paragraph::parse(&text);

        match start {
            BlockStart::ThematicBreak(node) => {
                (vec![para_node, node], ParsingContext::None(NoneContext))
            }
            BlockStart::AtxHeading(node) => {
                (vec![para_node, node], ParsingContext::None(NoneContext))
            }
            BlockStart::FencedCode(fenced_start) => {
                let context = ParsingContext::CodeBlockFenced(
                    super::CodeBlockFencedContext::new(fenced_start, Vec::new()),
                );
                (vec![para_node], context)
            }
            BlockStart::HtmlBlock(block_type) => {
                // 같은 줄에서 종료 조건도 충족하는지 확인
                if let Ok(html_block::HtmlBlockOk::End) = html_block::parse(line, Some(block_type)) {
                    let node = html_block::finalize(vec![line.to_string()]);
                    return (vec![para_node, node], ParsingContext::None(NoneContext));
                }
                let context = ParsingContext::HtmlBlock(super::HtmlBlockContext::new(
                    block_type,
                    vec![line.to_string()],
                ));
                (vec![para_node], context)
            }
            BlockStart::Blockquote(content) => {
                let context = ParsingContext::Blockquote(
                    super::BlockquoteContext::new(vec![content]),
                );
                (vec![para_node], context)
            }
            BlockStart::ListItem(item_start) => {
                let context = ParsingContext::List(ListContext {
                    current_content_indent: item_start.content_indent,
                    current_item_lines: vec![ItemLine::text(item_start.content.clone())],
                    first_item_start: item_start,
                    items: Vec::new(),
                    tight: true,
                    pending_blank_count: 0,
                });
                (vec![para_node], context)
            }
            BlockStart::IndentedCode(_) | BlockStart::SetextHeading(_) => {
                // can_interrupt_paragraph에서 false 반환하므로 여기에 올 수 없음
                unreachable!()
            }
        }
    }
}
