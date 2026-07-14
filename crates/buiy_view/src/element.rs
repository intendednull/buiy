//! [`Element<Msg>`] — the inert description value, its builders, the uniform
//! dot-method modifiers, and the `column!` / `row!` / `text!` macros.
//!
//! An `Element` is a plain value (NOT an entity): a widget [`Kind`], typed
//! props, modifier state (its whole [`LayoutProps`] layout surface, spec §2.2),
//! a typed press handler, and children. Built by the builders, consumed by the
//! reconciler ([`crate::reconcile`]). Generic over the app's message type `Msg`
//! so a handler stores a concrete `Msg` value (the replay-safety rule, spec §2).

use std::sync::Arc;

use bevy::asset::Handle;
use bevy::image::Image;
use bevy::prelude::Component;
use buiy_core::render::components::LineStyle;
use buiy_core::text::{FamilyEntry, FontStack, GenericFamily};

use crate::layout::LayoutProps;
use crate::tokens::{Color, Radius, Weight};

/// The `viewBox` default for the [`icon`] element (mirrors the render
/// `ICON_VIEWBOX`) — an app authoring on the widget-catalog 24×24 space.
pub const ICON_VIEWBOX: f32 = 24.0;

/// One box-shadow term authored by [`Element::shadow`] (F3): offset, blur,
/// spread (logical px) + [`Color`]. Chains front-to-back in CSS paint order;
/// lowered to `buiy_core::render::components::BoxShadow`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ShadowSpec {
    pub dx: f32,
    pub dy: f32,
    pub blur: f32,
    pub spread: f32,
    pub color: Color,
}

/// A text-input's per-keystroke handler — `value → Msg` (design §2, #13/#17).
///
/// Two forms, both replay-safe by construction (the recorded thing is the *result* `Msg`, not
/// the handler):
/// - [`Bare`](InputHandler::Bare) — a bare `fn(String) -> Msg` (an enum tuple-variant ctor like
///   `Msg::SetDraft` is exactly this). Cannot capture, so it is determinism-clean by type; the
///   default via [`Element::on_input`].
/// - [`Boxed`](InputHandler::Boxed) — an `Arc<dyn Fn>` for a **capturing** per-row handler
///   (`move |s| Msg::Edit(id, s)` — the inline-edit case, #17), via [`Element::on_input_with`].
///
/// **Purity contract (the author's, not statically enforced — #17).** A boxed handler MUST be
/// pure: capture only *values* (a row id, an index), never a `Res`/`World`/clock/RNG snapshot
/// that would diverge on a fresh-process replay. The bare form is pure by type; the boxed form
/// trusts the caller (mirroring the reducer's purity contract). Capturing a plain id — the whole
/// motivating case — satisfies it.
pub(crate) enum InputHandler<Msg> {
    /// A non-capturing `fn(String) -> Msg` (the replay-safe-by-type default).
    Bare(fn(String) -> Msg),
    /// A capturing `Fn(String) -> Msg` (the opt-in, author-purity-checked #17 form).
    Boxed(Arc<dyn Fn(String) -> Msg + Send + Sync>),
}

// Manual `Clone`: `Bare` is `Copy`, `Boxed` clones the `Arc` (cheap). Derive would demand
// `Msg: Clone`, which is wrong — the handler produces `Msg`, it does not hold one.
impl<Msg> Clone for InputHandler<Msg> {
    fn clone(&self) -> Self {
        match self {
            InputHandler::Bare(f) => InputHandler::Bare(*f),
            InputHandler::Boxed(f) => InputHandler::Boxed(f.clone()),
        }
    }
}

impl<Msg: 'static> InputHandler<Msg> {
    /// Apply the handler to the editor's live value.
    pub(crate) fn call(&self, value: String) -> Msg {
        match self {
            InputHandler::Bare(f) => f(value),
            InputHandler::Boxed(f) => f(value),
        }
    }

    /// Lift the produced message through `f` — the `on_input` half of [`Element::map`]. A bare
    /// handler cannot compose into a *new bare* fn (that needs a closure), so it lifts by
    /// **boxing** (`Boxed(move |s| f(bare(s)))`); a boxed handler composes its `Arc`. So a lifted
    /// input-bearing child no longer DROPS `on_input` (the P1 limitation #15/#17 removed).
    pub(crate) fn map<Parent: 'static>(self, f: fn(Msg) -> Parent) -> InputHandler<Parent> {
        match self {
            InputHandler::Bare(bare) => InputHandler::Boxed(Arc::new(move |s| f(bare(s)))),
            InputHandler::Boxed(boxed) => InputHandler::Boxed(Arc::new(move |s| f(boxed(s)))),
        }
    }
}

