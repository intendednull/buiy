//! Focus model: focus tree, Tab handling, focus-visible heuristic, focus
//! restoration. Phase 0 implements ordered Tab traversal; full focus tree
//! (roving tabindex, aria-activedescendant, traps, restoration, spatial nav)
//! lives in `buiy-focus-model-design`.
//!
//! See: docs/specs/2026-05-07-buiy-foundation/architecture.md § 2.3 and
//! accessibility.md (Focus management).
//!
//! # Phase 0 deferred behavior
//!
//! - **Auto tab order is `entity.index()`-based, not full "document order".**
//!   Bevy reuses entity indices after despawn; for two `Focusable`s with
//!   `tab_order = 0`, the resolved order depends on entity-index allocation,
//!   not insertion order. Insertion-order stability is owned by
//!   `buiy-focus-model-design`.
//! - **`FocusVisible` decay (`:focus-visible`).** Keyboard focus IS
//!   focus-visible: `handle_tab` sets `FocusVisible(true)`. Pointer focus is
//!   NOT: the shared [`focus_on_click`] observer (C3d, input-event-model.md
//!   § 2.7 / co-drive SC-2) sets `FocusVisible(false)` when a primary
//!   `Pointer<Press>` focuses a `Focusable`. C6 reads `entity ==
//!   FocusedEntity.0 && FocusVisible.0` to gate the focus ring. (The richer
//!   focus tree — roving tabindex, scopes, restoration — is still
//!   `buiy-focus-model-design`'s; C3d ships only the resource-level decay
//!   signal, not the ring shape or a `FocusVisible` representation change.)
//! - **Shift detection covers `ShiftLeft`/`ShiftRight` only.** Sticky-keys /
//!   accessibility-shell remappings of Shift to other key codes are out of
//!   scope; full key-binding abstraction lives in `buiy-input-events-design`.

use crate::BuiySet;
use crate::Length;
use crate::a11y::A11yHidden;
use crate::layout::{Stacking, TopLayer, TopLayerActivation};
use crate::render::color::ColorToken;
use crate::render::components::{CssVisibility, LineStyle, Outline};
use bevy::picking::events::{Pointer, Press};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;

/// Marks an entity as part of the focus tree.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct Focusable {
    /// Phase 0: 0 = Auto (in document order); negative = Skip; positive = explicit.
    pub tab_order: i32,
}

/// How a [`FocusScope`] confines Tab traversal (scroll-overlay-modal.md §C.1).
#[derive(Reflect, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FocusScopeMode {
    /// A non-modal focus-cycling region (e.g. a toolbar). Tab wraps *inside* the
    /// scope while focus is within it, but focus can still leave by other means
    /// (it is not the innermost-modal trap). Reserved for non-modal regions.
    Contain,
    /// A modal trap: while this scope is the innermost open
    /// [`TopLayer::Modal`](crate::layout::TopLayer) scope, Tab / Shift+Tab cycle
    /// **only** among its focusable descendants (wrapping at the ends) — focus
    /// cannot escape to a background element. Escape always closes/escapes (no
    /// keyboard trap, WCAG 2.1.2 — the close path is the overlay state machine,
    /// not the traversal).
    #[default]
    Trap,
}

/// A focus-traversal boundary (scroll-overlay-modal.md §C.1). When this scope is
/// the **innermost open modal** (a [`TopLayer::Modal`] entity carrying
/// [`FocusScopeMode::Trap`], keyed off the [`TopLayerActivation`] deque), Tab /
/// Shift+Tab cycle only among the scope's focusable descendants. Outside any
/// active trap, traversal is the flat-global set — today's non-modal behavior,
/// preserved exactly.
///
/// The trap scope is **derived**, not registered: `handle_tab` reads the back of
/// `TopLayerActivation.order` (the most-recently-activated top-layer entity) for
/// the innermost open `TopLayer::Modal` carrying a `FocusScope::Trap` — the
/// single-source-of-truth choice (§3.4: couple to the built activation deque, not
/// a free-floating registry).
#[derive(Component, Reflect, Clone, Copy, Debug, Default)]
#[reflect(Component, Default)]
pub struct FocusScope {
    pub mode: FocusScopeMode,
}

impl FocusScope {
    /// A modal trap scope (the Dialog/Menu container `#[require]`).
    pub fn trap() -> Self {
        Self {
            mode: FocusScopeMode::Trap,
        }
    }

