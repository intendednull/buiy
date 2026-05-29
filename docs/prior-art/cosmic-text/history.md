**Date:** 2026-05-22
**Status:** active
**Subject:** cosmic-text — chronological history, predecessors, and adoption timeline

# History

cosmic-text appeared because the Rust text-rendering landscape was bifurcated between **glyph-only crates** (no shaping, no fallback) and **wrappers around C libraries** (HarfBuzz, ICU, Pango). A pure-Rust shape-plus-layout-plus-rasterize stack with multi-line support, BiDi, font fallback, and an embeddable editor did not exist before 2022. System76 needed one to build the COSMIC desktop in Rust and committed to building it.

Cross-links: [governance.md](governance.md) (stewardship), [ecosystem.md](ecosystem.md) (current downstream), [architecture.md](architecture.md), [shaping.md](shaping.md).

## Pre-cosmic-text Rust text landscape (2018–2022)

The set of crates a Rust UI library could pick from before cosmic-text:

- **`glyph_brush` / `ab_glyph`** (`alexheretic`) — glyph-cache + bitmap rasterizer. **No shaping. No BiDi. No fallback.** You could ship Latin-script English UI with it (Bevy did, until 0.14); you could not ship Arabic, Hindi, Thai, or Korean correctly. The "1.0 of game UI text" of its era, but capped at the simple-script ceiling.
- **`font-kit`** (`pcwalton` / `servo`) — an Apple-style font-enumeration and metadata API. **No layout, no shaping** — purely the "find a font and read its metrics" layer. Used as a building block by other crates, never as a complete solution.
- **`harfbuzz_rs`** (`servo`) — safe Rust bindings to HarfBuzz the C library. Shaping but **no layout, no BiDi orchestration, no editor** — and pulls in a C build dependency, which is the main reason pure-Rust projects avoided it.
- **`rustybuzz`** (`RazrFalcon`) — a port of HarfBuzz to Rust. Shaping in pure Rust. **No layout above it.** This became cosmic-text's shaping engine at 0.1.
- **`fontdb`** (`RazrFalcon`) — pure-Rust font enumeration and loading (covering what `font-kit` did, but Rust-native). cosmic-text uses this from day one and still uses it.
- **`swash`** (`dfrg`) — pure-Rust font rasterizer and font-format reader. Used by cosmic-text for the glyph-bitmap step.
- **`unicode-bidi`** (`servo`) — UAX #9 BiDi algorithm implementation. Pure logic, no rendering.

The shape of the gap was: **shaping + layout + BiDi + multi-line + cursor + fallback, in one crate, pure Rust**. cosmic-text filled exactly that gap.

## COSMIC desktop genesis (2021–2022)

System76 announced COSMIC (an in-house desktop environment to replace its GNOME-based Pop!_OS shell) in 2021 and committed to Rust + iced as the implementation stack. The decision to build the desktop in Rust forced the text-stack decision: there was no pure-Rust text engine adequate for a desktop, so System76 funded one.

The first `cosmic-text` commit landed in September 2022. The crate's first crates.io release was 0.1.0 later that year. Jeremy Soller (System76 principal engineer, primary `redox-os` maintainer) is the original author and remains the package's sole listed `authors = [...]` entry in Cargo.toml through 0.19.0.

## Version timeline (verified against crates.io publish dates and GitHub release notes)

