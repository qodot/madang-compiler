//! CommonMark 노드 타입 정의
//!
//! ## 구조
//! - `Node`: 모든 노드의 공통 trait
//! - `InlineNode`: 인라인 노드 enum (Text, 향후 Emphasis, Strong 등)
//! - `BlockNode`: 블록 노드 enum (ThematicBreak, Heading, Paragraph 등)
//!
//! ## Block 분류
//! - **Container Blocks**: DocumentNode, BlockquoteNode, ListNode, ListItemNode
//! - **Leaf Blocks**: ThematicBreakNode, HeadingNode, CodeBlockNode, ParagraphNode

use std::fmt::Debug;

/// 모든 노드의 공통 trait
pub trait Node: Debug {}

// =============================================================================
// Inline Nodes
// =============================================================================

/// 텍스트 노드
#[derive(Debug, Clone, PartialEq)]
pub struct TextNode(pub String);

impl Node for TextNode {}

impl TextNode {
    pub fn new(s: &str) -> Self {
        TextNode(s.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Code Span 노드 (인라인 코드)
#[derive(Debug, Clone, PartialEq)]
pub struct CodeSpanNode(pub String);

impl Node for CodeSpanNode {}

impl CodeSpanNode {
    pub fn new(s: &str) -> Self {
        CodeSpanNode(s.to_string())
    }
}

/// Emphasis 노드 (*em* or _em_)
#[derive(Debug, Clone, PartialEq)]
pub struct EmphasisNode {
    pub children: Vec<InlineNode>,
}

impl Node for EmphasisNode {}

impl EmphasisNode {
    pub fn new(children: Vec<InlineNode>) -> Self {
        Self { children }
    }
}

/// Strong emphasis 노드 (**strong** or __strong__)
#[derive(Debug, Clone, PartialEq)]
pub struct StrongNode {
    pub children: Vec<InlineNode>,
}

impl Node for StrongNode {}

impl StrongNode {
    pub fn new(children: Vec<InlineNode>) -> Self {
        Self { children }
    }
}

/// Autolink 노드
#[derive(Debug, Clone, PartialEq)]
pub struct AutolinkNode {
    /// 표시 텍스트 (URI 또는 이메일)
    pub label: String,
    /// 링크 대상 (URI 그대로 또는 mailto:이메일)
    pub destination: String,
}

impl Node for AutolinkNode {}

impl AutolinkNode {
    pub fn uri(uri: &str) -> Self {
        Self {
            label: uri.to_string(),
            destination: uri.to_string(),
        }
    }

    pub fn email(email: &str) -> Self {
        Self {
            label: email.to_string(),
            destination: format!("mailto:{}", email),
        }
    }
}

/// Raw HTML (인라인) 노드
#[derive(Debug, Clone, PartialEq)]
pub struct RawHtmlNode {
    pub content: String,
}

impl Node for RawHtmlNode {}

impl RawHtmlNode {
    pub fn new(content: &str) -> Self {
        Self {
            content: content.to_string(),
        }
    }
}

/// 인라인 노드 enum
#[derive(Debug, Clone, PartialEq)]
pub enum InlineNode {
    Text(TextNode),
    CodeSpan(CodeSpanNode),
    Autolink(AutolinkNode),
    RawHtml(RawHtmlNode),
    Emphasis(EmphasisNode),
    Strong(StrongNode),
    HardBreak,
    SoftBreak,
    // 향후: Link, Image 등
}

impl Node for InlineNode {}

impl InlineNode {
    #[cfg(test)]
    pub fn text(s: &str) -> Self {
        InlineNode::Text(TextNode::new(s))
    }

    #[cfg(test)]
    pub fn code_span(s: &str) -> Self {
        InlineNode::CodeSpan(CodeSpanNode::new(s))
    }

    #[cfg(test)]
    pub fn emphasis(children: Vec<InlineNode>) -> Self {
        InlineNode::Emphasis(EmphasisNode::new(children))
    }

    #[cfg(test)]
    pub fn strong(children: Vec<InlineNode>) -> Self {
        InlineNode::Strong(StrongNode::new(children))
    }

    #[cfg(test)]
    pub fn autolink_uri(uri: &str) -> Self {
        InlineNode::Autolink(AutolinkNode::uri(uri))
    }

    #[cfg(test)]
    pub fn autolink_email(email: &str) -> Self {
        InlineNode::Autolink(AutolinkNode::email(email))
    }

    #[cfg(test)]
    pub fn raw_html(content: &str) -> Self {
        InlineNode::RawHtml(RawHtmlNode::new(content))
    }
}

// =============================================================================
// Leaf Block Nodes
// =============================================================================

/// Thematic Break 노드 (수평선)
#[derive(Debug, Clone, PartialEq)]
pub struct ThematicBreakNode;

impl Node for ThematicBreakNode {}

/// ATX Heading 노드
#[derive(Debug, Clone, PartialEq)]
pub struct HeadingNode {
    pub level: u8,
    pub children: Vec<InlineNode>,
}

impl Node for HeadingNode {}

impl HeadingNode {
    pub fn new(level: u8, children: Vec<InlineNode>) -> Self {
        Self { level, children }
    }
}

/// Code Block 노드 (fenced 또는 indented)
#[derive(Debug, Clone, PartialEq)]
pub struct CodeBlockNode {
    pub info: Option<String>,
    pub content: String,
}

impl Node for CodeBlockNode {}

impl CodeBlockNode {
    pub fn new(info: Option<String>, content: String) -> Self {
        Self { info, content }
    }

    pub fn fenced(info: Option<&str>, content: &str) -> Self {
        Self {
            info: info.map(|s| s.to_string()),
            content: content.to_string(),
        }
    }

    pub fn indented(content: &str) -> Self {
        Self {
            info: None,
            content: content.to_string(),
        }
    }
}

/// HTML Block 노드 (raw HTML)
#[derive(Debug, Clone, PartialEq)]
pub struct HtmlBlockNode {
    pub content: String,
}

impl Node for HtmlBlockNode {}

impl HtmlBlockNode {
    pub fn new(content: &str) -> Self {
        Self {
            content: content.to_string(),
        }
    }
}

/// Paragraph 노드
#[derive(Debug, Clone, PartialEq)]
pub struct ParagraphNode {
    pub children: Vec<InlineNode>,
}

impl Node for ParagraphNode {}

impl ParagraphNode {
    pub fn new(children: Vec<InlineNode>) -> Self {
        Self { children }
    }
}

// =============================================================================
// Container Block Nodes
// =============================================================================

/// Blockquote 노드
#[derive(Debug, Clone, PartialEq)]
pub struct BlockquoteNode {
    pub children: Vec<BlockNode>,
}

impl Node for BlockquoteNode {}

impl BlockquoteNode {
    pub fn new(children: Vec<BlockNode>) -> Self {
        Self { children }
    }
}

/// 리스트 타입
#[derive(Debug, Clone, PartialEq)]
pub enum ListType {
    /// Bullet 리스트 (-, +, *)
    Bullet,
    /// Ordered 리스트 (숫자 + 구분자)
    Ordered {
        /// 구분자 ('.' 또는 ')')
        delimiter: char,
    },
}

/// List 노드
#[derive(Debug, Clone, PartialEq)]
pub struct ListNode {
    pub list_type: ListType,
    pub start: usize,
    pub tight: bool,
    pub children: Vec<ListItemNode>,
}

impl Node for ListNode {}

impl ListNode {
    pub fn new(list_type: ListType, start: usize, tight: bool, children: Vec<ListItemNode>) -> Self {
        Self { list_type, start, tight, children }
    }

    pub fn bullet(tight: bool, children: Vec<ListItemNode>) -> Self {
        Self {
            list_type: ListType::Bullet,
            start: 1,
            tight,
            children,
        }
    }

    pub fn ordered(delimiter: char, start: usize, tight: bool, children: Vec<ListItemNode>) -> Self {
        Self {
            list_type: ListType::Ordered { delimiter },
            start,
            tight,
            children,
        }
    }
}

/// List Item 노드
#[derive(Debug, Clone, PartialEq)]
pub struct ListItemNode {
    pub children: Vec<BlockNode>,
}

impl Node for ListItemNode {}

impl ListItemNode {
    pub fn new(children: Vec<BlockNode>) -> Self {
        Self { children }
    }
}

/// Document 노드 (최상위 컨테이너)
#[derive(Debug, Clone, PartialEq)]
pub struct DocumentNode {
    pub children: Vec<BlockNode>,
}

impl Node for DocumentNode {}

impl DocumentNode {
    pub fn new(children: Vec<BlockNode>) -> Self {
        Self { children }
    }
}

// =============================================================================
// Block Node Enum
// =============================================================================

/// 블록 노드 enum
#[derive(Debug, Clone, PartialEq)]
pub enum BlockNode {
    ThematicBreak(ThematicBreakNode),
    Heading(HeadingNode),
    CodeBlock(CodeBlockNode),
    HtmlBlock(HtmlBlockNode),
    Paragraph(ParagraphNode),
    Blockquote(BlockquoteNode),
    List(ListNode),
    ListItem(ListItemNode),
}

impl Node for BlockNode {}

impl BlockNode {
    // 테스트용 빌더 메서드
    #[cfg(test)]
    pub fn thematic_break() -> Self {
        BlockNode::ThematicBreak(ThematicBreakNode)
    }

    #[cfg(test)]
    pub fn heading(level: u8, children: Vec<InlineNode>) -> Self {
        BlockNode::Heading(HeadingNode::new(level, children))
    }

    #[cfg(test)]
    pub fn html_block(content: &str) -> Self {
        BlockNode::HtmlBlock(HtmlBlockNode::new(content))
    }

    #[cfg(test)]
    pub fn code_block(info: Option<&str>, content: &str) -> Self {
        BlockNode::CodeBlock(CodeBlockNode::fenced(info, content))
    }

    #[cfg(test)]
    pub fn paragraph(children: Vec<InlineNode>) -> Self {
        BlockNode::Paragraph(ParagraphNode::new(children))
    }

    #[cfg(test)]
    pub fn blockquote(children: Vec<BlockNode>) -> Self {
        BlockNode::Blockquote(BlockquoteNode::new(children))
    }

    #[cfg(test)]
    pub fn bullet_list(tight: bool, children: Vec<ListItemNode>) -> Self {
        BlockNode::List(ListNode::bullet(tight, children))
    }

    #[cfg(test)]
    pub fn ordered_list(delimiter: char, start: usize, tight: bool, children: Vec<ListItemNode>) -> Self {
        BlockNode::List(ListNode::ordered(delimiter, start, tight, children))
    }

    #[cfg(test)]
    pub fn list_item(children: Vec<BlockNode>) -> Self {
        BlockNode::ListItem(ListItemNode::new(children))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_thematic_break() {
        let node = ThematicBreakNode;
        assert_eq!(format!("{:?}", node), "ThematicBreakNode");
    }

    #[test]
    fn create_heading() {
        let node = HeadingNode::new(2, vec![InlineNode::text("Hello")]);
        assert_eq!(node.level, 2);
        assert_eq!(node.children.len(), 1);
    }

    #[test]
    fn create_paragraph() {
        let node = ParagraphNode::new(vec![InlineNode::text("Hello world")]);
        assert_eq!(node.children.len(), 1);
    }

    #[test]
    fn create_document() {
        let doc = DocumentNode::new(vec![
            BlockNode::thematic_break(),
            BlockNode::heading(1, vec![InlineNode::text("Title")]),
        ]);
        assert_eq!(doc.children.len(), 2);
    }

    #[test]
    fn create_list() {
        let list = ListNode::bullet(true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("item 1")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("item 2")])]),
        ]);
        assert_eq!(list.children.len(), 2);
        assert!(list.tight);
    }
}
