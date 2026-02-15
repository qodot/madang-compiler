//! CommonMark 파서
//!
//! 라인 단위로 스캔하며 블록 레벨 요소를 파싱합니다.
//! fold 패턴을 사용하여 불변 상태 전이를 구현합니다.

mod blockquote;
mod code_block_fenced;
mod code_block_indented;
mod context;
mod heading;
mod heading_setext;
mod helpers;
mod html_block;
pub(crate) mod inline;
mod list;
mod list_item;
mod paragraph;
mod thematic_break;

use crate::node::{BlockNode, CodeBlockNode, DocumentNode};
use code_block_fenced::{parse as parse_code_block_fenced, CodeBlockFencedOk};
use code_block_indented::try_start as try_start_code_block_indented;
use context::{
    CodeBlockFencedStart, CodeBlockIndentedStartReason, HtmlBlockContext, LineResult, NoneContext,
    ParsingContext,
};
use helpers::trim_blank_lines;

/// 파서 상태: (완성된 노드들, 현재 컨텍스트) - fold 누적용
type ParserState = (Vec<BlockNode>, ParsingContext);

/// 문서 전체 파싱
pub fn parse(input: &str) -> DocumentNode {
    if input.is_empty() {
        return DocumentNode::new(vec![]);
    }

    // fold: 각 줄을 처리하며 상태 전이
    let (children, final_context) = input.lines().fold(
        (Vec::new(), ParsingContext::None(NoneContext)),
        |(children, context), line| process_line(line, context, children),
    );

    // 마지막 컨텍스트 마무리
    let children = finalize_context(final_context, children);

    DocumentNode::new(children)
}

/// 한 줄 처리 후 새 상태 반환
fn process_line(line: &str, context: ParsingContext, nodes: Vec<BlockNode>) -> ParserState {
    let (new_nodes, new_context) = match context {
        ParsingContext::None(ctx) => ctx.parse(line),
        ParsingContext::CodeBlockFenced { start, content } => {
            process_line_in_code_block(line, start, content)
        }
        ParsingContext::Paragraph(ctx) => ctx.parse(line),
        ParsingContext::Blockquote { pending_lines } => {
            process_line_in_blockquote(line, pending_lines)
        }
        ParsingContext::List(ctx) => ctx.parse(line),
        ParsingContext::CodeBlockIndented { pending_lines, pending_blank_count } => {
            process_line_in_code_block_indented(line, pending_lines, pending_blank_count)
        }
        ParsingContext::HtmlBlock(ctx) => ctx.parse(line),
    };

    // 새로 완성된 노드들을 누적
    let nodes = extend_nodes(nodes, new_nodes);
    (nodes, new_context)
}

/// 노드 벡터 확장 (불변 스타일)
fn extend_nodes(mut nodes: Vec<BlockNode>, new_nodes: Vec<BlockNode>) -> Vec<BlockNode> {
    nodes.extend(new_nodes);
    nodes
}

/// Code Block 상태에서 줄 처리
/// 반환: (새로 완성된 노드들, 새 컨텍스트)
fn process_line_in_code_block(
    current_line: &str,
    start: CodeBlockFencedStart,
    content: Vec<String>,
) -> LineResult {
    match parse_code_block_fenced(current_line, Some(&start)).unwrap() {
        CodeBlockFencedOk::End => {
            let node = code_block_fenced::finalize(start, content);
            (vec![node], ParsingContext::None(NoneContext))
        }
        CodeBlockFencedOk::Content(line) => {
            let content = push_string(content, line);
            let context = ParsingContext::CodeBlockFenced { start, content };
            (vec![], context)
        }
        CodeBlockFencedOk::Start(_) => unreachable!("parse with Some context should return End or Content"),
    }
}

/// Indented Code Block 상태에서 줄 처리
/// 반환: (새로 완성된 노드들, 새 컨텍스트)
fn process_line_in_code_block_indented(
    current_line: &str,
    pending_lines: Vec<String>,
    pending_blank_count: usize,
) -> LineResult {
    use context::CodeBlockIndentedNotStartReason;

    match try_start_code_block_indented(current_line) {
        // 4칸 이상 들여쓰기 → 코드 줄 추가
        Ok(CodeBlockIndentedStartReason::Started(start)) => {
            let mut pending_lines = pending_lines;
            for _ in 0..pending_blank_count {
                pending_lines = push_string(pending_lines, String::new());
            }
            let pending_lines = push_string(pending_lines, start.content);
            let context = ParsingContext::CodeBlockIndented {
                pending_lines,
                pending_blank_count: 0,
            };
            (vec![], context)
        }
        // 4칸 미만 빈 줄 → 대기 (코드 블록 종료 여부는 다음 줄에서 결정)
        Err(CodeBlockIndentedNotStartReason::Empty) => {
            let context = ParsingContext::CodeBlockIndented {
                pending_lines,
                pending_blank_count: pending_blank_count + 1,
            };
            (vec![], context)
        }
        // 4칸 미만 비빈 줄 → 코드 블록 종료
        Err(CodeBlockIndentedNotStartReason::InsufficientIndent) => {
            let content = trim_blank_lines(pending_lines);
            let code_node = BlockNode::CodeBlock(CodeBlockNode::new(None, content));
            // 현재 줄을 다시 처리
            let (more_nodes, new_context) = NoneContext.parse(current_line);
            let mut nodes = vec![code_node];
            nodes.extend(more_nodes);
            (nodes, new_context)
        }
    }
}

