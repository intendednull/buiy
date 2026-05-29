**Date:** 2026-05-29
**Status:** active
**Subject:** Servo / Stylo — chronological timeline from 2012 Mozilla Research origins through Quantum CSS, WebRender, the 2020 layoffs, and the Igalia-led Linux Foundation Europe revival

# Servo / Stylo — History

A dated timeline of the Servo engine, its Stylo style system, and the WebRender renderer. The throughline is a single governance lesson — a corporate steward funded a large Rust codebase, withdrew, and a community-plus-Igalia arrangement revived it — analysed in [governance.md](governance.md). All version numbers and dates below are verified against primary sources (see Sources); where a fact could not be pinned precisely it is flagged in the text rather than guessed.

## Timeline

### 2012 — Origin at Mozilla Research
Development of Servo began at the Mozilla Corporation (Mozilla Research) in 2012, as an R&D effort to build "an independent, modular, embeddable web engine" written in Rust. Servo co-evolved with the Rust language and was the first large non-compiler Rust codebase — it functioned as Rust's biggest early proving ground for memory-safety and fearless-concurrency claims.

### 2013–2016 — Parallel-everything experiments
The original layout engine (retrospectively the "legacy" layout, sometimes "layout 2013") and the parallel style system were built. The driving thesis: use Rust's safe concurrency to parallelise work browsers traditionally ran single-threaded — selector matching, the cascade, and layout. The bet was that Rust's ownership model would make data-parallel layout/style *safe to attempt at all*, where C++ engines had largely given up on parallel layout due to data-race risk. Stylo (the style system) and WebRender (the GPU renderer) emerged as the two components mature enough to be useful outside Servo itself; parallel *layout*, the harder problem, never reached the same maturity and is not the part Firefox adopted.

### 2016-10 — Project Quantum announced
In October 2016 Mozilla announced "Project Quantum": a plan to fold Servo's most production-ready components into Gecko incrementally rather than ship Servo as a wholesale Firefox replacement. Three components were named — **Stylo** (Quantum CSS), **WebRender** (Quantum Rendering), and **Quantum DOM** (background-tab responsiveness). This reframed Servo's value: the engine became a *parts supply* for Firefox. Stylo and WebRender were the two that actually shipped to users; this decision is why those components survived 2020 while the engine did not.

### 2017-11-14 — Quantum CSS ships in Firefox 57
Stylo was upstreamed into Firefox's Gecko engine and shipped — enabled by default on desktop — as **"Quantum CSS"** in **Firefox 57** (release name "Quantum"), released **2017-11-14**. This is Servo's highest-impact outcome: a Rust component reached hundreds of millions of users inside a shipping C++ browser. Stylo's parallel cascade (via `rayon`), the rule tree, the style-sharing cache, and Bloom-filter ancestor matching are detailed in [stylo.md](stylo.md). The mechanism matters for the governance arc: the *component* gained independent longevity by entering a product, while the *engine* stayed experimental.

### 2019-05-21 — WebRender ships in Firefox 67
WebRender — Servo's retained-scene, batched, GPU display-list renderer — began rolling out in **Firefox 67** (released 2019-05-21), initially to a narrow slice of users (Windows 10 + NVIDIA), then progressively widened (largely complete by Firefox 92 in 2021). Like Stylo, WebRender's survival was secured by Firefox adoption, not by Servo. Its architecture is covered in [rendering.md](rendering.md).

### 2020-08-11 — Mozilla layoffs; the Servo team is cut
Mozilla announced layoffs of roughly **250 employees (~25% of its workforce)** "to adapt its finances to a post-COVID-19 world and re-focus the organization on new commercial services." The **entire Servo team was cut**. Paid engineering on the project stopped effectively overnight. This is the corporate-steward-withdrawal event at the centre of the Buiy governance lesson.

