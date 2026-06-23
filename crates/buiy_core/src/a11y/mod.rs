//! AccessKit integration. Phase 0 builds an in-memory snapshot and exposes
//! an `A11yTreeBuilder` that can be serialized for snapshot tests; the real
//! `accesskit_winit::Adapter` wiring per-window happens once Bevy windows
//! are introduced (Task 13, BuiyPlugin).
//!
//! See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.6 and
//! accessibility.md § 3.11 (decomposed components per #17644).

use crate::{BuiySet, focus::Focusable};
use accesskit::{HasPopup, NodeId, Orientation, Toggled};
use bevy::ecs::query::QueryData;
use bevy::prelude::*;

pub mod adapter;
pub mod relations;
pub mod states;
pub mod translate;

pub use adapter::AccessKitAdapterPlugin;
pub use relations::A11yRelations;
pub use states::{
    A11yDisabled, A11yExpanded, A11yHasPopup, A11yHidden, A11yLive, A11yModal, A11yOrientation,
    A11yPlaceholder, A11ySelected, A11yTextValue, A11yToggled, A11yValue,
};
use translate::node_id_for;
pub use translate::{build_tree_update, resolve_live, to_accesskit_node};

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
    // Live-region roles (P1a batch 2). These imply a live-region policy in
    // `translate::resolve_live` — `Alert` ⇒ Assertive+atomic, `Status` ⇒
    // Polite+atomic, `Log` ⇒ Polite — so an alert/status/log surfaces the right
    // announcement even with no explicit `A11yLive` (semantic-tree.md §5).
    Status,
    Alert,
    Log,
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

/// Scroll geometry for a scroll container, exposed to AT (SC-4 — the single
/// coordinated wire-format change adding scroll to the a11y view; co-drive §5).
///
/// This is a **view-only projection** on [`A11yNodeView`], *not* an ECS
/// component: P1a lands the schema (default `None` everywhere) and the single
/// fold arm so the path is exercised end-to-end and the f64 scroll-setter
/// signatures are confirmed in the isolated fold; **C5 (Wave 4) populates it**
/// on real scroll containers by reading its own scroll component into
/// `build_tree`. C5 adds no competing scroll component to the view.
///
/// AccessKit exposure rides the f64 scroll setters (verified in accesskit
/// 0.24.1, `lib.rs:1971`): `offset` → `set_scroll_x`/`set_scroll_y`; the min
/// is always `0.0` → `set_scroll_x_min`/`set_scroll_y_min`; the max is the
/// overflow `content_extent − viewport_extent` (clamped ≥ 0) →
/// `set_scroll_x_max`/`set_scroll_y_max`.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct A11yScrollView {
    /// Current scroll offset (logical px) → `set_scroll_x`/`set_scroll_y`.
    pub offset: Vec2,
    /// Total scrollable content size (logical px). The scroll max is
    /// `content_extent − viewport_extent`.
    pub content_extent: Vec2,
    /// Visible viewport size (logical px).
    pub viewport_extent: Vec2,
    /// `true` iff `content_extent` exceeds `viewport_extent` on either axis.
    /// Carried for AT/consumer use; the fold derives the per-axis max directly
    /// from the extents, so this flag is informational in P1a.
    pub scrollable: bool,
}

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
    // Decomposed state projections (P1a, second batch):
    /// Valued range, projected from [`A11yValue`]. `None` ⇒ not a valued range.
    pub value: Option<A11yValue>,
    /// Single-line text value, projected from [`A11yTextValue`]. `None` ⇒ none.
    pub text_value: Option<String>,
    /// Placeholder text, projected from [`A11yPlaceholder`]. `None` ⇒ none.
    pub placeholder: Option<String>,
    /// Control orientation, projected from [`A11yOrientation`]. `None` ⇒ unset.
    pub orientation: Option<Orientation>,
    /// Popup kind, projected from [`A11yHasPopup`]. `None` ⇒ no popup.
    pub has_popup: Option<HasPopup>,
    /// Explicit live-region policy, projected from [`A11yLive`]. `None` ⇒ no
    /// explicit policy; [`resolve_live`] may still derive one from the role.
    pub live: Option<A11yLive>,
    // Relations resolved to `NodeId` at build time (Task 13). [`A11yRelations`]
    // stores `Entity`; `build_tree` resolves the **four wired** refs through
    // `node_id_for` so `Entity` never leaks past this seam (semantic-tree.md §3).
    // The four carried-but-unwired relation fields (`owns`/`flow_to`/`details`/
    // `error_message`) have no projection here and no fold arm (co-drive §3.2).
    /// Nodes that label this one, resolved → `set_labelled_by`. Empty ⇒ unset.
    pub labelled_by: Vec<NodeId>,
    /// Nodes that describe this one, resolved → `set_described_by`. Empty ⇒ unset.
    pub described_by: Vec<NodeId>,
    /// Nodes this one controls, resolved → `set_controls`. Empty ⇒ unset.
    pub controls: Vec<NodeId>,
    /// Active descendant of a composite widget, resolved → `set_active_descendant`.
    pub active_descendant: Option<NodeId>,
    /// Scroll geometry (SC-4), projected from [`A11yScrollView`]. `None` ⇒ not a
    /// scroll container (no scroll setter fires). **C5 populates this** in Wave 4;
    /// P1a lands the schema + the single fold arm with `None` everywhere.
    pub scroll: Option<A11yScrollView>,
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
            value: None,
            text_value: None,
            placeholder: None,
            orientation: None,
            has_popup: None,
            live: None,
            labelled_by: Vec::new(),
            described_by: Vec::new(),
            controls: Vec::new(),
            active_descendant: None,
            scroll: None,
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
            .register_type::<A11yValue>()
            .register_type::<A11yTextValue>()
            .register_type::<A11yPlaceholder>()
            .register_type::<A11yOrientation>()
            .register_type::<A11yHasPopup>()
            .register_type::<A11yLive>()
            .register_type::<A11yRelations>()
            .init_resource::<A11yTreeBuilder>()
            .add_systems(Update, build_tree.in_set(BuiySet::A11yUpdate));
    }
}

