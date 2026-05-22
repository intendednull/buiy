**Date:** 2026-05-22
**Status:** active
**Subject:** Taffy — known shortcomings, performance critiques, API ergonomics, long-promised features

# Taffy — critiques

Honest tone. Where there's an open issue or PR documenting a complaint, this file quotes it; where the criticism is shape-of-the-API rather than a specific bug, this file says so. The goal is to surface the actual rough edges Buiy will hit so the spec authors can decide what to layer above Taffy.

## 1. Subgrid and masonry have been "coming" for years

CSS Grid Level 2 subgrid: [issue #468](https://github.com/DioxusLabs/taffy/issues/468) opened **2023-04-24** by Nico Burns himself. Open as of 2026-05. No implementation, no experimental branch, no placeholder enum variant in `MinTrackSizingFunction` or `GridTemplateComponent`. The issue body acknowledges the design challenge: subgrid "requires access to non-direct descendants of a node," which conflicts with Taffy's strict parent-then-child traversal in `LayoutPartialTree`.

Three years between issue-open and any visible implementation work is a meaningful gap. The CSS-WG concluded subgrid in 2023; browsers shipped it (Firefox 71 (2019), Safari 16 (2022), Chrome 117 (2023)). Taffy lags.

CSS Grid Level 3 masonry: [issue #910](https://github.com/DioxusLabs/taffy/issues/910), opened **2026-01-05**, also by Burns. The issue body acknowledges *"It may well make sense to implement CSS Grid Level 2 (subgrid layout) first."* — i.e. masonry waits on subgrid, which waits on the recursive-traversal redesign. Reasonable engineering caution; an honest reading is "neither is shipping in 2026."

Buiy's `flex-and-grid.md` reserves API stubs for both ([Buiy README § 5](../../specs/2026-05-08-buiy-layout-design/README.md#5-open-questions)) and falls back to inherited templates + `warn!` until upstream lands. The bus factor is on Burns; if he disappears, neither lands.

## 2. Performance critiques

The README's own benchmarks are the honest framing: Taffy is *competitive* with Yoga, not categorically faster. Specifically:

> 100,000-node *wide* trees at depth 1: Taffy **247.42 ms** vs Yoga **135.78 ms** (Yoga ~1.8× faster).

The wide-tree case is the cache-invalidation worst case for Taffy's design. Taffy's per-node `Cache` is keyed by `(LayoutInput, LayoutOutput)`, and for tens of thousands of siblings the cache hit-rate during a flex-line pack is essentially zero — every sibling has a fresh input. Yoga's hand-tuned C++ inner loop wins on cache locality.

Open performance issues at 2026-05:

- **[#917](https://github.com/DioxusLabs/taffy/issues/917)** "Improve layout recalculation performance for small scoped changes" — opened 2026-02-18, labelled `performance`. The complaint: a single-style change on a deep leaf re-runs the whole tree because `mark_dirty` walks to root. Taffy's cache is correct but not minimal.
- **[#911](https://github.com/DioxusLabs/taffy/pull/911)** "More correct caching logic" — draft PR by Burns, opened 2026-01-31, labelled `bug`, `controversial`, `performance`. The cache-fix experimental branch (`0.11.0-experimental-cache-fix.3`) is the productionized form of this; Blitz pins to it because it fixes real correctness bugs at the cost of some perf.
- **[#907](https://github.com/DioxusLabs/taffy/issues/907)** "Implement LCRS-Like Data Structure for TaffyTree" — opened 2025-12-30 by `@samwhaleIV`. Proposes a left-child-right-sibling tree representation for cache locality.
- **[#685](https://github.com/DioxusLabs/taffy/issues/685)** "Investigate performance impact of `BoxSizing::ContentBox` support" — opened 2024-07-16 by Burns. Self-flagged.
- **[#915](https://github.com/DioxusLabs/taffy/issues/915)** "Port blink_perf_tests (Chrome layout benchmarks) to Taffy" — opened 2026-02-05 by Burns. The intent: bring Chromium's microbenchmark suite in-tree so regressions are caught.

The pattern: performance is a real, actively-tracked concern. Not a solved problem.

For Buiy specifically: the typical-tree-size case (hundreds to low-thousands of nodes, deep) is well-served by Taffy. Buiy's tens-of-thousands-of-nodes performance contract ([Buiy architecture.md § 9](../../specs/2026-05-08-buiy-layout-design/architecture.md#9-performance-contract)) assumes Taffy's cache stays warm; if Buiy ever generates a wide-flat tree (e.g. flat list of 50k entities in a single flex container), the benchmark would invert and Taffy would underperform a Yoga-bound implementation.

## 3. API ergonomics critiques

### 3.1 The `Length` / `LengthPercentage` / `LengthPercentageAuto` / `Dimension` split

Taffy splits its size-value types by what's allowed:

- `Length(f32)` — a raw pixel value.
- `LengthPercentage` — Length or Percent.
- `LengthPercentageAuto` — Length, Percent, or Auto.
- `Dimension` — LengthPercentageAuto + (eventually) `MinContent` / `MaxContent` / `FitContent`.

Different fields take different types. `padding` is `Rect<LengthPercentage>` (no `Auto`). `margin` is `Rect<LengthPercentageAuto>`. `size.width` is `Dimension`. `gap` is `Size<LengthPercentage>`.

This is **CSS-correct** (CSS itself splits which values are allowed where), but it's a known ergonomics complaint. The pattern hits embedders building their own type system — you can't have one "size" type that works everywhere; you have to choose. Buiy's response is the `Val` enum on its own components ([Buiy box-model.md](../../specs/2026-05-08-buiy-layout-design/box-model.md)), translated to the right Taffy type per field.

### 3.2 Const-construction discipline (post-0.8)

Since 0.8, `Dimension` / `LengthPercentage` etc. are tagged-pointer `CompactLength`. They're not enums anymore — they're an opaque pointer-sized type. Construction goes through helpers:

```rust
Style {
    size: Size { width: length(100.0), height: percent(0.5) },
    padding: Rect { left: length(8.0), right: length(8.0), top: length(0.0), bottom: length(0.0) },
    ..
}
```

The `length()`, `percent()`, `auto()`, `fr()` helpers are top-level functions exported from `taffy::prelude::*` (and re-exported as `Style::length` etc.). They're const-constructible. The complaint, raised in [#824](https://github.com/DioxusLabs/taffy/issues/824) (Taffy 0.8 — Allow retrieving underlying size from tagged pointer): the tagged-pointer representation makes inspection harder. You can't match on a `Dimension` because it's not an enum; you have to call accessor methods.

This is the price of `calc()` support. The `calc()` value carries an opaque `*const ()` to an embedder-allocated calc expression; making `Dimension` non-pointer-sized would have required either dropping calc or padding every other variant. Burns chose the pointer. The decision is documented in [PR #715](https://github.com/DioxusLabs/taffy/pull/715) and the 0.8 release notes.

### 3.3 The `Style` struct is opt-in-everywhere

`Style` is large (40+ fields) and `Default`-able. Every field has a sensible default; embedders set only what they care about. This is friendly for the common path (`Style { display: Display::Flex, ..default() }`) but unfriendly when the desired behavior depends on what gets inherited.

Specifically: Taffy doesn't model inheritance. There is no "the `direction` of my parent." If an embedder needs CSS-style inheritance, it has to walk its own tree and propagate the resolved value into each child's `Style` before passing to Taffy. Buiy handles this in `WritingModeResolved` and `ContainingBlock` private cache components ([Buiy architecture.md § 1.2](../../specs/2026-05-08-buiy-layout-design/architecture.md#change-detection-trigger-set)).

The complaint isn't that this is wrong — it's that Taffy is "layout for one node at a time," and embedders re-learn that contract for every property that has inheritance in CSS.

### 3.4 Measure function API churn

The `MeasureFunc` callback signature changed in 0.4 and again in 0.5, both breaking. The 0.5 signature added a `style: &Style` parameter; the 0.6 changes traitified `Style`. Downstream embedders ate three breakages in a year. The current signature (`compute_child_layout` returns a `LayoutOutput`) is stable since 0.7, but the churn is visible in any embedder code that's been around since 0.3.

## 4. Long-promised missing features

Beyond subgrid + masonry, the persistent gaps:

- **Inline layout** ([#308](https://github.com/DioxusLabs/taffy/issues/308) "Support Morphorm/Subform Layout") — opened 2022-12-30, labelled `controversial`. Taffy has never modeled inline formatting context. No `Display::Inline`, no line-box machinery. Embedders that need real inline (Servo, Blitz) layer their own line-box code above Taffy; embedders that don't (Bevy, Buiy, Slint, Zed, Lapce) treat text as opaque leaves. The fact that this issue is `controversial` reflects deeper disagreement about whether Taffy should grow inline at all.
- **`shape-outside`** — never tracked. Floats are rectangular only.
- **Replaced-element layout** — partial via `Style.item_is_replaced`, but [#679](https://github.com/DioxusLabs/taffy/issues/679) "Support 'replaced' layout" is open and unresolved.
- **`aspect-ratio` in flex layouts** — [#804](https://github.com/DioxusLabs/taffy/issues/804) reports it's broken. Open. Affects Bevy and Buiy.
- **NaN handling** — [#231](https://github.com/DioxusLabs/taffy/issues/231) "Taffy silently returns buggy layouts when NaN values are passed as input styles" — open since 2022-09. Defensive validation is the embedder's job.
- **`f64` values** — [#332](https://github.com/DioxusLabs/taffy/issues/332) requests `f64` for high-DPI / web-scale precision. Open since 2023-01. Taffy is `f32` throughout.
- **Sending styles between threads** — [#823](https://github.com/DioxusLabs/taffy/issues/823) "Taffy 0.8 — taffy values cannot be sent between threads" — open. The `!Send` posture is intentional ([architecture.md § 8](architecture.md#8-concurrency)) but the bug-report framing reflects user surprise.

## 5. Documentation critiques

The crate's docs.rs surface is comprehensive on the type level (every public item has a doc-comment) but light on *narrative*. The high-level "how do I integrate this" story lives in the README; the trait surface for embedders ([architecture.md § 2](architecture.md#2-the-trait-stack)) is documented only as Rustdoc on individual trait items. No `book` directory, no `mdbook` integration guide.

Third-party tutorials are mostly per-embedder: Bevy UI tutorials, Slint Cookbook, Floem examples. There's no canonical "writing your own Taffy embedder" walkthrough.

Specific gaps surfaced in Discussions:

- "How to hide a node in layout?" (2025-11-20) — the answer is `Display::None` or a `tree.disable(node_id)`, but it's a Q&A discovery, not a documented pattern.
- "Do `Style` properties matter for leaf node?" (2025-08-18) — answered, but the question reveals that the leaf-vs-container distinction isn't surfaced clearly in docs.

## 6. Bus factor

One. Nico Burns is the technical lead, and his absence would visibly stall the project. See [governance.md § 7](governance.md#7-bus-factor). The mitigating factor is the codebase is small (~15k LoC) and a competent CSS-spec-literate engineer could ramp; the unmitigating factor is that *finding* a competent CSS-spec-literate engineer is hard.

For a load-bearing dependency this is a real, real consideration. Buiy's exposure is the same as Bevy's, Servo's, Blitz's. The fork-if-necessary contingency is realistic.

## 7. The CSS-spec-tracking treadmill

CSS evolves. Chromium's `aspect-ratio` behavior changed in Chrome 124 ([#653](https://github.com/DioxusLabs/taffy/issues/653)). The CSS Flexbox content-alignment rules changed mid-2024 ([0.4.4 fix](https://github.com/DioxusLabs/taffy/blob/main/CHANGELOG.md#044)). Each change requires Taffy to port a spec change *after* the browser team has worked out the corner cases.

Taffy is a *follower* in this loop, not a leader. The lag is months. For embedders that don't care about pixel-perfect Chromium parity (game UIs, editors, terminals) this is fine. For embedders that do (Servo, Blitz) this is friction.

## Sources

- Issue #468 (subgrid, open since 2023-04-24): https://github.com/DioxusLabs/taffy/issues/468
- Issue #910 (masonry, open since 2026-01-05): https://github.com/DioxusLabs/taffy/issues/910
- Issue #308 (inline/morphorm, controversial): https://github.com/DioxusLabs/taffy/issues/308
- Issue #685 (BoxSizing perf): https://github.com/DioxusLabs/taffy/issues/685
- Issue #685, #911, #915, #917 (performance label): https://github.com/DioxusLabs/taffy/issues?q=label%3Aperformance
- Issue #804 (aspect-ratio in flex broken): https://github.com/DioxusLabs/taffy/issues/804
- Issue #679 (replaced layout): https://github.com/DioxusLabs/taffy/issues/679
- Issue #231 (NaN handling): https://github.com/DioxusLabs/taffy/issues/231
- Issue #332 (f64 values): https://github.com/DioxusLabs/taffy/issues/332
- Issue #823 (Send/Sync): https://github.com/DioxusLabs/taffy/issues/823
- Issue #824 (CompactLength tagged pointer access): https://github.com/DioxusLabs/taffy/issues/824
- Issue #653 (Chrome 124 grid aspect-ratio): https://github.com/DioxusLabs/taffy/issues/653
- README benchmark numbers: https://github.com/DioxusLabs/taffy/blob/main/README.md
- CHANGELOG: https://github.com/DioxusLabs/taffy/blob/main/CHANGELOG.md
- Sibling: [open-problems.md](open-problems.md), [layout-algorithms.md](layout-algorithms.md), [governance.md](governance.md)
