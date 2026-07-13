//! The widget-runtime **interaction-state visual layer** (spec §2.6, F5 part 3).
//!
//! A `view` is a pure `fn(&Model) -> Element`, so a *transient* pressed/hover
//! style — one that exists only while a finger is on the control — is **ephemeral
//! non-model state**. Threading it through the model would put per-frame pointer
//! state on the replay log and fight the pure-view contract; that is exactly why
//! the prototype shipped its 3D-press buttons **resting-only** (retro §3). This
//! module owns that state OUTSIDE the pure view, in the widget runtime — the same
//! place `buiy_widgets::Button` would own it (it carries none today): a small
//! [`InteractionState`] the pointer observers write, consumed by the
//! [`apply_press_visual`] resolver that dips a pressable node while it is held.
//!
//! **`ControlledLeaf` is NOT the vehicle** (spec §2.6 finding #1). That marker is
//! an opt-OUT *suppression* for controlled toggle inputs (reverse data flow, a
//! `Without<ControlledLeaf>` filter) — unrelated to press *visuals*. The layer
//! here is net-new.
//!
//! **A discrete-state resolver, not a transition engine** (spec §6 risk R3). v1
//! resolves three discrete states (`None`/`Hover`/`Press`) with no easing/tween;
//! the press-down is level-triggered off `Press`. Hover is tracked for a future
//! `:hover` style but does **not** drive the v1 default visual — so moving between
//! a container's children never flickers the press-down (only a real primary
//! [`Press`]/[`Release`] moves it, and those do not fire on a hover-only move).
//!
//! The concrete press-down *look* Dooduel's chunky buttons need (a deeper dip +
//! shadow-collapse) is applied later by F3 / the app via [`PressEffect`]; F5 keeps
//! the mechanism general.

use bevy::picking::events::{Out, Over, Pointer, Press, Release};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use buiy_core::layout::{Length, Translate};
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::Background;

/// The default press-down depth (logical px) a pressable node dips while held — a
/// modest, general value. F3 / the app raise it via [`PressEffect`] for a chunky
/// 3D-press; small enough here to stay unobtrusive on an unstyled `button`.
pub const DEFAULT_PRESS_DEPTH: f32 = 2.0;

/// The transient pointer-interaction state of a pressable node, owned by the
/// **widget runtime** (never the model). Written by the pointer observers
/// (`on_pointer_over` / `on_pointer_out` / `on_pointer_press` / `on_pointer_release`),
/// read by the `apply_press_visual` resolver.
///
/// A discrete three-value state (spec §6 R3 — no transition engine). Priority is
/// `Press > Hover > None`: a held primary button reads `Press` even while the
/// pointer is also hovering. The reconciler stamps it on every pressable node
/// (a `button`, and a clickable container / pressable `raster`).
#[derive(Component, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum InteractionState {
    /// Resting — the pointer is not over the node and the node is not pressed.
    #[default]
    None,
    /// The pointer is over the node but the primary button is not held.
    Hover,
    /// The primary button is held down on the node — the press-down state.
    Press,
}

/// How far (logical px) a pressable node dips while its [`InteractionState`] is
/// [`InteractionState::Press`]. The widget-runtime resolver (`apply_press_visual`)
/// applies it as a transient vertical [`Translate`] — the 3D-press-down, owned
/// *outside* the pure view. The reconciler stamps a [`DEFAULT_PRESS_DEPTH`] on
/// every pressable node; F3 / the app raise it for a chunkier press.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct PressEffect {
    /// Logical px the node moves down (screen +y) while held. `0.0` = no visible
    /// press (the state is still tracked, for `:hover` / probe use).
    pub depth: f32,
}

impl Default for PressEffect {
    fn default() -> Self {
        Self {
            depth: DEFAULT_PRESS_DEPTH,
        }
    }
}

