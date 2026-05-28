//! Decomposed layout components.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 2.1.
//!
//! Each component is small, public-fielded, and derives
//! `Reflect + Default + Clone + Component`. Phase 1 covers the surface
//! Phase 0's mega-`Style` reaches: `BoxModel`, `Display`, `Position`,
//! `FlexParams`, `FlexItem`. Other components (`Anchor`, `GridParams`,
//! `Container`, `WritingMode`, `Overflow`, `Scroll`, `Stacking`,
//! `Transform`, `Containment`, `MultiColumn`, `GridItem`) land in their
//! respective phase plans (see foundation plan §"Phasing strategy").

use super::types::{
    AlignContent, AlignItems, AnchorName, AnchorRef, AspectRatio, BoxSizing, BreakAfter,
    BreakBefore, BreakInside, ColumnCount, ColumnFill, ColumnRule, ColumnSpan, ContainerType,
    Direction, Edges, FlexAxis, FlexGap, FlexWrap, GridAreas, GridAutoFlow, GridLine, Inset,
    JustifyContent, JustifyItems, Length, OverflowMode, OverscrollBehavior, PositionKind,
    PositionTry, QueryCondition, ScrollBehavior, ScrollbarColor, ScrollbarGutter, ScrollbarWidth,
    Sizing, SnapAlign, SnapStop, SnapType, TextOrientation, TrackSize, UnicodeBidi,
    WritingModeKind,
};
use bevy::prelude::*;

/// Box-model dimensions: width / height (incl. min/max), padding, margin,
/// border, box-sizing, aspect-ratio.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/box-model.md § 2.
///
/// Phase 1 omits the spec's `gap` / `row_gap` / `column_gap` fields —
/// they are not yet wired to Taffy and `FlexParams.gap` carries the
/// flex-gap surface in this phase. A follow-up phase that wires
/// block-layout gap to Taffy adds them back.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct BoxModel {
    pub width: Sizing,
    pub height: Sizing,
    pub min_width: Sizing,
    pub min_height: Sizing,
    pub max_width: Sizing,
    pub max_height: Sizing,
    pub padding: Edges,
    pub margin: Edges,
    pub border: Edges,
    pub box_sizing: BoxSizing,
    pub aspect_ratio: Option<AspectRatio>,
}

/// `display` value. `Block` and `Flex(FlexAxis)` translate directly;
/// `Grid` / `InlineGrid` translate to `taffy::Display::Grid` (Phase 3;
/// Taffy 0.10 has no inline-grid variant). `Table*` variants are flagged
/// by the `table_layout` sub-pass 6b `warn!` and fall back to `Block`.
/// Remaining variants are reserved and translate to `Block` (Taffy default).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 1.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)]
pub enum Display {
    #[default]
    Block,
    Inline,
    InlineBlock,
    Flex(FlexAxis),
    InlineFlex(FlexAxis),
    Grid,
    InlineGrid,
    FlowRoot,
    Contents,
    Table,
    TableRowGroup,
    TableHeaderGroup,
    TableFooterGroup,
    TableRow,
    TableCell,
    TableCaption,
    TableColumnGroup,
    TableColumn,
    ListItem,
    Ruby,
    None,
}

impl Display {
    pub const fn flex_row() -> Self {
        Self::Flex(FlexAxis::Row)
    }

    pub const fn flex_column() -> Self {
        Self::Flex(FlexAxis::Column)
    }
}

/// `position` + `inset`. Phase 1 implements `Static`, `Relative`,
/// `Absolute`. `Sticky` is fully implemented (Phase 7) as a post-Taffy
/// overlay — sub-pass 6a (`sticky_offset`) computes the sticky
/// displacement against the nearest scroll container's reference frame.
/// `Fixed` still ships as a variant that translates to `Absolute`;
/// Phase 8 wires its real (viewport / transformed-ancestor) semantics.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 2.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct Position {
    pub kind: PositionKind,
    pub inset: Inset,
}

/// Flex container parameters. Active when the entity's `Display` is
/// `Display::Flex(_)` or `Display::InlineFlex(_)`; otherwise ignored.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 1.1.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct FlexParams {
    pub direction: FlexAxis,
    pub wrap: FlexWrap,
    pub justify_content: JustifyContent,
    pub align_items: AlignItems,
    pub align_content: AlignContent,
    pub gap: FlexGap,
}

