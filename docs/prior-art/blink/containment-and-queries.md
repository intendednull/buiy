**Date:** 2026-05-29
**Status:** active
**Subject:** Blink (Chromium) — CSS Containment (`contain`, `content-visibility`), container queries, and CSS anchor positioning, as the reference implementation behind Buiy's Phase 8 (Containment) and Phases 5/6 (container queries, anchor positioning)

# Blink: containment and queries

Three CSS modules that Buiy implements as a typed-Rust subset all trace their canonical implementation to Blink: CSS Containment Module Level 3 (`contain`, `content-visibility`), CSS Containment Level 3's `@container` size queries, and CSS Anchor Positioning Level 1. Containment is the load-bearing primitive under all three — container queries are *defined in terms of* containment, and `content-visibility: auto` is *defined as* an automatic, viewport-gated application of containment. This file maps Blink's implementation to Buiy's [`Containment`](../../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md) component (Phase 8, landed), [`Container` / `ContainerQuery`](../../specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md) (Phase 5/6), and [`Anchor`](../../specs/2026-05-08-buiy-layout-design/display-and-positioning.md) (Phase 5/6).

See [layout.md](layout.md) for LayoutNG's fragment tree (containment is what lets fragments be cached/skipped), [stacking-and-paint.md](stacking-and-paint.md) for `contain: paint` as a stacking-context and property-tree trigger, and [architecture.md](architecture.md) for where containment sits in the RenderingNG lifecycle.

## 1. `contain` — the four containment types (Chrome 52, June 2016)

`contain` shipped in Chrome 52 (June 2016) with four independent flags plus shorthands:

| `contain` value | Effect |
|---|---|
| `layout` | The element's internal layout is opaque to the outside; descendants cannot affect ancestor layout, and the element is a containing block + a formatting context. |
| `paint` | Descendants are clipped to the border box; nothing paints outside; the element forms a stacking context and an isolated paint root. |
| `size` | The element's own size is computed *without* examining descendants — it must declare a size, else it resolves to zero in the contained axes. |
| `style` | Counter/quote scopes (e.g. `counter-reset`) don't escape the element. |
| `content` (shorthand) | `layout paint style` (note: *not* `size`). |
| `strict` (shorthand) | `layout paint size style`. |

Containment is a performance opt-in: it lets Blink prove that work inside a subtree cannot escape, so style recalc, layout, and paint can be scoped or skipped. In LayoutNG terms, a contained subtree's immutable fragment can be reused when only the *outside* changed (see [layout.md](layout.md)).

**Buiy mapping.** Buiy's [`Containment.contain: ContainFlags`](../../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md) is a `bitflags` `u8` with exactly these flags — `LAYOUT`, `PAINT`, `SIZE`, `STYLE`, `INLINE_SIZE`, plus `CONTENT` (= `LAYOUT | PAINT | STYLE`) and `STRICT` (= `LAYOUT | PAINT | SIZE | STYLE`). The shorthand decompositions match Blink/the spec exactly, including the deliberate omission of `SIZE` from `CONTENT`. Buiy honors `LAYOUT` as a change-detection scope (a `Changed<X>` inside a `CONTENT`-contained subtree doesn't invalidate siblings) — the same proof Blink uses, expressed in ECS terms instead of dirty-bit propagation.

### 1.1 SIZE-zeroing: the same failure mode in both engines

Blink: `contain: size` with no declared size makes the contained axis resolve to zero (the element collapses) because the engine is forbidden from looking at descendants to find an intrinsic size. `contain-intrinsic-size` exists precisely to supply a fallback so the box doesn't collapse to zero.

Buiy reproduces this: if `ContainFlags::SIZE` (or `INLINE_SIZE`) is set and the relevant `Sizing` is `Auto`, Buiy treats it as `Length::px(0.0)` and emits a `warn!`. This is CSS-faithful — it is the *defined* behavior, not a Buiy shortcut — and the `warn!` is Buiy's substitute for Blink's silent collapse, which is a frequent author footgun.

## 2. `content-visibility: auto` — automatic containment (Chrome 85, Aug 2020)

`content-visibility` shipped in Chromium 85 (web.dev article published 2020-08-05). Its values:

- `visible` — default; no effect.
- `hidden` — skips rendering of contents (layout + paint), like `display: none` for the subtree but cheaper to toggle back; the subtree retains state and is not in the a11y tree.
- `auto` — the interesting one. The element gains layout, style, and paint containment unconditionally; **when off-screen and not user-relevant** (no focus/selection inside), it *additionally* gains size containment and skips laying out, painting, and hit-testing its descendants. When it scrolls on-screen, full rendering snaps back.

The documented win is large (web.dev cites a ~7× initial-render improvement on a long chunked page). The catch Blink hit, and the reason `content-visibility: auto` is tricky: when a skipped subtree has no `contain-intrinsic-size`, the engine must lay it out anyway to learn its size, which can cause scrollbar jumping and reflows as elements enter/leave the viewport. `contain-intrinsic-size` (and the `auto` keyword that remembers the last-rendered size) is the fix.

