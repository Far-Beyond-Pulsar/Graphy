//! The shared node picker (feature `gpui`): a profile-supplied catalogue
//! ([`CatalogEntry`]) with search, foldable categories and, when opened by a
//! dropped wire, only the entries that can take the wire's type.
//!
//! It is drawn with the app's UI toolkit (`ui`: text input, virtual list,
//! scrollbar, theme colours) so it looks like the rest of the editor, and it
//! serves every graph editor: the canvas opens it over itself, and a host can
//! dock the same entity as a palette panel. The list model is
//! [`crate::catalog`]; this file only draws it.

use std::collections::HashSet;
use std::rc::Rc;

use gpui::prelude::*;
use gpui::{
    div, px, size, AnyElement, App, Context, Entity, EventEmitter, FocusHandle, Focusable, Hsla,
    KeyDownEvent, MouseButton, Pixels, Render, Rgba, ScrollStrategy, SharedString, Size,
    Subscription, Window,
};
use graphy_render::Color;
use ui::input::{InputEvent, InputState, TextInput};
use ui::scroll::{Scrollbar, ScrollbarState};
use ui::{h_flex, v_flex, v_virtual_list, ActiveTheme, Icon, IconName, VirtualListScrollHandle};

use crate::catalog::{matching_count, visible_rows, Row};
use crate::profile::CatalogEntry;

/// Size of the picker popup.
pub const PICKER_SIZE: (f32, f32) = (320.0, 480.0);
const CATEGORY_HEADER_H: f32 = 28.0;
const NODE_ENTRY_H: f32 = 52.0;

fn hsla(c: Color) -> Hsla {
    Rgba {
        r: c[0],
        g: c[1],
        b: c[2],
        a: c[3],
    }
    .into()
}

/// What the picker reports.
#[derive(Clone, Debug)]
pub enum PickerEvent {
    /// The user chose this catalogue definition.
    Chosen(String),
    /// Escape.
    Dismissed,
}

/// Which wire opened the picker, shown as a chip in its header.
#[derive(Clone, Debug)]
pub struct DragChip {
    pub label: String,
    pub color: Option<Color>,
}

pub struct NodePicker {
    entries: Vec<CatalogEntry>,
    chip: Option<DragChip>,
    focus: FocusHandle,
    search: Entity<InputState>,
    scroll: VirtualListScrollHandle,
    scrollbar: ScrollbarState,
    /// Categories the user opened by hand. They show every node, even while a
    /// search is narrowing the others to their matches.
    expanded: HashSet<String>,
    _subscription: Subscription,
}

impl EventEmitter<PickerEvent> for NodePicker {}

impl Focusable for NodePicker {
    fn focus_handle(&self, _: &App) -> FocusHandle {
        self.focus.clone()
    }
}

impl NodePicker {
    pub fn new(
        entries: Vec<CatalogEntry>,
        chip: Option<DragChip>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> Self {
        let search = cx.new(|cx| InputState::new(window, cx).placeholder("Search nodes…"));
        let subscription = cx.subscribe_in(
            &search,
            window,
            |this, _input, event: &InputEvent, _window, cx| {
                if matches!(event, InputEvent::Change) {
                    this.scroll.scroll_to_item(0, ScrollStrategy::Top);
                    cx.notify();
                }
            },
        );
        Self {
            entries,
            chip,
            focus: cx.focus_handle(),
            search,
            scroll: VirtualListScrollHandle::new(),
            scrollbar: ScrollbarState::default(),
            expanded: HashSet::new(),
            _subscription: subscription,
        }
    }

    /// Replaces the catalogue (the host's entries changed), keeping the query.
    pub fn set_entries(&mut self, entries: Vec<CatalogEntry>, cx: &mut Context<Self>) {
        self.entries = entries;
        cx.notify();
    }

    /// The search field's focus handle, to focus when the picker opens.
    pub fn search_focus_handle(&self, cx: &App) -> FocusHandle {
        self.search.read(cx).focus_handle(cx)
    }

    /// Open the category if it was folded, fold it if the user had opened it.
    fn toggle_category(&mut self, name: &str, cx: &mut Context<Self>) {
        if !self.expanded.remove(name) {
            self.expanded.insert(name.to_string());
        }
        cx.notify();
    }
}

fn item_sizes(rows: &[Row]) -> Rc<Vec<Size<Pixels>>> {
    Rc::new(
        rows.iter()
            .map(|row| {
                size(
                    px(0.0),
                    px(match row {
                        Row::Header { .. } => CATEGORY_HEADER_H,
                        Row::Entry(_) => NODE_ENTRY_H,
                    }),
                )
            })
            .collect(),
    )
}

impl Render for NodePicker {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let query = self.search.read(cx).value().to_string();
        // Opened by a dropped wire: the list is already narrowed to what fits, so
        // every category opens.
        let rows = visible_rows(&self.entries, &query, &self.expanded, self.chip.is_some());
        let node_count = matching_count(&self.entries, &query);
        let sizes = item_sizes(&rows);
        let filter_color = self
            .chip
            .as_ref()
            .and_then(|c| c.color)
            .map(hsla)
            .unwrap_or_else(|| cx.theme().border);
        let filter_label = self.chip.as_ref().map(|c| c.label.clone());

