//! The one coherent **layout surface** for the view (spec §2.2) — sizing, flex,
//! spacing, text alignment, positioning, and scroll, designed as a single set.
//!
//! The vocabulary is **intents** ([`Justify`], [`Align`], [`fill`](Element::fill),
//! [`grow`](Element::grow), [`wrap`](Element::wrap), [`scroll_y`](Element::scroll_y),
//! …) exposed as thin `Element` builder methods that write [`LayoutProps`]. No raw
//! `Sizing`/`Percent`/`Length`/`Inset` leaks into the view vocabulary — the
//! reconciler ([`crate::reconcile`]) owns the lowering into the decomposed
//! `buiy_core::layout` components, every write a `set_if_neq`/`!=`-guarded drift.
//!
//! `LayoutProps` stores only neutral primitives (`Option<f32>` / `bool` / the
//! facades), so a container that sets no layout modifier lowers to the layout
//! defaults byte-for-byte (an unchanged snapshot never moves — spec §2.2 design
//! principle). Components `Node` does not `#[require]` (`FlexItem`, `ScrollOffset`,
//! the internal stick marker) are inserted on demand and toggled OFF by writing
//! the neutral value, never by `RemovedComponents`.

use crate::element::Element;
use crate::tokens::Space;

/// Main-axis distribution of a flex container's children — the complete 6-value
/// CSS `justify-content` facade (spec §2.2). Set by the `.justify_*` builders;
/// the reconciler lowers it to `FlexParams.justify_content`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Justify {
    /// Pack children at the main-axis start (`flex-start`; the layout default).
    #[default]
    Start,
    /// Center children on the main axis (`center`).
    Center,
    /// Pack children at the main-axis end (`flex-end`).
    End,
    /// First/last child at the edges, equal space between (`space-between`).
    Between,
    /// Equal space around each child (`space-around`).
    Around,
    /// Equal space between AND around each child (`space-evenly`).
    Evenly,
}

/// Cross-axis alignment of a flex container's children within their line — the
/// CSS `align-items` facade (spec §2.2). Set by the `.align_*` builders; the
/// reconciler lowers it to `FlexParams.align_items`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Align {
    /// Pack children at the cross-axis start (`flex-start`) — keeps each child at
    /// its natural cross size (the 3-pane idiom: a short card is not stretched to
    /// the tall column beside it).
    Start,
    /// Center children on the cross axis (`center`).
    Center,
    /// Pack children at the cross-axis end (`flex-end`).
    End,
    /// Stretch children to fill the cross axis (`stretch`; the layout default).
    #[default]
    Stretch,
}

/// Inline text alignment for a `text` node / `button` label — a 4-value facade
/// over the layout engine's `TextAlign` (spec §2.2). Passed to
/// [`Element::text_align`]; the reconciler lowers it onto the `Text` node.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TextAlign {
    /// Follow the line's writing direction (`start`; the default — left for LTR).
    #[default]
    Start,
    /// Center each line (`center`).
    Center,
    /// Align to the end of the line (`end`).
    End,
    /// Stretch each non-final line to the full width (`justify`).
    Justify,
}

/// Which positioning scheme a node uses (spec §2.2). Set by `.fixed()` /
/// `.absolute()` / `.relative()`; the reconciler lowers it to `Position.kind`.
///
/// `Relative` establishes a positioning context (a containing block for
/// `Absolute` descendants) without moving the node — the companion `.absolute()`
/// needs so a corner badge resolves against its own card, not the root.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub(crate) enum Positioning {
    #[default]
    Static,
    Relative,
    Absolute,
    Fixed,
}

/// Per-side optional logical-px lengths (padding / inset). `None` = "the author
/// set no value on this side" — the reconciler resolves it (to `0` for padding,
/// to `auto` for inset, with the positioning axis-default for `.fixed()`).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub(crate) struct Sides {
    pub top: Option<f32>,
    pub right: Option<f32>,
    pub bottom: Option<f32>,
    pub left: Option<f32>,
}

impl Sides {
    /// The same value on every side.
    fn all(v: f32) -> Self {
        Self {
            top: Some(v),
            right: Some(v),
            bottom: Some(v),
            left: Some(v),
        }
    }

    /// Distinct horizontal (`left`/`right`) vs. vertical (`top`/`bottom`) values.
    fn xy(h: f32, v: f32) -> Self {
        Self {
            top: Some(v),
            right: Some(h),
            bottom: Some(v),
            left: Some(h),
        }
    }
}

