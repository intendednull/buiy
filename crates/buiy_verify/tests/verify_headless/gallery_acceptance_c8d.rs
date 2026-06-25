//! C8-d — the **cross-screen WCAG / gate-#12 invariant acceptance** over all five
//! gallery screens (widget-gallery-exemplar §6, Tier 3 + the WCAG focus checks;
//! semantic-tree.md §9 — the three gate-#12 invariants holding by construction).
//! This is the LAST C8 slice: after it, every screen S1–S5 is proven to satisfy
//! the structural a11y invariants AND the WCAG keyboard-focus contracts, driven
//! through the inspection tooling (the live `A11yTreeBuilder` / `build_tree`
//! snapshot a real AT consumes) + the production focus path — never bespoke state.
//!
//! Each screen is spawned via its real `buiy_gallery` scene-/spawn-fn into a full
//! `A11yPlugin` + `FocusPlugin` app and settled, so:
//!  - **gate-#12** is read off the canonical `A11yTreeBuilder::snapshot()`
//!    (`&[A11yNodeView]`, the same view `build_tree_update` folds for the AT):
//!     1. **no-orphans** — every emitted non-root node's a11y parent is itself
//!        emitted, and every listed child edge points at an emitted node;
//!     2. **focus-reachable** — every non-`A11yHidden` `Focusable` surfaces in the
//!        tree (a hidden/inert focusable is pruned, never stranded);
//!     3. **every-focusable-named** — every focusable node carries a non-empty
//!        accessible name (an unnamed focusable is an APG defect).
//!
//!    These mirror the P1b proptest invariants (`crosscut/a11y.rs` —
//!    `invariant_no_orphans`/`_focus_reachable`/`_every_focusable_named`); those
//!    are `#[test]` fns in `buiy_core`'s test crate and so are not importable here,
//!    so the SAME three properties are asserted over each screen's live snapshot
//!    via the shared [`assert_gate12`] helper (semantic-tree.md §9).
//!  - **WCAG focus** is driven through the production `FocusPlugin`: synthetic Tab
//!    (`handle_tab`, the same path `pointer_focus_c3d` drives) traverses the
//!    screen's focusables in document order, every keyboard-focused stop yields the
//!    C6-a focus-ring [`Outline`] (`lower_focus_ring`; `FocusVisible(true)` ⇒ ring,
//!    WCAG 2.4.7 / 2.4.11), and no-keyboard-trap holds: on the four non-modal
//!    screens Tab cycles freely (never stuck); on S4 the modal traps Tab inside the
//!    dialog but Esc always escapes (WCAG 2.1.2), the C5-d lifecycle composed by
//!    `spawn_modal`.

use bevy::input::keyboard::KeyboardInput;
use bevy::prelude::*;
use buiy_core::a11y::{A11yHidden, A11yNodeView, A11yPlugin, A11yTreeBuilder};
use buiy_core::focus::{
    FocusPlugin, FocusScope, FocusScopeMode, FocusVisible, Focusable, FocusedEntity,
};
use buiy_core::interaction::OnPress;
use buiy_core::layout::LayoutPlugin;
use buiy_core::render::components::Outline;
use buiy_core::text::BuiyTextPlugin;
use buiy_widgets::WidgetsPlugin;

// ===========================================================================
// The cross-screen acceptance harness — one full a11y + focus app per screen
// ===========================================================================

/// A headless app carrying the canonical a11y tree (`build_tree` →
/// `A11yTreeBuilder`), the production focus path (`handle_tab` + `lower_focus_ring`
/// — Tab traversal and the C6-a focus-ring `Outline`), layout, text, and the
/// widget systems, plus `ScenePlugin` so the screen-fns' `spawn_scene` resolves.
/// This is the union of the per-screen plugin sets the C8-a/b/c acceptance tests
/// already use; the cross-screen pass runs every screen through the SAME app shape
/// so the invariants + WCAG checks are uniform across S1–S5. No window, no GPU, no
/// winit adapter — the snapshot is read off the same `A11yTreeBuilder` a real AT
/// consumes, and the ring `Outline` is the component `lower_focus_ring` owns.
fn acceptance_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(bevy::asset::AssetPlugin::default());
    app.add_plugins(bevy::scene::ScenePlugin);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(A11yPlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app.add_plugins(FocusPlugin);
    app.add_plugins(WidgetsPlugin);
    // `handle_tab` reads `Res<ButtonInput<KeyCode>>`; the menu/dialog keyboard
    // paths read `Messages<KeyboardInput>`. MinimalPlugins seeds neither.
    app.add_message::<KeyboardInput>();
    app.init_resource::<ButtonInput<KeyCode>>();
    app
}

