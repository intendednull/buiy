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
