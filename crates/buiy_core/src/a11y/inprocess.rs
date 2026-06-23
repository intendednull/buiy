//! The **in-process inspect + control driver** (P1c-c, inprocess-api.md §§2–5):
//! Buiy's own headless test driver and the transport-agnostic substrate the MCP
//! companion later wraps in a socket envelope without changing a line. Defined
//! ONCE over `&mut World` / `&mut App` — no winit, no GPU.
//!
//! Two halves of the same round-trip:
//!
//! - [`snapshot`] (inspect) runs the **production** translate path —
//!   `build_tree` populated the canonical `A11yNodeView` list this frame;
//!   [`snapshot`] feeds that ONE list through the same `build_tree_update` fold
//!   the real `accesskit_winit::Adapter` consumes into an
//!   [`accesskit_consumer::Tree`], and serializes a [`SemanticTree`] **from the
//!   consumer view** (role-implied defaults, relation resolution, and focus all
//!   resolve the way a real AT would). It does **not** fork a second tree path:
//!   the same `A11yTreeBuilder` views drive `buiy_verify`'s `semantic_tree`
//!   string helper, which projects from this very `snapshot`.
//! - [`perform`] (control) is the single primitive: build the `ActionRequest`,
//!   call [`dispatch_action_request`](super::dispatch_action_request) — the
//!   headless act seam (action-router.md §5) — tick the schedule, and
//!   auto-re-`snapshot`. **Act-then-observe in one call.** The thin sugar
//!   ([`click`]/[`set_value`]/[`focus`]/[`increment`]/[`expand`]) constructs the
//!   right `(action, data)` and funnels through `perform`; no parallel routing.
//!
//! [`get_by_role`] addresses a node above the bare `NodeId` (strict single-match,
//! the Playwright locator rule); [`wait_for`] blocks a real frame-loop on a
//! semantic condition (no sleeps, no pixel diff).
//!
//! **Deferred under demand-pull** (co-drive §3.2 — no gallery consumer):
//! `set_selection` (+ the `EditCommand::SetSelection` editor slice it needs) and
//! the whole actionability gate loop (`act_when_actionable` / the
//! `HitTargetable` + `Stable` gates). `wait_for` is *not* deferred — it is a
//! standalone semantic-condition poll, independent of the actionability gates.

use crate::a11y::contract::ActionError;
use crate::a11y::translate::{build_tree_update, node_id_for};
use crate::a11y::{A11yRole, A11yTreeBuilder};
use crate::focus::FocusedEntity;
use accesskit::{Action, ActionData, NodeId, Toggled};
use accesskit_consumer::{Node as ConsumerNode, Tree as ConsumerTree};
use bevy::prelude::{App, World};

/// A present-only snapshot of one node's state, read back **through the consumer**
/// (inprocess-api.md §2.1). A field is `Some`/non-default **only when the
/// component is present** — absence carries meaning (`toggled: None` ⇒ not
/// toggleable), matching the decomposed component model. Deterministic + diffable;
/// the new lowest verification tier (inprocess-api.md §4).
///
/// `Default` is hand-written (not derived): `accesskit::Live` is a foreign enum
/// without a `Default` impl, and the inert default is `Live::Off` (no
/// announcement) — the same "absence is inert" convention as the components.
#[derive(Clone, Debug, PartialEq)]
pub struct NodeState {
    /// Tri-state toggle (checkbox/switch/pressed). `None` ⇒ not toggleable.
    pub toggled: Option<Toggled>,
    /// Disclosure expanded/collapsed. `None` ⇒ not a disclosure.
    pub expanded: Option<bool>,
    /// Selected (option/tab). `None` ⇒ not selectable.
    pub selected: Option<bool>,
    /// Disabled. Present-only: `true` iff the node carries the disabled flag.
    pub disabled: bool,
    /// Modal (dialog/overlay). Present-only.
    pub modal: bool,
    /// Current numeric value of a valued range (slider/spinner). `None` ⇒ none.
    pub numeric_value: Option<f64>,
    /// Single-line text value / human-readable value text. `None` ⇒ none.
    pub value: Option<String>,
    /// Placeholder / prompt text. `None` ⇒ none.
    pub placeholder: Option<String>,
    /// Control orientation. `None` ⇒ unset.
    pub orientation: Option<accesskit::Orientation>,
    /// Popup kind a control opens. `None` ⇒ no popup.
    pub has_popup: Option<accesskit::HasPopup>,
    /// Effective live-region politeness (role-implied or explicit). `Off` is the
    /// inert default; carried verbatim so a status/alert region is observable.
    pub live: accesskit::Live,
    /// Whether this node currently holds focus (read from the consumer's focus).
    pub focused: bool,
    /// SC-4 scroll geometry, read back through the consumer's f64 scroll
    /// getters. `None` ⇒ not a scroll container (no scroll setter fired);
    /// `Some` ⇒ the live offset + per-axis maxima a scroll region reports, so an
    /// AT (and the inspection driver) observes the scroll position + extent the
    /// wheel/keyboard handlers leave (the C5/SC-4 source folded into the tree).
    pub scroll: Option<ScrollState>,
}