        // Owned snapshot for the 'static virtual-list closure.
        let entries = self.entries.clone();
        let view = cx.entity().clone();
        let scroll = self.scroll.clone();
        let scrollbar = self.scrollbar.clone();

        v_flex()
            .size_full()
            .bg(cx.theme().sidebar)
            .track_focus(&self.focus)
            .on_key_down(cx.listener(|_, ev: &KeyDownEvent, _, cx| {
                if ev.keystroke.key == "escape" {
                    cx.emit(PickerEvent::Dismissed);
                }
            }))
            // ── Header: title row + search box ────────────────────────────────
            .child(
                v_flex()
                    .w_full()
                    .child(
                        h_flex()
                            .w_full()
                            .px_3()
                            .py_2()
                            .bg(cx.theme().secondary)
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .items_center()
                            .justify_between()
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .child(Icon::new(IconName::Search).size(px(14.0)))
                                    .child(
                                        div()
                                            .text_sm()
                                            .font_weight(gpui::FontWeight::SEMIBOLD)
                                            .text_color(cx.theme().foreground)
                                            .child("Palette"),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .items_center()
                                    .when_some(filter_label, |row, label| {
                                        row.child(
                                            div()
                                                .px_1p5()
                                                .rounded_sm()
                                                .bg(filter_color.opacity(0.2))
                                                .border_1()
                                                .border_color(filter_color.opacity(0.6))
                                                .text_xs()
                                                .text_color(cx.theme().foreground)
                                                .child(label),
                                        )
                                    })
                                    .child(
                                        div()
                                            .text_xs()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(format!("{node_count} nodes")),
                                    ),
                            ),
                    )
                    .child(
                        h_flex()
                            .w_full()
                            .px_2()
                            .py_1p5()
                            .bg(cx.theme().sidebar)
                            .border_b_1()
                            .border_color(cx.theme().border.opacity(0.4))
                            .child(
                                TextInput::new(&self.search)
                                    .w_full()
                                    .appearance(false)
                                    .prefix(
                                        Icon::new(IconName::Search)
                                            .size(px(12.0))
                                            .text_color(cx.theme().muted_foreground),
                                    )
                                    .cleanable(),
                            ),
                    ),
            )
            // ── Scrollable node list ───────────────────────────────────────────
            .child(
                div()
                    .flex_1()
                    .min_h_0()
                    .overflow_hidden()
                    .relative()
                    .child(
                        v_virtual_list(
                            view,
                            "graphy-node-picker-list",
                            sizes,
                            move |_view, range, _window, cx| {
                                range
                                    .map(|ix| -> AnyElement {
                                        match rows.get(ix) {
                                            Some(Row::Header {
                                                name,
                                                color,
                                                count,
                                                open,
                                                matched,
                                            }) => category_header(
                                                ix,
                                                name,
                                                color.map(hsla),
                                                *count,
                                                *open,
                                                *matched,
                                                cx,
                                            )
                                            .into_any_element(),
                                            Some(Row::Entry(i)) => {
                                                node_row(ix, &entries[*i], cx).into_any_element()
                                            }
                                            None => div().h(px(NODE_ENTRY_H)).into_any_element(),
                                        }
                                    })
                                    .collect()
                            },
                        )
                        .size_full()
                        .track_scroll(&scroll),
                    )
                    .child(
                        div()
                            .absolute()
                            .inset_0()
                            .child(Scrollbar::vertical(&scrollbar, &scroll)),
                    ),
            )
    }
}