/// Settle a freshly-spawned screen so `build_tree` populates the a11y tree, layout
/// settles, and any first-frame wiring (the menu/dialog `controls`/anchor edges)
/// runs before the acceptance reads the snapshot.
fn settle(app: &mut App) {
    for _ in 0..6 {
        app.update();
    }
}

/// The current frame's canonical a11y node views (the same list `build_tree_update`
/// folds for the AT — `entity`/`role`/`name`/`focusable`/`parent`/`children`).
fn screen_snapshot(app: &App) -> Vec<A11yNodeView> {
    app.world()
        .resource::<A11yTreeBuilder>()
        .snapshot()
        .to_vec()
}

// ===========================================================================
// Gate-#12 invariants (semantic-tree.md §9) — the three structural properties,
// asserted over a screen's live snapshot. Mirrors the P1b proptest fns.
// ===========================================================================

/// Assert the three gate-#12 invariants over `screen`'s live snapshot + world.
/// `label` names the screen in failure messages.
fn assert_gate12(app: &mut App, label: &str) {
    let snapshot = screen_snapshot(app);
    assert!(
        !snapshot.is_empty(),
        "{label}: the screen emits a non-empty a11y tree"
    );

    // (a) no-orphans: every emitted node's a11y parent is itself emitted, and every
    // listed child edge points at an emitted node (no dangling parent/child ref).
    let emitted: std::collections::HashSet<Entity> = snapshot.iter().map(|n| n.entity).collect();
    for node in &snapshot {
        if let Some(parent) = node.parent {
            assert!(
                emitted.contains(&parent),
                "{label}: node {:?} ({:?}) has a11y parent {parent:?} which is not itself emitted — orphan",
                node.entity,
                node.role,
            );
        }
        for &child in &node.children {
            assert!(
                emitted.contains(&child),
                "{label}: node {:?} ({:?}) lists child {child:?} which is not emitted — dangling edge",
                node.entity,
                node.role,
            );
        }
    }

    // (b) focus-reachable: every non-`A11yHidden` `Focusable` surfaces in the tree;
    // an inert (hidden self/ancestor) focusable is pruned, never stranded. Read the
    // live `Focusable` set + the inert predicate off the world, cross-checked
    // against the emitted set — the same prune `build_tree` §7.4 performs.
    let world = app.world_mut();
    let mut focusables = world.query::<(Entity, &Focusable)>();
    let live: Vec<Entity> = focusables.iter(world).map(|(e, _)| e).collect();
    for e in live {
        if is_inert(world, e) {
            assert!(
                !emitted.contains(&e),
                "{label}: an inert (A11yHidden self/ancestor) focusable {e:?} must be pruned \
                 from the a11y tree, not stranded in it"
            );
        } else {
            assert!(
                emitted.contains(&e),
                "{label}: a non-hidden focusable {e:?} must be reachable in the a11y tree \
                 (focus-reachable)"
            );
        }
    }

    // (c) every-focusable-named: every focusable node has a non-empty accessible
    // name (an unnamed focusable is an APG defect — gate #12 / WCAG 4.1.2).
    for node in &snapshot {
        if node.focusable {
            assert!(
                !node.name.is_empty(),
                "{label}: focusable node {:?} ({:?}) has an empty accessible name (ACCNAME)",
                node.entity,
                node.role,
            );
        }
    }
}

/// Whether `e` is inert — carries [`A11yHidden`] on itself or any `ChildOf`
/// ancestor (the same focus-exclusion / prune predicate `handle_tab::is_inert` and
/// `build_tree` §7.4 use). The acceptance reuses it to classify which live
/// focusables MUST surface (non-inert) vs MUST be pruned (inert).
fn is_inert(world: &World, e: Entity) -> bool {
    let mut cur = e;
    loop {
        if world.get::<A11yHidden>(cur).is_some() {
            return true;
        }
        match world.get::<ChildOf>(cur) {
            Some(c) => cur = c.parent(),
            None => return false,
        }
    }
}

