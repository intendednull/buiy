//! Disclosure widget — Wave-3 slice-3 (P1d bundle + A11yContract + APG keyboard,
//! plus the C4 caret + panel-visibility visual; bundle-then-pixels in one pass).
//!
//! A disclosure is a **trigger** that shows/hides a **panel**. The single
//! disclosure is built here; an accordion (N disclosures) is DEFERRED per the
//! co-drive ledger.
//!
//! **P1d half (the a11y bundle + contract + APG keyboard):** the `Disclosure`
//! marker's `#[require(...)]` assembles the trigger as `A11yRole::Button` (so its
//! `Click` rides the shared **Button** contract → `OnPress`) PLUS the disclosure
//! state [`A11yExpanded`] + `Focusable` + `A11yLabel`. Expandability is modelled as
//! a **state-keyed capability layered on the role contract**, NOT a new role
//! (widget-contracts.md §5 "Disclosure-trigger"):
//!
//! - **Advertise** — the outbound fold (`translate.rs`) advertises `{Expand,
//!   Collapse}` for *any* node carrying `A11yExpanded`, in addition to the role's
//!   contract verbs. So the trigger advertises `{Click, Expand, Collapse, Focus,
//!   Blur}`.
//! - **Honor** — the router (`action.rs`) honors `Expand`/`Collapse` **generically**
//!   (set/clear `A11yExpanded`) for any `A11yExpanded` entity; these are the
//!   absolute AT set-verbs (idempotent at the target state).
//! - **Toggle** — `Click` (pointer / keyboard Enter+Space via the Button keymap /
//!   inbound AT `Action::Click`) lowers into the shared `OnPress` sink, and the
//!   [`advance_expanded_on_press`](crate::advance_expanded_on_press) consumer flips
//!   `A11yExpanded`. So pointer + keyboard + AT-`Click` all toggle through ONE
//!   path, while `Expand`/`Collapse` are the explicit AT set-verbs.
//!
//! Keying expandability on the state (`A11yExpanded`) instead of a bespoke role
//! keeps `contract_for` role-keyed and the capability reusable by any future
//! expandable (a tree item, an accordion section).
//!
//! **C4 half (the pixels + pick-through):** the constructor spawns a decorative
//! **caret** glyph child and a visible **label** `Text` child (both
//! `Pickable::IGNORE` so a hit resolves to the widget root the router addresses)
//! plus the controlled **panel** (`A11yRole::Region`) as a real sibling node, with
//! the trigger's `A11yRelations.controls = [panel]`. [`update_disclosure_visual`]
//! reads `Changed<A11yExpanded>` on each trigger and (a) **rotates** the caret via
//! the decomposed `Rotate` longhand (collapsed → pointing right, expanded → pointing
//! down) and (b) shows/hides the controlled panel via `CssVisibility`. The caret/
//! label carry the *pixels*; the root keeps `A11yLabel` (the AT name).

use bevy::picking::Pickable;
use bevy::prelude::*;
use buiy_core::{
    a11y::{A11yExpanded, A11yLabel, A11yRelations, A11yRole},
    components::Node,
    focus::Focusable,
    layout::{
        AlignItems, BoxModel, Display, FlexAxis, FlexGap, FlexParams, Inset, Length, Position,
        PositionKind, Rotate, Sizing, Style,
    },
    render::color::ColorToken,
    render::components::{Background, Border, Corners, CssVisibility, Radius, TextColor},
    text::{FontSize, Text},
};
use std::f32::consts::FRAC_PI_2;

/// The glyph the disclosure caret shows (a right-pointing triangle). When the
/// disclosure expands, [`update_disclosure_visual`] rotates it 90° clockwise so it
/// points down — the conventional disclosure-triangle affordance.
pub const CARET_GLYPH: &str = "▶";
/// The catalog font size for the disclosure caret + label glyphs (logical px).
pub(crate) const DISCLOSURE_FONT_SIZE: f32 = 16.0;

