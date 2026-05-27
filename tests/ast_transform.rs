//! Tests for AST transformation: inline_control_flow_function, extract_exec_output_labels.

use graphy::utils::{inline_control_flow_function, extract_exec_output_labels};
use std::collections::HashMap;

// ===========================================================================
// extract_exec_output_labels
// ===========================================================================

#[test]
fn extract_labels_from_branch() {
    let source = r#"
        fn branch(condition: bool) {
            if condition {
                exec_output!("True");
            } else {
                exec_output!("False");
            }
        }
    "#;

    let labels = extract_exec_output_labels(source).unwrap();
    assert_eq!(labels, vec!["True", "False"]);
}

#[test]
fn extract_labels_single() {
    let source = r#"
        fn do_thing() {
            exec_output!("Done");
        }
    "#;

    let labels = extract_exec_output_labels(source).unwrap();
    assert_eq!(labels, vec!["Done"]);
}

#[test]
fn extract_labels_none() {
    let source = r#"
        fn pure_function(a: i64, b: i64) -> i64 {
            a + b
        }
    "#;

    let labels = extract_exec_output_labels(source).unwrap();
    assert!(labels.is_empty());
}

#[test]
fn extract_labels_multiple_in_sequence() {
    let source = r#"
        fn multi_step() {
            exec_output!("Step1");
            exec_output!("Step2");
            exec_output!("Step3");
        }
    "#;

    let labels = extract_exec_output_labels(source).unwrap();
    assert_eq!(labels.len(), 3);
    assert_eq!(labels[0], "Step1");
    assert_eq!(labels[1], "Step2");
    assert_eq!(labels[2], "Step3");
}

#[test]
fn extract_labels_nested_in_loop() {
    let source = r#"
        fn loop_with_output(count: i64) {
            for i in 0..count {
                exec_output!("body");
            }
            exec_output!("completed");
        }
    "#;

    let labels = extract_exec_output_labels(source).unwrap();
    assert!(labels.contains(&"body".to_string()));
    assert!(labels.contains(&"completed".to_string()));
}

#[test]
fn extract_labels_invalid_source_returns_error() {
    let source = "this is not valid rust";
    let result = extract_exec_output_labels(source);
    assert!(result.is_err());
}

#[test]
fn extract_labels_empty_function() {
    let source = r#"
        fn empty() {}
    "#;

    let labels = extract_exec_output_labels(source).unwrap();
    assert!(labels.is_empty());
}

// ===========================================================================
// inline_control_flow_function - Basic
// ===========================================================================

#[test]
fn inline_branch_with_replacements() {
    let source = r#"
        fn branch(condition: bool) {
            if condition {
                exec_output!("True");
            } else {
                exec_output!("False");
            }
        }
    "#;

    let mut exec_replacements = HashMap::new();
    exec_replacements.insert("True".to_string(), "println!(\"yes\");".to_string());
    exec_replacements.insert("False".to_string(), "println!(\"no\");".to_string());

    let param_substitutions = HashMap::new();

    let result = inline_control_flow_function(source, exec_replacements, param_substitutions);
    assert!(result.is_ok());
    let code = result.unwrap();
    assert!(code.contains("println"));
}

#[test]
fn inline_with_param_substitution() {
    let source = r#"
        fn branch(condition: bool) {
            if condition {
                exec_output!("True");
            } else {
                exec_output!("False");
            }
        }
    "#;

    let mut exec_replacements = HashMap::new();
    exec_replacements.insert("True".to_string(), "println!(\"yes\");".to_string());
    exec_replacements.insert("False".to_string(), "println!(\"no\");".to_string());

    let mut param_substitutions = HashMap::new();
    param_substitutions.insert("condition".to_string(), "x > 5".to_string());

    let result = inline_control_flow_function(source, exec_replacements, param_substitutions);
    assert!(result.is_ok());
    let code = result.unwrap();
    assert!(code.contains("x > 5"));
}

