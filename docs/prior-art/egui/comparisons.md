**Date:** 2026-05-22
**Status:** active
**Subject:** egui — design-axis comparisons against Slint, Iced, Dioxus, Druid/Xilem, Floem, GPUI, Dear ImGui, and Buiy

# Comparisons

Two-axis framing: **paradigm (immediate vs retained vs reactive-retained)** × **scope (dev tools vs production app UI)**.

## Summary matrix

| Project | Paradigm | Backing | Primary niche | Renderer | a11y |
|---|---|---|---|---|---|
| egui | Immediate | Rerun.io | Dev tools, Rerun, dashboards | epaint + wgpu/glow | AccessKit (always-on 0.34) |
| Slint | Retained (DSL) | SixtyFPS GmbH | Embedded + commercial apps | Own renderer; femtovg, OpenGL, software | AT-SPI / UIA / NSAccessibility (own bridges) |
| Iced | Retained (Elm-like) | System76 (COSMIC) | Polished desktop apps | wgpu + cosmic-text | AccessKit (recent) |
| Dioxus | Retained (React-like VDOM) | DioxusLabs | Web-first, fullstack | Web view / wgpu (Blitz) | DOM a11y (web) / experimental (native) |
| Druid → Xilem | Retained → reactive-retained | Linebender | Research → production | Vello (GPU) | AccessKit |
| Floem | Reactive retained (signals) | Lapce community | Reactive desktop apps | wgpu | AccessKit |
| GPUI | Retained (custom) | Zed Industries | Zed editor only | Custom Metal/wgpu | Custom (Zed-internal) |
| Dear ImGui (C++) | Immediate | Omar Cornut + community | Dev tools (industry standard) | Own (OpenGL/DX/Metal) | Limited (community efforts) |
| **Buiy** | Retained (ECS + BSN) | (in development) | Bevy-native, web-platform-parity, AccessKit-first | Bevy render graph | AccessKit-first by design |

## vs Slint

Slint is retained-mode with a declarative `.slint` DSL — a designer-facing language that compiles to optimized Rust. SixtyFPS GmbH (commercial steward) targets embedded systems first, commercial desktop apps second. Slint has real designer tooling (a VSCode extension + a separate visual editor) and a serious focus on shipping polished apps.

**Key design difference vs egui.** Slint front-loads the cost of a DSL + retained-mode component model and rewards it with hot-reloadable design + per-component a11y wiring + branded visuals at low marginal cost. egui front-loads no DSL cost and rewards "write a tool in 30 minutes" but pays it back in the homogeneity + a11y-floor-not-ceiling story ([critiques.md](critiques.md)).

**Versus Buiy.** Buiy is closer to Slint in scope-ambition (production apps, branded visuals, full a11y) but rejects DSL-first authoring — BSN is Bevy-native, not a separate language. Buiy gets the retained benefits without the DSL adoption cost; Slint gets the visual-editor designer-handoff benefits Buiy doesn't try to provide.

## vs Iced

Iced is the closest Rust-ecosystem comparator to Buiy's substrate choices: retained-mode, built on wgpu, uses cosmic-text for text, has a real focus on polished apps. The Elm architecture (`Message` + `update` + `view`) is the structuring principle. System76 ships COSMIC Desktop applications on Iced — the most prominent retained-mode-Rust-UI deployment in the wild.

**Key design difference vs egui.** Iced is the "what egui would look like if it were retained-mode" choice — same Rust-substrate aesthetic, same eframe-class portability story, but with an Elm-architecture state model + retained tree + better-conditioned-for-distinctiveness theming.

**Versus Buiy.** Iced's `wgpu + cosmic-text + AccessKit` substrate is **the same substrate Buiy uses**. The divergence is integration model: Iced is a framework you write your app in; Buiy is a UI layer for Bevy apps. Bevy users who want polished retained-mode UI today reach for Iced (outside Bevy) or bevy_ui (inside Bevy); Buiy positions itself as the Bevy-inside option Iced can't be.