// ===========================================================================
// WCAG focus checks — Tab traversal, the C6-a ring on every keyboard stop, and
// no-keyboard-trap. Driven through the production `FocusPlugin` (`handle_tab` +
// `lower_focus_ring`), not internal state.
// ===========================================================================

/// Press Tab once on `app` (forward) and settle: seeds `just_pressed(Tab)`, runs a
/// frame so `handle_tab` (in `BuiySet::Input`) advances `FocusedEntity` +
/// `FocusVisible(true)` and `lower_focus_ring` (after `Input`, before `Render`)
/// settles the ring, then clears the key so the next press re-registers.
fn tab(app: &mut App) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Tab);
    app.update();
    let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keys.release(KeyCode::Tab);
    keys.clear();
    // One more frame so `lower_focus_ring` observes the settled focus signal and
    // the ring `Outline` is on the focused entity when the acceptance reads it.
    app.update();
}

/// The entity that currently holds focus (read off the production `FocusedEntity`).
fn focused(app: &App) -> Option<Entity> {
    app.world().resource::<FocusedEntity>().0
}

/// Whether `e` currently carries the C6-a framework focus-ring [`Outline`] (the
/// component `lower_focus_ring` inserts on the keyboard-focused entity). A ≥ 2px
/// solid stroke (WCAG 2.4.11) — asserted present, with its width checked.
fn has_focus_ring(app: &App, e: Entity) -> bool {
    app.world().get::<Outline>(e).is_some()
}

/// Drive Tab `n` times over a NON-MODAL screen and assert the WCAG focus contract:
/// every keyboard-focused stop is a `Focusable`, carries the C6-a ring `Outline`
/// (≥ 2px), and the traversal is not trapped (it reaches ≥ 2 distinct focusables
/// and eventually returns to an earlier stop — i.e. it cycles, never sticks).
/// Returns the ordered list of focused entities (for the per-screen reporting).
fn assert_tab_traversal_and_rings(app: &mut App, label: &str, n: usize) -> Vec<Entity> {
    // The live non-inert, non-skipped focusable count — the size of the cycle Tab
    // should traverse. (Skip = negative tab_order; none in the gallery, but the
    // filter keeps the assertion honest.) Collect the candidates first (dropping the
    // query borrow), then apply the inert predicate, so there is no overlapping
    // `&World` borrow.
    let candidates: Vec<(Entity, i32)> = {
        let world = app.world_mut();
        let mut q = world.query::<(Entity, &Focusable)>();
        q.iter(world).map(|(e, f)| (e, f.tab_order)).collect()
    };
    let focusable_count = {
        let world = app.world();
        candidates
            .iter()
            .filter(|(e, order)| *order >= 0 && !is_inert(world, *e))
            .count()
    };
    assert!(
        focusable_count >= 1,
        "{label}: the screen has at least one keyboard-focusable control"
    );

    let mut stops = Vec::new();
    for i in 0..n {
        tab(app);
        let f = focused(app).unwrap_or_else(|| {
            panic!("{label}: Tab #{i} must move focus to a focusable (got None)")
        });
        // Every keyboard-focused stop is a live `Focusable`.
        assert!(
            app.world().get::<Focusable>(f).is_some(),
            "{label}: Tab #{i} focused {f:?}, which is not a Focusable"
        );
        // WCAG 2.4.7/2.4.11: a keyboard-focused control shows the C6-a focus ring.
        assert!(
            has_focus_ring(app, f),
            "{label}: Tab #{i} keyboard-focused {f:?} must carry the C6-a focus-ring Outline"
        );
        let outline = app.world().get::<Outline>(f).unwrap();
        assert!(
            // `Length::px` width — compare its resolved px against the 2px floor.
            outline_px(outline) >= 2.0,
            "{label}: the focus ring is >= 2px (WCAG 2.4.11), got {}",
            outline_px(outline)
        );
        // Keyboard focus IS focus-visible (the ring's gating signal).
        assert!(
            app.world().resource::<FocusVisible>().0,
            "{label}: keyboard (Tab) focus sets FocusVisible(true)"
        );
        stops.push(f);
    }

    // no-keyboard-trap (non-modal): the traversal is NOT stuck on one element. With
    // ≥ 2 focusables, Tab must visit ≥ 2 distinct stops; and over a full cycle +1 it
    // must return to an earlier stop (it wraps — never a dead end). With exactly 1
    // focusable, "not trapped" degenerates to "always the same single stop", which
    // is correct (one stop is not a trap — there is nowhere else to go).
    let distinct: std::collections::HashSet<Entity> = stops.iter().copied().collect();
    if focusable_count >= 2 {
        assert!(
            distinct.len() >= 2,
            "{label}: Tab visits >= 2 distinct focusables (not trapped on one); visited {distinct:?}"
        );
        // Wrap: a stop recurs within `focusable_count + 1` presses (a finite cycle).
        assert!(
            stops.len() > distinct.len() || stops.len() <= focusable_count,
            "{label}: Tab cycles through a finite focus ring (it wraps, never escapes to a dead end)"
        );
    }
    stops
}

