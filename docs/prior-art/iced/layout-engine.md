**Date:** 2026-05-22
**Status:** active
**Subject:** Iced — bespoke flex layout engine in `iced_core::layout` (not Taffy)

# Iced layout engine

Iced has its own layout engine in `iced_core::layout`. **It does NOT use Taffy.** The implementation is a hand-rolled flex algorithm at `core/src/layout/flex.rs`, source-noted as "heavily inspired by the [druid] codebase." It is one of two production-scale alternatives to Taffy in the Rust GUI ecosystem (the other being Floem's `taffy::compute_*` direct calls + custom passes).

Sibling files: [`architecture.md`](architecture.md), [`widgets-and-styling.md`](widgets-and-styling.md).

## What Iced layout has

Layout primitives at the engine level:

- `Node` — tree of resolved boxes (`bounds: Rectangle`, `children: Vec<Node>`).
- `Limits { min: Size, max: Size, fill: Size }` — constraint structure propagated downward. `fill` indicates how much "fill" budget is available.
- `Layout<'a>` — read-only view onto a `Node` plus its absolute position, used during draw / event dispatch.
- `flex::resolve(axis, limits, items, spacing, alignment, ...)` — the workhorse: distributes children along a main axis with cross-axis alignment.
- Length primitives: `Length::Fixed(f32)`, `Length::Fill`, `Length::FillPortion(u16)`, `Length::Shrink` — analogous to CSS `width: Npx`, `width: 100%` (with flex-grow), `flex-grow: N`, and `width: max-content`.
- `Padding`, `Alignment`, `Axis::{Horizontal, Vertical}` — supporting types.

Layout primitives at the widget level:

- `Row`, `Column` — flex along one axis.
- `Container` — single child with padding / alignment.
- `Space` — fixed-size empty box.
- `Stack` — z-stack; all children share the same rectangle.
- `Scrollable` — viewport + scrollable inner.
- `PaneGrid` — hierarchical splittable layout via percentage splits.
- `Pin`, `Float` — 0.14 additions for overlay positioning.

## What Iced layout does NOT have

- **No CSS Grid.** No `grid-template-rows / -columns`, no `grid-area`, no `grid-template-areas`. Multi-column layouts are achieved by nesting `Row` of `Column` (manual gutter math) or by the new 0.14 `Grid` widget (which is a bounded-row-by-bounded-column structure, not the full CSS Grid algorithm).
- **No subgrid.**
- **No anchor positioning.** No CSS `anchor()` function; the new `Pin` / `Float` widgets in 0.14 do anchored overlays but at a much narrower scope (parent-relative, no fallback strategies, no `position-try`).
- **No container queries.** No way for a widget to size itself differently based on the size of an ancestor (other than its direct `Limits`).
- **No `float`.** No text-flow-around-element pattern. Markdown content with images uses block-stacking only.
- **No baseline alignment.** Cross-axis alignment is `Start | Center | End`; no CSS `align-items: baseline`.
- **No writing-mode awareness.** Layout is LTR-block-axis throughout; vertical writing modes are not represented in `Limits` or `Axis`.
- **No fragmentation / paging.** No way to split a layout across multiple pages.
- **No intrinsic-size sniffing.** `min-content` / `max-content` / `fit-content` (CSS3) have no direct equivalent. `Length::Shrink` is the closest — "size to content" — but it's binary (shrink or not).

## The algorithm

`flex::resolve` runs in four passes (the source describes "four-pass," the [WebFetch summary called it "five-pass"] — counting the alignment finalization separately):

1. **Non-fluid pass.** Lay out all children with fixed `Length` (`Length::Fixed`, `Length::Shrink`). These consume known space.
2. **Cross-axis compression (conditional).** If a cross-axis-`Fill` child reports a size exceeding limits, compress the cross-axis budget for siblings.
3. **Fluid distribution.** Remaining main-axis budget after pass 1 is divided among `Length::Fill` and `Length::FillPortion` children by their portion weights.
4. **Deferred resolution.** Any child whose layout depended on the resolved size of siblings (rare, but possible via `widget::Responsive`) re-runs with the resolved limits.
5. **Alignment finalization.** Cross-axis `Alignment` (`Start | Center | End | Fill`) is applied; each child's offset within the cross-axis is fixed.

Compared to Taffy's flexbox implementation (full CSS Flexbox Level 1, with `flex-basis` distinction from `width`, `flex-grow` / `flex-shrink` separately, baseline alignment, multi-line wrap), Iced's is a strict subset. Iced has no `flex-basis` separate from `Length`; no `flex-shrink: 0` to opt out of compression; no multi-line flex (`flex-wrap`). For the widget set Iced ships, this is sufficient; for a comprehensive UI library, it would not be.

## Constraint propagation

```
Parent layout()
  ├─ build Limits { min, max, fill } based on parent's own limits + padding + child count
  ├─ for each child:
  │    └─ call child.layout(&mut tree, renderer, &child_limits) → Node
  ├─ collect Nodes
  ├─ run flex distribution / alignment over collected Nodes
  └─ return parent Node containing children
```

`Limits` flow strictly downward; `Size` (encoded in returned `Node`) flows strictly upward. No mid-layout call upward to a parent; no two-phase intrinsic-then-final layout. This is the standard one-pass-down, one-pass-up shape — simple, fast, no convergence loop.

The cost: layout cannot ask "how big would my parent want me to be if I were 200px tall?" — there is no negotiation. CSS handles this via separate intrinsic-size and used-size passes; Iced ducks the question by limiting layout primitives to ones that don't require it.

## Performance

No published benchmarks at 1,000+ nodes from Iced upstream. Anecdotal:

- COSMIC desktop apps (cosmic-files, cosmic-edit) run smoothly with hundreds of widgets per view. The architecture's `Lazy` widget memoizes subtree layout against dep hashes, so unchanged subtrees skip the layout pass entirely.
- Halloy IRC client runs smoothly with hundreds of message-list children rendered via `Scrollable + Column + Keyed`.
- The per-frame full-relayout cost is real — Iced does NOT cache layout across frames at the engine level. Apps that need it use `Lazy` per subtree.

For Buiy's commitment to 1000+-node productivity-app fixtures ([verification.md](../../specs/2026-05-07-buiy-foundation/verification.md)), Iced's "you must memoize via `Lazy`" pattern is the load-bearing optimization. Without `Lazy`, large UIs re-layout every frame — same shape as the bevy_ui complaint about lack of layout caching ([`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § Avoid → Per-frame full layout rebuild).

## Why Iced didn't pick Taffy

Iced predates Taffy at the production level. Iced's first layout code shipped in 0.1 (2020-04); Taffy didn't exist yet (Stretch → Taffy didn't happen until 2022). By the time Taffy was production-viable (Taffy 0.3, mid-2023), Iced's bespoke layout was already shipping in COSMIC.

There is **no public design rationale** for not migrating to Taffy. Speculation:

- Iced's widget set is intentionally narrow (no CSS Grid widget). Migrating to Taffy would add a large dependency for capabilities Iced doesn't expose.
- Iced's runtime is renderer-agnostic; coupling to a layout-engine crate adds a coupling Iced has avoided.
- Iced's `Limits + Node` model is simpler than Taffy's `Style + LayoutTree + cache_style + compute_layout` machinery. The simpler model is sufficient for the widget set; Taffy would be overkill.

The trade for Iced is locked-in: Iced cannot add CSS Grid / subgrid / container queries / anchor positioning without either rewriting the layout engine or adopting Taffy. The 0.14 `Grid` widget is bounded-and-simple precisely because the layout engine doesn't support the full algorithm.

## Comparison to Buiy's Taffy-based approach

| Aspect | Iced layout | Buiy layout (Taffy) |
|---|---|---|
| Layout engine | Own; ~few-hundred-LOC flex + helpers | Taffy crate (~5K+ LOC, comprehensive) |
| CSS Flexbox | Subset (no `flex-basis`, no wrap, no baseline) | Full Level 1 |
| CSS Grid | None (the `Grid` widget is bespoke + bounded) | Full (Taffy 0.3+, including named lines / templates 0.9+) |
| CSS Block | None | Full (Taffy 0.4+) |
| Float / fragmentation | None | Float yes (Taffy 0.10); fragmentation no |
| Container queries | None | Planned via Taffy's measure-function injection |
| Anchor positioning | `Pin` / `Float` widgets only | Planned as Buiy-level layer above Taffy |
| Vertical writing modes | None | Planned as Buiy-level transform layer above Taffy |
| Subgrid | None | Future Taffy capability; Buiy inherits when shipped |
| Per-frame perf | Full relayout unless `Lazy` | Taffy 0.6+ has cache; Buiy adds change-detection layer |
| Algorithm complexity | Simple (1-down, 1-up + flex) | Multi-pass CSS-correct |

Buiy's commitment to Taffy ([architecture.md § 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md)) is the right bet for comprehensive feature parity. Iced's bespoke engine is the right bet for Iced's narrower scope. They're not the same target.

## Implications for Buiy

1. **Iced is not a layout-engine reference.** The flex algorithm is small enough to read in an hour and well-documented in the source. But its constraints (no Grid, no anchor, no writing modes) are exactly the constraints Buiy's feature inventory ([visuals.md § 3.2](../../specs/2026-05-07-buiy-foundation/visuals.md)) is committed to *exceeding*. Don't borrow the engine.
2. **Iced validates "narrow widget set + own layout" can ship at scale.** COSMIC desktop is the proof: a full desktop environment built on a flex-only layout engine. For UIs that don't need CSS Grid, the engine is sufficient. Buiy's commitment to comprehensive feature parity is what forces Taffy.
3. **`Lazy` subtree memoization is the load-bearing perf pattern.** Iced's apps depend on it for any UI above ~200 nodes. Buiy's ECS change-detection + observer model gives the same shape (skip work for unchanged entities) without an explicit `Lazy` widget; validate that Buiy's verification fixtures ([verification.md](../../specs/2026-05-07-buiy-foundation/verification.md)) cover the "many static nodes + a few changing" case.
4. **One-down-one-up constraint propagation is good enough for most cases.** Iced demonstrates that CSS-style multi-pass intrinsic-then-final layout is not required for production-grade UIs. Taffy supports both — Buiy gets the option to stay in one-pass when widgets allow.
5. **The "no Grid widget but a Grid layout via Taffy" gap is the differentiator.** Iced apps have to nest Row/Column for two-axis structures. Buiy apps will write `Grid` directly. This is a clear ergonomic win Buiy gets for free from Taffy adoption. Cross-link: [`../taffy/`](../taffy/) and [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § Validates → Taffy.

## Sources

- Iced core layout — https://github.com/iced-rs/iced/blob/0.14/core/src/layout.rs
- Iced flex impl — https://github.com/iced-rs/iced/blob/0.14/core/src/layout/flex.rs
- Iced 0.14 Cargo.toml (no taffy dep) — https://github.com/iced-rs/iced/blob/0.14/Cargo.toml
- Iced 0.14 release notes (Grid / Table / Pin / Float widgets) — https://github.com/iced-rs/iced/releases/tag/0.14.0
- Taffy repository — https://github.com/DioxusLabs/taffy
- Druid layout (cited as inspiration) — https://github.com/linebender/druid
- Buiy foundation visuals (Grid / anchor) — [`../../specs/2026-05-07-buiy-foundation/visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md)
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
