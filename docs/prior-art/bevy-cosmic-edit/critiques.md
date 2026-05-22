**Date:** 2026-05-22
**Status:** archived
**Subject:** bevy_cosmic_edit — critiques of the bridge architecture during active years + open problems at archive (IME, BiDi caret, undo/redo, performance, render pipeline).

# Critiques and open problems

This file catalogs the technical critiques bevy_cosmic_edit faced during its active years (2023-2025) and the open problems unresolved at archive. It is **not** a hit piece; the goal is to surface the structural issues so a Buiy spec author can avoid reproducing them.

## Bridge-crate maintenance burden

The dominant critique, surfaced in [`why-archived.md`](why-archived.md). Restated here in the cost-budget shape:

- Each Bevy minor (Bevy 0.11 → 0.12 → 0.13 → 0.14 → 0.15, ~quarterly): API breakage in `Component`, `Resource`, `Plugin`, render-graph, picking, input. Each migration was a tagged release.
- Each cosmic-text minor (~bimonthly pre-0.20): API breakage in `Buffer`, `Editor`, `Shaping`, `Cache`, `FontSystem`.
- Each supporting-crate minor (winit, wgpu, image, arboard, fontdb): occasional ABI breakage.

The maintenance graph looked roughly like: **40-60% of releases were "compatibility bumps"** rather than feature work. The 0.17 release explicitly *removed* placeholders, password fields, and undo/redo to shrink the surface (per CHANGELOG: "removed several features ... to maintain a minimal core"). That's the cost of being a bridge talking to itself.

## Render pipeline performance

