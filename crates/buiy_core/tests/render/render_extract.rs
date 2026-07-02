//! Headless unit tests for the per-view extract mapping. Pure-CPU: no wgpu
//! adapter, no RenderApp. Mirrors tests/render_instance.rs conventions.

use bevy::prelude::*;
use buiy_core::render::color::{ColorToken, SystemColorKeyword, ThemeContract, resolve_token};
use buiy_core::theme::{Theme, default_light_theme, forced_colors_theme};

#[test]
fn transparent_token_resolves_to_none() {
    let theme = Theme::default();
    let c = resolve_token(&ColorToken::Transparent, &theme);
    assert_eq!(c, Color::NONE);
}

#[test]
fn known_token_resolves_to_theme_color() {
    let theme = default_light_theme();
    let c = resolve_token(&ColorToken::SurfacePrimary, &theme);
    assert_eq!(c, Color::WHITE);
}

#[test]
fn system_color_resolves_through_forced_theme_on_the_r5_path() {
    // The R5 extract path (`extracted_node_for` → `color::resolve_token`) must
    // resolve `SystemColor(kw)` against the active theme's system-color map, not
    // hardcode the sentinel. Under the forced-colors stub theme, `Canvas` is a
    // real high-contrast color — NOT the magenta sentinel (the divergence the
    // old `extract::resolve_color_token` introduced). color-and-forced-colors.md
    // § 3.1.
    let theme = forced_colors_theme();
    let bg = Background {
        color: ColorToken::SystemColor(SystemColorKeyword::Canvas),
    };
    let layout = ResolvedLayout {
        position: Vec2::ZERO,
        size: Vec2::splat(10.0),
    };
    let gt = GlobalTransform::IDENTITY;
    let node = extracted_node_for(
        Entity::from_raw_u32(42).unwrap(),
        &gt,
        &layout,
        Some(&bg),
        None,
        &theme,
    );
    assert_ne!(
        node.color,
        Color::srgb(1.0, 0.0, 1.0),
        "SystemColor(Canvas) must resolve through the forced theme, not the sentinel"
    );
    assert_eq!(
        node.color,
        theme.resolve(ColorToken::SystemColor(SystemColorKeyword::Canvas)),
        "the R5 path must resolve to the forced theme's Canvas color"
    );
}

// The per-entity skip-predicate tests (`node_skip_reason`) moved with the
// predicate to `render/visibility.rs` (its `#[cfg(test)]` module) when the
// subtree visibility-suppression pass made the computed `ComputedPaintSkip`
// marker extract's single skip source — the predicate is now producer-side.
// The pass itself is covered by tests/render_paint_skip.rs.

use buiy_core::components::ResolvedLayout;
use buiy_core::layout::{Stacking, TopLayer};
use buiy_core::render::components::{AncestorClip, Background, ClipRect};
use buiy_core::render::extract::{effective_clip, extracted_node_for};

#[test]
fn extracted_node_carries_box_and_resolved_color() {
    let theme = default_light_theme();
    let layout = ResolvedLayout {
        position: Vec2::new(10.0, 20.0),
        size: Vec2::new(100.0, 40.0),
    };
    let gt = GlobalTransform::from_translation(Vec3::new(10.0, 20.0, 0.0));
    let bg = Background {
        color: ColorToken::SurfacePrimary,
    };
    let entity = Entity::from_raw_u32(7).unwrap();

    let node = extracted_node_for(entity, &gt, &layout, Some(&bg), None, &theme);

    assert_eq!(node.entity, entity);
    assert_eq!(node.size, Vec2::new(100.0, 40.0));
    // Position is taken from GlobalTransform.translation (xy), per pillar 5.
    assert_eq!(node.position, Vec2::new(10.0, 20.0));
    assert_eq!(node.color, Color::WHITE);
}

