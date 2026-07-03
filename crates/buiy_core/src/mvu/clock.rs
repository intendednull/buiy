//! `buiy_core::mvu::clock` — the **poll-clock-as-Msg** ergonomic (spec
//! `2026-07-03-dooduel-final-design.md` §2.8).
//!
//! A game (or any time-driven UI) needs the current time folded into its model every
//! frame so countdowns, reveals, and phase timeouts derive from `now`. The proven shape
//! is a **level-triggered poll clock**, NOT an edge-triggered timer:
//!
//! - [`ClockPlugin<M>`] enqueues `map(now)` onto every actor of model `M` **every frame**
//!   as an ordinary funnel [`Msg`](Model::Msg). The reducer folds it down to **derived**
//!   state (`seconds_left = total - (now - anchor)`), storing only the derived value —
//!   never raw `now`. A steady sub-second frame therefore folds byte-identically, and
//!   [`set_if_neq`](bevy::prelude::DetectChangesMut::set_if_neq) absorbs it
//!   ([`models_mutated == 0`](crate::mvu::MvuWorkCounters::models_mutated)) — the reducer is
//!   idempotent within a bucket, so the poll is perf-free in the steady state.
//!
//! This is deliberately **not** an edge-triggered `Cmd::interval` / `Cmd::timeout`: a
//! fired-once timer is an edge that is hard to replay and hard to keep `set_if_neq`-clean.
//! Delivering `now` and deriving is level-triggered — **replayable and idempotent by
//! construction**.
//!
//! ## Replay-safety
//!
//! The clock's non-determinism is confined to the `Msg` payload: `now` enters the record
//! log only as the `map(now)` message (the MVU §8 payload-carries-nondeterminism
//! invariant). [`crate::replay::replay_into`] re-feeds those logged messages and steps the
//! app; so the **live** driver must NOT inject fresh ticks during a replay (they would
//! diverge from the recorded stream). [`ClockPlugin`]'s driver is therefore **suppressed
//! while [`RecordSession::is_replaying`](crate::mvu::RecordSession::is_replaying)** — the same
//! discipline the async [`Cmd::Task`](crate::mvu::Cmd::Task) launch uses.
//!
//! ## Headless / deterministic captures
//!
//! [`advance_clock`] steps a headless app's **virtual** clock by a chosen delta and runs
//! one frame, so a test (or an animation capture) drives the poll clock with **zero
//! wall-clock flakiness** — no real sleeps. It mirrors `buiy_verify`'s per-timestamp
//! snapshot idiom (pin `TimeUpdateStrategy::ManualDuration` to `ZERO`, then advance
//! `Time<Virtual>`).
//!
//! ## Why a plugin, not a `Cmd::tick_every`
//!
//! A reducer-returned `Cmd::tick_every` would have to be **idempotent across folds** (a
//! reducer that returns it every fold must not spin up an unbounded number of drivers),
//! which requires a keyed per-actor subscription registry with add/remove diffing — the
//! `Subscription` machinery the MVU spec (`2026-06-29-mvu-as-core-design.md` §8) explicitly
//! **reserves for a later phase**. The prototype proved the poll clock is the whole answer
//! for game timing, so F7 ships the narrow, sufficient form: a plugin you add once per
//! clock-bearing model type. The reserved [`Origin::Subscription`](crate::mvu::Origin) stays a
//! §8 concern; a poll tick folds through the ordinary [`enqueue`] path
//! ([`Origin::User`](crate::mvu::Origin::User)).
//!
//! ## Example
//!
//! ```
//! use std::time::Duration;
//! use bevy::prelude::*;
//! use buiy_core::mvu::{Cmd, Model, MvuCorePlugin, MvuModelExt, LogicalId};
//! use buiy_core::mvu::clock::{ClockPlugin, advance_clock};
//!
//! #[derive(Component, Clone, PartialEq, Reflect, Default)]
//! #[reflect(Component)]
//! struct Countdown {
//!     // DERIVED state only — whole seconds elapsed. Never `now` itself.
//!     seconds: u64,
//! }
//! #[derive(Clone, Debug, Reflect, PartialEq)]
//! enum Msg {
//!     Tick(Duration),
//! }
//! impl Model for Countdown {
//!     type Msg = Msg;
//! }
//! fn update(m: &mut Countdown, msg: Msg) -> Cmd<Msg> {
//!     let Msg::Tick(now) = msg;
//!     m.seconds = now.as_secs(); // fold `now` down to derived state
//!     Cmd::none()
//! }
//!
//! let mut app = App::new();
//! app.add_plugins((bevy::time::TimePlugin, MvuCorePlugin));
//! app.mvu_model(update);
//! // Add the poll clock for this model (turbofish selects the model type — a bare
//! // `fn(Duration) -> Msg` like the `Tick` variant ctor cannot name it on its own).
//! app.add_plugins(ClockPlugin::<Countdown>::new(Msg::Tick));
//! let actor = app.world_mut().spawn((Countdown::default(), LogicalId(0))).id();
//!
//! advance_clock(&mut app, Duration::from_secs(3)); // step the virtual clock +1 frame
//! assert_eq!(app.world().get::<Countdown>(actor).unwrap().seconds, 3);
//! ```

