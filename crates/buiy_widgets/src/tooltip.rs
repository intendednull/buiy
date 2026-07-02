//! Tooltip-trigger widget — Wave-3 slice-5 (the LAST P1d widget bundle: the
//! `{ShowTooltip, HideTooltip}` capability + the `described_by` relation + the
//! minimal show/hide honor; NOT the placement/positioning geometry).
//!
//! A tooltip trigger is a focusable element that *hosts* a tooltip — a
//! non-interactive [`A11yRole::Tooltip`] node it `described_by`-references. The
//! trigger advertises `{ShowTooltip, HideTooltip, Focus, Blur}` (widget-contracts.md
//! §5 "Tooltip-trigger"). The two tooltip verbs are a **state-keyed capability**
//! (the [`A11yTooltipHost`] marker — NOT a bespoke role), exactly the disclosure
//! `A11yExpanded`-keyed `{Expand, Collapse}` precedent:
//!
//! - **Advertise** — the outbound fold (`translate.rs`) advertises `{ShowTooltip,
//!   HideTooltip}` for any node carrying [`A11yTooltipHost`]; `{Focus, Blur}` ride
//!   `Focusable`. So the trigger advertises exactly `{ShowTooltip, HideTooltip,
//!   Focus, Blur}` — and **no `Click`** (it keeps a neutral `A11yRole::Generic`,
//!   so no role contract contributes activation verbs).
//! - **Honor** — the router (`action.rs`) honors `ShowTooltip`/`HideTooltip`
//!   **generically** for any [`A11yTooltipHost`] entity: it shows/hides the
//!   trigger's `A11yRelations.described_by` tooltip node by writing its
//!   [`CssVisibility`] (`Visible`/`Hidden`). Minimal show/hide IS this slice's
//!   scope.
//!
//! Keying the verbs on the marker rather than a bespoke role keeps `contract_for`
//! role-keyed and the capability reusable by any element that hosts a tooltip.
//!
//! **Deferred to C5 (Wave 4) — NOT built here** (co-drive §3 demand-pull): the
//! tooltip **placement / positioning** geometry and the hover/focus **auto-show /
//! dismiss timing**. This slice builds only the bundle + the a11y shape + the
//! `described_by` relation + the minimal `CssVisibility` show/hide honor.

use crate::dismiss::LightDismiss;
use crate::popover::{PopoverPlacement, PopoverSide};
use bevy::picking::Pickable;
use bevy::prelude::*;
use buiy_core::{
    a11y::{A11yLabel, A11yRelations, A11yRole, A11yTooltipHost},
    components::Node,
    focus::Focusable,
    layout::{Anchor, AnchorRef, BoxModel, PositionTry, Stacking, Style, TopLayer, TryCondition},
    render::color::ColorToken,
    render::components::{Background, Border, Corners, CssVisibility, Radius, TextColor},
    text::{FontSize, Text, TextAlign},
};

/// The catalog font size for the tooltip-trigger + tooltip glyphs (logical px).
pub(crate) const TOOLTIP_FONT_SIZE: f32 = 14.0;

