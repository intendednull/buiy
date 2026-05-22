**Date:** 2026-05-22
**Status:** active
**Subject:** Taffy — feature-by-feature capability matrix and Buiy mitigation strategy

# Taffy — capabilities and gaps

What Taffy 0.10.1 can do, and what it can't do that web-platform-parity would need. This is the consult-this-when-designing summary. For algorithm depth see [layout-algorithms.md](layout-algorithms.md); for API surface see [api.md](api.md); for structural reasons see [architecture.md](architecture.md).

## 1. What Taffy can do (full list, 0.10.1)

**Layout algorithms (all feature-flagged, all default-on):**

- **Flexbox** — CSS Flexible Box Layout Level 1. Direction, wrap, justify, align, gap, basis/grow/shrink, align-self.
- **CSS Grid** — Level 1 + a Level 2 slice (named lines + named areas since 0.9). Track sizing with `auto`/`minmax`/`fr`/`fit-content`/`min-content`/`max-content`; placement by line index, span, area, named line; `repeat()` with `auto-fill`/`auto-fit`/count; dense packing (`row-dense`, `column-dense`); gap; alignment.
- **Block** — CSS Block layout. Block-flow stacking, margin-collapsing, absolute children in their BFC, legacy `text-align`.
- **Float** — `float: left/right/inline-start/inline-end`, `clear: left/right/both/inline-start/inline-end`. **New in 0.10.0**; sub-feature of `block_layout`.

**Properties / behaviors:**