**Buiy mapping.** Phase 8 ships `content-visibility` as a **stub**: the `ContentVisibility { Visible | Auto | Hidden }` value is stored on `Containment` but the off-screen rendering-skip is not yet wired. The Buiy spec ([transforms-and-containment.md § 5.2](../../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md)) describes the intended implementation — during pipeline step 1, check `Auto` + last-frame off-screen `ResolvedLayout`, mark the subtree for skip, feed Taffy a sentinel size, no-op descendant style sync — which is structurally Blink's approach (viewport-gate the size containment) adapted to Buiy's "layout writes, render reads" contract. Buiy has already learned Blink's lesson: the skip is only safe/useful *with* an intrinsic-size hint, so Buiy's design predicates the Taffy-skip on `contain-intrinsic-size` being set, exactly as Blink does.

`ContentVisibility::Hidden` maps to Blink's `hidden`: Buiy treats it as `Display::None` for descendants (skips layout) but is a cheaper toggle than mutating `Display`.

## 3. Container queries (Chrome 105, Aug 2022) — built ON containment

Container queries (the `@container` rule, `container-type`, `container-name`, `cqw`/`cqi` units) shipped in Chrome 105 (2022-08-30). The load-bearing design fact: **size container queries require containment to avoid infinite layout loops.** Establishing a query container with `container-type` *applies* containment to that element:

