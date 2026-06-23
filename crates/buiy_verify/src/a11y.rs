//! AccessKit tree snapshot — serializes Buiy's `A11yTreeBuilder` view to
//! stable JSON suitable for golden-file comparison.
//! See: docs/specs/2026-05-07-buiy-foundation/verification.md (CI gate #3).

use accesskit::{NodeId, TreeId};
use accesskit_consumer::Tree as ConsumerTree;
use bevy::prelude::App;
use buiy_core::a11y::translate::node_id_for;
use buiy_core::a11y::{A11yNodeView, A11yRole, A11yTreeBuilder, build_tree_update};
use serde::Serialize;

// LINT: Field order here is the snapshot wire format. Do not reorder
// without coordinating golden-file regeneration in every consumer.
#[derive(Serialize)]
struct WireNode<'a> {
    // Canonical AccessKit NodeId (= entity.to_bits()+1), set in snapshot_tree;
    // NOT raw entity bits. (A future phase renames this field to `ref`.)
    entity: u64,
    role: &'a str,
    name: &'a str,
    description: &'a str,
    focusable: bool,
}

// LINT: Keep this match in sync with `buiy_core::a11y::A11yRole`.
// `A11yRole` is `#[non_exhaustive]`, so Rust requires a wildcard arm and
// the compiler will *not* surface unhandled variants for us. When new
// roles land in `buiy_core::a11y` (e.g. the v0.x full ARIA taxonomy per
// accessibility.md § 3.11) the unknown stringification below shows up
// in snapshot goldens and PRs touching that file must add the named
// arms in the same PR. The fallback exists so external snapshots stay
// well-formed across version skew, not as a substitute for keeping this
// table current.
fn role_to_str(r: A11yRole) -> &'static str {
    match r {
        A11yRole::Generic => "Generic",
        A11yRole::Button => "Button",
        A11yRole::Link => "Link",
        A11yRole::Image => "Image",
        A11yRole::Text => "Text",
        A11yRole::Heading => "Heading",
        A11yRole::Dialog => "Dialog",
        A11yRole::AlertDialog => "AlertDialog",
        A11yRole::Tooltip => "Tooltip",
        A11yRole::Checkbox => "Checkbox",
        A11yRole::Switch => "Switch",
        A11yRole::Slider => "Slider",
        A11yRole::TextInput => "TextInput",
        A11yRole::MultilineTextInput => "MultilineTextInput",
        A11yRole::Region => "Region",
        A11yRole::Group => "Group",
        _ => "Unknown",
    }
}

pub fn snapshot_tree(nodes: &[A11yNodeView]) -> String {
    let wire: Vec<WireNode> = nodes
        .iter()
        .map(|n| WireNode {
            // Canonical AccessKit ref: node_id_for(entity) = to_bits() + 1.
            // This is the id an inbound ActionRequest's `target` carries, so the
            // snapshot's `entity` field round-trips with `entity_for_node_id`.
            entity: node_id_for(n.entity).0,
            role: role_to_str(n.role),
            name: &n.name,
            description: &n.description,
            focusable: n.focusable,
        })
        .collect();
    // `serde_json::to_string` on a `Vec<WireNode>` cannot fail: WireNode is
    // a fixed-shape struct of `u64 + &str + &str + &str + bool`, and
    // serde_json only errors on map keys that aren't strings, custom
    // Serialize impls that fail, or recursion-limit overruns. None apply
    // here. If WireNode ever grows a non-trivial Serialize, return a
    // `Result` from this fn instead of `unreachable!`.
    match serde_json::to_string(&wire) {
        Ok(s) => s,
        Err(e) => unreachable!("WireNode serialization is infallible by construction; got: {e}"),
    }
}

/// Returns `None` if identical, `Some(diff_text)` otherwise. Phase 0
/// emits a coarse `LEFT:\n…RIGHT:\n…` dump; v0.x will swap to a unified
/// diff via the `similar` crate when the AT-consumer harness (gate #15)
/// lands.
pub fn diff_snapshots(left: &str, right: &str) -> Option<String> {
    if left == right {
        None
    } else {
        Some(format!("LEFT:\n{}\n\nRIGHT:\n{}\n", left, right))
    }
}

// ---------------------------------------------------------------------------
// gate-#3 in-process `accesskit_consumer` read tier
//
// The lowest verification rung for the AccessKit tree (inprocess-api.md §4,
// verification.md "Gate #3"): build the SAME `TreeUpdate` the real adapter
// ships — via `buiy_core`'s production `build_tree_update` fold, NOT a
// test-private shortcut — and feed it into an `accesskit_consumer::Tree`, the
// same consumer an assistive technology drives. Fixtures then read nodes back
// the way an AT does (role / name / state getters), proving the producer →
// consumer round-trip end-to-end with no winit adapter and no GPU.
//
// This helper has ONE home (co-drive §5 SC-4): the P1a state fixtures, the C7
// widget-catalog assertions, and the P1c in-process driver all consume it here,
// so the three never fork a parallel consumer path.
// ---------------------------------------------------------------------------

/// Which projection of the canonical tree a snapshot reads.
///
/// `Unmerged` (the default) is the canonical structural tree — what an AT reads
/// before it self-merges, and the only projection P1a/C7 snapshot. `Merged`
/// (read-time collapse of `A11yMergeChildren` subtrees) is reserved for a later
/// phase; it is accepted here so the [`semantic_tree`] signature is stable, but
/// in this slice both behave identically (no merge components exist yet).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreeView {
    /// Canonical structural tree (the default; most diffable).
    #[default]
    Unmerged,
    /// Read-time projection collapsing `A11yMergeChildren` subtrees. Not yet
    /// distinct from `Unmerged` — reserved for the P1b nesting phase.
    Merged,
}

