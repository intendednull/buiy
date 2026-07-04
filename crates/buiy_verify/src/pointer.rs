//! Tier A — the headless synthetic-Pointer harness (C7 §2.1). Spawns a
//! REAL non-origin widget tree, lets the production layout -> bridge ->
//! transform-propagation chain produce `GlobalTransform`, then injects a
//! synthetic `PointerId` + `PointerLocation` and reads the resulting
//! `PointerHits` (and, once C3 lands, the durable widget-state flip + a
//! thin observer-capture log). This fixes `picking_backend.rs`'s blind
//! spot: that test hand-writes a single-node `ResolvedLayout` at an
//! absolute position and is structurally incapable of observing Bug 1
//! (C7 §1.1; umbrella §9.5).

use bevy::camera::{Camera2d, NormalizedRenderTarget, RenderTarget};
use bevy::ecs::message::Messages;
use bevy::input::mouse::MouseScrollUnit;
use bevy::input::touch::TouchPhase;
use bevy::picking::backend::PointerHits;
use bevy::picking::events::{
    Click, Drag, DragEnd, DragStart, Out, Over, Pointer, Press, Release, Scroll,
};
use bevy::picking::pointer::{
    Location, PointerAction, PointerButton, PointerId, PointerInput, PointerLocation,
};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window, WindowRef, WindowResolution};

use buiy_core::Node;
use buiy_core::components::ResolvedLayout;
use buiy_core::layout::Style;
use buiy_core::picking::{BuiyPickingBackendPlugin, MultiClick, PickingPlugin};

/// The thin observer-capture log (§2.1): C3b's recording observers push
/// `(entity, phase)` here so observer-capture / bubbling tests can read which
/// entities saw a `Pointer<E>` (and in what order). The recorder observes the
/// PRODUCTION events C3 defines (`Over`, `Out`, `Press`, `Release`, `Click`,
/// `Scroll`, and the Buiy-native `MultiClick`) — it does not replace them. The
/// `phase` string is the event name (`"over"`, `"click"`, …); `MultiClick`
/// records as `"multiclick"`. The ONLY test-only wiring on the production path.
#[derive(Resource, Default)]
pub struct CapturedEvents(pub Vec<(Entity, &'static str)>);

impl CapturedEvents {
    /// Every captured `(entity, phase)` pair whose phase equals `phase`.
    pub fn of_phase<'a>(&'a self, phase: &'a str) -> impl Iterator<Item = Entity> + 'a {
        self.0
            .iter()
            .filter(move |(_, p)| *p == phase)
            .map(|(e, _)| *e)
    }

    /// Whether any captured event of `phase` targeted `entity`.
    pub fn saw(&self, entity: Entity, phase: &str) -> bool {
        self.of_phase(phase).any(|e| e == entity)
    }
}

/// Detail capture for `Pointer<Scroll>` (the wheel entry, §2.6): the
/// `CapturedEvents` phase log only records the event *name*, but a scroll test
/// must assert the `unit` (the `deltaMode` carriage) and the `(x, y)` deltas, so
/// the scroll observer records the full payload here.
#[derive(Resource, Default)]
pub struct CapturedScroll(pub Vec<(Entity, MouseScrollUnit, f32, f32)>);

/// Detail capture for the drag taxonomy (`Pointer<DragStart>` / `Pointer<Drag>` /
/// `Pointer<DragEnd>`). The [`CapturedEvents`] phase log records only the event
/// *name*, but a drag / stroke test must assert the per-move `delta` and the
/// pointer `position`, so the drag observers record the full payload here.
///
/// The drag taxonomy is the one part of `bevy_picking`'s event surface the harness
/// could not exercise before the stroke driver ([`PointerHarness::stroke`]) landed:
/// every other input method writes `PointerLocation` directly and emits no `Move`
/// action, so the drag machine never fired.
#[derive(Resource, Default)]
pub struct CapturedDrag(pub Vec<DragSample>);

impl CapturedDrag {
    /// Every recorded sample of `phase` (`"dragstart"` / `"drag"` / `"dragend"`)
    /// that targeted `entity`, in emission order.
    pub fn of<'a>(
        &'a self,
        entity: Entity,
        phase: &'a str,
    ) -> impl Iterator<Item = &'a DragSample> {
        self.0
            .iter()
            .filter(move |s| s.entity == entity && s.phase == phase)
    }
}