#[test]
fn absent_background_is_transparent() {
    let theme = Theme::default();
    let layout = ResolvedLayout {
        position: Vec2::ZERO,
        size: Vec2::splat(8.0),
    };
    let gt = GlobalTransform::IDENTITY;
    let node = extracted_node_for(
        Entity::from_raw_u32(1).unwrap(),
        &gt,
        &layout,
        None,
        None,
        &theme,
    );
    assert_eq!(node.color, Color::NONE);
}

#[test]
fn resolve_background_color_prefers_animated_then_token_then_none() {
    // Parity § 2 REFINE: `resolve_background_color` auto-composites a live
    // `AnimatedBackgroundColor` (the crossfade a `BackgroundColorTween` writes)
    // OVER the static `Background` token, falls back to the resolved token when
    // no animation is present, and to `Color::NONE` (transparent) when neither
    // is present. The extract production loop wires this per node; this pins the
    // pure resolution priority. End-to-end wiring: render_extract_composite.rs.
    use buiy_core::animation::AnimatedBackgroundColor;
    use buiy_core::render::extract::resolve_background_color;

    let theme = default_light_theme();
    let bg = Background {
        color: ColorToken::SurfacePrimary,
    };
    let token_color = resolve_token(&bg.color, &theme);
    let animated = AnimatedBackgroundColor(Color::srgb(0.12, 0.34, 0.56));
    assert_ne!(
        animated.0, token_color,
        "test setup: animated must differ from token"
    );

    // Animated present ⇒ paint the interpolated color (token ignored).
    assert_eq!(
        resolve_background_color(Some(&bg), Some(&animated), &theme),
        animated.0
    );
    // Animated absent ⇒ resolve the Background token.
    assert_eq!(
        resolve_background_color(Some(&bg), None, &theme),
        token_color
    );
    // No background and no animation ⇒ transparent.
    assert_eq!(resolve_background_color(None, None, &theme), Color::NONE);
    // Animated present with NO background still paints the animated color (the
    // override does not require a token to ride on).
    assert_eq!(
        resolve_background_color(None, Some(&animated), &theme),
        animated.0
    );
}

#[test]
fn extracted_node_for_carries_clip_when_provided() {
    // The per-primitive clip AABB is threaded through extract: a `Some(clip)`
    // is carried verbatim onto the record; `None` (no clip / top-layer
    // sentinel) stays `None`. This is the CPU half of the fragment-discard
    // clip — render packs `None` to the full-view sentinel (R8b § 3.2).
    let theme = Theme::default();
    let layout = ResolvedLayout {
        position: Vec2::new(10.0, 20.0),
        size: Vec2::new(100.0, 40.0),
    };
    let gt = GlobalTransform::IDENTITY;
    let entity = Entity::from_raw_u32(9).unwrap();
    let clip = ClipRect {
        min: Vec2::new(10.0, 20.0),
        max: Vec2::new(60.0, 50.0),
    };

    let clipped = extracted_node_for(entity, &gt, &layout, None, Some(&clip), &theme);
    assert_eq!(clipped.clip, Some(clip), "Some(clip) is carried verbatim");

    let unclipped = extracted_node_for(entity, &gt, &layout, None, None, &theme);
    assert_eq!(unclipped.clip, None, "absent clip stays None (sentinel)");
}

#[test]
fn assemble_preserves_clip_per_entity() {
    // Each painter's record keeps the clip the build closure stamped on it; the
    // forward walk never mixes one entity's clip into another's.
    let order = vec![e(1), e(2), e(3)];
    let clip2 = ClipRect {
        min: Vec2::ZERO,
        max: Vec2::splat(50.0),
    };
    let nodes = assemble_in_paint_order(&order, |x| {
        Some(ExtractedNode {
            entity: x,
            position: Vec2::ZERO,
            size: Vec2::ONE,
            color: Color::WHITE,
            // Only entity 2 carries a clip; the others stay unclipped.
            clip: (x == e(2)).then_some(clip2),
            group: None,
            affine: [[1.0, 0.0], [0.0, 1.0]],
            outline: None,
            border: None,
            shadows: Vec::new(),
            gradients: Vec::new(),
        })
    });
    let clips: Vec<Option<ClipRect>> = nodes.nodes.iter().map(|n| n.clip).collect();
    assert_eq!(clips, vec![None, Some(clip2), None]);
}

