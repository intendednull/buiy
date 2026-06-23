//! Checkbox widget — Wave-3 slice-1 (P1d bundle + C4 visual, bundle-then-pixels
//! in one pass).
//!
//! **P1d half (the a11y bundle + contract + APG keyboard):** the `Checkbox`
//! marker's `#[require(...)]` assembles the decomposed a11y substrate —
//! `A11yRole::Checkbox` + the **tri-state** `A11yToggled` (`{False, True, Mixed}`,
//! `Mixed` is a first-class checkbox value) + `Focusable` + `A11yLabel` — plus the
//! visible box (the `Node`/`Style` decomposition + paint companions). The
//! `A11yContract for buiy_core::a11y::contract::Checkbox` (keyed by role in
//! `contract_for`) advertises `{Click, Focus, Blur}` and lowers `Click` into the
//! shared `OnPress` sink; the APG keyboard keymap (`keyboard_activation`) toggles
//! a checkbox on **Space only** — Enter does nothing (the canonical asymmetry vs
//! Button). All three modalities (pointer, Space, AT-`Click`) converge on the one
//! `OnPress` consumer, [`advance_toggle_on_press`](crate::advance_toggle_on_press).
//!
//! **C4 half (the pixels + pick-through):** the constructor spawns a child check/
//! dash glyph (the [`CheckboxMark`]) and a child label `Text`, both
//! `Pickable::IGNORE` so a hit resolves to the widget root the router addresses.
//! [`update_checkbox_visual`] reads `Changed<A11yToggled>` on each checkbox and
//! drives its mark: `True` → the check glyph shown, `Mixed` → the dash glyph
//! shown, `False` → the mark hidden (`CssVisibility::Hidden`, which keeps the
//! layout box + a11y presence but skips paint). The mark/label carry the *pixels*;
//! the root keeps `A11yLabel` (the AT name) — the decoupling C4 owns.

use bevy::picking::Pickable;
use bevy::prelude::*;
use buiy_core::{
    a11y::{A11yLabel, A11yRole, A11yToggled, Toggled},
    components::Node,
    focus::Focusable,
    layout::{BoxModel, Style},
    render::color::ColorToken,
    render::components::{Background, Border, Corners, CssVisibility, Radius, TextColor},
    text::{FontSize, Text},
};
use std::borrow::Cow;

/// The glyph a checkbox mark shows when checked (`Toggled::True`).
pub const CHECK_GLYPH: &str = "✓";
/// The glyph a checkbox mark shows when indeterminate (`Toggled::Mixed`).
pub const DASH_GLYPH: &str = "–";
/// The catalog font size for the checkbox mark glyph (logical px).
pub(crate) const CHECKBOX_MARK_FONT_SIZE: f32 = 16.0;

/// Checkbox widget marker. The `#[require(...)]` contract is the single source of
/// the checkbox shape (the `Button` precedent, spec § 4.1a): the bare marker —
/// `world.spawn(Checkbox)` / `bsn! { Checkbox }` — materializes the full
/// layout-visible + paintable + focusable + accessible entity, carrying the
/// **tri-state** `A11yToggled` the contract + visual read.
///
/// The require list:
/// - `Node` — the layout marker, which `#[require]`s the full `Style`
///   decomposition, so a checkbox is layout-visible without re-spelling those.
/// - `BoxModel = checkbox_box_model()` — the canonical 18×18 box (a direct
///   `#[require]` initializer wins over `Node`'s transitive default).
/// - `Background` / `Border` — the paint companions (the box fill + rounded edge).
/// - `Focusable` — keyboard-focusable (contributes the implicit `{Focus, Blur}`).
/// - `A11yRole = A11yRole::Checkbox` — drives the `contract_for(Checkbox)` lookup
///   (advertised verbs + `honor`) and the APG Space-only keymap.
/// - `A11yToggled` — the tri-state toggle (defaults to `False` / unchecked).
/// - `A11yLabel` — the accessible name (`Checkbox::new(label)` fills it).
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
#[require(
    Node,
    BoxModel = checkbox_box_model(),
    Background = checkbox_background(),
    Border = checkbox_border(),
    Focusable,
    A11yRole = A11yRole::Checkbox,
    A11yToggled,
    A11yLabel,
)]
pub struct Checkbox;

/// The visual mark child of a checkbox — the check (`✓`) / dash (`–`) glyph whose
/// presence + content [`update_checkbox_visual`] drives from `A11yToggled`. A
/// `buiy_widgets`-local **visual** marker; it defines no a11y component (the a11y
/// state lives on the widget root). Decorative ⇒ `Pickable::IGNORE` so a click on
/// the mark resolves to the widget root.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
pub struct CheckboxMark;

