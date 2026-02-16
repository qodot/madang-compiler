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
pub(crate) mod link_ref_def;
pub(crate) mod inline;
mod list;
mod list_item;
mod paragraph;
mod thematic_break;

use crate::node::{BlockNode, CodeBlockNode, DocumentNode, ParagraphNode};
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

    // Pass 2: link reference definitions 추출 및 인라인 재파싱
    let mut ref_map = link_ref_def::RefMap::new();
    let children = extract_link_ref_defs(children, &mut ref_map);

    // Pass 3: reference link 해석 (ref_map이 비어있지 않으면)
    let children = if !ref_map.is_empty() {
        resolve_references(children, &ref_map)
    } else {
        children
    };

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

/// Pass 2: 모든 paragraph에서 link reference definitions를 추출한다.
/// - link ref def가 paragraph 시작에 있으면 추출하고 남은 텍스트로 재파싱
/// - paragraph가 완전히 link ref def로만 구성되면 제거
fn extract_link_ref_defs(
    children: Vec<BlockNode>,
    ref_map: &mut link_ref_def::RefMap,
) -> Vec<BlockNode> {
    children
        .into_iter()
        .filter_map(|node| match node {
            BlockNode::Paragraph(para) => {
                if let Some(raw) = &para.raw_text {
                    let remaining = link_ref_def::extract_definitions(raw, ref_map);
                    if remaining.trim().is_empty() {
                        None // paragraph 전체가 link ref def
                    } else if remaining != *raw {
                        // 일부가 link ref def였으므로 남은 텍스트로 재파싱
                        Some(BlockNode::Paragraph(ParagraphNode::with_raw_text(
                            inline::parse_inlines(&remaining),
                            &remaining,
                        )))
                    } else {
                        Some(BlockNode::Paragraph(para))
                    }
                } else {
                    Some(BlockNode::Paragraph(para))
                }
            }
            BlockNode::Blockquote(mut bq) => {
                bq.children = extract_link_ref_defs(bq.children, ref_map);
                Some(BlockNode::Blockquote(bq))
            }
            BlockNode::List(mut list) => {
                for item in list.children.iter_mut() {
                    item.children = extract_link_ref_defs(
                        std::mem::take(&mut item.children),
                        ref_map,
                    );
                }
                Some(BlockNode::List(list))
            }
            other => Some(other),
        })
        .collect()
}

