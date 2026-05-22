**Date:** 2026-05-22
**Status:** active
**Subject:** Xilem + Masonry production adoption (essentially none) + Linebender substrate adoption (significant) + comparison vs Iced, Slint, Dioxus, GPUI, egui, Druid, Buiy

# Ecosystem & comparisons

This file answers two questions in one go: (a) who actually ships Xilem/Masonry today, and (b) how does Xilem/Masonry compare to the other "next-generation Rust UI" candidates in the same design-space neighborhood.

## Production adoption — the honest read

### Xilem (top-layer framework)

**Verified production users of Xilem itself: essentially none, as of folder-write.**

Specifically:

- **Placehero** — Mastodon client built by Daniel McNab, lives in `linebender/xilem` repo as an example. "Early stages" per release notes. Not generally available; not a public product.
- The README lists chess, calculator, to-do MVC as examples. These are demos.
- crates.io downloads: 7,596 lifetime per pre-amble. For comparison: bevy_ui's most-cited solo download counts are 7-figure; Iced is 6-figure; egui is 7-figure. Xilem is **squarely pre-adoption**.
- No flagship third-party app verified.

This isn't a criticism so much as an honest framing: Xilem is **experimental**, says so on the tin, and the adoption shape matches.

### Masonry (lower-level toolkit)

**Verified production users of Masonry: similarly minimal.**

- Xilem (the next layer up) is the main consumer.
- crates.io downloads: 17,690 lifetime (2.3× Xilem). The gap suggests some non-Xilem consumers exist, but no headline third-party adoption is verifiable.

### The substrate (Vello, Parley, Kurbo, Color)

**Verified production / significant adoption:**

