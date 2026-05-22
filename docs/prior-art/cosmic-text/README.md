**Date:** 2026-05-22
**Status:** active
**Subject:** cosmic-text — pure-Rust text shaping, BiDi, layout, editing, and color-emoji rasterization

## What it is

[`cosmic-text`](https://github.com/pop-os/cosmic-text) is the pure-Rust text engine built and maintained by System76 for the COSMIC desktop and Pop!_OS. It composes a HarfBuzz-port shaper (`harfrust`), a font-format rasterizer (`swash`), Servo's UAX #9 BiDi (`unicode-bidi`), Google Fonts' `read-fonts` wrapper (`skrifa`), and `fontdb` font discovery into a single crate that does shape + layout + cursor + selection + glyph caching for multi-line, multi-script, BiDi-correct text. It is the substrate Iced has used since 0.10.0 (2023-07-28) and the substrate Bevy adopted in 0.14 (2024-07-09, PR #10193), replacing the script-coverage-capped `ab_glyph` engine.

Buiy's foundation spec ([`text.md`](../../specs/2026-05-07-buiy-foundation/text.md)) treats cosmic-text as a **load-bearing dependency**: full BiDi, RTL, complex script shaping, IME composition, and color emoji parity with the web platform are committed to cosmic-text, with Buiy owning the glyph atlas, the IME-event-to-Action translation, the undo stack, and the render passes above it. **Buiy diverges from bevy_ui on text substrate post-0.19:** Bevy 0.19-dev (issue [#21765](https://github.com/bevyengine/bevy/issues/21765), 2025-11-06) migrated `bevy_text` from cosmic-text to **Parley + swash**; Buiy stays on cosmic-text. The bet is that cosmic-text's IME-aware editing primitives, System76 dogfooding via COSMIC, and `harfrust` shaping are the right pick for Buiy's web-parity surface — even though Parley's tighter integration with the newer Linebender stack (vello, kurbo, peniko) is a real alternative.

## Honest assessment

- **Production-proven.** Ships in the COSMIC desktop (every text-bearing surface), the COSMIC text editor, and Iced 0.10+. Real-world stress test: the Universal Declaration of Human Rights corpus (~500 languages, 8 MB, 106,746 lines) ships in the repo as the canonical correctness fixture.
- **Fast-moving substrate.** The shaping engine swap from `rustybuzz` to `harfrust` shipped inside a stable 0.15.0 release (2025-10-30, PR #417), not behind a feature flag. Pre-1.0 + no formal RFC process means breaking changes land in PR threads, not roadmap documents. Buiy must pin its cosmic-text version explicitly and follow PR queues, not blog posts.
- **The IME boundary is genuinely the embedder's problem.** Issue [#10](https://github.com/pop-os/cosmic-text/issues/10) ("IME support") has been open since 2022-10-24 with the entire body being "Will need a winit example to experiment with this." Three and a half years later this is unchanged. The structural answer — winit owns the IME state machine, embedders translate to cosmic-text `Action::Insert` and maintain a parallel preedit span — is what Buiy commits to.
- **Key gaps are structural, not on the roadmap.** Vertical writing modes (`writing-mode: vertical-rl/lr`), hyphenation, and COLRv1 color emoji (Fedora 43 ships Noto Color Emoji as COLRv1, issue [#446](https://github.com/pop-os/cosmic-text/issues/446)) have either no tracking issue or an upstream-blocked open issue. Parley has the same vertical-writing and hyphenation gaps but does support COLRv1.
- **Single-company stewardship.** Jeremy Soller (System76 founder/CEO, jackpot51) is the keystone. Frequent contributors are `valadaptive` (landed PR #417) and `benstigsen` (recent harfrust update). No `MAINTAINERS.md`, `CODEOWNERS`, or `GOVERNANCE.md` in the repo. Bus factor is real and called out in [governance.md](governance.md).
- **`bevy_cosmic_edit` was archived 2025-03-21.** The community third-party editor crate that tried to bridge cosmic-text into Bevy is no longer maintained. Buiy does not depend on it; Buiy implements its own text-edit surface.

## Key facts

| Fact | Value |
|---|---|
| Crate | `cosmic-text` |
| Latest stable | **0.19.0** (2026-04-22) |
| Repo | https://github.com/pop-os/cosmic-text |
| License | MIT OR Apache-2.0 |
| rust-version | 1.89 |
| Total downloads | 4,731,411 |
| Recent downloads | 1,299,778 |
| Steward | System76 (POP_OS / COSMIC desktop) |
| Primary maintainer | Jeremy Soller (`jackpot51`) |
| Shaper | **`harfrust 0.5.0`** (HarfBuzz v13.0.0 port) — replaced `rustybuzz` in **0.15.0** (PR #417, 2025-09-09) |
| Rasterizer | `swash 0.2.6` (outlines, COLRv0/CPAL, sbix, CBDT/CBLC; **NOT** COLRv1) |
| Font data | `skrifa 0.40.0` (Google Fonts' `read-fonts` wrapper) |
| Font discovery | `fontdb 0.23` |
| BiDi | `unicode-bidi 0.3.18` (`hardcoded-data`) |
| Line break | `unicode-linebreak 0.1.5` (UAX #14, no dictionary) |
| Editor decoupled from Buffer | 2022-10-31 |
| Iced adoption | 0.10.0 (2023-07-28) |
| Bevy adoption | 0.14 (2024-07-09, PR #10193); migrated away in 0.19-dev (issue #21765, 2025-11-06) |
| `bevy_cosmic_edit` | **archived 2025-03-21** |
| Open issues | ~98 (May 2026) |

## Contents

- [`architecture.md`](architecture.md) — Module layout, data model (`FontSystem` → `Buffer` → `BufferLine` → `ShapeLine` → `LayoutLine` → `LayoutGlyph`), shaping pipeline, glyph cache, font discovery, optional features.
- [`shaping.md`](shaping.md) — How cosmic-text drives `harfrust`, HarfBuzz feature parity, complex script support (Arabic, Indic, Thai, CJK, emoji), per-cluster fallback, subpixel positioning.
- [`bidi.md`](bidi.md) — UAX #9 wiring via `unicode-bidi`, paragraph-level handling, base-direction selection, mirroring, isolate/override characters, BiDi-aware caret movement.
- [`capabilities.md`](capabilities.md) — Can-do / can't-do matrix mapped to Buiy `text.md` tiers; gap analysis with `inherit` / `build above` / `fork+patch` / `out` stance per row.
- [`editing.md`](editing.md) — `Editor` / `ViEditor` types, `Action` enum, cursor and selection model, multi-line, soft-wrap, hard-wrap, indent, undo (split via `vi` feature), IME composition boundary, find/replace, spellcheck.
- [`integration.md`](integration.md) — Canonical embedder shape (one `FontSystem`, one `SwashCache`, per-node `Buffer`, per-edit `Editor`, embedder atlas); Iced integration; Bevy 0.14+ integration; COSMIC dogfood; Freya/Floem **not** downstream; Buiy plan; IME lifecycle.
- [`history.md`](history.md) — Pre-cosmic-text Rust text landscape, COSMIC genesis, version timeline, downstream adoption.
- [`governance.md`](governance.md) — System76 stewardship, commercial model, funding, licensing, release cadence, issue triage, contribution model.
- [`ecosystem.md`](ecosystem.md) — Substrate (harfrust, swash, skrifa, fontdb, unicode-*); Parley contrast table; downstream apps and crates; community misattributions.
- [`critiques.md`](critiques.md) — Performance issues (#505 `FontSystem::new` slow, full-buffer reshape, atlas churn), API friction (Buffer/Editor split, FontSystem non-Sync, Attrs lifetimes), missing features (vertical writing, hyphenation, COLRv1, variable-font axes), other open issues.
- [`lessons.md`](lessons.md) — **The consult-this-when-designing decision file.** Validates / Avoid / Borrow.
- [`glossary.md`](glossary.md) — System-specific terms.

## How to use this prior-art doc

Read [`lessons.md`](lessons.md) first when you're designing anything text-shaped in Buiy — it's the synthesis. Use [`capabilities.md`](capabilities.md) when checking whether a specific `text.md` row inherits, builds above, forks, or punts. Use [`editing.md`](editing.md) + [`integration.md`](integration.md) when wiring the IME boundary in `buiy-text-editing-design`. Use [`architecture.md`](architecture.md) + [`shaping.md`](shaping.md) when designing the glyph atlas and font-fallback strategy in `buiy-text-rendering-design`. Use [`critiques.md`](critiques.md) before declaring a feature done — most of the rows there are Buiy will-hit issues, not Buiy might-hit issues.

**Framing disclosure.** These docs are written from a **"Buiy commits to cosmic-text as the text engine even as bevy_ui diverges to Parley"** stance — most "Implications for Buiy" sub-sections frame cosmic-text's choices through that lens. Future readers auditing whether *cosmic-text* (vs Parley) is the right pick for Buiy should weigh the corpus accordingly: it's a learn-from-cosmic-text-into-Buiy artifact, not a neutral catalog comparing both engines exhaustively. Buiy's bet is that cosmic-text's IME-aware editing primitives, System76 dogfooding through COSMIC, and `harfrust`-as-HarfBuzz-port shaping make it the right substrate; Parley's tighter integration with the newer Linebender stack (vello + kurbo + peniko, with COLRv1 emoji via vello rather than swash) is a real alternative that this folder does not exhaustively explore. The COLRv1 gap is the single sharpest concrete cost of the cosmic-text bet vs Parley; vertical writing and hyphenation are gaps in both engines.

## Internal contradictions to fix in a polish pass

- **Harfrust migration version.** [`architecture.md`](architecture.md) (§ "Brief correction up front") and [`shaping.md`](shaping.md) (§ "Substrate correction") both state the rustybuzz → harfrust migration "happened in the 0.17.x / 0.18.x line." This is **wrong**. The canonical answer, verified per the 0.15.0 release notes ("Replace rustybuzz with HarfRust") and PR #417 commit `2610c86` merged 2025-09-09, is that **the migration first shipped in 0.15.0**. This README, [`lessons.md`](lessons.md), [`glossary.md`](glossary.md), [`history.md`](history.md), [`governance.md`](governance.md), [`ecosystem.md`](ecosystem.md), and [`integration.md`](integration.md) all say 0.15.0; the two Agent A files need to be edited to match.

## Sources

- cosmic-text repo and main-branch source — https://github.com/pop-os/cosmic-text
- crates.io metadata — https://crates.io/crates/cosmic-text
- Buiy text spec — `/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/text.md`
- Buiy foundation spec — `/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/README.md`
- Bevy PR #10193 (cosmic-text adoption) — https://github.com/bevyengine/bevy/pull/10193
- Bevy issue #21765 (cosmic-text → Parley migration) — https://github.com/bevyengine/bevy/issues/21765
- cosmic-text PR #417 (rustybuzz → harfrust) — https://github.com/pop-os/cosmic-text/pull/417
- Iced changelog — https://github.com/iced-rs/iced/blob/master/CHANGELOG.md
- bevy_cosmic_edit (archived) — https://github.com/StaffEngineer/bevy_cosmic_edit
- Parley (alternative) — https://github.com/linebender/parley
