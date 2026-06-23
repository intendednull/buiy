//! Slider widget — Wave-3 slice-2 (P1d bundle + contract + APG keyboard, plus the
//! C4 track/thumb visual + pick-through; bundle-then-pixels in one pass).
//!
//! **P1d half (the a11y bundle + contract + APG keyboard):** the `Slider` marker's
//! `#[require(...)]` assembles the decomposed a11y substrate (`A11yRole::Slider`,
//! the valued range [`A11yValue`] — `now`/`min`/`max`, optional
//! `step`/`jump`/`text` —, [`A11yOrientation`],
//! `Focusable`, and `A11yLabel`) plus the visible track box. The `A11yContract for
//! buiy_core::a11y::contract::Slider` advertises `{Increment, Decrement, SetValue,
//! Focus, Blur}` and — **unlike the toggle widgets** — does NOT lower through
//! `OnPress`: its verbs mutate the live `A11yValue` directly (the component the
//! outbound fold re-emits, so the AT observes the new value the same frame). The
//! APG slider keyboard (`slider_keyboard`, in `buiy_core`) dispatches the value
//! verbs through the same router seam an AT drives — Right/Up increment, Left/Down
//! decrement, Home → min, End → max, PageUp/PageDown → the page step. Keyboard and
//! agent converge on the one contract `honor`.
//!
//! **C4 half (the pixels + pick-through):** the constructor spawns a child
//! **track** (the rail), a child **thumb** (the draggable knob), and a child label
//! `Text`, all `Pickable::IGNORE` so a hit resolves to the widget root the router
//! addresses. [`update_slider_visual`] reads `Changed<A11yValue>` on each slider
//! and positions its thumb via the decomposed `Translate` longhand: the fraction
//! `(now − min) / (max − min)` maps onto `[0, SLIDER_THUMB_TRAVEL]`. The track/
//! thumb carry the *pixels*; the root keeps `A11yLabel` (the AT name).

use bevy::picking::Pickable;
use bevy::prelude::*;
use buiy_core::{
    a11y::{A11yLabel, A11yOrientation, A11yRole, A11yValue, Orientation},
    components::Node,
    focus::Focusable,
    layout::{BoxModel, Length, Style, Translate},
    render::color::ColorToken,
    render::components::{Background, Border, Corners, Radius, TextColor},
    text::{FontSize, Text},
};
use std::borrow::Cow;

/// The catalog font size for the slider label glyph (logical px).
pub(crate) const SLIDER_LABEL_FONT_SIZE: f32 = 16.0;
/// The track width (logical px) — the rail the thumb travels along.
pub const SLIDER_TRACK_WIDTH: f32 = 160.0;
/// The track height (logical px) — a thin rail.
pub(crate) const SLIDER_TRACK_HEIGHT: f32 = 4.0;
/// The thumb diameter (logical px) — the draggable knob.
pub(crate) const SLIDER_THUMB_SIZE: f32 = 16.0;
/// How far (logical px) the thumb's left edge travels from the `min` end to the
/// `max` end: the track width minus the thumb width, so the thumb stays inside the
/// rail at both extremes (`160 − 16 = 144`).
pub const SLIDER_THUMB_TRAVEL: f32 = SLIDER_TRACK_WIDTH - SLIDER_THUMB_SIZE;

