**Date:** 2026-05-22
**Status:** active
**Subject:** egui — chronological history from Emigui (2018) through 0.34 (2026-03)

# History

egui's story is one person — Emil Ernerfeldt — sustained first by a pandemic-summer push, then by Embark Studios employment, then by Rerun.io's full-time investment. The library predates the Rust GUI boom; it's been shipping continuously since 2018.

## Origins: Emigui (2018–2020)

The dates below are verified against the `## Earlier:` section at the bottom of the `egui` CHANGELOG.md, which Emil maintains as an authoritative project log.

- **2018-11-04** — Emil starts tinkering on a train. (Per CHANGELOG: "started tinkering on a train.")
- **2018-12-23** — Initial commit to GitHub (commit `856bbf4`). The project is called **Emigui** — "Emil's Immediate-mode GUI." Pure hobby project, no users.
- **2019-03-12** — Emil gives a talk at the Stockholm Rust Meetup about what would later become egui: "Immediate Mode GUI in Rust." Video on YouTube. This is the first public communication of the design.
- **2018-2019** — Slow off-hours iteration. Emil is at this point employed at Embark Studios (joined 2019) and Emigui is a side project. The shape — immediate-mode, hash-based widget IDs, single-tessellator output — is set in this period.

## The pandemic push (2020)

- **2020-04-01** — "Serious work starts (pandemic project)" per CHANGELOG. The COVID-19 lockdown turns Emigui from off-hours doodle into daily-driver development.
- **2020-05-30** — First release on crates.io as `egui = "0.1.0"`. The crate is *already* named `egui` even though the project is still labeled Emigui in the README.
- **2020-08-10** — Project officially **renamed Emigui → egui**. Pronounced "e-gooey." Emil's rationale (per public discussion): shorter, less ego-tied name as the library starts attracting external contributors.
- **2020-09-08** — `0.1.4` is the release that starts the CHANGELOG.md proper. Widget set at this point: label, button, hyperlink, checkbox, radio, slider, draggable value, text editing. Layouts: horizontal, vertical, columns.

## Maturation under Embark (2020–2022)

Embark Studios — Emil's employer — adopts egui internally for tooling. Embark commits no formal stewardship but does provide engineering time. egui's early "engine-agnostic, render-anywhere" framing aligns with Embark's polyglot in-house tools.

Major releases in this window:

- **0.10 (2021-02-28)** — Plot widget lands.
- **0.11 (2021-04-05)** — First screen-reader path (web-only, via WebSpeech), new layout logic, optimization pass.
- **0.13 (2021-06-24)** — Panels API redesigned. New visual style.
- **0.14 (2021-08-24)** — Ui panels and bug fixes.
- **0.15 (2021-10-24)** — Syntax highlighting, horizontal scroll.
- **0.16 (2021-12-29)** — Context menus, rich text (`RichText`).
- **0.17 (2022-02-22)** — Improved font selection + image handling. `egui_extras` and `egui-winit` / `egui-wgpu` introduced as sub-crates.
- **0.19 (2022-08-20)** — Hardening release between major-feature beats.

## AccessKit lands (December 2022)

