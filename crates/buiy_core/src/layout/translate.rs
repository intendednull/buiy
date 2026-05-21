//! Translation layer: decomposed Buiy layout components → `taffy::Style`.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 1.2.
//!
//! Pure function. Read by `sync_styles` (pipeline step 1). Phase 1 only
//! resolves `Length::Px` and `Length::Percent` — every other variant
//! lands in Phase 10 (`buiy-layout-units-calc`).

use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::warn;

use super::components::{
    BoxModel, Display, FlexItem, FlexParams, GridItem, GridParams, Overflow, Position, Scroll,
    WritingModeResolved,
};
use super::types::{
    AlignContent, AlignItems, BoxSizing, Direction, Edges, FlexAxis, FlexWrap, GridAreas,
    GridAutoFlow, GridLine, Inset, JustifyContent, JustifyItems, Length, OverflowMode,
    PositionKind, RepeatCount, ScrollbarWidth, Sizing, TrackSize, WritingModeKind,
};
// Bring helper free functions and grid-specific types from `taffy::prelude`
// into scope. The compiler infers each helper's return type from the
// function-return / binding annotation. See ~/.cargo/registry/.../taffy-0.10.1/src/prelude.rs.
//
// We selectively import only the helpers + grid types we need (rather than
// `prelude::*`) to avoid clashes with `super::components::Display`,
// `super::types::JustifyContent`, `JustifyItems`, etc., which the file
// already brings in by name.
use taffy::prelude::{
    GridPlacement, GridTemplateComponent, MaxTrackSizingFunction, MinTrackSizingFunction,
    TrackSizingFunction, auto, fit_content, fr, length, max_content, min_content, minmax, percent,
};

static WARNED_FR_OUTSIDE_GRID: AtomicBool = AtomicBool::new(false);

fn warn_once_fr_outside_grid() {
    if !WARNED_FR_OUTSIDE_GRID.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: Length::Fr is only meaningful inside TrackSize::Length \
             in a grid template; outside grid it falls back to 0 px / Auto \
             (warned once)"
        );
    }
}

/// Phase 5 Task 1 *temporary* fallback: container units resolve to
/// 0 px until Task 7 wires the real ancestor-driven path. The only
/// purpose of this helper is to keep the `Length` match exhaustive
/// in this commit (Phase 3 Task 1 prior art — atomic types.rs +
/// translate.rs).
///
/// **Task 7 deletes this function.** No warn is emitted here because
/// the warn-once gating lives in Task 7's real resolver
/// (`warn_once_cq_no_ancestor`). A temporary 0-px fallback is a
/// build-bridge, not behavior worth advertising.
fn cq_unit_fallback_px(_p: f32) -> f32 {
    0.0
}