/// Slider widget marker. The `#[require(...)]` contract is the single source of
/// the slider shape (the `Button`/`Checkbox`/`Switch` precedent): the bare marker
/// — `world.spawn(Slider)` / `bsn! { Slider }` — materializes the full
/// layout-visible + paintable + focusable + accessible entity, carrying the
/// valued range the contract + visual read.
///
/// The require list:
/// - `Node` — the layout marker (pulls the full `Style` decomposition).
/// - `BoxModel = slider_box_model()` — the canonical track bounding box.
/// - `Background` / `Border` — the rail fill + rounded edge.
/// - `Focusable` — keyboard-focusable (implicit `{Focus, Blur}`).
/// - `A11yRole = A11yRole::Slider` — drives `contract_for(Slider)` (advertised
///   `{Increment, Decrement, SetValue}`) and the APG slider keymap.
/// - `A11yValue` — the valued range (defaults to `now = min = max = 0`,
///   `step`/`jump`/`text` unset; `Slider::new` / the scene-fn fill the range).
/// - `A11yOrientation` — control orientation (defaults `Vertical` per the
///   accesskit enum-property default; the catalog slider is authored `Horizontal`).
/// - `A11yLabel` — the accessible name (`Slider::new(label, …)` fills it).
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
#[require(
    Node,
    BoxModel = slider_box_model(),
    Background = slider_background(),
    Border = slider_border(),
    Focusable,
    A11yRole = A11yRole::Slider,
    A11yValue,
    A11yOrientation,
    A11yLabel,
)]
pub struct Slider;

/// The decorative **track** (rail) child of a slider — the bar the thumb travels
/// along. A `buiy_widgets`-local **visual** marker; it defines no a11y component
/// (the a11y state lives on the widget root). Decorative ⇒ `Pickable::IGNORE`.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
pub struct SliderTrack;

/// The visual **thumb** child of a slider — the draggable knob whose `Translate`
/// position [`update_slider_visual`] drives from `A11yValue`. A `buiy_widgets`-local
/// **visual** marker; no a11y component. Decorative ⇒ `Pickable::IGNORE`.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
pub struct SliderThumb;

// The initializer fns are `pub(crate)` so the `scene` module's `slider()`
// scene-fn can spell the SAME canonical values as the `#[require]` path.

/// The canonical slider bounding box: the track width × the thumb height (so the
/// thumb fits vertically). Logical px.
// TODO(buiy-widget-catalog-design): replace hardcoded sizes with size tokens.
pub(crate) fn slider_box_model() -> BoxModel {
    Style::default()
        .width_px(SLIDER_TRACK_WIDTH)
        .height_px(SLIDER_THUMB_SIZE)
        .box_model
}

/// The default slider rail fill (the `color.surface.secondary` token).
pub(crate) fn slider_background() -> Background {
    Background {
        color: ColorToken::Token(Cow::Borrowed("color.surface.secondary")),
    }
}

/// The default slider border: fully-rounded (pill) ends on the rail.
pub(crate) fn slider_border() -> Border {
    Border {
        radius: Corners::all(Radius::circular(2.0)),
        ..Default::default()
    }
}

/// The track (rail) box: full width, thin height.
pub(crate) fn slider_track_box_model() -> BoxModel {
    Style::default()
        .width_px(SLIDER_TRACK_WIDTH)
        .height_px(SLIDER_TRACK_HEIGHT)
        .box_model
}

/// The track fill (the `color.surface.secondary` token — the inactive rail).
pub(crate) fn slider_track_background() -> Background {
    Background {
        color: ColorToken::Token(Cow::Borrowed("color.surface.secondary")),
    }
}

/// The thumb box: a 16×16 knob.
pub(crate) fn slider_thumb_box_model() -> BoxModel {
    Style::default()
        .width_px(SLIDER_THUMB_SIZE)
        .height_px(SLIDER_THUMB_SIZE)
        .box_model
}

/// The thumb fill (the `color.surface.primary` token — a contrasting knob).
pub(crate) fn slider_thumb_background() -> Background {
    Background {
        color: ColorToken::Token(Cow::Borrowed("color.surface.primary")),
    }
}

/// The thumb border: fully-rounded (circular) corners.
pub(crate) fn slider_thumb_border() -> Border {
    Border {
        radius: Corners::all(Radius::circular(8.0)),
        ..Default::default()
    }
}

