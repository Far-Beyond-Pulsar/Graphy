//! # Variable Name Generation
//!
//! Utilities for generating unique, valid variable names.

use std::collections::HashSet;

/// Variable name generator
///
/// Generates unique, sanitized variable names for nodes and intermediate results.
pub struct VariableNameGenerator {
    used_names: HashSet<String>,
    counter: usize,
}

impl VariableNameGenerator {
    pub fn new() -> Self {
        Self {
            used_names: HashSet::new(),
            counter: 0,
        }
    }

    /// Generate a unique variable name based on a node ID
    pub fn generate_for_node(&mut self, node_id: &str) -> String {
        let base_name = sanitize_name(node_id);
        let var_name = format!("node_{}_result", base_name);

        if self.used_names.contains(&var_name) {
            // Generate unique name with counter
            loop {
                self.counter += 1;
                let unique_name = format!("{}_{}", var_name, self.counter);
                if !self.used_names.contains(&unique_name) {
                    self.used_names.insert(unique_name.clone());
                    return unique_name;
                }
            }
        } else {
            self.used_names.insert(var_name.clone());
            var_name
        }
    }

    /// Generate a unique temporary variable name
    pub fn generate_temp(&mut self) -> String {
        loop {
            self.counter += 1;
            let temp_name = format!("temp_{}", self.counter);
            if !self.used_names.contains(&temp_name) {
                self.used_names.insert(temp_name.clone());
                return temp_name;
            }
        }
    }

    /// Mark a name as used
    pub fn mark_used(&mut self, name: impl Into<String>) {
        self.used_names.insert(name.into());
    }
}

impl Default for VariableNameGenerator {
    fn default() -> Self {
        Self::new()
    }
}

/// Sanitize a string to be a valid Rust variable name
pub fn sanitize_name(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || c == '_' { c } else { '_' })
        .collect()
}

/// Get the default value expression for a data type
pub fn get_default_value_for_type(type_str: &str) -> String {
    match type_str {
        "f32" | "f64" => "0.0".to_string(),
        "i8" | "i16" | "i32" | "i64" | "i128" | "isize" => "0".to_string(),
        "u8" | "u16" | "u32" | "u64" | "u128" | "usize" => "0".to_string(),
        "bool" => "false".to_string(),
        "char" => "'\\0'".to_string(),
        "String" => "String::new()".to_string(),
        _ if type_str.starts_with('(') && type_str.ends_with(')') => {
            // Tuple type
            let inner = &type_str[1..type_str.len() - 1];
            let parts: Vec<&str> = inner.split(',').collect();
            let defaults: Vec<String> = parts
                .iter()
                .map(|p| get_default_value_for_type(p.trim()))
                .collect();
            format!("({})", defaults.join(", "))
        }
        _ => "Default::default()".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_variable_name_generation() {
        let mut gen = VariableNameGenerator::new();

        let name1 = gen.generate_for_node("add_1");
        assert_eq!(name1, "node_add_1_result");

        let name2 = gen.generate_for_node("add_1");
        assert_ne!(name2, name1); // Should be unique

        let temp = gen.generate_temp();
        assert!(temp.starts_with("temp_"));
    }

    #[test]
    fn test_sanitize_name() {
        assert_eq!(sanitize_name("add-1"), "add_1");
        assert_eq!(sanitize_name("my.node.123"), "my_node_123");
        assert_eq!(sanitize_name("valid_name"), "valid_name");
    }

    #[test]
    fn test_default_values() {
        assert_eq!(get_default_value_for_type("f32"), "0.0");
        assert_eq!(get_default_value_for_type("i32"), "0");
        assert_eq!(get_default_value_for_type("bool"), "false");
        assert_eq!(get_default_value_for_type("String"), "String::new()");
        assert_eq!(get_default_value_for_type("(f32, f32)"), "(0.0, 0.0)");
    }
}
