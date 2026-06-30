//! AccessKit integration. Phase 0 builds an in-memory snapshot and exposes
//! an `A11yTreeBuilder` that can be serialized for snapshot tests; the real
//! `accesskit_winit::Adapter` wiring per-window happens once Bevy windows
//! are introduced (Task 13, BuiyPlugin).
//!
//! See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.6 and
//! accessibility.md § 3.11 (decomposed components per #17644).

use crate::{BuiySet, focus::Focusable};
use accesskit::NodeId;
use bevy::ecs::query::QueryData;
use bevy::prelude::*;
use std::collections::{HashMap, HashSet};

pub mod accname;
pub mod action;
pub mod adapter;
pub mod contract;
pub mod inprocess;
pub mod relations;
pub mod states;
pub mod translate;

pub use accname::{AccNameInputs, compute_accessible_name};
pub use action::{
    InlineActionHook, InlineActionRegistry, dispatch_action_request, keyboard_activation,
    route_action_requests, slider_keyboard,
};
pub use adapter::AccessKitAdapterPlugin;
pub use contract::{A11yContract, ActionError, ContractEntry, NotActionableReason, contract_for};
pub use inprocess::{
    NodeState, SemanticNode, SemanticTree, StateQuery, click, expand, focus, get_by_role,
    hide_tooltip, increment, perform, set_value, show_tooltip, snapshot, wait_for,
};
pub use relations::A11yRelations;
pub use states::{
    A11yDisabled, A11yExpanded, A11yHasPopup, A11yHidden, A11yLive, A11yModal, A11yOrientation,
    A11yPlaceholder, A11yReadOnly, A11yScroll, A11ySelected, A11yTextValue, A11yToggled,
    A11yTooltipHost, A11yValue,
};
// Re-export the foreign `accesskit::Toggled` tri-state enum + `Orientation` +
// `HasPopup` so downstream crates (e.g. `buiy_widgets`) can match on
// `A11yToggled.0` / `A11yOrientation.0` and author the slider orientation / the
// menu-button has-popup without taking a direct `accesskit` dependency.
// `A11yToggled`/`A11yOrientation`/`A11yHasPopup` wrap these.
pub use accesskit::{HasPopup, Orientation, Toggled};
// Re-export the foreign `accesskit::Action`/`ActionData` so a downstream crate
// (`buiy_widgets`) can author an [`InlineActionHook`] — whose signature names them —
// without taking a direct `accesskit` dependency (the same convenience as the
// `HasPopup`/`Toggled` re-exports above).
pub use accesskit::{Action, ActionData};
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
    // Menu roles (C5-c, scroll-overlay-modal.md §B.3). A `MenuButton` opens a
    // `Menu` (a popup the button `A11yHasPopup`-advertises + `controls`); the
    // `Menu` is the roving composite container whose `A11yRelations.active_descendant`
    // tracks the active `MenuItem`. These are the canonical APG `menu`/`menuitem`
    // roles (semantic-tree.md / widget-contracts.md); the menu keyboard nav +
    // active-descendant roving is C5-c's container behavior.
    Menu,
    MenuItem,
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
    /// Tooltip-host flag, projected from the [`A11yTooltipHost`] marker's
    /// presence. **No node-property fold arm** (AccessKit has no such property);
    /// it gates only the `{ShowTooltip, HideTooltip}` action advertisement in the
    /// outbound fold (the state-keyed capability, widget-contracts.md §5).
    pub tooltip_host: bool,
    /// Hidden flag, projected from the [`A11yHidden`] marker's presence. P1b's
    /// `build_tree` consumes it to **prune** the entity + its whole subtree, so a
    /// view carrying `hidden: true` is never emitted (it exists on the type only
    /// for the producer-tier "carried but not folded" assertion). The fold has no
    /// arm for it (semantic-tree.md §7.4).
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
    // Real ECS-tree nesting (P1b, semantic-tree.md §7). `build_tree` resolves
    // these from `ChildOf`/`Children` by collapsing presentational wrappers (the
    // `nearest_a11y_ancestor` walk), so they reference only entities that
    // themselves emit a node. `Entity` is fine here — these are internal to the
    // build and resolved to `NodeId` (via `node_id_for`) only inside
    // `build_tree_update`, alongside the synthetic-root parenting.
    /// The node's a11y parent — its nearest a11y-bearing, non-pruned ancestor in
    /// the `ChildOf` hierarchy. `None` ⇒ a top-level node parented to the
    /// synthetic/window root (semantic-tree.md §7.1–7.2).
    pub parent: Option<Entity>,
    /// The node's a11y children in document order — the nearest a11y-bearing,
    /// non-pruned descendants reached by collapsing presentational wrappers
    /// (semantic-tree.md §7.1). Each entry itself emits a node.
    pub children: Vec<Entity>,
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
            tooltip_host: false,
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
            parent: None,
            children: Vec::new(),
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
            .register_type::<A11yReadOnly>()
            .register_type::<A11yModal>()
            .register_type::<A11yTooltipHost>()
            .register_type::<A11yHidden>()
            .register_type::<A11yValue>()
            .register_type::<A11yTextValue>()
            .register_type::<A11yPlaceholder>()
            .register_type::<A11yOrientation>()
            .register_type::<A11yHasPopup>()
            .register_type::<A11yLive>()
            .register_type::<A11yScroll>()
            .register_type::<A11yRelations>()
            .init_resource::<A11yTreeBuilder>()
            // The inline-fold registry (spec §5.4): core OWNS + CONSULTS it (in the
            // generic Expand/Collapse honor, `action.rs`); `buiy_widgets` POPULATES it
            // (the menu hook, idempotent `init_resource` either side). Inited here so the
            // consult site has it even in a core-only harness (it stays empty ⇒ the
            // default direct `A11yExpanded` write is used).
            .init_resource::<InlineActionRegistry>()
            .add_systems(Update, build_tree.in_set(BuiySet::A11yUpdate));

        // P1c-b inbound action router + Button keyboard activation, both in
        // `BuiySet::Input` (action-router.md §7). `route_action_requests` MUST
        // run FIRST-in-Input — before every keyboard/pointer honor system — so a
        // synthesized `OnPress`/focus is consumed the SAME frame and reflected
        // outbound in the (later) `BuiySet::A11yUpdate`. Bevy does NOT order
        // within a set without an explicit constraint, so the router carries
        // explicit `.before(...)` against the CURRENT Input handlers:
        //   - `handle_tab` (focus.rs) — keyboard focus;
        //   - `apply_keyboard_edits` (text) — keyboard editing;
        //   - `keyboard_activation` (below) — the per-role APG keyboard keymap
        //     (Button Enter+Space, Checkbox Space-only, Switch Space+Enter).
        // (The C3 pointer producer `pointer_click_emits_on_press` and
        // `focus_on_click` are observers, not Input-set systems, so they are not
        // — and cannot be — named here; `emit_on_press_on_click` was deleted in
        // C3c.) A `.before` on a system a given harness doesn't schedule (e.g.
        // no `BuiyTextPlugin`) is silently ignored, so the constraint is safe
        // regardless of which sibling plugins are present.
        app.add_systems(
            Update,
            route_action_requests
                .in_set(BuiySet::Input)
                .before(crate::focus::handle_tab)
                .before(crate::text::edit::apply_keyboard_edits)
                .before(keyboard_activation)
                .before(slider_keyboard),
        );
        app.add_systems(Update, keyboard_activation.in_set(BuiySet::Input));
        // The APG slider keyboard control (slice-2): arrows / Home / End /
        // PageUp / PageDown on a focused `Slider` dispatch value verbs
        // (`Increment`/`Decrement`/`SetValue`) through the same router seam an AT
        // drives — NOT the `OnPress` activation sink. An exclusive `&mut World`
        // system (it lowers a value mutation through `dispatch_action_request`),
        // sibling to `keyboard_activation` in `BuiySet::Input`. It gates on a
        // focused Slider before touching the keyboard buffer, so it leaves a
        // non-slider focus's keys for `keyboard_activation`.
        app.add_systems(Update, slider_keyboard.in_set(BuiySet::Input));
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
    tooltip_host: Option<&'static A11yTooltipHost>,
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
    // Real ECS-tree nesting source (P1b, semantic-tree.md §7). `build_tree` reads
    // the layout hierarchy and collapses presentational wrappers via
    // `nearest_a11y_ancestor`, so a node's a11y parent/children skip every entity
    // that carries no a11y content (and the whole `A11yHidden` subtree is pruned).
    child_of: Option<&'static ChildOf>,
    children: Option<&'static Children>,
    // SC-4 scroll source (C5, Wave 4): the scroll container's [`A11yScroll`],
    // populated by `crate::scroll::update_a11y_scroll` from `ScrollOffset` +
    // `ScrollExtent`. `build_tree` projects it into the view's `scroll` field
    // (the P1a-landed `Option<A11yScrollView>` + six-setter fold).
    scroll: Option<&'static A11yScroll>,
}

