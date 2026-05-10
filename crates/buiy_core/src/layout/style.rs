//! `Style` — the hybrid builder over decomposed layout components.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 2.2-2.4.
//!
//! Two equally valid authoring forms write the same fields; on insert,
//! Bundle expansion produces the four decomposed components Phase 1
//! ships (`Display`, `BoxModel`, `Position`, `FlexParams`). Defaulted
//! fields produce defaulted components — the Phase 1 simplification is
//! that components are always inserted, not skipped on default.
//! Phase 4's `LogicalBoxModel` revisit will switch to skip-on-default.
//!
//! `FlexItem` is decomposed-only (per spec § 2.4); it is not included in
//! `Style`.

use super::components::{
    BoxModel, Display, FlexParams, GridParams, Overflow, Position, Scroll, WritingMode,
};
use super::types::{
    AlignContent, AlignItems, AspectRatio, BoxSizing, Direction, Edges, FlexAxis, FlexGap,
    FlexWrap, GridAreas, GridAutoFlow, Inset, JustifyContent, JustifyItems, Length, LogicalEdges,
    OverflowMode, PositionKind, ScrollBehavior, ScrollbarGutter, ScrollbarWidth, Sizing, SnapType,
    TextOrientation, TrackSize, UnicodeBidi, WritingModeKind,
};
use bevy::ecs::bundle::Bundle;

/// Hybrid builder over an entity's self-styling layout components.
///
/// Two authoring forms over the same fields:
///
/// ```ignore
/// // Struct-literal form.
/// let s = Style { display: Display::flex_row(), ..Default::default() };
///
/// // Fluent form.
/// let s = Style::default().flex_row();
/// ```
///
/// On `commands.spawn(s)` (or `entity.insert(s)`), expands into a Bundle
/// of `Display`, `BoxModel`, `Position`, `FlexParams`, `Overflow`,
/// `Scroll`. Decomposed components are canonical; the builder is sugar.
/// `ScrollOffset` (runtime state) and `ScrollSnapItem` (child-side) are
/// NOT in this Bundle — spawn them alongside `Style` per
/// `architecture.md § 2.4`.
#[derive(Bundle, Clone, Debug, Default)]
pub struct Style {
    pub display: Display,
    pub box_model: BoxModel,
    pub position: Position,
    pub flex_params: FlexParams,
    pub overflow: Overflow,
    pub scroll: Scroll,
    pub grid_params: GridParams,
    pub writing_mode: WritingMode,
}

impl Style {
    // ---- Display ----

    pub fn block(mut self) -> Self {
        self.display = Display::Block;
        self
    }

    pub fn flex_row(mut self) -> Self {
        self.display = Display::flex_row();
        self.flex_params.direction = FlexAxis::Row;
        self
    }

    pub fn flex_column(mut self) -> Self {
        self.display = Display::flex_column();
        self.flex_params.direction = FlexAxis::Column;
        self
    }

    pub fn flex_axis(mut self, axis: FlexAxis) -> Self {
        self.display = Display::Flex(axis);
        self.flex_params.direction = axis;
        self
    }

    pub fn display(mut self, d: Display) -> Self {
        self.display = d;
        if let Display::Flex(axis) | Display::InlineFlex(axis) = d {
            self.flex_params.direction = axis;
        }
        self
    }

    // ---- BoxModel: dimensions ----

    pub fn width(mut self, w: Sizing) -> Self {
        self.box_model.width = w;
        self
    }

    pub fn height(mut self, h: Sizing) -> Self {
        self.box_model.height = h;
        self
    }

    pub fn width_px(self, px: f32) -> Self {
        self.width(Sizing::Length(Length::Px(px)))
    }

    pub fn height_px(self, px: f32) -> Self {
        self.height(Sizing::Length(Length::Px(px)))
    }

    pub fn min_width(mut self, w: Sizing) -> Self {
        self.box_model.min_width = w;
        self
    }

    pub fn min_height(mut self, h: Sizing) -> Self {
        self.box_model.min_height = h;
        self
    }