/// Per-child flex parameters.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 1.2.
#[derive(Component, Reflect, Clone, Copy, Debug, PartialEq)]
#[reflect(Component)]
pub struct FlexItem {
    pub grow: f32,
    pub shrink: f32,
    pub basis: Sizing,
    pub order: i32,
    pub align_self: Option<AlignItems>,
}

impl Default for FlexItem {
    fn default() -> Self {
        Self {
            grow: 0.0,
            shrink: 1.0,
            basis: Sizing::Auto,
            order: 0,
            align_self: None,
        }
    }
}

/// Per-axis overflow handling and scroll/scrollbar configuration. CSS
/// `overflow`, `scrollbar-*`, `scroll-behavior`, `overscroll-behavior`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md § 1.
///
/// Phase 2 wires `x` / `y` and `scrollbar_width` to Taffy. The other
/// fields are stored for downstream consumers (render: `scrollbar_color`,
/// animate: `scroll_behavior`, input: `overscroll_*`). `scrollbar_gutter`
/// is stored but `Stable` does not yet reserve space on non-scrolling
/// containers — see plan coverage map.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct Overflow {
    pub x: OverflowMode,
    pub y: OverflowMode,
    pub scrollbar_gutter: ScrollbarGutter,
    pub scrollbar_width: ScrollbarWidth,
    pub scrollbar_color: ScrollbarColor,
    pub scroll_behavior: ScrollBehavior,
    pub overscroll_x: OverscrollBehavior,
    pub overscroll_y: OverscrollBehavior,
}

impl Overflow {
    /// True iff either axis is `Scroll` or `Auto`. Scroll containers
    /// establish a scroll viewport and a containing block for descendants
    /// with `Position::Sticky` (consumer: Phase 7 sub-pass 6a, called via
    /// `nearest_scroll_container` in `systems.rs`).
    ///
    /// Spec: docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md § 1.2.
    pub fn is_scroll_container(&self) -> bool {
        matches!(self.x, OverflowMode::Scroll | OverflowMode::Auto)
            || matches!(self.y, OverflowMode::Scroll | OverflowMode::Auto)
    }
}

/// Scroll-snap container settings. CSS `scroll-snap-type`,
/// `scroll-padding`, `scroll-margin`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md § 3.
///
/// Phase 2 stores; the snap-point math runs in
/// `buiy-input-events-design`'s scroll handler.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct Scroll {
    pub snap_type: SnapType,
    pub snap_padding: Edges,
    pub snap_margin: Edges,
}

/// Grid container parameters. Active when the entity's `Display` is
/// `Display::Grid` or `Display::InlineGrid`; otherwise ignored.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 2.1.
///
/// `template_areas` carries explicit rectangles plus an optional CSS-string
/// constructor (`GridAreas::from_lines`). Named-area resolution for child
/// `GridLine::Area(name)` happens Buiy-side at `sync_styles` time —
/// Taffy 0.10 has no native named-area placement, only named lines.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct GridParams {
    pub template_columns: Vec<TrackSize>,
    pub template_rows: Vec<TrackSize>,
    pub template_areas: Option<GridAreas>,
    pub auto_columns: Vec<TrackSize>,
    pub auto_rows: Vec<TrackSize>,
    pub auto_flow: GridAutoFlow,
    pub justify_items: JustifyItems,
    pub align_items: AlignItems,
    pub justify_content: JustifyContent,
    pub align_content: AlignContent,
    pub gap: FlexGap,
}

/// Per-child grid placement and self-alignment.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 2.2.
/// Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 2.4
/// (decomposed-only convention).
///
/// Decomposed-only — not in `Style`'s Bundle. Following the `FlexItem` /
/// `ScrollSnapItem` pattern: spawn alongside `Style` rather than nested.
/// `column.Area(name)` and `row.Area(name)` resolve against the parent's
/// `GridParams.template_areas`; mismatched names emit one `warn!` and
/// fall back to `GridLine::Auto`.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct GridItem {
    pub column: GridLine,
    pub row: GridLine,
    pub justify_self: Option<JustifyItems>,
    pub align_self: Option<AlignItems>,
}

