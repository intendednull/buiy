//! `write_clip_rects` render-prep pass: per-entity clip AABB computation.
//!
//! A top-down `Children` walk computes each entity's [`ClipRect`] (own border
//! box ∩ ancestor clips) and [`AncestorClip`] (ancestor clips only, for
//! `Outline`). Reads only layout output + the per-node clip inputs; emits NO
//! component when no ancestor clips the entity (absent ⇔ no clip). Runs in
//! `Update`, `.after(BuiySet::Animate).before(BuiySet::Picking)`, and reads no
//! `ScrollOffset` — the clip box is scroll-offset-independent (scroll moves
//! content via the transform bridge, not the clip box).
//!
//! This module also holds the render-side **consumption** of that clip:
//! [`scissor_rect`] turns a computed [`ClipRect`] into a physical-pixel wgpu
//! scissor rect. Render reads `ClipRect`; it never re-derives it (this pass is
//! the sole producer).
//!
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md § A
//!       (§ A.2 the producer + the scissor-rect derivation),
//!       docs/specs/2026-06-03-buiy-render-pipeline-design/paint-order-and-top-layer.md § 3.2.

use crate::components::{Node, ResolvedLayout};
use crate::layout::{
    BoxModel, ContainFlags, Containment, ContentVisibility, Display, Edges, Length, Overflow,
    OverflowMode,
};
use crate::render::components::{AncestorClip, ClipRect};
use bevy::prelude::*;

/// An axis-aligned clip box in logical px (y-down, window-relative).
#[derive(Clone, Copy, Debug, PartialEq)]
struct Aabb {
    min: Vec2,
    max: Vec2,
}

impl Aabb {
    /// The border box of an entity: top-left `position`, extent `size`.
    fn from_box(position: Vec2, size: Vec2) -> Self {
        Self {
            min: position,
            max: position + size,
        }
    }

    /// The axis-aligned bounds of an entity's border box AFTER its transform —
    /// the min/max of the 4 transformed corners. For an untransformed /
    /// translate-only entity this is bit-identical to
    /// `from_box(gt.translation(), size)` (an identity linear part maps each
    /// box-local corner to `translation + corner`). But a rotated/scaled entity's
    /// box no longer sits axis-aligned at `gt.translation()` — with a center
    /// `transform-origin` the translation is pivot-shifted — so the paint clip
    /// must bound the TRANSFORMED quad, not `[translation, translation+size]`
    /// (else a rotated element clips its own off-axis content away).
    fn from_transformed_box(gt: &GlobalTransform, size: Vec2) -> Self {
        // Fast path: no rotation/scale (identity linear part). The box is
        // axis-aligned at the translation — bit-identical to the prior
        // `from_box(gt.translation(), size)` clip (the untransformed corpus).
        if gt.affine().matrix3 == bevy::math::Mat3A::IDENTITY {
            return Self::from_box(gt.translation().truncate(), size);
        }
        let corners = [
            Vec2::ZERO,
            Vec2::new(size.x, 0.0),
            Vec2::new(0.0, size.y),
            size,
        ];
        let mut min = Vec2::splat(f32::INFINITY);
        let mut max = Vec2::splat(f32::NEG_INFINITY);
        for c in corners {
            let p = gt.transform_point(c.extend(0.0)).truncate();
            min = min.min(p);
            max = max.max(p);
        }
        Self { min, max }
    }

    /// Component-wise AABB intersection (may be degenerate if disjoint).
    fn intersect(self, other: Aabb) -> Aabb {
        Aabb {
            min: self.min.max(other.min),
            max: self.max.min(other.max),
        }
    }

    /// Inset by border edges only (border box → padding box). `border`
    /// edges are resolved px-only (non-px units → 0.0, matching the spec
    /// absent-default: a missing/unsupported border contributes no inset).
    fn inset_border(self, border: &Edges) -> Aabb {
        Aabb {
            min: Vec2::new(
                self.min.x + px_or_zero(border.left),
                self.min.y + px_or_zero(border.top),
            ),
            max: Vec2::new(
                self.max.x - px_or_zero(border.right),
                self.max.y - px_or_zero(border.bottom),
            ),
        }
    }
}

