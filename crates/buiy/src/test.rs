//! `buiy::test` — an ergonomic headless harness for **unit-testing an MVU model**.
//!
//! Gated behind the opt-in `test-support` feature, so it is **compiled out of
//! release builds** (a normal `cargo build` / `--release` never sees it; only
//! dev/test builds enable it — the crate enables the feature for its own dev
//! targets via a self `dev-dependency`). It is the shared answer to a pattern
//! every MVU suite otherwise re-derives by hand (`counter_app` in
//! `buiy_core/tests/crosscut/mvu.rs`, `app_with` in `buiy_view/tests/todomvc.rs`):
//! stand up the headless plugin preset → register the reducer → spawn the model →
//! step → read.
//!
//! It reuses the shipped, preluded building blocks — the GPU-free
//! [`BuiyProbePlugin`] preset (5b-1, which pulls in the
//! MVU substrate via the widgets plugin), [`mvu_model`](crate::MvuModelExt::mvu_model),
//! [`enqueue`](crate::enqueue)'s inbox, [`advance_clock`],
//! and the [`probe`](crate::probe) drive/inspect surface — behind one small type.
//!
//! # A model in ~5 lines
//!
//! ```
//! use buiy::prelude::*;
//! use buiy::test::MvuTestApp;
//!
//! #[derive(Component, Default, Clone, PartialEq, Reflect)]
//! #[reflect(Component)]
//! struct Counter {
//!     value: i64,
//! }
//! #[derive(Clone, Debug, PartialEq, Reflect)]
//! enum Msg {
//!     Increment,
//! }
//! impl Model for Counter {
//!     type Msg = Msg;
//! }
//! fn update(c: &mut Counter, msg: Msg) -> Cmd<Msg> {
//!     match msg {
//!         Msg::Increment => c.value += 1,
//!     }
//!     Cmd::none()
//! }
//!
//! let mut t = MvuTestApp::new(update); // preset + reducer; model type inferred
//! let e = t.spawn(Counter::default()); // spawned WITH a fresh LogicalId
//! t.enqueue(e, Msg::Increment); // enqueue — never fold
//! t.step(); // one frame: inbox → drain → bind
//! assert_eq!(t.read(e).value, 1); // read the live model
//! ```
//!
//! # Reading widget state through the probe
//!
//! Because the harness stands up the full GPU-free preset, the same instance can
//! drive and read **widget** state through [`buiy::probe`](crate::probe) — spawn a
//! widget on the harness's `world_mut`, settle, then use `get_by_role` / `click` /
//! `snapshot_report`. See `crates/buiy/tests/using_mvu_examples.rs`.

use std::marker::PhantomData;
use std::time::Duration;

use bevy::ecs::message::Messages;
use bevy::prelude::{App, Entity, World};

use buiy_core::mvu::IntoModelReducer;

use crate::{
    BuiyProbePlugin, ClockPlugin, Envelope, LogicalId, MinimalPlugins, Model, MvuModelExt,
    advance_clock,
};

/// A headless, single-model MVU test harness.
///
/// `M` is inferred from the reducer passed to [`new`](Self::new), so the common
/// single-model test needs no turbofish anywhere: [`spawn`](Self::spawn),
/// [`enqueue`](Self::enqueue) and [`read`](Self::read) all know the model type.
/// For a multi-model test, reach [`app_mut`](Self::app_mut) /
/// [`world_mut`](Self::world_mut) and register/drive the extra models directly.
///
/// See the [module docs](self) for the ~5-line usage and the widget-probe path.
pub struct MvuTestApp<M: Model> {
    app: App,
    next_id: u64,
    _model: PhantomData<M>,
}

impl<M: Model> MvuTestApp<M> {
    /// Stand up the GPU-free headless preset ([`BuiyProbePlugin`]
    /// on `MinimalPlugins` + `AssetPlugin` + `InputPlugin` — no window, no wgpu
    /// adapter) and register `reducer`. The model type `M` is **inferred from the
    /// reducer's `&mut M` argument** (the same trick as
    /// [`mvu_model`](crate::MvuModelExt::mvu_model)), so no turbofish is needed.
    ///
    /// The preset already installs the MVU substrate (via the widgets plugin), so
    /// the registered drain, inbox, work counters and — in debug builds — the
    /// Track-A [`LogicalId`] diagnostics are all live.
    pub fn new<Marker, F>(reducer: F) -> Self
    where
        F: IntoModelReducer<Marker, Model = M>,
        Marker: 'static,
    {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(bevy::asset::AssetPlugin::default())
            .add_plugins(bevy::input::InputPlugin)
            .add_plugins(BuiyProbePlugin);
        app.mvu_model(reducer);
        Self {
            app,
            next_id: 1,
            _model: PhantomData,
        }
    }

