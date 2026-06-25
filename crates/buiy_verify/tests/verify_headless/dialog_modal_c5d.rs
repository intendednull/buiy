//! C5-d — the Dialog open/focus-trap/Esc/restore + scoped FocusScope + inert
//! background overlay state machine, proven headless on the C7 `PointerHarness`
//! and the in-process a11y driver (scroll-overlay-modal.md §C.1/§C.2/§C.4/§C.5).
//! The LAST C5 slice.
//!
//! Gates exercised:
//!  - **Open** — activating an invoker opens the controlled dialog (`A11yModal`
//!    active, focus moves into the dialog to its first focusable).
//!  - **Focus-trap cycles + wraps** — with the modal open, repeated Tab cycles
//!    ONLY through the dialog's focusables and WRAPS; Shift+Tab reverses; a
//!    background element is NEVER reachable by Tab while the modal is open.
//!  - **Esc closes + restores** — Escape (WCAG 2.1.2, always escapes) closes the
//!    dialog and restores focus to the invoker; a `DialogClose` button too.
//!  - **Inert background** — while open, background elements are `A11yHidden` →
//!    pruned from the a11y tree (the driver snapshot shows only the modal subtree),
//!    and not focusable; restored on close.
//!  - **Non-modal Tab unregressed** — with no modal open, Tab is the flat-global
//!    traversal (the regression guard for the high-blast-radius `focus.rs` edit).
//!
//! The open path is driven by **focusing the invoker + writing the shared
//! `OnPress` sink** — the keyboard/AT activation route (Tab-to-invoker, Enter →
//! `OnPress`), the same isolation the C5-c menu fixture uses. This exercises the
//! full production lifecycle (`open_dialog_on_invoker_press` →
//! `apply_dialog_modal_state` → `resolve_pending_focus`) without coupling the test
//! to overlay picking geometry (a *hit-test* exclusion of hidden/inert content is
//! C3's `emit_picks` concern, not C5-d's). One pointer-driven close test confirms
//! the `DialogClose` button rides the real Click → `OnPress` path.

use bevy::prelude::*;
use buiy_core::CorePlugin;
use buiy_core::a11y::inprocess::snapshot;
use buiy_core::a11y::{A11yHidden, A11yLabel, A11yPlugin, A11yRole};
use buiy_core::components::Node;
use buiy_core::focus::{FocusPlugin, FocusVisible, FocusedEntity};
use buiy_core::interaction::OnPress;
use buiy_core::layout::{LayoutPlugin, Style, TopLayerActivation};
use buiy_core::render::components::CssVisibility;
use buiy_verify::pointer::PointerHarness;
use buiy_widgets::WidgetsPlugin;
use buiy_widgets::dialog::{Dialog, DialogClose, dialog_invoker};

// ---------------------------------------------------------------------------
// Fixture shared by the PointerHarness-driven tests.
// ---------------------------------------------------------------------------

/// A background button (a root, OUTSIDE the dialog) labelled `label`. A focusable
/// + a11y node — used to prove the trap excludes it and the inert prune drops it.
fn spawn_bg_button(world: &mut World, label: &str) -> Entity {
    world
        .spawn((buiy_widgets::Button, A11yLabel(label.to_string())))
        .id()
}

/// Spawn the modal fixture into `world`: a background button (outside the dialog),
/// an invoker that controls the dialog, and a closed `TopLayer::Modal` dialog with
/// two focusable close buttons inside, all under a window-sized root. Returns
/// `(invoker, dialog, bg, [close1, close2])`.
fn spawn_fixture(world: &mut World) -> (Entity, Entity, Entity, Vec<Entity>) {
    let bg = spawn_bg_button(world, "Background");

    let dialog = world
        .spawn((
            Dialog,
            children![
                (
                    buiy_widgets::Button,
                    DialogClose,
                    A11yLabel("OK".to_string()),
                ),
                (
                    buiy_widgets::Button,
                    DialogClose,
                    A11yLabel("Cancel".to_string()),
                ),
            ],
        ))
        .id();

    let invoker = world.spawn(dialog_invoker("Open dialog", dialog)).id();

    let root = world
        .spawn((Node, Style::default().width_px(800.0).height_px(600.0)))
        .id();
    world.entity_mut(root).add_children(&[invoker, bg]);
    (invoker, dialog, bg, Vec::new())
}

