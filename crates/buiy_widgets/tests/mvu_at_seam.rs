//! W6b (MVU-as-core, the AT synchronous act-then-observe SEAM) — the proof that an
//! inbound AT `Expand`/`Collapse` on a `MenuButton` now DRIVES the `MenuModel` machine
//! (spec §5). This closes the W5/W6a "advertised but inert" gap: before W6b an AT
//! `Expand` wrote `A11yExpanded` directly, which `bind_menu_model` re-clobbered from the
//! unchanged model, so the menu never opened.
//!
//! Two gates:
//!  - **The corrected contract (§5.1) — `live-component-synchronous + perform-then-update`**:
//!    an AT `Expand` mutates the LIVE `MenuModel.open` the instant `dispatch_action_request`
//!    returns (no `app.update()`); the early `MenuSet::Bind` projects it onto the button's
//!    `A11yExpanded`, so the a11y-tree snapshot shows `aria-expanded` after ONE `app.update()`.
//!    Symmetric `Collapse`.
//!  - **The §5.7 replay test**: an AT `Expand` lands a RECORDED `MenuMsg` in the log, and
//!    the session round-trips BYTE-IDENTICALLY through `replay_into` (the inline fold is a
//!    recorded Msg in the one global sequence — closes L5).

use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use buiy_core::CorePlugin;
use buiy_core::a11y::inprocess::TreeView;
use buiy_core::a11y::translate::node_id_for;
use buiy_core::a11y::{A11yPlugin, Action, perform, snapshot};
use buiy_core::focus::FocusPlugin;
use buiy_core::mvu::{LogicalId, MsgLog, RecordSession};
use buiy_core::render::components::CssVisibility;
use buiy_core::replay::replay_into;
use buiy_core::text::edit::EditLog;
use buiy_widgets::WidgetsPlugin;
use buiy_widgets::menu::{Menu, MenuButton, MenuItem, MenuModel};

/// A headless app with the a11y + focus + widget surface (mirrors `tests/menu.rs`):
/// `A11yPlugin` runs `build_tree` (so the in-process snapshot reflects the menu) and
/// inits the `InlineActionRegistry`; `WidgetsPlugin` carries the menu machine AND
/// populates the registry with the menu's inline AT hook (`menu_inline_action_hook`).
fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);
    app.add_plugins(FocusPlugin);
    app.add_plugins(WidgetsPlugin);
    // No `InputPlugin` under `MinimalPlugins`: supply the keyboard infra the focus /
    // keyboard-nav systems read (mirrors `tests/menu.rs`).
    app.init_resource::<ButtonInput<KeyCode>>();
    app.add_message::<KeyboardInput>();
    app
}

/// Spawn a 3-item menu button, settle the `wire_menu_button` edge wiring, and return
/// `(button, menu)`.
fn spawn_menu_button(app: &mut App) -> (Entity, Entity) {
    let button = app
        .world_mut()
        .spawn(MenuButton::new(
            "Edit",
            children![
                MenuItem::new("Cut"),
                MenuItem::new("Copy"),
                MenuItem::new("Paste"),
            ],
        ))
        .id();
    for _ in 0..3 {
        app.update();
    }
    let menu = menu_of(app, button);
    (button, menu)
}

fn menu_of(app: &App, button: Entity) -> Entity {
    let world = app.world();
    world
        .get::<Children>(button)
        .expect("button has children")
        .iter()
        .find(|&c| world.get::<Menu>(c).is_some())
        .expect("button has a Menu child")
}

/// The button node's projected `aria-expanded`, read out of the freshly-built a11y tree
/// (the in-process snapshot — the same cached `build_tree` views an AT consumes).
fn tree_expanded(app: &mut App, button: Entity) -> Option<bool> {
    let tree = snapshot(app.world_mut(), TreeView::default());
    tree.node(node_id_for(button))
        .expect("the menu button emits an a11y node")
        .state
        .expanded
}

