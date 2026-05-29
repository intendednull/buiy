**Date:** 2026-05-22
**Status:** active
**Subject:** Dioxus — React-style Rust UI framework targeting web/desktop/mobile; sibling project of Taffy under DioxusLabs

# Dioxus

Dioxus is a React-inspired Rust UI framework whose pitch is **"build for web, desktop, and mobile with a single codebase."** Components are functions; UI is declared with the `rsx!` macro; state lives in signals (since 0.5). The compiled component tree produces a virtual DOM, which is then rendered to one of several backends: a browser DOM (web/WASM), a native window via Webview (desktop today), an experimental WGPU-driven HTML/CSS engine called **Blitz** (Dioxus Native), or native iOS/Android shells. The project is maintained by **DioxusLabs**, the same organisation that owns [Taffy](../taffy/README.md); the day-to-day lead is **Jonathan Kelley** (@jkelleyrtp), an ex-Cloudflare/NASA engineer who founded the company through Y Combinator S23.

Dioxus is **not** a plausible Buiy substrate — Buiy is a Bevy-ECS-native, parallel-to-`bevy_ui` UI stack ([foundation README](../../specs/2026-05-07-buiy-foundation/README.md)) and Dioxus is a separate runtime with its own scheduler, VDOM, and renderer abstractions. Dioxus appears in this corpus because **(a)** it is the highest-profile React-shaped reactive UI framework in Rust and therefore the canonical comparison point for any Buiy reactivity-layer discussion ([foundation README § 1.3 non-goal — "no signals/computed/effects in v1"](../../specs/2026-05-07-buiy-foundation/README.md), open question § 5); **(b)** it is the largest production consumer of Taffy outside Bevy itself, so its integration pattern is directly relevant to Buiy's own Taffy bridge; **(c)** the `rsx!` macro is the most-iterated-on Rust authoring DSL and informs what BSN-shaped authoring could feel like in the steady state.

**Honest assessment.** Dioxus is **the most production-deployed React-shaped Rust UI framework** (1.5M+ downloads, named users include Airbus + ESA + Huawei via FutureWei sponsorship) and **simultaneously a multi-target framework whose targets each have their own maturity story**. The web target (DOM via WASM) is the most mature; the desktop-webview target is functional but inherits webview pain (binary size, IPC); the **Dioxus Native / Blitz** target is explicitly **pre-alpha** and not recommended for production by its own authors; the mobile-native target is the newest. Hot-patching ("Subsecond") shipped in 0.7 (October 2025) and is genuinely novel for Rust. Accessibility is **DOM-target-only today**; native targets inherit no AT story from their renderer. Signals (introduced in 0.5, March 2024) borrow heavily from Solid's signal-based reactivity model rather than React's `useState`.

## Key facts (verified 2026-05-22 via crates.io API + dioxuslabs.com + DioxusLabs/blitz README)

| Fact | Value |
|---|---|
| Crate | `dioxus` |
| Latest stable | **0.7.9** (2026-05-08) |
| Pre-release | **0.8.0-alpha.0** (2026-05-19) |
| Recent stable line | 0.7.4 (2026-03-27) → 0.7.5 (2026-04-07) → 0.7.6 (2026-04-22) → 0.7.7 (2026-05-01) → 0.7.8 (2026-05-07) → 0.7.9 (2026-05-08) |
| First release | 0.1.0, 2021-01-20 |
| Total downloads | **1,531,741** |
| 90-day downloads | **424,771** |
| Versions published | 54 |
| License | **MIT OR Apache-2.0** |
| MSRV | rust-version 1.83.0 |
| Edition | 2021 |
| Repo | https://github.com/DioxusLabs/dioxus/ |
| Homepage | https://dioxuslabs.com |
| Steward | **DioxusLabs** (also stewards [Taffy](../taffy/README.md), Blitz, dioxus-cli, manganis, sledgehammer) |
| Lead | **Jonathan Kelley** (@jkelleyrtp) — founder, YC S23, ex-Cloudflare/NASA |
| Crate owners | `jkelleyrtp` + the `dioxuslabs:publish` team |
| Funding | **YC S23 seed (~$500K)**, Pioneer Fund, GitHub Accelerator. Sponsors: FutureWei, Satellite.im. **No Series A round confirmed** (correction to brief — see below) |
| Team size | 4 (per YC company page) |
| GitHub stars | ~36.1k (per upstream README crawl) |
| Production users named upstream | Airbus, ESA (collision-avoidance system, via FutureWei sponsorship); Satellite.im; community apps |
| Reactivity model | **Signals** (0.5+, 2024-03-21) + **Stores** (0.7+, fine-grained nested) |
| Authoring DSL | `rsx!` proc-macro |
| Layout engine | **Taffy** (via Blitz on Native target only; web target uses browser DOM layout) |
| Text shaper | **Parley** (via Blitz on Native target); browser/webview otherwise |
| Renderer (Native) | **Vello** via Anyrender (Blitz dependency) |
| CSS engine (Native) | **Stylo** (Firefox-derived) via Blitz |

## Brief corrections

