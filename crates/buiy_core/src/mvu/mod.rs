//! `buiy_core::mvu` — the **Model-View-Update** substrate.
//!
//! Buiy's primary state interface: a recordable message substrate in `buiy_core` that
//! widgets route state changes through, governed by one ordered funnel. Design + rationale:
//! `docs/specs/2026-06-29-mvu-as-core-design.md` (§2 the substrate, §3 tiered granularity).
//!
//! Three load-bearing pieces:
//!
//! 1. **`set_if_neq` drain discipline** (spec §2 — the load-bearing perf rule). Folding
//!    directly on `Query<&mut M>` would trip `Changed<M>` on EVERY fold — including no-ops —
//!    which cascades bind → layout → re-extract (the perf-audit #2/#6 cliffs). The drain
//!    instead folds onto a **clone** and commits via `Mut::set_if_neq`, which `deref_mut`s —
//!    and therefore trips `Changed<M>` — *only* on a real change. An idempotent fold leaves
//!    change-detection untripped.
//! 2. **[`MvuWorkCounters`]** — the host-independent measurement gate (modeled on
//!    `render::RenderWorkCounters`): idle ⇒ all 0; idempotent fold ⇒ `models_mutated == 0`.
//! 3. **Schedule integration into [`crate::BuiySet`]** with a pinned `ApplyDeferred` between
//!    `Enqueue` and `Drain` (so deferred [`enqueue`](crate::mvu::enqueue) writes flush before the drain reads).
//!
//! A **drain** is the ONLY place a model changes. Most models use the single late ordered
//! drain ([`MvuSet::Drain`]); a tier that must fold at a *specific* point in the frame installs
//! its drain in a **caller-chosen** [`SystemSet`] via [`MvuAppExt::add_reducer_in_set`]
//! (the early-window model, spec §4 — the toggle leaf folds
//! `.after(BuiySet::Picking).before(BuiySet::A11yUpdate)`, so an AT-driver click reflects in the
//! a11y tree the SAME frame; the late drain would lag it one frame). Either way: handlers,
//! observers and callbacks may ONLY [`enqueue`](crate::mvu::enqueue) — never call a reducer. A drain is a normal
//! **system** in a pinned set, NEVER an observer (observers fire at unpredictable
//! command-flush points, re-entrantly — fatal for a deterministic ordered fold + record tap).
//!
//! ## Roadmap surface (kept minimal in v1)
//! - **Routing** (`OnPress → Model` bubbling) and **callbacks** — a later phase.
//! - `Cmd::task` / keyed `Subscription` / dead-letter / `catch_unwind` supervision — spec §8.
//! - The lazy bounded-ring record buffer + `#[derive(PureEnv)]` + `Local` in the env
//!   allowlist — flagged `TODO` at their sites.

#[cfg(debug_assertions)]
use bevy::ecs::change_detection::Tick;
use bevy::ecs::system::{StaticSystemParam, SystemParam, SystemParamItem};
use bevy::prelude::*;
use bevy::reflect::serde::{TypedReflectDeserializer, TypedReflectSerializer};
use bevy::reflect::{FromReflect, GetTypeRegistration, TypePath, TypeRegistry};
use serde::de::DeserializeSeed;
use std::any::TypeId;
use std::collections::{HashMap, VecDeque};

use crate::BuiySet;

/// The stateful-leaf tier (`A11yToggled` as a shared-reducer leaf model).
pub mod leaf;
pub use leaf::{ToggleLeafSet, ToggleMsg, register_toggle_leaf, toggle_reducer};

// ---------------------------------------------------------------------------
// Core model + message
// ---------------------------------------------------------------------------

/// A widget actor's state. The trait declares only associated types — the reducer is a
/// *free function* registered separately (it cannot be a method: methods can't take
/// `SystemParam`s, and we want the env to be a real, purity-checked param).
///
/// **The `Clone + PartialEq` bounds.** The drain folds onto a *clone* and
/// commits with `Mut::set_if_neq` (needs `Clone` for the working
/// copy, `PartialEq` for the change test) — the discipline that keeps an idempotent fold from
/// tripping `Changed<M>` and cascading a re-extract (spec §2).
pub trait Model:
    Component<Mutability = bevy::ecs::component::Mutable>
    + Reflect
    + GetTypeRegistration
    + Clone
    + PartialEq
    + Send
    + Sync
    + 'static
{
    /// The messages this model folds. Must be `Reflect` so the log round-trips cross-process
    /// (the record/replay capstone — a later wave).
    type Msg: Clone
        + std::fmt::Debug
        + Reflect
        + FromReflect
        + TypePath
        + GetTypeRegistration
        + Send
        + Sync
        + 'static;
}

/// The inbox transport. We use Bevy's buffered `Messages`, but **our drain is the only
/// reader** — that read is the determinism/record tap. Generic per model type, so the
/// one-Msg-type ↔ one-Model-type invariant is structural (5000 buttons of one kind share one
/// inbox + one drain; the drain is `O(messages/frame)`, never `O(instances)` — SYNTHESIS SCALE).
pub struct Envelope<M: Model> {
    pub target: Entity,
    pub msg: M::Msg,
}

// Manual impls: deriving on a generic-over-`Model` struct would demand `M: Message`/`M: Clone`,
// which is wrong — `M` is the model, not the message.
impl<M: Model> Message for Envelope<M> {}
impl<M: Model> Clone for Envelope<M> {
    fn clone(&self) -> Self {
        Envelope {
            target: self.target,
            msg: self.msg.clone(),
        }
    }
}

/// Effects-as-values. The reducer returns these; **only the drain applies them**, so the
/// reducer itself stays pure.
///
/// **v1 keeps `None`/`Emit`/`Batch` ONLY.** `task` (async), the keyed `Subscription`, and
/// dead-letter routing are roadmap concerns (spec §8); adding them now would be
/// scope creep on the substrate.
pub enum Cmd<Msg> {
    /// Do nothing.
    None,
    /// Fold another message back through the reducer on the same entity, run-to-completion
    /// within the same drain pass (the deterministic, tick-exact fold-back).
    Emit(Msg),
    /// Apply several commands in order.
    Batch(Vec<Cmd<Msg>>),
}

impl<Msg> Cmd<Msg> {
    pub fn none() -> Self {
        Cmd::None
    }
    pub fn emit(msg: Msg) -> Self {
        Cmd::Emit(msg)
    }
}

// ---------------------------------------------------------------------------
// Stable identity + the Reflect record tap
// ---------------------------------------------------------------------------

/// A session-stable, **logical** identity for an actor — aligned (by intent) with the
/// agent-interface test-id space (SYNTHESIS D7). The log keys on this, NOT raw `Entity`, so a
/// replay in a *fresh process* (different `Entity` allocation order) still lands correctly.
#[derive(Component, Clone, Copy, PartialEq, Eq, Hash, Debug, Reflect)]
pub struct LogicalId(pub u64);

