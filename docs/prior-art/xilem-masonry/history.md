**Date:** 2026-05-22
**Status:** active
**Subject:** Druid → Masonry → Xilem timeline — the seven-year evolution of Linebender's UI strategy

# History

This file traces the long arc from Druid (2018, Raph Levien at Google) through the Xilem paper (2022) and the Masonry split (2023+) to the co-released 0.4.0 alignment (2025-10-29). The arc matters because it's a documented multi-year strategy from one organization, and the design rationale at each transition tells us why pre-1.0 substrate moves are not random churn — they're the working-out of architectural commitments.

## 2018 — Druid begins (at Google)

Raph Levien starts Druid as a side project at Google. The original mission: a Rust GUI toolkit oriented toward design / fonts work. Druid is built on:

- **piet** — abstract 2D rendering API (Raph's design, predates Vello).
- **druid-shell** — windowing layer (predates winit's UI focus).
- **piet-common** + platform piets (cairo, direct2d, web-canvas) — backends.

Druid is OOP-flavored, with `Widget<Data>` trait, `Lens<Data, Sub>` for state projection, `WidgetPod<Widget>` for tree storage. The Lens pattern becomes the conceptual ancestor of Xilem's `Adapt` views.

## 2020-09-25 — "Towards principled reactive UI" (Raph)

Raph publishes ["Towards principled reactive UI"](https://raphlinus.github.io/rust/druid/2020/09/25/principled-reactive-ui.html). The post catalogues reactive UI architectures (Elm, React, SwiftUI, immediate-mode, signals, observables) and argues none of them quite work for Rust without ownership-friendly modifications. This is the start of the public design exploration that becomes Xilem.

## 2020-09-28 — "Rust 2021: GUI" (Raph)

["Rust 2021: GUI"](https://raphlinus.github.io/rust/gui/2020/09/28/rust-2021.html) — Raph's annual reflection. Identifies that Druid is hitting design ceilings around state-management ergonomics and async integration.

## 2022-05-07 — The Xilem paper

Raph publishes ["Xilem: an architecture for UI in Rust"](https://raphlinus.github.io/rust/gui/2022/05/07/ui-architecture.html). This is the design document for the post-Druid future:

- View trees as pure functions of state.
- Id-path-based message routing.
- `Adapt` views as the lensing successor.
- Memoization via `PartialEq` on `Data`.
- Incremental computation engine specialized for UI.

The paper is *conceptual*; no implementation yet. The crate `xilem` doesn't exist on crates.io.

## 2022-07-15 — "Advice for the next dozen Rust GUIs" (Raph)

["Advice for the next dozen Rust GUIs"](https://raphlinus.github.io/rust/gui/2022/07/15/next-dozen-guis.html) — Raph's lessons-learned post. Hard-won advice: text is harder than you think, accessibility is harder than you think, layout is harder than you think. Use existing substrate (Skia / harfrust / AccessKit) rather than building your own. This is the framing that Linebender's unbundled-substrate strategy executes against.

## 2023-02-28 — Druid 0.8.3 (last release)

The final Druid release. Around this time, Raph and contributors signal the strategic shift: Druid is in maintenance mode, Xilem is the future. The Druid README is updated to say "UNMAINTAINED - The Druid project has been discontinued." Repository is not formally `archived` on GitHub, but development stops.

## 2023-05-07 — Xilem 0.1.0 (first release)

The first Xilem crate hits crates.io. Initial implementation of the paper's ideas: view trees, diffing, Masonry-backed paint. Masonry exists at this point but is internal-shaped — version numbers are inconsistent (masonry-v0.2.0 ships alongside xilem v0.1.0, reflecting Masonry's earlier internal life as part of Druid before being lifted out).

The README calls both crates "experimental."

## 2023–2024 — "This Month in Xilem" blog posts

Monthly progress posts on Linebender's blog (mostly Daniel McNab, Raph Levien, Olivier Faure as authors). The posts document:

- API refinement on the view-trait surface.
- Async integration via `tokio`.
- Multi-window support.
- Text editing primitives.
- Accessibility integration via AccessKit (`Widget::accessibility` method shape).
- Vello / Parley integrations stabilizing.

The cadence is **monthly through 2024, then trails off** — the last "This Month in Xilem" appears to be August 2024 per the Linebender blog page. The lack of post-2024 monthly posts is notable; the project hasn't gone silent (0.4.0 ships 2025-10-29) but the blog cadence has dropped.

## 2024-05 — Tree arena refactor

`tree_arena` is split into its own crate (v0.1.0, 2025-05-10 per git tag — but this is the date the unified workspace tag landed, not the underlying refactor). Masonry's tree storage becomes pluggable between safe and unsafe-with-`UnsafeCell` implementations.

## 2025-05-11 — Xilem 0.3.0

Major cumulative release. Roughly a year after 0.1.0 with 500+ merged PRs. AccessKit pin lands in the 0.2x range. MSRV bumps to Rust 1.86. The release is positioned as **not yet stable** but increasingly usable.

## 2025-10-29 — Xilem + Masonry 0.4.0 (the sibling alignment)

Both crates ship at 0.4.0 simultaneously — the first time Xilem and Masonry have been version-aligned at a meaningful zero-point version. The release notes:

- Styling properties as a first-class concept on widgets.
- New crate split: `masonry_core` / `masonry_testing` / `masonry_winit` formalized.
- Multi-window support stabilized in `masonry_winit`.
- New widgets: slider, blinking text cursor.
- Layer system.
- Improved keyboard navigation.
- Updated to wgpu 26 (matching Bevy 0.17).
- Updated to AccessKit 0.21.1 (released crate; workspace HEAD has since moved to 0.24.0).
- MSRV bumps to Rust 1.88 (workspace HEAD already at 1.92).
- Placehero example added — a Mastodon client built in Xilem, used as the showcase non-trivial app.
- Release notes mention "we plan to start keeping a changelog after this release" — meaning no formal CHANGELOG exists *before* 0.4.0.

This is the **closest Xilem/Masonry have come to a production-ready posture** — though still labeled experimental.

## Pattern: the long arc

The full timeline:

- **2018 Q4** — Druid begins.
- **2020 Q3** — "Towards principled reactive UI" — paper-form articulation of Druid's limitations.
- **2022 Q2** — Xilem paper.
- **2023 Q1** — Druid declared unmaintained.
- **2023 Q2** — Xilem 0.1.0 ships.
- **2025 Q2** — Xilem 0.3.0 (one year of catch-up work).
- **2025 Q4** — Xilem + Masonry 0.4.0 sibling release.

That's **seven years from Druid's start to the 0.4.0 alignment**, with a **three-year gap between the Xilem paper and the first 0.4.0 sibling-aligned release**. This is consistent with Raph's known pace; the lesson for Buiy is that Linebender ships slowly but with high architectural quality, not quickly. Counting on Xilem/Masonry to hit 1.0 in the next 12 months is not a safe planning assumption.

## Pattern: the second-system trap (avoided here)

Druid → Xilem is the second-system rewrite that *succeeded* on architectural grounds — the paper's ideas pay off — but at a high time cost. Worth contrasting with kayak_ui → woodpecker_ui (see [`../woodpecker-ui/history.md`](../woodpecker-ui/history.md) and `lessons.md`): same shape (one author rewrites their own framework), but woodpecker_ui's adoption is 17× *smaller* than kayak_ui's even with the architectural improvements. Linebender's case differs because the substrate crates (Vello, Parley) get adopted *outside* their own framework, so the rewrite's worth is amortized across the ecosystem even if Xilem itself stays small.

**Lesson for Buiy:** ship the primitives so they're usable independent of the framework. Don't make Buiy's value contingent on Buiy-the-framework's adoption — make the substrate (own text pipeline, render passes, a11y components) directly studyable and quotably stable. The Linebender model says: even if your top-layer framework stays small, the substrate has independent value.

## Cross-link to AccessKit lineage

AccessKit (Matt Campbell) is Linebender-adjacent — not a Linebender crate but closely associated through Raph and the broader accessibility-in-Rust effort. The AccessKit history (Matt Campbell at Pneuma Solutions; lineage *not* through NVDA per [`../accesskit/lessons.md`](../accesskit/lessons.md) Avoid row "Misattributing AccessKit lineage to NVDA") is independent of Linebender but cooperates closely.

## Sources

- Druid repo: https://github.com/linebender/druid
- Xilem paper (Raph Levien, 2022-05-07): https://raphlinus.github.io/rust/gui/2022/05/07/ui-architecture.html
- Raph's blog index: https://raphlinus.github.io/
- "Advice for the next dozen Rust GUIs" (2022-07-15): https://raphlinus.github.io/rust/gui/2022/07/15/next-dozen-guis.html
- "Towards principled reactive UI" (2020-09-25): https://raphlinus.github.io/rust/druid/2020/09/25/principled-reactive-ui.html
- Xilem release tags: https://github.com/linebender/xilem/tags
- Linebender blog ("This Month in Xilem" series, 2024): https://linebender.org/blog/
- Cross-link: [`../woodpecker-ui/history.md`](../woodpecker-ui/history.md) for the second-system-trap comparison.
- Cross-link: [`../accesskit/`](../accesskit/) for the AccessKit history thread.