    pub fn max_width(mut self, w: Sizing) -> Self {
        self.box_model.max_width = w;
        self
    }

    pub fn max_height(mut self, h: Sizing) -> Self {
        self.box_model.max_height = h;
        self
    }

    pub fn aspect_ratio(mut self, ratio: AspectRatio) -> Self {
        self.box_model.aspect_ratio = Some(ratio);
        self
    }

    // ---- BoxModel: edges ----

    pub fn padding(mut self, px: f32) -> Self {
        self.box_model.padding = Edges::all(px);
        self
    }

    pub fn padding_edges(mut self, e: Edges) -> Self {
        self.box_model.padding = e;
        self
    }

    pub fn margin(mut self, px: f32) -> Self {
        self.box_model.margin = Edges::all(px);
        self
    }

    pub fn margin_edges(mut self, e: Edges) -> Self {
        self.box_model.margin = e;
        self
    }

    pub fn border(mut self, px: f32) -> Self {
        self.box_model.border = Edges::all(px);
        self
    }

    pub fn border_edges(mut self, e: Edges) -> Self {
        self.box_model.border = e;
        self
    }

    // ---- BoxModel: box-sizing ----

    pub fn content_box(mut self) -> Self {
        self.box_model.box_sizing = BoxSizing::ContentBox;
        self
    }

    pub fn border_box(mut self) -> Self {
        self.box_model.box_sizing = BoxSizing::BorderBox;
        self
    }

    pub fn box_sizing(mut self, b: BoxSizing) -> Self {
        self.box_model.box_sizing = b;
        self
    }

    // ---- Gap (Phase 1 surfaces gap exclusively via FlexParams.gap;
    //           BoxModel.gap is deferred — see Task 2 doc comment) ----

    pub fn gap_px(mut self, px: f32) -> Self {
        self.flex_params.gap = FlexGap {
            row: Length::Px(px),
            column: Length::Px(px),
        };
        self
    }

    // ---- Position ----

    pub fn position(mut self, kind: PositionKind) -> Self {
        self.position.kind = kind;
        self
    }

    pub fn relative(mut self) -> Self {
        self.position.kind = PositionKind::Relative;
        self
    }

    pub fn absolute(mut self) -> Self {
        self.position.kind = PositionKind::Absolute;
        self
    }

    pub fn inset(mut self, i: Inset) -> Self {
        self.position.inset = i;
        self
    }

    // ---- FlexParams ----

    pub fn flex_wrap(mut self, w: FlexWrap) -> Self {
        self.flex_params.wrap = w;
        self
    }

    pub fn justify_content(mut self, j: JustifyContent) -> Self {
        self.flex_params.justify_content = j;
        self
    }

    pub fn align_items(mut self, a: AlignItems) -> Self {
        self.flex_params.align_items = a;
        self
    }

    pub fn align_content(mut self, a: AlignContent) -> Self {
        self.flex_params.align_content = a;
        self
    }

    // ---- Overflow ----

    pub fn overflow_x(mut self, mode: OverflowMode) -> Self {
        self.overflow.x = mode;
        self
    }

    pub fn overflow_y(mut self, mode: OverflowMode) -> Self {
        self.overflow.y = mode;
        self
    }

    pub fn overflow(mut self, x: OverflowMode, y: OverflowMode) -> Self {
        self.overflow.x = x;
        self.overflow.y = y;
        self
    }

    pub fn overflow_hidden(self) -> Self {
        self.overflow(OverflowMode::Hidden, OverflowMode::Hidden)
    }

    pub fn overflow_y_scroll(self) -> Self {
        self.overflow_y(OverflowMode::Scroll)
    }

    pub fn overflow_x_scroll(self) -> Self {
        self.overflow_x(OverflowMode::Scroll)
    }

    pub fn scrollbar_gutter(mut self, g: ScrollbarGutter) -> Self {
        self.overflow.scrollbar_gutter = g;
        self
    }

    pub fn scrollbar_width(mut self, w: ScrollbarWidth) -> Self {
        self.overflow.scrollbar_width = w;
        self
    }

