**Date:** 2026-05-22
**Status:** active
**Subject:** Taffy — system-specific terminology used across this folder

# Taffy — glossary

Short definitions for terms used across the Taffy prior-art folder. Each entry cites the file where the full discussion lives.

## Tree and trait surface

- **`TaffyTree<NodeContext>`** — The high-level all-in-one tree type. Owns node storage (`slotmap::SlotMap`), per-node `Cache`, per-node `Style`, and parent/child topology. Construct with `TaffyTree::new()`, mutate with `new_leaf` / `new_with_children` / `set_style` / `set_children`, layout with `compute_layout(root, available_space)`. Buiy wraps a `TaffyTree<()>` in its `LayoutTree` `NonSendResource`. See [architecture.md § 1 + § 3](architecture.md), [api.md § 1](api.md).

- **`LayoutPartialTree`** — The hub trait of the low-level API. Provides `get_core_container_style(id) -> &Style`, `set_unrounded_layout(id, layout)`, and `compute_child_layout(id, inputs) -> LayoutOutput`. Taffy's algorithms call into this trait for child recursion. Servo, Blitz, GPUI, Slint implement it against their own node arenas; Bevy UI and Buiy use `TaffyTree`'s built-in impl. See [architecture.md § 2](architecture.md), [api.md § 2](api.md).

- **`TraversePartialTree`** — Required base trait. Provides `child_ids(parent) -> ChildIter`, `child_count(parent) -> usize`, `get_child_id(parent, index) -> NodeId`. Single-level topology only; Taffy's algorithms handle recursion internally. See [architecture.md § 2](architecture.md).

- **`CacheTree`** — The cache-backend trait, split out of `LayoutPartialTree` in 0.7.0. Provides `cache_get(id, key)` / `cache_store(id, key, value)` / `cache_clear(id)`. Embedders that don't implement it lose Taffy's frame-to-frame caching. See [architecture.md § 2](architecture.md).

- **`LayoutFlexboxContainer` / `LayoutGridContainer` / `LayoutBlockContainer`** — Algorithm-specific sub-traits. Each provides container-style and item-style readers for the algorithm it covers. Implementations can be limited to the algorithms the embedder cares about. See [api.md § 2](api.md).

## Style and value types

- **`Style`** — The single mega-struct describing all layout properties of one node. 40+ fields, all `Default`-able. Generic over `S: CheapCloneStr` since 0.9 (named grid lines/areas). `!Send + !Sync` since 0.8. Buiy decomposes this into ~15 typed ECS components and rebuilds the `Style` per node in `SyncStyles`. See [api.md § 3](api.md).

- **`Style::DEFAULT`** — Const-constructible default value. Pairs with the `..Style::DEFAULT` struct-update syntax to produce const static styles. See [api.md § 6](api.md).

- **`Length`** — A raw-pixel value type (`Length(f32)` semantically); `CompactLength`-backed since 0.8. See [api.md § 4](api.md), [critiques.md § 3.1](critiques.md).

- **`LengthPercentage`** — A length-or-percent value type. Used for `padding`, `gap`, `border`. No `auto` variant. See [api.md § 4](api.md).

- **`LengthPercentageAuto`** — `LengthPercentage` + `Auto`. Used for `margin` and `inset`. See [api.md § 4](api.md).

