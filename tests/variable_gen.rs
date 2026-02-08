//! Tests for VariableNameGenerator, sanitize_name, and get_default_value_for_type.

use graphy::utils::{VariableNameGenerator, sanitize_name, get_default_value_for_type};

// ===========================================================================
// VariableNameGenerator - Basic
// ===========================================================================

#[test]
fn vargen_new_generates_for_node() {
    let mut gen = VariableNameGenerator::new();
    let name = gen.generate_for_node("add_1");
    assert_eq!(name, "node_add_1_result");
}

#[test]
fn vargen_default_trait() {
    let mut gen = VariableNameGenerator::default();
    let name = gen.generate_for_node("test");
    assert_eq!(name, "node_test_result");
}

#[test]
fn vargen_unique_names_for_same_node() {
    let mut gen = VariableNameGenerator::new();
    let name1 = gen.generate_for_node("add");
    let name2 = gen.generate_for_node("add");
    assert_ne!(name1, name2);
    assert!(name2.starts_with("node_add_result_"));
}

#[test]
fn vargen_many_duplicates_all_unique() {
    let mut gen = VariableNameGenerator::new();
    let mut names: Vec<String> = Vec::new();
    for _ in 0..20 {
        names.push(gen.generate_for_node("x"));
    }
    // All should be unique
    let unique: std::collections::HashSet<_> = names.iter().collect();
    assert_eq!(unique.len(), 20);
}

#[test]
fn vargen_generate_temp() {
    let mut gen = VariableNameGenerator::new();
    let t1 = gen.generate_temp();
    let t2 = gen.generate_temp();
    assert!(t1.starts_with("temp_"));
    assert!(t2.starts_with("temp_"));
    assert_ne!(t1, t2);
}

#[test]
fn vargen_temp_names_unique_across_many() {
    let mut gen = VariableNameGenerator::new();
    let mut temps: Vec<String> = Vec::new();
    for _ in 0..50 {
        temps.push(gen.generate_temp());
    }
    let unique: std::collections::HashSet<_> = temps.iter().collect();
    assert_eq!(unique.len(), 50);
}

#[test]
fn vargen_mark_used() {
    let mut gen = VariableNameGenerator::new();
    gen.mark_used("node_conflict_result");

    // Now generating for "conflict" should detect the name is used
    let name = gen.generate_for_node("conflict");
    assert_ne!(name, "node_conflict_result");
    assert!(name.starts_with("node_conflict_result_"));
}

#[test]
fn vargen_mark_used_string() {
    let mut gen = VariableNameGenerator::new();
    gen.mark_used(String::from("reserved_name"));

    // Temp names shouldn't collide with custom names
    let t = gen.generate_temp();
    assert_ne!(t, "reserved_name");
}

#[test]
fn vargen_mixed_node_and_temp() {
    let mut gen = VariableNameGenerator::new();
    let n1 = gen.generate_for_node("alpha");
    let t1 = gen.generate_temp();
    let n2 = gen.generate_for_node("beta");
    let t2 = gen.generate_temp();

    let all = vec![&n1, &t1, &n2, &t2];
    let unique: std::collections::HashSet<_> = all.into_iter().collect();
    assert_eq!(unique.len(), 4);
}

// ===========================================================================
// sanitize_name
// ===========================================================================

#[test]
fn sanitize_alphanumeric_unchanged() {
    assert_eq!(sanitize_name("abc123"), "abc123");
}

#[test]
fn sanitize_underscore_preserved() {
    assert_eq!(sanitize_name("my_variable"), "my_variable");
}

#[test]
fn sanitize_hyphens_replaced() {
    assert_eq!(sanitize_name("my-node-1"), "my_node_1");
}

#[test]
fn sanitize_dots_replaced() {
    assert_eq!(sanitize_name("a.b.c"), "a_b_c");
}

#[test]
fn sanitize_spaces_replaced() {
    assert_eq!(sanitize_name("hello world"), "hello_world");
}

#[test]
fn sanitize_special_chars() {
    assert_eq!(sanitize_name("foo@bar#baz"), "foo_bar_baz");
}

#[test]
fn sanitize_empty_string() {
    assert_eq!(sanitize_name(""), "");
}

#[test]
fn sanitize_all_special() {
    assert_eq!(sanitize_name("!@#$%"), "_____");
}

#[test]
fn sanitize_unicode_letters() {
    // Unicode alphanumeric chars should be preserved
    let result = sanitize_name("café");
    assert!(result.contains("caf"));
}

// ===========================================================================
// get_default_value_for_type
// ===========================================================================

#[test]
fn default_f32() {
    assert_eq!(get_default_value_for_type("f32"), "0.0");
}

#[test]
fn default_f64() {
    assert_eq!(get_default_value_for_type("f64"), "0.0");
}

#[test]
fn default_signed_integers() {
    for t in ["i8", "i16", "i32", "i64", "i128", "isize"] {
        assert_eq!(get_default_value_for_type(t), "0", "failed for {}", t);
    }
}

#[test]
fn default_unsigned_integers() {
    for t in ["u8", "u16", "u32", "u64", "u128", "usize"] {
        assert_eq!(get_default_value_for_type(t), "0", "failed for {}", t);
    }
}

#[test]
fn default_bool() {
    assert_eq!(get_default_value_for_type("bool"), "false");
}

#[test]
fn default_char() {
    assert_eq!(get_default_value_for_type("char"), "'\\0'");
}

#[test]
fn default_string() {
    assert_eq!(get_default_value_for_type("String"), "String::new()");
}

#[test]
fn default_tuple_f32_f32() {
    assert_eq!(get_default_value_for_type("(f32, f32)"), "(0.0, 0.0)");
}

#[test]
fn default_tuple_i32_bool() {
    assert_eq!(get_default_value_for_type("(i32, bool)"), "(0, false)");
}

#[test]
fn default_tuple_three_elements() {
    assert_eq!(
        get_default_value_for_type("(f64, i32, bool)"),
        "(0.0, 0, false)"
    );
}

#[test]
fn default_unknown_type_uses_default_trait() {
    assert_eq!(
        get_default_value_for_type("MyCustomType"),
        "Default::default()"
    );
}

#[test]
fn default_vec_uses_default_trait() {
    assert_eq!(
        get_default_value_for_type("Vec<u8>"),
        "Default::default()"
    );
}
