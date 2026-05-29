//! Layout value types — units, edges, axis enums, position kind.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/box-model.md and
//! display-and-positioning.md.
//!
//! Phase 1 ships `Length::Px` / `Length::Percent` and the `Sizing` /
//! `Edges` / `BoxSizing` shapes. Em / Rem / viewport / container / Fr /
//! Calc resolution lands in Phase 10 (`buiy-layout-units-calc`); intrinsic
//! sizing keywords resolve to `Auto` until text rendering integrates.

use bevy::prelude::*;
use bevy::reflect::impl_reflect_opaque;

/// CSS-style length value.
///
/// Phase 1 shipped `Px`, `Percent`. Phase 3 added `Fr` (grid-only).
/// Phase 5 adds the container-query unit family (`Cqw`/`Cqh`/`Cqi`/`Cqb`/
/// `Cqmin`/`Cqmax`). Em / Rem / viewport / Calc resolution remains
/// deferred to Phase 10 (`buiy-layout-units-calc`).
///
/// Container units resolve in `style_to_taffy` against the entity's
/// nearest *queried* ancestor's previous-frame `ResolvedLayout` (an
/// ancestor whose `Container.container_type != Normal`). When no
/// queried ancestor exists, container units fall back to viewport
/// dimensions (resolved directly from `bevy::window::Window` until
/// Phase 10's `Length::Vw/Vh` infrastructure lands) with one `warn!`
/// per session (a single global `AtomicBool` gate). Spec:
/// docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.4.
#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub enum Length {
    /// Absolute logical pixels.
    Px(f32),
    /// Percentage of the containing block dimension on the relevant axis.
    Percent(f32),
    /// CSS `<flex>` unit — only meaningful inside `TrackSize::Length(Length::Fr(_))`.
    /// Outside grid templates, `Fr` warns once and resolves to `Auto` (or `0`px
    /// where the Taffy target type has no `Auto` variant — gap/padding/border,
    /// which translate through `length_to_lp`).
    Fr(f32),
    /// `cqw` — percentage of nearest queried ancestor's *width*.
    Cqw(f32),
    /// `cqh` — percentage of nearest queried ancestor's *height*.
    Cqh(f32),
    /// `cqi` — percentage of nearest queried ancestor's *inline* axis
    /// (depends on writing-mode).
    Cqi(f32),
    /// `cqb` — percentage of nearest queried ancestor's *block* axis.
    Cqb(f32),
    /// `cqmin` — percentage of `min(cqi, cqb)`.
    Cqmin(f32),
    /// `cqmax` — percentage of `max(cqi, cqb)`.
    Cqmax(f32),
}

impl Length {
    pub const ZERO: Self = Self::Px(0.0);

    pub const fn px(v: f32) -> Self {
        Self::Px(v)
    }

    pub const fn percent(v: f32) -> Self {
        Self::Percent(v)
    }
}

impl Default for Length {
    fn default() -> Self {
        Self::ZERO
    }
}

/// Width / height / min / max value type. Phase 1 ships `Auto`, `None`
/// (max-only), `Length`, and `Stretch`. Intrinsic keywords ship as
/// variants but resolve to `Auto` until Phase 10 + text rendering land.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq)]
pub enum Sizing {
    #[default]
    Auto,
    /// Valid only on `max-*` (semantics: no upper bound).
    None,
    Length(Length),
    /// CSS `min-content`. Resolves to `Auto` until text rendering integrates.
    MinContent,
    /// CSS `max-content`. Resolves to `Auto` until text rendering integrates.
    MaxContent,
    /// CSS `fit-content(<length>)`. Resolves to `Auto` until text rendering integrates.
    FitContent(Length),
    /// CSS `stretch` — fills the parent's free space along the affected axis.
    Stretch,
}

/// Per-edge length values for padding, margin, border, inset.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq)]
pub struct Edges {
    pub top: Length,
    pub right: Length,
    pub bottom: Length,
    pub left: Length,
}

impl Edges {
    pub const ZERO: Self = Self {
        top: Length::ZERO,
        right: Length::ZERO,
        bottom: Length::ZERO,
        left: Length::ZERO,
    };

    /// Uniform value on every edge.
    pub const fn all(v: f32) -> Self {
        Self {
            top: Length::Px(v),
            right: Length::Px(v),
            bottom: Length::Px(v),
            left: Length::Px(v),
        }
    }

    /// Distinct horizontal vs. vertical values.
    pub const fn axis(x: f32, y: f32) -> Self {
        Self {
            top: Length::Px(y),
            right: Length::Px(x),
            bottom: Length::Px(y),
            left: Length::Px(x),
        }
    }
}

/// `box-sizing` policy. CSS default is `ContentBox`; app UIs typically prefer
/// `BorderBox`. The Buiy default theme does not override the component
/// default — authors opt in.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoxSizing {
    #[default]
    ContentBox,
    BorderBox,
}

/// `aspect-ratio` value. Phase 1 stores a single ratio; CSS's
/// `aspect-ratio: auto` (intrinsic dimensions take precedence) is
/// represented by *not setting* `BoxModel.aspect_ratio` (the field is
/// `Option<AspectRatio>`). Stored on `BoxModel` only when the author
/// explicitly opts in.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq)]
pub struct AspectRatio {
    pub ratio: f32,
}

/// Flex / inline-flex main axis.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexAxis {
    #[default]
    Row,
    Column,
    RowReverse,
    ColumnReverse,
}

/// Flex wrap mode.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlexWrap {
    #[default]
    NoWrap,
    Wrap,
    WrapReverse,
}

/// Main-axis distribution of flex / grid items. CSS `justify-content`.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum JustifyContent {
    #[default]
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Cross-axis alignment of flex / grid items within their line. CSS `align-items`.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignItems {
    #[default]
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
    Baseline,
}

/// Cross-axis distribution of flex / grid lines (multi-line containers).
/// CSS `align-content`.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AlignContent {
    #[default]
    Stretch,
    FlexStart,
    FlexEnd,
    Center,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Flex / grid gap, distinguished by axis.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq)]
pub struct FlexGap {
    pub row: Length,
    pub column: Length,
}

