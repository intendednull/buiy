//! **THE KILLER USE CASE**: whole-UI record/replay of *widget-internal* state,
//! byte-identically, from REAL synthetic input (spec §7).
//!
//! Proves the ONE claim the "MVU-as-core" bet rests on: a complete, whole-UI record
//! that includes widget-internal state — toggle values AND the editor buffer + caret +
//! selection — replayable byte-identically into a FRESH app from the same seed. An
//! app-boundary-only log cannot produce this (it never sees the checkbox fold or the
//! editor command stream).
//!
//! ## What these tests drive
//! A controlled scene (two checkboxes + a switch + a seeded multi-line editor), each
//! carrying a stable [`LogicalId`], composed under the REAL [`BuiyPlugin`] (layout +
//! picking + the MVU chain + the editor). Recording is the W4 unified switch
//! ([`RecordSession`]) — ONE global sequence over BOTH the widget-fold log
//! ([`MsgLog`]) and the editor-command log ([`EditLog`]).
//!
//! - `whole_ui_session_replays_byte_identically` — drive a multi-step session of REAL
//!   synthetic input (pointer clicks → `OnPress` → toggle folds; raw `KeyboardInput`/`Ime`
//!   → editor edits), interleaved, with recording ON. Then replay the unified log into a
//!   FRESH scene app from the same seed and assert the WHOLE UI is byte-identical: every
//!   toggle state AND the editor value + caret + selection. This is the capability the
//!   app-boundary log can't deliver.
//! - `structural_ops_are_off_log_replay_does_not_recreate_a_spawn` — the HONEST gap
//!   (SYNTHESIS H8(i)): a spawn done OUTSIDE the funnel is not on the log, so replay does
//!   NOT recreate it, even though every on-log fold replays. Characterizes the scoped
//!   guarantee (H6) — complete over the MVU-governed subtree, not over structure.

use bevy::MinimalPlugins;
use bevy::app::App;
use bevy::asset::AssetPlugin;
use bevy::camera::{Camera2d, NormalizedRenderTarget, RenderTarget};
use bevy::input::ButtonInput;
use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput};
use bevy::math::Vec2;
use bevy::picking::pointer::{Location, PointerAction, PointerButton, PointerId, PointerInput};
use bevy::prelude::Entity;
use bevy::scene::ScenePlugin;
use bevy::transform::components::GlobalTransform;
use bevy::window::{Ime, PrimaryWindow, Window, WindowRef, WindowResolution};

use buiy::BuiyPlugin;
use buiy_core::ResolvedLayout;
use buiy_core::a11y::{A11yToggled, Toggled};
use buiy_core::focus::FocusedEntity;
use buiy_core::mvu::{LogicalId, MsgLog, RecordSession};
use buiy_core::replay::{replay_into, unified_stream};
use buiy_core::text::SharedFontSystem;
use buiy_core::text::edit::{Clipboard, EditCommand, EditLog, MemClipboard, TextEditState};
use buiy_widgets::{Checkbox, Switch, TextInput};

// --- Stable logical ids the scene assigns (the same in the record + replay apps). -----
const LID_CB_A: u64 = 101;
const LID_CB_B: u64 = 102;
const LID_SWITCH: u64 = 103;
const LID_EDITOR: u64 = 110;
const EDITOR_SEED: &str = "Hi";

/// A controlled-scene harness over the REAL Buiy stack (layout + picking + MVU + editor),
/// headless. Mirrors the gallery live-interaction tier (`interaction.rs`) but spawns a
/// fixed, byte-comparable scene instead of the full gallery.
struct Scene {
    app: App,
    window: Entity,
    cb_a: Entity,
    cb_b: Entity,
    switch: Entity,
    editor: Entity,
}

