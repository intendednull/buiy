//! [`Element<Msg>`] — the inert description value, its builders, the uniform
//! dot-method modifiers, and the `column!` / `row!` / `text!` macros.
//!
//! An `Element` is a plain value (NOT an entity): a widget [`Kind`], typed
//! props, modifier state, a typed press handler, and children. Built by the
//! builders, consumed by the reconciler ([`crate::reconcile`]). Generic over
//! the app's message type `Msg` so a handler stores a concrete `Msg` value (the
//! replay-safety rule, spec §2).

use bevy::prelude::Component;

use crate::tokens::{Color, Radius, Space};

/// Which retained widget an [`Element`] realizes into. Doubles as the component
/// the reconciler stamps on each spawned entity so it can tell "same kind ⇒
/// patch" from "different kind ⇒ replace".
///
/// PR1 (FW1) ships the four kinds the Counter needs. Stateful-leaf widgets
/// (`Checkbox`/`TextInput`) and the conditional `Empty` placeholder arrive in
/// later waves.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Column,
    Row,
    Text,
    Button,
}

/// An inert description of a piece of UI. Built by the widget builders,
/// consumed by the reconciler.
///
/// The prop set is ported from the validated prototype and trimmed to the FW1
/// surface (containers + text + button), plus the decomposed-style props the
/// reconciler patches in place (`background` / `radius`, spec §3 #9).
pub struct Element<Msg> {
    pub(crate) kind: Kind,
    /// Text content (a `Text` node's string, or a `Button`'s label).
    pub(crate) text: Option<String>,
    pub(crate) font_size: f32,
    /// Gap between children, logical px (containers only).
    pub(crate) gap: Option<f32>,
    /// Inner padding, logical px (containers only).
    pub(crate) padding: Option<f32>,
    /// Center children on the cross axis (containers only).
    pub(crate) align_center: bool,
    /// A disabled interactive element routes nothing and dims.
    pub(crate) disabled: bool,
    /// Background fill token (containers). Lowered to `Background`.
    pub(crate) background: Option<Color>,
    /// Corner radius token (containers). Lowered to the render `Border`.
    pub(crate) radius: Option<Radius>,
    /// The typed message a press enqueues. `None` ⇒ inert (or disabled).
    pub(crate) on_press: Option<Msg>,
    pub(crate) children: Vec<Element<Msg>>,
}

pub(crate) const DEFAULT_TEXT_SIZE: f32 = 24.0;

impl<Msg> Element<Msg> {
    pub(crate) fn new(kind: Kind) -> Self {
        Element {
            kind,
            text: None,
            font_size: DEFAULT_TEXT_SIZE,
            gap: None,
            padding: None,
            align_center: false,
            disabled: false,
            background: None,
            radius: None,
            on_press: None,
            children: Vec::new(),
        }
    }

    /// A vertical container (used by the [`column!`](crate::column!) macro).
    pub fn column(children: Vec<Element<Msg>>) -> Self {
        let mut e = Self::new(Kind::Column);
        e.children = children;
        e
    }

    /// A horizontal container (used by the [`row!`](crate::row!) macro).
    pub fn row(children: Vec<Element<Msg>>) -> Self {
        let mut e = Self::new(Kind::Row);
        e.children = children;
        e
    }

    // --- Uniform dot-method modifiers (spec §2: no method-vs-bare-attribute
    //     split — every modifier is a `.method(..)` returning `Self`). ---

    /// Gap between children (containers only).
    pub fn gap(mut self, s: Space) -> Self {
        self.gap = Some(s.px());
        self
    }

    /// Inner padding (containers only).
    pub fn padding(mut self, s: Space) -> Self {
        self.padding = Some(s.px());
        self
    }

    /// Center children on the cross axis (containers only).
    pub fn align_center(mut self) -> Self {
        self.align_center = true;
        self
    }

    /// Font size in logical px (text only).
    pub fn size(mut self, px: f32) -> Self {
        self.font_size = px;
        self
    }

    /// Background fill (containers). A theme-resolved [`Color`] token.
    pub fn background(mut self, c: Color) -> Self {
        self.background = Some(c);
        self
    }

    /// Corner radius (containers). A [`Radius`] token.
    pub fn radius(mut self, r: Radius) -> Self {
        self.radius = Some(r);
        self
    }

    /// Route this message when pressed.
    pub fn on_press(mut self, msg: Msg) -> Self {
        self.on_press = Some(msg);
        self.disabled = false;
        self
    }

    /// Declarative disable: `Some(msg)` enables + routes it; `None` disables +
    /// dims. The surface's answer to "how do you spell a conditionally-off
    /// action?" (`button("Reset").on_press_maybe((count != 0).then_some(Reset))`).
    pub fn on_press_maybe(mut self, msg: Option<Msg>) -> Self {
        self.disabled = msg.is_none();
        self.on_press = msg;
        self
    }

    /// Force the disabled flag (dims + routes nothing).
    pub fn disabled(mut self, d: bool) -> Self {
        self.disabled = d;
        self
    }
}

/// A text node (the `text(..)` builder; see also the [`text!`](crate::text!)
/// format macro).
pub fn text<Msg>(s: impl Into<String>) -> Element<Msg> {
    let mut e = Element::new(Kind::Text);
    e.text = Some(s.into());
    e
}

/// A labelled button (a real `buiy_widgets::Button`). Attach a handler with
/// `.on_press(Msg)` / `.on_press_maybe(Option<Msg>)`.
pub fn button<Msg>(label: impl Into<String>) -> Element<Msg> {
    let mut e = Element::new(Kind::Button);
    e.text = Some(label.into());
    e
}

/// `column![a, b, c]` — a vertical container.
#[macro_export]
macro_rules! column {
    ($($child:expr),* $(,)?) => {
        $crate::Element::column(::std::vec![$($child),*])
    };
}

/// `row![a, b, c]` — a horizontal container.
#[macro_export]
macro_rules! row {
    ($($child:expr),* $(,)?) => {
        $crate::Element::row(::std::vec![$($child),*])
    };
}

/// `text!("Count: {}", n)` — a `format!`-ed text node.
#[macro_export]
macro_rules! text {
    ($($arg:tt)*) => {
        $crate::text(::std::format!($($arg)*))
    };
}
