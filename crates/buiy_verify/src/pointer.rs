//! Tier A — the headless synthetic-Pointer harness (C7 §2.1). Spawns a
//! REAL non-origin widget tree, lets the production layout -> bridge ->
//! transform-propagation chain produce `GlobalTransform`, then injects a
//! synthetic `PointerId` + `PointerLocation` and reads the resulting
//! `PointerHits` (and, once C3 lands, the durable widget-state flip + a
//! thin observer-capture log). This fixes `picking_backend.rs`'s blind
//! spot: that test hand-writes a single-node `ResolvedLayout` at an
//! absolute position and is structurally incapable of observing Bug 1
//! (C7 §1.1; umbrella §9.5).

use bevy::camera::NormalizedRenderTarget;
use bevy::ecs::message::Messages;
use bevy::picking::backend::PointerHits;
use bevy::picking::pointer::{
    Location, PointerAction, PointerButton, PointerId, PointerInput, PointerLocation,
};
use bevy::prelude::*;
use bevy::window::{PrimaryWindow, Window, WindowRef, WindowResolution};

use buiy_core::Node;
use buiy_core::components::ResolvedLayout;
use buiy_core::layout::Style;
use buiy_core::picking::{BuiyPickingBackendPlugin, PickingPlugin};

/// The thin observer-capture log (§2.1): once C3's `Pointer<E>` observers
/// exist (Task 4, Wave 2) they push `(entity, phase)` here so propagation /
/// bubbling / propagate(false) tests can read which entities saw an event and
/// in what order. This is the ONLY test-only wiring; it observes the
/// production events C3 defines (it does not replace them). Empty in Wave 1.
#[derive(Resource, Default)]
pub struct CapturedEvents(pub Vec<(Entity, &'static str)>);

/// A headless synthetic-pointer driver over the production picking path.
pub struct PointerHarness {
    app: App,
    pointer: Entity,
    window: Entity,
}

impl PointerHarness {
    /// `MinimalPlugins + TransformPlugin + bevy::picking::PickingPlugin +
    /// CorePlugin + LayoutPlugin + the Buiy backend`. NO RenderPlugin, NO
    /// winit, NO AssetPlugin — picking runs as pure ECS so the full hit-test
    /// path is headless-CI-runnable. (C3's InteractionPlugin/FocusPlugin are
    /// added in Task 4 once C3 exists; §3.2 build-step confirms whether they
    /// read direct injection without PointerInputPlugin.)
    pub fn new() -> Self {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::transform::TransformPlugin)
            .add_plugins(bevy::picking::PickingPlugin)
            .add_plugins(buiy_core::CorePlugin)
            .add_plugins(buiy_core::layout::LayoutPlugin)
            .add_plugins(PickingPlugin)
            .add_plugins(BuiyPickingBackendPlugin);
        app.init_resource::<CapturedEvents>();

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

    fn write_button(&mut self, action: PointerAction) {
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

    /// Mutable world access for assertions (Checked/Pressed/Selected/
    /// FocusedEntity once C3/C4 land) and direct scene mutation.
    pub fn world_mut(&mut self) -> &mut World {
        self.app.world_mut()
    }

    /// Read the capture log (propagation tests; populated once C3 observers
    /// exist in Task 4).
    pub fn captured(&self) -> &CapturedEvents {
        self.app.world().resource::<CapturedEvents>()
    }
}

impl Default for PointerHarness {
    fn default() -> Self {
        Self::new()
    }
}
