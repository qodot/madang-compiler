# 마당 컴파일러 — CommonMark 0.31.2 명세 구현 현황

> 생성일: 2025-02-15 | 테스트 결과: **680 passed, 0 failed, 37 ignored**

## 📊 요약

| 항목 | 수량 | 비율 |
|------|------|------|
| 전체 Example | 652 | 100% |
| ✅ 구현 완료 | 298 | 45.7% |
| ⏸️ Ignored | 19 | 2.9% |
| ❌ 미구현 | 335 | 51.4% |

```
구현 ████████████████████░░░░░░░░░░░░░░░░░░░░ 45.7%
```

## 📋 섹션별 상세

### 1. Tabs (Example 1–11) — 1/11 ✅

- ✅ 구현: 10
- ⏸️ Ignored (10): 1–9, 11 — 탭 처리 미지원

### 2. Backslash escapes (Example 12–24) — 5/13 ✅

- ✅ 구현: 12–15, 17
- ❌ 미구현 (8): 16, 18–24

### 3. Entity and numeric character references (Example 25–41) — 0/17 ❌

- ❌ 전체 미구현

### 4. Precedence (Example 42) — 1/1 ✅

- ✅ 완료

### 5. Thematic breaks (Example 43–61) — 14/19 ✅

- ✅ 구현: 43–56
- ❌ 미구현 (5): 57–61

### 6. ATX headings (Example 62–79) — 15/18 ✅

- ✅ 구현: 62–64, 67–75, 77–79
- ❌ 미구현 (3): 65, 66, 76

### 7. Setext headings (Example 80–106) — 22/27 ✅

- ✅ 구현: 80–98, 100–102
- ⏸️ Ignored (2): 99, 103 — inline 파싱 미구현
- ❌ 미구현 (3): 104–106

### 8. Indented code blocks (Example 107–118) — 12/12 ✅✅

- ✅ 완전 구현!

### 9. Fenced code blocks (Example 119–147) — 24/29 ✅

- ✅ 구현: 119–120, 122–127, 129–137, 139–140, 142–144, 146–147
- ⏸️ Ignored (3): 121, 128, 141 — blockquote 내 코드 블록 등
- ❌ 미구현 (2): 138, 145

### 10. HTML blocks (Example 148–191) — 42/44 ✅

- ✅ 구현: 148–186, 189–191
- ❌ 미구현 (2): 187, 188

### 11. Link reference definitions (Example 192–218) — 0/27 ❌

- ❌ 전체 미구현

### 12. Paragraphs (Example 219–226) — 7/8 ✅

- ✅ 구현: 219–225, 227
- ❌ 미구현 (1): 226

### 13. Blank lines (Example 227) — 1/1 ✅

- ✅ 완료

### 14. Block quotes (Example 228–252) — 23/25 ✅

- ✅ 구현: 228–248, 250–251
- ⏸️ Ignored (2): 249, 252 — blockquote 내 indented code block 미지원

### 15. List items (Example 253–300) — 33/48 ✅

- ✅ 구현: 253–285
- ❌ 미구현 (15): 286–300

### 16. Lists (Example 301–326) — 24/26 ✅

- ✅ 구현: 301–320, 322–323, 325–326
- ⏸️ Ignored (2): 321, 324 — blockquote + fenced code 등

### 17. Inlines (Example 327) — 0/1 ❌

- ❌ 미구현

### 18. Code spans (Example 328–349) — 17/22 ✅

- ✅ 구현: 328–332, 335–343, 347–349
- ❌ 미구현 (5): 333, 334, 344–346

### 19. Emphasis and strong emphasis (Example 350–481) — 8/132 🔴

- ✅ 구현: 350, 351, 355, 357, 358, 360, 393, 410
- ❌ 미구현 (124): 352–354, 356, 359, 361–392, 394–409, 411–481

### 20. Links (Example 482–571) — 0/90 ❌

- ❌ 전체 미구현

### 21. Images (Example 572–593) — 0/22 ❌

- ❌ 전체 미구현

### 22. Autolinks (Example 594–612) — 18/19 ✅

- ✅ 구현: 594–611
- ❌ 미구현 (1): 612

### 23. Raw HTML (Example 613–632) — 20/20 ✅✅

- ✅ 완전 구현!

### 24. Hard line breaks (Example 633–647) — 9/15 ✅

- ✅ 구현: 633–637, 640, 641, 644, 645
- ❌ 미구현 (6): 638, 639, 642, 643, 646, 647

### 25. Soft line breaks (Example 648–649) — 2/2 ✅✅

- ✅ 완전 구현!

### 26. Textual content (Example 650–652) — 0/3 ❌

- ❌ 전체 미구현

## 🏆 완전 구현 섹션

- ✅ Indented code blocks (12/12)
- ✅ Raw HTML (20/20)
- ✅ Soft line breaks (2/2)
- ✅ Precedence (1/1)
- ✅ Blank lines (1/1)

## 🔥 주요 미구현 영역

| 섹션 | 미구현 | 우선순위 |
|------|--------|---------|
| **Emphasis/Strong** | 124개 | 높음 — inline 핵심 |
| **Links** | 90개 | 높음 — inline 핵심 |
| **Link ref definitions** | 27개 | 높음 — Links 의존 |
| **Images** | 22개 | 중간 — Links 구현 후 |
| **Entity/char refs** | 17개 | 중간 |
| **List items (후반)** | 15개 | 중간 |

## ⏸️ Ignored 테스트 사유

| 사유 | 수량 |
|------|------|
| 탭 처리 미지원 | 10 |
| blockquote 내 indented code block | 2 |
| blockquote 내 코드 블록 | 1 |
| inline 파싱 미구현 | 2 |
| container block 내부 HTML block | 2 |
| blockquote + fenced code 등 | 2 |

## 📈 다음 마일스톤 제안

1. **Emphasis/Strong emphasis** 구현 → +124 examples (45.7% → 64.7%)
2. **Links** 구현 → +90 examples (64.7% → 78.5%)
3. **Images** 구현 → +22 examples (78.5% → 81.9%)
4. **Link reference definitions** → +27 examples (81.9% → 86.0%)
5. 나머지 소규모 섹션 정리 → 90%+