fn close_buttons(world: &World, dialog: Entity) -> Vec<Entity> {
    world
        .get::<Children>(dialog)
        .map(|c| {
            c.iter()
                .filter(|&e| world.get::<DialogClose>(e).is_some())
                .collect()
        })
        .unwrap_or_default()
}

fn is_open(world: &World, e: Entity) -> bool {
    buiy_widgets::popover::is_open(world.get::<CssVisibility>(e))
}

fn focused(world: &World) -> Option<Entity> {
    world.resource::<FocusedEntity>().0
}

/// Open the dialog via the keyboard/AT activation route: focus the invoker, then
/// write the shared `OnPress(invoker)` sink (the same sink the Button keymap / AT
/// `Click` write). Settles the lifecycle (open → inert + FocusReturn → focus-into).
fn open_via_invoker(h: &mut PointerHarness, invoker: Entity) {
    {
        let mut f = h.world_mut().resource_mut::<FocusedEntity>();
        f.0 = Some(invoker);
    }
    h.world_mut().write_message(OnPress(invoker));
    for _ in 0..6 {
        h.update();
    }
}

// ---------------------------------------------------------------------------
// 1. Open: invoker activation opens the dialog + moves focus inside.
// ---------------------------------------------------------------------------

#[test]
fn invoker_activation_opens_the_dialog_and_focuses_inside() {
    let mut h = PointerHarness::new();
    let (invoker, dialog, _bg, _) = spawn_fixture(h.world_mut());
    for _ in 0..4 {
        h.update();
    }
    let closes = close_buttons(h.world(), dialog);
    assert!(!is_open(h.world(), dialog), "the dialog starts closed");

    open_via_invoker(&mut h, invoker);

    assert!(
        is_open(h.world(), dialog),
        "the invoker activation opened the dialog"
    );
    assert!(
        h.world()
            .resource::<TopLayerActivation>()
            .order
            .contains(&dialog),
        "the open modal joined the TopLayerActivation deque"
    );
    assert_eq!(
        focused(h.world()),
        closes.first().copied(),
        "focus moved to the dialog's first focusable descendant on open"
    );
}

// ---------------------------------------------------------------------------
// 2. Focus-trap: Tab cycles ONLY the dialog focusables and wraps; Shift+Tab
//    reverses; a background element is never reachable.
// ---------------------------------------------------------------------------

#[test]
fn tab_traps_inside_the_modal_and_wraps() {
    let mut h = PointerHarness::new();
    let (invoker, dialog, bg, _) = spawn_fixture(h.world_mut());
    for _ in 0..4 {
        h.update();
    }
    let closes = close_buttons(h.world(), dialog);
    open_via_invoker(&mut h, invoker);
    assert!(is_open(h.world(), dialog));
    assert_eq!(closes.len(), 2, "two focusable close buttons in the dialog");

    // Tab N times: focus is ALWAYS one of the two close buttons — never the
    // invoker, never the background button.
    for i in 0..8 {
        h.press_key(KeyCode::Tab);
        let f = focused(h.world());
        assert!(
            f.is_some_and(|f| closes.contains(&f)),
            "Tab #{i}: focus stayed inside the modal (was {f:?}); never the invoker \
             ({invoker:?}) or the background ({bg:?})"
        );
    }

    // Two close buttons ⇒ Tab wraps: pressing twice from a stop returns to it.
    h.press_key(KeyCode::Tab);
    let a = focused(h.world());
    h.press_key(KeyCode::Tab);
    let b = focused(h.world());
    h.press_key(KeyCode::Tab);
    let c = focused(h.world());
    assert_ne!(a, b, "Tab moves between the two close buttons");
    assert_eq!(a, c, "Tab wraps within the modal (back to the same stop)");
}

#[test]
fn shift_tab_reverses_inside_the_modal() {
    let mut h = PointerHarness::new();
    let (invoker, dialog, _bg, _) = spawn_fixture(h.world_mut());
    for _ in 0..4 {
        h.update();
    }
    let closes = close_buttons(h.world(), dialog);
    open_via_invoker(&mut h, invoker);
    assert!(is_open(h.world(), dialog));

    let start = focused(h.world());
    h.press_key(KeyCode::Tab);
    let fwd = focused(h.world());
    assert_ne!(start, fwd, "Tab moved to the other close button");

    // Shift+Tab back to the start (two-element cycle).
    {
        let mut keys = h.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release_all();
        keys.clear();
        keys.press(KeyCode::ShiftLeft);
        keys.press(KeyCode::Tab);
    }
    h.update();
    {
        let mut keys = h.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release_all();
        keys.clear();
    }
    assert_eq!(
        focused(h.world()),
        start,
        "Shift+Tab reverses back to the starting close button"
    );
    assert!(
        focused(h.world()).is_some_and(|f| closes.contains(&f)),
        "Shift+Tab stays inside the modal"
    );
}

