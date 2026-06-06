//! Headless unit tests for the per-view extract mapping. Pure-CPU: no wgpu
//! adapter, no RenderApp. Mirrors tests/render_instance.rs conventions.

use bevy::prelude::*;
use buiy_core::render::color::ColorToken;
use buiy_core::render::extract::resolve_color_token;
use buiy_core::theme::{Theme, default_light_theme};
use std::borrow::Cow;

#[test]
fn transparent_token_resolves_to_none() {
    let theme = Theme::default();
    let c = resolve_color_token(&ColorToken::Transparent, &theme);
    assert_eq!(c, Color::NONE);
}

#[test]
fn known_token_resolves_to_theme_color() {
    let theme = default_light_theme();
    let c = resolve_color_token(
        &ColorToken::Token(Cow::Borrowed("color.surface.primary")),
        &theme,
    );
    assert_eq!(c, Color::WHITE);
}

#[test]
fn missing_token_resolves_to_magenta_sentinel() {
    let theme = default_light_theme();
    let c = resolve_color_token(
        &ColorToken::Token(Cow::Borrowed("nope.not.a.token")),
        &theme,
    );
    // Same sentinel render/mod.rs uses for a missing token.
    assert_eq!(c, Color::srgb(1.0, 0.0, 1.0));
}

use buiy_core::render::components::CssVisibility;
use buiy_core::render::extract::{SkipReason, node_skip_reason};

// Helper mirroring what extract binds per entity: Option of each skip input.
fn skip(css_vis: Option<CssVisibility>, offscreen: bool) -> Option<SkipReason> {
    node_skip_reason(css_vis.as_ref(), offscreen)
}

#[test]
fn visible_entity_is_not_skipped() {
    assert_eq!(skip(None, false), None);
    assert_eq!(skip(Some(CssVisibility::Visible), false), None);
}

#[test]
fn css_visibility_hidden_is_skipped() {
    assert_eq!(
        skip(Some(CssVisibility::Hidden), false),
        Some(SkipReason::CssHidden)
    );
}

#[test]
fn css_visibility_collapse_is_not_a_paint_skip_in_v1() {
    // Collapse is a deferred table/flex marker (component-model.md § 12.1) —
    // v1 ships only the Hidden paint-skip, so Collapse paints normally.
    assert_eq!(skip(Some(CssVisibility::Collapse), false), None);
}

#[test]
fn offscreen_auto_is_skipped() {
    assert_eq!(skip(None, true), Some(SkipReason::OffscreenAuto));
}

#[test]
fn content_visibility_hidden_entity_still_paints_its_own_box() {
    // paint-order-and-top-layer.md § 5.2: a `content-visibility: hidden`
    // entity's OWN box paints; only its descendants are pruned, and that prune
    // happens layout-side (they never reach painters_z). Render therefore does
    // NOT skip the Hidden entity itself — Containment is not even a skip input.
    assert_eq!(skip(None, false), None);
}

#[test]
fn css_hidden_takes_precedence_over_offscreen() {
    // Precedence is observable; CssHidden is checked first.
    assert_eq!(
        skip(Some(CssVisibility::Hidden), true),
        Some(SkipReason::CssHidden)
    );
}

use buiy_core::components::ResolvedLayout;
use buiy_core::render::components::Background;
use buiy_core::render::extract::extracted_node_for;

#[test]
fn extracted_node_carries_box_and_resolved_color() {
    let theme = default_light_theme();
    let layout = ResolvedLayout {
        position: Vec2::new(10.0, 20.0),
        size: Vec2::new(100.0, 40.0),
    };
    let gt = GlobalTransform::from_translation(Vec3::new(10.0, 20.0, 0.0));
    let bg = Background {
        color: ColorToken::Token(Cow::Borrowed("color.surface.primary")),
    };
    let entity = Entity::from_raw_u32(7).unwrap();

    let node = extracted_node_for(entity, &gt, &layout, Some(&bg), &theme);

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
    let node = extracted_node_for(Entity::from_raw_u32(1).unwrap(), &gt, &layout, None, &theme);
    assert_eq!(node.color, Color::NONE);
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
    let node = extracted_node_for(Entity::from_raw_u32(2).unwrap(), &gt, &layout, None, &theme);
    assert_eq!(node.position, Vec2::new(200.0, 300.0));
}

use buiy_core::render::extract::{
    ExtractedNode, ExtractedNodes, assemble_context_tree, assemble_in_paint_order,
};

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
            })
        },
        &mut out,
    );
    let got: Vec<Entity> = out.iter().map(|n| n.entity).collect();
    // Root's OWN box paints first, then A, then the whole nested unit (its own
    // box NESTED, then C, D), then B — never A, NESTED, B, C, D.
    assert_eq!(got, vec![root, a, nested, c, d, b]);
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
                })
            }
        },
        &mut out,
    );
    let got: Vec<Entity> = out.iter().map(|n| n.entity).collect();
    assert_eq!(got, vec![root, nested, d, b]);
}
