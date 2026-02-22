//! CommonMark 0.31.2 spec 통합 테스트
//!
//! spec.json의 652개 example을 md → parse → render → expected html과 비교

use madang_compiler::{parse, render};
use serde::Deserialize;

#[derive(Deserialize)]
struct SpecExample {
    example: usize,
    markdown: String,
    html: String,
    section: String,
}

fn load_spec_examples() -> Vec<SpecExample> {
    let json = include_str!("fixtures/spec.json");
    serde_json::from_str(json).expect("Failed to parse spec.json")
}

/// 모든 652개 example을 한 번에 실행하고, 실패한 것들을 모아서 보고
#[test]
fn commonmark_spec() {
    let examples = load_spec_examples();
    let mut failures: Vec<(usize, String, String, String)> = Vec::new();

    for ex in &examples {
        let doc = parse(&ex.markdown);
        let actual = render(&doc);
        if actual != ex.html {
            failures.push((ex.example, ex.section.clone(), ex.html.clone(), actual));
        }
    }

    if !failures.is_empty() {
        let total = examples.len();
        let passed = total - failures.len();
        let mut msg = format!(
            "\n{}/{} examples passed, {} failed:\n\n",
            passed, total, failures.len()
        );
        // 처음 20개만 상세 출력
        for (i, (num, section, expected, actual)) in failures.iter().enumerate() {
            if i >= 20 {
                msg.push_str(&format!("  ... and {} more\n", failures.len() - 20));
                break;
            }
            msg.push_str(&format!(
                "  Example {} ({}):\n    expected: {:?}\n    actual:   {:?}\n\n",
                num, section, expected, actual
            ));
        }
        // 실패한 example 번호만 나열
        msg.push_str("Failed examples: ");
        let nums: Vec<String> = failures.iter().map(|(n, _, _, _)| n.to_string()).collect();
        msg.push_str(&nums.join(", "));
        msg.push('\n');
        panic!("{}", msg);
    }
}