/// View into the decomposed-component set for one entity. Built by
/// `sync_styles`'s query and passed to `style_to_taffy`.
pub struct StyleView<'a> {
    pub display: &'a Display,
    pub box_model: &'a BoxModel,
    pub position: &'a Position,
    pub flex_params: &'a FlexParams,
    pub flex_item: Option<&'a FlexItem>,
    pub overflow: &'a Overflow,
    pub scroll: &'a Scroll,
    pub grid_params: &'a GridParams,
    pub grid_item: Option<&'a GridItem>,
    /// Parent's `template_areas` if the parent is a grid container.
    /// Required to resolve `GridLine::Area(name)` because Taffy 0.10 has
    /// no native named-area placement — only named lines. `sync_styles`
    /// precomputes a per-entity map and feeds the lookup result here.
    pub parent_areas: Option<&'a GridAreas>,
    /// Resolved writing-mode (mode + direction + text-orientation +
    /// unicode-bidi). Populated by `inherit_writing_mode` (pipeline step
    /// `BuiyLayoutStep::WritingModeInherit`) and read here to drive
    /// `taffy::Style.direction`. Sideways-* modes hit a warn-once gate
    /// because their glyph rotation lives in `buiy-text-rendering-design`,
    /// not layout — layout treats them as their non-sideways equivalents.
    pub writing_mode_resolved: &'a WritingModeResolved,
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
        overflow: taffy::Point {
            x: map_overflow_mode(view.overflow.x),
            y: map_overflow_mode(view.overflow.y),
        },
        scrollbar_width: map_scrollbar_width(view.overflow.scrollbar_width),
        ..Default::default()
    };

    // Writing-mode → Taffy `direction`. `WritingModeResolved.direction`
    // is populated by `inherit_writing_mode` (pipeline step
    // `BuiyLayoutStep::WritingModeInherit`); RTL flips Taffy's main-axis
    // start/end. The `mode` field (HorizontalTb / VerticalRl / VerticalLr
    // / SidewaysRl / SidewaysLr) is NOT wired to Taffy in Phase 4 — Taffy
    // 0.10 has no writing-mode field; vertical layout is achieved on the
    // glyph-rendering side. The sideways-* fallback below names that
    // owner explicitly.
    s.direction = match view.writing_mode_resolved.direction {
        Direction::Ltr => taffy::Direction::Ltr,
        Direction::Rtl => taffy::Direction::Rtl,
    };

    // Sideways-* warn-once: layout treats them as their non-sideways
    // vertical equivalents; the glyph-rotation pass owns rotation.
    if matches!(
        view.writing_mode_resolved.mode,
        WritingModeKind::SidewaysRl | WritingModeKind::SidewaysLr
    ) {
        warn_once_sideways_layout_fallback();
    }

    // `Scroll` is included in `StyleView` so `Changed<Scroll>` flows
    // through `sync_styles`'s trigger filter (architecture.md § 1.2),
    // but it has no Taffy mapping — its data is consumed by render /
    // input / Phase 7 sticky systems, not by layout. Touch the field
    // here so it is unambiguously "read" and dead-code lints stay
    // honest. Phase 7 (sticky) and the input pipeline will replace
    // this no-op with real consumers.
    let _scroll_unused_in_layout = view.scroll;

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

    // Grid container fields. Only meaningful when display is Grid /
    // InlineGrid, but Taffy ignores them otherwise — so unconditional
    // population is safe and removes a branch.
    s.grid_template_columns = view
        .grid_params
        .template_columns
        .iter()
        .map(track_to_template)
        .collect();
    s.grid_template_rows = view
        .grid_params
        .template_rows
        .iter()
        .map(track_to_template)
        .collect();
    s.grid_auto_columns = view
        .grid_params
        .auto_columns
        .iter()
        .map(track_to_sizing)
        .collect();
    s.grid_auto_rows = view
        .grid_params
        .auto_rows
        .iter()
        .map(track_to_sizing)
        .collect();
    s.grid_auto_flow = map_grid_auto_flow(view.grid_params.auto_flow);
    if let Some(areas) = &view.grid_params.template_areas {
        s.grid_template_areas = areas
            .areas
            .iter()
            .map(|a| taffy::GridTemplateArea {
                // S = String (Taffy's DefaultCheapStr); clone the owned
                // String. Do not `.as_str().into()` — that requires a
                // 'static borrow that the runtime String doesn't have.
                name: a.name.clone(),
                row_start: a.row_start + 1,
                row_end: a.row_end + 1,
                column_start: a.column_start + 1,
                column_end: a.column_end + 1,
            })
            .collect();
    }

    // Grid item fields. Only honored when the parent is a grid container,
    // but Taffy ignores otherwise — so unconditional population is safe.
    if let Some(item) = view.grid_item {
        s.grid_column = grid_line_to_taffy(&item.column, GridAxis::Column, view.parent_areas);
        s.grid_row = grid_line_to_taffy(&item.row, GridAxis::Row, view.parent_areas);
    }

    // Grid alignment overrides. Taffy 0.10 has *one* shared set of
    // justify_items / align_items / justify_content / align_content fields
    // used by both flex and grid algorithms; the flex path above already
    // populated align_items / justify_content / align_content from
    // FlexParams. When the entity is a grid container, override these from
    // GridParams so grid alignment honors the grid-side surface.
    if matches!(view.display, Display::Grid | Display::InlineGrid) {
        s.justify_items = Some(map_justify_items(view.grid_params.justify_items));
        s.align_items = Some(map_align_items(view.grid_params.align_items));
        s.justify_content = Some(map_justify_content(view.grid_params.justify_content));
        s.align_content = Some(map_align_content(view.grid_params.align_content));
        s.gap = taffy::Size {
            width: length_to_lp(view.grid_params.gap.column),
            height: length_to_lp(view.grid_params.gap.row),
        };
    }

    if let Some(item) = view.grid_item
        && let Some(j) = item.justify_self
    {
        s.justify_self = Some(map_justify_items_as_self(j));
    }
    // GridItem has its own align_self, distinct from FlexItem.align_self
    // (the flex path above only fires when flex_item is Some). Honor it
    // when the grid item provides one.
    if let Some(item) = view.grid_item
        && let Some(a) = item.align_self
    {
        s.align_self = Some(map_align_items_as_self(a));
    }

    s
}

