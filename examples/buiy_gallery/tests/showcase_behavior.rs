//! Headless behavior gates for the S5 Controls showcase (`ShowcasePlugin`) — the
//! design's live links that the static layout snapshot + the C8c a11y-driver
//! acceptance do not cover:
//!
//! - **Slider → preview radius**: driving the slider's `A11yValue` updates the
//!   preview square's corner `Border.radius` AND the "Npx" value label live (the
//!   design's `border-radius:{radius}px` link, HTML 331).
//! - **Run build → meter animation**: pressing "Run build" arms the meter ramp; the
//!   per-frame tick advances the "N%" label toward 100% and clears `building` on
//!   completion (the design's `runBuild` 0→100% animation, HTML 371/373).
//!
//! Both are driven through the production widget/app paths (the slider contract's
//! `A11yValue` write; the button's `OnPress` sink) and asserted on the resulting
//! pixels + the `ShowcaseBuild` state — never bespoke internal reads.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::input::ButtonInput;
use bevy::input::keyboard::{KeyCode, KeyboardInput};
use bevy::prelude::{Entity, With};
use buiy::{BuiyTextPlugin, CorePlugin, LayoutPlugin, WidgetsPlugin};
use buiy_core::a11y::{A11yPlugin, A11yValue};
use buiy_core::focus::FocusPlugin;
use buiy_core::interaction::OnPress;
use buiy_core::layout::Length;
use buiy_core::render::components::Border;
use buiy_core::text::Text;
use buiy_gallery::{
    ShowcaseBuild, ShowcaseMeterLabel, ShowcasePlugin, ShowcasePreview, ShowcaseRadiusLabel,
    ShowcaseRunBuild, spawn_showcase,
};

/// A headless app with the a11y tree, layout, focus, text, the widget systems, and
/// `ShowcasePlugin` (the S5 behavior). `TimePlugin` so the build-ramp tick reads a
/// real `Time` delta. Mirrors the C8c `showcase_app` plugin set.
fn showcase_app() -> App {
    // `MinimalPlugins` already includes `TimePlugin` (the build-ramp tick reads its
    // `Time` delta).
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(bevy::asset::AssetPlugin::default())
        .add_plugins(bevy::scene::ScenePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(A11yPlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default())
        .add_plugins(FocusPlugin)
        .add_plugins(WidgetsPlugin)
        .add_plugins(ShowcasePlugin);
    // The slider's APG keyboard reads `Messages<KeyboardInput>`; MinimalPlugins has
    // neither it nor the key resource.
    app.add_message::<KeyboardInput>();
    app.init_resource::<ButtonInput<KeyCode>>();

    spawn_showcase(app.world_mut());
    for _ in 0..4 {
        app.update();
    }
    app
}

/// The single slider entity (the only `A11yValue`-bearing showcase widget).
fn slider_entity(app: &mut App) -> Entity {
    let mut q = app.world_mut().query_filtered::<Entity, With<A11yValue>>();
    q.iter(app.world())
        .next()
        .expect("one slider (A11yValue) in the showcase")
}

/// The preview square's corner radius (the top-left x radius, in logical px).
fn preview_radius(app: &mut App) -> f32 {
    let mut q = app
        .world_mut()
        .query_filtered::<&Border, With<ShowcasePreview>>();
    let border = q.single(app.world()).expect("one preview square");
    match border.radius.top_left.x {
        Length::Px(px) => px,
        _ => f32::NAN,
    }
}

/// The text of the first entity carrying marker `M`.
fn marker_text<M: bevy::prelude::Component>(app: &mut App) -> String {
    let mut q = app.world_mut().query_filtered::<&Text, With<M>>();
    q.iter(app.world())
        .next()
        .map(|t| t.0.clone())
        .expect("a marked text leaf")
}

#[test]
fn slider_value_drives_the_preview_radius_and_label_live() {
    let mut app = showcase_app();
    let slider = slider_entity(&mut app);

    // At rest, the preview radius == the seeded slider value (14px), and the "Npx"
    // label reads "14px".
    assert_eq!(
        preview_radius(&mut app),
        14.0,
        "the preview square starts at the seeded slider radius (14px)"
    );
    assert_eq!(marker_text::<ShowcaseRadiusLabel>(&mut app), "14px");

    // Drive the slider's value up (the contract's `A11yValue` write — the same the
    // keyboard arrows / AT `SetValue` produce). The driver system reads
    // `Changed<A11yValue>` and repaints the preview + label.
    {
        let mut value = app.world_mut().get_mut::<A11yValue>(slider).unwrap();
        value.now = 30.0;
    }
    app.update();

    assert_eq!(
        preview_radius(&mut app),
        30.0,
        "raising the slider value drives the preview square's corner radius live"
    );
    assert_eq!(
        marker_text::<ShowcaseRadiusLabel>(&mut app),
        "30px",
        "the 'Npx' value label tracks the slider value live"
    );
}

#[test]
fn run_build_animates_the_meter_to_100_percent() {
    let mut app = showcase_app();

    // Find the "Run build" button + fire its OnPress (the production sink — the same
    // a pointer/keyboard activation produces).
    let run = {
        let mut q = app
            .world_mut()
            .query_filtered::<Entity, With<ShowcaseRunBuild>>();
        q.single(app.world()).expect("one Run build button")
    };
    app.world_mut().write_message(OnPress(run));
    app.update();

    // The build is now in flight (the applier armed the ramp).
    assert!(
        app.world().resource::<ShowcaseBuild>().building,
        "pressing Run build arms the meter ramp (building = true)"
    );

    // Advance enough wall-clock that the ramp completes (>0.3s). `TimePlugin`'s
    // first updates have a tiny delta; pump frames until the ramp clears `building`.
    for _ in 0..240 {
        if !app.world().resource::<ShowcaseBuild>().building {
            break;
        }
        app.update();
    }

    assert!(
        !app.world().resource::<ShowcaseBuild>().building,
        "the build ramp completes (building clears) within the frame budget"
    );
    assert_eq!(
        app.world().resource::<ShowcaseBuild>().progress,
        1.0,
        "the build ramps the progress to 100%"
    );
    assert_eq!(
        marker_text::<ShowcaseMeterLabel>(&mut app),
        "100%",
        "the meter 'N%' label reaches 100% when the build finishes"
    );
}

#[test]
fn run_build_attaches_a_scale_tween_to_the_meter_fill() {
    // The meter fill is a left-anchored X-scale tween (the C2 `set_meter` path — the
    // parity rule: never animate a Taffy-owned width per frame). Run build re-targets
    // it 0→1; assert the fill entity gains a `ScaleTween` (the transform-only grow).
    let mut app = showcase_app();
    let fill = app
        .world()
        .resource::<ShowcaseBuild>()
        .fill
        .expect("the meter fill is recorded at mount");

    let run = {
        let mut q = app
            .world_mut()
            .query_filtered::<Entity, With<ShowcaseRunBuild>>();
        q.single(app.world()).unwrap()
    };
    app.world_mut().write_message(OnPress(run));
    app.update();

    assert!(
        app.world()
            .get::<buiy_core::animation::ScaleTween>(fill)
            .is_some(),
        "Run build attaches a ScaleTween to the meter fill (the 0→100% transform grow)"
    );
}
