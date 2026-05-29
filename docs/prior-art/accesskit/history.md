**Date:** 2026-05-22
**Status:** active
**Subject:** AccessKit — chronological history from 2021 announcement through the 2026-05-11 iOS 0.1.0 release, with platform-adapter landing dates and adoption milestones

## Genesis (2021)

AccessKit was started by Matt Campbell in 2021 (per the [Pneuma Solutions about page](https://pneumasolutions.com/about/): "Campbell also initiated the AccessKit project in 2021"). Campbell is co-founder and CTO of Pneuma Solutions, a cloud-accessibility company he co-founded with Mike Calvo in 2020 after the two had previously worked together at Serotek Corporation (early-2000s accessibility-software company). Campbell's prior work spans Linux accessibility (late 1990s onward), the Serotek "System Access to Go" browser-based screen reader, and the Windows Accessibility Team at Microsoft.

**Correction vs the brief preamble:** The preamble described Campbell as a "former NVDA developer." That is not corroborated by [NV Access's about page](https://nvaccess.org/about-nv-access/), which lists Michael Curran (creator) and James Teh (past co-lead) as the NVDA developers and does not mention Campbell. Campbell's screen-reader résumé is Serotek + Microsoft, not NVDA. This folder uses "co-founder and CTO of Pneuma Solutions, founder of AccessKit, prior screen-reader work at Serotek and Microsoft" as the canonical bio.

**Why Rust?** Per the upstream README: "We've chosen Rust for its combination of reliability and efficiency, including safe concurrency, which is especially important in modern software." The schema being Rust-canonical also gives bindings to C and Python the option to consume it without serialization overhead when calling Rust functions directly.

**Why cross-platform?** Three platform accessibility APIs (Windows UIA, macOS NSAccessibility, Linux AT-SPI D-Bus) and three mobile ones (Android, iOS, web) each require a substantial implementation. The pitch (per [accesskit.dev](https://accesskit.dev/)): "a cross-platform, cross-language abstraction over accessibility APIs, so toolkit developers only have to implement accessibility once." The Chromium accessibility abstraction (`ui::AXNode`) is the architectural lineage — AccessKit's data schema is "based largely on Chromium's cross-platform accessibility abstraction" per the README.

## 0.1 → 0.24 timeline (core `accesskit` crate)

The total count is 39 versions of the core crate on crates.io as of 2026-05-22 ([crates.io accesskit](https://crates.io/crates/accesskit)). Recent versions with dates:

| Version | Date | Notable |
|---|---|---|
| 0.24.0 | 2026-02-01 | Current at folder writing. |
| 0.23.0 | 2026-01-15 | |
| 0.22.0 | 2026-01-03 | |
| 0.21.1 | 2025-10-02 | |
| 0.21.0 | 2025-07-16 | egui bumped to this in egui 0.32.0 (2025-07-10). |
| 0.20.0 | 2025-06-26 | |
| 0.19.0 | 2025-05-07 | |
| 0.18.0 | 2025-03-06 | |
| 0.17.1 | (earlier) | |
| 0.17.0 | (earlier) | |

Older milestones (before 0.18, exact dates not directly verified for this folder — recommend a follow-up gh-API sweep at next refresh):

- **Initial 0.1** — common types + Windows adapter. The Windows adapter was first because UIA was the primary platform Campbell already had production experience with from Microsoft.
- **macOS adapter** — landed at 0.2-era; first egui blog post by Campbell ([emilk/egui PR #2294](https://github.com/emilk/egui/pull/2294), 2022-12-04) noted "AccessKit is only implemented for Windows so far" with macOS planned next. macOS first shipped as `accesskit_macos` shortly after.
- **Linux AT-SPI adapter** (`accesskit_unix`) — added later, on top of the pure-Rust `zbus` D-Bus implementation (verify exact landing version against `accesskit_unix` crates.io timeline at next refresh).
- **Android adapter** (`accesskit_android`) — landed pre-2026. Status as of 0.7.3 (2026-05-11): per the upstream README's status caveat, "the released adapters are all at rough feature parity" but "don't yet support all types of UI elements" — i.e., the Android adapter ships, but does not cover every Role.
- **iOS adapter** (`accesskit_ios`) — **v0.1.0 released 2026-05-11**, the most recent platform milestone (verified against [the GitHub releases page](https://github.com/AccessKit/accesskit/releases)). Until 2026-05-11, iOS was "in-progress upstream"; this is the version where it became a shipping adapter, though "v0.1.0" semver implies pre-stable. The Buiy [`architecture.md § 2.9`](../../specs/2026-05-07-buiy-foundation/architecture.md) note that iOS is "currently in-progress upstream" reflects the pre-2026-05-11 state and should be updated to "iOS v0.1.0 just shipped, treat as alpha for v1 platform support" in the next Buiy spec refresh.
- **Web adapter** — listed as "planned" in the README; no shipped crate. This is the single biggest open AccessKit platform gap and is called out under [`critiques.md`](critiques.md) and the Buiy spec's [`architecture.md § 2.9`](../../specs/2026-05-07-buiy-foundation/architecture.md).
- **`accesskit_winit`** — the cross-platform winit-integration adapter consolidated as the recommended embedding path. Current version: 0.33.0 (2026-05-11), which itself "Added Basic iOS adapter" support to the unified winit integration. The winit adapter has its own version stream because it bumps with both `winit` and `accesskit` releases.

## Notable architecture revisions

- **NodeClass shared-style optimization** — AccessKit's `Node`/`NodeBuilder` historically did per-instance allocation. A "node class" optimisation introduced shared style data (a `NodeClass` pool) to amortize allocation across nodes with identical style profiles. Exact landing version not pinned in this folder; flag for verification on next refresh.
- **`accesskit::Node` consolidation** — the `Node` and `NodeBuilder` split was simplified (the historical pattern was build via `NodeBuilder`, freeze to `Node`; current API exposes `Node` directly with setter methods). Exact landing version not pinned.
- **`ActivationHandler` / `ActionHandler` / `DeactivationHandler` triplet** — the lazy-tree gate (build the initial tree only when an AT activates) is implemented via the `ActivationHandler` trait. This was the architectural shape that made egui-style immediate-mode integration affordable.

## Adoption milestones

- **egui** — [PR #2294](https://github.com/emilk/egui/pull/2294) by Matt Campbell merged **2022-12-04**, Windows-only initially, enabled by default in eframe. The first major downstream and the reference implementation for immediate-mode-GUI AccessKit integration. egui's changelog explicitly tracks AccessKit version bumps (e.g. egui 0.32.0 on 2025-07-10 bumped to accesskit 0.21.0).
- **Bevy** — [PR #6874](https://github.com/bevyengine/bevy/pull/6874) merged **2023-03-01**, shipped in Bevy **0.10.0**. Created the `bevy_a11y` crate. Current `main` depends on accesskit 0.24.
- **Slint** — Accessibility primitives shipped in [v0.2.5 on 2022-07-06](https://github.com/slint-ui/slint/blob/master/CHANGELOG.md) (`accessible-*` properties on the `.slint` markup). AccessKit explicitly named in the changelog at v1.7.0 (2024-07-18) when the winit backend bumped to accesskit 0.16. Ongoing investment visible in 2025-11-03 PR #9919.
- **Freya** — depends on `accesskit 0.24.0` + `accesskit_winit 0.32.0` as a workspace dependency ([Freya Cargo.toml](https://raw.githubusercontent.com/marc2332/freya/main/Cargo.toml)). Tracks AccessKit closely.
- **Xilem / Masonry** — uses AccessKit as a foundational dependency ("AccessKit for plugging into accessibility APIs", per the [Xilem README](https://github.com/linebender/xilem)). Linebender's UI stack (Masonry lower-level, Xilem reactive on top) is built on `winit + Vello + wgpu + Parley + Fontique + AccessKit`. Druid (Xilem's predecessor) is **discontinued** ([Druid README](https://github.com/linebender/druid)) and never gained an AccessKit integration in its active lifetime.
- **Iced** — **not yet shipped.** As of 2026-05-22, AccessKit integration is in draft (PR #3111 opened 2025-11-11, plus earlier closed/WIP PRs). Iced 0.14.0 (2025-12-07) shipped without AccessKit. The brief preamble listed Iced as an adopter — that is wrong as of this date.
- **Tauri** — does **not** use AccessKit. Tauri renders via WebView (`tao` + `wry`); the system webview provides accessibility via the OS's native a11y for the embedded browser engine.
- **GPUI (Zed editor)** — no AccessKit integration visible in the [Zed repo](https://github.com/zed-industries/zed) or GPUI documentation as of 2026-05-22. The brief preamble's "GPUI (Zed)" entry was a "verify" hedge; verification returns negative.

## Language bindings

- **C bindings** — [`AccessKit/accesskit-c`](https://github.com/AccessKit/accesskit-c). Header generated via `cbindgen`. CMake + Meson integration. Current: v0.21.2 (2026-03-11).
- **Python bindings** — `accesskit` on PyPI, implemented via PyO3. The `accesskit` core crate has a `pyo3` feature flag (gated on `pyo3 = "0.26"`).

## Pneuma Solutions formation, commercial context

Pneuma Solutions was founded **2020** by Matt Campbell (CTO, co-founder) and Mike Calvo (CEO, co-founder), per the [Pneuma Solutions about page](https://pneumasolutions.com/about/). Both founders had previously worked together at Serotek Corporation (founded 2001 by Calvo). Pneuma ships commercial accessibility products (Remote Incident Manager / RIM, Scribe document remediation, Sero, DocuScan Plus); AccessKit appears to be open-source community work by Campbell rather than a direct commercial product.

The team is described on the company about page as "most of us are blind or visually impaired ourselves" — Campbell is described as a visually impaired developer, which is the lived-experience angle Pneuma emphasises.

There is **no public funding announcement** for AccessKit specifically (no Series-A press, no foundation grant tied to AccessKit by name). The project's contributors and copyright holders, per the [AUTHORS file](https://raw.githubusercontent.com/AccessKit/accesskit/main/AUTHORS), are:

- Matt Campbell
- Arnold Loubriat
- Google LLC
- Leonard de Ruijter

Google LLC's presence in AUTHORS indicates Google-employee contributions large enough that Google is the copyright holder — the most likely source is Chromium-derived code (the README explicitly notes "significant portions of AccessKit are derived from Chromium" under its BSD-style license).

## Talks and public communications

Hedge: a comprehensive talk inventory was not assembled for this folder. AccessKit has been discussed at Rust-ecosystem events (RustConf-adjacent, egui-related streams) but a verbatim list of named talks is not pinned. Recommend gathering this on the next folder refresh by searching the official AccessKit blog (if it exists at `accesskit.dev/blog`, which returns 404 as of 2026-05-22) and conference archives.

## Cross-links

- Current integration pattern: [`integration.md`](integration.md).
- Current API surface: [`api.md`](api.md).
- Current capabilities and gap list: [`capabilities.md`](capabilities.md).
- Open problems including the iOS / Android / web adapter readiness story: [`critiques.md`](critiques.md).
- Governance / funding / stewardship: [`governance.md`](governance.md).

## Sources

- https://accesskit.dev/
- https://crates.io/crates/accesskit
- https://github.com/AccessKit/accesskit/releases
- https://github.com/AccessKit/accesskit/blob/main/README.md
- https://raw.githubusercontent.com/AccessKit/accesskit/main/AUTHORS
- https://github.com/emilk/egui/pull/2294
- https://github.com/emilk/egui/blob/main/CHANGELOG.md
- https://github.com/bevyengine/bevy/pull/6874
- https://github.com/slint-ui/slint/blob/master/CHANGELOG.md
- https://github.com/iced-rs/iced/pulls?q=accesskit
- https://github.com/linebender/xilem
- https://github.com/linebender/druid
- https://pneumasolutions.com/about/
- https://nvaccess.org/about-nv-access/
- /home/user/buiy/docs/specs/2026-05-07-buiy-foundation/architecture.md
