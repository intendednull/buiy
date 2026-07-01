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
/// FW1 shipped the four kinds the Counter needs; FW2 adds the two stateful-leaf
/// widgets TodoMVC needs — a real `Checkbox` (its `A11yToggled` leaf IS the
/// model) and a real single-line `TextInput` (the command-sourced editor). The
/// conditional `Empty` placeholder is a later wave.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Column,
    Row,
    Text,
    Button,
    /// A real `buiy_widgets::Checkbox` (stateful leaf: `A11yToggled` is its
    /// model — the reconciler drives it from the controlled `checked` state).
    Checkbox,
    /// A real single-line `buiy_widgets::TextInput` (the command-sourced
    /// editor). Controlled: the reconciler keeps its buffer equal to `value`.
    TextInput,
    /// A zero-paint **placeholder that still occupies a slot** (FW3's
    /// conditional [`when`]). Swapping content↔`Empty` at a fixed child index is
    /// a *kind change* the reconciler despawns + respawns in place, so a hidden
    /// child never shifts its siblings' indices — the positional-churn cure for
    /// `if cond { .. }` show/hide.
    Empty,
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
    /// The typed message a press enqueues. `None` ⇒ inert (or disabled). Also
    /// carries a **checkbox**'s resolved toggle message: [`Element::on_toggle`]
    /// eagerly evaluates `f(!checked)` into this slot, so a checkbox press routes
    /// through the same `PressAction` value path as a button (no stored closure).
    pub(crate) on_press: Option<Msg>,
    pub(crate) children: Vec<Element<Msg>>,

    // --- FW2 additions (keyed lists + the two stateful-leaf widgets) --------
    /// The mandatory reconcile key for a [`keyed_column`] child (FW2's fix for
    /// the silent-`.key()` landmine — set by `keyed_column`, not the author).
    pub(crate) key: Option<u64>,
    /// Whether this container reconciles its children **by key** ([`keyed_column`])
    /// rather than by position (`column!`/`row!`).
    pub(crate) keyed: bool,
    /// A checkbox's controlled checked state (the model is the source of truth;
    /// the reconciler re-asserts the real leaf `A11yToggled` from it).
    pub(crate) checked: bool,
    /// A text-input's controlled value (flows from the model draft into the real
    /// editor buffer via the low-level `apply()` seam, drift-only).
    pub(crate) value: Option<String>,
    /// A text-input's placeholder prompt (shown when the value is empty).
    pub(crate) placeholder: Option<String>,
    /// A text-input's per-keystroke handler: `fn(new_value) -> Msg`. A **bare fn
    /// pointer** (an enum tuple-variant ctor like `Msg::SetDraft` qualifies), so
    /// it is `Copy` / determinism-safe and stored on the entity for the router —
    /// never a captured closure (the replay-safety rule, spec §2). Consumed by
    /// `route_text_input`.
    pub(crate) on_input: Option<fn(String) -> Msg>,
    /// A text-input's submit (Enter) message (a value, like `on_press`).
    pub(crate) on_submit: Option<Msg>,
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
            key: None,
            keyed: false,
            checked: false,
            value: None,
            placeholder: None,
            on_input: None,
            on_submit: None,
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

    /// An inert, zero-paint placeholder that occupies a slot (FW3). See
    /// [`when`]: a conditional renders this instead of removing the child, so
    /// show/hide is a kind-swap at a fixed index rather than a count change that
    /// churns siblings.
    pub fn empty() -> Self {
        Self::new(Kind::Empty)
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

    // --- FW2: checkbox + text-input handlers ------------------------------

    /// A **checkbox**'s toggle handler. `f` maps the *would-be-new* checked state
    /// to a `Msg`. Because a checkbox's toggle target is deterministic given the
    /// current model (`!checked`), `on_toggle` **evaluates `f(!checked)` right
    /// here** and stores the resulting `Msg` value — the closure is consumed at
    /// build time, never stored on an entity, so the route stays a plain
    /// `PressAction` value (replay-safe). This is why `f` may freely **capture**
    /// (e.g. the row id): eager evaluation dissolves the capture. (Contrast
    /// [`Element::on_input`], whose value is only known at keystroke time and so
    /// cannot resolve eagerly — hence its bare-`fn` constraint.)
    pub fn on_toggle(mut self, f: impl FnOnce(bool) -> Msg) -> Self {
        self.on_press = Some(f(!self.checked));
        self.disabled = false;
        self
    }

    /// A **text-input**'s placeholder prompt (shown when the value is empty).
    pub fn placeholder(mut self, s: impl Into<String>) -> Self {
        self.placeholder = Some(s.into());
        self
    }

    /// A **text-input**'s per-keystroke handler: `fn(new_value) -> Msg`. Takes a
    /// **bare fn pointer** (an enum tuple-variant ctor such as `Msg::SetDraft` is
    /// exactly `fn(String) -> Msg`), NOT an `Fn` closure — the runtime value
    /// forbids eager evaluation, so the fn is stored on the entity for the
    /// router; a bare fn cannot capture a `Res` snapshot, so it is
    /// determinism-safe by type (the replay-safety rule, spec §2). A *capturing*
    /// per-row `on_input` needs a boxed handler — deferred to PR3 (#17).
    pub fn on_input(mut self, f: fn(String) -> Msg) -> Self {
        self.on_input = Some(f);
        self
    }

    /// A **text-input**'s submit (Enter) message.
    pub fn on_submit(mut self, msg: Msg) -> Self {
        self.on_submit = Some(msg);
        self
    }

    // --- FW3: message-lifting (the Elm `Html.map` analog, spec §2 #6) ------

    /// **Lift** every message this subtree can emit through `f`, turning a
    /// reusable child component's `Element<Msg>` into the parent's
    /// `Element<Parent>` (`counter::view(&s.left).map(Msg::Left)`). This is how
    /// the surface composes sub-components while keeping ONE parent model + ONE
    /// `view`: the child is held as **parent-owned sub-state** (a field of the
    /// single model), and its `view`/`update` are reused verbatim; the parent
    /// reducer delegates one line (`counter::update(&mut s.left, cm)`). Keeping
    /// all structural truth in the one on-log model is what preserves the
    /// whole-UI-replay property (spec §5) through composition.
    ///
    /// `f` is a **bare fn pointer** — an enum tuple-variant ctor like `Msg::Left`
    /// is exactly `fn(ChildMsg) -> Parent`, so it is `Copy` + determinism-clean
    /// (the same discipline `on_input` uses). It maps the value handlers
    /// (`on_press`, `on_submit`) and recurses into children.
    ///
    /// **Known limit (spec §2, deferred to PR3 #17):** `on_input` is a bare
    /// `fn(String) -> Msg` and CANNOT be composed with `f` into a new bare
    /// `fn(String) -> Parent` (that needs a closure), so a lifted subtree
    /// **drops** `on_input`. Harmless for a button-only child (the Counter —
    /// asserted below in debug); lifting an *input-bearing* child needs a boxed
    /// `Fn` on the element — the P3 residual.
    pub fn map<Parent>(self, f: fn(Msg) -> Parent) -> Element<Parent> {
        debug_assert!(
            self.on_input.is_none(),
            "Element::map cannot lift `on_input` (a bare fn cannot compose into a \
             new bare fn — needs a boxed Fn); lifting an input-bearing child is a \
             PR3 gap (spec §2 #17)"
        );
        Element {
            kind: self.kind,
            text: self.text,
            font_size: self.font_size,
            gap: self.gap,
            padding: self.padding,
            align_center: self.align_center,
            disabled: self.disabled,
            background: self.background,
            radius: self.radius,
            on_press: self.on_press.map(f),
            children: self.children.into_iter().map(|c| c.map(f)).collect(),
            key: self.key,
            keyed: self.keyed,
            checked: self.checked,
            value: self.value,
            placeholder: self.placeholder,
            // A bare fn cannot be re-tagged into a new bare fn (see the doc note).
            on_input: None,
            on_submit: self.on_submit.map(f),
        }
    }
}

