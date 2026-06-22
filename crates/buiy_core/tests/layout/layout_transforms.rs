//! Phase 8 — transform composition + layout-flow invariance.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1, § 7.

use bevy::prelude::*;
use buiy_core::{
    CorePlugin, Node, ResolvedLayout, ResolvedTransform,
    layout::{
        Display, FlexAxis, LayoutPlugin, Length, Sizing, Style, TransformMatrix, UiTransform,
    },
};

/// Spawn a single sized `Node` with the given `style` and return the composed
/// `ResolvedTransform.matrix` after one update (or identity if none was
/// inserted — sub-pass 6e omits `ResolvedTransform` for an identity matrix).
fn resolved_matrix(width: Length, height: Length, style: Style) -> Mat4 {
    let mut app = app();
    let mut s = style;
    s.box_model.width = Sizing::Length(width);
    s.box_model.height = Sizing::Length(height);
    let e = app.world_mut().spawn((Node, s)).id();
    app.update();
    app.world()
        .get::<ResolvedTransform>(e)
        .map(|rt| rt.matrix)
        .unwrap_or(Mat4::IDENTITY)
}

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
fn translate_percent_x_resolves_against_own_width() {
    // translateX(50%) on a 100px-wide box → 50px x (CSS: x% of border-box width).
    let m = resolved_matrix(
        Length::px(100.0),
        Length::px(40.0),
        Style::default().translate(Length::percent(50.0), Length::px(0.0)),
    );
    assert_eq!(m, Mat4::from_translation(Vec3::new(50.0, 0.0, 0.0)));
}

#[test]
fn translate_percent_y_resolves_against_own_height() {
    // translateY(25%) on an 80px-tall box → 20px y (y% is of HEIGHT, not width).
    let m = resolved_matrix(
        Length::px(40.0),
        Length::px(80.0),
        Style::default().translate(Length::px(0.0), Length::percent(25.0)),
    );
    assert_eq!(m, Mat4::from_translation(Vec3::new(0.0, 20.0, 0.0)));
}

#[test]
fn translate_mixed_percent_and_px() {
    // 200x100 box; translate (10% , 15px) → x = 0.1*200 = 20, y = 15.
    // (`p * 0.01 * axis` carries a tiny float error, so compare within EPS.)
    fn assert_translation(m: Mat4, expected: Vec3) {
        let got = m.w_axis.truncate();
        assert!(
            got.abs_diff_eq(expected, 1e-4),
            "translation {got:?} != expected {expected:?}",
        );
        // The linear part stays identity (no rotation/scale leaked in).
        assert_eq!(m.x_axis, Vec4::new(1.0, 0.0, 0.0, 0.0));
        assert_eq!(m.y_axis, Vec4::new(0.0, 1.0, 0.0, 0.0));
        assert_eq!(m.z_axis, Vec4::new(0.0, 0.0, 1.0, 0.0));
    }

    let m = resolved_matrix(
        Length::px(200.0),
        Length::px(100.0),
        Style::default().translate(Length::percent(10.0), Length::px(15.0)),
    );
    assert_translation(m, Vec3::new(20.0, 15.0, 0.0));

    // Same percent resolution on the UiTransform.matrix path (not just the
    // Translate longhand) — TransformMatrix::Translate with a Percent term.
    let m_matrix = resolved_matrix(
        Length::px(200.0),
        Length::px(100.0),
        Style::default().ui_transform(UiTransform {
            matrix: TransformMatrix::Translate(
                Length::percent(10.0),
                Length::px(15.0),
                Length::ZERO,
            ),
            ..Default::default()
        }),
    );
    assert_translation(m_matrix, Vec3::new(20.0, 15.0, 0.0));
}

#[test]
fn cq_translate_is_residual_zero() {
    // Cqw translate is a RESIDUAL deferral (needs the nearest CQ-ancestor
    // frame, like sticky L4) — it resolves to 0.0 here, NOT to a cqw value.
    // With every other term identity, the matrix collapses to identity, so
    // sub-pass 6e inserts no ResolvedTransform. This documents scope discipline.
    let m = resolved_matrix(
        Length::px(100.0),
        Length::px(100.0),
        Style::default().ui_transform(UiTransform {
            matrix: TransformMatrix::Translate(Length::Cqw(50.0), Length::ZERO, Length::ZERO),
            ..Default::default()
        }),
    );
    assert_eq!(
        m,
        Mat4::IDENTITY,
        "Cq* translate is unresolved (0.0 residual) → identity matrix"
    );
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