impl LogicalId {
    pub const UNRESOLVED: LogicalId = LogicalId(u64::MAX);
}

/// The provenance of a recorded fold — **baked into the v1 log format** (spec §7.2 /
/// D14) so the §8 roadmap can filter and replay by source without a format break.
///
/// The drain stamps two of the four:
/// - [`Origin::User`] — the fold came from the inbox (a real [`enqueue`]).
/// - [`Origin::Folded`] — the fold came from a [`Cmd::Emit`] re-fold within the same
///   drain pass (the deterministic, tick-exact fold-back).
///
/// [`Origin::Command`] (async-command result) and [`Origin::Subscription`] (timer/OS
/// source) are **reserved** for the §8 roadmap — defined now only so adding them later is
/// not a format change. The goal here is *format stability*; correctness of the
/// User-vs-Folded split is a nice-to-have the drain happens to get right.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Reflect, Default)]
pub enum Origin {
    /// From the inbox / [`enqueue`] — a real external message. The default.
    #[default]
    User,
    /// Reserved (§8): the result of an async [`Cmd`] command. Not emitted in v1.
    Command,
    /// From a [`Cmd::Emit`] re-fold within the same drain pass.
    Folded,
    /// Reserved (§8): a subscription (timer/OS) source. Not emitted in v1.
    Subscription,
}

/// One recorded fold: which actor, in what order, the message [`Origin`], and the
/// `Reflect`-serialized message.
#[derive(Clone, Debug)]
pub struct LoggedEntry {
    pub lid: LogicalId,
    pub seq: u64,
    pub type_path: String,
    pub ron: String,
    /// The provenance of this fold (`User` from the inbox vs `Folded` from a
    /// `Cmd::Emit`; `Command`/`Subscription` reserved for §8). Baked into the v1 format
    /// for §8 format stability (spec §7.2 / D14).
    pub origin: Origin,
}

/// How the record tap behaves. Lives on the shared [`RecordSession`] switch (the unified
/// record session — one switch for the whole UI), NOT per-log.
///
/// **Default [`RecordMode::Off`] — production pays ZERO** (SYNTHESIS H7): in `Off`,
/// [`RecordSession::tick_seq`] returns `None`, so no entry is built, serialized, or
/// stored, in EITHER the widget log ([`MsgLog`]) or the editor log
/// ([`EditLog`](crate::text::edit::EditLog)).
///
/// `Ring(n)` and `Full` both record eagerly for now; the lazy, bounded, typed-message
/// ring (store `Box<dyn Reflect>`, serialize to RON only at export) is a later refinement.
// TODO(FINAL): make `Ring(n)` a bounded buffer of typed messages; serialize lazily at export.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum RecordMode {
    /// No recording. The hot-path default.
    #[default]
    Off,
    /// Keep the last `n` folds (v1: behaves like `Full`; bounding is a later refinement).
    Ring(usize),
    /// Record every fold.
    Full,
}

/// **The unified record session: ONE global record switch + ONE monotonic sequence** shared
/// by the two record taps — the widget-fold drain (writing [`MsgLog`]) and the editor
/// command-source taps (writing [`EditLog`](crate::text::edit::EditLog)).
///
/// Both per-domain logs share this one switch + sequence rather than owning a per-log `mode`
/// and `seq`, so the two streams CAN be totally ordered for an *interleaved* whole-UI replay.
/// Every recorded entry — widget fold OR editor command — draws its `seq` from this one
/// counter via [`RecordSession::tick_seq`], making the two logs **mergeable into one ordered
/// stream by `seq`** (see [`crate::replay`]). `tick_seq` is also the single gate: `None` when
/// [`RecordMode::Off`] ⇒ the tap does zero work (spec §7.1, "default-OFF pays zero").
///
/// Inited by both [`MvuCorePlugin`] and `BuiyTextPlugin` (idempotently), so either the
/// MVU chain or the text stack alone provides the switch.
#[derive(Resource, Default)]
pub struct RecordSession {
    mode: RecordMode,
    next_seq: u64,
}

impl RecordSession {
    /// The current record mode.
    pub fn mode(&self) -> RecordMode {
        self.mode
    }

    /// Set the record mode without touching the global sequence (the bench/scene knob).
    pub fn set_mode(&mut self, mode: RecordMode) {
        self.mode = mode;
    }

    /// Whether a session is currently recording (the cheap gate a tap checks).
    pub fn is_recording(&self) -> bool {
        self.mode != RecordMode::Off
    }

    /// Begin a fresh whole-UI recording session: [`RecordMode::Full`] + the global
    /// sequence reset to 0. (The per-domain logs are separate resources — clear them
    /// too, or start from a fresh app, for a clean stream.)
    pub fn start(&mut self) {
        self.mode = RecordMode::Full;
        self.next_seq = 0;
    }

    /// Stop recording (e.g. before a replay so it does not re-log).
    pub fn stop(&mut self) {
        self.mode = RecordMode::Off;
    }

    /// The next **global** sequence number iff recording, else `None` (zero work when
    /// [`RecordMode::Off`]). The ONE call both taps make to stamp + order an entry.
    pub fn tick_seq(&mut self) -> Option<u64> {
        if self.mode == RecordMode::Off {
            return None;
        }
        let seq = self.next_seq;
        self.next_seq += 1;
        Some(seq)
    }
}

/// The append-only **widget-fold** record tap. Lives as a resource; the drain writes
/// to it. The record switch + global `seq` live on the shared [`RecordSession`],
/// so this holds only the entries.
#[derive(Resource, Default)]
pub struct MsgLog {
    pub entries: Vec<LoggedEntry>,
}

impl MsgLog {
    /// Drop all recorded entries (test/checkpoint convenience).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Append one recorded fold, stamped with the **global** `seq` (from
    /// [`RecordSession::tick_seq`]) and its [`Origin`]. The caller only reaches here when
    /// recording is on.
    fn record<Msg>(
        &mut self,
        seq: u64,
        registry: &TypeRegistry,
        lid: LogicalId,
        msg: &Msg,
        origin: Origin,
    ) where
        Msg: Reflect + TypePath,
    {
        let ser = TypedReflectSerializer::new(msg, registry);
        let ron = ron::ser::to_string(&ser).expect("reflect-serialize message");
        self.entries.push(LoggedEntry {
            lid,
            seq,
            type_path: Msg::type_path().to_string(),
            ron,
            origin,
        });
    }
}

// ---------------------------------------------------------------------------
// Replay dispatch: re-enqueue a logged widget Msg by its type path
// ---------------------------------------------------------------------------