// ---------------------------------------------------------------------------
// 3. Esc / close-button close + restore focus to the invoker (no keyboard trap).
// ---------------------------------------------------------------------------

#[test]
fn escape_closes_the_dialog_and_restores_focus_to_the_invoker() {
    let mut h = PointerHarness::new();
    let (invoker, dialog, _bg, _) = spawn_fixture(h.world_mut());
    for _ in 0..4 {
        h.update();
    }
    open_via_invoker(&mut h, invoker);
    assert!(
        is_open(h.world(), dialog),
        "the dialog is open before Escape"
    );

    h.press_key(KeyCode::Escape);
    for _ in 0..3 {
        h.update();
    }

    assert!(
        !is_open(h.world(), dialog),
        "Escape closed the dialog (WCAG 2.1.2 — Escape always escapes)"
    );
    assert_eq!(
        focused(h.world()),
        Some(invoker),
        "closing restored focus to the invoker (WCAG 2.4.3)"
    );
}

#[test]
fn a_close_button_click_closes_the_dialog_and_restores_focus() {
    let mut h = PointerHarness::new();
    let (invoker, dialog, _bg, _) = spawn_fixture(h.world_mut());
    for _ in 0..4 {
        h.update();
    }
    let closes = close_buttons(h.world(), dialog);
    open_via_invoker(&mut h, invoker);
    assert!(is_open(h.world(), dialog));

    // A `DialogClose` button activation closes the dialog (the real Click →
    // OnPress route: write OnPress for the close button, as a click would).
    let close = *closes.first().unwrap();
    h.world_mut().write_message(OnPress(close));
    for _ in 0..3 {
        h.update();
    }

    assert!(
        !is_open(h.world(), dialog),
        "the close button closed the dialog"
    );
    assert_eq!(
        focused(h.world()),
        Some(invoker),
        "the close-button path restored focus to the invoker too"
    );
}

// ---------------------------------------------------------------------------
// 4. Inert background: while open, background a11y nodes are pruned; not
//    focusable; restored on close. Driven on a full A11yPlugin app so build_tree
//    populates the canonical tree the snapshot reads.
// ---------------------------------------------------------------------------

/// A headless app with the a11y + focus + layout + widget surface (build_tree
/// runs, so `snapshot` reflects the live tree). Mirrors the `a11y_inprocess`
/// harness + the widget lifecycle systems.
fn a11y_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(A11yPlugin);
    app.add_plugins(FocusPlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(WidgetsPlugin);
    app.init_resource::<ButtonInput<KeyCode>>();
    app
}

/// Open the dialog on an `A11yPlugin` app (focus the invoker + write OnPress).
fn open_on_app(app: &mut App, invoker: Entity) {
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(invoker);
    app.world_mut().write_message(OnPress(invoker));
    for _ in 0..6 {
        app.update();
    }
}

