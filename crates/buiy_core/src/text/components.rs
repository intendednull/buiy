//! Per-text-entity components (T2): the authored content + style surface
//! (font-assets §§ 6, 8; measure-and-layout § 4.1) and — added later in this
//! phase — the retained `TextBuffer` state and the `ComputedTextLayout`
//! output type (architecture § 3).

use std::sync::atomic::{AtomicBool, Ordering};

use bevy::prelude::*;
use bevy::reflect::impl_reflect_opaque;
use cosmic_text::{Align, Buffer, Cursor, Metrics, Shaping, Wrap};

use super::whitespace::CollapseMode;
use crate::render::color::ColorToken;

/// The authored UTF-8 text content (measure-and-layout § 4.1) — the string
/// `TextSync` feeds to `Buffer::set_text`, after the § 5.2 white-space
/// collapse pre-pass and the § 5.4 direction strong-mark prepend
/// ([`TextDirection`]).
///
/// Changing it is the canonical reshape trigger (architecture § 5.1):
/// `TextSync` rewrites the entity's `TextBuffer` in place via the 0.19 lazy
/// setters and dirty-marks the Taffy node. Shaping happens at the next
/// lock-bearing site (T3's measure closure / `TextCommit`), never here.
#[derive(Component, Reflect, Default, Clone, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub struct Text(pub String);

/// The `font-family` stack value (font-assets § 6; foundation text.md:10, F).
/// Ordered; first match wins. v1 components carry **explicit** stacks —
/// theme token→stack indirection is the font-assets § 9 theme seam.
///
/// Lowered by the Buiy-owned resolver (T5, [`resolve_spans`]): per-codepoint
/// fontdb `Query` walk, coverage span-splitting, `unicode-range` filtering;
/// stack misses fall through to cosmic-text's `FontFallbackIter` + the
/// deterministic `BuiyFallback`.
///
/// [`resolve_spans`]: super::resolver::resolve_spans
#[derive(Reflect, Clone, PartialEq, Eq, Debug)]
pub struct FontStack(pub Vec<FamilyEntry>);

impl Default for FontStack {
    /// The CSS-initial analogue: the `sans-serif` generic
    /// (`registered_fonts_db` pins all five generics to the embedded face).
    fn default() -> Self {
        Self(vec![FamilyEntry::Generic(GenericFamily::SansSerif)])
    }
}

/// One `font-family` stack entry (font-assets § 6).
#[derive(Reflect, Clone, PartialEq, Eq, Debug)]
pub enum FamilyEntry {
    /// A concrete family name, e.g. `"Fira Sans"`.
    Named(String),
    /// A CSS generic family, resolved through fontdb's `set_*_family` pins.
    Generic(GenericFamily),
}

/// The deterministic five CSS generic families (font-assets § 6). The
/// extended set (`system-ui`, `ui-monospace`, …) is C-tier, deferred with
/// the theme seam (font-assets § 9).
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GenericFamily {
    Serif,
    SansSerif,
    Cursive,
    Fantasy,
    Monospace,
}

impl GenericFamily {
    /// Lower to the cosmic-text (fontdb) generic. All five resolve through
    /// the `registered_fonts_db` family pins, so no generic ever dangles
    /// (font-assets § 4).
    pub fn to_cosmic(self) -> cosmic_text::Family<'static> {
        match self {
            GenericFamily::Serif => cosmic_text::Family::Serif,
            GenericFamily::SansSerif => cosmic_text::Family::SansSerif,
            GenericFamily::Cursive => cosmic_text::Family::Cursive,
            GenericFamily::Fantasy => cosmic_text::Family::Fantasy,
            GenericFamily::Monospace => cosmic_text::Family::Monospace,
        }
    }
}

/// `font-family` (font-assets § 8; text.md:10, F). Unset = the
/// `TextStyleDefaults` stack.
#[derive(Component, Reflect, Default, Clone, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub struct FontFamily(pub FontStack);

