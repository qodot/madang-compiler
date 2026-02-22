# CommonMark 0.31.2 Example Coverage

> 업데이트: 2026-02-17 | 1117 tests passed, 0 ignored

## 점검 결과

- 전체 652개 Example이 코드에 참조됨 ✅
- 모든 Example input이 panic 없이 파싱됨 ✅
- 0개 ignored ✅

## 섹션별 현황

| 섹션 | Examples | 상태 |
|------|----------|------|
| 2.2 Tabs | 1-11 (11) | ✅ |
| 2.4 Backslash escapes | 12-24 (13) | ✅ |
| 2.5 Entity/char refs | 25-41 (17) | ✅ |
| 3.1 Precedence | 42 (1) | ✅ |
| 4.1 Thematic breaks | 43-61 (19) | ✅ |
| 4.2 ATX headings | 62-79 (18) | ✅ |
| 4.3 Setext headings | 80-106 (27) | ✅ |
| 4.4 Indented code blocks | 107-118 (12) | ✅ |
| 4.5 Fenced code blocks | 119-147 (29) | ✅ |
| 4.6 HTML blocks | 148-191 (44) | ✅ |
| 4.7 Link ref definitions | 192-218 (27) | ✅ |
| 4.8 Paragraphs | 219-226 (8) | ✅ |
| 4.9 Blank lines | 227 (1) | ✅ |
| 5.1 Block quotes | 228-252 (25) | ✅ |
| 5.2 List items | 253-300 (48) | ✅ |
| 5.3 Lists | 301-327 (27) | ✅ |
| 6.1 Code spans | 328-349 (22) | ✅ |
| 6.2 Emphasis/strong | 350-481 (132) | ✅ |
| 6.3 Links | 482-571 (90) | ✅ |
| 6.4 Images | 572-593 (22) | ✅ |
| 6.5 Autolinks | 594-612 (19) | ✅ |
| 6.6 Raw HTML | 613-632 (20) | ✅ |
| 6.7 Hard line breaks | 633-647 (15) | ✅ |
| 6.8 Soft line breaks | 648-649 (2) | ✅ |
| 6.9 Textual content | 650-652 (3) | ✅ |

## 다음 단계

- HTML renderer 구현하여 Example output과 비교 가능하게
- 현재는 AST 수준에서 검증 중 (input → AST 일치 확인)
