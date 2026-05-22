**Date:** 2026-05-22
**Status:** active
**Subject:** Freya — ecosystem position and comparisons

# Ecosystem and comparisons

Freya sits in a narrow niche: **Dioxus-flavored desktop UI on Skia**. This file places Freya against the rest of the Rust GUI landscape, with particular attention to what makes Freya the closest existing-art for several of Buiy's concerns even though Buiy's substrate is entirely different.

## Production users (verified)

**None confirmed at the time of this corpus.** The Freya website has no production-user showcase; no notable apps publicly attribute themselves to Freya in 2026-05. The 33,720 lifetime downloads and 3,768 recent downloads are consistent with **experimentation / hobby use** rather than production deployment.

Searching the GitHub topic tag and crate reverse-deps:

- Several Freya-built **example apps** (file managers, calculators, todo apps) shipped by community members. None enterprise.
- No native app, editor, or game on a major platform's app store credits Freya in its tech stack.

Contrast with:

- **Slint:** OTIV (rail), KDAB (Qt consulting), Espressif partner, STMicroelectronics partner.
- **Iced:** pop-os (System76's COSMIC apps), Halloy IRC, multiple smaller productivity apps.
- **egui:** Rerun (CV/robotics tooling), Mullvad VPN GUI, many internal tools.
- **GPUI:** Zed editor (the framework's whole reason for existing).
- **Dioxus:** Various smaller web apps + sponsor companies.

Freya's adoption gap is the largest weakness identified in this corpus. The framework is *technically capable* of building a real app; the *track record* of doing so is missing.

## The Rust GUI landscape — where Freya sits

| Framework | Substrate | Reactivity | Layout | Text | Mature? | Production users |
|---|---|---|---|---|---|---|
| **Freya** | Skia | Dioxus signals | Torin | Skia textlayout | Pre-1.0 | None confirmed |
| **Slint** | own (sw + WGPU + Qt + femtovg backends) | Property bindings | own | own | 1.x stable | OTIV, KDAB, Espressif |
| **Iced** | wgpu + tiny-skia (sw fallback) | TEA | own | cosmic-text (until recent) | 0.x mature | pop-os, Halloy |
| **egui** | wgpu (and many backends) | Immediate mode | own (immediate) | own (basic) + epaint glyph | 0.x mature | Rerun, Mullvad |
| **Dioxus core** | (multiple renderers) | Dioxus signals | varies | varies | 0.7.x | YC, sponsors |
| **Blitz (Dioxus native)** | WGPU + Vello | Dioxus signals | Taffy | Parley | Pre-alpha | None |
| **Floem** | (Vger / wgpu) | own signals | Taffy | Parley | 0.2.x | Lapce |
| **GPUI** | Metal/wgpu (Zed-internal) | own | own | own | Internal-only | Zed |
| **Makepad** | own DSL + own backends | DSL bindings | own | own | 0.x | Robrix, some demos |
| **Vello (renderer-only)** | wgpu | n/a — it's a renderer | n/a | n/a (uses Parley) | 0.x | Bevy ecosystem |
| **Buiy** | wgpu via Bevy render graph | Bevy observers + change detection | Taffy | cosmic-text | Foundation | n/a (in design) |

The most-similar pair to **Freya** is **Floem** — both are Skia-or-Skia-adjacent renderers driven by signals on top of Taffy/Torin layout and Parley/Skia text. Both are pre-1.0. Both have one or two flagship users. The key Floem/Freya axis split:

- Floem uses **Parley + Vger** (pure Rust); Freya uses **Skia + Skia textlayout** (C++).
- Floem has its own signal library; Freya re-uses Dioxus signals.
- Floem's flagship is Lapce (an editor); Freya has no flagship.

## Vs Dioxus core / Blitz

Dioxus core is the *upstream*; Freya is a **community-maintained alternative renderer** for Dioxus, parallel to DioxusLabs's own renderers (web, desktop-webview, native/Blitz, mobile). The comparison axes:

- **Freya is a fourth-party Dioxus renderer** sitting in the same niche as Blitz but pre-dating Blitz's current form.
- **Freya is more mature than Blitz** at the renderer level — Blitz is *"pre-alpha by the authors' own admission"* per [`../dioxus/open-problems.md`](../dioxus/open-problems.md); Freya is at least *"shipping daily, pre-1.0."*
- **Blitz uses Stylo + Parley + Taffy + Vello** (the post-Servo Linebender stack); Freya uses **Skia + Torin**.

DioxusLabs has not pushed Freya into the official Dioxus monorepo, signalling that the project is firmly community-maintained and unlikely to be absorbed.

## Vs Iced (Elm vs Dioxus signals)

The structural difference is the **reactivity paradigm**:

- **Iced** uses the **Elm Architecture** (TEA) — `Message` enum + `update()` + `view()` + `subscription()`. Pure, strict, predictable. State-management is the user's responsibility outside the model struct.
- **Freya** uses **Dioxus signals** — fine-grained reactivity, component-scoped state. State management distributes through hooks + Stores.

Each is the canonical example of its paradigm in modern Rust GUI:

- TEA: Iced.
- Fine-grained signals: Dioxus → (Freya, Floem).
- Immediate mode: egui.
- ECS: Bevy UI + Buiy.
- Property bindings (Qt-style): Slint.

Buiy's foundation [§ 2.7](../../specs/2026-05-07-buiy-foundation/architecture.md#27-reactivity) does **observers + change detection only** for v1 — a fifth distinct paradigm. The choice of paradigm is the load-bearing decision for any GUI framework's authoring story.

## Vs Slint (DSL vs `rsx!`)

- **Slint** is **DSL-first** — `.slint` files compiled to native by the Slint compiler.
- **Freya** is **macro-first** — `rsx!` is a Rust proc-macro; everything is normal Rust source code.

Slint pays a build-time cost (the Slint compiler) for a hot-reload + cross-language story (Rust/C++/JS/Python all generate from the same `.slint`). Freya is Rust-only but has no separate compile step beyond Cargo.

Both ship a **CSS-flavored styling surface**. Both produce AccessKit trees. Slint is post-1.0 stable; Freya is not.

## Vs GPUI (Skia vs custom)

GPUI is **Zed's internal framework**, designed and used in production by one team. Architectural axes:

- **GPUI is custom Metal/wgpu, not Skia.** No C++ dependency. Built from scratch for Zed's specific needs.
- **GPUI uses its own reactive model**, not Dioxus.
- **GPUI is hyper-optimized for code-editor workloads** — text rendering, large scrolling lists, vim mode, multi-cursor.
- **GPUI's adoption is "Zed and nothing else."**

GPUI is the proof-point that **a custom GPU-backed UI framework can deliver production quality without depending on Skia or any C++ substrate** — closer to Buiy's wgpu commitment than Freya's Skia commitment. GPUI itself is not separable from Zed (no published crate) so cannot be a Buiy dependency, but its existence validates Buiy's substrate bet.

## Vs Buiy (wgpu vs Skia)

The two-line summary: **Freya** = Dioxus signals + Skia + Torin + AccessKit, desktop-only. **Buiy** = Bevy ECS + Bevy-render-graph-on-wgpu + Taffy + cosmic-text + AccessKit, Bevy's platform matrix.

Where they agree:

- **AccessKit-first** for native a11y. Same dependency.
- **Token-friendly theming** (Freya implicitly via `use_theme`, Buiy explicitly via tokens).
- **Same architectural primitives at the top of the stack** (focus, animation, themes, layout, text, render).

Where they diverge:

- **Substrate.** wgpu vs Skia. The C++ vs pure-Rust tradeoff dominates.
- **Reactivity model.** Bevy observers vs Dioxus signals. Foundation [§ 2.7](../../specs/2026-05-07-buiy-foundation/architecture.md#27-reactivity).
- **Layout.** Taffy vs Torin. Foundation [§ 2.2](../../specs/2026-05-07-buiy-foundation/architecture.md#22-underlying-primitives-buiy-integrates-directly).
- **Text.** cosmic-text vs Skia textlayout. See [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md).
- **Authoring.** BSN (typed components) vs `rsx!` (stringly-typed attrs). Foundation [§ 2.4](../../specs/2026-05-07-buiy-foundation/architecture.md#24-authoring-ecs-native-and-bsn-both-first-class).
- **Platform reach.** Desktop-only vs Bevy's full matrix.
- **Ecosystem coupling.** Freya is married to Dioxus's release cadence; Buiy is married to Bevy's.

## Sources

- crates.io stats — https://crates.io/crates/freya
- Freya site — https://freyaui.dev/
- Iced — https://github.com/iced-rs/iced
- Slint — https://github.com/slint-ui/slint
- egui — https://github.com/emilk/egui
- Dioxus — https://github.com/DioxusLabs/dioxus
- Blitz — https://github.com/DioxusLabs/blitz
- Floem — https://github.com/lapce/floem
- GPUI — https://github.com/zed-industries/zed/tree/main/crates/gpui
- Makepad — https://github.com/makepad/makepad
- Vello — https://github.com/linebender/vello
- Cross-references: [`../dioxus/ecosystem.md`](../dioxus/ecosystem.md), [`../dioxus/targets.md`](../dioxus/targets.md), [`../iced/ecosystem.md`](../iced/ecosystem.md), [`../slint/ecosystem-and-comparisons.md`](../slint/ecosystem-and-comparisons.md), [`../egui/ecosystem.md`](../egui/ecosystem.md) (if present), [`lessons.md`](lessons.md).
- Buiy foundation — [`README.md`](../../specs/2026-05-07-buiy-foundation/README.md), [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md).