    pub fn scroll_behavior(mut self, b: ScrollBehavior) -> Self {
        self.overflow.scroll_behavior = b;
        self
    }

    // ---- Scroll snap ----

    pub fn snap_type(mut self, t: SnapType) -> Self {
        self.scroll.snap_type = t;
        self
    }

    pub fn snap_padding(mut self, e: Edges) -> Self {
        self.scroll.snap_padding = e;
        self
    }

    pub fn snap_margin(mut self, e: Edges) -> Self {
        self.scroll.snap_margin = e;
        self
    }

    // ---- Grid ----

    /// Set `Display::Grid`. Other grid setters operate on `grid_params`.
    pub fn grid(mut self) -> Self {
        self.display = Display::Grid;
        self
    }

    pub fn inline_grid(mut self) -> Self {
        self.display = Display::InlineGrid;
        self
    }

    pub fn grid_template_columns(mut self, tracks: Vec<TrackSize>) -> Self {
        self.grid_params.template_columns = tracks;
        self
    }

    pub fn grid_template_rows(mut self, tracks: Vec<TrackSize>) -> Self {
        self.grid_params.template_rows = tracks;
        self
    }

    pub fn grid_template_areas(mut self, areas: GridAreas) -> Self {
        self.grid_params.template_areas = Some(areas);
        self
    }

    pub fn grid_auto_columns(mut self, tracks: Vec<TrackSize>) -> Self {
        self.grid_params.auto_columns = tracks;
        self
    }

    pub fn grid_auto_rows(mut self, tracks: Vec<TrackSize>) -> Self {
        self.grid_params.auto_rows = tracks;
        self
    }

    pub fn grid_auto_flow(mut self, flow: GridAutoFlow) -> Self {
        self.grid_params.auto_flow = flow;
        self
    }

    pub fn grid_justify_items(mut self, j: JustifyItems) -> Self {
        self.grid_params.justify_items = j;
        self
    }

    pub fn grid_align_items(mut self, a: AlignItems) -> Self {
        self.grid_params.align_items = a;
        self
    }

    pub fn grid_justify_content(mut self, j: JustifyContent) -> Self {
        self.grid_params.justify_content = j;
        self
    }

    pub fn grid_align_content(mut self, a: AlignContent) -> Self {
        self.grid_params.align_content = a;
        self
    }

    /// Set both row and column gap on `GridParams.gap`. Distinct from
    /// `gap_px` (which sets `FlexParams.gap`); when an entity is a flex
    /// container, only `FlexParams.gap` is honored, and conversely for
    /// grid. CSS-faithful unified gap is a follow-up plan.
    pub fn grid_gap_px(mut self, px: f32) -> Self {
        self.grid_params.gap = FlexGap {
            row: Length::Px(px),
            column: Length::Px(px),
        };
        self
    }

    // ---- WritingMode ----

    pub fn writing_mode(mut self, wm: WritingMode) -> Self {
        self.writing_mode = wm;
        self
    }

    pub fn writing_mode_kind(mut self, kind: WritingModeKind) -> Self {
        self.writing_mode.mode = kind;
        self
    }

    pub fn direction(mut self, d: Direction) -> Self {
        self.writing_mode.direction = d;
        self
    }

    pub fn ltr(mut self) -> Self {
        self.writing_mode.direction = Direction::Ltr;
        self
    }

    pub fn rtl(mut self) -> Self {
        self.writing_mode.direction = Direction::Rtl;
        self
    }

    pub fn text_orientation(mut self, t: TextOrientation) -> Self {
        self.writing_mode.text_orientation = t;
        self
    }

    pub fn unicode_bidi(mut self, u: UnicodeBidi) -> Self {
        self.writing_mode.unicode_bidi = u;
        self
    }
}