impl Scene {
    /// Build the scene + the picking stack, settle layout, and seed the editor buffer
    /// (the "Elm-flags" initial condition — NOT part of the recorded stream). Recording
    /// is OFF here; the caller turns it on for the session.
    fn new() -> Self {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(AssetPlugin::default())
            .add_plugins(ScenePlugin)
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(BuiyPlugin);

        // Deterministic, headless clipboard (BuiyTextPlugin defaults to arboard, which is
        // unavailable/non-deterministic in CI). The IME message is not registered without
        // winit, so add it so the IME tap can run.
        app.insert_resource(Clipboard(Box::new(MemClipboard::default())));
        app.add_message::<Ime>();

        // A synthetic primary window + a Camera2d targeting it (the headless picking
        // pattern — `emit_picks` resolves the camera's render target against the window).
        let window = app
            .world_mut()
            .spawn((
                Window {
                    resolution: WindowResolution::new(1280, 800),
                    ..Default::default()
                },
                PrimaryWindow,
            ))
            .id();
        app.world_mut()
            .spawn((Camera2d, RenderTarget::Window(WindowRef::Entity(window))));
        // The mouse pointer (its `#[require]` adds the location/press companions).
        app.world_mut().spawn(PointerId::Mouse);

        // The scene: a vertical stack of [checkbox, checkbox, switch, editor], each with a
        // stable LogicalId. The bare widget markers materialize their `#[require]` layout
        // contract, so they lay out at distinct, hit-testable boxes within the column.
        let root = app
            .world_mut()
            .spawn((
                buiy_core::Node,
                buiy_core::layout::Style::default()
                    .flex_column()
                    .gap_px(24.0)
                    .padding(24.0)
                    .width_px(420.0)
                    .height_px(600.0),
            ))
            .id();
        let cb_a = app.world_mut().spawn((Checkbox, LogicalId(LID_CB_A))).id();
        let cb_b = app.world_mut().spawn((Checkbox, LogicalId(LID_CB_B))).id();
        let switch = app.world_mut().spawn((Switch, LogicalId(LID_SWITCH))).id();
        let editor = app
            .world_mut()
            .spawn((TextInput::multi_line(""), LogicalId(LID_EDITOR)))
            .id();
        app.world_mut()
            .entity_mut(root)
            .add_children(&[cb_a, cb_b, switch, editor]);

        let mut scene = Self {
            app,
            window,
            cb_a,
            cb_b,
            switch,
            editor,
        };
        scene.settle(12);
        scene.seed_editor(EDITOR_SEED);
        scene.settle(2);
        scene
    }

    fn settle(&mut self, n: usize) {
        for _ in 0..n {
            self.app.update();
        }
    }

    /// Seed the editor buffer directly (the initial condition both apps share). Uses the
    /// same `EditCommand::Insert` seam the W3 crux test seeds with; recording is OFF.
    fn seed_editor(&mut self, text: &str) {
        let fonts = self.app.world().resource::<SharedFontSystem>().clone();
        let mut fs = fonts.lock();
        if let Some(mut state) = self.app.world_mut().get_mut::<TextEditState>(self.editor) {
            state.apply(&mut fs, EditCommand::Insert(text.into()), false, false);
        }
    }

    // --- picking (real synthetic pointer clicks → OnPress → toggle fold) ---------------

    fn loc(&self, pos: Vec2) -> Location {
        let target = WindowRef::Entity(self.window)
            .normalize(Some(self.window))
            .expect("normalize window ref");
        Location {
            target: NormalizedRenderTarget::Window(target),
            position: pos,
        }
    }

    fn center(&self, entity: Entity) -> Vec2 {
        let size = self
            .app
            .world()
            .get::<ResolvedLayout>(entity)
            .unwrap_or_else(|| panic!("entity {entity:?} not laid out"))
            .size;
        let gt = *self
            .app
            .world()
            .get::<GlobalTransform>(entity)
            .unwrap_or_else(|| panic!("entity {entity:?} has no GlobalTransform"));
        gt.translation().truncate() + size / 2.0
    }

    fn pointer_input(&mut self, pos: Vec2, action: PointerAction) {
        let location = self.loc(pos);
        self.app.world_mut().write_message(PointerInput {
            pointer_id: PointerId::Mouse,
            location,
            action,
        });
    }

