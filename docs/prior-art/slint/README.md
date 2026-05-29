**Date:** 2026-05-22
**Status:** active
**Subject:** Slint — declarative GUI toolkit (Rust + C++ + JS + Python) targeting embedded + desktop; commercial open-core via triple-licensed model

# Slint

Slint is a declarative GUI toolkit built around a custom DSL (`.slint` files) that compiles to native UI for Rust, C++, JavaScript/Node.js, and Python applications. It targets embedded devices (down to bare-metal MCUs) and desktop, with experimental mobile and WASM backends. Slint is the product of **SixtyFPS GmbH** — the legal entity that still holds the trademark and copyright — founded in Germany in 2020 by **Olivier Goffart, Simon Hausmann, and Aurindam Jana**, all of whom previously worked together on Qt at Trolltech / The Qt Company. The project was named *SixtyFPS* until February 2022 when it rebranded to Slint ("Straightforward, Lightweight, Native Toolkit").

Slint is the closest mainstream Rust-ecosystem example of the design shape Buiy is *not* — a **DSL-first authoring layer above a retained-mode runtime, distributed under an open-core triple-license model**. The corpus exists to give Buiy spec authors a version-pinned reference on the parts of Slint's design we *do* want to borrow (AccessKit-first integration patterns, the property-binding declarative shape, embedded-grade allocation discipline) and the parts we explicitly *don't* (GPL+commercial dual gate, DSL-only authoring, single-window popup limitations).

## Honest assessment

**Strong points.**

- **Shipped 1.0 (2023-04-05), now at 1.16.1 (2026-04-23).** ~3 years of post-1.0 stability with monthly-to-quarterly minor releases. 22.7k GitHub stars; 1.1M lifetime crate downloads; 236k recent-90-day downloads.
- **Verified production users in safety-critical industrial sectors.** OTIV (rail automation) is the most-cited; KDAB lists Slint as a service-partner offering. Espressif, STMicroelectronics, Raspberry Pi, Toradex, QNX, Yocto, and Zephyr are named silicon / OS partners.
- **AccessKit producer since June 2023.** PR #2865 landed initial AccessKit support before the 1.1 release; Slint is one of the **named "verified adopters" in [`../accesskit/ecosystem.md`](../accesskit/ecosystem.md)**.
- **Embedded-grade footprint.** Compiles to bare-metal microcontrollers via `no_std`; runs on RGB565 framebuffers; ESP-IDF + STM32 integration; software renderer doesn't require GPU. Allocation discipline is real.
- **Cross-language by design.** Same `.slint` file generates Rust, C++, JS/Node, and Python bindings — the Slint compiler is the source-of-truth, host language is a thin wrapper.
- **Founders are veteran Qt-internals engineers.** Goffart maintained the Qt meta-object compiler (moc); Hausmann was lead developer + maintainer of QtQml. The DSL-design competence is real.

**Triple license is a hard gate for proprietary commercial use.** Slint is **`GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0`**. The royalty-free option (added in 1.1, June 2023) lifts the GPL gate for *desktop* proprietary apps under specific terms; the **commercial license is mandatory for proprietary embedded or mobile deployment**. Adopting Slint into a closed-source product is a procurement decision, not a `cargo add` decision.

**DSL-only authoring locks out programmatic / dynamic UI cases.** Slint does ship runtime APIs to construct UI programmatically (`slint-interpreter`), but the supported authoring path is `.slint` source files compiled ahead of time. Apps that need to construct UI from data at runtime (game inventory grids, IDE outline trees, BSN-style template merging) fight the grain.