/// The `build_tree` projection: every a11y read for one entity, as a single
/// `#[derive(QueryData)]` struct.
///
/// **Why a struct, not a tuple.** The widened a11y read crossed Bevy's 15-term
/// query-data tuple-arity ceiling (phasing.md §Risks #1 — the wide query). The
/// prior slice worked around it by nesting the second batch in a sub-tuple
/// (which counts as one term); that was always flagged as a stopgap with the
/// `#[derive(QueryData)]` struct named as the clean fix. This is that fix: a
/// flat, named projection that has **no arity ceiling** (each field is one term,
/// but the derive expands them without the tuple cap) and reads far better than a
/// 19-deep nested tuple destructure. New a11y reads are added by adding a field
/// here + a populate line in [`build_tree`], never by re-nesting.
///
/// Read-only (no `#[query_data(mutable)]`): the derive generates the item type
/// `A11yNodeQueryItem<'_, '_>` that [`build_tree`] iterates.
#[derive(QueryData)]
pub(crate) struct A11yNodeQuery {
    entity: Entity,
    role: Option<&'static A11yRole>,
    label: Option<&'static A11yLabel>,
    description: Option<&'static A11yDescription>,
    focusable: Option<&'static Focusable>,
    // State components (one field per decomposed concept).
    toggled: Option<&'static A11yToggled>,
    expanded: Option<&'static A11yExpanded>,
    selected: Option<&'static A11ySelected>,
    disabled: Option<&'static A11yDisabled>,
    modal: Option<&'static A11yModal>,
    hidden: Option<&'static A11yHidden>,
    value: Option<&'static A11yValue>,
    text_value: Option<&'static A11yTextValue>,
    placeholder: Option<&'static A11yPlaceholder>,
    orientation: Option<&'static A11yOrientation>,
    has_popup: Option<&'static A11yHasPopup>,
    live: Option<&'static A11yLive>,
    // Relation refs (resolved to `NodeId` in `build_tree`; only the four wired
    // fields are read — `owns`/`flow_to`/`details`/`error_message` are
    // carried-but-unwired, co-drive §3.2).
    relations: Option<&'static A11yRelations>,
    // SC-4 scroll source: there is **no** scroll component to read in P1a, so no
    // field here yet — `build_tree` writes `scroll: None`. C5 (Wave 4) adds its
    // scroll component as a field here and projects it into the view's `scroll`.
}