/// The complete layout state of an [`Element`] (spec §2.2). Every field is a
/// neutral primitive; the reconciler lowers the set into the decomposed
/// `buiy_core::layout` components. The `Default` matches the layout engine's own
/// defaults (`grow = 0`, `shrink = 1`, `Justify::Start`, `Align::Stretch`, and
/// everything else off), so a node with no layout modifier is a byte-identical
/// no-op against a freshly-`#[require]`'d `Node`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct LayoutProps {
    // --- Sizing (per-axis, logical px; `None` = unset → content-sized) --------
    pub width: Option<f32>,
    pub height: Option<f32>,
    pub min_width: Option<f32>,
    pub min_height: Option<f32>,
    pub max_width: Option<f32>,
    pub max_height: Option<f32>,
    /// Fill the containing block along the axis (100%). `.fill()` sets both.
    pub fill_width: bool,
    pub fill_height: bool,

    // --- Flex item (main-axis grow/shrink; inserted on demand) ----------------
    pub grow: f32,
    pub shrink: f32,

    // --- Flex container -------------------------------------------------------
    pub justify: Justify,
    pub align: Align,
    pub wrap: bool,

    // --- Spacing --------------------------------------------------------------
    pub padding: Sides,
    pub gap: Option<f32>,

    // --- Positioning + overlays -----------------------------------------------
    pub position: Positioning,
    pub inset: Sides,
    /// Pixel-center this element within its containing block (absolute + centered).
    pub center_self: bool,
    pub top_layer: bool,

    // --- Scroll ---------------------------------------------------------------
    pub scroll_x: bool,
    pub scroll_y: bool,
    /// The model-owned stick-to-bottom intent (only meaningful with `scroll_y`).
    pub stick: bool,

    // --- Text -----------------------------------------------------------------
    pub text_align: Option<TextAlign>,
}

impl Default for LayoutProps {
    fn default() -> Self {
        Self {
            width: None,
            height: None,
            min_width: None,
            min_height: None,
            max_width: None,
            max_height: None,
            fill_width: false,
            fill_height: false,
            // The flex-item neutral values (a node without a `FlexItem` behaves
            // exactly as `grow: 0, shrink: 1` — so the reconciler only inserts one
            // when a non-default is asked, and toggling back writes these).
            grow: 0.0,
            shrink: 1.0,
            justify: Justify::Start,
            align: Align::Stretch,
            wrap: false,
            padding: Sides::default(),
            gap: None,
            position: Positioning::Static,
            inset: Sides::default(),
            center_self: false,
            top_layer: false,
            scroll_x: false,
            scroll_y: false,
            stick: false,
            text_align: None,
        }
    }
}

/// The layout modifiers (spec §2.2). Every method mutates the element's layout state and
/// returns `Self`, so they chain like the rest of the surface. The reconciler
/// owns every lowering.
impl<Msg> Element<Msg> {
    // --- Sizing ---------------------------------------------------------------

    /// Fixed width in logical px (any node).
    pub fn width(mut self, px: f32) -> Self {
        self.layout.width = Some(px);
        self
    }

    /// Fixed height in logical px (any node).
    pub fn height(mut self, px: f32) -> Self {
        self.layout.height = Some(px);
        self
    }

    /// Minimum width in logical px — the box never resolves narrower than this.
    pub fn min_width(mut self, px: f32) -> Self {
        self.layout.min_width = Some(px);
        self
    }

    /// Minimum height in logical px.
    pub fn min_height(mut self, px: f32) -> Self {
        self.layout.min_height = Some(px);
        self
    }

    /// Maximum width in logical px — the box never resolves wider than this
    /// (an overlay panel capped to a readable measure).
    pub fn max_width(mut self, px: f32) -> Self {
        self.layout.max_width = Some(px);
        self
    }

    /// Maximum height in logical px.
    pub fn max_height(mut self, px: f32) -> Self {
        self.layout.max_height = Some(px);
        self
    }

    /// Fill the containing block on BOTH axes (width + height 100%). A per-axis
    /// `.width`/`.height` still overrides. On the `ui()` root (whose containing
    /// block is the viewport) this spans the window, so
    /// `.fill().justify_center().align_center()` centers content in the window.
    ///
    /// **Fills against the containing block**, so under flex-shrink it can
    /// overflow when siblings compete for the main axis — use [`grow`](Element::grow)
    /// (flex-grow), not `.fill()`, to take the *remaining* space among siblings.
    pub fn fill(mut self) -> Self {
        self.layout.fill_width = true;
        self.layout.fill_height = true;
        self
    }

    /// Fill the containing block's WIDTH only (100% width, natural height) — a
    /// chat pane that spans its column but sizes to content vertically.
    pub fn fill_width(mut self) -> Self {
        self.layout.fill_width = true;
        self
    }

    /// Fill the containing block's HEIGHT only (100% height, natural width).
    pub fn fill_height(mut self) -> Self {
        self.layout.fill_height = true;
        self
    }

    // --- Flex item ------------------------------------------------------------

