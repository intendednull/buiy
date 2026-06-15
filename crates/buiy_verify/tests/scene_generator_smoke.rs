//! Task 2.7 RED — the `arb_scene` generator's structural bounds + the `realize`
//! round-trip. Plain `proptest!`/`assert!` (NOT a snapshot) so it cannot pass
//! vacuously: it pins that the generator terminates within its depth budget and
//! that `realize` threads every node through the production paint path exactly
//! once (invariants.md § "Scene generators").

use std::collections::HashSet;

use buiy_verify::invariant::{Scene, SceneNode, SceneParams, arb_scene, realize};
use proptest::prelude::*;

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
        prop_assert!(
            scene_depth(&scene) <= p.max_depth,
            "depth {} exceeds max_depth {}",
            scene_depth(&scene),
            p.max_depth,
        );

        // Structural hard cap: at most `Σ breadth^level` nodes per root tree.
        let per_root: usize = (0..p.max_depth)
            .map(|l| (p.max_breadth as usize).pow(l))
            .sum();
        let forest_cap = per_root.saturating_mul(2); // up to 2 roots
        prop_assert!(
            scene_node_count(&scene) <= forest_cap,
            "node count {} exceeds structural cap {}",
            scene_node_count(&scene),
            forest_cap,
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