/// Position kind. `Static`, `Relative`, and `Absolute` pass through to
/// Taffy directly. `Sticky` is fully implemented via sub-pass 6a
/// (`sticky_offset`, Phase 7) as a post-Taffy override; for the Taffy
/// pass itself it maps to `Relative`. `Fixed` remains a fall-back-to-
/// `Absolute` stub pending Phase 8 (top-layer / fixed-as-viewport). No
/// `warn!` is emitted for either translation.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PositionKind {
    #[default]
    Static,
    Relative,
    Absolute,
    Fixed,
    Sticky,
}

/// Inset values (`top`/`right`/`bottom`/`left`).
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq)]
pub struct Inset {
    pub top: Sizing,
    pub right: Sizing,
    pub bottom: Sizing,
    pub left: Sizing,
}

impl Inset {
    /// Place the anchored entity ABOVE the anchor: anchored box's bottom
    /// edge is `dist` above the anchor's top edge.
    ///
    /// Sets `bottom = Sizing::Length(dist)` and leaves the other three
    /// sides at `Sizing::default()` (`Auto`). Spec authoring example:
    /// docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.3.
    pub fn above(dist: Length) -> Self {
        Self {
            bottom: Sizing::Length(dist),
            ..Default::default()
        }
    }

    /// Place the anchored entity BELOW the anchor: anchored box's top
    /// edge is `dist` below the anchor's bottom edge.
    ///
    /// Sets `top = Sizing::Length(dist)`.
    pub fn below(dist: Length) -> Self {
        Self {
            top: Sizing::Length(dist),
            ..Default::default()
        }
    }

    /// Place the anchored entity to the LEFT of the anchor: anchored
    /// box's right edge is `dist` left of the anchor's left edge.
    ///
    /// Sets `right = Sizing::Length(dist)`.
    pub fn left_of(dist: Length) -> Self {
        Self {
            right: Sizing::Length(dist),
            ..Default::default()
        }
    }

    /// Place the anchored entity to the RIGHT of the anchor: anchored
    /// box's left edge is `dist` right of the anchor's right edge.
    ///
    /// Sets `left = Sizing::Length(dist)`.
    pub fn right_of(dist: Length) -> Self {
        Self {
            left: Sizing::Length(dist),
            ..Default::default()
        }
    }
}

/// Per-axis overflow handling. CSS `overflow`.
///
/// `Visible` (default) lets children render outside the box. `Hidden` and
/// `Clip` clip without scrolling; the difference is render-side (per spec
/// § 1.1, both map to `taffy::Overflow::Hidden`). `Scroll` always shows a
/// scrollbar; `Auto` shows one only when content overflows. Layout
/// treats `Scroll` and `Auto` identically (both produce a scroll
/// container per § 1.2); the distinction is rendering's.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverflowMode {
    #[default]
    Visible,
    Hidden,
    Clip,
    Scroll,
    Auto,
}

/// CSS `scrollbar-gutter`. `Stable` reserves space even when not
/// scrolling; `StableBothEdges` reserves on both inline edges (useful
/// for centering). Phase 2 stores the value but does not yet enforce
/// `Stable` on non-scrolling containers — see plan coverage map.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollbarGutter {
    #[default]
    Auto,
    Stable,
    StableBothEdges,
}

/// CSS `scrollbar-width`. Drives `taffy::Style.scrollbar_width`:
/// `Auto → 12.0`, `Thin → 8.0`, `None → 0.0`.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollbarWidth {
    #[default]
    Auto,
    Thin,
    None,
}

/// CSS `scrollbar-color`. Render-side concern; layout stores only.
///
/// `Color` derives `Reflect` but not `Eq` (it contains `f32`), so this
/// enum derives `PartialEq` only — matching the rest of the file's
/// convention for types that contain floats.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq)]
pub enum ScrollbarColor {
    #[default]
    Auto,
    Custom {
        thumb: Color,
        track: Color,
    },
}

/// CSS `scroll-behavior`. Honored by `BuiySet::Animate` for programmatic
/// scrolls; layout doesn't act on it.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollBehavior {
    #[default]
    Auto,
    Smooth,
}

/// CSS `overscroll-behavior`, per axis. Honored by
/// `buiy-input-events-design`'s scroll handler; layout stores only.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum OverscrollBehavior {
    #[default]
    Auto,
    Contain,
    None,
}

/// CSS `scroll-snap-type`. `*Mandatory` means snap is required after
/// scroll ends; `*Proximity` only snaps when close enough.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapType {
    #[default]
    None,
    XMandatory,
    XProximity,
    YMandatory,
    YProximity,
    BothMandatory,
    BothProximity,
}

/// CSS `scroll-snap-align`. Per-item alignment to the snap viewport.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapAlign {
    #[default]
    None,
    Start,
    End,
    Center,
}

/// CSS `scroll-snap-stop`. `Always` forces snap to land on this item
/// even if a fast fling would skip past.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapStop {
    #[default]
    Normal,
    Always,
}

/// CSS `repeat(<count>, ...)` repetition count.
///
/// Spec uses `u32`; Phase 3 uses `u16` to match Taffy 0.10's
/// `RepetitionCount::Count(u16)` directly without a lossy conversion at
/// translate time. 65 535 repetitions is well above any realistic UI grid.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum RepeatCount {
    #[default]
    AutoFill,
    AutoFit,
    Count(u16),
}

