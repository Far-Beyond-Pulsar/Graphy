//! # Compiler Diagnostics
//!
//! Structured error and warning reporting for Graphy compilation passes.
//!
//! Each diagnostic carries:
//! - a [`Severity`] (error, warning, info, hint)
//! - an optional source location (node ID and/or pin name)
//! - the emitting [`PassName`]
//! - a human-readable message
//!
//! ## Usage
//!
//! Passes accumulate [`Diagnostic`]s and return them inside a [`CompileResult`].
//! The caller decides whether to abort on warnings or only on errors.
//!
//! ```
//! use graphy::diagnostics::{Diagnostic, Severity, DiagnosticAccumulator, PassName};
//!
//! let mut acc = DiagnosticAccumulator::new(PassName::TypeChecker);
//! acc.error("node_1", None, "Expected f64, got String");
//! acc.warn("node_2", Some("value"), "Implicit coercion from i32 to f64");
//! let diags = acc.finish();
//! ```

use serde::{Deserialize, Serialize};

// ─────────────────────────────────────────────────────────────────────────────
// Severity
// ─────────────────────────────────────────────────────────────────────────────

/// Severity level of a compiler diagnostic.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// Purely informational — compilation succeeds with no caveats.
    Hint = 0,
    /// Non-fatal advisory — compilation succeeds but behavior may be surprising.
    Info = 1,
    /// Non-fatal issue — compilation succeeds but the graph has problems.
    Warning = 2,
    /// Fatal issue — compilation cannot proceed past this pass.
    Error = 3,
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Hint    => write!(f, "hint"),
            Self::Info    => write!(f, "info"),
            Self::Warning => write!(f, "warning"),
            Self::Error   => write!(f, "error"),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// PassName
// ─────────────────────────────────────────────────────────────────────────────

/// Identifies the compilation pass that emitted a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PassName {
    /// Subgraph expansion (inline macro/collapsed-region nodes).
    SubgraphExpander,
    /// Structural type checking and connection compatibility.
    TypeChecker,
    /// Data-flow analysis and topological sort.
    DataFlowAnalysis,
    /// Execution-flow routing and event tracing.
    ExecutionFlowAnalysis,
    /// Code generation backend.
    CodeGeneration,
    /// Any other pass — the contained string is the pass display name.
    Custom(String),
}

impl std::fmt::Display for PassName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SubgraphExpander      => write!(f, "subgraph_expander"),
            Self::TypeChecker           => write!(f, "type_checker"),
            Self::DataFlowAnalysis      => write!(f, "data_flow_analysis"),
            Self::ExecutionFlowAnalysis => write!(f, "execution_flow_analysis"),
            Self::CodeGeneration        => write!(f, "code_generation"),
            Self::Custom(s)             => write!(f, "{}", s),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// SourceLocation
// ─────────────────────────────────────────────────────────────────────────────

/// Source location within the graph that caused a diagnostic.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocation {
    /// ID of the node involved.
    pub node_id: String,

    /// ID of the pin involved, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pin_id: Option<String>,

    /// ID of the graph (for multi-graph documents).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub graph_id: Option<String>,
}

impl SourceLocation {
    pub fn node(node_id: impl Into<String>) -> Self {
        Self { node_id: node_id.into(), pin_id: None, graph_id: None }
    }

    pub fn pin(node_id: impl Into<String>, pin_id: impl Into<String>) -> Self {
        Self { node_id: node_id.into(), pin_id: Some(pin_id.into()), graph_id: None }
    }

    pub fn with_graph(mut self, graph_id: impl Into<String>) -> Self {
        self.graph_id = Some(graph_id.into());
        self
    }
}

impl std::fmt::Display for SourceLocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.pin_id {
            Some(pin) => write!(f, "{}.{}", self.node_id, pin),
            None      => write!(f, "{}", self.node_id),
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Diagnostic
// ─────────────────────────────────────────────────────────────────────────────

/// A single compiler diagnostic message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Diagnostic {
    /// Severity level.
    pub severity: Severity,

    /// Human-readable message.
    pub message: String,

    /// Source location in the graph, if available.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub location: Option<SourceLocation>,

