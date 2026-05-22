**Date:** 2026-05-22
**Status:** active
**Subject:** Makepad — Rust UI framework + Live DSL + own GPU renderer; standalone (not bevy_ui-shaped); desktop + mobile + WASM; reached 1.0 in 2025-05

# Makepad

Makepad is a Rust UI framework that shipped 1.0 on 2025-05-13 (crate `makepad-widgets` 1.0.0). It pairs a custom DSL — the **Live language**, `.live`-syntax-inlined-in-Rust — with its **own GPU renderer** (no wgpu; direct Metal / DirectX 11 / OpenGL / WebGL backends) and a **mobile-targeted toolchain** (`cargo-makepad`) that builds for iOS / Android / tvOS in addition to desktop and WASM. Lead architect **Rik Arends** (ex-Cloud9 IDE) is the public face; **Eddy Bruël** and **Sebastian Michailidis** are the named co-architects. **Project Robius**, a separate community led by **Kevin Boos** (Futurewei) and built on top of Makepad, ships the most-cited Makepad app, **Robrix** (a Matrix chat client; v1.0.0-alpha.1, 2026-05-05).

Makepad is the closest existing-art for **"DSL-based Rust UI with GPU rendering and mobile targets"**. It validates that *a DSL above a Rust UI runtime can ship 1.0 with mobile-first targeting* — the same shape Buiy's BSN authoring layer aspires to, lifted from the Slint precedent and pushed further into mobile. It is also the cleanest data point for what *not* to do on accessibility: Makepad has **no AccessKit integration**, no AT-SPI / UIA / NSAccessibility bridge, and the only open accessibility issue ([#196](https://github.com/makepad/makepad/issues/196), open since 2023-08-08) records a community request that the maintainers' presentation framed as "AI will soon do the heavy lifting." Buiy's AccessKit-first commitment is *exactly* the corrective to that posture.

## Honest assessment

**Strong points.**