impl From<Aabb> for ClipRect {
    fn from(a: Aabb) -> Self {
        Self {
            min: a.min,
            max: a.max,
        }
    }
}

impl From<Aabb> for AncestorClip {
    fn from(a: Aabb) -> Self {
        Self {
            min: a.min,
            max: a.max,
        }
    }
}

/// Resolve a `Length` to px, px-only: `Length::Px(v) => v`, every other unit
/// (Percent, Cq*, Fr) => 0.0. The render-internal "no inset / no radius for an
/// unsupported unit" rule shared by the clip border inset and the Phase-0
/// radius read (mod.rs). Deliberately px-only — NOT a public `Length` method,
/// since the px-vs-Percent distinction is render-specific (layout's
/// `length_to_px` resolves Percent and is a different contract).
pub(crate) fn px_or_zero(len: Length) -> f32 {
    match len {
        Length::Px(v) => v,
        _ => 0.0,
    }
}

/// A wgpu scissor rect in **physical** pixels: `(x, y, width, height)`.
/// `None` ⇒ the clip is degenerate (empty) ⇒ render must skip the entity.
pub type ScissorRect = Option<(u32, u32, u32, u32)>;

/// Derive a physical-pixel wgpu scissor rect from a logical-px [`ClipRect`].
///
/// `scale_factor` converts logical → physical px (the same scalar the view
/// uniform folds in, clip-and-transform.md § B.4). The result is clamped to
/// `[0, view_physical]` on both axes. A degenerate clip (`min.x >= max.x` or
/// `min.y >= max.y`, clip-and-transform.md § A.2) returns `None` — the entity
/// is fully clipped away. The clip is already y-down window-relative, the same
/// space wgpu's scissor expects, so NO y-flip happens here.
pub fn scissor_rect(clip: &ClipRect, scale_factor: f32, view_physical: UVec2) -> ScissorRect {
    if clip.min.x >= clip.max.x || clip.min.y >= clip.max.y {
        return None;
    }
    let min = (clip.min * scale_factor).max(Vec2::ZERO);
    let max = (clip.max * scale_factor)
        .min(Vec2::new(view_physical.x as f32, view_physical.y as f32))
        .max(Vec2::ZERO);
    if min.x >= max.x || min.y >= max.y {
        return None; // clamped away entirely (off-screen)
    }
    Some((
        min.x as u32,
        min.y as u32,
        (max.x - min.x) as u32,
        (max.y - min.y) as u32,
    ))
}

/// Which clip a primitive scissors against. A fill / background / border uses
/// the entity's own-box-intersected [`ClipRect`]. An `Outline` (painted outside
/// the border box) uses [`AncestorClip`] — the ancestor intersection WITHOUT the
/// own-box step — so a focus ring outside the box is cropped by ancestors but
/// not erased by the entity's own box (clip-and-transform.md § A.2; § 3.2).
///
/// Returns the AABB to scissor against, or `None` ⇒ no scissor (unclipped).
pub fn clip_for_primitive(
    is_outline: bool,
    own_clip: Option<&ClipRect>,
    ancestor_clip: Option<&AncestorClip>,
) -> Option<ClipRect> {
    if is_outline {
        ancestor_clip.map(|a| ClipRect {
            min: a.min,
            max: a.max,
        })
    } else {
        own_clip.copied()
    }
}

