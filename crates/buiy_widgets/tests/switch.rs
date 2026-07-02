//! Wave-3 slice-1 — Switch widget: the P1d a11y bundle (role + binary
//! `A11yToggled` + focus + a11y) plus the C4 visual (the thumb slid via the
//! `Translate` longhand, driven by `Changed<A11yToggled>`) and pick-through
//! (`Pickable::IGNORE` on the decorative children).

use bevy::picking::Pickable;
use bevy::prelude::*;
use buiy_core::{
    CorePlugin,
    a11y::{A11yLabel, A11yRole, A11yToggled, Toggled},
    components::Node,
    focus::Focusable,
    interaction::OnPress,
    layout::{Display, Length, Translate},
    render::color::ColorToken,
    render::components::{Background, Border},
    text::Text,
};
use buiy_widgets::WidgetsPlugin;
use buiy_widgets::switch::{SWITCH_THUMB_TRAVEL, Switch, SwitchThumb, SwitchTrack};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(WidgetsPlugin);
    app
}

fn press(app: &mut App, entity: Entity) {
    app.world_mut().write_message(OnPress(entity));
    app.update();
}

/// Walk Switch → `SwitchTrack` → `SwitchThumb`: the thumb is now a grandchild
/// (the pill moved off the root onto its own track child).
fn track_of(app: &App, sw: Entity) -> Entity {
    let world = app.world();
    world
        .get::<Children>(sw)
        .unwrap()
        .iter()
        .find(|&c| world.get::<SwitchTrack>(c).is_some())
        .expect("a SwitchTrack child")
}

fn thumb_of(app: &App, sw: Entity) -> Entity {
    let world = app.world();
    let track = track_of(app, sw);
    world
        .get::<Children>(track)
        .unwrap()
        .iter()
        .find(|&c| world.get::<SwitchThumb>(c).is_some())
        .expect("a SwitchThumb grandchild under the track")
}

fn thumb_x(app: &App, thumb: Entity) -> f32 {
    match app.world().get::<Translate>(thumb).unwrap().0 {
        Length::Px(px) => px,
        other => panic!("thumb translate x is not px: {other:?}"),
    }
}

fn track_bg(app: &App, sw: Entity) -> ColorToken {
    let track = track_of(app, sw);
    app.world().get::<Background>(track).unwrap().color
}

#[test]
fn bare_switch_marker_materializes_the_full_required_contract() {
    // Post-rendering-fix contract: the bare `Switch` is the focusable, accessible
    // flex-ROW substrate; the visible 40×20 pill + fill + border now live on the
    // `SwitchTrack` child (so the label can sit BESIDE the pill instead of squishing
    // inside it). The pill paint companions therefore moved OFF the root.
    let mut app = app();
    let sw = app.world_mut().spawn(Switch).id();
    app.update();

    let world = app.world();
    assert!(world.get::<Node>(sw).is_some(), "Node");
    assert_eq!(
        world.get::<Display>(sw).copied(),
        Some(Display::flex_row()),
        "the root lays its [track, label] out in a row"
    );
    assert!(world.get::<Focusable>(sw).is_some(), "Focusable");
    assert_eq!(
        world.get::<A11yRole>(sw).copied(),
        Some(A11yRole::Switch),
        "role defaults to Switch"
    );
    assert_eq!(
        world.get::<A11yToggled>(sw).map(|t| t.0),
        Some(Toggled::False),
        "A11yToggled present, defaulting to False (off)"
    );
    assert!(world.get::<A11yLabel>(sw).is_some(), "A11yLabel");
    // The pill paint companions are NOT on the row root (they are the track's).
    assert!(
        world.get::<Background>(sw).is_none(),
        "the pill fill moved to the SwitchTrack child"
    );
    assert!(world.get::<Border>(sw).is_none(), "the pill border too");
}