    /// A non-modal focus-cycling region.
    pub fn contain() -> Self {
        Self {
            mode: FocusScopeMode::Contain,
        }
    }
}

/// Focus-restoration target captured when an overlay opens (scroll-overlay-modal.md
/// §C.4): the entity that held focus *before* the overlay took it. On close, focus
/// is restored to this entity (WCAG 2.4.3 focus order). Stored on the overlay so
/// nested overlays restore in LIFO order. `None` = nothing was focused at open
/// time (close clears focus).
#[derive(Component, Reflect, Clone, Copy, Debug, Default)]
#[reflect(Component, Default)]
pub struct FocusReturn(pub Option<Entity>);

/// Currently focused entity (None = nothing focused).
#[derive(Resource, Reflect, Default, Clone, Debug)]
#[reflect(Resource)]
pub struct FocusedEntity(pub Option<Entity>);

/// Tracks whether the most recent focus change was keyboard / programmatic
/// (true) or pointer (false). Drives the `:focus-visible` heuristic — focus
/// rings render only when this is true.
#[derive(Resource, Reflect, Default, Clone, Debug)]
#[reflect(Resource)]
pub struct FocusVisible(pub bool);

/// Marks an [`Outline`] this crate's focus-ring lowering ([`lower_focus_ring`])
/// owns, so the lowering only ever inserts/removes the FRAMEWORK ring and never
/// touches an author's own `Outline`. A paint-only marker, framework-written
/// (never author-set) — hence the leaner derives (no `Reflect`/`Default`,
/// matching the computed render-prep markers). styling-f-tier.md § 2.6.
#[derive(Component, Clone, Copy, Debug)]
pub struct FocusRingMarker;

/// Focus-ring width in logical px. ≥ 2px satisfies WCAG 2.4.11 (the focus
/// indicator must be at least a 2px-thick perimeter), styling-f-tier.md § 2.6.
const FOCUS_RING_WIDTH_PX: f32 = 2.0;
/// Focus-ring offset (gap between the border box and the ring) in logical px.
const FOCUS_RING_OFFSET_PX: f32 = 2.0;

/// The framework focus-ring [`Outline`]: a `Solid`, ≥ 2px, offset-2px stroke in
/// the [`ColorToken::FocusRing`] token (WCAG 2.4.11). It resolves at extract
/// against the active theme — the default light theme's focus-ring color (a
/// high-contrast accent, ≥ 3:1 vs the white canvas) and, under forced-colors,
/// the system `Highlight` value the forced resolve maps `FocusRing` to
/// (theme.rs) — so the ring re-tints on a theme/forced-colors change with no
/// relowering. `FocusRing` is deliberately one of the forced-colors-**safe**
/// kinds ([`ColorToken::is_forced_colors_safe`]): the gate-#11 analyzer must not
/// flag it, since the forced theme keeps the ring visible on purpose.
fn focus_ring_outline() -> Outline {
    Outline {
        color: ColorToken::FocusRing,
        style: LineStyle::Solid,
        width: Length::px(FOCUS_RING_WIDTH_PX),
        offset: Length::px(FOCUS_RING_OFFSET_PX),
    }
}