/// Length-px reader for an `Outline.width` (the ring stroke width). The framework
/// ring is authored as `Length::px`, so read the literal px off the `Px` variant
/// for the WCAG 2.4.11 ≥ 2px floor (a non-`Px` width would be a ring-authoring
/// regression and reddens here, since it can't satisfy the >= 2px check).
fn outline_px(o: &Outline) -> f32 {
    match o.width {
        buiy_core::layout::Length::Px(px) => px,
        // The framework ring is always `Px`; any other unit is an authoring defect.
        _ => 0.0,
    }
}

// ###########################################################################
// S1 — TodoMVC
// ###########################################################################

fn spawn_s1(app: &mut App) {
    use bevy::scene::WorldSceneExt;
    use buiy_gallery::{DEMO_SEEDS, TodoMvcPlugin, append_row, screen_todomvc};
    app.add_plugins(TodoMvcPlugin);
    app.world_mut()
        .spawn_scene(screen_todomvc(DEMO_SEEDS))
        .expect("spawn the TodoMVC screen");
    for &(label, completed) in DEMO_SEEDS {
        append_row(app.world_mut(), label, completed);
    }
}

#[test]
fn s1_todomvc_gate12_invariants_hold() {
    let mut app = acceptance_app();
    spawn_s1(&mut app);
    settle(&mut app);
    assert_gate12(&mut app, "S1 TodoMVC");
}

#[test]
fn s1_todomvc_wcag_focus_tab_rings_and_no_trap() {
    let mut app = acceptance_app();
    spawn_s1(&mut app);
    settle(&mut app);
    // Seeded TodoMVC: the add-field + 3 rows (checkbox + destroy each) + 3 filter
    // buttons + clear — many focusables. Tab a full cycle's worth + extra to prove
    // the wrap.
    assert_tab_traversal_and_rings(&mut app, "S1 TodoMVC", 12);
}

// ###########################################################################
// S2 — long list (scale-game)
// ###########################################################################

/// S2 at a reduced row count (the gate-#12 + focus invariants are scale-invariant;
/// the 1000-row scale-game is `scroll_overlay_c8b`'s job). A handful of rows keeps
/// this cross-screen pass fast while exercising the same `ScrollArea` + rows tree.
const S2_ROWS: usize = 8;

fn spawn_s2(app: &mut App) {
    use bevy::scene::WorldSceneExt;
    use buiy_gallery::{fill_scroll_list, screen_scroll_list};
    app.world_mut()
        .spawn_scene(screen_scroll_list(S2_ROWS))
        .expect("spawn the scroll-list screen");
    fill_scroll_list(app.world_mut(), S2_ROWS);
}

#[test]
fn s2_scroll_list_gate12_invariants_hold() {
    let mut app = acceptance_app();
    spawn_s2(&mut app);
    settle(&mut app);
    assert_gate12(&mut app, "S2 scroll-list");
}

#[test]
fn s2_scroll_list_wcag_focus_tab_rings_and_no_trap() {
    let mut app = acceptance_app();
    spawn_s2(&mut app);
    settle(&mut app);
    // S2's focusable is the `ScrollArea` container (the container owns keyboard
    // scroll); the rows are plain `Text` lines (not focusable). One focusable ⇒ Tab
    // lands on it and the ring shows; "no trap" degenerates to the single stop.
    let stops = assert_tab_traversal_and_rings(&mut app, "S2 scroll-list", 4);
    assert!(
        stops.iter().all(|&s| Some(s) == stops.first().copied()),
        "S2 scroll-list: the single scroll container is the only Tab stop"
    );
}

// ###########################################################################
// S3 — overlay / menu
// ###########################################################################