/// A CSS Grid track sizing function — what one column or row of a grid
/// template can be. Used inside `GridParams.template_columns` /
/// `template_rows` (where `Repeat` is permitted) and recursively inside
/// `MinMax` (where it's not — see translation gates in `translate.rs`).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 2.1.
///
/// Recursion is permitted by the type but constrained by CSS grammar:
/// `MinMax(Repeat(...), _)` and `Repeat(_, [Subgrid])` are invalid CSS;
/// `style_to_taffy` emits `warn!` once per session and falls back to
/// `Auto` for these.
///
/// Implementation note: `MinMax`'s two arguments are stored as a
/// `Vec<TrackSize>` (expected `len() == 2`) rather than the spec's
/// `(Box<TrackSize>, Box<TrackSize>)` because `bevy_reflect` 0.18 has
/// no `Reflect` impl for `Box<T>`. Translation validates the arity and
/// emits `warn!` once per session if it isn't exactly 2.
#[derive(Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(no_field_bounds)]
pub enum TrackSize {
    #[default]
    Auto,
    Length(Length),
    MinContent,
    MaxContent,
    FitContent(Length),
    /// CSS `minmax(<min>, <max>)`. Vec is expected to contain exactly
    /// 2 elements `[min, max]`; other arities warn-once and fall back
    /// to `Auto` at translation time.
    MinMax(Vec<TrackSize>),
    /// CSS `repeat(<count>, <tracks>)`. Only valid at the top of a
    /// template list (not inside another `Repeat` or inside `MinMax`).
    Repeat(RepeatCount, Vec<TrackSize>),
    /// CSS `subgrid`. Reserved — Taffy 0.10 has no subgrid support.
    /// Phase 3 emits one `warn!` per session and falls back to `Auto`.
    Subgrid,
}

/// A CSS Grid placement on the `grid-row` or `grid-column` axis.
///
/// Spec uses `i32` / `u32`; Phase 3 uses `i16` / `u16` to match Taffy
/// 0.10's `GridLine` / `Span` underlying types. Spec uses `SmolStr` for
/// area names; Phase 3 uses `String` to avoid a new direct dep — area
/// names are set once per spawn and never on a hot path.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 2.2.
#[derive(Reflect, Default, Clone, Debug, PartialEq, Eq)]
pub enum GridLine {
    #[default]
    Auto,
    /// 1-indexed line; negative counts from the end (per CSS).
    Start(i16),
    /// Span N tracks from the auto-placed origin.
    Span(u16),
    /// Explicit `<start> / <end>`.
    StartEnd(i16, i16),
    /// Resolved against the parent container's `GridParams.template_areas`.
    /// If the name doesn't match any area, `style_to_taffy` warns and
    /// falls back to `Auto`.
    Area(String),
}

/// One named cell rectangle inside a `GridAreas`. CSS `grid-template-areas`
/// requires every named region to be a rectangle; `GridAreas::from_lines`
/// validates that.
///
/// Coordinates are zero-based and exclusive on the end side
/// (`row_end - row_start` is the span height in rows).
#[derive(Reflect, Default, Clone, Debug, PartialEq, Eq)]
pub struct NamedArea {
    pub name: String,
    pub row_start: u16,
    pub row_end: u16,
    pub column_start: u16,
    pub column_end: u16,
}

/// CSS `grid-template-areas` — a registry of named rectangular regions
/// laid out across the container's grid.
///
/// Construct from explicit rectangles via `area(...)` calls, or from CSS
/// string-grid syntax via `from_lines`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 2.1.
#[derive(Reflect, Default, Clone, Debug, PartialEq, Eq)]
pub struct GridAreas {
    pub areas: Vec<NamedArea>,
}

impl GridAreas {
    // Consumed by Phase 3 Task 7 (re-exports) and downstream Style /
    // GridParams builders introduced in Task 3; unused until those tasks land.
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self::default()
    }

    /// Append one explicit area. `rows` and `cols` are exclusive-end ranges.
    // Consumed by Phase 3 Task 3 (Style fluent setters); unused until then.
    #[allow(dead_code)]
    pub fn area(
        mut self,
        name: impl Into<String>,
        rows: std::ops::Range<u16>,
        cols: std::ops::Range<u16>,
    ) -> Self {
        self.areas.push(NamedArea {
            name: name.into(),
            row_start: rows.start,
            row_end: rows.end,
            column_start: cols.start,
            column_end: cols.end,
        });
        self
    }

    /// Parse CSS-style `grid-template-areas` lines: each `&str` is one row,
    /// space-separated cells. The literal `.` (period) is an empty cell.
    /// Identical adjacent cells form one named region; the parser groups
    /// them into the smallest enclosing rectangle.
    ///
    /// CSS requires every named area to be rectangular. If a name appears
    /// in non-rectangular cells, the parser still emits the bounding
    /// rectangle and a `warn!` is emitted once at translation time when
    /// the area is referenced (by `style_to_taffy`'s area-resolution
    /// helper, not here — `from_lines` does no logging).
    // Consumed by Phase 3 Task 3 (Style fluent setters) and the test below;
    // unused at lib build until Task 3 lands (test-only consumers don't
    // count toward dead_code at lib level).
    #[allow(dead_code)]
    pub fn from_lines(lines: &[&str]) -> Self {
        use std::collections::BTreeMap;
        // Parse into a 2D grid.
        let rows: Vec<Vec<&str>> = lines
            .iter()
            .map(|l| l.split_whitespace().collect())
            .collect();
        // Group by name, accumulating bounding rectangle.
        let mut bounds: BTreeMap<String, (u16, u16, u16, u16)> = BTreeMap::new();
        for (r, row) in rows.iter().enumerate() {
            for (c, &cell) in row.iter().enumerate() {
                if cell == "." {
                    continue;
                }
                let name = cell.to_string();
                let entry = bounds.entry(name).or_insert((
                    r as u16,
                    (r + 1) as u16,
                    c as u16,
                    (c + 1) as u16,
                ));
                entry.0 = entry.0.min(r as u16);
                entry.1 = entry.1.max((r + 1) as u16);
                entry.2 = entry.2.min(c as u16);
                entry.3 = entry.3.max((c + 1) as u16);
            }
        }
        let areas = bounds
            .into_iter()
            .map(|(name, (rs, re, cs, ce))| NamedArea {
                name,
                row_start: rs,
                row_end: re,
                column_start: cs,
                column_end: ce,
            })
            .collect();
        Self { areas }
    }
}

/// CSS `grid-auto-flow`. `*Dense` lets the placement algorithm backfill
/// earlier tracks. `Masonry` is reserved (CSS-WG flux) — Phase 3 emits one
/// `warn!` per session and falls back to `Row`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 2.4.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridAutoFlow {
    #[default]
    Row,
    Column,
    RowDense,
    ColumnDense,
    /// Reserved for forward compatibility — CSS Masonry Layout. Currently
    /// degrades to `Row` with one `warn!` per session.
    Masonry,
}