- **Pre-amble said "VC-backed FutureWei + Khosla Ventures Series A."** Verified false. Public funding records (Y Combinator, Crunchbase via Tracxn, Pioneer Fund disclosures) show YC S23 seed (~$500K), Pioneer Fund, GitHub Accelerator. FutureWei is a sponsor/customer, not an equity investor. **No Khosla Ventures involvement found.** No Series A round announced. See [`governance.md`](governance.md).
- **Pre-amble said 0.7.9 published 2026-05-19.** Verified false. crates.io API timestamp for 0.7.9 is **2026-05-08T00:07:59Z**. 2026-05-19 is the 0.8.0-alpha.0 publish date.
- **Pre-amble said "Servo-fork-style" Blitz.** Verified false. Blitz v0.2+ is an *independent* HTML/CSS engine built from Stylo (Mozilla MPL-2.0 component, not a fork) + Taffy + Parley + Vello. The pre-0.2 v0.1 branch is archived. See [`targets.md`](targets.md).

## Contents

| File | Subject |
|---|---|
| [`README.md`](README.md) | This file — overview, key facts, ToC, framing disclosure. |
| [`lessons.md`](lessons.md) | **Consult-this-when-designing decision file.** Validates / Avoid / Borrow. |
| [`glossary.md`](glossary.md) | Dioxus-specific terms. |
| [`architecture.md`](architecture.md) | `VirtualDom`, components-as-functions, hooks, renderer abstraction, scheduler. |
| [`rsx-macro.md`](rsx-macro.md) | The `rsx!` proc-macro; syntax; capabilities; JSX/HTML/BSN comparison. |
| [`signals-and-state.md`](signals-and-state.md) | Signals (0.5+), Stores (0.7+), `Copy`-by-default, comparison vs React `useState` / Solid / Sycamore. |
| [`targets.md`](targets.md) | Web (DOM/WASM), Desktop (Webview / WGPU-Blitz), Mobile (native + WASM), maturity-per-target. |
| [`integration-with-taffy.md`](integration-with-taffy.md) | Blitz's Taffy usage; not present in web target; cross-reference to [`prior-art/taffy/`](../taffy/). |
| [`history.md`](history.md) | Genesis (2021), 0.1 → 0.7 trajectory, Subsecond hot-patching, Stores, YC S23. |
| [`governance.md`](governance.md) (combined w/ distribution) | DioxusLabs incorporation, funding, contributors, license, Cargo features, distribution. |
| [`ecosystem.md`](ecosystem.md) (combined w/ comparisons) | Users in production; vs Yew, Leptos, Sycamore, egui, Iced, React/Solid; vs Buiy positioning. |
| [`open-problems.md`](open-problems.md) (combined w/ critiques) | Multi-target fragmentation, Blitz pre-alpha, hot-reload reliability, a11y gap, bundle size, SSR maturity. |

## How to use

This corpus exists to inform Buiy's reactivity, authoring-DSL, and Taffy-integration decisions — not to evaluate Dioxus as a substrate. Start at [`lessons.md`](lessons.md); the other files are evidence.

**Framing disclosure.** These docs are written from a **Bevy-ECS-native, parallel-to-`bevy_ui`, no-signals-in-v1, BSN-friendly-by-construction** Buiy stance — most "Implications for Buiy" framings read Dioxus's choices through that lens. Dioxus's framework-runtime model (its own scheduler, its own VDOM, its own renderer abstractions) is **structurally not what Buiy is**. Future readers auditing whether the parallel-to-`bevy_ui`-with-no-signals stance is itself the right primitive should weigh the corpus accordingly: it's a learn-from-Dioxus-into-Buiy artifact, not a neutral catalog. In particular, Dioxus's success at multi-target deployment is a **warning sign** for Buiy ([`open-problems.md`](open-problems.md) § "Multi-target fragmentation"), not an aspiration — the cost-per-target is high and quality per-target is uneven.

## Sources

- crates.io API for `dioxus` (verified 2026-05-22): https://crates.io/api/v1/crates/dioxus
- crates.io API for `dioxus/0.7.9` (license, MSRV, publisher): https://crates.io/api/v1/crates/dioxus/0.7.9
- Repo + README: https://github.com/DioxusLabs/dioxus/
- Homepage: https://dioxuslabs.com
- Blitz repo + README: https://github.com/DioxusLabs/blitz
- Dioxus 0.7 release notes: https://dioxuslabs.com/blog/release-070
- Dioxus 0.6 release notes: https://dioxuslabs.com/blog/release-060
- Dioxus 0.5 release notes: https://dioxuslabs.com/blog/release-050
- Y Combinator company page: https://www.ycombinator.com/companies/dioxus-labs
- DioxusLabs GitHub org: https://github.com/DioxusLabs
- Tracxn funding profile (cross-check): https://tracxn.com/d/companies/dioxus
- Crunchbase profile (cross-check): https://www.crunchbase.com/organization/dioxuslabs
- Sibling prior-art folder: [`../taffy/`](../taffy/)
- Buiy foundation: [`../../specs/2026-05-07-buiy-foundation/`](../../specs/2026-05-07-buiy-foundation/)
