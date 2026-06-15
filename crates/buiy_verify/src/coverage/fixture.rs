//! The fixture as single source of truth (coverage.md § same).
//!
//! A [`Fixture`] is a BSN scene factory plus a `(name, state)` identity — the
//! catalog row, authored once. It is the same `fn(&mut App)` shape every other
//! tier consumes (reftest, golden, snapshot), so a fixture is enrollable
//! everywhere with no adapter. Adding **one** fixture file auto-enrolls it
//! across **every** tier by construction (the decisive coverage property).
//!
//! Fixtures register via the [`fixture!`] macro, which emits an
//! [`inventory::submit!`] so [`catalog`] enumerates every fixture with **zero
//! edits to a central list**. The `inventory` link-time registry is the typed
//! `&[Fixture]` the GPU / invariant tiers iterate (they are not file-driven);
//! the two `insta` snapshot tiers additionally use `glob!` over the fixture
//! directory, and `verify_catalog_matches_glob` asserts the two views never
//! drift.

use bevy::app::App;

/// One catalog row: a widget × state scene factory, authored once.
///
/// `spawn` MUST spawn a `Camera2d` (so a capture-capable app has a view) and
/// MUST tag the widget root with a [`Name`](bevy::prelude::Name) — every dump
/// keys entities by `Name`, never by `Entity` bits (snapshot.md). One fixture =
/// one widget × state; the `state` axis (resting / hover / focus / pressed /
/// disabled) is **per-fixture** (one file per state), encoded by spawning the
/// widget already in that state.
#[derive(Clone, Copy)]
pub struct Fixture {
    /// Stable identity. Becomes the `widget` stem component and the `Name` the
    /// root is tagged with. `lower-kebab`, unique within the corpus.
    pub name: &'static str,
    /// Per-fixture interaction state (`resting`, `hover`, …). The
    /// `widget × state` pair is the corpus key (unique).
    pub state: &'static str,
    /// Spawns the scene into a deterministic app.
    pub spawn: fn(&mut App),
}

impl std::fmt::Debug for Fixture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Fixture")
            .field("name", &self.name)
            .field("state", &self.state)
            .finish_non_exhaustive()
    }
}

inventory::collect!(Fixture);

/// Every registered [`Fixture`], collected once via `inventory` link-time
/// registration. A new `fixture!` file enrolls with **zero edits** to any
/// central list. Iteration order is registration order (link order), which is
/// not guaranteed stable across builds — callers that need a stable order
/// (stems, dumps) sort by `(name, state)`, which is the corpus key.
pub fn catalog() -> impl Iterator<Item = &'static Fixture> {
    inventory::iter::<Fixture>.into_iter()
}

/// The catalog as a `(name, state)`-sorted `Vec`, the order every tier iterates
/// for determinism (the raw `inventory` order is link-order, not stable).
pub fn sorted_catalog() -> Vec<&'static Fixture> {
    let mut v: Vec<&'static Fixture> = catalog().collect();
    v.sort_by_key(|f| (f.name, f.state));
    v
}

/// Register a [`Fixture`] in the `inventory` catalog. The body is a
/// `fn(&mut App)` spawning the scene (it MUST spawn a `Camera2d` and `Name`-tag
/// the root). Emitting an `inventory::submit!` is what makes the fixture
/// enroll across every tier with no central-list edit.
///
/// ```ignore
/// fixture! {
///     name  = "button",
///     state = "resting",
///     spawn = |app| { /* spawn Camera2d + Name-tagged button */ },
/// }
/// ```
#[macro_export]
macro_rules! fixture {
    (name = $name:expr, state = $state:expr, spawn = $spawn:expr $(,)?) => {
        ::inventory::submit! {
            $crate::coverage::fixture::Fixture {
                name: $name,
                state: $state,
                spawn: $spawn,
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_is_nonempty_and_unique() {
        let fixtures = sorted_catalog();
        assert!(
            !fixtures.is_empty(),
            "the catalog must hold at least one fixture (the button)"
        );
        // (name, state) is the corpus key — it must be unique.
        let mut keys: Vec<(&str, &str)> = fixtures.iter().map(|f| (f.name, f.state)).collect();
        let before = keys.len();
        keys.sort_unstable();
        keys.dedup();
        assert_eq!(
            before,
            keys.len(),
            "fixture (name, state) keys must be unique"
        );
    }

    #[test]
    fn button_resting_is_registered() {
        assert!(
            sorted_catalog()
                .iter()
                .any(|f| f.name == "button" && f.state == "resting"),
            "the button/resting fixture must be registered"
        );
    }
}