- `display: block | flex | grid | none`.
- `direction: ltr | rtl` — inline-axis flow for Block/Flex/Grid (since 0.10.0).
- `position: relative | absolute`.
- `box-sizing: border-box | content-box`.
- `overflow: visible | clip | hidden | scroll` per axis, with `scrollbar_width` reservation.
- `width` / `height` / `min-*` / `max-*` as `Dimension` (`length` | `percent` | `auto`; intrinsic-sizing keywords not yet — see gap §2).
- `aspect-ratio` (since 0.5, with continued fixes through 0.10).
- `margin` / `padding` / `border` rectangles.
- `gap` / `row-gap` / `column-gap` (Flex + Grid).
- `inset` (`top`/`right`/`bottom`/`left`) for absolute / relative offsets.
- `calc()` values — opaque-pointer-with-resolver model since 0.8. Embedder owns the calc evaluator.
- `flex-direction`, `flex-wrap`, `flex-grow`, `flex-shrink`, `flex-basis`, `justify-content`, `align-items`, `align-self`, `align-content`.
- All Grid container + item properties listed in [api.md § 5](api.md#5-enums).
- Compressible replaced elements (`item_is_replaced: true`) for images/form-fields in Flex/Grid.
- Detailed grid info post-layout (track sizes, item rects) when feature `detailed_layout_info` is on.
- `text-align: auto | start | end | legacy-{center,left,right}` (Block only, since 0.6 — for `<center>` / `<div align="...">`).
- Frame-to-frame layout caching via `CacheTree` (extractable since 0.7).
- WPT-derived test suite in CI ([issue #639](https://github.com/DioxusLabs/taffy/issues/639) closed).
- `FromStr` parsing of all leaf style value types (`feature = "parse"`, since 0.10).
- Debug printing via `print_tree` / `write_tree` (since 0.9.3).
- `no_std` support (`feature = "alloc"` without `std`).

## 2. What Taffy cannot do (gap matrix)

The columns: **CSS feature** · **Taffy 0.10.1 status** · **Buiy mitigation** (cited from [Buiy layout spec](../../specs/2026-05-08-buiy-layout-design/)).

| CSS feature | Taffy status | Buiy mitigation |
|---|---|---|
| `display: inline` | Not implemented. No `Display::Inline` variant. | Buiy treats text leaves as opaque measure-function leaves; no real inline-formatting context. Plan in [display-and-positioning.md] — `Display::Inline` is in Buiy's enum but currently behaves like `Block`. |
| `display: inline-block` | Not implemented. | Same as above; Buiy carries the variant in its `Display` enum, behavior stub. |
| `display: inline-flex` / `inline-grid` | Not implemented. | Variants reserved in Buiy's `Display`; falls back to block-flex / block-grid. |
| `display: table` family | Not implemented. `item_is_table: bool` is a sizing hint only. | Buiy implements table layout as **post-Taffy sub-pass 6b** ([architecture.md § 3](../../specs/2026-05-08-buiy-layout-design/architecture.md#3-system-pipeline)). Children of `Display::Table*` are positioned by Buiy after Taffy. |
| `display: list-item` | Not implemented. | Buiy variant reserved; renders as block with a marker pseudo-element handled at paint time. |
| `display: contents` | Not implemented. | Buiy implements at the bridge: an entity with `Display::Contents` is skipped in `style_to_taffy`, its children parented to its parent in Taffy. |
| `display: flow-root` | Not implemented. | Buiy variant reserved; floats and BFC-establishment are partially modeled via the float context. |
| `display: ruby` | Not implemented. | Buiy variant reserved; not on near-term roadmap. |
| `position: static` | Not present; `Position::Relative` with zero inset is the substitute. | Buiy's `Position::Static` translates to Taffy's `Relative` + zero inset. |
| `position: fixed` | Not implemented. | Buiy treats `Fixed` as `Absolute` against the viewport root. |
| `position: sticky` | Not implemented. | Buiy implements as **post-Taffy sub-pass 6a** (sticky offset). |
| `writing-mode: vertical-rl / vertical-lr / sideways-rl / sideways-lr` | Not implemented. [Issue #752](https://github.com/DioxusLabs/taffy/issues/752) open. | Buiy carries the full enum on `WritingMode` component; ships HorizontalTb only in v1 with `warn!` for vertical/sideways. [Open question](../../specs/2026-05-08-buiy-layout-design/README.md#5-open-questions) on whether to ship a Buiy-side rotation pass. |
| `text-orientation` | Not implemented. | Same — Buiy's `WritingMode` component carries the field, no-op in v1. |
| `unicode-bidi` | Not implemented. | Resolved by `buiy-text-rendering-design`, not by layout. |
| `subgrid` | Not implemented. [Issue #468](https://github.com/DioxusLabs/taffy/issues/468) open since 2022. | Buiy reserves `TrackSize::Subgrid`; falls back to inherited template + `warn!` per [flex-and-grid.md § 2.3](../../specs/2026-05-08-buiy-layout-design/flex-and-grid.md#23-subgrid). |
| Masonry | Not implemented. [Issue #910](https://github.com/DioxusLabs/taffy/issues/910) open. CSS-WG syntax in flux. | Buiy reserves `GridAutoFlow::Masonry`; tier-E, v1 ships nothing per [flex-and-grid.md § 2.4](../../specs/2026-05-08-buiy-layout-design/flex-and-grid.md#24-masonry). |
| Multi-column (`column-count`, `column-width`, `break-*`) | Not implemented. | Buiy implements as **post-Taffy sub-pass 6c**, tier-E with stub algorithm in v1. |
| Container queries (`@container`) | Not implemented. | Buiy implements **above Taffy** as same-frame re-layout, capped 2× Taffy invocation per frame ([architecture.md § 3.2](../../specs/2026-05-08-buiy-layout-design/architecture.md#32-container-query-re-layout)). |
| Container units (`cqw`, `cqh`, `cqi`, `cqb`, `cqmin`, `cqmax`) | Not implemented (no container-query infra). | Buiy resolves these against the nearest queried ancestor in `style_to_taffy`. |
| Anchor positioning (`anchor-name`, `position-anchor`, `position-try`) | Not implemented. | Buiy implements as **post-Taffy sub-pass 6d** ([architecture.md § 3.3](../../specs/2026-05-08-buiy-layout-design/architecture.md#33-anchor-resolution)). |
| `shape-outside` | Not implemented. | Out of scope for Buiy v1. |
| `vertical-align`, line-box layout, baseline alignment for inline | Not implemented (no inline FC). | `buiy-text-rendering-design` handles within text leaves. |
| Intrinsic sizing keywords as `Dimension` (`width: min-content`) | Not implemented. [Issue #751](https://github.com/DioxusLabs/taffy/issues/751) open. `AvailableSpace::MinContent`/`MaxContent` exist as inputs, not as author-set sizes. | Buiy's `Sizing` enum carries `MinContent` / `MaxContent` / `FitContent`; resolves via measure-function trick in v1. |
| `box-shadow`, filters, blends affecting layout | N/A — Taffy is not a renderer. | Buiy's render pipeline owns these. |
| Scroll-snap (`scroll-snap-type`, `scroll-snap-align`, `scroll-snap-stop`, snap padding/margin) | Not implemented. | Buiy's `Scroll` component owns snap; resolved post-Taffy against `ResolvedLayout` ([overflow-and-scrolling.md](../../specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md)). |
| `scrollbar-gutter` | Not implemented (`scrollbar_width` is the only scroll-related geometry hook). | Buiy's `Overflow` component carries gutter; resolved at translate-time by adjusting padding. |
| `overscroll-behavior` | Not in Taffy (a runtime-input concern, not layout). | Buiy resolves at input-event layer. |
| `transform` / `translate` / `rotate` / `scale` | Not in Taffy (paint-time, not layout-time in CSS). | Buiy's `Transform` component is consumed by the render pipeline, not layout. |
| `contain: layout/paint/size/strict` (CSS Containment Level 3) | Not implemented. | Buiy's `Containment` component carries the values; v1 uses them as hints to invalidation only. |
| `content-visibility: auto/hidden` | Not implemented. | Buiy's `Containment` carries the field; v1 uses for visibility culling, not layout skipping. |
| `will-change` | Not implemented. | Buiy uses as a hint to cache reservation; no layout effect. |
| Float `shape-outside` integration | Not implemented (floats are rectangular). | Buiy does not plan to ship `shape-outside`. |
| Tab order / focus from CSS | N/A. | Buiy derives from topological order ([architecture.md § 5](../../specs/2026-05-08-buiy-layout-design/architecture.md#5-topological-invariant)). |
| `z-index` / stacking contexts | Not in Taffy (paint-time). | Buiy's `Stacking` component is consumed by the render pipeline. |
| Top layer (modal/popover escape) | Not in Taffy. | Buiy's `Stacking` carries the marker; resolved by the render pipeline. |

## 3. The shape of the gap

Counting: of the items above, Taffy's missing surface clusters into three buckets:

1. **Algorithmically required but unimplemented** — inline FC, table, multi-column, subgrid, masonry, writing-modes, intrinsic-size-as-Dimension, container queries, anchor positioning. These are "Taffy could ship them but hasn't yet." Buiy implements them above or after Taffy.

2. **Painting / rendering concerns** — transforms, filters, shadows, z-index, top-layer, will-change. By design out of scope; Taffy is not a renderer. Buiy's render pipeline owns these.

3. **Runtime concerns** — overscroll-behavior, scroll-snap behavior, content-visibility culling, contain hints. Buiy's input/render layers own these; layout records the intent in component fields.

Bucket 1 is the load-bearing gap for Buiy's web-platform-parity ambition. Buiy's 8-step pipeline ([Buiy architecture.md § 3](../../specs/2026-05-08-buiy-layout-design/architecture.md#3-system-pipeline)) is explicitly designed so that buckets 2 and 3 stay in their own layers and bucket 1 is filled by Buiy passes that wrap Taffy without forking it.

## 4. Versioning gotchas Buiy must watch

- **Tagged-pointer style values (0.8+)** — `Style` is `!Send + !Sync` and will stay that way. `LayoutTree` must remain a `NonSendResource`. Already cemented in [Buiy architecture.md § 1.1](../../specs/2026-05-08-buiy-layout-design/architecture.md#11-layouttree--the-bridge-state).
- **0.7 `set_children` semantics change** — children automatically removed from prior parents. Buiy already calls `set_children` per the hierarchy-change path; safe.
- **0.10 default features added** (`float_layout`, `calc`) — if Buiy ever wants `default-features = false` for a stripped build, it needs to opt these back in explicitly. The current Buiy `Cargo.toml` pins `taffy = "0.10"` with default features, so this is latent.
- **`grid` feature is mandatory for Buiy** — Buiy ships `GridParams`/`GridItem`. Don't ever opt out of `grid`.
- **`detailed_layout_info` is default-on** — Buiy doesn't currently use it, but disabling reduces binary size; defer until needed.
- **MSRV 1.71** — Buiy's CI MSRV must be >= this.
- **Experimental cache-fix branches** (`0.10.1-experimental-cache-fix.1` / `0.10.2-experimental-cache-fix.2` / `0.11.0-experimental-cache-fix.3`) — do not pin to these; they exist for the maintainer to test cache-correctness fixes. Pin to `0.10.1`.

## 5. What this means for Buiy

The gap analysis cleanly justifies Buiy's [architectural pillars 2 and 3](../../specs/2026-05-08-buiy-layout-design/README.md#2-architectural-pillars-one-line-summaries):

- **Bridge model, not fork.** Every gap above is fillable as a Buiy pass that wraps Taffy. None of them require touching Taffy's internals. So the one-directional bridge holds.
- **Sub-pass composition is bounded.** Sticky / table / multi-column / anchor are exactly four sub-passes (6a–6d). Container queries are one above-Taffy re-layout. Writing-modes wait on upstream. That's the entire gap-mitigation surface; it's well-scoped.
- **Subgrid + masonry are explicit "wait on upstream" items.** Buiy reserves the API shape; the stub-and-warn convention ([flex-and-grid.md § 2.3](../../specs/2026-05-08-buiy-layout-design/flex-and-grid.md#23-subgrid), [§ 2.4](../../specs/2026-05-08-buiy-layout-design/flex-and-grid.md#24-masonry)) means a Taffy bump for those features is a "remove the warn" cutover, not a redesign.

The single biggest risk is **writing-modes** — issue #752 has been open since 2024-12-04, vertical-* and sideways-* modes are tablestakes for i18n, and the Buiy-side rotation pass option ([Buiy README § 5](../../specs/2026-05-08-buiy-layout-design/README.md#5-open-questions) open question) is non-trivial. Track this issue.

## Sources

- Cargo.toml verified default-feature set + MSRV: https://github.com/DioxusLabs/taffy/blob/main/Cargo.toml
- `src/style/mod.rs` (Display + Position enum variants verified): https://github.com/DioxusLabs/taffy/blob/main/src/style/mod.rs
- CHANGELOG (verified float in 0.10, direction in 0.10, named lines/areas in 0.9, set_children change in 0.7, CompactLength in 0.8): https://github.com/DioxusLabs/taffy/blob/main/CHANGELOG.md
- Issue #468 (Subgrid, open): https://github.com/DioxusLabs/taffy/issues/468
- Issue #639 (WPT, closed): https://github.com/DioxusLabs/taffy/issues/639
- Issue #751 (intrinsic sizing keywords, open): https://github.com/DioxusLabs/taffy/issues/751
- Issue #752 (writing-mode, open): https://github.com/DioxusLabs/taffy/issues/752
- Issue #804 (aspect-ratio in flex, open): https://github.com/DioxusLabs/taffy/issues/804
- Issue #910 (masonry, open): https://github.com/DioxusLabs/taffy/issues/910
- Buiy layout design (all referenced sub-files): [`docs/specs/2026-05-08-buiy-layout-design/`](../../specs/2026-05-08-buiy-layout-design/)
