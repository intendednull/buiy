//! Buiy's core component types.
//!
//! Every Buiy component is small, public-fielded, observable, and
//! decomposed by concern. Layout components live in
//! `crate::layout::components`; this file holds the cross-cutting
//! `Node` marker, the shared `ResolvedLayout` output, and the temporary
//! `Visual` component (token-based rendering surface — eventual owner
//! is `buiy-render-pipeline-design`).
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

/// Visual surface: theme-token references and corner radius for the
/// Phase 0/1 render pipeline. Optional — entities without `Visual` are
/// skipped by the render extract.
///
/// **Temporary home.** This is a Phase 0 carry-over, kept alive in
/// Phase 1 only because the render extract still consumes
/// `background_token` / `border_radius`. The eventual owner of these
/// concerns is `buiy-render-pipeline-design` (unwritten as of Phase 1);
/// when that spec lands, `Visual` is replaced by richer
/// `Background` / `Border` / `Stroke` / etc. components.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct Visual {
    /// Theme token for the fill (e.g. `"color.surface.secondary"`).
    /// Empty string → render skips the fill (transparent).
    pub background_token: String,
    /// Theme token for foreground / text color (e.g. `"color.text.primary"`).
    /// Reserved for the text-rendering integration; Phase 1 render does
    /// not consume it.
    pub foreground_token: String,
    /// Uniform corner radius in logical pixels.
    pub border_radius: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolved_transform_default_is_identity() {
        assert_eq!(ResolvedTransform::default().matrix, Mat4::IDENTITY);
    }
}