// These guard the production `effective_clip` (the per-entity decision
// `extract_buiy_nodes` runs before `extracted_node_for`): a top-layer member is
// forced to the `None` full-view sentinel, an in-flow member takes
// `clip_for_primitive`. Each assertion exercises the real branch and the spec
// property each half implements.
#[test]
fn top_layer_entity_gets_none_clip_regardless_of_clip_rect() {
    // paint-order-and-top-layer.md § 3.2: a top-layer member escapes every
    // ancestor clip and paints over the full view, so it ALWAYS resolves to the
    // `None` sentinel — even when a `ClipRect`/`AncestorClip` is present on the
    // entity (a stale clip from before it was promoted, say). Any non-`None`
    // `TopLayer` variant triggers the escape.
    let clip = ClipRect {
        min: Vec2::new(10.0, 20.0),
        max: Vec2::new(60.0, 50.0),
    };
    let anc = AncestorClip {
        min: Vec2::ZERO,
        max: Vec2::splat(200.0),
    };
    for variant in [
        TopLayer::Modal,
        TopLayer::Popover,
        TopLayer::Tooltip,
        TopLayer::Fullscreen,
    ] {
        let stacking = Stacking {
            top_layer: variant,
            ..Default::default()
        };
        assert_eq!(
            effective_clip(Some(&stacking), Some(&clip), Some(&anc)),
            None,
            "{variant:?} top-layer member must get the full-view sentinel"
        );
    }
}

#[test]
fn in_flow_clipped_entity_gets_clip_from_clip_for_primitive() {
    // An in-flow member (no `Stacking`, or `TopLayer::None`) takes the fill clip
    // straight from `clip_for_primitive(false, …)`: its own-box `ClipRect` when
    // present, `None` when nothing clips it. The branch never substitutes the
    // sentinel for an in-flow node.
    let clip = ClipRect {
        min: Vec2::new(10.0, 20.0),
        max: Vec2::new(60.0, 50.0),
    };
    let anc = AncestorClip {
        min: Vec2::ZERO,
        max: Vec2::splat(200.0),
    };

    // No Stacking at all → in-flow → own-box clip carried verbatim.
    assert_eq!(
        effective_clip(None, Some(&clip), Some(&anc)),
        Some(clip),
        "in-flow fill clips to its own box (clip_for_primitive(false, …))"
    );
    // `TopLayer::None` is in-flow too → same own-box clip.
    let in_flow = Stacking::default();
    assert_eq!(
        effective_clip(Some(&in_flow), Some(&clip), Some(&anc)),
        Some(clip),
        "TopLayer::None is in-flow, not a top-layer escape"
    );
    // No clip inputs → unclipped (`None`), NOT promoted to the sentinel by the
    // top-layer branch — `clip_for_primitive` itself returns `None`.
    assert_eq!(effective_clip(None, None, None), None);
}

#[test]
fn extracted_node_position_follows_global_transform() {
    // The bridge folds ResolvedLayout.position + ResolvedTransform into a Bevy
    // Transform; render reads the propagated GlobalTransform, NOT
    // ResolvedLayout.position directly. A transformed entity's painted origin
    // is the GlobalTransform translation.
    let theme = Theme::default();
    let layout = ResolvedLayout {
        position: Vec2::new(0.0, 0.0), // pre-transform box origin
        size: Vec2::splat(50.0),
    };
    let gt = GlobalTransform::from_translation(Vec3::new(200.0, 300.0, 0.0));
    let node = extracted_node_for(
        Entity::from_raw_u32(2).unwrap(),
        &gt,
        &layout,
        None,
        None,
        &theme,
    );
    assert_eq!(node.position, Vec2::new(200.0, 300.0));
}

