**Date:** 2026-05-22
**Status:** active
**Subject:** Slint — governance (SixtyFPS GmbH stewardship), triple-license model (GPL + royalty-free + commercial), Cargo features, platform-support matrix

# Governance and distribution

This file combines two structurally linked topics: how Slint is *governed* (single-vendor open-core, no foundation, no charter) and how it's *distributed* (triple-licensed crate; multi-language packaging; per-renderer + per-backend Cargo features). The two are entangled because Slint's commercial-license revenue stream is the explicit reason the project can operate at scale without external grants.

## Stewardship

- **Legal entity**: **SixtyFPS GmbH**, registered in Brandenburg state, Germany. Founded 2020 by Olivier Goffart, Simon Hausmann, and Aurindam Jana (see [`history.md`](history.md)). The legal entity is the copyright holder on every Slint commit and the named licensor on the commercial license terms.
- **Trademark**: "Slint" and the Slint logo are SixtyFPS GmbH trademarks.
- **Team**: small (~5–10 named team members on the about-us page as of folder-writing); remote-first across Germany, Finland, USA. Co-founders all directly contributing to the codebase per public GitHub history.
- **Governance model**: **single-vendor open core**, no foundation, no formal RFC process. Architectural decisions are made by the SixtyFPS team. External contributions are welcomed and merged through standard pull-request review, but ownership of design direction is unambiguous.
- **Rust Foundation membership**: Slint joined as a **Silver Member** (Rust Foundation member spotlight, February 2023).
- **External funding visible**: no public Series-A / venture round; no NLnet / Sovereign-Tech-Fund grants tagged to Slint by name in public sources. Revenue model is **commercial license sales** + **support contracts** + **partner ecosystem** (KDAB, Spyrosoft, Crossware, etc. do the integration consulting).

## Governance critiques worth surfacing

- **Bus factor**: Goffart's GitHub activity dominates the commit graph. If the founders were to leave / pivot, the project's future direction is uncertain. There's no governance succession plan visible.
- **No public roadmap commitment**. The blog announces features as they land; there's no quarterly roadmap RFC or public design doc archive (`docs/specs/` analog).
- **Foundation gap**. AccessKit has a similar single-vendor concentration ([`../accesskit/governance.md`](../accesskit/governance.md)); Iced has no foundation; egui is Emil Ernerfelt's personal project with consulting support. The pattern across the Rust UI ecosystem is "small, founder-led, no foundation." Slint is the most commercial of the bunch.

## The triple-license model

This is the most-discussed Slint distribution fact. crates.io metadata as of 1.16.1:

```
license = "GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0"
```

Three options, the user picks one (the SPDX `OR` semantics):

### 1. GPL-3.0-only

Slint as GPL-3.0 — the standard copyleft option. Any code that links against Slint must itself be available under a GPL-3.0-compatible license. Suitable for: GPL-licensed open-source projects, free-software hobby work, internal tools that never ship to third parties.

### 2. `LicenseRef-Slint-Royalty-free-2.0`

Added in **1.1.0 (June 2023)**. Permits proprietary **desktop** use without buying a commercial license, subject to attribution + a few terms (notably: distributing apps that compete with Slint itself is excluded; the Slint splash screen / attribution in About menu may be required depending on terms). Read the license file directly for the binding text.

This is the option most proprietary desktop Slint users land on. The royalty-free terms explicitly **exclude embedded and mobile deployment** — proprietary use on those platforms still requires the commercial license.

### 3. `LicenseRef-Slint-Software-3.0`

The **paid commercial license**. Bilateral contract between SixtyFPS GmbH and the licensee. Required for:

- Proprietary embedded systems (industrial automation, medical devices, automotive, aerospace).
- Proprietary mobile applications (iOS + Android).
- Use cases not covered by the royalty-free terms.

Pricing is not published; sales are direct. KDAB and other service partners can resell.

## Why the model matters for Buiy

A game-engine UI library inhabits a different licensing ecosystem than Slint. Bevy itself is dual-MIT OR Apache-2.0; the Bevy ecosystem expects that default. **Buiy is committed to MIT OR Apache-2.0**:

- Game studios shipping proprietary commercial games don't want a GPL gate (most game engines and game-dev tools are MIT / Apache / BSD / proprietary; GPL is rare).
- The royalty-free / commercial distinction Slint draws (desktop free, embedded paid) doesn't map — Buiy's target deployment is desktop + mobile + browser-via-Bevy-WASM, all paying tiers in Slint's model.
- Buiy is intentionally not building a commercial open-core business — Buiy is a library for the Bevy ecosystem to use freely.

The triple-license is therefore a **hard incompatibility** between Slint's product and Buiy's mission. This is the load-bearing licensing finding of the entire corpus.

## Cargo features

Slint's Cargo features partition the renderer / backend / language-binding surface so embedded builds can drop everything they don't need:

```toml
[dependencies.slint]
version = "1.16"
default-features = false
features = [
  "compat-1-2",           # API stability tier
  "std",                  # standard library (omit for no_std MCU)
  "backend-winit",        # winit-based windowing
  "renderer-skia",        # Skia GPU renderer
  # alternatives:
  # "renderer-femtovg",
  # "renderer-femtovg-wgpu",
  # "renderer-software",
  # "backend-qt",
  # "backend-android-activity-05",
  # "backend-linuxkms",
]
```

