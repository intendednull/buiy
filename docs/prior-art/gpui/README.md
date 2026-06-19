**Date:** 2026-05-22
**Status:** active
**Subject:** GPUI — Zed's GPU-accelerated UI framework; the strongest existing-art for "production app UI on a custom retained-mode GPU pipeline"; single-product dogfooded; no AccessKit; recently deprioritized for community use

# GPUI

[`gpui`](https://github.com/zed-industries/zed/tree/main/crates/gpui) is **Zed Industries'** custom UI framework — the renderer + reactive layer that powers the [Zed editor](https://zed.dev/). It is the closest existing-art for Buiy's foundation §2.2-§2.3 commitment to "own the render pipeline on top of wgpu, draw a 2D UI scene like a game engine, integrate Taffy directly for layout." Zed renders its entire editor surface this way at 120 FPS on Apple silicon (Antonio Scandurra, [_Leveraging Rust and the GPU to render user interfaces at 120 FPS_](https://zed.dev/blog/videogame), 2023-03-07).

Two caveats up front, both load-bearing for any Buiy decision that cites GPUI:

1. **GPUI is dogfooded for Zed and only Zed.** As of February 2026 the maintainers publicly stated they are pausing community-facing GPUI work to focus on Zed business priorities ([HN thread 47003569](https://news.ycombinator.com/item?id=47003569)). A community fork — [`gpui-ce`](https://github.com/gpui-ce/gpui-ce) — exists; activity is sparse (~381 commits behind mainline at fork time, single-digit merged PRs). The crates.io publish (`gpui = 0.2.2`, 2025-10-22, 101k downloads) is a courtesy; the framework lives in Zed's monorepo and breaks between Zed releases.
2. **GPUI has no AccessKit and no screen-reader support.** Zed's accessibility tracking discussion ([#6576](https://github.com/zed-industries/zed/discussions/6576)) has been open since June 2023; Windows Zed has "zero practical accessibility" per maintainer admissions. AccessKit integration is on the wish list, not the roadmap. This is the single largest disqualifier from copying GPUI's architecture wholesale into Buiy.

Despite those caveats, GPUI is the **production-proven existence proof** that a custom-GPU retained-mode UI for a serious productivity app is shippable in Rust. Zed 1.0 (October 2025) ships on this stack. Buiy can borrow the rendering primitives, the four-stage paint pipeline, and the `Entity` + effect-queue ownership model without inheriting GPUI's tightly-coupled-to-Zed shape or its accessibility debt.

## Key facts

- **Crate:** [`gpui`](https://crates.io/crates/gpui) — latest stable **0.2.2** (2025-10-22). Total downloads: ~101k (recent 30-day: ~59k). Pre-1.0; breaking changes between versions are expected per the README.
- **Repository:** [`zed-industries/zed`](https://github.com/zed-industries/zed), `crates/gpui/`. License: **Apache-2.0** (single license; not dual MIT/Apache like most Rust crates).
- **Steward:** [Zed Industries](https://zed.dev/) — venture-backed; $32M Series B led by Sequoia (August 2025), plus a $10M Series A from Redpoint and Roots Ventures (~$42M total). Lead architects: Nathan Sobo (CEO), Antonio Scandurra. Both worked on Atom at GitHub (2011-2018) and co-built [Teletype](https://teletype.atom.io/) (CRDT collaborative editing) before founding Zed.
- **Paradigm:** _hybrid immediate + retained mode_ (per the README) — not pure retained. High-level views use a declarative `Render` trait that rebuilds on `notify()`; low-level custom elements implement the `Element` trait imperatively for fine-grained layout control. State lives in `Entity<T>` handles owned by a single global `App`; observers + effect queue propagate change.
- **Layout:** [Taffy](https://github.com/DioxusLabs/taffy) (`=0.10.1` pin in `Cargo.toml`) — same crate Buiy uses. Provides Flexbox and Block layout.
- **Text:** custom `TextSystem` over `font-kit` (Zed's fork), `ttf-parser`, OS shaping APIs (Core Text on macOS; DirectWrite on Windows; on Linux, custom shaping over the same family of crates). **Not** cosmic-text; **not** Parley; **not** swash.
- **GPU backends (three, not unified):**
  - **macOS:** Metal directly via the [`metal`](https://crates.io/crates/metal) crate. No wgpu.
  - **Linux:** historically [Blade](https://github.com/kvark/blade) (Vulkan) → migrating to **wgpu** as of PR [#46758](https://github.com/zed-industries/zed/pull/46758) (late 2025 / early 2026). Wayland + X11 windowing.
  - **Windows:** DirectX 11 + DirectWrite ([Zed on Windows announcement](https://zed.dev/windows), 2025). Custom Win32 windowing.
  - **Mobile/Web:** none. Tracking issues [#43206 (iOS)](https://github.com/zed-industries/zed/issues/43206), [#43207 (Android)](https://github.com/zed-industries/zed/issues/43207) are open with no committed work.
- **Windowing:** custom platform-specific code paths. Does **not** use [`winit`](https://crates.io/crates/winit).
- **Accessibility:** none. No AccessKit. No platform a11y APIs wired up. Open discussion [#6576](https://github.com/zed-industries/zed/discussions/6576) since 2023.
- **Production users:** [Zed](https://zed.dev/) (primary). [Longbridge Pro](https://longportapp.com/) (trading-desktop rewrite of an Electron app; built atop the third-party [`gpui-component`](https://github.com/longbridge/gpui-component) widget library because GPUI ships zero widgets — see [community-champion post](https://zed.dev/blog/community-champion-jason-lee)). Third-party adoption beyond Zed + Longbridge is negligible.

## Folder contents

| File | Purpose |
|---|---|
| [`README.md`](README.md) | This file — overview, key facts, ToC, framing. |
| [`architecture.md`](architecture.md) | The four-stage render pipeline (layout / prepaint / paint / GPU submit); element-tree + view-tree + entity model; cross-platform abstraction layer. |
| [`element-tree.md`](element-tree.md) | `Element` trait, `Render` trait, `Div`, `Styled`, `Interactivity`. The hybrid immediate/retained decomposition. Comparison to React/Elm/immediate-mode. |
| [`gpu-rendering.md`](gpu-rendering.md) | SDF-based primitive shaders (rectangles, shadows, glyphs, icons, images); three-backend story (Metal / wgpu / DX11); the Blade→wgpu migration; clipping, rounded corners, gradients. |
| [`text-and-input.md`](text-and-input.md) | The custom `TextSystem`, shaping via OS APIs (Core Text / DirectWrite), font-kit fork, glyph atlasing, line-wrapping, the action + focus + key-binding system. |
| [`accessibility.md`](accessibility.md) | The gap — no AccessKit, no screen-reader support, the 2023-present open discussion. Why this matters for any project that copies GPUI's architecture. |
| [`history.md`](history.md) | Atom (2011-2018) → Atom's discontinuation (2022) → Zed announcement (2023) → open-source (2024) → Zed 1.0 (October 2025) → community-fork era. Nathan Sobo and Antonio Scandurra's design trajectory. |
| [`distribution-and-governance.md`](distribution-and-governance.md) | Apache-2.0-only license. VC funding (Sequoia Series B). Cross-platform support matrix. Crates.io publish vs monorepo divergence. |
| [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md) | Production users (Zed primary, Longbridge secondary). `gpui-component` widget library. The `gpui-ce` community fork. Comparisons to egui, Iced, Slint, Dioxus, Xilem/Masonry, Buiy. |
| [`critiques-and-open-problems.md`](critiques-and-open-problems.md) | Zed-only dogfooding limits generality; Apache-only license; macOS-first cross-platform parity; documentation maturity; pre-1.0 API churn; monorepo-vs-publish divergence; February-2026 community-deprioritization announcement. |
| [`lessons.md`](lessons.md) | **The decision file.** Validates / Avoid / Borrow for the Buiy foundation. |
| [`glossary.md`](glossary.md) | `App`, `AppContext`, `Window`, `Entity`, `View`, `Model`, `Render`, `Element`, `Div`, `Styled`, `Interactivity`, `TextSystem`, `Action`, `KeyContext`, `WeakEntity`. |

## Why Buiy researches GPUI specifically

Three slots in Buiy's foundation map directly onto questions GPUI has already answered (or failed to answer) in production:

1. **"Can a custom retained-mode GPU pipeline ship a serious productivity app in Rust?"** GPUI says yes — Zed ships on it. This is the validating signal for Buiy [`architecture.md § 2.3`](../../specs/2026-05-07-buiy-foundation/architecture.md) ("Render pipeline — custom Bevy render passes that walk Buiy hierarchies. Full control over rounded clipping, `clip-path` shapes, mask-image, backdrop-filter, mix-blend-mode...").
2. **"Is wgpu the right cross-platform abstraction, or are platform-specific backends required for quality parity?"** GPUI's answer is nuanced: macOS uses Metal directly (not wgpu), Linux is migrating to wgpu, Windows uses DX11 directly. Three backends, not one. Buiy commits to Bevy's wgpu pipeline (foundation §2.2) — GPUI's split-backend strategy is a counterpoint worth understanding, especially if Bevy's wgpu becomes the cross-platform-parity bottleneck.
3. **"What does it cost to skip AccessKit at v1?"** GPUI's 2.5-year-and-counting accessibility debt is the answer. Zed is unusable for screen-reader users in 2026. Buiy's accessibility-first commitment (foundation §2.6) is informed directly by what happens when a high-quality UI framework defers a11y indefinitely.

## Framing disclosure

- **Author:** Buiy team, single-agent compressed folder, 2026-05-22.
- **Sources:** Zed's engineering blog ([_Videogame_](https://zed.dev/blog/videogame), [_Ownership_](https://zed.dev/blog/gpui-ownership)), the GPUI README + `Cargo.toml` on `main`, [docs.rs/gpui](https://docs.rs/gpui/latest/gpui/), the DeepWiki GPUI section, GitHub issue/discussion threads (#6576 a11y, #46758 wgpu migration, #43206/43207 mobile, #46183 examples-outdated), Hacker News discussion 47003569 (community-deprioritization, Feb 2026), Sequoia/BusinessWire funding coverage, the [`gpui-ce`](https://github.com/gpui-ce/gpui-ce) fork, the [`longbridge/gpui-component`](https://github.com/longbridge/gpui-component) widget library.
- **Coverage gaps and verified-falsified claims** (relative to the original research brief):
  - **CORRECTED:** Original brief said "retained-mode + custom paradigm." Actual: _hybrid immediate + retained_. See [`element-tree.md`](element-tree.md).
  - **CORRECTED:** Original brief said "GPU-rendered via Metal (macOS) / wgpu (cross-platform)." Actual: three backends — Metal (macOS, direct), wgpu (Linux, migrating from Blade), DirectX 11 (Windows, direct). See [`gpu-rendering.md`](gpu-rendering.md).
  - **CORRECTED:** Original brief speculated "Patrick Collison among backers." Not found in public reporting. Confirmed investors: Sequoia (lead Series B), Redpoint, Roots, AI Futures Fund, Nimble Partners, Preston-Werner Ventures, Prototype Capital.
  - **CONFIRMED:** No AccessKit integration. No screen-reader support.
  - **CONFIRMED:** Zed-monorepo divergence — `gpui 0.2.x` on crates.io trails `crates/gpui/` in `zed-industries/zed`.
  - **NEW (not in brief):** February 2026 deprioritization announcement; `gpui-ce` community fork; `longbridge/gpui-component` third-party widget library; Longbridge Pro as second production user. Material for the Buiy decision.
- **Not verified directly:** Zed-internal performance numbers (the 120-FPS claim is from a 2023 blog post predating Linux/Windows shipping); the precise GPU memory budget on integrated graphics; whether `gpui-ce` will reach feature parity with mainline.

## Cross-links into the Buiy corpus

- [`docs/prior-art/bevy-egui/lessons.md`](../bevy-egui/lessons.md) — the "Zed uses GPUI, NOT egui" anti-conflation note.
- [`docs/prior-art/egui/lessons.md`](../egui/lessons.md) — the immediate-mode-only counterpoint to GPUI's hybrid.
- [`docs/prior-art/accesskit/lessons.md`](../accesskit/lessons.md) — names GPUI as a verified-false AccessKit adopter.
- [`docs/prior-art/iced/`](../iced/) — Iced ships on `iced_wgpu` end-to-end; useful as the wgpu-only counterpoint.
- [`docs/prior-art/slint/`](../slint/), [`docs/prior-art/dioxus/`](../dioxus/) — the DSL-driven and React-style alternatives GPUI explicitly rejects.

## Sources

- GPUI README on `main`: https://github.com/zed-industries/zed/blob/main/crates/gpui/README.md
- GPUI `Cargo.toml` on `main`: https://github.com/zed-industries/zed/blob/main/crates/gpui/Cargo.toml
- GPUI crate page: https://crates.io/crates/gpui
- GPUI docs.rs: https://docs.rs/gpui/latest/gpui/
- _Leveraging Rust and the GPU to render user interfaces at 120 FPS_ (Scandurra, 2023): https://zed.dev/blog/videogame
- _Ownership and data flow in GPUI_ (Zed blog): https://zed.dev/blog/gpui-ownership
- DeepWiki GPUI section: https://deepwiki.com/zed-industries/zed/2.2-ui-framework-(gpui)
- Zed team page: https://zed.dev/team
- Zed on Windows announcement: https://zed.dev/windows
- _We Have to Start Over: From Atom to Zed_: https://zed.dev/blog/we-have-to-start-over
- _Sequoia Backs Zed's Vision for Collaborative Coding_: https://zed.dev/blog/sequoia-backs-zed
- Zed Series B BusinessWire: https://www.businesswire.com/news/home/20250820782241/en/
- Accessibility discussion: https://github.com/zed-industries/zed/discussions/6576
- Blade→wgpu migration PR #46758: https://github.com/zed-industries/zed/pull/46758
- iOS tracking issue #43206: https://github.com/zed-industries/zed/issues/43206
- Android tracking issue #43207: https://github.com/zed-industries/zed/issues/43207
- HN: Zed deprioritizing GPUI community work: https://news.ycombinator.com/item?id=47003569
- `gpui-ce` community fork: https://github.com/gpui-ce/gpui-ce
- `longbridge/gpui-component`: https://github.com/longbridge/gpui-component
- _Community Champion Spotlight: Jason Lee_: https://zed.dev/blog/community-champion-jason-lee