/// Tooltip-trigger widget marker. The `#[require(...)]` contract is the single
/// source of the trigger shape (the `Button`/`Disclosure`/… precedent): the bare
/// marker — `world.spawn(TooltipTrigger)` / `bsn! { TooltipTrigger }` —
/// materializes the full layout-visible + paintable + focusable + accessible
/// **trigger** carrying the [`A11yTooltipHost`] capability the contract reads.
///
/// The require list:
/// - `Node` — the layout marker (pulls the full `Style` decomposition).
/// - `BoxModel = tooltip_trigger_box_model()` — the canonical trigger box.
/// - `Background` / `Border` — the trigger fill + rounded edge.
/// - `Focusable` — keyboard-focusable (contributes the implicit `{Focus, Blur}`,
///   and a tooltip shows on focus per WCAG 1.4.13 — the focus driver is C5).
/// - `A11yRole = A11yRole::Generic` — a NEUTRAL role (no role contract), so the
///   trigger advertises NO `Click` (the tooltip verbs ride the marker, not the
///   role). A tooltip trigger is not an activatable widget.
/// - `A11yTooltipHost` — the tooltip-host capability. Its presence is what makes
///   the trigger advertise `{ShowTooltip, HideTooltip}` and what the router honors
///   generically (the state-keyed capability, the disclosure `A11yExpanded`
///   precedent).
/// - `A11yLabel` — the accessible name (`TooltipTrigger::new(label)` fills it).
///
/// The controlled `tooltip` node + the `A11yRelations.described_by` edge are
/// authored by [`TooltipTrigger::new`] / the `tooltip_trigger(...)` scene-fn (they
/// reference the tooltip's entity), not the `#[require]` (which cannot name a
/// sibling).
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
#[require(
    Node,
    BoxModel = tooltip_trigger_box_model(),
    Background = tooltip_trigger_background(),
    Border = tooltip_trigger_border(),
    Focusable,
    A11yRole = A11yRole::Generic,
    A11yTooltipHost,
    A11yLabel,
)]
pub struct TooltipTrigger;

/// The controlled **tooltip** node — a real `A11yRole::Tooltip`, **non-interactive**
/// node the trigger's `A11yRelations.described_by` references. It carries the
/// visible tooltip pixels and starts hidden (`CssVisibility::Hidden`); the router's
/// generic `ShowTooltip`/`HideTooltip` honor flips its `CssVisibility`. A
/// `buiy_widgets`-local marker so the constructor / scene-fn can find it among the
/// trigger's children. Decorative (non-interactive) ⇒ `Pickable::IGNORE` so a hit
/// resolves to the trigger root.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
#[require(Node, A11yRole = A11yRole::Tooltip)]
pub struct TooltipNode;

// The initializer fns are `pub(crate)` so the `scene` module's `tooltip_trigger()`
// scene-fn can spell the SAME canonical values as the `#[require]` path.

/// The canonical tooltip-trigger box: a 24×24 square (meets WCAG 2.5.8 ≥24×24).
// TODO(buiy-widget-catalog-design): replace hardcoded sizes with size tokens.
pub(crate) fn tooltip_trigger_box_model() -> BoxModel {
    Style::default().width_px(24.0).height_px(24.0).box_model
}

/// The default tooltip-trigger fill (the `color.surface.secondary` token).
pub(crate) fn tooltip_trigger_background() -> Background {
    Background {
        color: ColorToken::SurfaceSecondary,
    }
}

/// The default tooltip-trigger border: lightly rounded corners (`radius.sm`).
pub(crate) fn tooltip_trigger_border() -> Border {
    Border {
        radius: Corners::all(Radius::circular(4.0)),
        ..Default::default()
    }
}

/// The tooltip popup box: a wider content strip than the trigger.
pub(crate) fn tooltip_box_model() -> BoxModel {
    Style::default().width_px(160.0).height_px(28.0).box_model
}

/// The tooltip popup fill (the `color.surface.primary` token — a distinct bubble).
pub(crate) fn tooltip_background() -> Background {
    Background {
        color: ColorToken::SurfacePrimary,
    }
}

