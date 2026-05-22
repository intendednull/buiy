**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_egui — comparison to bevy_ui, bevy_lunex, bevy_feathers, woodpecker_ui, Buiy, imgui, Slint/Iced/Dioxus; the dev-tool-vs-production-UI axis

# Comparisons

bevy_egui occupies a specific slot in the Bevy UI landscape: **third-party, immediate-mode, paradigm-distinct from every retained-mode neighbor**. This file places it row-by-row against its closest contemporaries — in the Bevy ecosystem and in the wider Rust GUI landscape — and against Buiy. Each row is the elevator-pitch difference (2–4 sentences) plus the load-bearing fact a spec author needs.

## At a glance

| UI | Paradigm | Built on | Scope | In-tree? |
|---|---|---|---|---|
| `bevy_egui` | Immediate-mode (egui) | Own render path (bypasses `bevy_ui`) | Dev tools, debug overlays, hobbyist game UI | No, third-party |
| `bevy_ui` | Retained, ECS-native | Bevy core + Taffy + cosmic-text (→ parley 0.19) + bevy_a11y + AccessKit | General | Yes |
| `bevy_lunex` | Retained, `Transform`-based | Own layout + render | Game UI, worldspace UI | No |
| `bevy_feathers` | Retained, opinionated widget kit | `bevy_ui` + `bevy_ui_widgets` | Editor / tooling | Yes, experimental |
| `woodpecker_ui` | Retained, custom declarative | Own runtime | General | No |
| **Buiy** | Retained, web-platform-parity | Taffy + cosmic-text + AccessKit + bevy_picking + Bevy render graph (direct) | Game + app, comprehensive | No, parallel stack |
| Dear ImGui | Immediate-mode (C++ progenitor) | Own | Dev tools (industry standard) | n/a |
| `imgui-rs` | Rust binding for ImGui | C++ ImGui via FFI | Dev tools | n/a |
| Slint | Retained, declarative DSL | Own runtime | App / embedded | n/a |
| Iced | Retained, Elm-architecture | Own + wgpu | App | n/a |
| Dioxus | Retained, React-like VDOM | Own + WebView / WGPU | App / web | n/a |

## vs `bevy_ui` (retained-mode, ECS-native — different paradigm)

`bevy_ui` is Bevy's first-party retained-mode UI: ECS-native, Taffy-backed layout, AccessKit-bridged accessibility, the official substrate for Bevy app UI. Released alongside Bevy 0.4 (2020-09), it has matured through 0.18 (2026-01) — ~5.5 years of development. See [`../bevy-ui/architecture.md`](../bevy-ui/architecture.md).

**Key design difference.** Paradigms don't blend. bevy_egui rebuilds the widget tree every frame from imperative calls; `bevy_ui` stores widgets as ECS entities with components and updates them on change. Hit-testing, focus, accessibility, picking, layout — none of these are shared between the two systems. They can coexist in the same `App` but **cannot share a window** in any meaningful sense — bevy_egui paints on top of (or under, since 0.36) bevy_ui without integration. For Buiy: the comparison reinforces that **a single window must commit to one UI paradigm**; mixing them costs more than it saves.

## vs `bevy_lunex` (transform-based retained-mode — different paradigm)

`bevy_lunex` is a third-party retained-mode UI that does *not* build on `bevy_ui`. It uses `Transform`-based positioning (UI is just a 2D entity tree) with explicit pixel / percent / relative containers and renders via the standard 2D sprite pipeline. It targets game UI explicitly — HUDs, menus, animated UI, worldspace / diegetic UI.

**Key design difference.** bevy_lunex and bevy_egui are both third-party Bevy UIs but on opposite sides of the immediate/retained line and opposite sides of the layout-engine choice. bevy_lunex anchors are simpler than Flexbox/Grid (closer to "old-school sprite UI"); bevy_egui's layout is similarly simple but immediate-mode. Neither targets web-platform-parity. bevy_lunex *can* do diegetic UI natively; bevy_egui added mesh-picking worldspace in 0.35 but it's an addition, not the design center.

## vs `bevy_feathers` (official widget kit, retained-mode)

`bevy_feathers` is the Bevy Foundation's in-tree opinionated widget kit, built on `bevy_ui` + `bevy_ui_widgets`. Single dark OKLCH palette, ~14 controls, editor-aimed. Experimental as of Bevy 0.17–0.18. See [`../bevy-feathers/`](../bevy-feathers/).

**Key design difference.** Authority and scope. `bevy_egui` is the *current* de-facto editor / dev-tool UI by ecosystem momentum (bevy-inspector-egui being the canonical example); `bevy_feathers` is the *intended* official replacement on a retained-mode foundation. The Bevy editor roadmap signals that future editor work will be on `bevy_feathers` + `bevy_ui_widgets`, not on bevy_egui — see [`ecosystem.md`](ecosystem.md) § "Bevy editor experiments." For a developer choosing today: bevy_egui has more polish and ecosystem; bevy_feathers has Foundation backing and a clearer continuity story.

