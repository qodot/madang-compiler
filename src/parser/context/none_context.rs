//! NoneContext: 새 블록 시작 대기 상태

use super::{
    CodeBlockIndentedStartReason, ItemLine, LineResult, ListContext, ParagraphContext,
    ParsingContext,
};
use crate::parser::code_block_fenced::{parse as parse_code_block_fenced, CodeBlockFencedOk};
use crate::parser::code_block_indented::try_start as try_start_code_block_indented;
use crate::parser::html_block;
use crate::parser::{blockquote, heading, list_item, thematic_break};

#[derive(Debug, Clone, Default)]
pub struct NoneContext;

impl NoneContext {
    pub fn parse(self, line: &str) -> LineResult {
        let trimmed = line.trim();

        // 빈 줄은 무시
        if trimmed.is_empty() {
            return (vec![], ParsingContext::None(NoneContext));
        }

        // 한 줄 블록들 (Thematic Break, ATX Heading)
        if let Ok(node) = thematic_break::parse(line) {
            return (vec![node], ParsingContext::None(NoneContext));
        }

        if let Ok(node) = heading::parse(line) {
            return (vec![node], ParsingContext::None(NoneContext));
        }

        // Fenced Code Block 시작 감지
        if let Ok(CodeBlockFencedOk::Start(start)) = parse_code_block_fenced(line, None) {
            let context = ParsingContext::CodeBlockFenced {
                start,
                content: Vec::new(),
            };
            return (vec![], context);
        }

        // HTML Block 시작 감지
        if let Ok(html_block::HtmlBlockOk::Start(block_type)) = html_block::parse(line, None) {
            // 같은 줄에서 종료 조건도 충족하는지 확인
            if let Ok(html_block::HtmlBlockOk::End) = html_block::parse(line, Some(block_type)) {
                let node = html_block::finalize(vec![line.to_string()]);
                return (vec![node], ParsingContext::None(NoneContext));
            }
            let context = ParsingContext::HtmlBlock(super::HtmlBlockContext::new(
                block_type,
                vec![line.to_string()],
            ));
            return (vec![], context);
        }

        // Blockquote 시작 감지
        if let Ok(content) = blockquote::parse(line) {
            let context = ParsingContext::Blockquote {
                pending_lines: vec![content],
            };
            return (vec![], context);
        }

        // List 시작 감지
        if let Ok(list_item::ListItemOk::Started(start)) = list_item::parse(line) {
            let context = ParsingContext::List(ListContext {
                current_content_indent: start.content_indent,
                current_item_lines: vec![ItemLine::text(start.content.clone())],
                first_item_start: start,
                items: Vec::new(),
                tight: true,
                pending_blank_count: 0,
            });
            return (vec![], context);
        }

        // Indented Code Block 시작 감지 (List 후에 체크 - 명세상 List가 우선)
        if let Ok(CodeBlockIndentedStartReason::Started(start)) =
            try_start_code_block_indented(line)
        {
            let context = ParsingContext::CodeBlockIndented {
                pending_lines: vec![start.content],
                pending_blank_count: 0,
            };
            return (vec![], context);
        }

        // 나머지는 Paragraph 시작
        let context = ParsingContext::Paragraph(ParagraphContext::new(
            vec![line.trim().to_string()],
        ));
        (vec![], context)
    }
}