/// The consumer-side view of a scroll container's SC-4 scroll geometry — the
/// `Some` payload of [`NodeState::scroll`]. Each axis carries the current offset
/// and the maximum scrollable offset (`content_extent − viewport_extent`, clamped
/// ≥ 0); the min is always `0.0` so it is not re-carried. Read back through the
/// `accesskit_consumer` scroll getters, so it reflects exactly what the AT sees.
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct ScrollState {
    /// Current horizontal scroll offset (logical px) — `scroll_x()`.
    pub x: f64,
    /// Current vertical scroll offset (logical px) — `scroll_y()`.
    pub y: f64,
    /// Maximum horizontal offset (`content − viewport`, ≥ 0) — `scroll_x_max()`.
    pub x_max: f64,
    /// Maximum vertical offset (`content − viewport`, ≥ 0) — `scroll_y_max()`.
    pub y_max: f64,
}

impl Default for NodeState {
    fn default() -> Self {
        Self {
            toggled: None,
            expanded: None,
            selected: None,
            disabled: false,
            modal: false,
            numeric_value: None,
            value: None,
            placeholder: None,
            orientation: None,
            has_popup: None,
            // `accesskit::Live` has no `Default`; `Off` is the inert no-announce
            // policy — the present-only convention applied to the live region.
            live: accesskit::Live::Off,
            focused: false,
            scroll: None,
        }
    }
}

/// A present-only predicate over [`NodeState`] for [`get_by_role`] disambiguation
/// (inprocess-api.md §3.2). Every field defaults to "don't care" (`None` /
/// `false`); a set field must match the node's same-named state exactly. Matched
/// against the **same** decomposed state [`snapshot`] exposes.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct StateQuery {
    /// Require this exact tri-state toggle.
    pub toggled: Option<Toggled>,
    /// Require this expanded/collapsed value.
    pub expanded: Option<bool>,
    /// Require this selected value.
    pub selected: Option<bool>,
    /// Require the node to be disabled (`true`) — `false` ⇒ don't care.
    pub disabled: bool,
    /// Require the node to be focused (`true`) — `false` ⇒ don't care.
    pub focused: bool,
}

impl StateQuery {
    /// Whether `state` satisfies every set field of this predicate. A field left
    /// at its "don't care" default never excludes a node.
    fn matches(&self, state: &NodeState) -> bool {
        (self.toggled.is_none() || self.toggled == state.toggled)
            && (self.expanded.is_none() || self.expanded == state.expanded)
            && (self.selected.is_none() || self.selected == state.selected)
            && (!self.disabled || state.disabled)
            && (!self.focused || state.focused)
    }
}