/// The clip box `entity` imposes on its descendants, or `None` if it does not
/// clip. Overflow contributes the padding box per clipping axis;
/// `contain: paint` contributes the border box. A `Visible` overflow axis
/// contributes no bound on that axis. A node that is both an overflow clipper
/// and paint-contained contributes the intersection of the two bounds.
fn clip_contribution(
    own: Aabb,
    box_model: Option<&BoxModel>,
    overflow: Option<&Overflow>,
    containment: Option<&Containment>,
) -> Option<Aabb> {
    let zero = Edges::default();
    let border = box_model.map(|b| &b.border).unwrap_or(&zero);
    let padding = own.inset_border(border);

    // Per-axis overflow: start fully unbounded, then bind only the clipping
    // axes (Hidden/Clip/Scroll/Auto) to the padding box; a `Visible` axis
    // stays at ±infinity. Exception: a scroll container clips BOTH axes — CSS
    // computes a `Visible` axis to `auto` when its sibling axis scrolls, so
    // `overflow-x: scroll; overflow-y: visible` still has a 2D viewport.
    let (ox, oy) = overflow
        .map(|o| (o.x, o.y))
        .unwrap_or((OverflowMode::Visible, OverflowMode::Visible));
    let scroll_container = overflow.is_some_and(|o| o.is_scroll_container());
    let x_clips = scroll_container || !matches!(ox, OverflowMode::Visible);
    let y_clips = scroll_container || !matches!(oy, OverflowMode::Visible);
    let overflow_bound = (x_clips || y_clips).then(|| {
        let mut b = Aabb {
            min: Vec2::splat(f32::NEG_INFINITY),
            max: Vec2::splat(f32::INFINITY),
        };
        if x_clips {
            b.min.x = padding.min.x;
            b.max.x = padding.max.x;
        }
        if y_clips {
            b.min.y = padding.min.y;
            b.max.y = padding.max.y;
        }
        b
    });

    // `contain: paint` clips to the BORDER box (own box, no inset).
    let paint_bound = containment
        .filter(|c| c.contain.contains(ContainFlags::PAINT))
        .map(|_| own);

    intersect_opt(overflow_bound, paint_bound)
}

/// Intersect two optional clip boxes: both present → their intersection; one
/// present → that one; neither → `None`.
fn intersect_opt(a: Option<Aabb>, b: Option<Aabb>) -> Option<Aabb> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.intersect(b)),
        (a, b) => a.or(b),
    }
}

/// Per-node layout/clip inputs read by the walk. `ResolvedLayout` is optional
/// so the walk can still read `Display` / `ContentVisibility` (to prune + clear
/// stale clips) on a node that lacks a resolved box this frame.
type ClipNodeData<'w> = (
    Option<&'w ResolvedLayout>,
    Option<&'w BoxModel>,
    Option<&'w Overflow>,
    Option<&'w Containment>,
    Option<&'w Display>,
    Option<&'w GlobalTransform>,
);

/// Render-prep — computes each entity's [`ClipRect`] (own box ∩ ancestor
/// clips) and [`AncestorClip`] (ancestor clips only) by a top-down `Children`
/// walk. Emits NO component when no ancestor clips the entity. Prunes
/// `Display::None` / `ContentVisibility::Hidden` subtrees (spec § A.3, shared
/// with paint-order § 5). Reads no `ScrollOffset` (spec § A.4).
///
/// Runs in `Update`, `.after(BuiySet::Animate).before(BuiySet::Picking)`
/// (architecture.md § 5.2), so picking + extract see settled clips.
pub fn write_clip_rects(
    mut commands: Commands,
    all_nodes: Query<Entity, With<Node>>,
    child_of: Query<&ChildOf>,
    node_marker: Query<(), With<Node>>,
    children: Query<&Children>,
    // SPEC § A.4: ScrollOffset is intentionally NOT a clip input.
    nodes: Query<ClipNodeData, With<Node>>,
    existing: Query<(Option<&ClipRect>, Option<&AncestorClip>)>,
) {
    // A clip root is a Node with no `ChildOf`, OR whose `ChildOf` parent is
    // not a Node — the same two-disjunct root definition layout uses (spec
    // § A.3). Seeding only detached Nodes would silently drop the clip walk
    // for a Buiy subtree parented under a non-Node Bevy entity.
    for entity in all_nodes.iter() {
        let is_root = match child_of.get(entity) {
            Ok(parent) => node_marker.get(parent.parent()).is_err(),
            Err(_) => true,
        };
        if is_root {
            walk(entity, None, &mut commands, &children, &nodes, &existing);
        }
    }
}

