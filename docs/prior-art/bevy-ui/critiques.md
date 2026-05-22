**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui — first-party and third-party critiques of design, renderer, ergonomics

# Critiques

This file collects the substantive criticisms of bevy_ui — from its own maintainers, from issue authors, and from third-party UI builders who chose to write parallel stacks. The goal is honesty, not piling on: every critique here motivates a design bet in the Buiy foundation spec. Where a critique has been *resolved* by a recent release, that is noted.

## The renderer feature gaps

These are the renderer-side gaps that the Buiy foundation spec explicitly cites as the reason for going parallel rather than building on top of bevy_ui (foundation README § 1.4 "Parallel to bevy_ui"):

### Non-rectangular clipping

Issue [**#9381**](https://github.com/bevyengine/bevy/issues/9381) — "UI clipping only works for untransformed axis-aligned rectangles." Author: **@ickshonpe**, opened 2023-08-07, **still open** as of 2026-05-22. Verbatim from the search synthesis:

> "Bevy currently only supports rectangular clipping regions (which are the easiest to implement and cheapest from a performance standpoint), but which are inadequate for the kinds of UIs we want to build. Unfortunately, there's no solution to the clipping problem which isn't expensive: path-based clip regions, off-screen render targets, or stencil buffers all have significant costs."

The issue is open and **labeled "needs design consideration."** No path to resolution as of 0.18.

### Rounded-corner clipping bug

Issue [**#13093**](https://github.com/bevyengine/bevy/issues/13093) — "Rounded corners are not clipped." Author: **@viridia**, opened 2024-04-25, **still open**. Verbatim summary: "When UI elements featuring border radius values greater than zero are positioned partially off-screen, the rounded corner effect is rendered following the clipping operation rather than before it." This is a render-order bug — the rounding shader runs after the rect clip, producing wrong visuals at viewport edges.

### Transform-aware clipping

Same issue #9381 also documents: "The clipping Rect's size takes its bounds from the unscaled size of the UI node. If the UI node's Transform has a scaling it will be clipped incorrectly. Rotation is much more difficult. A minimal fix would be just to ignore the Overflow setting for any UI node with a rotation and its descendants." Translation: bevy_ui's clipping does not respect transforms — a documented limitation.

### backdrop-filter, mix-blend-mode, isolation, true top layer

I could not find dedicated open issues for these in bevy_ui's tracker as of 2026-05-22. The Buiy foundation spec cites them as "renderer caps several capabilities (non-rect clipping, backdrop-filter, mix-blend-mode, isolation, true top layer) that web parity requires" (foundation README § 1.4). The absence of even open issues for backdrop-filter / mix-blend-mode / isolation in bevy_ui is itself evidence — these are not on bevy_ui's roadmap, they have not been requested loudly enough to file. **For web feature parity, Buiy must implement them itself, not wait for upstream.**

## The 10 Challenges (#11100)

Discussion [**#11100**](https://github.com/bevyengine/bevy/issues/11100) — "10 Challenges for Bevy UI Frameworks." Author: **@TimJentzsch**, opened 2023-12-27, still open. The challenges are not critiques of bevy_ui per se — they are *fixture problems* TimJentzsch set up so different UI frameworks could demonstrate their capability. Verbatim list:

1. **Game Menu** — main menu with sub-menus for audio/graphics; buttons; volume slider; dropdown for graphics quality.
2. **Inventory** — fixed-size grid, item slots with images + count overlays.
3. **Health Bar** — 3D scene with a character (sphere); HP bar + name anchored in world-space.
4. **Responsive Menu** — buttons with a nine-patch system; image to the right of buttons.
5. **Character Editor** — 3D scene on the left, UI panel on the right with selection buttons.
6. **HUD** — top-left minimap; bottom-left HP counter + bar; bottom-center game time + team scores.
7. **Bug Report Form** — bug-type dropdown; single-line title input; multi-line textarea.
8. **Scoreboard** — grid of players with avatar/name/K/D/A columns.
9. **Dark/Light Theme** — UI with a toggle button; theme switch updates all colors.
10. **Design Specification** — exercise of building a specific designed UI (cited as the "styling matters" challenge).

The implicit critique: as of late 2023, **no Bevy UI framework had cleanly demonstrated all 10**. Many of them remain hard in bevy_ui today: #3 (worldspace-anchored UI) requires bevy_lunex's transform approach or Buiy's `buiy_3d` design; #9 (dark/light theme switching) doesn't have a built-in token system in bevy_ui (Buiy fills this — foundation architecture.md § 2.5); #2 + #8 (grids) only got proper CSS Grid in bevy_ui after the Taffy 0.6 upgrade (PR #15844, late 2024). The list is the closest thing the Bevy community has to a UI-capability benchmark.

## The megacomponent problem (#17644)

Issue [**#17644**](https://github.com/bevyengine/bevy/issues/17644) — "Design of bevy_a11y is BSN-unfriendly." Author: **@viridia**, opened ~2025-02-02. **Partially mitigated by PR #24308 (added `AccessibleLabel` sibling that mirrors into the unchanged `AccessibilityNode`); the megacomponent itself remains as of Bevy 0.19.0-rc.1.** Core argument:

> "Because of this, I can well imagine wanting to merge together multiple BSN templates, each of which has opinions about various accessibility attributes."

The technical specifics: `AccessibilityNode` (Bevy's a11y component) had **private fields exposed only through method-style setters**. BSN works by *patching component property values from layered templates* (PR #20158). If properties are private, BSN can't reach them. If methods are inconsistent (some `set_x`, some `with_x`, some via builder), BSN can't introspect them either. So `bevy_a11y` was BSN-incompatible.

**Why this generalizes** (and the lesson Buiy embeds — foundation README § 1.3, architecture.md § 2.4): any "megacomponent" with private fields blocks BSN authoring. The Buiy hard rule is that every component is small, public-fielded, observable, and decomposed by concern. Forced into the bones of every Buiy subsystem. The `bevy_a11y` mistake is the cautionary tale.

This also motivates the architecture decision to **replace bevy_a11y rather than layer over it** (foundation architecture.md § 2.6) — even after PR #24308 added `AccessibleLabel`, the underlying `AccessibilityNode` megacomponent is unchanged; the partial decomposition that has shipped serves bevy_ui's needs, not Buiy's. Buiy needs its own decomposed a11y components (`A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations`) keyed to its own component model.

## Third-party "why we built parallel" critiques

### bevy_lunex

The README pitches the transform-based approach as a fix for "bevy_ui nodes all have Transform and GlobalTransform components, but you're not allowed to touch them." This is a real friction point — game UIs frequently want to position UI relative to world-space objects (the HUD challenge #3 above). bevy_lunex's design choice is to let UI nodes use Bevy's standard `Transform` hierarchy. Cost: doesn't compose with Taffy-driven flexbox/grid in the natural way. **Buiy's choice:** keep Taffy as the primary positioning and add `buiy_3d` as a distinct subsystem for 3D-anchored UI (foundation architecture.md § 2.3, 3D-anchored UI bullet).

### woodpecker_ui (successor to kayak_ui)

@StarArawn's post-mortem on **kayak_ui** (paraphrased from publicly cited commentary; I could not retrieve a verbatim author critique): kayak_ui "suffered from overly complicated internals, making it difficult to contribute to and causing fundamental bugs." Woodpecker_ui was built as a reset — reactive framework, Vello-based rendering, ECS-first design. The decision to use **Vello** (a separate GPU 2D vector renderer) rather than Bevy's render-graph is itself a critique: it implies the Bevy render-graph's UI integration was not flexible enough for Woodpecker's needs.

### "How do nice UI in Bevy?!?" (deadmoney.gg)

Could not retrieve — the site returned HTTP 403. The article was cited in multiple search results as a third-party developer's pain log for building UI on Bevy. Listed here as a known critique source that future Buiy doc updates should grab via an authenticated fetch.

## Performance critiques

bevy_ui has well-documented performance limits at moderate node counts:

- Issue [**#677**](https://github.com/bevyengine/bevy/issues/677) — "very low performance when spawning UI nodes one inside the other": "becomes unresponsive after 15-20 iterations."
- Issue [**#276**](https://github.com/bevyengine/bevy/issues/276) — "Improve Performance of bevy_ui parent with many children" — "parent containing many children (on the order of 10-20, not lots) is slow."
- Issue [**#2451**](https://github.com/bevyengine/bevy/issues/2451) — "Performance degradation over time due to UI" — frame rate "can start at 60 but degrade to 40 after a few seconds, and after a minute or two drop to 20."

These are old issues (2020–2021 vintage) and the situation has improved with subsequent Taffy upgrades + GPU-driven UI work in 0.16. Current performance ceiling: the unofficial Bevy Cheat Book says a "full" iyes_perf_ui overlay "can add a few hundred microseconds of frame time on typical gaming hardware, most of which is CPU time spent in Bevy's UI layout systems (in PostUpdate)." This is fine for game-UI sizes (~10s-100s of nodes) but **not validated at productivity-app sizes** (1000s of nodes — Buiy's target per foundation README goal 6 "Game and app, both").

I could not find a published bevy_ui benchmark at 1000+ nodes. **This is a real gap in our research.**

## cart's own published views on bevy_ui limitations

In discussion **#14437** ("Bevy's Next Generation Scene/UI System") cart writes: "Bevy users will learn exactly one data model for *everything*, making it easier to 'learn Bevy'. Cognitive load will be reduced across the board." This is the *positive* framing — but it also implies a critique of the current state: the data model is *not* yet unified across scenes and UI, which is why the BSN refactor is being undertaken.

On bundles: "They are an additional object-defining concept, which must be learned separately from components. Notably, Bundles are not present at runtime, which is confusing and limiting." This is cart's own self-critique of the bundle pattern that bevy_ui used through 0.14.

On `Construct`: "Some components require `World` state to be constructed, which prevents them from being initialized using parameterless constructors like `Default`." A subtle but pointed critique of the Required Components design as it landed in 0.15 — Required Components need `Default`, but a11y, picking, and theme components often need world context (asset handles, OS-pref resources). `Construct` is the proposed fix; not yet landed.

## Sources

- Issue #9381 non-rect clipping — `https://github.com/bevyengine/bevy/issues/9381`.
- Issue #13093 rounded-corner clipping bug — `https://github.com/bevyengine/bevy/issues/13093`.
- Issue #11100 10 Challenges — `https://github.com/bevyengine/bevy/issues/11100`.
- Issue #17644 bevy_a11y BSN-unfriendly — `https://github.com/bevyengine/bevy/issues/17644`.
- Issue #677 deep-nested UI perf — `https://github.com/bevyengine/bevy/issues/677`.
- Issue #276 many-children perf — `https://github.com/bevyengine/bevy/issues/276`.
- Issue #2451 UI perf degradation — `https://github.com/bevyengine/bevy/issues/2451`.
- Discussion #14437 BSN tracking — `https://github.com/bevyengine/bevy/discussions/14437`.
- bevy_lunex README — `https://github.com/bytestring-net/bevy_lunex`.
- woodpecker_ui README — `https://github.com/StarArawn/woodpecker_ui`.
- "How do Nice UI in Bevy?!?" (deadmoney.gg) — `https://deadmoney.gg/news/articles/how-do-nice-ui-in-bevy` (HTTP 403 at fetch time; cited for future revision).
- Bevy Cheat Book performance notes — `https://bevy-cheatbook.github.io/pitfalls/performance.html`.
- iyes_perf_ui — `https://github.com/IyesGames/iyes_perf_ui`.
