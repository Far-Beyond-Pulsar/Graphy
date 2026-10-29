//! The shared node picker and context menu (feature `gpui`).
//!
//! Both are drawn by [`GraphCanvas`](crate::canvas::GraphCanvas) itself, as
//! children of the canvas element positioned in **canvas-relative** pixels, so
//! they open exactly at the pointer (right-click or wire drop) whatever the
//! window offset, pan, zoom or DPI scale. Colours come from the canvas
//! [`Theme`], so the popups match the graph and the host's theme.
//!
//! The picker is a profile-supplied catalogue ([`CatalogEntry`]) with search,
//! foldable categories and, when opened by a dropped wire, only the entries
//! that can take the wire's type. It has its own tiny text field (type,
//! backspace, paste, arrows, enter, escape) so Graphy needs no UI toolkit.

use std::collections::HashSet;
use std::rc::Rc;

use gpui::{
    div, px, uniform_list, App, Context, EventEmitter, FocusHandle, Focusable, Hsla,
    InteractiveElement, IntoElement, KeyDownEvent, MouseButton, ParentElement, Render, Rgba,
    ScrollStrategy, SharedString, Styled, UniformListScrollHandle,
    Window,
};
use graphy_render::{Color, Theme};

use crate::catalog::{matching_count, visible_rows, Row};
use crate::profile::CatalogEntry;

/// Size of the picker popup.
pub const PICKER_SIZE: (f32, f32) = (320.0, 440.0);
/// Width of a context menu.
pub const MENU_WIDTH: f32 = 220.0;
/// Height of one context-menu row.
pub const MENU_ROW_H: f32 = 28.0;
const ROW_H: f32 = 40.0;

pub(crate) fn rgba(c: Color) -> Rgba {
    Rgba {
        r: c[0],
        g: c[1],
        b: c[2],
        a: c[3],
    }
}

pub(crate) fn hsla(c: Color) -> Hsla {
    rgba(c).into()
}

fn with_alpha(c: Color, a: f32) -> Hsla {
    hsla([c[0], c[1], c[2], c[3] * a])
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
    query: String,
    expanded: HashSet<String>,
    /// Index into the visible rows, always an entry row when there is one.
    highlighted: usize,
    /// A list already narrowed to what fits a wire opens every category.
    open_all: bool,
    chip: Option<DragChip>,
    theme: Theme,
    focus: FocusHandle,
    scroll: UniformListScrollHandle,
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
        theme: Theme,
        cx: &mut Context<Self>,
    ) -> Self {
        let mut picker = Self {
            open_all: chip.is_some(),
            entries,
            query: String::new(),
            expanded: HashSet::new(),
            highlighted: 0,
            chip,
            theme,
            focus: cx.focus_handle(),
            scroll: UniformListScrollHandle::new(),
        };
        picker.highlighted = picker.first_entry_row().unwrap_or(0);
        picker
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }

    fn rows(&self) -> Vec<Row> {
        visible_rows(&self.entries, &self.query, &self.expanded, self.open_all)
    }

    fn first_entry_row(&self) -> Option<usize> {
        self.rows().iter().position(|r| matches!(r, Row::Entry(_)))
    }

    fn set_query(&mut self, query: String, cx: &mut Context<Self>) {
        self.query = query;
        self.highlighted = self.first_entry_row().unwrap_or(0);
        self.scroll.scroll_to_item(0, ScrollStrategy::Top);
        cx.notify();
    }

    fn step(&mut self, forward: bool, cx: &mut Context<Self>) {
        let rows = self.rows();
        let entry_rows: Vec<usize> = rows
            .iter()
            .enumerate()
            .filter_map(|(i, r)| matches!(r, Row::Entry(_)).then_some(i))
            .collect();
        if entry_rows.is_empty() {
            return;
        }
        let at = entry_rows.iter().position(|&i| i == self.highlighted);
        let next = match (at, forward) {
            (Some(i), true) => (i + 1).min(entry_rows.len() - 1),
            (Some(i), false) => i.saturating_sub(1),
            (None, _) => 0,
        };
        self.highlighted = entry_rows[next];
        self.scroll
            .scroll_to_item(self.highlighted, ScrollStrategy::Nearest);
        cx.notify();
    }

    fn choose_highlighted(&mut self, cx: &mut Context<Self>) {
        let rows = self.rows();
        let row = rows
            .get(self.highlighted)
            .filter(|r| matches!(r, Row::Entry(_)))
            .or_else(|| rows.iter().find(|r| matches!(r, Row::Entry(_))));
        if let Some(Row::Entry(i)) = row {
            cx.emit(PickerEvent::Chosen(self.entries[*i].definition.clone()));
        }
    }

    fn toggle(&mut self, category: &str, cx: &mut Context<Self>) {
        if !self.expanded.remove(category) {
            self.expanded.insert(category.to_string());
        }
        cx.notify();
    }

    fn on_key(&mut self, ev: &KeyDownEvent, cx: &mut Context<Self>) {
        let k = &ev.keystroke;
        let command = k.modifiers.control || k.modifiers.platform;
        match k.key.as_str() {
            "escape" => cx.emit(PickerEvent::Dismissed),
            "enter" => self.choose_highlighted(cx),
            "down" => self.step(true, cx),
            "up" => self.step(false, cx),
            "backspace" => {
                let mut q = self.query.clone();
                q.pop();
                self.set_query(q, cx);
            }
            "v" if command => {
                if let Some(text) = cx.read_from_clipboard().and_then(|i| i.text()) {
                    let line: String = text.lines().next().unwrap_or_default().to_string();
                    self.set_query(format!("{}{line}", self.query), cx);
                }
            }
            _ if !command && !k.modifiers.alt => {
                if let Some(ch) = k.key_char.as_deref().filter(|c| !c.chars().any(char::is_control))
                {
                    self.set_query(format!("{}{ch}", self.query), cx);
                }
            }
            _ => {}
        }
        cx.stop_propagation();
    }
}

