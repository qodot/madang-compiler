# Progress 011 — Entity References, Ignored Tests, Tab Processing

## 날짜
2026-02-16 ~ 2026-02-17

## 주요 작업

### 1. Entity/Character References (Examples 25-41)
- `src/parser/inline/entity.rs` 신규 생성
- `try_parse_entity()`: named (`&amp;`), decimal (`&#35;`), hex (`&#xcab;`) 세 종류 파싱
- `resolve_entities()`: 문자열 전체 entity 치환 헬퍼
- `entities` 크레이트 추가 (HTML5 named entity 조회)
- `parse_inlines`에서 `&` 만나면 entity 파싱
- entity로 생성된 문자는 마크다운 구문으로 재해석 안 됨 (스펙 준수)

### 2. Entity 치환 확장
- link destination/title: `resolve_entities()` 적용 (Ex 32)
- ref def destination/title: `resolve_entities()` 적용 (Ex 33)
- fenced code info string: `resolve_entities()` 적용 (Ex 34)

### 3. Ignored Tests 대거 해결
- **Ex 493**: angle dest `<foo\>` — backslash escape로 `>` 안 닫힘, 텍스트 처리 (테스트 기대값 수정)
- **Ex 575**: image 안 link 가능하도록 bracket 비활성화 로직 수정 — 외부 image bracket은 유지
- **Ex 195, 202**: ref def의 percent-encoding은 렌더러 책임이므로 테스트 기대값 수정
- **Ex 210**: title 뒤 extra text → title 되돌리고 title 없이 ref def 인정
- **Ex 549**: full reference link에서 `parse_label` 재사용 (escaped bracket 처리)
- **Ex 215, 216**: setext heading에서 ref def 추출 — HeadingNode에 `raw_text` 추가, Pass 2에서 heading도 처리
- **Ex 520**: 복잡한 nested image — bracket 수정 덕에 자동 통과

### 4. Link Reference Definition 테스트 (서브 에이전트)
- Examples 193-218, 527-571, 573-591 테스트 추가
- 26개 unignore 성공

### 5. Tab Processing (Examples 1-11)
- `expand_tabs()`: parse() 진입 시 모든 탭을 4칸 탭 스톱 기준 spaces로 확장
- `remove_leading_indent()`: 탭 스톱 고려한 indent 제거 헬퍼
- `calculate_indent()`: 이미 탭 지원 (기존)
- `parse_block_simple`에 indented code block 감지 추가
- 코드 블록 content의 탭도 spaces로 확장 (시각적 동일, 구현 단순화)

### 6. 누락 Example 추가 (서브 에이전트 진행 중)
- 16개 Examples: 20-23, 138, 145, 187, 188, 226, 333-334, 344-346, 642-643

## 테스트 현황
- **시작**: 895 passed, 54 ignored
- **종료**: 1043+ passed, 36 ignored (서브 에이전트 결과 대기)

## 핵심 결정
- **탭 확장 방식**: 모든 탭을 parse() 진입 시 spaces로 확장 (column tracking 대신)
  - 장점: 구현 단순, blockquote/list 안에서도 자연스럽게 작동
  - 단점: 코드 블록 content에서 탭이 spaces로 변환됨

- **HeadingNode에 raw_text 추가**: setext heading에서 ref def 추출을 위해
  - `PartialEq` 수동 구현으로 raw_text를 비교에서 제외

- **Image bracket 비활성화**: link 완성 시 외부 link bracket만 비활성화, image bracket은 유지

## 남은 과제
- Lazy continuation (Ex 290, 291) — block parser 구조 변경 필요
- List item code block indent (Ex 7) — content indent 계산
- HTML renderer 구현 (percent-encoding 등)
