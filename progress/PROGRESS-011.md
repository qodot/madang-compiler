# 학습 진행 기록 011

## 날짜
2026-02-15

## 학습 주제
블록 Example 점검 + 인라인 파서 기초 구현

## 완료한 작업

### 1. 블록 레벨 Example 전수 점검
- CommonMark 0.31.2 명세 대비 모든 블록 Example 커버리지 점검
- 누락된 Example 분류: 바로 추가 가능 / inline 의존 / 탭 처리 / Link ref def
- 바로 추가 가능한 14개 Example 테스트 추가 (Ex 42, 45, 115, 184-186, 189-191, 227, 233, 252)
- Tabs Example 1-11 테스트 추가 (10개 ignore, 1개 통과)
- 커밋: `ea35f14`

### 2. 인라인 파서 모듈 뼈대
- `src/parser/inline/mod.rs` 생성 — `parse_inlines()` 진입점
- paragraph, heading, setext heading, list context에서 `parse_inlines` 사용
- 커밋: `c318803`

### 3. Code Span 구현
- `code_span.rs` 헬퍼: `parse_code_span`, `normalize_content`, `count_backticks`
- `parse_inlines`에 연결, `InlineNode::CodeSpan` variant 추가
- 매칭 안 된 백틱 시퀀스는 통째로 텍스트 처리
- Example 328-349 테스트
- 커밋: `9b34f2e`, `1286830`

### 4. Backslash Escape 구현
- `backslash_escape.rs` 헬퍼: `is_ascii_punctuation`, `try_escape`
- `parse_inlines`에서 `\` + ASCII 구두점 → 이스케이프
- code span이 먼저 처리되므로 code span 내 `\`는 리터럴 유지
- Example 12-17 테스트
- 커밋: `5bce708`, `04edd5d`

### 5. Hard/Soft Line Break 구현
- `line_break.rs` 헬퍼: trailing spaces 카운트/제거, leading spaces 스킵
- `\` + `\n` → HardBreak, trailing spaces 2+ + `\n` → HardBreak, 그 외 `\n` → SoftBreak
- 기존 블록 테스트 34개 expected 값 업데이트 (`text("foo\nbar")` → SoftBreak 분리)
- Example 633-649 테스트
- 커밋: `a621d8b`

### 6. Autolink 구현
- `autolink.rs` 헬퍼: URI autolink (scheme 2-32글자) + Email autolink
- `AutolinkNode` (label + destination) 추가
- `parse_inlines`에서 `<` → autolink 시도 → 실패 시 텍스트
- Example 594-612 테스트
- 커밋: `a11e46d`, `530b0fc`

### 7. Raw HTML (inline) — 진행 중
- 서브에이전트에서 구현 중

## 학습 내용

### 인라인 파싱의 설계 원칙
1. **문자 단위 스캔**: `parse_inlines`가 한 문자씩 보면서 특수 문자에 반응
2. **우선순위 순서**: `\` (escape) → `` ` `` (code span) → `<` (autolink/raw HTML) → `\n` (line break) → 텍스트
3. **헬퍼 분리**: 각 인라인 요소는 별도 모듈의 헬퍼 함수로 파싱, `parse_inlines`는 조합만
4. **연속 텍스트 합치기**: `push_text_char` 헬퍼로 연속된 텍스트를 하나의 TextNode로

### Code Span 주의점
- 매칭 안 된 백틱 시퀀스는 **통째로** 텍스트 (한 글자씩 X)
- code span 내부에서는 다른 인라인 구문 무시 (escape, line break 등)

### Line Break와 기존 테스트
- SoftBreak 도입 시 기존 `text("foo\nbar")` 테스트 전부 업데이트 필요
- 큰 변경이지만 명세에 맞는 올바른 방향

## 커밋 히스토리
```
530b0fc feat(inline): parse_inlines에 autolink 연결
a11e46d feat(inline): autolink 헬퍼 함수 추가
a621d8b feat(inline): hard/soft line break 파싱 추가
04edd5d feat(inline): parse_inlines에 backslash escape 연결
5bce708 feat(inline): backslash escape 헬퍼 함수 추가
1286830 feat(inline): parse_inlines에 code span 파싱 연결
9b34f2e feat(inline): code span 헬퍼 함수 추가
c318803 refactor: inline 파서 모듈 뼈대 추가
ea35f14 test: 누락된 CommonMark block-level Example 테스트 추가
```

## 다음 학습 시 시작점
- Raw HTML (inline) 구현 완료 확인
- Emphasis/Strong emphasis 구현 (delimiter run 알고리즘 — 132 Examples)
- Links/Images 구현
- 또는 학습 종료