/// A text-input's **submit** (Enter) handler — two shapes sharing one field (F7, spec §2.8):
///
/// - [`Static`](SubmitHandler::Static) — a fixed `Msg` value that ignores the submitted text
///   ([`Element::on_submit`], the original form). The submitted text goes unread, exactly as
///   before (byte-identical routing).
/// - [`Capturing`](SubmitHandler::Capturing) — folds the **submitted text** into a message
///   ([`Element::on_submit_with`]). This deletes the two-message dance an
///   `on_input → SetDraft → on_submit → Submit` round-trip needed just to carry the value:
///   the submit reads the editor's live value directly. Reuses [`InputHandler`] (bare or
///   boxed), so it is replay-safe by the same rule and lifts through [`Element::map`].
pub(crate) enum SubmitHandler<Msg> {
    /// `on_submit(msg)` — the submitted text is ignored.
    Static(Msg),
    /// `on_submit_with(f)` — the submitted text is folded into the message.
    Capturing(InputHandler<Msg>),
}

// Manual `Clone`: `Static` clones the held `Msg` (so this bound is `Msg: Clone`, which every
// `M::Msg` satisfies — the `Model` trait requires it); `Capturing` clones its `InputHandler`
// (a `Copy` fn or a cheap `Arc`).
impl<Msg: Clone> Clone for SubmitHandler<Msg> {
    fn clone(&self) -> Self {
        match self {
            SubmitHandler::Static(m) => SubmitHandler::Static(m.clone()),
            SubmitHandler::Capturing(h) => SubmitHandler::Capturing(h.clone()),
        }
    }
}

impl<Msg: 'static> SubmitHandler<Msg> {
    /// Lift the produced message through `f` — the `on_submit` half of [`Element::map`]. No
    /// `Clone` bound (matching [`Element::map`]): `Static` moves its `Msg` through `f`,
    /// `Capturing` composes its [`InputHandler`].
    pub(crate) fn map<Parent: 'static>(self, f: fn(Msg) -> Parent) -> SubmitHandler<Parent> {
        match self {
            SubmitHandler::Static(m) => SubmitHandler::Static(f(m)),
            SubmitHandler::Capturing(h) => SubmitHandler::Capturing(h.map(f)),
        }
    }
}

impl<Msg: Clone + 'static> SubmitHandler<Msg> {
    /// Resolve the message a submit enqueues from the editor's live `value` (ignored by
    /// [`Static`](SubmitHandler::Static), folded in by [`Capturing`](SubmitHandler::Capturing)).
    pub(crate) fn resolve(&self, value: String) -> Msg {
        match self {
            SubmitHandler::Static(m) => m.clone(),
            SubmitHandler::Capturing(h) => h.call(value),
        }
    }
}

/// Which retained widget an [`Element`] realizes into. Doubles as the component
/// the reconciler stamps on each spawned entity so it can tell "same kind ⇒
/// patch" from "different kind ⇒ replace".
///
/// FW1 shipped the four kinds the Counter needs; FW2 adds the two stateful-leaf
/// widgets TodoMVC needs — a real `Checkbox` (its `A11yToggled` leaf IS the
/// model) and a real single-line `TextInput` (the command-sourced editor). The
/// conditional `Empty` placeholder is a later wave; F2 adds `Raster` (the
/// texture-presenting drawing-canvas node).
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
    /// A **raster (drawing-canvas)** node carrying `buiy_core`'s `RasterImage`
    /// (F1's textured-quad primitive), sized by `.width`/`.height` (F2). The
    /// reconciler patches its `Handle<Image>` **by identity**, preserving the
    /// entity so an unrelated re-render never drops the canvas texture. Authored
    /// via [`raster`].
    Raster,
    /// A **vector icon** node carrying `buiy_core`'s `Icon` (SVG-path → lyon
    /// coverage). Authored via [`icon`]; the reconciler patches its path / size /
    /// stroke / viewBox / color in place. The SAME node can also carry a
    /// `.background()` + `.radius()` + `.width`/`.height`, so ONE icon node paints
    /// a tinted circular badge with the doodle stroked centered over it (the fill
    /// quad is below the icon coverage — Dooduel's doodle avatars). F3.
    Icon,
}

