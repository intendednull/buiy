**Date:** 2026-05-22
**Status:** active
**Subject:** Taffy — what each layout algorithm covers, CSS-conformance posture, and gaps

# Taffy — layout algorithms

Four algorithm modules are exposed: Flexbox, CSS Grid, Block, and Float (a sub-module of Block). Each one is feature-flagged (default-on for all four) and can be invoked either through `TaffyTree::compute_layout` (dispatch by `Display`) or as standalone `compute_*_layout` functions against the low-level traits. This file enumerates what each algorithm covers, what it doesn't, and how Taffy's CSS-conformance posture compares to the web reference implementations.

Sibling files: [architecture.md](architecture.md) for the trait + cache shape, [api.md](api.md) for the public surface, [capabilities.md](capabilities.md) for the full feature-by-feature gap matrix.

## 1. Flexbox

The original algorithm; ships since 0.1. Default `Display` value when the `flexbox` feature is enabled. Source: [`src/compute/flexbox.rs`](https://github.com/DioxusLabs/taffy/blob/main/src/compute/flexbox.rs).

**Container properties** (read off `FlexboxContainerStyle` for embedders, the `Style.flex_*` fields for the default `Style`):

- `flex_direction`: `Row` | `Column` | `RowReverse` | `ColumnReverse`.
- `flex_wrap`: `NoWrap` | `Wrap` | `WrapReverse`.
- `justify_content`: `Start` | `End` | `FlexStart` | `FlexEnd` | `Center` | `SpaceBetween` | `SpaceAround` | `SpaceEvenly` | `Stretch`.
- `align_items`: `Start` | `End` | `FlexStart` | `FlexEnd` | `Center` | `Baseline` | `Stretch`.
- `align_content`: same value space as `align_items` + `SpaceBetween` / `SpaceAround` / `SpaceEvenly`.
- `gap`: `Size<LengthPercentage>` — row gap + column gap. Implements CSS `gap` / `row-gap` / `column-gap`.

**Item properties** (off `FlexboxItemStyle`):

- `flex_grow`: `f32`.
- `flex_shrink`: `f32` (default 1.0).
- `flex_basis`: `Dimension` (default `Auto`).
- `align_self`: `Option<AlignSelf>` (falls back to parent's `align_items` if `None`).

**`order` is not present.** The CSS `order` property is unsupported; children lay out in document order. Bevy UI exposes `order` separately in its own `Style` and resolves it before handing children to Taffy. Buiy's design ([flex-and-grid.md § 1.2](../../specs/2026-05-08-buiy-layout-design/flex-and-grid.md#12-flexitem-on-flex-children)) carries `order` on the Buiy-side `FlexItem` for the same reason.

**Conformance to CSS Flexbox.** The README documents Taffy aims to implement Flexbox "faithfully" and points users at MDN as the spec reference. The CHANGELOG shows continuous spec-tracking fixes — e.g. 0.4.4's "content alignment behaviour was updated to match the latest spec (and Chrome 123+)" (commit #635). The known incompatibilities at 0.10.1:

- **Aspect-ratio in flex layouts** is partially broken — see [issue #804](https://github.com/DioxusLabs/taffy/issues/804) (open).
- **Intrinsic sizing keywords** (`min-content` / `max-content` / `fit-content` as `Dimension` values for `width` / `height`) are not yet implemented — see [issue #751](https://github.com/DioxusLabs/taffy/issues/751) (open). They exist as `AvailableSpace` algorithmic inputs but not as author-set sizes.
- Many smaller compliance issues are tracked in WPT-derived tests; the issue tracker has a long tail.

## 2. CSS Grid

Shipped in **0.3** (release issue #4). Source: [`src/compute/grid/`](https://github.com/DioxusLabs/taffy/tree/main/src/compute/grid).

**Container properties** (off `GridContainerStyle`):

- `grid_template_columns`: `Vec<GridTemplateComponent<S>>` — track sizing functions. Includes `Single(TrackSizingFunction)` and `Repeat(GridTemplateRepetition)` variants. `TrackSizingFunction` itself is a `(MinTrackSizingFunction, MaxTrackSizingFunction)` pair, so `minmax(min, max)` is the canonical form; the helpers `length(px)`, `percent(p)`, `fr(f)`, `auto()`, `min_content()`, `max_content()`, `fit_content(arg)` produce the common cases.
- `grid_template_rows`: same shape.
- `grid_template_areas`: `Vec<GridTemplateArea<S>>` — named areas, added in 0.9.
- `grid_template_column_names`, `grid_template_row_names`: `Vec<Vec<S>>` — named lines between tracks, added in 0.9.
- `grid_auto_rows`, `grid_auto_columns`: implicit-grid track sizes.
- `grid_auto_flow`: `Row` | `Column` | `RowDense` | `ColumnDense`.
- Alignment: `justify_items`, `align_items`, `justify_content`, `align_content`. All Grid-defined values supported.
- `gap`: shared with Flexbox.

**Item properties** (off `GridItemStyle`):

- `grid_row`: `Line<GridPlacement<S>>` — start + end placement. `GridPlacement` is `Auto` | `Line(i16)` | `Span(u16)` | `NamedLine(S, i16)` | `NamedSpan(S, u16)`.
- `grid_column`: same shape.
- `justify_self`, `align_self`: `Option<AlignSelf>`.

**Track sizing.** `MinTrackSizingFunction` supports `Auto`, `MinContent`, `MaxContent`, `Length(f32)`, `Percent(f32)`, and (with `calc` feature) `Calc(*const ())`. `MaxTrackSizingFunction` adds `Fr(f32)` and `FitContent`. Repeats accept `Count(u16)`, `AutoFill`, `AutoFit`.

**Dense packing.** `GridAutoFlow::RowDense` / `ColumnDense` implement CSS Grid's dense packing — empty cells get filled by later items if they fit. Implemented.

**Named areas + lines.** Both implemented; `grid_template_areas` was added in 0.9, alongside `grid_template_column_names` / `grid_template_row_names`. To use them the `Style` carries a generic `S: CheapCloneStr` parameter; the recommended choices are `Arc<str>` or `string_cache::Atom`.

**Compressible replaced elements.** A 0.8 addition: `Style.item_is_replaced = true` opts a node into the CSS "compressible replaced" sizing path (images / form fields / video). Improves correctness for replaced grid items vs the older auto-min-size pessimism.

**Detailed grid info.** Feature `detailed_layout_info` (default-on) populates `DetailedGridInfo` after layout — computed track sizes, item rectangles, expanded auto-repeats. Accessible via `TaffyTree::detailed_layout_info` (added 0.7.2; takes `&self` since 0.7.3). This is the API Bevy UI uses for grid devtools.

### 2.1 Subgrid — not shipped

CSS Grid Level 2 subgrid (`grid-template-columns: subgrid;`) is **not implemented**. Tracked in [issue #468](https://github.com/DioxusLabs/taffy/issues/468) (open since 2022). The brief stated "subgrid (experimental? in 0.10? verify)" — the actual status is: **no implementation, no experimental flag, no placeholder enum variant in `MinTrackSizingFunction` or `GridTemplateComponent`**. Buiy reserves a `TrackSize::Subgrid` variant in its component shape and falls back to inherited template + `warn!` until upstream lands ([flex-and-grid.md § 2.3](../../specs/2026-05-08-buiy-layout-design/flex-and-grid.md#23-subgrid)).

### 2.2 Masonry — not shipped

CSS Grid Level 3 masonry (`grid-template-rows: masonry;`) is **not implemented**. Tracked in [issue #910](https://github.com/DioxusLabs/taffy/issues/910) (open). The CSS-WG itself is mid-debate about masonry's syntax (`display: masonry;` vs the Grid-extension form), which is one reason Taffy is holding off. Same brief-correction: **no experimental flag**. Buiy marks masonry tier-E and ships nothing ([flex-and-grid.md § 2.4](../../specs/2026-05-08-buiy-layout-design/flex-and-grid.md#24-masonry)).

## 3. Block

Shipped in **0.4** (closes [issue #405](https://github.com/DioxusLabs/taffy/issues/405)). Source: [`src/compute/block.rs`](https://github.com/DioxusLabs/taffy/blob/main/src/compute/block.rs).

**What it covers.** Block containers with block-level children. Each child lays out top-to-bottom (or right-to-left when `direction: rtl`), stacking vertically with margins (and with margin-collapsing semantics matching CSS for adjacent sibling blocks). Padding and border apply normally. Absolute-positioned children participate in the block formatting context's containing block.

**What it does not cover.**

- **No inline layout.** Children that should be inline ("text inside a `<p>`") cannot mix with block children. There is no `Display::Inline` variant. Embedders that need text leaves in block flow do so via measure functions returning a single rect (i.e. the entire text block is one inline-equivalent leaf, not a series of line boxes).
- **No `inline-block`.** No `Display::InlineBlock`. Embedders fake it by making a `Display::Block` child and constraining its size.
- **No `display: list-item`.** No automatic marker layout.
- **No `display: table*` family.** Tables are entirely the embedder's problem; Buiy implements them as post-Taffy sub-pass 6b.
- **Legacy `text-align`** (the inline-axis alignment of block contents) is supported via `Style.text_align` since 0.6 — but only the subset needed for `<center>` and `<div align="...">`, not full inline text alignment. The variant is `TextAlign::Auto | Start | End | Legacy{Center, Left, Right}`.

## 4. Float

Shipped in **0.10.0**. Source: [`src/compute/float.rs`](https://github.com/DioxusLabs/taffy/blob/main/src/compute/float.rs) and [`src/compute/block.rs`](https://github.com/DioxusLabs/taffy/blob/main/src/compute/block.rs).

**This is a brief-correction.** The orchestrator brief said "Float layout: NOT supported (verify)." That was true through 0.9.x; it is **false at 0.10.x**.

**What it covers.**

- `Float`: `None | Left | Right | InlineStart | InlineEnd` — author intent.
- `Clear`: `None | Left | Right | Both | InlineStart | InlineEnd` — clearance request.
- A `FloatContext` is created per block formatting context (`BlockContext` owns the `FloatContext`) and shared across all the BFC's children. Floated children register themselves in the context; subsequent siblings (block and floated) consult the context to find their position.
- Feature-flagged via `float_layout` (sub-feature of `block_layout`). Default-on.

**Limitations.**

- No `shape-outside` — floats are rectangular, content wraps the bounding box only.
- No interaction with inline-layout line-box shortening — there are no inline boxes for Taffy to shorten. Floats only affect block-sibling positioning. (If an embedder wants real CSS-paragraph float-wrap, they need their own inline layout above Taffy.)
- The implementation is new (0.10, May 2026); it is not yet WPT-validated as thoroughly as Flexbox or Grid.

## 5. The `Display` enum — what's missing

For reference, the full `Display` enum is exactly:

```rust
pub enum Display {
    #[cfg(feature = "block_layout")] Block,
    #[cfg(feature = "flexbox")]      Flex,
    #[cfg(feature = "grid")]         Grid,
    None,
}
```

No inline / inline-block / inline-flex / inline-grid / table* / list-item / ruby / contents / flow-root. The brief mentioned an "inline variant story" — the story is: there isn't one. Embedders implement inline layout themselves (Servo + Blitz do; Bevy / Buiy / Slint / Dioxus don't, treating text as opaque leaves).

There is one secondary signal: `Style.item_is_table: bool` (since 0.6, [PR #701](https://github.com/DioxusLabs/taffy/pull/701)) lets a block-flow child request table-like sizing. This isn't real table layout; it's a sizing-mode hint that exists because table layout is "not implemented" (per `Style` doc comment) but block layout needed *some* way to size table-shaped children differently. Embedders that ship real table layout (Buiy plans to, via post-Taffy sub-pass 6b) wire `item_is_table = true` and override the geometry.

## 6. Writing modes — partial

`Direction` (`Ltr` / `Rtl`) is supported since **0.10** for Block, Flexbox, and Grid (closes [issue #213](https://github.com/DioxusLabs/taffy/issues/213)). This is the CSS `direction` property — it controls inline-axis flow.

`writing-mode` (`horizontal-tb` / `vertical-rl` / `vertical-lr` / `sideways-rl` / `sideways-lr`) is **not supported**. Tracked in [issue #752](https://github.com/DioxusLabs/taffy/issues/752) (open). Taffy's geometry assumes a horizontal-tb / inline-x / block-y orientation throughout; rotating the formatting context requires more than the existing `Direction` flip.

This is exactly the gap [Buiy's writing-modes spec](../../specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md) calls out: Taffy supports a subset (LTR/RTL only). Buiy's design carries `WritingMode::{HorizontalTb, VerticalRl, VerticalLr, SidewaysRl, SidewaysLr}` on its component and decides whether to ship a Buiy-side rotation pass or wait on Taffy. The open question is still open.

## 7. Container queries — not in Taffy

`@container` queries are a layout-feedback feature: a child's style depends on the *resolved* size of a nominated container ancestor. This is not implemented in Taffy and is not on the roadmap. Buiy implements container queries above Taffy as a same-frame re-layout, capped at 2× Taffy ([Buiy architecture.md § 3.2](../../specs/2026-05-08-buiy-layout-design/architecture.md#32-container-query-re-layout)). Taffy is unaware container queries exist — Buiy reads the resolved layout from `tree.layout(node_id)`, evaluates queries Buiy-side, sets/clears marker components, and re-runs `compute_layout` if any flipped.

## 8. Anchor positioning — not in Taffy

CSS Anchor Positioning Module Level 1 (`anchor-name`, `position-anchor`, `position-try`) is not implemented and not roadmapped. Buiy implements it as a post-Taffy overlay pass ([Buiy architecture.md § 3.3](../../specs/2026-05-08-buiy-layout-design/architecture.md#33-anchor-resolution)). Same shape as container queries: Taffy lays out anchored elements with their declared dimensions; Buiy overrides their `ResolvedLayout.position` afterwards.

## 9. WPT conformance

Taffy ships a WPT-derived test suite as part of CI; [issue #639](https://github.com/DioxusLabs/taffy/issues/639) (closed) was the umbrella for hooking up Web Platform Tests for layout. The exact pass-rate percentage is not advertised in the README, the docs, or the CHANGELOG; the project tracks individual WPT-derived bugs as discrete issues (e.g. `Fix resolving flexible lengths (WPT css/flexbox-multiline-min-max test)` in 0.6.0). Compared to Yoga — which the README benchmark contrasts against — the conformance posture is meaningfully wider: Yoga implements Flexbox only, no Grid, no Block, no Float, no Direction. Concretely: Taffy is the only Rust-native engine that ships Flexbox + Grid + Block + Float in one crate.

## Sources

- CHANGELOG (verified Block in 0.4, named lines/areas in 0.9, float + direction in 0.10, item_is_table in 0.6): https://github.com/DioxusLabs/taffy/blob/main/CHANGELOG.md
- `src/compute/` (algorithm modules): https://github.com/DioxusLabs/taffy/tree/main/src/compute
- `src/style/mod.rs` (Display enum, Style struct, Direction): https://github.com/DioxusLabs/taffy/blob/main/src/style/mod.rs
- `src/style/grid.rs` (GridTemplateComponent, TrackSizingFunction, GridPlacement)
- Issue #213 (Direction): https://github.com/DioxusLabs/taffy/issues/213 (closed)
- Issue #468 (Subgrid): https://github.com/DioxusLabs/taffy/issues/468 (open)
- Issue #639 (WPT): https://github.com/DioxusLabs/taffy/issues/639 (closed)
- Issue #751 (intrinsic sizing keywords): https://github.com/DioxusLabs/taffy/issues/751 (open)
- Issue #752 (writing-mode): https://github.com/DioxusLabs/taffy/issues/752 (open)
- Issue #804 (aspect-ratio in flex): https://github.com/DioxusLabs/taffy/issues/804 (open)
- Issue #910 (masonry): https://github.com/DioxusLabs/taffy/issues/910 (open)
- README benchmarks (Yoga comparison): https://github.com/DioxusLabs/taffy/blob/main/README.md
- Buiy flex-and-grid spec: [`docs/specs/2026-05-08-buiy-layout-design/flex-and-grid.md`](../../specs/2026-05-08-buiy-layout-design/flex-and-grid.md)
- Buiy writing-modes / container-queries spec: [`docs/specs/2026-05-08-buiy-layout-design/README.md`](../../specs/2026-05-08-buiy-layout-design/README.md)
