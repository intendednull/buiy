**Date:** 2026-05-22
**Status:** active
**Subject:** GPUI — history: from Atom (2011-2018) to Zed (2023-) to GPUI as Zed's substrate to the crates.io publish and community-fork era

# History

GPUI is the third generation of editor-UI work by the same core team. Tracing the lineage is necessary to understand why GPUI is shaped the way it is — and why its design priorities don't always match a general-purpose UI library's design priorities.

## Phase 1: Atom at GitHub (2011-2018)

Nathan Sobo joined GitHub in late 2011 to build the Atom text editor. The team grew through the early 2010s: Antonio Scandurra joined in 2014 (while still a university student) on the strength of his open-source contributions; Max Brunsfeld joined from Pivotal Labs and would create [Tree-sitter](https://tree-sitter.github.io/), the incremental parsing framework now standard across the editor ecosystem.

Atom was built on **Electron** (HTML/CSS/JS, Chromium-rendered). At its peak Atom was one of the most popular IDEs of its era; it became the template VS Code followed, and helped drive Electron's mainstream adoption. The team also built [Teletype](https://teletype.atom.io/) — pioneering CRDT-based collaborative editing in a real product.

GitHub put Atom into maintenance mode in 2018, then sunset it entirely in 2022. The lessons the founders carried forward:

- Electron's "browser-in-a-window" performance ceiling is real. Large files, multi-pane workflows, and high-frequency interactions (typing latency, scroll smoothness) all hit limits a DOM-backed editor cannot escape.
- The CSS/JS plugin model invited churn — every plugin could redefine layout, blocking the main thread.
- A native rendering approach with strong typing was the obvious next bet.

This is the origin of GPUI's "treat the screen like a video game scene" thesis. Atom proved the negative; Zed/GPUI is the positive proposal.

## Phase 2: GPUI 1 — internal prototype (2019-2022)

After Atom, Nathan, Antonio, and Max regrouped privately and began building Zed. The first GPUI was an internal-only Rust UI framework — never published, never open-sourced, never named publicly in detail. It went through extensive iteration as the editor itself took shape.

The 2022 era was Zed-as-closed-source. The team was building both editor and framework simultaneously, learning what the editor needed, repeatedly tearing down GPUI versions when the architecture proved wrong.

## Phase 3: GPUI 2 announcement (December 2023) — "GPUI 2 is now in production"

The first public reference to GPUI by name was Antonio Scandurra's [March 2023 blog post](https://zed.dev/blog/videogame) explaining the rendering architecture. By late 2023, GPUI had been rewritten ("GPUI 2") and Zed was running on it. The [_GPUI 2 is now in production_](https://news.ycombinator.com/item?id=38871732) HN announcement is the public marker.

GPUI 2 introduced the hybrid immediate/retained architecture documented in this corpus: `Render` trait for views, `Element` trait for elements, `Entity<T>` ownership, the effect-queue model. The earlier GPUI 1 was reportedly more aggressively retained-mode; GPUI 2 added the immediate-mode escape hatch for editor-specific custom layout (line rendering, the file tree, terminal cells).

## Phase 4: Zed open-sourced (January 2024)

Zed was open-sourced under GPL-3.0 (the editor) + Apache-2.0 (GPUI and supporting crates). This made GPUI source-available for the first time, though still as part of the Zed monorepo — not as a standalone library.

The license split is deliberate: editor is GPL to keep forks copyleft; GPUI is Apache so others can use it without forced source disclosure. **This is not a dual MIT/Apache license** (the common Rust pattern); it's Apache-only. Implication: re-exporting GPUI source code in MIT/Apache-dual projects requires careful attribution handling.

## Phase 5: Linux release (mid-2024)

Zed shipped Linux support in 2024, initially using Blade (Vulkan) for rendering. The Linux release exposed GPUI to a wider audience of system-toolchain developers, and complaints about text rendering quality, NVIDIA-PRIME issues, and Wayland-vs-X11 edge cases accumulated.

The [_Linux when?_ blog post](https://zed.dev/blog/zed-decoded-linux-when) is the public retrospective on what shipping Linux took — a substantial engineering effort spanning windowing, GPU pipeline, IME, and HiDPI handling.

## Phase 6: Crates.io publish (2024-2025) — `gpui = 0.2.x`

GPUI was published to crates.io as a separate crate, allowing third parties to depend on it without vendoring the Zed monorepo. The published versions trail the monorepo HEAD significantly:

- `gpui = 0.2.0` — first crates.io release
- `gpui = 0.2.1`
- `gpui = 0.2.2` — current latest (2025-10-22)

Three publishes in ~18 months. The crate is **pre-1.0 and explicitly disclaims API stability** — breaking changes between minor versions are expected. Total downloads ~101k as of May 2026; recent 30-day downloads ~59k. Numbers reflect Zed itself (Zed's CI builds against the published crate? — verify; or downstream tooling) plus a small number of experimental adopters. Not a sign of broad adoption.

The crates.io publish is **a courtesy, not a productization signal**. There's no separate release cadence, no separate maintenance, no separate docs. The crate is whatever was in Zed's `crates/gpui/` at the time of publish.

## Phase 7: Windows release (2025)

[_Zed on Windows_](https://zed.dev/windows) announced the Windows release in 2025. Notable architectural choice: DirectX 11 + DirectWrite native, not wgpu. A dedicated full-time Windows engineering team was committed.

Windows brought a new set of integration concerns (WSL integration for remote editing, Windows extension compatibility, ClearType text rendering) and a new accessibility crisis: Windows users with screen readers found Zed unusable. The accessibility discussion in [#6576](https://github.com/zed-industries/zed/discussions/6576) accelerated.

## Phase 8: Sequoia Series B + Zed 1.0 (August - October 2025)

In August 2025, Zed Industries announced a $32M Series B led by Sequoia ([BusinessWire](https://www.businesswire.com/news/home/20250820782241/en/), [Zed blog](https://zed.dev/blog/sequoia-backs-zed)). Adding to a $10M Series A from Redpoint and Roots Ventures, total funding reached ~$42M. The strategic positioning was "collaborative AI coding" — the Teletype CRDT lineage merged with LLM workflows.

Zed 1.0 shipped October 2025 ([LinuxIAC coverage](https://linuxiac.com/zed-code-editor-hits-1-0-with-gpu-accelerated-ui/)). GPUI was now the substrate of a 1.0 commercial product across macOS, Linux, and Windows.

## Phase 9: Blade→wgpu migration (late 2025 / early 2026)

PR [#46758](https://github.com/zed-industries/zed/pull/46758) reimplemented the Linux backend on wgpu, removing Blade. The motivation in the PR description was unusually candid:

> The blade graphics library is a mess and causes several issues for both Zed users as well as other 3rd party apps using GPUI.

The migration is significant for Buiy: GPUI, despite its native-API bias, **converged on wgpu for the platform where no native API dominates**. This validates the broader Rust UI ecosystem's wgpu bet.

## Phase 10: Community deprioritization (February 2026)

[HN thread 47003569](https://news.ycombinator.com/item?id=47003569) marks the explicit pause of community-facing GPUI work:

> We gotta focus on some business relevant work in 2026.

The decision is framed in HN discussion as "GPUI was built with Zed in mind, and it is hard for Zed Industries to justify work on GPUI that is purely for the community." The Sequoia funding implies revenue-pursuit timeframes; an open-source UI library has no path to direct revenue.

A community fork — [`gpui-ce`](https://github.com/gpui-ce/gpui-ce) — was started by a former employee. As of May 2026 it has ~348 stars, ~23 forks, single-digit merged PRs, and is ~381 commits behind mainline. The fork is alive but not at scale.

Separately, [`longbridge/gpui-component`](https://github.com/longbridge/gpui-component) — a 60+-widget UI kit built atop GPUI for Longbridge's Pro trading client — is the second production application of GPUI. Its existence is the only meaningful evidence that GPUI can be used as a general-purpose UI library outside Zed.

## What this history means for Buiy

Three takeaways for any Buiy decision that cites GPUI:

1. **GPUI's design priorities are Zed's design priorities.** The hybrid paradigm, the lack of widgets, the Apache-only license, the no-AccessKit story — all of these served Zed's needs at each stage. They do not reflect a survey of UI-library needs.
2. **Single-product dogfooding got GPUI to 1.0 fast and to ecosystem-friendly slowly.** Zed shipped a working editor on top of an unstable framework. The community paid the cost in broken examples (issue [#46183](https://github.com/zed-industries/zed/issues/46183)), pre-1.0 API churn, and now community-deprioritization. Buiy's ECS-host model (Bevy as upstream) is a deliberately different bet: Buiy is downstream of an ecosystem (Bevy), not the upstream of one.
3. **The Atom-Zed transition is the cautionary tale for any "we must rebuild on Rust" framing.** It took Nathan, Antonio, and Max ~5 years of private iteration to get GPUI to a shippable state. The cost of building a custom retained-mode GPU pipeline from zero is **multi-person-years**. Buiy avoids that cost by integrating Bevy's render graph and existing primitives (foundation §2.2). The right comparison is "GPUI took ~5 years to ship; Buiy ships in 18 months because Bevy did the engine work."

## Sources

- _We Have to Start Over: From Atom to Zed_: https://zed.dev/blog/we-have-to-start-over
- _Leveraging Rust and the GPU to render user interfaces at 120 FPS_ (Scandurra 2023): https://zed.dev/blog/videogame
- _GPUI 2 is now in production_ HN: https://news.ycombinator.com/item?id=38871732
- _Linux when?_ Zed blog: https://zed.dev/blog/zed-decoded-linux-when
- _Sequoia Backs Zed's Vision for Collaborative Coding_: https://zed.dev/blog/sequoia-backs-zed
- Series B BusinessWire: https://www.businesswire.com/news/home/20250820782241/en/
- _Zed on Windows_: https://zed.dev/windows
- _Zed 1.0_ coverage: https://linuxiac.com/zed-code-editor-hits-1-0-with-gpu-accelerated-ui/
- Blade→wgpu PR #46758: https://github.com/zed-industries/zed/pull/46758
- HN: Zed deprioritizing GPUI community work: https://news.ycombinator.com/item?id=47003569
- `gpui-ce` fork: https://github.com/gpui-ce/gpui-ce
- `longbridge/gpui-component`: https://github.com/longbridge/gpui-component
- _Community Champion Spotlight: Jason Lee_ (Longbridge story): https://zed.dev/blog/community-champion-jason-lee
- _Goodbye Atom. Hello Zed._ (Changelog podcast 531): https://changelog.com/podcast/531
- _Episode #136: Antonio Scandurra_ (DevTools.fm): https://www.devtools.fm/episode/136
- Zed (text editor) Wikipedia: https://en.wikipedia.org/wiki/Zed_(text_editor)