/// A per-model-type replay applier: deserialize a logged widget Msg (its RON) back
/// into the concrete `M::Msg` and write it to the model's inbox, so the SAME
/// registered drain re-folds it during replay (the "drain path"). Cross-process
/// capable — it round-trips through `Reflect`/RON + the app's `TypeRegistry`.
type ReplayApplier = Box<dyn Fn(&mut World, Entity, &str) + Send + Sync>;

/// Maps a widget `Msg` **type path** → the closure that re-enqueues a logged entry of
/// that type onto its target. Populated once per model type by
/// [`MvuAppExt::add_model`]; consumed by [`crate::replay`] to re-fold the widget half
/// of a unified whole-UI record stream into a fresh app. The editor half replays
/// through `TextEditState::apply_recorded` instead (the editor is not a `Model`).
#[derive(Resource, Default)]
pub struct ReplayRegistry {
    appliers: HashMap<String, ReplayApplier>,
}

impl ReplayRegistry {
    /// The re-enqueue closure for a logged entry's `type_path`, if a model of that
    /// `Msg` type was registered.
    pub fn applier(&self, type_path: &str) -> Option<&ReplayApplier> {
        self.appliers.get(type_path)
    }
}

// ---------------------------------------------------------------------------
// Work counters (the host-independent measurement gate — SYNTHESIS H3/PERF)
// ---------------------------------------------------------------------------

/// Per-frame MVU work counts — the deterministic, host-independent gate the perf design
/// mandates (modeled on `render::RenderWorkCounters`). A settled scene asserts these EXACTLY,
/// identical on any CPU, so a re-introduced cliff reddens on a slow runner just as on the dev box.
///
/// **Overwrite convention (mirrors `RenderWorkCounters`).** Fields are RESET to 0 BEFORE any
/// MVU drain each frame (`reset_mvu_counters`, anchored `.before(BuiySet::Picking)` so it
/// precedes BOTH the early caller-chosen leaf drain (the early-window model, spec §4) and the
/// late [`MvuSet::Drain`]) and then accumulated by the drain(s) (one per model type) + the bind
/// counters. So the values read after `app.update()` describe THIS frame only — an idle frame
/// reads all-0.
///
/// **Invariants the substrate tests assert:** idle ⇒ every field 0; one message ⇒ `drain_folds == 1`
/// with `models_mutated == 1`; idempotent fold ⇒ `models_mutated == 0` (the load-bearing
/// `set_if_neq` proof) and `binds_fired == 0` (the cascade does NOT propagate).
#[derive(Resource, Default, Debug, Clone, Copy)]
pub struct MvuWorkCounters {
    /// Reducer `fold` invocations this frame (incl. `Emit` re-folds; excl. dead-letters).
    pub drain_folds: u64,
    /// Folds whose message was actually written to the log (0 when `RecordMode::Off`).
    pub messages_recorded: u64,
    /// Folds that produced a REAL change — `set_if_neq` returned `true`, tripping `Changed<M>`.
    /// `0` on an idempotent fold: the discipline that stops the funnel re-introducing the
    /// re-extract cliff.
    pub models_mutated: u64,
    /// Entities observed `Changed<M>` at the [`MvuSet::Bind`] stage this frame — the proof the
    /// change signal (or its absence, under `set_if_neq`) reaches the derived-view stage.
    pub binds_fired: u64,
    /// `Emit` effects re-queued back through the drain this frame.
    pub emits_refolded: u64,
}

// ---------------------------------------------------------------------------
// Debug-only write-outside-the-funnel auditor (spec §7.5)
// ---------------------------------------------------------------------------

/// Per-entity "last funnel-write tick" stamp — the substrate of the §7.5 single-writer
/// auditor. Each funnel fold that *changes* a model records the change tick here (in
/// `fold_one_with`, shared by the batch drain and the AT seam); the bind-stage
/// `count_binds` system (debug builds) then flags any `Model`-bearing component whose
/// `last_changed()` differs from its stamp — a write that did NOT come through the funnel.
///
/// **Entirely `cfg(debug_assertions)`** — it compiles OUT of release/bench builds, where
/// `debug_assertions` is off, so the iai perf gate's instruction count is unaffected. The
/// audit is folded INTO the existing per-model `count_binds` system (no NEW system, so it
/// adds no system entity and cannot shift entity-id-keyed layout snapshots), via a **resource
/// map** (not a per-entity component, so no archetype move on the stamped entity) that touches
/// no [`MvuWorkCounters`] field — the auditor is invisible to every work-counter gate.
///
/// **Scope:** the env-free drain ([`MvuAppExt::add_reducer_in_set`]) and the AT seam
/// ([`fold_one_inline`]) — every shipped model folds through one of these. An *env-reading*
/// reducer ([`MvuAppExt::add_reducer_env`]) does not stamp, so its models are simply not
/// audited (its folds look like un-stamped seeds — a false **negative**, never a false
/// positive). No shipped model uses an env reducer.
#[cfg(debug_assertions)]
#[derive(Resource, Default)]
pub struct FunnelWriteStamps(HashMap<Entity, Tick>);

/// One detected write-outside-the-funnel — a `Model`-bearing component mutated after its
/// entity's first funnel fold, by something other than the drain (spec §7.5 / §10's L6
/// escape-hatch trap). Collected by the debug `count_binds` audit so a test or tool can
/// assert on the boundary; also `warn!`-logged as it happens.
#[cfg(debug_assertions)]
#[derive(Clone, Debug)]
pub struct FunnelViolation {
    /// The entity whose model was written outside the funnel.
    pub entity: Entity,
    /// The `Model` component type that was raw-written (`type_name`).
    pub type_name: &'static str,
}

/// The append-only log of write-outside-the-funnel violations the §7.5 auditor detected
/// (debug builds only). Empty in a single-writer-clean app; a non-empty list flags an
/// escape-hatched raw-ECS write of `Model` state. Tests read this to prove the auditor fires
/// **only** on a genuine runtime violation (never on a legit fold, an AT-seam fold, or a
/// spawn-time seed).
#[cfg(debug_assertions)]
#[derive(Resource, Default)]
pub struct FunnelAuditLog {
    /// Every violation observed since the app started (or since a test cleared it).
    pub violations: Vec<FunnelViolation>,
}

// ---------------------------------------------------------------------------
// Scheduling
// ---------------------------------------------------------------------------

