//! https://spec.commonmark.org/0.31.2/#fenced-code-blocks

use crate::node::{BlockNode, CodeBlockNode};
use super::helpers::{count_leading_char, remove_indent};
use super::inline::entity::resolve_entities;
use super::inline::backslash_escape::is_ascii_punctuation;

#[derive(Debug, Clone, PartialEq)]
pub struct CodeBlockFencedStart {
    /// 펜스 문자 ('`' 또는 '~')
    pub fence_char: char,
    /// 펜스 길이 (최소 3)
    pub fence_len: usize,
    /// info string (언어 등)
    pub info: Option<String>,
    /// 여는 펜스의 들여쓰기
    pub indent: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CodeBlockFencedOk {
    /// 시작 줄 (여는 펜스)
    Start(CodeBlockFencedStart),
    /// 내용 줄 (들여쓰기 제거됨)
    Content(String),
    /// 종료 줄 (닫는 펜스)
    End,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CodeBlockFencedErr {
    /// 4칸 이상 들여쓰기 (indented code block으로 해석됨)
    CodeBlockIndented,
    /// 펜스 문자 없음 (```, ~~~가 아님)
    NoFence,
}

pub fn parse(line: &str, start: Option<&CodeBlockFencedStart>) -> Result<CodeBlockFencedOk, CodeBlockFencedErr> {
    match start {
        None => parse_start(line),
        Some(s) => Ok(parse_continue(line, s)),
    }
}

fn parse_start(line: &str) -> Result<CodeBlockFencedOk, CodeBlockFencedErr> {
    let indent = count_leading_char(line, ' ');

    // 4칸 이상 들여쓰기는 indented code block
    if indent > 3 {
        return Err(CodeBlockFencedErr::CodeBlockIndented);
    }

    let after_indent = &line[indent..];

    // 펜스 문자와 길이 확인
    let (fence_char, fence_len) = if after_indent.starts_with("```") {
        ('`', count_leading_char(after_indent, '`'))
    } else if after_indent.starts_with("~~~") {
        ('~', count_leading_char(after_indent, '~'))
    } else {
        return Err(CodeBlockFencedErr::NoFence);
    };

    // info string 추출
    let info = {
        let after_fence = &after_indent[fence_len..];
        let trimmed = after_fence.trim();
        if trimmed.is_empty() {
            None
        } else {
            // backtick fence의 info string에 backtick이 포함되면 fenced code block 아님
            if fence_char == '`' && trimmed.contains('`') {
                return Err(CodeBlockFencedErr::NoFence);
            }
            Some(trimmed.to_string())
        }
    };

    Ok(CodeBlockFencedOk::Start(CodeBlockFencedStart {
        fence_char,
        fence_len,
        info,
        indent,
    }))
}

fn parse_continue(line: &str, start: &CodeBlockFencedStart) -> CodeBlockFencedOk {
    let indent = count_leading_char(line, ' ');

    // 4칸 이상 들여쓰기는 내용
    if indent > 3 {
        return CodeBlockFencedOk::Content(remove_indent(line, start.indent).to_string());
    }

    let after_indent = &line[indent..];
    let closing_len = count_leading_char(after_indent, start.fence_char);

    // 닫는 펜스 조건: 같은 문자, 충분한 길이, 뒤에 텍스트 없음
    if closing_len >= start.fence_len && after_indent[closing_len..].trim().is_empty() {
        return CodeBlockFencedOk::End;
    }

    CodeBlockFencedOk::Content(remove_indent(line, start.indent).to_string())
}

/// Backslash escape를 풀어준다 (info string용)
fn resolve_backslash_escapes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&next) = chars.peek() {
                if is_ascii_punctuation(next) {
                    result.push(next);
                    chars.next();
                    continue;
                }
            }
        }
        result.push(c);
    }
    result
}

pub fn finalize(start: CodeBlockFencedStart, content: Vec<String>) -> BlockNode {
    let content_str = content.join("\n");
    let info = start.info.map(|s| resolve_backslash_escapes(&resolve_entities(&s)));
    BlockNode::CodeBlock(CodeBlockNode::new(info, content_str))
}

pub fn parse_text(text: &str) -> Option<BlockNode> {
    let lines: Vec<&str> = text.lines().collect();

    if lines.is_empty() {
        return None;
    }

    // 여는 펜스 확인
    let first_line = lines[0];
    let start = match parse(first_line, None) {
        Ok(CodeBlockFencedOk::Start(s)) => s,
        _ => return None,
    };

    // 닫는 펜스 찾기
    let has_closing_fence = if lines.len() >= 2 {
        let last_line = lines[lines.len() - 1];
        matches!(parse(last_line, Some(&start)), Ok(CodeBlockFencedOk::End))
    } else {
        false
    };

    // 내용 추출 (들여쓰기 제거)
    let content_lines: Vec<&str> = if has_closing_fence {
        lines[1..lines.len() - 1].to_vec()
    } else {
        lines[1..].to_vec()
    };

    let content: Vec<String> = content_lines
        .iter()
        .map(|line| remove_indent(line, start.indent).to_string())
        .collect();

    Some(finalize(start, content))
}

#[cfg(test)]
mod tests {
    use crate::node::{BlockNode, InlineNode};
    use crate::parser::parse;
    use rstest::rstest;