/// An inert description of a piece of UI. Built by the widget builders,
/// consumed by the reconciler.
///
/// The whole layout / positioning / scroll surface lives in `LayoutProps`
/// (spec §2.2), reached via the `.width`/`.grow`/`.padding`/`.fixed`/`.scroll_y`
/// … modifiers in the `layout` module; the reconciler lowers it into the
/// decomposed layout components. Text / handler / style props stay here.
pub struct Element<Msg> {
    pub(crate) kind: Kind,
    /// Text content (a `Text` node's string, or a `Button`'s label).
    pub(crate) text: Option<String>,
    pub(crate) font_size: f32,
    /// The whole layout / positioning / scroll state (spec §2.2). Its Default is
    /// a no-op against a freshly-`#[require]`'d `Node`, so a node with no layout
    /// modifier lowers byte-identically.
    pub(crate) layout: LayoutProps,
    /// A disabled interactive element routes nothing and dims.
    pub(crate) disabled: bool,
    /// Background fill token (containers). Lowered to `Background`.
    pub(crate) background: Option<Color>,
    /// Declarative `:hover`/`:active` fill (Track D). `Some` ⇒ the reconciler
    /// stamps a `HoverStyle` so the runtime paints this token while the node is
    /// hovered or pressed. Pure `Element` intent (replay-safe); the resolved fill
    /// is runtime-only. Background only in v1; inert on a non-pressable node.
    /// Lowered via `update_hover_style`.
    pub(crate) hover_background: Option<Color>,
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
    pub(crate) on_input: Option<InputHandler<Msg>>,
    /// A text-input's submit (Enter) handler — a static message
    /// ([`Element::on_submit`]) or a capturing fold of the submitted text
    /// ([`Element::on_submit_with`]). Consumed by `route_text_submit`.
    pub(crate) on_submit: Option<SubmitHandler<Msg>>,

    // --- F2 addition (the raster / drawing-canvas element) ------------------
    /// A [`Kind::Raster`] node's source image (authored via [`raster`]). Lowered
    /// to `buiy_core::render::RasterImage`; patched **by identity** (entity
    /// preserved) on change, so an unrelated fold never re-uploads the texture.
    pub(crate) raster: Option<Handle<Image>>,

    // --- F3 styling additions (color / type / outline / rounding / shadow / icon)
    /// Explicit foreground [`Color`] for a `Text` node / `Button` label / `Icon`
    /// (`.color(..)`). `None` ⇒ the theme default ink. Lowered to `TextColor`
    /// (text/label) or `Icon.color` (icon).
    pub(crate) color: Option<Color>,
    /// Explicit font family for a `Text` node / `Button` label (`.font("Caveat")`).
    /// `None` ⇒ the default sans. Lowered to `FontFamily`.
    pub(crate) font_family: Option<FontStack>,
    /// Explicit font [`Weight`] for a `Text` node / `Button` label (`.weight(..)`)
    /// — the variable-font weight axis. `None` ⇒ the family's default instance.
    /// Lowered to `buiy_core::text::FontWeight`.
    pub(crate) font_weight: Option<Weight>,
    /// A uniform border `(width_px, color, style)` on any painting node
    /// (`.border(w, c, style)`), lowered to `BoxModel.border` (width) + a 4-side
    /// `Border` (color + style + the `.radius`/`.radius_corners` corners). The
    /// `style` makes dashed *requestable* (its rasterization is F4b). `None` ⇒
    /// no border.
    pub(crate) border: Option<(f32, Color, LineStyle)>,
    /// Explicit per-corner radius `(tl, tr, br, bl)` in logical px
    /// (`.radius_corners(..)`, the design's asymmetric wobble). Takes precedence
    /// over the uniform `radius` token when set. `None` ⇒ use `radius`.
    pub(crate) radius_corners: Option<[f32; 4]>,
    /// Box-shadow terms (`.shadow(..)`), front-to-back in CSS paint order. Empty
    /// ⇒ no shadow. Lowered to `BoxShadow`.
    pub(crate) shadows: Vec<ShadowSpec>,
    /// A [`Kind::Icon`] node's SVG path `d` (authored on the `icon_viewbox`
    /// space). Lowered to `Icon.path_d`.
    pub(crate) icon_path: Option<String>,
    /// A [`Kind::Icon`]'s stroke width in `icon_viewbox` units (lowered to
    /// `Icon.stroke_width`; icons here are always stroked, round cap/join).
    pub(crate) icon_stroke_width: f32,
    /// A [`Kind::Icon`]'s render size in logical px (lowered to `Icon.size_px`;
    /// the doodle paints centered in the node's box at this size).
    pub(crate) icon_size_px: u16,
    /// A [`Kind::Icon`]'s author viewBox extent (lowered to `Icon.viewbox`; the
    /// coordinate space `icon_path` + `icon_stroke_width` are drawn in).
    pub(crate) icon_viewbox: f32,
}

