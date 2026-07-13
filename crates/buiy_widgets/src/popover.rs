//! Popover — the anchored top-layer positioning primitive (C5-b,
//! scroll-overlay-modal.md §B.2).
//!
//! A `Popover` is **not** an APG control and has **no a11y role of its own** —
//! the wrapping widget (menu / tooltip / anchored panel) supplies the role. It
//! is the shared *positioning* primitive the later C5 widgets (Menu, Dialog)
//! reuse: an overlay container that
//!
//! - lives in the **top layer** (`Stacking { top_layer: TopLayer::Popover }`),
//!   so it escapes its parent stacking context and paints above the page, and
//!   joins the global `TopLayerActivation` deque (the most-recently-activated
//!   overlay is at the back — the "top-most open overlay" the light-dismiss /
//!   Esc handlers consult), and
//! - is **positioned relative to an anchor (trigger) entity** with flip/shift
//!   fallback so it stays on-screen.
//!
//! # Positioning reuses the layout anchor pipeline (single source of truth)
//!
//! Rather than re-implement anchor-relative placement + the fit-in-window flip,
//! `Popover` **lowers to the layout [`Anchor`] component** (display-and-positioning.md
//! §3): [`position_popover`] translates the popover's ordered [`PopoverPlacement`]
//! candidates into an `Anchor.position_try` fallback chain (each candidate guarded
//! by [`TryCondition::FitsInViewport`], with a final unconditional fallback), and
//! the existing `anchor_resolution` (layout sub-pass 6d) walks that chain, applies
//! the first try whose box fits the viewport, and writes the resolved position
//! through `PostTaffyPositionOverrides` into `ResolvedLayout.position`. So a
//! popover near the bottom edge whose first (below-anchor) candidate would overflow
//! **flips** to the next (above-anchor) candidate exactly the way an authored
//! `Anchor` chain does — one positioning engine, not two.
//!
//! `position_popover` runs **`.before(BuiySet::Layout)`** and mutates the popover's
//! already-`#[require]`d `Anchor` *in place* (not via `Commands`), so the write is
//! visible to the same-frame `anchor_resolution` (which runs *inside*
//! `BuiySet::Layout`) — no command-sync frame lag.
//!
//! # Deferred to later C5 slices
//!
//! - `Menu`/`MenuItem`/`MenuButton` (compose `Popover` + keyboard nav +
//!   `A11yHasPopup`/`A11yExpanded`), `Dialog` open/focus-trap/Esc/restore +
//!   `FocusScope` + roving + `Inert` — all later C5 slices.

use crate::dismiss::LightDismiss;
use bevy::prelude::*;
use buiy_core::{
    components::Node,
    layout::{Anchor, AnchorRef, Inset, Length, PositionTry, Stacking, TopLayer, TryCondition},
    render::components::CssVisibility,
};

/// Which side of the anchor the popover sits on.
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PopoverSide {
    Top,
    #[default]
    Bottom,
    Left,
    Right,
}

/// How the popover aligns along the anchor edge it sits against.
///
/// v1 lowers `Start`/`Center`/`End` onto the layout anchor pipeline, which
/// aligns the leading edges (the `try_anchored_position` convention: a zero
/// cross-axis inset keeps the anchored box's leading edge flush with the
/// anchor's). `Center`/`End` are named in the contract and reserved for the
/// follow-up that adds cross-axis offset terms; today every align resolves to
/// the leading-edge placement. (Recorded as a known v1 limitation, not a silent
/// drop — the side flip, which is what keeps the popover on-screen, is fully
/// honored.)
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum PopoverAlign {
    #[default]
    Start,
    Center,
    End,
}

/// One ordered placement candidate: which `side` of the anchor, how it
/// `align`s, and the `gap` (logical px) between the anchor edge and the popover.
#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub struct PopoverPlacement {
    pub side: PopoverSide,
    pub align: PopoverAlign,
    pub gap: f32,
}

impl Default for PopoverPlacement {
    fn default() -> Self {
        Self {
            side: PopoverSide::Bottom,
            align: PopoverAlign::Start,
            gap: DEFAULT_POPOVER_GAP,
        }
    }
}

impl PopoverPlacement {
    /// The [`Inset`] that places a popover on this candidate's side, `gap` px
    /// from the anchor edge. Maps `side` onto the `try_anchored_position`
    /// edge convention (a positive `top` inset places the popover *below* the
    /// anchor, a positive `bottom` inset *above*, etc.). `pub(crate)` so the
    /// tooltip placement (which reuses the same `PopoverPlacement` candidates)
    /// lowers through the same mapping.
    pub(crate) fn to_inset(self) -> Inset {
        let gap = Length::px(self.gap);
        match self.side {
            PopoverSide::Bottom => Inset::below(gap),
            PopoverSide::Top => Inset::above(gap),
            PopoverSide::Right => Inset::right_of(gap),
            PopoverSide::Left => Inset::left_of(gap),
        }
    }
}

