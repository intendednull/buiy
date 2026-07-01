//! `buiy_view` — the view-authoring surface ("safer V") for Buiy.
//!
//! The whole app-author surface is **`Model` + `enum Msg` + `fn update` +
//! `fn view`** — nothing else. No `route_*`, no `bind_*`, no `Changed<Model>`
//! system. This is what deletes DX-2 (no hand-written declarative view) and DX-3
//! (no hand-written `OnPress → Model` routing).
//!
//! ```no_run
//! use bevy::prelude::*;
//! use buiy_core::mvu::{Cmd, Model};
//! use buiy_view::{Element, Space, BuiyViewAppExt, button, column, row, text};
//!
//! #[derive(Component, Default, Clone, PartialEq, Reflect)]
//! #[reflect(Component)]
//! struct Counter { count: i32 }
//! impl Model for Counter { type Msg = Msg; }
//!
//! #[derive(Clone, Debug, PartialEq, Reflect)]
//! enum Msg { Inc, Dec, Reset }
//!
//! fn update(s: &mut Counter, m: Msg) -> Cmd<Msg> {
//!     match m {
//!         Msg::Inc => s.count += 1,
//!         Msg::Dec => s.count -= 1,
//!         Msg::Reset => s.count = 0,
//!     }
//!     Cmd::none()
//! }
//!
//! fn view(s: &Counter) -> Element<Msg> {
//!     column![
//!         text!("Count: {}", s.count).size(48.0),
//!         row![
//!             button("-").on_press(Msg::Dec),
//!             button("+").on_press(Msg::Inc),
//!             button("Reset").on_press_maybe((s.count != 0).then_some(Msg::Reset)),
//!         ].gap(Space::Sm),
//!     ]
//!     .gap(Space::Md)
//!     .padding(Space::Xl)
//!     .align_center()
//! }
//! ```
//!
//! ## Crate boundary (spec §1)
//!
//! `buiy_view` depends on `buiy_core` (the MVU substrate + the decomposed
//! layout / render / theme / interaction / text components) and `buiy_widgets`
//! (the `Button` constructor) — **not** on `buiy` or `buiy_bsn`. `buiy` depends
//! on `buiy_view`, never the reverse, so there is no dependency cycle. The
//! `buiy::view` sub-prelude (a distinct import path from `buiy::prelude`,
//! because these `Element`-returning builders collide name-for-name with the
//! `bsn!` scene-fns) is wired in a later wave.
//!
//! PR1 / FW1 ships the positional reconciler over the four kinds the Counter
//! needs (`column!` / `row!` / `text` / `button`) with the structural refines
//! the prototype deferred (decomposed-style patching, reconcile-before-layout,
//! the internal `ViewSlot`, drift-only writes). Keyed lists, the editor bridge,
//! conditionals, and `map` composition arrive in FW2–FW4.

mod app;
mod element;
mod reconcile;
mod router;
mod tokens;

pub use app::{BuiyViewAppExt, IntoViewReducer, MODEL_LID, ViewSet};
pub use element::{Element, Kind, button, text};
pub use tokens::{Color, Radius, Space};
// `column!` / `row!` / `text!` are `#[macro_export]`ed at the crate root by
// `element` — reachable as `buiy_view::{column, row, text}` (the macro `text!`
// and the fn `text` share one path, distinct namespaces).

use bevy::prelude::*;
use buiy_core::mvu::Model;

use crate::router::PressAction;

// ---------------------------------------------------------------------------
// Test / capture helpers (drive the REAL press path from a harness).
// ---------------------------------------------------------------------------

/// Find the interactive entity whose press enqueues `want` (so a harness can
/// synthesize a real `OnPress` on it). `None` if no *enabled* handler matches.
pub fn find_press_target<M: Model>(world: &mut World, want: &M::Msg) -> Option<Entity>
where
    M::Msg: PartialEq,
{
    let mut q = world.query::<(Entity, &PressAction<M>)>();
    q.iter(world).find(|(_, a)| &a.msg == want).map(|(e, _)| e)
}

/// Whether `entity` currently carries an enabled press handler (proves the
/// reconciler attached/detached a handler as the model changed).
pub fn has_press_handler<M: Model>(world: &mut World, entity: Entity) -> bool {
    world.get::<PressAction<M>>(entity).is_some()
}