/// Disclosure-trigger widget marker. The `#[require(...)]` contract is the single
/// source of the trigger shape (the `Button`/`Checkbox`/`Switch`/`Slider`
/// precedent): the bare marker — `world.spawn(Disclosure)` / `bsn! { Disclosure }`
/// — materializes the full layout-visible + paintable + focusable + accessible
/// **trigger** carrying the disclosure state the contract + visual read.
///
/// The require list:
/// - `Node` — the layout marker (pulls the full `Style` decomposition).
/// - `BoxModel = disclosure_box_model()` — the canonical trigger row box.
/// - `Background` / `Border` — the trigger fill + rounded edge.
/// - `Focusable` — keyboard-focusable (contributes the implicit `{Focus, Blur}`).
/// - `A11yRole = A11yRole::Button` — drives `contract_for(Button)` (the `Click`
///   verb + the APG Enter+Space keymap). Expandability is layered *on top* via
///   `A11yExpanded`, not via the role.
/// - `A11yExpanded` — the disclosure state (defaults to `false` / collapsed). Its
///   presence is what makes the trigger advertise `{Expand, Collapse}` and honor
///   them (the state-keyed capability).
/// - `A11yLabel` — the accessible name (`Disclosure::new(label)` fills it).
///
/// The controlled `panel` + the `A11yRelations.controls` edge are authored by
/// [`Disclosure::new`] / the `disclosure(...)` scene-fn (they need the panel's
/// entity), not the `#[require]` (which cannot reference a sibling entity).
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component, Default)]
#[require(
    Node,
    BoxModel = disclosure_box_model(),
    Background = disclosure_background(),
    Border = disclosure_border(),
    // Lay the [caret, label] HEADER out as a centered row (like every other
    // labelled widget). The controlled panel is the 3rd child but is taken OUT of
    // this flow (`DisclosurePanel` is `Position::Absolute`), so the header row is
    // just [caret, label] and the panel sits BELOW. `Position::Relative` makes the
    // trigger the absolute panel's containing block.
    Display = Display::flex_row(),
    FlexParams = disclosure_row_flex(),
    Position = disclosure_relative_position(),
    Focusable,
    A11yRole = A11yRole::Button,
    A11yExpanded,
    A11yLabel,
)]
pub struct Disclosure;

/// The decorative **caret** child of a disclosure trigger — the right/down triangle
/// whose `Rotate` [`update_disclosure_visual`] drives from `A11yExpanded`. A
/// `buiy_widgets`-local **visual** marker; it defines no a11y component (the a11y
/// state lives on the trigger root). Decorative ⇒ `Pickable::IGNORE` so a click on
/// the caret resolves to the widget root.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
pub struct DisclosureCaret;

/// The controlled **panel** of a disclosure — a real `A11yRole::Region` node the
/// trigger's `A11yRelations.controls` references. Unlike the caret/label, the panel
/// is a real semantic node (it can contain interactive content), so it is NOT
/// `Pickable::IGNORE`. Its `CssVisibility` is driven from the trigger's
/// `A11yExpanded` by [`update_disclosure_visual`]: hidden when collapsed, visible
/// when expanded. A `buiy_widgets`-local marker so the visual can find it among the
/// trigger's children.
#[derive(Component, Reflect, Default, Clone, Copy, Debug)]
#[reflect(Component, Default)]
#[require(Node, A11yRole = A11yRole::Region, Position = disclosure_panel_position())]
pub struct DisclosurePanel;

// The initializer fns are `pub(crate)` so the `scene` module's `disclosure()`
// scene-fn can spell the SAME canonical values as the `#[require]` path.

/// The canonical disclosure trigger box: a 160×24 row (the label + caret sit in it).
// TODO(buiy-widget-catalog-design): replace hardcoded sizes with size tokens.
pub(crate) fn disclosure_box_model() -> BoxModel {
    Style::default().width_px(160.0).height_px(24.0).box_model
}

/// The trigger HEADER row: `[caret, label]` laid out horizontally, vertically
/// centered, with a small gap. (The controlled panel is out of this flow.)
pub(crate) fn disclosure_row_flex() -> FlexParams {
    FlexParams {
        direction: FlexAxis::Row,
        align_items: AlignItems::Center,
        gap: FlexGap {
            row: Length::px(6.0),
            column: Length::px(6.0),
        },
        ..Default::default()
    }
}

/// `Position::Relative` on the trigger — it moves nowhere (default inset) but
/// becomes the containing block for the absolutely-positioned panel below.
pub(crate) fn disclosure_relative_position() -> Position {
    Position {
        kind: PositionKind::Relative,
        inset: Inset::default(),
    }
}

/// The controlled panel is `Position::Absolute`, anchored just BELOW the 24px
/// header (top inset = the header height), so it is OUT of the header row's flow
/// (collapsed ⇒ the trigger is a clean `[caret, label]` row) and drops below the
/// trigger when revealed.
pub(crate) fn disclosure_panel_position() -> Position {
    Position {
        kind: PositionKind::Absolute,
        inset: Inset {
            top: Sizing::Length(Length::px(24.0)),
            left: Sizing::Length(Length::px(0.0)),
            ..Default::default()
        },
    }
}