pub(crate) fn build_tree(mut builder: ResMut<A11yTreeBuilder>, q: Query<A11yNodeQuery>) {
    builder.nodes.clear();
    for n in q.iter() {
        // Skip entities that have no a11y content at all. A decomposed state
        // component is a11y content on its own, so it must keep the node alive.
        // A relation-only entity is likewise a11y content (it points at others).
        let has_state = n.toggled.is_some()
            || n.expanded.is_some()
            || n.selected.is_some()
            || n.disabled.is_some()
            || n.modal.is_some()
            || n.hidden.is_some()
            || n.value.is_some()
            || n.text_value.is_some()
            || n.placeholder.is_some()
            || n.orientation.is_some()
            || n.has_popup.is_some()
            || n.live.is_some()
            || n.relations.is_some();
        if n.role.is_none()
            && n.label.is_none()
            && n.description.is_none()
            && n.focusable.is_none()
            && !has_state
        {
            continue;
        }
        // Resolve the four WIRED relation refs from `Entity` to `NodeId` here, at
        // build time, so `Entity` never leaks past this seam (semantic-tree.md §3).
        // The four carried-but-unwired fields are read by nothing.
        let (labelled_by, described_by, controls, active_descendant) = match n.relations {
            Some(r) => (
                r.labelled_by.iter().map(|&e| node_id_for(e)).collect(),
                r.described_by.iter().map(|&e| node_id_for(e)).collect(),
                r.controls.iter().map(|&e| node_id_for(e)).collect(),
                r.active_descendant.map(node_id_for),
            ),
            None => (Vec::new(), Vec::new(), Vec::new(), None),
        };
        builder.nodes.push(A11yNodeView {
            entity: n.entity,
            role: n.role.copied().unwrap_or_default(),
            name: n.label.map(|l| l.0.clone()).unwrap_or_default(),
            description: n.description.map(|d| d.0.clone()).unwrap_or_default(),
            focusable: n.focusable.is_some(),
            // Project each component to its view field (one-to-one with the
            // fold). Wrappers unwrap to their inner accesskit value; markers
            // project to a presence `bool`.
            toggled: n.toggled.map(|t| t.0),
            expanded: n.expanded.map(|e| e.0),
            selected: n.selected.map(|s| s.0),
            disabled: n.disabled.is_some(),
            modal: n.modal.is_some(),
            hidden: n.hidden.is_some(),
            // The valued-range and live components clone whole (multi-field); the
            // text/placeholder newtypes project their inner `String`; the two
            // enum-property markers unwrap their accesskit enum.
            value: n.value.cloned(),
            text_value: n.text_value.map(|t| t.0.clone()),
            placeholder: n.placeholder.map(|p| p.0.clone()),
            orientation: n.orientation.map(|o| o.0),
            has_popup: n.has_popup.map(|h| h.0),
            live: n.live.copied(),
            // Relations resolved above (Entity → NodeId at build time).
            labelled_by,
            described_by,
            controls,
            active_descendant,
            // SC-4: no scroll component exists in P1a, so the view carries `None`
            // everywhere. C5 (Wave 4) reads its scroll component into this field.
            scroll: None,
        });
    }
}
