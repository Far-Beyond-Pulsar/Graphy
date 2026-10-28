//! The one layout model.
//!
//! Drawing, text placement, wire endpoints, hit-testing and the spatial index
//! all read geometry from [`NodeLayout`]; nothing else computes a pin
//! position. A layout is stored in node-local coordinates (origin = node
//! top-left), so moving a node never relayouts it.
//!
//! Rows are the unit of vertical flow. A node's body is a list of
//! [`RowSpec`]s, each with its own height, so Blueprint-style fixed pin rows
//! and Shader-style variable-height rows (a texture preview under a pin) are
//! the same mechanism: the presenter decides each row's height through
//! [`crate::Presenter::row_height`] and [`ExtraRow`].

use crate::geom::{Rect, Vec2};

/// Layout constants, in graph units (unscaled by zoom).
#[derive(Clone, Debug, PartialEq)]
pub struct LayoutMetrics {
    pub header_h: f32,
    pub header_pad_x: f32,
    pub body_pad_y: f32,
    pub row_h: f32,
    pub row_gap: f32,
    pub pin_size: f32,
    /// Distance from the card edge to the pin centre.
    pub pin_inset: f32,
    /// Gap between a pin and its label.
    pub label_gap: f32,
    /// Minimum horizontal gap between an input label and an output label that
    /// share a row.
    pub column_gap: f32,
    pub icon_size: f32,
    pub min_width: f32,
    pub corner_radius: f32,
    /// Hit radius of a pin in graph units; the hit-test widens it at low zoom
    /// so the on-screen target never drops below a few pixels.
    pub pin_hit_radius: f32,
    pub grid_snap: f32,
    pub title_font: f32,
    pub label_font: f32,
    pub reroute_size: f32,
    pub pill_height: f32,
    pub comment_header_h: f32,
    pub comment_resize_handle: f32,
}

impl Default for LayoutMetrics {
    fn default() -> Self {
        Self {
            header_h: 26.0,
            header_pad_x: 10.0,
            body_pad_y: 6.0,
            row_h: 20.0,
            row_gap: 2.0,
            pin_size: 9.0,
            pin_inset: 12.0,
            label_gap: 7.0,
            column_gap: 16.0,
            icon_size: 14.0,
            min_width: 90.0,
            corner_radius: 6.0,
            pin_hit_radius: 11.0,
            grid_snap: 10.0,
            title_font: 12.0,
            label_font: 11.0,
            reroute_size: 16.0,
            pill_height: 22.0,
            comment_header_h: 24.0,
            comment_resize_handle: 14.0,
        }
    }
}

impl LayoutMetrics {
    /// Rounds `value` up to the grid so node sizes align with snapping.
    pub fn snap_up(&self, value: f32) -> f32 {
        (value / self.grid_snap).ceil() * self.grid_snap
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum PinSide {
    Input,
    Output,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NodeShape {
    /// Header + body card.
    Card,
    /// Small round node with a single pin on each side at the centre.
    Reroute,
    /// Compact one-line pill (conversion nodes): pins at the left/right ends.
    Pill,
}

/// What one body row holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowContent {
    /// An input pin on the left and/or an output pin on the right.
    Pins { input: Option<usize>, output: Option<usize> },
    /// Host content (texture preview, inline widget, custom body) identified by
    /// a profile-chosen slot id.
    Custom { slot: u32 },
}

/// A row with its resolved height.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowSpec {
    pub content: RowContent,
    pub height: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RowPosition {
    /// Between the header and the first pin row.
    Top,
    /// After the pin row with this index.
    AfterRow(usize),
    /// After the last pin row.
    Bottom,
}

/// A host-owned row (preview, widget) inserted among the pin rows.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ExtraRow {
    pub position: RowPosition,
    pub slot: u32,
    pub height: f32,
    /// Narrowest card that still fits this content (graph units).
    pub min_width: f32,
}

/// Everything layout needs to know about a node. Widths are text widths in
/// graph units, measured by the caller.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeSpec {
    pub shape: NodeShape,
    pub title_width: f32,
    pub has_icon: bool,
    pub input_label_widths: Vec<f32>,
    pub output_label_widths: Vec<f32>,
    /// Output drawn in the header's right end (a pure node's return value).
    pub header_output: Option<usize>,
    /// Resolved body rows, top to bottom.
    pub rows: Vec<RowSpec>,
    pub min_width: f32,
}