/// **Thumb vertical-centering regression guard (review round-3 finding).** The
/// 16px thumb is nested in the 20px pill; without cross-axis centering it laid out
/// at the pill's content-top (2px high of centre). `SwitchTrack`'s flex
/// `align_items: Center` must straddle the thumb symmetrically over the pill.
/// (The other switch tests only check the x-axis `Translate`; this is the missing
/// y assertion — mirrors the slider's.)
#[test]
fn thumb_is_vertically_centered_on_the_pill() {
    use buiy_core::ResolvedLayout;
    use buiy_core::layout::LayoutPlugin;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(WidgetsPlugin);
    let sw = app.world_mut().spawn(Switch::new("Wi-Fi")).id();
    app.update();
    app.update();

    let track = track_of(&app, sw);
    let thumb = thumb_of(&app, sw);
    let tl = app
        .world()
        .get::<ResolvedLayout>(track)
        .expect("the pill is laid out");
    let th = app
        .world()
        .get::<ResolvedLayout>(thumb)
        .expect("the thumb is laid out");
    // `ResolvedLayout.position` is parent-relative: the thumb's y is relative to
    // the pill. The knob's vertical centre must coincide with the pill's.
    let pill_center = tl.size.y / 2.0;
    let thumb_center = th.position.y + th.size.y / 2.0;
    assert!(
        (pill_center - thumb_center).abs() < 0.5,
        "thumb vertically centered on the pill: pill_center={pill_center} \
         thumb_center={thumb_center} (thumb.pos.y={}, thumb.size.y={}, pill.size.y={})",
        th.position.y,
        th.size.y,
        tl.size.y,
    );
}

#[test]
fn switch_new_spawns_label_and_track_children_pick_through() {
    // Post-fix shape: the root's children are `[track, label]` (the thumb is a
    // grandchild under the track). The track carries the pill fill; the label sits
    // BESIDE it; both root children are `Pickable::IGNORE` (pick-through).
    let mut app = app();
    let sw = app.world_mut().spawn(Switch::new("Wi-Fi")).id();
    app.update();

    assert_eq!(
        app.world().get::<A11yLabel>(sw).map(|l| l.0.clone()),
        Some("Wi-Fi".to_string()),
        "the accessible name stays on the widget root"
    );

    let children = app
        .world()
        .get::<Children>(sw)
        .expect("switch has children")
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(children.len(), 2, "track + label children");
    for &child in &children {
        assert_eq!(
            app.world().get::<Pickable>(child).copied(),
            Some(Pickable::IGNORE),
            "decorative child carries Pickable::IGNORE (pick-through)"
        );
    }

    let world = app.world();
    let label = children
        .iter()
        .copied()
        .find(|&c| world.get::<SwitchTrack>(c).is_none())
        .unwrap();
    assert_eq!(
        world.get::<Text>(label).map(|t| t.0.clone()),
        Some("Wi-Fi".to_string()),
        "the label child carries the visible pixels"
    );

    // The track IS the visible 40×20 pill (fill + border on the track, not the
    // root).
    let track = track_of(&app, sw);
    let world = app.world();
    assert!(
        world.get::<Background>(track).is_some(),
        "the track carries the pill fill"
    );
    assert!(
        world.get::<Border>(track).is_some(),
        "the track carries the pill border"
    );

    // The thumb (a grandchild under the track) starts at the off position (x = 0).
    let thumb = thumb_of(&app, sw);
    assert_eq!(thumb_x(&app, thumb), 0.0, "thumb starts off (x = 0)");
}

#[test]
fn toggling_a11y_toggled_slides_the_thumb() {
    let mut app = app();
    let sw = app.world_mut().spawn(Switch::new("Wi-Fi")).id();
    app.update();
    let thumb = thumb_of(&app, sw);

    assert_eq!(thumb_x(&app, thumb), 0.0, "False ⇒ thumb off");

    app.world_mut().get_mut::<A11yToggled>(sw).unwrap().0 = Toggled::True;
    app.update();
    assert_eq!(
        thumb_x(&app, thumb),
        SWITCH_THUMB_TRAVEL,
        "True ⇒ thumb slid on (x = travel)"
    );

    app.world_mut().get_mut::<A11yToggled>(sw).unwrap().0 = Toggled::False;
    app.update();
    assert_eq!(thumb_x(&app, thumb), 0.0, "False ⇒ thumb back off");
}