    /// The pass that emitted this diagnostic.
    pub pass: PassName,

    /// Optional machine-readable error code (e.g. `"E0001"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub code: Option<String>,

    /// Optional contextual note (e.g. secondary explanation or hint).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

impl Diagnostic {
    pub fn new(
        severity: Severity,
        pass: PassName,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            message: message.into(),
            location: None,
            pass,
            code: None,
            note: None,
        }
    }

    /// Builder: attach a source location.
    #[must_use]
    pub fn at(mut self, loc: SourceLocation) -> Self {
        self.location = Some(loc);
        self
    }

    /// Builder: attach a node-only location.
    #[must_use]
    pub fn at_node(mut self, node_id: impl Into<String>) -> Self {
        self.location = Some(SourceLocation::node(node_id));
        self
    }

    /// Builder: attach a pin location.
    #[must_use]
    pub fn at_pin(mut self, node_id: impl Into<String>, pin_id: impl Into<String>) -> Self {
        self.location = Some(SourceLocation::pin(node_id, pin_id));
        self
    }

    /// Builder: attach an error code.
    #[must_use]
    pub fn with_code(mut self, code: impl Into<String>) -> Self {
        self.code = Some(code.into());
        self
    }

    /// Builder: attach a note.
    #[must_use]
    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.note = Some(note.into());
        self
    }

    /// Returns `true` if this diagnostic is fatal (Severity::Error).
    #[inline]
    pub fn is_error(&self) -> bool {
        self.severity == Severity::Error
    }

    /// Returns `true` if this diagnostic is a warning.
    #[inline]
    pub fn is_warning(&self) -> bool {
        self.severity == Severity::Warning
    }
}

impl std::fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}][{}]", self.severity, self.pass)?;
        if let Some(loc) = &self.location {
            write!(f, " {}:", loc)?;
        }
        write!(f, " {}", self.message)?;
        if let Some(note) = &self.note {
            write!(f, " (note: {})", note)?;
        }
        Ok(())
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// DiagnosticAccumulator
// ─────────────────────────────────────────────────────────────────────────────

/// Helper for passes to accumulate diagnostics during a compilation phase.
///
/// Passes construct one accumulator at the start, push diagnostics as they
/// discover issues, then call [`finish`] to extract the list.
pub struct DiagnosticAccumulator {
    pass: PassName,
    diags: Vec<Diagnostic>,
}

impl DiagnosticAccumulator {
    pub fn new(pass: PassName) -> Self {
        Self { pass, diags: Vec::new() }
    }

    fn push(&mut self, sev: Severity, node: &str, pin: Option<&str>, msg: impl Into<String>) {
        let loc = match pin {
            Some(p) => SourceLocation::pin(node, p),
            None    => SourceLocation::node(node),
        };
        self.diags.push(
            Diagnostic::new(sev, self.pass.clone(), msg).at(loc)
        );
    }

    pub fn error(&mut self, node: &str, pin: Option<&str>, msg: impl Into<String>) {
        self.push(Severity::Error, node, pin, msg);
    }

    pub fn warn(&mut self, node: &str, pin: Option<&str>, msg: impl Into<String>) {
        self.push(Severity::Warning, node, pin, msg);
    }

    pub fn info(&mut self, node: &str, pin: Option<&str>, msg: impl Into<String>) {
        self.push(Severity::Info, node, pin, msg);
    }

    pub fn hint(&mut self, node: &str, pin: Option<&str>, msg: impl Into<String>) {
        self.push(Severity::Hint, node, pin, msg);
    }

    /// Push a pre-built diagnostic.
    pub fn push_diagnostic(&mut self, d: Diagnostic) {
        self.diags.push(d);
    }

    /// Returns `true` if any error-level diagnostics have been added.
    pub fn has_errors(&self) -> bool {
        self.diags.iter().any(|d| d.is_error())
    }