/// One captured drag-taxonomy event. `delta` carries the `Pointer<Drag>.delta`
/// (the per-move movement) for a `"drag"` sample and the `Pointer<DragEnd>.distance`
/// (the total start→end vector) for a `"dragend"` sample; it is `Vec2::ZERO` for the
/// payload-less `"dragstart"`. `position` is the pointer's window-space location at
/// the event.
#[derive(Clone, Copy, Debug)]
pub struct DragSample {
    pub entity: Entity,
    /// `"dragstart"` | `"drag"` | `"dragend"`.
    pub phase: &'static str,
    pub position: Vec2,
    pub delta: Vec2,
}

/// A headless synthetic-pointer driver over the production picking path.
pub struct PointerHarness {
    app: App,
    pointer: Entity,
    window: Entity,
    /// The lazily-spawned `PointerId::Touch(0)` pointer (the touch-tap tests). The
    /// harness has no `PointerInputPlugin`, so no `touch_pick_events` spawns it —
    /// [`Self::touch_tap`] spawns it on first use.
    touch: Option<Entity>,
}

impl PointerHarness {
    /// `MinimalPlugins + TransformPlugin + bevy::picking::PickingPlugin +
    /// CorePlugin + LayoutPlugin + the Buiy backend`. NO RenderPlugin, NO
    /// winit, NO AssetPlugin — picking runs as pure ECS so the full hit-test
    /// path is headless-CI-runnable.
    ///
    /// C3b: Buiy's `PickingPlugin` now also adds bevy_picking's
    /// `InteractionPlugin` (the hover stage that emits the `Pointer<E>`
    /// taxonomy), and the harness adds a `Camera2d` targeting the synthetic
    /// window so `emit_picks` resolves a REAL camera (its no-camera filter would
    /// otherwise drop every hit). The harness injects `PointerInput` directly
    /// (the lessons-sanctioned synthetic path), so it does NOT add
    /// `PointerInputPlugin` (the winit reader) — that would spawn a duplicate
    /// `PointerId::Mouse`. Recording observers (`record_observers`) push every
    /// `Pointer<E>` into [`CapturedEvents`] so observer-capture is assertable.
    pub fn new() -> Self {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::transform::TransformPlugin)
            .add_plugins(bevy::picking::PickingPlugin)
            .add_plugins(buiy_core::CorePlugin)
            .add_plugins(buiy_core::layout::LayoutPlugin)
            .add_plugins(PickingPlugin)
            .add_plugins(BuiyPickingBackendPlugin)
            // C3d: focus is part of the production interaction stack the harness
            // models — `FocusPlugin` registers the shared
            // `focus::focus_on_click` observer (focus-on-click + the
            // `:focus-visible` decay) and the `FocusedEntity`/`FocusVisible`
            // resources the C3 focus tests assert on. `handle_tab` reads
            // `Res<ButtonInput<KeyCode>>` (an `InputPlugin` resource, absent
            // under `MinimalPlugins`), so seed it below before any update.
            .add_plugins(buiy_core::focus::FocusPlugin)
            // C5-a: the scroll input pipeline — the `Pointer<Scroll>` →
            // `ScrollOffset` observer, keyboard scroll, the `ScrollExtent` cache.
            // Part of the production interaction stack the harness models, so a
            // synthetic wheel/keyboard scroll drives the real clamp path.
            .add_plugins(buiy_core::scroll::ScrollInputPlugin)
            // C5-b: the widget plugin carries the overlay positioning +
            // light-dismiss interaction the harness now models — the
            // `light_dismiss_on_press` observer (a primary `Pointer<Press>`
            // outside the top-most open overlay closes it), the `escape_dismiss`
            // keyboard handler, and `position_popover`/`position_tooltip`. Adding
            // it here lets a synthetic press / Escape drive the REAL dismiss path
            // headless. Its other widget systems are inert with no matching
            // entities (each is query-gated).
            .add_plugins(buiy_widgets::WidgetsPlugin);
        app.init_resource::<ButtonInput<bevy::input::keyboard::KeyCode>>();
        app.init_resource::<CapturedEvents>();
        app.init_resource::<CapturedScroll>();
        app.init_resource::<CapturedDrag>();
        Self::record_observers(&mut app);