- **Reached 1.0 in production.** `makepad-widgets` 1.0.0 published 2025-05-13. Seven published versions on crates.io. **6,418 GitHub stars**, 331 forks, 132 open issues; created 2019-02-20 — six years of development before the 1.0.
- **Own GPU pipeline, no wgpu dependency.** Direct backends: **Metal (macOS / iOS), DirectX 11 (Windows), OpenGL (Linux), WebGL (WASM)**. Bypasses the wgpu version-pin treadmill at the cost of carrying four backend implementations in-house.
- **Mobile-first toolchain ships.** `cargo-makepad` installs iOS / Android / tvOS toolchains; Robrix demonstrably runs on macOS / Linux / Windows / Android / iOS / iPadOS (OpenHarmony builds but doesn't run yet).
- **Live DSL with shipping hot-reload.** Property bindings, animations, shader code, and full UI trees live in `.live`-syntax blocks (embedded in Rust via `live_design!` or pulled from `.live` files). Edits hot-reload into a running Makepad app — the `hotload_ui` example demonstrates this end-to-end.
- **Founder competence is load-bearing and real.** Rik Arends previously built **Cloud9 IDE** (acquired by Amazon, became AWS Cloud9). Eddy Bruël has Mozilla/Firefox JavaScript engine history. Sebastian Michailidis is a long-time Makepad contributor and the #1 GitHub committer (under the handle `okapii`, 154 contributions).
- **One canonical Makepad-built shipped app: Makepad Studio.** The Makepad IDE / live-editing environment is itself a Makepad app, dogfooded daily by the team.
- **Project Robius umbrella adds adoption.** Robrix (Matrix client, 448 stars), `makepad_wechat`, `makepad_wonderous` ports demonstrate Makepad reaching beyond the founder team. Robrix presented at Rust China Conf 2025 and GOSIM Europe/China 2024.

**Adoption is small despite 1.0.** Total `makepad-widgets` downloads: **16,974** (recent 90-day: 1,768). For comparison, Slint at 1.0+ has 1.1M+ lifetime downloads. The 6.4k GitHub stars suggest curiosity outpaces production use. The 132 open issues vs. the small contributor base (top 10 contributors: ~445 commits combined) point to a small, founder-driven core team.

**No AccessKit. No accessibility story at all.** Issue [#196](https://github.com/makepad/makepad/issues/196) sits open since 2023-08-08 with zero team responses recorded. The community report quotes Rik Arends from a presentation: "Rik mentioned that for Accessibility most likely AI would soon do the heavy lifting for us." There is no `accesskit` dependency in the workspace, no `accesskit_winit` adapter integration, no AT-SPI / UIA / NSAccessibility bridge. For any consumer who needs WCAG conformance, Makepad is unusable as-shipped. Buiy's AccessKit-first design ([architecture.md § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)) is the explicit corrective.

**Documentation is thin.** `docs.rs` reports **5.92% documentation coverage** on `makepad-widgets` 1.0.0. `makepad-live-compiler` is at **0%**. The Live language has no reference manual on the project site (`makepad.dev` returns "Makepad is loading" — a JS SPA shell with no static fallback). Learning Makepad is example-driven, not documentation-driven.

**Live DSL is yet-another-language.** `.live` is a new syntax (not JSON, not Rust, not a Lisp). Editor support comes from the Makepad Studio IDE itself; no separate `makepad-lsp` exists for VS Code / Helix / Neovim users (vs Slint's editor-agnostic `slint-lsp`). Cross-language refactoring (rename a property in Rust, propagate to `.live`) is manual.

**Single-vendor (de facto).** The project-level `okapii` (Sebastian Michailidis), `eddybruel` (Eddy Bruël), and Rik Arends are the de facto stewards. No foundation, no charter, no formal RFC process. The `kevinaboos` contributor (104 commits, Robius lead) is downstream Robius work, not core Makepad. Bus factor is concentrated.

**Standalone framework, not a game-engine UI.** Makepad is **not** a Bevy-shaped or game-engine-shaped UI library. It does not integrate with an ECS; it does not interop with Bevy's render graph; it is its own world. Buiy comparisons must treat Makepad as a *DSL-and-renderer reference*, not as a UI library Buiy could integrate.

## Key facts

| Fact | Value | Source |
|---|---|---|
| Crate | `makepad-widgets` 1.0.0 (2025-05-13) | [crates.io API](https://crates.io/crates/makepad-widgets) |
| All versions | 1.0.0 (2025-05-13), 0.9.1 (2025-05-12), 0.9.0 (2025-05-12), 0.6.0 (2023-09-21), 0.5.0, 0.4.0, 0.3.0 | crates.io API |
| Total downloads | 16,974 lifetime; 1,768 recent (90d) | crates.io API |
| Repo | https://github.com/makepad/makepad | — |
| Stars / forks | 6,418 stars / 331 forks (2026-05-22) | GitHub API |
| Open issues | 132 | GitHub API |
| Created | 2019-02-20 (GitHub repo) | GitHub API |
| License (crates.io) | **`MIT OR Apache-2.0`** (per crates.io version metadata, all versions) | crates.io API |
| License (repo) | **MIT** (per GitHub repo `license.spdx_id`) | GitHub API — note the dual-license discrepancy below |
| Lead architects | **Rik Arends** (public face, ex-Cloud9 IDE), **Eddy Bruël** (Dutch spelling: `Bruël`, not `Bruel`), **Sebastian Michailidis** (Germany; GitHub `okapii`) | README + verified GitHub profiles |
| Crate publisher | `makepaddev` (single bot account for all 7 releases) | crates.io API |
| Top contributor | `okapii` = Sebastian Michailidis (154 commits) | GitHub contributors API + profile |
| Description | "Makepad widgets" (crates.io); "AI-accelerated application development environment for Rust" (README) | crates.io + README |
| DSL | **Live language**, `.live` syntax, embedded in Rust via `live_design!` macro or external `.live` files | docs.rs + repo |
| Renderers | **Direct backends**: Metal (macOS / iOS), DirectX 11 (Windows), OpenGL (Linux), WebGL (WASM). No wgpu. | README |
| Mobile toolchain | `cargo-makepad` installer for iOS, Android, tvOS, OpenHarmony | README |
| Hot-reload | `.live` edits hot-reload into running Makepad apps; `hotload_ui` example demos this | repo `examples/hotload_ui/` |
| Accessibility | **No AccessKit integration. No AT-SPI / UIA / NSAccessibility. Issue [#196](https://github.com/makepad/makepad/issues/196) open since 2023-08-08 with zero team responses.** Community-reported maintainer position: "AI will soon do the heavy lifting" | issue #196 verbatim |
| Documentation | docs.rs: **5.92%** documented for `makepad-widgets` 1.0.0; **0%** for `makepad-live-compiler` 1.0.0 | docs.rs |
| Project site | `makepad.dev` (JS SPA shell, "Makepad is loading" with no static fallback) | direct fetch |
| Canonical app | **Makepad Studio** (the Makepad IDE, built on Makepad itself, dogfooded) | repo + README |
| Downstream apps | **Robrix** (Matrix client, 448 stars, v1.0.0-alpha.1 2026-05-05, presented Rust China Conf 2025 + GOSIM 2024); `makepad_wechat`; `makepad_wonderous` — all under **Project Robius**, led by **Kevin Boos** (Futurewei) | github.com/project-robius |

## Contents

Each file is independently skimmable. Sources are listed per file.

**Technical subsystems**

- [**architecture.md**](architecture.md) — The Live-DSL-as-source-of-truth model + own GPU renderer. How `.live` blocks compose with Rust through `live_design!`. The runtime, the live registry, hot-reload pipeline.
- [**live-language.md**](live-language.md) — The Live DSL surface: syntax, types, property bindings, animations, components. Hot-reload mechanism. Editor / tooling story.
- [**gpu-rendering.md**](gpu-rendering.md) — Direct Metal / DX11 / OpenGL / WebGL backends. Shader pipeline, batching, glyph atlas. The "no wgpu" choice and its costs.
- [**mobile-targets.md**](mobile-targets.md) — `cargo-makepad` toolchain; iOS / Android / tvOS / OpenHarmony status; touch input handling; mobile-specific widgets and patterns.

**Project lens**

- [**history.md**](history.md) — Rik Arends's prior work (Cloud9 IDE), Makepad genesis ~2019 GitHub repo, six-year pre-1.0 development arc, 1.0 release May 2025. Co-architect lineages.
- [**distribution-and-governance.md**](distribution-and-governance.md) — Dual-license metadata (crates.io vs repo discrepancy), single-publisher account, Project Robius downstream, Futurewei funding angle, bus factor.
- [**ecosystem-and-comparisons.md**](ecosystem-and-comparisons.md) — Makepad Studio (canonical app), Robrix worked example, Project Robius community. Comparisons vs Slint, Dioxus, egui, Iced, and Buiy.

**Open posture**

- [**open-problems.md**](open-problems.md) — Accessibility absence + #196 quote. Documentation coverage. Adoption gap despite 1.0. APG widget coverage. Live-DSL learning curve. wgpu skipped — backend maintenance cost.
- [**critiques.md**](critiques.md) — DSL learning curve; small adoption despite 1.0; mobile-first scope shapes desktop features; `.live`-vs-Rust integration friction; Live is yet-another-DSL; AI-replaces-a11y posture is a Buiy red line.

**Reference**

- [**lessons.md**](lessons.md) — **The consult-this-when-designing decision file.** Validates / Avoid / Borrow.
- [**glossary.md**](glossary.md) — Makepad-specific terms.

## How to use this prior-art doc

When designing a Buiy feature that touches DSL-based authoring, hot-reload, GPU rendering, or mobile targeting:

1. Start in [**lessons.md**](lessons.md). It enumerates which Buiy choices Makepad's experience validates (DSL above runtime is shippable; mobile-first is a real differentiator; GPU rendering for production app UI works), which Makepad postures Buiy explicitly rejects (AI-replaces-a11y, DSL-as-primary-authoring, no AccessKit), and which Makepad primitives are worth borrowing (the Live hot-reload pattern for `.bsn`, mobile-input primitives, animation/transition primitives baked into the authoring layer).
2. If lessons.md references a subsystem, read the matching file.
3. If you're evaluating "should Buiy ship a DSL above the ECS authoring layer" alongside [`../slint/dsl-language.md`](../slint/dsl-language.md), [`live-language.md`](live-language.md) is the second data point — Slint's `.slint` and Makepad's `.live` are the two production Rust-ecosystem DSLs.
4. If you're looking at the mobile question (Buiy foundation README § 5 open question on platform support staging), [`mobile-targets.md`](mobile-targets.md) is the field report: what `cargo-makepad`'s mobile toolchain looks like, what Robrix demonstrably ships on, what doesn't yet work.

Cross-links to sibling prior-art:

- [`../slint/`](../slint/) — DSL precedent. Slint and Makepad together are the two Rust-ecosystem DSL UI toolkits at 1.0+. Their choices diverge sharply (Slint: AccessKit-first, GPL+commercial gate, embedded-first; Makepad: no a11y, MIT/Apache, mobile-first, own renderer) but the DSL-above-Rust-runtime shape is shared.
- [`../accesskit/`](../accesskit/) — what Makepad lacks. Makepad's accessibility absence is the cleanest argument for Buiy's AccessKit-first commitment.
- [`../bevy-ui/`](../bevy-ui/), [`../bevy-feathers/`](../bevy-feathers/), [`../bevy-ui-widgets/`](../bevy-ui-widgets/) — Bevy-ecosystem UI alternatives. Makepad is outside this ecosystem entirely (standalone framework, no ECS).
- [`../dioxus/`](../dioxus/), [`../iced/`](../iced/), [`../egui/`](../egui/) — other Rust UI frameworks Makepad sits alongside.

**Framing disclosure.** This corpus is written from the same **Buiy is fully-open MIT/Apache + ECS-and-BSN-native + parallel-to-bevy_ui + AccessKit-first** stance as the rest of `docs/prior-art/`. Three biases worth naming:

- **Accessibility framing is unfavorable to Makepad.** Buiy treats AccessKit-first as foundational ([architecture.md § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)); the corpus reads Makepad's a11y absence as a structural failure, not a deferred-feature choice. A future reader weighing "should Buiy de-prioritize a11y as well?" should read the corpus on its own terms first.
- **DSL-as-primary-authoring framing is unfavorable to Makepad and Slint both.** Buiy's ECS+BSN-both-first-class choice reads both `.slint` and `.live` as authoring lock-outs. A future Buiy spec author considering "should we ship a DSL?" should re-read [`live-language.md`](live-language.md) and [`../slint/dsl-language.md`](../slint/dsl-language.md) on their own terms first.
- **Adoption-vs-stars framing is intentionally harsh.** 16,974 lifetime downloads at 1.0 is small; the corpus says so directly. A reader weighing the Makepad gestalt should treat low downloads as a fact about adoption, not necessarily about technical quality.

Doc lives, not snapshot — bump the date in this file's header on every meaningful update. The most date-sensitive facts are crate versions, the open-issues count on #196, and the Robrix release status; refresh those when next iterating.

## Sources

- Makepad repo: https://github.com/makepad/makepad
- Makepad on crates.io (API): https://crates.io/api/v1/crates/makepad-widgets
- `makepad-widgets` docs.rs: https://docs.rs/makepad-widgets/1.0.0/makepad_widgets/
- `makepad-live-compiler` docs.rs: https://docs.rs/makepad-live-compiler/latest/makepad_live_compiler/
- Makepad project site: https://makepad.nl/ (vanity domain) / https://makepad.dev (JS SPA shell, no static content at fetch time)
- Rik Arends Twitter: https://twitter.com/rikarends → redirects to https://x.com/rikarends
- Sebastian Michailidis GitHub: https://github.com/okapii
- Eddy Bruël GitHub: https://github.com/eddybruel
- Kevin Boos GitHub: https://github.com/kevinaboos
- Issue #196 "On Accessibility": https://github.com/makepad/makepad/issues/196
- Project Robius: https://github.com/project-robius
- Robrix: https://github.com/project-robius/robrix
- Sibling files (per-section `## Sources`)
- Buiy foundation: [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md), [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Sibling prior-art: [`../slint/`](../slint/), [`../accesskit/`](../accesskit/)