**AccessKit version pin drifts behind upstream.** Slint 1.7 (July 2024) pinned AccessKit 0.16; issue [#8148](https://github.com/slint-ui/slint/issues/8148) (April 2025) flags that Slint cannot upgrade to `accesskit_winit` 0.26 because of an event-loop API change. As of folder-writing time, Slint had updated AccessKit in 1.12 and 1.13 but had not yet caught up to the recent (2026-05) co-release pin set seen in [`../accesskit/README.md`](../accesskit/README.md).

**Desktop posture is "becoming production-ready," not "is."** [Making Slint Desktop-Ready](https://slint.dev/blog/making-slint-desktop-ready) lists eight major desktop gaps as of late 2025: rich text, modal windows, system tray, drag-and-drop across windows, popup windows that are real windows (current popups render inside the host window), better keyboard-shortcut support, two-way bindings (landed in 1.15), and a system clipboard model. Buiy's web-platform-parity target is a much larger surface than what Slint commits to.

**Single-vendor stewardship.** SixtyFPS GmbH is the only entity contributing core changes at scale. No foundation, no charter, no formal RFC process — same shape as AccessKit's stewardship concentration but with a commercial-license revenue stream as the funding model (vs AccessKit's NLnet grants).

## Key facts

| Fact | Value | Source |
|---|---|---|
| Crate | `slint` 1.16.1 (2026-04-23) | [crates.io](https://crates.io/crates/slint) |
| Recent versions | 1.16.0 (2026-04-16), 1.15.1 (2026-02-12), 1.15.0 (2026-02-04), 1.14.1 (2025-10-23), 1.14.0 (2025-10-21) | crates.io |
| Repo | https://github.com/slint-ui/slint | — |
| Stars | 22.7k | GitHub |
| Total downloads | 1,102,671 lifetime; 236,814 recent (90d) | crates.io |
| License | **`GPL-3.0-only OR LicenseRef-Slint-Royalty-free-2.0 OR LicenseRef-Slint-Software-3.0`** | crates.io versions metadata |
| Legal entity | **SixtyFPS GmbH** (Brandenburg, Germany) | [About Us](https://slint.dev/about-us) |
| Founders | **Olivier Goffart, Simon Hausmann, Aurindam Jana** — ex-Trolltech / Qt Company | [Rust Foundation member spotlight](https://rustfoundation.org/media/member-spotlight-slint/), search verification |
| Tobias Hunger | **Software Engineer**, not a co-founder (brief had this wrong) | [About Us](https://slint.dev/about-us) |
| Founded | 2020, as **SixtyFPS**; rebranded to Slint on **2022-02-10** | [SixtyFPS becomes Slint](https://slint.dev/blog/sixtyfps-becomes-slint) |
| 1.0 release | 2023-04-05 | [Slint blog](https://slint.dev/) |
| Languages | Rust (stable), C++ (stable), JavaScript / Node.js (beta), Python (beta — added in 1.13.0) | README |
| DSL | `.slint` files — declarative, property-bound, callback-driven | [language reference](https://docs.slint.dev/latest/docs/slint/) |
| Renderers | Qt (native styling), Skia (default on Win/macOS since 1.14), FemtoVG (OpenGL ES 2.0), software (CPU; MCU-grade), FemtoVG-WGPU (since 1.12) | README + CHANGELOG |
| Backends | winit, Qt, Android (since 1.5), iOS (since 1.10), LinuxKMS, ESP-IDF, STM32 | CHANGELOG |
| AccessKit integration | Initial: PR [#2865](https://github.com/slint-ui/slint/pull/2865) merged 2023-06-15 with `accesskit` 0.11.0 + `accesskit_winit` 0.14.0; updated to 0.16 in 1.7 (2024-07); subsequent updates in 1.12 / 1.13; pin drift documented in issue [#8148](https://github.com/slint-ui/slint/issues/8148) | PR/issue history |
| Production users (verified) | **OTIV** (rail automation); KDAB consulting clients; LibrePCB 2.0 (Qt → Slint migration shipped; 2.0.0 in 2025, 2.0.1 in Feb 2026) | [Rust Foundation spotlight](https://rustfoundation.org/media/member-spotlight-slint/), [partners page](https://slint.dev/partners) |
| Tooling | VSCode extension (Slint.slint marketplace), Live Preview, Figma plugin (1.10+), Slint compiler `slint-compiler`, `slint-viewer`, `slint-lsp` | [VS Code marketplace](https://marketplace.visualstudio.com/items?itemName=Slint.slint) |
| Categories on crates.io | gui, rendering::engine, no-std | crates.io |

## Contents

Each file is independently skimmable. Sources are listed per file.

**Technical subsystems**

- [**architecture.md**](architecture.md) — The DSL-compiler-as-source-of-truth model: `.slint` files parsed at compile-time, compiled to native code (Rust macro `slint!` or build.rs codegen) + interpreted at runtime via `slint-interpreter`; the retained-mode runtime model; native + WASM + JS targets.
- [**dsl-language.md**](dsl-language.md) — The `.slint` DSL surface: syntax, types, property qualifiers (`in`/`out`/`in-out`), bindings, two-way binding (`<=>`), callbacks, animations, states, components; the tooling story (VSCode extension, Live Preview, Figma plugin).
- [**accessibility.md**](accessibility.md) — Slint's AccessKit integration: producer-side wiring through `accesskit_winit`; pinned versions and the upgrade-lag pattern; widget-level `accessible-role` / `accessible-label` properties; verified-adopter status in `../accesskit/ecosystem.md`.
- [**embedded-focus.md**](embedded-focus.md) — Why Slint targets embedded: small footprint, MCU support (ESP-IDF, STM32, RGB565 framebuffers), low-allocation rendering, software renderer without GPU, partial-update / dirty-region story.

**Project lens**

- [**history.md**](history.md) — 2020 SixtyFPS founding through 2022-02 rebrand through 1.0 (2023-04) through 1.16.1 (2026-04-23); founders' Qt / Trolltech lineage; major-release milestones; AccessKit integration timeline.
- [**governance-and-distribution.md**](governance-and-distribution.md) — Combined: SixtyFPS GmbH stewardship; triple-license model (GPL-3 + royalty-free + commercial); Cargo features; platform-support matrix; CONTRIBUTING; bus factor; Rust Foundation Silver Member.
- [**ecosystem-and-comparisons.md**](ecosystem-and-comparisons.md) — Combined: production users (OTIV, KDAB clients, LibrePCB); partners (Espressif, ST, Raspberry Pi, Toradex, QNX, Yocto, Zephyr); comparisons vs Qt/QML, egui, Iced, Dioxus, Druid/Xilem, and Buiy.
- [**open-problems.md**](open-problems.md) — Combined critiques + open problems: GPL+commercial dual gate, DSL learning curve, .slint-vs-Rust integration friction, renderer maturity, AccessKit version-pin drift, single-window popup limitation, desktop-readiness gaps, single-vendor stewardship, full WCAG conformance, mobile maturity, complex animation surface.

**Reference**

- [**lessons.md**](lessons.md) — **The consult-this-when-designing decision file.** Validates / Avoid / Borrow.
- [**glossary.md**](glossary.md) — Slint-specific terms.

## How to use this prior-art doc

When designing a Buiy feature that touches DSL authoring, retained-mode runtime shape, AccessKit producer wiring, or embedded-grade allocation discipline:

1. Start in [**lessons.md**](lessons.md). It enumerates which Buiy choices Slint's experience validates, which Slint pitfalls Buiy has mitigations for, and which Slint primitives are worth borrowing (especially the `.slint` property-binding shape and AccessKit integration patterns).
2. If lessons references a subsystem, read the matching file.
3. If you're evaluating "should Buiy ship a DSL above the Bevy ECS authoring layer," [`dsl-language.md`](dsl-language.md) + [`open-problems.md`](open-problems.md) are the load-bearing reads — Slint's DSL is the most mature data point in the Rust ecosystem, and the friction patterns are well-documented.
4. If you're looking at the commercial-licensing question (Buiy ships under MIT OR Apache-2.0 dual; Slint ships under GPL OR royalty-free OR commercial), [`governance-and-distribution.md`](governance-and-distribution.md) is the file.

**Framing disclosure.** This corpus is written from a **"Buiy is fully-open MIT/Apache + ECS-and-BSN-native + parallel-to-bevy_ui"** stance. Most "Implications for Buiy" lines treat Slint's commercial open-core model and DSL-first authoring as choices Buiy explicitly rejects, not options Buiy is evaluating. Future readers auditing whether **Buiy should adopt a DSL-first authoring layer or commercial licensing model** should weigh the corpus accordingly — it's a learn-from-Slint-into-Buiy artifact, not a neutral catalog. Two biases worth naming:

- **Triple-license framing is unfavorable to Slint by Buiy's defaults.** A game-engine ecosystem where Bevy itself is dual-MIT/Apache-2.0 makes Slint's GPL+commercial gate a structural mismatch. The corpus describes it as a hard gate; a reader weighing whether Buiy could *use* a commercial license itself should read the description as Buiy-incompatible, not Slint-incompetent.
- **DSL-first authoring is framed as a lock-out.** Slint's DSL is genuinely well-engineered and the founders' Qt-internals competence shows. Buiy's "ECS + BSN authoring, no DSL" choice reads Slint's DSL as friction, but a future Buiy spec author considering "should we ship a DSL?" should re-read [`dsl-language.md`](dsl-language.md) on Slint's terms first.

Doc lives, not snapshot — bump the date in this file's header on every meaningful update. The most date-sensitive facts are crate versions, AccessKit pin drift, and the desktop-readiness checklist; refresh those when next iterating.

## Sources

- Slint website: https://slint.dev/
- Slint About Us: https://slint.dev/about-us
- Slint partners: https://slint.dev/partners
- Slint repo: https://github.com/slint-ui/slint
- Slint on crates.io: https://crates.io/crates/slint
- Slint docs.rs: https://docs.rs/slint/latest/slint/
- Slint blog "SixtyFPS becomes Slint": https://slint.dev/blog/sixtyfps-becomes-slint
- Slint blog "Making Slint Desktop-Ready": https://slint.dev/blog/making-slint-desktop-ready
- Rust Foundation member spotlight: https://rustfoundation.org/media/member-spotlight-slint/
- AccessKit PR #2865 (initial Slint integration): https://github.com/slint-ui/slint/pull/2865
- Slint issue #8148 (AccessKit pin drift): https://github.com/slint-ui/slint/issues/8148
- Sibling files (per-section `## Sources`)