### 2020 (Nov) — Stewardship to the Linux Foundation
Following the layoffs, "stewardship of Servo moved from Mozilla Research to the Linux Foundation in 2020" — a custodial transfer of the trademark, repositories, and copyright to a neutral home. This was **not** a re-funding: it preserved the assets but did not pay engineers. (Note: this was the *Linux Foundation*, not Linux Foundation Europe, which did not yet exist — sources that say "LF Europe in 2020" are imprecise.) Development stayed near-dormant through 2021 and most of 2022.

### 2023-01-16 — Reactivation announced
Servo announced "new external funding" that "enabled a team of developers to reactivate the project," with the first task being "to reactivate the project and the community around it." The 2023 roadmap focused on choosing between the two extant layout engines and reaching basic CSS2 conformance. This external funding was Igalia's involvement, made explicit later in the year.

### 2023 — "Layout 2020" chosen; legacy layout removed
For most of Servo's life two layout engines coexisted: the original "legacy"/"layout 2013" engine and a newer rewrite. The 2023 roadmap's first structural decision was to stop maintaining both. The modern **"layout 2020"** rewrite — a formatting-context- and fragment-tree-based design closer to how the CSS specs are actually written — was selected for continued development, and the legacy engine was removed. Carrying two layout engines had been a maintenance tax the revived (smaller) team could not afford; consolidating on one was a precondition for progress. Layout 2020 owns its **own** block / inline / table / float algorithms, but it **delegates flexbox and CSS grid to Taffy** (the engine Buiy depends on) via `components/layout/taffy/` and the `stylo_taffy` adapter — so "Servo does not use Taffy" is wrong; Servo *embeds* Taffy for two of its formatting contexts. See [layout.md](layout.md) §1 for the formatting-context model and [../taffy/](../taffy/) for the contrast with how Buiy stacks passes above whole-tree Taffy.

### 2023-09-07 — Joins Linux Foundation Europe
Servo "officially joined Linux Foundation Europe," with renewed activity "led by Igalia, a Linux Foundation Europe member that now has a team of engineers working on the project." This is the formal organisational home of the revived project (distinct from the 2020 Linux Foundation custody). Governance now runs through a **Technical Steering Committee (TSC)**.

### 2024 — Community funding + feature catch-up
- **2024-03-12** — Servo Open Collective + GitHub Sponsors launched (donations fund CI infrastructure, not salaries; see [governance.md](governance.md)).
- Features added across 2024 (Igalia-led): CSS **Flexbox**, **tables**, **floats**, WOFF2 fonts, color emoji, tabbed browsing in the `servoshell` reference browser, gamepad support, improved **WebGPU** (wgpu upgraded from 0.6 to 0.16), plus work on Shadow DOM, `ReadableStream`, and WebXR. New platforms: **Android** and **OpenHarmony**.
- **By end of 2024:** Servo passed **1,515,229 WPT subtests (~79%)**; **129 unique contributors** (+143%) landed **1,771 PRs** (+163%); Igalia made 679 commits / 26% of PRs.

### 2024 (Feb) — FOSDEM "reboot" framing
At FOSDEM 2024 the Servo team outlined a "reboot," positioning the engine as a *practical embeddable rendering engine* (think: a web-content surface other applications can host) rather than a Firefox competitor. This embeddable-engine framing is directly relevant to Buiy, which is itself an embeddable retained-mode UI surface for Bevy applications — both projects sell "render web-shaped content inside my app" rather than "be the whole browser."

