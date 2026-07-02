//! MVU substrate bench scenes (the L1 pricer input). Spec §11.
//!
//! Shared by the criterion bench (`benches/mvu.rs`) and the iai-callgrind scaffold
//! (`benches/mvu_iai.rs`) so both drive ONE set of scenes — the same single-home discipline
//! the render harness above follows. Dev-only (this crate is never in a production graph).
//!
//! Scenes:
//! - [`build_mvu_idle_app`] — `n` DISTINCT model TYPES (each one instance), to price the idle
//!   floor's `O(N_model_types)` shape (spec §3/§11: per-type, not per-instance).
//! - [`build_mvu_single_app`] — one model + one instance, with a chosen [`RecordMode`], for the
//!   one-message / fold-storm / record-off-vs-on cases.

use std::time::Duration;

use bevy::ecs::message::Messages;
use bevy::prelude::*;

use buiy_core::Node;
use buiy_core::layout::Style;
use buiy_core::mvu::{
    Cmd, Envelope, LogicalId, Model, MvuAppExt, MvuCorePlugin, MvuSet, RecordMode, RecordSession,
};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{Background, ComputedPaintSkip, SkipReason};

use crate::PipelineHarness;

/// The message all the bench counter types fold (one shared `Msg` keeps the scenes uniform).
#[derive(Clone, Debug, Reflect, PartialEq)]
pub enum CounterMsg {
    Increment,
    Add(i64),
    /// Run-to-completion `Emit` (bump until `value == target`) — the fold-storm-in-one-pass case.
    TickTo(i64),
}

/// Shared reducer logic over a bare `i64` so every generated type folds identically.
fn fold_counter(value: &mut i64, msg: CounterMsg) -> Cmd<CounterMsg> {
    match msg {
        CounterMsg::Increment => *value += 1,
        CounterMsg::Add(n) => *value += n,
        CounterMsg::TickTo(target) => {
            if *value < target {
                *value += 1;
                return Cmd::emit(CounterMsg::TickTo(target));
            }
        }
    }
    Cmd::none()
}

/// Stamp out a distinct counter model TYPE + its reducer. Distinct types are what give the
/// idle floor its `O(N_model_types)` cost (each type has its own inbox + drain).
macro_rules! def_counter {
    ($name:ident, $reducer:ident) => {
        #[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
        #[reflect(Component)]
        pub struct $name {
            pub value: i64,
        }
        impl Model for $name {
            type Msg = CounterMsg;
        }
        pub fn $reducer(c: &mut $name, msg: CounterMsg) -> Cmd<CounterMsg> {
            fold_counter(&mut c.value, msg)
        }
    };
}

def_counter!(C0, update_c0);
def_counter!(C1, update_c1);
def_counter!(C2, update_c2);
def_counter!(C3, update_c3);
def_counter!(C4, update_c4);
def_counter!(C5, update_c5);
def_counter!(C6, update_c6);
def_counter!(C7, update_c7);
def_counter!(C8, update_c8);
def_counter!(C9, update_c9);
def_counter!(C10, update_c10);
def_counter!(C11, update_c11);
def_counter!(C12, update_c12);
def_counter!(C13, update_c13);
def_counter!(C14, update_c14);
def_counter!(C15, update_c15);

/// The largest idle-floor `n` the scene supports (16 distinct model types defined above).
pub const MAX_IDLE_TYPES: usize = 16;

/// Base headless app: `App::new()` + the MVU substrate plugin. No `MinimalPlugins`/`CorePlugin`
/// (the minimal shape — the MVU chain orders against empty `BuiySet` anchors).
fn base_app() -> App {
    let mut app = App::new();
    app.add_plugins(MvuCorePlugin);
    app
}

/// Register one counter model type (reflect + inbox + drain + bind counter) and spawn one
/// instance of it.
fn register_counter<C>(app: &mut App, reducer: fn(&mut C, CounterMsg) -> Cmd<CounterMsg>)
where
    C: Model<Msg = CounterMsg> + Default,
{
    app.register_type::<C>();
    app.add_model::<C>();
    app.add_reducer::<C, _>(reducer);
    app.world_mut().spawn((C::default(), LogicalId(0)));
}