/// `font-size` in logical px (font-assets § 8; text.md:12, F — cosmic-text
/// `Metrics` are unit-agnostic px; Buiy pins logical px end-to-end,
/// architecture § 6). The `small`/`medium`/`large` keyword table is named
/// in font-assets § 8, not built in T2. Unset = `TextStyleDefaults.size`.
#[derive(Component, Reflect, Clone, Copy, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct FontSize(pub f32);

impl Default for FontSize {
    /// 16 px — the CSS `medium` initial.
    fn default() -> Self {
        Self(16.0)
    }
}

/// `font-weight` (font-assets § 8; text.md:13, F) — lowered to
/// `cosmic_text::Weight(u16)`. Variable-font weight rides the committed
/// `Attrs.weight → Query.weight → get_font(id, weight)` surface end-to-end
/// (font-assets § 6); style/stretch synthesis stays C-tier.
#[derive(Component, Reflect, Clone, Copy, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub struct FontWeight(pub u16);

impl Default for FontWeight {
    /// 400 — CSS `normal`.
    fn default() -> Self {
        Self(400)
    }
}

/// Plugin-level defaults covering UNSET font components (font-assets § 8:
/// "Plugin-level defaults (`BuiyTextPlugin`'s default stack/size/weight)
/// cover unset components"). Single source of truth: constructed from the
/// component `Default` impls so the two surfaces can never diverge. Swap
/// the resource to retheme app-wide defaults; per-entity components win.
#[derive(Resource, Clone, PartialEq, Debug)]
pub struct TextStyleDefaults {
    /// Default `font-family` stack for entities without `FontFamily`.
    pub family: FontStack,
    /// Default `font-size` (logical px) for entities without `FontSize`.
    pub size: f32,
    /// Default `font-weight` for entities without `FontWeight`.
    pub weight: u16,
}

impl Default for TextStyleDefaults {
    fn default() -> Self {
        Self {
            family: FontStack::default(),
            size: FontSize::default().0,
            weight: FontWeight::default().0,
        }
    }
}

/// CSS `line-height` (measure § 5.1, F) — feeds `Metrics.line_height`, the
/// Σ term of measured height. Per-span line-height (`AttrsList`) is the
/// C-tier rich-text path, named in § 5.1's runner-up, not built.
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Debug)]
#[reflect(Component, Default)]
pub enum LineHeight {
    /// `line-height: normal` — the common UA factor 1.2
    /// (`DEFAULT_LINE_HEIGHT_SCALE`, the T2 stand-in, now the Normal arm).
    #[default]
    Normal,
    /// Unitless number — multiplier on font-size (`Metrics::relative`).
    Scale(f32),
    /// Fixed logical px (`Metrics::new`).
    Px(f32),
}

/// CSS `white-space` (measure § 5.2, F) — resolves to a
/// (collapse mode × `Wrap`) pair via the normative value table.
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub enum WhiteSpace {
    #[default]
    Normal,
    Nowrap,
    Pre,
    PreWrap,
    PreLine,
}

impl WhiteSpace {
    /// The table's collapse column (pre-pass mode, CSS Text L3 § 4.1 phase I).
    pub fn collapse_mode(self) -> CollapseMode {
        match self {
            WhiteSpace::Normal | WhiteSpace::Nowrap => CollapseMode::Collapse,
            WhiteSpace::Pre | WhiteSpace::PreWrap => CollapseMode::Preserve,
            WhiteSpace::PreLine => CollapseMode::PreserveBreaks,
        }
    }

    /// The table's `Wrap` column. `text-wrap` composes over it
    /// ([`resolve_wrap`]); the C-tier `overflow-wrap` later flips
    /// `Word` → `WordOrGlyph`/`Glyph` (measure § 5.5 — named, not built).
    pub fn base_wrap(self) -> Wrap {
        match self {
            WhiteSpace::Normal | WhiteSpace::PreWrap | WhiteSpace::PreLine => Wrap::Word,
            WhiteSpace::Nowrap | WhiteSpace::Pre => Wrap::None,
        }
    }
}