### 2025 — Versioned releases begin
- The `stylo` crate continued independent releases; **v0.17.0** was published 2026-05-06 (verified against the crates.io versions API for `stylo`; repo `github.com/servo/stylo`), with sibling crates `selectors` (0.38.0) and `cssparser` (0.37.0) — Stylo now has a life of its own as a reusable crate, consumed by [Blitz](#blitz) below. (The crate ships roughly monthly: v0.16.0 was 2026-04-12, v0.15.0 2026-04-01 — so a recent point release is expected; re-check crates.io for the current `max_version`.)
- **2025-10** — **Servo v0.0.1** was released, the first numbered release, featuring Apple-silicon support; later 2025 point releases (e.g. 0.0.5) added items like post-quantum cryptography. The numbered-release cadence signals the revived project treating Servo as a shippable embeddable engine, not only a research vehicle.

<a name="blitz"></a>
### Parallel track — Stylo extracted into Blitz
Independent of Servo's own browser, the standalone `stylo` crate is consumed by **Blitz** (DioxusLabs, `github.com/DioxusLabs/blitz`) — a modular HTML/CSS renderer that combines **Stylo** (CSS resolution) + **Taffy** (box layout) + a GPU renderer (Vello/`anyrender`; Blitz's own README also references Parley for text). Blitz is the closest existing analogue to Buiy's substrate — Stylo+Taffy+GPU — *minus* Bevy and ECS. The divergences: Blitz uses **Stylo** (MPL-2.0) for a full CSS resolver where Buiy implements a typed-Rust CSS subset itself; Blitz targets HTML documents where Buiy targets ECS-authored UI. See [../dioxus/integration-with-taffy.md](../dioxus/integration-with-taffy.md) and [../taffy/ecosystem.md](../taffy/ecosystem.md).

## What the timeline teaches Buiy

- **Components outlive engines when they enter a host product.** Stylo and WebRender survived the 2020 collapse because Firefox shipped them; the engine itself went dormant. Buiy's longevity argument is the same: be useful *inside Bevy*, not as a standalone island.
- **Custody ≠ funding.** The 2020 Linux Foundation transfer preserved the IP but produced ~2 years of near-zero development. A neutral legal home protects assets but does not keep code moving — paid engineers do. Buiy should not mistake a permissive license or a foundation home for project health.
- **CSS-faithfulness has a reference implementation here.** Servo's "layout 2020" is a from-scratch Rust implementation of the same W3C modules Buiy implements as a subset (Display 3, Positioned Layout, Containment 3, Writing Modes 4). Servo is a real-world check on how those specs interact — especially fragment-tree formation and stacking, relevant to Buiy's Phase 9 stacking + top-layer work in [../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md).

## Sources

- Servo (software), Wikipedia: https://en.wikipedia.org/wiki/Servo_(software)
- "Servo to Advance in 2023", servo.org: https://servo.org/blog/2023/01/16/servo-2023/
- "Servo web rendering engine joins Linux Foundation Europe", linuxfoundation.eu: https://linuxfoundation.eu/newsroom/servo-web-rendering-engine-joins-linux-foundation-europe
- "Servo in 2024: stats, features and donations", servo.org: https://servo.org/blog/2025/01/31/servo-in-2024/
- "You can now sponsor Servo…", servo.org: https://servo.org/blog/2024/03/12/sponsoring-servo/
- "Servo Revival: 2023-2024", Igalia: https://blogs.igalia.com/mrego/servo-revival-2023-2024/
- Firefox 57 (Quantum) release notes / MDN: https://developer.mozilla.org/en-US/docs/Mozilla/Firefox/Releases/57
- Project Quantum (2016-10), Quantum (Mozilla)/Gecko, Wikipedia: https://en.wikipedia.org/wiki/Quantum_(Mozilla)
- "Firefox WebRender Rollout begins with Firefox 67", gHacks (2019-05-20): https://www.ghacks.net/2019/05/20/firefox-webrender-rollout-begins-with-the-release-of-firefox-67/
- "Mozilla lays off 250 employees…", gHacks (2020-08-11): https://www.ghacks.net/2020/08/11/mozilla-lays-off-250-employees-in-massive-company-reorganization/
- `stylo` crate (v0.17.0, MPL-2.0, servo org): https://crates.io/crates/stylo
- Blitz (DioxusLabs): https://github.com/DioxusLabs/blitz
- Buiy stacking + top-layer spec: [../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md)
- Sibling: [governance.md](governance.md), [stylo.md](stylo.md), [layout.md](layout.md), [rendering.md](rendering.md), [../dioxus/integration-with-taffy.md](../dioxus/integration-with-taffy.md)
