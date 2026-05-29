**Date:** 2026-05-22
**Status:** active
**Subject:** Dioxus — chronological history: 2021 genesis through 0.7 (Oct 2025) and 0.8.0-alpha (May 2026)

# History

Dioxus is a single-founder project (Jonathan Kelley) that scaled into a small VC-backed company (Dioxus Labs, YC S23) over five years. The release history is unusually well-documented for a Rust framework because each minor release ships with a blog post.

## Timeline

| Date | Version | Highlight | Source |
|---|---|---|---|
| 2021-01-20 | 0.1.0 | Initial crates.io publish. Single-maintainer experiment. | crates.io API |
| 2021-2022 | 0.2 — 0.3 | VDOM, web target, hooks. Built primarily by Kelley. | crates.io versions |
| 2023 (Summer) | YC S23 | Dioxus Labs admitted to Y Combinator. ~$500K seed. Founder: Kelley. | YC company page |
| 2023 | 0.4 | `dioxus-fullstack` (server functions); Tauri-style desktop. Fermi state-management (Recoil-shaped). | crates.io + release notes |
| 2024-03-21 | **0.5** | **Signals.** Replaces `use_state` / `use_ref`. `generational-box` crate carved out. `dioxus-core` unsafe removed. ~100K LoC changed, 1400 commits between 0.4.3 and 0.5. | release-050 blog |
| 2024-12-09 | **0.6** | Redesigned `dx` CLI; first-class iOS/Android (`dx serve --platform ios/android`); suspense + streaming HTML; SSG/ISG; Manganis asset system stabilized; rust-analyzer partial-parse for `rsx!`. | release-060 blog |
| 2025-10-08 | 0.7.0-rc.1 | First 0.7 RC. | crates.io |
| 2025-10-31 | **0.7.0** | **Dioxus Native** (WGPU + Blitz, pre-alpha); **Subsecond hot-patching** across WASM + desktop + mobile; **Stores** primitive; Axum-based fullstack overhaul; WebSocket/SSE server functions; Tailwind autodetect. | release-070 blog (2025-09-08 announcement post-dated to early-release schedule) |
| 2025-11-06 | 0.7.1 | Patch | crates.io |
| 2025-12-05 | 0.7.2 | Patch | crates.io |
| 2026-01-17 | 0.7.3 | Patch | crates.io |
| 2026-03-27 | 0.7.4 | Patch | crates.io |
| 2026-04-07 | 0.7.5 | Patch | crates.io |
| 2026-04-22 | 0.7.6 | Patch | crates.io |
| 2026-05-01 | 0.7.7 | Patch | crates.io |
| 2026-05-07 | 0.7.8 | Patch | crates.io |
| **2026-05-08** | **0.7.9** | Current stable. | crates.io |
| **2026-05-19** | **0.8.0-alpha.0** | In flight. Scope undisclosed in public release notes as of 2026-05-22. | crates.io |