/// CSS writing-mode + direction + text-orientation + unicode-bidi, all on
/// one component because they're authored together. Joins `Style`'s
/// Bundle. The inherited effective value is computed by the
/// `inherit_writing_mode` system and stored in `WritingModeResolved`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 2.1.
///
/// Note: vertical writing-modes (`VerticalRl` / `VerticalLr`) do *not*
/// reorient the Taffy main axis — Taffy 0.10 has no writing-mode
/// awareness at the layout-engine level. Vertical modes are honored only
/// by the `LogicalBoxModel` / `LogicalInset` ergonomic builders. Authors
/// who want top-to-bottom flow under vertical-rl use
/// `Display::Flex(Column)` explicitly. See plan § Decisions made #5.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component, Default)]
pub struct WritingMode {
    pub mode: WritingModeKind,
    pub direction: Direction,
    pub text_orientation: TextOrientation,
    pub unicode_bidi: UnicodeBidi,
}

/// Inherited effective writing-mode for an entity. Synced by the
/// `inherit_writing_mode` system in `BuiyLayoutStep::WritingModeInherit`,
/// run before `SyncStyles`. **Private cache — not author-set, not in
/// `Style`'s Bundle.**
///
/// The translation layer (`style_to_taffy`) reads this value to wire
/// `Direction::Rtl` to `taffy::Style.direction` and to gate `Sideways{Rl,Lr}`
/// through the warn-once fallback. The `LogicalBoxModel` and `LogicalInset`
/// builders take a `&WritingMode` directly (not the Resolved cache),
/// because they translate at construct time on the author's side.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 2.2.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component, Default)]
pub struct WritingModeResolved {
    pub mode: WritingModeKind,
    pub direction: Direction,
    pub text_orientation: TextOrientation,
    pub unicode_bidi: UnicodeBidi,
}

impl WritingModeResolved {
    /// Construct from a parent `WritingMode`. Used by the inheritance
    /// system to copy fields one-to-one.
    pub(crate) fn from_writing_mode(wm: &WritingMode) -> Self {
        Self {
            mode: wm.mode,
            direction: wm.direction,
            text_orientation: wm.text_orientation,
            unicode_bidi: wm.unicode_bidi,
        }
    }
}

/// Marks an entity as a CSS container (or not). Descendants resolve
/// `@container` rules and container units (`cqw`, `cqi`, ...) against
/// the nearest ancestor whose `container_type` is `Size` or `InlineSize`.
///
/// `container_name` is an optional opaque label (CSS `container-name`).
/// When set, descendant `ContainerQuery` rules with `container:
/// Some(name)` match this container by name; rules with `container: None`
/// match the nearest queried ancestor regardless of name. `String` is used
/// for the same reason as `GridLine::Area` (Phase 3): avoids a new direct
/// `SmolStr` dep, and container names are set at spawn time, not on a hot
/// path.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.1.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq, Eq)]
#[reflect(Component, Default)]
pub struct Container {
    pub container_type: ContainerType,
    pub container_name: Option<String>,
}

/// CSS multi-column container (tier-E).
///
/// **Status:** API stub. v1 ships every field for forward
/// compatibility, but the multi-column packing algorithm is a no-op
/// — sub-pass 6c emits one `warn!` per session on the first
/// `MultiColumn` entity it encounters and produces single-column
/// layout. Authors can write multi-column-aware code that compiles
/// against v1; the algorithm lands in a v1.x point release.
///
/// `Eq` is intentionally NOT derived: `column_width` and `column_gap`
/// carry `Length` values which contain `f32`, and `column_rule.color`
/// is a `bevy::color::Color` which lacks `Eq`. `PartialEq` is
/// sufficient for tests and authoring ergonomics.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.
///
/// Sub-pass: 6c (`multicol_pack`) in `BuiyLayoutStep::PostTaffyOverrides`.
#[derive(Component, Reflect, Clone, Copy, PartialEq, Debug, Default)]
#[reflect(Component, Default)]
pub struct MultiColumn {
    pub column_count: ColumnCount,
    pub column_width: Option<Length>,
    pub column_gap: Option<Length>,
    pub column_rule: ColumnRule,
    pub column_span: ColumnSpan,
    pub column_fill: ColumnFill,
    pub break_inside: BreakInside,
    pub break_before: BreakBefore,
    pub break_after: BreakAfter,
}