/// CSS `text-wrap` (measure § 5.2, F). `balance`/`pretty`/`stable` parse
/// and degrade to greedy `Word` wrap with a warn-once — no engine support
/// (cosmic-text and Parley both lack balancing); promotable later without
/// resharpening this seam.
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub enum TextWrap {
    #[default]
    Wrap,
    Nowrap,
    Balance,
    Pretty,
    Stable,
}

/// measure § 5.2: `text-wrap` composes where CSS says it does —
/// `nowrap` forces `Wrap::None` over the white-space table's wrap column;
/// `wrap` keeps it; the style keywords degrade to it (warn-once).
pub fn resolve_wrap(white_space: WhiteSpace, text_wrap: TextWrap) -> Wrap {
    match text_wrap {
        TextWrap::Nowrap => Wrap::None,
        TextWrap::Wrap => white_space.base_wrap(),
        TextWrap::Balance | TextWrap::Pretty | TextWrap::Stable => {
            warn_once_text_wrap_style_degrades();
            white_space.base_wrap()
        }
    }
}

/// CSS `text-align` (measure § 5.3, F) — applied at `TextCommit` (a
/// finalize concern: cosmic `Align` positions runs against the final line
/// width), never during measure. `match-parent` is deliberately NOT a
/// variant: per the § 5.3 table it is resolved at style time (the parent's
/// computed value lowered against the parent's direction) and never
/// reaches cosmic-text as a distinct value — a style-resolution-tier seam.
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub enum TextAlign {
    #[default]
    Start,
    End,
    Left,
    Right,
    Center,
    Justify,
    JustifyAll,
}

impl TextAlign {
    /// The § 5.3 value table. `Start` → `None`: cosmic-text's unaligned
    /// default follows the line's BiDi direction — exactly CSS `start`.
    /// `justify-all` degrades to `Justified` with a warn-once (last-line
    /// justification is not exposed upstream; promotable without
    /// reshaping this seam).
    pub fn to_cosmic(self) -> Option<Align> {
        match self {
            TextAlign::Start => None,
            TextAlign::End => Some(Align::End),
            TextAlign::Left => Some(Align::Left),
            TextAlign::Right => Some(Align::Right),
            TextAlign::Center => Some(Align::Center),
            TextAlign::Justify => Some(Align::Justified),
            TextAlign::JustifyAll => {
                warn_once_justify_all_degrades();
                Some(Align::Justified)
            }
        }
    }
}

/// CSS `dir` analogue (measure § 5.4, F). Lowered ENTIRELY in the TextSync
/// pre-pass as a strong direction mark prepended per non-empty buffer line
/// AFTER the § 5.2 collapse: UAX #9 P2 finds the mark as the line's first
/// strong character and forces the paragraph level — base direction then
/// drives reordering, the unaligned `start` default, `Align::End`, and
/// `ComputedTextLine.rtl`. Absent component = `Auto` (cosmic's
/// first-strong default IS `dir=auto`). Inline span direction (`<bdi>`,
/// isolates) is the rich-text seam — an isolate wrap can never set P2
/// (the § 5.4 rejected runner-up).
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub enum TextDirection {
    Ltr,
    Rtl,
    #[default]
    Auto,
}

bitflags::bitflags! {
    /// CSS `text-decoration-line` value set (decoration-and-paint § 2.2;
    /// text.md:51, F). A bitflag set: any combination of the three lines.
    /// The plural component name (`TextDecorations`) deliberately avoids
    /// colliding with `cosmic_text::TextDecoration`, which the sync
    /// lowering binds.
    #[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub struct DecorationLines: u8 {
        /// Painted UNDER the text (quad tier, § 4.2).
        const UNDERLINE    = 1 << 0;
        /// Painted UNDER the text (quad tier, § 4.2).
        const OVERLINE     = 1 << 1;
        /// Painted OVER the text (solid-stamp glyph tier, § 4.2) — the CSS
        /// Text Decoration L3 painting-order requirement.
        const LINE_THROUGH = 1 << 2;
    }
}