/// Lower the SC-2 keyboard-focus-visible signal into a framework-owned
/// [`Outline`] focus ring (styling-f-tier.md § 2.6 / § 3.6 — C6-a). Reads the
/// settled `FocusedEntity` + `FocusVisible` resource pair (the signal C3/C5
/// own); it writes NO focus-tree state, only the paint-only ring. The ring is
/// shown on the entity iff `Some(e) == FocusedEntity.0 && FocusVisible.0`
/// (keyboard focus), and removed everywhere else — so a pointer-focused entity
/// (`FocusVisible(false)` from C3d's `focus_on_click`) gets NO ring, the correct
/// `:focus-visible` behavior. The ring `Outline` is gated by [`FocusRingMarker`]
/// so the lowering never disturbs an author's own `Outline`.
///
/// Scheduled `.after(BuiySet::Input)` (NOT `BuiySet::Style`, which the foundation
/// runs *before* `Input` — the focus signal is produced by `handle_tab` /
/// `focus_on_click` in `Input`, so lowering in `Style` would read last frame's
/// signal and lag the ring by a frame). The inserted/removed `Outline` is applied
/// at the next command sync, which is before the render-world ExtractSchedule, so
/// extract sees the settled ring the same frame.
pub fn lower_focus_ring(
    focused: Res<FocusedEntity>,
    visible: Res<FocusVisible>,
    rings: Query<Entity, With<FocusRingMarker>>,
    // Distinguishes an AUTHOR `Outline` (no marker) from no outline at all, so
    // the lowering never clobbers an author's own outline on focus.
    outlines: Query<(Has<Outline>, Has<FocusRingMarker>)>,
    mut commands: Commands,
) {
    // The single entity that should carry the keyboard-focus ring this frame.
    let target = if visible.0 { focused.0 } else { None };

    // Remove the ring from any entity we own one on that is no longer the
    // visibly-focused target (focus moved, focus lost, or focus-visible decayed
    // to pointer). Removing both the `Outline` and the marker keeps the two in
    // lockstep — a stale marker without an `Outline` (or vice versa) can never
    // accumulate.
    for entity in rings.iter() {
        if Some(entity) != target {
            commands
                .entity(entity)
                .remove::<Outline>()
                .remove::<FocusRingMarker>();
        }
    }

    // Insert the ring on the visibly-focused entity, UNLESS it already carries an
    // author `Outline` (no marker) — the framework never clobbers an author's own
    // outline (styling-f-tier.md § 2.6: the lowering only owns rings it marks). A
    // re-insert on the already-ringed target is skipped (it already has the
    // marker), so a steady keyboard focus issues no per-frame structural op.
    if let Some(entity) = target
        && let Ok((has_outline, has_ring)) = outlines.get(entity)
        && !has_ring
        && !has_outline
    {
        commands
            .entity(entity)
            .insert((focus_ring_outline(), FocusRingMarker));
    }
}

pub struct FocusPlugin;

impl Plugin for FocusPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Focusable>()
            .register_type::<FocusedEntity>()
            .register_type::<FocusVisible>()
            .register_type::<FocusScope>()
            .register_type::<FocusReturn>()
            .init_resource::<FocusedEntity>()
            .init_resource::<FocusVisible>()
            .add_systems(Update, handle_tab.in_set(BuiySet::Input))
            // C6-a (styling-f-tier.md § 2.6): lower the keyboard-focus-visible
            // signal into the framework focus-ring `Outline`. Runs AFTER
            // `BuiySet::Input` (where `handle_tab` / `focus_on_click` produce the
            // signal) and BEFORE `BuiySet::Render`, so the inserted/removed ring
            // `Outline` is settled when extract runs. NOT in `BuiySet::Style`
            // (which precedes `Input`): lowering there would read last frame's
            // signal and lag the ring by a frame.
            .add_systems(
                Update,
                lower_focus_ring
                    .after(BuiySet::Input)
                    .before(BuiySet::Render),
            )
            // C3d (input-event-model.md § 2.7): the single, widget-agnostic
            // focus-on-click observer. Owns `FocusedEntity` for ALL pointer
            // focus — the editor's `editor_pointer_press` and the `TextInput`
            // widget no longer set it themselves; they keep only their
            // non-focus logic (cursor placement / nothing). Lives here in
            // `FocusPlugin` so it covers every `Focusable`, not just editors.
            .add_observer(focus_on_click);
    }
}

/// The single, widget-agnostic focus-on-click observer (input-event-model.md
/// § 2.7 / co-drive SC-2). On a primary [`Pointer<Press>`] it walks from the
/// picked target up the [`ChildOf`] chain to the nearest [`Focusable`] and, if
/// one is found, sets `FocusedEntity` to it AND `FocusVisible(false)` — pointer
/// focus is NOT keyboard-`:focus-visible` (the decay half § 2.7 / C6 needs;
/// `handle_tab` sets the `true` half).
///
/// This consolidates focus-on-click that C3c had split across two per-widget
/// observers (the editor's `editor_pointer_press` and the `TextInput`
/// `focus_on_click`). Both were `Focusable`, so both now focus through this one
/// path; the editor keeps its click-to-place-cursor and the `TextInput` widget
/// no longer needs a focus observer at all.
///
/// **Nearest-`Focusable`-ancestor target:** the picked entity is often a
/// decorative leaf inside a focusable widget root (the picked target need not be
/// the `Focusable` itself). Walking up `ChildOf` focuses the widget, not its
/// inner glyph/child. A press that resolves to no `Focusable` ancestor (a plain
/// node) leaves focus untouched — clicking empty chrome does not steal focus.
/// (Spec § 2.7 notes C3 "ships the leaf version"; the ancestor walk is the
/// robust generalization — for a bare `Focusable` it reduces to the leaf, and it
/// pre-satisfies the C5 "nearest focusable ancestor" refinement without a
/// per-entity focus component.)
///
/// `FocusedEntity`/`FocusVisible` are init by this same `FocusPlugin`, so the
/// resources are always present when this observer is registered — no
/// `Option<Res…>` guard is needed (unlike the editor/widget observers, which ran
/// in harnesses that add `BuiyTextPlugin`/`WidgetsPlugin` without `FocusPlugin`).
/// Observers fire only when the picking pipeline is present, so a headless
/// harness without it is inert by construction.
pub fn focus_on_click(
    press: On<Pointer<Press>>,
    focusables: Query<(), With<Focusable>>,
    parents: Query<&ChildOf>,
    mut focused: ResMut<FocusedEntity>,
    mut visible: ResMut<FocusVisible>,
) {
    if press.event.button != PointerButton::Primary {
        return;
    }
    let Some(target) = nearest_focusable(press.entity, &focusables, &parents) else {
        return; // pressed a non-focusable subtree — leave focus untouched
    };
    focused.0 = Some(target);
    // Pointer focus is NOT focus-visible (the `:focus-visible` decay, § 2.7).
    visible.0 = false;
}