/// CSS `justify-items` — main-axis alignment of grid items within their
/// cell. (Distinct from `JustifyContent`, which distributes whole tracks.)
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum JustifyItems {
    #[default]
    Stretch,
    Start,
    End,
    Center,
    Baseline,
}

/// CSS `writing-mode`.
///
/// `Sideways{Rl,Lr}` are tier-C polish modes that rotate text glyphs but
/// otherwise behave like `Vertical{Rl,Lr}` for layout. Glyph rotation is
/// `buiy-text-rendering-design`'s concern; layout treats them as their
/// non-sideways equivalents and emits one `warn!` per session.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 2.1.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum WritingModeKind {
    #[default]
    HorizontalTb,
    VerticalRl,
    VerticalLr,
    SidewaysRl,
    SidewaysLr,
}

/// CSS `container-type`. Determines whether an entity is a query
/// container (i.e., whether descendant `@container` rules and container
/// units resolve against it).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.1.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ContainerType {
    /// Not a query container. The default.
    #[default]
    Normal,
    /// Both axes queryable; `cqw/cqh/cqi/cqb` all resolve.
    Size,
    /// Only inline axis queryable; `cqb` against this container falls
    /// back to viewport-block with warn-once.
    InlineSize,
}

/// CSS `@container (orientation: ...)` condition value.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.2.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Orientation {
    /// Container's inline axis is shorter than its block axis.
    #[default]
    Portrait,
    /// Container's inline axis is longer than its block axis.
    Landscape,
}

/// One `@container` condition — a single predicate on the resolved size
/// of the query container. A `ContainerQuery` AND-combines multiple of
/// these.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 1.2.
#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub enum QueryCondition {
    /// Activates when container `width >= value`.
    MinWidth(Length),
    /// Activates when container `width <= value`.
    MaxWidth(Length),
    /// Activates when container `height >= value`.
    MinHeight(Length),
    /// Activates when container `height <= value`.
    MaxHeight(Length),
    /// Activates when container `width/height >= ratio`.
    MinAspectRatio(f32),
    /// Activates when container `width/height <= ratio`.
    MaxAspectRatio(f32),
    /// Activates when container orientation matches.
    Orientation(Orientation),
}

/// CSS `direction`. Maps directly to `taffy::Direction`.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    #[default]
    Ltr,
    Rtl,
}

/// CSS `text-orientation`. Stored on `WritingMode`; consumed by
/// `buiy-text-rendering-design`, not layout.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TextOrientation {
    #[default]
    Mixed,
    Upright,
    Sideways,
}

/// CSS `unicode-bidi`. Stored on `WritingMode`; resolution lives in
/// `buiy-i18n-design`.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub enum UnicodeBidi {
    #[default]
    Normal,
    Embed,
    Isolate,
    BidiOverride,
    IsolateOverride,
    Plaintext,
}

/// Logical-edge values (writing-mode-aware). Construct + call `to_edges`
/// to get a physical `Edges` for layout consumption.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/box-model.md § 4 +
/// docs/specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md § 2.3.
#[derive(Reflect, Default, Clone, Copy, Debug, PartialEq)]
pub struct LogicalEdges {
    pub inline_start: Length,
    pub inline_end: Length,
    pub block_start: Length,
    pub block_end: Length,
}

impl LogicalEdges {
    /// Translate to physical `Edges` honoring the given writing-mode + direction.
    /// 6-row mapping table:
    /// - horizontal-tb + ltr: inline-start = left, block-start = top
    /// - horizontal-tb + rtl: inline-start = right, block-start = top
    /// - vertical-rl + ltr: inline-start = top, block-start = right
    /// - vertical-rl + rtl: inline-start = bottom, block-start = right
    /// - vertical-lr + ltr: inline-start = top, block-start = left
    /// - vertical-lr + rtl: inline-start = bottom, block-start = left
    ///
    /// Sideways modes (`SidewaysRl` / `SidewaysLr`) are normalized to
    /// their non-sideways vertical equivalents — glyph rotation lives
    /// in `buiy-text-rendering-design`, layout treats them identically.
    pub fn to_edges(self, mode: WritingModeKind, direction: Direction) -> Edges {
        use WritingModeKind::*;
        let mode = match mode {
            SidewaysRl => VerticalRl,
            SidewaysLr => VerticalLr,
            other => other,
        };
        match (mode, direction) {
            (HorizontalTb, Direction::Ltr) => Edges {
                left: self.inline_start,
                right: self.inline_end,
                top: self.block_start,
                bottom: self.block_end,
            },
            (HorizontalTb, Direction::Rtl) => Edges {
                right: self.inline_start,
                left: self.inline_end,
                top: self.block_start,
                bottom: self.block_end,
            },
            (VerticalRl, Direction::Ltr) => Edges {
                top: self.inline_start,
                bottom: self.inline_end,
                right: self.block_start,
                left: self.block_end,
            },
            (VerticalRl, Direction::Rtl) => Edges {
                bottom: self.inline_start,
                top: self.inline_end,
                right: self.block_start,
                left: self.block_end,
            },
            (VerticalLr, Direction::Ltr) => Edges {
                top: self.inline_start,
                bottom: self.inline_end,
                left: self.block_start,
                right: self.block_end,
            },
            (VerticalLr, Direction::Rtl) => Edges {
                bottom: self.inline_start,
                top: self.inline_end,
                left: self.block_start,
                right: self.block_end,
            },
            // Sideways modes were normalized above; this is unreachable.
            (SidewaysRl, _) | (SidewaysLr, _) => unreachable!("sideways normalized to vertical"),
        }
    }
}

/// CSS anchor name. `Implicit` means "referenced by `Entity` ID alone" —
/// no name lookup, the anchor target is identified directly. `Named(_)`
/// participates in the `AnchorNameRegistry` lookup.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.1.
///
/// Spec uses `SmolStr` for the named payload; Phase 6 follows the
/// Phase 3 `GridAreas` precedent and uses `String` to avoid a new direct
/// dep (`crates/buiy_core/src/layout/types.rs:394`).
#[derive(Reflect, Clone, Debug, PartialEq, Eq, Default)]
pub enum AnchorName {
    #[default]
    Implicit,
    Named(String),
}