/// One node of the observed [`SemanticTree`] (inprocess-api.md §2.1): the role,
/// the computed accessible name, present-only state, advertised actions the
/// router can honor, resolved relation refs, the node's own `ref`
/// ([`NodeId`]), and its children in document order. The `ref` round-trips:
/// `perform(world, action, node.r#ref, data)` addresses this same entity.
#[derive(Clone, Debug, PartialEq)]
pub struct SemanticNode {
    /// The node's stable AccessKit id (`node_id_for(entity)`). Round-trips with
    /// [`perform`]'s `target` and `entity_for_node_id`.
    pub r#ref: NodeId,
    /// Decomposed role (lockstep with `translate::role_to_accesskit`).
    pub role: A11yRole,
    /// Computed accessible name (ACCNAME — read back through the consumer).
    pub name: String,
    /// Present-only decomposed state.
    pub state: NodeState,
    /// The advertised verbs the inbound router can honor on this node (the same
    /// set the outbound fold advertised). Used by [`get_by_role`] consumers and
    /// to assert capability without driving the action.
    pub actions: Vec<Action>,
    /// Nodes that label this one (`labelled_by`), as their `ref`s.
    pub labelled_by: Vec<NodeId>,
    /// Nodes that describe this one (`described_by`), as their `ref`s.
    pub described_by: Vec<NodeId>,
    /// Nodes this one controls (`controls`), as their `ref`s.
    pub controls: Vec<NodeId>,
    /// Active descendant of a composite widget, as its `ref`.
    pub active_descendant: Option<NodeId>,
    /// This node's a11y children in document order, as their `ref`s.
    pub children: Vec<NodeId>,
}

/// The structured observe result of [`snapshot`] / [`perform`]: every emitted
/// a11y node, in the canonical `build_tree` order, each read back through the
/// in-process consumer. Flat list keyed by `ref` (the nesting is carried per
/// node in [`SemanticNode::children`]) so lookups and predicates over the whole
/// tree stay O(n) without a recursive walk.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SemanticTree {
    /// Every emitted node, in canonical `build_tree` order.
    pub nodes: Vec<SemanticNode>,
}

impl SemanticTree {
    /// The node addressed by `r#ref`, if present.
    pub fn node(&self, r#ref: NodeId) -> Option<&SemanticNode> {
        self.nodes.iter().find(|n| n.r#ref == r#ref)
    }

    /// All nodes whose role is `role` (in canonical order).
    pub fn by_role(&self, role: A11yRole) -> impl Iterator<Item = &SemanticNode> {
        self.nodes.iter().filter(move |n| n.role == role)
    }
}

/// Which projection of the canonical tree [`snapshot`] reads. Mirrors
/// `buiy_verify::a11y::TreeView` (inprocess-api.md §2): `Unmerged` is the
/// canonical structural tree (the default, what P1c/C7 read); `Merged` (read-time
/// collapse of `A11yMergeChildren` subtrees) is reserved for a later phase and is
/// accepted here only so the signature is stable — it behaves identically to
/// `Unmerged` until merge components exist (co-drive §3.2 defers the distinction).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TreeView {
    /// Canonical structural tree (the default; most diffable).
    #[default]
    Unmerged,
    /// Read-time `A11yMergeChildren` collapse. Not yet distinct from `Unmerged`.
    Merged,
}

/// Build the in-process [`accesskit_consumer::Tree`] for the current frame's
/// canonical views, focusing `focused` (the synthetic root if `None`). The ONE
/// consumer construction in `buiy_core`: the same `build_tree_update` fold the
/// live adapter consumes, fed into the same consumer a real AT drives.
fn consume(views: &[crate::a11y::A11yNodeView], focused: Option<NodeId>) -> ConsumerTree {
    // `root_entity: None` — headless, so the synthetic root keys off ROOT_NODE_ID
    // (semantic-tree.md §7.2). `is_host_focused = true`: the Buiy "window" is the
    // focused host in a headless fixture, so the consumer applies `focus` directly.
    ConsumerTree::new(build_tree_update(views, focused, None), true)
}

