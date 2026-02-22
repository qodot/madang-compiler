//! CodeBlockIndentedContext: Indented Code Block 파싱 중 상태

use super::{CodeBlockIndentedStartReason, LineResult, NoneContext, ParsingContext};
use crate::node::{BlockNode, CodeBlockNode};
use crate::parser::code_block_indented::{self, CodeBlockIndentedNotStartReason};
use crate::parser::helpers::trim_blank_lines;

pub struct CodeBlockIndentedContext {
    /// 축적된 코드 줄 (4칸 들여쓰기 제거 후)
    pub pending_lines: Vec<String>,
    /// 대기 중인 빈 줄 개수 (다음 코드 줄이 오면 내용에 추가)
    pub pending_blank_count: usize,
}

impl CodeBlockIndentedContext {
    pub fn new(pending_lines: Vec<String>, pending_blank_count: usize) -> Self {
        Self {
            pending_lines,
            pending_blank_count,
        }
    }

    /// Indented Code Block 상태에서 줄 처리
    pub fn parse(self, line: &str) -> LineResult {
        match code_block_indented::try_start(line) {
            // 4칸 이상 들여쓰기 → 코드 줄 추가
            Ok(CodeBlockIndentedStartReason::Started(start)) => {
                let mut pending_lines = self.pending_lines;
                for _ in 0..self.pending_blank_count {
                    pending_lines.push(String::new());
                }
                pending_lines.push(start.content);
                let context = ParsingContext::CodeBlockIndented(CodeBlockIndentedContext {
                    pending_lines,
                    pending_blank_count: 0,
                });
                (vec![], context)
            }
            // 4칸 미만 빈 줄 → 대기 (코드 블록 종료 여부는 다음 줄에서 결정)
            Err(CodeBlockIndentedNotStartReason::Empty) => {
                let context = ParsingContext::CodeBlockIndented(CodeBlockIndentedContext {
                    pending_lines: self.pending_lines,
                    pending_blank_count: self.pending_blank_count + 1,
                });
                (vec![], context)
            }
            // 4칸 미만 비빈 줄 → 코드 블록 종료
            Err(CodeBlockIndentedNotStartReason::InsufficientIndent) => {
                let content = trim_blank_lines(self.pending_lines);
                let code_node = BlockNode::CodeBlock(CodeBlockNode::new(None, content));
                // 현재 줄을 다시 처리
                let (more_nodes, new_context) = NoneContext.parse(line);
                let mut nodes = vec![code_node];
                nodes.extend(more_nodes);
                (nodes, new_context)
            }
        }
    }
}
