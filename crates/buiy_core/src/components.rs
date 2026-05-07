//! Buiy's core component types.
//!
//! Every Buiy component is small, public-fielded, observable, and decomposed
//! by concern. Every component derives `Reflect + Default + Clone + Component`
//! (and Bevy 0.18's `Reflect` derive auto-generates `FromReflect`). See:
//! docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.4.

use bevy::prelude::*;

/// A Buiy node — the parallel-to-bevy_ui::Node primitive. Marker that this
/// entity participates in Buiy's layout / render / a11y trees.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct Node;

/// Box-model + layout style. Not exhaustive in Phase 0 — only the surface
/// the layout system reads.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct Style {
    /// Width in logical pixels. 0.0 = auto.
    pub width: f32,
    /// Height in logical pixels. 0.0 = auto.
    pub height: f32,
    /// Padding on all sides.
    pub padding: f32,
    /// Margin on all sides.
    pub margin: f32,
    /// Border radius (uniform; per-corner is a later sub-spec).
    pub border_radius: f32,
    /// Flex direction. 0 = Row, 1 = Column. Mapped to Taffy in `layout.rs`.
    pub flex_direction: u8,
    /// Token reference for background color (e.g., "color.surface.primary").
    pub background_token: String,
    /// Token reference for foreground/text color.
    pub foreground_token: String,
}

/// Resolved layout output, written by the layout system in `BuiySet::Layout`.
/// Read by render and picking in subsequent sets.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct ResolvedLayout {
    /// Top-left position in logical pixels (window-relative).
    pub position: Vec2,
    /// Size in logical pixels.
    pub size: Vec2,
}