/// The `Option<Element>` → [`Kind::Empty`] auto-wrap (spec §2 #5, "defense in
/// depth"). The `column!` / `row!` child slots accept `impl Into<Element<Msg>>`,
/// so a stray `Option<Element>` at a non-terminal position lowers to a
/// **stable-index** `Empty` slot instead of changing the child count and
/// churning every following sibling. `Some(e) ⇒ e`, `None ⇒ Empty`. ([`when`] is
/// the blessed spelling; this is the safety net for a raw `Option`.)
impl<Msg> From<Option<Element<Msg>>> for Element<Msg> {
    fn from(opt: Option<Element<Msg>>) -> Self {
        opt.unwrap_or_else(Element::empty)
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

/// A **checkbox** bound to `checked` (the model is the source of truth — the
/// reconciler drives the real `Checkbox`'s `A11yToggled` from this). Attach a
/// handler with [`Element::on_toggle`]. The box is unlabelled here (put the
/// row's label in a sibling [`text`]).
pub fn checkbox<Msg>(checked: bool) -> Element<Msg> {
    let mut e = Element::new(Kind::Checkbox);
    e.checked = checked;
    e
}

/// A single-line **text input** bound to `value` (controlled — the reconciler
/// keeps the real editor's content equal to this). Attach [`Element::on_input`]
/// / [`Element::on_submit`] / [`Element::placeholder`].
pub fn text_input<Msg>(value: impl Into<String>) -> Element<Msg> {
    let mut e = Element::new(Kind::TextInput);
    e.value = Some(value.into());
    e
}

/// A **conditional slot** (FW3, spec §2 #5): `el` when `cond`, else an
/// [`Element::empty`] placeholder that STILL occupies the position. Because the
/// slot is always present, a show/hide is a content↔`Empty` **kind-swap at a
/// fixed index** — the reconciler despawns the old + spawns the new WITHOUT
/// shifting the siblings after it (contrast a bare absent child, which changes
/// the child count and churns every following sibling under positional
/// reconcile). The author may also write a plain `if cond { a } else { b }`
/// returning two *different-kind* elements at one slot; the reconciler swaps them
/// the same way. `when` is the spelling for the show/*hide* case where there is
/// no natural "else" element.
pub fn when<Msg>(cond: bool, el: Element<Msg>) -> Element<Msg> {
    if cond { el } else { Element::empty() }
}

/// A **required-key** list (FW2's headline builder). Unlike [`column!`](crate::column!),
/// the key is a *mandatory argument* — the deliberate fix for the panel's
/// silent-`.key()` landmine. The reconciler matches rows to elements **by key**,
/// so it spawns new keys, despawns missing keys, and **reorders existing rows
/// without rebuilding them** (preserving each row's widget-entity identity +
/// internal state — the surviving checkbox keeps its `A11yToggled`, the surviving
/// editor keeps its buffer).
///
/// `key_fn` yields a stable `u64` per item; `view_fn` renders one item.
pub fn keyed_column<T, Msg>(
    iter: impl IntoIterator<Item = T>,
    key_fn: impl Fn(&T) -> u64,
    view_fn: impl Fn(&T) -> Element<Msg>,
) -> Element<Msg> {
    let children = iter
        .into_iter()
        .map(|item| {
            let key = key_fn(&item);
            let mut child = view_fn(&item);
            child.key = Some(key);
            child
        })
        .collect();
    let mut e = Element::column(children);
    e.keyed = true;
    e
}

/// `column![a, b, c]` — a vertical container. Each child slot accepts
/// `impl Into<Element>`, so a stray `Option<Element>` auto-wraps to a stable
/// `Empty` slot (spec §2 #5) rather than churning siblings.
#[macro_export]
macro_rules! column {
    ($($child:expr),* $(,)?) => {
        $crate::Element::column(::std::vec![$(::core::convert::Into::into($child)),*])
    };
}

/// `row![a, b, c]` — a horizontal container. Each child slot accepts
/// `impl Into<Element>` (see [`column!`](crate::column!)).
#[macro_export]
macro_rules! row {
    ($($child:expr),* $(,)?) => {
        $crate::Element::row(::std::vec![$(::core::convert::Into::into($child)),*])
    };
}

/// `text!("Count: {}", n)` — a `format!`-ed text node.
#[macro_export]
macro_rules! text {
    ($($arg:tt)*) => {
        $crate::text(::std::format!($($arg)*))
    };
}