#[test]
fn extracted_node_carries_affine_basis_from_global_transform() {
    // The 2D linear part of GlobalTransform's affine is carried onto the record
    // so the GPU vertex stage can apply rotation/scale (not just the
    // translation). A 90deg z-rotation is ASYMMETRIC, so it catches a transpose:
    // R(90) maps x_axis -> (0,1) and y_axis -> (-1,0), so col0 = [0,1] and
    // col1 = [-1,0]. The translation.xy must still be the painted top-left.
    use std::f32::consts::FRAC_PI_2;
    let theme = Theme::default();
    let layout = ResolvedLayout {
        position: Vec2::ZERO,
        size: Vec2::splat(50.0),
    };
    let affine3 = bevy::math::Affine3A::from_rotation_translation(
        Quat::from_rotation_z(FRAC_PI_2),
        Vec3::new(11.0, 22.0, 0.0),
    );
    let gt = GlobalTransform::from(affine3);
    let node = extracted_node_for(
        Entity::from_raw_u32(3).unwrap(),
        &gt,
        &layout,
        None,
        None,
        &theme,
    );
    // col0 = xy of x_axis, col1 = xy of y_axis (columns, NOT rows).
    let eps = 1e-5;
    assert!(
        (node.affine[0][0] - 0.0).abs() < eps,
        "m00 = {}",
        node.affine[0][0]
    );
    assert!(
        (node.affine[0][1] - 1.0).abs() < eps,
        "m10 = {}",
        node.affine[0][1]
    );
    assert!(
        (node.affine[1][0] - -1.0).abs() < eps,
        "m01 = {}",
        node.affine[1][0]
    );
    assert!(
        (node.affine[1][1] - 0.0).abs() < eps,
        "m11 = {}",
        node.affine[1][1]
    );
    assert_eq!(node.position, Vec2::new(11.0, 22.0));
}

#[test]
fn extracted_node_identity_affine_is_identity_basis() {
    // An identity GlobalTransform yields the [[1,0],[0,1]] basis — the
    // byte-identical fast path (every pre-affine pixel/test stays unchanged).
    let theme = Theme::default();
    let layout = ResolvedLayout {
        position: Vec2::ZERO,
        size: Vec2::splat(10.0),
    };
    let node = extracted_node_for(
        Entity::from_raw_u32(4).unwrap(),
        &GlobalTransform::IDENTITY,
        &layout,
        None,
        None,
        &theme,
    );
    assert_eq!(node.affine, [[1.0, 0.0], [0.0, 1.0]]);
}

#[test]
fn extracted_node_nonuniform_scale_basis() {
    // A (2,3) non-uniform scale yields the diagonal basis [[2,0],[0,3]] —
    // faithful for non-uniform scale (within the bridge's TRS range).
    let theme = Theme::default();
    let layout = ResolvedLayout {
        position: Vec2::ZERO,
        size: Vec2::splat(10.0),
    };
    let affine3 = bevy::math::Affine3A::from_scale(Vec3::new(2.0, 3.0, 1.0));
    let gt = GlobalTransform::from(affine3);
    let node = extracted_node_for(
        Entity::from_raw_u32(5).unwrap(),
        &gt,
        &layout,
        None,
        None,
        &theme,
    );
    let eps = 1e-5;
    assert!((node.affine[0][0] - 2.0).abs() < eps);
    assert!((node.affine[0][1] - 0.0).abs() < eps);
    assert!((node.affine[1][0] - 0.0).abs() < eps);
    assert!((node.affine[1][1] - 3.0).abs() < eps);
}

use buiy_core::render::extract::{
    ExtractedNode, ExtractedNodes, assemble_context_tree, assemble_in_paint_order,
};
use buiy_verify::snapshot::{NameLookup, assert_display_list_snapshot};

#[test]
fn extracted_nodes_default_is_empty_with_unit_scale() {
    let d = ExtractedNodes::default();
    assert!(d.nodes.is_empty());
    // Manual Default: scale_factor is 1.0, NOT the derived 0.0.
    assert_eq!(d.scale_factor, 1.0);
    assert_eq!(d.logical_size, Vec2::ZERO);
}