fn model(app: &App, menu: Entity) -> MenuModel {
    app.world().get::<MenuModel>(menu).cloned().unwrap()
}

fn is_visible(app: &App, e: Entity) -> bool {
    buiy_widgets::popover::is_open(app.world().get::<CssVisibility>(e))
}

// ===========================================================================
// GATE 1 — the corrected contract (§5.1): live-component-synchronous at
// dispatch-return; aria-expanded projected after ONE app.update(). Symmetric.
// ===========================================================================

#[test]
fn at_expand_on_menu_button_is_live_synchronous_then_projects_after_update() {
    let mut app = app();
    let (button, menu) = spawn_menu_button(&mut app);

    // Precondition: the menu starts closed (both the model and the projected tree).
    assert!(!model(&app, menu).open, "the menu starts closed (model)");
    assert_eq!(
        tree_expanded(&mut app, button),
        Some(false),
        "the button starts aria-expanded=false"
    );

    // --- AT Expand: dispatch + snapshot, NO interceding app.update() (`perform` is
    //     `dispatch_action_request` then `snapshot`, inprocess.rs §3). ----------------
    perform(app.world_mut(), Action::Expand, node_id_for(button), None)
        .expect("AT Expand honored on the MenuButton");

    // (a) LIVE-COMPONENT-SYNCHRONOUS: the inline hook folded `MenuMsg::Open` through the
    //     SAME `menu_reducer` the batch drain uses, so the live `MenuModel` mutated the
    //     instant `dispatch_action_request` returned — observed directly, NO app.update().
    let m = model(&app, menu);
    assert!(
        m.open,
        "LIVE-SYNC: MenuModel.open is true the instant dispatch returned (no app.update) \
         — the inline fold, not the inert direct A11yExpanded write the bind would clobber"
    );
    assert_eq!(
        m.active,
        Some(0),
        "the reducer ran fully (Open highlights the first item) — absolute Open, not Toggle"
    );

    // (b) PERFORM-THEN-UPDATE: the early `MenuSet::Bind` projects `MenuModel.open →
    //     button.A11yExpanded` and `build_tree` (A11yUpdate) reads it — so the snapshot
    //     shows aria-expanded=true after exactly ONE app.update().
    app.update();
    assert_eq!(
        tree_expanded(&mut app, button),
        Some(true),
        "PERFORM-THEN-UPDATE: the a11y tree shows aria-expanded=true after one app.update()"
    );
    assert!(
        is_visible(&app, menu),
        "the menu is visible (CssVisibility projected by the early bind)"
    );

    // --- Symmetric AT Collapse (absolute Close, not Toggle). -------------------------
    perform(app.world_mut(), Action::Collapse, node_id_for(button), None)
        .expect("AT Collapse honored on the MenuButton");
    assert!(
        !model(&app, menu).open,
        "LIVE-SYNC (symmetric): MenuModel.open is false the instant Collapse returned"
    );
    app.update();
    assert_eq!(
        tree_expanded(&mut app, button),
        Some(false),
        "PERFORM-THEN-UPDATE (symmetric): aria-expanded=false after one app.update()"
    );
    assert!(
        !is_visible(&app, menu),
        "the menu is hidden again after Collapse"
    );
}

#[test]
fn at_expand_is_absolute_idempotent_not_a_toggle() {
    // The W6a gap was that AT-Expand was inert; the W6b risk is that it folds `Toggle`
    // (which would CLOSE an already-open menu). Prove the hook folds the ABSOLUTE verb:
    // a second `Expand` on an already-open menu LEAVES it open.
    let mut app = app();
    let (button, menu) = spawn_menu_button(&mut app);

    perform(app.world_mut(), Action::Expand, node_id_for(button), None).unwrap();
    assert!(model(&app, menu).open, "first Expand opens");
    perform(app.world_mut(), Action::Expand, node_id_for(button), None).unwrap();
    assert!(
        model(&app, menu).open,
        "ABSOLUTE: a second Expand on an open menu keeps it open (NOT a Toggle that closes)"
    );
}

