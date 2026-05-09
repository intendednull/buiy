//! Translation layer: decomposed Buiy layout components → `taffy::Style`.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 1.2.
//!
//! Pure function. Read by `sync_styles` (pipeline step 1). Phase 1 only
//! resolves `Length::Px` and `Length::Percent` — every other variant
//! lands in Phase 10 (`buiy-layout-units-calc`).

use super::components::{BoxModel, Display, FlexItem, FlexParams, Position};
use super::types::{
    AlignContent, AlignItems, BoxSizing, Edges, FlexAxis, FlexWrap, Inset, JustifyContent, Length,
    PositionKind, Sizing,
};

/// View into the Phase 1 decomposed-component set for one entity. Built
/// by `sync_styles`'s query and passed to `style_to_taffy`.
pub struct StyleView<'a> {
    pub display: &'a Display,
    pub box_model: &'a BoxModel,
    pub position: &'a Position,
    pub flex_params: &'a FlexParams,
    pub flex_item: Option<&'a FlexItem>,
}

pub fn style_to_taffy(view: StyleView<'_>) -> taffy::Style {
    let mut s = taffy::Style {
        display: map_display(view.display),
        box_sizing: map_box_sizing(view.box_model.box_sizing),
        position: map_position_kind(view.position.kind),
        size: taffy::Size {
            width: sizing_to_dim(view.box_model.width),
            height: sizing_to_dim(view.box_model.height),
        },
        min_size: taffy::Size {
            width: sizing_to_dim(view.box_model.min_width),
            height: sizing_to_dim(view.box_model.min_height),
        },
        max_size: taffy::Size {
            width: sizing_to_dim(view.box_model.max_width),
            height: sizing_to_dim(view.box_model.max_height),
        },
        padding: edges_to_lp(view.box_model.padding),
        margin: edges_to_lpa(view.box_model.margin),
        border: edges_to_lp(view.box_model.border),
        inset: inset_to_lpa(view.position.inset),
        flex_direction: map_flex_axis(view.flex_params.direction),
        flex_wrap: map_flex_wrap(view.flex_params.wrap),
        justify_content: Some(map_justify_content(view.flex_params.justify_content)),
        align_items: Some(map_align_items(view.flex_params.align_items)),
        align_content: Some(map_align_content(view.flex_params.align_content)),
        gap: taffy::Size {
            width: length_to_lp(view.flex_params.gap.column),
            height: length_to_lp(view.flex_params.gap.row),
        },
        ..Default::default()
    };

    if let Some(item) = view.flex_item {
        s.flex_grow = item.grow;
        s.flex_shrink = item.shrink;
        s.flex_basis = sizing_to_dim(item.basis);
        // Taffy 0.10 has no `order` field on Style. CSS `order` would
        // need a Buiy-side sibling sort before `set_children`; that
        // lands later (tracked under flex-and-grid follow-ups). Phase 1
        // stores `FlexItem.order` but does not act on it; document this
        // as a Phase 1 limitation (warn once per session).
        let _unused_order_in_phase_1 = item.order;
        s.align_self = item.align_self.map(map_align_items_as_self);
    }

    if let Some(ar) = view.box_model.aspect_ratio {
        s.aspect_ratio = Some(ar.ratio);
    }

    s
}

fn map_display(d: &Display) -> taffy::Display {
    use Display::*;
    // Phase 1 maps Grid/InlineGrid to Block. Translating them to
    // taffy::Display::Grid without GridParams/GridItem would silently
    // create templateless grid containers and tempt premature reliance
    // on Grid before Phase 3 ships the components. Phase 3 replaces
    // this row with `Grid | InlineGrid => taffy::Display::Grid`.
    match d {
        Block | Inline | InlineBlock | FlowRoot | Contents | ListItem | Ruby | Table
        | TableRowGroup | TableHeaderGroup | TableFooterGroup | TableRow | TableCell
        | TableCaption | TableColumnGroup | TableColumn | Grid | InlineGrid => {
            taffy::Display::Block
        }
        Flex(_) | InlineFlex(_) => taffy::Display::Flex,
        None => taffy::Display::None,
    }
}

fn map_box_sizing(b: BoxSizing) -> taffy::BoxSizing {
    match b {
        BoxSizing::ContentBox => taffy::BoxSizing::ContentBox,
        BoxSizing::BorderBox => taffy::BoxSizing::BorderBox,
    }
}

fn map_position_kind(k: PositionKind) -> taffy::Position {
    use PositionKind::*;
    // Phase 1: Static / Relative / Absolute pass through; Fixed translates
    // to Absolute and Sticky translates to Relative. Phase 7 (sticky) and
    // Phase 8 (top-layer / fixed-as-viewport) wire the real semantics.
    match k {
        Static | Relative | Sticky => taffy::Position::Relative,
        Absolute | Fixed => taffy::Position::Absolute,
    }
}

