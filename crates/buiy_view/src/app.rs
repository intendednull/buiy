//! The `ui(init, update, view)` one-call install (spec §2 / §3 #7) and the
//! reconciler-state resources it inserts.
//!
//! `ui()` does, in one call: `register_type::<M>()` + `add_model::<M>()` +
//! install the reducer (the real MVU wiring); insert the `ViewFn` + `UiRoot`
//! reconciler-state resources; spawn the model entity with a stable
//! [`LogicalId`] + a 2D camera at `Startup`; install the press router
//! ([`MvuSet::Enqueue`]) and the reconciler (in [`ViewSet::Reconcile`], ordered
//! **`.before(BuiySet::Layout)`** — #10).

use std::marker::PhantomData;

use bevy::prelude::*;
use buiy_core::BuiySet;
use buiy_core::mvu::{Cmd, LogicalId, Model, MvuAppExt, MvuSet};

use crate::element::Element;
use crate::reconcile::reconcile;
use crate::router::route_presses;

/// The stable [`LogicalId`] the single `ui()` model carries, so a recorded
/// session replays into a fresh app (the model's identity is the same in both,
/// spec §7.4 — replay resolves entries by `LogicalId`, not raw `Entity`). The
/// reconciler-spawned view children carry NO `LogicalId`: they are pure derived
/// structure, re-derived from the replayed model.
pub const MODEL_LID: u64 = 0;

/// The set the reconciler runs in — ordered **`.before(BuiySet::Layout)`**
/// (spec §3 #10), so a structurally-new node is laid out the same frame it is
/// created (the unlaid-out flash is eliminated). Value patches therefore land
/// one drain later than a `Bind`-time reconcile would — by design, matching
/// retained-mode / Elm / Iced.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ViewSet {
    Reconcile,
}

/// The stored `view` function for model `M` (a plain fn pointer — no capture).
#[derive(Resource)]
pub(crate) struct ViewFn<M: Model> {
    pub(crate) view: fn(&M) -> Element<M::Msg>,
}

/// The retained realization root for model `M`'s view tree.
#[derive(Resource)]
pub(crate) struct UiRoot<M: Model> {
    pub(crate) root: Option<Entity>,
    pub(crate) _pd: PhantomData<M>,
}

/// Lets [`BuiyViewAppExt::ui`] pin the model type `M` from **all three**
/// arguments (`init: M`, the reducer, `view: fn(&M) -> _`) with no turbofish —
/// the `IntoModelReducer` trick (Bevy's `IntoSystem` marker pattern),
/// generalized to the view surface. The marker `fn(&mut M, M::Msg)` carries `M`
/// into the trait reference (spec §3 #7).
pub trait IntoViewReducer<M: Model, Marker>: Send + Sync + 'static {
    fn install_reducer(self, app: &mut App);
}

impl<M, F> IntoViewReducer<M, fn(&mut M, M::Msg)> for F
where
    M: Model,
    F: FnMut(&mut M, M::Msg) -> Cmd<M::Msg> + Send + Sync + 'static,
{
    fn install_reducer(self, app: &mut App) {
        // The env-free late-drain reducer (the real MVU wiring).
        app.add_reducer::<M, F>(self);
    }
}

/// Installs a "safer V" UI: `App::new()...ui(init, update, view)`.
pub trait BuiyViewAppExt {
    /// Spawn the model + a 2D camera, register the reducer (the real MVU
    /// `register_type` + `add_model` + `add_reducer`), and install the press
    /// router + the reconciler — all in one call, with `M` inferred from
    /// `init` + `reducer` + `view` (no turbofish).
    ///
    /// **P1 single-root constraint** (spec §2): `ui()` stamps one *fixed*
    /// [`MODEL_LID`], one `UiRoot`, and one 2D camera, so P1 supports exactly
    /// one `ui()` / one root / one model per app. It must be called on an app
    /// that already carries the Buiy plugins (which provide the MVU
    /// scaffolding + `BuiySet` chain).
    fn ui<M, Marker>(
        &mut self,
        init: M,
        reducer: impl IntoViewReducer<M, Marker>,
        view: fn(&M) -> Element<M::Msg>,
    ) -> &mut Self
    where
        M: Model;
}

impl BuiyViewAppExt for App {
    fn ui<M, Marker>(
        &mut self,
        init: M,
        reducer: impl IntoViewReducer<M, Marker>,
        view: fn(&M) -> Element<M::Msg>,
    ) -> &mut Self
    where
        M: Model,
    {
        // The real MVU wiring (spelled out so the model type is pinned to `M`):
        // reflect + inbox/Msg + bind counter + the late-drain reducer.
        self.register_type::<M>();
        self.add_model::<M>();
        reducer.install_reducer(self);

        self.insert_resource(ViewFn::<M> { view });
        self.insert_resource(UiRoot::<M> {
            root: None,
            _pd: PhantomData,
        });

        // Spawn the model (its own entity, with a stable LogicalId) + a 2D
        // camera at startup. The reconciler materializes the view on frame 1
        // (the model spawns `Changed`).
        self.add_systems(Startup, move |mut commands: Commands| {
            commands.spawn(Camera2d);
            commands.spawn((
                init.clone(),
                LogicalId(MODEL_LID),
                Name::new("buiy-view-model"),
            ));
        });

        // The press router (Enqueue) + the reconciler (before Layout, #10). No
        // app-authored routing (DX-3) and no app-authored `Changed<Model>` bind
        // (DX-2) — the library does both.
        self.add_systems(Update, route_presses::<M>.in_set(MvuSet::Enqueue));
        self.configure_sets(Update, ViewSet::Reconcile.before(BuiySet::Layout));
        self.add_systems(Update, reconcile::<M>.in_set(ViewSet::Reconcile));
        self
    }
}