| Version | Date | Headline change |
|---|---|---|
| 0.1.0 | late 2022 | Initial release: rustybuzz shaping, swash rasterizer, single-line layout, fontdb fallback. |
| 0.4.x | early 2023 | Multi-line `Buffer`, `Attrs` span model, `LayoutRun` API. |
| 0.6 / 0.7 | mid 2023 | `Editor` introduced (the decoupling commit from `Buffer` landed 2022-10-31; first stable surface in this band). BiDi-aware cursor traversal. |
| 0.10 | ~late 2023 | Adopted as Iced's text engine (Iced 0.10.0, 2023-07-28 — the cosmic-text version was earlier; Iced bumped to cosmic-text 0.10 in Iced 0.13.0 on 2024-09-18). |
| 0.11 / 0.12 | early-mid 2024 | `cosmic_undo_2` integration via the `vi` feature. `ViEditor` becomes the canonical undo-stack-bearing editor. Color-emoji path stabilized via swash's CPALv0 support. |
| 0.13 | mid 2024 | Bevy 0.15 (released 2024-11-29) ships with cosmic-text via PR #10193 (merged 2024-07-04 during the 0.14 development cycle). |
| 0.14.0 | 2025-03-31 | (Per crates.io publish date.) Stable cycle. MSRV pinned to 1.75. |
| 0.14.2 | 2025-04-14 | MSRV 1.75 cement. |
| 0.15.0 | 2025-10-30 | **Variable font support** lands. Pixel font flag, pixel-based scrolling for `Editor`. ASCII fast path optimization. The `rustybuzz → HarfRust` migration commit (PR #417, `2610c86`, 2025-09-09) lands in this window. |
| 0.16.0 | 2025-12-29 | `Renderer` trait for flexible rendering. `Hinting` enum makes hinting configurable. |
| 0.17.0 | 2026-01-29 | Fixed variable-font weight and font-fallback logic. Prevented line-break opportunities from splitting ligatures. |
| 0.17.1 | 2026-01-30 | rust-version set to 1.89 (the MSRV documented in the current crate metadata). |
| 0.17.2 | 2026-02-18 | Fixed `Motion::Home` and `Motion::End` on unwrapped lines. |
| 0.18.0 | 2026-02-19 | Ellipsizing at start, middle, end of a line. |
| 0.18.1 | 2026-02-20 | Aggressive-ellipsizing fix. |
| 0.18.2 | 2026-02-20 | Further ellipsize fixes. |
| 0.19.0 | 2026-04-22 | **Text-decoration support** (underline / strikethrough), `layout_runs` for `BufferLine`, `cursor_position` and `is_rtl` helpers. Current latest. |

Lateral-release notes for feature-landing-version questions:

- **BiDi support** — landed before 0.10 (used to be a "TODO" in 0.1's README; production-ready in the 0.6–0.7 band when `Editor` arrived).
- **Editor module** — first commit `Decouple editing from buffer` dated 2022-10-31, refined through 2023.
- **Color emoji** — present from 0.1 via swash's CPAL/COLR table support. **COLRv1 is NOT supported** as of 0.19.0 — see [critiques.md § COLRv1](critiques.md) and issue #446. Fedora 43 shipping Noto Color Emoji as COLRv1 has broken rendering for cosmic-text users on that distro.
- **Variable fonts** — formal support shipped in 0.15.0 (October 2025). Earlier versions could load variable-font files but did not respect variation axes.
- **Text decoration** — shipped in 0.19.0 (April 2026). Before that, embedders drew underlines from glyph metrics directly.
- **Ellipsize** — shipped in 0.18.0 (February 2026).
- **HarfRust** — shaping engine migration from rustybuzz happened in PR #417 (commit `2610c86`), merged 2025-09-09, first released in 0.15.0. Before 0.15, the shaper was rustybuzz.

## Adoption timeline (downstream)

- **Iced 0.10.0** (2023-07-28) — adopts cosmic-text as the text engine. Changelog: *"Text shaping, font fallback, and `iced_wgpu` overhaul. [#1697]"*. This was the largest single non-COSMIC consumer at the time.
- **Bevy 0.15** (2024-11-29) — PR #10193 merged 2024-07-04 during the 0.14 development cycle. Migrated from `ab_glyph` to cosmic-text. First release with system-font support. (Common mis-statement: that Bevy migrated *from* `glyph_brush`. `glyph_brush` had been a transitive dep through earlier renderer layers; the canonical migration was `ab_glyph` → cosmic-text.)
- **`bevy_cosmic_edit`** (`Dimchikkk`) — community plugin layering an editing UI on top of `bevy_cosmic_edit`. Most recent compatibility is Bevy 0.15. **Archived 2025-03-21**, no longer maintained.
- **Zed / GPUI** — appears in cosmic-text's reverse-dependency list on crates.io (gpui crate showed historical dependency). The current `zed-industries/zed` main-branch Cargo.toml shows no cosmic-text reference, suggesting Zed has either migrated away or vendored it. Status unclear from public sources.
- **Floem** (`lapce/floem`) — does NOT use cosmic-text. Uses Parley. Sometimes mis-attributed as a cosmic-text consumer because Lapce-the-editor (which Floem-the-toolkit is part of the same org as) uses cosmic-text indirectly through glyphon.
- **Freya** (`marc2332/freya`) — does NOT use cosmic-text. Uses Skia (via `freya-skia-safe`). Sometimes mis-attributed.

## What's still missing as of 0.19.0

Not a critique list (see [critiques.md](critiques.md) for that), just the chronological "still pending" state:

- IME composition (issue #10, open since 2022-10-24)
- Vertical writing modes (no tracking issue)
- Hyphenation (no tracking issue)
- COLRv1 color fonts (issue #446)
- Fontconfig alias resolution (issue #499)
- `FontSystem::new` startup time (issue #505 — still slow as of 0.14.2 per profiling)
- Variable-font axis exposure beyond weight (issue #406)

## Sources

- crates.io cosmic-text release index — https://crates.io/crates/cosmic-text/versions
- GitHub releases — https://github.com/pop-os/cosmic-text/releases
- PR #417 (rustybuzz → HarfRust) — https://github.com/pop-os/cosmic-text/pull/417
- PR #10193 (Bevy adoption) — https://github.com/bevyengine/bevy/pull/10193
- Iced changelog — https://github.com/iced-rs/iced/blob/master/CHANGELOG.md
- Editor decoupling commit (2022-10-31) — git blame `src/edit/editor.rs`
- bevy_cosmic_edit archived banner — https://github.com/StaffEngineer/bevy_cosmic_edit