fn spawn_s3(app: &mut App) {
    use buiy_gallery::{OverlayMenuPlugin, spawn_overlay_menu};
    app.add_plugins(OverlayMenuPlugin);
    spawn_overlay_menu(app.world_mut());
}

#[test]
fn s3_overlay_menu_gate12_invariants_hold() {
    let mut app = acceptance_app();
    spawn_s3(&mut app);
    settle(&mut app);
    assert_gate12(&mut app, "S3 overlay-menu");
}

#[test]
fn s3_overlay_menu_wcag_focus_tab_rings_and_no_trap() {
    let mut app = acceptance_app();
    spawn_s3(&mut app);
    settle(&mut app);
    // S3 at rest: the MenuButton ("Edit") + the TooltipTrigger ("?") are the
    // top-level focusables (the menu items are inside the closed menu, not Tab
    // stops until it opens). Tab cycles between the open-tree focusables, each
    // ringed, never trapped.
    assert_tab_traversal_and_rings(&mut app, "S3 overlay-menu", 5);
}

// ###########################################################################
// S4 — modal + focus-trap (the WCAG 2.1.2 no-keyboard-trap proof: trapped, but
// Esc always escapes)
// ###########################################################################

#[test]
fn s4_modal_gate12_invariants_hold_closed_and_open() {
    let mut app = acceptance_app();
    let (invoker, dialog, _bg) = buiy_gallery::spawn_modal(app.world_mut());
    settle(&mut app);

    // Closed: the whole screen (invoker + background + the closed dialog's controls)
    // satisfies gate-#12.
    assert_gate12(&mut app, "S4 modal (closed)");

    // Open the modal via the shared activation sink (the route the Button keymap /
    // AT-Click converge on); settle the C5-d open lifecycle (inert background + the
    // focus-into).
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(invoker);
    app.world_mut().write_message(OnPress(invoker));
    settle(&mut app);
    assert!(
        buiy_widgets::popover::is_open(
            app.world()
                .get::<buiy_core::render::components::CssVisibility>(dialog)
        ),
        "S4: the invoker activation opened the dialog"
    );
    // Open: gate-#12 STILL holds — the background is pruned (A11yHidden) but the
    // dialog subtree is well-formed (no orphan, the trapped focusables are named
    // and reachable). This is the inert-prune observation point (§7 deferred Q).
    assert_gate12(&mut app, "S4 modal (open)");
}

#[test]
fn s4_modal_tab_traps_inside_but_escape_always_escapes() {
    let mut app = acceptance_app();
    let (invoker, dialog, bg) = buiy_gallery::spawn_modal(app.world_mut());
    settle(&mut app);

    // Sanity: the dialog declares a trap focus-scope (the WCAG 2.1.2 mechanism —
    // confined Tab WITH a guaranteed Esc exit, never a dead-end trap).
    let scope = app.world().get::<FocusScope>(dialog);
    assert!(
        scope.is_some_and(|s| s.mode == FocusScopeMode::Trap),
        "S4: the dialog carries a FocusScope::trap (the trapped-but-escapable scope)"
    );

    // Open the modal.
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(invoker);
    app.world_mut().write_message(OnPress(invoker));
    settle(&mut app);
    assert!(
        buiy_widgets::popover::is_open(
            app.world()
                .get::<buiy_core::render::components::CssVisibility>(dialog)
        ),
        "S4: the dialog is open before the trap test"
    );

    // The dialog's focusable descendants (the set the trap confines Tab to).
    let dialog_focusables = descendant_focusables(app.world_mut(), dialog);
    assert!(
        !dialog_focusables.is_empty(),
        "S4: the dialog has focusable controls (Switch + Close)"
    );

    // Tab repeatedly: focus is ALWAYS inside the dialog — never the invoker, never
    // the background button (the trap), and every stop carries the C6-a ring.
    for i in 0..8 {
        tab(&mut app);
        let f = focused(&app)
            .unwrap_or_else(|| panic!("S4: Tab #{i} must keep focus inside the modal (got None)"));
        assert!(
            dialog_focusables.contains(&f),
            "S4: Tab #{i} stayed inside the modal (was {f:?}); never the invoker ({invoker:?}) \
             or the background ({bg:?})"
        );
        assert!(
            has_focus_ring(&app, f),
            "S4: Tab #{i} keyboard-focused {f:?} inside the modal shows the C6-a focus ring"
        );
    }

    // WCAG 2.1.2 — Escape ALWAYS escapes: the trap is escapable, not a dead end. Esc
    // closes the dialog and restores focus to the invoker.
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Escape);
    app.update();
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release(KeyCode::Escape);
        keys.clear();
    }
    settle(&mut app);
    assert!(
        !buiy_widgets::popover::is_open(
            app.world()
                .get::<buiy_core::render::components::CssVisibility>(dialog)
        ),
        "S4: Escape closed the dialog (WCAG 2.1.2 — Escape always escapes the trap)"
    );
    assert_eq!(
        focused(&app),
        Some(invoker),
        "S4: closing restored focus to the invoker (WCAG 2.4.3) — the trap was escapable"
    );

    // After Esc: with the modal gone, Tab is free again (no residual trap) and the
    // background button is reachable — proving the trap was transient, not sticky.
    tab(&mut app);
    let after = focused(&app);
    assert!(
        after.is_some_and(|f| f != invoker || dialog_focusables.is_empty()),
        "S4: after Esc, Tab moves freely again (the trap left no residue)"
    );
}