## vs `woodpecker_ui` (custom declarative retained-mode)

`woodpecker_ui` (StarArawn, successor to ideas in the archived `kayak_ui`) is a retained-mode declarative widget tree with its own runtime, not built on `bevy_ui`. JSX-style composition through Rust macros, event-driven update model.

**Key design difference.** Both bevy_egui and woodpecker bypass `bevy_ui` and ship their own systems, but for opposite ergonomics. bevy_egui is imperative ("write the code that draws the UI this frame"); woodpecker is declarative ("describe the tree that exists and let the runtime reconcile"). Woodpecker re-implements layout, rendering, event handling inside its subsystem; bevy_egui borrows egui upstream's. Woodpecker has lower adoption than bevy_egui; kayak_ui's archival in 2024 ([`../bevy-feathers/comparisons.md`](../bevy-feathers/comparisons.md) § "vs kayak_ui") is the cautionary tale for the "custom-declarative-runtime" approach.

## vs Buiy (parallel-stack web-platform-parity retained-mode AccessKit-first)

Buiy occupies a slot bevy_egui does not target. The axis-by-axis comparison:

| Axis | bevy_egui | Buiy |
|---|---|---|
| **Paradigm** | Immediate-mode | Retained-mode, ECS-native |
| **Built on** | egui upstream + Bevy render graph | Taffy + cosmic-text + AccessKit + bevy_picking + Bevy render graph (direct) |
| **Layout** | egui's simple flow / Grid widget | Full Taffy: Flexbox + CSS Grid + block + absolute + (named lines, subgrid coming) |
| **Theming** | `Visuals` struct (flat, no tokens, no cascade) | Multi-variant tokens; OS-preference-driven; subtree overrides; hot-reloadable; contrast linter |
| **Accessibility** | AccessKit (opt-in, 0.38+); implicit tree from per-frame calls | Direct AccessKit; decomposed `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations`; WCAG 2.2 AA per-widget mapping |
| **Animation** | Minimal (basic interpolate; no keyframes/springs/layout-transitions/reduced-motion gate) | Transitions, keyframes, layout transitions, springs, reduced-motion-gated |
| **Renderer features** | egui's tessellator (rounded rects, shapes, custom paint callbacks) | Owns render pipeline: rounded clipping, `clip-path`, `backdrop-filter`, `mix-blend-mode`, isolation, true top layer, gradients in any color space |
| **Custom widgets** | Hard — verbose `Memory`/`Id` state, low-level paint callback for visuals | Standard pattern — spawn an entity with the standard components |
| **Touch / mobile** | Working but rough (virtual keyboard 0.30+, hit-target sizing not tuned) | Targets web-platform-parity (24×24 minimum target size, gesture recognizers planned) |
| **Multi-window** | Yes (since 0.4, 2021); strong | Yes (per-window adapters, single architecture) |
| **WASM** | Strong (egui upstream is heavily WASM-exercised) | Targeted, deferred until AccessKit web adapter ships |
| **License** | MIT only | (TBD — likely MIT-OR-Apache-2.0 to match Bevy) |
| **Bus factor** | 1 (vladbat00) | 1 (intendednull) — same risk shape |

The diverging design bets Buiy makes vs bevy_egui:

1. **Retained-mode, not immediate-mode.** Buiy is built around long-lived entities and change-detection; bevy_egui rebuilds every frame. The retained-mode bet pays off in a11y (stable IDs), animation (state across frames), and performance at scale (only changed widgets pay the per-frame cost).
2. **AccessKit-first, not AccessKit-opt-in.** Buiy bakes AccessKit into the component model from day one; bevy_egui added it as an opt-in feature five years into the project.
3. **Web-platform-parity feature catalog.** Buiy targets the modern-web UI feature set (anchor positioning, container queries, top layer, the popover state machine, full APG widget catalog); bevy_egui's catalog matches egui upstream's, which is dev-tool-oriented.
4. **Game and app, both.** Buiy commits to both production game UI and productivity-app UI; bevy_egui's adoption shows it dominates dev tools and struggles at production game UI (see [`critiques.md`](critiques.md) § "Dev tool, not production UI").

## vs Dear ImGui / `imgui-rs` (C++ immediate-mode progenitor)

Dear ImGui (Omar Cornut, 2014) is the C++ immediate-mode-GUI library that egui's API consciously echoes. `imgui-rs` is a Rust binding to the C++ ImGui via FFI.

