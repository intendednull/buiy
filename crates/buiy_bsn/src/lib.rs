//! `buiy_bsn` — the BSN (Bevy Scene Notation) authoring surface for Buiy.
//!
//! BSN authoring shipped upstream in **Bevy 0.19** (PR #23413) inside the
//! `bevy_scene` crate: the [`bsn!`](bevy::scene::bsn) /
//! [`bsn_list!`](bevy::scene::bsn_list) macros and the
//! `Template` / `Scene` machinery. `bsn!` authoring is **compile-time and
//! reflection-free** — a component is authorable as soon as it is
//! `Component + Clone + Default` (the upstream blanket `Template` impl), which
//! every Buiy component already satisfies by construction. So this crate is
//! intentionally **thin**: it adds *no* new authoring syntax and does *not*
//! wrap or re-skin `bsn!`. The macro vocabulary is Bevy/Buiy component types
//! (Rust identifiers), per the dioxus prior-art lesson — "resist HTML
//! cosmetics; component types are Rust identifiers".
//!
//! Its sole job is **ergonomic re-exports**: it surfaces the BSN macros and
//! the spawn extension traits in a focused [`prelude`] so Buiy users reach
//! BSN authoring without taking a direct `bevy_scene` dependency or learning
//! Bevy's prelude layout. Everything here lives in `bevy::scene` (the
//! `bevy_scene` crate, enabled by the workspace `bevy_scene` feature).
//!
//! See: docs/specs/2026-06-18-buiy-bsn-integration-design.md § 4.
//!
//! # Authoring Buiy components in BSN
//!
//! ```
//! # use bevy::prelude::*;
//! # use bevy::scene::ScenePlugin;
//! use buiy_bsn::prelude::*;
//! use buiy_core::render::components::Background;
//! use buiy_core::layout::BoxModel;
//!
//! # let mut app = App::new();
//! # app.add_plugins((bevy::asset::AssetPlugin::default(), ScenePlugin));
//! // Decomposed style components author directly — no `Style` builder in BSN.
//! let scene = bsn! {
//!     Background { color: { buiy_core::render::color::ColorToken::CurrentColor } }
//!     BoxModel { }
//! };
//! app.world_mut().spawn_scene(scene).unwrap();
//! ```
//!
//! ## Why `ScenePlugin` is required
//!
//! [`WorldSceneExt::spawn_scene`](bevy::scene::WorldSceneExt::spawn_scene)
//! resolves the scene through the
//! `Assets<ScenePatch>` registry and reads the `AssetServer`, so a `World`
//! that spawns BSN scenes needs both `AssetPlugin` and `bevy::scene::ScenePlugin`
//! added. (Inline `bsn!` does not load any `.bsn` asset file — that loader is
//! deferred upstream — but the spawn path still routes through the asset
//! registry.)

#![forbid(unsafe_code)]

/// The Buiy BSN authoring prelude.
///
/// Glob-import this (`use buiy_bsn::prelude::*;`) to bring the `bsn!` /
/// `bsn_list!` macros and the scene spawn extension traits into scope. These
/// re-export from `bevy::scene` (the `bevy_scene` crate); they are also folded
/// into `buiy::prelude`, so `use buiy::prelude::*;` brings them in too.
pub mod prelude {
    /// The BSN authoring macros: [`bsn!`](bevy::scene::bsn) builds a `Scene`,
    /// [`bsn_list!`](bevy::scene::bsn_list) builds a `SceneList`.
    pub use bevy::scene::{bsn, bsn_list};

    /// The scene spawn extension traits. `spawn_scene` / `apply_scene` live on
    /// `World`, `Commands`, `EntityWorldMut`, and `EntityCommands` through
    /// these traits.
    pub use bevy::scene::{
        CommandsSceneExt, EntityCommandsSceneExt, EntityWorldMutSceneExt, WorldSceneExt,
    };

    /// The core scene types + the inline-observer / value helpers used inside
    /// `bsn!` literals. `Scene`/`SceneList` are the values the macros expand
    /// to; `on` attaches an entity observer; `template_value` inserts a
    /// component value from an expression.
    pub use bevy::scene::{Scene, SceneList, on, template_value};
}
