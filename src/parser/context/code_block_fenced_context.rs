//! CodeBlockFencedContext: Fenced Code Block 파싱 중 상태

use super::{LineResult, NoneContext, ParsingContext};
use crate::parser::code_block_fenced::{self, parse as parse_code_block_fenced, CodeBlockFencedOk, CodeBlockFencedStart};

pub struct CodeBlockFencedContext {
    /// 시작 정보 (불변)
    pub start: CodeBlockFencedStart,
    /// 축적된 코드 줄
    pub content: Vec<String>,
}

impl CodeBlockFencedContext {
    pub fn new(start: CodeBlockFencedStart, content: Vec<String>) -> Self {
        Self { start, content }
    }

    /// Fenced Code Block 상태에서 줄 처리
    pub fn parse(self, line: &str) -> LineResult {
        match parse_code_block_fenced(line, Some(&self.start)).unwrap() {
            CodeBlockFencedOk::End => {
                let node = code_block_fenced::finalize(self.start, self.content);
                (vec![node], ParsingContext::None(NoneContext))
            }
            CodeBlockFencedOk::Content(content_line) => {
                let mut content = self.content;
                content.push(content_line);
                let context = ParsingContext::CodeBlockFenced(CodeBlockFencedContext {
                    start: self.start,
                    content,
                });
                (vec![], context)
            }
            CodeBlockFencedOk::Start(_) => {
                unreachable!("parse with Some context should return End or Content")
            }
        }
    }
}
