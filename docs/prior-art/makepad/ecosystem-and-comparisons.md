**Date:** 2026-05-22
**Status:** active
**Subject:** Makepad — production users (Makepad Studio + Robrix), Project Robius community, and side-by-side comparisons vs Slint / Dioxus / egui / Iced / Bevy / Buiy

# Ecosystem and comparisons

## Production users (what's actually shipping on Makepad)

### Makepad Studio — the canonical Makepad app

The IDE is itself a Makepad application. Dogfooded by the core team. Capabilities (per repo `makepad-studio/` and screenshots):

- Code editor (multi-buffer, syntax highlighting, custom font rendering).
- File tree, split-pane layout, dock system.
- Live Preview of `.live` files — edit in one pane, see the result render in another.
- Inline shader editing — the `pixel` function in a Live block compiles and applies live.
- Designed to host AI-assisted authoring (per the 2026-era README).

Build: `cargo run -p makepad-studio --release`.

This is **the most important production user** for Makepad — the framework's existence is justified by the IDE that uses it. Compare with Slint, where the IDE (VS Code + `slint-lsp`) is *not* a Slint app, so Slint can't dogfood the same way; or with Bevy, where there is no canonical Bevy-built IDE.

### Robrix — the most-cited Makepad downstream app

Matrix chat client built on Makepad by **Project Robius / Kevin Boos**. v1.0.0-alpha.1 released **2026-05-05**. 448 GitHub stars; 59 forks; 3,078 commits.

What works (per Robrix README, as of v1.0.0-alpha.1):