#[test]
fn assemble_emits_in_painters_z_order() {
    // painters_z is the already-sorted forward order; assembly must preserve it.
    let order = vec![
        Entity::from_raw_u32(30).unwrap(),
        Entity::from_raw_u32(10).unwrap(),
        Entity::from_raw_u32(20).unwrap(),
    ];
    // Build closure: every entity paints; record carries its entity for the
    // order assertion.
    let nodes = assemble_in_paint_order(&order, |e| {
        Some(ExtractedNode {
            entity: e,
            position: Vec2::ZERO,
            size: Vec2::ONE,
            color: Color::WHITE,
            clip: None,
            group: None,
            affine: [[1.0, 0.0], [0.0, 1.0]],
            outline: None,
            border: None,
            shadows: Vec::new(),
            gradients: Vec::new(),
        })
    });
    let got: Vec<Entity> = nodes.nodes.iter().map(|n| n.entity).collect();
    assert_eq!(
        got, order,
        "emission order must equal painters_z index order"
    );
}

#[test]
fn assemble_drops_skipped_entities() {
    let order = vec![
        Entity::from_raw_u32(1).unwrap(),
        Entity::from_raw_u32(2).unwrap(), // skipped
        Entity::from_raw_u32(3).unwrap(),
    ];
    let nodes = assemble_in_paint_order(&order, |e| {
        if e == Entity::from_raw_u32(2).unwrap() {
            None // skip
        } else {
            Some(ExtractedNode {
                entity: e,
                position: Vec2::ZERO,
                size: Vec2::ONE,
                color: Color::WHITE,
                clip: None,
                group: None,
                affine: [[1.0, 0.0], [0.0, 1.0]],
                outline: None,
                border: None,
                shadows: Vec::new(),
                gradients: Vec::new(),
            })
        }
    });
    let got: Vec<Entity> = nodes.nodes.iter().map(|n| n.entity).collect();
    assert_eq!(
        got,
        vec![
            Entity::from_raw_u32(1).unwrap(),
            Entity::from_raw_u32(3).unwrap()
        ]
    );
}

#[test]
fn hit_test_order_is_paint_order_reversed() {
    // The ordering identity (paint-order § 2): hit-test = painters_z reversed.
    // Both halves are asserted against the INDEPENDENT input `order` (the
    // painters_z forward list) — not re-derived from the assembled output — so
    // the reverse-identity claim actually exercises `assemble_in_paint_order`.
    // R5 ships no production picking-order helper (§ 2.2: the reverse walk is
    // owned by the picking subsystem); the reverse is expressed here directly
    // as the spec property `hit == reverse(painters_z)`.
    let order = vec![
        Entity::from_raw_u32(1).unwrap(),
        Entity::from_raw_u32(2).unwrap(),
        Entity::from_raw_u32(3).unwrap(),
    ];
    let nodes = assemble_in_paint_order(&order, |e| {
        Some(ExtractedNode {
            entity: e,
            position: Vec2::ZERO,
            size: Vec2::ONE,
            color: Color::WHITE,
            clip: None,
            group: None,
            affine: [[1.0, 0.0], [0.0, 1.0]],
            outline: None,
            border: None,
            shadows: Vec::new(),
            gradients: Vec::new(),
        })
    });
    // Paint order is painters_z forward.
    let paint: Vec<Entity> = nodes.nodes.iter().map(|n| n.entity).collect();
    assert_eq!(paint, order, "paint order must equal painters_z forward");
    // Hit-test order (back-to-front walk of the assembled output) is the exact
    // reverse of the painters_z input.
    let hit: Vec<Entity> = nodes.nodes.iter().rev().map(|n| n.entity).collect();
    let mut expected_hit = order.clone();
    expected_hit.reverse();
    assert_eq!(
        hit, expected_hit,
        "hit-test order must equal painters_z reversed"
    );
}

// Builds the entity->painters_z lookup the recursive tree assembler consumes:
// a context root maps to its painters_z slice; a plain painter maps to None.
fn ctx_lookup<'a>(
    map: &'a std::collections::HashMap<Entity, Vec<Entity>>,
) -> impl Fn(Entity) -> Option<&'a [Entity]> + 'a {
    move |e| map.get(&e).map(Vec::as_slice)
}