/// The declarative `:hover`/`:active` fill (Track D, spec §3) — a node's
/// [`Background`] token while its [`InteractionState`] is [`Hover`](InteractionState::Hover)
/// or [`Press`](InteractionState::Press), authored by `Element::hover_bg`. Owned by
/// the widget runtime (never the model): the *intent* is pure `Element` data, the
/// *resolved* fill is transient non-model state — the same place the press-down
/// lives, and for the same replay-safety reason.
///
/// **Background only in v1.** `:active` is folded into the same `hover` token (the
/// existing depth dip is the distinct pressed look), so the resolver applies it
/// under `Hover` OR `Press` — see [`resolve_hover_background`].
///
/// Unlike [`PressEffect`]'s `Translate` (which nothing else writes), `Background`
/// is **shared-ownership**: `reconcile` re-derives it from `Element::background`
/// every `Changed<M>` frame. So [`resting`](HoverStyle::resting) records the fill
/// to return to (the author's `.background()` token, or the node's own default
/// captured once at install — e.g. a `button()`'s `SurfaceSecondary`), and the
/// resolver ([`apply_hover_visual`]) re-wins the frame after a reconcile write via
/// its `Or<Changed<Background>>` gate.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub(crate) struct HoverStyle {
    /// The fill to return to when the node is resting ([`InteractionState::None`]).
    /// `None` ⇒ [`ColorToken::Transparent`] (an unstyled container with no default
    /// fill). Tracks the author's `.background()` token when set, else the fill
    /// captured once at install.
    pub(crate) resting: Option<ColorToken>,
    /// The fill applied while the node is hovered or pressed.
    pub(crate) hover: ColorToken,
}

/// The pure hover-fill resolver (spec §3) — unit-testable without the picking
/// pipeline. Priority folds `:active` into `:hover`: a held or hovered node reads
/// [`HoverStyle::hover`]; a resting node reads [`HoverStyle::resting`], defaulting
/// to [`ColorToken::Transparent`] when the node has no resting fill.
pub(crate) fn resolve_hover_background(state: InteractionState, style: HoverStyle) -> ColorToken {
    match state {
        InteractionState::Hover | InteractionState::Press => style.hover,
        InteractionState::None => style.resting.unwrap_or(ColorToken::Transparent),
    }
}

/// The discrete pointer phase an observer maps its `Pointer<E>` event to — the
/// input alphabet of the [`transition`] state machine.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PointerPhase {
    /// `Pointer<Over>` — the pointer entered the node.
    Enter,
    /// `Pointer<Out>` — the pointer left the node.
    Leave,
    /// A primary `Pointer<Press>` on the node.
    PrimaryDown,
    /// A primary `Pointer<Release>` on the node.
    PrimaryUp,
}

/// The pure discrete transition (spec §6 R3). Kept a free fn so the state machine
/// is unit-testable **without** the picking pipeline. Priority `Press > Hover >
/// None`:
/// * entering while resting → `Hover`; entering while pressed → **stay** `Press`
///   (a spurious re-enter must not drop the press-down);
/// * leaving → `None` (a drag-off clears both hover and press);
/// * primary-down → `Press` (even from a cold `None`, e.g. touch, which has no
///   prior hover);
/// * primary-up → `Hover` (a release still over the node lands on hover; a real
///   drag-off already fired `Leave` → `None`, and no `Release` targets this node).
pub(crate) fn transition(cur: InteractionState, phase: PointerPhase) -> InteractionState {
    use InteractionState::{Hover, None, Press};
    match (cur, phase) {
        (Press, PointerPhase::Enter) => Press,
        (_, PointerPhase::Enter) => Hover,
        (_, PointerPhase::Leave) => None,
        (_, PointerPhase::PrimaryDown) => Press,
        (_, PointerPhase::PrimaryUp) => Hover,
    }
}

