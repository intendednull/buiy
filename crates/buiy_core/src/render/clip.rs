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
//! Spec: docs/specs/2026-06-03-buiy-render-pipeline-design/clip-and-transform.md § A.

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
                self.min.x + px_edge(border.left),
                self.min.y + px_edge(border.top),
            ),
            max: Vec2::new(
                self.max.x - px_edge(border.right),
                self.max.y - px_edge(border.bottom),
            ),
        }
    }
}

/// Resolve a border-edge `Length` to px. Layout emits only `Length::Px`
/// borders into Taffy; any other unit means "no border inset here".
fn px_edge(len: Length) -> f32 {
    match len {
        Length::Px(v) => v,
        _ => 0.0,
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

    // Per-axis overflow: a `Visible` axis leaves that axis unbounded
    // (±infinity); only a clipping axis (Hidden/Clip/Scroll/Auto) binds.
    let (ox, oy) = overflow
        .map(|o| (o.x, o.y))
        .unwrap_or((OverflowMode::Visible, OverflowMode::Visible));
    let x_clips = !matches!(ox, OverflowMode::Visible);
    let y_clips = !matches!(oy, OverflowMode::Visible);
    let overflow_bound = (x_clips || y_clips).then_some(Aabb {
        min: Vec2::new(
            if x_clips {
                padding.min.x
            } else {
                f32::NEG_INFINITY
            },
            if y_clips {
                padding.min.y
            } else {
                f32::NEG_INFINITY
            },
        ),
        max: Vec2::new(
            if x_clips {
                padding.max.x
            } else {
                f32::INFINITY
            },
            if y_clips {
                padding.max.y
            } else {
                f32::INFINITY
            },
        ),
    });

    // `contain: paint` clips to the BORDER box (own box, no inset).
    let paint_bound = containment
        .filter(|c| c.contain.contains(ContainFlags::PAINT))
        .map(|_| own);

    match (overflow_bound, paint_bound) {
        (Some(a), Some(b)) => Some(a.intersect(b)),
        (Some(a), None) => Some(a),
        (None, Some(b)) => Some(b),
        (None, None) => None,
    }
}

/// Per-node layout/clip inputs read by the walk.
type ClipNodeData<'w> = (
    &'w ResolvedLayout,
    Option<&'w BoxModel>,
    Option<&'w Overflow>,
    Option<&'w Containment>,
    Option<&'w Display>,
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
    roots: Query<Entity, (With<Node>, Without<ChildOf>)>,
    children: Query<&Children>,
    // SPEC § A.4: ScrollOffset is intentionally NOT a clip input.
    nodes: Query<ClipNodeData>,
    existing: Query<(Option<&ClipRect>, Option<&AncestorClip>)>,
) {
    for root in roots.iter() {
        walk(root, None, &mut commands, &children, &nodes, &existing);
    }
}

/// Carries the running ancestor-clip AABB (`None` = no ancestor clips yet)
/// down the tree, writing each entity's [`ClipRect`] / [`AncestorClip`].
fn walk(
    entity: Entity,
    ancestor: Option<Aabb>,
    commands: &mut Commands,
    children: &Query<&Children>,
    nodes: &Query<ClipNodeData>,
    existing: &Query<(Option<&ClipRect>, Option<&AncestorClip>)>,
) {
    let Ok((rl, box_model, overflow, containment, display)) = nodes.get(entity) else {
        return;
    };
    // Prune Display::None / ContentVisibility::Hidden subtrees (spec § A.3).
    if matches!(display, Some(Display::None)) {
        return;
    }
    if containment.is_some_and(|c| c.content_visibility == ContentVisibility::Hidden) {
        return;
    }

    let own = Aabb::from_box(rl.position, rl.size);
    let clip: Option<Aabb> = ancestor.map(|a| a.intersect(own));
    reconcile(entity, clip, ancestor, commands, existing);

    // The clip box THIS node imposes on its descendants, folded into the
    // running ancestor AABB.
    let own_contribution = clip_contribution(own, box_model, overflow, containment);
    let child_ancestor = match (ancestor, own_contribution) {
        (Some(a), Some(c)) => Some(a.intersect(c)),
        (None, Some(c)) => Some(c),
        (some, None) => some,
    };

    if let Ok(kids) = children.get(entity) {
        for child in kids.iter() {
            walk(child, child_ancestor, commands, children, nodes, existing);
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

    match (clip, prev_clip) {
        (Some(a), prev) => {
            let next = ClipRect {
                min: a.min,
                max: a.max,
            };
            if prev != Some(&next) {
                commands.entity(entity).insert(next);
            }
        }
        (None, Some(_)) => {
            commands.entity(entity).remove::<ClipRect>();
        }
        (None, None) => {}
    }

    match (ancestor, prev_anc) {
        (Some(a), prev) => {
            let next = AncestorClip {
                min: a.min,
                max: a.max,
            };
            if prev != Some(&next) {
                commands.entity(entity).insert(next);
            }
        }
        (None, Some(_)) => {
            commands.entity(entity).remove::<AncestorClip>();
        }
        (None, None) => {}
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
    fn px_edge_resolves_px_only() {
        assert_eq!(px_edge(Length::Px(7.0)), 7.0);
        assert_eq!(px_edge(Length::Percent(50.0)), 0.0);
    }
}
