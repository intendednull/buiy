//! Phase 8 — transform composition + layout-flow invariance.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1, § 7.

use bevy::prelude::*;
use buiy_core::{
    CorePlugin, Node, ResolvedLayout, ResolvedTransform,
    layout::{Display, FlexAxis, LayoutPlugin, Length, Sizing, Style},
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app
}

#[test]
fn translate_transform_composes_to_resolved_transform() {
    let mut app = app();
    let e = app
        .world_mut()
        .spawn((Node, Style::default().translate_px(15.0, 25.0)))
        .id();
    app.update();
    let rt = app
        .world()
        .get::<ResolvedTransform>(e)
        .expect("non-identity → ResolvedTransform");
    assert_eq!(
        rt.matrix,
        Mat4::from_translation(Vec3::new(15.0, 25.0, 0.0))
    );
}

#[test]
fn transform_does_not_change_own_resolved_layout_position() {
    // A transformed element occupies its un-transformed box (spec § 1.2).
    let mut app = app();
    // Build the SAME box twice: once with a transform, once without,
    // under identical parents; assert ResolvedLayout.position matches.
    let parent_plain = app
        .world_mut()
        .spawn((Node, Style::default().flex_axis(FlexAxis::Row)))
        .id();
    let child_plain = app.world_mut().spawn((Node, Style::default())).id();
    app.world_mut()
        .entity_mut(parent_plain)
        .add_child(child_plain);

    let parent_xf = app
        .world_mut()
        .spawn((Node, Style::default().flex_axis(FlexAxis::Row)))
        .id();
    let child_xf = app
        .world_mut()
        .spawn((Node, Style::default().translate_px(100.0, 100.0)))
        .id();
    app.world_mut().entity_mut(parent_xf).add_child(child_xf);

    app.update();

    let p = app
        .world()
        .get::<ResolvedLayout>(child_plain)
        .unwrap()
        .position;
    let x = app
        .world()
        .get::<ResolvedLayout>(child_xf)
        .unwrap()
        .position;
    assert_eq!(p, x, "transform must NOT move the layout box (spec § 1.2)");
}

#[test]
fn transform_does_not_change_sibling_positions() {
    // Flex row with three children; middle child rotated; assert the
    // siblings' ResolvedLayout positions match the un-rotated case.
    fn build(app: &mut App, rotate_middle: bool) -> [Entity; 3] {
        let parent = app
            .world_mut()
            .spawn((Node, Style::default().flex_axis(FlexAxis::Row)))
            .id();
        let mut kids = [Entity::PLACEHOLDER; 3];
        for (i, slot) in kids.iter_mut().enumerate() {
            let mut s = Style::default();
            // give each child a fixed size so positions are deterministic
            s.box_model.width = Sizing::Length(Length::px(50.0));
            s.box_model.height = Sizing::Length(Length::px(50.0));
            if i == 1 && rotate_middle {
                s = s.rotate_z(std::f32::consts::FRAC_PI_4);
            }
            let c = app.world_mut().spawn((Node, s)).id();
            app.world_mut().entity_mut(parent).add_child(c);
            *slot = c;
        }
        kids
    }

    let mut plain = app();
    let kp = build(&mut plain, false);
    plain.update();

    let mut rot = app();
    let kr = build(&mut rot, true);
    rot.update();

    for i in 0..3 {
        let pp = plain.world().get::<ResolvedLayout>(kp[i]).unwrap().position;
        let rp = rot.world().get::<ResolvedLayout>(kr[i]).unwrap().position;
        assert_eq!(
            pp, rp,
            "child {i} position must be unaffected by a sibling's transform"
        );
    }
}

#[test]
fn display_none_transformed_entity_gets_no_resolved_transform() {
    let mut app = app();
    let mut s = Style::default().translate_px(10.0, 10.0);
    s.display = Display::None;
    let e = app.world_mut().spawn((Node, s)).id();
    app.update();
    assert!(
        app.world().get::<ResolvedTransform>(e).is_none(),
        "Display::None is skipped by sub-pass 6e"
    );
}
