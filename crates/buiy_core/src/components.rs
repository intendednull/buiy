//! Buiy's core component types.
//!
//! Every Buiy component is small, public-fielded, observable, and
//! decomposed by concern. Layout components live in
//! `crate::layout::components`; this file holds the cross-cutting
//! `Node` marker and the shared `ResolvedLayout` output. The Phase-0
//! `Visual` token-rendering surface was replaced by the render-side
//! `Background` / `Border` components (`crate::render::components`) in the
//! render-pipeline spec.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.4.

use bevy::prelude::*;

// The `Node` `#[require]` list references the Style-decomposition components by
// their fully-qualified `crate::layout::components::…` paths rather than `use`-
// importing them: several names (`Scroll`, `Display`, …) collide with
// `bevy::prelude` glob entries, so an unqualified `#[require(Scroll)]` would
// resolve to bevy's input `Scroll`, not Buiy's layout component.

/// A Buiy node — the parallel-to-`bevy_ui::Node` primitive. Marker that
/// this entity participates in Buiy's layout / render / a11y trees.
///
/// `Node` `#[require]`s the full **`Style` decomposition** — the layout-input
/// components `sync_styles` queries **non-optionally** (`layout/systems.rs`):
/// `Display`, `BoxModel`, `Position`, `FlexParams`, `Overflow`, `Scroll`,
/// `GridParams`, `WritingMode`, `Container`, `MultiColumn`, `UiTransform`,
/// `Containment`, `Stacking`, `ContainIntrinsicSize` (all at their `Default`).
/// Without this, an entity with `Node` but missing any of them is **silently
/// skipped by layout** and never gets a `ResolvedLayout`. Requiring them makes
/// "this entity participates in layout" structural rather than conventional —
/// so a `bsn! { Node Display::… BoxModel { … } Children […] }` container is
/// layout-valid by construction, exactly the way the `Style` bundle was on the
/// `commands.spawn` path. (`WritingModeResolved` is *computed* by
/// `inherit_writing_mode` for every `Node`, so it is not required here.)
///
/// The `commands.spawn` ergonomic sugar — the [`Style`](crate::layout::Style)
/// bundle — still decomposes into these same components; the two authoring
/// paths now agree on the contract a layout node carries.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
#[require(
    crate::layout::Display,
    crate::layout::BoxModel,
    crate::layout::Position,
    crate::layout::FlexParams,
    crate::layout::Overflow,
    crate::layout::Scroll,
    crate::layout::GridParams,
    crate::layout::WritingMode,
    crate::layout::Container,
    crate::layout::MultiColumn,
    crate::layout::UiTransform,
    crate::layout::Containment,
    crate::layout::Stacking,
    crate::layout::ContainIntrinsicSize
)]
pub struct Node;

/// Resolved layout output, written by `BuiyLayoutStep::WriteResolvedLayout`.
/// Read by render, picking, and any other downstream subsystem.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct ResolvedLayout {
    /// Top-left position in logical pixels, **parent-relative** (Taffy's
    /// per-node `location`; only `PostTaffyPositionOverrides` substitutes it —
    /// sticky/table/multicol/anchor — never a general accumulation). This is NOT
    /// an absolute coordinate: the transform bridge (`render/bridge.rs`) is the
    /// sole accumulator (`position − ancestor_scroll` → `Transform` →
    /// `GlobalTransform`). Absolute consumers (picking, clip, render extract,
    /// overlays) MUST read `GlobalTransform.translation().truncate()`, never this
    /// field. See docs/specs/2026-06-22-buiy-widget-catalog-design/coordinate-space-correctness.md.
    pub position: Vec2,
    /// Size in logical pixels.
    pub size: Vec2,
}

/// Resolved composed transform, written by sub-pass 6e
/// (`transform_composition`) when an entity has a non-identity
/// `UiTransform` / `Translate` / `Rotate` / `Scale`. The render
/// handoff for transforms — mirrors how `ResolvedLayout` is the
/// render handoff for position+size. Absent on entities with an
/// identity transform (sub-pass 6e inserts it only when non-identity
/// and removes a stale one otherwise — spec § 7).
///
/// **Not** written into a Bevy `Transform`/`GlobalTransform` in
/// Phase 8 (deliberate divergence from spec § 2 approach (a): render
/// reads `ResolvedLayout` directly and `buiy_core` has no
/// `TransformPlugin` wiring — the Bevy-`Transform` ownership bridge
/// is a render-pipeline follow-up). Stored as `Mat4` (3D-ready,
/// represents perspective + arbitrary 4×4).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1, § 1.1, § 2.
#[derive(Component, Reflect, Clone, Debug, PartialEq)]
#[reflect(Component)]
pub struct ResolvedTransform {
    /// The composed transform matrix `M = T·R·S·M_transform`. A child
    /// point `p` is transformed as `M · p`.
    pub matrix: Mat4,
}

impl Default for ResolvedTransform {
    fn default() -> Self {
        Self {
            matrix: Mat4::IDENTITY,
        }
    }
}

/// Private render handoff for stacking: the paint order of every
/// descendant within this entity's stacking context, written by
/// sub-pass 6f (`stacking_context`) on each entity that forms a
/// stacking context (and removed when it stops forming one). Mirrors
/// how `ResolvedTransform` is the render handoff for the composed
/// matrix. Not author-set, but reflectable so devtools can inspect it.
///
/// `painters_z` is sorted per spec § 2.1: negative-`z_index` first,
/// then in-flow non-positioned (document order), then floats (always
/// empty in Buiy), then in-flow positioned with `z_index: Auto`
/// (document order), then positive `z_index`. Nested stacking contexts
/// appear as a single entry sorted by their own `z_index`. Top-layer
/// entities (spec § 4) are excluded from their parent context and
/// appended to the root context.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md § 2.1, § 5.
#[derive(Component, Reflect, Clone, Default, Debug, PartialEq)]
#[reflect(Component)]
pub struct StackingContext {
    pub painters_z: Vec<Entity>,
    /// Cross-ROOT paint rank: the order this context sorts against OTHER root
    /// contexts when it is itself a root (not listed as a painter in any other
    /// context). `0` for an in-flow root (paints first / bottom); a higher value
    /// for a TOP-LAYER root so a *parentless* top-layer tree (a dialog/popover
    /// authored outside the main content tree) paints LAST — over the whole window
    /// — instead of wherever its raw entity id falls (the M6 modal-under-shell
    /// bug). Set by sub-pass 6f from the entity's `Stacking.top_layer`
    /// (`render::extract::cross_root_rank`). Ignored for a NESTED context (it
    /// never reaches the root sort) and for the within-root order (a parented
    /// top-layer node escapes to its root's `painters_z` tail in 6f, untouched by
    /// this rank). The single source of truth read by every paint-order consumer
    /// (render node + glyph walks, picking depth) so they order roots identically.
    pub cross_root_rank: u8,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_transform_default_is_identity() {
        assert_eq!(ResolvedTransform::default().matrix, Mat4::IDENTITY);
    }

    #[test]
    fn stacking_context_default_is_empty() {
        assert!(StackingContext::default().painters_z.is_empty());
    }
}