/// One a11y-bearing entity's read, captured in the `build_tree` scan: the
/// projected view (sans the nesting fields, filled at emit time) plus the
/// `A11yHidden` flag, the `aria-labelledby` targets, and the node's *local* name.
///
/// The `labelled_by` targets and the local name are kept so the deferred ACCNAME
/// arms (semantic-tree.md §6) can resolve other nodes' names without re-querying:
/// `labelledby` reads a target's local name from this map, `contents`
/// concatenates the local names of the node's a11y descendants.
struct NodeMeta {
    view: A11yNodeView,
    /// Carries `A11yHidden` (this entity + its whole subtree are pruned, §7.4).
    hidden: bool,
    /// `aria-labelledby` targets (raw entities) for the deferred ACCNAME arm.
    labelled_by_targets: Vec<Entity>,
    /// The node's name from its *local* sources only (`label > value >
    /// placeholder`) — i.e. ACCNAME without the `labelledby`/`contents` arms.
    /// Used as the contribution when *another* node references this one via
    /// `labelledby`, and as this node's contribution to an ancestor's `contents`.
    local_name: String,
}

/// The raw ECS layout hierarchy of **every** entity, a11y-bearing or not. The
/// nesting walk needs the full `ChildOf`/`Children` graph — not just the a11y
/// subset — so a presentational wrapper (which carries no a11y component, hence
/// no [`NodeMeta`]) can still be *traversed through* when collapsing wrappers
/// (semantic-tree.md §7.1). Keyed by entity; absent entities have no edges.
#[derive(Default)]
struct Hierarchy {
    parent: HashMap<Entity, Entity>,
    children: HashMap<Entity, Vec<Entity>>,
}