// ===========================================================================
// GATE 2 — the §5.7 replay test: the AT fold is a RECORDED Msg; the session
// round-trips BYTE-IDENTICALLY through replay_into.
// ===========================================================================

const LID_MENU: u64 = 7700;

#[test]
fn at_expand_records_a_menu_msg_and_replays_byte_identically() {
    // --- Record app: spawn, assign the menu a stable LogicalId, record an AT Expand. -
    let mut rec = app();
    let (rec_button, rec_menu) = spawn_menu_button(&mut rec);
    // The fold targets the MENU (the cross-entity hop resolves button → menu), so the
    // MENU carries the LogicalId the log keys on (the same id the replay app assigns).
    rec.world_mut()
        .entity_mut(rec_menu)
        .insert(LogicalId(LID_MENU));
    rec.update();

    // Record ON (the unified switch, seq reset to 0).
    rec.world_mut().resource_mut::<RecordSession>().start();
    perform(
        rec.world_mut(),
        Action::Expand,
        node_id_for(rec_button),
        None,
    )
    .expect("AT Expand honored (recording)");

    // (1) The AT action became a RECORDED Msg in the log (closes L5): exactly one entry,
    //     keyed by the menu's LogicalId, a `MenuMsg::Open`.
    let recorded_model = model(&rec, rec_menu);
    {
        let log = rec.world().resource::<MsgLog>();
        assert_eq!(
            log.entries.len(),
            1,
            "the inline AT fold recorded exactly one MenuMsg (Open emits nothing)"
        );
        let entry = &log.entries[0];
        assert_eq!(
            entry.lid,
            LogicalId(LID_MENU),
            "keyed by the menu's LogicalId"
        );
        assert!(
            entry.type_path.contains("MenuMsg"),
            "the entry is a MenuMsg: {}",
            entry.type_path
        );
        assert!(
            entry.ron.contains("Open"),
            "the recorded message is the ABSOLUTE Open (not Toggle): {}",
            entry.ron
        );
    }
    assert!(recorded_model.open, "the record app's menu is open");

    // --- Replay into a FRESH app from the SAME seed (same menu, SAME LogicalId). ------
    let mut replay = app();
    let (_replay_button, replay_menu) = spawn_menu_button(&mut replay);
    replay
        .world_mut()
        .entity_mut(replay_menu)
        .insert(LogicalId(LID_MENU));
    replay.update();
    assert!(
        !model(&replay, replay_menu).open,
        "precondition: the fresh app's menu is closed before replay"
    );

    let dead_letters = {
        let world = rec.world();
        let msg_log = world.resource::<MsgLog>();
        // The menu app has no editor log; an empty `EditLog` covers the widget-only stream.
        replay_into(&mut replay, msg_log, &EditLog::default())
    };
    replay.update(); // settle the re-folded model's projection

    // (2) Zero dead-letters (the menu's LogicalId resolved in the fresh app).
    assert!(
        dead_letters.is_empty(),
        "the replay resolved every logged target (no dead letters): {dead_letters:?}"
    );

    // (3) BYTE-IDENTICAL: the replayed MenuModel equals the recorded one (open + active +
    //     dismissed all reproduced by re-folding the logged MenuMsg::Open).
    let replayed_model = model(&replay, replay_menu);
    assert_eq!(
        replayed_model, recorded_model,
        "AT-EXPAND REPLAY IS BYTE-IDENTICAL: re-folding the logged MenuMsg::Open into a \
         fresh app from the same seed reproduces the menu's whole machine state \
         (open/active/dismissed)"
    );
    // Spell the expected end-state explicitly (Open ⇒ open + first item active).
    assert_eq!(
        replayed_model,
        MenuModel {
            open: true,
            active: Some(0),
            dismissed: None,
        },
        "the replayed model is the Open end-state"
    );
}