/// A reference to an anchor target — either a direct `Entity` handle or
/// a name lookup against the `AnchorNameRegistry`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.1.
#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub enum AnchorRef {
    Entity(bevy::prelude::Entity),
    Name(String),
}

/// One entry in an `Anchor.position_try` fallback chain. The first
/// `PositionTry` whose `conditions` all evaluate true is applied.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.1.
#[derive(Reflect, Clone, Debug, PartialEq, Default)]
pub struct PositionTry {
    /// The offset relative to the anchor's resolved box for this try.
    pub inset: Inset,
    /// All conditions must pass for this try to apply.
    pub conditions: Vec<TryCondition>,
}

/// A condition guarding a `PositionTry`. All conditions on a try must
/// pass simultaneously for the try to apply.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.1.
#[derive(Reflect, Clone, Debug, PartialEq)]
pub enum TryCondition {
    /// The anchored entity's would-be box does not overflow the viewport.
    FitsInViewport,
    /// The anchored entity's would-be box fits inside the referenced
    /// container's resolved box. The container is identified the same
    /// way as `Anchor.position_anchor` — by `Entity` or by registered
    /// name.
    FitsInContainer(AnchorRef),
    /// The anchor's resolved box intersects the viewport.
    AnchorVisible,
}

// ============================================================
// Phase 7 — multi-column types (flex-and-grid.md § 3)
// ============================================================

/// CSS `column-count`. Tier-E. Currently a stub field on
/// `MultiColumn`; the algorithm warns-once and falls back to
/// single-column layout in v1.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.1.
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ColumnCount {
    #[default]
    Auto,
    Count(u32),
}

/// CSS `column-rule` shorthand (width / style / color triple).
/// Render side honors this; layout side passes it through.
///
/// `Color` derives `Reflect` but not `Eq` (it contains `f32`), so this
/// struct derives `PartialEq` only — matching the `ScrollbarColor`
/// convention at `types.rs:326-334`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.1.
#[derive(Reflect, Clone, Copy, PartialEq, Debug, Default)]
pub struct ColumnRule {
    pub width: Length,
    pub style: ColumnRuleStyle,
    pub color: bevy::color::Color,
}

/// CSS `column-rule-style`. Subset of CSS line-style values.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.1.
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ColumnRuleStyle {
    #[default]
    None,
    Solid,
    Dashed,
    Dotted,
    Double,
}

/// CSS `column-span`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.1.
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ColumnSpan {
    #[default]
    None,
    All,
}

/// CSS `column-fill`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.1.
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum ColumnFill {
    #[default]
    Balance,
    Auto,
}

/// CSS `break-inside`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.1.
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BreakInside {
    #[default]
    Auto,
    Avoid,
    AvoidColumn,
}

/// CSS `break-before`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.1.
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BreakBefore {
    #[default]
    Auto,
    Always,
    Avoid,
    Column,
    AvoidColumn,
}

/// CSS `break-after`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.1.
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum BreakAfter {
    #[default]
    Auto,
    Always,
    Avoid,
    Column,
    AvoidColumn,
}

/// Per-frame anchor-error category for the warn-dedup `HashSet` in
/// `LayoutAnchorWarnedThisFrame`. Spec § 3.2 step 4: "warn fires once
/// per (entity, frame)".
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.2.
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AnchorErrorKind {
    /// The anchor target was missing, despawned, or carried `Display::None`.
    TargetMissing,
    /// Every `PositionTry` in the chain failed its conditions.
    AllFallbacksFailed,
    /// The entity was in an anchor cycle; its edge was dropped.
    InCycle,
    /// Two entities declared the same `anchor_name`; the later wins.
    /// Reported on the *late* insert. Distinct from spec's "warn once
    /// per (name, frame)" only in that the per-entity gate also avoids
    /// repeat warns if the same entity re-inserts within the same frame.
    DuplicateName,
    /// `anchor-size()` used in a `PositionTry::inset` term. Tier-C
    /// deferred to v1.x; the term resolves to zero with a warn.
    AnchorSizeUsed,
}

/// Phase 7 — session-scoped warn-once dedup key. Variants cover the
/// non-anchor layout error/stub conditions introduced in Phase 7.
///
/// Anchor errors continue to use the per-frame
/// `LayoutAnchorWarnedThisFrame` resource — that divergence from
/// spec § 6 is preserved by Phase 7 (see Phase 6 CHANGELOG).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/architecture.md § 6
/// ("deduplicated via a `HashSet` resource cleared on `BuiyExit`")
/// and plan decision D3 (sticky inset semantics) +
/// D6 (per-session warn dedup) in
/// docs/plans/2026-05-22-buiy-layout-sticky-table-multicol.md.
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum LayoutWarnOnceKey {
    /// `Display::Table*` entity encountered. Sub-pass 6b emits one
    /// warn per (entity, session) — the table algorithm is deferred
    /// to v1.x.
    ///
    /// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 1.2.
    TableUnsupported(Entity),

    /// `MultiColumn` entity encountered. Sub-pass 6c emits one warn
    /// per session (no Entity payload — first multicol entity triggers,
    /// all subsequent are silent) — the multicol algorithm is
    /// deferred to v1.x.
    ///
    /// Spec: docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md § 3.2.
    MulticolUnsupported,

    /// Sticky entity uses `Length::Fr` inset. `fr` is grid-only;
    /// applying it as a sticky inset is semantically invalid. Warn
    /// once per (entity, session); inset resolves to 0.0.
    ///
    /// Spec: plan decision D3 in
    /// docs/plans/2026-05-22-buiy-layout-sticky-table-multicol.md.
    StickyFrUnsupported(Entity),

    /// Sticky entity uses a `Length::Cq*` inset (container query
    /// unit). Full cq-context resolution for sticky is deferred to
    /// a Phase 7.x follow-up (port from Phase 6 `length_inset_to_px`).
    /// v1 resolves to 0.0. One warn per (entity, session).
    ///
    /// Spec: plan decision D3 in
    /// docs/plans/2026-05-22-buiy-layout-sticky-table-multicol.md.
    StickyCqDeferred(Entity),
}