impl Hierarchy {
    fn parent_of(&self, e: Entity) -> Option<Entity> {
        self.parent.get(&e).copied()
    }
    fn children_of(&self, e: Entity) -> &[Entity] {
        self.children.get(&e).map(Vec::as_slice).unwrap_or(&[])
    }
}

/// Build the a11y view list with **real ECS-tree nesting** (semantic-tree.md §7).
///
/// The shape of the build:
/// 1. **Hierarchy scan** — read every entity's `ChildOf`/`Children` into a
///    [`Hierarchy`] so the collapse can traverse *through* non-a11y wrappers.
/// 2. **A11y scan** — project every a11y-bearing entity into [`NodeMeta`] (the
///    view + the `A11yHidden` flag + the local name + the labelledby targets).
/// 3. **Prune** (§7.4) — an entity with `A11yHidden`, and its whole subtree,
///    emits no node. The emitted set is `a11y-bearing ∧ not pruned`.
/// 4. **Collapse + emit** (§7.1) — each node's a11y parent is its
///    [`nearest_a11y_ancestor`] (wrappers collapse with no hole); its ordered
///    children are the nearest emitted descendants in document order. The
///    deferred ACCNAME arms (`labelledby`/`contents`, §6) resolve here over the
///    now-known tree.
pub(crate) fn build_tree(mut builder: ResMut<A11yTreeBuilder>, q: Query<A11yNodeQuery>) {
    builder.nodes.clear();

    // --- 1. Hierarchy scan: every entity's raw layout edges. ----------------
    // Read from the SAME query (it carries `child_of`/`children` for every
    // entity, a11y-bearing or not — the `Option<&...>` terms match all
    // archetypes), so a single pass populates both the hierarchy and the a11y
    // meta. A wrapper with no a11y content still lands here and is traversable.
    let mut hierarchy = Hierarchy::default();
    let mut meta: HashMap<Entity, NodeMeta> = HashMap::default();

    for n in q.iter() {
        if let Some(c) = n.child_of {
            hierarchy.parent.insert(n.entity, c.parent());
        }
        if let Some(c) = n.children {
            hierarchy
                .children
                .insert(n.entity, c.iter().collect::<Vec<_>>());
        }

        // --- 2. A11y scan (only a11y-bearing entities enter `meta`). --------
        // Skip entities that have no a11y content at all. A decomposed state
        // component is a11y content on its own, so it must keep the node alive.
        // A relation-only entity is likewise a11y content (it points at others).
        let has_state = n.toggled.is_some()
            || n.expanded.is_some()
            || n.selected.is_some()
            || n.disabled.is_some()
            || n.modal.is_some()
            || n.tooltip_host.is_some()
            || n.hidden.is_some()
            || n.value.is_some()
            || n.text_value.is_some()
            || n.placeholder.is_some()
            || n.orientation.is_some()
            || n.has_popup.is_some()
            || n.live.is_some()
            || n.scroll.is_some()
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
        // The node's *local* name (`label > value > placeholder`) — the ACCNAME
        // arms a node carries directly, with the tree-walk arms (`labelledby`,
        // `contents`) left `None`. This is the contribution another node uses
        // when it references this one (semantic-tree.md §6); the node's *own*
        // final name (which may instead come from labelledby/contents) is
        // computed at emit time once the tree is known.
        let local_name = compute_accessible_name(AccNameInputs {
            labelledby_name: None,
            label: n.label,
            value: n.text_value,
            placeholder: n.placeholder,
            contents_name: None,
        });
        let labelled_by_targets = n
            .relations
            .map(|r| r.labelled_by.clone())
            .unwrap_or_default();
        let view = A11yNodeView {
            entity: n.entity,
            role: n.role.copied().unwrap_or_default(),
            // Filled with the final ACCNAME (incl. labelledby/contents) at emit.
            name: local_name.clone(),
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
            tooltip_host: n.tooltip_host.is_some(),
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
            // SC-4 (C5, Wave 4): project the `A11yScroll` source into the view's
            // scroll field. `None` ⇒ not a scroll container (no setter fires).
            scroll: n.scroll.map(|s| s.view()),
            // Nesting filled at emit time.
            parent: None,
            children: Vec::new(),
        };
        meta.insert(
            n.entity,
            NodeMeta {
                view,
                hidden: n.hidden.is_some(),
                labelled_by_targets,
                local_name,
            },
        );
    }

    // --- 3. Prune the `A11yHidden` subtrees (§7.4). -------------------------
    // An entity is pruned if it OR any ancestor carries `A11yHidden`. The walk
    // climbs `ChildOf` through non-a11y wrappers too (they're in `hierarchy` but
    // not `meta`, so they carry no marker and the climb continues past them).
    let pruned: HashSet<Entity> = meta
        .keys()
        .copied()
        .filter(|&e| is_hidden_or_descendant(e, &meta, &hierarchy))
        .collect();

    // The emitted set: a11y-bearing AND not pruned. Only these become nodes and
    // only these are valid a11y parents/children.
    let emits = |e: Entity| meta.contains_key(&e) && !pruned.contains(&e);

    // --- 4. Collapse wrappers → a11y parent + ordered children (§7.1). ------
    // HashMap iteration is unspecified, so collect + sort for a deterministic
    // flat emission order. Document order is preserved *within* each node's
    // `children` (the collapse walk descends `Children` in order), independent
    // of this flat ordering — which the synthetic/window root re-parents anyway.
    let mut emitted: Vec<Entity> = meta.keys().copied().filter(|&e| emits(e)).collect();
    emitted.sort_unstable_by_key(|e| e.to_bits());

    // A pruned a11y node (a11y-bearing but excluded) must stop the child descent
    // wholesale — its subtree is hidden (§7.4) — whereas a non-a11y wrapper is
    // traversed *through*. The two are distinguished by `meta` membership: in
    // `meta` but not emitting ⇒ pruned; absent from `meta` ⇒ wrapper.
    let is_pruned = |e: Entity| meta.contains_key(&e) && pruned.contains(&e);

    let mut nodes: Vec<A11yNodeView> = Vec::with_capacity(emitted.len());
    for e in emitted {
        let m = &meta[&e];
        let parent = nearest_a11y_ancestor(e, &hierarchy, &emits);
        let children = collapsed_children(e, &hierarchy, &emits, &is_pruned);

        // Deferred ACCNAME arms (semantic-tree.md §6), now that the tree exists:
        // - `labelledby` (highest precedence): the space-joined *local* names of
        //   the `aria-labelledby` targets, in order.
        // - `contents` (below placeholder): the node's own subtree text — the
        //   space-joined local names of its collapsed a11y children.
        //
        // The final ladder is `labelledby > local > contents`, where `local`
        // (`label > value > placeholder`) was already resolved into
        // `m.local_name`. We re-run the canonical ladder feeding the precomputed
        // local name through the `label` arm (the highest local arm;
        // `value`/`placeholder` are `None`, so it cannot be overtaken) — keeping
        // `compute_accessible_name` the single source of precedence truth rather
        // than re-implementing the `.or_else` chain here.
        let labelledby_name = resolve_labelledby_name(&m.labelled_by_targets, &meta, &emits);
        let contents_name = resolve_contents_name(&children, &meta);
        let local_label = A11yLabel(m.local_name.clone());
        let name = compute_accessible_name(AccNameInputs {
            labelledby_name: labelledby_name.as_deref(),
            label: Some(&local_label),
            value: None,
            placeholder: None,
            contents_name: contents_name.as_deref(),
        });

        let mut view = m.view.clone();
        view.name = name;
        view.parent = parent;
        view.children = children;
        nodes.push(view);
    }

    builder.nodes = nodes;
}