/// Builder for the box-model surface using logical (writing-mode-aware)
/// dimensions. **Not stored** — call `.to_box_model(&WritingMode)` to
/// produce a `BoxModel` and pass that into your `Style`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/box-model.md § 4.
// Author-side helper consumed via re-export. Lib-level `dead_code` fires
// until Phase 4 Tasks 6-7 wire `Style` setters and the public re-export.
#[allow(dead_code)]
#[derive(Default, Clone, Debug, PartialEq)]
pub struct LogicalBoxModel {
    pub inline_size: Sizing,
    pub block_size: Sizing,
    pub min_inline_size: Sizing,
    pub min_block_size: Sizing,
    pub max_inline_size: Sizing,
    pub max_block_size: Sizing,
    pub padding: LogicalEdges,
    pub margin: LogicalEdges,
    pub border: LogicalEdges,
    pub box_sizing: BoxSizing,
    pub aspect_ratio: Option<AspectRatio>,
}

impl LogicalBoxModel {
    /// Translate to a physical `BoxModel` honoring the given writing-mode.
    /// Vertical modes swap inline ↔ block onto width ↔ height; physical
    /// edges follow the LogicalEdges 6-row table.
    #[allow(dead_code)]
    pub fn to_box_model(&self, wm: &WritingMode) -> BoxModel {
        let is_vertical = matches!(
            wm.mode,
            WritingModeKind::VerticalRl
                | WritingModeKind::VerticalLr
                | WritingModeKind::SidewaysRl
                | WritingModeKind::SidewaysLr
        );
        let (width, height) = if is_vertical {
            (self.block_size, self.inline_size)
        } else {
            (self.inline_size, self.block_size)
        };
        let (min_width, min_height) = if is_vertical {
            (self.min_block_size, self.min_inline_size)
        } else {
            (self.min_inline_size, self.min_block_size)
        };
        let (max_width, max_height) = if is_vertical {
            (self.max_block_size, self.max_inline_size)
        } else {
            (self.max_inline_size, self.max_block_size)
        };
        BoxModel {
            width,
            height,
            min_width,
            min_height,
            max_width,
            max_height,
            padding: self.padding.to_edges(wm.mode, wm.direction),
            margin: self.margin.to_edges(wm.mode, wm.direction),
            border: self.border.to_edges(wm.mode, wm.direction),
            box_sizing: self.box_sizing,
            aspect_ratio: self.aspect_ratio,
        }
    }
}

/// Builder for the inset surface using logical (writing-mode-aware)
/// edges. **Not stored** — call `.to_inset(&WritingMode)` to produce an
/// `Inset`.
// Author-side helper consumed via re-export. Lib-level `dead_code` fires
// until Phase 4 Tasks 6-7 wire `Style` setters and the public re-export.
#[allow(dead_code)]
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct LogicalInset {
    pub inline_start: Sizing,
    pub inline_end: Sizing,
    pub block_start: Sizing,
    pub block_end: Sizing,
}