    #[rstest]
    // Example 119: 백틱 펜스
    #[case("```\n<\n >\n```", vec![BlockNode::code_block(None, "<\n >")])]
    // Example 120: 틸드 펜스
    #[case("~~~\n<\n >\n~~~", vec![BlockNode::code_block(None, "<\n >")])]
    // Example 122: 백틱으로 시작, 내부에 틸드, 백틱으로 끝
    #[case("```\naaa\n~~~\n```", vec![BlockNode::code_block(None, "aaa\n~~~")])]
    // Example 123: 틸드로 시작, 내부에 백틱, 틸드로 끝
    #[case("~~~\naaa\n```\n~~~", vec![BlockNode::code_block(None, "aaa\n```")])]
    // Example 124: 닫는 펜스가 더 길어도 OK (짧은 펜스는 내용)
    #[case("````\naaa\n```\n``````", vec![BlockNode::code_block(None, "aaa\n```")])]
    // Example 125: 틸드 버전
    #[case("~~~~\naaa\n~~~\n~~~~", vec![BlockNode::code_block(None, "aaa\n~~~")])]
    // Example 126: 닫히지 않은 코드 블록 (빈 내용)
    #[case("```", vec![BlockNode::code_block(None, "")])]
    // Example 127: 닫히지 않은 코드 블록 (내용 있음)
    #[case("`````\n\n```\naaa", vec![BlockNode::code_block(None, "\n```\naaa")])]
    // Example 129: 빈 줄과 공백만 있는 내용
    #[case("```\n\n  \n```", vec![BlockNode::code_block(None, "\n  ")])]
    // Example 130: 빈 코드 블록
    #[case("```\n```", vec![BlockNode::code_block(None, "")])]
    // Example 131: 들여쓰기 1칸 (내용에서 1칸 제거)
    #[case(" ```\n aaa\naaa\n```", vec![BlockNode::code_block(None, "aaa\naaa")])]
    // Example 132: 들여쓰기 2칸 (내용에서 2칸 제거)
    #[case("  ```\naaa\n  aaa\naaa\n  ```", vec![BlockNode::code_block(None, "aaa\naaa\naaa")])]
    // Example 133: 들여쓰기 3칸 (내용에서 3칸 제거)
    #[case("   ```\n   aaa\n    aaa\n  aaa\n   ```", vec![BlockNode::code_block(None, "aaa\n aaa\naaa")])]
    // Example 134: 4칸 들여쓰기는 indented code block
    #[case("    ```\n    aaa\n    ```", vec![BlockNode::code_block(None, "```\naaa\n```")])]
    // Example 135: 닫는 펜스 들여쓰기 다름 (0-3칸은 OK)
    #[case("```\naaa\n  ```", vec![BlockNode::code_block(None, "aaa")])]
    // Example 136: 닫는 펜스 들여쓰기 다름 (0-3칸은 OK)
    #[case("   ```\naaa\n  ```", vec![BlockNode::code_block(None, "aaa")])]
    // Example 137: 닫는 펜스 4칸 들여쓰기는 내용
    #[case("```\naaa\n    ```", vec![BlockNode::code_block(None, "aaa\n    ```")])]
    // Example 139: 닫는 펜스 뒤 공백+문자는 내용
    #[case("~~~~~~\naaa\n~~~ ~~", vec![BlockNode::code_block(None, "aaa\n~~~ ~~")])]
    // Example 140: paragraph 사이의 코드 블록
    #[case("foo\n```\nbar\n```\nbaz", vec![BlockNode::paragraph(vec![InlineNode::text("foo")]), BlockNode::code_block(None, "bar"), BlockNode::paragraph(vec![InlineNode::text("baz")])])]
    // Example 142: info string (ruby)
    #[case("```ruby\ndef foo(x)\n  return 3\nend\n```", vec![BlockNode::code_block(Some("ruby"), "def foo(x)\n  return 3\nend")])]
    // Example 143: info string 앞뒤 공백 제거
    #[case("~~~~    ruby startline=3 $%@#$\ndef foo(x)\n  return 3\nend\n~~~~~~~", vec![BlockNode::code_block(Some("ruby startline=3 $%@#$"), "def foo(x)\n  return 3\nend")])]
    // Example 144: info string 특수 문자
    #[case("````;\n````", vec![BlockNode::code_block(Some(";"), "")])]
    // Example 146: 틸드 펜스 info string에 백틱 허용
    #[case("~~~ aa ``` ~~~\nfoo\n~~~", vec![BlockNode::code_block(Some("aa ``` ~~~"), "foo")])]
    // Example 147: 닫는 펜스에 info string은 내용
    #[case("```\n``` aaa\n```", vec![BlockNode::code_block(None, "``` aaa")])]
    fn test_fenced_code_block(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse(input);
        assert_eq!(doc.children, expected);
    }

    #[rstest]
    // Example 128: blockquote 내부 닫히지 않은 코드 블록
    #[case("> ```\n> aaa\n\nbbb", vec![BlockNode::blockquote(vec![BlockNode::code_block(None, "aaa")]), BlockNode::paragraph(vec![InlineNode::text("bbb")])])]
    
    fn test_fenced_code_block_pending(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse(input);
        assert_eq!(doc.children, expected);
    }

    #[rstest]
    // Example 141: setext heading + code block + heading
    #[case("foo\n---\n~~~\nbar\n~~~\n# baz", vec![BlockNode::heading(2, vec![InlineNode::text("foo")]), BlockNode::code_block(None, "bar"), BlockNode::heading(1, vec![InlineNode::text("baz")])])]
    fn test_fenced_code_block_with_setext(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse(input);
        assert_eq!(doc.children, expected);
    }

    // TODO: Example 121, 138, 145는 inline code 구현 시 추가
}