/// Category-header row: click to fold or unfold the category. While a search
/// narrows the category the count reads `matched / total`.
fn category_header(
    ix: usize,
    name: &str,
    accent: Option<Hsla>,
    node_count: usize,
    expanded: bool,
    matched: Option<usize>,
    cx: &mut Context<NodePicker>,
) -> impl IntoElement {
    let accent = accent.unwrap_or(cx.theme().primary);
    let toggle_name = name.to_string();
    let count = match matched {
        Some(matched) if matched < node_count => format!("{matched} / {node_count}"),
        _ => node_count.to_string(),
    };
    let muted = cx.theme().muted_foreground;

    h_flex()
        .id(("graphy-picker-category", ix as u64))
        .w_full()
        .h(px(CATEGORY_HEADER_H))
        .items_center()
        .justify_between()
        .cursor_pointer()
        .bg(cx.theme().muted.opacity(if expanded { 0.22 } else { 0.15 }))
        .border_b_1()
        .border_color(cx.theme().border.opacity(0.3))
        .hover(|s| s.bg(cx.theme().muted.opacity(0.3)))
        // Coloured left-edge accent bar
        .child(
            div()
                .w(px(3.0))
                .h(px(CATEGORY_HEADER_H))
                .flex_shrink_0()
                .bg(accent.opacity(if expanded { 0.9 } else { 0.7 })),
        )
        .child(
            Icon::new(if expanded {
                IconName::ChevronDown
            } else {
                IconName::ChevronRight
            })
            .size(px(12.0))
            .text_color(muted)
            .ml_2(),
        )
        .child(
            div()
                .flex_1()
                .px_2()
                .text_xs()
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(if expanded {
                    cx.theme().foreground
                } else {
                    muted
                })
                .child(SharedString::from(name.to_uppercase())),
        )
        .child(div().px_3().text_xs().text_color(muted).child(count))
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |view, _event, _window, cx| {
                view.toggle_category(&toggle_name, cx);
            }),
        )
}

/// Clickable node-entry row: icon pill + name/description.
fn node_row(ix: usize, entry: &CatalogEntry, cx: &mut Context<NodePicker>) -> impl IntoElement {
    let accent = entry.color.map(hsla).unwrap_or(cx.theme().primary);
    let definition = entry.definition.clone();
    let glyph = if entry.icon.is_empty() {
        entry
            .title
            .chars()
            .next()
            .map(String::from)
            .unwrap_or_default()
    } else {
        entry.icon.clone()
    };

    h_flex()
        .id(("graphy-picker-node", ix as u64))
        .w_full()
        .h(px(NODE_ENTRY_H))
        .px_3()
        .gap_2()
        .items_center()
        .cursor_pointer()
        .border_b_1()
        .border_color(cx.theme().border.opacity(0.15))
        .hover(|s| s.bg(cx.theme().accent.opacity(0.06)))
        // Icon pill
        .child(
            div()
                .w(px(28.0))
                .h(px(28.0))
                .flex_shrink_0()
                .rounded_full()
                .bg(accent.opacity(0.18))
                .flex()
                .items_center()
                .justify_center()
                .text_base()
                .child(SharedString::from(glyph)),
        )
        // Name + description
        .child(
            v_flex()
                .flex_1()
                .min_w_0()
                .gap_0p5()
                .child(
                    div()
                        .text_xs()
                        .font_weight(gpui::FontWeight::SEMIBOLD)
                        .text_color(cx.theme().foreground)
                        .child(SharedString::from(entry.title.clone())),
                )
                .child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(SharedString::from(entry.description.clone())),
                ),
        )
        .on_mouse_down(
            MouseButton::Left,
            cx.listener(move |_view, _event, _window, cx| {
                cx.emit(PickerEvent::Chosen(definition.clone()));
            }),
        )
}