use std::time::Duration;

use bevy::prelude::*;
use bevy::time::{TimeUpdateStrategy, Virtual};

use super::{Model, MvuSet, RecordSession, enqueue};

/// Holds a model `M`'s clock mapper `fn(Duration) -> M::Msg`. A resource (not a captured
/// closure) so the driver is an ordinary named generic system — the codebase idiom for
/// per-model generic systems (mirrors [`PendingTasks`](super::PendingTasks)). The `fn`
/// pointer is `Copy` + determinism-clean by type (it cannot capture a `Res`/clock/RNG
/// snapshot), so the poll stays replay-safe.
#[derive(Resource)]
pub struct ClockSource<M: Model> {
    map: fn(Duration) -> M::Msg,
}

/// The **poll-clock** plugin: installs a driver that enqueues `map(now)` onto **every**
/// actor of model `M` each frame, folded through the ordinary funnel.
///
/// Add it **once per clock-bearing model type**, after [`MvuCorePlugin`](super::MvuCorePlugin)
/// (which owns the [`MvuSet::Enqueue`] window the driver runs in) and a time source
/// (`TimePlugin` / `MinimalPlugins` / `DefaultPlugins`, so `Res<Time>` exists):
///
/// ```ignore
/// app.add_plugins(ClockPlugin::<Dooduel>::new(Msg::Tick));
/// ```
///
/// The driver enqueues onto `Query<Entity, With<M>>` — so 0 actors ⇒ a no-op (the frame
/// before the model spawns is skipped), 1 actor ⇒ the common singleton game model, N ⇒ every
/// actor gets the tick (a clock is a global source). It is **suppressed during replay** (see
/// the module docs): [`RecordSession::is_replaying`](super::RecordSession::is_replaying) short-
/// circuits it so a live tick cannot diverge a re-fed recording.
pub struct ClockPlugin<M: Model> {
    map: fn(Duration) -> M::Msg,
}

impl<M: Model> ClockPlugin<M> {
    /// A poll clock delivering `map(now)` — where `now` is the elapsed time
    /// (`Res<Time>::elapsed()`) — to model `M` every frame. `map` is a **bare `fn`** (an
    /// enum tuple-variant ctor such as `Msg::Tick` is exactly `fn(Duration) -> M::Msg`), so
    /// it is `Copy` and cannot capture — determinism-safe by type.
    pub fn new(map: fn(Duration) -> M::Msg) -> Self {
        ClockPlugin { map }
    }
}

impl<M: Model> Plugin for ClockPlugin<M> {
    fn build(&self, app: &mut App) {
        app.insert_resource(ClockSource::<M> { map: self.map });
        app.add_systems(Update, drive_clock::<M>.in_set(MvuSet::Enqueue));
    }
}

