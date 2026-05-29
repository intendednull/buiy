**Date:** 2026-05-22
**Status:** active
**Subject:** iced — head-to-head vs other Rust GUI libraries and vs Buiy

# Comparisons

This file places iced next to its closest Rust-GUI neighbours and against Buiy itself. Each row is a 2–4 sentence summary plus the one design difference that matters most. Companion to [`ecosystem.md`](ecosystem.md) (where iced sits in the broader landscape) and [`critiques.md`](critiques.md) (iced's specific limits).

## vs `egui`

| Aspect | iced | egui |
|---|---|---|
| Paradigm | Retained, Elm Architecture | Immediate-mode |
| Adoption | 1.88M crate downloads (lifetime) | ~3M crate downloads (lifetime) |
| Theming | Function-based `Theme` trait; rich | Minimal style; flat color/font knobs |
| Performance at scale | Better at large retained UIs | Better at small / dynamic UIs |
| WASM | Limited, via wgpu+WebGL | First-class via `eframe` |
| Mobile | Limited | Limited |
| Accessibility | None (issue #552 open since 2020) | Limited; some AccessKit integration via eframe |

Summary: egui is the most-downloaded Rust GUI library overall; its immediate-mode model wins for dev-tools and embedded inspectors (Rerun, bevy-inspector-egui, Tauri devtools). iced wins for retained desktop apps where the Model is a first-class durable concept (COSMIC, Cryptowatch). **The key design difference: state ownership.** egui doesn't own application state — your render function reads it and emits widget calls. iced owns application state via Model + Message and updates only on user-emitted Messages.

## vs Slint

| Aspect | iced | Slint |
|---|---|---|
| Paradigm | Retained, Rust-only | Retained, `.slint` DSL + Rust glue |
| License | MIT | GPL-3 / royalty-free / commercial (triple) |
| Target | Cross-platform desktop | Embedded systems (priority) + desktop |
| DSL | None ("everything in plain Rust") | `.slint` is mandatory |
| Mobile | Limited | First-class iOS + Android |
| Tooling | Devtools in 0.14 | LSP, VSCode plugin, live preview, Figma plugin |
| Governance | Single maintainer (Héctor) | SixtyFPS GmbH, commercial company |

Summary: Slint is the only Rust GUI with credible mobile and embedded stories. Its DSL means designers can edit the UI without Rust skills; iced rejects that path on principle. **The key design difference: DSL acceptance.** iced's philosophy chapter explicitly rejects DSLs: *"You will write everything in plain Rust"*. Slint embraces the DSL as the artifact a designer-and-engineer share.

## vs Dioxus

| Aspect | iced | Dioxus |
|---|---|---|
| Paradigm | Retained, Elm Architecture | Retained, React-flavored signals |
| Targets | Desktop + Web (limited) | Desktop (via WebView), Web (DOM), Mobile (via WebView) |
| Render | Native via wgpu + cosmic-text | DOM (web) / WebView (desktop & mobile) |
| Component model | `Element<Message>` tree, plain enums | RSX macro, reactive hooks |
| State | Single Model | Per-component hooks + signals + contexts |
| Hot reload | 0.14 (`hot` feature) | Long-supported |

Summary: Dioxus is the Rust GUI library closest to React's developer experience, and the only one with a credible full-stack (frontend + backend + bundler) story. iced commits to native rendering throughout; Dioxus offloads rendering to the browser/WebView. **The key design difference: render path.** iced ships a complete Rust+wgpu render stack; Dioxus uses HTML/CSS/JS as its render target on the platforms where they're available.

## vs Druid / Xilem (Linebender)

| Aspect | iced | Xilem |
|---|---|---|
| Paradigm | Retained, Elm Architecture | Retained, signal-based reactive |
| Adoption | 1.88M downloads | Pre-1.0, low downloads |
| Backing | Single maintainer + Kraken sponsorship | Microsoft Research + Google funding via Linebender Foundation |
| Text engine | cosmic-text (via cryoglyph) | Parley + Vello |
| Layout | Own engine | `masonry` (Linebender's own) |
| Production users | COSMIC, Cryptowatch, many smaller apps | Pre-production; targeted as Druid's successor |

Druid (deprecated as of ~2024) was Linebender's earlier attempt; Xilem is the successor. Linebender's substrate (Vello, Parley, Skrifa) is parallel to iced's (wgpu, cosmic-text, swash) — both are Rust-native, both production-grade for text, neither will converge. **The key design difference: text engine.** iced commits to cosmic-text; Linebender commits to Parley. Both engines exist, both ship, both are mutually-incompatible-but-architecturally-similar choices.

## vs Floem (Lapce)

| Aspect | iced | Floem |
|---|---|---|
| Paradigm | Retained, Elm Architecture | Retained, signal-based reactive |
| Adoption | 1.88M downloads | ~tens of thousands (driven mostly by Lapce) |
| Text engine | cosmic-text (via cryoglyph) | Parley |
| State | Single Model | Signal-graph (`floem-reactive`) |
| Flagship user | COSMIC | Lapce (code editor) |

Summary: Floem powers the [Lapce](https://github.com/lapce/lapce) code editor. Its signal-based reactive model is closer to SolidJS than to Elm. **The key design difference: reactivity granularity.** iced re-runs `view(&model)` on every dirty pass and diffs the resulting `Element` tree; Floem maintains a signal graph and only invalidates the leaf subscribers that observed a changed signal. The Floem model is more performant for fine-grained reactivity but harder to reason about for global state changes.

## vs GPUI (Zed)

| Aspect | iced | GPUI |
|---|---|---|
| Paradigm | Retained, Elm Architecture | Retained, hierarchical view tree |
| Publishing | crates.io | Not published; lives inside Zed monorepo |
| Render | wgpu + cosmic-text | Custom GPU pipeline + custom text shaping |
| Target | Cross-platform desktop | Mac (best), Linux + Windows (added 2024+) |
| Adoption | 1.88M downloads | ~0 external (Zed only) |
| License | MIT | Apache-2.0 OR GPL-3.0 (Zed's license; GPUI is not published separately) |

Summary: GPUI is Zed's custom UI framework, designed for one application. It's not a general-purpose option. iced is the closest commercially-supported, publicly-available alternative for "I want a GPU-first retained Rust GUI." **The key design difference: public surface.** iced ships a stable public API and semver releases; GPUI is internal-to-Zed and breaks freely.

## vs GTK-rs

| Aspect | iced | GTK-rs |
|---|---|---|
| Paradigm | Retained, Elm Architecture | Native widget bindings (GObject) |
| Render | wgpu + cosmic-text (own pipeline) | GTK's native rendering (Cairo/GSK) |
| Theming | iced's `Theme` trait | OS-themed (GTK CSS-like rules) |
| Platform | Cross-platform (Win/Mac/Linux/Web) | Linux-best, macOS/Windows via GTK-on-X port |
| Accessibility | None | First-class via AT-SPI |
| Generation | Pre-immediate-mode generation, 2010-era widget kit | 2010s GTK runtime, bindings 2018+ |

Summary: GTK-rs is the bridge to a mature OS-native widget kit. It inherits GTK's accessibility, IME, and OS-integration story for free. iced inherits none of that but renders consistently across platforms. **The key design difference: native vs uniform rendering.** GTK-rs uses the platform's native widgets and theming; iced uses one rendering pipeline everywhere.

## vs Buiy

| Aspect | iced | Buiy |
|---|---|---|
| Host | Standalone app (owns the window + event loop via winit) | Plug-in for Bevy (Bevy owns window + event loop) |
| Component model | Elm: Model + Message + update + view | ECS + BSN: per-entity decomposed components |
| Layout engine | iced's own (Flexbox-flavored row/column) | Taffy (CSS Flexbox + Grid + Block) |
| Text engine | cosmic-text (via cryoglyph) | cosmic-text (direct integration) |
| Render | iced_wgpu + iced_tiny_skia | Bevy's render graph (wgpu-based, shared with Bevy 3D) |
| Accessibility | None | AccessKit-first (foundation goal 2: WCAG 2.2 AA floor) |
| State | Single Model | ECS (many entities, decomposed components) |
| Mobile / WASM | Limited / Limited | Wherever Bevy targets (Android, iOS, WASM all supported via Bevy) |
| License | MIT | Inherits Bevy's `MIT OR Apache-2.0` ecosystem |
| Animation | 0.14 Animation API, no CSS transitions | Foundation [interaction.md § 3.7](../../specs/2026-05-07-buiy-foundation/interaction.md) — full spec planned |

Summary: iced and Buiy occupy non-overlapping niches. iced is for cross-platform desktop apps built in pure Rust; Buiy is for Bevy game/app UI that needs web-platform feature parity + WCAG 2.2 AA. The substrate overlap (cosmic-text, wgpu) is real but the host (winit vs Bevy) is different. **The key design difference: host engine.** iced *owns* the host; Buiy *plugs into* the host (Bevy).

`bevy_iced` is the existence proof that iced apps can run inside Bevy as embedded UI, but the bridge runs two layout engines + two text caches + two event-handler chains. Buiy's parallel-stack approach (foundation [architecture.md § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md)) keeps the integration costs in the Buiy stack itself.

## Where iced sits in the rectangle

If you plot Rust GUI libraries on (axis 1: state-model granularity, axis 2: renderer ownership):

- **Single Model, owns renderer:** iced.
- **Single Model, hosts in something else:** (none well-established).
- **Signal graph, owns renderer:** Floem, Xilem.
- **Signal graph, hosts in something else:** Dioxus (hosts in WebView/DOM).
- **Immediate-mode (no Model), owns renderer:** egui.
- **No state model, native widgets:** GTK-rs.
- **ECS, plugs into game engine:** Buiy, bevy_ui.

iced's quadrant is fully its own. It is not "egui but retained" (different state model) or "Xilem but Elm" (different reactivity). It is the only large-adoption Rust GUI library with single-Model + own-renderer + cross-platform-desktop.

## Sources

- iced philosophy chapter — https://book.iced.rs/philosophy.html
- egui repo — https://github.com/emilk/egui
- Slint — https://slint.dev
- Dioxus — https://dioxuslabs.com
- Xilem — https://github.com/linebender/xilem
- Floem — https://github.com/lapce/floem
- GPUI (in Zed monorepo) — https://github.com/zed-industries/zed/tree/main/crates/gpui
- GTK-rs — https://gtk-rs.org
- bevy_iced — https://github.com/tasgon/bevy_iced
- Linebender Foundation — https://linebender.org
