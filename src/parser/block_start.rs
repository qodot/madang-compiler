//! 블록 시작 감지 — 단일 진실 공급원 (Single Source of Truth)
//!
//! CommonMark 명세에서 줄이 어떤 블록 구조의 시작인지 판단하는 로직을
//! 한 곳에 모아둔다. 각 컨텍스트는 이 함수를 호출한 뒤 필터링만 수행.

use crate::node::BlockNode;
use super::blockquote;
use super::code_block_fenced::{self, CodeBlockFencedOk, CodeBlockFencedStart};
use super::code_block_indented::{self, CodeBlockIndentedStart, CodeBlockIndentedStartReason};
use super::heading;
use super::heading_setext::{self, HeadingSetextStart, HeadingSetextStartReason};
use super::helpers::calculate_indent;
use super::html_block::{self, HtmlBlockType};
use super::list_item::{self, ListItemStart};
use super::thematic_break;

/// 블록 시작의 종류
#[derive(Debug)]
pub enum BlockStart {
    /// Thematic break (`---`, `***`, `___`)
    ThematicBreak(BlockNode),
    /// ATX heading (`# ...`)
    AtxHeading(BlockNode),
    /// Fenced code block (`` ``` `` or `~~~`)
    FencedCode(CodeBlockFencedStart),
    /// HTML block (type 1-7)
    HtmlBlock(HtmlBlockType),
    /// Blockquote (`> ...`), 내부 content 포함
    Blockquote(String),
    /// List item (`- ...`, `1. ...`)
    ListItem(ListItemStart),
    /// Indented code block (4+ spaces)
    IndentedCode(CodeBlockIndentedStart),
    /// Setext heading underline (`===` or `---`)
    SetextHeading(HeadingSetextStart),
}

/// 줄에서 블록 시작을 감지한다 (CommonMark 우선순위 순).
///
/// setext heading은 paragraph 컨텍스트에서만 의미가 있으므로
/// `check_setext`가 true일 때만 체크한다.
pub fn detect(line: &str, check_setext: bool) -> Option<BlockStart> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    // 1. Setext heading (paragraph 컨텍스트 전용)
    //    Thematic break보다 먼저 체크해야 함 (---가 setext 밑줄로 해석됨)
    if check_setext {
        let indent = calculate_indent(line);
        if let Ok(HeadingSetextStartReason::Started(start)) =
            heading_setext::try_start(trimmed, indent)
        {
            return Some(BlockStart::SetextHeading(start));
        }
    }

    // 2. Thematic break
    if let Ok(node) = thematic_break::parse(line) {
        return Some(BlockStart::ThematicBreak(node));
    }

    // 3. ATX heading
    if let Ok(node) = heading::parse(line) {
        return Some(BlockStart::AtxHeading(node));
    }

    // 4. Fenced code block
    if let Ok(CodeBlockFencedOk::Start(start)) = code_block_fenced::parse(line, None) {
        return Some(BlockStart::FencedCode(start));
    }

    // 5. HTML block
    if let Ok(html_block::HtmlBlockOk::Start(block_type)) = html_block::parse(line, None) {
        return Some(BlockStart::HtmlBlock(block_type));
    }

    // 6. Blockquote
    if let Ok(content) = blockquote::parse(line) {
        return Some(BlockStart::Blockquote(content));
    }

    // 7. List item
    if let Ok(list_item::ListItemOk::Started(start)) = list_item::parse(line) {
        return Some(BlockStart::ListItem(start));
    }

    // 8. Indented code block (list보다 후순위 — 명세상 list가 우선)
    if let Ok(CodeBlockIndentedStartReason::Started(start)) =
        code_block_indented::try_start(line)
    {
        return Some(BlockStart::IndentedCode(start));
    }

    None
}

/// paragraph를 interrupt할 수 있는 블록 시작인지 판단.
///
/// CommonMark 명세 기준:
/// - Setext heading: interrupt가 아니라 paragraph를 heading으로 변환
/// - HTML block type 7: paragraph interrupt 불가
/// - Indented code: paragraph interrupt 불가
/// - 빈 list item: paragraph interrupt 불가
/// - Ordered list start != 1: paragraph interrupt 불가
pub fn can_interrupt_paragraph(start: &BlockStart) -> bool {
    match start {
        BlockStart::ThematicBreak(_) => true,
        BlockStart::AtxHeading(_) => true,
        BlockStart::FencedCode(_) => true,
        BlockStart::HtmlBlock(block_type) => html_block::can_interrupt_paragraph(*block_type),
        BlockStart::Blockquote(_) => true,
        BlockStart::ListItem(item) => {
            // 빈 아이템은 paragraph interrupt 불가
            if item.content.is_empty() {
                return false;
            }
            // Ordered list는 1로 시작할 때만 paragraph interrupt 가능
            if let list_item::ListMarker::Ordered { start: num, .. } = &item.marker {
                return *num == 1;
            }
            true
        }
        BlockStart::IndentedCode(_) => false,
        BlockStart::SetextHeading(_) => false, // 특수 처리 (paragraph → heading 변환)
    }
}
