# 파서 설계 리팩터링 계획

> 2026-02-17

## 발견된 설계 결함

### 1. ParsingContext enum 일관성 없음 (우선순위: 높음)
- `None`, `Paragraph`, `List`, `HtmlBlock`은 struct 래핑
- `Blockquote`, `CodeBlockFenced`, `CodeBlockIndented`는 인라인 필드
- blockquote/code block 로직이 mod.rs의 free function으로 흩어짐

### 2. block 감지 로직 중복 (우선순위: 높음)
- `NoneContext::parse`, `ParagraphContext::parse`, `is_paragraph_open`, `is_block_structure`에 같은 block 감지 체인 반복
- 우선순위 변경 시 여러 곳 수정 필요

### 3. 가짜 불변 헬퍼 (우선순위: 중간)
- `push_node`, `push_string`, `extend_nodes` — Vec을 move로 받아 mut 후 반환
- 불변 스타일이 아니라 불필요한 래핑

### 4. inline parser 거대 함수 (우선순위: 중간)
- `parse_inlines_with_refs` 280줄+ monolithic
- mutable state가 하나의 스코프에 전부 존재

### 5. blockquote finalize 이중 파싱 전략 (우선순위: 중간)
- `has_lazy = contents.iter().any(|c| c.contains('\n'))` — fragile heuristic
- `parse_block_simple` vs `parse` 이중 구조

### 6. Node trait 미사용 (우선순위: 낮음)
- `pub trait Node: Debug {}` — 어디서도 trait으로 호출 안 됨

### 7. `#[cfg(test)]` 빌더 메서드 (우선순위: 낮음)
- renderer에서도 필요한 생성자가 test 전용으로 묶여 있음

## 작업 순서 (PR별)

### PR 1: CodeBlockFencedContext 추출
- `ParsingContext::CodeBlockFenced { start, content }` → `CodeBlockFencedContext` struct
- `process_line_in_code_block` → `CodeBlockFencedContext::parse`
- base: `refactor/html-block-context`

### PR 2: CodeBlockIndentedContext 추출
- `ParsingContext::CodeBlockIndented { ... }` → `CodeBlockIndentedContext` struct
- `process_line_in_code_block_indented` → `CodeBlockIndentedContext::parse`
- base: PR 1 브랜치

### PR 3: BlockquoteContext 추출
- `ParsingContext::Blockquote { pending_lines }` → `BlockquoteContext` struct
- `process_line_in_blockquote` + `is_paragraph_open` + lazy continuation → `BlockquoteContext::parse`
- base: PR 2 브랜치

### PR 4: block 감지 통일
- `detect_block_start(line) -> Option<BlockStartKind>` 함수 추출
- `NoneContext`, `ParagraphContext`, `BlockquoteContext` 등에서 공통 사용
- base: PR 3 브랜치

### PR 5: 가짜 불변 헬퍼 제거
- `push_node`, `push_string`, `extend_nodes` 제거
- 직접 `.push()`, `.extend()` 사용
- base: PR 4 브랜치

### PR 6: inline parser struct화
- `InlineParser` struct 추출
- `parse_inlines_with_refs` → `InlineParser::parse`
- 각 문자 처리를 메서드로 분리
- base: PR 5 브랜치
