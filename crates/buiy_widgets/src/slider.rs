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
//! **C4 half (the pixels + pick-through):** a slider is a horizontal **row** of
//! `[track, label]` (so the label sits BESIDE the rail instead of squishing inside
//! it — the widget-catalog rendering bug, the `Checkbox` precedent). The root is
//! the focusable, accessible flex-row container; the visible rail lives on the
//! [`SliderTrack`] child, and the draggable [`SliderThumb`] is a child of THAT
//! track (a grandchild of the slider root). The constructor adds the visible label
//! as a sibling `Text` beside the track. The track + label are `Pickable::IGNORE`
//! so a hit anywhere on the row resolves to the widget root the router addresses.
//! [`update_slider_visual`] reads `Changed<A11yValue>` on each slider, walks to the
//! thumb under its track, and positions it via the decomposed `Translate` longhand:
//! the fraction `(now − min) / (max − min)` maps onto `[0, SLIDER_THUMB_TRAVEL]`.
//! The track/thumb carry the *pixels*; the root keeps `A11yLabel` (the AT name).

use bevy::picking::Pickable;
use bevy::prelude::*;
use buiy_core::{
    a11y::{A11yLabel, A11yOrientation, A11yRole, A11yValue, Orientation},
    components::Node,
    focus::Focusable,
    layout::{
        AlignItems, BoxModel, Display, FlexAxis, FlexGap, FlexParams, Length, Style, Translate,
    },
    render::color::ColorToken,
    render::components::{Background, Border, Corners, Radius, TextColor},
    text::{FontSize, Text},
};

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
/// The require list — a slider is a horizontal **row** of `[track, label]`: the
/// root is the focusable, accessible flex-row container; the visible rail + its
/// draggable thumb live on the [`SliderTrack`] child, and the visible label is the
/// sibling `Text` the constructor adds. Putting the rail box on the root (the
/// pre-fix shape) trapped the label inside the rail's bounding box where it wrapped
/// to one glyph per line — the widget-catalog rendering bug.
/// - `Node` — the layout marker (pulls the full `Style` decomposition).
/// - `Display = flex_row()` + `FlexParams = slider_row_flex()` — lay the track and
///   label out in a centered row with a gap.
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
    Display = Display::flex_row(),
    FlexParams = slider_row_flex(),
    Focusable,
    A11yRole = A11yRole::Slider,
    A11yValue,
    A11yOrientation,
    A11yLabel,
)]
pub struct Slider;

/// The visual **track** (rail) child of a slider — the bar the thumb travels
/// along, carrying the draggable [`SliderThumb`] as ITS child. The `#[require]`
/// carries the rail geometry + fill so the track renders identically whether
/// spawned bare or via the constructor. A `buiy_widgets`-local **visual** marker;
/// it defines no a11y component (the a11y state lives on the widget root).
/// Decorative ⇒ `Pickable::IGNORE` so a click on the track resolves to the widget
/// root.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
#[require(
    Node,
    BoxModel = slider_track_box_model(),
    Background = slider_track_background(),
    // Center the 16px thumb on the 4px rail (cross-axis). The thumb is the only
    // child; `align_items: Center` straddles it symmetrically over the thin rail
    // (overflow is `Visible`), and `update_slider_visual`'s post-layout `Translate`
    // x slides it along — so the knob stays vertically centered at every value.
    // Without this the thumb laid out at the rail's content-top and hung 6px below
    // the rail centre, overflowing the slider row.
    Display = Display::flex_row(),
    FlexParams = slider_track_center_flex(),
)]
pub struct SliderTrack;

/// The visual **thumb** child of a slider's [`SliderTrack`] — the draggable knob
/// whose `Translate` position [`update_slider_visual`] drives from `A11yValue`. A
/// `buiy_widgets`-local **visual** marker; no a11y component. Decorative ⇒
/// `Pickable::IGNORE`.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
pub struct SliderThumb;

// The initializer fns are `pub(crate)` so the `scene` module's `slider()`
// scene-fn can spell the SAME canonical values as the `#[require]` path.

/// The slider ROOT row: `[track, label]` laid out horizontally, vertically
/// centered, with an 8px gap between the rail and the label.
pub(crate) fn slider_row_flex() -> FlexParams {
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

/// The track's content flex: center the thumb on the rail's cross axis (the
/// thumb is the rail's only child; its horizontal travel is a post-layout
/// `Translate`).
pub(crate) fn slider_track_center_flex() -> FlexParams {
    FlexParams {
        direction: FlexAxis::Row,
        align_items: AlignItems::Center,
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
        color: ColorToken::SurfaceSecondary,
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
        color: ColorToken::SurfacePrimary,
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
    /// (role + valued range + orientation + focus + a11y) plus two children laid
    /// out in the flex-**row** substrate: the **track** rail (which itself carries
    /// the draggable **thumb**) and the visible **label** `Text` BESIDE it. The
    /// track + label are `Pickable::IGNORE` so a hit anywhere on the row resolves
    /// to the widget root (pick-through, co-drive SC-3).
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
                // The rail the thumb travels along (geometry + fill from its own
                // `#[require]`), carrying the sliding thumb as ITS child.
                (
                    SliderTrack,
                    Pickable::IGNORE,
                    children![(
                        // The sliding thumb — starts at x = 0; `update_slider_visual`
                        // positions it from `A11yValue` on the first frame.
                        SliderThumb,
                        Node,
                        slider_thumb_box_model(),
                        slider_thumb_background(),
                        slider_thumb_border(),
                        Translate(Length::px(0.0), Length::px(0.0), Length::px(0.0)),
                        Pickable::IGNORE,
                    )],
                ),
                // The visible label pixels BESIDE the track (the AT name stays on
                // the root).
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
/// For each changed slider, walk its `Children` to the [`SliderTrack`] child and
/// then to the [`SliderThumb`] *grandchild* (the thumb is a child of the track),
/// and set the thumb's `Translate` x offset to [`thumb_offset`] of the live value:
/// `now == min` → `0` (thumb at the `min` end), `now == max` →
/// [`SLIDER_THUMB_TRAVEL`] (thumb at the `max` end), linearly in between.
pub fn update_slider_visual(
    changed: Query<(&A11yValue, &Children), ChangedSlider>,
    tracks: Query<&Children, With<SliderTrack>>,
    mut thumbs: Query<&mut Translate, With<SliderThumb>>,
) {
    for (value, children) in &changed {
        let x = thumb_offset(value);
        // Slider → SliderTrack → SliderThumb: the thumb is a grandchild now that
        // it moved off the root onto the track child.
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
