//! ParagraphContext: Paragraph 파싱 중 상태

use super::{
    HeadingSetextStartReason, ItemLine, LineResult, ListContext, ParsingContext,
};
use crate::node::HeadingNode;
use crate::parser::code_block_fenced::{parse as parse_code_block_fenced, CodeBlockFencedOk};
use crate::parser::html_block;
use crate::parser::{blockquote, heading, inline, list_item, paragraph, thematic_break};
use crate::parser::heading_setext::try_start as try_start_heading_setext;
use crate::parser::helpers::calculate_indent;
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

        // Fenced Code Block 시작이면 Paragraph 종료 후 Code Block 시작
        if let Ok(CodeBlockFencedOk::Start(start)) = parse_code_block_fenced(line, None) {
            let text = self.pending_lines.join("\n");
            let context = ParsingContext::CodeBlockFenced(
                super::CodeBlockFencedContext::new(start, Vec::new()),
            );
            return (vec![paragraph::parse(&text)], context);
        }

        let indent = calculate_indent(line);

        // Setext Heading 밑줄이면 Paragraph를 Heading으로 변환
        // 중요: Thematic Break보다 먼저 확인해야 함 (---가 Setext 밑줄로 해석됨)
        if let Ok(HeadingSetextStartReason::Started(start)) = try_start_heading_setext(trimmed, indent) {
            let text = self.pending_lines.join("\n");
            let text_trimmed = text.trim_end();
            let node = crate::node::BlockNode::Heading(HeadingNode::with_raw_text(
                start.level.to_level(),
                inline::parse_inlines(text_trimmed),
                &text,
            ));
            return (vec![node], ParsingContext::None(NoneContext));
        }

        // Thematic Break이면 Paragraph 종료
        if let Ok(node) = thematic_break::parse(line) {
            let text = self.pending_lines.join("\n");
            return (vec![paragraph::parse(&text), node], ParsingContext::None(NoneContext));
        }

        // ATX Heading이면 Paragraph 종료
        if let Ok(node) = heading::parse(line) {
            let text = self.pending_lines.join("\n");
            return (vec![paragraph::parse(&text), node], ParsingContext::None(NoneContext));
        }

        // HTML Block 시작이면 (type 1-6만) Paragraph 종료 후 HTML Block 시작
        if let Ok(html_block::HtmlBlockOk::Start(block_type)) = html_block::parse(line, None) {
            if html_block::can_interrupt_paragraph(block_type) {
                let text = self.pending_lines.join("\n");
                // 같은 줄에서 종료 조건도 충족하는지 확인
                if let Ok(html_block::HtmlBlockOk::End) = html_block::parse(line, Some(block_type)) {
                    let node = html_block::finalize(vec![line.to_string()]);
                    return (vec![paragraph::parse(&text), node], ParsingContext::None(NoneContext));
                }
                let context = ParsingContext::HtmlBlock(super::HtmlBlockContext::new(
                    block_type,
                    vec![line.to_string()],
                ));
                return (vec![paragraph::parse(&text)], context);
            }
        }

        // Blockquote 시작이면 Paragraph 종료 후 Blockquote 시작
        if let Ok(content) = blockquote::parse(line) {
            let text = self.pending_lines.join("\n");
            let context = ParsingContext::Blockquote(
                super::BlockquoteContext::new(vec![content]),
            );
            return (vec![paragraph::parse(&text)], context);
        }

        // List 시작이면 Paragraph 종료 후 List 시작
        // CommonMark 명세:
        // - 빈 아이템은 Paragraph 인터럽트 불가
        // - Ordered list는 1로 시작할 때만 Paragraph 인터럽트 가능 (Example 304-305)
        if let Ok(list_item::ListItemOk::Started(start)) = list_item::parse(line) {
            // 빈 아이템은 Paragraph 인터럽트 불가
            if start.content.is_empty() {
                // 줄 추가하고 계속
                let mut pending_lines = self.pending_lines;
                pending_lines.push(line.trim_start().to_string());
                return (vec![], ParsingContext::Paragraph(ParagraphContext::new(pending_lines)));
            }
            
            // Ordered list는 1로 시작할 때만 Paragraph 인터럽트 가능
            if let list_item::ListMarker::Ordered { start: num, .. } = &start.marker {
                if *num != 1 {
                    // 1이 아닌 숫자로 시작하면 인터럽트 불가
                    let mut pending_lines = self.pending_lines;
                    pending_lines.push(line.trim_start().to_string());
                    return (vec![], ParsingContext::Paragraph(ParagraphContext::new(pending_lines)));
                }
            }
            
            // Paragraph 인터럽트 가능 → List 시작
            let text = self.pending_lines.join("\n");
            let context = ParsingContext::List(ListContext {
                current_content_indent: start.content_indent,
                current_item_lines: vec![ItemLine::text(start.content.clone())],
                first_item_start: start,
                items: Vec::new(),
                tight: true,
                pending_blank_count: 0,
            });
            return (vec![paragraph::parse(&text)], context);
        }

        // 줄 추가
        let mut pending_lines = self.pending_lines;
        pending_lines.push(line.trim_start().to_string());
        (vec![], ParsingContext::Paragraph(ParagraphContext::new(pending_lines)))
    }
}