- `container-type: size` → applies **layout + style + size** containment (both axes queryable).
- `container-type: inline-size` → applies **layout + style + inline-size** containment (only the inline axis queryable; this is the common case because querying the block axis requires a fixed block size, which most flow content doesn't have).
- `container-type: normal` → no containment; only style queries (which need no containment) work.

The reason is a loop-prevention argument, quoted by MDN: size containment ensures child size can't size the parent; layout containment ensures children can't affect outside layout; style containment ensures counters can't escape — "All three of those are necessary precautions to avoid looping behavior, where changes inside a query could impact the results of the query." This is the same loop that makes naive container queries impossible without it.

**Buiy mapping.** Buiy's [`Container { container_type: ContainerType::{Normal|Size|InlineSize} }`](../../specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md) mirrors Blink's three `container-type` values one-to-one. Buiy does **not** auto-apply the full containment bundle as a side-effect of `container-type` the way CSS does; instead Buiy's container-query pass handles the loop directly via its **same-frame re-layout** strategy: step 2 (`CqActivate`) evaluates rules against the *previous* frame's `ResolvedLayout`, step 4 (`CqFlipCheck`) re-checks against the current frame and triggers at most one re-run of `SyncStyles` + `TaffyCompute` (`CqFlipReRun`). The 2×-Taffy cost ceiling is Buiy's loop-breaker, where Blink's is the mandatory `size`/`layout` containment. Both prevent the same oscillation; Buiy's is explicit and bounded, Blink's is structural via the containment requirement. Buiy's `INLINE_SIZE` containment flag exists precisely so that `ContainerType::InlineSize` can opt the same SIZE-zeroing on one axis that Blink applies — see [§ 1.1](#11-size-zeroing-the-same-failure-mode-in-both-engines).

## 4. CSS anchor positioning (Chrome 125, May 2024)

CSS Anchor Positioning Level 1 shipped in Chrome 125 (rollout began 2024-05-14) — the first browser to ship it. It lets an absolutely-positioned element tether to one or more anchor elements declaratively (no JS): `anchor-name` registers an anchor, the `anchor()` function references the anchor's edges in `inset` properties, and `position-try-fallbacks` declares fallback positions when the preferred one overflows. A common use is positioning a popover/tooltip next to its trigger.

Honest caveat worth recording: anchor positioning shipped *unprefixed but with syntax churn*. Chrome 125 shipped `inset-area` and `position-try-options`; the CSS WG then renamed these to `position-area` and `position-try-fallbacks`, which Blink shipped in **Chrome 129**. Anyone reading Chrome-125-era code/tutorials will see the now-renamed property names. Cross-browser support also lagged for over a year (Firefox/Safari trailed).

**Buiy mapping.** Buiy's [`Anchor`](../../specs/2026-05-08-buiy-layout-design/display-and-positioning.md) is a post-Taffy pass (sub-pass 6d) that walks entities with `position_anchor.is_some()`, resolves the target via an `AnchorNameRegistry`, applies position-try fallbacks, and writes the resolved offset. Buiy mirrors CSS's fallback model (`PositionTry`, `FitsInContainer`, `AnchorVisible` conditions) and, like Blink, treats a missing/`Display::None` anchor as a resolution failure (Buiy: defaults to `(0,0)` + `LayoutAnchorBroken` marker + once-per-frame `warn!`). Because anchor positioning is a cross-tree post-layout dependency — fundamentally unlike an intra-formatting-context algorithm — neither Taffy nor LayoutNG's core can express it; both Blink and Buiy bolt it on as a positioning pass *after* the main layout. (Taffy's anchor-positioning issue [#703] remains open; see [../taffy/open-problems.md](../taffy/open-problems.md).)

## 5. Where this sits in the pipeline

In RenderingNG, containment is consumed across the lifecycle (style → layout → pre-paint → paint; see [architecture.md](architecture.md)): `contain: style` scopes style recalc, `contain: layout`/`size` scope LayoutNG, `contain: paint` is a pre-paint property-tree + stacking-context trigger ([stacking-and-paint.md](stacking-and-paint.md)). Buiy collapses this into its single layout pipeline: containment flags are read during `SyncStyles`/`TaffyCompute` (for SIZE-zeroing and change-detection scope) and handed to render via the private `ResolvedTransform`/containment-flag handoff. The contract — layout writes, render reads, render never recomputes — means Buiy's `contain: paint` participation lands in Phase 9's stacking sub-pass (6f), not Phase 8.

**`contain: paint` does two things, not one — and `isolation: isolate` does only one.** Both appear in Phase 9's stacking-context-trigger *union*, and it is tempting for a 6f author to treat every union entry as "just forms a stacking context." But `contain: paint` *also clips*: per MDN, a descendant that overflows "will be clipped to the containing element's border-box," and `paint`/`layout`/`strict`/`content` each create a new stacking context **plus** a new containing block and block formatting context. `isolation: isolate`, by contrast, creates a stacking context *with no clip* — it exists only to isolate `mix-blend-mode`. So when 6f detects the union, the clip side of the render hand-off must fire for `Paint`/`Strict` containment but **not** for `isolation`. Conflating them either over-clips isolated subtrees or lets paint-contained descendants escape the border box. Buiy keeps these as separate components (`Containment::contain` vs `Stacking`'s `Isolation`), so the fix is to keep the SC-trigger union and the clip output as two distinct results of 6f — see [stacking-and-paint.md § 1.1](stacking-and-paint.md).

## Implications for Buiy

- **The SIZE-zeroing + `warn!` is correct and Blink-aligned.** Blink collapses to zero silently; Buiy's `warn!` surfaces the footgun. Keep it.
- **Don't auto-apply containment from `container-type`.** Buiy's bounded same-frame re-layout already breaks the loop that CSS solves via mandatory containment. Auto-applying paint containment (a stacking-context trigger) as a side-effect of `container-type` would be a surprising visual change; Buiy's explicit `INLINE_SIZE`-on-`Container` opt-in is cleaner. Verify this stays intentional when Phase 5/6 lands.
- **`content-visibility: auto` is worth the intrinsic-size dependency.** Blink's ~7× win is real, but only with `contain-intrinsic-size`. Buiy's stub already encodes that precondition; when un-stubbing, ship `contain-intrinsic-size` *first*, or the skip causes scroll jank for the same reason it did in Blink.
- **Anchor positioning syntax is not settled prior art.** Buiy chose typed components, sidestepping the `inset-area`→`position-area` churn entirely, but should track the CSS WG's resolution of fallback semantics rather than the Chrome-125 snapshot.

## Sources

- Blink announcement (WebKit fork, 2013-04-03): https://blog.chromium.org/2013/04/blink-rendering-engine-for-chromium.html
- CSS Containment in Chrome 52 (2016-06): https://developer.chrome.com/blog/css-containment
- `contain` reference: https://developer.mozilla.org/en-US/docs/Web/CSS/contain
- `content-visibility` (Chromium 85, 2020-08-05): https://web.dev/articles/content-visibility
- `content-visibility` reference: https://developer.mozilla.org/en-US/docs/Web/CSS/content-visibility
- Container queries land in Chromium 105 (2022): https://developer.chrome.com/blog/has-with-cq-m105
- New in Chrome 105 (shipped 2022-08-30): https://developer.chrome.com/blog/new-in-chrome-105
- Container queries + containment requirement: https://developer.mozilla.org/en-US/docs/Web/CSS/Guides/Containment/Container_queries
- CSS Containment Module Level 3 (W3C): https://www.w3.org/TR/css-contain-3/
- CSS anchor positioning API (Chrome 125, rollout 2024-05-14): https://developer.chrome.com/blog/anchor-positioning-api
- Anchor positioning syntax changes (renamed in Chrome 129): https://developer.chrome.com/blog/anchor-syntax-changes
- RenderingNG architecture (pipeline stages): https://developer.chrome.com/docs/chromium/renderingng-architecture
- Buiy transforms + containment spec: [../../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md](../../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md)
- Buiy container queries + writing modes spec: [../../specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md](../../specs/2026-05-08-buiy-layout-design/container-queries-and-writing-modes.md)
- Buiy display + positioning (anchor) spec: [../../specs/2026-05-08-buiy-layout-design/display-and-positioning.md](../../specs/2026-05-08-buiy-layout-design/display-and-positioning.md)
- Buiy foundation overview: [../../specs/2026-05-07-buiy-foundation/README.md](../../specs/2026-05-07-buiy-foundation/README.md)
- Sibling prior-art: [architecture.md](architecture.md), [layout.md](layout.md), [stacking-and-paint.md](stacking-and-paint.md), [style.md](style.md)
- Taffy's missing anchor positioning / container queries: [../taffy/open-problems.md](../taffy/open-problems.md)
