//! Switch widget — Wave-3 slice-1 (P1d bundle + C4 visual, bundle-then-pixels in
//! one pass).
//!
//! **P1d half (the a11y bundle + contract + APG keyboard):** the `Switch` marker's
//! `#[require(...)]` assembles the decomposed a11y substrate (`A11yRole::Switch`,
//! a **binary** `A11yToggled` — a switch has no `Mixed` —, `Focusable`, and
//! `A11yLabel`) plus the visible track box. The `A11yContract for
//! buiy_core::a11y::contract::Switch` advertises `{Click, Focus, Blur}` and lowers
//! `Click` into the shared `OnPress` sink; the APG keyboard keymap toggles a
//! switch on **both Space and Enter** (like a button — the asymmetry against the
//! Space-only Checkbox). All three modalities converge on the one `OnPress`
//! consumer, [`advance_toggle_on_press`](crate::advance_toggle_on_press).
//!
//! **C4 half (the pixels + pick-through):** a switch is a horizontal **row** of
//! `[track-pill, label]` (so the label sits BESIDE the pill instead of squishing
//! inside it — the widget-catalog rendering bug, the `Checkbox` precedent). The
//! root is the focusable, accessible flex-row container; the visible 40×20 pill
//! lives on the [`SwitchTrack`] child, and the sliding [`SwitchThumb`] is a child
//! of THAT track (a grandchild of the switch root). The constructor adds the
//! visible label as a sibling `Text` beside the track. The track + label are
//! `Pickable::IGNORE` so a hit anywhere on the row resolves to the widget root the
//! router addresses. [`update_switch_visual`] reads `Changed<A11yToggled>` on each
//! switch, walks to the thumb under its track, and positions it via the decomposed
//! `Translate` longhand: `False` puts the thumb off (left, `x = 0`), `True` slides
//! it on (right, by [`SWITCH_THUMB_TRAVEL`]). The thumb carries the *pixels*; the
//! root keeps `A11yLabel` (the AT name).

use bevy::picking::Pickable;
use bevy::prelude::*;
use buiy_core::{
    a11y::{A11yLabel, A11yRole, A11yToggled, Toggled},
    components::Node,
    focus::Focusable,
    layout::{
        AlignItems, BoxModel, Display, FlexAxis, FlexGap, FlexParams, Length, Style, Translate,
    },
    render::color::ColorToken,
    render::components::{Background, Border, Corners, Radius, TextColor},
    text::{FontSize, Text},
};

/// The catalog font size for the switch label glyph (logical px).
pub(crate) const SWITCH_LABEL_FONT_SIZE: f32 = 16.0;
/// How far (logical px) the thumb slides from off (left) to on (right). The
/// track is 40px wide with an 16px thumb + 2px inset, so the travel is
/// `40 − 16 − 2·2 = 20`.
pub const SWITCH_THUMB_TRAVEL: f32 = 20.0;

/// Switch widget marker. The `#[require(...)]` contract is the single source of
/// the switch shape (the `Button`/`Checkbox` precedent): the bare marker —
/// `world.spawn(Switch)` / `bsn! { Switch }` — materializes the full
/// layout-visible + paintable + focusable + accessible entity, carrying the
/// **binary** `A11yToggled` the contract + visual read.
///
/// The require list — a switch is a horizontal **row** of `[track-pill, label]`:
/// the root is the focusable, accessible flex-row container; the visible 40×20
/// pill + its sliding thumb live on the [`SwitchTrack`] child, and the visible
/// label is the sibling `Text` the constructor adds. Putting the pill on the root
/// (the pre-fix shape) trapped the label inside the 40×20 pill where it wrapped to
/// one glyph per line — the widget-catalog rendering bug.
/// - `Node` — the layout marker (pulls the full `Style` decomposition).
/// - `Display = flex_row()` + `FlexParams = switch_row_flex()` — lay the track and
///   label out in a centered row with a gap.
/// - `Focusable` — keyboard-focusable (implicit `{Focus, Blur}`).
/// - `A11yRole = A11yRole::Switch` — drives `contract_for(Switch)` (advertised
///   verbs + `honor`) and the APG Space+Enter keymap.
/// - `A11yToggled` — the binary toggle (defaults to `False` / off).
/// - `A11yLabel` — the accessible name (`Switch::new(label)` fills it).
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
#[require(
    Node,
    Display = Display::flex_row(),
    FlexParams = switch_row_flex(),
    Focusable,
    A11yRole = A11yRole::Switch,
    A11yToggled,
    A11yLabel,
)]
pub struct Switch;