## vs Dioxus

Dioxus is React-like Rust: hooks (`use_state`, `use_effect`), a virtual DOM, components-as-functions. Targets web (WASM) first, desktop (WebView via Tauri-shape or Blitz wgpu-renderer), mobile (experimental), and a fullstack story (`dioxus-fullstack`).

**Key design difference vs egui.** Dioxus is **web-developer-shaped**; egui is **systems-developer-shaped**. Dioxus is the right answer if you're building a web app in Rust because you want Rust-end-to-end; egui is the right answer if you're building a Rust dev tool and the UI is incidental.

**Versus Buiy.** Buiy and Dioxus are both web-platform-parity-aware but for different reasons — Dioxus targets the web platform literally (DOM, browser APIs); Buiy targets the web platform's *feature set* (ARIA, layout, theming, complex text) on top of Bevy. The component model shapes are different: Dioxus is React/hooks; Buiy is BSN over Bevy ECS.

## vs Druid → Xilem (Linebender stack)

Druid was Linebender's retained-mode Rust UI experiment (Raph Levien, ~2019-2023). Xilem is its evolution — reactive-retained, "view trees" rebuilt cheaply, optimized for the `Vello` GPU renderer + `Parley` text. The Linebender stack (Vello + Parley + Xilem + Masonry) is Google/Adobe-funded research aiming at production retained-mode for the long-haul.

**Key design difference vs egui.** Xilem represents the **other end** of the design space from egui: GPU-first rendering (Vello vs egui's CPU tessellator), parley for text (vs egui's skrifa+vello_cpu CPU path), explicit reactive primitives. The bet is "retained is the right primitive, the cost is the cost." egui's counter-bet is "immediate is good enough for 90% of Rust UI workloads."

**Versus Buiy.** Buiy and Xilem share retained-mode commitment, share the AccessKit-first commitment, share the wgpu-rendering substrate. They diverge on **integration target**: Xilem is standalone-app-shaped; Buiy is Bevy-shaped. Buiy could in principle integrate parley + Vello but currently uses cosmic-text — see [`prior-art/cosmic-text/`](../cosmic-text/) for that thread.

## vs Floem

Floem is signal-based reactive-retained Rust UI, used by the Lapce editor. Signals are the primitive (think SolidJS, Leptos); the framework re-runs reactive scopes when signals change. Built on wgpu.

**Key design difference vs egui.** Floem makes **reactive state** first-class; egui makes **stateless rebuild** first-class. Floem's authoring shape ("button with a signal-bound label") looks closer to modern web frameworks; egui's looks closer to Dear ImGui.

**Versus Buiy.** Buiy's foundation spec (section 5 open questions) lists "signal-style reactivity" as a follow-up sub-spec, not v1. Floem represents the design space if Buiy chose signals over ECS-observers for reactivity. Both can work; Buiy bets on Bevy's existing observers + change detection as the reactivity primitive.

## vs GPUI

GPUI is Zed Industries' custom retained-mode UI framework powering the Zed editor. Closed-source historically; open-sourced 2024. Custom paradigm (not exactly Elm, not exactly React) — closer to a hand-rolled retained-immediate hybrid optimized for Zed's specific perf needs.

**Key design difference vs egui.** GPUI is **one-app's-framework-extracted**; egui is **general-purpose framework**. GPUI's API surface is shaped by Zed's editor workload (sub-pixel-perfect text rendering, low-latency input, custom 3D-text-like effects). Outside Zed, almost nobody uses GPUI.

**Versus Buiy.** Both are retained, both target a specific consumer app surface — but Buiy is general (any Bevy app) while GPUI is single-product. Worth keeping in mind that Zed shipping on GPUI is the highest-profile retained-mode-Rust-UI deployment, and it explicitly rejected egui as inadequate for Zed's needs.