    /// Returns all accumulated diagnostics, consuming the accumulator.
    pub fn finish(self) -> Vec<Diagnostic> {
        self.diags
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// CompileResult
// ─────────────────────────────────────────────────────────────────────────────

/// The result of a compilation pass: a value paired with any diagnostics.
///
/// A `CompileResult` can carry diagnostics even when it is `Ok` (warnings and
/// hints don't prevent compilation from succeeding). A result is considered
/// failed when it is `Err` OR when any of its diagnostics have
/// `Severity::Error`.
#[derive(Debug)]
pub struct CompileResult<T> {
    /// The output value, if the pass succeeded.
    pub value: Option<T>,

    /// All diagnostics emitted during the pass.
    pub diagnostics: Vec<Diagnostic>,
}

impl<T> CompileResult<T> {
    /// Creates a successful result with no diagnostics.
    pub fn ok(value: T) -> Self {
        Self { value: Some(value), diagnostics: Vec::new() }
    }

    /// Creates a successful result with accompanying diagnostics (warnings/hints).
    pub fn ok_with_diags(value: T, diags: Vec<Diagnostic>) -> Self {
        Self { value: Some(value), diagnostics: diags }
    }

    /// Creates a failed result (no value) with at least one error diagnostic.
    pub fn err(diags: Vec<Diagnostic>) -> Self {
        Self { value: None, diagnostics: diags }
    }

    /// Creates a failed result from a single error message.
    pub fn err_msg(pass: PassName, msg: impl Into<String>) -> Self {
        let d = Diagnostic::new(Severity::Error, pass, msg);
        Self { value: None, diagnostics: vec![d] }
    }

    /// Returns `true` if the pass succeeded with no error diagnostics.
    pub fn is_ok(&self) -> bool {
        self.value.is_some() && !self.has_errors()
    }

    /// Returns `true` if any error-level diagnostics are present.
    pub fn has_errors(&self) -> bool {
        self.diagnostics.iter().any(|d| d.is_error())
    }

    /// Returns all error-level diagnostics.
    pub fn errors(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter().filter(|d| d.is_error())
    }

    /// Returns all warning-level diagnostics.
    pub fn warnings(&self) -> impl Iterator<Item = &Diagnostic> {
        self.diagnostics.iter().filter(|d| d.is_warning())
    }

    /// Converts to a standard `Result`, discarding diagnostics on failure.
    pub fn into_result(self) -> Result<T, Vec<Diagnostic>> {
        if let Some(v) = self.value {
            Ok(v)
        } else {
            Err(self.diagnostics)
        }
    }

    /// Maps the inner value without touching diagnostics.
    pub fn map<U, F: FnOnce(T) -> U>(self, f: F) -> CompileResult<U> {
        CompileResult {
            value: self.value.map(f),
            diagnostics: self.diagnostics,
        }
    }
}

impl<T: std::fmt::Debug> CompileResult<T> {
    /// Formats all diagnostics as a multi-line string for logging/display.
    pub fn format_diagnostics(&self) -> String {
        self.diagnostics.iter()
            .map(|d| d.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accumulator_counts_errors() {
        let mut acc = DiagnosticAccumulator::new(PassName::TypeChecker);
        acc.warn("n1", None, "just a warning");
        assert!(!acc.has_errors());
        acc.error("n2", Some("out"), "bad type");
        assert!(acc.has_errors());
    }

    #[test]
    fn compile_result_ok_with_diags() {
        let mut acc = DiagnosticAccumulator::new(PassName::SubgraphExpander);
        acc.warn("n1", None, "deprecated node");
        let r: CompileResult<i32> = CompileResult::ok_with_diags(42, acc.finish());
        assert!(r.is_ok());
        assert_eq!(r.warnings().count(), 1);
        assert_eq!(r.errors().count(), 0);
    }

    #[test]
    fn compile_result_err() {
        let r: CompileResult<i32> = CompileResult::err_msg(PassName::CodeGeneration, "boom");
        assert!(!r.is_ok());
        assert!(r.has_errors());
    }

    #[test]
    fn diagnostic_display() {
        let d = Diagnostic::new(Severity::Error, PassName::TypeChecker, "bad connection")
            .at_pin("node_1", "output");
        let s = d.to_string();
        assert!(s.contains("error"));
        assert!(s.contains("node_1"));
        assert!(s.contains("bad connection"));
    }
}