/// An idle scene with `n` DISTINCT model types (1 instance each), settled. `app.update()` then
/// runs `n` empty-inbox drains — the `O(N_model_types)` idle floor the pricer should show flat.
pub fn build_mvu_idle_app(n: usize) -> App {
    assert!(
        n <= MAX_IDLE_TYPES,
        "only {MAX_IDLE_TYPES} counter model types are defined"
    );
    let mut app = base_app();
    // An array of per-type registrations; `take(n)` selects the first n.
    let registrations: [fn(&mut App); MAX_IDLE_TYPES] = [
        |a| register_counter::<C0>(a, update_c0),
        |a| register_counter::<C1>(a, update_c1),
        |a| register_counter::<C2>(a, update_c2),
        |a| register_counter::<C3>(a, update_c3),
        |a| register_counter::<C4>(a, update_c4),
        |a| register_counter::<C5>(a, update_c5),
        |a| register_counter::<C6>(a, update_c6),
        |a| register_counter::<C7>(a, update_c7),
        |a| register_counter::<C8>(a, update_c8),
        |a| register_counter::<C9>(a, update_c9),
        |a| register_counter::<C10>(a, update_c10),
        |a| register_counter::<C11>(a, update_c11),
        |a| register_counter::<C12>(a, update_c12),
        |a| register_counter::<C13>(a, update_c13),
        |a| register_counter::<C14>(a, update_c14),
        |a| register_counter::<C15>(a, update_c15),
    ];
    for register in registrations.iter().take(n) {
        register(&mut app);
    }
    for _ in 0..3 {
        app.update(); // settle spawn ticks so the timed frame is genuinely idle
    }
    app
}

/// A single-model scene (`C0`, one instance) with the chosen [`RecordMode`], settled. Returns
/// the app + the actor entity. Drive folds by [`enqueue_direct`] then `app.update()`.
pub fn build_mvu_single_app(mode: RecordMode) -> (App, Entity) {
    let mut app = base_app();
    app.register_type::<C0>();
    app.add_model::<C0>();
    app.add_reducer::<C0, _>(update_c0);
    // The record switch lives on the shared `RecordSession`, not per-`MsgLog`.
    app.world_mut()
        .resource_mut::<RecordSession>()
        .set_mode(mode);
    let e = app.world_mut().spawn((C0::default(), LogicalId(1))).id();
    for _ in 0..3 {
        app.update();
    }
    (app, e)
}

/// Write a message straight to `C0`'s inbox (the direct-inbox idiom — drives the drain without
/// the `enqueue` command round-trip, keeping the bench's timed region the fold itself).
pub fn enqueue_direct(app: &mut App, target: Entity, msg: CounterMsg) {
    app.world_mut()
        .resource_mut::<Messages<Envelope<C0>>>()
        .write(Envelope::user(target, msg));
}

// ============================================================================
// The L1 perf go/no-go `BlinkLeaf` fixture (spec §11, D11).
//
// A deliberately **can-fail** fixture: a `BlinkMsg::Tick(Duration)` whose payload
// CHANGES every frame is routed through the funnel, but the reducer stores only
// the DERIVED blink phase (a `bool` flipping every ~500ms). `set_if_neq` then
// absorbs the steady-frame Ticks (phase unchanged ⇒ `models_mutated == 0`); only
// a 500ms-bucket crossing flips the phase and mutates. A WRONG reducer that stored
// `now` directly (or bypassed `set_if_neq`) would mutate EVERY frame and FAIL the
// gate — that is the whole point (the v1 gate was tautological under `set_if_neq`).
//
// NOT a production primitive — production caret-blink stays render-prep
// (`text/visual.rs` `write_caret_blink`). This `Tick` lives ONLY in this bench/test
// fixture (the RD2×RD3 scope trap the spec calls out).
// ============================================================================

/// The blink cadence half-period in ms: the derived phase flips on each crossing.
pub const BLINK_PERIOD_MS: u128 = 500;

/// The blink leaf model. Its state is the DERIVED phase only — never `now`. (Storing
/// `now` directly is the can-fail counterfactual the gate exists to catch.)
#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
pub struct BlinkLeaf {
    /// `(now / 500ms) % 2 == 0`. Flips every ~500ms; stable within a bucket.
    pub phase: bool,
}

impl Model for BlinkLeaf {
    type Msg = BlinkMsg;
}

/// The per-frame tick. The `Duration` payload changes EVERY frame; the reducer folds
/// it down to the derived phase only — the genuinely-per-frame message the gate routes
/// through the funnel.
#[derive(Clone, Debug, Reflect, PartialEq)]
pub enum BlinkMsg {
    Tick(Duration),
}

/// Fold `Tick(now)` into the derived phase `(now / 500ms) % 2 == 0`. A steady frame
/// (same 500ms bucket) leaves `phase` unchanged ⇒ `set_if_neq` no-op ⇒
/// `models_mutated == 0`; a bucket crossing flips it ⇒ `models_mutated == 1`.
pub fn blink_reducer(leaf: &mut BlinkLeaf, msg: BlinkMsg) -> Cmd<BlinkMsg> {
    let BlinkMsg::Tick(now) = msg;
    leaf.phase = (now.as_millis() / BLINK_PERIOD_MS).is_multiple_of(2);
    Cmd::none()
}