Selected features (1.16-era; verify against current docs.rs before lifting):

- **Backends**: `backend-winit`, `backend-qt`, `backend-android-activity-05`, `backend-linuxkms`, `backend-software` (MCU).
- **Renderers**: `renderer-skia`, `renderer-femtovg`, `renderer-femtovg-wgpu`, `renderer-software`.
- **`std` / `no_std`**: opt-in / opt-out for the standard library.
- **Live preview / interpreter**: `compat-1-x` API-compatibility tiers; `slint-interpreter` is a separate crate.
- **wgpu version**: `wgpu_27`, `wgpu_28` — choice of wgpu major bound at compile time.

The feature surface is one of the most carefully managed in the Rust ecosystem; embedded builds routinely drop to `std = false, backend-software, renderer-software, no qt, no winit` and get a binary in the low hundreds of kilobytes.

## Platform-support matrix

| Platform | Status | Backend / renderer combo |
|---|---|---|
| Windows | Production | winit + Skia (default since 1.14); Qt optional |
| macOS | Production | winit + Skia (default since 1.14); Qt optional |
| Linux (X11/Wayland) | Production | winit + FemtoVG; LinuxKMS for direct framebuffer |
| Linux (KMS, no compositor) | Production | LinuxKMS + software / FemtoVG-WGPU (since 1.16) |
| Android | Production (since 1.5) | `backend-android-activity-05` |
| iOS | Beta (since 1.10) | iOS native; Simulator wheels for Python (1.12+) |
| WASM (browser) | Production for visual; a11y partial | winit (wasm) + FemtoVG / Skia |
| ESP-IDF (ESP32) | Production | `backend-software` + software renderer + RGB565 |
| STM32 (Cortex-M) | Production (since 1.8) | `backend-software` + software renderer |
| QNX | Production via partner | partner-integrated; commercial license |
| Zephyr | Partner-supported | partner-integrated |

The matrix breadth is unusual; few other Rust UI toolkits ship to MCUs, iOS, and the browser from one codebase.

## Distribution shapes per host language

- **Rust**: `cargo add slint` from crates.io. Codegen via `slint!` macro or `slint_build::compile()` in `build.rs`.
- **C++**: SixtyFPS distributes prebuilt binaries; CMake integration via `find_package(Slint)`; codegen via CMake.
- **JS / Node.js**: `npm install slint-ui` ships a native module via napi-rs (port to napi-rs 3.0 in 1.16.0).
- **Python**: `pip install slint` ships a PyO3-backed wheel; supports asyncio (added in 1.9.0); Python bindings became "stable" in 1.13.0 (September 2025). iOS Simulator wheels added in 1.12.0.

The compiler is the same across languages — the language is the DSL, not the host.

## Implications for Buiy

- **Buiy's MIT OR Apache-2.0 commitment is structurally incompatible with Slint's model.** This is the single most important governance fact in the corpus. Future Buiy spec authors evaluating "could we adopt Slint" should treat the answer as "no, the license model is incompatible with Bevy ecosystem norms" without needing to evaluate technical fit.
- **Single-vendor open-core is a viable governance model.** SixtyFPS GmbH ships a real product with real customers and a real revenue stream, without a foundation or external grants. The data point is "this works at this scale"; the data point isn't "Buiy should be open-core" — Buiy's plan is foundation-style (specs + plans in tree, MIT/Apache, community-owned). Both shapes ship; pick deliberately.
- **Cargo-features partitioning is a model for Buiy's crate split.** Slint's per-renderer / per-backend / per-language feature surface lets embedded users drop everything; Buiy's [foundation README § 5 open question on `buiy_core` vs `buiy_render` / `buiy_a11y` / `buiy_layout` / `buiy_focus` / `buiy_theme` split](../../specs/2026-05-07-buiy-foundation/README.md) is the same shape of decision. Slint validates that a careful feature split is worth the maintenance cost — embedded-grade builds *require* it; desktop builds *benefit* from it.
- **Platform-support breadth is achievable from one codebase, at a maintenance cost.** Slint supports Windows / macOS / Linux / Android / iOS / WASM / MCU from one source tree. Buiy's foundation README § 5 open question "Platform support staging" is the same decision. The data point: it's achievable; it's a multi-year build-up (Slint shipped Android in 1.5, iOS in 1.10 — 5 minors apart). Don't expect "v1 on all platforms" if Slint's experience is the baseline.
- **Bus-factor concentration is the open-core risk.** Slint's commercial-license revenue underwrites the team; a founder departure would not trivially be replaceable. Buiy's foundation-style governance plan (community-owned, MIT/Apache) sidesteps this — at the cost of having no revenue stream to underwrite full-time maintainer attention.

## Sources

- crates.io `slint` license metadata: https://crates.io/crates/slint
- Slint LICENSES directory: https://github.com/slint-ui/slint/tree/master/LICENSES
- Slint About Us: https://slint.dev/about-us
- Slint partners: https://slint.dev/partners
- Rust Foundation member spotlight: https://rustfoundation.org/media/member-spotlight-slint/
- Slint 1.1 royalty-free announcement: https://slint.dev/blog/slint-1.1-released
- Slint Cargo features (docs.rs): https://docs.rs/slint/latest/slint/
- Buiy foundation README § 5 (open questions): [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Sibling files: [`history.md`](history.md), [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md), [`open-problems.md`](open-problems.md)
