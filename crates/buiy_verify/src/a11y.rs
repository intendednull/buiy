//! AccessKit tree snapshot — serializes Buiy's `A11yTreeBuilder` view to
//! stable JSON suitable for golden-file comparison.
//! See: docs/specs/2026-05-07-buiy-foundation/verification.md (CI gate #3).

use buiy_core::a11y::{A11yNodeView, A11yRole};
use serde::Serialize;

// LINT: Field order here is the snapshot wire format. Do not reorder
// without coordinating golden-file regeneration in every consumer.
#[derive(Serialize)]
struct WireNode<'a> {
    entity: u64,
    role: &'a str,
    name: &'a str,
    description: &'a str,
    focusable: bool,
}

// LINT: Keep this match in sync with `buiy_core::a11y::A11yRole`. When
// the v0.x full ARIA taxonomy expansion lands (38+ roles per
// accessibility.md § 3.11), Rust's exhaustiveness check will force
// new arms — add them here in the same PR.
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
    }
}

pub fn snapshot_tree(nodes: &[A11yNodeView]) -> String {
    let wire: Vec<WireNode> = nodes
        .iter()
        .map(|n| WireNode {
            entity: n.entity.to_bits(),
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