/// Carries the running ancestor-clip AABB (`None` = no ancestor clips yet)
/// down the tree, writing each entity's [`ClipRect`] / [`AncestorClip`].
fn walk(
    entity: Entity,
    ancestor: Option<Aabb>,
    commands: &mut Commands,
    children: &Query<&Children>,
    nodes: &Query<ClipNodeData, With<Node>>,
    existing: &Query<(Option<&ClipRect>, Option<&AncestorClip>)>,
) {
    // A non-Node entity in the Children tree is not a Buiy node — skip it and
    // its subtree (clip applies to the Buiy node tree).
    let Ok((rl, box_model, overflow, containment, display, gt)) = nodes.get(entity) else {
        return;
    };

    // Prune Display::None / ContentVisibility::Hidden subtrees (spec § A.3): a
    // pruned node paints nothing, so it AND its descendants must drop any clip
    // a prior frame wrote, and compute no new clip below.
    if matches!(display, Some(Display::None))
        || containment.is_some_and(|c| c.content_visibility == ContentVisibility::Hidden)
    {
        clear_subtree(entity, commands, children, existing);
        return;
    }

    // Without a resolved box (or without a GlobalTransform — never bridged) this
    // node cannot be clipped; clear its own stale clip but keep walking
    // descendants with the unchanged ancestor clip.
    let child_ancestor = match (rl, gt) {
        (Some(rl), Some(gt)) => {
            // C1: absolute basis, not rl.position. The transformed-corner AABB is
            // bit-identical to `from_box(gt.translation, size)` for untransformed /
            // translate-only nodes, but correctly bounds a rotated/scaled node's
            // off-axis paint (a center-pivot rotation shifts `gt.translation()`
            // away from the box top-left — using it raw clips the content away).
            let own = Aabb::from_transformed_box(gt, rl.size);
            let clip = ancestor.map(|a| a.intersect(own));
            reconcile(entity, clip, ancestor, commands, existing);
            // The clip box THIS node imposes on its descendants, folded into
            // the running ancestor AABB.
            let contribution = clip_contribution(own, box_model, overflow, containment);
            intersect_opt(ancestor, contribution)
        }
        // No resolved box OR no GlobalTransform (never bridged): cannot be
        // clipped or contribute a clip — clear any stale own clip, keep walking
        // descendants with the unchanged ancestor (D2: no fallback to rl.position).
        _ => {
            reconcile(entity, None, None, commands, existing);
            ancestor
        }
    };

    if let Ok(kids) = children.get(entity) {
        for child in kids.iter() {
            walk(child, child_ancestor, commands, children, nodes, existing);
        }
    }
}

/// Remove any stale [`ClipRect`] / [`AncestorClip`] on `entity` and its whole
/// subtree — a pruned (`Display::None` / `ContentVisibility::Hidden`) node
/// paints nothing, so it and its descendants must drop the clips a prior frame
/// wrote (and compute no new clip).
fn clear_subtree(
    entity: Entity,
    commands: &mut Commands,
    children: &Query<&Children>,
    existing: &Query<(Option<&ClipRect>, Option<&AncestorClip>)>,
) {
    reconcile(entity, None, None, commands, existing);
    if let Ok(kids) = children.get(entity) {
        for child in kids.iter() {
            clear_subtree(child, commands, children, existing);
        }
    }
}

