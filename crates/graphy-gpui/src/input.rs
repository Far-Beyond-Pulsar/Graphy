//! Host-neutral input. The GPUI glue translates `gpui` events into these; the
//! interaction engine never sees a GPUI type, so the same logic is tested
//! headless and reused by any host.

use graphy_render::Vec2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Modifiers {
    pub shift: bool,
    /// The platform's primary modifier (Ctrl, or Cmd on macOS).
    pub ctrl: bool,
    pub alt: bool,
}

impl Modifiers {
    pub const NONE: Modifiers = Modifiers {
        shift: false,
        ctrl: false,
        alt: false,
    };

    pub fn additive(self) -> bool {
        self.shift || self.ctrl
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    Delete,
    Backspace,
    Escape,
    Enter,
    Home,
    Left,
    Right,
    Up,
    Down,
    /// A printable key, lower-case.
    Char(char),
}

/// Positions are screen-space logical pixels relative to the canvas origin.
#[derive(Clone, Debug, PartialEq)]
pub enum InputEvent {
    PointerDown {
        pos: Vec2,
        button: MouseButton,
        mods: Modifiers,
        clicks: u8,
    },
    PointerMove {
        pos: Vec2,
        mods: Modifiers,
    },
    PointerUp {
        pos: Vec2,
        button: MouseButton,
        mods: Modifiers,
    },
    /// `pixels` is true for precise (trackpad) deltas.
    Wheel {
        pos: Vec2,
        delta: Vec2,
        pixels: bool,
        mods: Modifiers,
    },
    KeyDown {
        key: Key,
        mods: Modifiers,
    },
    /// Text from the system clipboard, in answer to [`crate::Action::ClipboardReadRequest`].
    Paste(String),
    PointerLeave,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CursorHint {
    Default,
    Pointer,
    Grab,
    Grabbing,
    Move,
    ResizeNwse,
    Crosshair,
}