fn map_flex_axis(a: FlexAxis) -> taffy::FlexDirection {
    match a {
        FlexAxis::Row => taffy::FlexDirection::Row,
        FlexAxis::Column => taffy::FlexDirection::Column,
        FlexAxis::RowReverse => taffy::FlexDirection::RowReverse,
        FlexAxis::ColumnReverse => taffy::FlexDirection::ColumnReverse,
    }
}

fn map_flex_wrap(w: FlexWrap) -> taffy::FlexWrap {
    match w {
        FlexWrap::NoWrap => taffy::FlexWrap::NoWrap,
        FlexWrap::Wrap => taffy::FlexWrap::Wrap,
        FlexWrap::WrapReverse => taffy::FlexWrap::WrapReverse,
    }
}

fn map_justify_content(j: JustifyContent) -> taffy::JustifyContent {
    match j {
        JustifyContent::FlexStart => taffy::JustifyContent::FlexStart,
        JustifyContent::FlexEnd => taffy::JustifyContent::FlexEnd,
        JustifyContent::Center => taffy::JustifyContent::Center,
        JustifyContent::SpaceBetween => taffy::JustifyContent::SpaceBetween,
        JustifyContent::SpaceAround => taffy::JustifyContent::SpaceAround,
        JustifyContent::SpaceEvenly => taffy::JustifyContent::SpaceEvenly,
    }
}

fn map_align_items(a: AlignItems) -> taffy::AlignItems {
    match a {
        AlignItems::Stretch => taffy::AlignItems::Stretch,
        AlignItems::FlexStart => taffy::AlignItems::FlexStart,
        AlignItems::FlexEnd => taffy::AlignItems::FlexEnd,
        AlignItems::Center => taffy::AlignItems::Center,
        AlignItems::Baseline => taffy::AlignItems::Baseline,
    }
}

fn map_align_items_as_self(a: AlignItems) -> taffy::AlignSelf {
    match a {
        AlignItems::Stretch => taffy::AlignSelf::Stretch,
        AlignItems::FlexStart => taffy::AlignSelf::FlexStart,
        AlignItems::FlexEnd => taffy::AlignSelf::FlexEnd,
        AlignItems::Center => taffy::AlignSelf::Center,
        AlignItems::Baseline => taffy::AlignSelf::Baseline,
    }
}

fn map_align_content(a: AlignContent) -> taffy::AlignContent {
    match a {
        AlignContent::Stretch => taffy::AlignContent::Stretch,
        AlignContent::FlexStart => taffy::AlignContent::FlexStart,
        AlignContent::FlexEnd => taffy::AlignContent::FlexEnd,
        AlignContent::Center => taffy::AlignContent::Center,
        AlignContent::SpaceBetween => taffy::AlignContent::SpaceBetween,
        AlignContent::SpaceAround => taffy::AlignContent::SpaceAround,
        AlignContent::SpaceEvenly => taffy::AlignContent::SpaceEvenly,
    }
}

fn sizing_to_dim(s: Sizing) -> taffy::Dimension {
    // Phase 1 ships Auto / None / Length / Stretch as the "real" surface;
    // intrinsic keywords resolve silently to Auto until Phase 10 + text
    // rendering integrate.
    match s {
        Sizing::Auto | Sizing::MinContent | Sizing::MaxContent | Sizing::FitContent(_) => {
            taffy::Dimension::auto()
        }
        Sizing::None => taffy::Dimension::auto(),
        Sizing::Length(l) => length_to_dim(l),
        Sizing::Stretch => taffy::Dimension::auto(), // taffy 0.10 doesn't ship `stretch`; treated as auto.
    }
}

fn length_to_dim(l: Length) -> taffy::Dimension {
    match l {
        Length::Px(v) => taffy::Dimension::length(v),
        Length::Percent(p) => taffy::Dimension::percent(p / 100.0),
    }
}

fn length_to_lp(l: Length) -> taffy::LengthPercentage {
    match l {
        Length::Px(v) => taffy::LengthPercentage::length(v),
        Length::Percent(p) => taffy::LengthPercentage::percent(p / 100.0),
    }
}

fn length_to_lpa(l: Length) -> taffy::LengthPercentageAuto {
    match l {
        Length::Px(v) => taffy::LengthPercentageAuto::length(v),
        Length::Percent(p) => taffy::LengthPercentageAuto::percent(p / 100.0),
    }
}