/// Whether `e` is hidden by `A11yHidden` on itself or any `ChildOf` ancestor
/// (semantic-tree.md §7.4). The whole subtree under a hidden node is pruned.
fn is_hidden_or_descendant(
    e: Entity,
    meta: &HashMap<Entity, NodeMeta>,
    hierarchy: &Hierarchy,
) -> bool {
    let mut cur = Some(e);
    while let Some(c) = cur {
        // Only a11y entities (in `meta`) can carry `A11yHidden`; a non-a11y
        // wrapper is transparent — climb through it via the full hierarchy.
        if meta.get(&c).is_some_and(|m| m.hidden) {
            return true;
        }
        cur = hierarchy.parent_of(c);
    }
    false
}

/// The nearest a11y-bearing, non-pruned ancestor of `e` (semantic-tree.md §7.1):
/// climb `ChildOf` through the full hierarchy, skipping every entity that emits
/// no node (presentational wrappers and pruned nodes), and return the first one
/// that does — or `None` for a top-level node (parented to the synthetic/window
/// root, §7.2).
fn nearest_a11y_ancestor(
    e: Entity,
    hierarchy: &Hierarchy,
    emits: &impl Fn(Entity) -> bool,
) -> Option<Entity> {
    let mut cur = hierarchy.parent_of(e);
    while let Some(c) = cur {
        if emits(c) {
            return Some(c);
        }
        cur = hierarchy.parent_of(c);
    }
    None
}