pub(crate) const DEFAULT_TEXT_SIZE: f32 = 24.0;

impl<Msg> Element<Msg> {
    pub(crate) fn new(kind: Kind) -> Self {
        Element {
            kind,
            text: None,
            font_size: DEFAULT_TEXT_SIZE,
            layout: LayoutProps::default(),
            disabled: false,
            background: None,
            hover_background: None,
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
            raster: None,
            color: None,
            font_family: None,
            font_weight: None,
            border: None,
            radius_corners: None,
            shadows: Vec::new(),
            icon_path: None,
            icon_stroke_width: 0.0,
            icon_size_px: 0,
            icon_viewbox: ICON_VIEWBOX,
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
    //     split — every modifier is a `.method(..)` returning `Self`). The whole
    //     LAYOUT surface (sizing / flex / spacing / positioning / scroll) lives
    //     in `crate::layout`; the text / style / handler modifiers are here. ---

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

    /// Declarative `:hover`/`:active` background fill (Track D) — the node paints
    /// [`Color`] `c` while the pointer is over it **or** it is pressed, and reverts
    /// to its resting fill (its [`background`](Self::background), or the widget's
    /// own default) otherwise. A flat modifier mirroring [`background`](Self::background);
    /// the runtime applies it from the interaction state, never the model, so it
    /// stays out of the pure-view replay log.
    ///
    /// **v1 scope:** background only, and **pressable-only** — inert on a node
    /// without an [`on_press`](Self::on_press) handler (the same install scope as
    /// the press-down visual). `:active` folds into this same fill; the existing
    /// press-down depth is the distinct pressed look.
    pub fn hover_bg(mut self, c: Color) -> Self {
        self.hover_background = Some(c);
        self
    }

    /// Corner radius (containers). A [`Radius`] token.
    pub fn radius(mut self, r: Radius) -> Self {
        self.radius = Some(r);
        self
    }

    /// Explicit per-corner radius `(tl, tr, br, bl)` in logical px (F3) — the
    /// design's asymmetric "wobble". Takes precedence over the uniform
    /// [`radius`](Self::radius) token.
    pub fn radius_corners(mut self, tl: f32, tr: f32, br: f32, bl: f32) -> Self {
        self.radius_corners = Some([tl, tr, br, bl]);
        self
    }

    /// Explicit foreground [`Color`] for a `Text` node, a `Button`'s label, or an
    /// `Icon` (F3). Without it text renders in the theme default ink — so this is
    /// what expresses an accent eyebrow, a muted caption, a white on-accent button
    /// label, or a status-green score. Lowered to `TextColor` (text/label) or
    /// `Icon.color` (icon).
    pub fn color(mut self, c: Color) -> Self {
        self.color = Some(c);
        self
    }

    /// Select a font **family** by name for a `Text` node or a `Button` label
    /// (`.font("Caveat")`, F3), with a sans-serif fallback. The family must be
    /// registered (see `FontRegistry`) and equal the font's internal family name.
    /// Lowered to `FontFamily`; unset ⇒ the default sans.
    pub fn font(mut self, family: impl Into<String>) -> Self {
        self.font_family = Some(FontStack(vec![
            FamilyEntry::Named(family.into()),
            FamilyEntry::Generic(GenericFamily::SansSerif),
        ]));
        self
    }

    /// Select a font [`Weight`] for a `Text` node or a `Button` label (F3) — the
    /// variable-font weight axis. Unset ⇒ the family's default instance. Lowered
    /// to `buiy_core::text::FontWeight`.
    pub fn weight(mut self, w: Weight) -> Self {
        self.font_weight = Some(w);
        self
    }

    /// A uniform border of `width` logical px in [`Color`] `c` drawn with
    /// [`LineStyle`] `style` (F3). The `style` makes dashed / dotted borders
    /// *requestable* from the view (the design's room-code box, join input);
    /// their rasterization lands in F4b — a `Dashed` request renders solid until
    /// then. Pair with `.radius(..)` / `.radius_corners(..)` for a rounded outline.
    pub fn border(mut self, width: f32, c: Color, style: LineStyle) -> Self {
        self.border = Some((width, c, style));
        self
    }

    /// Append a box-shadow term `(dx, dy, blur, spread)` logical px in [`Color`]
    /// `c` (F3). Chains front-to-back in CSS paint order (the first `.shadow`
    /// paints on top) — the ambient card shadow + the 3D-press underside stack by
    /// repeated calls. Lowered to `BoxShadow`.
    pub fn shadow(mut self, dx: f32, dy: f32, blur: f32, spread: f32, c: Color) -> Self {
        self.shadows.push(ShadowSpec {
            dx,
            dy,
            blur,
            spread,
            color: c,
        });
        self
    }

    /// Route this message when pressed.
    pub fn on_press(mut self, msg: Msg) -> Self {
        self.on_press = Some(msg);
        // NB: do NOT reset `disabled` here — attaching a handler must not silently
        // re-enable a button an author explicitly `.disabled(true)`'d (a fresh
        // Element is enabled by default, so the reset only ever clobbered an
        // intentional disable, contradicting the `disabled` builder's contract).
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

    /// The accessible **name** for a pressable non-text node — a clickable
    /// container or a pressable [`raster`] (Dooduel's pick-word tiles, the
    /// custom-avatar seat chip; and, once F3 lands the element, a clickable
    /// `icon`). Applied by the reconciler only when the node also carries an
    /// [`on_press`](Element::on_press): it stamps the `A11yLabel` alongside the
    /// activatable `A11yRole::Button`, so the node is locatable by role+name
    /// (probe `get_by_role(Button, name)`) and announced to a screen reader.
    ///
    /// Reuses the `text` slot (unused for painting on a container / raster). Inert
    /// on a widget that owns its own name — a `button`'s label is already its
    /// `text`, a `text` node names itself — and on any node with no `on_press`.
    pub fn label(mut self, name: impl Into<String>) -> Self {
        self.text = Some(name.into());
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
    /// determinism-safe by type (the replay-safety rule, spec §2). For a
    /// *capturing* per-row handler (inline edit) use [`Element::on_input_with`] (#17).
    pub fn on_input(mut self, f: fn(String) -> Msg) -> Self {
        self.on_input = Some(InputHandler::Bare(f));
        self
    }

    /// A **capturing** per-keystroke handler: `Fn(new_value) -> Msg` that may close over values
    /// (e.g. a row id — `on_input_with(move |s| Msg::EditTitle(id, s))`), the inline-edit case
    /// the bare [`on_input`](Element::on_input) can't express (#17). Stored boxed (`Arc<dyn Fn>`).
    ///
    /// **Purity is the author's contract** (not statically enforced): the closure must capture
    /// only *values*, never a `Res`/clock/RNG snapshot that would diverge on a fresh-process
    /// replay — mirroring the reducer's purity rule. The recorded thing is the produced `Msg`, so
    /// a pure handler stays replay-clean; capturing a plain id (the motivating case) is pure.
    pub fn on_input_with(mut self, f: impl Fn(String) -> Msg + Send + Sync + 'static) -> Self {
        self.on_input = Some(InputHandler::Boxed(Arc::new(f)));
        self
    }

    /// A **text-input**'s submit (Enter) message — a fixed value that ignores the
    /// submitted text (the counterpart of [`on_press`](Element::on_press)).
    pub fn on_submit(mut self, msg: Msg) -> Self {
        self.on_submit = Some(SubmitHandler::Static(msg));
        self
    }

    /// A **text-input**'s **capturing** submit handler: `fn(submitted_text) -> Msg`. Folds the
    /// editor's live value directly into the message on Enter, deleting the two-message dance a
    /// static [`on_submit`](Element::on_submit) needs when it wants the typed text (the
    /// `on_input → SetDraft → on_submit → Submit` round-trip through a model field). A **bare fn**
    /// (an enum tuple-variant ctor like `Msg::SubmitGuess` is exactly `fn(String) -> Msg`), so it
    /// is `Copy` / determinism-safe by type — the same replay-safety rule as
    /// [`on_input`](Element::on_input). (Contrast the value-capturing inline-edit case, which
    /// [`on_input_with`](Element::on_input_with) covers with a boxed closure.)
    pub fn on_submit_with(mut self, f: fn(String) -> Msg) -> Self {
        self.on_submit = Some(SubmitHandler::Capturing(InputHandler::Bare(f)));
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
    /// (`on_press`, `on_submit`), lifts `on_input` (see below), and recurses into
    /// children. The layout / style props are pure data and pass through verbatim.
    ///
    /// **`on_input` is now lifted (was the P1 drop-limitation, closed by #17).** A bare
    /// `fn(String) -> Msg` can't compose into a new *bare* `fn(String) -> Parent`, so it lifts by
    /// **boxing** (`InputHandler::map` → `Boxed(move |s| f(bare(s)))`); a boxed handler composes
    /// its `Arc`. So lifting an *input-bearing* child (an inline-edit row via `on_input_with`)
    /// preserves its input handler — the residual gap the prototype flagged is gone.
    pub fn map<Parent>(self, f: fn(Msg) -> Parent) -> Element<Parent>
    where
        Msg: 'static,
        Parent: 'static,
    {
        Element {
            kind: self.kind,
            text: self.text,
            font_size: self.font_size,
            layout: self.layout,
            disabled: self.disabled,
            background: self.background,
            hover_background: self.hover_background,
            radius: self.radius,
            on_press: self.on_press.map(f),
            children: self.children.into_iter().map(|c| c.map(f)).collect(),
            key: self.key,
            keyed: self.keyed,
            checked: self.checked,
            value: self.value,
            placeholder: self.placeholder,
            // Lift `on_input` by boxing (see the doc note — #17 closed the drop-limitation).
            on_input: self.on_input.map(|h| h.map(f)),
            // Lift `on_submit` through its handler (`Static` maps the value, `Capturing` the fn).
            on_submit: self.on_submit.map(|h| h.map(f)),
            raster: self.raster,
            color: self.color,
            font_family: self.font_family,
            font_weight: self.font_weight,
            border: self.border,
            radius_corners: self.radius_corners,
            shadows: self.shadows,
            icon_path: self.icon_path,
            icon_stroke_width: self.icon_stroke_width,
            icon_size_px: self.icon_size_px,
            icon_viewbox: self.icon_viewbox,
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

/// A **raster (drawing-canvas)** node sampling `handle`, sized `width`×`height`
/// logical px (F2). Reconciles to a `Node` carrying `buiy_core`'s `RasterImage`
/// (F1's textured-quad primitive), so a drawing canvas can live INSIDE a `view`
/// tree — not as a hand-spawned side root. The `Handle<Image>` patches **in
/// place, by identity**, and the entity is preserved across unrelated re-renders,
/// so the canvas never loses its GPU texture on a model change elsewhere. The app
/// owns + paints the image; this element only places + samples it.
///
/// Fixed size is mandatory (a canvas maps window px → texel 1:1); the two args set
/// `.width`/`.height` so the caller cannot forget them. It also defaults
/// `.shrink(false)` so a tight `.fill()`/`.grow()` flex parent cannot squish it.
pub fn raster<Msg>(handle: Handle<Image>, width: f32, height: f32) -> Element<Msg> {
    let mut e = Element::new(Kind::Raster);
    e.raster = Some(handle);
    e.layout.width = Some(width);
    e.layout.height = Some(height);
    // A canvas is fixed-size (window px → texel); never let a tight flex parent
    // squish it below its size (the canvas-squish finding — a `.fill()`/`.grow()`
    // parent shrank the 450px canvas). Pin shrink off by construction.
    e.layout.shrink = 0.0;
    e
}

/// A **vector icon** (F3): the SVG path `d` (multi-subpath), authored on a
/// `viewbox`×`viewbox` coordinate space, rendered at `size_px` logical px with a
/// round-cap/join stroke of `stroke_width` (in viewBox units). Defaults to the
/// theme ink; set the stroke color with [`Element::color`].
///
/// Reconciles to a `Node` carrying `buiy_core`'s `Icon`, painted CENTERED in the
/// node's box. Give the node a `.width`/`.height` + `.background()` + `.radius()`
/// to make it a tinted circular badge with the doodle stroked on top (the
/// doodle-avatar idiom); leave those off for a bare glyph. The `viewbox` arg lets
/// an app author on its own design viewBox (e.g. 40×40) with no per-path
/// pre-scale — pass [`ICON_VIEWBOX`] (24.0) for the widget-catalog space.
pub fn icon<Msg>(
    path_d: impl Into<String>,
    size_px: u16,
    stroke_width: f32,
    viewbox: f32,
) -> Element<Msg> {
    let mut e = Element::new(Kind::Icon);
    e.icon_path = Some(path_d.into());
    e.icon_size_px = size_px;
    e.icon_stroke_width = stroke_width;
    e.icon_viewbox = viewbox;
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

/// A **vertically-scrolling column** (F2) — `column!` children in a container
/// that scrolls when they overflow (`.scroll_y()`). Chat / scoreboard panes.
/// Add `.stick_to_bottom()` for the controlled pin-to-bottom-on-append.
pub fn scroll_column<Msg>(children: Vec<Element<Msg>>) -> Element<Msg> {
    Element::column(children).scroll_y()
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

#[cfg(test)]
mod submit_handler_tests {
    use super::{Element, InputHandler, SubmitHandler};

    #[derive(Clone, Debug, PartialEq)]
    enum Msg {
        Add,
        Submit(String),
    }
    #[derive(Clone, Debug, PartialEq)]
    enum Parent {
        Child(Msg),
    }

    #[test]
    fn static_ignores_the_value_capturing_folds_it() {
        let stat: SubmitHandler<Msg> = SubmitHandler::Static(Msg::Add);
        assert_eq!(stat.resolve("ignored".into()), Msg::Add);

        let cap: SubmitHandler<Msg> = SubmitHandler::Capturing(InputHandler::Bare(Msg::Submit));
        assert_eq!(cap.resolve("cat".into()), Msg::Submit("cat".into()));
    }

    #[test]
    fn map_lifts_both_shapes() {
        let stat: SubmitHandler<Msg> = SubmitHandler::Static(Msg::Add);
        assert_eq!(
            stat.map(Parent::Child).resolve("x".into()),
            Parent::Child(Msg::Add)
        );

        let cap: SubmitHandler<Msg> = SubmitHandler::Capturing(InputHandler::Bare(Msg::Submit));
        assert_eq!(
            cap.map(Parent::Child).resolve("dog".into()),
            Parent::Child(Msg::Submit("dog".into())),
        );
    }

    #[test]
    fn builders_populate_the_field() {
        // `on_submit` stores Static; `on_submit_with` stores Capturing.
        let e: Element<Msg> = Element::empty().on_submit(Msg::Add);
        assert!(matches!(e.on_submit, Some(SubmitHandler::Static(Msg::Add))));

        let e: Element<Msg> = Element::empty().on_submit_with(Msg::Submit);
        let Some(SubmitHandler::Capturing(h)) = e.on_submit else {
            panic!("on_submit_with stores a capturing handler");
        };
        assert_eq!(h.call("hi".into()), Msg::Submit("hi".into()));
    }
}