/// The default disclosure trigger fill (the `color.surface.secondary` token).
pub(crate) fn disclosure_background() -> Background {
    Background {
        color: ColorToken::SurfaceSecondary,
    }
}

/// The default disclosure trigger border: lightly rounded corners (`radius.sm`).
pub(crate) fn disclosure_border() -> Border {
    Border {
        radius: Corners::all(Radius::circular(4.0)),
        ..Default::default()
    }
}

/// The controlled panel box: full width, a content row taller than the trigger.
pub(crate) fn disclosure_panel_box_model() -> BoxModel {
    Style::default().width_px(160.0).height_px(48.0).box_model
}

/// The panel surface fill (the `color.surface.primary` token — a distinct panel).
pub(crate) fn disclosure_panel_background() -> Background {
    Background {
        color: ColorToken::SurfacePrimary,
    }
}

/// The caret's `Rotate` for the collapsed state: identity (the `▶` glyph points
/// right). The single source of the collapsed caret orientation, shared by the
/// constructor / scene-fn / visual so spawn and the first frame agree.
pub fn caret_rotation_collapsed() -> Rotate {
    Rotate(Quat::IDENTITY)
}

/// The caret's `Rotate` for the expanded state: a 90° turn about z so the `▶`
/// glyph points **down** (the conventional expanded disclosure triangle). The
/// single source of the expanded caret orientation.
pub fn caret_rotation_expanded() -> Rotate {
    // +z turns ▶ to ▼ under the render's affine convention (y-down screen space,
    // columns [m00,m10,m01,m11] from GlobalTransform's linear part): the tip at
    // +x maps to +y (down). Empirically verified once the coverage path applied
    // the affine (before that, the rotation was dropped and never rendered).
    Rotate(Quat::from_rotation_z(FRAC_PI_2))
}

impl Disclosure {
    /// Spawn-ready bundle for a labelled disclosure. Returns `impl Bundle` carrying
    /// the full trigger contract (role `Button` + the `A11yExpanded` state + focus +
    /// a11y + box) plus three children: the decorative **caret** glyph, the visible
    /// **label** `Text` (both `Pickable::IGNORE` so a hit resolves to the trigger
    /// root — pick-through, co-drive SC-3), and the controlled **panel**
    /// (`A11yRole::Region`). The trigger's `A11yRelations.controls = [panel]` is set
    /// so an AT/agent reads the disclosure→panel edge.
    ///
    /// The caret starts collapsed (`Rotate` identity ⇒ pointing right) and the panel
    /// starts hidden (`CssVisibility::Hidden`) because the default `A11yExpanded` is
    /// `false`; [`update_disclosure_visual`] rotates the caret + reveals the panel on
    /// the first `Changed<A11yExpanded>` once the state flips.
    #[allow(clippy::new_ret_no_self)]
    pub fn new(label: impl Into<String>) -> impl Bundle {
        let label = label.into();
        (
            Disclosure,
            A11yLabel(label.clone()),
            // The trigger CONTROLS the panel; the panel id is resolved after spawn
            // via the `children!` order (the panel is the third child). The
            // `controls` edge is wired by `wire_disclosure_controls` once the
            // children exist (a `#[require]`/closure cannot reference a sibling
            // entity at construction).
            children![
                // The decorative caret — starts collapsed (pointing right).
                (
                    DisclosureCaret,
                    Text(CARET_GLYPH.into()),
                    FontSize(DISCLOSURE_FONT_SIZE),
                    TextColor::default(),
                    caret_rotation_collapsed(),
                    Pickable::IGNORE,
                ),
                // The visible label pixels (the AT name stays on the trigger root).
                (
                    Text(label),
                    FontSize(DISCLOSURE_FONT_SIZE),
                    TextColor::default(),
                    Pickable::IGNORE,
                ),
                // The controlled panel (a real Region) — starts hidden (collapsed).
                (
                    DisclosurePanel,
                    Node,
                    disclosure_panel_box_model(),
                    disclosure_panel_background(),
                    CssVisibility::Hidden,
                ),
            ],
        )
    }

    /// Read whether the disclosure is **expanded** from its [`A11yExpanded`] state
    /// (Track C domain accessor — `Query<&A11yExpanded, With<Disclosure>>` then
    /// `Disclosure::expanded(e)`).
    pub fn expanded(state: &A11yExpanded) -> bool {
        state.0
    }
}

