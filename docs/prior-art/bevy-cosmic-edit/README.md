**Date:** 2026-05-22
**Status:** archived
**Subject:** bevy_cosmic_edit — third-party Bevy plugin bridging cosmic-text into bevy_ui (and 2D sprites); archived case study.

# bevy_cosmic_edit

> **Why archived.** The upstream repository (`Dimchikkk/bevy_cosmic_edit`) was [archived by its owner on 2025-03-21](https://github.com/Dimchikkk/bevy_cosmic_edit) and is now read-only. The last release was **0.26.0 on 2024-12-07**, pinned to **Bevy 0.15**. There is no migration path to Bevy 0.16+. Bevy's `main` (0.19-dev) has additionally migrated `bevy_text` off cosmic-text onto Parley + swash (issue [#21765](https://github.com/bevyengine/bevy/issues/21765)), removing the original architectural niche this crate filled.
>
> This folder is preserved as a **historical case study** of a third-party crate that tried to bridge two fast-moving Rust ecosystems (Bevy + cosmic-text) and got caught in the seam. It is **not a reference for active use**. The synthesis lives in [`lessons.md`](lessons.md) — read that first if you came here from a Buiy spec.

## What it was

bevy_cosmic_edit was a third-party Bevy plugin that bridged the [`cosmic-text`](../cosmic-text/README.md) crate into Bevy applications as a multi-line text-input widget. It worked on both `Sprite` (2D world) and `Node` (bevy_ui) entities, exposed an `EditorBuffer` query type wrapping a `CosmicEditor` + `CosmicEditBuffer` pair, painted glyphs into Bevy textures, and routed `winit` keyboard + mouse + clipboard input into cosmic-text `Action` calls. At peak it was the de-facto answer to "how do I get a real text input in a Bevy app" while bevy_ui's own `Text` widget was display-only.

It is the canonical instance of a structural anti-pattern: a third-party crate maintaining hand-rolled compatibility with two upstream projects that each cut breaking releases on independent cadences (Bevy every ~3 months; cosmic-text every ~1–2 months pre-0.20). When the cost of keeping the bridge alive exceeded the volunteer maintainer's bandwidth — and bevy_ui's own text stack began catching up — the bridge was abandoned. See [`why-archived.md`](why-archived.md).

## Key facts

| Field | Value |
|---|---|
| Crate name | `bevy_cosmic_edit` |
| Latest (final) version | **0.26.0** (2024-12-07) |
| Bevy compat at archive | **0.15** |
| Repository | `https://github.com/Dimchikkk/bevy_cosmic_edit` |
| Crates.io owner | `StaffEngineer` (publishing alias) |
| GitHub owner (de-facto maintainer) | `Dimchikkk` (Dima) |
| Other repeat contributors | `ActuallyHappening`, `databasedav`, `bytemunch`, `iancormac84` |
| License | MIT OR Apache-2.0 |
| Total downloads (lifetime) | 40,832 (per crates.io, May 2026) |
| Stars at archive | 110 |
| Forks at archive | 14 |
| Total PRs (closed + open at archive) | 98 (97 merged/closed, 1 left open) |
| Last commit on `main` | 2025-02-04 ("Lil cleanup", PR #170) |
| **Archive date** | **2025-03-21** |
| Description (crates.io) | "Bevy cosmic-text multiline text input" |

## Table of contents

- [`architecture.md`](architecture.md) — how the bridge worked: `CosmicEditPlugin`, the `Editor` + `Buffer` split, render-to-texture pipeline, input routing.
- [`api.md`](api.md) — public component surface, prelude, event flow, how an app spawned a text input.
- [`history.md`](history.md) — version timeline from earliest tags through 0.26.0, Bevy version pinning per release, post-release commit activity.
- [`why-archived.md`](why-archived.md) — the structural analysis. Bridge-crate maintenance burden, Bevy's catch-up trajectory, bus-factor failure, the cosmic-text 0.20 churn.
- [`integration.md`](integration.md) — what plugging the crate in looked like; Cargo features; coexistence with bevy_ui's own `Text`.
- [`ecosystem.md`](ecosystem.md) — peak production usage, downstream consumers, and the comparison space (vs bevy_ui Text, vs cosmic-text directly, vs egui editing).
- [`critiques.md`](critiques.md) — known critiques during the active years + open problems at the time of archive (IME, BiDi, undo, perf).
- [`lessons.md`](lessons.md) — **the consult-this-when-designing file.** What bevy_cosmic_edit's archive validates about Buiy's stance; what to avoid; what design patterns are worth borrowing.
- [`glossary.md`](glossary.md) — system-specific terms.

## How to use this folder

This is a **learn-from-failure** artifact. The folder exists because:

1. Buiy's foundation spec ([`text.md` § 3.5 Text editing](../../specs/2026-05-07-buiy-foundation/text.md#35-text-editing)) commits Buiy to owning its text-edit surface end-to-end. bevy_cosmic_edit's archive is the canonical real-world data point that **third-party bridge crates between two fast-moving Rust UI ecosystems are not sustainable** without funded stewardship.
2. Several Buiy specs will be tempted to cite bevy_cosmic_edit as prior art for component shape, IME boundary, render-to-texture pipeline, or input routing. Some of those patterns are worth borrowing; the **crate itself must not be a dependency**. See [`lessons.md`](lessons.md) for the split.
3. The cosmic-text and bevy_ui folders both cite bevy_cosmic_edit as an anti-pattern. This folder is their footnote.

When designing a Buiy text-edit feature: start at [`lessons.md`](lessons.md); read [`why-archived.md`](why-archived.md) before deciding to revive any pattern; ignore [`api.md`](api.md) unless you specifically need to compare component-surface shapes.

**Framing disclosure.** This folder is written from a Buiy-owns-its-text-edit stance. The "Lessons for Buiy" framing in [`lessons.md`](lessons.md) treats bevy_cosmic_edit's archive as **validating** that stance. A reader auditing whether Buiy should instead try to revive or fork bevy_cosmic_edit should weigh this corpus accordingly: it's a learn-from-bevy_cosmic_edit-into-own-the-text-edit-surface artifact, not a neutral catalog.

## Cross-references

- [`docs/prior-art/cosmic-text/`](../cosmic-text/) — the substrate bevy_cosmic_edit bridged. Read [`cosmic-text/lessons.md`](../cosmic-text/lessons.md) row "Depending on `bevy_cosmic_edit`."
- [`docs/prior-art/bevy-ui/`](../bevy-ui/) — the host ecosystem bevy_cosmic_edit plugged into. Read [`bevy-ui/text-and-input.md`](../bevy-ui/text-and-input.md) for the three-engine text-rendering timeline (ab_glyph → cosmic-text → Parley) bevy_cosmic_edit could not keep up with.
- [`docs/specs/2026-05-07-buiy-foundation/text.md`](../../specs/2026-05-07-buiy-foundation/text.md) § 3.5 — the Buiy editor-surface spec this folder informs.

## Glossary stub

See [`glossary.md`](glossary.md) for the full list. The most-cited terms:

- **bridge crate** — a third-party crate whose primary job is to adapt one upstream project's API to another upstream project's component model. bevy_cosmic_edit was a bridge crate between cosmic-text and bevy_ui.
- **CosmicEditPlugin** — the entry-point Bevy plugin the consumer added to their `App`.
- **EditorBuffer** — a Bevy `QueryData` type the consumer queried mutably to access the cosmic-text `Editor` + `Buffer` for a text-input entity.
- **render-to-texture** — bevy_cosmic_edit rasterized cosmic-text glyphs into a CPU `image::RgbaImage`, uploaded as a Bevy `Image`, and displayed via `Sprite` or `ImageNode`. It did NOT integrate with bevy_text's glyph atlas.

## Sources

- bevy_cosmic_edit repository (archive banner) — https://github.com/Dimchikkk/bevy_cosmic_edit
- crates.io listing — https://crates.io/crates/bevy_cosmic_edit
- docs.rs page for 0.26.0 — https://docs.rs/crate/bevy_cosmic_edit/0.26.0
- Dimchikkk's GitHub profile — https://github.com/Dimchikkk
- StaffEngineer GitHub profile (crates.io owner alias) — https://github.com/StaffEngineer
- Bevy issue #21765 (cosmic-text → Parley) — https://github.com/bevyengine/bevy/issues/21765
- Sibling files in this folder.