// ============================================================
// Phase 8 — transform value types (transforms-and-containment.md § 1)
// ============================================================

/// The transform matrix variant for `UiTransform`. `None` is identity.
/// `Compose([A, B, …])` is the matrix product `A · B · …` (outermost
/// first); the rightmost/innermost entry transforms a child point
/// first. See [`crate::layout::UiTransform`] composition convention.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1.
#[derive(Reflect, Clone, Default, PartialEq, Debug)]
pub enum TransformMatrix {
    /// Identity transform.
    #[default]
    None,
    /// 3D translate.
    Translate(Length, Length, Length),
    /// Arbitrary 3D rotation.
    Rotate(Quat),
    /// Per-axis scale.
    Scale(f32, f32, f32),
    /// Skew along x, y in radians.
    Skew(f32, f32),
    /// Explicit 4×4 matrix.
    Matrix(Mat4),
    /// Matrix product `A · B · …` (outermost first).
    Compose(Vec<TransformMatrix>),
}

/// CSS `transform-origin`. Default is `50% 50% 0` (hand-written —
/// `#[derive(Default)]` would give all-zero `Length`s, which is wrong).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1.
#[derive(Reflect, Clone, Copy, PartialEq, Debug)]
pub struct TransformOrigin {
    pub x: Length,
    pub y: Length,
    pub z: Length,
}

impl Default for TransformOrigin {
    fn default() -> Self {
        Self {
            x: Length::Percent(50.0),
            y: Length::Percent(50.0),
            z: Length::ZERO,
        }
    }
}

/// CSS `transform-style`. Render-side concern; layout stores.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1, § 4.
#[derive(Reflect, Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum TransformStyle {
    #[default]
    Flat,
    Preserve3d,
}

/// CSS `backface-visibility`. Render-side concern; layout stores.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 1, § 4.
#[derive(Reflect, Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum BackfaceVisibility {
    #[default]
    Visible,
    Hidden,
}

// ============================================================
// Phase 8 — containment value types (transforms-and-containment.md § 5)
// ============================================================

bitflags::bitflags! {
    /// CSS `contain` flags. `CONTENT` and `STRICT` are unions of the
    /// primitive bits (not standalone bits), so `.contains(PAINT)` is
    /// true for a `CONTENT`- or `STRICT`-contained entity.
    ///
    /// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 5.
    #[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub struct ContainFlags: u8 {
        /// Isolate internal layout from the rest of the tree.
        const LAYOUT      = 1 << 0;
        /// Confine painting to the entity's box.
        const PAINT       = 1 << 1;
        /// The entity's size must be explicit (auto → 0 with a warn).
        const SIZE        = 1 << 2;
        /// Counters / quotes do not escape the subtree.
        const STYLE       = 1 << 3;
        /// Inline-axis variant of `SIZE`.
        const INLINE_SIZE = 1 << 4;
        /// `contain: content` — union of `LAYOUT | PAINT | STYLE`.
        const CONTENT = Self::LAYOUT.bits() | Self::PAINT.bits() | Self::STYLE.bits();
        /// `contain: strict` — union of `LAYOUT | PAINT | SIZE | STYLE`.
        const STRICT  = Self::LAYOUT.bits()
            | Self::PAINT.bits()
            | Self::SIZE.bits()
            | Self::STYLE.bits();
    }
}

// `bitflags!` doesn't compose with `#[derive(Reflect)]` — register the
// opaque type manually (`impl_reflect_value!` → `impl_reflect_opaque!`
// in bevy_reflect 0.18).
impl_reflect_opaque!((in crate::layout::types) ContainFlags(Default, PartialEq));

/// CSS `content-visibility`. Phase 8 stores the value; `Auto` /
/// `Hidden` enforcement is deferred (warn-once
/// `LayoutWarnOnceKey::ContentVisibilityDeferred`).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 5, § 5.2.
#[derive(Reflect, Clone, Copy, Default, PartialEq, Eq, Debug)]
pub enum ContentVisibility {
    /// Always rendered (CSS initial value).
    #[default]
    Visible,
    /// Skip rendering off-screen content (deferred in Phase 8).
    Auto,
    /// Skip rendering content like `display: none` for descendants (deferred).
    Hidden,
}

/// CSS `will-change`. Tier-E forward-compat hint; Phase 8 stores only
/// (no layer promotion, no SC trigger — those are render / Phase 9).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 5.3.
#[derive(Reflect, Clone, Default, PartialEq, Debug)]
pub enum WillChange {
    /// No hint (CSS initial value).
    #[default]
    Auto,
    /// Author hints these properties will change.
    Properties(Vec<WillChangeProperty>),
}

/// Properties an author hints will change (`will-change: <prop>`).
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md § 5.3.
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug)]
pub enum WillChangeProperty {
    /// `transform`.
    Transform,
    /// `opacity`.
    Opacity,
    /// `filter`.
    Filter,
    /// `z-index`.
    ZIndex,
    /// `scroll-position`.
    ScrollPosition,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn length_constructors_round_trip() {
        assert_eq!(Length::px(10.0), Length::Px(10.0));
        assert_eq!(Length::percent(50.0), Length::Percent(50.0));
        assert_eq!(Length::ZERO, Length::Px(0.0));
    }

    #[test]
    fn transform_matrix_default_is_none() {
        assert_eq!(TransformMatrix::default(), TransformMatrix::None);
    }

    #[test]
    fn transform_origin_default_is_50_50_0() {
        let o = TransformOrigin::default();
        assert_eq!(o.x, Length::Percent(50.0));
        assert_eq!(o.y, Length::Percent(50.0));
        assert_eq!(o.z, Length::ZERO);
    }

    #[test]
    fn transform_style_and_backface_defaults() {
        assert_eq!(TransformStyle::default(), TransformStyle::Flat);
        assert_eq!(BackfaceVisibility::default(), BackfaceVisibility::Visible);
    }

