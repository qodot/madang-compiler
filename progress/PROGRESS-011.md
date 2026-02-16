# PROGRESS-011: Emphasis 재작성 + Links + Images 구현

**날짜**: 2026-02-15 ~ 2026-02-16
**테스트**: 825 → 895 passed (+70), 39 → 54 ignored

## 완료 항목

### Emphasis process_emphasis 재작성
- cmark 참조 구현의 알고리즘으로 전면 재작성
- 기존 2단계(find_matches → build_tree) → 1단계(노드 직접 조작)
- delimiter run에서 부분 소비 + 재사용 지원
- 21개 실패 테스트 전부 수정
- merge_adjacent_text 재귀 처리 (emphasis/strong/link children)

### Backslash Escape 수정
- delimiter 뒤 backslash escape가 delimiter 텍스트에 합쳐지는 버그 수정
- force_new_text 플래그 체크 추가

### Inline Links 구현
- `LinkNode` (children, destination, title)
- `link.rs` — parse_link_destination (angle bracket, bare, title)
- bracket stack으로 `[` / `]` 처리
- emphasis + link 조합: delimiter를 bracket 기준으로 분할
- Example 482-526: 42/45 통과

### Inline Images 구현
- `ImageNode` (children, destination, title) — Link 구조 재활용
- `![` bracket 처리, image bracket은 outer bracket 비활성화 안 함
- Example 572-593: 9/22 통과 (나머지 link ref def 의존)

### 기타 소규모 Example 추가
- Thematic breaks 57-61 (thematic break > list item 우선순위 수정)
- ATX headings 65, 66, 76
- Setext headings 99, 103-106
- List items 286-300 (13통과, 2 ignored)
- Misc examples: backslash, code span, autolink, hard break, textual content

## 남은 작업
- **Link Reference Definitions** (27개 Example) — 블록 + 인라인 양쪽 필요
- **Reference Links/Images** (45+13 Example) — link ref def 구현 후
- **Entity/Char References** (17개)
- angle dest escape (`<foo\>`) 1개
- lazy continuation 2개
- Example 520 (복잡한 중첩 image)
- Example 575 (image 내 link → bracket 비활성화 문제)

## 커밋 로그
- `41b9a37` thematic break examples 57-61
- `cd5b1ad` ATX heading examples 65, 66, 76
- `f6fa331` setext heading examples 99, 103-106
- `f3115f7` misc spec examples
- `335897a` list items examples 286-300
- `1d9017e` emphasis examples 350-481
- `6967566` backslash escape fix
- `9aedd10` emphasis process_emphasis rewrite
- `11583c5` inline link parsing
- `e3c7204` inline link examples 482-526
- `b84ee64` emphasis within link children fix
- `947481c` inline image parsing
- `0cfa993` image examples 572-593