/// The MVU sub-sets, chained `Enqueue → Drain → Bind` and folded into [`crate::BuiySet`]
/// between `A11yUpdate` and `Render` (spec §3/§4). Press handlers / observers / callbacks
/// (which only enqueue) run in `Enqueue`; binds that read `Changed<Model>` run in `Bind`.
///
/// This is the **default (machine-tier) slot**. A tier that must fold earlier in the frame
/// installs its drain in a caller-chosen [`SystemSet`] via [`MvuAppExt::add_reducer_in_set`]
/// (the early-window model, spec §4 — the toggle leaf folds before `BuiySet::A11yUpdate`),
/// bypassing this set.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum MvuSet {
    /// Enqueue-only edge (press/observer/callback handlers).
    Enqueue,
    /// The default late ordered drain (machine tier): record tap + reducer + `set_if_neq`
    /// commit. The early-folding leaf tier installs its drain in its own set instead (spec §4).
    Drain,
    /// Bind/derived-view writes that react to `Changed<Model>`.
    Bind,
}

// ---------------------------------------------------------------------------
// Enqueue (the ONLY way a message enters the funnel)
// ---------------------------------------------------------------------------

/// Enqueue `msg` for `target` via the model's inbox. Usable from any system or observer that
/// holds `Commands` (it defers a write into `Messages<Envelope<M>>`). With the pinned
/// `ApplyDeferred` between [`MvuSet::Enqueue`] and [`MvuSet::Drain`], a write from an `Enqueue`
/// system is flushed and drained **in the same frame**. This is the hard rule's single
/// sanctioned mutation point: handlers enqueue, they never fold.
pub fn enqueue<M: Model>(commands: &mut Commands, target: Entity, msg: M::Msg) {
    commands.queue(move |world: &mut World| {
        world
            .resource_mut::<Messages<Envelope<M>>>()
            .write(Envelope { target, msg });
    });
}

// ---------------------------------------------------------------------------
// Purity enforcement: the sealed `PureEnv` allowlist
// ---------------------------------------------------------------------------

mod sealed {
    pub trait Sealed {}
}

/// A reducer's read-only **environment** — the params it may read while folding.
///
/// This is the *real* purity gate (NOT `ReadOnlySystemParam`): it is a sealed **allowlist**.
/// `()`/`Res`/read-only `Query`/tuples are blessed; `Commands` is deliberately absent. In Bevy
/// 0.19 `Commands: ReadOnlySystemParam` (it only *defers* its structural mutation), so a
/// `ReadOnlySystemParam` bound would let a reducer `spawn`/`despawn` — unrecorded mutation that
/// breaks replay. An allowlist is secure by construction: a type is pure iff *we* blessed it,
/// and the orphan rule stops downstream crates from blessing `Commands`.
///
/// **v1 keeps the minimal allowlist** (`()`, `Res`, read-only `Query`, tuples up to 3).
// TODO(roadmap): add `Local` to the allowlist + a `#[derive(PureEnv)]` that blesses a user
// struct of blessed fields (the residual gap — spec §2). Deferred so v1 ships the minimal gate.
pub trait PureEnv: SystemParam + sealed::Sealed {}

impl sealed::Sealed for () {}
impl PureEnv for () {}

impl<'w, T: Resource> sealed::Sealed for Res<'w, T> {}
impl<'w, T: Resource> PureEnv for Res<'w, T> {}

impl<'w, 's, D, F> sealed::Sealed for Query<'w, 's, D, F>
where
    D: bevy::ecs::query::ReadOnlyQueryData + 'static,
    F: bevy::ecs::query::QueryFilter + 'static,
{
}
impl<'w, 's, D, F> PureEnv for Query<'w, 's, D, F>
where
    D: bevy::ecs::query::ReadOnlyQueryData + 'static,
    F: bevy::ecs::query::QueryFilter + 'static,
{
}

macro_rules! impl_pure_env_tuple {
    ($($p:ident),+) => {
        impl<$($p: PureEnv),+> sealed::Sealed for ($($p,)+) {}
        impl<$($p: PureEnv),+> PureEnv for ($($p,)+) {}
    };
}
impl_pure_env_tuple!(P0);
impl_pure_env_tuple!(P0, P1);
impl_pure_env_tuple!(P0, P1, P2);

// ---------------------------------------------------------------------------
// The reducer + the drain
// ---------------------------------------------------------------------------

/// A reducer over model `M` reading the pure environment `E`. Implemented for any
/// `FnMut(&mut M, M::Msg, &EnvItem) -> Cmd`. The env is passed **by shared reference** so one
/// fetched env item can be reused across the many per-message folds in a single drain pass
/// (`SystemParam` items aren't `Clone`). Implementors are `Send + Sync + 'static`.
pub trait Reducer<M: Model, E: PureEnv>: Send + Sync + 'static {
    fn fold(&mut self, model: &mut M, msg: M::Msg, env: &SystemParamItem<E>) -> Cmd<M::Msg>;
}

impl<M, E, F> Reducer<M, E> for F
where
    M: Model,
    E: PureEnv,
    F: FnMut(&mut M, M::Msg, &SystemParamItem<E>) -> Cmd<M::Msg> + Send + Sync + 'static,
{
    fn fold(&mut self, model: &mut M, msg: M::Msg, env: &SystemParamItem<E>) -> Cmd<M::Msg> {
        self(model, msg, env)
    }
}

/// Per-model bind-stage instrument: counts entities observed `Changed<M>` this frame into
/// [`MvuWorkCounters::binds_fired`]. Installed per model type by [`MvuAppExt::add_model`] in
/// [`MvuSet::Bind`]. Under `set_if_neq`, an idempotent fold leaves `Changed<M>` untripped, so
/// this counts `0` — the proof the no-op does not cascade to derived views.
///
/// `Option<ResMut<_>>` so the system is inert when the counter resource is unregistered
/// (the `RenderWorkCounters` idiom — no missing-resource skip, no registration drift).
#[cfg(not(debug_assertions))]
fn count_binds<M: Model>(
    changed: Query<(), Changed<M>>,
    mut counters: Option<ResMut<MvuWorkCounters>>,
) {
    let n = changed.iter().count() as u64;
    if n == 0 {
        return;
    }
    if let Some(c) = counters.as_deref_mut() {
        c.binds_fired += n;
    }
}

