//! The `WriteEffectGroups` render-prep pass: derive the `EffectGroup`
//! marker from the five effect formers.
//!
//! The effect-input components (`Opacity`, `Filter`/`FilterFn`,
//! `MixBlendMode`, `BackdropFilter`) and the predicate output
//! (`EffectGroup` / `EffectReason`) are owned by R1's
//! `crate::render::components` — this module defines **no** types, only the
//! predicate (`effect_reason_for`) and the system (`write_effect_groups`).
//! The layout-owned `Stacking.isolation` field is the fifth input.
//!
//! **Scope:** this module derives the `EffectGroup` *boundary marker*
//! only. Per-group geometry (painted bounds, bucketed `TextureDescriptor`,
//! post-order index) and the off-screen render targets are a render-world
//! Prepare pass owned by a later phase (effect-compositor.md § 1.1, § 2) —
//! NOT here.
//!
//! Predicate + ownership:
//! docs/specs/2026-06-03-buiy-render-pipeline-design/effect-compositor.md § 1.
//! Struct shapes (owned by R1): component-model.md §§ 6, 8, 10.

use bevy::prelude::*;

// All effect types are owned by R1 (render/components.rs) — imported, never
// redefined here. `EffectGroup` / `EffectReason` are re-exported (`pub use`)
// so the predicate output is nameable through `render::effect` (where the
// system and predicate that produce it live) without re-defining R1's types.
use crate::components::Node;
use crate::layout::{Isolation, Stacking};
use crate::render::components::{BackdropFilter, Filter, MixBlendMode, Opacity};
pub use crate::render::components::{EffectGroup, EffectReason};

/// Canonical effect-group-former predicate (effect-compositor.md § 1):
/// an entity forms an `EffectGroup` iff ANY of —
///
/// 1. `Opacity < 1`,
/// 2. `Stacking.isolation == Isolation::Isolate`,
/// 3. `Filter` non-empty,
/// 4. `MixBlendMode != Normal`,
/// 5. `BackdropFilter` non-empty.
///
/// Returns the OR of every reason that held, or `None` if the entity
/// forms no group. Absent render components are passed as `None` and
/// read as their CSS-initial (no-group) value.
///
/// `backdrop-filter` sets `BACKDROP_FILTER` but is deliberately NOT a
/// stacking-context trigger (effect-compositor.md § 1) — that distinction
/// is layout 6f's concern, not this predicate's; here it is simply a
/// fifth former bit.
pub(crate) fn effect_reason_for(
    opacity: Option<Opacity>,
    isolation: Isolation,
    filter: Option<Filter>,
    blend: Option<MixBlendMode>,
    backdrop: Option<BackdropFilter>,
) -> Option<EffectReason> {
    let mut reason = EffectReason::empty();
    if opacity.is_some_and(|o| o.0 < 1.0) {
        reason |= EffectReason::OPACITY;
    }
    if isolation == Isolation::Isolate {
        reason |= EffectReason::ISOLATION;
    }
    if filter.is_some_and(|f| !f.0.is_empty()) {
        reason |= EffectReason::FILTER;
    }
    if blend.is_some_and(|b| b != MixBlendMode::Normal) {
        reason |= EffectReason::MIX_BLEND;
    }
    if backdrop.is_some_and(|b| !b.0.is_empty()) {
        reason |= EffectReason::BACKDROP_FILTER;
    }
    (!reason.is_empty()).then_some(reason)
}

// The per-entity read-shape for `write_effect_groups`: the five effect
// formers (`Stacking` carries the layout-owned `isolation` field) plus the
// currently-derived marker. A `type` alias keeps the `Query` signature under
// clippy's `type_complexity` bar, matching the sibling `ClipNodeData` in
// `render::clip`.
type EffectInputs<'w> = (
    Entity,
    Option<&'w Opacity>,
    Option<&'w Stacking>,
    Option<&'w Filter>,
    Option<&'w MixBlendMode>,
    Option<&'w BackdropFilter>,
    Option<&'w EffectGroup>,
);