fn sizing_to_lpa(s: Sizing) -> taffy::LengthPercentageAuto {
    match s {
        Sizing::Auto
        | Sizing::None
        | Sizing::MinContent
        | Sizing::MaxContent
        | Sizing::FitContent(_)
        | Sizing::Stretch => taffy::LengthPercentageAuto::auto(),
        Sizing::Length(l) => length_to_lpa(l),
    }
}

fn edges_to_lp(e: Edges) -> taffy::Rect<taffy::LengthPercentage> {
    taffy::Rect {
        top: length_to_lp(e.top),
        right: length_to_lp(e.right),
        bottom: length_to_lp(e.bottom),
        left: length_to_lp(e.left),
    }
}

fn edges_to_lpa(e: Edges) -> taffy::Rect<taffy::LengthPercentageAuto> {
    taffy::Rect {
        top: length_to_lpa(e.top),
        right: length_to_lpa(e.right),
        bottom: length_to_lpa(e.bottom),
        left: length_to_lpa(e.left),
    }
}

fn inset_to_lpa(i: Inset) -> taffy::Rect<taffy::LengthPercentageAuto> {
    taffy::Rect {
        top: sizing_to_lpa(i.top),
        right: sizing_to_lpa(i.right),
        bottom: sizing_to_lpa(i.bottom),
        left: sizing_to_lpa(i.left),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::components::{BoxModel, Display, FlexItem, FlexParams, Position};
    use crate::layout::types::{
        AlignItems, BoxSizing, Edges, FlexAxis, FlexGap, FlexWrap, JustifyContent, Length,
        PositionKind, Sizing,
    };

    #[test]
    fn translate_default_components_to_taffy_default() {
        let bm = BoxModel::default();
        let display = Display::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let item: Option<&FlexItem> = None;
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: item,
        });
        // Default Display::Block + ContentBox + everything Auto produces taffy default Display::Block.
        assert_eq!(taffy.display, taffy::Display::Block);
        assert_eq!(taffy.size.width, taffy::Dimension::auto());
        assert_eq!(taffy.size.height, taffy::Dimension::auto());
    }

    #[test]
    fn translate_flex_row_with_dimensions() {
        let display = Display::Flex(FlexAxis::Row);
        let bm = BoxModel {
            width: Sizing::Length(Length::Px(200.0)),
            height: Sizing::Length(Length::Px(100.0)),
            padding: Edges::all(8.0),
            box_sizing: BoxSizing::BorderBox,
            ..Default::default()
        };
        let position = Position::default();
        let flex = FlexParams {
            direction: FlexAxis::Row,
            gap: FlexGap {
                row: Length::Px(4.0),
                column: Length::Px(4.0),
            },
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            wrap: FlexWrap::NoWrap,
            ..Default::default()
        };
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: None,
        });
        assert_eq!(taffy.display, taffy::Display::Flex);
        assert_eq!(taffy.flex_direction, taffy::FlexDirection::Row);
        assert_eq!(taffy.size.width, taffy::Dimension::length(200.0));
        assert_eq!(taffy.size.height, taffy::Dimension::length(100.0));
        assert_eq!(taffy.box_sizing, taffy::BoxSizing::BorderBox);
        assert_eq!(taffy.justify_content, Some(taffy::JustifyContent::Center));
        assert_eq!(taffy.align_items, Some(taffy::AlignItems::Center));
    }

    #[test]
    fn translate_position_absolute_emits_absolute_with_inset() {
        let display = Display::default();
        let bm = BoxModel::default();
        let position = Position {
            kind: PositionKind::Absolute,
            inset: crate::layout::types::Inset {
                top: Sizing::Length(Length::Px(10.0)),
                left: Sizing::Length(Length::Px(20.0)),
                ..Default::default()
            },
        };
        let flex = FlexParams::default();
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: None,
        });
        assert_eq!(taffy.position, taffy::Position::Absolute);
        assert_eq!(taffy.inset.top, taffy::LengthPercentageAuto::length(10.0));
        assert_eq!(taffy.inset.left, taffy::LengthPercentageAuto::length(20.0));
    }

    #[test]
    fn translate_flex_item_basis_grow_shrink() {
        let display = Display::default();
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let item = FlexItem {
            grow: 2.0,
            shrink: 0.5,
            basis: Sizing::Length(Length::Px(100.0)),
            order: 3,
            align_self: Some(AlignItems::Center),
        };
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: Some(&item),
        });
        assert_eq!(taffy.flex_grow, 2.0);
        assert_eq!(taffy.flex_shrink, 0.5);
        assert_eq!(taffy.flex_basis, taffy::Dimension::length(100.0));
        assert_eq!(taffy.align_self, Some(taffy::AlignSelf::Center));
        // FlexItem.order is stored but Taffy 0.10 has no `order` on
        // Style; Phase 1 does not honor it. Documented as a Phase 1
        // limitation in the translation module's doc comment.
    }
}