fn map_display(d: &Display) -> taffy::Display {
    use Display::*;
    // Phase 3 routes Grid / InlineGrid to taffy::Display::Grid. Taffy
    // 0.10 has no inline-grid variant, so InlineGrid translates to the
    // same thing (Phase 4 writing-modes may revisit if line-box context
    // distinction matters; layout-side it doesn't).
    match d {
        Block | Inline | InlineBlock | FlowRoot | Contents | ListItem | Ruby | Table
        | TableRowGroup | TableHeaderGroup | TableFooterGroup | TableRow | TableCell
        | TableCaption | TableColumnGroup | TableColumn => taffy::Display::Block,
        Flex(_) | InlineFlex(_) => taffy::Display::Flex,
        Grid | InlineGrid => taffy::Display::Grid,
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

fn map_overflow_mode(o: OverflowMode) -> taffy::Overflow {
    use OverflowMode::*;
    match o {
        Visible => taffy::Overflow::Visible,
        // Spec § 1.1 maps both Hidden and Clip to taffy::Hidden. Taffy 0.10
        // distinguishes Hidden (clips and reserves scrollbar gutter via
        // scrollbar_width) from Clip (clips with no gutter); the spec
        // chose CSS-faithful: both CSS Hidden and CSS Clip route through
        // taffy::Hidden so ScrollbarGutter::Stable can later reserve a
        // gutter consistently when the author opts in.
        Hidden | Clip => taffy::Overflow::Hidden,
        // Auto (conditional scrollbar) is a render-time distinction;
        // layout treats it as Scroll so children may exceed the box.
        Scroll | Auto => taffy::Overflow::Scroll,
    }
}

fn map_scrollbar_width(w: ScrollbarWidth) -> f32 {
    // Approximate common platform scrollbar widths. Auto = ~12 px (GTK /
    // overlay style), Thin = ~8 px (CSS `scrollbar-width: thin` typical
    // rendering), None = 0 px (no gutter reserved). Revisit when
    // buiy-render-pipeline-design picks canonical widths.
    match w {
        ScrollbarWidth::Auto => 12.0,
        ScrollbarWidth::Thin => 8.0,
        ScrollbarWidth::None => 0.0,
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
        Length::Fr(_) => {
            warn_once_fr_outside_grid();
            taffy::Dimension::auto()
        }
        Length::Cqw(p)
        | Length::Cqh(p)
        | Length::Cqi(p)
        | Length::Cqb(p)
        | Length::Cqmin(p)
        | Length::Cqmax(p) => taffy::Dimension::length(cq_unit_fallback_px(p)),
    }
}

fn length_to_lp(l: Length) -> taffy::LengthPercentage {
    match l {
        Length::Px(v) => taffy::LengthPercentage::length(v),
        Length::Percent(p) => taffy::LengthPercentage::percent(p / 100.0),
        // taffy::LengthPercentage has no Auto variant — fall back to 0
        // (CSS-equivalent for Fr-in-non-grid: undefined, ill-formed).
        Length::Fr(_) => {
            warn_once_fr_outside_grid();
            taffy::LengthPercentage::length(0.0)
        }
        Length::Cqw(p)
        | Length::Cqh(p)
        | Length::Cqi(p)
        | Length::Cqb(p)
        | Length::Cqmin(p)
        | Length::Cqmax(p) => taffy::LengthPercentage::length(cq_unit_fallback_px(p)),
    }
}

fn length_to_lpa(l: Length) -> taffy::LengthPercentageAuto {
    match l {
        Length::Px(v) => taffy::LengthPercentageAuto::length(v),
        Length::Percent(p) => taffy::LengthPercentageAuto::percent(p / 100.0),
        Length::Fr(_) => {
            warn_once_fr_outside_grid();
            taffy::LengthPercentageAuto::auto()
        }
        Length::Cqw(p)
        | Length::Cqh(p)
        | Length::Cqi(p)
        | Length::Cqb(p)
        | Length::Cqmin(p)
        | Length::Cqmax(p) => taffy::LengthPercentageAuto::length(cq_unit_fallback_px(p)),
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

// --- Grid mapping helpers (Phase 3) -------------------------------------

fn map_grid_auto_flow(f: GridAutoFlow) -> taffy::GridAutoFlow {
    use GridAutoFlow::*;
    match f {
        Row => taffy::GridAutoFlow::Row,
        Column => taffy::GridAutoFlow::Column,
        RowDense => taffy::GridAutoFlow::RowDense,
        ColumnDense => taffy::GridAutoFlow::ColumnDense,
        // Masonry is reserved (CSS-WG flux) — Taffy 0.10 has no
        // GridAutoFlow::Masonry, so we degrade to Row and emit one warn!
        // per session naming the limitation.
        Masonry => {
            warn_once_masonry();
            taffy::GridAutoFlow::Row
        }
    }
}

fn map_repeat_count(c: RepeatCount) -> taffy::RepetitionCount {
    match c {
        RepeatCount::AutoFill => taffy::RepetitionCount::AutoFill,
        RepeatCount::AutoFit => taffy::RepetitionCount::AutoFit,
        RepeatCount::Count(n) => taffy::RepetitionCount::Count(n),
    }
}

fn map_justify_items(j: JustifyItems) -> taffy::JustifyItems {
    use JustifyItems::*;
    // Note: in Taffy 0.10, `JustifyItems = AlignItems` (a type alias) —
    // so we map onto `taffy::AlignItems` variants here.
    match j {
        Stretch => taffy::AlignItems::Stretch,
        Start => taffy::AlignItems::Start,
        End => taffy::AlignItems::End,
        Center => taffy::AlignItems::Center,
        Baseline => taffy::AlignItems::Baseline,
    }
}

fn map_justify_items_as_self(j: JustifyItems) -> taffy::JustifySelf {
    use JustifyItems::*;
    // `taffy::JustifySelf = AlignItems` (type alias) — same pattern as
    // `map_justify_items` above.
    match j {
        Stretch => taffy::AlignItems::Stretch,
        Start => taffy::AlignItems::Start,
        End => taffy::AlignItems::End,
        Center => taffy::AlignItems::Center,
        Baseline => taffy::AlignItems::Baseline,
    }
}

/// Convert a `TrackSize` into a single `taffy::TrackSizingFunction`.
/// Used inside `Repeat`'s tracks list and inside `MinMax`'s arms — both
/// CSS contexts where a `Repeat` or another `MinMax` is invalid. If
/// callers pass an invalid nested `Repeat`/`Subgrid`/`MinMax`, we warn
/// once and return `auto`.
///
/// `auto()`, `length(_)`, `percent(_)`, `fr(_)`, `fit_content(_)`,
/// `min_content()`, `max_content()`, `minmax(_, _)` come from
/// `taffy::prelude`; the compiler infers the output type from the
/// function-return / binding annotation.
fn track_to_sizing(t: &TrackSize) -> TrackSizingFunction {
    match t {
        TrackSize::Auto => auto(),
        TrackSize::MinContent => min_content(),
        TrackSize::MaxContent => max_content(),
        TrackSize::FitContent(l) => fit_content(length_to_lp(*l)),
        TrackSize::Length(Length::Fr(v)) => fr(*v),
        TrackSize::Length(Length::Px(v)) => length(*v),
        TrackSize::Length(Length::Percent(p)) => percent(p / 100.0),
        TrackSize::Length(
            Length::Cqw(p)
            | Length::Cqh(p)
            | Length::Cqi(p)
            | Length::Cqb(p)
            | Length::Cqmin(p)
            | Length::Cqmax(p),
        ) => length(cq_unit_fallback_px(*p)),
        // `MinMax` carries a Vec<TrackSize>; spec/Phase3 invariant is
        // exactly 2 elements [min, max]. Other arities warn-once and
        // degrade to Auto. (Bevy 0.18 Reflect lacks a Box<T> impl, so we
        // can't store this as `(Box<TrackSize>, Box<TrackSize>)`.)
        TrackSize::MinMax(parts) if parts.len() == 2 => {
            minmax(track_to_min(&parts[0]), track_to_max(&parts[1]))
        }
        TrackSize::MinMax(_) => {
            warn_once_invalid_track_nesting();
            auto()
        }
        TrackSize::Repeat(_, _) => {
            warn_once_invalid_track_nesting();
            auto()
        }
        TrackSize::Subgrid => {
            warn_once_subgrid();
            auto()
        }
    }
}

fn track_to_min(t: &TrackSize) -> MinTrackSizingFunction {
    match t {
        TrackSize::Auto => auto(),
        TrackSize::MinContent => min_content(),
        TrackSize::MaxContent => max_content(),
        TrackSize::Length(Length::Px(v)) => length(*v),
        TrackSize::Length(Length::Percent(p)) => percent(p / 100.0),
        TrackSize::Length(
            Length::Cqw(p)
            | Length::Cqh(p)
            | Length::Cqi(p)
            | Length::Cqb(p)
            | Length::Cqmin(p)
            | Length::Cqmax(p),
        ) => length(cq_unit_fallback_px(*p)),
        // CSS forbids these in MinMax's min slot:
        // - Fr (fr-in-min is grammar-invalid)
        // - FitContent (Min has no TaffyFitContent impl in Taffy 0.10)
        // - MinMax / Repeat / Subgrid (recursion-invalid)
        TrackSize::Length(Length::Fr(_))
        | TrackSize::FitContent(_)
        | TrackSize::MinMax(_)
        | TrackSize::Repeat(_, _)
        | TrackSize::Subgrid => {
            warn_once_invalid_track_nesting();
            auto()
        }
    }
}

fn track_to_max(t: &TrackSize) -> MaxTrackSizingFunction {
    match t {
        TrackSize::Auto => auto(),
        TrackSize::MinContent => min_content(),
        TrackSize::MaxContent => max_content(),
        // MaxTrackSizingFunction has TaffyFitContent impl (Taffy 0.10
        // grid.rs:700) — fit_content() from prelude resolves to it.
        TrackSize::FitContent(l) => fit_content(length_to_lp(*l)),
        TrackSize::Length(Length::Fr(v)) => fr(*v),
        TrackSize::Length(Length::Px(v)) => length(*v),
        TrackSize::Length(Length::Percent(p)) => percent(p / 100.0),
        TrackSize::Length(
            Length::Cqw(p)
            | Length::Cqh(p)
            | Length::Cqi(p)
            | Length::Cqb(p)
            | Length::Cqmin(p)
            | Length::Cqmax(p),
        ) => length(cq_unit_fallback_px(*p)),
        TrackSize::MinMax(_) | TrackSize::Repeat(_, _) | TrackSize::Subgrid => {
            warn_once_invalid_track_nesting();
            auto()
        }
    }
}

/// Convert a top-level `TrackSize` (in `template_columns` / `template_rows`)
/// into a `taffy::GridTemplateComponent`. `Repeat` is permitted at this
/// level (but not nested inside another `Repeat` or `MinMax`).
///
/// Return type uses the default `<S>` (= `String` via `DefaultCheapStr`),
/// matching `taffy::Style`'s default. Annotating `<&'static str>` would
/// fail because runtime `String` has no 'static lifetime.
fn track_to_template(t: &TrackSize) -> GridTemplateComponent<String> {
    match t {
        TrackSize::Repeat(count, tracks) => {
            GridTemplateComponent::Repeat(taffy::GridTemplateRepetition {
                count: map_repeat_count(*count),
                tracks: tracks.iter().map(track_to_sizing).collect(),
                // line_names is Vec<Vec<S>>; an empty outer Vec means
                // no named lines are declared on this repeat.
                line_names: Vec::new(),
            })
        }
        other => GridTemplateComponent::Single(track_to_sizing(other)),
    }
}

#[derive(Clone, Copy)]
enum GridAxis {
    Column,
    Row,
}

/// Convert a `GridLine` plus optional parent named-area registry into a
/// `taffy::Line<GridPlacement>`. `axis` selects column vs row resolution
/// when the line is `Area(name)`.
///
/// Note on indexing: `NamedArea`'s coordinates are 0-indexed (CSS
/// authoring convention 0..N maps to grid cells 0..N), while Taffy's
/// `GridPlacement::Line` uses CSS Grid's 1-indexed line coordinates
/// (line 1 is the start of the explicit grid). We add 1 when emitting.
fn grid_line_to_taffy(
    line: &GridLine,
    axis: GridAxis,
    parent_areas: Option<&GridAreas>,
) -> taffy::Line<GridPlacement<String>> {
    match line {
        GridLine::Auto => taffy::Line {
            start: GridPlacement::Auto,
            end: GridPlacement::Auto,
        },
        GridLine::Start(i) => taffy::Line {
            start: GridPlacement::Line((*i).into()),
            end: GridPlacement::Auto,
        },
        GridLine::Span(n) => taffy::Line {
            start: GridPlacement::Span(*n),
            end: GridPlacement::Auto,
        },
        GridLine::StartEnd(s, e) => taffy::Line {
            start: GridPlacement::Line((*s).into()),
            end: GridPlacement::Line((*e).into()),
        },
        GridLine::Area(name) => {
            match parent_areas.and_then(|areas| areas.areas.iter().find(|a| a.name == *name)) {
                Some(a) => match axis {
                    GridAxis::Column => taffy::Line {
                        // CSS named-area resolution: column_start (0-indexed)
                        // becomes line (column_start + 1) in 1-indexed CSS,
                        // and column_end becomes line (column_end + 1).
                        start: GridPlacement::Line(((a.column_start as i16) + 1).into()),
                        end: GridPlacement::Line(((a.column_end as i16) + 1).into()),
                    },
                    GridAxis::Row => taffy::Line {
                        start: GridPlacement::Line(((a.row_start as i16) + 1).into()),
                        end: GridPlacement::Line(((a.row_end as i16) + 1).into()),
                    },
                },
                None => {
                    warn_once_unresolved_area(name);
                    taffy::Line {
                        start: GridPlacement::Auto,
                        end: GridPlacement::Auto,
                    }
                }
            }
        }
    }
}

// Warn-once gates for invalid track nesting + unresolved named areas +
// Subgrid + Masonry. The Fr-outside-grid gate `WARNED_FR_OUTSIDE_GRID` is
// declared at the top of the file and is shared with the length_to_*
// helpers.
static WARNED_INVALID_TRACK_NESTING: AtomicBool = AtomicBool::new(false);
static WARNED_UNRESOLVED_AREA: AtomicBool = AtomicBool::new(false);
static WARNED_SUBGRID: AtomicBool = AtomicBool::new(false);
static WARNED_MASONRY: AtomicBool = AtomicBool::new(false);

fn warn_once_invalid_track_nesting() {
    if !WARNED_INVALID_TRACK_NESTING.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: invalid TrackSize nesting (Repeat inside Repeat/MinMax, \
             non-leaf inside MinMax slot, or MinMax with arity != 2) — \
             falling back to Auto (warned once)"
        );
    }
}

fn warn_once_unresolved_area(name: &str) {
    if !WARNED_UNRESOLVED_AREA.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: GridLine::Area({:?}) did not match any name in the parent's \
             template_areas; falling back to Auto (warned once)",
            name
        );
    }
}