/// The a11y children of `e` in document order (semantic-tree.md §7.1): descend
/// `Children` through the full hierarchy, collapsing every presentational
/// wrapper into its nearest emitting descendants (so wrappers leave no hole) and
/// excluding pruned `A11yHidden` subtrees wholesale.
fn collapsed_children(
    e: Entity,
    hierarchy: &Hierarchy,
    emits: &impl Fn(Entity) -> bool,
    is_pruned: &impl Fn(Entity) -> bool,
) -> Vec<Entity> {
    let mut out = Vec::new();
    for &child in hierarchy.children_of(e) {
        collect_a11y_descendants(child, hierarchy, emits, is_pruned, &mut out);
    }
    out
}

/// Document-order DFS over one raw child subtree, appending the nearest emitting
/// descendants. Three cases for `e`:
/// - **emits** ⇒ it is itself the a11y child; push it and do **not** recurse (its
///   own children belong to *it*).
/// - **pruned** (`A11yHidden` self/ancestor, §7.4) ⇒ skip it and its whole
///   subtree — contribute nothing.
/// - **wrapper** (no a11y content) ⇒ recurse into its `Children` to surface the
///   emitters underneath, collapsing the wrapper.
fn collect_a11y_descendants(
    e: Entity,
    hierarchy: &Hierarchy,
    emits: &impl Fn(Entity) -> bool,
    is_pruned: &impl Fn(Entity) -> bool,
    out: &mut Vec<Entity>,
) {
    if emits(e) {
        out.push(e);
        return;
    }
    if is_pruned(e) {
        return;
    }
    // Presentational wrapper: collapse it by descending into its children.
    for &child in hierarchy.children_of(e) {
        collect_a11y_descendants(child, hierarchy, emits, is_pruned, out);
    }
}

/// `aria-labelledby` contribution (semantic-tree.md §6, highest precedence):
/// the space-joined *local* names of the referenced targets that still emit a
/// node. A reference to a pruned/absent target contributes nothing. Empty ⇒
/// `None` (no labelledby contribution; the local arms take over).
fn resolve_labelledby_name(
    targets: &[Entity],
    meta: &HashMap<Entity, NodeMeta>,
    emits: &impl Fn(Entity) -> bool,
) -> Option<String> {
    let joined = targets
        .iter()
        .filter(|&&t| emits(t))
        .filter_map(|t| meta.get(t))
        .map(|m| m.local_name.as_str())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    (!joined.is_empty()).then_some(joined)
}

/// `contents` contribution (semantic-tree.md §6, below placeholder): the node's
/// own subtree text — the space-joined *local* names of its collapsed a11y
/// children, in document order. Empty ⇒ `None`.
fn resolve_contents_name(children: &[Entity], meta: &HashMap<Entity, NodeMeta>) -> Option<String> {
    let joined = children
        .iter()
        .filter_map(|c| meta.get(c))
        .map(|m| m.local_name.as_str())
        .filter(|s| !s.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    (!joined.is_empty()).then_some(joined)
}