/// Debug build of [`count_binds`](count_binds): the SAME `binds_fired` counting **plus** the
/// §7.5 write-outside-the-funnel audit, folded into this one already-registered system so the
/// auditor adds **no new system** (and therefore no system entity — entity-id-keyed layout
/// snapshots are undisturbed) and stays perfectly perf-neutral in release (where this whole
/// arm compiles out and the plain counter above is used instead).
///
/// The `Changed<M>` filter keeps the iteration cost identical to release — only entities whose
/// model changed this frame are visited; `Ref<M>` gives those the `last_changed()` access the
/// audit needs. For each changed entity:
/// - **no stamp** ⇒ the entity has never folded through the funnel ⇒ the change is a
///   **spawn-time seed** (authored initial state, §10) ⇒ skipped, not flagged;
/// - **stamp present, `last_changed() == stamp`** ⇒ the funnel was the writer ⇒ clean;
/// - **stamp present, `last_changed() != stamp`** ⇒ a raw write bypassed the drain ⇒ a
///   [`FunnelViolation`] (`warn!`-logged + pushed to [`FunnelAuditLog`]).
///
/// Runs in [`MvuSet::Bind`] (after every fold this frame — the early leaf/machine drains, the
/// late drain, and the inline AT seam), so a same-frame raw write before `Bind` is caught
/// immediately and one after it on the next frame — the eventual-detection a debug diagnostic
/// wants. `Option<Res/ResMut<_>>` keep it inert when the audit resources are unregistered.
#[cfg(debug_assertions)]
fn count_binds<M: Model>(
    changed: Query<(Entity, Ref<M>), Changed<M>>,
    stamps: Option<Res<FunnelWriteStamps>>,
    mut audit: Option<ResMut<FunnelAuditLog>>,
    mut counters: Option<ResMut<MvuWorkCounters>>,
) {
    let mut n = 0u64;
    for (entity, model) in &changed {
        n += 1;
        // The §7.5 single-writer audit (stamps present only when the resource is registered).
        let Some(stamps) = stamps.as_deref() else {
            continue;
        };
        // No stamp ⇒ never folded ⇒ a spawn-time seed (§10), not a violation.
        let Some(stamp) = stamps.0.get(&entity).copied() else {
            continue;
        };
        // A funnel fold sets the stamp to the SAME tick it writes `M`; any later raw write
        // bumps `M`'s changed tick off the stamp.
        if model.last_changed() != stamp {
            let type_name = core::any::type_name::<M>();
            warn!(
                "MVU single-writer violation (spec §7.5): {entity:?}'s model `{type_name}` \
                 was written OUTSIDE the funnel (changed tick {:?} ≠ last funnel-fold tick \
                 {stamp:?}). Route the mutation through `enqueue` instead of a raw ECS write.",
                model.last_changed(),
            );
            if let Some(audit) = audit.as_deref_mut() {
                audit.violations.push(FunnelViolation { entity, type_name });
            }
        }
    }
    if n == 0 {
        return;
    }
    if let Some(c) = counters.as_deref_mut() {
        c.binds_fired += n;
    }
}

// ---------------------------------------------------------------------------
// The shared per-message fold body (structural single-source — spec §5.3)
// ---------------------------------------------------------------------------

/// **The single per-message fold body** — the ONE place a model `M` is folded onto its
/// committed value, shared by BOTH the env-free batch drain ([`MvuAppExt::add_reducer_in_set`])
/// and the AT-seam [`fold_one_inline`] (spec §5.3 "structural single-source"). Folds exactly
/// the one supplied `msg` onto `target`'s `M`, runs the returned [`Cmd`] stack
/// **run-to-completion on a LOCAL queue** (each [`Cmd::Emit`] re-records + re-folds in this
/// same call, never into [`Messages`]), records every folded msg (gated by [`RecordSession`]),
/// and bumps [`MvuWorkCounters`] — byte-for-byte the same record/counter behavior the batch
/// drain has, so the seam and the drain cannot diverge in what they log or count.
///
/// Returns whether the **first** fold changed the model (`set_if_neq` tripped) — the
/// `live-component-synchronous` signal the AT seam returns to the caller.
///
/// Operates on `&mut World` (not `SystemParam`s) precisely so the exclusive batch drain and
/// the exclusive AT-dispatch seam can both call it. `reducer` is taken by `&mut F` so the
/// batch drain can re-use its captured (possibly stateful) reducer across the inbox loop; the
/// AT seam passes a bare-`fn` (env-free, capture-free) reducer via [`fold_one_inline`].
fn fold_one_with<M, F>(world: &mut World, target: Entity, msg: M::Msg, reducer: &mut F) -> bool
where
    M: Model,
    F: FnMut(&mut M, M::Msg) -> Cmd<M::Msg>,
{
    // Resolve the actor's logical identity once (constant for this entity across the
    // Emit cascade); `UNRESOLVED` when the entity carries no `LogicalId`.
    let lid = world
        .get::<LogicalId>(target)
        .copied()
        .unwrap_or(LogicalId::UNRESOLVED);

    // Local work queue: the supplied msg is `User`; `Cmd::Emit` re-folds are `Folded`. Run
    // the Cmd stack to completion HERE (never into `Messages`) — identical to the batch
    // drain's per-message body (spec §5.3 / §5.6).
    let mut work: VecDeque<(M::Msg, Origin)> = VecDeque::new();
    work.push_back((msg, Origin::User));

    let mut first_changed = false;
    let mut first = true;

    while let Some((msg, origin)) = work.pop_front() {
        // Record tap — gated by `RecordSession` (free under `Off`: `tick_seq` is `None`, so
        // the `AppTypeRegistry`/`MsgLog` are never even touched — "default-OFF pays zero",
        // SYNTHESIS H7). Records intent even for a dead-letter; counts only a real fold.
        // For the AT seam this is what makes the action a RECORDED Msg in the one global
        // sequence (closes L5).
        let seq = world.resource_mut::<RecordSession>().tick_seq();
        if let Some(seq) = seq {
            let registry = world.resource::<AppTypeRegistry>().clone();
            let registry = registry.read();
            world
                .resource_mut::<MsgLog>()
                .record(seq, &registry, lid, &msg, origin);
            if let Some(mut c) = world.get_resource_mut::<MvuWorkCounters>() {
                c.messages_recorded += 1;
            }
        }

        // === The load-bearing rule (set_if_neq, spec §2) =========================
        // Fold onto a CLONE, then commit via `set_if_neq`: `Changed<M>` (and the bind →
        // re-extract cascade) trips ONLY on a real change. An idempotent fold leaves
        // change-detection untripped — `models_mutated == 0`.
        let Some(mut model) = world.get_mut::<M>(target) else {
            // Dead-letter: target gone (despawned). Intent was recorded above; it is NOT
            // counted as a fold (the batch-drain convention).
            continue;
        };
        let mut next = (*model).clone();
        let cmd = reducer(&mut next, msg);
        let changed = model.set_if_neq(next);
        // The tick `set_if_neq` just wrote `M` at — captured for the §7.5 auditor stamp
        // below (debug only). Reading it here keeps the `model` borrow minimal.
        #[cfg(debug_assertions)]
        let funnel_tick = model.last_changed();
        // ==========================================================================
        if first {
            first_changed = changed;
            first = false;
        }

        // Apply effects: `None` / `Emit` (re-fold, run-to-completion) / `Batch`.
        let mut emits = 0u64;
        let mut stack = vec![cmd];
        while let Some(c) = stack.pop() {
            match c {
                Cmd::None => {}
                Cmd::Emit(m) => {
                    work.push_back((m, Origin::Folded));
                    emits += 1;
                }
                Cmd::Batch(v) => stack.extend(v),
            }
        }

        if let Some(mut c) = world.get_resource_mut::<MvuWorkCounters>() {
            c.drain_folds += 1;
            if changed {
                c.models_mutated += 1;
            }
            c.emits_refolded += emits;
        }

        // §7.5 single-writer auditor stamp (debug only): record THIS funnel write's tick so
        // the bind-stage `count_binds` audit can distinguish it from a raw write-outside-the-
        // funnel. Only a real change advances `M`'s changed tick, so only stamp when
        // `changed`. Writes a separate resource (no archetype move, no `MvuWorkCounters`
        // touch), so it cannot perturb any work counter. Compiled out of release/bench.
        #[cfg(debug_assertions)]
        if changed && let Some(mut stamps) = world.get_resource_mut::<FunnelWriteStamps>() {
            stamps.0.insert(target, funnel_tick);
        }
    }
    first_changed
}

