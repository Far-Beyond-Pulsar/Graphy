//! Visual tokens for the canvas.
//!
//! One dark theme, derived from the reference mockup's palette (near-black
//! canvas, neutral raised surfaces, hairline borders, a single blue focus
//! colour, category colour used only as a small accent). The GPU layer reads
//! these values; no colour is hard-coded in a shader except glyph-level AA.

use crate::geom::{mix, rgba, with_alpha, Color};

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
            canvas: rgba(0x17, 0x17, 0x17, 1.0),
            grid_minor: rgba(0xff, 0xff, 0xff, 0.10),
            grid_major: rgba(0xff, 0xff, 0xff, 0.19),

            card_body: rgba(0x0a, 0x0a, 0x0a, 1.0),
            card_header: rgba(0x1c, 0x1c, 0x1c, 1.0),
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
            dim_target: rgba(0x17, 0x17, 0x17, 1.0),
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

/// The few colours a host's UI theme supplies; [`Theme::from_colors`] derives the
/// whole canvas palette from them. All are display-space RGBA.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ThemeColors {
    /// The app's page colour; node bodies and headers are mixed from it.
    pub background: Color,
    /// The graph's own background (the grid sits on it), e.g. the theme's
    /// secondary surface.
    pub canvas: Color,
    /// Primary text colour.
    pub foreground: Color,
    /// Secondary text colour.
    pub muted_foreground: Color,
    /// Hairline colour.
    pub border: Color,
    /// Selection and focus colour.
    pub accent: Color,
}