#[test]
fn inline_empty_replacements_erases_exec_output_macros() {
    // Regression: unmatched exec_output!() must NOT appear in output.
    // Old behaviour left them verbatim → Rust compiler error "cannot find macro exec_output".
    let source = r#"
        fn noop(condition: bool) {
            if condition {
                exec_output!("True");
            } else {
                exec_output!("False");
            }
        }
    "#;

    let exec_replacements = HashMap::new(); // nothing connected
    let param_substitutions = HashMap::new();

    let result = inline_control_flow_function(source, exec_replacements, param_substitutions);
    assert!(result.is_ok());

    let code = result.unwrap();
    // Must NOT contain bare exec_output! — that would be a compile error in the emitted Rust.
    assert!(
        !code.contains("exec_output"),
        "unmatched exec_output! survived into output: {code}"
    );
}

#[test]
fn inline_invalid_source_returns_error() {
    let result = inline_control_flow_function(
        "not valid rust code",
        HashMap::new(),
        HashMap::new(),
    );
    assert!(result.is_err());
}

#[test]
fn inline_no_function_body_returns_error() {
    // A valid statement but not a function
    let result = inline_control_flow_function(
        "let x = 5;",
        HashMap::new(),
        HashMap::new(),
    );
    assert!(result.is_err());
}

// ===========================================================================
// inline_control_flow_function - Multiple parameters
// ===========================================================================

#[test]
fn inline_multiple_param_substitutions() {
    let source = r#"
        fn compute(a: i64, b: i64) {
            let result = a + b;
            exec_output!("Done");
        }
    "#;

    let mut exec_replacements = HashMap::new();
    exec_replacements.insert("Done".to_string(), "println!(\"{}\", result);".to_string());

    let mut param_substitutions = HashMap::new();
    param_substitutions.insert("a".to_string(), "42".to_string());
    param_substitutions.insert("b".to_string(), "58".to_string());

    let result = inline_control_flow_function(source, exec_replacements, param_substitutions);
    assert!(result.is_ok());
    let code = result.unwrap();
    assert!(code.contains("42"));
    assert!(code.contains("58"));
}

// ===========================================================================
// inline_control_flow_function - Complex control flow
// ===========================================================================

#[test]
fn inline_nested_if_else() {
    let source = r#"
        fn multi_branch(a: bool, b: bool) {
            if a {
                if b {
                    exec_output!("AB");
                } else {
                    exec_output!("AnotB");
                }
            } else {
                exec_output!("notA");
            }
        }
    "#;

    let labels = extract_exec_output_labels(source).unwrap();
    assert_eq!(labels.len(), 3);

    let mut exec_replacements = HashMap::new();
    exec_replacements.insert("AB".to_string(), "println!(\"AB\");".to_string());
    exec_replacements.insert("AnotB".to_string(), "println!(\"A!B\");".to_string());
    exec_replacements.insert("notA".to_string(), "println!(\"!A\");".to_string());

    let result = inline_control_flow_function(source, exec_replacements, HashMap::new());
    assert!(result.is_ok());
}

// ===========================================================================
// REGRESSION: exec_output! substitution bugs (fixed in cd8961d)
// ===========================================================================

/// A Sequence-like node with four ordered outputs — the canonical blueprint pattern.
const SEQUENCE_SOURCE: &str = r#"
    fn sequence() {
        exec_output!("Then0");
        exec_output!("Then1");
        exec_output!("Then2");
        exec_output!("Then3");
    }
"#;

/// Regression: single-statement replacement must appear in output.
#[test]
fn regression_sequence_single_stmt_per_output() {
    let mut exec_replacements = HashMap::new();
    exec_replacements.insert("Then0".to_string(), "foo();".to_string());
    exec_replacements.insert("Then1".to_string(), "bar();".to_string());
    exec_replacements.insert("Then2".to_string(), "baz();".to_string());
    exec_replacements.insert("Then3".to_string(), "qux();".to_string());

    let code = inline_control_flow_function(SEQUENCE_SOURCE, exec_replacements, HashMap::new())
        .expect("inline failed");

    assert!(code.contains("foo"), "Then0 stmt missing: {code}");
    assert!(code.contains("bar"), "Then1 stmt missing: {code}");
    assert!(code.contains("baz"), "Then2 stmt missing: {code}");
    assert!(code.contains("qux"), "Then3 stmt missing: {code}");
    assert!(!code.contains("exec_output"), "bare exec_output! in output: {code}");
}