/// Enqueue `map(now)` onto every actor of model `M` this frame — the poll clock's per-frame
/// work. Runs in [`MvuSet::Enqueue`], so the pinned `ApplyDeferred` flushes the enqueue into
/// the SAME-frame drain (one designed frame of latency, no extra). Reads the elapsed time off
/// the generic `Res<Time>` (which the `TimePlugin` syncs from `Time<Virtual>` at the head of
/// each frame — so [`advance_clock`] drives it deterministically).
///
/// **Replay guard.** Short-circuits while a replay is in progress: [`replay_into`] re-feeds the
/// logged `map(now)` messages and steps the app, so a live tick here would double-drive the
/// model and diverge the replay (the same reason a re-folded [`Cmd::Task`] is suppressed).
///
/// [`replay_into`]: crate::replay::replay_into
/// [`Cmd::Task`]: super::Cmd::Task
fn drive_clock<M: Model>(
    time: Res<Time>,
    session: Res<RecordSession>,
    source: Res<ClockSource<M>>,
    actors: Query<Entity, With<M>>,
    mut commands: Commands,
) {
    if session.is_replaying() {
        return;
    }
    let now = time.elapsed();
    let map = source.map;
    for actor in &actors {
        enqueue::<M>(&mut commands, actor, map(now));
    }
}