// `bitflags!` doesn't compose with `#[derive(Reflect)]` — register the
// opaque type manually (the layout ContainFlags precedent).
impl_reflect_opaque!((in crate::text::components) DecorationLines(Default, PartialEq));

/// `text-decoration-style`, the § 9 strategy enum at the quad-emission seam
/// — realized with its working arms (decoration-and-paint § 9; T6 plan
/// decision 2): `Solid`/`Double` lower to cosmic's
/// `UnderlineStyle::Single`/`Double`; `Dotted`/`Dashed`/`Wavy` parse and
/// DEGRADE to `Solid` with a warn-once (the `TextWrap::Balance` precedent).
/// Dotted/dashed are future Buiy emission patterns (segmented quads); the
/// wavy unblock path of record is upstream-PR-first (the literal
/// `// TODO: Wavy` in 0.19's enum) — never bake the fallback into F-tier
/// types. Upstream carries `Double` for the underline only, so overline and
/// line-through stay single-line under `Double` (the upstream asymmetry,
/// documented not hidden).
#[derive(Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DecorationLineStyle {
    #[default]
    Solid,
    Double,
    Dotted,
    Dashed,
    Wavy,
}

impl DecorationLineStyle {
    /// The cosmic `UnderlineStyle` this style lowers to (the sync mapping).
    pub fn to_cosmic_underline(self) -> cosmic_text::UnderlineStyle {
        match self {
            DecorationLineStyle::Solid => cosmic_text::UnderlineStyle::Single,
            DecorationLineStyle::Double => cosmic_text::UnderlineStyle::Double,
            DecorationLineStyle::Dotted
            | DecorationLineStyle::Dashed
            | DecorationLineStyle::Wavy => {
                warn_once_decoration_style_degrades();
                cosmic_text::UnderlineStyle::Single
            }
        }
    }
}

/// CSS `text-decoration` (decoration-and-paint § 2.2; text.md:51, F): which
/// lines to draw, their style, and the optional `text-decoration-color`
/// override. `color: None` = `currentColor` — the § 3.2 precedence, resolved
/// AT EXTRACT against the live theme (decision 1: line bits ride
/// `Attrs.text_decoration`; the color token never does — a theme swap
/// re-emits instances, never reshapes).
///
/// The `style` field supersedes the spec's two-field § 2.2 pin (T6 erratum
/// 2): § 3.2 specifies Double's paint math and the campaign demands its
/// golden, so the knob ships here rather than behind a C-tier follow-up.
///
/// Component REMOVAL is not a resync trigger (the T2-erratum-1 carrier
/// precedent): a removed `TextDecorations` resyncs on the next other
/// trigger.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct TextDecorations {
    /// Which decoration lines to draw (any combination).
    pub line: DecorationLines,
    /// Line style; `Solid` default. See [`DecorationLineStyle`].
    pub style: DecorationLineStyle,
    /// `text-decoration-color`; `None` = `currentColor` (§ 3.2 tier 3 via
    /// the span/entity fallbacks).
    pub color: Option<ColorToken>,
}

/// The caret's paint-input state (decoration-and-paint § 6.3 — the pinned
/// shape, verbatim): whether the caret currently paints, and its rect in
/// CONTENT-BOX-LOCAL logical px (`(caret_x, line_top)` → `(caret_x +
/// caret_w, line_top + line_height)`, § 6.1's terms). The producer folds
/// the entity origin and applies the § 3.3 physical-px snap to (x, width)
/// at emission — the rect here is unsnapped, scale-agnostic geometry.
///
/// WRITERS: `rect` is authored by the editing model (the successor
/// `buiy-text-editing` campaign; tests/examples until then — T7 paints
/// FROM state, it does not own editing); `visible` is managed by the
/// `write_caret_blink` render-prep writer (T7.2, edge-only — § 6.3).
/// Presence = "an editor wants a caret here"; REMOVAL hides it (focus
/// loss). Machinery state — not reflect-registered (the
/// ComputedTextLayout convention).
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct CaretVisual {
    /// Blink-phase visibility (square wave of the app clock; steady true
    /// under prefers-reduced-motion). The producer emits no stamp when
    /// false.
    pub visible: bool,
    /// Content-box-local caret rect, logical px, unsnapped.
    pub rect: bevy::math::Rect,
}