    #[test]
    fn contain_content_includes_paint_layout_style() {
        assert!(ContainFlags::CONTENT.contains(ContainFlags::PAINT));
        assert!(ContainFlags::CONTENT.contains(ContainFlags::LAYOUT));
        assert!(ContainFlags::CONTENT.contains(ContainFlags::STYLE));
        assert!(!ContainFlags::CONTENT.contains(ContainFlags::SIZE));
    }

    #[test]
    fn contain_strict_includes_size() {
        assert!(ContainFlags::STRICT.contains(ContainFlags::SIZE));
        assert!(ContainFlags::STRICT.contains(ContainFlags::PAINT));
        assert!(ContainFlags::STRICT.contains(ContainFlags::LAYOUT));
        assert!(ContainFlags::STRICT.contains(ContainFlags::STYLE));
    }

    #[test]
    fn contain_flags_default_is_empty() {
        assert_eq!(ContainFlags::default(), ContainFlags::empty());
    }

    #[test]
    fn content_visibility_and_will_change_defaults() {
        assert_eq!(ContentVisibility::default(), ContentVisibility::Visible);
        assert_eq!(WillChange::default(), WillChange::Auto);
    }

    #[test]
    fn edges_helpers_produce_uniform_and_axis_values() {
        let all = Edges::all(8.0);
        assert_eq!(all.top, Length::Px(8.0));
        assert_eq!(all.right, Length::Px(8.0));
        assert_eq!(all.bottom, Length::Px(8.0));
        assert_eq!(all.left, Length::Px(8.0));

        let axis = Edges::axis(4.0, 12.0);
        assert_eq!(axis.left, Length::Px(4.0));
        assert_eq!(axis.right, Length::Px(4.0));
        assert_eq!(axis.top, Length::Px(12.0));
        assert_eq!(axis.bottom, Length::Px(12.0));

        assert_eq!(Edges::ZERO, Edges::all(0.0));
    }

    #[test]
    fn enum_defaults_match_spec() {
        assert_eq!(BoxSizing::default(), BoxSizing::ContentBox);
        assert_eq!(FlexAxis::default(), FlexAxis::Row);
        assert_eq!(FlexWrap::default(), FlexWrap::NoWrap);
        assert_eq!(JustifyContent::default(), JustifyContent::FlexStart);
        assert_eq!(AlignItems::default(), AlignItems::Stretch);
        assert_eq!(AlignContent::default(), AlignContent::Stretch);
        assert_eq!(PositionKind::default(), PositionKind::Static);
    }

    #[test]
    fn sizing_default_is_auto() {
        assert_eq!(Sizing::default(), Sizing::Auto);
    }

    #[test]
    fn overflow_mode_default_is_visible() {
        assert_eq!(OverflowMode::default(), OverflowMode::Visible);
    }

    #[test]
    fn scrollbar_gutter_default_is_auto() {
        assert_eq!(ScrollbarGutter::default(), ScrollbarGutter::Auto);
    }

    #[test]
    fn scrollbar_width_default_is_auto() {
        assert_eq!(ScrollbarWidth::default(), ScrollbarWidth::Auto);
    }

    #[test]
    fn scrollbar_color_default_is_auto() {
        assert_eq!(ScrollbarColor::default(), ScrollbarColor::Auto);
    }

    #[test]
    fn scroll_behavior_default_is_auto() {
        assert_eq!(ScrollBehavior::default(), ScrollBehavior::Auto);
    }

    #[test]
    fn overscroll_behavior_default_is_auto() {
        assert_eq!(OverscrollBehavior::default(), OverscrollBehavior::Auto);
    }

    #[test]
    fn snap_type_default_is_none() {
        assert_eq!(SnapType::default(), SnapType::None);
    }

    #[test]
    fn snap_align_default_is_none() {
        assert_eq!(SnapAlign::default(), SnapAlign::None);
    }

    #[test]
    fn snap_stop_default_is_normal() {
        assert_eq!(SnapStop::default(), SnapStop::Normal);
    }

    #[test]
    fn length_fr_constructor_round_trip() {
        // Pin the Fr variant. Used inside TrackSize::Length(Length::Fr(_));
        // outside grid contexts it warns and falls back to Auto.
        let fr = Length::Fr(1.5);
        match fr {
            Length::Fr(v) => assert_eq!(v, 1.5),
            _ => panic!("expected Fr"),
        }
    }

    #[test]
    fn length_container_unit_variants_distinct_and_round_trip() {
        // Same variant + same payload compares equal (exercises the
        // derived `PartialEq` end-to-end).
        assert_eq!(Length::Cqw(50.0), Length::Cqw(50.0));
        assert_eq!(Length::Cqh(25.0), Length::Cqh(25.0));
        assert_eq!(Length::Cqi(50.0), Length::Cqi(50.0));
        assert_eq!(Length::Cqb(25.0), Length::Cqb(25.0));
        assert_eq!(Length::Cqmin(10.0), Length::Cqmin(10.0));
        assert_eq!(Length::Cqmax(90.0), Length::Cqmax(90.0));

        // Different variants with the same payload compare *not* equal —
        // guards against a hand-written `PartialEq` impl that collapses
        // all `Cq*` variants together (would silently break container-
        // unit resolution while keeping these tests green).
        assert_ne!(Length::Cqw(50.0), Length::Cqh(50.0));
        assert_ne!(Length::Cqi(50.0), Length::Cqb(50.0));
        assert_ne!(Length::Cqmin(50.0), Length::Cqmax(50.0));
        assert_ne!(Length::Cqw(50.0), Length::Cqi(50.0));
        assert_ne!(Length::Cqh(50.0), Length::Cqb(50.0));

        // Different payload, same variant compare *not* equal.
        assert_ne!(Length::Cqw(50.0), Length::Cqw(51.0));
    }

    #[test]
    fn track_size_default_is_auto() {
        assert_eq!(TrackSize::default(), TrackSize::Auto);
    }

    #[test]
    fn repeat_count_count_carries_u16() {
        let _: RepeatCount = RepeatCount::Count(3u16);
        assert_eq!(RepeatCount::default(), RepeatCount::AutoFill);
    }

    #[test]
    fn grid_line_default_is_auto() {
        assert_eq!(GridLine::default(), GridLine::Auto);
    }