    /// Install a poll clock ([`ClockPlugin`]) that folds
    /// `map(now)` onto every actor each frame — for time-driven models
    /// (countdowns, animations, game loops). Drive it deterministically with
    /// [`advance_clock`](Self::advance_clock) (no real sleeps). Chainable:
    ///
    /// ```
    /// # use buiy::prelude::*;
    /// # use buiy::test::MvuTestApp;
    /// # use std::time::Duration;
    /// # #[derive(Component, Default, Clone, PartialEq, Reflect)]
    /// # #[reflect(Component)] struct Timer { secs: u64 }
    /// # #[derive(Clone, Debug, PartialEq, Reflect)] enum Msg { Tick(Duration) }
    /// # impl Model for Timer { type Msg = Msg; }
    /// # fn update(t: &mut Timer, msg: Msg) -> Cmd<Msg> {
    /// #     let Msg::Tick(now) = msg;
    /// #     t.secs = now.as_secs(); // store the DERIVED value, never `now` raw
    /// #     Cmd::none()
    /// # }
    /// let mut t = MvuTestApp::new(update).with_clock(Msg::Tick);
    /// let e = t.spawn(Timer::default());
    /// t.advance_clock(Duration::from_secs(3));
    /// assert_eq!(t.read(e).secs, 3);
    /// ```
    #[must_use]
    pub fn with_clock(mut self, map: fn(Duration) -> M::Msg) -> Self {
        self.app.add_plugins(ClockPlugin::<M>::new(map));
        self
    }

    /// Spawn a model actor **with a fresh, unique [`LogicalId`]**
    /// so Track A's fail-loud id checks stay quiet (a missing id corrupts the replay
    /// log; a duplicate id mis-routes folds — both warn in debug builds). Returns
    /// the spawned entity.
    pub fn spawn(&mut self, model: M) -> Entity {
        let id = self.next_id;
        self.next_id += 1;
        self.spawn_with_id(model, id)
    }

    /// Spawn a model actor with an **explicit** [`LogicalId`] — for
    /// replay tests that must match a recorded id, or to deliberately exercise the
    /// duplicate-id diagnostic. Auto-ids from [`spawn`](Self::spawn) skip past `id`,
    /// so mixing the two never collides.
    pub fn spawn_with_id(&mut self, model: M, id: u64) -> Entity {
        self.next_id = self.next_id.max(id + 1);
        self.app.world_mut().spawn((model, LogicalId(id))).id()
    }

    /// Enqueue `msg` for `target` — the single sanctioned mutation point. It writes
    /// the same [`Envelope`] that [`enqueue`](crate::enqueue)
    /// ultimately produces onto the model's inbox; the next [`step`](Self::step)
    /// drains and folds it. **Never** call the reducer directly (that skips the
    /// funnel, the record tap, and `set_if_neq`).
    pub fn enqueue(&mut self, target: Entity, msg: M::Msg) {
        self.app
            .world_mut()
            .resource_mut::<Messages<Envelope<M>>>()
            .write(Envelope::user(target, msg));
    }

    /// Run one frame (`app.update()`): enqueued messages flush, the drain folds
    /// them, and the bind stage projects any change. Chainable.
    pub fn step(&mut self) -> &mut Self {
        self.app.update();
        self
    }

    /// Run a few frames to clear the spawn-time `Added`/`Changed` ticks and let
    /// layout + a11y + text settle — the same discipline the hand-rolled harnesses
    /// use before asserting on an idle frame. Chainable.
    pub fn settle(&mut self) -> &mut Self {
        for _ in 0..3 {
            self.app.update();
        }
        self
    }

    /// Step the **virtual** clock by `delta` and run one frame, so a
    /// [`with_clock`](Self::with_clock) poll clock delivers the advanced `now` with
    /// zero wall-clock flakiness. Composes over repeated calls. Chainable.
    pub fn advance_clock(&mut self, delta: Duration) -> &mut Self {
        advance_clock(&mut self.app, delta);
        self
    }

    /// Read the live model on `entity`. Panics if the entity carries no `M` (a wrong
    /// entity, or one not spawned on this harness) — a loud test failure, not a
    /// silent `None`.
    pub fn read(&self, entity: Entity) -> &M {
        self.try_read(entity)
            .expect("no model of this type on the entity — did you spawn() it on this harness?")
    }

    /// Read the live model on `entity`, or `None` if absent.
    pub fn try_read(&self, entity: Entity) -> Option<&M> {
        self.app.world().get::<M>(entity)
    }

    /// The GPU-free probe's semantic-tree report (roles / names / state / layout
    /// rects) — the same string an agent inspects. Use it (with
    /// [`world_mut`](Self::world_mut) + the [`probe`](crate::probe) verbs) to read or
    /// drive **widget** state, e.g. a spawned `Checkbox`'s `[checked]` flag.
    pub fn snapshot_report(&mut self) -> String {
        crate::probe::snapshot_report(self.app.world_mut())
    }

    /// The underlying [`App`] — the escape hatch for anything the harness does not
    /// wrap (extra plugins, systems, a second model type).
    pub fn app_mut(&mut self) -> &mut App {
        &mut self.app
    }

    /// The underlying [`World`] (shared) — for direct component reads / the
    /// [`probe`](crate::probe) inspection verbs.
    pub fn world(&self) -> &World {
        self.app.world()
    }

    /// The underlying [`World`] (exclusive) — for the [`probe`](crate::probe)
    /// drive verbs (`get_by_role` / `click` / `set_value` / …) and spawning widgets.
    pub fn world_mut(&mut self) -> &mut World {
        self.app.world_mut()
    }
}