impl Default for CaretVisual {
    /// Visible (matches the t=0 blink phase and editing § 10's
    /// "caret becomes visible" on focus gain), zero rect.
    fn default() -> Self {
        Self {
            visible: true,
            rect: bevy::math::Rect::default(),
        }
    }
}

/// The selection's paint-input state (decoration-and-paint § 5.1: "the
/// endpoints reach the producer through the render-prep-written
/// `SelectionVisual` state"): the NORMALIZED endpoint pair — the
/// `Editor::selection_bounds() -> Option<(Cursor, Cursor)>` output shape
/// verbatim. The producer derives the § 5.1 rects per run via
/// `LayoutRun::highlight` and the § 5.2 re-tint per glyph from these same
/// endpoints (one source of truth; § 6.3's "rect list plus re-tint
/// ranges" phrasing is a T7 erratum — see the campaign plan).
///
/// Presence = "a selection exists"; REMOVAL clears it. A collapsed pair
/// (`start == end`) paints nothing (a collapsed selection is a caret).
/// v1 is single-range; the multi-range generalization is additive with
/// the editing campaign's `TextSelection` (editing-and-ime § 4.2).
/// Machinery state — not reflect-registered (carries `cosmic_text::Cursor`,
/// which is legal here: this module IS the cosmic boundary, the
/// `TextBuffer` precedent).
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct SelectionVisual {
    /// Logically-first endpoint (`start ≤ end` — the constructor enforces).
    pub start: Cursor,
    /// Logically-last endpoint.
    pub end: Cursor,
}

impl SelectionVisual {
    /// Build from an UNORDERED endpoint pair, normalizing to
    /// `start ≤ end` ((line, index) lexicographic — the
    /// `selection_bounds()` ordering).
    pub fn new(a: Cursor, b: Cursor) -> Self {
        if (b.line, b.index) < (a.line, a.index) {
            Self { start: b, end: a }
        } else {
            Self { start: a, end: b }
        }
    }

    /// `start == end` (position-wise) — paints nothing.
    pub fn is_collapsed(&self) -> bool {
        (self.start.line, self.start.index) == (self.end.line, self.end.index)
    }
}

static WARNED_TEXT_WRAP_STYLE: AtomicBool = AtomicBool::new(false);
static WARNED_JUSTIFY_ALL: AtomicBool = AtomicBool::new(false);
static WARNED_DECORATION_STYLE: AtomicBool = AtomicBool::new(false);

/// The translate.rs `warn_once_fr_outside_grid` precedent.
fn warn_once_text_wrap_style_degrades() {
    if !WARNED_TEXT_WRAP_STYLE.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: text-wrap balance/pretty/stable have no engine support; \
             degrading to greedy word wrap (warned once)"
        );
    }
}

fn warn_once_justify_all_degrades() {
    if !WARNED_JUSTIFY_ALL.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: text-align: justify-all degrades to justify — last-line \
             justification is not exposed by cosmic-text (warned once)"
        );
    }
}

fn warn_once_decoration_style_degrades() {
    if !WARNED_DECORATION_STYLE.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: text-decoration-style dotted/dashed/wavy are not built; \
             degrading to solid (decoration-and-paint § 9 — wavy's unblock \
             path is upstream-PR-first; warned once)"
        );
    }
}

/// The one shaping mode Buiy ever passes to `set_text` (architecture § 3.2):
/// `Shaping::Basic` breaks complex scripts for a micro-optimization and is
/// never exposed. The unit test below is the drift tripwire.
pub const TEXT_SHAPING: Shaping = Shaping::Advanced;