        // Pause the virtual clock so `Time` (which the `MultiClick` deriver reads
        // via the `ClickTracker`) only advances when the test explicitly calls
        // `advance_time` — making the slow-vs-fast double-click classification
        // (the 450ms multi-click window) deterministic with no wall-clock sleep.
        // Without pausing, `update_virtual_time` would advance `Time` from the
        // real clock each frame, and a manual advance would be clobbered.
        app.world_mut()
            .resource_mut::<bevy::time::Time<bevy::time::Virtual>>()
            .pause();

        // A synthetic primary window — the layout solver reads its viewport
        // from a plain Query<&Window, With<PrimaryWindow>> (no WindowPlugin).
        let window = app
            .world_mut()
            .spawn((
                Window {
                    resolution: WindowResolution::new(800, 600),
                    ..Default::default()
                },
                PrimaryWindow,
            ))
            .id();

        // A Camera2d targeting the synthetic primary window. `emit_picks`
        // resolves the pointer's target window → this camera (§3.1); without a
        // matching camera the backend emits no hits (the no-Buiy-window filter).
        app.world_mut()
            .spawn((Camera2d, RenderTarget::Window(WindowRef::Entity(window))));

        // The synthetic pointer entity (spawned once). PointerLocation is
        // (re)written by `move_to`. `PointerId::Mouse` passes through the
        // normal backend pipeline (picking_backend.rs).
        let target = WindowRef::Entity(window)
            .normalize(Some(window))
            .expect("normalize window target");
        let pointer = app
            .world_mut()
            .spawn((
                PointerId::Mouse,
                PointerLocation::new(Location {
                    target: NormalizedRenderTarget::Window(target),
                    position: Vec2::ZERO,
                }),
            ))
            .id();

