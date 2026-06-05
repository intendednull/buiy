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

/// A Buiy node — the parallel-to-`bevy_ui::Node` primitive. Marker that
/// this entity participates in Buiy's layout / render / a11y trees.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct Node;

/// Resolved layout output, written by `BuiyLayoutStep::WriteResolvedLayout`.
/// Read by render, picking, and any other downstream subsystem.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct ResolvedLayout {
    /// Top-left position in logical pixels (window-relative).
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