- Room and timeline viewing.
- Message editing and reactions.
- Image and multimedia support (via Matrix's Sliding Sync API).
- End-to-end encryption (Matrix-standard).
- User profiles and avatars.
- Offline functionality with persistent caching.
- Room search and discovery.
- Direct messaging views.
- Spaces support.
- Read receipts.
- Backwards pagination.

Platforms (verified shipping): macOS, Linux, Windows, Android, iOS, iPadOS. OpenHarmony builds but doesn't run.

Known issues (from Robrix README):

- Matrix.to links not fully handled in-app.
- Ignoring users clears timelines.
- Android geolocation permissions friction.
- Requires Matrix homeservers with native Sliding Sync support.

**Conference visibility:** Robrix has been the Makepad public-relations vehicle at:

- Rust China Conf 2025 — "Robrix: a complex, multi-platform app in Rust for secure chat using Matrix"
- GOSIM China 2024 — Multi-platform Matrix client presentation
- GOSIM Europe 2024 — Chat client and broader applications discussion
- Matrix Live interview

Robrix's existence is the strongest argument that **Makepad scales to a real, complex, mobile-and-desktop production application**. Without Robrix, Makepad's adoption story is harder to make.

### Other Robius-community Makepad apps

- **`makepad_wechat`** (28 stars) — WeChat-like messaging UI port. Demonstration / learning artifact.
- **`makepad_wonderous`** (4 stars) — Port of the Wonderous Flutter showcase app to Makepad. Demonstration of cross-framework visual fidelity.

These are smaller / experimental and do not represent additional production deployments. They are valuable as *visible* Makepad apps but not as adoption proof.

### Beyond Robius

No other widely-cited Makepad applications are visible from public search. The Makepad README links Discord + the project social channels but does not maintain a list of third-party apps. Adoption is concentrated in: (a) the Makepad core team's own work (Studio), and (b) the Robius community's work (Robrix + ports).

## Comparisons

Side-by-side of Makepad versus the Rust UI alternatives Buiy designers will encounter. Each row collapses subtle distinctions to a one-cell verdict; consult the linked prior-art folders for nuance.

| Aspect | **Makepad** | **Slint** | **Dioxus** | **egui** | **Iced** | **Bevy UI** | **Buiy** |
|---|---|---|---|---|---|---|---|
| Authoring model | Live DSL + Rust glue | `.slint` DSL + Rust glue | RSX (JSX-like in Rust) + components | Immediate-mode Rust | Elm (`message + update + view`) | ECS spawn (+ BSN in 0.18+) | **ECS spawn + BSN (both first-class)** |
| GPU substrate | Direct Metal / DX11 / OpenGL / WebGL | Skia / FemtoVG / Qt / Software | wgpu (via Blitz) | wgpu | wgpu | wgpu (via Bevy render graph) | **wgpu (via Bevy render graph)** |
| Layout | Custom Live-driven | Custom Slint-driven | Custom (Taffy under Blitz) | Custom | Custom | Taffy | **Taffy (direct)** |
| Text shaping | Custom Makepad font stack | cosmic-text (since 1.7) | Web-text engines | egui-native | cosmic-text | cosmic-text | **cosmic-text (direct)** |
| Accessibility | **None (no AccessKit; issue #196 open since 2023-08)** | AccessKit (since 1.1; pin drift) | Limited (some AccessKit work, dependent on platform) | AccessKit (enabled in `eframe` since ~0.27) | Draft PRs only (no merged AccessKit) | bevy_a11y (BSN-hostile, megacomponent) | **AccessKit-first, decomposed components** |
| Mobile | iOS / Android / tvOS / OpenHarmony (Robrix ships) | Android (since 1.5), iOS (since 1.10) — experimental | Mobile via Tauri-like wrappers | Limited | Limited | Bevy mobile (general) | **Staged (per Buiy foundation accessibility.md)** |
| Hot-reload | Yes (incl. shaders) | Yes (Live Preview) | Yes (RSX hot-reload) | N/A (immediate-mode rebuilds every frame) | N/A (similar) | BSN hot-reload (in 0.18+) | **Planned (BSN integration sub-spec)** |
| LSP / editor | Makepad Studio only | `slint-lsp` (editor-agnostic) | `rust-analyzer` (RSX is Rust) | `rust-analyzer` | `rust-analyzer` | `rust-analyzer` | **`rust-analyzer` + BSN tooling** |
| License | MIT OR Apache-2.0 | GPL-3 OR royalty-free OR commercial | MIT OR Apache-2.0 | MIT | MIT | MIT OR Apache-2.0 | **MIT OR Apache-2.0** |
| Game-engine integration | Standalone | Standalone | Standalone | Often embedded in game engines | Standalone | **Bevy-native** | **Bevy-native (parallel to bevy_ui)** |
| Lifetime downloads | 16,974 | 1.1M+ | Multi-million | Multi-million | Multi-million | (Bevy itself) | — |
| Maturity at folder-write | 1.0 (May 2025) | 1.16.1 (April 2026) | 0.6 (production) | 0.31+ (production) | 0.13 (production) | (in Bevy main) | spec phase |

### Versus Slint specifically

These are the two production Rust UI DSL toolkits at 1.0+. The comparison matters for the "should Buiy ship a DSL?" question.

| | Slint | Makepad |
|---|---|---|
| DSL filename | `.slint` | `.live` (or embedded `live_design!`) |
| DSL property direction | Explicit (`in`, `out`, `in-out`) | Implicit (Rust `#[live]` / `#[rust]`) |
| Hot-reload | Live Preview (values + layout) | Live source + shader hot-reload |
| Accessibility | AccessKit since 1.1 | None |
| Mobile | Beta (Android 1.5, iOS 1.10) | Production (Robrix ships) |
| GPU | Skia / FemtoVG / Qt / software | Direct Metal / DX11 / OpenGL / WebGL |
| Text | cosmic-text | Custom (no BiDi / complex shaping) |
| Embedded MCU | Yes (bare-metal STM32, RGB565) | Not in scope |
| License | GPL+commercial gate | MIT/Apache permissive |
| Editor | `slint-lsp` (editor-agnostic) | Makepad Studio only |
| Bus factor | SixtyFPS GmbH (~5–10 employees) | 3 architects + Robius community |

**Reading.** Slint has the better accessibility / text / IDE story; Makepad has the better mobile / hot-reload / shader story. **Neither solves the Buiy problem set** (web-platform parity + WCAG 2.2 AA + Bevy ECS integration + BSN authoring). Buiy is solving a different problem and should read both as inputs, not blueprints.

### Versus Dioxus

Dioxus is the closest comparator on "Rust UI with familiar declarative authoring." RSX-in-Rust beats DSLs on editor / refactor ergonomics. But Dioxus is web-tech-shaped under the hood (DOM virtualization, web renderers like Blitz) — closer in spirit to React-in-Rust. Makepad and Dioxus are *not* competing for the same use case: Makepad is GPU-native; Dioxus is DOM-translation-shaped.

### Versus egui

egui is immediate-mode — every frame rebuilds the UI tree from scratch. Wrong for productivity apps (text editing, IME, focus continuity), great for tooling and game-internal UI. Makepad and egui occupy non-overlapping niches.

### Versus Iced

Iced is Elm-architectured: message → update → view. Best-in-class API ergonomics for state-machine UIs but no AccessKit yet (draft PRs only). Comparable mobile coverage (Limited). Doesn't compete with Makepad's mobile-first story.

### Versus Bevy UI

Bevy's built-in `bevy_ui` is ECS-native (matches what Buiy parallels). Smaller widget catalog. Accessibility via `bevy_a11y` is BSN-hostile (the lesson source — bevy issue #17644). The Buiy → Makepad comparison is "different framework, same DSL-above-runtime question"; the Buiy → bevy_ui comparison is "same framework, different posture on widget catalog scope."

### Versus Buiy

Different design center. Makepad targets standalone-Rust-UI; Buiy targets Bevy-game-engine-UI with web-platform parity and WCAG 2.2 AA conformance. The *DSL question* (should Buiy ship a DSL above ECS?) is the only place they directly compete in design space — and Buiy's answer (no, ECS+BSN both first-class) explicitly rejects Makepad and Slint's DSL-first model.

## Implications for Buiy

- **Two real downstream consumers are enough to call a UI framework "shipping."** Makepad's adoption is small (16k downloads) but its two shipping consumers (Makepad Studio, Robrix) are both substantial multi-platform applications. The 1.0 marker is justified, even with small download counts. Buiy's similar maturity target should be: *2–3 production-grade Bevy apps using Buiy* — not crate-download counts.
- **Confines of the prior-art comparison matter.** Makepad ≠ Bevy UI alternative. Makepad ≠ Slint substitute. Makepad is its own niche: standalone-Rust + DSL + mobile + custom GPU. Treat the lessons as principles (hot-reload, mobile shipping, GPU-renderer-in-production) not as architectural blueprints.
- **Conference / public-relations visibility matters.** Robrix at Rust China Conf 2025 and GOSIM 2024 is how Makepad reaches new users. Buiy should plan for similar visibility once a real Buiy app exists — RustConf, BevyConf, GOSIM Europe.

## Sources

- Makepad Studio: https://github.com/makepad/makepad/tree/dev/makepad-studio
- Robrix: https://github.com/project-robius/robrix
- Project Robius: https://github.com/project-robius
- `makepad_wechat`: https://github.com/project-robius/makepad_wechat
- `makepad_wonderous`: https://github.com/project-robius/makepad_wonderous
- Comparison sources: [`../slint/`](../slint/), [`../dioxus/`](../dioxus/), [`../iced/`](../iced/), [`../egui/`](../egui/), [`../bevy-ui/`](../bevy-ui/), [`../bevy-feathers/`](../bevy-feathers/), [`../accesskit/ecosystem.md`](../accesskit/ecosystem.md)
- Sibling files: [`README.md`](README.md), [`history.md`](history.md), [`distribution-and-governance.md`](distribution-and-governance.md), [`mobile-targets.md`](mobile-targets.md), [`open-problems.md`](open-problems.md)
- Buiy foundation: [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