/// A `@container` rule pinned to a single entity. The rule activates
/// when *all* `conditions` hold against the resolved size of the
/// matched query container (by name, or nearest queried ancestor when
/// `container` is `None`).
///
/// When the rule's activation state flips, `cq_activate` toggles
/// `ContainerQueryActive` <-> `ContainerQueryInactive` on this same
/// entity. Authors observe those markers and react however they want —
/// the spec calls out (§ 1.2 last paragraph) that style-bundle
/// application is consumer-responsibility.
///
/// v1 stores at most one `ContainerQuery` per entity (Bevy's
/// `Component` is single-instance).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.2.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct ContainerQuery {
    /// `None` = nearest queried ancestor. `Some(name)` = nearest
    /// ancestor with `Container { container_name: Some(name), .. }`.
    pub container: Option<String>,
    /// All conditions must hold for the rule to be active. Empty list
    /// = always active (matches CSS `@container (width)` which is
    /// always true if there's a container at all — Phase 5 simplifies
    /// to "always active").
    pub conditions: Vec<QueryCondition>,
}

/// Marker — set by `cq_activate` when the entity's `ContainerQuery`
/// matched its container's resolved size on the current activation
/// pass. Mutually exclusive with `ContainerQueryInactive`.
///
/// Authors observe `With<ContainerQueryActive>` to apply whatever
/// behavior they want on activation. Spec § 1.2: style-bundle
/// application is consumer-responsibility.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component, Default)]
pub struct ContainerQueryActive;

/// Marker — set by `cq_activate` when the entity's `ContainerQuery`
/// did *not* match. Mutually exclusive with `ContainerQueryActive`.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
#[reflect(Component, Default)]
pub struct ContainerQueryInactive;

/// Runtime scroll position of a scroll container. Mutated by the
/// scroll-input handler in `buiy-input-events-design`. Read by render
/// (drawing) and picking (hit-testing) at consume time, and by Phase 7
/// sub-pass 6a (`StickyOffset`).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md § 2.
///
/// **Mutating `ScrollOffset` must NOT invalidate `ResolvedLayout`.** The
/// invariant is enforced by excluding `Changed<ScrollOffset>` from the
/// `sync_styles` trigger filter (see the `Or<(Changed<...>)>` change
/// filter on `sync_styles` in `systems.rs`) and asserted by
/// `tests/layout_scroll_offset_no_invalidate.rs`.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct ScrollOffset {
    pub x: f32,
    pub y: f32,
}

/// Per-snap-item child-side configuration. Lives on each child of a
/// scroll container that participates in snap.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md § 3.
/// Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 2.4 (decomposed-only convention).
///
/// Decomposed-only — not in `Style`'s Bundle. Following the `FlexItem`
/// pattern: spawn alongside `Style` rather than nested. The snap-point
/// math runs in `buiy-input-events-design`'s scroll handler at consume
/// time; this component stores the per-item declaration.
#[derive(Component, Reflect, Default, Clone, Copy, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct ScrollSnapItem {
    pub align: SnapAlign,
    pub stop: SnapStop,
}

/// CSS anchor positioning — declares this entity as an anchor target
/// (via `anchor_name`) and/or anchors this entity TO another (via
/// `position_anchor`). When `position_anchor.is_some()`, the
/// `anchor_resolution` system (sub-pass 6d) overrides this entity's
/// `ResolvedLayout.position` by walking the `position_try` chain and
/// applying the first try whose conditions all pass.
///
/// Decomposed-only by spec § 2.4: not folded into the `Style` Bundle
/// because anchored elements are rare (tooltips, popovers) and each
/// carries a non-trivial `position_try` chain. Spawn alongside `Style`:
///
/// ```ignore
/// commands.spawn((
///     Style::default(),
///     Anchor {
///         position_anchor: Some(AnchorRef::Name("submit-btn".into())),
///         position_try: vec![PositionTry { /* ... */ }],
///         ..default()
///     },
/// ));
/// ```
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.1.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct Anchor {
    /// Names this entity AS an anchor (so other entities can reference
    /// it via `AnchorRef::Name`). `None` means the entity is not a
    /// named anchor target (but can still be a target via direct
    /// `AnchorRef::Entity(_)` references).
    pub anchor_name: Option<AnchorName>,
    /// Declares that this entity is anchored TO another. `None` means
    /// the entity participates in normal layout. `Some(_)` triggers
    /// the anchor-resolution pass for this entity.
    pub position_anchor: Option<AnchorRef>,
    /// Ordered fallback chain. The first try whose `conditions` all
    /// pass wins; if every try fails, the entity gets a
    /// `LayoutAnchorBroken` marker and `ResolvedLayout.position`
    /// defaults to `(0, 0)`.
    pub position_try: Vec<PositionTry>,
}

