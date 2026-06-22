//! Task 2.7 RED — the `arb_scene` generator's structural bounds + the `realize`
//! round-trip. Plain `proptest!`/`assert!` (NOT a snapshot) so it cannot pass
//! vacuously: it pins that the generator terminates within its depth budget and
//! that `realize` threads every node through the production paint path exactly
//! once (invariants.md § "Scene generators").

use std::collections::HashSet;

use buiy_verify::invariant::{Scene, SceneNode, SceneParams, arb_scene, realize};
use proptest::prelude::*;
use proptest::strategy::ValueTree;
use proptest::test_runner::TestRunner;

/// Max nesting depth of a scene forest (a single root is depth 1).
fn scene_depth(scene: &Scene) -> u32 {
    scene.roots.iter().map(node_depth).max().unwrap_or(0)
}

fn node_depth(node: &SceneNode) -> u32 {
    1 + node.children.iter().map(node_depth).max().unwrap_or(0)
}

/// Total node count of a scene forest.
fn scene_node_count(scene: &Scene) -> usize {
    scene.roots.iter().map(subtree_count).sum()
}

fn subtree_count(node: &SceneNode) -> usize {
    1 + node.children.iter().map(subtree_count).sum::<usize>()
}

/// Collect every node name in the scene (the generator renames pre-order).
fn scene_names(scene: &Scene) -> Vec<String> {
    let mut out = Vec::new();
    for root in &scene.roots {
        collect_names(root, &mut out);
    }
    out
}

fn collect_names(node: &SceneNode, out: &mut Vec<String>) {
    out.push(node.name.clone());
    for child in &node.children {
        collect_names(child, out);
    }
}

/// Visit every node of a scene forest (pre-order), running `f` on each.
fn for_each_node(scene: &Scene, mut f: impl FnMut(&SceneNode)) {
    fn rec(node: &SceneNode, f: &mut impl FnMut(&SceneNode)) {
        f(node);
        for child in &node.children {
            rec(child, f);
        }
    }
    for root in &scene.roots {
        rec(root, &mut f);
    }
}