/// Build the production [`accesskit::TreeUpdate`] for `views` via
/// `buiy_core`'s isolated translate fold and wrap it in an in-process
/// [`accesskit_consumer::Tree`] — the gate-#3 read tier.
///
/// `focused` is the AccessKit `NodeId` of the focused node (use
/// [`node_id_for`]); `None` focuses the
/// synthetic root. Read a node back with [`node_for`] (or, manually,
/// `tree.state().node_by_tree_local_id(node_id_for(entity), TreeId::ROOT)`).
///
/// This is the *same* `TreeUpdate` the real `accesskit_winit::Adapter` consumes
/// (`build_tree_update`), so what the fixture observes is what a live AT would.
///
/// **API note (accesskit_consumer 0.36):** the consumer keys nodes by its own
/// `accesskit_consumer::NodeId` — a `(TreeIndex, LocalNodeId)` pair distinct
/// from the producer's `accesskit::NodeId`. A producer-side id (what
/// [`node_id_for`] returns, what an inbound `ActionRequest.target` carries) is
/// resolved with `node_by_tree_local_id(id, TreeId::ROOT)`, NOT `node_by_id`
/// (which takes the internal pair). The single-tree-per-window model means
/// `TreeId::ROOT` is always the tree id (`build_tree_update` sets it).
pub fn consume(views: &[A11yNodeView], focused: Option<NodeId>) -> ConsumerTree {
    let update = build_tree_update(views, focused);
    // `is_host_focused = true`: in a headless fixture the Buiy "window" is the
    // focused host, so the consumer applies the update's `focus` directly.
    ConsumerTree::new(update, true)
}

/// Resolve a producer-side [`accesskit::NodeId`] (from
/// [`node_id_for`]) to its
/// [`accesskit_consumer::Node`] in `tree`, or `None` if the producer never
/// emitted it.
///
/// Wraps the 0.36 `node_by_tree_local_id(id, TreeId::ROOT)` lookup so fixtures
/// address a node by the same id an inbound `ActionRequest.target` carries
/// without repeating the `TreeId::ROOT` boilerplate or tripping on the
/// `node_by_id`-vs-`node_by_tree_local_id` distinction (see [`consume`]).
pub fn node_for(tree: &ConsumerTree, id: NodeId) -> Option<accesskit_consumer::Node<'_>> {
    tree.state().node_by_tree_local_id(id, TreeId::ROOT)
}

/// Snapshot a running [`App`]'s AccessKit tree through the in-process consumer,
/// as an insta-friendly stable string (one line per node: `role  name`).
///
/// Runs the **production** translate path: it reads the `A11yTreeBuilder`
/// resource the `build_tree` system populates each frame, feeds the views
/// through [`consume`], and serializes each node *from the consumer view* — the
/// same nodes a real AT reads. The caller is expected to have driven at least
/// one `app.update()` so the builder reflects the current world; this fn does
/// not tick the schedule (so the caller controls when the frame settles).
///
/// State is intentionally minimal in this slice: P0 carries only role + name +
/// focusable, so the line is `role  name`. As the P1a decomposed state
/// components land, this serializer grows a present-only `state` projection
/// (toggled/selected/value/…) read through the consumer getters — the line
/// format is additive, so existing snapshots only gain fields.
pub fn semantic_tree(app: &mut App, view: TreeView) -> String {
    // `Merged` is accepted but not yet distinct (no merge components exist).
    let _ = view;
    let views: Vec<A11yNodeView> = app
        .world()
        .resource::<A11yTreeBuilder>()
        .snapshot()
        .to_vec();
    let tree = consume(&views, None);

    let mut lines: Vec<String> = Vec::with_capacity(views.len());
    for v in &views {
        // Read role + name back THROUGH the consumer, not off the view — this is
        // what makes it a consumer-tier assertion (role-implied defaults and
        // relation resolution would surface here too once they exist).
        let (role, name) = match node_for(&tree, node_id_for(v.entity)) {
            Some(node) => (role_to_str(v.role), node.label().unwrap_or_default()),
            // A view with no consumer node is a producer/consumer divergence;
            // surface it in the snapshot rather than silently dropping the row.
            None => ("<missing-in-consumer>", String::new()),
        };
        lines.push(format!("{role}  {name}"));
    }
    lines.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Hardcoded list of every Phase 0 `A11yRole` variant. The test
    /// below checks each maps to a non-`"Unknown"` string. New variants
    /// added to `buiy_core::a11y::A11yRole` MUST also be added here AND
    /// to `role_to_str` — renaming a variant will break this test at
    /// compile time, which is the forcing function. (The reverse
    /// direction — adding to A11yRole without updating role_to_str — is
    /// caught downstream by snapshot golden-file diffs surfacing
    /// `"Unknown"`.)
    const KNOWN_ROLES: &[A11yRole] = &[
        A11yRole::Generic,
        A11yRole::Button,
        A11yRole::Link,
        A11yRole::Image,
        A11yRole::Text,
        A11yRole::Heading,
        A11yRole::Dialog,
        A11yRole::AlertDialog,
        A11yRole::Tooltip,
        A11yRole::Checkbox,
        A11yRole::Switch,
        A11yRole::Slider,
        A11yRole::TextInput,
        A11yRole::MultilineTextInput,
        A11yRole::Region,
        A11yRole::Group,
    ];

    #[test]
    fn role_to_str_handles_every_known_variant() {
        for role in KNOWN_ROLES {
            let s = role_to_str(*role);
            assert_ne!(
                s, "Unknown",
                "role {role:?} stringifies to Unknown — add it to role_to_str",
            );
        }
    }
}