/// Insert/remove [`ClipRect`] (from `clip`) and [`AncestorClip`] (from
/// `ancestor`) only when they differ from what is already stored — a
/// steady-state frame issues zero structural ops (change-gate, spec § A.3).
fn reconcile(
    entity: Entity,
    clip: Option<Aabb>,
    ancestor: Option<Aabb>,
    commands: &mut Commands,
    existing: &Query<(Option<&ClipRect>, Option<&AncestorClip>)>,
) {
    let (prev_clip, prev_anc) = existing.get(entity).unwrap_or((None, None));
    reconcile_one(commands, entity, clip.map(ClipRect::from), prev_clip);
    reconcile_one(commands, entity, ancestor.map(AncestorClip::from), prev_anc);
}

/// Insert `next` only when it differs from `prev`; remove the component when
/// `next` is absent but a stale one exists. The change-gate that makes a
/// steady-state frame issue zero structural ops (spec § A.3). `pub(crate)`:
/// the `write_paint_skip` visibility pass (render/visibility.rs) shares this
/// exact reconcile for its `ComputedPaintSkip` marker.
pub(crate) fn reconcile_one<C: Component + PartialEq>(
    commands: &mut Commands,
    entity: Entity,
    next: Option<C>,
    prev: Option<&C>,
) {
    match next {
        Some(n) if prev != Some(&n) => {
            commands.entity(entity).insert(n);
        }
        None if prev.is_some() => {
            commands.entity(entity).remove::<C>();
        }
        // unchanged (Some == prev) or absent-and-was-absent: no structural op.
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_box_makes_min_max_from_pos_size() {
        let a = Aabb::from_box(Vec2::new(10.0, 20.0), Vec2::new(100.0, 50.0));
        assert_eq!(a.min, Vec2::new(10.0, 20.0));
        assert_eq!(a.max, Vec2::new(110.0, 70.0));
    }

    #[test]
    fn transformed_box_clip_matches_from_box_for_translate() {
        // Identity linear part (translate only) → bit-identical to from_box, so the
        // untransformed corpus's clips are unchanged.
        use bevy::transform::components::{GlobalTransform, Transform};
        let gt = GlobalTransform::from(Transform::from_xyz(10.0, 20.0, 0.0));
        let a = Aabb::from_transformed_box(&gt, Vec2::new(100.0, 50.0));
        let b = Aabb::from_box(Vec2::new(10.0, 20.0), Vec2::new(100.0, 50.0));
        assert_eq!(a, b);
    }

    #[test]
    fn transformed_box_clip_bounds_a_rotated_box() {
        // A 16×16 box rotated +90° about the origin: corners (0,0),(16,0),(0,16),
        // (16,16) → (0,0),(0,16),(-16,0),(-16,16), so the AABB is [-16,0]×[0,16].
        // The pre-fix `from_box(gt.translation(), size)` would have returned
        // [0,16]×[0,16] (translation = origin), clipping the rotated content away.
        use bevy::math::Quat;
        use bevy::transform::components::{GlobalTransform, Transform};
        use std::f32::consts::FRAC_PI_2;
        let gt = GlobalTransform::from(Transform::from_rotation(Quat::from_rotation_z(FRAC_PI_2)));
        let a = Aabb::from_transformed_box(&gt, Vec2::splat(16.0));
        assert!(
            (a.min - Vec2::new(-16.0, 0.0)).length() < 1e-4,
            "min {:?}",
            a.min
        );
        assert!(
            (a.max - Vec2::new(0.0, 16.0)).length() < 1e-4,
            "max {:?}",
            a.max
        );
    }

    #[test]
    fn intersect_takes_inner_overlap() {
        let a = Aabb {
            min: Vec2::new(0.0, 0.0),
            max: Vec2::new(100.0, 100.0),
        };
        let b = Aabb {
            min: Vec2::new(50.0, 25.0),
            max: Vec2::new(200.0, 75.0),
        };
        let i = a.intersect(b);
        assert_eq!(i.min, Vec2::new(50.0, 25.0));
        assert_eq!(i.max, Vec2::new(100.0, 75.0));
    }

    #[test]
    fn inset_border_makes_padding_box() {
        let bb = Aabb {
            min: Vec2::ZERO,
            max: Vec2::splat(100.0),
        };
        let padding = bb.inset_border(&Edges::all(10.0));
        assert_eq!(padding.min, Vec2::splat(10.0));
        assert_eq!(padding.max, Vec2::splat(90.0));
    }

    #[test]
    fn px_or_zero_resolves_px_only() {
        assert_eq!(px_or_zero(Length::Px(7.0)), 7.0);
        assert_eq!(px_or_zero(Length::Percent(50.0)), 0.0);
    }
}

#[cfg(test)]
mod scissor_tests {
    use super::*;

    #[test]
    fn full_box_maps_to_full_physical_rect() {
        let clip = ClipRect {
            min: Vec2::ZERO,
            max: Vec2::new(800.0, 600.0),
        };
        let s = scissor_rect(&clip, 1.0, UVec2::new(800, 600));
        assert_eq!(s, Some((0, 0, 800, 600)));
    }

    #[test]
    fn scale_factor_scales_to_physical_px() {
        let clip = ClipRect {
            min: Vec2::new(10.0, 20.0),
            max: Vec2::new(110.0, 220.0),
        };
        let s = scissor_rect(&clip, 2.0, UVec2::new(1600, 1200));
        // (10,20)..(110,220) logical → (20,40) origin, 200x400 physical.
        assert_eq!(s, Some((20, 40, 200, 400)));
    }

    #[test]
    fn clip_is_clamped_to_view() {
        // A clip wider than the view clamps to the view bounds (no overflow).
        let clip = ClipRect {
            min: Vec2::new(-50.0, -50.0),
            max: Vec2::new(2000.0, 2000.0),
        };
        let s = scissor_rect(&clip, 1.0, UVec2::new(800, 600));
        assert_eq!(s, Some((0, 0, 800, 600)));
    }

    #[test]
    fn degenerate_clip_returns_none() {
        // min.x >= max.x ⇒ empty rect ⇒ skip (clip-and-transform.md § A.2).
        let clip = ClipRect {
            min: Vec2::new(100.0, 0.0),
            max: Vec2::new(100.0, 50.0),
        };
        assert_eq!(scissor_rect(&clip, 1.0, UVec2::new(800, 600)), None);
        let clip2 = ClipRect {
            min: Vec2::new(0.0, 80.0),
            max: Vec2::new(50.0, 40.0),
        };
        assert_eq!(scissor_rect(&clip2, 1.0, UVec2::new(800, 600)), None);
    }
}

#[cfg(test)]
mod outline_clip_tests {
    use super::*;

    #[test]
    fn fill_uses_own_clip() {
        let own = ClipRect {
            min: Vec2::ZERO,
            max: Vec2::splat(50.0),
        };
        let anc = AncestorClip {
            min: Vec2::ZERO,
            max: Vec2::splat(200.0),
        };
        assert_eq!(clip_for_primitive(false, Some(&own), Some(&anc)), Some(own));
    }

    #[test]
    fn outline_uses_ancestor_clip_not_own() {
        let own = ClipRect {
            min: Vec2::ZERO,
            max: Vec2::splat(50.0),
        };
        let anc = AncestorClip {
            min: Vec2::ZERO,
            max: Vec2::splat(200.0),
        };
        // Outline must NOT be clipped to the 50x50 own box — it uses the 200x200
        // ancestor clip, so the ring outside the border box survives.
        let got = clip_for_primitive(true, Some(&own), Some(&anc));
        assert_eq!(
            got,
            Some(ClipRect {
                min: anc.min,
                max: anc.max
            })
        );
    }

    #[test]
    fn absent_clips_are_unclipped() {
        assert_eq!(clip_for_primitive(false, None, None), None);
        assert_eq!(clip_for_primitive(true, None, None), None);
    }
}