/// The default gap (logical px) between the anchor edge and the popover.
pub const DEFAULT_POPOVER_GAP: f32 = 4.0;

/// The anchored top-layer overlay positioning primitive (§B.2). The
/// `#[require(...)]` assembles the container substrate:
///
/// - `Node` — the layout marker (pulls the `Style` decomposition), so the
///   popover is a real layout box anchor resolution can place.
/// - `Stacking = popover_stacking()` — `top_layer: TopLayer::Popover`, so it
///   escapes its parent stacking context, paints above the page, and joins the
///   `TopLayerActivation` deque.
/// - `Anchor` — the layout positioning component [`position_popover`] writes
///   into; `anchor_resolution` consumes it. Required (not author-set) so the
///   lowering can mutate it in place with no command-sync lag.
///
/// The popover's `anchor` (the trigger entity) + the ordered `positions`
/// candidates + the `window_margin` are author-set on the `Popover` component
/// itself; [`position_popover`] lowers them onto the required `Anchor` each
/// frame the popover or its inputs change.
///
/// A popover defaults to the `auto` light-dismiss policy (`#[require]`s
/// [`LightDismiss`]): an outside press or Escape closes it. The opening
/// `trigger` is kept exempt — [`position_popover`] syncs `LightDismiss.trigger`
/// from `Popover.anchor`, so a press on the trigger does not fight a trigger that
/// re-opens on the same press.
///
/// A popover has **no a11y role** — the wrapping widget supplies it.
#[derive(Component, Reflect, Clone, Debug, PartialEq)]
#[reflect(Component)]
#[require(Node, Stacking = popover_stacking(), Anchor, LightDismiss)]
pub struct Popover {
    /// The anchor (trigger) entity the popover is positioned relative to.
    /// `None` = positioned by the author (no anchor lowering — `position_popover`
    /// leaves the `Anchor` untouched).
    pub anchor: Option<Entity>,
    /// Ordered placement candidates. The first whose resolved box fits the
    /// viewport wins; if none fit, the **last** candidate is applied
    /// unconditionally (least-bad, so the popover is never left at the origin).
    pub positions: Vec<PopoverPlacement>,
    /// Margin (logical px) kept from the window edge when testing fit. Reserved
    /// for the cross-axis-shift follow-up; the side flip (the on-screen
    /// guarantee) is honored today via `FitsInViewport`.
    pub window_margin: f32,
}

impl Default for Popover {
    fn default() -> Self {
        Self {
            anchor: None,
            // Default candidate chain: below the anchor, else above (the
            // canonical tooltip/menu flip).
            positions: vec![
                PopoverPlacement {
                    side: PopoverSide::Bottom,
                    ..Default::default()
                },
                PopoverPlacement {
                    side: PopoverSide::Top,
                    ..Default::default()
                },
            ],
            window_margin: DEFAULT_POPOVER_MARGIN,
        }
    }
}

/// The default window margin (logical px).
pub const DEFAULT_POPOVER_MARGIN: f32 = 8.0;

/// The canonical popover `Stacking`: `top_layer: TopLayer::Popover`. `pub(crate)`
/// so the scene module / composing widgets spell the SAME value.
pub(crate) fn popover_stacking() -> Stacking {
    Stacking {
        top_layer: TopLayer::Popover,
        ..Default::default()
    }
}

impl Popover {
    /// A popover anchored to `anchor`, using the default below-then-above flip
    /// chain. Spawn it as a top-layer overlay; author its visible content as
    /// `children!`.
    pub fn anchored_to(anchor: Entity) -> Self {
        Self {
            anchor: Some(anchor),
            ..Default::default()
        }
    }

    /// Read whether this popover is currently **open** (shown), as a plain `bool` —
    /// the domain accessor over its [`CssVisibility`] show/hide state (the
    /// `Checkbox::checked` / `Disclosure::expanded` pattern: read widget state through
    /// the marker rather than matching the `CssVisibility` variants by hand — the enum
    /// stays un-preluded). Delegates to the shared [`crate::popover::is_open`] free
    /// function, the single source of truth for the open/closed channel
    /// (`Visible`/absent = open, `Hidden`/`Collapse` = closed). Query the visibility
    /// alongside the marker and pass it in:
    ///
    /// ```ignore
    /// fn read(q: Query<Option<&CssVisibility>, With<Popover>>) {
    ///     for vis in &q {
    ///         if Popover::is_open(vis) { /* … */ }
    ///     }
    /// }
    /// ```
    pub fn is_open(vis: Option<&CssVisibility>) -> bool {
        crate::popover::is_open(vis)
    }
}