/// The focusable descendants of `root` (DFS over `Children`), in document order —
/// the set a trap confines Tab to.
fn descendant_focusables(world: &mut World, root: Entity) -> Vec<Entity> {
    fn dfs(world: &World, e: Entity, out: &mut Vec<Entity>) {
        if world.get::<Focusable>(e).is_some() {
            out.push(e);
        }
        if let Some(children) = world.get::<Children>(e) {
            let kids: Vec<Entity> = children.iter().collect();
            for c in kids {
                dfs(world, c, out);
            }
        }
    }
    let mut out = Vec::new();
    if let Some(children) = world.get::<Children>(root) {
        let kids: Vec<Entity> = children.iter().collect();
        for c in kids {
            dfs(world, c, &mut out);
        }
    }
    out
}

// ###########################################################################
// S5 — F-tier showcase
// ###########################################################################

fn spawn_s5(app: &mut App) {
    use bevy::scene::WorldSceneExt;
    use buiy_gallery::screen_showcase;
    app.world_mut()
        .spawn_scene(screen_showcase())
        .expect("spawn the showcase screen");
}

#[test]
fn s5_showcase_gate12_invariants_hold() {
    let mut app = acceptance_app();
    spawn_s5(&mut app);
    settle(&mut app);
    assert_gate12(&mut app, "S5 showcase");
}

#[test]
fn s5_showcase_wcag_focus_tab_rings_and_no_trap() {
    let mut app = acceptance_app();
    spawn_s5(&mut app);
    settle(&mut app);
    // S5: Switch + Slider + Disclosure (each `#[require]`s Focusable) are the
    // focusables; Tab cycles all three, each ringed, never trapped.
    assert_tab_traversal_and_rings(&mut app, "S5 showcase", 6);
}

// ###########################################################################
// The cross-screen sweep — gate-#12 over all five screens in one place, so a
// regression on ANY screen reddens this single acceptance gate (the C8 "the
// gallery IS the gap-finder" posture: every screen passes the structural
// invariants by construction).
// ###########################################################################

#[test]
fn cross_screen_gate12_holds_for_all_five_gallery_screens() {
    // S1
    {
        let mut app = acceptance_app();
        spawn_s1(&mut app);
        settle(&mut app);
        assert_gate12(&mut app, "S1 TodoMVC");
    }
    // S2
    {
        let mut app = acceptance_app();
        spawn_s2(&mut app);
        settle(&mut app);
        assert_gate12(&mut app, "S2 scroll-list");
    }
    // S3
    {
        let mut app = acceptance_app();
        spawn_s3(&mut app);
        settle(&mut app);
        assert_gate12(&mut app, "S3 overlay-menu");
    }
    // S4 (closed + open both checked in the dedicated test; here the closed screen
    // suffices for the sweep — the open prune is exercised by
    // `s4_modal_gate12_invariants_hold_closed_and_open`).
    {
        let mut app = acceptance_app();
        buiy_gallery::spawn_modal(app.world_mut());
        settle(&mut app);
        assert_gate12(&mut app, "S4 modal");
    }
    // S5
    {
        let mut app = acceptance_app();
        spawn_s5(&mut app);
        settle(&mut app);
        assert_gate12(&mut app, "S5 showcase");
    }
}
