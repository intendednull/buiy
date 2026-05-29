**Date:** 2026-05-22
**Status:** active
**Subject:** Taffy — feature-by-feature catalog of what's missing, deferred, or out-of-scope; Buiy's commits to layering above

# Taffy — open problems

This file is the consult-this-when-designing reference for "what Taffy does NOT do." Sibling [layout-algorithms.md](layout-algorithms.md) covers what Taffy *does*; sibling [critiques.md](critiques.md) covers known rough edges in what it does. This file is the catalog of *gaps* — features Buiy will need to either layer above Taffy, defer, or work around.

For each gap: current Taffy status, the linked issue if any, and the Buiy posture per the layout spec.

## 1. Subgrid (CSS Grid Level 2)

**Status:** not implemented. No experimental branch. No placeholder enum variant.

**Tracker:** [#468](https://github.com/DioxusLabs/taffy/issues/468), open since 2023-04-24, author Nico Burns. Marked "Todo" on the Roadmap project.

**What blocks shipping:** the algorithm "requires access to non-direct descendants of a node" (per the issue body), which conflicts with Taffy's strict parent-to-child traversal in `LayoutPartialTree`. The redesign needed is non-trivial — likely a new trait that exposes "skip down through this child's children" in a controlled way, or a two-pass scheme where the subgrid pass runs separately. No design doc is published.

**Browser status:** Firefox 71 (2019), Safari 16 (2022), Chrome 117 (2023). Stable in every major browser. Taffy lags by 3+ years.

**Buiy posture:** Reserves a `TrackSize::Subgrid` variant in its `GridParams` component shape and falls back to inherited template + `warn!` until upstream lands. Tracked as an open question in [Buiy README § 5](../../specs/2026-05-08-buiy-layout-design/README.md#5-open-questions). Buiy does **not** commit to implementing subgrid above Taffy — the algorithm is genuinely subtle and forking it would multiply maintenance.

## 2. Masonry (CSS Grid Level 3)

**Status:** not implemented. No experimental branch.

**Tracker:** [#910](https://github.com/DioxusLabs/taffy/issues/910), open since 2026-01-05, author Nico Burns.

**Issue body acknowledges:** *"`display: grid-lanes` (previously known as 'masonry') is a new layout mode which is useful for creating layouts that are otherwise difficult to achieve [...] It may well make sense to implement CSS Grid Level 2 (subgrid layout) first."*

**Why it's stuck:** CSS-WG itself is mid-debate on the syntax (`display: masonry;` vs the Grid-extension form), and Safari/WebKit have implemented `display: grid-lanes` per the WebKit blog cited in the issue. The spec is "starting to settle" per Burns, but not settled. Taffy waits on both the spec convergence and the subgrid prerequisite.

**Buiy posture:** Tier-E (deferred, not in v1). No API stub. Buiy ships nothing. Tracked in [Buiy README § 5](../../specs/2026-05-08-buiy-layout-design/README.md#5-open-questions). If users want masonry in Buiy v1, they get a stack of flex columns instead.

## 3. Anchor positioning (CSS Anchor Positioning Module Level 1)

**Status:** not implemented. Not on the active roadmap.

**Tracker:** [#703](https://github.com/DioxusLabs/taffy/issues/703), open since 2024-08-03, author `@giannissc`. Labelled `Feature`. A related issue [#879](https://github.com/DioxusLabs/taffy/issues/879) by `@softmarshmallow` (2025-10-20) requests "compute config for mimicking constraints layout / auto layout / css anchor."

**Why it's stuck:** Anchor positioning is a *post-layout* relationship — element A's position depends on element B's *resolved* layout, where B is anywhere in the tree. Taffy's algorithms are intra-formatting-context; cross-tree position dependency doesn't fit the model. This isn't a missing feature, it's a different layout primitive.

**Browser status:** Chrome 125 (2024) shipped behind a flag, on by default since 128. Safari and Firefox tracking implementations. Not yet broadly cross-browser.

**Buiy posture:** Buiy commits to implementing anchor positioning as a **post-Taffy overlay pass** ([Buiy architecture.md § 3.3](../../specs/2026-05-08-buiy-layout-design/architecture.md#33-anchor-resolution)). Anchored elements lay out via Taffy first using their author-declared dimensions; a Buiy pass then walks every `Anchor` component, looks up the anchor target's `ResolvedLayout`, and overrides the anchored entity's position per the `position-try` chain. The decomposed `Anchor` component lives in [display-and-positioning.md § 3.1](../../specs/2026-05-08-buiy-layout-design/display-and-positioning.md). The anchor target's resolved layout is read from `tree.layout(node_id)`, not entity-side `ResolvedLayout`.

This is one of the two features Buiy explicitly layers above Taffy.

## 4. Container queries (CSS Containment Module Level 3)

**Status:** not implemented. No item on the Taffy roadmap as of 2026-05.

**Tracker:** no specific issue. Closest is [#308](https://github.com/DioxusLabs/taffy/issues/308) which mentions container-query-shaped use cases but is broader.

**Why it's stuck:** Container queries are a *style-feedback* feature, not a layout feature per se. A child's style depends on the resolved size of a nominated container ancestor. This requires running layout, evaluating queries against the resolved sizes, possibly flipping rules, then re-running layout. Taffy doesn't model the rule-evaluation step — it's a styling concern, and Taffy is "not a styling system, just layout."

**Browser status:** Chrome 105 (2022), Safari 16 (2022), Firefox 110 (2023). Stable everywhere.

**Buiy posture:** Buiy commits to implementing container queries **above Taffy** via the same-frame re-layout strategy ([Buiy architecture.md § 3.2](../../specs/2026-05-08-buiy-layout-design/architecture.md#32-container-query-re-layout)):

1. Step 4 evaluates each `@container` rule against the resolved size of its query container, computed in step 3.
2. If any rule's activation state flipped, the entities subject to that rule have a marker component toggled.
3. Steps 1 and 3 re-run once.
4. Cap at 2× Taffy compute per frame; no fixed-point iteration.

The decomposed `Container` component and the rule-carrier `ContainerQuery` live in [container-queries-and-writing-modes.md](../../specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md). Taffy is unaware; it sees only the resulting `taffy::Style` differences after Buiy flips markers.

This is the second feature Buiy explicitly layers above Taffy.

## 5. Writing modes

**Status:** partial.

**Supported in Taffy 0.10:** `Direction::Ltr` / `Direction::Rtl` (closes [#213](https://github.com/DioxusLabs/taffy/issues/213)). This is CSS `direction`, which controls the inline-axis flow.

**Not supported:** `writing-mode: vertical-rl`, `vertical-lr`, `sideways-rl`, `sideways-lr`. The entire vertical-writing-mode family. Tracker: [#752](https://github.com/DioxusLabs/taffy/issues/752), open since 2024-12-04, author Burns.

**Why it's stuck:** Taffy's geometry assumes a horizontal-tb / inline-x / block-y orientation throughout. Rotating the formatting context requires more than the existing `Direction` flip — every algorithm needs to swap axis semantics conditionally. This is a substantial internal refactor.

**Buiy posture:** Buiy's `WritingMode` component carries all five values (HorizontalTb / VerticalRl / VerticalLr / SidewaysRl / SidewaysLr). Open question in [Buiy README § 5](../../specs/2026-05-08-buiy-layout-design/README.md#5-open-questions): whether to ship a Buiy-side rotation pass or wait on Taffy. The cosmic-text text shaper handles vertical orientation regardless; the layout-engine gap is purely about box-flow direction.

## 6. CSS `calc()`

**Status:** supported (since 0.8.0, 2025-04-01) — but via a *typed* not a *symbolic* representation.

**How Taffy does it:** `LengthPercentage::calc(ptr: *const ())` carries an opaque pointer to an embedder-allocated calc expression. The expression's *evaluation* is done by `Style.calc_resolver`, an embedder-provided callback that takes `(*const (), ResolverContext) -> f32`. Taffy never inspects the calc expression; it asks the embedder to resolve it when a value is needed.

**What's missing:** symbolic `calc()` (where Taffy itself owns and evaluates an AST). Not on the roadmap.

**Buiy posture:** Buiy uses Taffy's typed `Length` / `Auto` / `Fr` for the common cases and does **not** ship a calc evaluator in v1. Authors needing `calc(100% - 20px)` express it as a Bevy system that computes the resolved value into a `BoxModel.width = Val::Px(...)` — i.e. calc is in the application layer, not the layout layer. This matches Bevy UI's current posture.

## 7. Float / clear

**Status:** supported (since 0.10.0, 2026-03-31).

**Brief-correction:** the orchestrator brief said "Float / clear / shape-outside — not supported, may never be." That was true through 0.9.x; **false at 0.10.x**. Float and clear are shipped, feature-gated by `float_layout` (sub-feature of `block_layout`, default-on). See [layout-algorithms.md § 4](layout-algorithms.md#4-float).

**Still missing:** `shape-outside`. Floats are rectangular only; content wraps the bounding box. No path to inset-only or polygon-shaped wrap.

**Buiy posture:** v1 ships float and clear via Taffy's support. `shape-outside` is tier-O (not modeled). Buiy's `BoxModel` carries no `shape_outside` field.

## 8. Inline layout

**Status:** not implemented. No `Display::Inline`, `InlineBlock`, `InlineFlex`, `InlineGrid`. No line-box machinery.

**Why:** Taffy treats text as opaque leaves. The embedder's measure function returns one rect for a text leaf; Taffy doesn't do line breaking. Real inline formatting context (line boxes, baseline alignment within a line, `vertical-align`, ruby) is the embedder's job.

**Tracker:** [#308](https://github.com/DioxusLabs/taffy/issues/308) is the broadest issue but is labelled `controversial` because there's deep disagreement about whether Taffy should grow inline at all. Servo and Blitz implement inline above Taffy; everyone else treats text as opaque.

**Buiy posture:** Treat text as opaque leaves via the cosmic-text bridge. Buiy's `Display` enum includes `Inline`, `InlineBlock`, `InlineFlex`, `InlineGrid` as targets but maps them to Taffy as follows:

- `Inline` → block leaf with measure function (single-line for short text, multi-line if the measure function returns a wrapped size).
- `InlineBlock` → `Display::Block` child of an inline parent (so the parent does the inline-positioning); Buiy provides no real "inline formatting context" — the closest is "a horizontal flex parent."
- `InlineFlex` → `Display::Flex`; the "inline-ness" of the container is a paint-time / hit-test-time concern, not a layout concern.
- `InlineGrid` → `Display::Grid`; same as InlineFlex.

This is the same workaround every Taffy embedder uses except Servo and Blitz. Not satisfying; acceptably honest.

## 9. Styling system (media queries, prefers-*, forced-colors)

**Status:** N/A. Taffy is **not** a styling system; it's a layout engine.

`@media`, `@supports`, `prefers-reduced-motion`, `prefers-color-scheme`, `forced-colors` are not in scope. The embedder's styling layer decides which `taffy::Style` to set on each node; Taffy gets the resolved value.

**Buiy posture:** Out of scope for the layout spec. A higher-level "stylesheet" layer above Buiy's typed components is a foundation open question ([Buiy README § 1 non-goals](../../specs/2026-05-08-buiy-layout-design/README.md#1-goals-and-non-goals)).

## 10. `position: sticky`

**Status:** not implemented as such.

**Tracker:** [#771](https://github.com/DioxusLabs/taffy/issues/771) "Support `position: sticky` in Overflow::Scroll nodes" — opened 2024-12-29, author `@PPakalns`. Open, labelled `Feature`.

**What Taffy has:** `Position::Static` / `Relative` / `Absolute`. No `Sticky` variant on the `Position` enum.

**Why it's stuck:** Sticky requires reading the scroll offset and adjusting position per frame. Taffy doesn't model scroll state — that's the embedder's domain.

**Buiy posture:** Buiy implements sticky as a **post-Taffy sub-pass** ([Buiy architecture.md § 3 step 6a](../../specs/2026-05-08-buiy-layout-design/architecture.md#3-system-pipeline)). The `Position::Sticky` variant lives on Buiy's `Position` component; the sub-pass walks every sticky-positioned entity, reads the nearest scrollable ancestor's `ScrollOffset`, and overrides `ResolvedLayout.position`.

## 11. CSS scroll snap

**Status:** not implemented in Taffy.

**Why:** Scroll snap is a behavior layered on top of scrolling — given the resolved layout of snap targets, it constrains where the scroll position lands. Taffy doesn't model scrolling.

**Buiy posture:** Buiy implements scroll snap above Taffy. The `Scroll` component carries `snap_type`, `snap_align`, `snap_stop`, snap padding, snap margin. The application layer reads `ResolvedLayout` for snap-target rectangles and constrains scroll position. See [overflow-and-scrolling.md](../../specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md).

## 12. `aspect-ratio`

**Status:** **supported but partially broken in flex layouts.**

The `Style.aspect_ratio: Option<f32>` field exists and works for block + grid. The bug tracker has [#804](https://github.com/DioxusLabs/taffy/issues/804) "aspect_ratio is not respected in flex layouts" — open, author unknown. The brief asked for verification; the answer is: present but flaky in flexbox specifically.

**Buiy posture:** Expose `aspect_ratio` on `BoxModel`. Document the flex-layout caveat in [box-model.md](../../specs/2026-05-08-buiy-layout-design/box-model.md). Don't work around upstream; track Taffy's fix.

## 13. `gap` for block layout

**Status:** present on `Style` as a top-level field; **honored only for Flex and Grid**, not Block.

Taffy's `Style.gap: Size<LengthPercentage>` is read by `LayoutFlexboxContainer::flexbox_container_style().gap` and `LayoutGridContainer::grid_container_style().gap`. There is no `BlockContainerStyle::gap` reader; Block layout ignores `gap`.

**Browser status:** CSS WG approved `gap` for block layout (it's in CSS Box Sizing Level 4); Chrome 142 shipped it. Taffy hasn't ported the change.

**Buiy posture:** Document the limitation. Authors needing visible gaps between block siblings express them as `margin-bottom` on each child until Taffy lands block `gap`.

## 14. `display: contents`, `display: flow-root`, `display: list-item`, `display: ruby`, `display: table*`

**Status:** none implemented.

The Taffy `Display` enum is exactly `Block | Flex | Grid | None` (feature-gated). No `Contents`, `FlowRoot`, `ListItem`, `Ruby`, `Table`, `TableRow`, `TableCell`, etc.

**Buiy posture:** Buiy's `Display` enum carries all of them in its target shape ([display-and-positioning.md](../../specs/2026-05-08-buiy-layout-design/display-and-positioning.md)):

- `Contents` → effectively removes the box from layout; children participate in the parent's formatting context. Implemented by Buiy as a `Display::None`-equivalent that promotes children to grandparent participation; cannot be done purely via Taffy because Taffy doesn't merge children across the boundary.
- `FlowRoot` → block layout with no margin collapsing through parents; mapped to Taffy's `Display::Block` with a custom containment flag.
- `ListItem` → a `Display::Block` plus marker layout. Marker layout is a Buiy concern.
- `Ruby` → marked tier-E (deferred). No implementation in v1.
- `Table*` → Buiy implements **table algorithm as post-Taffy sub-pass 6b** ([Buiy architecture.md § 3 step 6b](../../specs/2026-05-08-buiy-layout-design/architecture.md#3-system-pipeline)). Taffy's `Style.item_is_table: bool` hint lets the block-flow sizing be table-shaped; Buiy then overrides geometry post-compute.

## 15. Intrinsic sizing keywords as `Dimension` values

**Status:** not implemented as author-set sizes.

**Tracker:** [#751](https://github.com/DioxusLabs/taffy/issues/751), open. Author Burns.

`MinContent`, `MaxContent`, `FitContent` exist as `AvailableSpace` values (algorithmic inputs) and as `MaxTrackSizingFunction` variants for grid tracks. They do **not** exist as `Dimension` variants for `Style.size.width` / `height`. You cannot say `style.size.width = Dimension::MinContent;`.

**Buiy posture:** Document the gap. Buiy's `BoxModel.width` carries `Val::MinContent` / `MaxContent` / `FitContent` as target-shape variants, falling back to `Auto` until upstream lands.

## 16. Multi-column

**Status:** not implemented in Taffy.

`column-count`, `column-width`, `column-gap`, `column-rule`, `column-span`, `column-fill`, `break-inside`, `break-before`, `break-after` are all absent.

**Buiy posture:** Buiy carries a `MultiColumn` component ([Buiy README, components table](../../specs/2026-05-08-buiy-layout-design/README.md)) marked tier-E (stub in v1). The post-Taffy sub-pass 6c "MulticolPack" implements basic column packing above Taffy. Full multi-column with break-inside / break-before / break-after is tier-E (deferred).

## Summary table

| Feature | Taffy status | Buiy posture |
|---|---|---|
| Subgrid | Not shipped, no PR | Stub + warn; wait on upstream |
| Masonry | Not shipped, no PR | Tier-E (deferred) |
| Anchor positioning | Not shipped, no roadmap | **Above Taffy** (post-pass 6d) |
| Container queries | Not shipped, no roadmap | **Above Taffy** (2× re-layout) |
| Writing modes (vertical) | Direction only | Open question |
| `calc()` | Typed (since 0.8), no symbolic | Use typed |
| Float / clear | Shipped 0.10 (brief was wrong) | Use upstream |
| `shape-outside` | Not shipped, unlikely | Tier-O |
| Inline layout | No `Display::Inline` | Map to flex/block; opaque text |
| Media queries / prefers-* | Out of scope | Buiy/foundation layer |
| `position: sticky` | Not shipped | **Above Taffy** (sub-pass 6a) |
| Scroll snap | Not shipped | **Above Taffy** (app layer) |
| `aspect-ratio` | Shipped, flex-broken | Expose, track upstream bug |
| `gap` for block | Not shipped | Workaround via margins |
| `display: contents` etc. | Not shipped | **Above Taffy** (sub-pass 6b) |
| Intrinsic size keywords | Algorithmic-input only | Stub + Auto fallback |
| Multi-column | Not shipped | **Above Taffy** (sub-pass 6c) |

Six features Buiy commits to layering above Taffy: anchor positioning, container queries, sticky, scroll snap, tables (Display::Table*), multi-column. Three deferred (subgrid, masonry, ruby). One out-of-scope (shape-outside). The rest mapped or worked around.

## Sources

- Issue #468 (subgrid): https://github.com/DioxusLabs/taffy/issues/468
- Issue #910 (masonry): https://github.com/DioxusLabs/taffy/issues/910
- Issue #703 (anchor positioning): https://github.com/DioxusLabs/taffy/issues/703
- Issue #879 (constraints/auto-layout/anchor): https://github.com/DioxusLabs/taffy/issues/879
- Issue #752 (writing-mode vertical): https://github.com/DioxusLabs/taffy/issues/752
- Issue #771 (position: sticky): https://github.com/DioxusLabs/taffy/issues/771
- Issue #751 (intrinsic sizing keywords): https://github.com/DioxusLabs/taffy/issues/751
- Issue #804 (aspect-ratio flex bug): https://github.com/DioxusLabs/taffy/issues/804
- Issue #213 (Direction property, closed in 0.10): https://github.com/DioxusLabs/taffy/issues/213
- Issue #308 (Morphorm/inline, controversial): https://github.com/DioxusLabs/taffy/issues/308
- Roadmap issue #345: https://github.com/DioxusLabs/taffy/issues/345
- Taffy CHANGELOG (verify float in 0.10, calc in 0.8): https://github.com/DioxusLabs/taffy/blob/main/CHANGELOG.md
- Buiy layout architecture: [`docs/specs/2026-05-08-buiy-layout-design/architecture.md`](../../specs/2026-05-08-buiy-layout-design/architecture.md)
- Buiy layout README + open questions: [`docs/specs/2026-05-08-buiy-layout-design/README.md`](../../specs/2026-05-08-buiy-layout-design/README.md)
- Sibling: [layout-algorithms.md](layout-algorithms.md), [critiques.md](critiques.md), [integration.md](integration.md)
