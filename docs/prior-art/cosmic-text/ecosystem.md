---
**Date:** 2026-05-22
**Status:** active
**Subject:** cosmic-text — adjacent crates, downstream consumers, community surface
---

# Ecosystem

cosmic-text sits at a contested layer of the Rust text stack. Above it: UI toolkits choosing between cosmic-text, Parley, and Skia-based engines. Below it: a small set of pure-Rust font-and-shaping crates that cosmic-text composes. This file maps the neighborhood.

Cross-links: [history.md](history.md) (chronological adoption), [governance.md](governance.md) (stewardship contrast with Parley), [integration.md](integration.md) (per-embedder integration shape), [critiques.md](critiques.md) (cosmic-text vs Parley tradeoffs).

## Substrate (what cosmic-text builds on)

Verified from the 0.19.0 `Cargo.toml` dependencies list:

- **`harfrust 0.5.0`** — pure-Rust HarfBuzz port. The shaping engine. Replaced `rustybuzz` in PR #417 (merged 2025-09-09, first in 0.15.0).
- **`swash 0.2.6`** — pure-Rust font format reader and rasterizer (`dfrg`). Provides the bitmap glyph output.
- **`skrifa 0.40.0`** — pure-Rust font metadata and outline access (Google Fonts team / `googlefonts/fontations`).
- **`fontdb 0.23`** — pure-Rust font enumeration and loading (`RazrFalcon`).
- **`unicode-bidi 0.3.18`** — UAX #9 BiDi (Servo project).
- **`unicode-linebreak 0.1.5`** — UAX #14 line-break opportunity finder.
- **`unicode-script 0.5.8`** — script-property lookup.
- **`unicode-segmentation 1.12.0`** — grapheme / word / sentence iterators (Servo).
- **`rangemap 1.7.1`** — interval-map data structure for attribute spans.
- **`self_cell 1.2.2`** — owning-cell pattern used internally.
- **`smol_str 0.3.2`** — small-string optimization (Rust-Analyzer team).
- **`linebender_resource_handle 0.1.1`** — Linebender's font-resource abstraction (interesting: cosmic-text consumes a tiny piece of Linebender infrastructure even though it's not on Parley).
- **Optional (`vi` feature):** `modit 0.1.5` (vi-keys parser, System76), `syntect 5.3.0` (syntax highlighting), `cosmic_undo_2 0.2.0` (System76's undo crate).

The pure-Rust stack is the defining trait. cosmic-text pulls in zero C code in its default configuration. (`syntect` optionally uses `onig` C bindings, but cosmic-text only pulls `syntect` under the `vi` feature.)

## Adjacent: alternative Rust text engines

### Parley (Linebender)

**The competing modern Rust text-layout crate.** Maintained by the Linebender umbrella project (xilem, vello, kurbo, druid lineage). Current latest: 0.7.0.

| | cosmic-text | Parley |
|---|---|---|
| Shaping | HarfRust (HarfBuzz port; rustybuzz pre-0.15) | swash's shaper + `harfrust` (Parley 0.4+ added harfrust path) |
| Rasterization | swash | swash (via vello / piet rendering) |
| BiDi | unicode-bidi | unicode-bidi |
| Layout | hand-written in cosmic-text | hand-written in Parley |
| Editor surface | yes (`Editor`, `ViEditor`) | yes (`PlainEditor`, recent addition) |
| Color emoji (COLRv1) | No (open issue #446) | Yes (vello / swash with COLRv1 path) |
| Variable fonts | Yes since 0.15 (weight axis works; other axes patchy per #406) | Yes |
| Vertical writing | No | No (also missing) |
| Hyphenation | No | No (also missing) |
| Stewardship | System76 (single company) | Linebender (foundation-style, multi-org) |
| Primary downstream | COSMIC, Iced, Bevy | xilem, vello-based tools, Floem |
| License | MIT OR Apache-2.0 | Apache-2.0 OR MIT |

**Buiy chose cosmic-text** per the foundation spec ([README.md goal #4](../../specs/2026-05-07-buiy-foundation/README.md)) — cosmic-text is the same primitive Bevy's `bevy_text` uses, so Buiy-parallel-to-bevy_ui integrating cosmic-text directly matches the substrate choice Bevy already shipped. Picking Parley would have meant divergence from the rest of the Bevy ecosystem; cosmic-text is the lower-friction choice for "parallel-to-bevy_ui."

The downside Buiy inherits: cosmic-text has the COLRv1 gap that Parley does not (because Parley's vello rendering path goes around swash for color glyphs). Buiy's `buiy-text-rendering-design` sub-spec will need to address the COLRv1 question — likely by maintaining its own color-emoji rasterizer or waiting on swash's COLRv1 implementation.

### glyph_brush / ab_glyph (legacy)

`alexheretic`'s ecosystem. `ab_glyph` for individual glyph parsing, `glyph_brush` for batched rasterization + texture-atlas management. **No shaping. No BiDi. No fallback.** Bevy used this through 0.13 and migrated away in 0.14. Still actively used by simple games that don't need complex script support. Not a serious alternative for a comprehensive UI library — the script-coverage ceiling is too low.

### font-kit (Servo)

Apple-style font enumeration and metadata API. Not a layout engine. Sometimes shows up under other engines, including older versions of cosmic-text-adjacent code.

### Pango-rs, harfbuzz_rs

Bindings to C libraries (Pango, HarfBuzz). Drag in C build dependencies; few pure-Rust UI libraries adopt them. Wezterm and Alacritty historically used variants of these but have been migrating away.

### sergei / piet-text / others

Smaller experiments, none production-grade enough to displace cosmic-text or Parley.

## Downstream apps and crates (active)

Confirmed via crates.io reverse-dependency search and direct repo inspection:

**Production UI toolkits using cosmic-text:**

- **Iced** (`iced-rs/iced`) — since 0.10.0 (2023-07-28). Currently on cosmic-text 0.15 in Iced 0.14.0 (2025-12-07). Uses `glyphon` for wgpu-side atlas. Largest non-COSMIC consumer.
- **Bevy** (`bevyengine/bevy`) — since 0.14 (2024-07-09). `bevy_text` crate wraps cosmic-text. Sets the precedent for Buiy's parallel approach.
- **libcosmic** (`pop-os/libcosmic`) — the COSMIC desktop's iced fork. Indirect consumer via Iced.

**Production apps using cosmic-text (mostly via COSMIC stack):**

- COSMIC text editor (`pop-os/cosmic-edit`) — uses `ViEditor` with syntect.
- COSMIC Files, COSMIC Settings, COSMIC OSD, COSMIC Panel — all via libcosmic.
- COSMIC Compositor (`pop-os/cosmic-comp`) — title bars, OSD text.

**Bevy-side editing plugin (archived):**

- **`bevy_cosmic_edit`** (`StaffEngineer/bevy_cosmic_edit`, originally `Dimchikkk`) — most recent Bevy 0.15 compat. **Archived 2025-03-21.** Buiy's `buiy-text-editing-design` sub-spec will likely build its own editing surface rather than resurrect this; the lessons (especially the multi-line BiDi caret traversal pattern) are reusable but the maintenance trail isn't worth picking up.

**Other crates depending on cosmic-text** (per crates.io reverse-dependency search, with verified download numbers as of authoring):

- **`iced_glyphon`** (527,400 downloads) — Iced's glyphon fork.
- **`cryoglyph`** (238,193 downloads) — a glyphon fork/successor used by some downstream renderers.
- **`iced_graphics`** (237,470 downloads) — Iced's renderer-agnostic graphics layer.
- **`iced_tiny_skia`** (235,250 downloads) — Iced's tiny-skia CPU backend.
- **`bevy_text`** (193,413 downloads) — Bevy's text crate.
- **`gpui`** (89,329 downloads) — Zed's UI framework. **Status unclear**: present on crates.io reverse-deps but absent from `zed-industries/zed` main Cargo.toml as of mid-2026, suggesting Zed either migrated or vendored.
- **`glyphon`** (5,141 downloads) — the upstream `grovesNL/glyphon`. Most actual usage flows through `iced_glyphon` or `cryoglyph` instead.
- **`femtovg`** (3,485 downloads) — vector graphics + text.
- **`basalt`** (1,894 downloads) — small UI/render lib.
- **`uiua`** (808 downloads) — array programming language editor.

The Iced + Bevy + COSMIC trio accounts for the overwhelming majority of cosmic-text's runtime exposure.

## Not actually downstream (common misattributions)

- **Freya** (`marc2332/freya`) — uses Skia (via `freya-skia-safe`), not cosmic-text. Sometimes listed wrongly.
- **Floem** (`lapce/floem`) — uses Parley, not cosmic-text. Lapce-the-editor uses cosmic-text indirectly via glyphon, but Floem-the-toolkit does not.
- **Egui-cosmic-edit** — the pre-amble named this possibility; no such crate found on crates.io at folder-authoring time. The active egui text path uses `epaint`'s own simpler layout.

## Community surface

- **GitHub Issues** (`pop-os/cosmic-text/issues`) — primary support channel. ~98 open issues at folder authoring time.
- **GitHub Discussions** — enabled but lightly used; most conversation happens in PR threads or issues.
- **No Discord, no Matrix room** specifically for cosmic-text. The COSMIC desktop has its own Discord; cosmic-text conversation happens there incidentally.
- **System76 Mattermost** — internal, not public.
- **#bevy-ui, #iced channels** on the respective Bevy and Iced Discords — where most integration troubleshooting happens (not on the cosmic-text repo).

## Sources

- cosmic-text Cargo.toml — https://github.com/pop-os/cosmic-text/blob/main/Cargo.toml
- crates.io reverse-deps API — https://crates.io/api/v1/crates/cosmic-text/reverse_dependencies
- Parley repo — https://github.com/linebender/parley
- glyph_brush / ab_glyph — https://github.com/alexheretic/glyph-brush
- Freya Cargo.toml (confirmed Skia, not cosmic-text) — https://github.com/marc2332/freya/blob/main/Cargo.toml
- Floem Cargo.toml (confirmed Parley, not cosmic-text) — https://github.com/lapce/floem/blob/main/Cargo.toml
- bevy_cosmic_edit (archived) — https://github.com/StaffEngineer/bevy_cosmic_edit
- libcosmic — https://github.com/pop-os/libcosmic
- COSMIC desktop — https://github.com/pop-os/cosmic
