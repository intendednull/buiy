//! Editor visual state, render-prep half (decoration-and-paint § 6.3):
//! the caret-blink writer. Caret visibility is a SQUARE-WAVE FUNCTION OF
//! THE APP CLOCK, evaluated here — in the `Animate→Picking` window the
//! other render-prep passes share (write_clip_rects / write_paint_skip) —
//! and written EDGE-ONLY: `Mut` marks the component changed only on
//! DerefMut, so a non-flipping frame issues zero ticks and the glyph
//! producer's § 6.2 union stays cold (the O(0) steady state; an
//! unconditional write would rebuild ExtractedGlyphs every frame).
//!
//! Reduced motion (text.md:90): the caret is STEADY — phase pinned true,
//! no blink. The blink PERIOD is a plugin resource (the Phase-0 Theme has
//! no motion scale; the token indirection is buiy-theme-tokens-design's
//! seam).
//!
//! **E3 rework (editing-and-ime §§ 5, 10):** the blink is now PER-ENTITY
//! phase-relative — `now − CaretBlink.origin`, where the origin is reset on
//! every edit / caret move by `write_caret_and_selection`, so the caret is
//! solid for one half-period immediately after the user acts (web parity).
//! T7's original "square-wave function of the app clock" survives as the
//! fallback for any caret WITHOUT a `TextEditState` (the `None` arm = the
//! absolute-clock global phase), so bare display carets are unchanged.

use std::time::Duration;

use bevy::math::{Rect, Vec2};
use bevy::prelude::*;

use crate::theme::UserPreferences;

use super::components::CaretVisual;
use super::decoration::{snap_thickness, snap_y};
use super::edit::TextEditState;

/// The blink HALF-period (time spent in each phase; a full cycle is 2×).
/// Default 500 ms — the conventional desktop rate. Zero ⇒ steady visible.
/// Swap the resource to retheme; the theme-token indirection is
/// `buiy-theme-tokens-design`'s seam (§ 6.3: "a theme/animation value,
/// not pinned here").
#[derive(Resource, Clone, Copy, PartialEq, Eq, Debug)]
pub struct CaretBlinkInterval(pub Duration);

impl Default for CaretBlinkInterval {
    fn default() -> Self {
        Self(Duration::from_millis(500))
    }
}

/// The square wave: even half-periods are visible (t=0 ⇒ visible — a
/// fresh caret shows immediately). Integer nanos, no float drift — and
/// `as_nanos()` is nonzero whenever `!is_zero()`, so the guard actually
/// covers the division (the plan's `as_micros` snippet truncated 1–999 ns
/// to 0 and panicked; recorded as a plan erratum). A zero interval is
/// steady visible (defensive — a misconfigured resource must not divide
/// by zero or strobe).
pub fn blink_phase(elapsed: Duration, half_period: Duration) -> bool {
    if half_period.is_zero() {
        return true;
    }
    (elapsed.as_nanos() / half_period.as_nanos()).is_multiple_of(2)
}

/// Render-prep: drive every `CaretVisual.visible` from the caret's PER-ENTITY
/// blink phase (editing-and-ime §§ 5, 10) — `now − CaretBlink.origin`, where the
/// origin is reset on every edit / caret move by `write_caret_and_selection`. So
/// the caret is solid for one half-period immediately after the user acts (web
/// parity), instead of the T7 global square wave. Edge-only: `Mut` ticks only on
/// a flip (the O(0) steady state). Carets WITHOUT a `TextEditState` (a bare T7
/// display caret, if any) fall back to the global phase. Reduced-motion ⇒ steady.
/// `UserPreferences` is `Option` so the plugin stays self-sufficient without
/// `ThemePlugin` (the apply_forced_colors_theme precedent).
pub fn write_caret_blink(
    time: Res<Time>,
    prefs: Option<Res<UserPreferences>>,
    interval: Res<CaretBlinkInterval>,
    mut carets: Query<(&mut CaretVisual, Option<&TextEditState>)>,
) {
    let steady = prefs.is_some_and(|p| p.prefers_reduced_motion);
    let now = time.elapsed();
    for (mut caret, state) in &mut carets {
        let elapsed = match state {
            Some(s) => now.saturating_sub(s.blink_origin()),
            None => now,
        };
        let phase = steady || blink_phase(elapsed, interval.0);
        // Edge-only: DerefMut (and the change tick) ONLY on a flip.
        if caret.visible != phase {
            caret.visible = phase;
        }
    }
}

/// The caret stamp rect (§ 6.1 + § 3.3), pure: fold the entity origin,
/// snap x to the physical grid and floor the width to whole physical px
/// (min 1) — the decoration snap rule rotated 90°: for a horizontal rule
/// the THIN axis is y/height; for the caret bar it is x/width. y/height
/// stay unsnapped (the caret spans the full line box — not a hairline
/// dimension). `snap_y` is an axis-agnostic scalar grid snap (named for
/// its T6 underline call site).
pub fn caret_stamp_rect(origin: Vec2, rect: Rect, scale_factor: f32) -> [f32; 4] {
    [
        snap_y(origin.x + rect.min.x, scale_factor),
        origin.y + rect.min.y,
        snap_thickness(rect.width(), scale_factor),
        rect.height(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// § 3.3 applied to the caret (the snap_thickness/snap_y math is
    /// pinned by T6's tests; these pin the composition + axis choice).
    #[test]
    fn caret_rect_snaps_x_and_floors_width() {
        let r = Rect::new(12.3, 0.0, 13.3, 19.2); // 1 px wide, unsnapped x
        // scale 1.0: x 22.3 → 22.0; w 1.0 → 1.0; y/h untouched.
        assert_eq!(
            caret_stamp_rect(Vec2::new(10.0, 20.0), r, 1.0),
            [22.0, 20.0, 1.0, 19.2]
        );
        // scale 1.5: x 22.3 → 33.45 phys → 33 → 22.0; w 1.0 → 1.5 phys →
        // round 2 phys → 4/3 logical (the § 3.3 pin: never 1.5 phys px).
        let [x, y, w, h] = caret_stamp_rect(Vec2::new(10.0, 20.0), r, 1.5);
        assert_eq!([x, y, h], [33.0 / 1.5, 20.0, 19.2]);
        assert_eq!(w, 2.0 / 1.5);
    }
}