/// Project the derived phase to a STRUCTURAL change the render extract sees (§11): a
/// [`ComputedPaintSkip`] toggle — the SINGLE per-entity skip source extract reads.
/// `phase == false` inserts the marker (the node vanishes ⇒ extract Full-rebuilds,
/// `node_rebuilds == 1`); `phase == true` removes it (the lift rides
/// `RemovedComponents<ComputedPaintSkip>` ⇒ Full rebuild too). It runs ONLY on
/// `Changed<BlinkLeaf>`, so a steady frame (the `set_if_neq` no-op leaves
/// `Changed<BlinkLeaf>` untripped) fires NO structural op at all (`node_rebuilds == 0`).
/// It never touches `CssVisibility`, so the seed-gated `write_paint_skip` pass stays
/// dormant and never clobbers the marker.
fn project_blink_phase(
    mut commands: Commands,
    changed: Query<(Entity, &BlinkLeaf), Changed<BlinkLeaf>>,
) {
    for (e, leaf) in changed.iter() {
        if leaf.phase {
            commands.entity(e).remove::<ComputedPaintSkip>();
        } else {
            commands.entity(e).insert(ComputedPaintSkip {
                reason: SkipReason::CssHidden,
            });
        }
    }
}

/// Wire the blink leaf model + its projection bind into `app` (the caller has already
/// added [`MvuCorePlugin`]): register the model + [`blink_reducer`] in the default late
/// `MvuSet::Drain`, and the [`project_blink_phase`] bind in `MvuSet::Bind` (after the
/// drain trips `Changed<BlinkLeaf>`, before the frame's extract).
fn wire_blink(app: &mut App) {
    app.register_type::<BlinkLeaf>();
    app.add_model::<BlinkLeaf>();
    app.add_reducer::<BlinkLeaf, _>(blink_reducer);
    app.add_systems(Update, project_blink_phase.in_set(MvuSet::Bind));
}

/// Write a `BlinkMsg` straight to the blink inbox (the direct-inbox idiom).
pub fn enqueue_blink_direct(app: &mut App, target: Entity, msg: BlinkMsg) {
    app.world_mut()
        .resource_mut::<Messages<Envelope<BlinkLeaf>>>()
        .write(Envelope::user(target, msg));
}

/// A MINIMAL headless blink app (no render pipeline) for the iai funnel-cost pricer:
/// `App::new` + [`MvuCorePlugin`] + the blink model, one instance, settled at a steady
/// `phase == true` bucket. Returns the app + the actor entity. Drive a steady tick via
/// [`enqueue_blink_direct`] + `app.update()`.
pub fn build_blink_app(mode: RecordMode) -> (App, Entity) {
    let mut app = base_app();
    wire_blink(&mut app);
    app.world_mut()
        .resource_mut::<RecordSession>()
        .set_mode(mode);
    let e = app
        .world_mut()
        .spawn((BlinkLeaf { phase: true }, LogicalId(2)))
        .id();
    // Settle in the phase==true bucket: now=100ms ⇒ (0/500)%2==0 ⇒ true.
    for _ in 0..3 {
        enqueue_blink_direct(&mut app, e, BlinkMsg::Tick(Duration::from_millis(100)));
        app.update();
    }
    (app, e)
}

/// Build the adapter-free render harness wired with the blink fixture — the home of the
/// §11 HARD gate (`blink_funneled_node_rebuilds_zero`). A `Background` node carrying the
/// `BlinkLeaf` model sits in a padded flex column; `project_blink_phase` toggles its
/// `ComputedPaintSkip` on a phase flip. Returns the harness + the blink node entity. NOT
/// yet settled — the caller runs several [`tick_blink`]s to clear the spawn `Added` ticks
/// first (the `RenderWorkCounters` settle idiom).
pub fn build_blink_render_scene() -> (PipelineHarness, Entity) {
    let mut h = PipelineHarness::new();
    // The render harness's `App` carries the layout+render pipeline but NOT the MVU chain;
    // add it (and the blink model) before the first frame.
    h.app.add_plugins(MvuCorePlugin);
    wire_blink(&mut h.app);

    let node = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(40.0).height_px(20.0),
            Background {
                // Opaque placeholder fill (see `build_flat_bg_scene`): the bench
                // only needs a quad emitted, not a specific color.
                color: ColorToken::Custom(Color::WHITE),
            },
            BlinkLeaf { phase: true },
            LogicalId(1),
        ))
        .id();
    let root = h
        .app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(800.0)
                .padding(8.0)
                .gap_px(2.0),
        ))
        .id();
    h.app.world_mut().entity_mut(root).add_children(&[node]);
    (h, node)
}

/// Enqueue one blink `Tick(now)` and run a full pipeline frame (`app.update()` + extract).
pub fn tick_blink(h: &mut PipelineHarness, target: Entity, now: Duration) {
    enqueue_blink_direct(&mut h.app, target, BlinkMsg::Tick(now));
    h.frame();
}