impl NodeSpec {
    /// Builds the row list: one pin row per `max(inputs, outputs)` (the header
    /// output is excluded), with `extras` inserted, and `height_of` supplying
    /// each pin row's height.
    pub fn build_rows(
        inputs: usize,
        outputs: usize,
        header_output: Option<usize>,
        extras: &[ExtraRow],
        mut height_of: impl FnMut(&RowContent) -> f32,
    ) -> (Vec<RowSpec>, f32) {
        let body_outputs: Vec<usize> = (0..outputs).filter(|o| Some(*o) != header_output).collect();
        let pin_rows = inputs.max(body_outputs.len());
        let mut rows = Vec::with_capacity(pin_rows + extras.len());
        let mut min_width = 0.0f32;
        let push_extras = |rows: &mut Vec<RowSpec>, pos: RowPosition, min_width: &mut f32| {
            for e in extras.iter().filter(|e| e.position == pos) {
                rows.push(RowSpec { content: RowContent::Custom { slot: e.slot }, height: e.height });
                *min_width = min_width.max(e.min_width);
            }
        };
        push_extras(&mut rows, RowPosition::Top, &mut min_width);
        for r in 0..pin_rows {
            let content = RowContent::Pins {
                input: (r < inputs).then_some(r),
                output: body_outputs.get(r).copied(),
            };
            let height = height_of(&content);
            rows.push(RowSpec { content, height });
            push_extras(&mut rows, RowPosition::AfterRow(r), &mut min_width);
        }
        push_extras(&mut rows, RowPosition::Bottom, &mut min_width);
        (rows, min_width)
    }
}

/// Where a pin sits on its node.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PinLayout {
    /// Node-local centre.
    pub center: Vec2,
    /// Body row index, or `None` for a header/shape pin.
    pub row: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RowLayout {
    /// Node-local rectangle (full card width).
    pub rect: Rect,
    pub content: RowContent,
}

/// Node-local geometry computed once from a [`NodeSpec`].
#[derive(Clone, Debug, PartialEq)]
pub struct NodeLayout {
    pub shape: NodeShape,
    pub size: Vec2,
    pub header: Rect,
    pub rows: Vec<RowLayout>,
    pub inputs: Vec<PinLayout>,
    pub outputs: Vec<PinLayout>,
}

impl NodeLayout {
    pub fn compute(spec: &NodeSpec, m: &LayoutMetrics) -> NodeLayout {
        match spec.shape {
            NodeShape::Reroute => Self::reroute(spec, m),
            NodeShape::Pill => Self::pill(spec, m),
            NodeShape::Card => Self::card(spec, m),
        }
    }

    fn reroute(spec: &NodeSpec, m: &LayoutMetrics) -> NodeLayout {
        let s = m.reroute_size;
        let c = Vec2::new(s * 0.5, s * 0.5);
        NodeLayout {
            shape: NodeShape::Reroute,
            size: Vec2::new(s, s),
            header: Rect::default(),
            rows: Vec::new(),
            inputs: vec![PinLayout { center: c, row: None }; spec.input_label_widths.len().max(1)],
            outputs: vec![PinLayout { center: c, row: None }; spec.output_label_widths.len().max(1)],
        }
    }

