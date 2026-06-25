//! Headless layout-snapshot gate for the S1 TodoMVC screen (Tier 1 of the
//! `buiy_verify` pyramid — no GPU, no window). Drives the **same**
//! `screen_todomvc` tree the binary authors + seeds the demo rows, then pins the
//! resolved layout of every `#Name`-tagged entity. A structural regression (a
//! dropped child, a lost merge, a wrong box) shows as a `.snap` diff.
//!
//! This is the "example IS the fixture" discipline applied to S1: the screen is
//! authored once (`buiy_gallery::screen_todomvc`) and both the runnable binary
//! and this gate spawn the exact same tree. Matrix enrollment of screen fixtures
//! (the reduced `Matrix::gallery_screen()`) is a later C8 slice; this dedicated
//! scene-based snapshot covers S1's layout structure without modifying the
//! coverage `build_app` (which has no `ScenePlugin`).

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::scene::{ScenePlugin, WorldSceneExt};
use buiy::{BuiyTextPlugin, CorePlugin, LayoutPlugin, WidgetsPlugin};
use buiy_core::ResolvedLayout;
use buiy_core::text::{ComputedTextLayout, Text};
use buiy_gallery::{DEMO_SEEDS, append_row, screen_todomvc};
use buiy_verify::snapshot::assert_layout_snapshot;

/// Build the live TodoMVC tree (the same one the binary authors): the static
/// screen + the imperatively-seeded demo rows.
fn todomvc_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default())
        .add_plugins(WidgetsPlugin);

    // Author the static screen, then seed the demo rows imperatively (rows are
    // dynamic — the binary seeds them in `setup` the same way).
    app.world_mut()
        .spawn_scene(screen_todomvc(DEMO_SEEDS))
        .expect("spawn the todomvc screen");
    for &(label, completed) in DEMO_SEEDS {
        append_row(app.world_mut(), label, completed);
    }
    app
}

#[test]
fn todomvc_screen_lays_out_as_expected() {
    let mut app = todomvc_app();
    assert_layout_snapshot(&mut app, "todomvc_screen");
}

/// **The widget-catalog rendering-bug regression guard — gallery-grounded.**
/// Every NON-EMPTY content label in the live TodoMVC (the 3 row labels, the All/
/// Active/Completed/Clear filter button labels, the per-row "×", the "N items
/// left" status) must be LAID OUT at a non-zero box AND SHAPED to real glyph
/// geometry — the two preconditions for the text reaching the screen. The
/// rendering bug this campaign fixed was precisely scene-authored labels with an
/// accessible name but NO `Node` → no `ResolvedLayout`, no `ComputedTextLayout`,
/// no glyphs → an invisible gallery whose headless layout/a11y gates were all
/// green. This asserts the paint precondition the snapshot only implies.
#[test]
fn todomvc_content_text_is_laid_out_and_shaped_so_it_paints() {
    let mut app = todomvc_app();
    // Settle: TextSync → measure → TextCommit must run for the shaped outputs.
    for _ in 0..4 {
        app.update();
    }

    let mut q = app.world_mut().query::<(
        Entity,
        &Text,
        Option<&ResolvedLayout>,
        Option<&ComputedTextLayout>,
    )>();
    let world = app.world();
    let mut checked = 0usize;
    for (e, text, layout, computed) in q.iter(world) {
        // Skip the EMPTY carriers (the input editor's `Text("")` display carrier,
        // an unchecked checkbox mark) — they legitimately paint nothing.
        if text.0.trim().is_empty() {
            continue;
        }
        let layout = layout.unwrap_or_else(|| {
            panic!(
                "content label {e:?} {:?} has NO ResolvedLayout — not laid out, so it paints \
                 nothing (the require(Node) regression)",
                text.0
            )
        });
        assert!(
            layout.size.x > 0.0 && layout.size.y > 0.0,
            "content label {:?} has a zero-size box {:?} — it occupies no space and paints nothing",
            text.0,
            layout.size,
        );
        let computed = computed.unwrap_or_else(|| {
            panic!(
                "content label {:?} has NO ComputedTextLayout — never shaped",
                text.0
            )
        });
        assert!(
            !computed.lines.is_empty() && computed.size.x > 0.0,
            "content label {:?} shaped to no glyph geometry {computed:?}",
            text.0,
        );
        checked += 1;
    }
    // 3 row labels + 4 filter labels + 3 "×" + the status = 11 visible labels;
    // assert we actually exercised the content (a vacuous 0 would hide a
    // regression where everything got filtered as empty).
    assert!(
        checked >= 8,
        "expected the live TodoMVC to carry many painted content labels (rows + buttons + \
         status), only saw {checked} — content text is missing"
    );
}