/// Render-prep pass: derive the `EffectGroup` marker from the five effect
/// formers (effect-compositor.md § 1). Inserts `EffectGroup { reason }`
/// when any former holds; removes a stale marker when none do.
/// Writes ONLY the boundary marker — per-group target sizing/allocation is
/// a render-world Prepare pass (effect-compositor.md § 1.1, later phase).
///
/// Runs alongside `WriteClipRects` in the render-prep window
/// (`.after(BuiySet::Animate).before(BuiySet::Picking)`); wiring is Task 8.
pub fn write_effect_groups(mut commands: Commands, query: Query<EffectInputs, With<Node>>) {
    for (entity, opacity, stacking, filter, blend, backdrop, existing) in &query {
        let isolation = stacking.map(|s| s.isolation).unwrap_or_default();
        let reason = effect_reason_for(
            opacity.copied(),
            isolation,
            filter.cloned(),
            blend.copied(),
            backdrop.cloned(),
        );
        match (reason, existing) {
            (Some(reason), _) => {
                // Insert or overwrite the marker with the current reason set.
                commands.entity(entity).insert(EffectGroup { reason });
            }
            (None, Some(_)) => {
                // Former no longer holds — drop the stale marker (Task 5).
                commands.entity(entity).remove::<EffectGroup>();
            }
            (None, None) => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::{Isolation, Length};
    use crate::render::components::FilterFn;

    // A small constructor matching the system's read-shape: the four
    // render-owned inputs plus the one layout-owned `Isolation` field.
    fn reason_of(
        opacity: Option<f32>,
        isolation: Isolation,
        filter_len: usize,
        blend: MixBlendMode,
        backdrop_len: usize,
    ) -> Option<EffectReason> {
        effect_reason_for(
            opacity.map(Opacity),
            isolation,
            (filter_len > 0).then(|| Filter(vec![FilterFn::Blur(Length::px(1.0)); filter_len])),
            (blend != MixBlendMode::Normal).then_some(blend),
            (backdrop_len > 0)
                .then(|| BackdropFilter(vec![FilterFn::Blur(Length::px(1.0)); backdrop_len])),
        )
    }

    #[test]
    fn opacity_below_one_forms_opacity_reason() {
        assert_eq!(
            reason_of(Some(0.5), Isolation::Auto, 0, MixBlendMode::Normal, 0),
            Some(EffectReason::OPACITY)
        );
    }

    #[test]
    fn opacity_exactly_one_forms_no_group() {
        assert_eq!(
            reason_of(Some(1.0), Isolation::Auto, 0, MixBlendMode::Normal, 0),
            None
        );
    }

    #[test]
    fn absent_opacity_is_treated_as_one() {
        assert_eq!(
            reason_of(None, Isolation::Auto, 0, MixBlendMode::Normal, 0),
            None
        );
    }

    #[test]
    fn isolate_forms_isolation_reason() {
        assert_eq!(
            reason_of(None, Isolation::Isolate, 0, MixBlendMode::Normal, 0),
            Some(EffectReason::ISOLATION)
        );
    }

    #[test]
    fn isolation_auto_forms_no_group() {
        assert_eq!(
            reason_of(None, Isolation::Auto, 0, MixBlendMode::Normal, 0),
            None
        );
    }

    #[test]
    fn non_empty_filter_forms_filter_reason() {
        assert_eq!(
            reason_of(None, Isolation::Auto, 1, MixBlendMode::Normal, 0),
            Some(EffectReason::FILTER)
        );
    }

    #[test]
    fn non_normal_blend_forms_mix_blend_reason() {
        assert_eq!(
            reason_of(None, Isolation::Auto, 0, MixBlendMode::Multiply, 0),
            Some(EffectReason::MIX_BLEND)
        );
    }

    #[test]
    fn non_empty_backdrop_forms_backdrop_filter_reason() {
        assert_eq!(
            reason_of(None, Isolation::Auto, 0, MixBlendMode::Normal, 1),
            Some(EffectReason::BACKDROP_FILTER)
        );
    }

    #[test]
    fn combined_triggers_or_their_reason_bits() {
        // opacity<1 AND isolate AND filter -> OR of the three bits.
        assert_eq!(
            reason_of(Some(0.25), Isolation::Isolate, 2, MixBlendMode::Normal, 0),
            Some(EffectReason::OPACITY | EffectReason::ISOLATION | EffectReason::FILTER)
        );
    }

    #[test]
    fn all_five_triggers_or_every_bit() {
        assert_eq!(
            reason_of(Some(0.1), Isolation::Isolate, 1, MixBlendMode::Screen, 1),
            Some(EffectReason::all())
        );
    }

    // The `reason_of` helper normalizes each no-op input to `None` before
    // calling the predicate, so it never exercises the predicate's own
    // `!= Normal` / `!is_empty()` guards. The system, however, passes the
    // raw present value (`Some(MixBlendMode::Normal)`, `Some(Filter(vec![]))`,
    // `Some(BackdropFilter(vec![]))`) — see Task 4. These three cases call
    // `effect_reason_for` directly with the present-but-no-op value to guard
    // that the predicate itself rejects them (a guard mutated to `is_some()`
    // must turn one RED).

    #[test]
    fn present_normal_blend_forms_no_group() {
        assert_eq!(
            effect_reason_for(
                None,
                Isolation::Auto,
                None,
                Some(MixBlendMode::Normal),
                None,
            ),
            None
        );
    }

    #[test]
    fn present_empty_filter_forms_no_group() {
        assert_eq!(
            effect_reason_for(None, Isolation::Auto, Some(Filter(vec![])), None, None),
            None
        );
    }

    #[test]
    fn present_empty_backdrop_forms_no_group() {
        assert_eq!(
            effect_reason_for(
                None,
                Isolation::Auto,
                None,
                None,
                Some(BackdropFilter(vec![])),
            ),
            None
        );
    }
}