/// The visual **track** child of a switch — the 40×20 pill (fill + pill-rounded
/// border) that carries the sliding [`SwitchThumb`] as ITS child. The `#[require]`
/// carries the pill geometry + paint companions so the track renders identically
/// whether spawned bare or via the constructor. A `buiy_widgets`-local **visual**
/// marker; it defines no a11y component (the a11y state lives on the widget root).
/// Decorative ⇒ `Pickable::IGNORE` so a click on the track resolves to the widget
/// root.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
#[require(
    Node,
    BoxModel = switch_box_model(),
    Background = switch_background(),
    Border = switch_border(),
    // Center the 16px thumb on the 20px pill's cross axis (mirrors SliderTrack /
    // CheckboxMark). `update_switch_visual` only writes the thumb's horizontal
    // `Translate`, so without this the knob laid out at the pill's content-top
    // (2px high of centre). Now it straddles the pill centered.
    Display = Display::flex_row(),
    FlexParams = switch_track_center_flex(),
)]
pub struct SwitchTrack;

/// The visual thumb child of a switch's [`SwitchTrack`] — the sliding knob whose
/// `Translate` position [`update_switch_visual`] drives from `A11yToggled`. A
/// `buiy_widgets`-local **visual** marker; it defines no a11y component (the a11y
/// state lives on the widget root). Decorative ⇒ `Pickable::IGNORE`.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
pub struct SwitchThumb;

// The initializer fns are `pub(crate)` so the `scene` module's `switch()`
// scene-fn can spell the SAME canonical values as the `#[require]` path.

/// The canonical switch track: 40×20 logical px (the pill).
// TODO(buiy-widget-catalog-design): replace hardcoded sizes with size tokens.
pub(crate) fn switch_box_model() -> BoxModel {
    Style::default().width_px(40.0).height_px(20.0).box_model
}

/// The default switch track fill (the `color.surface.secondary` token).
pub(crate) fn switch_background() -> Background {
    Background {
        color: ColorToken::SurfaceSecondary,
    }
}

/// The default switch border: fully-rounded (pill) corners.
pub(crate) fn switch_border() -> Border {
    Border {
        radius: Corners::all(Radius::circular(10.0)),
        ..Default::default()
    }
}

/// The switch ROOT row: `[track-pill, label]` laid out horizontally, vertically
/// centered, with an 8px gap between the pill and the label.
pub(crate) fn switch_row_flex() -> FlexParams {
    FlexParams {
        direction: FlexAxis::Row,
        align_items: AlignItems::Center,
        gap: FlexGap {
            row: Length::px(8.0),
            column: Length::px(8.0),
        },
        ..Default::default()
    }
}

/// The pill's content flex: center the thumb on the pill's cross axis (the thumb
/// is the pill's only child; its horizontal slide is a post-layout `Translate`).
pub(crate) fn switch_track_center_flex() -> FlexParams {
    FlexParams {
        direction: FlexAxis::Row,
        align_items: AlignItems::Center,
        ..Default::default()
    }
}