/// Regression: multi-statement replacement — ALL statements must appear, not just the first.
///
/// The old bug: `first_stmt.clone()` dropped every statement after the first, so a
/// chain of four nodes following a Sequence output became a single call in the Rust output.
#[test]
fn regression_sequence_multi_stmt_chain_all_emitted() {
    let multi_stmt = "
        print_number(1.0);
        print_number(2.0);
        println(String::new());
        print_formatted(String::new(), String::new());
    ";

    let mut exec_replacements = HashMap::new();
    exec_replacements.insert("Then0".to_string(), multi_stmt.to_string());
    exec_replacements.insert("Then1".to_string(), "print_bool(false);".to_string());
    exec_replacements.insert("Then2".to_string(), String::new()); // empty
    exec_replacements.insert("Then3".to_string(), String::new()); // empty

    let code = inline_control_flow_function(SEQUENCE_SOURCE, exec_replacements, HashMap::new())
        .expect("inline failed");

    // quote! emits token streams with spaces between tokens (e.g. `print_number (1.0)`),
    // so check by function name only rather than the full call site.
    //
    // All four calls in Then0's chain must be present.
    // print_number appears for both 1.0 and 2.0, println, and print_formatted.
    let print_number_count = code.matches("print_number").count();
    assert!(
        print_number_count >= 2,
        "Then0 should have 2× print_number, got {print_number_count}: {code}"
    );
    assert!(code.contains("println"),         "3rd stmt of Then0 (println) missing: {code}");
    assert!(code.contains("print_formatted"), "4th stmt of Then0 (print_formatted) missing: {code}");

    // Then1's single stmt must also be present.
    assert!(code.contains("print_bool"),      "Then1 stmt missing: {code}");

    // No bare exec_output! should survive into the output.
    assert!(!code.contains("exec_output"),    "bare exec_output! in output: {code}");
}

/// Regression: unconnected outputs (no replacement key) must produce empty code,
/// NOT leave `exec_output!("Then2")` verbatim — which causes a Rust compile error.
#[test]
fn regression_unconnected_outputs_erased_not_emitted() {
    // Only Then0 is connected; Then1–Then3 have no replacement at all.
    let mut exec_replacements = HashMap::new();
    exec_replacements.insert("Then0".to_string(), "connected();".to_string());
    // Then1, Then2, Then3 intentionally absent from the map.

    let code = inline_control_flow_function(SEQUENCE_SOURCE, exec_replacements, HashMap::new())
        .expect("inline failed");

    assert!(code.contains("connected"),    "Then0 stmt missing: {code}");
    assert!(!code.contains("exec_output"), "bare exec_output! survived: {code}");
}

/// Regression: empty-string replacement (explicitly empty) must also not emit exec_output!.
#[test]
fn regression_empty_string_replacement_erased() {
    let mut exec_replacements = HashMap::new();
    exec_replacements.insert("Then0".to_string(), "do_work();".to_string());
    exec_replacements.insert("Then1".to_string(), String::new()); // explicit empty
    exec_replacements.insert("Then2".to_string(), String::new());
    exec_replacements.insert("Then3".to_string(), String::new());

    let code = inline_control_flow_function(SEQUENCE_SOURCE, exec_replacements, HashMap::new())
        .expect("inline failed");

    assert!(code.contains("do_work"),      "Then0 stmt missing: {code}");
    assert!(!code.contains("exec_output"), "bare exec_output! survived: {code}");
}

