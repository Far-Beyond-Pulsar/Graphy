//! View transform: pan/zoom, anchored zoom, visible rect, framing and the
//! zoom-tier level of detail.
//!
//! Convention (shared with the shaders): `screen = (graph + pan) * zoom`, so
//! `pan` is in graph units and the graph origin sits at `pan * zoom` on screen.
//! Screen space here is logical pixels; the GPU layer multiplies by the
//! display scale factor.

use crate::geom::{Rect, Vec2};

pub const MIN_ZOOM: f32 = 0.05;
pub const MAX_ZOOM: f32 = 4.0;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ViewTransform {
    pub pan: Vec2,
    pub zoom: f32,
}

impl Default for ViewTransform {
    fn default() -> Self {
        Self {
            pan: Vec2::ZERO,
            zoom: 1.0,
        }
    }
}

impl ViewTransform {
    pub fn graph_to_screen(&self, p: Vec2) -> Vec2 {
        (p + self.pan) * self.zoom
    }

    pub fn screen_to_graph(&self, p: Vec2) -> Vec2 {
        p * (1.0 / self.zoom) - self.pan
    }

    pub fn graph_rect_to_screen(&self, r: Rect) -> Rect {
        Rect {
            min: self.graph_to_screen(r.min),
            max: self.graph_to_screen(r.max),
        }
    }

    /// Pans by a screen-space delta (a drag of the canvas).
    pub fn pan_by_screen(&mut self, delta: Vec2) {
        self.pan = self.pan + delta * (1.0 / self.zoom);
    }

    /// Multiplies the zoom by `factor`, keeping the graph point under
    /// `anchor_screen` fixed on screen. Returns whether the zoom changed.
    pub fn zoom_at(&mut self, anchor_screen: Vec2, factor: f32) -> bool {
        let target = (self.zoom * factor).clamp(MIN_ZOOM, MAX_ZOOM);
        if (target - self.zoom).abs() < f32::EPSILON {
            return false;
        }
        let anchor_graph = self.screen_to_graph(anchor_screen);
        self.zoom = target;
        // screen = (graph + pan) * zoom  =>  pan = screen / zoom - graph
        self.pan = anchor_screen * (1.0 / target) - anchor_graph;
        true
    }

    /// Graph-space rectangle visible in a viewport of `viewport` logical
    /// pixels, grown by `pad_px` screen pixels on every side.
    pub fn visible_rect(&self, viewport: Vec2, pad_px: f32) -> Rect {
        let min = self.screen_to_graph(Vec2::new(-pad_px, -pad_px));
        let max = self.screen_to_graph(Vec2::new(viewport.x + pad_px, viewport.y + pad_px));
        Rect { min, max }
    }

    /// Fits `bounds` into the viewport with `margin_px` around it, centred.
    /// `max_zoom` keeps a single small node from being blown up.
    pub fn framing(bounds: Rect, viewport: Vec2, margin_px: f32, max_zoom: f32) -> ViewTransform {
        let avail = Vec2::new(
            (viewport.x - margin_px * 2.0).max(1.0),
            (viewport.y - margin_px * 2.0).max(1.0),
        );
        let size = Vec2::new(bounds.width().max(1.0), bounds.height().max(1.0));
        let zoom = (avail.x / size.x)
            .min(avail.y / size.y)
            .clamp(MIN_ZOOM, max_zoom.min(MAX_ZOOM));
        let center = bounds.center();
        // the viewport centre maps to the centre of the bounds
        let pan = Vec2::new(
            viewport.x * 0.5 / zoom - center.x,
            viewport.y * 0.5 / zoom - center.y,
        );
        ViewTransform { pan, zoom }
    }
}

/// Level of detail chosen from the zoom, so the overview stays clean and the
/// close view stays useful.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum LodTier {
    /// Silhouettes only: card blocks and coarse wires, no text or pins.
    Overview,
    /// Titles and pin dots; no pin labels, inline widgets or previews.
    Summary,
    /// Everything.
    Detail,
}

impl LodTier {
    pub const OVERVIEW_BELOW: f32 = 0.30;
    pub const SUMMARY_BELOW: f32 = 0.62;

    pub fn for_zoom(zoom: f32) -> LodTier {
        if zoom < Self::OVERVIEW_BELOW {
            LodTier::Overview
        } else if zoom < Self::SUMMARY_BELOW {
            LodTier::Summary
        } else {
            LodTier::Detail
        }
    }

    pub fn shows_titles(self) -> bool {
        self >= LodTier::Summary
    }

    pub fn shows_pins(self) -> bool {
        self >= LodTier::Summary
    }

    pub fn shows_pin_labels(self) -> bool {
        self == LodTier::Detail
    }

    /// Line segments per bezier wire.
    pub fn wire_segments(self) -> u32 {
        match self {
            LodTier::Overview => 8,
            LodTier::Summary => 16,
            LodTier::Detail => 32,
        }
    }
}