    /// A full primary click at an entity's laid-out center, through the live backend
    /// (move → press → release → `Pointer<Click>` → the `OnPress` producer).
    fn click(&mut self, entity: Entity) {
        let pos = self.center(entity);
        self.pointer_input(pos, PointerAction::Move { delta: Vec2::ZERO });
        self.app.update();
        self.pointer_input(pos, PointerAction::Press(PointerButton::Primary));
        self.app.update();
        self.pointer_input(pos, PointerAction::Release(PointerButton::Primary));
        self.app.update();
        self.app.update(); // settle the OnPress → enqueue → drain in the same frame chain
    }

    // --- keyboard / IME (raw input → editor edits) -------------------------------------

    fn focus_editor(&mut self) {
        self.app.world_mut().resource_mut::<FocusedEntity>().0 = Some(self.editor);
        self.app.update();
    }

    fn keys_press(&mut self, key: KeyCode) {
        self.app
            .world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(key);
    }

    fn keys_release(&mut self, key: KeyCode) {
        self.app
            .world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .release(key);
    }

    fn send_key(&mut self, key_code: KeyCode, logical: Key, text: Option<&str>) {
        self.app.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key: logical,
            state: ButtonState::Pressed,
            text: text.map(Into::into),
            repeat: false,
            window: self.window,
        });
        self.app.update();
    }

    fn type_char(&mut self, ch: &str) {
        self.send_key(KeyCode::KeyA, Key::Character(ch.into()), Some(ch));
    }

    fn arrow_left(&mut self) {
        self.send_key(KeyCode::ArrowLeft, Key::ArrowLeft, None);
    }

    /// Shift-extend one cell left (a held modifier + ArrowLeft → `Motion(Left, true)`).
    fn shift_left(&mut self) {
        self.keys_press(KeyCode::ShiftLeft);
        self.send_key(KeyCode::ArrowLeft, Key::ArrowLeft, None);
        self.keys_release(KeyCode::ShiftLeft);
    }

    fn backspace(&mut self) {
        self.send_key(KeyCode::Backspace, Key::Backspace, None);
    }

    /// Paste the current clipboard via the command modifier (Ctrl/Cmd-V).
    fn paste(&mut self, clip_text: &str) {
        self.app
            .world_mut()
            .resource_mut::<Clipboard>()
            .0
            .set_text(clip_text.to_string());
        let cmd = if cfg!(target_os = "macos") {
            KeyCode::SuperLeft
        } else {
            KeyCode::ControlLeft
        };
        self.keys_press(cmd);
        self.send_key(KeyCode::KeyV, Key::Character("v".into()), Some("v"));
        self.keys_release(cmd);
    }

    fn ime_preedit(&mut self, value: &str, cursor: Option<(usize, usize)>) {
        let window = self.window;
        self.app.world_mut().write_message(Ime::Preedit {
            window,
            value: value.to_string(),
            cursor,
        });
        self.app.update();
    }

    fn ime_commit(&mut self, value: &str) {
        let window = self.window;
        self.app.world_mut().write_message(Ime::Commit {
            window,
            value: value.to_string(),
        });
        self.app.update();
    }

    // --- observation -------------------------------------------------------------------

    fn toggled(&self, e: Entity) -> Toggled {
        self.app.world().get::<A11yToggled>(e).unwrap().0
    }

    fn editor_snap(&self) -> EditorSnap {
        let state = self.app.world().get::<TextEditState>(self.editor).unwrap();
        EditorSnap {
            value: state.value(),
            caret: format!("{:?}", state.caret()),
            selection: format!("{:?}", state.mirror_selection()),
        }
    }

    fn whole_ui(&self) -> WholeUi {
        WholeUi {
            cb_a: self.toggled(self.cb_a),
            cb_b: self.toggled(self.cb_b),
            switch: self.toggled(self.switch),
            editor: self.editor_snap(),
        }
    }
}

