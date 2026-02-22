//! NoneContext: 새 블록 시작 대기 상태

use super::{
    ItemLine, LineResult, ListContext, ParagraphContext,
    ParsingContext,
};
use crate::parser::block_start::{self, BlockStart};
use crate::parser::html_block;

#[derive(Debug, Clone, Default)]
pub struct NoneContext;

impl NoneContext {
    pub fn parse(self, line: &str) -> LineResult {
        let trimmed = line.trim();

        // 빈 줄은 무시
        if trimmed.is_empty() {
            return (vec![], ParsingContext::None(NoneContext));
        }

        // 블록 시작 감지 (setext heading은 NoneContext에서 불필요)
        if let Some(start) = block_start::detect(line, false) {
            return self.handle_block_start(start, line);
        }

        // 나머지는 Paragraph 시작
        let context = ParsingContext::Paragraph(ParagraphContext::new(
            vec![line.trim_start().to_string()],
        ));
        (vec![], context)
    }

    fn handle_block_start(self, start: BlockStart, line: &str) -> LineResult {
        match start {
            BlockStart::ThematicBreak(node) => {
                (vec![node], ParsingContext::None(NoneContext))
            }
            BlockStart::AtxHeading(node) => {
                (vec![node], ParsingContext::None(NoneContext))
            }
            BlockStart::FencedCode(fenced_start) => {
                let context = ParsingContext::CodeBlockFenced(
                    super::CodeBlockFencedContext::new(fenced_start, Vec::new()),
                );
                (vec![], context)
            }
            BlockStart::HtmlBlock(block_type) => {
                // 같은 줄에서 종료 조건도 충족하는지 확인
                if let Ok(html_block::HtmlBlockOk::End) = html_block::parse(line, Some(block_type)) {
                    let node = html_block::finalize(vec![line.to_string()]);
                    return (vec![node], ParsingContext::None(NoneContext));
                }
                let context = ParsingContext::HtmlBlock(super::HtmlBlockContext::new(
                    block_type,
                    vec![line.to_string()],
                ));
                (vec![], context)
            }
            BlockStart::Blockquote(content) => {
                let context = ParsingContext::Blockquote(
                    super::BlockquoteContext::new(vec![content]),
                );
                (vec![], context)
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
                (vec![], context)
            }
            BlockStart::IndentedCode(code_start) => {
                let context = ParsingContext::CodeBlockIndented(
                    super::CodeBlockIndentedContext::new(vec![code_start.content], 0),
                );
                (vec![], context)
            }
            BlockStart::SetextHeading(_) => {
                // NoneContext에서는 setext heading 불가 (paragraph 없음)
                // detect에서 check_setext=false로 호출하므로 여기에 올 수 없음
                unreachable!("SetextHeading should not be detected in NoneContext")
            }
        }
    }
}