/// **The AT synchronous act-then-observe substrate primitive** (spec §5.3): fold exactly one
/// `msg` onto `target`'s model `M` **inline + synchronously** through the SAME
/// `fold_one_with` body the batch drain uses, **bypassing the inbox**. Returns whether the
/// live model changed (`set_if_neq` tripped).
///
/// The contract this serves is **"live-component-synchronous + perform-then-update"** (spec
/// §5.1): the live `M` mutates the instant this returns (so an act-then-observe seam reading
/// the component directly — with no `app.update()` — sees the change); a bind that PROJECTS
/// `M` onto a *consumed* a11y component (e.g. `bind_menu_model`: `MenuModel.open →
/// A11yExpanded`) refreshes the cached a11y tree on the next `app.update()`.
///
/// **The reducer is a bare `fn` pointer, NOT an `FnMut` closure** (spec §5.5, soundness
/// reviewer): a closure could capture a `Res` snapshot at registration that diverges on a
/// fresh-process replay; a bare `fn` cannot capture, so the seam path is determinism-safe **by
/// type**. [`toggle_reducer`] and a machine's reducer (e.g.
/// `menu_reducer`) are free fns and already qualify. An AT set-verb folds the **absolute** verb
/// (`Open`/`Close`, never `Toggle`), so it emits nothing here in practice — but the `Emit`
/// run-to-completion machinery is shared with the drain regardless (substrate generality).
pub fn fold_one_inline<M: Model>(
    world: &mut World,
    target: Entity,
    msg: M::Msg,
    reducer: fn(&mut M, M::Msg) -> Cmd<M::Msg>,
) -> bool {
    let mut reducer = reducer;
    fold_one_with::<M, _>(world, target, msg, &mut reducer)
}

/// App-builder extension for wiring an MVU model.
pub trait MvuAppExt {
    /// Register a model type: its inbox message + `Msg` reflect registration + the per-model
    /// bind-stage counter system.
    fn add_model<M: Model>(&mut self) -> &mut Self;

    /// Register an **environment-free** reducer `fn(&mut M, M::Msg) -> Cmd` + install its drain
    /// in the default late [`MvuSet::Drain`]. No `World`/`Commands`/sibling access in scope ⇒
    /// purity is structural.
    fn add_reducer<M, F>(&mut self, reducer: F) -> &mut Self
    where
        M: Model,
        F: FnMut(&mut M, M::Msg) -> Cmd<M::Msg> + Send + Sync + 'static;

    /// Register an **environment-reading** reducer + install its drain in the default late
    /// [`MvuSet::Drain`]. The env type `E` is given by turbofish
    /// (`add_reducer_env::<Counter, MyEnv, _>(update)`); the reducer reads it via `&MyEnv`.
    /// `E: PureEnv` is the compile-time purity gate.
    fn add_reducer_env<M, E, R>(&mut self, reducer: R) -> &mut Self
    where
        M: Model,
        E: PureEnv + 'static,
        R: Reducer<M, E>;

    /// Like [`add_reducer`](Self::add_reducer), but installs the drain in a **caller-chosen**
    /// [`SystemSet`] instead of the default late [`MvuSet::Drain`].
    ///
    /// The substrate primitive behind the early-window model (spec §4): a tier whose model must
    /// fold at a *specific* point in the frame supplies its own early window. The toggle **leaf**
    /// uses this to fold `.after(BuiySet::Picking).before(BuiySet::A11yUpdate)` — BEFORE the a11y
    /// tree is built — so an AT-driver click reflects in the tree the SAME frame (the late drain
    /// would lag it one frame). MACHINES keep the late default (their model feeds
    /// a *later* bind, so the late slot is correct for them). The caller owns ordering `set` into
    /// the frame (e.g. via `configure_sets` + a pinned `ApplyDeferred` between enqueue and drain).
    fn add_reducer_in_set<M, F>(&mut self, reducer: F, set: impl SystemSet) -> &mut Self
    where
        M: Model,
        F: FnMut(&mut M, M::Msg) -> Cmd<M::Msg> + Send + Sync + 'static;

    /// The environment-reading form of [`add_reducer_in_set`](Self::add_reducer_in_set): an
    /// env-reading reducer whose drain is installed in a caller-chosen [`SystemSet`].
    fn add_reducer_env_in_set<M, E, R>(&mut self, reducer: R, set: impl SystemSet) -> &mut Self
    where
        M: Model,
        E: PureEnv + 'static,
        R: Reducer<M, E>;
}

impl MvuAppExt for App {
    fn add_model<M: Model>(&mut self) -> &mut Self {
        self.add_message::<Envelope<M>>();
        self.register_type::<M::Msg>();
        self.add_systems(Update, count_binds::<M>.in_set(MvuSet::Bind));

        // Register this model's replay applier: turn a logged RON entry of its
        // `Msg` type back into an `Envelope<M>` on the inbox, so replay re-folds it
        // through the registered drain. Keyed by the `Msg` type path (the same key the
        // log stores), so `crate::replay` can dispatch a unified stream generically.
        self.init_resource::<ReplayRegistry>();
        let type_path = <M::Msg as TypePath>::type_path().to_string();
        let applier: ReplayApplier = Box::new(|world: &mut World, target: Entity, ron: &str| {
            let registry = world.resource::<AppTypeRegistry>().clone();
            let msg = {
                let registry = registry.read();
                let registration = registry
                    .get(TypeId::of::<M::Msg>())
                    .expect("the model's Msg type is registered (add_model registers it)");
                let mut de =
                    ron::Deserializer::from_str(ron).expect("a logged Msg entry is valid RON");
                let dynamic = TypedReflectDeserializer::new(registration, &registry)
                    .deserialize(&mut de)
                    .expect("reflect-deserialize the logged Msg");
                <M::Msg as FromReflect>::from_reflect(dynamic.as_ref())
                    .expect("from_reflect the logged Msg")
            };
            world
                .resource_mut::<Messages<Envelope<M>>>()
                .write(Envelope { target, msg });
        });
        self.world_mut()
            .resource_mut::<ReplayRegistry>()
            .appliers
            .insert(type_path, applier);
        self
    }