/// The query data [`wire_disclosure_controls`] reads per trigger: the trigger
/// entity, its `Children` (to find the panel), and its current `A11yRelations` (to
/// preserve author-set edges + check idempotency). Aliased so the system signature
/// stays under clippy's `type_complexity` bar.
type TriggerControlsData = (Entity, &'static Children, Option<&'static A11yRelations>);

/// The change-detection filter for [`wire_disclosure_controls`]: a `Disclosure`
/// that just gained its `Children` this frame (the `children!` macro inserts
/// `Children` after the root spawns), so the panel-edge wiring runs once per
/// trigger. Aliased to keep the system signature simple.
type NewlyChildedTrigger = (With<Disclosure>, Added<Children>);

/// Wire each disclosure trigger's `A11yRelations.controls` to its
/// [`DisclosurePanel`] child (Wave-3 slice-3). The `controls` edge references the
/// panel **entity**, which does not exist until the trigger's `children!` are
/// spawned, so it cannot be set in `Disclosure::new`'s bundle / the `#[require]`
/// contract; this system fills it once, on the frame the trigger gains its
/// children.
///
/// Gated on `Added<Children>` so it runs once per newly-childed trigger (the
/// `children!` macro inserts `Children` after the root spawns). It inserts the
/// `A11yRelations { controls: [panel] }` edge if the trigger does not already
/// carry it (the scene-fn path authors `controls` directly, so this is idempotent
/// — it never overwrites an author-set edge). Registered in `WidgetsPlugin`.
pub fn wire_disclosure_controls(
    mut commands: Commands,
    triggers: Query<TriggerControlsData, NewlyChildedTrigger>,
    panels: Query<(), With<DisclosurePanel>>,
) {
    for (trigger, children, relations) in &triggers {
        // The `controls` edge is already authored (scene-fn path) ⇒ leave it.
        if relations.is_some_and(|r| !r.controls.is_empty()) {
            continue;
        }
        let Some(panel) = children.iter().find(|&c| panels.get(c).is_ok()) else {
            continue; // No panel child (a malformed trigger) — nothing to wire.
        };
        // Preserve any other author-set relations; only fill `controls`.
        let mut next = relations.cloned().unwrap_or_default();
        next.controls = vec![panel];
        commands.entity(trigger).insert(next);
    }
}

/// The change-detection filter for [`update_disclosure_visual`]: a `Disclosure`
/// whose `A11yExpanded` changed this frame. Aliased so the system signature stays
/// under clippy's `type_complexity` bar.
type ChangedDisclosure = (With<Disclosure>, Changed<A11yExpanded>);

/// C4 visual system: drive each disclosure's caret rotation + panel visibility from
/// its `A11yExpanded` state, gated on `Changed<A11yExpanded>` so the repaint fires
/// exactly once per flip (the single source of truth is the agent-interface state;
/// a flip from any modality — pointer, keyboard Enter/Space, an inbound AT `Click`
/// via the `OnPress` consumer, or an absolute AT `Expand`/`Collapse` via the router
/// — lands in this `A11yExpanded` write, and this system reacts).
///
/// For each changed trigger, walk its `Children`:
/// - the [`DisclosureCaret`] child gets its `Rotate` set —
///   [`caret_rotation_expanded`] (points down) when expanded,
///   [`caret_rotation_collapsed`] (points right) when collapsed.
/// - the [`DisclosurePanel`] child gets its `CssVisibility` set — `Visible` when
///   expanded, `Hidden` when collapsed. `CssVisibility::Hidden` keeps the layout
///   box + the a11y presence (the panel stays in the tree, controlled-but-hidden);
///   only paint is skipped.
pub fn update_disclosure_visual(
    changed: Query<(&A11yExpanded, &Children), ChangedDisclosure>,
    mut carets: Query<&mut Rotate, With<DisclosureCaret>>,
    mut panels: Query<&mut CssVisibility, With<DisclosurePanel>>,
) {
    for (expanded, children) in &changed {
        let rotation = if expanded.0 {
            caret_rotation_expanded()
        } else {
            caret_rotation_collapsed()
        };
        let visibility = if expanded.0 {
            CssVisibility::Visible
        } else {
            CssVisibility::Hidden
        };
        for &child in children {
            if let Ok(mut rotate) = carets.get_mut(child) {
                *rotate = rotation.clone();
            }
            if let Ok(mut vis) = panels.get_mut(child) {
                *vis = visibility;
            }
        }
    }
}