/// Main-world retained per-text-entity state (architecture § 3.1; the field
/// shape — buffer plus cached intrinsics — is owned by measure-and-layout
/// § 2.3): the cosmic-text `Buffer`, mutated IN PLACE. Rebuilding the buffer
/// would discard the per-`BufferLine` shape/layout caches — the
/// typing-latency win the retained component exists for.
///
/// **Change-detection contract (measure-and-layout § 7):** every in-place
/// mutation — `TextSync` here, the measure closure and `TextCommit` in T3 —
/// goes through `Mut::bypass_change_detection`. `Changed<TextBuffer>` is
/// reserved for NOTHING: author intent rides `Changed<Text>` + the
/// text-style carriers; downstream damage keys on the commit outputs
/// (`ComputedTextLayout`), never on this component's ticks. The only tick
/// ever observed is the insertion tick (the `Added<TextBuffer>` trigger
/// edge architecture § 5.1 consumes).
///
/// Despawn cleanup is free (plain component); `Text`-removal cleanup is
/// `text_sync_buffers`' removed-stream arm. Editable entities will own
/// their authoritative buffer inside `TextEditState` (the successor
/// `buiy-text-editing` campaign); the one shared accessor over both — the
/// `TextBufferAccess` QueryData pinned by measure-and-layout § 2.3 — is
/// DEFERRED to that campaign (T3 erratum, superseding T2's "built in T3"
/// seam row): its `edit` arm binds `TextEditState`, which cannot exist
/// before the editor lands, and a one-arm wrapper today is dead
/// abstraction. The measure closure and `TextCommit` bind
/// `&mut TextBuffer` directly; the swap is mechanical when the editor
/// lands.
#[derive(Component)]
pub struct TextBuffer {
    /// The retained buffer. Logical px end-to-end (architecture § 6) —
    /// physical-px rasterization happens at emission (T4), never here.
    pub buffer: Buffer,
    /// Cached intrinsic widths, keyed by content version (measure § 3.2):
    /// `TextSync` invalidates on every content change; the T3 measure
    /// closure computes and re-caches.
    intrinsics: Option<IntrinsicWidths>,
}

impl TextBuffer {
    /// A new, empty, unshaped buffer. `Buffer::new_empty` takes no
    /// `FontSystem` — the lock-free TextSync contract (architecture § 1.2:
    /// exactly three lock sites, TextSync is none of them; `Buffer::new`
    /// takes `&mut FontSystem` and would be a forbidden fourth).
    pub fn new(metrics: Metrics) -> Self {
        Self {
            buffer: Buffer::new_empty(metrics),
            intrinsics: None,
        }
    }

    /// The cached intrinsic min-/max-content widths, if valid for the
    /// current content version. `None` until the T3 measure closure
    /// computes them, and after every `TextSync` invalidation.
    pub fn intrinsics(&self) -> Option<IntrinsicWidths> {
        self.intrinsics
    }

    /// Drop the cached intrinsics: every content change (text / attrs /
    /// font / metrics / wrap) invalidates them (measure § 3.2).
    pub(crate) fn invalidate_intrinsics(&mut self) {
        self.intrinsics = None;
    }

    /// Fill the cache (the T3 measure closure is the only writer).
    pub(crate) fn cache_intrinsics(&mut self, widths: IntrinsicWidths) {
        self.intrinsics = Some(widths);
    }
}

/// Cached intrinsic widths (measure-and-layout §§ 2.3, 3.2), logical px.
/// Computed by the T3 measure closure: min-content = longest-word width
/// under `Wrap::Word` (`set_size(Some(0.0), None)`); max-content =
/// unwrapped width (`set_size(None, None)`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct IntrinsicWidths {
    /// Longest-word width under `Wrap::Word`.
    pub min_content: f32,
    /// Unwrapped single-line width.
    pub max_content: f32,
}