#[test]
fn toggling_recolors_the_track_accent_on_raisedalt_off() {
    // N1 (2026-07-02 audit): the track fill is the primary on/off affordance. A
    // freshly-spawned OFF switch rests at `SurfaceRaisedAlt`; toggling on recolors
    // the track to `Accent` and back on off. The thumb is a constant white knob.
    let mut app = app();
    let sw = app.world_mut().spawn(Switch::new("Wi-Fi")).id();
    app.update();

    assert_eq!(
        track_bg(&app, sw),
        ColorToken::SurfaceRaisedAlt,
        "resting OFF ⇒ track is surface.raised-alt (no first-toggle color jump)"
    );
    let thumb = thumb_of(&app, sw);
    assert_eq!(
        app.world().get::<Background>(thumb).unwrap().color,
        ColorToken::White,
        "the thumb is a constant white knob"
    );

    app.world_mut().get_mut::<A11yToggled>(sw).unwrap().0 = Toggled::True;
    app.update();
    assert_eq!(
        track_bg(&app, sw),
        ColorToken::Accent,
        "ON ⇒ track recolors to the live accent"
    );
    assert_eq!(
        app.world().get::<Background>(thumb).unwrap().color,
        ColorToken::White,
        "the thumb stays white when on"
    );

    app.world_mut().get_mut::<A11yToggled>(sw).unwrap().0 = Toggled::False;
    app.update();
    assert_eq!(
        track_bg(&app, sw),
        ColorToken::SurfaceRaisedAlt,
        "OFF ⇒ track restores to surface.raised-alt"
    );
}

#[test]
fn seeded_on_switch_paints_accent_track_on_first_update() {
    // A switch seeded `Toggled::True` at spawn (the S4 modal register switch) must
    // render its accent track on the first update: the spawn initializer sets the
    // OFF color, but `Added` counts as `Changed`, so `update_switch_visual` fires and
    // paints `Accent` before first paint.
    let mut app = app();
    // C4: seed via the builder's `.on(true)` — the builder now CARRIES
    // `A11yToggled`, so a sibling `A11yToggled(..)` would be a duplicate-component
    // panic.
    let sw = app.world_mut().spawn(Switch::new("Register").on(true)).id();
    app.update();
    assert_eq!(
        track_bg(&app, sw),
        ColorToken::Accent,
        "seeded-on switch shows the accent track after the first update"
    );
    let thumb = thumb_of(&app, sw);
    assert_eq!(
        thumb_x(&app, thumb),
        SWITCH_THUMB_TRAVEL,
        "and the thumb is slid on"
    );
}

#[test]
fn on_press_toggles_switch_binary_and_slides_thumb() {
    let mut app = app();
    let sw = app.world_mut().spawn(Switch::new("Wi-Fi")).id();
    app.update();
    let thumb = thumb_of(&app, sw);

    // Press 1: off → on.
    press(&mut app, sw);
    assert_eq!(
        app.world().get::<A11yToggled>(sw).map(|t| t.0),
        Some(Toggled::True),
        "first OnPress flips False→True"
    );
    assert_eq!(thumb_x(&app, thumb), SWITCH_THUMB_TRAVEL, "thumb slid on");

    // Press 2: on → off (binary — never Mixed).
    press(&mut app, sw);
    assert_eq!(
        app.world().get::<A11yToggled>(sw).map(|t| t.0),
        Some(Toggled::False),
        "second OnPress flips True→False (binary, no Mixed)"
    );
    assert_eq!(thumb_x(&app, thumb), 0.0, "thumb back off");
}

// ── Track C / C4b: SwitchBuilder + .on + the nested grandchild subtree ─────────

/// `.on(true)` seeds the real binary `A11yToggled`, and the observer builds the
/// thumb as a GRANDCHILD (child of the `SwitchTrack`, not the root) — the
/// structure `update_switch_visual`'s root→track→thumb walk depends on (a
/// mis-nested thumb would be a silent, GPU-invisible no-slide).
#[test]
fn switch_new_on_seeds_true_with_thumb_as_grandchild() {
    use buiy_core::a11y::A11yToggled;

    let mut app = app();
    let sw = app.world_mut().spawn(Switch::new("Wi-Fi").on(true)).id();
    app.update();

    let world = app.world();
    assert!(
        Switch::on(world.get::<A11yToggled>(sw).unwrap()),
        "`.on(true)` seeds Toggled::True",
    );

    // The thumb is NOT a direct child of the root…
    let root_children: Vec<_> = world.get::<Children>(sw).unwrap().iter().collect();
    assert!(
        !root_children
            .iter()
            .any(|&c| world.get::<SwitchThumb>(c).is_some()),
        "thumb must NOT be a direct child of the switch root",
    );
    // …it is a grandchild under the SwitchTrack.
    let track = root_children
        .iter()
        .copied()
        .find(|&c| world.get::<SwitchTrack>(c).is_some())
        .expect("switch has a SwitchTrack child");
    assert!(
        world
            .get::<Children>(track)
            .unwrap()
            .iter()
            .any(|c| world.get::<SwitchThumb>(c).is_some()),
        "the thumb is a grandchild under the track",
    );
}