    fn add_reducer<M, F>(&mut self, reducer: F) -> &mut Self
    where
        M: Model,
        F: FnMut(&mut M, M::Msg) -> Cmd<M::Msg> + Send + Sync + 'static,
    {
        // The default late slot (machine tier).
        self.add_reducer_in_set::<M, F>(reducer, MvuSet::Drain)
    }

    fn add_reducer_env<M, E, R>(&mut self, reducer: R) -> &mut Self
    where
        M: Model,
        E: PureEnv + 'static,
        R: Reducer<M, E>,
    {
        // The default late slot (machine tier).
        self.add_reducer_env_in_set::<M, E, R>(reducer, MvuSet::Drain)
    }

    fn add_reducer_in_set<M, F>(&mut self, mut reducer: F, set: impl SystemSet) -> &mut Self
    where
        M: Model,
        F: FnMut(&mut M, M::Msg) -> Cmd<M::Msg> + Send + Sync + 'static,
    {
        // STRUCTURAL SINGLE-SOURCE (spec §5.3): the env-free batch drain is an EXCLUSIVE
        // `&mut World` system that drains the inbox and folds each message through the
        // SHARED per-message body [`fold_one_with`] — the SAME body the AT seam's
        // [`fold_one_inline`] uses. The batch loop is re-expressed as
        // `for env in inbox.drain() { fold_one_with(world, env.target, env.msg, reducer) }`,
        // so the seam and the drain cannot diverge in what they record or count. (The
        // env-READING variant keeps its own `SystemParam` drain in
        // [`add_reducer_env_in_set`](Self::add_reducer_env_in_set) — env reducers are
        // unaffected; only this env-free path shares the inline body.)
        let drain = move |world: &mut World| {
            // Drain the whole inbox out first (releasing the `Messages` borrow) so each
            // `fold_one_with` can take `&mut World`. The drain is the sole reader, so
            // draining everything pending each frame is correct.
            let msgs: Vec<Envelope<M>> = {
                let Some(mut inbox) = world.get_resource_mut::<Messages<Envelope<M>>>() else {
                    return;
                };
                inbox.drain().collect()
            };
            for Envelope { target, msg } in msgs {
                fold_one_with::<M, _>(world, target, msg, &mut reducer);
            }
        };
        self.add_systems(Update, drain.in_set(set));
        self
    }

    fn add_reducer_env_in_set<M, E, R>(&mut self, mut reducer: R, set: impl SystemSet) -> &mut Self
    where
        M: Model,
        E: PureEnv + 'static,
        R: Reducer<M, E>,
    {
        let drain = move |mut inbox: MessageReader<Envelope<M>>,
                          mut models: Query<&mut M>,
                          ids: Query<&LogicalId>,
                          mut log: ResMut<MsgLog>,
                          mut session: ResMut<RecordSession>,
                          registry: Res<AppTypeRegistry>,
                          mut counters: Option<ResMut<MvuWorkCounters>>,
                          env: StaticSystemParam<E>| {
            // Snapshot the inbox so we can fold `Emit`s run-to-completion without holding the
            // reader borrow. Each work item carries its [`Origin`] (spec §7.2): the initial
            // inbox items are `User`; `Cmd::Emit` push_backs are `Folded`. This is contained
            // to the drain + `LoggedEntry` + `MsgLog::record` — `Envelope`/`enqueue` are
            // untouched (the origin is drain-local provenance, not part of the transport).
            let mut work: VecDeque<(Envelope<M>, Origin)> = inbox
                .read()
                .cloned()
                .map(|env| (env, Origin::User))
                .collect();
            if work.is_empty() {
                return;
            }
            // Fetch the env once; reuse `&env` across every fold this pass.
            let env = env.into_inner();
            let registry = registry.read();
            while let Some((Envelope { target, msg }, origin)) = work.pop_front() {
                let lid = ids.get(target).copied().unwrap_or(LogicalId::UNRESOLVED);
                // Record tap, stamped with the SHARED global `seq` (the unified session) + the
                // item's `origin`. Free under `RecordMode::Off` — `tick_seq` returns `None`,
                // so nothing is built, serialized, or stored. Records intent even for a
                // dead-letter; counts only a real write.
                if let Some(seq) = session.tick_seq() {
                    log.record(seq, &registry, lid, &msg, origin);
                    if let Some(c) = counters.as_deref_mut() {
                        c.messages_recorded += 1;
                    }
                }
                let Ok(mut model) = models.get_mut(target) else {
                    // Dead-letter: target gone (despawned). The drain drops it (the
                    // everywhere-safe path) and does NOT count it as a fold; replay's typed
                    // `DeadLetter` is where a genuine miss surfaces loudly (spec §7.4).
                    continue;
                };
                // === The load-bearing rule (set_if_neq, spec §2) =====================
                // Fold onto a CLONE, then commit via `set_if_neq`: `Changed<M>` is tripped
                // (and the bind → re-extract cascade fires) ONLY on a real change. An
                // idempotent fold leaves change-detection untripped — `models_mutated == 0`.
                let mut next = (*model).clone();
                let cmd = reducer.fold(&mut next, msg, &env);
                let changed = model.set_if_neq(next);
                // ======================================================================
                // Apply effects: `None` / `Emit` (re-fold) / `Batch` only.
                let mut emits = 0u64;
                let mut stack = vec![cmd];
                while let Some(c) = stack.pop() {
                    match c {
                        Cmd::None => {}
                        Cmd::Emit(m) => {
                            // A re-fold within the same drain pass — provenance `Folded`.
                            work.push_back((Envelope { target, msg: m }, Origin::Folded));
                            emits += 1;
                        }
                        Cmd::Batch(v) => stack.extend(v),
                    }
                }
                // One counter touch per folded message (drain_folds + the conditional fields).
                if let Some(c) = counters.as_deref_mut() {
                    c.drain_folds += 1;
                    if changed {
                        c.models_mutated += 1;
                    }
                    c.emits_refolded += emits;
                }
            }
        };
        self.add_systems(Update, drain.in_set(set));
        self
    }
}

// ---------------------------------------------------------------------------
// One-call model wiring (the model type is INFERRED from the reducer)
// ---------------------------------------------------------------------------