**Key design difference.** egui is **pure Rust** with no FFI; ImGui requires a C++ toolchain (or vendored binary blobs) and ABI care. The same paradigm shape, but a different Rust-ecosystem cost. ImGui has a much longer track record in shipped tools (3D content tools, game-engine editors, scientific software, AAA studio internal tooling), while egui is younger but growing in the same niche. For Bevy specifically there are also `imgui-rs` Bevy bindings, but bevy_egui has won the ecosystem battle decisively — egui is the de-facto immediate-mode UI in Rust.

## vs Slint / Iced / Dioxus (separate prior-art folders to come)

These are the major Rust app-UI competitors outside the Bevy ecosystem; full prior-art folders should land in Wave 4. Brief framing:

**Slint** — declarative `.slint` DSL, retained-mode, designed for embedded + desktop apps. Commercial backing (SixtyFPS GmbH). Has a real designer-facing tool. Targets app-UI seriously, not dev-tool-UI. The cleanest comparator for "what production retained-mode Rust UI looks like."

**Iced** — Elm-architecture retained-mode in pure Rust. Built on `wgpu` and cosmic-text. Used in COSMIC Desktop's apps (System76). Strong on web (via WASM) and desktop. The closest Rust-ecosystem comparator to Buiy's "retained + cosmic-text + wgpu" substrate stack — many of Buiy's substrate choices match Iced's, with the divergence being Buiy's ECS-native integration vs Iced's framework-style runtime.

**Dioxus** — React-like VDOM in Rust, targets web / desktop (WebView) / native (`Blitz`, wgpu-based). Component model is closest to web React; component lifecycle is virtual-DOM-style.

None of these target Bevy or worldspace UI. bevy_egui's overlap with them is narrow — it's a game-engine UI plugin, not a general Rust app framework.

## The dev-tool vs production-UI axis

The single most useful axis for placing bevy_egui:

```
                  PRODUCTION GAME UI                    DEV-TOOL / DEBUG UI
                  (player-facing, polished, animated)   (developer-facing, fast iteration)
                  ──────────────────────────────────    ───────────────────────────────
   Bevy ecosystem  bevy_lunex                            bevy_egui ★ (canonical)
                   bevy_feathers (intended)              bevy_feathers (editor)
                   bevy_ui                               bevy-inspector-egui
                   Buiy (intended)
                  ──────────────────────────────────    ───────────────────────────────
   Wider Rust      Slint                                 Dear ImGui ★
                   Iced                                  imgui-rs
                   Dioxus                                egui (general)
```

bevy_egui sits firmly in the **right column**: dev tools, debug overlays, internal-facing UI. Its closest paradigm cousin (Dear ImGui) dominates exactly the same niche in C++ land. The "production game UI" cell is where bevy_egui is absent — see [`open-problems.md`](open-problems.md) § "The production game UI gap" and [`critiques.md`](critiques.md) § "Dev tool, not production UI."

Buiy targets the left column explicitly. It is **not competing with bevy_egui** on the dev-tool axis — bevy_egui's strengths there (fast iteration, low API surface, reflection-driven inspectors via bevy-inspector-egui) are genuine and Buiy doesn't try to displace them. The two systems would coexist in many real-world projects: bevy_egui for dev overlays and editor panels, Buiy for shipped player-facing UI. Buiy's per-window coexistence policy ([`../../specs/2026-05-07-buiy-foundation/`](../../specs/2026-05-07-buiy-foundation/) cross-cutting.md § 3.18) accommodates this.

## Sources

- bevy_egui — `https://github.com/vladbat00/bevy_egui`.
- egui — `https://github.com/emilk/egui`.
- bevy_ui — `https://github.com/bevyengine/bevy/tree/main/crates/bevy_ui`; [`../bevy-ui/`](../bevy-ui/).
- bevy_lunex — `https://github.com/bytestring-net/bevy_lunex`.
- bevy_feathers — [`../bevy-feathers/`](../bevy-feathers/).
- woodpecker_ui — `https://github.com/StarArawn/woodpecker_ui`.
- kayak_ui (archived 2024) — `https://github.com/StarArawn/kayak_ui`.
- Dear ImGui — `https://github.com/ocornut/imgui`.
- imgui-rs — `https://github.com/imgui-rs/imgui-rs`.
- Slint — `https://slint.dev`.
- Iced — `https://iced.rs`.
- Dioxus — `https://dioxuslabs.com`.
- COSMIC Desktop (Iced consumer) — `https://system76.com/cosmic`.
- Bevy 0.17 release notes (editor direction) — `https://bevy.org/news/bevy-0-17/`.
- Sibling files: [`distribution.md`](distribution.md), [`history.md`](history.md), [`ecosystem.md`](ecosystem.md), [`critiques.md`](critiques.md), [`open-problems.md`](open-problems.md).
- [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md).