**0.20.0 (2022-12-08)** — release subtitle "AccessKit, prettier text, overlapping widgets." Optional integration with [AccessKit](https://accesskit.dev/) for platform accessibility APIs (PR [#2294](https://github.com/emilk/egui/pull/2294)). This is the moment egui crosses from "no a11y story" to "AccessKit-first" — three years before the comparable retained-mode Rust UIs (`iced`, `Slint`) had any equivalent.

AccessKit was Matt Campbell's project (Pneuma Solutions, GNOME a11y veteran) and egui was one of the first non-trivial adopters. The integration shape — egui builds AccessKit nodes during its end-of-frame pass and hands them to an AccessKit adapter — is the same shape Buiy intends to use via `bevy_a11y`.

The AccessKit feature flag stayed opt-in from 0.20 through 0.33, then became **always-on** in 0.34.0 (PR [#7701](https://github.com/emilk/egui/pull/7701), 2026-03-26) — removing the feature flag entirely. Verified in the 0.34.0 changelog: "Remove `accesskit` feature and always depend on `accesskit`."

## Rerun spin-off (2022–2023)

Mid-to-late 2022, Emil leaves Embark and co-founds **Rerun.io** with Niko Reece and Moritz Rieger. Rerun is a streaming-data visualization product for ML / robotics teams; its desktop and web viewers are built on egui. From this point forward, egui is structurally a **Rerun project** — Rerun pays Emil's salary, and the engineering investment Rerun makes into egui shows up in releases.

Visible signs of the Rerun era:

- **0.21 (2023-02-08)** — "Deadlock fix and style customizability." The deadlock had blocked Rerun's viewer.
- **0.22 (2023-05-23)** — "A plethora of small improvements." Quality polish driven by Rerun's daily use.
- **0.23 (2023-09-27)** — New image API. Rerun ingests image streams; the API redesign was Rerun-driven.
- **0.24 (2023-11-23)** — **Multi-viewport** lands. Rerun needed multi-window for the docked-floating panel UX. Multi-viewport is a substantial architectural addition (per [architecture.md](architecture.md)) and it lands because Rerun needed it.

## The "looks like egui" moment (2023–2024)

Around 0.23–0.25, the Rust dev-tools community converges visibly on egui. Bevy's editor experiments, custom in-game debug overlays, ML scratch GUIs, and crypto explorers all start looking the same: rounded panels, slate-gray surface, blue-accent active elements, the same default font (Ubuntu-Light + Hack). The community half-jokes about the "looks like egui" homogeneity on Twitter/r/rust/HN.

This is not a release event — it's a cultural moment. The honest critique is captured in [critiques.md § homogeneity](critiques.md). egui won the dev-tool niche so completely that "an egui-shaped app" became the default Rust-internal-tool aesthetic.

## Recent maturation (2024–2026)

- **0.25 (2024-01-08)** — Better keyboard input.
- **0.26 (2024-02-05)** — Text selection in labels.
- **0.27 (2024-03-26)** — Nicer menus + new hit-test logic.
- **0.28 (2024-07-03)** — Sizing pass, `UiStack`, GIF support.
- **0.29 (2024-09-26)** — **Multipass, `UiBuilder`, and visual improvements.** Multipass is a major addition: egui can now do multiple layout passes per frame to settle auto-sizing widgets (tooltips, popovers). Lifts the headline limitation of immediate mode "can't size things based on their own content."
- **0.30 (2024-12-16)** — Modals + better layer support.
- **0.31 (2025-02-04)** — `Scene` container + improved rendering quality.
- **0.32 (2025-07-10)** — **Atoms + popups + better SVG support.** Atoms are a small composable widget-content primitive; popups are a unified popup-management API. Tracks parley for font rendering (still ab_glyph at this point).
- **0.33 (2025-10-09)** — `egui::Plugin` trait (replaces `Context::on_begin_pass`/`on_end_pass`); kitdiff snapshot viewer; better kerning. Font-rendering refactor lands as prep for the parley/skrifa migration.
- **0.34.0 (2026-03-26)** — **Skrifa + vello_cpu** replaces `ab_glyph` for font rendering. Font hinting + variable-font axes. The font subsystem now sits on the same Linebender stack as Vello/Parley. MSRV bumped to 1.92. `Ui` deref-to-`Context`; `Context` no longer the main entrypoint. Unified `Panel` replaces `SidePanel`/`TopBottomPanel`. AccessKit feature flag removed (always-on).
- **0.34.2 (2026-05-04)** — Latest stable as of writing. Text-selection bug fixes + a regression test for an O(n²) word-boundary scan.

## Emil's design talks

- 2019-03-12 — Stockholm Rust Meetup, the predecessor to RustConf talks. https://www.youtube.com/watch?v=-pmwLHw5Gbs
- Multiple later talks at Rerun events + Rust-community streams; less canonical than the early one.

## Adoption milestones

- **2020-2021** — Embark internal dogfooding (asset browsers, debug HUDs).
- **2020-08-14** — `bevy_egui` first release (vladbat00 fork; not under emilk). Becomes the default Bevy debug-UI within ~12 months. See [`prior-art/bevy-egui/`](../bevy-egui/).
- **2023** — Rerun viewer launches publicly on egui — the canonical at-scale egui app, multi-megabyte WASM bundle, sustained-60fps streaming data display.
- **2024-2025** — Embark continues internal use; multiple game studios adopt for tools (no AAA in-game shipped UI, per [critiques.md](critiques.md)).

egui has shipped continuously without a hiatus in seven years. The bus factor remains "Emil + Rerun engineering" — see [governance.md](governance.md).

## Sources

- egui CHANGELOG (covers 0.1.4 onward + Earlier section) — https://raw.githubusercontent.com/emilk/egui/main/CHANGELOG.md
- egui README — https://raw.githubusercontent.com/emilk/egui/main/README.md
- crates.io version list — https://crates.io/api/v1/crates/egui/versions
- Rerun.io — https://rerun.io
- AccessKit project — https://accesskit.dev
- Stockholm Rust Meetup talk (2019) — https://www.youtube.com/watch?v=-pmwLHw5Gbs
- PR #2294 (AccessKit integration) — https://github.com/emilk/egui/pull/2294
- PR #7701 (AccessKit always-on) — https://github.com/emilk/egui/pull/7701
- PR #7694 (skrifa migration) — https://github.com/emilk/egui/pull/7694
