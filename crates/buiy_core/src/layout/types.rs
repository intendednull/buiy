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

/// CSS-style length value. Phase 1 ships only `Px` and `Percent`; other
/// variants are reserved for later phases. The variants present here cover
/// every value the Phase 1 translation layer can emit to Taffy without
/// further resolution.
#[derive(Reflect, Clone, Copy, Debug, PartialEq)]
pub enum Length {
    /// Absolute logical pixels.
    Px(f32),
    /// Percentage of the containing block dimension on the relevant axis.
    Percent(f32),
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

/// Position kind. Phase 1 implements `Static`, `Relative`, `Absolute`;
/// `Fixed` and `Sticky` ship as variants but emit a one-shot `warn!` and
/// translate to `Absolute` / `Relative` respectively until Phases 7/8 land.
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
}
