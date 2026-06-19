**Date:** 2026-05-22
**Status:** active
**Subject:** Iced — the most-adopted retained-mode Rust GUI library; Elm-architecture; cosmic-text via cryoglyph; flagship at-scale user is System76's COSMIC desktop

# Iced

[`iced`](https://github.com/iced-rs/iced) is the most-adopted retained-mode Rust GUI library — 1,885,134 lifetime downloads on crates.io as of 2026-05-22, latest stable `0.14.0` published 2025-12-07. It is the canonical Rust implementation of **The Elm Architecture** (Model + Message + Update + View), bet on `wgpu` for GPU rendering before any peer, and shipped on **cosmic-text via the [`cryoglyph`](https://github.com/iced-rs/cryoglyph) fork of `glyphon`** since the text-engine overhaul in 0.10. System76's [COSMIC desktop](https://github.com/pop-os/cosmic-epoch) — every COSMIC app from `cosmic-files` to `cosmic-settings` to `cosmic-terminal` — is the flagship at-scale deployment.

For Buiy, Iced is the strongest empirical signal that **cosmic-text scales to production retained-mode UI outside Bevy**. Buiy and Iced share the cosmic-text substrate (validates Buiy's commitment, especially given Bevy `main`'s 0.19-dev migration to Parley + swash per issue [#21765](https://github.com/bevyengine/bevy/issues/21765)). Buiy and Iced diverge on every other axis that matters: layout engine (Buiy = Taffy; Iced = its own hand-rolled flex algorithm in `iced_core::layout`), accessibility (Buiy = AccessKit-first from day one; Iced = absent since 2020), state model (Buiy = ECS + BSN; Iced = single global Model), and renderer (Buiy = Bevy's wgpu render graph; Iced = `iced_wgpu` + `iced_tiny_skia` end-to-end).

## Honest assessment

- **COSMIC desktop is the only at-scale flagship.** System76's commercial bet on Iced for Pop!_OS COSMIC is the load-bearing signal of long-term viability. The other production users in `awesome-iced` (Halloy IRC, Sniffnet, Cryptowatch desktop, Veloren menus, ajour, ludusavi) are real but order-of-magnitude smaller. No published commercial deployment outside System76 has shipped at COSMIC's surface area.
- **AccessKit has been absent for 5.5+ years.** Issue [#552](https://github.com/iced-rs/iced/issues/552) ("Implement accessibility support") opened 2020-10-05; no PR has a credible landing path. The COSMIC team reportedly built bespoke AccessKit wiring inside `libcosmic` that has not been upstreamed in any form Héctor has accepted. This is the single largest structural gap — it makes Iced unviable for any productivity app with screen-reader requirements and is the gap Buiy's foundation goal 2 (WCAG 2.2 AA floor) refuses to repeat.
- **Owning the renderer + owning the layout engine = doubled maintenance.** `iced_wgpu` chases `wgpu`'s pre-1.0 API churn (22.0 → 27.0 across the 0.14 cycle); the bespoke flex layout in `core/src/layout/flex.rs` caps at Flexbox-flavored row/column (no CSS Grid algorithm, no anchor positioning, no subgrid). The `cryoglyph` fork of `glyphon` (March 2025) commits Iced to also maintaining the cosmic-text → wgpu adapter. Each is defensible in isolation; together they make Iced's surface area unusually large for a single-architect project.
- **Elm-architecture is divisive at scale.** Production reports from COSMIC, Halloy, and Cryptowatch say the single global `Model` + central `Message` enum start producing maintenance friction at 200+ Message variants. Iced's recommended mitigation (component-level decomposition with `Element::map` plumbing) is verbose. ECS-native authoring (Buiy + bevy_ui) trades different costs but doesn't accumulate this one.
- **Kraken/Cryptowatch sponsorship gives runway, not a roadmap.** The Cryptowatch desktop application is iced's most commercially-significant non-COSMIC user and Kraken funds Héctor's development (per the README's Sponsors section). Funding has not translated into release-cadence predictability (the 0.13 → 0.14 gap was 14 months) or a published RFC process.
- **Single-architect bus factor.** Héctor Ramón ([hecrj](https://github.com/hecrj)) is the only person with commit on `iced-rs/iced`. Pattern is structurally identical to Bevy-cart and to System76 / Jeremy Soller (`jackpot51`) for cosmic-text; Iced's scale plus COSMIC backing makes it bearable for now.

## Key facts (verified 2026-05-22)

| Fact | Value |
|---|---|
| Crate | `iced` |
| Latest stable | **0.14.0** (published 2025-12-07; `master` is 0.15.0-dev) |
| License | **MIT** (single — not `MIT OR Apache-2.0`) |
| MSRV | rust-version 1.88, edition 2024 (master = 1.92) |
| Lifetime downloads | 1,885,134 |
| 90-day downloads | 358,674 |
| Repository | https://github.com/iced-rs/iced |
| Lead architect | Héctor Ramón (`hecrj`) — founder, BDFL, sole committer |
| Primary sponsor | Cryptowatch team at [Kraken.com](https://kraken.com) (per README) |
| Paradigm | Retained-mode with **The Elm Architecture** (Model + Message + Update + View) |
| Renderer | Own (`iced_wgpu` GPU + `iced_tiny_skia` CPU fallback); NOT Bevy's render graph, NOT vello |
| Text engine | **cosmic-text via `cryoglyph`** (Iced's March 2025 fork of `glyphon`); NOT Parley |
| Layout engine | Own (hand-rolled flex in `iced_core::layout::flex`, "inspired by druid"); NOT Taffy |
| Window/event | `winit 0.30` (same as Bevy 0.15+) |
| Flagship at-scale user | System76 **COSMIC desktop** (Wayland compositor + ~7 apps) |
| AccessKit integration | **ABSENT** — issue [#552](https://github.com/iced-rs/iced/issues/552) open since 2020-10-05 |
| Multi-window | Landed 0.12.0, 2024-02-15 (no 0.11.x release) |
| First crates.io publish | 2019-05-29 (`0.0.0` placeholder); first real release 2020-04-02 (`0.1.0`) |

## Contents

| File | Subject |
|---|---|
| [`README.md`](README.md) | This file — overview, honest assessment, key facts, ToC, framing disclosure. |
| [`lessons.md`](lessons.md) | **The consult-this-when-designing decision file.** Validates / Avoid / Borrow. |
| [`glossary.md`](glossary.md) | System-specific terms used across the corpus. |
| [`architecture.md`](architecture.md) | Runtime topology, Element tree, renderer stack, layout engine, text pipeline, system layer. |
| [`elm-architecture.md`](elm-architecture.md) | The Model + Message + Update + View pattern in Rust; `Task<Message>` for side effects; component decomposition at scale; comparison to ECS. |
| [`widgets-and-styling.md`](widgets-and-styling.md) | Built-in widget catalog (0.14), custom widgets via the `Widget` trait, function-based styling, themes, animation primitives. |
| [`layout-engine.md`](layout-engine.md) | The bespoke flex engine in `iced_core::layout`, why Iced didn't pick Taffy, comparison to Buiy's Taffy bet. |
| [`text-and-cosmic.md`](text-and-cosmic.md) | Iced's cosmic-text adoption (since 0.10, via `cryoglyph` since March 2025); IME support added 0.14; BiDi via `unicode-bidi`; the brief-correction on Parley. |
| [`history.md`](history.md) | Version-by-version timeline (0.1 → 0.14) including the text-engine switch in 0.10, the COSMIC era beginning 0.5, multi-window in 0.12, the testing/devtools/animation 0.14 release. |
| [`distribution.md`](distribution.md) | Workspace crates, Cargo features (0.14.0), platform support, MSRV, release cadence. |
| [`governance.md`](governance.md) | Lead architect, organization layout, contributors, funding (Kraken + GitHub Sponsors), license posture, RFC/decision process. |
| [`ecosystem.md`](ecosystem.md) | COSMIC desktop, other production users (Halloy, Cryptowatch, Veloren, Sniffnet), community widget libraries (`iced_aw`, `iced_audio`, `cosmic-time`), place in the Rust GUI landscape. |
| [`comparisons.md`](comparisons.md) | Head-to-head vs egui, Slint, Dioxus, Druid/Xilem, Floem, GPUI (Zed), GTK-rs, and Buiy. |
| [`critiques.md`](critiques.md) | No CSS cascade, no Grid layout, no AccessKit, Elm-architecture verbosity at scale, single state tree, limited WASM/mobile, owns-renderer maintenance cost, release-cadence drift, maintainer review style, single-maintainer bus-factor. |
| [`open-problems.md`](open-problems.md) | Forward-looking gaps: AccessKit, mobile, WASM completeness, Grid/advanced layout, animation/transitions, multi-window depth, WCAG coverage, drag-and-drop, touch/gamepad, Model size limits, the Parley question (will Iced migrate? — currently No), theme tokenization. |

## How to use this prior-art doc

1. **If you are designing a Buiy feature that touches text**, read [`text-and-cosmic.md`](text-and-cosmic.md) and the corresponding rows in [`lessons.md`](lessons.md). Iced is the strongest independent confirmation that cosmic-text scales to production retained-mode UI; cross-link to [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) § Validates.
2. **If you are auditing the AccessKit-first commitment**, read [`open-problems.md`](open-problems.md) § "AccessKit integration" and [`critiques.md`](critiques.md) § "No native AccessKit integration." Iced is the cautionary tale, not the model — five years without AccessKit, with known production demand, and no credible PR. Buiy's foundation [accessibility.md](../../specs/2026-05-07-buiy-foundation/accessibility.md) starts where Iced stopped.
3. **If you are evaluating layout engine choices**, read [`layout-engine.md`](layout-engine.md). Iced's bespoke flex is the existence proof that a narrow widget set can ship at scale without Taffy; the constraints (no CSS Grid algorithm, no anchor, no subgrid, no writing-mode awareness) are exactly the constraints Buiy commits to *exceeding*.
4. **If you are studying state-management and async-effect patterns**, read [`elm-architecture.md`](elm-architecture.md). `Task<Message>` (renamed from `Command<Message>` in 0.13) is the cleanest async-effect descriptor in Rust GUI — Buiy can study it as a reference even though Buiy is ECS-native.
5. **If you are planning the widget catalog or styling surface**, read [`widgets-and-styling.md`](widgets-and-styling.md). Iced's ~40 built-in widgets cover most WAI-ARIA APG patterns minus the productivity-app pieces (tree, virtualized list, true modal, drag-and-drop). Function-based styling is type-safe but not a token system; libcosmic's add-on layer above Iced demonstrates the gap.
6. **If you are tracking what shipped when**, start at [`history.md`](history.md) and [`distribution.md`](distribution.md).

## Framing disclosure

These docs are written from a **"Buiy is a parallel UI library for the Bevy game engine + Taffy + cosmic-text + AccessKit-first"** stance. Most "Implications for Buiy" sub-sections frame Iced's choices through that lens: Iced's cosmic-text bet validates Buiy's (Validates); Iced's bespoke layout engine is exactly what Taffy avoids (Avoid); Iced's missing AccessKit is the gap Buiy refuses to repeat (Avoid); Iced's `Task<Message>` and `Subscription` patterns are studyable references (Borrow) even though Buiy's ECS model isn't Elm-architecture-shaped.

Iced and Buiy share the cosmic-text substrate (and therefore harfrust + swash + fontdb + unicode-bidi) and depend on `winit` transitively (Iced directly; Buiy via Bevy). They diverge on layout engine (Taffy vs Iced's own flex), accessibility posture (AccessKit-first vs absent), state model (ECS+BSN vs single Model), renderer ownership (Bevy's wgpu render graph vs `iced_wgpu` + `iced_tiny_skia`), license posture (`MIT OR Apache-2.0` to match Bevy + AccessKit + Taffy + cosmic-text vs single MIT), and host (Bevy-plugin vs standalone-via-winit).

Future readers auditing whether this stance is itself the right primitive should weigh the corpus accordingly. A future reader evaluating "should we build a retained-mode desktop Rust GUI library (not for Bevy)?" should read Iced as the most important paradigm reference. Buiy is the game-engine-bound subset of that question.

A secondary disclosure: Iced is *not* an integration target — it is a peer in the design space. Buiy does not depend on Iced; `bevy_iced` exists as a community embed-iced-in-Bevy bridge but Buiy's foundation [README § Goal 4](../../specs/2026-05-07-buiy-foundation/README.md) explicitly chose the parallel-to-bevy_ui path over a port-iced-to-Bevy path. The corpus may underweight Iced's strengths because Buiy is not consuming it; pressure-test where Iced's experience suggests an approach Buiy hasn't considered (e.g. function-based styling has real ergonomic upsides for app-level code that a strict token system loses).

## Sources

- iced on crates.io — https://crates.io/crates/iced
- crates.io API metadata (fetched 2026-05-22) — https://crates.io/api/v1/crates/iced
- Iced repository — https://github.com/iced-rs/iced
- Iced 0.14.0 release notes — https://github.com/iced-rs/iced/releases/tag/0.14.0
- Iced book — https://book.iced.rs/
- awesome-iced (community user list) — https://github.com/iced-rs/awesome-iced
- COSMIC desktop — https://github.com/pop-os/cosmic-epoch
- libcosmic — https://github.com/pop-os/libcosmic
- cryoglyph (Iced's glyphon fork) — https://github.com/iced-rs/cryoglyph
- cosmic-text — https://github.com/pop-os/cosmic-text
- Iced issue #552 (accessibility) — https://github.com/iced-rs/iced/issues/552
- Bevy issue #21765 (cosmic-text → Parley migration) — https://github.com/bevyengine/bevy/issues/21765
- Buiy foundation README — `docs/specs/2026-05-07-buiy-foundation/README.md`
- Buiy foundation architecture — `docs/specs/2026-05-07-buiy-foundation/architecture.md`
- cosmic-text prior-art folder — [`../cosmic-text/`](../cosmic-text/)
- bevy-ui prior-art folder — [`../bevy-ui/`](../bevy-ui/)