/// **Headless virtual-clock advance** — step the app's virtual clock by `delta` and run one
/// frame, so a [`ClockPlugin`] poll clock delivers the advanced `now` with **zero wall-clock
/// flakiness** (no real sleeps). The deterministic driver for tests and animation captures.
///
/// It pins the clock to manual stepping (`TimeUpdateStrategy::ManualDuration` of `ZERO`,
/// idempotent) so the `TimePlugin`'s automatic wall-clock advance cannot leak into the logical
/// time, then advances `Time<Virtual>` by `delta`; the `TimePlugin` syncs `Virtual` into the
/// generic `Time` at the head of the `update()`, which is what the [`ClockPlugin`] driver reads.
/// Advancing by a **relative** `delta` each call composes into any schedule of steps.
///
/// **Precondition:** a `TimePlugin` (via `MinimalPlugins` / `DefaultPlugins` /
/// `BuiyHeadlessPlugin` / the probe preset), so `Time<Virtual>` and
/// `TimeUpdateStrategy` exist.
pub fn advance_clock(app: &mut App, delta: Duration) {
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::ZERO));
    app.world_mut()
        .resource_mut::<Time<Virtual>>()
        .advance_by(delta);
    app.update();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mvu::{
        Cmd, Envelope, LogicalId, MvuCorePlugin, MvuModelExt, MvuWorkCounters, RecordMode,
    };
    use bevy::time::TimePlugin;

    // A derived-countdown model in the BlinkLeaf mold: it stores only the DERIVED whole
    // seconds elapsed, never raw `now`. So a steady sub-second frame folds byte-identically
    // and `set_if_neq` absorbs it.
    #[derive(Component, Clone, PartialEq, Reflect, Default)]
    #[reflect(Component)]
    struct Countdown {
        seconds: u64,
    }

    #[derive(Clone, Debug, Reflect, PartialEq)]
    enum ClockMsg {
        Tick(Duration),
    }

    impl Model for Countdown {
        type Msg = ClockMsg;
    }

    /// The RIGHT reducer: fold `now` down to derived whole seconds.
    fn derive_seconds(m: &mut Countdown, msg: ClockMsg) -> Cmd<ClockMsg> {
        let ClockMsg::Tick(now) = msg;
        m.seconds = now.as_secs();
        Cmd::none()
    }

    // The WRONG counterfactual model: it stores raw `now`, so it mutates EVERY frame.
    #[derive(Component, Clone, PartialEq, Reflect, Default)]
    #[reflect(Component)]
    struct RawClock {
        now: Duration,
    }
    impl Model for RawClock {
        type Msg = ClockMsg;
    }
    fn store_raw(m: &mut RawClock, msg: ClockMsg) -> Cmd<ClockMsg> {
        let ClockMsg::Tick(now) = msg;
        m.now = now; // the anti-pattern the gate exists to catch
        Cmd::none()
    }

    fn base_app() -> App {
        let mut app = App::new();
        app.add_plugins((TimePlugin, MvuCorePlugin));
        app
    }

    /// The clock delivers `map(now)` and the reducer folds it into derived state, driven
    /// deterministically by [`advance_clock`].
    #[test]
    fn poll_clock_folds_now_into_derived_state() {
        let mut app = base_app();
        app.mvu_model(derive_seconds);
        app.add_plugins(ClockPlugin::<Countdown>::new(ClockMsg::Tick));
        let actor = app
            .world_mut()
            .spawn((Countdown::default(), LogicalId(0)))
            .id();

        advance_clock(&mut app, Duration::from_secs(1));
        assert_eq!(app.world().get::<Countdown>(actor).unwrap().seconds, 1);
        advance_clock(&mut app, Duration::from_secs(4));
        assert_eq!(app.world().get::<Countdown>(actor).unwrap().seconds, 5);
    }

    /// The set_if_neq steady-frame **no-cascade gate** (the BlinkLeaf shape, spec §2.8): a
    /// steady sub-second tick leaves the derived `seconds` unchanged, so `models_mutated == 0`
    /// AND `binds_fired == 0` (the change signal does not reach the derived-view stage). A
    /// bucket-crossing frame mutates exactly once. The wrong reducer (storing raw `now`) FAILS
    /// this — proving the gate has teeth.
    #[test]
    fn steady_tick_does_not_cascade() {
        let mut app = base_app();
        app.mvu_model(derive_seconds);
        app.add_plugins(ClockPlugin::<Countdown>::new(ClockMsg::Tick));
        app.world_mut().spawn((Countdown::default(), LogicalId(0)));

        // Settle inside the first whole second (seconds == 0): warm-up frames clear the
        // spawn's `Added`/`Changed` tick so `binds_fired` reflects only fold-driven change
        // (the `RenderWorkCounters`/BlinkLeaf settle idiom). Stays under 1s, so no crossing.
        for _ in 0..3 {
            advance_clock(&mut app, Duration::from_millis(10)); // 30ms, seconds == 0
        }

        // A genuinely steady sub-second frame — still bucket 0, no mutate, no cascade.
        advance_clock(&mut app, Duration::from_millis(10)); // now 40ms
        let c = *app.world().resource::<MvuWorkCounters>();
        assert!(c.drain_folds >= 1, "the tick was delivered + folded");
        assert_eq!(
            c.models_mutated, 0,
            "derived seconds unchanged ⇒ set_if_neq no-op"
        );
        assert_eq!(
            c.binds_fired, 0,
            "no change ⇒ no cascade to the derived-view stage"
        );

        // Another steady sub-second frame — still bucket 0, still no cascade.
        advance_clock(&mut app, Duration::from_millis(10)); // now 50ms
        let c = *app.world().resource::<MvuWorkCounters>();
        assert_eq!(c.models_mutated, 0);
        assert_eq!(c.binds_fired, 0);

        // Cross into the next whole second — now the derived value flips exactly once.
        advance_clock(&mut app, Duration::from_millis(1000)); // now 1050ms ⇒ seconds == 1
        let c = *app.world().resource::<MvuWorkCounters>();
        assert_eq!(c.models_mutated, 1, "a bucket crossing mutates once");
        assert_eq!(
            c.binds_fired, 1,
            "the crossing DOES cascade to the derived-view stage"
        );
    }

    /// The counterfactual: a reducer that stores raw `now` mutates on a steady sub-second
    /// frame — the failure the [`steady_tick_does_not_cascade`] gate catches.
    #[test]
    fn wrong_reducer_storing_raw_now_cascades_every_frame() {
        let mut app = base_app();
        app.mvu_model(store_raw);
        app.add_plugins(ClockPlugin::<RawClock>::new(ClockMsg::Tick));
        app.world_mut().spawn((RawClock::default(), LogicalId(0)));

        advance_clock(&mut app, Duration::from_millis(100));
        advance_clock(&mut app, Duration::from_millis(100));
        let c = *app.world().resource::<MvuWorkCounters>();
        assert_eq!(
            c.models_mutated, 1,
            "storing raw now mutates every steady frame — the anti-pattern"
        );
    }

    /// The live clock is **suppressed during replay**: while `is_replaying()` is set, no tick
    /// is enqueued (so a re-fed recording is not double-driven). It resumes when cleared.
    #[test]
    fn clock_suppressed_during_replay() {
        let mut app = base_app();
        app.mvu_model(derive_seconds);
        app.add_plugins(ClockPlugin::<Countdown>::new(ClockMsg::Tick));
        app.world_mut().spawn((Countdown::default(), LogicalId(0)));

        app.world_mut()
            .resource_mut::<RecordSession>()
            .set_replaying(true);
        advance_clock(&mut app, Duration::from_secs(1));
        assert_eq!(
            app.world().resource::<MvuWorkCounters>().drain_folds,
            0,
            "no tick folds while replaying — the live driver is suppressed"
        );

        app.world_mut()
            .resource_mut::<RecordSession>()
            .set_replaying(false);
        advance_clock(&mut app, Duration::from_secs(1));
        assert!(
            app.world().resource::<MvuWorkCounters>().drain_folds >= 1,
            "the live clock resumes once replay ends"
        );
    }

    /// [`advance_clock`] is deterministic: two apps advanced by the identical delta schedule
    /// reach the identical model state (zero wall-clock flakiness — the property that lets a
    /// headless test drive a full match instantly).
    #[test]
    fn advance_clock_is_deterministic() {
        fn run() -> u64 {
            let mut app = base_app();
            app.mvu_model(derive_seconds);
            app.add_plugins(ClockPlugin::<Countdown>::new(ClockMsg::Tick));
            let actor = app
                .world_mut()
                .spawn((Countdown::default(), LogicalId(0)))
                .id();
            for _ in 0..7 {
                advance_clock(&mut app, Duration::from_secs(1));
            }
            app.world().get::<Countdown>(actor).unwrap().seconds
        }
        assert_eq!(run(), 7);
        assert_eq!(run(), run());
    }

    /// The direct-injection sim pattern (the prototype's virtual-clock idiom): writing
    /// `Tick(now)` straight to the inbox drives the reducer with no `ClockPlugin` and no time
    /// source at all — the "instant full-match sim" a pure-logic test uses.
    #[test]
    fn direct_tick_injection_drives_the_reducer() {
        let mut app = base_app();
        app.mvu_model(derive_seconds);
        let actor = app
            .world_mut()
            .spawn((Countdown::default(), LogicalId(0)))
            .id();

        for n in [0u64, 2, 5, 9] {
            app.world_mut()
                .resource_mut::<bevy::ecs::message::Messages<Envelope<Countdown>>>()
                .write(Envelope::user(
                    actor,
                    ClockMsg::Tick(Duration::from_secs(n)),
                ));
            app.update();
            assert_eq!(app.world().get::<Countdown>(actor).unwrap().seconds, n);
        }
    }

    /// The clock's `Tick(now)` payload is recorded when a session is on — the record tap sees
    /// it like any funnel message (so a recording captures the clock stream for replay).
    #[test]
    fn tick_payload_is_recorded_when_session_on() {
        let mut app = base_app();
        app.mvu_model(derive_seconds);
        app.add_plugins(ClockPlugin::<Countdown>::new(ClockMsg::Tick));
        app.world_mut().spawn((Countdown::default(), LogicalId(0)));
        app.world_mut()
            .resource_mut::<RecordSession>()
            .set_mode(RecordMode::Full);

        advance_clock(&mut app, Duration::from_secs(1));
        let log = app.world().resource::<crate::mvu::MsgLog>();
        assert!(
            log.entries.iter().any(|e| e.ron.contains("Tick")),
            "the clock's Tick(now) enters the log as an ordinary funnel message"
        );
    }
}
