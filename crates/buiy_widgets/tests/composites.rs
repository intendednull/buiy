//! The **promoted general composites** (`buiy_widgets::composites`, widget-catalog
//! parity Wave 5 refinement): smoke + key-behavior unit tests. These builders are
//! imperative `World`-spawning trees, so the marker/structure assertions need no
//! GPU and (for all but `search_input`) no plugins — a bare `World` is enough. The
//! one exception is `search_input`, which spawns a real `text_input` scene and so
//! needs the scene infrastructure (the `scene.rs` test harness).
//!
//! Font-NEUTRALITY is the contract that made these promotable out of the gallery:
//! every text-bearing builder takes a [`FontFamily`] argument (the app owns the
//! typeface), so the tests thread an arbitrary named face and assert the structure
//! the design depends on (a meter fraction scale, a selected row's accent bar, a
//! ⌘-prefixed kbd's vector icon, a status dot's glow, a pulse's tween).

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::hierarchy::Children;
use bevy::prelude::{Entity, World};
use bevy::scene::ScenePlugin;
use buiy_core::animation::{OpacityTween, ScaleTween};
use buiy_core::focus::Focusable;
use buiy_core::layout::Scale;
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{Background, BoxShadow, Icon};
use buiy_core::text::{FamilyEntry, FontFamily, FontStack, Text};
use buiy_widgets::WidgetsPlugin;
use buiy_widgets::composites::{
    MeterFill, RowSelBar, TableRow, TableRowData, kbd, kbd_content, meter, pulse_blink,
    search_input, set_meter, set_table_row_selected, status_dot, table_header, table_row,
};

/// An arbitrary named monospace face — the composites are font-neutral, so the
/// tests just need *a* family to thread; the exact name never matters here.
fn mono() -> FontFamily {
    FontFamily(FontStack(vec![FamilyEntry::Named("Geist Mono".into())]))
}

/// The first child of `parent` carrying marker `T`.
fn child_with<T: bevy::ecs::component::Component>(world: &World, parent: Entity) -> Option<Entity> {
    world
        .get::<Children>(parent)?
        .iter()
        .copied()
        .find(|&c| world.get::<T>(c).is_some())
}

// ===========================================================================
// meter / set_meter — the left-anchored progress fill
// ===========================================================================

/// `meter` returns `(track, fill)`; the fill carries the [`MeterFill`] marker and a
/// resting [`Scale`] whose X = the fraction (the design's left-anchored fill), and
/// is parented under the track.
#[test]
fn meter_fill_carries_fraction_scale_and_is_track_child() {
    let mut world = World::new();
    let (track, fill) = meter(&mut world, 240.0, 0.5);

    assert!(
        world.get::<MeterFill>(fill).is_some(),
        "the fill must carry the MeterFill marker"
    );
    let scale = world
        .get::<Scale>(fill)
        .expect("the fill must carry a Scale");
    assert!(
        (scale.0 - 0.5).abs() < 1e-4 && (scale.1 - 1.0).abs() < 1e-4,
        "the fill's X scale is the fraction (0.5) and Y is 1, got {scale:?}"
    );
    // The fill is a child of the track (the track owns the overflow:hidden mask).
    let is_child = world
        .get::<Children>(track)
        .is_some_and(|c| c.iter().copied().any(|e| e == fill));
    assert!(is_child, "the fill must be parented under the track");
}

/// The fraction clamps to `[0, 1]` (a caller passing 1.5 gets a full, not overscaled,
/// fill).
#[test]
fn meter_fraction_clamps() {
    let mut world = World::new();
    let (_track, fill) = meter(&mut world, 100.0, 1.5);
    assert!((world.get::<Scale>(fill).unwrap().0 - 1.0).abs() < 1e-4);
}

/// `set_meter` attaches a [`ScaleTween`] (the transform-only animation, never a
/// per-frame Taffy width) that starts at the fill's current scale and targets the
/// new fraction.
#[test]
fn set_meter_attaches_scale_tween_from_current() {
    let mut world = World::new();
    let (_track, fill) = meter(&mut world, 240.0, 0.2);
    set_meter(&mut world, fill, 0.8);

    let tween = world
        .get::<ScaleTween>(fill)
        .expect("set_meter must attach a ScaleTween");
    assert!(
        (tween.0.from.x - 0.2).abs() < 1e-4,
        "the tween starts at the current fraction (0.2), got {}",
        tween.0.from.x
    );
    assert!(
        (tween.0.to.x - 0.8).abs() < 1e-4,
        "the tween targets the new fraction (0.8), got {}",
        tween.0.to.x
    );
}

