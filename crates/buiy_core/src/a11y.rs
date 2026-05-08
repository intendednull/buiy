//! AccessKit integration. Phase 0 builds an in-memory snapshot and exposes
//! an `A11yTreeBuilder` that can be serialized for snapshot tests; the real
//! `accesskit_winit::Adapter` wiring per-window happens once Bevy windows
//! are introduced (Task 13, BuiyPlugin).
//!
//! See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.6 and
//! accessibility.md § 3.11 (decomposed components per #17644).

use crate::{BuiySet, focus::Focusable};
use bevy::prelude::*;

/// Decomposed AccessKit role component.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[reflect(Component)]
pub enum A11yRole {
    #[default]
    Generic,
    Button,
    Link,
    Image,
    Text,
    Heading,
    Dialog,
    AlertDialog,
    Tooltip,
    // Phase 0 stops here; full taxonomy is in the foundation spec accessibility.md.
}

/// Decomposed accessible name. ACCNAME 1.2 computation is in `buiy-accessibility-design`;
/// Phase 0 is the literal-string fast path.
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component)]
pub struct A11yLabel(pub String);

/// Decomposed accessible description.
#[derive(Component, Reflect, Clone, Debug, Default)]
#[reflect(Component)]
pub struct A11yDescription(pub String);

/// One node in the tree as Buiy sees it. Will be translated into
/// `accesskit::Node` by the adapter in Task 13. Decoupled here so we can
/// snapshot it without needing a winit window.
#[derive(Clone, Debug, PartialEq)]
pub struct A11yNodeView {
    pub entity: Entity,
    pub role: A11yRole,
    pub name: String,
    pub description: String,
    pub focusable: bool,
}

/// Tree builder: rebuilt each frame from changed components in BuiySet::A11yUpdate.
#[derive(Resource, Default)]
pub struct A11yTreeBuilder {
    nodes: Vec<A11yNodeView>,
}

impl A11yTreeBuilder {
    pub fn snapshot(&self) -> &[A11yNodeView] {
        &self.nodes
    }
}

pub struct A11yPlugin;

impl Plugin for A11yPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<A11yRole>()
            .register_type::<A11yLabel>()
            .register_type::<A11yDescription>()
            .init_resource::<A11yTreeBuilder>()
            .add_systems(Update, build_tree.in_set(BuiySet::A11yUpdate));
    }
}

#[allow(clippy::type_complexity)]
fn build_tree(
    mut builder: ResMut<A11yTreeBuilder>,
    q: Query<(
        Entity,
        Option<&A11yRole>,
        Option<&A11yLabel>,
        Option<&A11yDescription>,
        Option<&Focusable>,
    )>,
) {
    builder.nodes.clear();
    for (entity, role, label, desc, focusable) in q.iter() {
        // Skip entities that have no a11y content at all.
        if role.is_none() && label.is_none() && desc.is_none() && focusable.is_none() {
            continue;
        }
        builder.nodes.push(A11yNodeView {
            entity,
            role: role.copied().unwrap_or_default(),
            name: label.map(|l| l.0.clone()).unwrap_or_default(),
            description: desc.map(|d| d.0.clone()).unwrap_or_default(),
            focusable: focusable.is_some(),
        });
    }
}