        Self {
            app,
            pointer,
            window,
            touch: None,
        }
    }

    /// Spawn `scene` as the single child of a root translated by the EXPLICIT
    /// `offset`, returning the `scene` entity (the entity under test). The
    /// root's `Translate` is folded by the production bridge into the child's
    /// `GlobalTransform` while the child's `ResolvedLayout.position` stays
    /// PARENT-LOCAL (Taffy `location`, write_resolved_layout) — so
    /// `GlobalTransform.translation` (absolute) diverges from
    /// `ResolvedLayout.position` (local) by exactly `offset`. That divergence
    /// is what the offset RED proof exercises. Drives a bounded settle so
    /// layout + bridge + the three propagation systems produce a steady
    /// `GlobalTransform` on the returned entity.
    pub fn spawn_offset_tree(&mut self, offset: Vec2, scene: impl Bundle) -> Entity {
        let target = self.app.world_mut().spawn(scene).id();
        let _root = self
            .app
            .world_mut()
            .spawn((
                Node,
                // Translate (not padding) so the child's parent-local position
                // stays small while its accumulated global = offset + local.
                Style::default()
                    .flex_column()
                    .width_px(800.0)
                    .height_px(600.0)
                    .translate_px(offset.x, offset.y),
            ))
            .add_child(target)
            .id();
        // Bounded settle: the bridge + the three propagation systems run in
        // Update before Picking; a few frames produce GlobalTransform.
        for _ in 0..4 {
            self.app.update();
        }
        target
    }

    /// The absolute (window-logical) center of `entity`, from the
    /// `GlobalTransform` the production chain produced + its `ResolvedLayout`
    /// size.
    pub fn global_center(&self, entity: Entity) -> Vec2 {
        let world = self.app.world();
        let gt = world
            .get::<GlobalTransform>(entity)
            .expect("entity has GlobalTransform (went through the bridge)");
        let size = world
            .get::<ResolvedLayout>(entity)
            .expect("entity has ResolvedLayout")
            .size;
        gt.translation().truncate() + size * 0.5
    }

    /// Move the synthetic pointer to a WINDOW-space position and run one
    /// update so the backend re-emits `PointerHits` for the new location.
    pub fn move_to(&mut self, pos: Vec2) {
        let target = WindowRef::Entity(self.window)
            .normalize(Some(self.window))
            .expect("normalize window target");
        {
            let mut loc = self
                .app
                .world_mut()
                .get_mut::<PointerLocation>(self.pointer)
                .expect("synthetic pointer has PointerLocation");
            *loc = PointerLocation::new(Location {
                target: NormalizedRenderTarget::Window(target),
                position: pos,
            });
        }
        self.app.update();
    }

    /// The top-most entity the backend reports under the pointer this frame
    /// (index 0 of `picks`, the closest). `None` if no Buiy node is hit.
    pub fn top_hit(&mut self) -> Option<Entity> {
        let messages = self.app.world().resource::<Messages<PointerHits>>();
        let mut cursor = messages.get_cursor();
        let mut latest: Option<Entity> = None;
        for hits in cursor.read(messages) {
            latest = hits.picks.first().map(|(e, _)| *e);
        }
        latest
    }

    /// Write a `PointerInput` Press of `button` at the current pointer
    /// location and run an update. Direct injection (§3.2 — the
    /// lessons-sanctioned synthetic path; NOT PointerInputPlugin, whose job is
    /// to translate winit events we are replacing). RE-CONFIRM AT WAVE 2: if
    /// C3's InteractionPlugin depends on PointerInputPlugin running first, add
    /// PointerInputPlugin in `new()` and feed it PointerInput rather than
    /// hand-maintaining state (§3.2 caveat).
    pub fn press(&mut self, button: PointerButton) {
        self.write_button(PointerAction::Press(button));
    }

    /// Write a `PointerInput` Release of `button` at the current pointer
    /// location and run an update.
    pub fn release(&mut self, button: PointerButton) {
        self.write_button(PointerAction::Release(button));
    }

    /// A press immediately followed by a release of `button` (a full click).
    pub fn click(&mut self, button: PointerButton) {
        self.press(button);
        self.release(button);
    }

    /// Inject a `PointerId::Touch(0)` tap: a primary Press at `press_pos` then a
    /// Release at `release_pos`, each in its own frame, with NO prior hover/move
    /// (the COLD-tap case). The touch pointer is spawned lazily with a stale
    /// off-screen location, so the touch-input fix must set its location from the
    /// press input ([`sync_pointer_location_on_button`]) and activate on the
    /// release via the CURRENT hover map ([`touch_tap_activates`]) — bevy_picking's
    /// `Pointer<Click>`/`Pointer<Release>` can't (both read the previous frame's
    /// empty hover map for a first-touch pointer). `press_pos == release_pos` is a
    /// normal tap; a different `release_pos` models a finger dragged off the target.
    ///
    /// [`sync_pointer_location_on_button`]: buiy_core::picking::backend
    /// [`touch_tap_activates`]: buiy_core::picking::activation::touch_tap_activates
    pub fn touch_tap(&mut self, press_pos: Vec2, release_pos: Vec2) {
        let target = WindowRef::Entity(self.window)
            .normalize(Some(self.window))
            .expect("normalize window target");
        let loc = |p: Vec2| Location {
            target: NormalizedRenderTarget::Window(target),
            position: p,
        };
        let touch_id = PointerId::Touch(0);
        if self.touch.is_none() {
            let e = self
                .app
                .world_mut()
                .spawn((touch_id, PointerLocation::new(loc(Vec2::splat(-1.0e6)))))
                .id();
            self.touch = Some(e);
        }
        for (pos, action) in [
            (press_pos, PointerAction::Press(PointerButton::Primary)),
            (release_pos, PointerAction::Release(PointerButton::Primary)),
        ] {
            self.app.world_mut().write_message(PointerInput {
                pointer_id: touch_id,
                location: loc(pos),
                action,
            });
            self.app.update();
        }
    }

    /// A cold touch tap at a single point (press + release at `pos`).
    pub fn touch_tap_cold(&mut self, pos: Vec2) {
        self.touch_tap(pos, pos);
    }

    /// A full click immediately repeated — a double-click — at the current
    /// pointer location. Within the `ClickTracker` window+radius (the two clicks
    /// are emitted in adjacent frames at the same position), so the second one
    /// derives a `MultiClick { count: 2 }`. For a "slow" second click outside
    /// the window, advance the clock between the two `click` calls (the
    /// `ClickTracker` reads `Res<Time>`).
    pub fn double_click(&mut self, button: PointerButton) {
        self.click(button);
        self.click(button);
    }

    /// Inject a wheel `PointerInput::Scroll` at the current pointer location and
    /// run an update so the hover stage emits `Pointer<Scroll>` over the hovered
    /// entity (§2.6 wheel entry). `unit` distinguishes line vs pixel deltas
    /// (the `deltaMode` carriage); `(x, y)` are the scroll amounts.
    pub fn scroll(&mut self, unit: MouseScrollUnit, x: f32, y: f32) {
        self.write_action(PointerAction::Scroll {
            unit,
            x,
            y,
            phase: TouchPhase::Moved,
        });
    }

    /// Drive a REAL press → drag → release **stroke** through the production
    /// pointer pipeline, one frame per input, so `bevy_picking`'s `pointer_events`
    /// derives `DragStart` → `Drag` → `DragEnd` on the press target.
    ///
    /// This is the counterpart to [`move_to`](Self::move_to). `move_to` writes
    /// `PointerLocation` directly and emits **no** `Move` action, so it never trips
    /// the drag machine — a naive "sequence of `move_to`s" between a press and a
    /// release silently produces zero drags. `stroke` instead presses primary at
    /// `path[0]`, writes a `PointerAction::Move` to each subsequent point (updating
    /// `PointerLocation` coherently via `PointerInput::receive`), and releases at the
    /// last point. The emitted events are recorded in
    /// [`captured_drag`](Self::captured_drag). A `path` shorter than two points is a
    /// no-op (nothing to drag).
    ///
    /// This is the harness's first driver of `bevy_picking`'s drag machine — every
    /// other method exercises hover / press / click / scroll, none of which emit a
    /// `Move` — so it is what a headless test (or a multi-agent playtest) uses to
    /// drive a freehand drawing surface through the real pointer path.
    pub fn stroke(&mut self, path: &[Vec2]) {
        drive_stroke(&mut self.app, self.window, self.pointer, path);
    }

    /// A straight [`stroke`](Self::stroke) from `from` to `to` sampled at `steps`
    /// equal intervals (so `steps` `Move`s fire, hence `steps` `Pointer<Drag>` events
    /// on the press target). `steps` is clamped to at least 1.
    pub fn drag(&mut self, from: Vec2, to: Vec2, steps: usize) {
        let steps = steps.max(1);
        let path: Vec<Vec2> = (0..=steps)
            .map(|i| from.lerp(to, i as f32 / steps as f32))
            .collect();
        self.stroke(&path);
    }

    /// Read the drag-taxonomy capture log (the `Pointer<DragStart>` / `Drag` /
    /// `DragEnd` payloads a [`stroke`](Self::stroke) produced).
    pub fn captured_drag(&self) -> &CapturedDrag {
        self.app.world().resource::<CapturedDrag>()
    }

    fn write_button(&mut self, action: PointerAction) {
        self.write_action(action);
    }

    fn write_action(&mut self, action: PointerAction) {
        let location = self
            .app
            .world()
            .get::<PointerLocation>(self.pointer)
            .expect("synthetic pointer has PointerLocation")
            .location()
            .expect("pointer has a location")
            .clone();
        self.app.world_mut().write_message(PointerInput {
            pointer_id: PointerId::Mouse,
            location,
            action,
        });
        self.app.update();
    }

    /// Register the recording observers that push every production `Pointer<E>`
    /// (and the Buiy-native `MultiClick`) into [`CapturedEvents`]. The observers
    /// read `On<Pointer<E>>` / `On<MultiClick>` and the `CapturedEvents` resource
    /// — the only test-only wiring on the production pipeline; it observes the
    /// real events, it does not synthesize them.
    fn record_observers(app: &mut App) {
        fn rec<E>(phase: &'static str) -> impl Fn(On<Pointer<E>>, ResMut<CapturedEvents>)
        where
            E: core::fmt::Debug + Clone + Reflect,
        {
            move |ev: On<Pointer<E>>, mut log: ResMut<CapturedEvents>| {
                log.0.push((ev.entity, phase));
            }
        }
        app.add_observer(rec::<Over>("over"));
        app.add_observer(rec::<Out>("out"));
        app.add_observer(rec::<Press>("press"));
        app.add_observer(rec::<Release>("release"));
        app.add_observer(rec::<Click>("click"));
        // Scroll records both the phase (for `saw`) and the full payload (for
        // the `unit`/`y` assertions) — the wheel-entry test reads both.
        app.add_observer(
            |ev: On<Pointer<Scroll>>,
             mut log: ResMut<CapturedEvents>,
             mut scroll: ResMut<CapturedScroll>| {
                log.0.push((ev.entity, "scroll"));
                scroll
                    .0
                    .push((ev.entity, ev.event.unit, ev.event.x, ev.event.y));
            },
        );
        app.add_observer(|ev: On<MultiClick>, mut log: ResMut<CapturedEvents>| {
            log.0.push((ev.entity, "multiclick"));
        });
        // The drag taxonomy: the phase log records the name (for `saw`), and
        // `CapturedDrag` records the payload (`delta` / `distance` + position) the
        // drag / stroke tests assert on. `DragStart` carries no delta.
        app.add_observer(
            |ev: On<Pointer<DragStart>>,
             mut log: ResMut<CapturedEvents>,
             mut drag: ResMut<CapturedDrag>| {
                log.0.push((ev.entity, "dragstart"));
                drag.0.push(DragSample {
                    entity: ev.entity,
                    phase: "dragstart",
                    position: ev.pointer_location.position,
                    delta: Vec2::ZERO,
                });
            },
        );
        app.add_observer(
            |ev: On<Pointer<Drag>>,
             mut log: ResMut<CapturedEvents>,
             mut drag: ResMut<CapturedDrag>| {
                log.0.push((ev.entity, "drag"));
                drag.0.push(DragSample {
                    entity: ev.entity,
                    phase: "drag",
                    position: ev.pointer_location.position,
                    delta: ev.event.delta,
                });
            },
        );
        app.add_observer(
            |ev: On<Pointer<DragEnd>>,
             mut log: ResMut<CapturedEvents>,
             mut drag: ResMut<CapturedDrag>| {
                log.0.push((ev.entity, "dragend"));
                drag.0.push(DragSample {
                    entity: ev.entity,
                    phase: "dragend",
                    position: ev.pointer_location.position,
                    delta: ev.event.distance,
                });
            },
        );
    }

    /// Mutable world access for assertions (Checked/Pressed/Selected/
    /// FocusedEntity once C3/C4 land) and direct scene mutation.
    pub fn world_mut(&mut self) -> &mut World {
        self.app.world_mut()
    }

    /// Read-only world access for assertions (component reads on the
    /// post-settle world without taking `&mut`).
    pub fn world(&self) -> &World {
        self.app.world()
    }

    /// The currently focused entity (`FocusedEntity.0`) — the focus-on-click
    /// (C3d) flip a press over a `Focusable` produces.
    pub fn focused(&self) -> Option<Entity> {
        self.app
            .world()
            .resource::<buiy_core::focus::FocusedEntity>()
            .0
    }

    /// The `:focus-visible` decay signal (`FocusVisible.0`, C3d / § 2.7):
    /// `true` after keyboard (Tab) focus, `false` after pointer focus.
    pub fn focus_visible(&self) -> bool {
        self.app
            .world()
            .resource::<buiy_core::focus::FocusVisible>()
            .0
    }

    /// Press `key`, run one update so `FocusPlugin::handle_tab` (in
    /// `BuiySet::Input`) sees it, then release+clear it. Drives the keyboard
    /// focus path (Tab) the C3d decay test needs without an `InputPlugin`
    /// PreUpdate clear wiping the press first.
    pub fn press_key(&mut self, key: bevy::input::keyboard::KeyCode) {
        {
            let mut keys = self
                .app
                .world_mut()
                .resource_mut::<ButtonInput<bevy::input::keyboard::KeyCode>>();
            keys.release_all();
            keys.clear();
            keys.press(key);
        }
        self.app.update();
        let mut keys = self
            .app
            .world_mut()
            .resource_mut::<ButtonInput<bevy::input::keyboard::KeyCode>>();
        keys.release_all();
        keys.clear();
    }

    /// Read the capture log (observer-capture / bubbling tests).
    pub fn captured(&self) -> &CapturedEvents {
        self.app.world().resource::<CapturedEvents>()
    }

    /// Run one full `App` update (layout → bridge → propagation → picking).
    /// For tests that mutate the scene via [`world_mut`](Self::world_mut) (e.g.
    /// add a child) and need the production chain to settle a new
    /// `GlobalTransform` before pressing.
    pub fn update(&mut self) {
        self.app.update();
    }

    /// The `(unit, y)` of the most recent `Pointer<Scroll>` recorded for
    /// `entity`, if any — the wheel-entry detail (§2.6).
    pub fn last_scroll(&self, entity: Entity) -> Option<(MouseScrollUnit, f32)> {
        self.app
            .world()
            .resource::<CapturedScroll>()
            .0
            .iter()
            .rev()
            .find(|(e, ..)| *e == entity)
            .map(|(_, unit, _, y)| (*unit, *y))
    }

    /// Advance the (paused) virtual clock by `delta`, then run one update so the
    /// generic `Time` the `MultiClick` deriver reads reflects it. With the
    /// virtual clock paused in `new()`, this is the ONLY way the clock moves, so
    /// a test exercises the slow-vs-fast double-click classification (the 450ms
    /// multi-click window) deterministically, with no real wall-clock sleeping.
    pub fn advance_time(&mut self, delta: std::time::Duration) {
        self.app
            .world_mut()
            .resource_mut::<bevy::time::Time<bevy::time::Virtual>>()
            .advance_by(delta);
        // Propagate the virtual advance into the generic `Time`
        // (`update_virtual_time` copies `virt.as_generic()` each update).
        self.app.update();
    }
}