/// Regression: fully unconnected Sequence (nothing wired up) produces
/// valid output with no exec_output! calls.
#[test]
fn regression_fully_unconnected_sequence_no_exec_output_in_output() {
    let code = inline_control_flow_function(SEQUENCE_SOURCE, HashMap::new(), HashMap::new())
        .expect("inline failed");

    assert!(
        !code.contains("exec_output"),
        "bare exec_output! survived in fully-unconnected sequence: {code}"
    );
}

/// Regression: the exact 6-node blueprint from the bug report.
///
/// Graph: BeginPlay → print_number → print_number → println → print_formatted
///        → Sequence { Then0 → print_bool, Then1 → print_number, Then2 ∅, Then3 ∅ }
///
/// Before the fix, the Rust output only had 4 calls (the linear chain) and either
/// missed the Sequence branches or emitted bare exec_output! causing a compile error.
#[test]
fn regression_six_node_blueprint_all_calls_emitted() {
    // Simulate the exec chains the code generator passes in.
    // In the real graph the Sequence node is reached AFTER the 4-node linear chain;
    // for this test we only exercise the Sequence inlining itself.
    let then0_chain = "
        print_bool(false);
    ";
    let then1_chain = "
        print_number(0.0f64);
    ";

    let mut exec_replacements = HashMap::new();
    exec_replacements.insert("Then0".to_string(), then0_chain.to_string());
    exec_replacements.insert("Then1".to_string(), then1_chain.to_string());
    // Then2, Then3: not in the map (simulates unconnected outputs)

    let code = inline_control_flow_function(SEQUENCE_SOURCE, exec_replacements, HashMap::new())
        .expect("inline failed");

    // Both Sequence branches must be emitted.
    assert!(code.contains("print_bool"),   "Then0 (print_bool) missing: {code}");
    assert!(code.contains("print_number"), "Then1 (print_number) missing: {code}");

    // No bare exec_output! in output.
    assert!(!code.contains("exec_output"), "bare exec_output! in output: {code}");
}

/// Regression: a Sequence where Then0 has a LONG chain (5 nodes).
/// Every node in the chain must appear — not just the first.
#[test]
fn regression_long_chain_after_sequence_output_fully_emitted() {
    let long_chain = "
        step_one();
        step_two();
        step_three();
        step_four();
        step_five();
    ";

    let mut exec_replacements = HashMap::new();
    exec_replacements.insert("Then0".to_string(), long_chain.to_string());
    exec_replacements.insert("Then1".to_string(), String::new());
    exec_replacements.insert("Then2".to_string(), String::new());
    exec_replacements.insert("Then3".to_string(), String::new());

    let code = inline_control_flow_function(SEQUENCE_SOURCE, exec_replacements, HashMap::new())
        .expect("inline failed");

    for i in 1..=5 {
        let name = format!("step_{}", ["one", "two", "three", "four", "five"][i - 1]);
        assert!(code.contains(&name), "step {i} missing from long chain: {code}");
    }
    assert!(!code.contains("exec_output"), "bare exec_output! in output: {code}");
}

/// Regression: multi-statement chains on MULTIPLE outputs must all be present.
#[test]
fn regression_multi_stmt_on_multiple_outputs() {
    let mut exec_replacements = HashMap::new();
    exec_replacements.insert("Then0".to_string(), "a1(); a2(); a3();".to_string());
    exec_replacements.insert("Then1".to_string(), "b1(); b2();".to_string());
    exec_replacements.insert("Then2".to_string(), "c1();".to_string());
    exec_replacements.insert("Then3".to_string(), String::new());

    let code = inline_control_flow_function(SEQUENCE_SOURCE, exec_replacements, HashMap::new())
        .expect("inline failed");

    for sym in &["a1", "a2", "a3", "b1", "b2", "c1"] {
        assert!(code.contains(sym), "{sym} missing from multi-output multi-stmt: {code}");
    }
    assert!(!code.contains("exec_output"), "bare exec_output! in output: {code}");
}
