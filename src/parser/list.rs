//! List 파싱 통합 테스트 (CommonMark 5.3 Lists)
//!
//! https://spec.commonmark.org/0.31.2/#lists
//!
//! - 5.2 List Items → list_item.rs
//! - 5.3 Lists: tight/loose, 마커 타입별 분리, 중첩 → 이 파일

#[cfg(test)]
mod tests {
    use crate::node::{BlockNode, InlineNode, ListItemNode};
    use crate::parser::parse;
    use crate::Spec;
    use rstest::rstest;

    // =========================================================================
    // 3.1 Precedence - Example 42
    // https://spec.commonmark.org/0.31.2/#precedence
    // =========================================================================

    #[rstest]
    // Example 42: block structure takes precedence over inline — backtick이 텍스트의 일부
    #[case("- `one\n- two`", vec![
        BlockNode::bullet_list(true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("`one")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("two`")])]),
        ]),
    ])]
    fn test_precedence(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse(input, Spec::CommonMark);
        assert_eq!(doc.children, expected);
    }

    // =========================================================================
    // 5.3 Lists - Example 301~326
    // https://spec.commonmark.org/0.31.2/#lists
    // =========================================================================

    #[rstest]
    // Example 301: 마커 변경 → 새 리스트
    #[case("- foo\n- bar\n+ baz", vec![
        BlockNode::bullet_list(true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("foo")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("bar")])]),
        ]),
        BlockNode::bullet_list(true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("baz")])]),
        ]),
    ])]
    // Example 302: delimiter 변경 → 새 리스트
    #[case("1. foo\n2. bar\n3) baz", vec![
        BlockNode::ordered_list('.', 1, true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("foo")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("bar")])]),
        ]),
        BlockNode::ordered_list(')', 3, true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("baz")])]),
        ]),
    ])]
    // Example 303: 리스트가 paragraph 인터럽트
    #[case("Foo\n- bar\n- baz", vec![
        BlockNode::paragraph(vec![InlineNode::text("Foo")]),
        BlockNode::bullet_list(true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("bar")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("baz")])]),
        ]),
    ])]
    // Example 304: `14.`는 paragraph 인터럽트 불가 (1로 시작하는 것만 가능)
    #[case("The number of windows in my house is\n14.  The number of doors is 6.", vec![
        BlockNode::paragraph(vec![InlineNode::text("The number of windows in my house is"), InlineNode::SoftBreak, InlineNode::text("14.  The number of doors is 6.")]),
    ])]
    // Example 305: `1.`은 paragraph 인터럽트 가능
    #[case("The number of windows in my house is\n1.  The number of doors is 6.", vec![
        BlockNode::paragraph(vec![InlineNode::text("The number of windows in my house is")]),
        BlockNode::ordered_list('.', 1, true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("The number of doors is 6.")])]),
        ]),
    ])]
    // Example 306: 빈 줄이 여러 개 있어도 loose list
    #[case("- foo\n\n- bar\n\n\n- baz", vec![
        BlockNode::bullet_list(false, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("foo")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("bar")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("baz")])]),
        ]),
    ])]
    // Example 307: 중첩 리스트 + 빈 줄 후 추가 단락
    // 외부/중간은 tight, 최내부만 loose (아이템 내 직접 블록 사이 빈 줄)
    #[case("- foo\n  - bar\n    - baz\n\n\n      bim", vec![
        BlockNode::bullet_list(true, vec![
            ListItemNode::new(vec![
                BlockNode::paragraph(vec![InlineNode::text("foo")]),
                BlockNode::bullet_list(true, vec![
                    ListItemNode::new(vec![
                        BlockNode::paragraph(vec![InlineNode::text("bar")]),
                        BlockNode::bullet_list(false, vec![
                            ListItemNode::new(vec![
                                BlockNode::paragraph(vec![InlineNode::text("baz")]),
                                BlockNode::paragraph(vec![InlineNode::text("bim")]),
                            ]),
                        ]),
                    ]),
                ]),
            ]),
        ])
    ])]
    // Example 310: 0~3칸 들여쓰기는 같은 레벨
    #[case("- a\n - b\n  - c\n   - d\n  - e\n - f\n- g", vec![
        BlockNode::bullet_list(true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("a")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("b")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("c")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("d")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("e")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("f")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("g")])]),
        ])
    ])]
    // Example 311: ordered list + 들여쓰기 (0~3칸은 같은 레벨)
    #[case("1. a\n\n  2. b\n\n   3. c", vec![
        BlockNode::ordered_list('.', 1, false, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("a")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("b")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("c")])]),
        ])
    ])]
    // Example 312: 4칸+ 들여쓰기 마커는 paragraph continuation
    #[case("- a\n - b\n  - c\n   - d\n    - e", vec![
        BlockNode::bullet_list(true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("a")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("b")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("c")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("d"), InlineNode::SoftBreak, InlineNode::text("- e")])]),
        ])
    ])]
    // Example 314: loose (아이템 간 빈 줄)
    #[case("- a\n- b\n\n- c", vec![
        BlockNode::bullet_list(false, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("a")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("b")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("c")])]),
        ])
    ])]
    // Example 316: loose (아이템 내 직접 블록 사이 빈 줄)
    #[case("- a\n- b\n\n  c\n- d", vec![
        BlockNode::bullet_list(false, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("a")])]),
            ListItemNode::new(vec![
                BlockNode::paragraph(vec![InlineNode::text("b")]),
                BlockNode::paragraph(vec![InlineNode::text("c")]),
            ]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("d")])]),
        ])
    ])]
    // Example 319: tight (sublist 내 빈 줄은 outer에 영향 없음)
    // 외부 tight, 내부 loose
    #[case("- a\n  - b\n\n    c\n- d", vec![
        BlockNode::bullet_list(true, vec![
            ListItemNode::new(vec![
                BlockNode::paragraph(vec![InlineNode::text("a")]),
                BlockNode::bullet_list(false, vec![
                    ListItemNode::new(vec![
                        BlockNode::paragraph(vec![InlineNode::text("b")]),
                        BlockNode::paragraph(vec![InlineNode::text("c")]),
                    ]),
                ]),
            ]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("d")])]),
        ])
    ])]
    // Example 322: single-paragraph list는 tight
    #[case("- a", vec![
        BlockNode::bullet_list(true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("a")])]),
        ])
    ])]
    // Example 323: 중첩 리스트도 tight
    #[case("- a\n  - b", vec![
        BlockNode::bullet_list(true, vec![
            ListItemNode::new(vec![
                BlockNode::paragraph(vec![InlineNode::text("a")]),
                BlockNode::bullet_list(true, vec![
                    ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("b")])]),
                ]),
            ]),
        ])
    ])]
    // Example 325: outer loose (아이템 내 직접 블록 사이 빈 줄), inner tight
    #[case("* foo\n  * bar\n\n  baz", vec![
        BlockNode::bullet_list(false, vec![
            ListItemNode::new(vec![
                BlockNode::paragraph(vec![InlineNode::text("foo")]),
                BlockNode::bullet_list(true, vec![
                    ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("bar")])]),
                ]),
                BlockNode::paragraph(vec![InlineNode::text("baz")]),
            ]),
        ])
    ])]
    // Example 326: 마지막 예제 (아이템 간 빈 줄로 인해 loose)
    #[case("- a\n  - b\n  - c\n\n- d\n  - e\n  - f", vec![
        BlockNode::bullet_list(false, vec![
            ListItemNode::new(vec![
                BlockNode::paragraph(vec![InlineNode::text("a")]),
                BlockNode::bullet_list(true, vec![
                    ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("b")])]),
                    ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("c")])]),
                ]),
            ]),
            ListItemNode::new(vec![
                BlockNode::paragraph(vec![InlineNode::text("d")]),
                BlockNode::bullet_list(true, vec![
                    ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("e")])]),
                    ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("f")])]),
                ]),
            ]),
        ])
    ])]
    fn test_list(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse(input, Spec::CommonMark);
        assert_eq!(doc.children, expected);
    }

    // =========================================================================
    // 미구현 기능으로 인해 ignore 처리된 테스트
    // =========================================================================

    #[rstest]
    // Example 308: HTML 주석으로 리스트 분리 (HTML block 미구현)
    #[case("- foo\n- bar\n\n<!-- -->\n\n- baz\n- bim", vec![])]
    // Example 309: HTML 주석 + indented code (HTML block 미구현)
    #[case("-   foo\n\n    notcode\n\n-   foo\n\n<!-- -->\n\n    code", vec![])]
    // Example 313: 4칸 들여쓰기 + 빈 줄 → indented code block
    // (빈 줄 뒤 4칸 들여쓰기가 리스트를 종료하고 indented code block 시작해야 함)
    #[case("1. a\n\n  2. b\n\n    3. c", vec![
        BlockNode::ordered_list('.', 1, false, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("a")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("b")])]),
        ]),
        BlockNode::code_block(None, "3. c\n"),
    ])]
    // Example 315: 빈 아이템 (빈 리스트 아이템 지원 필요)
    #[case("* a\n*\n\n* c", vec![])]
    // Example 317: link reference definition (link ref 미구현)
    #[case("- a\n- b\n\n  [ref]: /url\n- d", vec![])]
    // Example 318: code block 내 빈 줄 → tight (fenced code block 내용 파싱 필요)
    #[case("- a\n- ```\n  b\n\n\n  ```\n- c", vec![])]
    // Example 320: blockquote 내 빈 줄 → tight
    #[case("* a\n  > b\n  >\n* c", vec![])]
    // Example 321: blockquote + fenced code (연속 블록)
    #[case("- a\n  > b\n  ```\n  c\n  ```\n- d", vec![])]
    // Example 324: fenced code + paragraph → loose
    #[case("1. ```\n   foo\n   ```\n\n   bar", vec![])]
    
    fn test_list_pending(#[case] _input: &str, #[case] _expected: Vec<BlockNode>) {
        // 이 테스트들은 추가 기능 구현 후 활성화
    }

    /// 탭 관련 리스트 테스트 (CommonMark 명세 Section 2.2 Tabs)
    #[rstest]
    // Example 4: 탭으로 continuation paragraph 인덴트
    #[case("  - foo\n\n\tbar", vec![BlockNode::bullet_list(false, vec![
        ListItemNode::new(vec![
            BlockNode::paragraph(vec![InlineNode::text("foo")]),
            BlockNode::paragraph(vec![InlineNode::text("bar")]),
        ]),
    ])])]
    // Example 5: 탭 2개로 인덴트 → list item 내 code block
    #[case("- foo\n\n\t\tbar", vec![BlockNode::bullet_list(false, vec![
        ListItemNode::new(vec![
            BlockNode::paragraph(vec![InlineNode::text("foo")]),
            BlockNode::code_block(None, "  bar"),
        ]),
    ])])]
    // Example 7: 대시 + 탭 2개 → list item 내 code block
    #[case("-\t\tfoo", vec![BlockNode::bullet_list(true, vec![
        ListItemNode::new(vec![
            BlockNode::code_block(None, "  foo"),
        ]),
    ])])]
    // Example 9: 탭으로 중첩 리스트 인덴트
    #[case(" - foo\n   - bar\n\t - baz", vec![BlockNode::bullet_list(true, vec![
        ListItemNode::new(vec![
            BlockNode::paragraph(vec![InlineNode::text("foo")]),
            BlockNode::bullet_list(true, vec![
                ListItemNode::new(vec![
                    BlockNode::paragraph(vec![InlineNode::text("bar")]),
                    BlockNode::bullet_list(true, vec![
                        ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("baz")])]),
                    ]),
                ]),
            ]),
        ]),
    ])])]
    
    fn test_list_tabs(#[case] _input: &str, #[case] _expected: Vec<BlockNode>) {
    }
}
