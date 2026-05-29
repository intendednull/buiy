**Date:** 2026-05-22
**Status:** active
**Subject:** Slint — project history: 2020 SixtyFPS founding through 2022 rebrand through 1.0 (2023) through 1.16.1 (2026-04-23)

# History

Slint's history is the story of three Qt-internals engineers leaving Trolltech / The Qt Company, founding a Berlin-area GUI-toolkit company in 2020, shipping under one name (SixtyFPS) for two years, renaming after a community discussion (2022), reaching 1.0 in April 2023, and growing into one of the more mature Rust-ecosystem GUI products by 2026.

## Founders' Qt / Trolltech lineage

The three co-founders all worked together on Qt at Trolltech in Oslo (the company that built Qt; later acquired by Nokia 2008; spun out as Digia → The Qt Company):

- **Olivier Goffart** — long-time Qt core maintainer; primary maintainer of the Qt meta-object compiler (moc); co-founded **Woboq** (software consulting + the Woboq Code Browser) in 2011 between Trolltech and SixtyFPS.
- **Simon Hausmann** — lead developer + maintainer of the QtQml engine at The Qt Company; one of the canonical authors of QML's reactive bindings.
- **Aurindam Jana** — Qt engineering manager; technical and partner-relationship background. Listed as Slint co-founder per Rust Foundation member spotlight and Slint about-us.

Brief correction from the seed brief: **Tobias Hunger** is a Software Engineer at SixtyFPS GmbH, *not* a co-founder. The brief's "Hunger + Goffart" framing was wrong; verified via the company about-us page which lists Goffart / Hausmann / Jana as Co-Founders and Hunger as Software Engineer. Tobias Hunger (GitHub `@hunger`) is one of the most active engineering contributors. The initial AccessKit integration PR ([#2865](https://github.com/slint-ui/slint/pull/2865)) was authored by co-founder **Simon Hausmann** (GitHub `@tronical`) with collaboration from Matt Campbell.

## Major milestones

