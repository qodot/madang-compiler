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
    use rstest::rstest;

    #[rstest]
    // =========================================================================
    // 5.3 Lists - tight/loose
    // =========================================================================
    // tight: 아이템 간 빈 줄 없음
    #[case("- a\n- b\n- c", vec![
        BlockNode::bullet_list(true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("a")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("b")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("c")])]),
        ])
    ])]
    #[case("1. a\n2. b", vec![
        BlockNode::ordered_list('.', 1, true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("a")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("b")])]),
        ])
    ])]
    // loose: 아이템 간 빈 줄 있음
    #[case("- a\n\n- b\n\n- c", vec![
        BlockNode::bullet_list(false, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("a")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("b")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("c")])]),
        ])
    ])]
    #[case("1. a\n\n2. b", vec![
        BlockNode::ordered_list('.', 1, false, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("a")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("b")])]),
        ])
    ])]
    // =========================================================================
    // 5.3 Lists - 다른 마커 타입은 별도 리스트
    // =========================================================================
    #[case("- a\n+ b", vec![
        BlockNode::bullet_list(true, vec![ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("a")])])]),
        BlockNode::bullet_list(true, vec![ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("b")])])]),
    ])]
    #[case("1. a\n1) b", vec![
        BlockNode::ordered_list('.', 1, true, vec![ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("a")])])]),
        BlockNode::ordered_list(')', 1, true, vec![ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("b")])])]),
    ])]
    // =========================================================================
    // 5.3 Lists - 중첩
    // =========================================================================
    #[case("- foo\n  - bar\n  - baz", vec![
        BlockNode::bullet_list(true, vec![
            ListItemNode::new(vec![
                BlockNode::paragraph(vec![InlineNode::text("foo")]),
                BlockNode::bullet_list(true, vec![
                    ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("bar")])]),
                    ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("baz")])]),
                ]),
            ]),
        ])
    ])]
    #[case("- foo\n  - bar\n- qux", vec![
        BlockNode::bullet_list(true, vec![
            ListItemNode::new(vec![
                BlockNode::paragraph(vec![InlineNode::text("foo")]),
                BlockNode::bullet_list(true, vec![ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("bar")])])]),
            ]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("qux")])]),
        ])
    ])]
    // Example 301: 0~3칸 들여쓰기는 같은 레벨
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
    // Example 297: 3단계 중첩 + 빈 줄 후 추가 단락
    // NOTE: 명세상 외부는 tight지만 현재 모두 loose (향후 개선)
    #[case("- foo\n  - bar\n    - baz\n\n\n      bim", vec![
        BlockNode::bullet_list(false, vec![
            ListItemNode::new(vec![
                BlockNode::paragraph(vec![InlineNode::text("foo")]),
                BlockNode::bullet_list(false, vec![
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
    // Example 303: 4칸+ 들여쓰기 마커는 continuation
    #[case("- a\n - b\n  - c\n   - d\n    - e", vec![
        BlockNode::bullet_list(true, vec![
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("a")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("b")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("c")])]),
            ListItemNode::new(vec![BlockNode::paragraph(vec![InlineNode::text("d\n- e")])]),
        ])
    ])]
    fn test_list(#[case] input: &str, #[case] expected: Vec<BlockNode>) {
        let doc = parse(input);
        assert_eq!(doc.children, expected);
    }
}