#[test]
fn open_modal_prunes_the_background_from_the_a11y_tree_and_restores_on_close() {
    let mut app = a11y_app();
    let (invoker, dialog, bg, _) = spawn_fixture(app.world_mut());
    for _ in 0..4 {
        app.update();
    }

    // Before open: the background button IS in the a11y tree.
    let before = snapshot(app.world_mut(), Default::default());
    assert!(
        before
            .by_role(A11yRole::Button)
            .any(|n| n.name == "Background"),
        "the background button is in the a11y tree before the modal opens"
    );

    open_on_app(&mut app, invoker);
    assert!(is_open(app.world(), dialog), "the dialog opened");

    // The dialog reports modal through the consumer.
    let during = snapshot(app.world_mut(), Default::default());
    let dialog_node = during
        .by_role(A11yRole::Dialog)
        .next()
        .expect("the dialog is in the a11y tree while open");
    assert!(
        dialog_node.state.modal,
        "the open dialog reports A11yModal through the consumer"
    );

    // While open: the background button + the invoker are pruned (A11yHidden on
    // the rest-of-tree roots, build_tree §7.4). The modal subtree survives.
    assert!(
        !during
            .by_role(A11yRole::Button)
            .any(|n| n.name == "Background"),
        "the background button is pruned from the a11y tree while the modal is open"
    );
    assert!(
        !during
            .by_role(A11yRole::Button)
            .any(|n| n.name == "Open dialog"),
        "the invoker (a background root) is pruned while the modal is open"
    );
    assert!(
        during.by_role(A11yRole::Button).any(|n| n.name == "OK"),
        "a focusable inside the dialog survives the prune"
    );
    // The inert marker is set on the background's top-level root ancestor (whose
    // whole subtree — invoker + bg — is then pruned + focus-excluded). The
    // lifecycle marks roots, not every descendant, since `build_tree`/`handle_tab`
    // climb `ChildOf` to find an inert ancestor.
    let bg_root = top_level_root(app.world(), bg);
    assert!(
        app.world().get::<A11yHidden>(bg_root).is_some(),
        "the background's top-level root carries A11yHidden while the modal is open \
         (its whole subtree is inert)"
    );

    // Close: the prune is reversed — the background button returns + A11yHidden
    // is removed.
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.press(KeyCode::Escape);
    }
    app.update();
    {
        let mut keys = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keys.release_all();
        keys.clear();
    }
    for _ in 0..3 {
        app.update();
    }
    assert!(!is_open(app.world(), dialog), "Escape closed the dialog");
    let after = snapshot(app.world_mut(), Default::default());
    assert!(
        after
            .by_role(A11yRole::Button)
            .any(|n| n.name == "Background"),
        "the background button returns to the a11y tree after the modal closes"
    );
    assert!(
        app.world()
            .get::<A11yHidden>(top_level_root(app.world(), bg))
            .is_none(),
        "the lifecycle removed A11yHidden from the background root on close"
    );
}

/// The top-level root ancestor of `e` (the entity with no `ChildOf`), walking up
/// the chain. The lifecycle marks these roots inert, not every descendant.
fn top_level_root(world: &World, e: Entity) -> Entity {
    let mut cur = e;
    while let Some(parent) = world.get::<ChildOf>(cur) {
        cur = parent.parent();
    }
    cur
}

// ---------------------------------------------------------------------------
// 5. Non-modal Tab is unregressed (the high-blast-radius focus.rs guard).
// ---------------------------------------------------------------------------

#[test]
fn non_modal_tab_is_flat_global_traversal() {
    // With NO modal open, Tab cycles the flat-global Focusable set exactly as
    // before the scope generalization (the regression guard for the focus.rs edit).
    let mut h = PointerHarness::new();
    let a = spawn_bg_button(h.world_mut(), "A");
    let b = spawn_bg_button(h.world_mut(), "B");
    let c = spawn_bg_button(h.world_mut(), "C");
    let root = h
        .world_mut()
        .spawn((Node, Style::default().width_px(800.0).height_px(600.0)))
        .id();
    h.world_mut().entity_mut(root).add_children(&[a, b, c]);
    for _ in 0..3 {
        h.update();
    }

    // No FocusScope/modal exists ⇒ all three are reachable by Tab.
    let mut seen = std::collections::HashSet::new();
    for _ in 0..6 {
        h.press_key(KeyCode::Tab);
        if let Some(f) = focused(h.world()) {
            seen.insert(f);
        }
    }
    assert!(
        seen.contains(&a) && seen.contains(&b) && seen.contains(&c),
        "non-modal Tab reaches every flat-global focusable (a={a:?}, b={b:?}, \
         c={c:?}, seen={seen:?})"
    );
}

#[test]
fn a_closed_modal_does_not_trap_focus() {
    // A degenerate guard: a Dialog that exists but is CLOSED must NOT trap focus —
    // a closed top-layer modal is in the activation deque but is_open is false.
    let mut h = PointerHarness::new();
    let (_invoker, dialog, bg, _) = spawn_fixture(h.world_mut());
    for _ in 0..4 {
        h.update();
    }
    assert!(!is_open(h.world(), dialog), "the dialog is closed");

    let mut reached_bg = false;
    for _ in 0..6 {
        h.press_key(KeyCode::Tab);
        if focused(h.world()) == Some(bg) {
            reached_bg = true;
            break;
        }
    }
    assert!(
        reached_bg,
        "a CLOSED modal does not trap focus — the background is still reachable"
    );

    // Sanity: FocusVisible is set by Tab (the keyboard focus signal).
    assert!(
        h.world().resource::<FocusVisible>().0,
        "Tab set FocusVisible (keyboard focus)"
    );
}