impl Default for PointerHarness {
    fn default() -> Self {
        Self::new()
    }
}

/// Drive a REAL press → drag → release **stroke** over `path` through the
/// production pointer pipeline on ANY `app`, targeting window `window` with the
/// pointer entity `pointer` (which must carry a `PointerId` + `PointerLocation`).
/// Presses primary at `path[0]`, writes a `PointerAction::Move` to each subsequent
/// point — updating `PointerLocation` coherently via `PointerInput::receive` so
/// `pointer_events` derives `DragStart` → `Drag` → `DragEnd` on the PRESS target —
/// then releases primary at the last point, ticking one frame per input. A `path`
/// shorter than two points is a no-op.
///
/// This is the reusable core [`PointerHarness::stroke`] delegates to; it is also how
/// a *foreign* app — one that stands up its own window/camera/pointer rather than
/// borrowing a [`PointerHarness`] (e.g. a drawing-canvas end-to-end test, or a
/// long-running playtest host that drives agent strokes) — exercises its own canvas
/// through the same real drag pipeline.
///
/// **Precondition:** `app` must run the picking stack that turns a `PointerInput`
/// into the `Pointer<E>` taxonomy — `bevy::picking::PickingPlugin` (the
/// `PointerInput::receive` + hit-test scheduling), Buiy's [`PickingPlugin`] (the
/// `InteractionPlugin` hover stage) + [`BuiyPickingBackendPlugin`], plus a
/// `Camera2d` targeting `window` so `emit_picks` resolves a camera.
/// [`PointerHarness::new`] composes exactly this; a probe-preset app adds those four
/// on top (the "unified headless driver" recipe — the probe preset omits picking, so
/// re-adding it conflicts with nothing). A GPU-free host that presents a texture also
/// needs `app.init_asset::<Image>()` (no `ImagePlugin`/`RenderPlugin` registers
/// `Assets<Image>` under a headless preset).
pub fn drive_stroke(app: &mut App, window: Entity, pointer: Entity, path: &[Vec2]) {
    if path.len() < 2 {
        return;
    }
    let pointer_id = *app
        .world()
        .get::<PointerId>(pointer)
        .expect("pointer entity carries a PointerId");
    let normalized = WindowRef::Entity(window)
        .normalize(Some(window))
        .expect("normalize window target");
    let render_target = NormalizedRenderTarget::Window(normalized);
    let at = |p: Vec2| Location {
        target: render_target.clone(),
        position: p,
    };

    // Settle a hover at the start with a DIRECT location write (no `Move` action,
    // exactly `move_to`'s discipline) so the backend hit-tests `path[0]` and the
    // press captures the node under it as the drag's target.
    {
        let mut loc = app
            .world_mut()
            .get_mut::<PointerLocation>(pointer)
            .expect("pointer entity has PointerLocation");
        *loc = PointerLocation::new(at(path[0]));
    }
    app.update();

    // Press primary at the start.
    write_pointer_input(
        app,
        pointer_id,
        at(path[0]),
        PointerAction::Press(PointerButton::Primary),
    );

    // Move through the rest: each `Move` updates `PointerLocation` (via
    // `PointerInput::receive`) and derives `DragStart` / `Drag` on the press target.
    // A zero-delta step is skipped by `pointer_events`, so only distinct points
    // produce a `Drag`.
    let mut prev = path[0];
    for &p in &path[1..] {
        write_pointer_input(
            app,
            pointer_id,
            at(p),
            PointerAction::Move { delta: p - prev },
        );
        prev = p;
    }

    // Release primary at the final point → `DragEnd`.
    write_pointer_input(
        app,
        pointer_id,
        at(prev),
        PointerAction::Release(PointerButton::Primary),
    );
}

/// Write one `PointerInput` and run a frame (the shared per-input step of
/// [`drive_stroke`]). A free fn (not a closure) so the `&mut App` reborrows at the
/// call site and stays usable across the many writes.
fn write_pointer_input(
    app: &mut App,
    pointer_id: PointerId,
    location: Location,
    action: PointerAction,
) {
    app.world_mut().write_message(PointerInput {
        pointer_id,
        location,
        action,
    });
    app.update();
}