/// Devtools marker — present when this entity's anchor resolution
/// failed this frame (target missing, every fallback failed, or in a
/// cycle whose edge was dropped). Idempotent: present iff broken,
/// absent iff resolved. Authors observe `With<LayoutAnchorBroken>` to
/// surface broken anchors in inspectors.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.2 step 4.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct LayoutAnchorBroken;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_model_default_is_auto_zero_padding() {
        let bm = BoxModel::default();
        assert_eq!(bm.width, Sizing::Auto);
        assert_eq!(bm.height, Sizing::Auto);
        assert_eq!(bm.padding, Edges::ZERO);
        assert_eq!(bm.margin, Edges::ZERO);
        assert_eq!(bm.border, Edges::ZERO);
        assert_eq!(bm.box_sizing, BoxSizing::ContentBox);
        assert_eq!(bm.aspect_ratio, None);
    }

    #[test]
    fn display_default_is_block() {
        assert_eq!(Display::default(), Display::Block);
    }

    #[test]
    fn position_default_is_static_with_auto_inset() {
        let pos = Position::default();
        assert_eq!(pos.kind, PositionKind::Static);
        assert_eq!(pos.inset, Inset::default());
    }

    #[test]
    fn flex_params_and_item_defaults_match_spec() {
        let fp = FlexParams::default();
        assert_eq!(fp.direction, FlexAxis::Row);
        assert_eq!(fp.wrap, FlexWrap::NoWrap);
        assert_eq!(fp.justify_content, JustifyContent::FlexStart);
        assert_eq!(fp.align_items, AlignItems::Stretch);
        assert_eq!(fp.align_content, AlignContent::Stretch);
        assert_eq!(fp.gap, FlexGap::default());

        let fi = FlexItem::default();
        assert_eq!(fi.grow, 0.0);
        assert_eq!(fi.shrink, 1.0);
        assert_eq!(fi.basis, Sizing::Auto);
        assert_eq!(fi.order, 0);
        assert_eq!(fi.align_self, None);
    }

    #[test]
    fn display_helpers_produce_flex_axis() {
        assert_eq!(Display::flex_row(), Display::Flex(FlexAxis::Row));
        assert_eq!(Display::flex_column(), Display::Flex(FlexAxis::Column));
    }

    #[test]
    fn overflow_default_is_visible_both_axes() {
        let o = Overflow::default();
        assert_eq!(o.x, OverflowMode::Visible);
        assert_eq!(o.y, OverflowMode::Visible);
        assert_eq!(o.scrollbar_gutter, ScrollbarGutter::Auto);
        assert_eq!(o.scrollbar_width, ScrollbarWidth::Auto);
        assert_eq!(o.scrollbar_color, ScrollbarColor::Auto);
        assert_eq!(o.scroll_behavior, ScrollBehavior::Auto);
        assert_eq!(o.overscroll_x, OverscrollBehavior::Auto);
        assert_eq!(o.overscroll_y, OverscrollBehavior::Auto);
    }

    #[test]
    fn overflow_is_scroll_container_only_when_either_axis_scrolls() {
        assert!(!Overflow::default().is_scroll_container());
        assert!(
            !Overflow {
                x: OverflowMode::Hidden,
                y: OverflowMode::Hidden,
                ..Default::default()
            }
            .is_scroll_container()
        );
        assert!(
            Overflow {
                x: OverflowMode::Scroll,
                ..Default::default()
            }
            .is_scroll_container()
        );
        assert!(
            Overflow {
                y: OverflowMode::Auto,
                ..Default::default()
            }
            .is_scroll_container()
        );
        assert!(
            Overflow {
                x: OverflowMode::Auto,
                y: OverflowMode::Scroll,
                ..Default::default()
            }
            .is_scroll_container()
        );
    }

    #[test]
    fn scroll_default_is_no_snap_zero_padding() {
        let s = Scroll::default();
        assert_eq!(s.snap_type, SnapType::None);
        assert_eq!(s.snap_padding, Edges::ZERO);
        assert_eq!(s.snap_margin, Edges::ZERO);
    }

    #[test]
    fn scroll_offset_default_is_origin() {
        let s = ScrollOffset::default();
        assert_eq!(s.x, 0.0);
        assert_eq!(s.y, 0.0);
    }

    #[test]
    fn scroll_snap_item_default_is_none_normal() {
        let s = ScrollSnapItem::default();
        assert_eq!(s.align, SnapAlign::None);
        assert_eq!(s.stop, SnapStop::Normal);
    }

    #[test]
    fn grid_params_default_is_empty_templates_row_flow() {
        let g = GridParams::default();
        assert!(g.template_columns.is_empty());
        assert!(g.template_rows.is_empty());
        assert!(g.template_areas.is_none());
        assert!(g.auto_columns.is_empty());
        assert!(g.auto_rows.is_empty());
        assert_eq!(g.auto_flow, GridAutoFlow::Row);
        assert_eq!(g.justify_items, JustifyItems::Stretch);
        assert_eq!(g.align_items, AlignItems::Stretch);
        assert_eq!(g.justify_content, JustifyContent::FlexStart);
        assert_eq!(g.align_content, AlignContent::Stretch);
        assert_eq!(g.gap, FlexGap::default());
    }

    #[test]
    fn grid_item_default_is_auto_lines_no_self() {
        let g = GridItem::default();
        assert_eq!(g.column, GridLine::Auto);
        assert_eq!(g.row, GridLine::Auto);
        assert_eq!(g.justify_self, None);
        assert_eq!(g.align_self, None);
    }

    #[test]
    fn writing_mode_default_is_horizontal_tb_ltr_mixed_normal() {
        let wm = WritingMode::default();
        assert_eq!(wm.mode, WritingModeKind::HorizontalTb);
        assert_eq!(wm.direction, Direction::Ltr);
        assert_eq!(wm.text_orientation, TextOrientation::Mixed);
        assert_eq!(wm.unicode_bidi, UnicodeBidi::Normal);
    }

    #[test]
    fn writing_mode_resolved_default_is_horizontal_tb_ltr_mixed_normal() {
        let wm = WritingModeResolved::default();
        assert_eq!(wm.mode, WritingModeKind::HorizontalTb);
        assert_eq!(wm.direction, Direction::Ltr);
        assert_eq!(wm.text_orientation, TextOrientation::Mixed);
        assert_eq!(wm.unicode_bidi, UnicodeBidi::Normal);
    }

    #[test]
    fn container_default_is_normal_unnamed() {
        let c = Container::default();
        assert_eq!(c.container_type, ContainerType::Normal);
        assert_eq!(c.container_name, None);
    }

    #[test]
    fn container_query_default_is_anonymous_and_empty() {
        let q = ContainerQuery::default();
        assert_eq!(q.container, None);
        assert!(q.conditions.is_empty());
    }

    #[test]
    fn container_query_active_inactive_are_distinct_markers() {
        let _a = ContainerQueryActive;
        let _i = ContainerQueryInactive;
    }

    #[test]
    fn anchor_default_is_empty() {
        let a = Anchor::default();
        assert_eq!(a.anchor_name, None);
        assert_eq!(a.position_anchor, None);
        assert!(a.position_try.is_empty());
    }

    #[test]
    fn anchor_full_round_trips_partial_eq() {
        let a = Anchor {
            anchor_name: Some(AnchorName::Named("btn".into())),
            position_anchor: Some(AnchorRef::Name("other".into())),
            position_try: vec![PositionTry::default()],
        };
        let b = a.clone();
        assert_eq!(a, b);
    }

    #[test]
    fn anchor_differs_when_position_try_diverges() {
        let a = Anchor {
            position_try: vec![PositionTry::default()],
            ..default()
        };
        let b = Anchor {
            position_try: vec![],
            ..default()
        };
        assert_ne!(a, b);
    }

    #[test]
    fn layout_anchor_broken_is_unit_marker() {
        let _m = LayoutAnchorBroken;
        // existence + Default suffice; the marker carries no data.
        let _d = LayoutAnchorBroken;
    }

    #[test]
    fn multi_column_default_is_auto() {
        let m = MultiColumn::default();
        assert_eq!(m.column_count, ColumnCount::Auto);
        assert!(m.column_width.is_none());
        assert!(m.column_gap.is_none());
        assert_eq!(m.column_span, ColumnSpan::None);
        assert_eq!(m.column_fill, ColumnFill::Balance);
        assert_eq!(m.break_inside, BreakInside::Auto);
        assert_eq!(m.break_before, BreakBefore::Auto);
        assert_eq!(m.break_after, BreakAfter::Auto);
    }
}