    /// Grow to consume the parent container's main-axis free space
    /// (`flex-grow: 1`). Inserted on demand (`Node` does not `#[require]`
    /// `FlexItem`).
    pub fn grow(mut self) -> Self {
        self.layout.grow = 1.0;
        self
    }

    /// Grow with an explicit flex-grow factor (relative to grow-siblings).
    pub fn grow_by(mut self, factor: f32) -> Self {
        self.layout.grow = factor;
        self
    }

    /// Control flex-shrink. `.shrink(false)` pins a fixed-size child so a tight
    /// `.fill()`/`.grow()` flex parent cannot squeeze it below its size
    /// (`flex-shrink: 0`) — a [`raster`](crate::raster) canvas defaults to this.
    /// `.shrink(true)` restores the flex default.
    pub fn shrink(mut self, allow: bool) -> Self {
        self.layout.shrink = if allow { 1.0 } else { 0.0 };
        self
    }

    /// Shrink with an explicit flex-shrink factor.
    pub fn shrink_by(mut self, factor: f32) -> Self {
        self.layout.shrink = factor;
        self
    }

    // --- Flex container: main-axis (justify) ----------------------------------

    /// Pack children at the main-axis start (`justify-content: flex-start`).
    pub fn justify_start(mut self) -> Self {
        self.layout.justify = Justify::Start;
        self
    }

    /// Center children along the main axis (`justify-content: center`) — vertical
    /// for a `column!`, horizontal for a `row!`.
    pub fn justify_center(mut self) -> Self {
        self.layout.justify = Justify::Center;
        self
    }

    /// Pack children at the main-axis end (`justify-content: flex-end`).
    pub fn justify_end(mut self) -> Self {
        self.layout.justify = Justify::End;
        self
    }

    /// First/last child at the edges, equal space between (`space-between`) — the
    /// top-bar / footer idiom.
    pub fn justify_between(mut self) -> Self {
        self.layout.justify = Justify::Between;
        self
    }

    /// Equal space around each child (`justify-content: space-around`).
    pub fn justify_around(mut self) -> Self {
        self.layout.justify = Justify::Around;
        self
    }

    /// Equal space between and around each child (`justify-content: space-evenly`).
    pub fn justify_evenly(mut self) -> Self {
        self.layout.justify = Justify::Evenly;
        self
    }

    // --- Flex container: cross-axis (align) + wrap ----------------------------

    /// Pack children at the cross-axis start (`align-items: flex-start`).
    pub fn align_start(mut self) -> Self {
        self.layout.align = Align::Start;
        self
    }

    /// Center children on the cross axis (`align-items: center`).
    pub fn align_center(mut self) -> Self {
        self.layout.align = Align::Center;
        self
    }

    /// Pack children at the cross-axis end (`align-items: flex-end`).
    pub fn align_end(mut self) -> Self {
        self.layout.align = Align::End;
        self
    }

    /// Stretch children to fill the cross axis (`align-items: stretch`; the
    /// default — call it to restore stretch after another alignment).
    pub fn align_stretch(mut self) -> Self {
        self.layout.align = Align::Stretch;
        self
    }

    /// Let children wrap onto multiple lines when they overflow the main axis
    /// (`flex-wrap: wrap`) — a swatch toolbar that flows to a second row instead
    /// of overflowing its container.
    pub fn wrap(mut self) -> Self {
        self.layout.wrap = true;
        self
    }

    // --- Spacing --------------------------------------------------------------

    /// Uniform inner padding on every side (containers only).
    pub fn padding(mut self, s: Space) -> Self {
        self.layout.padding = Sides::all(s.px());
        self
    }

    /// Distinct horizontal (`left`/`right`) and vertical (`top`/`bottom`) padding.
    pub fn padding_xy(mut self, horizontal: Space, vertical: Space) -> Self {
        self.layout.padding = Sides::xy(horizontal.px(), vertical.px());
        self
    }

    /// Inner padding on the top edge only (composes with the other per-side
    /// setters; an unset side stays `0`).
    pub fn padding_top(mut self, s: Space) -> Self {
        self.layout.padding.top = Some(s.px());
        self
    }

    /// Inner padding on the right edge only.
    pub fn padding_right(mut self, s: Space) -> Self {
        self.layout.padding.right = Some(s.px());
        self
    }

    /// Inner padding on the bottom edge only.
    pub fn padding_bottom(mut self, s: Space) -> Self {
        self.layout.padding.bottom = Some(s.px());
        self
    }

    /// Inner padding on the left edge only.
    pub fn padding_left(mut self, s: Space) -> Self {
        self.layout.padding.left = Some(s.px());
        self
    }

