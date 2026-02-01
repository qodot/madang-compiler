//! 파싱 컨텍스트 타입 정의
//!
//! 시작 정보(Start)와 파싱 상태(ParsingContext)를 분리하여 관리합니다.

mod list_context;
mod none_context;
mod paragraph_context;

pub use list_context::ListContext;
pub use none_context::NoneContext;
pub use paragraph_context::ParagraphContext;

use crate::node::BlockNode;

// 각 파서 모듈에서 타입 re-export
pub use super::code_block_fenced::CodeBlockFencedStart;
pub use super::code_block_indented::{
    CodeBlockIndentedNotStartReason, CodeBlockIndentedStartReason,
};
pub use super::heading_setext::HeadingSetextStartReason;
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
    CodeBlockFenced {
        /// 시작 정보 (불변)
        start: CodeBlockFencedStart,
        /// 축적된 코드 줄 (가변)
        content: Vec<String>,
    },

    /// Paragraph 파싱 중 (여러 줄이 하나의 문단)
    Paragraph(ParagraphContext),

    /// Blockquote 파싱 중 (여러 줄 수집)
    Blockquote { pending_lines: Vec<String> },

    /// List 파싱 중
    List(ListContext),

    /// Indented Code Block 파싱 중
    CodeBlockIndented {
        /// 축적된 코드 줄 (4칸 들여쓰기 제거 후)
        pending_lines: Vec<String>,
        /// 대기 중인 빈 줄 개수 (다음 코드 줄이 오면 내용에 추가)
        pending_blank_count: usize,
    },
}