fn e(raw: u32) -> Entity {
    Entity::from_raw_u32(raw).unwrap()
}

#[test]
fn nested_context_is_entered_atomically_at_its_parent_position() {
    // paint-order-and-top-layer.md § 1.1: a nested SC root is a single atomic
    // entry in its parent's painters_z; when the forward walk reaches it, render
    // descends into the nested context AT THAT POSITION as a unit. The bug this
    // guards: flat-concatenating each context's painters_z paints the nested
    // descendants [C, D] at the END of their own list instead of between the
    // parent's A and B. Tree: root R = [A, NESTED, B]; NESTED = [C, D].
    //
    // The `assert_eq!(got, vec![root, a, nested, c, d, b])` order check becomes
    // a Name-keyed display-list snapshot: the assembled paint order IS the node
    // line order in the dump, so the flat-concat regression shows as a line
    // reorder (snapshots.md § Tier 2 — "a z-sort regression shows as a line
    // reorder, the exact bug class pixels name poorly").
    let (root, a, nested, b, c, d) = (e(1), e(2), e(3), e(4), e(5), e(6));
    let mut map: std::collections::HashMap<Entity, Vec<Entity>> = std::collections::HashMap::new();
    map.insert(root, vec![a, nested, b]);
    map.insert(nested, vec![c, d]);
    let painters_z_of = ctx_lookup(&map);

    let mut out = Vec::new();
    assemble_context_tree(
        root,
        &painters_z_of,
        &mut |x| {
            Some(ExtractedNode {
                entity: x,
                position: Vec2::ZERO,
                size: Vec2::ONE,
                color: Color::WHITE,
                clip: None,
                group: None,
                affine: [[1.0, 0.0], [0.0, 1.0]],
                outline: None,
                border: None,
                shadows: Vec::new(),
                gradients: Vec::new(),
            })
        },
        &mut out,
    );
    let nodes = ExtractedNodes {
        nodes: out,
        ..Default::default()
    };
    // Name the synthetic entities so the dump is diff-stable by Name (not raw
    // Entity bits). The dump's node lines read root, a, nested, c, d, b — the
    // expected atomic-descent order.
    let names = NameLookup::from_pairs([
        (root, "root"),
        (a, "a"),
        (nested, "nested"),
        (b, "b"),
        (c, "c"),
        (d, "d"),
    ]);
    assert_display_list_snapshot(&nodes, "nested_context_paint_order", &names);
}

#[test]
fn tree_assembly_skips_dropped_entities_across_the_boundary() {
    // The build closure's None (a skip, § 5) drops the entity from BOTH the
    // parent list and a nested context, without disturbing the surrounding
    // order. Root R = [A, NESTED, B]; NESTED = [C, D]; A and C are skipped.
    let (root, a, nested, b, c, d) = (e(1), e(2), e(3), e(4), e(5), e(6));
    let mut map: std::collections::HashMap<Entity, Vec<Entity>> = std::collections::HashMap::new();
    map.insert(root, vec![a, nested, b]);
    map.insert(nested, vec![c, d]);
    let painters_z_of = ctx_lookup(&map);

    let mut out = Vec::new();
    assemble_context_tree(
        root,
        &painters_z_of,
        &mut |x| {
            if x == a || x == c {
                None
            } else {
                Some(ExtractedNode {
                    entity: x,
                    position: Vec2::ZERO,
                    size: Vec2::ONE,
                    color: Color::WHITE,
                    clip: None,
                    group: None,
                    affine: [[1.0, 0.0], [0.0, 1.0]],
                    outline: None,
                    border: None,
                    shadows: Vec::new(),
                    gradients: Vec::new(),
                })
            }
        },
        &mut out,
    );
    let got: Vec<Entity> = out.iter().map(|n| n.entity).collect();
    assert_eq!(got, vec![root, nested, d, b]);
}