impl TooltipTrigger {
    /// Spawn-ready bundle for a labelled tooltip trigger + its tooltip. Returns
    /// `impl Bundle` carrying the full trigger contract (the `A11yTooltipHost`
    /// capability + focus + a11y + box) plus the **tooltip** child (a real
    /// `A11yRole::Tooltip` node, `Pickable::IGNORE`) carrying the tip text. The
    /// tooltip starts hidden (`CssVisibility::Hidden`); the router's generic
    /// `ShowTooltip`/`HideTooltip` honor flips its `CssVisibility`.
    ///
    /// The trigger's `A11yRelations.described_by = [tooltip]` is wired by
    /// [`wire_tooltip_described_by`] once the `children!` exist (the tooltip entity
    /// is unknown at construction — the disclosure `controls` precedent).
    ///
    /// **No placement / auto-show timing** — that is C5 (Wave 4); this is the
    /// static a11y shape + the minimal `CssVisibility` show/hide only.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(label: impl Into<String>, tip: impl Into<String>) -> impl Bundle {
        let label = label.into();
        let tip = tip.into();
        (
            TooltipTrigger,
            A11yLabel(label.clone()),
            children![
                // The visible trigger glyph (e.g. "?") — the rendered icon; the
                // accessible name stays on the root `A11yLabel`. Pick-through so a
                // hit resolves to the trigger root.
                (
                    Text(label),
                    FontSize(TOOLTIP_FONT_SIZE),
                    TextColor::default(),
                    TextAlign::Center,
                    Pickable::IGNORE,
                ),
                (
                    // The controlled tooltip — a real Tooltip node, starts hidden.
                    // Decorative (non-interactive) ⇒ `Pickable::IGNORE` so a hit
                    // resolves to the trigger root.
                    TooltipNode,
                    Text(tip.clone()),
                    FontSize(TOOLTIP_FONT_SIZE),
                    TextColor::default(),
                    A11yLabel(tip),
                    tooltip_box_model(),
                    tooltip_background(),
                    CssVisibility::Hidden,
                    Pickable::IGNORE,
                ),
            ],
        )
    }
}

/// The query data [`wire_tooltip_described_by`] reads per trigger: the trigger
/// entity, its `Children` (to find the tooltip), and its current `A11yRelations`
/// (to preserve author-set edges + check idempotency). Aliased so the system
/// signature stays under clippy's `type_complexity` bar.
type TriggerDescribedByData = (Entity, &'static Children, Option<&'static A11yRelations>);

/// The change-detection filter for [`wire_tooltip_described_by`]: a
/// `TooltipTrigger` that just gained its `Children` this frame, so the
/// `described_by` wiring runs once per trigger. Aliased to keep the system
/// signature simple.
type NewlyChildedTrigger = (With<TooltipTrigger>, Added<Children>);

/// Wire each tooltip trigger's `A11yRelations.described_by = [tooltip]` to its
/// [`TooltipNode`] child (Wave-3 slice-5). The `described_by` edge references the
/// tooltip **entity**, which does not exist until the trigger's `children!` are
/// spawned, so it cannot be set in `TooltipTrigger::new`'s bundle / the
/// `#[require]` contract; this system fills it once, on the frame the trigger
/// gains its children — the disclosure `wire_disclosure_controls` precedent.
///
/// This edge is also the **source of truth the router's show/hide honor reads**
/// (`set_described_tooltips_visibility`): `ShowTooltip`/`HideTooltip` resolve the
/// trigger's `described_by` to find which node to show/hide. Gated on
/// `Added<Children>` so it runs once per newly-childed trigger; idempotent over
/// the scene-fn path (which authors `described_by` directly). Registered in
/// `WidgetsPlugin`.
pub fn wire_tooltip_described_by(
    mut commands: Commands,
    triggers: Query<TriggerDescribedByData, NewlyChildedTrigger>,
    tooltips: Query<(), With<TooltipNode>>,
) {
    for (trigger, children, relations) in &triggers {
        // The `described_by` edge is already authored (scene-fn path) ⇒ leave it.
        if relations.is_some_and(|r| !r.described_by.is_empty()) {
            continue;
        }
        let Some(tooltip) = children.iter().find(|&c| tooltips.get(c).is_ok()) else {
            continue; // No tooltip child (a malformed trigger) — nothing to wire.
        };
        // Preserve any other author-set relations; only fill `described_by`.
        let mut next = relations.cloned().unwrap_or_default();
        next.described_by = vec![tooltip];
        commands.entity(trigger).insert(next);
    }
}

/// The gap (logical px) between the trigger edge and the tooltip bubble.
pub(crate) const TOOLTIP_GAP_PX: f32 = 6.0;