**Cross-correction:** Zed does *not* use egui (common misconception; see [ecosystem.md](ecosystem.md)).

## vs Dear ImGui (C++ progenitor)

Dear ImGui (Omar Cornut, 2014) is the C++ immediate-mode-GUI library egui's API consciously echoes. Same `ui.button("Hi")` API shape, same hash-based-Id system, same rebuild-every-frame model.

**Key design difference vs egui.** Dear ImGui is **C++ + FFI**; egui is **pure Rust**. Same paradigm; different ecosystem cost. Dear ImGui has the substantially longer track record (Unity/Unreal editor mods, AAA studio internal tooling, scientific software, 3D content tools, NASA mission control panels) and broader battle-testing. egui is the younger, growing, Rust-native incarnation.

**Versus Buiy.** Buiy is not the Rust answer to Dear ImGui — bevy_egui (and egui itself) is. Buiy occupies a different cell: retained-mode-for-Bevy-production-UI. The two coexist comfortably (you can use bevy_egui for dev tools and Buiy for game UI in the same app).

## vs Buiy (the framing-disclosure-aware comparison)

Buiy is a parallel UI stack for Bevy, retained-mode, ECS + BSN-authored, integrating Taffy / cosmic-text / AccessKit / bevy_picking directly, with web-platform-parity as the comprehensive-feature goal and WCAG 2.2 AA as the accessibility floor.

| Axis | egui | Buiy |
|---|---|---|
| Paradigm | Immediate (rebuild every frame) | Retained (ECS components persist across frames) |
| Authoring | Method-chained `ui.button(...)` | Bevy ECS + BSN (component graph + spawning) |
| Layout | egui's own simple solver (multipass since 0.29) | Taffy (Flexbox + Grid future) |
| Text | skrifa + vello_cpu (CPU, hint, variable fonts; 0.34+) | cosmic-text (HarfBuzz via rustybuzz) |
| a11y | AccessKit-integrated (always-on 0.34) | AccessKit-first by design, BSN-decomposed for tree shape |
| Theming | `Style`/`Visuals` (flat, ad-hoc) | Semantic-token system (cascading, OS-pref-bound, variants) |
| Component model | None (functions calling functions) | First-class (small, public-fielded, observable ECS components) |
| State persistence | Manual `Memory::data` per-widget | Automatic (components are persistent ECS data) |
| Custom widgets | `impl Widget` (light) + state-mgmt (heavy) | Compose decomposed components |
| Renderer | Own (epaint tessellator + wgpu/glow backends) | Bevy render graph (own passes, parallel to bevy_ui) |
| Window model | Multi-viewport (0.24+) | Bevy windows |
| Integration | Standalone (eframe) + bridges (bevy_egui, etc.) | Bevy-native (parallel-to-bevy_ui) |
| WASM | First-class (eframe) | Open question (foundation spec § open questions) |
| Mobile | Workable (rough) | Open question (foundation spec § open questions) |

**Different scope, different problem.** egui solves "ship a Rust UI in 30 minutes that works everywhere." Buiy solves "ship a production Bevy UI with web-platform-parity and full a11y." Both can be right; both can ship in the same app. **bevy_egui is the bridge for the egui-in-Bevy use case (dev tools, debug overlays), not a competitor to Buiy.**

## Sources

- Slint — https://slint.dev
- Iced — https://iced.rs ; corpus at `prior-art/iced/`
- Dioxus — https://dioxuslabs.com
- Linebender (Druid/Xilem/Vello/Parley/Masonry) — https://linebender.org
- Floem — https://github.com/lapce/floem
- GPUI — https://www.gpui.rs
- Dear ImGui — https://github.com/ocornut/imgui
- bevy_egui comparisons (cross-link) — `prior-art/bevy-egui/comparisons.md`
- Buiy foundation spec — `docs/specs/2026-05-07-buiy-foundation/README.md`
