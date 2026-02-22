use crate::node::BlockNode;
use crate::parser::html_block::{self, HtmlBlockOk, HtmlBlockType};

use super::{LineResult, NoneContext, ParsingContext};

/// HTML Block 파싱 컨텍스트
pub struct HtmlBlockContext {
    /// HTML block 타입 (1-7)
    pub block_type: HtmlBlockType,
    /// 축적된 줄
    pub pending_lines: Vec<String>,
}

impl HtmlBlockContext {
    pub fn new(block_type: HtmlBlockType, pending_lines: Vec<String>) -> Self {
        Self {
            block_type,
            pending_lines,
        }
    }

    pub fn parse(self, line: &str) -> LineResult {
        match html_block::parse(line, Some(self.block_type)) {
            Ok(HtmlBlockOk::End) => {
                // Type 6/7: 빈 줄로 종료 — 빈 줄은 블록에 포함하지 않음
                // Type 1-5: 종료 줄은 블록에 포함
                let mut pending_lines = self.pending_lines;
                if !self.block_type.ends_on_blank_line() {
                    pending_lines.push(line.to_string());
                }
                let node = html_block::finalize(pending_lines);
                (vec![node], ParsingContext::None(NoneContext))
            }
            _ => {
                let mut pending_lines = self.pending_lines;
                pending_lines.push(line.to_string());
                (
                    vec![],
                    ParsingContext::HtmlBlock(HtmlBlockContext {
                        block_type: self.block_type,
                        pending_lines,
                    }),
                )
            }
        }
    }
}
