//! Visual tokens for the canvas.
//!
//! One dark theme, derived from the reference mockup's palette (near-black
//! canvas, neutral raised surfaces, hairline borders, a single blue focus
//! colour, category colour used only as a small accent). The GPU layer reads
//! these values; no colour is hard-coded in a shader except glyph-level AA.

use crate::geom::{rgba, Color};

#[derive(Clone, Debug, PartialEq)]
pub struct Theme {
    pub canvas: Color,
    pub grid_minor: Color,
    pub grid_major: Color,

    pub card_body: Color,
    pub card_header: Color,
    pub card_border: Color,
    pub card_border_hover: Color,
    pub shadow: Color,

    pub select: Color,
    pub focus: Color,
    pub marquee_fill: Color,
    pub marquee_border: Color,

    pub text: Color,
    pub text_dim: Color,
    pub text_faint: Color,

    pub comment_fill: Color,
    pub comment_border: Color,

    pub preview_well: Color,

    /// Wire colour used when a profile gives none.
    pub wire_default: Color,
    /// Colour pins and wires fade toward when dimmed (usually the canvas).
    pub dim_target: Color,
}

impl Theme {
    pub fn dark() -> Theme {
        Theme {
            canvas: rgba(0x0d, 0x0d, 0x0d, 1.0),
            grid_minor: rgba(0xff, 0xff, 0xff, 0.030),
            grid_major: rgba(0xff, 0xff, 0xff, 0.062),

            card_body: rgba(0x17, 0x17, 0x17, 1.0),
            card_header: rgba(0x20, 0x20, 0x20, 1.0),
            card_border: rgba(0x4a, 0x4a, 0x4e, 1.0),
            card_border_hover: rgba(0x73, 0x73, 0x78, 1.0),
            shadow: rgba(0x00, 0x00, 0x00, 0.55),

            select: rgba(0xe4, 0xe4, 0xe7, 1.0),
            focus: rgba(0x3b, 0x82, 0xf6, 1.0),
            marquee_fill: rgba(0x3b, 0x82, 0xf6, 0.10),
            marquee_border: rgba(0x3b, 0x82, 0xf6, 0.85),

            text: rgba(0xf2, 0xf2, 0xf2, 1.0),
            text_dim: rgba(0xa3, 0xa3, 0xa3, 1.0),
            text_faint: rgba(0x73, 0x73, 0x73, 1.0),

            comment_fill: rgba(0xff, 0xff, 0xff, 0.030),
            comment_border: rgba(0xff, 0xff, 0xff, 0.14),

            preview_well: rgba(0x0b, 0x0b, 0x0b, 1.0),

            wire_default: rgba(0x8b, 0x8b, 0x8b, 1.0),
            dim_target: rgba(0x0d, 0x0d, 0x0d, 1.0),
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Theme::dark()
    }
}

/// Stroke weights in graph units at zoom 1 (the renderer keeps them at least
/// one physical pixel wide).
#[derive(Clone, Debug, PartialEq)]
pub struct WireWeights {
    pub base: f32,
    pub emphasis: f32,
    pub dim: f32,
}

impl Default for WireWeights {
    fn default() -> Self {
        Self {
            base: 1.7,
            emphasis: 2.6,
            dim: 1.3,
        }
    }
}
