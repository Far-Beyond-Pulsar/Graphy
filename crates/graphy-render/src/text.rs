//! Text measurement seam. The headless crate never rasterises; it only needs
//! advance widths to size nodes and truncate labels. The GPU layer implements
//! [`TextMeasure`] with the real font so layout and glyphs agree.

pub trait TextMeasure {
    /// Advance width of `text` at `size` (graph units == logical pixels at
    /// zoom 1).
    fn width(&self, text: &str, size: f32) -> f32;
}

/// Deterministic measure for tests and headless tools: every char advances by
/// `em_fraction * size`.
#[derive(Clone, Copy, Debug)]
pub struct FixedAdvance(pub f32);

impl Default for FixedAdvance {
    fn default() -> Self {
        FixedAdvance(0.55)
    }
}

impl TextMeasure for FixedAdvance {
    fn width(&self, text: &str, size: f32) -> f32 {
        text.chars().count() as f32 * self.0 * size
    }
}

/// Truncates `text` with an ellipsis so it fits `max_width`.
pub fn fit_text(measure: &dyn TextMeasure, text: &str, size: f32, max_width: f32) -> String {
    if measure.width(text, size) <= max_width {
        return text.to_string();
    }
    let ellipsis = "…";
    let budget = max_width - measure.width(ellipsis, size);
    if budget <= 0.0 {
        return String::new();
    }
    let mut out = String::new();
    for c in text.chars() {
        out.push(c);
        if measure.width(&out, size) > budget {
            out.pop();
            break;
        }
    }
    out.push_str(ellipsis);
    out
}