/// Walk from `entity` up the [`ChildOf`] chain, returning the first entity that
/// is itself `Focusable` (including `entity`), or `None` if no ancestor is.
fn nearest_focusable(
    entity: Entity,
    focusables: &Query<(), With<Focusable>>,
    parents: &Query<&ChildOf>,
) -> Option<Entity> {
    let mut current = entity;
    loop {
        if focusables.contains(current) {
            return Some(current);
        }
        current = parents.get(current).ok()?.parent();
    }
}

/// The per-entity trap-scope query data: a candidate scope's `FocusScope` (mode),
/// its `Stacking` (top-layer membership), and its `CssVisibility` (open state — a
/// closed modal does not trap). Aliased to keep [`TrapInputs`] readable.
type ScopeData = (
    &'static FocusScope,
    &'static Stacking,
    Option<&'static CssVisibility>,
);

/// The active-modal-trap derivation inputs for [`handle_tab`] (scroll-overlay-modal.md
/// §C.1), aliased so the system signature stays under clippy's `type_complexity`
/// bar. The `TopLayerActivation` deque + the per-entity scope query jointly
/// resolve the innermost open `TopLayer::Modal` trap scope; the
/// `ChildOf`/`A11yHidden` queries scope + filter the candidate set.
type TrapInputs<'w, 's> = (
    Option<Res<'w, TopLayerActivation>>,
    Query<'w, 's, ScopeData>,
    Query<'w, 's, &'static ChildOf>,
    Query<'w, 's, (), With<A11yHidden>>,
);

/// `pub(crate)` so the P1c action router (`a11y::action`) can name it in its
/// intra-`BuiySet::Input` `.before(handle_tab)` ordering constraint
/// (action-router.md §7): the router must drain inbound requests *before* the
/// keyboard focus/edit handlers so a synthesized focus/activation is consumed
/// the same frame. Referencing it across plugins is sound — both systems live in
/// `Update`; a `.before` on a system that isn't scheduled (a harness without
/// `FocusPlugin`) is silently ignored.
pub(crate) fn handle_tab(
    keys: Res<ButtonInput<KeyCode>>,
    focusables: Query<(Entity, &Focusable)>,
    trap: TrapInputs,
    mut focused: ResMut<FocusedEntity>,
    mut visible: ResMut<FocusVisible>,
) {
    let pressed_tab = keys.just_pressed(KeyCode::Tab);
    if !pressed_tab {
        return;
    }
    let forward = !keys.pressed(KeyCode::ShiftLeft) && !keys.pressed(KeyCode::ShiftRight);
    let (activation, scopes, parents, hidden) = trap;

    // The innermost open modal trap scope, derived from the activation deque
    // (the back is the most-recently-activated top-layer entity). When present,
    // Tab traversal is confined to its focusable descendants; when absent, the
    // flat-global set is used (today's non-modal behavior, preserved).
    let active_scope = active_trap_scope(activation.as_deref(), &scopes);

    // Build the candidate set: every `Focusable` that is NOT inert (`A11yHidden`
    // self/ancestor) and — when a trap is active — is a descendant of the scope.
    // Inert filtering applies in BOTH the modal and non-modal cases (an
    // `A11yHidden` background element is never a Tab stop).
    let entries: Vec<(Entity, Focusable)> = focusables
        .iter()
        .filter(|(e, _)| !is_inert(*e, &parents, &hidden))
        .filter(|(e, _)| match active_scope {
            Some(scope) => is_descendant_of(*e, scope, &parents),
            None => true,
        })
        .map(|(e, f)| (e, f.clone()))
        .collect();

    focused.0 = compute_next_focus(&entries, focused.0, forward);
    visible.0 = true;
}