/// WCAG relative luminance of an opaque colour.
pub fn relative_luminance(c: Color) -> f32 {
    let lin = |v: f32| {
        if v <= 0.03928 {
            v / 12.92
        } else {
            ((v + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * lin(c[0]) + 0.7152 * lin(c[1]) + 0.0722 * lin(c[2])
}

/// WCAG contrast ratio (1..21) between two opaque colours.
pub fn contrast_ratio(a: Color, b: Color) -> f32 {
    let (la, lb) = (relative_luminance(a), relative_luminance(b));
    (la.max(lb) + 0.05) / (la.min(lb) + 0.05)
}

/// `fg` moved toward black or white (whichever the surface is farther from)
/// until it reaches `min_ratio` against every surface in `over`.
fn readable(fg: Color, over: &[Color], min_ratio: f32) -> Color {
    let worst = |c: Color| {
        over.iter()
            .map(|&bg| contrast_ratio(c, bg))
            .fold(f32::MAX, f32::min)
    };
    if worst(fg) >= min_ratio {
        return fg;
    }
    let mean = over.iter().map(|&c| relative_luminance(c)).sum::<f32>() / over.len() as f32;
    let target = if mean < 0.4 {
        [1.0, 1.0, 1.0, 1.0]
    } else {
        [0.0, 0.0, 0.0, 1.0]
    };
    (1..=20)
        .map(|i| mix(fg, target, i as f32 / 20.0))
        .find(|&c| worst(c) >= min_ratio)
        .unwrap_or(target)
}

impl Theme {
    /// A palette derived from a host theme: the canvas takes the theme's
    /// background; cards, the grid and the borders are the foreground mixed
    /// into it, so the graph blends with light and dark themes alike, and text
    /// is pushed to stay readable on every surface it is drawn over.
    pub fn from_colors(c: &ThemeColors) -> Theme {
        let background = with_alpha(c.background, 1.0);
        let canvas = with_alpha(c.canvas, 1.0);
        let fg = with_alpha(c.foreground, 1.0);
        let dark = relative_luminance(background) < 0.4;
        let card_body = background;
        let card_header = mix(background, fg, 0.08);
        let surfaces = [canvas, background, card_body, card_header];
        let (grid_minor, grid_major) = if dark { (0.10, 0.19) } else { (0.07, 0.14) };
        let accent = with_alpha(c.accent, 1.0);
        Theme {
            canvas,
            grid_minor: with_alpha(fg, grid_minor),
            grid_major: with_alpha(fg, grid_major),

            card_body,
            card_header,
            card_border: mix(background, fg, 0.36),
            card_border_hover: mix(background, fg, 0.52),
            shadow: [0.0, 0.0, 0.0, if dark { 0.55 } else { 0.25 }],

            select: fg,
            focus: accent,
            marquee_fill: with_alpha(accent, 0.10),
            marquee_border: with_alpha(accent, 0.85),

            text: readable(fg, &surfaces, 7.0),
            text_dim: readable(with_alpha(c.muted_foreground, 1.0), &surfaces, 4.5),
            text_faint: readable(mix(canvas, fg, 0.45), &surfaces, 3.0),

            comment_fill: with_alpha(fg, if dark { 0.03 } else { 0.05 }),
            comment_border: with_alpha(fg, 0.14),

            preview_well: mix(canvas, [0.0, 0.0, 0.0, 1.0], if dark { 0.2 } else { 0.08 }),

            wire_default: readable(mix(canvas, fg, 0.55), &[canvas], 3.0),
            dim_target: canvas,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn colors(background: Color, foreground: Color, muted: Color) -> ThemeColors {
        ThemeColors {
            background,
            canvas: mix(background, foreground, 0.06),
            foreground,
            muted_foreground: muted,
            border: mix(background, foreground, 0.2),
            accent: rgba(0x3b, 0x82, 0xf6, 1.0),
        }
    }

    fn themes() -> Vec<(&'static str, ThemeColors)> {
        vec![
            ("default dark", colors(rgba(0x0a, 0x0a, 0x0a, 1.0), rgba(0xf2, 0xf2, 0xf2, 1.0), rgba(0xa3, 0xa3, 0xa3, 1.0))),
            ("light", colors(rgba(0xfa, 0xfa, 0xfa, 1.0), rgba(0x17, 0x17, 0x17, 1.0), rgba(0x73, 0x73, 0x73, 1.0))),
            ("mid blue", colors(rgba(0x2a, 0x35, 0x5a, 1.0), rgba(0xe0, 0xe6, 0xff, 1.0), rgba(0x90, 0x9a, 0xc0, 1.0))),
            ("sepia", colors(rgba(0xf4, 0xec, 0xd8, 1.0), rgba(0x5b, 0x46, 0x36, 1.0), rgba(0x8a, 0x76, 0x62, 1.0))),
            // A muted colour that would be unreadable as given.
            ("low contrast", colors(rgba(0x10, 0x10, 0x10, 1.0), rgba(0xf0, 0xf0, 0xf0, 1.0), rgba(0x20, 0x20, 0x20, 1.0))),
        ]
    }

    #[test]
    fn default_canvas_is_the_default_dark_secondary() {
        assert_eq!(Theme::dark().canvas, rgba(0x17, 0x17, 0x17, 1.0));
        assert_eq!(Theme::default().dim_target, Theme::dark().canvas);
    }

    #[test]
    fn text_stays_readable_on_every_surface_of_every_theme() {
        for (name, c) in themes() {
            let t = Theme::from_colors(&c);
            for (label, surface) in [("canvas", t.canvas), ("body", t.card_body), ("header", t.card_header)] {
                assert!(contrast_ratio(t.text, surface) >= 7.0, "{name}: text on {label}");
                assert!(contrast_ratio(t.text_dim, surface) >= 4.5, "{name}: dim text on {label}");
                assert!(contrast_ratio(t.text_faint, surface) >= 3.0, "{name}: faint text on {label}");
            }
        }
    }

    #[test]
    fn cards_and_grid_are_distinguishable_from_the_canvas() {
        for (name, c) in themes() {
            let t = Theme::from_colors(&c);
            assert_eq!(t.canvas, with_alpha(c.canvas, 1.0), "{name}");
            let over = |line: Color| mix(t.canvas, with_alpha(line, 1.0), line[3]);
            assert!(contrast_ratio(over(t.grid_minor), t.canvas) > 1.03, "{name}: minor grid");
            assert!(contrast_ratio(over(t.grid_major), t.canvas) > contrast_ratio(over(t.grid_minor), t.canvas), "{name}: major > minor");
            assert!(contrast_ratio(t.card_header, t.card_body) > 1.05, "{name}: header chip");
            assert!(contrast_ratio(t.card_border, t.card_body) > 1.5, "{name}: border");
        }
    }

    #[test]
    fn the_canvas_is_the_host_canvas_colour_and_light_themes_stay_light() {
        let dark = Theme::from_colors(&themes()[0].1);
        assert_eq!(dark.canvas, with_alpha(themes()[0].1.canvas, 1.0));
        let light = Theme::from_colors(&themes()[1].1);
        assert!(relative_luminance(light.canvas) > 0.8);
        assert!(relative_luminance(light.card_body) > 0.7, "cards stay light on a light theme");
    }
}