| Date | Version | Milestone |
|---|---|---|
| 2020 | — | **SixtyFPS** project + GmbH founded by Olivier Goffart, Simon Hausmann, Aurindam Jana. Initially Berlin; later relocated company HQ to Brandenburg state, Germany. Mission: a modern declarative GUI toolkit written in Rust, drawing on Qt-internals experience. |
| 2021-01 | 0.0.x | First crates.io releases under name `sixtyfps`. The DSL is in place; Rust + C++ codegen working; demos shipping. |
| 2022-02-01 | 0.2.0 | `slint` crate first published on crates.io (the project was being renamed). |
| 2022-02-10 | — | **Rebrand from SixtyFPS to Slint** announced on blog. The community-voted name "Slint" stands for "Straightforward, Lightweight, Native Toolkit." The legal entity remains SixtyFPS GmbH. |
| 2022-07-06 | 0.2.5 | Initial accessibility annotations on standard widgets (pre-AccessKit). |
| 2022-Q3–Q4 | 0.3.x | Backend split (winit / Qt); Skia renderer first appearance; Live Preview shipping. |
| 2023-04-05 | **1.0.0** | First stable release. Three years of pre-1.0 iteration. Same-day blog post + community-wide announcement. |
| 2023-06-15 | (between 1.0 and 1.1) | **PR [#2865](https://github.com/slint-ui/slint/pull/2865) merged**: initial AccessKit producer wiring through `accesskit_winit`. Author: Simon Hausmann (@tronical), with collaboration from Matt Campbell (Pneuma Solutions / AccessKit). Pinned `accesskit` 0.11.0. |
| 2023-06 | **1.1.0** | Added **royalty-free license** as the third triple-license option (alongside GPL-3 and commercial). Major shift in business model — desktop proprietary apps can now use Slint without buying a commercial license, under specific terms. Embedded / mobile proprietary still require commercial. |
| 2024-03-14 | 1.5.0 | Android backend added (`backend-android-activity-05`); RGB565 framebuffer support solidified. |
| 2024-07-18 | 1.7.0 | Winit 0.30 upgrade + **AccessKit 0.16** upgrade; multi-window support; live-preview redesign; date / time picker popups. |
| 2024-09 | 1.8.0 | STM32 first-class support landed; SwipeGestureHandler. |
| 2024-12-18 | 1.9.0 | Translation bundling; **Python `asyncio` integration**; Figma inspector preview. |
| 2025-02-28 | 1.10.0 | **Figma plugin (full release)**; **iOS backend (initial)**; SDF glyph rendering; live-preview console. |
| 2025-04-23 | 1.11.0 | `@conic-gradient`; `let` local variables in expressions; MenuBar standard widget. |
| 2025-06-16 | 1.12.0 | FemtoVG-WGPU renderer; iOS Simulator wheels for Python. |
| 2025-09-03 | 1.13.0 | **Python bindings (stable)**; callback-syntax simplifications; conical-gradient rendering. |
| 2025-10-21 | 1.14.0 | **Fontique / Parley text layout** (modern shaping); rotation + scaling transforms; **Skia becomes default renderer on Windows / macOS**. |
| 2026-02-04 | 1.15.0 | Two-way struct bindings; safe-area properties (for mobile notch / status-bar avoidance); GridLayout runtime properties. |
| 2026-04-16 | 1.16.0 | **Fluent default style**; styled text; ScaleRotateGestureHandler; SDF font improvements; LinuxKMS GPU support; **napi-rs 3.0 port for Node.js**. |
| 2026-04-23 | **1.16.1** | Latest stable at folder-writing. ListView dirty-region fixes; ComboBox text eliding. |

## The naming history

The community-vote rename (2022-02-10) is unusual enough to be worth its own note. The seed name **SixtyFPS** was chosen for the project's "60 FPS smooth UI" pitch. As newer screens shipped at 90 / 120 / 144 / 165 Hz refresh rates, social-media commentary started reading the name as dated. The team opened [discussion #636](https://github.com/slint-ui/slint/discussions/636) on GitHub asking for ideas; @karoofish suggested "Slant"; the team picked "Slint" and back-formed the acronym "Straightforward, Lightweight, Native Toolkit." The GitHub org renamed from `sixtyfpsui` to `slint-ui`; the npm and crates.io packages were re-published under `slint`.

The legal entity **SixtyFPS GmbH** kept its name through the rebrand and is still the copyright holder on every Slint commit and the named licensor on the commercial license terms.

## Trademark / brand notes

The Slint trademark is held by SixtyFPS GmbH. The `slint` name and logo are protected. The triple-license terms ([LICENSE-Royalty-free.md](https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Royalty-free-2.0.md), [LICENSE-Software-3.0.md](https://github.com/slint-ui/slint/blob/master/LICENSES/LicenseRef-Slint-Software-3.0.md)) treat the Slint brand explicitly — the royalty-free option includes attribution requirements; the commercial license is bilateral and per-project.

## Implications for Buiy

- **Three years pre-1.0 is the rough baseline for shipping a GUI toolkit at this scope.** SixtyFPS / Slint was founded 2020, hit 1.0 in 2023 — three years of pre-1.0 iteration before the API was committable. Buiy's foundation spec (May 2026) is the pre-design phase; if Buiy follows a similar arc, 1.0 lands roughly 2029. The plans (`docs/plans/`) own the timeline; this is just the data point.
- **Founders' Qt-internals competence is load-bearing for DSL quality.** Goffart / Hausmann maintained QtQml and the moc; the DSL family-resemblance to QML is by design and competent. A Buiy-designed DSL would need equivalent depth of property-system / reactive-binding expertise — not impossible, but the kind of multi-year specialty that doesn't transfer from "we're good at Bevy."
- **Triple-license rebrand mid-1.x changed the business model in 1.1.** The royalty-free license added in 1.1 (June 2023, two months after 1.0) substantially changed Slint's commercial posture — desktop proprietary apps became free-with-attribution; embedded proprietary stayed commercial. Buiy is committed to MIT OR Apache-2.0 from the start, so this evolution doesn't apply, but the data point is "even an open-core toolkit might need to revisit its license terms mid-1.x to match adoption realities."
- **Community-voted rename is a real risk.** The name "SixtyFPS" aged badly within two years. Buiy is a generic brand; the name choice is a one-off but the lesson — community sentiment shifts on naming faster than maintainers expect — is a reminder.

## Sources

- Slint repo: https://github.com/slint-ui/slint
- Slint About Us: https://slint.dev/about-us
- Slint blog "SixtyFPS becomes Slint": https://slint.dev/blog/sixtyfps-becomes-slint
- Slint discussion #636 (rename): https://github.com/slint-ui/slint/discussions/636
- Slint issue #635 (rename): https://github.com/slint-ui/slint/issues/635
- Rust Foundation member spotlight: https://rustfoundation.org/media/member-spotlight-slint/
- Slint 1.1 royalty-free license blog: https://slint.dev/blog/slint-1.1-released
- Slint 1.7 release blog (AccessKit 0.16): https://slint.dev/blog/slint-1.7-released
- Slint 1.10 release blog (Figma): https://slint.dev/blog/slint-1.10-released
- Olivier Goffart's GitHub: https://github.com/ogoffart
- FOSDEM 2023 Goffart speaker page: https://archive.fosdem.org/2023/schedule/speaker/olivier_goffart/
- AccessKit PR #2865: https://github.com/slint-ui/slint/pull/2865
- Sibling files: [`architecture.md`](architecture.md), [`governance-and-distribution.md`](governance-and-distribution.md), [`accessibility.md`](accessibility.md)
