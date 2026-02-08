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
fn inline_empty_replacements() {
    let source = r#"
        fn noop(condition: bool) {
            if condition {
                exec_output!("True");
            } else {
                exec_output!("False");
            }
        }
    "#;

    let exec_replacements = HashMap::new();
    let param_substitutions = HashMap::new();

    // Should succeed even without replacements (labels just stay as-is or get ignored)
    let result = inline_control_flow_function(source, exec_replacements, param_substitutions);
    assert!(result.is_ok());
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