/// Project one consumer [`ConsumerNode`] into a [`SemanticNode`], reading state
/// **through the consumer** (role-implied defaults applied). `view` supplies the
/// decomposed `A11yRole` (the consumer exposes the AccessKit `Role`, but Buiy's
/// `A11yRole` is the addressing vocabulary) and the resolved relation/children
/// refs (already `NodeId` in the view, no `Entity` leak).
fn project_node(node: &ConsumerNode, view: &crate::a11y::A11yNodeView) -> SemanticNode {
    // Advertised verbs the router can honor: walk the small fixed `Action` set and
    // keep the ones the consumer node supports. No parent filter (`|_| Include`)
    // — Buiy advertises every honored verb directly on the node, never inherited.
    let actions = ADDRESSABLE_ACTIONS
        .iter()
        .copied()
        .filter(|&a| node.supports_action(a, &|_| accesskit_consumer::FilterResult::Include))
        .collect();

    let state = NodeState {
        toggled: node.toggled(),
        // `is_expanded` lives on the raw node data (the consumer wraps only the
        // role-derived `supports_expand_collapse`), so read it there.
        expanded: node.data().is_expanded(),
        selected: node.is_selected(),
        disabled: node.is_disabled(),
        modal: node.is_modal(),
        numeric_value: node.numeric_value(),
        value: node.value(),
        placeholder: node.placeholder().map(str::to_owned),
        orientation: node.orientation(),
        has_popup: node.has_popup(),
        live: node.live(),
        focused: node.is_focused(),
        // SC-4 scroll: a scroll container fires `set_scroll_x`/`set_scroll_y`, so
        // `scroll_y()` is `Some` exactly for a scroll region. Read the live offset
        // + per-axis maxima back through the consumer (the same getters a real AT
        // calls); a non-scroll node reports `None` on every getter ⇒ `None` here.
        scroll: node.scroll_y().or(node.scroll_x()).map(|_| ScrollState {
            x: node.scroll_x().unwrap_or(0.0),
            y: node.scroll_y().unwrap_or(0.0),
            x_max: node.scroll_x_max().unwrap_or(0.0),
            y_max: node.scroll_y_max().unwrap_or(0.0),
        }),
    };

    SemanticNode {
        // The producer-side `NodeId` (`node_id_for(entity)`), NOT the consumer's
        // internal `(TreeIndex, LocalNodeId)` id — this is the ref an inbound
        // `ActionRequest.target` carries, so it round-trips through `perform`.
        r#ref: node_id_for(view.entity),
        role: view.role,
        name: node.label().unwrap_or_default(),
        state,
        actions,
        labelled_by: view.labelled_by.clone(),
        described_by: view.described_by.clone(),
        controls: view.controls.clone(),
        active_descendant: view.active_descendant,
        children: view.children.iter().map(|&e| node_id_for(e)).collect(),
    }
}

/// The verbs [`SemanticNode::actions`] probes. The small fixed set Buiy can
/// advertise + honor today (the contract surface, contract.rs); deferred verbs
/// (`SetTextSelection`/`ReplaceSelectedText`/`Scroll*`/`CustomAction`) are not
/// probed because nothing advertises them (co-drive §3.2).
const ADDRESSABLE_ACTIONS: &[Action] = &[
    Action::Click,
    Action::Focus,
    Action::Blur,
    Action::SetValue,
    Action::Increment,
    Action::Decrement,
    Action::Expand,
    Action::Collapse,
    Action::ShowTooltip,
    Action::HideTooltip,
];

/// Read the live a11y tree as a structured [`SemanticTree`] (inprocess-api.md §2).
///
/// Reads the canonical `A11yNodeView` list `build_tree` populated this frame from
/// the [`A11yTreeBuilder`] resource — the **single** source of truth — feeds it
/// through the production [`build_tree_update`] fold into an in-process
/// [`accesskit_consumer::Tree`], and serializes each node back from the consumer
/// view. The current [`FocusedEntity`] (when the resource exists) is the consumer
/// focus, so [`NodeState::focused`] reflects real focus.
///
/// The caller is expected to have driven at least one `app.update()` so the
/// builder reflects the current world; this fn does **not** tick the schedule
/// (the caller controls when the frame settles). [`perform`]/[`wait_for`] tick
/// before snapshotting.
pub fn snapshot(world: &mut World, _view: TreeView) -> SemanticTree {
    // `Merged` is accepted but not yet distinct from `Unmerged` (no merge
    // components exist) — the projection is the same either way (co-drive §3.2).
    let views: Vec<crate::a11y::A11yNodeView> = world
        .get_resource::<A11yTreeBuilder>()
        .map(|b| b.snapshot().to_vec())
        .unwrap_or_default();

    // Focus the consumer on the live `FocusedEntity` so `is_focused` is faithful
    // (action-router.md honors Focus into this resource). Absent resource
    // (partial harness) ⇒ focus the synthetic root.
    let focused = world
        .get_resource::<FocusedEntity>()
        .and_then(|f| f.0)
        .map(node_id_for);
    let tree = consume(&views, focused);

    let mut nodes = Vec::with_capacity(views.len());
    for view in &views {
        let id = node_id_for(view.entity);
        match tree
            .state()
            .node_by_tree_local_id(id, accesskit::TreeId::ROOT)
        {
            Some(node) => nodes.push(project_node(&node, view)),
            // A view with no consumer node is a producer/consumer divergence; it
            // cannot happen for an emitted view (every one is in the update), but
            // stay total rather than panic.
            None => continue,
        }
    }
    SemanticTree { nodes }
}