/// The innermost OPEN modal trap scope (scroll-overlay-modal.md §C.1): the
/// back-most entry of `TopLayerActivation.order` (most-recently-activated) that is
/// a visible [`TopLayer::Modal`](crate::layout::TopLayer) carrying a
/// [`FocusScopeMode::Trap`] `FocusScope`. A modal is in the activation deque even
/// while `CssVisibility::Hidden` (it keeps its layout box), so the **open** filter
/// (`CssVisibility != Hidden/Collapse`) is what gates the trap on/off — a closed
/// dialog must not trap focus. `None` when no open modal trap is up — the
/// flat-global non-modal case.
fn active_trap_scope(
    activation: Option<&TopLayerActivation>,
    scopes: &Query<ScopeData>,
) -> Option<Entity> {
    let activation = activation?;
    activation.order.iter().rev().copied().find(|&e| {
        scopes.get(e).is_ok_and(|(scope, stacking, vis)| {
            scope.mode == FocusScopeMode::Trap
                && stacking.top_layer == TopLayer::Modal
                && scope_is_open(vis)
        })
    })
}

/// Whether a scope's [`CssVisibility`] counts as **open** (the same predicate the
/// widget overlay layer uses): `Visible` (or absent — the default) = open,
/// `Hidden`/`Collapse` = closed. A closed modal does not trap.
fn scope_is_open(vis: Option<&CssVisibility>) -> bool {
    !matches!(
        vis,
        Some(CssVisibility::Hidden) | Some(CssVisibility::Collapse)
    )
}

/// Whether `e` is `ancestor` or any of its `ChildOf` ancestors is `ancestor`
/// (the modal-subtree descendant test). The scope entity itself counts as inside
/// its own scope.
fn is_descendant_of(e: Entity, ancestor: Entity, parents: &Query<&ChildOf>) -> bool {
    let mut cur = e;
    loop {
        if cur == ancestor {
            return true;
        }
        match parents.get(cur) {
            Ok(p) => cur = p.parent(),
            Err(_) => return false,
        }
    }
}

/// Whether `e` is inert — carries [`A11yHidden`] on itself or any `ChildOf`
/// ancestor (the inert-background focus-exclusion predicate, §C.2). The modal
/// lifecycle marks the rest-of-tree `A11yHidden` on open, so a background element
/// is never a Tab stop while a modal is up. This is the focus-traversal half of
/// the "one marker, three walks" inert model (the a11y prune is owned by
/// `build_tree`, semantic-tree.md §7.4).
fn is_inert(e: Entity, parents: &Query<&ChildOf>, hidden: &Query<(), With<A11yHidden>>) -> bool {
    let mut cur = e;
    loop {
        if hidden.contains(cur) {
            return true;
        }
        match parents.get(cur) {
            Ok(p) => cur = p.parent(),
            Err(_) => return false,
        }
    }
}