// ===========================================================================
// table_row / table_header / set_table_row_selected — the columned row
// ===========================================================================

fn sample_row<'a>(idx: &'a str, name: &'a str) -> TableRowData<'a> {
    TableRowData {
        idx,
        indent_px: 0.0,
        dot_color: ColorToken::StatusOk,
        node_type: "Button",
        name,
        ms: "0.42",
        ms_warn: false,
        state: "OK",
        state_color: ColorToken::StatusOk,
    }
}

/// A selected `table_row` paints the `accent.soft` fill and parents a [`RowSelBar`]
/// accent bar; an unselected row is transparent with no bar.
#[test]
fn table_row_selected_has_accent_soft_and_bar() {
    let mut world = World::new();
    let sel = table_row(&mut world, &sample_row("00", "a"), mono(), true);
    let unsel = table_row(&mut world, &sample_row("01", "b"), mono(), false);

    assert_eq!(
        world.get::<Background>(sel).map(|b| b.color.clone()),
        Some(ColorToken::AccentSoft),
        "the selected row is accent.soft"
    );
    assert!(
        child_with::<RowSelBar>(&world, sel).is_some(),
        "the selected row parents an accent left-bar"
    );

    assert_eq!(
        world.get::<Background>(unsel).map(|b| b.color.clone()),
        Some(ColorToken::Transparent),
        "the unselected row is transparent"
    );
    assert!(
        child_with::<RowSelBar>(&world, unsel).is_none(),
        "the unselected row has no left-bar"
    );
    // Both rows carry the TableRow marker.
    assert!(world.get::<TableRow>(sel).is_some() && world.get::<TableRow>(unsel).is_some());
}

/// `set_table_row_selected` flips the full selected representation (bg + bar) and is
/// idempotent (no duplicate bar on a re-select; bar removed on deselect).
#[test]
fn set_table_row_selected_toggles_idempotently() {
    let mut world = World::new();
    let row = table_row(&mut world, &sample_row("00", "a"), mono(), false);

    set_table_row_selected(&mut world, row, true);
    assert_eq!(
        world.get::<Background>(row).map(|b| b.color.clone()),
        Some(ColorToken::AccentSoft)
    );
    assert!(child_with::<RowSelBar>(&world, row).is_some());

    // Re-select: still exactly one bar (idempotent).
    set_table_row_selected(&mut world, row, true);
    let bars = world
        .get::<Children>(row)
        .map(|c| {
            c.iter()
                .copied()
                .filter(|&e| world.get::<RowSelBar>(e).is_some())
                .count()
        })
        .unwrap_or(0);
    assert_eq!(bars, 1, "re-selecting must not duplicate the bar");

    // Deselect: bar removed, bg transparent.
    set_table_row_selected(&mut world, row, false);
    assert!(child_with::<RowSelBar>(&world, row).is_none());
    assert_eq!(
        world.get::<Background>(row).map(|b| b.color.clone()),
        Some(ColorToken::Transparent)
    );
}

/// `table_header` builds one cell per column (a fixed-width or `flex:1` `Text` leaf).
#[test]
fn table_header_builds_one_cell_per_column() {
    let mut world = World::new();
    let header = table_header(
        &mut world,
        &[("INDEX", Some(46.0)), ("NODE", None), ("STATE", Some(42.0))],
        mono(),
    );
    let cells = world.get::<Children>(header).map(|c| c.len()).unwrap_or(0);
    assert_eq!(cells, 3, "three columns → three header cells");
    // Each cell is a Text leaf.
    let all_text = world
        .get::<Children>(header)
        .unwrap()
        .iter()
        .copied()
        .all(|c| world.get::<Text>(c).is_some());
    assert!(all_text, "every header cell is a Text leaf");
}

// ===========================================================================
// kbd / kbd_content — the keyboard-shortcut chip (⌘ as a vector icon)
// ===========================================================================

