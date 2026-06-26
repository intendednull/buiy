//! Headless layout-snapshot gate for the unified **shell** (parity Wave C1 —
//! Tier 1 of the `buiy_verify` pyramid: no GPU, no window-on-screen). Drives the
//! SAME `build_shell` + `mount_screens` tree the binary boots, then pins the
//! resolved layout of every `#Name`-tagged shell entity. A structural regression
//! (a dropped pane, a wrong chrome height, a lost screen mount, a mis-sized rail/
//! inspector) shows as a `.snap` diff.
//!
//! The shell root sizes to `100%`, so the test stands up a headless
//! `(Window, PrimaryWindow)` at the design preview size (1280×800) — the layout
//! viewport the `100%` root resolves against (the `picking_backend` headless
//! pattern). The default screen (Todo) is active; the inactive 4 are
//! `Display::None`, so they (and their subtrees, incl. the 1000 scroll rows) are
//! pruned from the dump — proving the router's spawn-all-once + `Display::None`
//! mechanism keeps hidden screens out of layout.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::ecs::entity::Entity;
use bevy::scene::ScenePlugin;
use bevy::window::{PrimaryWindow, Window, WindowResolution};
use buiy::{BuiyTextPlugin, CorePlugin, LayoutPlugin, WidgetsPlugin};
use buiy_core::ResolvedLayout;
use buiy_core::text::{ComputedTextLayout, Text};
use buiy_core::theme::default_dark_theme;
use buiy_gallery::inspector::build_inspector_content;
use buiy_gallery::shell::{ScreenRouter, build_shell, mount_screens_with};
use buiy_verify::snapshot::assert_layout_snapshot;

/// The S2 scroll-row count the shell snapshot seeds. The hidden (`Display::None`)
/// scroll screen collapses every row to a `(0,0)` zero-box, so MANY identically-
/// named `ScrollRow`s would make the dump ambiguous; the shell skeleton pins the
/// 5 panes + that the screens mount, NOT the scroll-row internals (those have their
/// own `scroll_list_layout.rs` snapshot). 0 rows keeps the dump unambiguous.
const SHELL_SNAPSHOT_SCROLL_ROWS: usize = 0;

/// Build the live shell tree (the same one the binary boots): a 1280×800 headless
/// window, the dark theme, the `ScreenRouter` resource, then `build_shell` +
/// `mount_screens`.
fn shell_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(AssetPlugin::default())
        .add_plugins(ScenePlugin)
        .add_plugins(CorePlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default())
        .add_plugins(WidgetsPlugin);

    // The dark theme (the gallery opts in; the framework default is light).
    app.insert_resource(default_dark_theme());
    // The router resource the shell mount reads (default = Todo active).
    app.init_resource::<ScreenRouter>();

    // A headless primary window at the design preview size — the layout viewport
    // the shell root's `100%` width/height resolves against.
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(1280, 800),
            ..Default::default()
        },
        PrimaryWindow,
    ));

    // `build_shell` / `mount_screens_with` take `&mut World` directly (the
    // imperative shell-building idiom), so call them straight on the world.
    // `build_inspector_content` fills the C4 inspector pane (name/desc + composed-
    // of chips + live-state rows + accent swatches) so the snapshot pins it too.
    let world = app.world_mut();
    build_shell(world);
    mount_screens_with(world, SHELL_SNAPSHOT_SCROLL_ROWS);
    build_inspector_content(world);
    app
}

#[test]
fn shell_lays_out_as_expected() {
    let mut app = shell_app();
    assert_layout_snapshot(&mut app, "shell_skeleton");
}

/// **The widget-catalog rendering-bug regression guard — shell-grounded.** Every
/// NON-EMPTY chrome/rail/header/status content label (the "buiy" wordmark, the
/// "widget catalog" badge, the `$ cargo run` chip, the 5 nav names/descs/indices,
/// the Stats rows, the viewport-header name/path/size, the status bar) must be
/// LAID OUT at a non-zero box AND SHAPED to real glyph geometry — the two
/// preconditions for the text reaching the screen. (The campaign's invisible-
/// gallery bug was scene labels with a name but no `Node`/layout/shaping.) Asserts
/// the paint precondition the snapshot only implies, for the shell chrome.
#[test]
fn shell_chrome_text_is_laid_out_and_shaped_so_it_paints() {
    let mut app = shell_app();
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
        // Skip the EMPTY carriers (a text-input display carrier, an unchecked
        // checkbox mark) — they legitimately paint nothing.
        if text.0.trim().is_empty() {
            continue;
        }
        let Some(layout) = layout else {
            panic!(
                "content label {e:?} {:?} has NO ResolvedLayout — not laid out, so it paints \
                 nothing (the require(Node) regression class)",
                text.0
            );
        };
        // A `Display::None` (hidden-screen) label legitimately collapses to a
        // zero box — skip those; the shell-chrome labels (always-on) must NOT.
        if layout.size.x <= 0.0 || layout.size.y <= 0.0 {
            continue;
        }
        let Some(computed) = computed else {
            panic!(
                "content label {:?} has NO ComputedTextLayout — never shaped",
                text.0
            );
        };
        assert!(
            !computed.lines.is_empty() && computed.size.x > 0.0,
            "content label {:?} shaped to no glyph geometry {computed:?}",
            text.0,
        );
        checked += 1;
    }
    // The chrome alone carries many always-on labels: "buiy", "widget catalog",
    // "$ "/"cargo run…", "dark", 5 nav names + 5 descs + 5 indices, 3 stat keys +
    // 3 vals, viewport name/path/size, status labels. Assert we exercised a broad
    // set (a vacuous 0 would hide a regression where everything filtered as empty).
    assert!(
        checked >= 20,
        "expected the shell to carry many painted chrome/rail/header content labels, only saw \
         {checked} — shell content text is missing"
    );
}