/// The editor's logical state projection — the byte-identity surface (NOT the cosmic
/// `Editor`: value + caret + selection, all byte/index-based for horizontal edits).
#[derive(Debug, PartialEq)]
struct EditorSnap {
    value: String,
    caret: String,
    selection: String,
}

/// The WHOLE UI's widget-internal state — what an app-boundary log cannot capture.
#[derive(Debug, PartialEq)]
struct WholeUi {
    cb_a: Toggled,
    cb_b: Toggled,
    switch: Toggled,
    editor: EditorSnap,
}

/// Drive the representative multi-step session (toggles + editor edits, interleaved) on
/// `s`, with recording assumed ON. Factored so the killer test and the gap test share the
/// exact same input.
fn drive_session(s: &mut Scene) {
    // 1. click checkbox A (False → True)
    s.click(s.cb_a);
    // 2. type into the editor (raw keyboard)
    s.focus_editor();
    s.type_char("a");
    s.type_char("b");
    s.type_char("c");
    // 3. click the switch (False → True)
    s.click(s.switch);
    // 4. motion + shift-select + delete the selection
    s.focus_editor();
    s.arrow_left();
    s.shift_left();
    s.shift_left();
    s.backspace();
    // 5. click checkbox A again (True → False) — a fold-BACK, recorded as its own Msg
    s.click(s.cb_a);
    // 6. paste + IME compose/commit
    s.focus_editor();
    s.paste("ZZ");
    s.ime_preedit("ni", Some((0, 2)));
    s.ime_commit("world");
    // 7. click checkbox B (False → True)
    s.click(s.cb_b);
}

// ===========================================================================
// THE KILLER USE CASE — whole-UI replay is byte-identical.
// ===========================================================================

#[test]
fn whole_ui_session_replays_byte_identically() {
    // --- Record a whole-UI session of REAL synthetic input. -------------------
    let mut rec = Scene::new();
    rec.app.world_mut().resource_mut::<RecordSession>().start(); // unified switch ON, seq=0
    drive_session(&mut rec);

    // Precondition: the session produced non-trivial widget-internal state.
    let recorded_ui = rec.whole_ui();
    assert_eq!(
        recorded_ui.cb_a,
        Toggled::False,
        "cb_a ends unchecked (clicked twice: False→True→False)"
    );
    assert_eq!(recorded_ui.cb_b, Toggled::True, "cb_b ends checked");
    assert_eq!(recorded_ui.switch, Toggled::True, "switch ends on");
    assert!(
        recorded_ui.editor.value.starts_with("Hi") && recorded_ui.editor.value.len() > 2,
        "the editor holds real edited content beyond the seed: {:?}",
        recorded_ui.editor.value
    );

    // The unified log: ONE global sequence over BOTH the widget folds AND the editor
    // commands (the W4 unification). Assert it is a single, contiguous, totally-ordered
    // stream containing BOTH kinds — the thing that makes interleaved replay possible.
    {
        let world = rec.app.world();
        let msg_log = world.resource::<MsgLog>();
        let edit_log = world.resource::<EditLog>();
        let stream = unified_stream(msg_log, edit_log);
        let widget_entries = stream.iter().filter(|e| e.is_widget()).count();
        let edit_entries = stream.len() - widget_entries;
        assert_eq!(
            widget_entries, 4,
            "four toggle folds recorded (cb_a ×2, switch, cb_b)"
        );
        assert!(edit_entries >= 6, "the editor command stream was recorded");
        // ONE global monotonic sequence shared by both logs: seqs are exactly 0..n.
        let seqs: Vec<u64> = stream.iter().map(|e| e.seq()).collect();
        let expected: Vec<u64> = (0..stream.len() as u64).collect();
        assert_eq!(
            seqs, expected,
            "the two logs share ONE contiguous global sequence (W4 unification)"
        );
        // And it is genuinely interleaved (a widget fold sits between editor commands),
        // so the merge order is load-bearing, not separable per-log.
        let first_widget = stream.iter().position(|e| e.is_widget()).unwrap();
        let last_widget = stream.iter().rposition(|e| e.is_widget()).unwrap();
        let has_edit_between = stream[first_widget..=last_widget]
            .iter()
            .any(|e| !e.is_widget());
        assert!(
            has_edit_between,
            "widget folds and editor commands are interleaved in the one stream"
        );
    }

    // --- Replay the unified log into a FRESH app from the same seed. ----------
    let mut replay = Scene::new();
    {
        let world = rec.app.world();
        let msg_log = world.resource::<MsgLog>();
        let edit_log = world.resource::<EditLog>();
        replay_into(&mut replay.app, msg_log, edit_log);
    }
    replay.settle(2);

    // --- Assert the WHOLE UI is byte-identical. -------------------------------
    let replay_ui = replay.whole_ui();
    assert_eq!(
        recorded_ui, replay_ui,
        "WHOLE-UI REPLAY IS BYTE-IDENTICAL: every toggle state (cb_a/cb_b/switch) AND the \
         editor value + caret + selection match after replaying the unified log into a \
         fresh app from the same seed — the widget-INTERNAL completeness an app-boundary \
         log cannot deliver"
    );
}