/// An owned snapshot of one row, so the list closure needs no access to the view.
enum RowView {
    Header {
        name: String,
        color: Color,
        count: String,
        open: bool,
    },
    Entry {
        definition: String,
        title: String,
        description: String,
        icon: String,
        color: Color,
        highlighted: bool,
    },
}

impl Render for NodePicker {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let theme = self.theme.clone();
        let rows = self.rows();
        let total = matching_count(&self.entries, &self.query);
        let accent = |c: Option<Color>| c.unwrap_or(theme.focus);

        let views: Rc<Vec<RowView>> = Rc::new(
            rows.iter()
                .enumerate()
                .map(|(ix, row)| match row {
                    Row::Header {
                        name,
                        color,
                        count,
                        open,
                        matched,
                    } => RowView::Header {
                        name: name.clone(),
                        color: accent(*color),
                        count: match matched {
                            Some(m) if *m < *count => format!("{m} / {count}"),
                            _ => count.to_string(),
                        },
                        open: *open,
                    },
                    Row::Entry(i) => {
                        let e = &self.entries[*i];
                        RowView::Entry {
                            definition: e.definition.clone(),
                            title: e.title.clone(),
                            description: e.description.clone(),
                            icon: e.icon.clone(),
                            color: accent(e.color),
                            highlighted: ix == self.highlighted,
                        }
                    }
                })
                .collect(),
        );