impl Switch {
    /// Spawn-ready bundle for a labelled switch. Returns `impl Bundle` carrying
    /// the full contract (role + binary toggle + focus + a11y) plus two children
    /// laid out in the flex-**row** substrate: the **track** pill (which itself
    /// carries the sliding **thumb**) and the visible **label** `Text` BESIDE it.
    /// The track + label are `Pickable::IGNORE` so a hit anywhere on the row
    /// resolves to the widget root (pick-through, co-drive SC-3).
    ///
    /// The thumb starts at the off position (`Translate` x = 0) because the
    /// default `A11yToggled` is `False`; [`update_switch_visual`] slides it on the
    /// first `Changed<A11yToggled>` once the state flips.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(label: impl Into<String>) -> impl Bundle {
        let label = label.into();
        (
            Switch,
            A11yLabel(label.clone()),
            children![
                // The 40×20 track pill (geometry + fill + border from its own
                // `#[require]`), carrying the sliding thumb as ITS child.
                (
                    SwitchTrack,
                    Pickable::IGNORE,
                    children![(
                        // The sliding thumb — starts off (Translate x = 0).
                        SwitchThumb,
                        Node,
                        switch_thumb_box_model(),
                        switch_thumb_background(),
                        switch_thumb_border(),
                        Translate(Length::px(0.0), Length::px(0.0), Length::px(0.0)),
                        Pickable::IGNORE,
                    )],
                ),
                // The visible label pixels BESIDE the track (the AT name stays on
                // the root).
                (
                    Text(label),
                    FontSize(SWITCH_LABEL_FONT_SIZE),
                    TextColor::default(),
                    Pickable::IGNORE,
                ),
            ],
        )
    }

    /// Read whether the switch is **on**, as a plain `bool` — the domain accessor
    /// over its (binary) [`A11yToggled`] state so a caller never touches the
    /// `accesskit::Toggled` enum (Track C / F1). Query the state alongside the
    /// marker and pass it in:
    ///
    /// ```ignore
    /// fn read(q: Query<&A11yToggled, With<Switch>>) {
    ///     for toggled in &q {
    ///         if Switch::on(toggled) { /* … */ }
    ///     }
    /// }
    /// ```
    ///
    /// A switch is binary; an out-of-contract `Mixed` reads as **not** on.
    pub fn on(state: &A11yToggled) -> bool {
        matches!(state.0, Toggled::True)
    }
}

/// The thumb box: a 16×16 knob.
pub(crate) fn switch_thumb_box_model() -> BoxModel {
    Style::default().width_px(16.0).height_px(16.0).box_model
}

/// The thumb fill (the `color.surface.primary` token — a contrasting knob).
pub(crate) fn switch_thumb_background() -> Background {
    Background {
        color: ColorToken::SurfacePrimary,
    }
}

/// The thumb border: fully-rounded (circular) corners.
pub(crate) fn switch_thumb_border() -> Border {
    Border {
        radius: Corners::all(Radius::circular(8.0)),
        ..Default::default()
    }
}

/// The change-detection filter for [`update_switch_visual`]: a `Switch` whose
/// `A11yToggled` changed this frame. Aliased so the system signature stays under
/// clippy's `type_complexity` bar.
type ChangedSwitch = (With<Switch>, Changed<A11yToggled>);

/// C4 visual system: drive each switch's thumb from its `A11yToggled` state,
/// gated on `Changed<A11yToggled>` so the slide fires exactly once per flip. A
/// toggle from any modality (pointer, Space/Enter, AT-`Click`) flows through the
/// one `OnPress` consumer into this `A11yToggled` write, and this system reacts.
///
/// For each changed switch, walk its `Children` to the [`SwitchTrack`] child and
/// then to the [`SwitchThumb`] *grandchild* (the thumb is a child of the track),
/// and set the thumb's `Translate` x offset: `True` → slid right by
/// [`SWITCH_THUMB_TRAVEL`], `False` → back to `0`. (`Mixed` is not a switch state;
/// the binary contract never produces it, but it is treated as `True` for
/// robustness, matching `A11yToggled::toggle_switch`.)
pub fn update_switch_visual(
    changed: Query<(&A11yToggled, &Children), ChangedSwitch>,
    tracks: Query<&Children, With<SwitchTrack>>,
    mut thumbs: Query<&mut Translate, With<SwitchThumb>>,
) {
    for (toggled, children) in &changed {
        let x = match toggled.0 {
            Toggled::False => 0.0,
            Toggled::True | Toggled::Mixed => SWITCH_THUMB_TRAVEL,
        };
        // Switch → SwitchTrack → SwitchThumb: the thumb is a grandchild now that
        // the pill moved off the root onto its own track child.
        for &child in children {
            let Ok(track_children) = tracks.get(child) else {
                continue;
            };
            for &grandchild in track_children {
                if let Ok(mut translate) = thumbs.get_mut(grandchild) {
                    translate.0 = Length::px(x);
                }
            }
        }
    }
}
