//! # AST Transformation Utilities
//!
//! Tools for parsing and transforming Rust Abstract Syntax Trees.
//!
//! This module provides utilities for:
//! - Parsing Rust function source code
//! - Replacing `exec_output!()` macro calls with actual code
//! - Substituting parameter values in function bodies
//! - Inlining control flow nodes

use crate::GraphyError;
use std::collections::HashMap;
use syn::{
    visit::{self, Visit},
    visit_mut::{self, VisitMut},
    Block, Expr, ExprMacro, ItemFn, Stmt,
};

/// Inline a control flow function with substitutions
///
/// This function:
/// 1. Parses the function source as AST
/// 2. Replaces `exec_output!("Label")` with connected code
/// 3. Substitutes parameter names with actual values
/// 4. Returns the inlined function body as a string
///
/// # Arguments
///
/// * `function_source` - The Rust source code of the function
/// * `exec_replacements` - Map of exec output labels to replacement code
/// * `param_substitutions` - Map of parameter names to their values
pub fn inline_control_flow_function(
    function_source: &str,
    exec_replacements: HashMap<String, String>,
    param_substitutions: HashMap<String, String>,
) -> Result<String, GraphyError> {
    tracing::info!("[AST] Inlining control flow function");
    tracing::info!("[AST] Exec replacements: {:?}", exec_replacements);
    tracing::info!("[AST] Param substitutions: {:?}", param_substitutions);

    // Parse the function
    let item_fn = parse_function(function_source)?;

    // Replace exec_output!() calls
    let replacer = ExecOutputReplacer::new(exec_replacements);
    let item_fn = replacer.replace_in_function(item_fn)?;

    // Substitute parameters
    let substitutor = ParameterSubstitutor::new(param_substitutions);
    let item_fn = substitutor.substitute_in_function(item_fn)?;

    // Convert back to source code
    let body_code = quote::quote! { #item_fn }.to_string();

    // Extract just the function body (remove fn signature)
    extract_function_body(&body_code)
}

/// Parse a function from source code
fn parse_function(source: &str) -> Result<ItemFn, GraphyError> {
    syn::parse_str::<ItemFn>(source)
        .map_err(|e| GraphyError::AstParsing(format!("Failed to parse function: {}", e)))
}

/// Extract the function body from generated code
fn extract_function_body(code: &str) -> Result<String, GraphyError> {
    // Find the function body between { and }
    if let Some(start) = code.find('{') {
        if let Some(end) = code.rfind('}') {
            let body = &code[start + 1..end];
            return Ok(body.trim().to_string());
        }
    }

    Err(GraphyError::AstParsing(
        "Could not extract function body".to_string(),
    ))
}

/// Replace `exec_output!()` calls with actual code
struct ExecOutputReplacer {
    replacements: HashMap<String, String>,
}

impl ExecOutputReplacer {
    pub fn new(replacements: HashMap<String, String>) -> Self {
        Self { replacements }
    }

    pub fn replace_in_function(mut self, func: ItemFn) -> Result<ItemFn, GraphyError> {
        let mut func = func;
        self.visit_item_fn_mut(&mut func);
        Ok(func)
    }
}

impl VisitMut for ExecOutputReplacer {
    fn visit_stmt_mut(&mut self, stmt: &mut Stmt) {
        match stmt {
            Stmt::Expr(expr, _) => {
                self.visit_expr_mut(expr);
            }
            Stmt::Macro(stmt_macro) => {
                if stmt_macro.mac.path.is_ident("exec_output") {
                    if let Ok(label) = syn::parse2::<syn::LitStr>(stmt_macro.mac.tokens.clone()) {
                        let label_value = label.value();

                        if let Some(replacement_code) = self.replacements.get(&label_value) {
                            tracing::info!(
                                "[AST] Replacing exec_output!(\"{}\") with: {}",
                                label_value,
                                replacement_code
                            );

                            // Parse replacement code and substitute
                            if let Ok(parsed_stmts) =
                                syn::parse_str::<syn::File>(&format!("fn dummy() {{{}}}", replacement_code))
                            {
                                if let Some(syn::Item::Fn(item_fn)) = parsed_stmts.items.first() {
                                    if let Some(first_stmt) = item_fn.block.stmts.first() {
                                        *stmt = first_stmt.clone();
                                    }
                                }
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        visit_mut::visit_stmt_mut(self, stmt);
    }

    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        if let Expr::Macro(ExprMacro { mac, .. }) = expr {
            if mac.path.is_ident("exec_output") {
                if let Ok(label) = syn::parse2::<syn::LitStr>(mac.tokens.clone()) {
                    let label_value = label.value();

                    if let Some(replacement_code) = self.replacements.get(&label_value) {
                        tracing::info!(
                            "[AST] Replacing exec_output!(\"{}\") expr with: {}",
                            label_value,
                            replacement_code
                        );

                        match syn::parse_str::<Expr>(replacement_code) {
                            Ok(replacement_expr) => {
                                *expr = replacement_expr;
                                return;
                            }
                            Err(_) => {
                                if let Ok(block) =
                                    syn::parse_str::<Block>(&format!("{{{}}}", replacement_code))
                                {
                                    *expr = Expr::Block(syn::ExprBlock {
                                        attrs: vec![],
                                        label: None,
                                        block,
                                    });
                                    return;
                                }
                            }
                        }
                    }
                }
            }
        }
        visit_mut::visit_expr_mut(self, expr);
    }
}

/// Substitute parameter names with actual values
struct ParameterSubstitutor {
    substitutions: HashMap<String, String>,
}

impl ParameterSubstitutor {
    pub fn new(substitutions: HashMap<String, String>) -> Self {
        Self { substitutions }
    }

    pub fn substitute_in_function(mut self, func: ItemFn) -> Result<ItemFn, GraphyError> {
        let mut func = func;
        self.visit_item_fn_mut(&mut func);
        Ok(func)
    }
}

impl VisitMut for ParameterSubstitutor {
    fn visit_expr_mut(&mut self, expr: &mut Expr) {
        if let Expr::Path(expr_path) = expr {
            if expr_path.path.segments.len() == 1 {
                let ident = &expr_path.path.segments[0].ident;
                let ident_str = ident.to_string();

                if let Some(replacement) = self.substitutions.get(&ident_str) {
                    tracing::info!("[AST] Substituting {} with {}", ident_str, replacement);

                    if let Ok(replacement_expr) = syn::parse_str::<Expr>(replacement) {
                        *expr = replacement_expr;
                        return;
                    }
                }
            }
        }
        visit_mut::visit_expr_mut(self, expr);
    }
}

/// Extract exec output labels from a function
///
/// Parses the function and finds all `exec_output!("Label")` calls.
pub fn extract_exec_output_labels(function_source: &str) -> Result<Vec<String>, GraphyError> {
    let item_fn = parse_function(function_source)?;
    let mut extractor = ExecOutputLabelExtractor { labels: Vec::new() };
    extractor.visit_item_fn(&item_fn);
    
    tracing::debug!("[AST] Extracted {} exec_output labels", extractor.labels.len());
    
    Ok(extractor.labels)
}

struct ExecOutputLabelExtractor {
    labels: Vec<String>,
}

impl<'ast> Visit<'ast> for ExecOutputLabelExtractor {
    fn visit_stmt(&mut self, stmt: &'ast Stmt) {
        // Check for macro statements (exec_output! as a statement)
        if let Stmt::Macro(stmt_macro) = stmt {
            if stmt_macro.mac.path.is_ident("exec_output") {
                if let Ok(label) = syn::parse2::<syn::LitStr>(stmt_macro.mac.tokens.clone()) {
                    self.labels.push(label.value());
                }
            }
        }
        
        // Continue visiting nested statements and expressions
        visit::visit_stmt(self, stmt);
    }
    
    fn visit_expr(&mut self, expr: &'ast Expr) {
        // Also check for macro expressions (exec_output! in expression position)
        if let Expr::Macro(ExprMacro { mac, .. }) = expr {
            if mac.path.is_ident("exec_output") {
                if let Ok(label) = syn::parse2::<syn::LitStr>(mac.tokens.clone()) {
                    self.labels.push(label.value());
                }
            }
        }
        
        // Continue visiting nested expressions
        visit::visit_expr(self, expr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_exec_labels() {
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
    fn test_inline_control_flow() {
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
        assert!(code.contains("println"));
    }
}