/// Lower each [`Popover`] onto its required [`Anchor`] (§B.2): translate the
/// ordered [`PopoverPlacement`] candidates into an `Anchor.position_try` fallback
/// chain (each guarded by [`TryCondition::FitsInViewport`], plus a final
/// unconditional fallback so a popover with no fitting candidate still lands on
/// the last side rather than the origin), and point `Anchor.position_anchor` at
/// the popover's `anchor` entity.
///
/// Runs **`.before(BuiySet::Layout)`** and mutates the `Anchor` **in place** (not
/// via `Commands`), so the same-frame `anchor_resolution` (inside `BuiySet::Layout`)
/// sees the lowered chain — no command-sync frame lag. Writes only on an actual
/// change (the `PartialEq` gate) so a steady popover does not re-trigger
/// `Changed<Anchor>` (which would re-run `sync_styles`).
///
/// A popover with `anchor: None` is author-positioned — the lowering clears the
/// `position_anchor` so `anchor_resolution` skips it.
///
/// It also keeps the popover's [`LightDismiss::trigger`] in sync with
/// `Popover.anchor`, so the opening trigger stays exempt from the outside-press
/// dismiss (a press on the trigger toggles the overlay, it does not also dismiss),
/// and **enforces the top-layer `Stacking`**. The `#[require(Stacking = ...)]`
/// initializer is silently defeated when the popover is co-spawned with a `Style`
/// (whose Bundle inserts an explicit `Stacking::default()` that suppresses the
/// require — the § 4.1c suppression gotcha), so the top-layer membership is
/// re-asserted here to make it robust regardless of the authoring path.
pub fn position_popover(mut q: Query<(&Popover, &mut Anchor, &mut LightDismiss, &mut Stacking)>) {
    for (popover, mut anchor, mut dismiss, mut stacking) in q.iter_mut() {
        // Keep the trigger exemption in sync with the popover's anchor.
        if dismiss.trigger != popover.anchor {
            dismiss.trigger = popover.anchor;
        }
        // Enforce top-layer membership (robust against the Style-co-spawn
        // suppression of the `#[require]` initializer).
        if stacking.top_layer != TopLayer::Popover {
            stacking.top_layer = TopLayer::Popover;
        }

        let Some(target) = popover.anchor else {
            // Author-positioned popover: ensure we are not anchoring it.
            if anchor.position_anchor.is_some() {
                anchor.position_anchor = None;
                anchor.position_try.clear();
            }
            continue;
        };

        let mut position_try: Vec<PositionTry> = Vec::with_capacity(popover.positions.len());
        let last = popover.positions.len().saturating_sub(1);
        for (i, placement) in popover.positions.iter().enumerate() {
            // Every candidate but the last is guarded by FitsInViewport (so a
            // candidate that would overflow is skipped and the chain flips to
            // the next); the last candidate is unconditional (least-bad: the
            // popover is placed on the final side rather than abandoned to the
            // origin, the Floating-UI "fall back to least-bad" rule).
            let conditions = if i == last {
                Vec::new()
            } else {
                vec![TryCondition::FitsInViewport]
            };
            position_try.push(PositionTry {
                inset: placement.to_inset(),
                conditions,
            });
        }

        let next_position_anchor = Some(AnchorRef::Entity(target));
        // Mutate only on a real change so steady state is a no-op for change
        // detection (the anchor-pipeline `sync_styles` re-run gate).
        if anchor.position_anchor != next_position_anchor || anchor.position_try != position_try {
            anchor.position_anchor = next_position_anchor;
            anchor.position_try = position_try;
        }
    }
}

/// Whether `popover` is currently **open** (shown). An overlay's open/closed
/// state rides the existing [`CssVisibility`] show/hide channel (the same channel
/// the P1d tooltip honor flips): `Visible` (or absent — the default) = open,
/// `Hidden`/`Collapse` = closed. The light-dismiss + Esc handlers (`dismiss.rs`)
/// treat only open overlays as dismissable.
pub fn is_open(vis: Option<&CssVisibility>) -> bool {
    !matches!(
        vis,
        Some(CssVisibility::Hidden) | Some(CssVisibility::Collapse)
    )
}