/// Lets [`MvuModelExt::mvu_model`] infer the model type from the reducer's `&mut M` argument:
/// the marker `fn(&mut M, M::Msg)` carries `M` into the trait reference (the same trick Bevy
/// uses for `IntoSystem`), so no turbofish is needed.
pub trait IntoModelReducer<Marker>: Send + Sync + 'static {
    type Model: Model;
    fn install(self, app: &mut App);
}

impl<M, F> IntoModelReducer<fn(&mut M, M::Msg)> for F
where
    M: Model,
    F: FnMut(&mut M, M::Msg) -> Cmd<M::Msg> + Send + Sync + 'static,
{
    type Model = M;
    fn install(self, app: &mut App) {
        app.register_type::<M>(); // model state (snapshots / replay)
        app.add_model::<M>(); // inbox + Msg reflect + bind counter
        app.add_reducer::<M, _>(self); // the drain
    }
}

/// A chainable handle returned by [`MvuModelExt::mvu_model`], carrying the (inferred) model
/// type so opt-in extras need no turbofish.
pub struct ModelWiring<'a, M: Model> {
    app: &'a mut App,
    _marker: std::marker::PhantomData<M>,
}

impl<'a, M: Model> ModelWiring<'a, M> {
    /// Escape back to the `App` to continue chaining other builder calls.
    pub fn app(self) -> &'a mut App {
        self.app
    }
    // TODO(roadmap): `with_routing()` returns once `mvu::routing` lands (the routing phase):
    // it will `self.app.add_routing::<M>()` (press → nearest-ancestor model) and return `self`.
}

/// One-call model wiring.
pub trait MvuModelExt {
    /// Register a model + its reducer in ONE call — `register_type` + `add_model` +
    /// `add_reducer` — with the model type **inferred from the reducer** (no turbofish).
    fn mvu_model<Marker, F>(&mut self, reducer: F) -> ModelWiring<'_, F::Model>
    where
        F: IntoModelReducer<Marker>,
        Marker: 'static;
}

impl MvuModelExt for App {
    fn mvu_model<Marker, F>(&mut self, reducer: F) -> ModelWiring<'_, F::Model>
    where
        F: IntoModelReducer<Marker>,
        Marker: 'static,
    {
        reducer.install(self);
        ModelWiring {
            app: self,
            _marker: std::marker::PhantomData,
        }
    }
}

// ---------------------------------------------------------------------------
// Plugin + per-frame counter reset
// ---------------------------------------------------------------------------

/// Zero [`MvuWorkCounters`] BEFORE any MVU work each frame (the `RenderWorkCounters`
/// overwrite convention). A *single* reset before all per-type drains gives correct aggregate
/// semantics: every drain accumulates into the freshly-zeroed counter, so an idle frame (no
/// drain work) reads all-0.
///
/// **Anchored `.before(BuiySet::Input)`** (spec §5.6, soundness reviewer) — NOT merely
/// `.before(BuiySet::Picking)`. The AT seam folds INLINE inside `route_action_requests`, which
/// runs in [`BuiySet::Input`] and bumps these counters via [`fold_one_inline`]; resetting only
/// `.before(Picking)` left the reset unordered against `Input`, so it could run *after* the
/// inline fold and erase its bumps. `Input` precedes `Picking` (and `A11yUpdate`/`Render`), so
/// this anchor also precedes BOTH the early caller-chosen leaf/machine drains (the early-window
/// model, spec §4 — `.after(Picking).before(A11yUpdate)`) and the late [`MvuSet::Drain`].
/// Nothing writes the counters before `Input`.
fn reset_mvu_counters(mut counters: ResMut<MvuWorkCounters>) {
    *counters = MvuWorkCounters::default();
}

/// Installs the MVU runtime scaffolding: the log + work-counter resources, the [`LogicalId`]
/// reflect registration, the [`MvuSet`] chain folded into [`crate::BuiySet`], the pinned
/// `ApplyDeferred`, and the per-frame counter reset.
///
/// **Kept SEPARATE from `CorePlugin` (decision: cheap-when-absent).** The base core is
/// untouched when MVU is not used — no inbox GC, no drain, no counter reset run unless this
/// plugin is added. (A future revision may compose it from `CorePlugin`; keeping it opt-in
/// isolates the MVU cost so the perf gate can measure it.)
///
/// The chain orders itself `.after(BuiySet::A11yUpdate).before(BuiySet::Render)`. When
/// `CorePlugin` is absent (minimal test apps), those `BuiySet` anchors exist only as empty
/// ordering sets — harmless; the MVU chain still runs in `Update`.
pub struct MvuCorePlugin;

impl Plugin for MvuCorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MsgLog>()
            .init_resource::<RecordSession>()
            .init_resource::<ReplayRegistry>()
            .init_resource::<MvuWorkCounters>()
            .register_type::<LogicalId>();

        // §7.5 single-writer auditor state (debug builds only — compiled out of release/bench,
        // so the perf gate is untouched). The audit is folded into the per-model `count_binds`
        // system (see `add_model`).
        #[cfg(debug_assertions)]
        app.init_resource::<FunnelWriteStamps>()
            .init_resource::<FunnelAuditLog>();

        // The MVU sub-chain, pinned late in `Update` between a11y and render (spec §3).
        app.configure_sets(
            Update,
            (MvuSet::Enqueue, MvuSet::Drain, MvuSet::Bind)
                .chain()
                .after(BuiySet::A11yUpdate)
                .before(BuiySet::Render),
        );

        // Explicit sync point: flush `enqueue`'s deferred `commands.queue`
        // writes into `Messages<Envelope<M>>` BEFORE the drain reads the inbox, so an enqueue
        // from an `Enqueue` system is drained in the SAME frame (one designed frame of latency).
        app.add_systems(
            Update,
            ApplyDeferred.after(MvuSet::Enqueue).before(MvuSet::Drain),
        );

        // Per-frame counter reset BEFORE any MVU work (overwrite convention). Anchored
        // `.before(BuiySet::Input)` (spec §5.6): the AT seam folds INLINE in
        // `route_action_requests` (in `BuiySet::Input`) and bumps `MvuWorkCounters` via
        // `fold_one_inline`, so the reset must precede `Input` — not merely `Picking` — or its
        // bumps would be reset away. `Input` precedes `Picking`/`A11yUpdate`/`Render`, so this
        // also precedes BOTH the EARLY caller-chosen leaf/machine drains (the early-window
        // model, spec §4 — `.after(Picking).before(A11yUpdate)`) and the late `MvuSet::Drain`.
        // Nothing writes
        // `MvuWorkCounters` before `Input`.
        app.add_systems(Update, reset_mvu_counters.before(BuiySet::Input));
    }
}