/// Blockquote 상태에서 줄 처리
/// 반환: (새로 완성된 노드들, 새 컨텍스트)
fn process_line_in_blockquote(current_line: &str, pending_lines: Vec<String>) -> LineResult {
    let trimmed = current_line.trim();

    // 빈 줄이면 Blockquote 종료
    if trimmed.is_empty() {
        let node = blockquote::finalize(pending_lines, parse_block_simple);
        return (vec![node], ParsingContext::None(NoneContext));
    }

    // Fenced Code Block 시작이면 Blockquote 종료
    if let Ok(CodeBlockFencedOk::Start(start)) = parse_code_block_fenced(current_line, None) {
        let node = blockquote::finalize(pending_lines, parse_block_simple);
        let context = ParsingContext::CodeBlockFenced {
            start,
            content: Vec::new(),
        };
        return (vec![node], context);
    }

    // HTML Block 시작이면 Blockquote 종료
    if let Ok(html_block::HtmlBlockOk::Start(block_type)) = html_block::parse(current_line, None) {
        if html_block::can_interrupt_paragraph(block_type) {
            let bq_node = blockquote::finalize(pending_lines, parse_block_simple);
            if let Ok(html_block::HtmlBlockOk::End) = html_block::parse(current_line, Some(block_type)) {
                let html_node = html_block::finalize(vec![current_line.to_string()]);
                return (vec![bq_node, html_node], ParsingContext::None(NoneContext));
            }
            let context = ParsingContext::HtmlBlock(HtmlBlockContext::new(
                block_type,
                vec![current_line.to_string()],
            ));
            return (vec![bq_node], context);
        }
    }

    // Thematic Break이면 Blockquote 종료
    if let Ok(node) = thematic_break::parse(current_line) {
        let bq_node = blockquote::finalize(pending_lines, parse_block_simple);
        return (vec![bq_node, node], ParsingContext::None(NoneContext));
    }

    // ATX Heading이면 Blockquote 종료
    if let Ok(node) = heading::parse(current_line) {
        let bq_node = blockquote::finalize(pending_lines, parse_block_simple);
        return (vec![bq_node, node], ParsingContext::None(NoneContext));
    }

    // > 로 시작하면 마커 제거 후 저장, 아니면 lazy continuation
    let content = match blockquote::parse(current_line) {
        Ok(stripped) => stripped,
        Err(_) => trimmed.to_string(),
    };
    let pending_lines = push_string(pending_lines, content);
    (vec![], ParsingContext::Blockquote { pending_lines })
}

/// 마지막 컨텍스트 마무리
fn finalize_context(context: ParsingContext, nodes: Vec<BlockNode>) -> Vec<BlockNode> {
    match context {
        ParsingContext::None(NoneContext) => nodes,
        ParsingContext::CodeBlockFenced { start, content } => {
            let node = code_block_fenced::finalize(start, content);
            push_node(nodes, node)
        }
        ParsingContext::Paragraph(ctx) => {
            let text = ctx.pending_lines.join("\n");
            push_node(nodes, paragraph::parse(&text))
        }
        ParsingContext::Blockquote { pending_lines } => {
            let node = blockquote::finalize(pending_lines, parse_block_simple);
            push_node(nodes, node)
        }
        ParsingContext::List(ctx) => {
            let list_node = ctx.build_list_node();
            push_node(nodes, list_node)
        }
        ParsingContext::CodeBlockIndented { pending_lines, pending_blank_count: _ } => {
            let content = trim_blank_lines(pending_lines);
            let node = BlockNode::CodeBlock(CodeBlockNode::new(None, content));
            push_node(nodes, node)
        }
        ParsingContext::HtmlBlock(ctx) => {
            let node = html_block::finalize(ctx.pending_lines);
            push_node(nodes, node)
        }
    }
}

/// 벡터에 요소 추가 후 반환 (불변 스타일)
fn push_node(mut vec: Vec<BlockNode>, node: BlockNode) -> Vec<BlockNode> {
    vec.push(node);
    vec
}

/// 문자열 벡터에 요소 추가 후 반환
fn push_string(mut vec: Vec<String>, s: String) -> Vec<String> {
    vec.push(s);
    vec
}

/// 단일 블록 파싱 (blockquote 내부 등에서 사용)
fn parse_block_simple(block: &str) -> BlockNode {
    if let Some(node) = code_block_fenced::parse_text(block) {
        return node;
    }

    // 중첩 blockquote 처리를 위해 blockquote 파싱 시도
    if let Some(node) = blockquote::parse_text(block, parse_block_simple) {
        return node;
    }

    if let Ok(node) = thematic_break::parse(block) {
        return node;
    }

    if let Ok(node) = heading::parse(block) {
        return node;
    }

    // HTML block detection for simple block parsing
    if let Ok(html_block::HtmlBlockOk::Start(_)) = html_block::parse(block, None) {
        return html_block::finalize(vec![block.to_string()]);
    }

    paragraph::parse(block.trim())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_empty_string() {
        let doc = parse("");
        assert_eq!(doc.children.len(), 0);
    }
}