/// A plain shortcut (`F2`) is a single mono `Text` leaf; a ⌘-prefixed shortcut
/// (`⌘K`) becomes a flex-row of a vector ⌘ [`Icon`] + the remaining text (so the
/// Command symbol renders crisply in a font that lacks U+2318).
#[test]
fn kbd_content_plain_is_leaf_cmd_is_icon_row() {
    let mut world = World::new();

    let plain = kbd_content(&mut world, "#k", "F2", mono(), ColorToken::TextDim);
    assert!(
        world.get::<Text>(plain).is_some(),
        "a plain key is a single Text leaf"
    );
    assert!(
        world.get::<Children>(plain).is_none_or(|c| c.is_empty()),
        "a plain key has no icon/text split children"
    );

    let cmd = kbd_content(&mut world, "#k", "⌘K", mono(), ColorToken::TextDim);
    let children: Vec<Entity> = world
        .get::<Children>(cmd)
        .map(|c| c.iter().copied().collect())
        .unwrap_or_default();
    assert_eq!(children.len(), 2, "⌘K is a [icon][text] row");
    assert!(
        children.iter().any(|&c| world.get::<Icon>(c).is_some()),
        "the ⌘ glyph is rendered as a vector Icon"
    );
    assert!(
        children
            .iter()
            .any(|&c| world.get::<Text>(c).is_some_and(|t| t.0 == "K")),
        "the remaining text ('K') stays a mono leaf"
    );
}

/// `kbd` wraps the content in a chip body (a bordered, bg-filled box) holding the
/// content.
#[test]
fn kbd_wraps_content_in_a_chip() {
    let mut world = World::new();
    let chip = kbd(&mut world, "F2", mono());
    assert!(
        world.get::<Background>(chip).is_some(),
        "the kbd chip has a background"
    );
    let content = world.get::<Children>(chip).map(|c| c.len()).unwrap_or(0);
    assert_eq!(content, 1, "the chip holds its single content leaf");
}

// ===========================================================================
// status_dot / pulse_blink — the glowing status indicator
// ===========================================================================

/// A `status_dot` carries a [`BoxShadow`] glow.
#[test]
fn status_dot_has_glow_shadow() {
    let mut world = World::new();
    let dot = status_dot(&mut world, ColorToken::StatusOk, ColorToken::StatusOk, 6.0, 0.0);
    let shadow = world
        .get::<BoxShadow>(dot)
        .expect("the status dot must carry a glow BoxShadow");
    assert_eq!(shadow.0.len(), 1, "one glow layer");
}

/// `pulse_blink` attaches an [`OpacityTween`] (the infinite ping-pong pulse).
#[test]
fn pulse_blink_attaches_opacity_tween() {
    let mut world = World::new();
    let dot = status_dot(&mut world, ColorToken::Accent, ColorToken::AccentSoft, 0.0, 4.0);
    pulse_blink(&mut world, dot);
    assert!(
        world.get::<OpacityTween>(dot).is_some(),
        "pulse_blink must attach an OpacityTween"
    );
}

// ===========================================================================
// search_input — needs the scene infra (spawns a real text_input field)
// ===========================================================================

/// The BSN spawn machinery + the widget plugins (required-components registered
/// before any spawn). No GPU. Mirrors the `scene.rs` harness.
fn scene_test_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(buiy_core::text::BuiyTextPlugin::default())
        .add_plugins(WidgetsPlugin);
    app
}

/// `search_input` builds a row holding a leading search [`Icon`] + a real focusable
/// single-line text field (the row supplies the chrome; the field is editable).
#[test]
fn search_input_has_icon_and_focusable_field() {
    let mut app = scene_test_app();
    let row = {
        let world = app.world_mut();
        search_input(world, "Filter…", mono(), 240.0)
    };

    let world = app.world();
    let children: Vec<Entity> = world
        .get::<Children>(row)
        .map(|c| c.iter().copied().collect())
        .unwrap_or_default();
    assert_eq!(children.len(), 2, "the row is [search-icon][field]");
    assert!(
        children.iter().any(|&c| world.get::<Icon>(c).is_some()),
        "a leading search Icon"
    );
    assert!(
        children
            .iter()
            .any(|&c| world.get::<Focusable>(c).is_some()),
        "the field is a real focusable text input"
    );
}