/// The single control primitive — **act-then-observe in one round-trip**
/// (inprocess-api.md §3): build `ActionRequest { action, target, data }`, call
/// [`dispatch_action_request`](super::dispatch_action_request) directly (the
/// headless seam — same liveness + capability + live-state filter, same per-verb
/// lowering into the real `OnPress`/`FocusedEntity`/contract sinks), then
/// re-[`snapshot`] and return the post-action [`SemanticTree`] inline.
///
/// Failure is **typed and loud** ([`ActionError`]) — never a silent no-op. On
/// error the world is left as the dispatch left it (the guards reject *before*
/// any mutation), and no snapshot is returned.
///
/// Note: like the dispatch seam, this does not itself tick `Update` — it mutates
/// the world synchronously (writing the `OnPress` message / setting
/// `FocusedEntity`) and snapshots the result. To let a *system* observe the
/// consequence (e.g. a Checkbox advancing `A11yToggled` off `OnPress`), drive
/// `app.update()` after `perform` (or use the [`App`]-level helpers / [`wait_for`]).
pub fn perform(
    world: &mut World,
    action: Action,
    target: NodeId,
    data: Option<ActionData>,
) -> Result<SemanticTree, ActionError> {
    let req = accesskit::ActionRequest {
        action,
        target_tree: accesskit::TreeId::ROOT,
        target_node: target,
        data,
    };
    super::dispatch_action_request(world, &req)?;
    Ok(snapshot(world, TreeView::default()))
}

/// Click `target` (`Action::Click`, no data) — thin sugar over [`perform`].
pub fn click(world: &mut World, target: NodeId) -> Result<SemanticTree, ActionError> {
    perform(world, Action::Click, target, None)
}

/// Focus `target` (`Action::Focus`, no data) — thin sugar over [`perform`]. The
/// returned tree shows `target` focused (`NodeState::focused`).
pub fn focus(world: &mut World, target: NodeId) -> Result<SemanticTree, ActionError> {
    perform(world, Action::Focus, target, None)
}

/// Increment `target`'s value (`Action::Increment`, no data) — sugar over
/// [`perform`]. Surfaces [`ActionError::Unsupported`] when the role does not
/// advertise it (e.g. a Button), as a `Result`, never a panic.
pub fn increment(world: &mut World, target: NodeId) -> Result<SemanticTree, ActionError> {
    perform(world, Action::Increment, target, None)
}

/// Expand `target` (`Action::Expand`, no data) — sugar over [`perform`].
pub fn expand(world: &mut World, target: NodeId) -> Result<SemanticTree, ActionError> {
    perform(world, Action::Expand, target, None)
}

/// Show `target`'s tooltip (`Action::ShowTooltip`, no data) — sugar over
/// [`perform`]. The router lowers this generically over the trigger's
/// `described_by` tooltip node (it shows the tooltip's
/// [`CssVisibility`](crate::render::components::CssVisibility)). Surfaces
/// [`ActionError::Unsupported`] on a node without `A11yTooltipHost`.
pub fn show_tooltip(world: &mut World, target: NodeId) -> Result<SemanticTree, ActionError> {
    perform(world, Action::ShowTooltip, target, None)
}