/// Apply `phase` to a node's [`InteractionState`] **iff it has one**. The bubbling
/// `Pointer<E>` fires the observer for each entity in the child→parent path, so a
/// clickable **container** updates even when a child intercepted the hit; a node
/// without the component is silently skipped. `set_if_neq` keeps a same-state event
/// from tripping `Changed` (so the resolver stays idle on a no-op event).
fn drive(entity: Entity, phase: PointerPhase, q: &mut Query<&mut InteractionState>) {
    if let Ok(mut state) = q.get_mut(entity) {
        state.set_if_neq(transition(*state, phase));
    }
}

/// `Pointer<Over>` observer → [`PointerPhase::Enter`].
pub(crate) fn on_pointer_over(ev: On<Pointer<Over>>, mut q: Query<&mut InteractionState>) {
    drive(ev.entity, PointerPhase::Enter, &mut q);
}

/// `Pointer<Out>` observer → [`PointerPhase::Leave`].
pub(crate) fn on_pointer_out(ev: On<Pointer<Out>>, mut q: Query<&mut InteractionState>) {
    drive(ev.entity, PointerPhase::Leave, &mut q);
}

/// `Pointer<Press>` observer → [`PointerPhase::PrimaryDown`] (primary button only —
/// a secondary/middle press must not paint a press-down).
pub(crate) fn on_pointer_press(ev: On<Pointer<Press>>, mut q: Query<&mut InteractionState>) {
    if ev.event.button == PointerButton::Primary {
        drive(ev.entity, PointerPhase::PrimaryDown, &mut q);
    }
}

/// `Pointer<Release>` observer → [`PointerPhase::PrimaryUp`] (primary button only).
pub(crate) fn on_pointer_release(ev: On<Pointer<Release>>, mut q: Query<&mut InteractionState>) {
    if ev.event.button == PointerButton::Primary {
        drive(ev.entity, PointerPhase::PrimaryUp, &mut q);
    }
}

/// The **press-style resolver** (spec §2.6 part 3): dip a pressable node while it
/// is held. Reads `Changed<InteractionState>` + [`PressEffect`] and writes the
/// node's vertical [`Translate`] — `depth` px down while [`InteractionState::Press`],
/// back to `0` otherwise.
///
/// Level-triggered off the discrete state, so a steady frame with no pointer change
/// never runs (an untripped `Changed` — the F7 steady-frame discipline: no
/// re-upload while nothing moves). The node is **pre-stamped** a `Translate` by the
/// reconciler, so this only mutates it in place — no deferred insert, so the
/// press-down lands the SAME frame as the press (no one-frame lag).
pub(crate) fn apply_press_visual(
    mut q: Query<(&InteractionState, &PressEffect, &mut Translate), Changed<InteractionState>>,
) {
    for (state, effect, mut translate) in &mut q {
        let depth = if *state == InteractionState::Press {
            effect.depth
        } else {
            0.0
        };
        let want = Length::px(depth);
        // `!=`-guarded: an event that leaves the depth unchanged (e.g. Hover↔None)
        // does not re-mark `Translate` and so does not re-trigger the transform
        // composition.
        if translate.1 != want {
            translate.1 = want;
        }
    }
}