/// The horizontal offset (logical px) the thumb sits at for value `value`: the
/// fraction `(now − min) / (max − min)` mapped onto `[0, SLIDER_THUMB_TRAVEL]`. A
/// degenerate range (`max <= min`) maps to `0` (the thumb at the `min` end) rather
/// than dividing by zero. The single source of the thumb geometry, shared by the
/// visual system and its tests.
pub fn thumb_offset(value: &A11yValue) -> f32 {
    let span = value.max - value.min;
    if span <= 0.0 {
        return 0.0;
    }
    let fraction = ((value.now - value.min) / span).clamp(0.0, 1.0) as f32;
    fraction * SLIDER_THUMB_TRAVEL
}

impl Slider {
    /// Spawn-ready bundle for a labelled slider over `[min, max]` starting at
    /// `now`, stepping by `step`. Returns `impl Bundle` carrying the full contract
    /// (role + valued range + orientation + focus + a11y + track box) plus three
    /// decorative children — the **track** rail, the sliding **thumb**, and the
    /// visible **label** `Text` — all `Pickable::IGNORE` so a hit resolves to the
    /// widget root (pick-through, co-drive SC-3).
    ///
    /// The thumb is positioned by [`update_slider_visual`] on the first
    /// `Changed<A11yValue>` once the schedule runs (the `A11yValue` written here is
    /// a change on spawn), so its `Translate` x starts at `0` and settles to
    /// [`thumb_offset`] of `now` on the first frame. The slider is authored
    /// **horizontal** (the catalog default), overriding the `A11yOrientation`
    /// `Vertical` default.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(label: impl Into<String>, now: f64, min: f64, max: f64, step: f64) -> impl Bundle {
        let label = label.into();
        (
            Slider,
            A11yLabel(label.clone()),
            A11yValue {
                now,
                min,
                max,
                step: Some(step),
                jump: None,
                text: None,
            },
            A11yOrientation(Orientation::Horizontal),
            children![
                // The rail the thumb travels along.
                (
                    SliderTrack,
                    Node,
                    slider_track_box_model(),
                    slider_track_background(),
                    Pickable::IGNORE,
                ),
                // The sliding thumb — starts at x = 0; `update_slider_visual`
                // positions it from `A11yValue` on the first frame.
                (
                    SliderThumb,
                    Node,
                    slider_thumb_box_model(),
                    slider_thumb_background(),
                    slider_thumb_border(),
                    Translate(Length::px(0.0), Length::px(0.0), Length::px(0.0)),
                    Pickable::IGNORE,
                ),
                // The visible label pixels (the AT name stays on the root).
                (
                    Text(label),
                    FontSize(SLIDER_LABEL_FONT_SIZE),
                    TextColor::default(),
                    Pickable::IGNORE,
                ),
            ],
        )
    }
}

/// The change-detection filter for [`update_slider_visual`]: a `Slider` whose
/// `A11yValue` changed this frame. Aliased so the system signature stays under
/// clippy's `type_complexity` bar.
type ChangedSlider = (With<Slider>, Changed<A11yValue>);

/// C4 visual system: drive each slider's thumb position from its `A11yValue`,
/// gated on `Changed<A11yValue>` so the thumb repositions exactly once per value
/// change. The single source of truth is the agent-interface state; a value change
/// from any modality (keyboard arrows, an AT `Increment`/`SetValue`) flows through
/// the slider contract's `honor` into this `A11yValue` write, and this system
/// reacts.
///
/// For each changed slider, walk its `Children` to the `SliderThumb` child and set
/// its `Translate` x offset to [`thumb_offset`] of the live value: `now == min` →
/// `0` (thumb at the `min` end), `now == max` → [`SLIDER_THUMB_TRAVEL`] (thumb at
/// the `max` end), linearly in between.
pub fn update_slider_visual(
    changed: Query<(&A11yValue, &Children), ChangedSlider>,
    mut thumbs: Query<&mut Translate, With<SliderThumb>>,
) {
    for (value, children) in &changed {
        let x = thumb_offset(value);
        for &child in children {
            if let Ok(mut translate) = thumbs.get_mut(child) {
                translate.0 = Length::px(x);
            }
        }
    }
}