(0.7.9 in the brief was dated 2026-05-19; that's actually the 0.8.0-alpha date — see [`README.md`](README.md) § "Brief corrections".)

## Key inflection points

### 0.1 — 0.4: Pre-funding open-source

Single-maintainer Rust hobby project. React-shape borrowed from Yew + JSX-in-Rust experiments. State via `use_state` + `Rc<RefCell<T>>`. The codebase used `unsafe` for VDOM lifetime tricks (later removed). Adoption was small but the architecture solidified — VDOM + components-as-functions + hooks + diff-to-mutation-stream — and remained stable through subsequent rewrites.

### Y Combinator S23 (2023)

Dioxus Labs admitted to Y Combinator Summer 2023 batch. Initial seed funding (~$500K total per Tracxn/Crunchbase) from YC + Pioneer Fund. Kelley left his prior employer (Cloudflare; before that NASA per his LinkedIn) to work on Dioxus full-time. Y Combinator's "Launch YC" post described the company as building "web, desktop, and mobile apps with one codebase in Rust." Team scaled to ~4 (per YC company page) over the next year.

**Sponsorship arrangements distinct from equity.** FutureWei (a US-based Huawei R&D subsidiary) and Satellite.im have publicly sponsored Dioxus development. These are *sponsorships*, not equity investments — the distinction matters for the corpus's framing-disclosure honesty (the brief incorrectly labeled FutureWei as a Series A investor; see [`README.md`](README.md) § "Brief corrections").

### 0.5 (2024-03-21) — Signals

The defining mid-life rewrite. `use_state` and `use_ref` were de-emphasized in favor of `Signal<T>: Copy`. Backed by the new `generational-box` crate (also DioxusLabs-stewarded). The 0.5 changelog reports ~100,000 lines of code changed in ~1,400 commits between 0.4.3 and 0.5.0 — a sign that this was a top-to-bottom rewrite, not an incremental feature add. The `dioxus-core` crate was rewritten to remove all `unsafe`.

The signal model adopted from Solid.js (Ryan Carniato) — Copy-by-default, subscribe-in-render-path, fine-grained re-render — has held in subsequent releases. See [`signals-and-state.md`](signals-and-state.md).

### 0.6 (2024-12-09) — Mobile first-class + tooling polish

`dx serve --platform ios/android` made mobile genuinely usable. The `dx` CLI was rewritten with live progress bars, inline log-level adjustment, and inline WASM stack traces. `rsx!` got partial-parse support for rust-analyzer (autocomplete inside the macro). Suspense + streaming HTML landed for fullstack. Manganis (asset bundling system) stabilized.

### 0.7 (2025-10-31) — Native target + hot-patching + Stores

The most ambitious release. Three load-bearing changes:

1. **Dioxus Native + Blitz** — a brand-new WGPU-based rendering path bypassing webview. Pre-alpha; production-not-recommended even by Dioxus's authors. The bet is that webview-on-desktop is a transitional state and native-WGPU is the future. See [`targets.md`](targets.md) § "Desktop (WGPU/Blitz)".
2. **Subsecond hot-patching** — runtime code-modification via incremental linking + explicit `subsecond::call()` integration points. Works across WASM + desktop + mobile. The most aggressive hot-reload story in Rust UI. See [`architecture.md`](architecture.md) § "Implications for Buiy."
3. **Stores** — derivable trait for nested reactive state where individual fields and collection entries are subscribable. Closes the "signals don't scale to app state" gap that 0.5/0.6 had. See [`signals-and-state.md`](signals-and-state.md) § "Stores."

The fullstack story was overhauled onto Axum, gaining WebSocket, SSE, streaming data, and typed forms. Tailwind detection became automatic.

### 0.7.1 → 0.7.9 (Nov 2025 — May 2026)

Steady patch cadence — nine patch releases in seven months. Most fixes target Subsecond reliability, Blitz feature completeness, mobile CLI ergonomics, and SSR edge cases. The cadence is closer to monthly than quarterly, which reads as a project with real users hitting real bugs.

### 0.8.0-alpha.0 (2026-05-19)

In flight as of this corpus's date. No release notes published. The version is on crates.io but only ~75 downloads at writing time. Whatever 0.8 holds, the pre-alpha-of-Blitz, the Subsecond story, and the multi-target tax (per-target a11y, per-target maturity) are the inherited weight.

## Lineage and inspirations

Dioxus is open about its lineage:

- **React** — components-as-functions, hooks, VDOM-diff-to-mutation, JSX-shape authoring. The most-borrowed.
- **Solid.js** (Ryan Carniato, 2021–) — the fine-grained signals model that 0.5 adopted.
- **Yew** (2017–) — the prior Rust attempt at React-in-WASM. Dioxus is generally faster and more ergonomic in benchmarks, partly because it landed after Yew's lessons were public.
- **Svelte** — referenced in release notes as the source of some compile-time-template DX choices, though Dioxus stayed VDOM-based rather than Svelte-style compile-to-imperative.
- **Tauri** (and the wry/tao stack) — directly used by `dioxus-desktop`.

The framework's stated lineage in the 0.7 release notes: *"borrowing ideas from React, Solid, and Svelte."*

## Implications for Buiy

- **Five years from 0.1 to "production multi-target."** Even with VC funding, YC support, and a small full-time team, Dioxus reached "production on the most mature target (web), pre-alpha on the most ambitious target (native)" only after five years. Buiy's foundation timeline should be calibrated against this — single-platform-on-Bevy is a smaller-scope problem, but multi-target ambitions (web/desktop/mobile parity) carry a years-long quality tax. Foundation [non-goal § 1.3 — "non-Bevy frontends"](../../specs/2026-05-07-buiy-foundation/README.md) is the right scope discipline.
- **Hot-reload is a recurring user demand and a 0.7-level feature.** Dioxus's Subsecond is the most ambitious hot-reload story in Rust UI and shipped as a flagship 0.7 feature, not a 0.1 feature. Buiy's BSN hot-reload semantics ([foundation open question § 5](../../specs/2026-05-07-buiy-foundation/README.md)) should be designed in foundation but not over-promised — production-grade hot-reload across all paths is a years-long investment.
- **Funding doesn't guarantee scope velocity.** YC seed + sponsorship is enough to keep Dioxus full-time-staffed at ~4 people but not enough to make Blitz production-ready in one year. Buiy's scope discipline (foundation README § 1) is the correct stance; resist the "we'll get there with more contributors" temptation.

## Sources

- crates.io API timestamps for every release: https://crates.io/api/v1/crates/dioxus
- Dioxus 0.5 release notes: https://dioxuslabs.com/blog/release-050
- Dioxus 0.6 release notes: https://dioxuslabs.com/blog/release-060
- Dioxus 0.7 release notes: https://dioxuslabs.com/blog/release-070
- Y Combinator company page: https://www.ycombinator.com/companies/dioxus-labs
- Y Combinator launch post: https://www.ycombinator.com/launches/JBK-dioxus-labs-web-desktop-and-mobile-apps-with-one-codebase
- Jonathan Kelley LinkedIn (founder background, Cloudflare/NASA prior employment): https://www.linkedin.com/in/jonathan-r-kelley
- Tracxn funding profile: https://tracxn.com/d/companies/dioxus
- Crunchbase profile: https://www.crunchbase.com/organization/dioxuslabs
