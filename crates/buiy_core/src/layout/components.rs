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
    AlignContent, AlignItems, AspectRatio, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap, Inset,
    JustifyContent, OverflowMode, OverscrollBehavior, PositionKind, ScrollBehavior, ScrollbarColor,
    ScrollbarGutter, ScrollbarWidth, Sizing, SnapType,
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

/// `display` value. Phase 1 implements `Block` and `Flex(FlexAxis)`; other
/// variants are reserved and translate to `Block` (Taffy default).
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
/// `Absolute`. `Fixed` and `Sticky` ship as variants but currently
/// translate to `Absolute` / `Relative`; Phases 7/8 wire the real semantics.
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
    /// with `Position::Sticky` (consumer: Phase 7 sub-pass 6a).
    ///
    /// Spec: docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md § 1.2.
    // Called by Phase 7 sticky-positioning pass; unused until that phase lands.
    #[allow(dead_code)]
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
}