The render-to-texture choice (see [`architecture.md` § Render pipeline](architecture.md#render-pipeline--render-to-texture-not-glyph-atlas)) had measurable consequences:

- **Per-keystroke re-upload of the CPU image.** Typing into a wide text field re-rasterized the visible area and uploaded a new `Image` asset every frame the buffer was dirty. For small inputs (~300x40 px) the cost was tolerable on a desktop GPU; for wider multi-line documents (1000x800 px) the per-frame upload became visible in profilers.
- **No subpixel positioning across editors.** Each `CosmicEditBuffer` had its own `SwashCache` (cosmic-text's glyph cache). Two editors showing the same font at the same size duplicated all the rasterized glyphs.
- **No GPU atlas sharing with bevy_text.** A bevy_text label and a bevy_cosmic_edit input next to each other rasterized the same characters twice.

Issue [#145](https://github.com/Dimchikkk/bevy_cosmic_edit/issues/145) ("buffer does not update while widget is focused") captured a related perf bug where layout re-runs from neighbor components could stall the focused editor's update — a side effect of the all-or-nothing dirty-flagging.

There was **no published benchmark** at any document size. The "1000+ node verification" target in Buiy's foundation spec (foundation `verification.md`) has no comparable data point in bevy_cosmic_edit's archived history.

## IME boundary

Surface-level: bevy_cosmic_edit set `Window.ime_allowed = true` on focus and translated `winit::event::Ime::Commit(text)` to cosmic-text `Action::Insert(c)` per character. **Preedit was not rendered.**

What this meant for users:

- Typing Chinese / Japanese / Korean with system IME: the candidate window appeared (via winit), but nothing showed in the text field until commit.
- The user had no in-line visual feedback of the composition state.
- The system's preedit underline / highlight was invisible.
- Composition + cancel (Esc) silently dropped the composition with no visible state change.

This was a known shape-of-the-problem gap: cosmic-text leaves IME to the embedder (per [`../cosmic-text/editing.md` § IME](../cosmic-text/editing.md#ime-composition-the-embedder-boundary)), and bevy_cosmic_edit could not implement preedit without either patching cosmic-text upstream (not within the maintainer's scope) or maintaining a parallel preedit overlay (which would have required substantial work in `render.rs`).

This gap directly informs Buiy's IME requirement ([`text.md` § IME composition](../../specs/2026-05-07-buiy-foundation/text.md#34-typography)): preedit rendering, preedit cursor positioning, composition as undo unit, composition popup positioning. All four are **F**oundation tier.

## BiDi caret traversal

cosmic-text's caret model (`Cursor { line, index, affinity }`) is BiDi-correct, and visual cursor motion via `Motion::Left / Right` produces the right behavior on mixed-direction lines (see [`../cosmic-text/bidi.md`](../cosmic-text/bidi.md)).

bevy_cosmic_edit inherited this — but several second-order behaviors weren't surfaced in its API:

- **Selection extension across direction changes** was buggy in mixed-direction text. A selection started in a Latin run and extended into an Arabic run could end up with the selection rectangle on the "wrong side" of the boundary in some cases (no formal bug filed, anecdotal community reports).
- **`affinity` choice on click** was driven by cosmic-text's heuristic; bevy_cosmic_edit didn't expose a way for the consumer to override.
- **No `dir="auto"` analog.** Per-paragraph direction inference (e.g. Hebrew vs English email body) had to be done by the consumer pre-`set_text` by inserting `U+2066–U+2069` isolate characters — same trap as cosmic-text itself (see [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md) row on this).

## Undo / redo — removed, never restored

Per CHANGELOG, undo/redo was *removed* in 0.17 to shrink the surface, with a stated plan to restore via "internal plugins if users request them through pull requests." No such PR was ever merged.

What survived: cosmic-text's `Editor` emits `Change` values (see [`../cosmic-text/editing.md`](../cosmic-text/editing.md)). A consumer could in principle accumulate these and implement undo themselves. bevy_cosmic_edit did not surface this; the consumer would have had to maintain a parallel `Change` log keyed to the entity.

For Buiy: Buiy's foundation spec marks "Undo / redo with composition-aware grouping" as **F**oundation tier. bevy_cosmic_edit's archive-time gap means there's no shippable Bevy ecosystem precedent to point at; Buiy will be implementing this from cosmic-text primitives.

## Event API thinness

There was one Bevy event (`CosmicTextChanged`) and no others (see [`api.md` § Events](api.md#events)). Consumers had to poll. Missing events that any production text-edit surface needs:

- `CursorMoved` / `SelectionChanged`.
- `CompositionStart` / `CompositionUpdate` / `CompositionEnd`.
- `EditCommitted` / `EditCancelled`.
- `FocusGained` / `FocusLost` (separate from `FocusedWidget` resource changes).

This is consistent with bridge-crate scope-limiting: implementing a rich event API costs maintenance.

## Render-coexistence with bevy_ui

Already covered in [`integration.md` § Coexistence](integration.md#coexistence-with-bevy_uis-text). The repeat-from-here detail is: **a bevy_ui layout containing both a `Text` label and a `CosmicEditBuffer` input had no shared font cache and no shared atlas**. Memory and rasterization work were duplicated. The cost compounded with the number of editor instances on screen.

## 3D rendering — never landed

PR #168 "Basic 3D Support" (ActuallyHappening, 2024-12-13) attempted to render text-edit on 3D meshes. It was left open at archive. There is no shipping 3D text-edit on bevy_cosmic_edit, and no fork has continued the work.

## Documentation coverage

docs.rs reported **67.11% documentation coverage** for 0.26.0 (51 of 76 items documented). Issue [#131](https://github.com/Dimchikkk/bevy_cosmic_edit/issues/131) was a documentation tracking issue, open since April 2024, never closed. The crate shipped with examples (~11 example files covering basic UI, sprites, font-per-widget, image-background, multiple sprites, password, placeholder, readonly, scroll, sprite+UI clickable) but the API surface itself was sparsely documented.

## Open problems unresolved at archive

| Problem | Status |
|---|---|
| Preedit IME rendering | Not implemented. No tracking issue. |
| Per-paragraph BiDi direction override | Not implemented. Consumer must insert Unicode formatting chars. |
| Undo / redo | Removed in 0.17, never restored. |
| Glyph atlas sharing with bevy_text | Not implemented; architectural mismatch. |
| Vertical writing modes | Not implemented (cosmic-text gap; bevy_cosmic_edit inherited). |
| Hyphenation | Not implemented (cosmic-text gap; inherited). |
| 3D text rendering | PR #168 open at archive; never merged. |
| 1000+ node perf bench | No published number. |
| Spell-check OS integration | Not implemented. |
| Autocorrect / autocapitalize | Not implemented. |
| Multi-cursor | Not implemented. |
| Find / replace | Not implemented. |
| Rich-text "toggle bold for selection" interactive editing | Not implemented; `set_rich_text` was set-only, not toggle-on-selection. |
| Placeholder not operating on text buffer (issue #171) | Open at archive, never resolved. |

## The structural anti-pattern, summarized

bevy_cosmic_edit's critiques cluster into three categories:

1. **Architecture limits the maintainer couldn't fix.** Render-to-texture pipeline, lack of atlas sharing, per-buffer SwashCache. Fixing these required deep restructuring; no volunteer-bandwidth path.
2. **Features the bridge couldn't extend upstream for.** Preedit IME, BiDi-correct selection extension, hyphenation, vertical writing modes. cosmic-text owned these gaps; the bridge could only inherit them.
3. **Scope explicitly removed for tractability.** Undo/redo (in 0.17), placeholder/password as core (split out to opt-in plugins). The maintainer made the cost-cut visibly.

All three categories are the same shape: **third-party crate trying to maintain compat with two upstream fast-movers, with no funding or dogfood substrate to drive prioritization, eventually retreats and then archives.** That's the pattern. Buiy's job is to not reproduce it. See [`lessons.md`](lessons.md).

## Sources

- Issue list at archive — https://github.com/Dimchikkk/bevy_cosmic_edit/issues
- Issue #131 (Documentation tracking) — https://github.com/Dimchikkk/bevy_cosmic_edit/issues/131
- Issue #145 (buffer-does-not-update-while-focused) — https://github.com/Dimchikkk/bevy_cosmic_edit/issues/145
- Issue #171 (Placeholder buffer mutation) — https://github.com/Dimchikkk/bevy_cosmic_edit/issues/171
- PR #168 (Basic 3D Support, never merged) — https://github.com/Dimchikkk/bevy_cosmic_edit/pull/168
- CHANGELOG entry for 0.17 — https://github.com/Dimchikkk/bevy_cosmic_edit/blob/main/CHANGELOG.md
- docs.rs coverage (67.11%) — https://docs.rs/crate/bevy_cosmic_edit/0.26.0
- cosmic-text IME boundary — [`../cosmic-text/editing.md`](../cosmic-text/editing.md)
- cosmic-text BiDi — [`../cosmic-text/bidi.md`](../cosmic-text/bidi.md)
- cosmic-text lessons — [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md)
- Buiy text.md (IME requirements) — [`../../specs/2026-05-07-buiy-foundation/text.md`](../../specs/2026-05-07-buiy-foundation/text.md)