// ===========================================================================
// THE STRUCTURAL-OPS GAP (H8(i)) — honestly characterized, not papered over.
// ===========================================================================

#[test]
fn structural_ops_are_off_log_replay_does_not_recreate_a_spawn() {
    const LID_SPAWNED: u64 = 999;

    // Record a session, then perform a STRUCTURAL op OUTSIDE the funnel: spawn a new
    // checkbox directly (the decomposed analogue of "append a todo row" — a keyed-
    // reconcile spawn that happens in a system, not as a logged Msg fold).
    let mut rec = Scene::new();
    rec.app.world_mut().resource_mut::<RecordSession>().start();
    rec.click(rec.cb_a); // an on-log fold (False → True)
    let spawned = rec
        .app
        .world_mut()
        .spawn((Checkbox, LogicalId(LID_SPAWNED)))
        .id();
    rec.settle(2);

    // Sanity: the spawn happened in the record app, and it left NO trace on the log
    // (structural ops are off-log — there is no Msg entry that creates an entity).
    assert!(
        rec.app.world().get::<A11yToggled>(spawned).is_some(),
        "the structural op added a checkbox to the RECORD app"
    );
    let spawned_lid_entries = {
        let world = rec.app.world();
        let stream = unified_stream(world.resource::<MsgLog>(), world.resource::<EditLog>());
        stream
            .iter()
            .filter(|e| e.lid() == LogicalId(LID_SPAWNED))
            .count()
    };
    assert_eq!(
        spawned_lid_entries, 0,
        "the spawn produced ZERO log entries (it never entered the funnel)"
    );

    // Replay into a fresh app (the initial scene — no spawned checkbox).
    let mut replay = Scene::new();
    {
        let world = rec.app.world();
        replay_into(
            &mut replay.app,
            world.resource::<MsgLog>(),
            world.resource::<EditLog>(),
        );
    }
    replay.settle(2);

    // The ON-LOG fold replayed: cb_a is reproduced byte-identically.
    assert_eq!(
        replay.toggled(replay.cb_a),
        Toggled::True,
        "the on-log toggle fold replayed (cb_a True)"
    );

    // THE GAP (H8(i)): the OFF-LOG spawn did NOT replay — no entity with the spawned
    // LogicalId exists in the replay app. Whole-UI replay is complete over the
    // MVU-governed subtree (the folds), NOT over structure (spawn/despawn). The cure
    // the FINAL must choose: record structural ops on-log, or derive structure as a pure
    // function of on-log parent state (SYNTHESIS H8(i) / open-Q1).
    let replay_has_spawned = {
        let mut q = replay.app.world_mut().query::<&LogicalId>();
        q.iter(replay.app.world())
            .any(|lid| *lid == LogicalId(LID_SPAWNED))
    };
    assert!(
        !replay_has_spawned,
        "STRUCTURAL-OPS GAP: the off-log spawn is NOT recreated by replay (the scoped \
         guarantee — folds replay, structure does not)"
    );
}