        let this = cx.entity();
        let list = {
            let theme = theme.clone();
            let views = views.clone();
            uniform_list("graphy-picker-list", views.len(), move |range, _, _| {
                range
                    .map(|ix| match &views[ix] {
                        RowView::Header {
                            name,
                            color,
                            count,
                            open,
                        } => {
                            let name_key = name.clone();
                            let this = this.clone();
                            div()
                                .id(("graphy-picker-header", ix))
                                .h(px(ROW_H * 0.7))
                                .w_full()
                                .flex()
                                .items_center()
                                .gap_2()
                                .bg(hsla(theme.card_header))
                                .border_b_1()
                                .border_color(with_alpha(theme.card_border, 0.5))
                                .cursor_pointer()
                                .hover(|s| s.bg(with_alpha(theme.focus, 0.15)))
                                .child(div().w(px(3.0)).h_full().bg(hsla(*color)))
                                .child(
                                    div()
                                        .w(px(12.0))
                                        .text_xs()
                                        .text_color(hsla(theme.text_dim))
                                        .child(if *open { "v" } else { ">" }),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .text_xs()
                                        .font_weight(gpui::FontWeight::SEMIBOLD)
                                        .text_color(hsla(theme.text))
                                        .child(SharedString::from(name.to_uppercase())),
                                )
                                .child(
                                    div()
                                        .pr_3()
                                        .text_xs()
                                        .text_color(hsla(theme.text_dim))
                                        .child(SharedString::from(count.clone())),
                                )
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    this.update(cx, |p, cx| p.toggle(&name_key, cx));
                                })
                                .into_any_element()
                        }
                        RowView::Entry {
                            definition,
                            title,
                            description,
                            icon,
                            color,
                            highlighted,
                        } => {
                            let definition = definition.clone();
                            let this = this.clone();
                            div()
                                .id(("graphy-picker-entry", ix))
                                .h(px(ROW_H))
                                .w_full()
                                .px_3()
                                .flex()
                                .items_center()
                                .gap_2()
                                .cursor_pointer()
                                .when_highlighted(*highlighted, theme.focus)
                                .hover(|s| s.bg(with_alpha(theme.focus, 0.18)))
                                .child(
                                    div()
                                        .w(px(26.0))
                                        .h(px(26.0))
                                        .flex_shrink_0()
                                        .rounded_full()
                                        .bg(with_alpha(*color, 0.22))
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_color(hsla(*color))
                                        .text_sm()
                                        .child(SharedString::from(if icon.is_empty() {
                                            title.chars().next().map(String::from).unwrap_or_default()
                                        } else {
                                            icon.clone()
                                        })),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .overflow_hidden()
                                        .child(
                                            div()
                                                .text_xs()
                                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                                .text_color(hsla(theme.text))
                                                .child(SharedString::from(title.clone())),
                                        )
                                        .child(
                                            div()
                                                .text_xs()
                                                .text_color(hsla(theme.text_dim))
                                                .whitespace_nowrap()
                                                .overflow_hidden()
                                                .child(SharedString::from(description.clone())),
                                        ),
                                )
                                .on_mouse_down(MouseButton::Left, move |_, _, cx| {
                                    let definition = definition.clone();
                                    this.update(cx, |_, cx| cx.emit(PickerEvent::Chosen(definition)));
                                })
                                .into_any_element()
                        }
                    })
                    .collect()
            })
            .track_scroll(&self.scroll)
            .flex_1()
            .min_h_0()
        };

        let chip = self.chip.clone().map(|chip| {
            let color = chip.color.unwrap_or(theme.focus);
            div()
                .px_1p5()
                .rounded_sm()
                .bg(with_alpha(color, 0.2))
                .border_1()
                .border_color(with_alpha(color, 0.6))
                .text_xs()
                .text_color(hsla(theme.text))
                .child(SharedString::from(chip.label))
        });
        let query_text: SharedString = if self.query.is_empty() {
            "Search nodes…".into()
        } else {
            format!("{}\u{258F}", self.query).into()
        };

        div()
            .size_full()
            .flex()
            .flex_col()
            .track_focus(&self.focus)
            .key_context("GraphyPicker")
            .on_key_down(cx.listener(|p, ev: &KeyDownEvent, _, cx| p.on_key(ev, cx)))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .px_3()
                    .py_2()
                    .bg(hsla(theme.card_header))
                    .border_b_1()
                    .border_color(hsla(theme.card_border))
                    .child(
                        div()
                            .text_sm()
                            .font_weight(gpui::FontWeight::SEMIBOLD)
                            .text_color(hsla(theme.text))
                            .child("Add node"),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .children(chip)
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(hsla(theme.text_dim))
                                    .child(SharedString::from(format!("{total} nodes"))),
                            ),
                    ),
            )
            .child(
                div()
                    .px_2()
                    .py_1p5()
                    .border_b_1()
                    .border_color(with_alpha(theme.card_border, 0.6))
                    .child(
                        div()
                            .px_2()
                            .py_1()
                            .rounded_sm()
                            .bg(hsla(theme.card_body))
                            .border_1()
                            .border_color(if self.focus.is_focused(window) {
                                hsla(theme.focus)
                            } else {
                                hsla(theme.card_border)
                            })
                            .text_sm()
                            .text_color(if self.query.is_empty() {
                                hsla(theme.text_faint)
                            } else {
                                hsla(theme.text)
                            })
                            .child(query_text),
                    ),
            )
            .child(list)
    }
}

/// `when_highlighted` keeps the row builder chain readable.
trait HighlightExt: Styled + Sized {
    fn when_highlighted(self, on: bool, color: Color) -> Self {
        if on {
            self.bg(with_alpha(color, 0.22))
        } else {
            self
        }
    }
}

impl<T: Styled + Sized> HighlightExt for T {}