    #[test]
    fn grid_areas_from_lines_parses_simple_grid() {
        let g = GridAreas::from_lines(&["a a", "b ."]);
        let mut by_name: std::collections::BTreeMap<&str, &NamedArea> =
            g.areas.iter().map(|a| (a.name.as_str(), a)).collect();
        let a = by_name.remove("a").expect("area `a`");
        assert_eq!((a.row_start, a.row_end), (0, 1));
        assert_eq!((a.column_start, a.column_end), (0, 2));
        let b = by_name.remove("b").expect("area `b`");
        assert_eq!((b.row_start, b.row_end), (1, 2));
        assert_eq!((b.column_start, b.column_end), (0, 1));
        assert!(by_name.is_empty(), "no extra areas");
    }

    #[test]
    fn grid_auto_flow_default_is_row() {
        assert_eq!(GridAutoFlow::default(), GridAutoFlow::Row);
    }

    #[test]
    fn justify_items_default_is_stretch() {
        assert_eq!(JustifyItems::default(), JustifyItems::Stretch);
    }

    #[test]
    fn writing_mode_kind_default_is_horizontal_tb() {
        assert_eq!(WritingModeKind::default(), WritingModeKind::HorizontalTb);
    }

    #[test]
    fn container_type_default_is_normal() {
        assert_eq!(ContainerType::default(), ContainerType::Normal);
    }

    #[test]
    fn orientation_default_is_portrait() {
        // Width <= height -> portrait. CSS default ambiguous; we pick
        // Portrait so the default `Orientation(Portrait)` condition is
        // a useful sentinel.
        assert_eq!(Orientation::default(), Orientation::Portrait);
    }

    #[test]
    fn query_condition_variants_construct() {
        let c1 = QueryCondition::MinWidth(Length::Px(600.0));
        let c2 = QueryCondition::MaxAspectRatio(1.5);
        let c3 = QueryCondition::Orientation(Orientation::Landscape);
        // PartialEq derive covers structural equality.
        assert_ne!(c1, c2);
        assert_ne!(c2, c3);
        // Copy bound — implicit copy through assignment.
        let c4 = c1;
        assert_eq!(c4, c1);
    }

    #[test]
    fn direction_default_is_ltr() {
        assert_eq!(Direction::default(), Direction::Ltr);
    }

    #[test]
    fn text_orientation_default_is_mixed() {
        assert_eq!(TextOrientation::default(), TextOrientation::Mixed);
    }

    #[test]
    fn unicode_bidi_default_is_normal() {
        assert_eq!(UnicodeBidi::default(), UnicodeBidi::Normal);
    }

    #[test]
    fn logical_edges_to_edges_horizontal_tb_ltr() {
        let logical = LogicalEdges {
            inline_start: Length::Px(1.0),
            inline_end: Length::Px(2.0),
            block_start: Length::Px(3.0),
            block_end: Length::Px(4.0),
        };
        let physical = logical.to_edges(WritingModeKind::HorizontalTb, Direction::Ltr);
        assert_eq!(physical.left, Length::Px(1.0));
        assert_eq!(physical.right, Length::Px(2.0));
        assert_eq!(physical.top, Length::Px(3.0));
        assert_eq!(physical.bottom, Length::Px(4.0));
    }

    #[test]
    fn logical_edges_to_edges_vertical_rl_ltr() {
        let logical = LogicalEdges {
            inline_start: Length::Px(1.0),
            inline_end: Length::Px(2.0),
            block_start: Length::Px(3.0),
            block_end: Length::Px(4.0),
        };
        let physical = logical.to_edges(WritingModeKind::VerticalRl, Direction::Ltr);
        // vertical-rl + ltr: inline-start = top, block-start = right
        assert_eq!(physical.top, Length::Px(1.0));
        assert_eq!(physical.bottom, Length::Px(2.0));
        assert_eq!(physical.right, Length::Px(3.0));
        assert_eq!(physical.left, Length::Px(4.0));
    }

    #[test]
    fn logical_edges_to_edges_vertical_lr_ltr() {
        let logical = LogicalEdges {
            inline_start: Length::Px(1.0),
            inline_end: Length::Px(2.0),
            block_start: Length::Px(3.0),
            block_end: Length::Px(4.0),
        };
        let physical = logical.to_edges(WritingModeKind::VerticalLr, Direction::Ltr);
        // vertical-lr + ltr: inline-start = top, block-start = left
        assert_eq!(physical.top, Length::Px(1.0));
        assert_eq!(physical.bottom, Length::Px(2.0));
        assert_eq!(physical.left, Length::Px(3.0));
        assert_eq!(physical.right, Length::Px(4.0));
    }

    #[test]
    fn anchor_name_named_round_trips() {
        let n = AnchorName::Named("tooltip-anchor".into());
        let copy = n.clone();
        assert_eq!(n, copy);
    }

    #[test]
    fn anchor_name_implicit_vs_named_are_distinct() {
        assert_ne!(AnchorName::Implicit, AnchorName::Named("x".into()));
    }

    #[test]
    fn anchor_ref_entity_and_name_are_distinct() {
        let e = AnchorRef::Entity(bevy::prelude::Entity::PLACEHOLDER);
        let n = AnchorRef::Name("x".into());
        assert_ne!(e, n);
    }

    #[test]
    fn position_try_default_is_empty() {
        let p = PositionTry::default();
        assert_eq!(p.inset, Inset::default());
        assert!(p.conditions.is_empty());
    }

    #[test]
    fn try_condition_fits_in_container_carries_ref() {
        let c = TryCondition::FitsInContainer(AnchorRef::Name("parent".into()));
        let copy = c.clone();
        assert_eq!(c, copy);
    }

    #[test]
    fn try_condition_variants_are_distinct() {
        assert_ne!(TryCondition::FitsInViewport, TryCondition::AnchorVisible);
    }

    #[test]
    fn anchor_error_kind_hashes_and_compares() {
        use std::collections::HashSet;
        let mut s = HashSet::new();
        s.insert(AnchorErrorKind::TargetMissing);
        s.insert(AnchorErrorKind::AllFallbacksFailed);
        s.insert(AnchorErrorKind::TargetMissing);
        assert_eq!(s.len(), 2);
    }
}