/// Pass 3: reference link/image를 해석한다.
/// raw_text가 있는 paragraph/heading을 ref_map과 함께 인라인 재파싱
fn resolve_references(children: Vec<BlockNode>, ref_map: &link_ref_def::RefMap) -> Vec<BlockNode> {
    children
        .into_iter()
        .map(|node| match node {
            BlockNode::Paragraph(mut para) => {
                if let Some(raw) = &para.raw_text {
                    para.children = inline::parse_inlines_with_refs(raw, Some(ref_map));
                }
                BlockNode::Paragraph(para)
            }
            BlockNode::Heading(mut h) => {
                // heading은 raw_text가 없으므로 children에서 재귀 처리
                // TODO: heading에도 raw_text 보존 필요
                BlockNode::Heading(h)
            }
            BlockNode::Blockquote(mut bq) => {
                bq.children = resolve_references(bq.children, ref_map);
                BlockNode::Blockquote(bq)
            }
            BlockNode::List(mut list) => {
                for item in list.children.iter_mut() {
                    item.children = resolve_references(
                        std::mem::take(&mut item.children),
                        ref_map,
                    );
                }
                BlockNode::List(list)
            }
            other => other,
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::node::InlineNode;

    #[test]
    fn parse_empty_string() {
        let doc = parse("");
        assert_eq!(doc.children.len(), 0);
    }

    // Example 192: basic link reference definition
    #[test]
    fn example_192() {
        let doc = parse("[foo]: /url \"title\"\n\n[foo]");
        println!("{:#?}", doc);
        assert_eq!(doc.children.len(), 1); // link ref def removed, only paragraph with link
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "/url", Some("title"))
            ]);
        } else {
            panic!("Expected paragraph, got {:?}", doc.children[0]);
        }
    }

    // Example 57: list → thematic break → list
    #[test]
    fn example_57() {
        let doc = parse("- foo\n***\n- bar\n");
        assert_eq!(doc.children.len(), 3);
        assert!(matches!(&doc.children[0], BlockNode::List(_)));
        assert!(matches!(&doc.children[1], BlockNode::ThematicBreak(_)));
        assert!(matches!(&doc.children[2], BlockNode::List(_)));
    }

    // Example 58: paragraph → thematic break → paragraph
    #[test]
    fn example_58() {
        let doc = parse("Foo\n***\nbar\n");
        assert_eq!(doc.children.len(), 3);
        assert!(matches!(&doc.children[0], BlockNode::Paragraph(_)));
        assert!(matches!(&doc.children[1], BlockNode::ThematicBreak(_)));
        assert!(matches!(&doc.children[2], BlockNode::Paragraph(_)));
    }

    // Example 59: setext heading (Foo + ---) → paragraph
    #[test]
    fn example_59() {
        let doc = parse("Foo\n---\nbar\n");
        assert_eq!(doc.children.len(), 2);
        assert!(matches!(&doc.children[0], BlockNode::Heading(_)));
        assert!(matches!(&doc.children[1], BlockNode::Paragraph(_)));
    }

    // Example 60: * list → thematic break (* * *) → * list
    #[test]
    fn example_60() {
        let doc = parse("* Foo\n* * *\n* Bar\n");
        assert_eq!(doc.children.len(), 3);
        assert!(matches!(&doc.children[0], BlockNode::List(_)));
        assert!(matches!(&doc.children[1], BlockNode::ThematicBreak(_)));
        assert!(matches!(&doc.children[2], BlockNode::List(_)));
    }

    // Example 65: escaped # → not a heading
    #[test]
    fn example_65() {
        let doc = parse("\\## foo\n");
        println!("{:#?}", doc);
        assert_eq!(doc.children.len(), 1);
        assert!(matches!(&doc.children[0], BlockNode::Paragraph(_)));
    }

    // Example 66: heading with emphasis and escaped *
    #[test]
    fn example_66() {
        let doc = parse("# foo *bar* \\*baz\\*\n");
        println!("{:#?}", doc);
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Heading(h) = &doc.children[0] {
            assert_eq!(h.level, 1);
            // children: text("foo "), emphasis([text("bar")]), text(" *baz*")
            assert_eq!(h.children.len(), 3);
        } else {
            panic!("Expected heading");
        }
    }

    // Example 99: list followed by --- → list + thematic break (not setext)
    #[test]
    fn example_99() {
        let doc = parse("- foo\n-----\n");
        assert_eq!(doc.children.len(), 2);
        assert!(matches!(&doc.children[0], BlockNode::List(_)));
        assert!(matches!(&doc.children[1], BlockNode::ThematicBreak(_)));
    }

    // Example 103: blank line prevents setext
    #[test]
    fn example_103() {
        let doc = parse("Foo\n\nbar\n---\nbaz\n");
        assert_eq!(doc.children.len(), 3);
        assert!(matches!(&doc.children[0], BlockNode::Paragraph(_)));
        assert!(matches!(&doc.children[1], BlockNode::Heading(_)));
        assert!(matches!(&doc.children[2], BlockNode::Paragraph(_)));
    }

    // Example 104: multiline paragraph + thematic break
    #[test]
    fn example_104() {
        let doc = parse("Foo\nbar\n\n---\n\nbaz\n");
        assert_eq!(doc.children.len(), 3);
        assert!(matches!(&doc.children[0], BlockNode::Paragraph(_)));
        assert!(matches!(&doc.children[1], BlockNode::ThematicBreak(_)));
        assert!(matches!(&doc.children[2], BlockNode::Paragraph(_)));
    }

    // Example 105: paragraph + thematic break (* * *) + paragraph
    #[test]
    fn example_105() {
        let doc = parse("Foo\nbar\n* * *\nbaz\n");
        assert_eq!(doc.children.len(), 3);
        assert!(matches!(&doc.children[0], BlockNode::Paragraph(_)));
        assert!(matches!(&doc.children[1], BlockNode::ThematicBreak(_)));
        assert!(matches!(&doc.children[2], BlockNode::Paragraph(_)));
    }

    // Example 106: escaped --- in paragraph continuation
    #[test]
    fn example_106() {
        let doc = parse("Foo\nbar\n\\---\nbaz\n");
        assert_eq!(doc.children.len(), 1);
        assert!(matches!(&doc.children[0], BlockNode::Paragraph(_)));
    }

    // Example 76: escaped # in closing sequence
    #[test]
    fn example_76() {
        let doc = parse("### foo \\###\n## foo #\\##\n# foo \\#\n");
        println!("{:#?}", doc);
        assert_eq!(doc.children.len(), 3);
        if let BlockNode::Heading(h) = &doc.children[0] {
            assert_eq!(h.level, 3);
            // "foo ###" after inline parsing of "foo \###"
        }
        if let BlockNode::Heading(h) = &doc.children[1] {
            assert_eq!(h.level, 2);
        }
        if let BlockNode::Heading(h) = &doc.children[2] {
            assert_eq!(h.level, 1);
        }
    }

    // Example 61: thematic break inside list item
    #[test]
    fn example_61() {
        let doc = parse("- Foo\n- * * *\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::List(list) = &doc.children[0] {
            assert_eq!(list.children.len(), 2);
            assert!(matches!(&list.children[1].children[0], BlockNode::ThematicBreak(_)));
        } else {
            panic!("Expected list");
        }
    }

    // Example 16: backslash escape → hard break
    #[test]
    fn example_16() {
        let doc = parse("foo\\\nbar\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert!(p.children.iter().any(|c| matches!(c, InlineNode::HardBreak)));
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 18: indented code block (escapes not processed)
    #[test]
    fn example_18() {
        let doc = parse("    \\[\\]\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::CodeBlock(cb) = &doc.children[0] {
            assert_eq!(cb.content, "\\[\\]");
        } else {
            panic!("Expected code block");
        }
    }

    // Example 19: fenced code block
    #[test]
    fn example_19() {
        let doc = parse("~~~\n\\[\\]\n~~~\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::CodeBlock(cb) = &doc.children[0] {
            assert_eq!(cb.content, "\\[\\]");
        } else {
            panic!("Expected code block");
        }
    }

    // Example 24: fenced code with info string
    #[test]
    fn example_24() {
        let doc = parse("``` foo\\+bar\nfoo\n```\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::CodeBlock(cb) = &doc.children[0] {
            assert_eq!(cb.info.as_deref(), Some("foo\\+bar"));
            assert_eq!(cb.content, "foo");
        } else {
            panic!("Expected code block");
        }
    }

    // Example 327: code span + text
    #[test]
    fn example_327() {
        let doc = parse("`hi`lo`\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert!(p.children.iter().any(|c| matches!(c, InlineNode::CodeSpan(_))));
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 612: plain text (no autolink without angle brackets)
    #[test]
    fn example_612() {
        let doc = parse("foo@bar.example.com\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            // Should NOT be an autolink
            assert!(!p.children.iter().any(|c| matches!(c, InlineNode::Autolink(_))));
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 638: emphasis with hard break inside
    #[test]
    fn example_638() {
        let doc = parse("*foo  \nbar*\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert!(p.children.iter().any(|c| matches!(c, InlineNode::Emphasis(_))));
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 639: emphasis with backslash hard break inside
    #[test]
    fn example_639() {
        let doc = parse("*foo\\\nbar*\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert!(p.children.iter().any(|c| matches!(c, InlineNode::Emphasis(_))));
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 646: heading with trailing backslash
    #[test]
    fn example_646() {
        let doc = parse("### foo\\\n");
        assert_eq!(doc.children.len(), 1);
        assert!(matches!(&doc.children[0], BlockNode::Heading(_)));
    }

    // Example 647: heading with trailing spaces stripped
    #[test]
    fn example_647() {
        let doc = parse("### foo  \n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Heading(h) = &doc.children[0] {
            assert_eq!(h.level, 3);
        } else {
            panic!("Expected heading");
        }
    }

    // Example 650: paragraph with punctuation
    #[test]
    fn example_650() {
        let doc = parse("hello $.;'there\n");
        assert_eq!(doc.children.len(), 1);
        assert!(matches!(&doc.children[0], BlockNode::Paragraph(_)));
    }

    // Example 651: paragraph with unicode text
    #[test]
    fn example_651() {
        let doc = parse("Foo χρῆν\n");
        assert_eq!(doc.children.len(), 1);
        assert!(matches!(&doc.children[0], BlockNode::Paragraph(_)));
    }

    // Example 652: paragraph preserving multiple spaces
    #[test]
    fn example_652() {
        let doc = parse("Multiple     spaces\n");
        assert_eq!(doc.children.len(), 1);
        assert!(matches!(&doc.children[0], BlockNode::Paragraph(_)));
    }

    // Example 286: list item with 1 space indent, containing paragraph + code + blockquote
    #[test]
    fn example_286() {
        let doc = parse(" 1.  A paragraph\n     with two lines.\n\n         indented code\n\n     > A block quote.\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::List(list) = &doc.children[0] {
            assert_eq!(list.children.len(), 1);
            let item = &list.children[0];
            // loose list item: paragraph, code block, blockquote
            assert_eq!(item.children.len(), 3);
            assert!(matches!(&item.children[0], BlockNode::Paragraph(_)));
            assert!(matches!(&item.children[1], BlockNode::CodeBlock(_)));
            assert!(matches!(&item.children[2], BlockNode::Blockquote(_)));
        } else {
            panic!("Expected list");
        }
    }

    // Example 287: list item with 2 space indent, containing paragraph + code + blockquote
    #[test]
    fn example_287() {
        let doc = parse("  1.  A paragraph\n      with two lines.\n\n          indented code\n\n      > A block quote.\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::List(list) = &doc.children[0] {
            assert_eq!(list.children.len(), 1);
            let item = &list.children[0];
            assert_eq!(item.children.len(), 3);
            assert!(matches!(&item.children[0], BlockNode::Paragraph(_)));
            assert!(matches!(&item.children[1], BlockNode::CodeBlock(_)));
            assert!(matches!(&item.children[2], BlockNode::Blockquote(_)));
        } else {
            panic!("Expected list");
        }
    }

    // Example 288: list item with 3 space indent, containing paragraph + code + blockquote
    #[test]
    fn example_288() {
        let doc = parse("   1.  A paragraph\n       with two lines.\n\n           indented code\n\n       > A block quote.\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::List(list) = &doc.children[0] {
            assert_eq!(list.children.len(), 1);
            let item = &list.children[0];
            assert_eq!(item.children.len(), 3);
            assert!(matches!(&item.children[0], BlockNode::Paragraph(_)));
            assert!(matches!(&item.children[1], BlockNode::CodeBlock(_)));
            assert!(matches!(&item.children[2], BlockNode::Blockquote(_)));
        } else {
            panic!("Expected list");
        }
    }

    // Example 289: 4 space indent → indented code block, not a list
    #[test]
    fn example_289() {
        let doc = parse("    1.  A paragraph\n        with two lines.\n\n            indented code\n\n        > A block quote.\n");
        assert_eq!(doc.children.len(), 1);
        assert!(matches!(&doc.children[0], BlockNode::CodeBlock(_)));
    }

    // Example 290: lazy continuation in list item
    // TODO: lazy continuation not supported — paragraph text without indent not folded into list item
    #[test]
    #[ignore]
    fn example_290() {
        let doc = parse("  1.  A paragraph\nwith two lines.\n\n          indented code\n\n      > A block quote.\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::List(list) = &doc.children[0] {
            assert_eq!(list.children.len(), 1);
            let item = &list.children[0];
            assert_eq!(item.children.len(), 3);
            assert!(matches!(&item.children[0], BlockNode::Paragraph(_)));
            assert!(matches!(&item.children[1], BlockNode::CodeBlock(_)));
            assert!(matches!(&item.children[2], BlockNode::Blockquote(_)));
        } else {
            panic!("Expected list");
        }
    }

    // Example 291: partial indent continuation → tight list with one item
    // TODO: lazy continuation not supported — partially indented continuation line not recognized
    #[test]
    #[ignore]
    fn example_291() {
        let doc = parse("  1.  A paragraph\n    with two lines.\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::List(list) = &doc.children[0] {
            assert_eq!(list.children.len(), 1);
            assert!(list.tight);
        } else {
            panic!("Expected list");
        }
    }

    // Example 292: blockquote > ordered list > blockquote (lazy continuation)
    #[test]
    fn example_292() {
        let doc = parse("> 1. > Blockquote\ncontinued here.\n");
        assert_eq!(doc.children.len(), 1);
        assert!(matches!(&doc.children[0], BlockNode::Blockquote(_)));
    }

    // Example 293: blockquote > ordered list > blockquote (non-lazy continuation)
    #[test]
    fn example_293() {
        let doc = parse("> 1. > Blockquote\n> continued here.\n");
        assert_eq!(doc.children.len(), 1);
        assert!(matches!(&doc.children[0], BlockNode::Blockquote(_)));
    }

    // Example 294: nested bullet lists (foo > bar > baz > boo)
    #[test]
    fn example_294() {
        let doc = parse("- foo\n  - bar\n    - baz\n      - boo\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::List(list) = &doc.children[0] {
            assert_eq!(list.children.len(), 1);
            // foo item should contain a nested list
            assert!(list.children[0].children.len() >= 2);
        } else {
            panic!("Expected list");
        }
    }

    // Example 295: insufficient indent → all items in same list
    #[test]
    fn example_295() {
        let doc = parse("- foo\n - bar\n  - baz\n   - boo\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::List(list) = &doc.children[0] {
            assert_eq!(list.children.len(), 4);
        } else {
            panic!("Expected list");
        }
    }

    // Example 296: ordered list with sub bullet (10) foo + indented - bar)
    #[test]
    fn example_296() {
        let doc = parse("10) foo\n    - bar\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::List(list) = &doc.children[0] {
            assert_eq!(list.start, 10);
            assert_eq!(list.children.len(), 1);
            // item contains paragraph and nested list
            assert!(list.children[0].children.len() >= 2);
        } else {
            panic!("Expected ordered list");
        }
    }

    // Example 297: ordered list + separate bullet list (insufficient indent)
    #[test]
    fn example_297() {
        let doc = parse("10) foo\n   - bar\n");
        assert_eq!(doc.children.len(), 2);
        assert!(matches!(&doc.children[0], BlockNode::List(_)));
        assert!(matches!(&doc.children[1], BlockNode::List(_)));
    }

    // Example 298: nested bullet list (- - foo)
    #[test]
    fn example_298() {
        let doc = parse("- - foo\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::List(list) = &doc.children[0] {
            assert_eq!(list.children.len(), 1);
            // inner should have a nested list
            assert!(list.children[0].children.iter().any(|c| matches!(c, BlockNode::List(_))));
        } else {
            panic!("Expected list");
        }
    }

    // Example 299: deeply nested mixed lists (1. - 2. foo)
    #[test]
    fn example_299() {
        let doc = parse("1. - 2. foo\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::List(list) = &doc.children[0] {
            assert_eq!(list.children.len(), 1);
        } else {
            panic!("Expected list");
        }
    }

    // Example 300: list items with heading and setext heading
    #[test]
    fn example_300() {
        let doc = parse("- # Foo\n- Bar\n  ---\n  baz\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::List(list) = &doc.children[0] {
            assert_eq!(list.children.len(), 2);
            // First item contains an ATX heading
            assert!(list.children[0].children.iter().any(|c| matches!(c, BlockNode::Heading(_))));
            // Second item contains a setext heading
            assert!(list.children[1].children.iter().any(|c| matches!(c, BlockNode::Heading(_))));
        } else {
            panic!("Expected list");
        }
    }
}