/// The tooltip placement chain: below the trigger, else above (the canonical
/// tooltip flip). `pub(crate)` so the scene-fn can spell the same default and a
/// test can assert it.
pub(crate) fn tooltip_placements() -> [PopoverPlacement; 2] {
    [
        PopoverPlacement {
            side: PopoverSide::Bottom,
            gap: TOOLTIP_GAP_PX,
            ..Default::default()
        },
        PopoverPlacement {
            side: PopoverSide::Top,
            gap: TOOLTIP_GAP_PX,
            ..Default::default()
        },
    ]
}

/// Build the tooltip's `Anchor.position_try` chain — below the trigger, flipping
/// above when below would overflow the viewport (the last candidate is
/// unconditional, the least-bad fallback so the tooltip is never stranded at the
/// origin). Mirrors `popover::position_popover`'s lowering for the fixed
/// tooltip placements.
fn tooltip_position_try() -> Vec<PositionTry> {
    let placements = tooltip_placements();
    let last = placements.len() - 1;
    placements
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let conditions = if i == last {
                Vec::new()
            } else {
                vec![TryCondition::FitsInViewport]
            };
            PositionTry {
                inset: p.to_inset(),
                conditions,
            }
        })
        .collect()
}

/// Position each [`TooltipNode`] relative to its trigger (its parent) and place
/// it in the **tooltip top layer** (C5-b, §B.4 — the placement/positioning the
/// P1d slice deferred). On the frame a tooltip node gains its `ChildOf` link to
/// the trigger, this wires:
///
/// - `Anchor { position_anchor: <trigger>, position_try: [below, above] }` — so
///   the existing `anchor_resolution` (layout sub-pass 6d) positions the tooltip
///   adjacent to the trigger (and flips above when below would overflow). This
///   is the SAME layout anchor pipeline the [`Popover`](crate::popover::Popover)
///   primitive lowers onto — one positioning engine.
/// - `Stacking { top_layer: TopLayer::Tooltip }` — the tooltip paints above the
///   page and joins the `TopLayerActivation` deque (so the light-dismiss / Esc
///   handlers can find it as a top-most open overlay).
/// - `LightDismiss { trigger }` — Escape (and an outside press) dismisses the
///   tooltip per WCAG 1.4.13; a press on the trigger does not.
///
/// The query data [`position_tooltip`] reads per tooltip node: the node entity,
/// its `ChildOf` (to find the trigger parent), and its current `Anchor` (for the
/// idempotency check). Aliased so the system signature stays under clippy's
/// `type_complexity` bar (the `TriggerDescribedByData` precedent).
type TooltipPlacementData = (Entity, &'static ChildOf, Option<&'static Anchor>);

/// The change-detection filter for [`position_tooltip`]: a `TooltipNode` that just
/// gained its `ChildOf` link to the trigger this frame, so the placement wiring
/// runs once per tooltip. Aliased to keep the system signature simple.
type NewlyChildedTooltip = (With<TooltipNode>, Added<ChildOf>);

/// Gated on `Added<ChildOf>` for `TooltipNode`s (the tooltip node gains its
/// parent link when the trigger's `children!` spawn), so it wires once per
/// tooltip. Idempotent: a tooltip that already carries an author-set
/// `position_anchor` is left untouched.
pub fn position_tooltip(
    mut commands: Commands,
    tooltips: Query<TooltipPlacementData, NewlyChildedTooltip>,
) {
    for (tooltip, parent, anchor) in &tooltips {
        // Author already anchored this tooltip — leave it.
        if anchor.is_some_and(|a| a.position_anchor.is_some()) {
            continue;
        }
        let trigger = parent.parent();
        let mut next = anchor.cloned().unwrap_or_default();
        next.position_anchor = Some(AnchorRef::Entity(trigger));
        next.position_try = tooltip_position_try();
        commands.entity(tooltip).insert((
            next,
            Stacking {
                top_layer: TopLayer::Tooltip,
                ..Default::default()
            },
            LightDismiss {
                trigger: Some(trigger),
            },
        ));
    }
}