// The initializer fns are `pub(crate)` so the `scene` module's `checkbox()`
// scene-fn can spell the SAME canonical values as the `#[require]` path — one
// source of truth shared between the require initializers and the scene-fn.

/// The canonical checkbox box: 18×18 logical px. (The hit target is enlarged to
/// the WCAG 2.5.8 ≥24×24 minimum by the label row in a real layout; the box glyph
/// itself is the conventional control size.)
// TODO(buiy-widget-catalog-design): replace hardcoded sizes with size tokens.
pub(crate) fn checkbox_box_model() -> BoxModel {
    Style::default().width_px(18.0).height_px(18.0).box_model
}

/// The default checkbox surface fill (the `color.surface.secondary` token).
pub(crate) fn checkbox_background() -> Background {
    Background {
        color: ColorToken::Token(Cow::Borrowed("color.surface.secondary")),
    }
}

/// The default checkbox border: lightly rounded corners (`radius.sm`).
pub(crate) fn checkbox_border() -> Border {
    Border {
        radius: Corners::all(Radius::circular(4.0)),
        ..Default::default()
    }
}

impl Checkbox {
    /// Spawn-ready bundle for a labelled checkbox. Returns `impl Bundle` carrying
    /// the full contract (role + tri-state toggle + focus + a11y + box) plus two
    /// decorative children: the check/dash **mark** glyph and the visible
    /// **label** `Text`, both `Pickable::IGNORE` so a hit resolves to the widget
    /// root the router addresses (pick-through, co-drive SC-3).
    ///
    /// The mark starts hidden (`CssVisibility::Hidden`) because the default
    /// `A11yToggled` is `False`; [`update_checkbox_visual`] reveals it on the
    /// first `Changed<A11yToggled>` once the state flips.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(label: impl Into<String>) -> impl Bundle {
        let label = label.into();
        (
            Checkbox,
            A11yLabel(label.clone()),
            children![
                // The check/dash mark glyph — hidden until the box is checked.
                (
                    CheckboxMark,
                    Text(CHECK_GLYPH.into()),
                    FontSize(CHECKBOX_MARK_FONT_SIZE),
                    TextColor::default(),
                    // Default state is `False` ⇒ the mark is hidden.
                    CssVisibility::Hidden,
                    Pickable::IGNORE,
                ),
                // The visible label pixels (the AT name stays on the root).
                (
                    Text(label),
                    FontSize(CHECKBOX_MARK_FONT_SIZE),
                    TextColor::default(),
                    Pickable::IGNORE,
                ),
            ],
        )
    }
}

/// The change-detection filter for [`update_checkbox_visual`]: a `Checkbox` whose
/// `A11yToggled` changed this frame. Aliased so the system signature stays under
/// clippy's `type_complexity` bar.
type ChangedCheckbox = (With<Checkbox>, Changed<A11yToggled>);

/// C4 visual system: drive each checkbox's mark from its `A11yToggled` state,
/// gated on `Changed<A11yToggled>` so the repaint fires exactly once per flip
/// (the single source of truth is the agent-interface state; a toggle from any
/// modality — pointer, Space, AT-`Click` — flows through the one `OnPress`
/// consumer into this `A11yToggled` write, and this system reacts).
///
/// For each changed checkbox, walk its `Children` to the `CheckboxMark` child and
/// set its `CssVisibility` + glyph: `True` → visible + `✓`, `Mixed` → visible +
/// `–`, `False` → hidden. `CssVisibility::Hidden` keeps the layout box and a11y
/// presence; only paint is skipped.
pub fn update_checkbox_visual(
    changed: Query<(&A11yToggled, &Children), ChangedCheckbox>,
    mut marks: Query<(&mut CssVisibility, &mut Text), With<CheckboxMark>>,
) {
    for (toggled, children) in &changed {
        let (visibility, glyph) = match toggled.0 {
            Toggled::True => (CssVisibility::Visible, CHECK_GLYPH),
            Toggled::Mixed => (CssVisibility::Visible, DASH_GLYPH),
            Toggled::False => (CssVisibility::Hidden, CHECK_GLYPH),
        };
        for &child in children {
            if let Ok((mut vis, mut text)) = marks.get_mut(child) {
                *vis = visibility;
                // Only the glyph CONTENT differs between True/Mixed; rewrite it so
                // a False→Mixed→True walk shows the right mark even though it is
                // hidden in `False`.
                if text.0 != glyph {
                    text.0 = glyph.to_string();
                }
            }
        }
    }
}