- **`Dimension`** — `LengthPercentageAuto` + (per [#751](https://github.com/DioxusLabs/taffy/issues/751)) eventual `MinContent` / `MaxContent` / `FitContent`. Used for `width`/`height`/`min-*`/`max-*`. The intrinsic-sizing variants are not yet shipped as author-set values. See [api.md § 4](api.md), [open-problems.md § 15](open-problems.md).

- **`Fr`** — Flex-fraction unit for CSS Grid track sizing. A `MaxTrackSizingFunction::Fr(f32)` variant; constructed via `fr(1.0)`. See [api.md § 4 + § 5](api.md).

- **`CompactLength`** — Tagged-pointer-sized opaque type introduced in 0.8 that backs `Dimension`, `LengthPercentage`, `LengthPercentageAuto`. Carries `Length` / `Percent` / `Auto` / `Fr` / `Calc(*const ())` / `MinContent` / `MaxContent` / `FitContent` in pointer-sized space. The reason `Style` is `!Send + !Sync`. See [architecture.md § 8](architecture.md), [critiques.md § 3.2](critiques.md).

## Layout-property enums

- **`Display`** — `Block | Flex | Grid | None`. **Four variants only.** No inline-*, no table*, no list-item, no contents, no flow-root, no ruby. Buiy carries all 13+ CSS variants on its own `Display` enum and maps to Taffy. See [api.md § 5](api.md), [open-problems.md § 14](open-problems.md).

- **`Position`** — `Relative | Absolute`. **Two variants only.** No Static, Fixed, Sticky. Buiy carries all five; Sticky is post-Taffy sub-pass 6a, Fixed is `Absolute` against the viewport. See [api.md § 5](api.md), [open-problems.md § 10](open-problems.md).

- **`Overflow`** — `Visible | Clip | Hidden | Scroll`. **No `Auto` variant.** Buiy's `Overflow::auto()` maps to Scroll + a runtime check `content_size > size`. See [api.md § 5](api.md).

- **`Direction`** — `Ltr | Rtl`. CSS `direction` property (inline-axis flow). Shipped per-container in 0.10. *Not* `writing-mode` — vertical writing modes are open in [#752](https://github.com/DioxusLabs/taffy/issues/752). See [layout-algorithms.md § 6](layout-algorithms.md).

- **`Float`** — `None | Left | Right | InlineStart | InlineEnd`. CSS `float` property, shipped in 0.10 behind `float_layout` feature. See [layout-algorithms.md § 4](layout-algorithms.md).

- **`Clear`** — `None | Left | Right | Both | InlineStart | InlineEnd`. CSS `clear` property, shipped in 0.10. See [layout-algorithms.md § 4](layout-algorithms.md).

## Grid-specific types

- **`TrackSizingFunction`** — A `(MinTrackSizingFunction, MaxTrackSizingFunction)` pair representing one CSS grid track. The `minmax(min, max)` shape is canonical; the `length()` / `percent()` / `fr()` / `auto()` / `min_content()` / `max_content()` / `fit_content()` helpers produce common cases. See [layout-algorithms.md § 2](layout-algorithms.md).

- **`GridLine`** — A line between grid tracks. Named lines are supported via `grid_template_column_names` / `grid_template_row_names` since 0.9. See [layout-algorithms.md § 2](layout-algorithms.md).

- **`GridPlacement<S>`** — How a grid item is placed onto the track grid. Variants: `Auto | Line(i16) | Span(u16) | NamedLine(S, i16) | NamedSpan(S, u16)`. Used in `grid_row` and `grid_column`. See [api.md § 4](api.md).

- **`GridAutoFlow`** — `Row | Column | RowDense | ColumnDense`. Controls how items are placed into the implicit grid. Dense packing fills empty cells with later items if they fit. See [layout-algorithms.md § 2](layout-algorithms.md).

## Algorithmic inputs

- **`AvailableSpace`** — `Definite(f32) | MinContent | MaxContent`. The available-space-on-each-axis input to `compute_layout`. `Size::MAX_CONTENT` is the typical root call when the container has no fixed size; `Definite(viewport_width)` is the typical browser-style call. Buiy reuses this enum for container-query unit resolution. See [architecture.md § 5](architecture.md), [api.md § 1](api.md).

- **`MeasureFunc`** — The closure signature `FnMut(Size<Option<f32>>, Size<AvailableSpace>, NodeId, Option<&mut NodeContext>, &Style) -> Size<f32>` that an embedder passes to `compute_layout_with_measure` to size leaves. Buiy's text and image leaves register a measure function via cosmic-text shaping and image-asset sizing. See [integration.md § 6](integration.md), [api.md § 1](api.md).

## Placeholder / non-shipped concepts

- **`Subgrid`** — CSS Grid Level 2 `grid-template-columns: subgrid;`. **Not implemented** in Taffy. No placeholder enum variant in `MinTrackSizingFunction` or `GridTemplateComponent`. Tracked in [#468](https://github.com/DioxusLabs/taffy/issues/468) since 2023-04-24. Buiy reserves a `TrackSize::Subgrid` variant in its `GridParams` shape and `warn!`s until upstream lands. See [layout-algorithms.md § 2.1](layout-algorithms.md), [open-problems.md § 1](open-problems.md).

- **`Masonry`** — CSS Grid Level 3 `grid-template-rows: masonry;`. **Not implemented**. CSS-WG is mid-debate on the syntax. Tracked in [#910](https://github.com/DioxusLabs/taffy/issues/910). Buiy reserves a `GridAutoFlow::Masonry` variant; tier-E, v1 ships nothing. See [layout-algorithms.md § 2.2](layout-algorithms.md), [open-problems.md § 2](open-problems.md).

## External terms

- **WPT** — [Web Platform Tests](https://github.com/web-platform-tests/wpt). The W3C-hosted browser-conformance suite. Taffy imports a WPT-derived subset of layout tests as fixtures (under `test_fixtures/`); the umbrella was [#639](https://github.com/DioxusLabs/taffy/issues/639) (closed). Pass-rate not advertised. Buiy can import the same fixtures via the `scripts/import-yoga-tests` and `scripts/gentest` patterns. See [ecosystem.md § 3](ecosystem.md).

- **Stretch** — The predecessor crate, by Emil Sjölander at Visly Inc. (Stockholm), first published 2018-12-29. Repo lives at [`vislyhq/stretch`](https://github.com/vislyhq/stretch); last commit 2020-05-22. Flexbox only. Bevy UI shipped on `stretch 0.3.2` from Bevy 0.5 through Bevy 0.8 carrying known bugs that couldn't be fixed without a maintained upstream. Defunct. See [history.md § 1](history.md).

- **stretch2** — Jonathan Kelley's fork of Stretch, published to crates.io 2022-03-09 to unblock the Dioxus + Bevy UI consumers. Final release `0.4.3` (2022-05-23) lists Alice Cecile as author — the maintenance bridge to Taffy. Defunct. See [history.md § 2](history.md).

- **Blitz** — Browser engine in the Dioxus org. The **actual major Dioxus-org Taffy consumer** (not Dioxus core, which is virtualdom-only). Uses Taffy via the low-level traits against `blitz-dom`'s own DOM arena (not `TaffyTree`). Pins exact-version `taffy = "=0.11.0-experimental-cache-fix.3"` for cache-correctness. Combines Stylo (CSS parsing) + Taffy (layout) + Vello (paint). Repo: https://github.com/DioxusLabs/blitz. See [integration.md § 3](integration.md), [ecosystem.md § 1](ecosystem.md).

- **Yoga** — Facebook's Flexbox-only layout engine (C++ 20, MIT, 18.7k stars). Latest 3.2.1 (2024-12-13). The "Yoga vs Taffy" comparison framing in Taffy's README — Yoga implements Flexbox only; Taffy implements Flexbox + Grid + Block + Float. On wide-flat trees Yoga is ~1.8× faster than Taffy; on deep-nested trees Taffy is ~10–30% faster. The README's honest benchmarks show Taffy is competitive, not categorically faster. See [ecosystem.md § 2](ecosystem.md), [critiques.md § 2](critiques.md).

## Sources

- All terms cross-link to their full discussion in the sibling files; each sibling has its own `## Sources` section with upstream URLs.