fn warn_once_subgrid() {
    if !WARNED_SUBGRID.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: TrackSize::Subgrid is reserved — Taffy 0.10 has no subgrid \
             support; falling back to Auto (warned once)"
        );
    }
}

fn warn_once_masonry() {
    if !WARNED_MASONRY.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: GridAutoFlow::Masonry is reserved — CSS-WG flux + no Taffy \
             support; falling back to Row (warned once)"
        );
    }
}

static WARNED_SIDEWAYS_FALLBACK: AtomicBool = AtomicBool::new(false);

fn warn_once_sideways_layout_fallback() {
    if !WARNED_SIDEWAYS_FALLBACK.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: WritingModeKind::Sideways{{Rl,Lr}} glyph rotation lives in \
             buiy-text-rendering-design; layout treats them as VerticalRl / \
             VerticalLr (warned once)"
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::components::{
        BoxModel, Display, FlexItem, FlexParams, GridItem, GridParams, Overflow, Position, Scroll,
        WritingModeResolved,
    };
    use crate::layout::types::{
        AlignItems, BoxSizing, Direction, Edges, FlexAxis, FlexGap, FlexWrap, GridAreas, GridLine,
        JustifyContent, Length, NamedArea, OverflowMode, PositionKind, RepeatCount, ScrollbarWidth,
        Sizing, TrackSize,
    };

    #[test]
    fn translate_default_components_to_taffy_default() {
        let bm = BoxModel::default();
        let display = Display::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let item: Option<&FlexItem> = None;
        let overflow = Overflow::default();
        let scroll = Scroll::default();
        let grid_params = GridParams::default();
        let writing_mode_resolved = WritingModeResolved::default();
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: item,
            overflow: &overflow,
            scroll: &scroll,
            grid_params: &grid_params,
            grid_item: None,
            parent_areas: None,
            writing_mode_resolved: &writing_mode_resolved,
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
        let overflow = Overflow::default();
        let scroll = Scroll::default();
        let grid_params = GridParams::default();
        let writing_mode_resolved = WritingModeResolved::default();
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: None,
            overflow: &overflow,
            scroll: &scroll,
            grid_params: &grid_params,
            grid_item: None,
            parent_areas: None,
            writing_mode_resolved: &writing_mode_resolved,
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
        let overflow = Overflow::default();
        let scroll = Scroll::default();
        let grid_params = GridParams::default();
        let writing_mode_resolved = WritingModeResolved::default();
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: None,
            overflow: &overflow,
            scroll: &scroll,
            grid_params: &grid_params,
            grid_item: None,
            parent_areas: None,
            writing_mode_resolved: &writing_mode_resolved,
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
        let overflow = Overflow::default();
        let scroll = Scroll::default();
        let grid_params = GridParams::default();
        let writing_mode_resolved = WritingModeResolved::default();
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: Some(&item),
            overflow: &overflow,
            scroll: &scroll,
            grid_params: &grid_params,
            grid_item: None,
            parent_areas: None,
            writing_mode_resolved: &writing_mode_resolved,
        });
        assert_eq!(taffy.flex_grow, 2.0);
        assert_eq!(taffy.flex_shrink, 0.5);
        assert_eq!(taffy.flex_basis, taffy::Dimension::length(100.0));
        assert_eq!(taffy.align_self, Some(taffy::AlignSelf::Center));
        // FlexItem.order is stored but Taffy 0.10 has no `order` on
        // Style; Phase 1 does not honor it. Documented as a Phase 1
        // limitation in the translation module's doc comment.
    }

    #[test]
    fn translate_overflow_modes_to_taffy() {
        let display = Display::default();
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let cases: &[(OverflowMode, OverflowMode, taffy::Overflow, taffy::Overflow)] = &[
            (
                OverflowMode::Visible,
                OverflowMode::Visible,
                taffy::Overflow::Visible,
                taffy::Overflow::Visible,
            ),
            (
                OverflowMode::Hidden,
                OverflowMode::Hidden,
                taffy::Overflow::Hidden,
                taffy::Overflow::Hidden,
            ),
            (
                OverflowMode::Clip,
                OverflowMode::Clip,
                taffy::Overflow::Hidden,
                taffy::Overflow::Hidden,
            ),
            (
                OverflowMode::Scroll,
                OverflowMode::Auto,
                taffy::Overflow::Scroll,
                taffy::Overflow::Scroll,
            ),
            (
                OverflowMode::Auto,
                OverflowMode::Visible,
                taffy::Overflow::Scroll,
                taffy::Overflow::Visible,
            ),
        ];
        let grid_params = GridParams::default();
        let writing_mode_resolved = WritingModeResolved::default();
        for (x_in, y_in, x_expected, y_expected) in cases.iter().copied() {
            let overflow = Overflow {
                x: x_in,
                y: y_in,
                ..Default::default()
            };
            let scroll = Scroll::default();
            let taffy = style_to_taffy(StyleView {
                display: &display,
                box_model: &bm,
                position: &position,
                flex_params: &flex,
                flex_item: None,
                overflow: &overflow,
                scroll: &scroll,
                grid_params: &grid_params,
                grid_item: None,
                parent_areas: None,
                writing_mode_resolved: &writing_mode_resolved,
            });
            assert_eq!(
                taffy.overflow.x, x_expected,
                "x {x_in:?} → expected {x_expected:?}"
            );
            assert_eq!(
                taffy.overflow.y, y_expected,
                "y {y_in:?} → expected {y_expected:?}"
            );
        }
    }

    #[test]
    fn translate_scrollbar_width_to_taffy_f32() {
        let display = Display::default();
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let scroll = Scroll::default();
        let grid_params = GridParams::default();
        let writing_mode_resolved = WritingModeResolved::default();
        for (input, expected) in [
            (ScrollbarWidth::Auto, 12.0_f32),
            (ScrollbarWidth::Thin, 8.0),
            (ScrollbarWidth::None, 0.0),
        ] {
            let overflow = Overflow {
                scrollbar_width: input,
                ..Default::default()
            };
            let taffy = style_to_taffy(StyleView {
                display: &display,
                box_model: &bm,
                position: &position,
                flex_params: &flex,
                flex_item: None,
                overflow: &overflow,
                scroll: &scroll,
                grid_params: &grid_params,
                grid_item: None,
                parent_areas: None,
                writing_mode_resolved: &writing_mode_resolved,
            });
            assert_eq!(taffy.scrollbar_width, expected, "{input:?}");
        }
    }

    #[test]
    fn map_display_grid_routes_to_taffy_grid() {
        // Direct unit test of the helper. The full StyleView path is
        // tested in `translate_display_grid_to_taffy_grid` below now
        // that the view is widened with grid fields.
        assert_eq!(map_display(&Display::Grid), taffy::Display::Grid);
        assert_eq!(map_display(&Display::InlineGrid), taffy::Display::Grid);
    }

    #[test]
    fn translate_display_grid_to_taffy_grid() {
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let overflow = Overflow::default();
        let scroll = Scroll::default();
        let grid_params = GridParams::default();
        let writing_mode_resolved = WritingModeResolved::default();
        for display in [Display::Grid, Display::InlineGrid] {
            let taffy = style_to_taffy(StyleView {
                display: &display,
                box_model: &bm,
                position: &position,
                flex_params: &flex,
                flex_item: None,
                overflow: &overflow,
                scroll: &scroll,
                grid_params: &grid_params,
                grid_item: None,
                parent_areas: None,
                writing_mode_resolved: &writing_mode_resolved,
            });
            assert_eq!(taffy.display, taffy::Display::Grid, "{display:?}");
        }
    }

    #[test]
    fn translate_grid_template_columns_to_taffy() {
        let display = Display::Grid;
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let overflow = Overflow::default();
        let scroll = Scroll::default();
        let grid_params = GridParams {
            template_columns: vec![
                TrackSize::Length(Length::Fr(1.0)),
                TrackSize::Length(Length::Fr(2.0)),
            ],
            ..Default::default()
        };
        let writing_mode_resolved = WritingModeResolved::default();
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: None,
            overflow: &overflow,
            scroll: &scroll,
            grid_params: &grid_params,
            grid_item: None,
            parent_areas: None,
            writing_mode_resolved: &writing_mode_resolved,
        });
        assert_eq!(taffy.grid_template_columns.len(), 2);
        assert!(matches!(
            &taffy.grid_template_columns[0],
            taffy::GridTemplateComponent::Single(_)
        ));
    }

    #[test]
    fn translate_grid_repeat_to_taffy() {
        let display = Display::Grid;
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let overflow = Overflow::default();
        let scroll = Scroll::default();
        let grid_params = GridParams {
            template_columns: vec![TrackSize::Repeat(
                RepeatCount::AutoFill,
                vec![TrackSize::Length(Length::Px(100.0))],
            )],
            ..Default::default()
        };
        let writing_mode_resolved = WritingModeResolved::default();
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: None,
            overflow: &overflow,
            scroll: &scroll,
            grid_params: &grid_params,
            grid_item: None,
            parent_areas: None,
            writing_mode_resolved: &writing_mode_resolved,
        });
        assert_eq!(taffy.grid_template_columns.len(), 1);
        assert!(matches!(
            &taffy.grid_template_columns[0],
            taffy::GridTemplateComponent::Repeat(_)
        ));
    }

    #[test]
    fn translate_grid_line_start_end_to_taffy() {
        let display = Display::Grid;
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let overflow = Overflow::default();
        let scroll = Scroll::default();
        let grid_params = GridParams::default();
        let item = GridItem {
            column: GridLine::StartEnd(1, 4),
            row: GridLine::Auto,
            ..Default::default()
        };
        let writing_mode_resolved = WritingModeResolved::default();
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: None,
            overflow: &overflow,
            scroll: &scroll,
            grid_params: &grid_params,
            grid_item: Some(&item),
            parent_areas: None,
            writing_mode_resolved: &writing_mode_resolved,
        });
        // Line(1) and Line(4) — the values are GridPlacement variants.
        // Pin the discriminants by construction.
        assert!(matches!(
            taffy.grid_column.start,
            taffy::GridPlacement::Line(_)
        ));
        assert!(matches!(
            taffy.grid_column.end,
            taffy::GridPlacement::Line(_)
        ));
    }

    #[test]
    fn translate_grid_line_area_resolved_via_parent_areas() {
        let display = Display::Grid;
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let overflow = Overflow::default();
        let scroll = Scroll::default();
        let grid_params = GridParams::default();
        let item = GridItem {
            column: GridLine::Area("header".to_string()),
            row: GridLine::Area("header".to_string()),
            ..Default::default()
        };
        let parent_areas = GridAreas {
            areas: vec![NamedArea {
                name: "header".to_string(),
                row_start: 0,
                row_end: 1,
                column_start: 0,
                column_end: 2,
            }],
        };
        let writing_mode_resolved = WritingModeResolved::default();
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: None,
            overflow: &overflow,
            scroll: &scroll,
            grid_params: &grid_params,
            grid_item: Some(&item),
            parent_areas: Some(&parent_areas),
            writing_mode_resolved: &writing_mode_resolved,
        });
        // Column resolves to Line(1)..Line(3) (1-indexed, end is exclusive
        // in CSS spec terms — column_start 0 → Line(1), column_end 2 →
        // Line(3), spanning 2 cells).
        assert!(matches!(
            taffy.grid_column.start,
            taffy::GridPlacement::Line(_)
        ));
        assert!(matches!(
            taffy.grid_column.end,
            taffy::GridPlacement::Line(_)
        ));
    }

    #[test]
    fn translate_direction_rtl_to_taffy_rtl() {
        let display = Display::default();
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let overflow = Overflow::default();
        let scroll = Scroll::default();
        let grid_params = GridParams::default();
        let wmr = WritingModeResolved {
            direction: Direction::Rtl,
            ..Default::default()
        };
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: None,
            overflow: &overflow,
            scroll: &scroll,
            grid_params: &grid_params,
            grid_item: None,
            parent_areas: None,
            writing_mode_resolved: &wmr,
        });
        assert!(matches!(taffy.direction, taffy::Direction::Rtl));
    }

    #[test]
    fn translate_direction_ltr_to_taffy_ltr() {
        let display = Display::default();
        let bm = BoxModel::default();
        let position = Position::default();
        let flex = FlexParams::default();
        let overflow = Overflow::default();
        let scroll = Scroll::default();
        let grid_params = GridParams::default();
        let wmr = WritingModeResolved::default();
        let taffy = style_to_taffy(StyleView {
            display: &display,
            box_model: &bm,
            position: &position,
            flex_params: &flex,
            flex_item: None,
            overflow: &overflow,
            scroll: &scroll,
            grid_params: &grid_params,
            grid_item: None,
            parent_areas: None,
            writing_mode_resolved: &wmr,
        });
        assert!(matches!(taffy.direction, taffy::Direction::Ltr));
    }
}