/// The **hover-style resolver** (Track D, spec §3): paint a node's declarative
/// [`HoverStyle`] fill from its [`InteractionState`]. Writes [`Background`] to the
/// hover token while `Hover`/`Press`, back to the resting token otherwise
/// ([`resolve_hover_background`]), `set_if_neq` so an unchanged fill never re-marks
/// the component.
///
/// **The gate is materially different from [`apply_press_visual`]'s — do not
/// copy-paste it.** The press resolver gates on `Changed<InteractionState>` alone,
/// safe only because nothing else writes its `Translate`. `Background` is
/// **shared-ownership**: `reconcile` re-derives it from `Element::background` every
/// `Changed<M>` frame (`apply_background` / `apply_button_style`). Gating on
/// `Changed<InteractionState>` alone would be **silently clobbered** whenever a
/// hovered node's model also changes that frame (e.g. a running clock tick) —
/// intermittent hover-fill flicker. The `Or<(Changed<InteractionState>,
/// Changed<Background>)>` filter re-trips this system when an earlier
/// reconcile write in the *same* frame touched `Background`, so it re-wins the race
/// — provided it is scheduled `.after(reconcile::<M>)` (see `app.rs`). A steady
/// frame with neither input changed is a true no-op (empty query), so the 60 Hz
/// idle floor is unaffected.
#[allow(clippy::type_complexity)] // the `Or` race gate is load-bearing; see the doc above.
pub(crate) fn apply_hover_visual(
    mut q: Query<
        (&InteractionState, &HoverStyle, &mut Background),
        Or<(Changed<InteractionState>, Changed<Background>)>,
    >,
) {
    for (state, style, mut bg) in &mut q {
        let want = Background {
            color: resolve_hover_background(*state, *style),
        };
        bg.set_if_neq(want);
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_PRESS_DEPTH, HoverStyle, InteractionState, PointerPhase, PressEffect,
        resolve_hover_background, transition,
    };
    use InteractionState::{Hover, None, Press};
    use PointerPhase::{Enter, Leave, PrimaryDown, PrimaryUp};
    use buiy_core::render::color::ColorToken;

    #[test]
    fn discrete_transitions_cover_the_press_lifecycle() {
        // Resting → hover on enter; hover → press on primary-down; press → hover
        // on primary-up (released still over); hover → none on leave.
        assert_eq!(transition(None, Enter), Hover);
        assert_eq!(transition(Hover, PrimaryDown), Press);
        assert_eq!(transition(Press, PrimaryUp), Hover);
        assert_eq!(transition(Hover, Leave), None);
    }

    #[test]
    fn a_drag_off_while_pressed_clears_the_press_down() {
        // Leaving while held → None (the press-down must not stick when the
        // pointer is dragged off the control).
        assert_eq!(transition(Press, Leave), None);
    }

    #[test]
    fn a_spurious_reenter_while_pressed_keeps_the_press() {
        // Priority Press > Hover: an Over that arrives while still held must not
        // downgrade the press-down to hover.
        assert_eq!(transition(Press, Enter), Press);
    }

    #[test]
    fn a_cold_primary_down_presses_without_prior_hover() {
        // Touch has no prior hover map — a cold primary-down still presses.
        assert_eq!(transition(None, PrimaryDown), Press);
    }

    #[test]
    fn press_effect_default_is_the_documented_depth() {
        assert_eq!(PressEffect::default().depth, DEFAULT_PRESS_DEPTH);
    }

    // --- The declarative hover-fill resolver (Track D, spec §3) -------------

    #[test]
    fn hover_and_press_both_resolve_to_the_hover_token() {
        // `:active` folds into `:hover`: the fill is applied under BOTH Hover and
        // Press (else it would flash back to resting during a press, since the
        // state priority is Press > Hover > None).
        let style = HoverStyle {
            resting: Some(ColorToken::SurfaceSecondary),
            hover: ColorToken::Accent,
        };
        assert_eq!(resolve_hover_background(Hover, style), ColorToken::Accent);
        assert_eq!(resolve_hover_background(Press, style), ColorToken::Accent);
    }

    #[test]
    fn resting_resolves_to_the_resting_token() {
        let style = HoverStyle {
            resting: Some(ColorToken::SurfaceSecondary),
            hover: ColorToken::Accent,
        };
        assert_eq!(
            resolve_hover_background(None, style),
            ColorToken::SurfaceSecondary
        );
    }

    #[test]
    fn resting_with_no_resting_token_is_transparent() {
        // An unstyled container (no `.background()`, no captured default fill)
        // reverts to transparent — the pre-hover appearance.
        let style = HoverStyle {
            resting: Option::None,
            hover: ColorToken::Accent,
        };
        assert_eq!(
            resolve_hover_background(None, style),
            ColorToken::Transparent
        );
    }
}
