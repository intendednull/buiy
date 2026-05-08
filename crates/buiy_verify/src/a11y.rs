//! AccessKit tree snapshot — serializes Buiy's `A11yTreeBuilder` view to
//! stable JSON suitable for golden-file comparison.
//! See: docs/specs/2026-05-07-buiy-foundation/verification.md (CI gate #3).

use buiy_core::a11y::{A11yNodeView, A11yRole};
use serde::Serialize;

#[derive(Serialize)]
struct WireNode<'a> {
    entity: u64,
    role: &'a str,
    name: &'a str,
    description: &'a str,
    focusable: bool,
}

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
    serde_json::to_string(&wire).expect("snapshot serializes")
}

/// Returns `None` if identical, `Some(unified_diff_text)` otherwise.
pub fn diff_snapshots(left: &str, right: &str) -> Option<String> {
    if left == right {
        None
    } else {
        Some(format!("LEFT:\n{}\n\nRIGHT:\n{}\n", left, right))
    }
}