/// Hide `target`'s tooltip (`Action::HideTooltip`, no data) — sugar over
/// [`perform`]. The generic counterpart of [`show_tooltip`].
pub fn hide_tooltip(world: &mut World, target: NodeId) -> Result<SemanticTree, ActionError> {
    perform(world, Action::HideTooltip, target, None)
}

/// Set `target`'s text value (`Action::SetValue` carrying the string) — sugar
/// over [`perform`]. The router lowers this through the **existing**
/// `SelectAll` + `Insert` editor channel (co-drive §3.1 / §6 text set-channel
/// loop) — no new `EditCommand`. Deferred `SetSelection` is a separate slice
/// (co-drive §3.2), not reached here.
pub fn set_value(
    world: &mut World,
    target: NodeId,
    value: &str,
) -> Result<SemanticTree, ActionError> {
    perform(
        world,
        Action::SetValue,
        target,
        Some(ActionData::Value(value.into())),
    )
}

/// Resolve a node by role (+ optional accessible name + optional state
/// predicate) over the current [`snapshot`] — **strict single-match**
/// (inprocess-api.md §3.2, the Playwright strict-locator rule):
///
/// - exactly one match ⇒ its `ref`;
/// - **zero** matches ⇒ [`ActionError::NotFound`];
/// - **more than one** match ⇒ [`ActionError::NotFound`] (ambiguity is a *test*
///   bug — never first-match, never a retry).
///
/// `name`, when `Some`, must equal the node's computed accessible name exactly.
/// `state`, when `Some`, is a present-only [`StateQuery`] matched against the same
/// decomposed state the snapshot exposes.
///
/// The disambiguation `target` carried by the `>1` error is the synthetic
/// [`ROOT_NODE_ID`](super::translate::ROOT_NODE_ID): the failure is about the
/// *query*, not any single node, and the root is the one stable non-widget id.
pub fn get_by_role(
    world: &mut World,
    role: A11yRole,
    name: Option<&str>,
    state: Option<&StateQuery>,
) -> Result<NodeId, ActionError> {
    let tree = snapshot(world, TreeView::default());
    let mut matches = tree.by_role(role).filter(|n| {
        name.is_none_or(|want| n.name == want) && state.is_none_or(|q| q.matches(&n.state))
    });

    let Some(first) = matches.next() else {
        // Zero matches — the addressed widget is not in the tree.
        return Err(ActionError::NotFound {
            target: super::translate::ROOT_NODE_ID,
        });
    };
    if matches.next().is_some() {
        // Ambiguous: the strict locator refuses to guess (Playwright rule).
        return Err(ActionError::NotFound {
            target: super::translate::ROOT_NODE_ID,
        });
    }
    Ok(first.r#ref)
}

/// Block on a **semantic** condition over the [`SemanticTree`], stepping real
/// frames between checks (inprocess-api.md §5.2). Calls `app.update()` then
/// snapshots, up to `timeout_frames` times; returns the first [`SemanticTree`]
/// satisfying `cond`, or [`ActionError::NotActionable`]
/// ([`NotActionableReason::Timeout`](super::NotActionableReason::Timeout)) if the
/// condition never holds. Never a sleep, never a pixel diff — the building block
/// for async/animated flows (and the MCP companion exposes the same `wait_for`).
///
/// Independent of the deferred actionability *gates* (co-drive §3.2): this is a
/// standalone condition poll, not the act-when-actionable loop.
pub fn wait_for(
    app: &mut App,
    cond: impl Fn(&SemanticTree) -> bool,
    timeout_frames: u32,
) -> Result<SemanticTree, ActionError> {
    for _ in 0..timeout_frames {
        app.update();
        let tree = snapshot(app.world_mut(), TreeView::default());
        if cond(&tree) {
            return Ok(tree);
        }
    }
    Err(ActionError::NotActionable {
        // The condition is over the whole tree, not one node; the synthetic root
        // is the stable non-widget id to carry.
        target: super::translate::ROOT_NODE_ID,
        action: Action::Focus, // sentinel — `wait_for` is action-agnostic.
        reason: super::NotActionableReason::Timeout,
    })
}
