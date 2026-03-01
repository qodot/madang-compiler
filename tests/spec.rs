//! CommonMark 0.31.2 / GFM extension 통합 테스트
//!
//! spec.json (652 examples) + gfm-spec.json (24 extension examples)

use madang_compiler::{parse, render, Spec};
use serde::Deserialize;

#[derive(Deserialize)]
struct SpecExample {
    example: usize,
    markdown: String,
    html: String,
    section: String,
}

fn report_failures(failures: &[(usize, String, String, String)], total: usize) {
    if failures.is_empty() {
        return;
    }
    let passed = total - failures.len();
    let mut msg = format!(
        "\n{}/{} examples passed, {} failed:\n\n",
        passed, total, failures.len()
    );
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
    msg.push_str("Failed examples: ");
    let nums: Vec<String> = failures.iter().map(|(n, _, _, _)| n.to_string()).collect();
    msg.push_str(&nums.join(", "));
    msg.push('\n');
    panic!("{}", msg);
}

fn run_spec(json_str: &str, spec: Spec) {
    let examples: Vec<SpecExample> =
        serde_json::from_str(json_str).expect("Failed to parse spec json");
    let mut failures: Vec<(usize, String, String, String)> = Vec::new();

    for ex in &examples {
        let doc = parse(&ex.markdown, spec);
        let actual = render(&doc);
        if actual != ex.html {
            failures.push((ex.example, ex.section.clone(), ex.html.clone(), actual));
        }
    }

    report_failures(&failures, examples.len());
}

/// CommonMark 0.31.2: 652 examples
#[test]
fn commonmark_spec() {
    run_spec(include_str!("fixtures/spec.json"), Spec::CommonMark);
}

/// GFM extensions: 24 examples (Tables, Task list, Strikethrough, Autolinks ext, Disallowed HTML)
#[test]
fn gfm_spec() {
    run_spec(include_str!("fixtures/gfm-spec.json"), Spec::Gfm);
}
