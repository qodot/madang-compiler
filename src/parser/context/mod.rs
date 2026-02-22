//! 파싱 컨텍스트 타입 정의
//!
//! 시작 정보(Start)와 파싱 상태(ParsingContext)를 분리하여 관리합니다.

mod blockquote_context;
mod code_block_fenced_context;
mod code_block_indented_context;
mod html_block_context;
mod list_context;
mod none_context;
mod paragraph_context;

pub use blockquote_context::BlockquoteContext;
pub use code_block_fenced_context::CodeBlockFencedContext;
pub use code_block_indented_context::CodeBlockIndentedContext;
pub use html_block_context::HtmlBlockContext;
pub use list_context::ListContext;
pub use none_context::NoneContext;
pub use paragraph_context::ParagraphContext;

use crate::node::BlockNode;

// 각 파서 모듈에서 타입 re-export
pub use super::code_block_indented::CodeBlockIndentedStartReason;
pub use super::list_item::{ItemLine, ListItemStart};

/// 한 줄 처리 결과: (새로 완성된 노드들, 새 컨텍스트)
pub type LineResult = (Vec<BlockNode>, ParsingContext);

// =============================================================================
// Parsing Context
// =============================================================================

/// 파싱 중인 컨텍스트 (상태 기계의 상태)
pub enum ParsingContext {
    /// 새 블록 시작 대기
    None(NoneContext),

    /// Fenced Code Block 파싱 중
    CodeBlockFenced(CodeBlockFencedContext),

    /// Paragraph 파싱 중 (여러 줄이 하나의 문단)
    Paragraph(ParagraphContext),

    /// Blockquote 파싱 중 (여러 줄 수집)
    Blockquote(BlockquoteContext),

    /// List 파싱 중
    List(ListContext),

    /// Indented Code Block 파싱 중
    CodeBlockIndented(CodeBlockIndentedContext),

    /// HTML Block 파싱 중
    HtmlBlock(HtmlBlockContext),
}