    fn pill(spec: &NodeSpec, m: &LayoutMetrics) -> NodeLayout {
        let w = m.snap_up((spec.title_width + m.header_pad_x * 2.0 + m.pin_size).max(50.0));
        let h = m.pill_height;
        NodeLayout {
            shape: NodeShape::Pill,
            size: Vec2::new(w, h),
            header: Rect::from_pos_size(Vec2::ZERO, Vec2::new(w, h)),
            rows: Vec::new(),
            inputs: vec![PinLayout { center: Vec2::new(0.0, h * 0.5), row: None }; spec.input_label_widths.len().max(1)],
            outputs: vec![PinLayout { center: Vec2::new(w, h * 0.5), row: None }; spec.output_label_widths.len().max(1)],
        }
    }

    fn card(spec: &NodeSpec, m: &LayoutMetrics) -> NodeLayout {
        // ---- width -------------------------------------------------------
        let icon_w = if spec.has_icon { m.icon_size + 6.0 } else { 0.0 };
        let header_out_w = if spec.header_output.is_some() { m.pin_inset + m.pin_size * 0.5 + 8.0 } else { 0.0 };
        let header_req = m.header_pad_x * 2.0 + icon_w + spec.title_width + header_out_w;
        let mut body_req = 0.0f32;
        for row in &spec.rows {
            if let RowContent::Pins { input, output } = row.content {
                let in_w = input.and_then(|i| spec.input_label_widths.get(i)).copied();
                let out_w = output.and_then(|o| spec.output_label_widths.get(o)).copied();
                let side = |w: Option<f32>| w.map_or(0.0, |w| m.pin_inset + m.pin_size * 0.5 + m.label_gap + w);
                let gap = if in_w.is_some() && out_w.is_some() { m.column_gap } else { 0.0 };
                body_req = body_req.max(side(in_w) + side(out_w) + gap);
            }
        }
        let width = m.snap_up(header_req.max(body_req).max(spec.min_width).max(m.min_width));

        // ---- vertical flow -----------------------------------------------
        let header = Rect::from_pos_size(Vec2::ZERO, Vec2::new(width, m.header_h));
        let mut y = m.header_h + m.body_pad_y;
        let mut rows = Vec::with_capacity(spec.rows.len());
        let mut inputs = vec![PinLayout { center: Vec2::ZERO, row: None }; spec.input_label_widths.len()];
        let mut outputs = vec![PinLayout { center: Vec2::ZERO, row: None }; spec.output_label_widths.len()];
        for (i, row) in spec.rows.iter().enumerate() {
            let rect = Rect::from_pos_size(Vec2::new(0.0, y), Vec2::new(width, row.height));
            if let RowContent::Pins { input, output } = row.content {
                let cy = y + row.height * 0.5;
                if let Some(slot) = input.and_then(|k| inputs.get_mut(k)) {
                    *slot = PinLayout { center: Vec2::new(m.pin_inset, cy), row: Some(i) };
                }
                if let Some(slot) = output.and_then(|k| outputs.get_mut(k)) {
                    *slot = PinLayout { center: Vec2::new(width - m.pin_inset, cy), row: Some(i) };
                }
            }
            rows.push(RowLayout { rect, content: row.content });
            y += row.height + m.row_gap;
        }
        if let Some(h) = spec.header_output {
            if let Some(slot) = outputs.get_mut(h) {
                *slot = PinLayout { center: Vec2::new(width - m.pin_inset, m.header_h * 0.5), row: None };
            }
        }
        let body_h = if spec.rows.is_empty() { 0.0 } else { y - m.row_gap - (m.header_h + m.body_pad_y) };
        let height = m.snap_up(m.header_h + m.body_pad_y * 2.0 + body_h);
        NodeLayout { shape: NodeShape::Card, size: Vec2::new(width, height), header, rows, inputs, outputs }
    }

    pub fn pin(&self, side: PinSide, index: usize) -> Option<&PinLayout> {
        match side {
            PinSide::Input => self.inputs.get(index),
            PinSide::Output => self.outputs.get(index),
        }
    }

    /// Node-local rectangle of the card.
    pub fn local_rect(&self) -> Rect {
        Rect::from_pos_size(Vec2::ZERO, self.size)
    }
}