    /// Gap between children (containers only).
    pub fn gap(mut self, s: Space) -> Self {
        self.layout.gap = Some(s.px());
        self
    }

    // --- Text -----------------------------------------------------------------

    /// Inline alignment for a `text` node / `button` label ([`TextAlign`]).
    pub fn text_align(mut self, align: TextAlign) -> Self {
        self.layout.text_align = Some(align);
        self
    }

    // --- Positioning + overlays -----------------------------------------------

    /// Escape to the **top layer** so this subtree paints OVER all in-flow
    /// content (`Stacking.top_layer = Popover`) — an overlay / dialog / scrim.
    pub fn top_layer(mut self) -> Self {
        self.layout.top_layer = true;
        self
    }

    /// Take this box out of normal flow, anchored to the **viewport**
    /// (`Position::Fixed`), so an overlay covers the window without pushing the
    /// in-flow layout. With no explicit inset it pins to the viewport origin
    /// `(0,0)` regardless of the root's padding; pair with `.fill()` for a
    /// full-viewport scrim or `.inset_*` to offset from an edge.
    pub fn fixed(mut self) -> Self {
        self.layout.position = Positioning::Fixed;
        self
    }

    /// Take this box out of normal flow, anchored to its nearest **positioned**
    /// ancestor (`Position::Absolute`) — place it with `.inset_*`. Give the
    /// ancestor `.relative()` so the box resolves against it (a per-seat corner
    /// badge on its card), else it resolves against the viewport.
    pub fn absolute(mut self) -> Self {
        self.layout.position = Positioning::Absolute;
        self
    }

    /// Establish a positioning context (`Position::Relative`) WITHOUT moving the
    /// node — the containing block an `.absolute()` descendant resolves against
    /// (the companion `.absolute()` needs for a card-local corner badge).
    pub fn relative(mut self) -> Self {
        self.layout.position = Positioning::Relative;
        self
    }

    /// Offset all four edges of an absolutely/fixed-positioned box, logical px.
    pub fn inset(mut self, top: f32, right: f32, bottom: f32, left: f32) -> Self {
        self.layout.inset = Sides {
            top: Some(top),
            right: Some(right),
            bottom: Some(bottom),
            left: Some(left),
        };
        self
    }

    /// Offset from the top edge of the containing block (logical px).
    pub fn inset_top(mut self, px: f32) -> Self {
        self.layout.inset.top = Some(px);
        self
    }

    /// Offset from the right edge of the containing block (logical px).
    pub fn inset_right(mut self, px: f32) -> Self {
        self.layout.inset.right = Some(px);
        self
    }

    /// Offset from the bottom edge of the containing block (logical px).
    pub fn inset_bottom(mut self, px: f32) -> Self {
        self.layout.inset.bottom = Some(px);
        self
    }

    /// Offset from the left edge of the containing block (logical px).
    pub fn inset_left(mut self, px: f32) -> Self {
        self.layout.inset.left = Some(px);
        self
    }

    /// Pixel-center THIS element within its containing block (absolute + inset
    /// 50% + a half-size margin). **Distinct** from centering a container's
    /// *children* (which is `.justify_center().align_center()`). Set an explicit
    /// `.width()`/`.height()` for exact centering — the classic centered modal
    /// panel. Implies `.absolute()` unless already `.fixed()` (a viewport-centered
    /// modal is `.fixed().center_self()`).
    pub fn center_self(mut self) -> Self {
        self.layout.center_self = true;
        if self.layout.position == Positioning::Static {
            self.layout.position = Positioning::Absolute;
        }
        self
    }

    // --- Scroll ---------------------------------------------------------------

    /// Make this container scroll VERTICALLY when its content overflows
    /// (`overflow-y: scroll`) — the chat / scoreboard column.
    pub fn scroll_y(mut self) -> Self {
        self.layout.scroll_y = true;
        self
    }

    /// Make this container scroll HORIZONTALLY when its content overflows
    /// (`overflow-x: scroll`) — the mobile scoreboard strip.
    pub fn scroll_x(mut self) -> Self {
        self.layout.scroll_x = true;
        self
    }

    /// **Controlled stick-to-bottom** for a `scroll_y` container: while this
    /// intent is set (a `bool` the MODEL owns and the view re-derives), the
    /// reconciler keeps the scroll pinned to the bottom as content is appended —
    /// so a new chat line stays visible — WITHOUT yanking a user who scrolled up
    /// (the app clears the model intent on scroll-away). Implies `.scroll_y()`.
    ///
    /// The pin is asserted by a post-layout system (`ScrollOffset = max` only
    /// while the intent is set), because the new content's scroll extent is known
    /// only after layout runs.
    pub fn stick_to_bottom(mut self) -> Self {
        self.layout.stick = true;
        self.layout.scroll_y = true;
        self
    }
}