/// The settled line geometry `TextCommit` (T3) writes after final-width
/// shaping (architecture § 3.3) — read by caret math, picking, a11y bounds,
/// and the extract damage probes (damage keys on THIS component, never on
/// `TextBuffer` ticks — measure § 6).
///
/// **Write contract (enforced by the T3 writer; its idempotency test lands
/// with it):** idempotent-insert — bump the change tick only when the value
/// actually changed, copying `write_resolved_layout`'s guard
/// (layout/systems.rs ~:2657–2691). An unconditional re-insert keeps
/// `Changed<ComputedTextLayout>` perpetually true and cascades a full
/// extract rebuild every frame. The `PartialEq` derive IS that guard's
/// comparison. Logical px (architecture § 6). Computed output — not
/// reflect-registered (the render components.rs convention).
#[derive(Component, Clone, PartialEq, Debug, Default)]
pub struct ComputedTextLayout {
    /// One entry per laid-out line, visual top-to-bottom order.
    pub lines: Vec<ComputedTextLine>,
    /// Laid-out extent: (max line width, Σ line heights).
    pub size: Vec2,
    /// Content-box top-left offset from the entity's border-box top-left
    /// (border + padding, logical px). The glyph producer's § 5.1 content
    /// origin term: run/glyph coordinates are content-box relative, while
    /// `GlobalTransform` lands on the border box (T4 decision 2 — the
    /// spec's unpinned "content origin" source, pinned here).
    pub content_offset: Vec2,
}

/// Baseline offsets from the node's content-box top, from the first/last
/// `LayoutRun.line_y` ("Y offset to baseline of line", verified 0.19) —
/// measure § 6. Written by `TextCommit` idempotently; REMOVED when no
/// laid-out run carries glyphs (empty text has no baseline — consumers
/// branch on presence; cosmic synthesizes a glyph-less run for empty
/// lines, so presence keys on glyphs, not runs — the decision-15
/// lowering, see `commit::computed_outputs`). Future consumers
/// (vertical-align, inline baseline
/// alignment, AccessKit text geometry) are C-tier seams named in measure
/// § 5.5 — none are built in T3. Computed output — not reflect-registered.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct ResolvedBaseline {
    /// First line's baseline offset.
    pub first: f32,
    /// Last line's baseline offset (== `first` for single-line text).
    pub last: f32,
}

/// One laid-out line — the verified 0.19 `LayoutRun` per-line fields.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ComputedTextLine {
    /// Y offset of the line's baseline from the content-box top
    /// (`LayoutRun::line_y`, "Y offset to baseline of line") — the
    /// `ResolvedBaseline` (T3) source.
    pub line_y: f32,
    /// Y offset of the line's top (`LayoutRun::line_top`).
    pub line_top: f32,
    /// The line's height (`LayoutRun::line_height`).
    pub line_height: f32,
    /// The line's laid-out width (`LayoutRun::line_w`).
    pub line_w: f32,
    /// Whether the line's base direction resolved right-to-left
    /// (`LayoutRun::rtl`) — the flag the editing campaign's caret model
    /// consumes (measure § 5.4).
    pub rtl: bool,
}

#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_text::{Metrics, Shaping};

    /// architecture § 3.2: `Shaping::Basic` breaks complex scripts for a
    /// micro-optimization and is never exposed. Drift tripwire.
    #[test]
    fn shaping_is_pinned_to_advanced() {
        assert_eq!(TEXT_SHAPING, Shaping::Advanced);
    }

    #[test]
    fn new_buffers_start_with_no_cached_intrinsics() {
        let buffer = TextBuffer::new(Metrics::new(16.0, 19.2));
        assert_eq!(buffer.intrinsics(), None);
    }

    #[test]
    fn invalidate_drops_cached_intrinsics() {
        let mut buffer = TextBuffer::new(Metrics::new(16.0, 19.2));
        buffer.intrinsics = Some(IntrinsicWidths {
            min_content: 10.0,
            max_content: 80.0,
        });
        buffer.invalidate_intrinsics();
        assert_eq!(buffer.intrinsics(), None);
    }
}
