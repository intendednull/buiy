//! AccessKit integration. Phase 0 builds an in-memory snapshot and exposes
//! an `A11yTreeBuilder` that can be serialized for snapshot tests; the real
//! `accesskit_winit::Adapter` wiring per-window happens once Bevy windows
//! are introduced (Task 13, BuiyPlugin).
//!
//! See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.6 and
//! accessibility.md § 3.11 (decomposed components per #17644).

use crate::{BuiySet, focus::Focusable};
use accesskit::Toggled;
use bevy::prelude::*;

pub mod adapter;
pub mod states;
pub mod translate;

pub use adapter::AccessKitAdapterPlugin;
pub use states::{A11yDisabled, A11yExpanded, A11yHidden, A11yModal, A11ySelected, A11yToggled};
pub use translate::{build_tree_update, to_accesskit_node};

/// Decomposed AccessKit role component.
///
/// Marked `#[non_exhaustive]` because the v0.x full ARIA taxonomy
/// expansion (38+ roles, see foundation spec accessibility.md § 3.11)
/// will add variants pre-1.0. External matches must include a wildcard
/// arm; the in-tree `buiy_verify::a11y::role_to_str` is structured this
/// way already.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[reflect(Component)]
#[non_exhaustive]
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
    // Widget-catalog prerequisite roles (inert until the widget-catalog phase —
    // no widget emits these yet). See foundation spec accessibility.md § 3.11.
    Checkbox,
    Switch,
    Slider,
    TextInput,
    MultilineTextInput,
    Region,
    Group,
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

/// One node in the tree as Buiy sees it, translated into an `accesskit::Node`
/// by the `to_accesskit_node` derive fold. Decoupled from winit so it can be
/// snapshot-tested without a real window.
///
/// Beyond the P0 flat fields (`entity`/`role`/`name`/`description`/`focusable`),
/// each decomposed state component projects into one `Option`/`bool` field here.
/// The view stores the *projected* accesskit value (e.g. `Option<Toggled>`), so
/// the fold arm is a trivial one-to-one with the setter. **Absence (`None` /
/// `false`) ⇒ the setter is not called** (semantic-tree.md §§2,5). P1b widens
/// this further (`parent`/`children`); later P1a tasks add the value/text/live/
/// orientation/has-popup/relation/scroll fields.
#[derive(Clone, Debug, PartialEq)]
pub struct A11yNodeView {
    // Existing (P0):
    pub entity: Entity,
    pub role: A11yRole,
    pub name: String,
    pub description: String,
    pub focusable: bool,
    // Decomposed state projections (P1a, first batch):
    /// Tri-state toggle, projected from [`A11yToggled`]. `None` ⇒ not toggleable.
    pub toggled: Option<Toggled>,
    /// Disclosure expanded/collapsed, projected from [`A11yExpanded`].
    pub expanded: Option<bool>,
    /// Selected state, projected from [`A11ySelected`].
    pub selected: Option<bool>,
    /// Disabled flag, projected from the [`A11yDisabled`] marker's presence.
    pub disabled: bool,
    /// Modal flag, projected from the [`A11yModal`] marker's presence.
    pub modal: bool,
    /// Hidden flag, projected from the [`A11yHidden`] marker's presence.
    /// **Carried only in P1a** — no fold arm; P1b consumes it to prune the
    /// node + subtree (semantic-tree.md §7.4).
    pub hidden: bool,
}

impl Default for A11yNodeView {
    fn default() -> Self {
        // `Entity` has no `Default`; the placeholder is the canonical "unset"
        // entity, matching the rest of Bevy's default-entity idiom.
        Self {
            entity: Entity::PLACEHOLDER,
            role: A11yRole::default(),
            name: String::new(),
            description: String::new(),
            focusable: false,
            toggled: None,
            expanded: None,
            selected: None,
            disabled: false,
            modal: false,
            hidden: false,
        }
    }
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
            .register_type::<A11yToggled>()
            .register_type::<A11yExpanded>()
            .register_type::<A11ySelected>()
            .register_type::<A11yDisabled>()
            .register_type::<A11yModal>()
            .register_type::<A11yHidden>()
            .init_resource::<A11yTreeBuilder>()
            .add_systems(Update, build_tree.in_set(BuiySet::A11yUpdate));
    }
}

#[allow(clippy::type_complexity)]
pub(crate) fn build_tree(
    mut builder: ResMut<A11yTreeBuilder>,
    q: Query<(
        Entity,
        Option<&A11yRole>,
        Option<&A11yLabel>,
        Option<&A11yDescription>,
        Option<&Focusable>,
        Option<&A11yToggled>,
        Option<&A11yExpanded>,
        Option<&A11ySelected>,
        Option<&A11yDisabled>,
        Option<&A11yModal>,
        Option<&A11yHidden>,
    )>,
) {
    builder.nodes.clear();
    for (
        entity,
        role,
        label,
        desc,
        focusable,
        toggled,
        expanded,
        selected,
        disabled,
        modal,
        hidden,
    ) in q.iter()
    {
        // Skip entities that have no a11y content at all. A decomposed state
        // component is a11y content on its own, so it must keep the node alive.
        let has_state = toggled.is_some()
            || expanded.is_some()
            || selected.is_some()
            || disabled.is_some()
            || modal.is_some()
            || hidden.is_some();
        if role.is_none() && label.is_none() && desc.is_none() && focusable.is_none() && !has_state
        {
            continue;
        }
        builder.nodes.push(A11yNodeView {
            entity,
            role: role.copied().unwrap_or_default(),
            name: label.map(|l| l.0.clone()).unwrap_or_default(),
            description: desc.map(|d| d.0.clone()).unwrap_or_default(),
            focusable: focusable.is_some(),
            // Project each component to its view field (one-to-one with the
            // fold). Wrappers unwrap to their inner accesskit value; markers
            // project to a presence `bool`.
            toggled: toggled.map(|t| t.0),
            expanded: expanded.map(|e| e.0),
            selected: selected.map(|s| s.0),
            disabled: disabled.is_some(),
            modal: modal.is_some(),
            hidden: hidden.is_some(),
        });
    }
}
