//! # graphy-gpui
//!
//! The shared graph-editor front end:
//!
//! - [`controller`] — the interaction engine: selection, marquee, move/resize
//!   with snapping, connect/disconnect/re-route drags, reroute insertion,
//!   clipboard, framing, context-menu requests. Input in, [`Action`]s out;
//!   every document edit is a `graphy::GraphCommand` batch the editor applies
//!   through its own undo/save path ([`apply_transaction`]).
//! - [`profile`] — [`GraphProfile`], the extension API a domain implements
//!   (successor of the Blueprint plugin's `EditorProfile`).
//! - `canvas` (feature `gpui`) — the GPUI element that owns the GPU surface,
//!   feeds [`graphy_render_wgpu`] and translates GPUI events into
//!   [`InputEvent`]s.
//!
//! Without the `gpui` feature nothing here links GPUI or WGPU.

pub mod clipboard;
pub mod controller;
pub mod input;
pub mod profile;
pub mod snap;

#[cfg(feature = "gpui")]
pub mod canvas;

pub use controller::{
    apply_transaction, Action, Controller, GraphCtx, InteractionSettings, Outcome, Selection,
};
pub use input::{CursorHint, InputEvent, Key, Modifiers, MouseButton};
pub use profile::{
    default_can_connect, dynamic_pin_commands, CatalogEntry, GraphProfile, MacroTarget, MenuItem,
    MenuTarget, PinUpdate,
};
