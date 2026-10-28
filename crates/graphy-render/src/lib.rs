//! # graphy-render
//!
//! Headless graph-canvas model shared by every Graphy editor. No GPUI, no
//! WGPU: this crate decides *where things are* and *what is drawn*; backends
//! decide how.
//!
//! | Module | Owns |
//! |---|---|
//! | [`layout`] | the single node layout model (rows with per-row heights, pin positions) |
//! | [`view`] | pan/zoom transform, anchored zoom, framing, zoom-tier LOD |
//! | [`wire`] | bezier and rounded-orthogonal wire geometry and distance queries |
//! | [`spatial`] | R-tree index for nodes, comments and wires (culling, picking) |
//! | [`scene`] | derived scene from a `graphy::GraphDescription`, incremental updates |
//! | [`hit`] | hit-testing from the same layout the renderer draws |
//! | [`draw`] | renderer-neutral draw data: culling, LOD and state hierarchy |
//! | [`presenter`] | domain presentation hooks (titles, pin styles, row heights, wires) |
//! | [`style`] | visual tokens |
//!
//! Dependency direction: `graphy` <- `graphy-render` <- (`graphy-render-wgpu`,
//! `graphy-gpui`). Compiler-only consumers never depend on this crate.

pub mod draw;
pub mod geom;
pub mod hit;
pub mod layout;
pub mod presenter;
pub mod samples;
pub mod scene;
pub mod spatial;
pub mod style;
pub mod text;
pub mod view;
pub mod wire;

pub use draw::{
    flags, BoxDraw, BoxKind, CanvasDraw, ConnectPreview, DrawData, DrawParams, DrawStats, InteractionState, PinDraw,
    PreviewSlot, TextAlign, TextDraw, WireDraw,
};
pub use geom::{Color, Rect, Vec2};
pub use hit::{Hit, HitTolerance, NodePart, PinAddr};
pub use layout::{
    ExtraRow, LayoutMetrics, NodeLayout, NodeShape, NodeSpec, PinLayout, PinSide, RowContent, RowLayout, RowPosition,
    RowSpec,
};
pub use presenter::{
    category_accent, default_pin_style, prettify, CommentPresentation, DefaultPresenter, NodePresentation, PinShape,
    PinStyle, Presenter, WireStyle,
};
pub use scene::{PinInfo, Scene, SceneComment, SceneNode, SceneWire};
pub use style::{Theme, WireWeights};
pub use text::{fit_text, FixedAdvance, TextMeasure};
pub use view::{LodTier, ViewTransform, MAX_ZOOM, MIN_ZOOM};
pub use wire::{WireRouting, WireShape};