/// Audit #38 (T4.6): generator axis-distinctness self-test. The Tier-3 invariant
/// proptests only *pass* — they say nothing about whether the generator actually
/// EXERCISES each axis. If a strategy silently regressed to a constant (e.g.
/// `arb_position_kind` always `Static`, `arb_transform` always identity, sizes
/// pinned to one box — the "constant box" the audit named), every invariant would
/// still pass over a strictly smaller domain and nothing would catch the lost
/// coverage. This self-test draws a fixed-seed sample of scenes and asserts each
/// generated axis takes **≥ 2 distinct values** across the sample — direct
/// evidence the domain is non-degenerate. It is deterministic (a pinned RNG seed)
/// so it never flakes and a regression reproduces exactly.
#[test]
fn generator_exercises_every_axis_with_distinct_values() {
    // TestRunner::deterministic() PINS the RNG seed, so the SAMPLES draws below
    // are reproducible (no OS-entropy flake; a failure reproduces exactly) —
    // unlike Config::default(), whose rng_seed is Random. (`Config.cases` is
    // irrelevant here: the SAMPLES loop drives `new_tree` directly, not
    // `TestRunner::run`, so the draw count is SAMPLES, not `cases`.)
    let mut runner = TestRunner::deterministic();
    let strategy = arb_scene(SceneParams::default());

    // Distinct-value accumulators, one per axis. Enums key on their `Debug`
    // string so the self-test need not import every axis type; the numeric axes
    // key on bit patterns (NaN-free by construction here).
    let mut position_kinds: HashSet<String> = HashSet::new();
    let mut z_indices: HashSet<Option<i32>> = HashSet::new();
    let mut isolations: HashSet<bool> = HashSet::new();
    let mut top_layers: HashSet<String> = HashSet::new();
    let mut transform_identity: HashSet<bool> = HashSet::new();
    let mut sizes: HashSet<(u32, u32)> = HashSet::new();
    let mut background_present: HashSet<bool> = HashSet::new();

    const SAMPLES: usize = 400;
    for _ in 0..SAMPLES {
        let tree = strategy
            .new_tree(&mut runner)
            .expect("arb_scene produces a value");
        let scene = tree.current();
        for_each_node(&scene, |node| {
            position_kinds.insert(format!("{:?}", node.position_kind));
            z_indices.insert(node.z_index);
            isolations.insert(node.isolation);
            top_layers.insert(format!("{:?}", node.top_layer));
            transform_identity.insert(node.transform.is_identity());
            sizes.insert((node.size.0.to_bits(), node.size.1.to_bits()));
            background_present.insert(node.background.is_some());
        });
    }

    // Each axis must show variation — a degenerate (constant) strategy fails here.
    assert!(
        position_kinds.len() >= 2,
        "position_kind is pinned to a single value {position_kinds:?} — the \
         generator must reach both Static and a positioned kind (testing-audit #13)"
    );
    assert!(
        z_indices.len() >= 2,
        "z_index never varies ({z_indices:?}) — auto + explicit z must both occur"
    );
    assert!(
        isolations.len() >= 2,
        "isolation is constant ({isolations:?}) — both Isolate and Auto must occur"
    );
    assert!(
        top_layers.len() >= 2,
        "top_layer is pinned ({top_layers:?}) — an escaping variant must occur \
         (else `top_layer_dominates` runs over an empty top layer)"
    );
    assert!(
        transform_identity.len() >= 2,
        "transform is always identity (or always non-identity) — both must occur \
         so the context-forming transform branch is exercised"
    );
    assert!(
        sizes.len() >= 2,
        "size is a single constant box ({} distinct) — the generator must vary the \
         box geometry, not pin it (the audit's 'constant box' regression)",
        sizes.len()
    );
    assert!(
        background_present.len() >= 2,
        "background presence never varies ({background_present:?}) — both painted \
         and color-less nodes must occur"
    );
    // The positioned kinds specifically: at least one NON-static kind must appear
    // (a stricter check than ≥2 distinct, since {Static, <one other>} satisfies
    // the count but we also want to know a positioned node is genuinely reachable
    // for the tier-2 paint class — testing-audit #13).
    assert!(
        position_kinds.iter().any(|k| k != "Static"),
        "no positioned kind ever generated — tier-2 (positioned, auto-z) is \
         unreachable, the very gap #13 widened the generator to close"
    );
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, ..ProptestConfig::default() })]

    /// `prop_recursive` HARD-caps recursion depth at `max_depth` (proptest
    /// guarantee), so a generated scene never nests deeper than the budget. The
    /// node count is a soft statistical target, so we bound it only by the true
    /// structural maximum (full `max_breadth`-ary tree of `max_depth` levels per
    /// root), never the soft `max_nodes`.
    #[test]
    fn arb_scene_respects_bounds(scene in arb_scene(SceneParams::default())) {
        let p = SceneParams::default();
        // `prop_recursive(depth, …)` HARD-caps the number of recursive
        // combinator LEVELS at `depth`; the base (non-recursive) leaf adds the
        // final level, so a realized tree nests at most `max_depth + 1` deep (a
        // single-node scene is depth 1). This is still a hard bound, never the
        // soft node count.
        let max_levels = p.max_depth + 1;
        prop_assert!(
            scene_depth(&scene) <= max_levels,
            "depth {} exceeds max_depth+1 = {}",
            scene_depth(&scene),
            max_levels,
        );

        // Structural hard cap: at most `Σ breadth^level` nodes over the
        // `max_depth + 1` levels of the single root tree.
        let cap: usize = (0..max_levels)
            .map(|l| (p.max_breadth as usize).pow(l))
            .sum();
        prop_assert!(
            scene_node_count(&scene) <= cap,
            "node count {} exceeds structural cap {}",
            scene_node_count(&scene),
            cap,
        );

        // Names are unique (the pre-order rename) and cover `n0..nK`.
        let names = scene_names(&scene);
        let unique: HashSet<&String> = names.iter().collect();
        prop_assert_eq!(unique.len(), names.len(), "node names must be unique");
    }

    /// `realize` threads a scene through the production CPU paint assembly into
    /// a flat node list whose entities are EXACTLY the scene's nodes, each once
    /// (the round-trip: no node dropped, none duplicated).
    #[test]
    fn realize_round_trips_every_node(scene in arb_scene(SceneParams::default())) {
        let nodes = realize(&scene);
        let painted: HashSet<bevy::prelude::Entity> =
            nodes.nodes.iter().map(|n| n.entity).collect();
        prop_assert_eq!(
            painted.len(),
            nodes.nodes.len(),
            "no entity is painted twice"
        );
        prop_assert_eq!(
            nodes.nodes.len(),
            scene_node_count(&scene),
            "every scene node is realized exactly once"
        );
    }
}
