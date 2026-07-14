//! Tier-3 structural invariant (F6, spec §2.7): **"no transparent top-layer
//! container is a pick occluder."**
//!
//! The invisible-occluder bug class — an invisible-but-pickable box that sits
//! topmost and swallows every click beneath it — has shipped **three times** in
//! project history (the parity campaign's detached modal; the wasm
//! cold-synthetic-click artifact; the prototype's `.fixed().fill().top_layer()`
//! theme toggle). It is invisible to a11y-probe clicks and screenshots (both
//! bypass pick occlusion), so discipline-based gating (remembering to click)
//! kept missing it.
//!
//! This predicate closes the class *structurally*: run it over a fixture catalog
//! and it fails the moment a fixture (re)introduces the class, **independent of
//! anyone remembering to click it**. It is the property the `buiy_view`
//! reconciler's auto-`Pickable::IGNORE` rule upholds — a proptest/fixture-catalog
//! test asserts the two agree (the reconciler makes the class *unwritable*, this
//! invariant proves it stayed unwritable).
//!
//! ## What counts as the class
//!
//! A node is a *transparent top-layer occluder* when ALL of:
//! - it is a real layout node in the **top layer** (`Node` + `Stacking.top_layer
//!   != TopLayer::None`);
//! - it is **not** hidden (`ComputedPaintSkip` absent — a paint-skipped node is
//!   already dropped by the picking backend, so it cannot occlude);
//! - it **paints nothing** the user can see: none of the render-tier fill
//!   components — no `Background` (solid), `BackgroundLayers` (gradient / layered),
//!   `RasterImage` (textured quad), `Text` (glyphs), `Icon` (icon coverage),
//!   `Border` (border band), `Outline` (outline band), or `BoxShadow` (drop
//!   shadow) — a node painting any of these is visibly present, not the
//!   *invisible*-occluder class;
//! - it is **not** `Pickable::IGNORE` (so it still occludes — the bug). A
//!   transparent top-layer node is safe ONLY when it carries `Pickable::IGNORE`
//!   (neither hit-target nor occluder).
//!
//! App-agnostic (it names only `buiy_core` + `bevy` component types), so it runs
//! over any Buiy `World` — a reconciled `buiy_view` tree or a hand-authored
//! retained one.
//!
//! The per-entity class test is NOT duplicated here: it is the canonical
//! [`buiy_core::is_transparent_top_layer_occluder`] predicate, which `buiy_core`'s
//! own debug-only `Last` coherence system also upholds — so the test-time invariant
//! (this sweep) and the run-time fail-loud panic can never disagree about what the
//! class is. This module is the world-sweep wrapper: it collects the offenders into
//! a [`Violation`] so a failing fixture names exactly which box re-introduced the
//! invisible-occluder class.

use bevy::prelude::{Entity, World};

use buiy_core::is_transparent_top_layer_occluder;

use super::predicates::Violation;

/// The Tier-3 invariant (F6, spec §2.7): assert **no transparent top-layer node
/// occludes picks** across `world`. Delegates the per-entity class test to the
/// canonical [`buiy_core::is_transparent_top_layer_occluder`] predicate and returns
/// the offending entities in the `Violation` detail.
///
/// See the [module docs](self) for the exact structural definition.
pub fn no_transparent_top_layer_occluder(world: &World) -> Result<(), Violation> {
    let offenders: Vec<Entity> = world
        .iter_entities()
        .filter(|entity| is_transparent_top_layer_occluder(*entity))
        .map(|entity| entity.id())
        .collect();
    if offenders.is_empty() {
        Ok(())
    } else {
        Err(Violation::new(
            "no_transparent_top_layer_occluder",
            format!(
                "{n} transparent top-layer node(s) occlude picks — a transparent \
                 `.top_layer()` container must be `Pickable::IGNORE` (missing on): {offenders:?}",
                n = offenders.len(),
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::picking::Pickable;
    use bevy::prelude::World;

    use buiy_core::Node;
    use buiy_core::layout::{Stacking, TopLayer};
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::Background;

    /// A `Stacking` promoted to the top layer (the `.top_layer()` lowering).
    fn top_layer() -> Stacking {
        Stacking {
            top_layer: TopLayer::Popover,
            ..Default::default()
        }
    }

    /// RED — a transparent top-layer node with NO `Pickable::IGNORE` is the bug;
    /// the predicate MUST flag it (a predicate that never fails is worthless).
    #[test]
    fn flags_a_transparent_top_layer_occluder() {
        let mut world = World::new();
        let bug = world.spawn((Node, top_layer())).id();
        let err = no_transparent_top_layer_occluder(&world)
            .expect_err("a transparent top-layer node with no IGNORE must be flagged");
        assert_eq!(err.rule, "no_transparent_top_layer_occluder");
        assert!(
            err.detail.contains(&format!("{bug:?}")),
            "the violation names the offending entity, got: {}",
            err.detail,
        );
    }

    /// GREEN — the SAME transparent top-layer node with `Pickable::IGNORE` is safe
    /// (neither hit-target nor occluder). This is the state the reconciler's
    /// auto-ignore produces.
    #[test]
    fn a_transparent_top_layer_ignore_node_is_not_an_occluder() {
        let mut world = World::new();
        world.spawn((Node, top_layer(), Pickable::IGNORE));
        assert!(no_transparent_top_layer_occluder(&world).is_ok());
    }

    /// GREEN — a top-layer node that PAINTS a fill is visibly present (a real
    /// scrim/panel), not the invisible-occluder class, so it may legitimately
    /// occlude.
    #[test]
    fn a_painted_top_layer_node_is_not_the_class() {
        let mut world = World::new();
        world.spawn((
            Node,
            top_layer(),
            Background {
                color: ColorToken::SurfacePrimary,
            },
        ));
        assert!(no_transparent_top_layer_occluder(&world).is_ok());
    }

    /// GREEN — a transparent node NOT in the top layer is at its natural stacking
    /// position (later-painted siblings cover it), so it is out of scope.
    #[test]
    fn a_transparent_in_flow_node_is_out_of_scope() {
        let mut world = World::new();
        world.spawn(Node); // default Stacking ⇒ TopLayer::None
        assert!(no_transparent_top_layer_occluder(&world).is_ok());
    }
}