- **Vello** — used by `bevy_vello` (which is used by woodpecker_ui, per [`../woodpecker-ui/lessons.md`](../woodpecker-ui/lessons.md) Borrow #1); used by Lapce (code editor) experimentally; standalone demos.
- **Parley** — Bevy 0.19-dev migrated bevy_text to Parley (issue [#21765](https://github.com/bevyengine/bevy/issues/21765)); used in some Iced-experiment branches; cited in GPUI-adjacent work.
- **Kurbo** — *the* Rust 2D-curve library. Used everywhere — Vello, woodpecker_ui transitively, custom Rust graphics tools, font-editing tooling. Six-figure download count.
- **Color** — relatively new; adoption is starting (gradient-spec implementations in browser-adjacent Rust UI).
- **Skrifa / Fontique** — used in Parley + font-tooling; Skrifa is part of `googlefonts/fontations` and has Google-backed adoption.

**This is the load-bearing observation:** Linebender's substrate is **multi-consumer-adopted** even though Linebender's top-layer framework isn't. The unbundled-substrate posture is *validating itself* — the substrate crates outpace the framework in real-world use.

For Buiy this matters because Buiy can *study* the substrate without depending on Xilem/Masonry — and the substrate's stability is established by its non-Linebender consumers (Bevy, woodpecker_ui, Lapce).

## Comparison to peer "next-generation Rust UI" candidates

| Project | Paradigm | Substrate | Maturity | Notable adoption | Buiy stance |
|---|---|---|---|---|---|
| **Xilem/Masonry** | Reactive (Xilem) over retained toolkit (Masonry) | Vello + Parley + AccessKit + winit | Experimental, pre-1.0 | Linebender internal | Reference, not dep |
| **Iced** | Elm-style (functional reactive) | wgpu + cosmic-text + tiny-skia | 0.13+, more mature | Cryptocurrency wallets, some desktop apps | Reference; same cosmic-text choice |
| **Slint** | Declarative DSL (`.slint`) | wgpu + sw renderer + own text | 1.x, commercial-backed | Embedded systems, some desktop | Reference; different paradigm |
| **Dioxus** | React-style (function components, signals) | DOM (web) / Iced+wgpu (desktop) / native (mobile) | 0.5+, growing | Web/desktop hybrid apps | Reference; cross-platform-first posture |
| **GPUI** | Custom (Zed's in-house) | wgpu + own text + skia | Production (Zed) | Zed editor exclusively | Reference; production case study |
| **egui** | Immediate-mode | wgpu + own paint + own text | 0.27+, very mature | Many tools, tracking UIs | Reference; IM is non-overlapping paradigm |
| **Druid (legacy)** | Retained, OOP-flavored | piet (legacy) | Discontinued | Some legacy apps | Historical reference only |
| **bevy_ui** | Retained, ECS | wgpu + cosmic-text → Parley + AccessKit + Taffy + bevy_picking | Production, Bevy-internal | Tiny Glade (with own UI), Foresight, others (some) | Sibling — Buiy is parallel |
| **bevy_feathers** | Widgets-on-bevy_ui | bevy_ui's stack | Pre-stable | None verified | Sibling — Buiy may coexist |
| **woodpecker_ui** | Retained, ECS, dioxus-devtools hot-reload | Vello (via `bevy_vello`) + Parley + Taffy + bevy_picking | Solo author, ~15-month dev cycle, currently dormant | None verified | Reference, validates parallel-to-bevy_ui |
| **Buiy** | Retained, ECS, BSN-friendly | Buiy's own wgpu pipeline + cosmic-text + Taffy + AccessKit + bevy_picking | Pre-foundation | n/a yet | Self |

## Where Xilem/Masonry sit vs Buiy specifically

The **closest peer** to Buiy in the table above is **Xilem/Masonry** for the substrate decomposition shape and **woodpecker_ui** for the Bevy-ecosystem-parallel-to-bevy_ui shape. The two together triangulate Buiy's design space:

- From Linebender: how to ship an unbundled substrate (Vello/Parley/Kurbo each individually viable, framework optional).
- From woodpecker_ui: how to ship a parallel-to-bevy_ui crate that integrates the same `bevy_picking` + Taffy substrate.

Buiy sits at the intersection of these two playbooks: unbundled substrate (foundation [`architecture.md § 2.2`](../../specs/2026-05-07-buiy-foundation/architecture.md)) *and* parallel-to-bevy_ui Bevy integration.

## Per-feature comparison

How does Xilem stack up against the others on the [`#11100 "10 Challenges for Bevy UI Frameworks"`](https://github.com/bevyengine/bevy/discussions/11100) lens?

| Challenge | Xilem/Masonry | bevy_ui | woodpecker_ui | egui | Iced | Buiy (target) |
|---|---|---|---|---|---|---|
| 1. Stable identity / observable state | Via id-paths + Adapt | ECS observers | Hooks + PreviousWidget | Per-frame stack | Per-frame state | ECS observers + change det. |
| 2. Composition w/o leaks | View functions + Adapt | Required components | WidgetChildren + apply | Egui Ui | Component composition | BSN composition |
| 3. Layout flexibility | BoxConstraints (Flutter-style) | Taffy (Flexbox+Grid) | Taffy (Flexbox+Grid) | Manual | Custom | Taffy (Flexbox+Grid+more) |
| 4. Theming | Per-widget setters | Limited | `WoodpeckerStyle` (megacomponent) | Style structs | Theme | Semantic tokens + variants |
| 5. Accessibility | AccessKit-correct | AccessKit + bevy_a11y (megacomponent issue) | **None** | None | AccessKit (recent) | AccessKit-first (decomposed) |
| 6. Hot reload | Not built-in | Limited | dioxus-devtools | None | None | BSN hot-reload (foundation goal) |
| 7. Animation | Minimal | bevy_animation interop | None | Per-frame | Animation type | First-class subspec |
| 8. i18n | Not addressed | Not addressed | Not addressed | Partial | Partial | First-class subspec |
| 9. 3D-anchored UI | Not addressed (app-only) | Limited | Possible via Bevy | None | None | First-class subspec |
| 10. Performance at scale | Untested at 1000+ nodes | Improved in 0.18; benchmark gap | Untested (bevy-trait-query dispatch) | Battle-tested | Untested at scale | Verification harness 1000+ fixtures |

Xilem scores cleanly on (1), (2), (5); has a known gap on (3) Taffy-style layout; doesn't address (6)-(10) yet. Buiy's foundation aims to address all 10 with explicit verification commitments.

## What this means for Buiy's relative positioning

Buiy is **not better than Xilem** today; Buiy is **not even shipped** today. The point of the comparison isn't competition — it's calibration. Specifically:

- Buiy's foundation spec promises *substantially more* (full APG coverage, 60-pattern widget catalog, WCAG 2.2 AA gate, i18n, 3D-anchored, hot-reload, animation, theming, devtools) than any of the comparators ships today. That's an ambitious promise.
- The only project in the comparison set that *can credibly attempt this* is one with a wide bus factor and structured ownership. Linebender's collective + Bevy's foundation are roughly the right shape; solo-author projects (woodpecker_ui, kayak_ui) demonstrably can't deliver to this scope.
- Buiy's verification harness commitment ([`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)) is the load-bearing differentiator: every claim is testable. None of the comparators ships this.

## What Buiy can study from the comparator set

- **From Xilem/Masonry:** unbundled substrate + Widget::accessibility shape + masonry_testing infrastructure.
- **From Iced:** Elm-style reactive model (if Buiy ever adds a reactivity sub-spec); cosmic-text production hardening.
- **From Slint:** commercial-backing model + embedded targets + DSL ergonomics.
- **From Dioxus:** cross-platform reactive sharing + hot-patch tooling (woodpecker_ui borrows this; Buiy will study it via `buiy-bsn-integration-design`).
- **From GPUI:** production-game-editor scale + custom-text-pipeline patterns.
- **From egui:** immediate-mode-is-fine validation for some workloads; tooling-UI dominance.
- **From bevy_ui + bevy_feathers:** the official trajectory Buiy parallels.
- **From woodpecker_ui:** the most-similar third-party precedent on the Bevy side.

## Sources

- crates.io download counts: per pre-amble + spot-checked
- Bevy issue #11100 (10 Challenges): https://github.com/bevyengine/bevy/discussions/11100
- Bevy issue #21765 (Parley migration): https://github.com/bevyengine/bevy/issues/21765
- Cross-link to `../woodpecker-ui/lessons.md`, `../bevy-ui/lessons.md`, `../cosmic-text/lessons.md`, `../accesskit/lessons.md`, `../iced/`, `../dioxus/`, `../slint/`, `../egui/` (those folders exist or will).