fn compute_next_focus(
    focusables: &[(Entity, Focusable)],
    current: Option<Entity>,
    forward: bool,
) -> Option<Entity> {
    let mut entries: Vec<(Entity, Focusable)> = focusables
        .iter()
        .filter(|(_, f)| f.tab_order >= 0)
        .cloned()
        .collect();
    if entries.is_empty() {
        return None;
    }
    // Sort: explicit positive tab_order first (ascending), then Auto (0) in document order.
    entries.sort_by_key(|(e, f)| (if f.tab_order > 0 { 0 } else { 1 }, f.tab_order, e.index()));

    let idx = current.and_then(|e| entries.iter().position(|(x, _)| *x == e));
    let n = entries.len();
    let next_idx = match (idx, forward) {
        (None, true) => 0,
        (None, false) => n - 1,
        (Some(i), true) => (i + 1) % n,
        (Some(i), false) => (i + n - 1) % n,
    };
    Some(entries[next_idx].0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::prelude::Entity;

    fn e(i: u32) -> Entity {
        Entity::from_raw_u32(i).unwrap()
    }

    fn f(tab_order: i32) -> Focusable {
        Focusable { tab_order }
    }

    /// Audit #5 (T2.17): the `Skip` branch — a `Focusable` with a negative
    /// `tab_order` is filtered out (`tab_order >= 0`, line 92) and must never
    /// be returned by traversal, even when it is the only other candidate and
    /// even across a wrap. With the skip filter removed this test reddens:
    /// the negative entity would re-enter the candidate list and be reachable.
    #[test]
    fn negative_tab_order_is_skipped() {
        // One focusable, one Skip(-1). The skipped entity is given the LOWER
        // entity index so a broken filter (or an `e.index()`-only sort) would
        // surface it first.
        let skip = e(1);
        let auto = e(2);
        let entries = vec![(skip, f(-1)), (auto, f(0))];

        // From nothing, forward, the only reachable focusable is the Auto one.
        assert_eq!(
            compute_next_focus(&entries, None, true),
            Some(auto),
            "negative tab_order must be skipped, leaving only the Auto focusable"
        );
        // Advancing from the Auto entity wraps back to itself — the skip
        // candidate is never reached.
        assert_eq!(
            compute_next_focus(&entries, Some(auto), true),
            Some(auto),
            "skip candidate must not appear in the wrap"
        );
        // Backward is identical: still only the Auto focusable.
        assert_eq!(
            compute_next_focus(&entries, None, false),
            Some(auto),
            "negative tab_order is skipped in both directions"
        );
    }

    /// Audit #5 (T2.17): the explicit-priority sort — positive `tab_order`s
    /// come before Auto(0), and positives are ordered ascending by their value
    /// (sort key `(if >0 {0} else {1}, tab_order, index)`, line 99). The
    /// entity indices are chosen to FIGHT the sort key: the Auto entity has the
    /// lowest index and the higher-priority positive (tab_order=1) has the
    /// highest index, so an index-only or group-dropped sort would order them
    /// differently and redden this test.
    #[test]
    fn positive_tab_orders_precede_auto_in_ascending_order() {
        let auto = e(1); // tab_order 0, lowest index
        let pos2 = e(2); // tab_order 2
        let pos1 = e(3); // tab_order 1, highest index

        let entries = vec![(auto, f(0)), (pos2, f(2)), (pos1, f(1))];

        // Resolved traversal order must be: pos1 (1) -> pos2 (2) -> auto (0).
        let first = compute_next_focus(&entries, None, true);
        assert_eq!(first, Some(pos1), "lowest positive tab_order comes first");
        let second = compute_next_focus(&entries, first, true);
        assert_eq!(second, Some(pos2), "positives ascend by tab_order value");
        let third = compute_next_focus(&entries, second, true);
        assert_eq!(third, Some(auto), "Auto(0) comes after all positives");
        // Wrap back to the first positive.
        let wrapped = compute_next_focus(&entries, third, true);
        assert_eq!(wrapped, Some(pos1), "traversal wraps to the first positive");
    }

    /// C5-d (scroll-overlay-modal.md §C.1): when a modal trap is active, `handle_tab`
    /// feeds `compute_next_focus` a candidate set ALREADY filtered to the scope's
    /// (non-inert) focusable descendants; the pure traversal then wraps within that
    /// set. This proves the wrap-within-scope invariant at the unit tier — given a
    /// two-element scoped set, forward cycles `a -> b -> a` and never escapes to a
    /// (here absent) background candidate. The descendant/inert filtering that
    /// produces this set is exercised end-to-end in `dialog_modal_c5d`.
    #[test]
    fn scoped_candidate_set_wraps_within_the_trap() {
        // The two focusable descendants of an (already-filtered) modal scope.
        let a = e(10);
        let b = e(11);
        let scoped = vec![(a, f(0)), (b, f(0))];

        // From the first, forward cycles a -> b -> a (wrap inside the scope).
        let one = compute_next_focus(&scoped, Some(a), true);
        assert_eq!(one, Some(b), "Tab moves to the next scoped focusable");
        let two = compute_next_focus(&scoped, one, true);
        assert_eq!(two, Some(a), "Tab wraps within the scoped set (the trap)");
        // Backward reverses: a -> b (wrap the other way).
        let back = compute_next_focus(&scoped, Some(a), false);
        assert_eq!(back, Some(b), "Shift+Tab reverses within the scoped set");
    }
}
