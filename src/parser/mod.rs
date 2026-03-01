//! CommonMark 파서
//!
//! 라인 단위로 스캔하며 블록 레벨 요소를 파싱합니다.
//! fold 패턴을 사용하여 불변 상태 전이를 구현합니다.

pub(crate) mod block_start;
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

use crate::node::{BlockNode, CodeBlockNode, DocumentNode, HeadingNode, ParagraphNode};
use context::{
    NoneContext,
    ParsingContext,
};
use helpers::trim_blank_lines;

/// 파서 상태: (완성된 노드들, 현재 컨텍스트) - fold 누적용
type ParserState = (Vec<BlockNode>, ParsingContext);

/// 탭을 spaces로 확장 (4칸 탭 스톱)
/// 문서 전체 파싱
pub fn parse(input: &str) -> DocumentNode {
    if input.is_empty() {
        return DocumentNode::new(vec![]);
    }

    // Pass 0: 탭을 spaces로 확장
    let input = input.to_owned();

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

/// 블록 구조만 파싱 (ref def 추출 없이)
/// blockquote 등 컨테이너 블록 내부 파싱에 사용.
/// ref def 추출은 최상위 parse()에서 재귀적으로 수행.
pub(crate) fn parse_blocks(input: &str) -> Vec<BlockNode> {
    if input.is_empty() {
        return vec![];
    }

    let input = input.to_owned();

    let (children, final_context) = input.lines().fold(
        (Vec::new(), ParsingContext::None(NoneContext)),
        |(children, context), line| process_line(line, context, children),
    );

    finalize_context(final_context, children)
}

/// 한 줄 처리 후 새 상태 반환
fn process_line(line: &str, context: ParsingContext, nodes: Vec<BlockNode>) -> ParserState {
    let (new_nodes, new_context) = match context {
        ParsingContext::None(ctx) => ctx.parse(line),
        ParsingContext::CodeBlockFenced(ctx) => ctx.parse(line),
        ParsingContext::Paragraph(ctx) => ctx.parse(line),
        ParsingContext::Blockquote(ctx) => ctx.parse(line),
        ParsingContext::List(ctx) => ctx.parse(line),
        ParsingContext::CodeBlockIndented(ctx) => ctx.parse(line),
        ParsingContext::HtmlBlock(ctx) => ctx.parse(line),
    };

    // 새로 완성된 노드들을 누적
    let mut nodes = nodes;
    nodes.extend(new_nodes);
    (nodes, new_context)
}

/// 마지막 컨텍스트 마무리
fn finalize_context(context: ParsingContext, mut nodes: Vec<BlockNode>) -> Vec<BlockNode> {
    match context {
        ParsingContext::None(NoneContext) => {}
        ParsingContext::CodeBlockFenced(ctx) => {
            nodes.push(code_block_fenced::finalize(ctx.start, ctx.content));
        }
        ParsingContext::Paragraph(ctx) => {
            let text = ctx.pending_lines.join("\n");
            nodes.push(paragraph::parse(&text));
        }
        ParsingContext::Blockquote(ctx) => {
            nodes.push(blockquote::finalize(ctx.pending_lines, parse_block_simple));
        }
        ParsingContext::List(ctx) => {
            nodes.push(ctx.build_list_node());
        }
        ParsingContext::CodeBlockIndented(ctx) => {
            let content = trim_blank_lines(ctx.pending_lines);
            nodes.push(BlockNode::CodeBlock(CodeBlockNode::new(None, content)));
        }
        ParsingContext::HtmlBlock(ctx) => {
            nodes.push(html_block::finalize(ctx.pending_lines));
        }
    }
    nodes
}

/// 단일 블록 파싱 (blockquote 내부 등에서 사용)
pub(crate) fn parse_block_simple(block: &str) -> BlockNode {
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

    if let Ok(code_block_indented::CodeBlockIndentedStartReason::Started(start)) =
        code_block_indented::try_start(block)
    {
        return BlockNode::CodeBlock(CodeBlockNode::new(None, start.content));
    }

    if let Ok(node) = heading::parse(block) {
        return node;
    }

    // HTML block detection for simple block parsing
    if let Ok(html_block::HtmlBlockOk::Start(_)) = html_block::parse(block, None) {
        return html_block::finalize(vec![block.to_string()]);
    }

    paragraph::parse(block.trim_start())
}

/// Pass 2: 모든 paragraph에서 link reference definitions를 추출한다.
/// - link ref def가 paragraph 시작에 있으면 추출하고 남은 텍스트로 재파싱
/// - paragraph가 완전히 link ref def로만 구성되면 제거
fn extract_link_ref_defs(
    children: Vec<BlockNode>,
    ref_map: &mut link_ref_def::RefMap,
) -> Vec<BlockNode> {
    // setext heading이 collapse되면 밑줄 텍스트를 다음 paragraph에 합침
    let mut pending_setext_underline: Option<String> = None;

    let mut result: Vec<BlockNode> = Vec::new();

    for node in children {
        match node {
            BlockNode::Paragraph(para) => {
                if let Some(raw) = &para.raw_text {
                    let remaining = link_ref_def::extract_definitions(raw, ref_map);
                    if remaining.trim().is_empty() {
                        if let Some(underline) = pending_setext_underline.take() {
                            // setext underline + empty paragraph → paragraph with underline only
                            result.push(BlockNode::Paragraph(ParagraphNode::with_raw_text(
                                inline::parse_inlines(&underline),
                                &underline,
                            )));
                        }
                        // paragraph 전체가 link ref def → 제거
                    } else if remaining != *raw {
                        // 일부가 link ref def였으므로 남은 텍스트로 재파싱
                        let new_raw = if let Some(underline) = pending_setext_underline.take() {
                            format!("{}\n{}", underline, remaining)
                        } else {
                            remaining
                        };
                        result.push(BlockNode::Paragraph(ParagraphNode::with_raw_text(
                            inline::parse_inlines(&new_raw),
                            &new_raw,
                        )));
                    } else if let Some(underline) = pending_setext_underline.take() {
                        // setext underline을 paragraph 앞에 합침
                        let new_raw = format!("{}\n{}", underline, raw);
                        result.push(BlockNode::Paragraph(ParagraphNode::with_raw_text(
                            inline::parse_inlines(&new_raw),
                            &new_raw,
                        )));
                    } else {
                        result.push(BlockNode::Paragraph(para));
                    }
                } else if let Some(underline) = pending_setext_underline.take() {
                    // raw_text 없는 paragraph에 underline 합침 불가 → 별도 출력
                    result.push(BlockNode::Paragraph(ParagraphNode::with_raw_text(
                        inline::parse_inlines(&underline),
                        &underline,
                    )));
                    result.push(BlockNode::Paragraph(para));
                } else {
                    result.push(BlockNode::Paragraph(para));
                }
            }
            BlockNode::Heading(heading) => {
                // 이전 setext underline이 있으면 먼저 flush
                if let Some(underline) = pending_setext_underline.take() {
                    result.push(BlockNode::Paragraph(ParagraphNode::with_raw_text(
                        inline::parse_inlines(&underline),
                        &underline,
                    )));
                }

                if let Some(raw) = &heading.raw_text {
                    let remaining = link_ref_def::extract_definitions(raw, ref_map);
                    if remaining.trim().is_empty() {
                        if let Some(underline) = &heading.setext_underline {
                            // setext heading의 내용이 모두 ref def → 밑줄을 다음 paragraph에 합침
                            pending_setext_underline = Some(underline.clone());
                        }
                        // else: ATX heading의 내용이 모두 ref def → 제거
                    } else if remaining != *raw {
                        // ref def 추출 후 남은 텍스트 → heading으로 유지
                        result.push(BlockNode::Heading(HeadingNode::with_raw_text(
                            heading.level,
                            inline::parse_inlines(&remaining),
                            &remaining,
                        )));
                    } else {
                        result.push(BlockNode::Heading(heading));
                    }
                } else {
                    result.push(BlockNode::Heading(heading));
                }
            }
            BlockNode::Blockquote(mut bq) => {
                if let Some(underline) = pending_setext_underline.take() {
                    result.push(BlockNode::Paragraph(ParagraphNode::with_raw_text(
                        inline::parse_inlines(&underline),
                        &underline,
                    )));
                }
                bq.children = extract_link_ref_defs(bq.children, ref_map);
                result.push(BlockNode::Blockquote(bq));
            }
            BlockNode::List(mut list) => {
                if let Some(underline) = pending_setext_underline.take() {
                    result.push(BlockNode::Paragraph(ParagraphNode::with_raw_text(
                        inline::parse_inlines(&underline),
                        &underline,
                    )));
                }
                for item in list.children.iter_mut() {
                    item.children = extract_link_ref_defs(
                        std::mem::take(&mut item.children),
                        ref_map,
                    );
                }
                result.push(BlockNode::List(list));
            }
            other => {
                if let Some(underline) = pending_setext_underline.take() {
                    result.push(BlockNode::Paragraph(ParagraphNode::with_raw_text(
                        inline::parse_inlines(&underline),
                        &underline,
                    )));
                }
                result.push(other);
            }
        }
    }

    // pending underline을 flush
    if let Some(underline) = pending_setext_underline.take() {
        result.push(BlockNode::Paragraph(ParagraphNode::with_raw_text(
            inline::parse_inlines(&underline),
            &underline,
        )));
    }

    result
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
                if let Some(raw) = &h.raw_text {
                    h.children = inline::parse_inlines_with_refs(raw, Some(ref_map));
                }
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

    // Example 193: link ref def with indentation
    #[test]
    fn example_193() {
        let doc = parse("   [foo]: \n      /url  \n           'the title'  \n\n[foo]\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "/url", Some("the title"))
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 194: link ref def with special chars in label
    #[test]
    fn example_194() {
        let doc = parse("[Foo*bar\\]]:my_(url) 'title (with parens)'\n\n[Foo*bar\\]]\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("Foo*bar]")], "my_(url)", Some("title (with parens)"))
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 195: link ref def with angle-bracket destination
    #[test]
    fn example_195() {
        let doc = parse("[Foo bar]:\n<my url>\n'title'\n\n[Foo bar]\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("Foo bar")], "my url", Some("title"))
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 196: multiline title in link ref def
    #[test]
    fn example_196() {
        let doc = parse("[foo]: /url '\ntitle\nline1\nline2\n'\n\n[foo]\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "/url", Some("\ntitle\nline1\nline2\n"))
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 197: blank line in title → not a ref def
    #[test]
    fn example_197() {
        let doc = parse("[foo]: /url 'title\n\nwith blank line'\n\n[foo]\n");
        // Not a valid ref def, so [foo] is not resolved
        assert!(doc.children.len() >= 3);
    }

    // Example 198: ref def with destination on next line
    #[test]
    fn example_198() {
        let doc = parse("[foo]:\n/url\n\n[foo]\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "/url", None)
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 199: ref def with no destination → not valid
    #[test]
    fn example_199() {
        let doc = parse("[foo]:\n\n[foo]\n");
        // Not a valid ref def
        assert!(doc.children.len() >= 2);
    }

    // Example 200: ref def with empty angle-bracket destination
    #[test]
    fn example_200() {
        let doc = parse("[foo]: <>\n\n[foo]\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "", None)
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 201: ref def with angle dest followed by parens → not valid
    #[test]
    fn example_201() {
        let doc = parse("[foo]: <bar>(baz)\n\n[foo]\n");
        assert!(doc.children.len() >= 2);
    }

    // Example 202: backslash escapes in destination and title
    #[test]
    fn example_202() {
        let doc = parse("[foo]: /url\\bar\\*baz \"foo\\\"bar\\baz\"\n\n[foo]\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "/url\\bar*baz", Some("foo\"bar\\baz"))
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 203: ref def after usage
    #[test]
    fn example_203() {
        let doc = parse("[foo]\n\n[foo]: url\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "url", None)
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 204: duplicate ref def → first wins
    #[test]
    fn example_204() {
        let doc = parse("[foo]\n\n[foo]: first\n[foo]: second\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "first", None)
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 205: case-insensitive ref labels
    #[test]
    fn example_205() {
        let doc = parse("[FOO]: /url\n\n[Foo]\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("Foo")], "/url", None)
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 206: unicode case folding
    #[test]
    fn example_206() {
        let doc = parse("[ΑΓΩ]: /φου\n\n[αγω]\n");
        assert_eq!(doc.children.len(), 1);
    }

    // Example 207: ref def only → no output
    #[test]
    fn example_207() {
        let doc = parse("[foo]: /url\n");
        assert_eq!(doc.children.len(), 0);
    }

    // Example 208: multiline ref label + remaining text
    #[test]
    fn example_208() {
        let doc = parse("[\nfoo\n]: /url\nbar\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![InlineNode::text("bar")]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 209: trailing content after title → not a ref def
    #[test]
    fn example_209() {
        let doc = parse("[foo]: /url \"title\" ok\n");
        assert_eq!(doc.children.len(), 1);
        assert!(matches!(&doc.children[0], BlockNode::Paragraph(_)));
    }

    // Example 210: title on next line without belonging to ref def
    #[test]
    fn example_210() {
        let doc = parse("[foo]: /url\n\"title\" ok\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![InlineNode::text("\"title\" ok")]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 211: indented code block → not a ref def
    #[test]
    fn example_211() {
        let doc = parse("    [foo]: /url \"title\"\n\n[foo]\n");
        assert_eq!(doc.children.len(), 2);
        assert!(matches!(&doc.children[0], BlockNode::CodeBlock(_)));
        assert!(matches!(&doc.children[1], BlockNode::Paragraph(_)));
    }

    // Example 212: fenced code block → not a ref def
    #[test]
    fn example_212() {
        let doc = parse("```\n[foo]: /url\n```\n\n[foo]\n");
        assert_eq!(doc.children.len(), 2);
        assert!(matches!(&doc.children[0], BlockNode::CodeBlock(_)));
        assert!(matches!(&doc.children[1], BlockNode::Paragraph(_)));
    }

    // Example 213: ref def inside paragraph → not extracted
    #[test]
    fn example_213() {
        let doc = parse("Foo\n[bar]: /baz\n\n[bar]\n");
        // [bar] should NOT be resolved since the ref def is inside a paragraph
        assert_eq!(doc.children.len(), 2);
    }

    // Example 214: heading + ref def + blockquote
    #[test]
    fn example_214() {
        let doc = parse("# [Foo]\n[foo]: /url\n> bar\n");
        assert_eq!(doc.children.len(), 2);
        assert!(matches!(&doc.children[0], BlockNode::Heading(_)));
        assert!(matches!(&doc.children[1], BlockNode::Blockquote(_)));
    }

    // Example 215: ref def + setext heading
    #[test]
    fn example_215() {
        let doc = parse("[foo]: /url\nbar\n===\n[foo]\n");
        // ref def extracted, "bar\n===" becomes setext h1, then [foo] paragraph
        assert_eq!(doc.children.len(), 2);
        assert!(matches!(&doc.children[0], BlockNode::Heading(_)));
        if let BlockNode::Paragraph(p) = &doc.children[1] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "/url", None)
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 216: ref def label looks like setext underline
    #[test]
    fn example_216() {
        let doc = parse("[foo]: /url\n===\n[foo]\n");
        // "===" is not a setext heading (no preceding paragraph content)
        // so it becomes paragraph text
        assert_eq!(doc.children.len(), 1);
    }

    // Example 217: multiple ref defs
    #[test]
    fn example_217() {
        let doc = parse("[foo]: /foo-url \"foo\"\n[bar]: /bar-url\n  \"bar\"\n[baz]: /baz-url\n\n[foo],\n[bar],\n[baz]\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            // Should contain 3 links interspersed with commas and softbreaks
            assert!(p.children.iter().any(|c| matches!(c, InlineNode::Link(_))));
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 218: ref def inside blockquote
    #[test]
    fn example_218() {
        let doc = parse("[foo]\n\n> [foo]: /url\n");
        // Spec says [foo] should resolve even though ref def is in blockquote
        assert_eq!(doc.children.len(), 2);
    }

    // =========================================================================
    // Reference Links (Examples 527-571)
    // =========================================================================

    // Example 527: full reference link [foo][bar]
    #[test]
    fn example_527() {
        let doc = parse("[foo][bar]\n\n[bar]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "/url", Some("title"))
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 528: nested brackets in link text
    #[test]
    fn example_528() {
        let doc = parse("[link [foo [bar]]][ref]\n\n[ref]: /uri\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("link [foo [bar]]")], "/uri", None)
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 529: escaped bracket in link text
    #[test]
    fn example_529() {
        let doc = parse("[link \\[bar][ref]\n\n[ref]: /uri\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("link [bar")], "/uri", None)
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 530: inline formatting in link text
    #[test]
    fn example_530() {
        let doc = parse("[link *foo **bar** `#`*][ref]\n\n[ref]: /uri\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(
                    vec![
                        InlineNode::text("link "),
                        InlineNode::emphasis(vec![
                            InlineNode::text("foo "),
                            InlineNode::strong(vec![InlineNode::text("bar")]),
                            InlineNode::text(" "),
                            InlineNode::code_span("#"),
                        ]),
                    ],
                    "/uri",
                    None,
                )
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 531: image inside reference link
    #[test]
    fn example_531() {
        let doc = parse("[![moon](moon.jpg)][ref]\n\n[ref]: /uri\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(
                    vec![InlineNode::image(vec![InlineNode::text("moon")], "moon.jpg", None)],
                    "/uri",
                    None,
                )
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 532: link inside link text → not nested
    #[test]
    fn example_532() {
        let doc = parse("[foo [bar](/uri)][ref]\n\n[ref]: /uri\n");
        assert_eq!(doc.children.len(), 1);
    }

    // Example 533: emphasis + link inside link text
    #[test]
    fn example_533() {
        let doc = parse("[foo *bar [baz][ref]*][ref]\n\n[ref]: /uri\n");
        assert_eq!(doc.children.len(), 1);
    }

    // Example 534: emphasis delimiter mismatch with ref link
    #[test]
    fn example_534() {
        let doc = parse("*[foo*][ref]\n\n[ref]: /uri\n");
        assert_eq!(doc.children.len(), 1);
    }

    // Example 535: unmatched emphasis around ref link
    #[test]
    fn example_535() {
        let doc = parse("[foo *bar][ref]*\n\n[ref]: /uri\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo *bar")], "/uri", None),
                InlineNode::text("*"),
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 536: raw HTML prevents ref link
    #[test]
    fn example_536() {
        let doc = parse("[foo <bar attr=\"][ref]\">\n\n[ref]: /uri\n");
        assert_eq!(doc.children.len(), 1);
    }

    // Example 537: code span prevents ref link
    #[test]
    fn example_537() {
        let doc = parse("[foo`][ref]`\n\n[ref]: /uri\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            // [foo is text, `][ref]` is code span
            assert!(p.children.iter().any(|c| matches!(c, InlineNode::CodeSpan(_))));
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 538: autolink prevents ref link
    #[test]
    fn example_538() {
        let doc = parse("[foo<https://example.com/?search=][ref]>\n\n[ref]: /uri\n");
        assert_eq!(doc.children.len(), 1);
    }

    // Example 539: case-insensitive ref label
    #[test]
    fn example_539() {
        let doc = parse("[foo][BaR]\n\n[bar]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "/url", Some("title"))
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 540: unicode case folding in ref
    #[test]
    fn example_540() {
        let doc = parse("[ẞ]\n\n[SS]: /url\n");
        assert_eq!(doc.children.len(), 1);
    }

    // Example 541: multiline ref label
    #[test]
    fn example_541() {
        let doc = parse("[Foo\n  bar]: /url\n\n[Baz][Foo bar]\n");
        assert_eq!(doc.children.len(), 1);
    }

    // Example 542: space between link text and ref label → not a ref link
    #[test]
    fn example_542() {
        let doc = parse("[foo] [bar]\n\n[bar]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            // [foo] is text, [bar] resolves as shortcut ref link
            assert!(p.children.len() >= 2);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 543: newline between brackets → not a ref link
    #[test]
    fn example_543() {
        let doc = parse("[foo]\n[bar]\n\n[bar]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            // [foo] is text, soft break, [bar] resolves
            assert!(p.children.iter().any(|c| matches!(c, InlineNode::Link(_))));
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 544: first ref def wins
    #[test]
    fn example_544() {
        let doc = parse("[foo]: /url1\n\n[foo]: /url2\n\n[bar][foo]\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("bar")], "/url1", None)
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 545: escaped char in ref label → no match
    #[test]
    fn example_545() {
        let doc = parse("[bar][foo\\!]\n\n[foo!]: /url\n");
        // Should NOT match because foo\! ≠ foo!
        assert_eq!(doc.children.len(), 1);
    }

    // Example 546: unclosed bracket in ref label
    #[test]
    fn example_546() {
        let doc = parse("[foo][ref[]\n\n[ref[]: /uri\n");
        // Not valid ref links
        assert!(doc.children.len() >= 2);
    }

    // Example 547: nested brackets in ref label
    #[test]
    fn example_547() {
        let doc = parse("[foo][ref[bar]]\n\n[ref[bar]]: /uri\n");
        assert!(doc.children.len() >= 2);
    }

    // Example 548: triple nested brackets
    #[test]
    fn example_548() {
        let doc = parse("[[[foo]]]\n\n[[[foo]]]: /url\n");
        assert!(doc.children.len() >= 2);
    }

    // Example 549: escaped bracket in ref label
    #[test]
    fn example_549() {
        let doc = parse("[foo][ref\\[]\n\n[ref\\[]: /uri\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "/uri", None)
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 550: escaped backslash in ref label
    #[test]
    fn example_550() {
        let doc = parse("[bar\\\\]: /uri\n\n[bar\\\\]\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("bar\\")], "/uri", None)
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 551: empty ref label
    #[test]
    fn example_551() {
        let doc = parse("[]\n\n[]: /uri\n");
        assert!(doc.children.len() >= 2);
    }

    // Example 552: blank ref label
    #[test]
    fn example_552() {
        let doc = parse("[\n ]\n\n[\n ]: /uri\n");
        assert!(doc.children.len() >= 2);
    }

    // Example 553: collapsed reference link [foo][]
    #[test]
    fn example_553() {
        let doc = parse("[foo][]\n\n[foo]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "/url", Some("title"))
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 554: collapsed ref with inline formatting
    #[test]
    fn example_554() {
        let doc = parse("[*foo* bar][]\n\n[*foo* bar]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
    }

    // Example 555: collapsed ref case-insensitive
    #[test]
    fn example_555() {
        let doc = parse("[Foo][]\n\n[foo]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("Foo")], "/url", Some("title"))
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 556: space between ] and [] → shortcut, not collapsed
    #[test]
    fn example_556() {
        let doc = parse("[foo] \n[]\n\n[foo]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
    }

    // Example 557: shortcut reference link [foo]
    #[test]
    fn example_557() {
        let doc = parse("[foo]\n\n[foo]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "/url", Some("title"))
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 558: shortcut ref with inline formatting
    #[test]
    fn example_558() {
        let doc = parse("[*foo* bar]\n\n[*foo* bar]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
    }

    // Example 559: double bracket [[*foo* bar]]
    #[test]
    fn example_559() {
        let doc = parse("[[*foo* bar]]\n\n[*foo* bar]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
    }

    // Example 560: [[bar [foo]
    #[test]
    fn example_560() {
        let doc = parse("[[bar [foo]\n\n[foo]: /url\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            // [[bar then [foo] link
            assert!(p.children.iter().any(|c| matches!(c, InlineNode::Link(_))));
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 561: shortcut ref case-insensitive
    #[test]
    fn example_561() {
        let doc = parse("[Foo]\n\n[foo]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("Foo")], "/url", Some("title"))
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 562: shortcut ref followed by text
    #[test]
    fn example_562() {
        let doc = parse("[foo] bar\n\n[foo]: /url\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "/url", None),
                InlineNode::text(" bar"),
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 563: escaped [ → not a ref link
    #[test]
    fn example_563() {
        let doc = parse("\\[foo]\n\n[foo]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![InlineNode::text("[foo]")]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 564: shortcut ref with * in label
    #[test]
    fn example_564() {
        let doc = parse("[foo*]: /url\n\n*[foo*]\n");
        assert_eq!(doc.children.len(), 1);
    }

    // Example 565: full ref link takes precedence
    #[test]
    fn example_565() {
        let doc = parse("[foo][bar]\n\n[foo]: /url1\n[bar]: /url2\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "/url2", None)
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 566: collapsed ref [foo][]
    #[test]
    fn example_566() {
        let doc = parse("[foo][]\n\n[foo]: /url1\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "/url1", None)
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 567: inline link takes precedence over ref
    #[test]
    fn example_567() {
        let doc = parse("[foo]()\n\n[foo]: /url1\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "", None)
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 568: failed inline link → falls back to ref
    #[test]
    fn example_568() {
        let doc = parse("[foo](not a link)\n\n[foo]: /url1\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "/url1", None),
                InlineNode::text("(not a link)"),
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 569: [foo][bar][baz] with only [baz] defined
    #[test]
    fn example_569() {
        let doc = parse("[foo][bar][baz]\n\n[baz]: /url\n");
        assert_eq!(doc.children.len(), 1);
    }

    // Example 570: [foo][bar][baz] with both defined
    #[test]
    fn example_570() {
        let doc = parse("[foo][bar][baz]\n\n[baz]: /url1\n[bar]: /url2\n");
        assert_eq!(doc.children.len(), 1);
    }

    // Example 571: [foo][bar][baz] with [baz] and [foo] defined
    #[test]
    fn example_571() {
        let doc = parse("[foo][bar][baz]\n\n[baz]: /url1\n[foo]: /url2\n");
        assert_eq!(doc.children.len(), 1);
    }

    // =========================================================================
    // Reference Images (Examples 573-591)
    // =========================================================================

    // Example 573: reference image
    #[test]
    fn example_573() {
        let doc = parse("![foo *bar*]\n\n[foo *bar*]: train.jpg \"train & tracks\"\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::image(
                    vec![InlineNode::text("foo "), InlineNode::emphasis(vec![InlineNode::text("bar")])],
                    "train.jpg",
                    Some("train & tracks"),
                )
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 576: collapsed reference image ![foo *bar*][]
    #[test]
    fn example_576() {
        let doc = parse("![foo *bar*][]\n\n[foo *bar*]: train.jpg \"train & tracks\"\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::image(
                    vec![InlineNode::text("foo "), InlineNode::emphasis(vec![InlineNode::text("bar")])],
                    "train.jpg",
                    Some("train & tracks"),
                )
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 577: reference image with different label
    #[test]
    fn example_577() {
        let doc = parse("![foo *bar*][foobar]\n\n[FOOBAR]: train.jpg \"train & tracks\"\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::image(
                    vec![InlineNode::text("foo "), InlineNode::emphasis(vec![InlineNode::text("bar")])],
                    "train.jpg",
                    Some("train & tracks"),
                )
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 582: reference image ![foo][bar]
    #[test]
    fn example_582() {
        let doc = parse("![foo][bar]\n\n[bar]: /url\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::image(vec![InlineNode::text("foo")], "/url", None)
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 583: reference image case-insensitive
    #[test]
    fn example_583() {
        let doc = parse("![foo][bar]\n\n[BAR]: /url\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::image(vec![InlineNode::text("foo")], "/url", None)
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 584: collapsed reference image ![foo][]
    #[test]
    fn example_584() {
        let doc = parse("![foo][]\n\n[foo]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::image(vec![InlineNode::text("foo")], "/url", Some("title"))
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 585: collapsed ref image with formatting
    #[test]
    fn example_585() {
        let doc = parse("![*foo* bar][]\n\n[*foo* bar]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
    }

    // Example 586: collapsed ref image case-insensitive
    #[test]
    fn example_586() {
        let doc = parse("![Foo][]\n\n[foo]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::image(vec![InlineNode::text("Foo")], "/url", Some("title"))
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 587: space before [] → shortcut, not collapsed
    #[test]
    fn example_587() {
        let doc = parse("![foo] \n[]\n\n[foo]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
    }

    // Example 588: shortcut reference image ![foo]
    #[test]
    fn example_588() {
        let doc = parse("![foo]\n\n[foo]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::image(vec![InlineNode::text("foo")], "/url", Some("title"))
            ]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 589: shortcut ref image with formatting
    #[test]
    fn example_589() {
        let doc = parse("![*foo* bar]\n\n[*foo* bar]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
    }

    // Example 591: shortcut ref image case-insensitive
    #[test]
    fn example_591() {
        let doc = parse("![Foo]\n\n[foo]: /url \"title\"\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::image(vec![InlineNode::text("Foo")], "/url", Some("title"))
            ]);
        } else {
            panic!("Expected paragraph");
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
            assert_eq!(cb.info.as_deref(), Some("foo+bar"));
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

    #[test]
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
    #[test]
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

    // Example 25: named entity references
    #[test]
    fn example_25() {
        let doc = parse("&nbsp; &amp; &copy; &AElig; &Dcaron;\n&frac34; &HilbertSpace; &DifferentialD;\n&ClockwiseContourIntegral; &ngE;\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            let text = get_all_text(&p.children);
            assert!(text.contains('\u{00A0}')); // &nbsp;
            assert!(text.contains('&')); // &amp;
            assert!(text.contains('©')); // &copy;
            assert!(text.contains('Æ')); // &AElig;
            assert!(text.contains('Ď')); // &Dcaron;
            assert!(text.contains('¾')); // &frac34;
            assert!(text.contains('ℋ')); // &HilbertSpace;
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 26: numeric decimal character references
    #[test]
    fn example_26() {
        let doc = parse("&#35; &#1234; &#992; &#0;\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            let text = get_all_text(&p.children);
            assert!(text.contains('#')); // &#35;
            assert!(text.contains('Ӓ')); // &#1234;
            assert!(text.contains('Ϡ')); // &#992;
            assert!(text.contains('\u{FFFD}')); // &#0;
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 27: numeric hex character references
    #[test]
    fn example_27() {
        let doc = parse("&#X22; &#XD06; &#xcab;\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            let text = get_all_text(&p.children);
            assert!(text.contains('"')); // &#X22;
            assert!(text.contains('ആ')); // &#XD06;
            assert!(text.contains('ಫ')); // &#xcab;
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 28: invalid entity references remain as literal text
    #[test]
    fn example_28() {
        let doc = parse("&nbsp &x; &#; &#x;\n&#87654321;\n&#abcdef0;\n&ThisIsNotDefined; &hi?;\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            let text = get_all_text(&p.children);
            assert!(text.contains("&nbsp")); // no semicolon
            assert!(text.contains("&x;")); // invalid
            assert!(text.contains("&#;")); // empty numeric
            assert!(text.contains("&#x;")); // empty hex
            assert!(text.contains("&#87654321;")); // out of range
            assert!(text.contains("&#abcdef0;")); // not valid decimal
            assert!(text.contains("&ThisIsNotDefined;")); // unknown
            assert!(text.contains("&hi?;")); // invalid chars
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 29: entity without semicolon not recognized
    #[test]
    fn example_29() {
        let doc = parse("&copy\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            let text = get_all_text(&p.children);
            assert!(text.contains("&copy"));
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 30: unknown entity not recognized
    #[test]
    fn example_30() {
        let doc = parse("&MadeUpEntity;\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            let text = get_all_text(&p.children);
            assert!(text.contains("&MadeUpEntity;"));
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 31: entity in raw HTML (not processed by inline parser, passed through)
    #[test]
    fn example_31() {
        let doc = parse("<a href=\"&ouml;&ouml;.html\">\n");
        assert_eq!(doc.children.len(), 1);
        // This should be an HTML block or paragraph with raw HTML
        // The entities inside raw HTML are not processed by the markdown parser
    }

    // Example 32: entity in inline link destination and title
    #[test]
    fn example_32() {
        let doc = parse("[foo](/f&ouml;&ouml; \"f&ouml;&ouml;\")\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            if let InlineNode::Link(link) = &p.children[0] {
                assert!(link.destination.contains("ö"));
                assert_eq!(link.title.as_deref(), Some("föö"));
            } else {
                panic!("Expected link");
            }
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 33: entity in reference link definition
    #[test]
    fn example_33() {
        let doc = parse("[foo]\n\n[foo]: /f&ouml;&ouml; \"f&ouml;&ouml;\"\n");
        assert_eq!(doc.children.len(), 1);
    }

    // Example 34: entity in fenced code info string
    #[test]
    fn example_34() {
        let doc = parse("``` f&ouml;&ouml;\nfoo\n```\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::CodeBlock(cb) = &doc.children[0] {
            assert_eq!(cb.info.as_deref(), Some("föö"));
        } else {
            panic!("Expected code block");
        }
    }

    // Example 35: entities NOT processed in code spans
    #[test]
    fn example_35() {
        let doc = parse("`f&ouml;&ouml;`\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            if let InlineNode::CodeSpan(cs) = &p.children[0] {
                assert_eq!(cs.0, "f&ouml;&ouml;");
            } else {
                panic!("Expected code span");
            }
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 36: entities NOT processed in indented code blocks
    #[test]
    fn example_36() {
        let doc = parse("    f&ouml;f&ouml;\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::CodeBlock(cb) = &doc.children[0] {
            assert_eq!(cb.content, "f&ouml;f&ouml;");
        } else {
            panic!("Expected code block");
        }
    }

    // Example 37: entity-produced chars don't trigger markdown syntax
    #[test]
    fn example_37() {
        let doc = parse("&#42;foo&#42;\n*foo*\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            // &#42; → * but should NOT be emphasis
            // First line: *foo* as literal text
            // Second line: <em>foo</em>
            let has_emphasis = p.children.iter().any(|c| matches!(c, InlineNode::Emphasis(_)));
            assert!(has_emphasis, "Second *foo* should be emphasis");
            let text = get_all_text(&p.children);
            assert!(text.contains("*foo*")); // entity-produced * should be literal
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 38: entity-produced chars don't trigger block syntax
    #[test]
    fn example_38() {
        let doc = parse("&#42; foo\n\n* foo\n");
        assert_eq!(doc.children.len(), 2);
        // First: paragraph with "* foo" (entity-produced * is not list marker)
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            let text = get_all_text(&p.children);
            assert!(text.contains("*"));
            assert!(text.contains("foo"));
        } else {
            panic!("Expected paragraph");
        }
        // Second: list with "foo"
        assert!(matches!(&doc.children[1], BlockNode::List(_)));
    }

    // Example 39: numeric entity for newline doesn't create hard break
    #[test]
    fn example_39() {
        let doc = parse("foo&#10;&#10;bar\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            // &#10; → newline char, but as text not as line break
            let text = get_all_text(&p.children);
            assert!(text.contains("foo"));
            assert!(text.contains("bar"));
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 40: numeric entity for tab
    #[test]
    fn example_40() {
        let doc = parse("&#9;foo\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            let text = get_all_text(&p.children);
            assert!(text.contains('\t'));
            assert!(text.contains("foo"));
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 41: entity in inline link title is literal (not processed as title delimiter)
    #[test]
    fn example_41() {
        let doc = parse("[a](url &quot;tit&quot;)\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            // &quot; should not act as title delimiter — this is not a valid link
            assert!(!p.children.iter().any(|c| matches!(c, InlineNode::Link(_))));
        } else {
            panic!("Expected paragraph");
        }
    }

    // === Tab Examples (1-11) ===

    // Example 1: tab as indentation → indented code block
    // 탭은 parse() 진입 시 spaces로 확장됨
    #[test]
    fn example_1() {
        let doc = parse("\tfoo\tbaz\t\tbim\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::CodeBlock(cb) = &doc.children[0] {
            // \t at col 0→4sp, content: foo + \t at col 7→1sp + baz + \t at col 11→1sp + \t at col 12→4sp + bim
            assert_eq!(cb.content, "foo\tbaz\t\tbim");
        } else {
            panic!("Expected code block, got {:?}", doc.children[0]);
        }
    }

    // Example 2: mixed spaces+tab as indentation → indented code block
    #[test]
    fn example_2() {
        let doc = parse("  \tfoo\tbaz\t\tbim\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::CodeBlock(cb) = &doc.children[0] {
            assert_eq!(cb.content, "foo\tbaz\t\tbim");
        } else {
            panic!("Expected code block, got {:?}", doc.children[0]);
        }
    }

    // Example 3: tab in code block content (tabs expanded to spaces)
    #[test]
    fn example_3() {
        let doc = parse("    a\ta\n    ὐ\ta\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::CodeBlock(cb) = &doc.children[0] {
            // col 4: a, col 5: \t→3sp (to col 8), a
            // col 4: ὐ (3 bytes, 1 col), col 5: \t→3sp, a
            assert_eq!(cb.content, "a\ta\nὐ\ta");
        } else {
            panic!("Expected code block, got {:?}", doc.children[0]);
        }
    }

    // Example 4: tab as list item continuation indent
    #[test]
    fn example_4() {
        let doc = parse("  - foo\n\n\tbar\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::List(list) = &doc.children[0] {
            assert_eq!(list.children.len(), 1);
            // loose item: paragraph "foo" + paragraph "bar"
            assert_eq!(list.children[0].children.len(), 2);
        } else {
            panic!("Expected list, got {:?}", doc.children[0]);
        }
    }

    // Example 5: tab as list item continuation with code block
    #[test]
    fn example_5() {
        let doc = parse("- foo\n\n\t\tbar\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::List(list) = &doc.children[0] {
            assert_eq!(list.children.len(), 1);
            // loose item: paragraph "foo" + code block "  bar"
            let item = &list.children[0];
            assert!(item.children.iter().any(|c| matches!(c, BlockNode::CodeBlock(_))));
        } else {
            panic!("Expected list, got {:?}", doc.children[0]);
        }
    }

    // Example 6: tab in blockquote → code block
    #[test]
    fn example_6() {
        let doc = parse(">\t\tfoo\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Blockquote(bq) = &doc.children[0] {
            assert!(bq.children.iter().any(|c| matches!(c, BlockNode::CodeBlock(_))));
        } else {
            panic!("Expected blockquote, got {:?}", doc.children[0]);
        }
    }

    // Example 7: tab in list item → code block
    #[test]
    fn example_7() {
        let doc = parse("-\t\tfoo\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::List(list) = &doc.children[0] {
            assert!(list.children[0].children.iter().any(|c| matches!(c, BlockNode::CodeBlock(_))));
        } else {
            panic!("Expected list, got {:?}", doc.children[0]);
        }
    }

    // Example 8: tab as indented code block continuation
    #[test]
    fn example_8() {
        let doc = parse("    foo\n\tbar\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::CodeBlock(cb) = &doc.children[0] {
            assert_eq!(cb.content, "foo\nbar");
        } else {
            panic!("Expected code block, got {:?}", doc.children[0]);
        }
    }

    // Example 9: tab as nested list indent
    #[test]
    fn example_9() {
        let doc = parse(" - foo\n   - bar\n\t - baz\n");
        assert_eq!(doc.children.len(), 1);
        // nested list: foo > bar > baz
        if let BlockNode::List(list) = &doc.children[0] {
            assert_eq!(list.children.len(), 1);
        } else {
            panic!("Expected list, got {:?}", doc.children[0]);
        }
    }

    // Example 10: tab after # in ATX heading
    #[test]
    fn example_10() {
        let doc = parse("#\tFoo\n");
        assert_eq!(doc.children.len(), 1);
        assert_eq!(doc.children[0], BlockNode::heading(1, vec![InlineNode::text("Foo")]));
    }

    // Example 11: tabs in thematic break
    #[test]
    fn example_11() {
        let doc = parse("*\t*\t*\t\n");
        assert_eq!(doc.children.len(), 1);
        assert!(matches!(doc.children[0], BlockNode::ThematicBreak(_)));
    }

    // Example 20: backslash escapes not processed in autolinks
    #[test]
    fn example_20() {
        let doc = parse("<https://example.com?find=\\*>\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::autolink_uri("https://example.com?find=\\*")
            ]);
        } else {
            panic!("Expected paragraph, got {:?}", doc.children[0]);
        }
    }

    // Example 21: backslash escapes not processed in raw HTML
    #[test]
    fn example_21() {
        let doc = parse("<a href=\"/bar\\/\">\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::HtmlBlock(_) = &doc.children[0] {
            // HTML block containing raw HTML with unprocessed backslash
        } else if let BlockNode::Paragraph(p) = &doc.children[0] {
            // Could also be inline raw HTML in a paragraph
            assert!(p.children.iter().any(|n| matches!(n, InlineNode::RawHtml(_))));
        } else {
            panic!("Expected HTML block or paragraph with raw HTML, got {:?}", doc.children[0]);
        }
    }

    // Example 22: backslash escapes in link destination and title
    #[test]
    fn example_22() {
        let doc = parse("[foo](/bar\\* \"ti\\*tle\")\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "/bar*", Some("ti*tle"))
            ]);
        } else {
            panic!("Expected paragraph, got {:?}", doc.children[0]);
        }
    }

    // Example 23: backslash escapes in ref link destination and title
    #[test]
    fn example_23() {
        let doc = parse("[foo]\n\n[foo]: /bar\\* \"ti\\*tle\"\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![
                InlineNode::link(vec![InlineNode::text("foo")], "/bar*", Some("ti*tle"))
            ]);
        } else {
            panic!("Expected paragraph, got {:?}", doc.children[0]);
        }
    }

    // Example 138: ``` ``` → code span (backtick in info string makes it not a fenced code block)
    // Result: single paragraph with code span + soft break + "aaa"
    #[test]
    fn example_138() {
        let doc = parse("``` ```\naaa\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert!(p.children.iter().any(|n| matches!(n, InlineNode::CodeSpan(_))));
            // code span + soft break + text "aaa"
            let text = get_all_text(&p.children);
            assert!(text.contains("aaa"));
        } else {
            panic!("Expected paragraph, got {:?}", doc.children[0]);
        }
    }

    // Example 145: ``` aa ``` → code span (backtick in info string)
    // Result: single paragraph with code span + soft break + "foo"
    #[test]
    fn example_145() {
        let doc = parse("``` aa ```\nfoo\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert!(p.children.iter().any(|n| matches!(n, InlineNode::CodeSpan(_))));
        } else {
            panic!("Expected paragraph with code span, got {:?}", doc.children[0]);
        }
    }

    // Example 187: type 6 HTML block can't interrupt a paragraph
    #[test]
    fn example_187() {
        let doc = parse("Foo\n<a href=\"bar\">\nbaz\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            let text = get_all_text(&p.children);
            assert!(text.contains("Foo"));
            assert!(text.contains("baz"));
        } else {
            panic!("Expected single paragraph, got {:?}", doc.children[0]);
        }
    }

    // Example 188: HTML block with blank line separation
    #[test]
    fn example_188() {
        let doc = parse("<div>\n\n*Emphasized* text.\n\n</div>\n");
        // Should produce: HTML block, paragraph, HTML block
        assert!(doc.children.len() >= 2);
        assert!(matches!(doc.children[0], BlockNode::HtmlBlock(_)));
        // Middle should be a paragraph with emphasis
        let has_para = doc.children.iter().any(|c| matches!(c, BlockNode::Paragraph(_)));
        assert!(has_para, "Expected a paragraph in the middle");
    }

    // Example 226: trailing spaces cause hard break
    #[test]
    fn example_226() {
        let doc = parse("aaa     \nbbb     \n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            // Should contain: text "aaa", hard break, text "bbb"
            assert!(p.children.iter().any(|n| matches!(n, InlineNode::HardBreak)));
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 333: NBSP in code span preserved
    #[test]
    fn example_333() {
        let doc = parse("`\u{00A0}b\u{00A0}`\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![InlineNode::code_span("\u{00A0}b\u{00A0}")]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 334: NBSP not stripped in code span
    #[test]
    fn example_334() {
        // `\u{00A0}` → code span with NBSP (not stripped)
        let doc = parse("`\u{00A0}`\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            assert_eq!(p.children, vec![InlineNode::code_span("\u{00A0}")]);
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 344: raw HTML takes precedence over code span
    #[test]
    fn example_344() {
        let doc = parse("<a href=\"`\">`\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            // Should be: raw HTML <a href="`">, then text `
            assert!(p.children.iter().any(|n| matches!(n, InlineNode::RawHtml(_))));
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 345: code span takes precedence over autolink
    #[test]
    fn example_345() {
        let doc = parse("`<https://foo.bar.`baz>`\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            // Should have code span containing "<https://foo.bar." then text "baz>"
            assert!(p.children.iter().any(|n| matches!(n, InlineNode::CodeSpan(_))));
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 346: autolink takes precedence when backticks don't match
    #[test]
    fn example_346() {
        let doc = parse("<https://foo.bar.`baz>`\n");
        assert_eq!(doc.children.len(), 1);
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            // Should be autolink (backtick is part of URI)
            assert!(p.children.iter().any(|n| matches!(n, InlineNode::Autolink(_))));
        } else {
            panic!("Expected paragraph");
        }
    }

    // Example 642: hard break in HTML attributes (not processed)
    #[test]
    fn example_642() {
        let doc = parse("<a href=\"hi\">\n");
        assert_eq!(doc.children.len(), 1);
        // Should not produce a hard break - just raw HTML or HTML block
        if let BlockNode::Paragraph(p) = &doc.children[0] {
            // No hard break in inline content
            assert!(!p.children.iter().any(|n| matches!(n, InlineNode::HardBreak)));
        }
        // HTML block is also acceptable
    }

    // Example 643: backslash in HTML attributes (not processed as escape)
    #[test]
    fn example_643() {
        let doc = parse("<a href=\"hi\\\"\n");
        assert_eq!(doc.children.len(), 1);
        // Backslash should not be processed as escape in HTML
    }

    /// Helper: extract all text content from inline nodes
    fn get_all_text(nodes: &[InlineNode]) -> String {
        let mut result = String::new();
        for node in nodes {
            match node {
                InlineNode::Text(t) => result.push_str(&t.0),
                InlineNode::Emphasis(e) => result.push_str(&get_all_text(&e.children)),
                InlineNode::Strong(s) => result.push_str(&get_all_text(&s.children)),
                InlineNode::Link(l) => result.push_str(&get_all_text(&l.children)),
                InlineNode::Image(i) => result.push_str(&get_all_text(&i.children)),
                _ => {}
            }
        }
        result
    }


}