impl LogicalInset {
    #[allow(dead_code)]
    pub fn to_inset(self, wm: &WritingMode) -> Inset {
        // Inset uses Sizing (not Length), so we duplicate the 6-row
        // mapping rather than reusing LogicalEdges::to_edges.
        use WritingModeKind::*;
        let mode = match wm.mode {
            SidewaysRl => VerticalRl,
            SidewaysLr => VerticalLr,
            other => other,
        };
        match (mode, wm.direction) {
            (HorizontalTb, Direction::Ltr) => Inset {
                left: self.inline_start,
                right: self.inline_end,
                top: self.block_start,
                bottom: self.block_end,
            },
            (HorizontalTb, Direction::Rtl) => Inset {
                right: self.inline_start,
                left: self.inline_end,
                top: self.block_start,
                bottom: self.block_end,
            },
            (VerticalRl, Direction::Ltr) => Inset {
                top: self.inline_start,
                bottom: self.inline_end,
                right: self.block_start,
                left: self.block_end,
            },
            (VerticalRl, Direction::Rtl) => Inset {
                bottom: self.inline_start,
                top: self.inline_end,
                right: self.block_start,
                left: self.block_end,
            },
            (VerticalLr, Direction::Ltr) => Inset {
                top: self.inline_start,
                bottom: self.inline_end,
                left: self.block_start,
                right: self.block_end,
            },
            (VerticalLr, Direction::Rtl) => Inset {
                bottom: self.inline_start,
                top: self.inline_end,
                left: self.block_start,
                right: self.block_end,
            },
            (SidewaysRl, _) | (SidewaysLr, _) => unreachable!("sideways normalized"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::components::{
        BoxModel, Display, FlexParams, GridParams, Overflow, Position, Scroll, WritingMode,
    };
    use crate::layout::types::{
        AlignItems, BoxSizing, Direction, Edges, FlexAxis, FlexGap, GridAutoFlow, JustifyContent,
        Length, OverflowMode, ScrollbarWidth, Sizing, SnapType, TrackSize, WritingModeKind,
    };
    use bevy::app::App;
    use bevy::prelude::MinimalPlugins;

    fn spawn_and_extract(
        style: Style,
    ) -> (
        Display,
        BoxModel,
        Position,
        FlexParams,
        Overflow,
        Scroll,
        GridParams,
        WritingMode,
    ) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let entity = app.world_mut().spawn(style).id();
        let world = app.world();
        let display = *world.get::<Display>(entity).expect("Display inserted");
        let box_model = world
            .get::<BoxModel>(entity)
            .expect("BoxModel inserted")
            .clone();
        let position = world
            .get::<Position>(entity)
            .expect("Position inserted")
            .clone();
        let flex_params = *world
            .get::<FlexParams>(entity)
            .expect("FlexParams inserted");
        let overflow = world
            .get::<Overflow>(entity)
            .expect("Overflow inserted")
            .clone();
        let scroll = world
            .get::<Scroll>(entity)
            .expect("Scroll inserted")
            .clone();
        let grid_params = world
            .get::<GridParams>(entity)
            .expect("GridParams inserted")
            .clone();
        let writing_mode = *world
            .get::<WritingMode>(entity)
            .expect("WritingMode inserted");
        (
            display,
            box_model,
            position,
            flex_params,
            overflow,
            scroll,
            grid_params,
            writing_mode,
        )
    }

    #[test]
    fn struct_literal_and_fluent_produce_identical_components() {
        let literal = Style {
            display: Display::Flex(FlexAxis::Column),
            box_model: BoxModel {
                padding: Edges::all(16.0),
                box_sizing: BoxSizing::BorderBox,
                width: Sizing::Length(Length::Px(200.0)),
                height: Sizing::Length(Length::Px(100.0)),
                ..Default::default()
            },
            flex_params: FlexParams {
                direction: FlexAxis::Column,
                gap: FlexGap {
                    row: Length::Px(8.0),
                    column: Length::Px(8.0),
                },
                justify_content: JustifyContent::SpaceBetween,
                align_items: AlignItems::Center,
                ..Default::default()
            },
            overflow: Overflow {
                x: OverflowMode::Hidden,
                y: OverflowMode::Auto,
                scrollbar_width: ScrollbarWidth::Thin,
                ..Default::default()
            },
            scroll: Scroll {
                snap_type: SnapType::YMandatory,
                snap_padding: Edges::all(4.0),
                ..Default::default()
            },
            grid_params: GridParams {
                template_columns: vec![
                    TrackSize::Length(Length::Fr(1.0)),
                    TrackSize::Length(Length::Fr(2.0)),
                ],
                auto_flow: GridAutoFlow::Column,
                ..Default::default()
            },
            ..Default::default()
        };
        let fluent = Style::default()
            .flex_column()
            .padding(16.0)
            .border_box()
            .width_px(200.0)
            .height_px(100.0)
            .gap_px(8.0)
            .justify_content(JustifyContent::SpaceBetween)
            .align_items(AlignItems::Center)
            .overflow_x(OverflowMode::Hidden)
            .overflow_y(OverflowMode::Auto)
            .scrollbar_width(ScrollbarWidth::Thin)
            .snap_type(SnapType::YMandatory)
            .snap_padding(Edges::all(4.0))
            .grid_template_columns(vec![
                TrackSize::Length(Length::Fr(1.0)),
                TrackSize::Length(Length::Fr(2.0)),
            ])
            .grid_auto_flow(GridAutoFlow::Column);

        assert_eq!(spawn_and_extract(literal), spawn_and_extract(fluent));
    }

    #[test]
    fn default_style_inserts_every_decomposed_component() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let entity = app.world_mut().spawn(Style::default()).id();
        let world = app.world();
        assert!(world.get::<Display>(entity).is_some());
        assert!(world.get::<BoxModel>(entity).is_some());
        assert!(world.get::<Position>(entity).is_some());
        assert!(world.get::<FlexParams>(entity).is_some());
        assert!(world.get::<Overflow>(entity).is_some());
        assert!(world.get::<Scroll>(entity).is_some());
        assert!(world.get::<GridParams>(entity).is_some());
        assert!(world.get::<WritingMode>(entity).is_some());
    }

    #[test]
    fn grid_template_columns_setter_overrides() {
        let s = Style::default().grid().grid_template_columns(vec![
            TrackSize::Length(Length::Fr(1.0)),
            TrackSize::Length(Length::Fr(2.0)),
        ]);
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let entity = app.world_mut().spawn(s).id();
        let g = app
            .world()
            .get::<GridParams>(entity)
            .expect("GridParams inserted");
        assert_eq!(g.template_columns.len(), 2);
        assert!(matches!(
            g.template_columns[0],
            TrackSize::Length(Length::Fr(_))
        ));
    }

    #[test]
    fn grid_helpers_set_display_grid() {
        let s = Style::default().grid();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let entity = app.world_mut().spawn(s).id();
        let d = app
            .world()
            .get::<Display>(entity)
            .copied()
            .expect("Display inserted");
        assert_eq!(d, Display::Grid);
    }

    #[test]
    fn logical_box_model_inline_size_under_horizontal_tb_is_width() {
        let logical = LogicalBoxModel {
            inline_size: Sizing::Length(Length::Px(100.0)),
            block_size: Sizing::Length(Length::Px(50.0)),
            ..Default::default()
        };
        let wm = WritingMode::default(); // horizontal-tb + ltr
        let bm = logical.to_box_model(&wm);
        assert_eq!(bm.width, Sizing::Length(Length::Px(100.0)));
        assert_eq!(bm.height, Sizing::Length(Length::Px(50.0)));
    }

    #[test]
    fn logical_box_model_inline_size_under_vertical_rl_is_height() {
        let logical = LogicalBoxModel {
            inline_size: Sizing::Length(Length::Px(100.0)),
            block_size: Sizing::Length(Length::Px(50.0)),
            ..Default::default()
        };
        let wm = WritingMode {
            mode: WritingModeKind::VerticalRl,
            ..Default::default()
        };
        let bm = logical.to_box_model(&wm);
        assert_eq!(bm.height, Sizing::Length(Length::Px(100.0)));
        assert_eq!(bm.width, Sizing::Length(Length::Px(50.0)));
    }

    #[test]
    fn logical_inset_inline_start_under_vertical_rl_is_top() {
        let logical = LogicalInset {
            inline_start: Sizing::Length(Length::Px(8.0)),
            ..Default::default()
        };
        let wm = WritingMode {
            mode: WritingModeKind::VerticalRl,
            ..Default::default()
        };
        let inset = logical.to_inset(&wm);
        assert_eq!(inset.top, Sizing::Length(Length::Px(8.0)));
    }

    #[test]
    fn writing_mode_setter_overrides() {
        let s = Style::default().writing_mode(WritingMode {
            mode: WritingModeKind::VerticalRl,
            ..Default::default()
        });
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let entity = app.world_mut().spawn(s).id();
        let wm = app
            .world()
            .get::<WritingMode>(entity)
            .copied()
            .expect("WritingMode inserted");
        assert_eq!(wm.mode, WritingModeKind::VerticalRl);
    }

    #[test]
    fn rtl_setter_flips_direction() {
        let s = Style::default().rtl();
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        let entity = app.world_mut().spawn(s).id();
        let wm = app
            .world()
            .get::<WritingMode>(entity)
            .copied()
            .expect("WritingMode inserted");
        assert_eq!(wm.direction, Direction::Rtl);
    }
}
