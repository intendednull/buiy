# Buiy follow-ups drain — campaign plan

**Date:** 2026-06-18
**Status:** active
**Spec:** realizes the named deferrals in `docs/plans/follow-ups.md` against their originating specs (layout, render-pipeline, text-editing).

**Goal:** Drain every *actionable* open follow-up in `docs/plans/follow-ups.md`, update the spec + follow-ups entry as each lands, and explicitly re-classify the ones that are blocked, superseded, speculative, or deliberately deferred — so the backlog reflects reality.

**Architecture:** Slices are grouped into tracks by file-locality. The single ~5700-line `crates/buiy_core/src/layout/systems.rs` is the master serialization point — every layout follow-up edits it, so the layout track is **strictly sequential**. Render and text-editing tracks own disjoint file domains. Execution per slice: a workflow drafts a TDD plan → fresh skeptical plan review → fix → TDD implement task-by-task with per-task review (no commits) → **I run the full gate myself** (headless `xvfb-run cargo test --workspace`; GPU `--ignored` lane for slices with `#[ignore]` GPU tests) → commit → PR. The per-slice full-gate discipline is load-bearing: it has caught cross-phase regressions that per-task reviews missed.

This plan was produced by the `followups-triage` workflow (6 cluster analyzers → synthesis → skeptical review). The review's one MAJOR finding (render PackedInstance contract coupling) is folded into the render track below.

---

## Build list (actionable — ordered)

### Track LAYOUT — sequential (shared `layout/systems.rs`), branch `followups-layout`

1. **Refactor `anchor_resolution` into sub-helpers** — behavior-preserving base, sequenced FIRST so the anchor body-edits rebase onto clean helpers. `build_anchor_edge_map` / `apply_anchor_broken_markers` / `emit_anchor_warns`. Pinned by the existing anchor suite staying green. (`systems.rs`)
2. **`anchor-size()` term** — add `Length::AnchorSize(AxisDimension)` + `AxisDimension{Width,Height}`; resolve against the per-try anchor box (plumbing already in scope at `try_anchored_position`); retire/repurpose `AnchorErrorKind::AnchorSizeUsed`. **Reverses the § 3.4 v1.x deferral** — update spec § 3.4 + README § 5. Drop the over-specified `AnchorRef` payload from the sketch. (`types.rs`, `systems.rs`, `tests/layout_anchor_positioning.rs`)
3. **`AnchorRef::Entity` end-to-end test** — test-only; fills the coverage gap (all 11 existing cases use `AnchorRef::Name`). Add a direct-ref positive + a despawned/`Display::None` `TargetMissing` negative. (`tests/layout_anchor_positioning.rs`)
4. **Sticky `Length::Cq*` inset resolution** — reuse `resolve_cq_unit_px`; sticky's CQ frame is the sticky entity's own nearest CQ ancestor. (`systems.rs`, `tests/layout_sticky.rs`)
5. **Sticky both-top-and-bottom dual clamp** — band-clamp in `compute_sticky_displacement`; flip the `sticky_both_top_and_bottom_inset_top_wins` regression test. (`systems.rs`, `tests/layout_sticky.rs`)
6. **Sticky inside sticky** — `world_position` consults `PostTaffyPositionOverrides` when walking ancestors; depth-sort so outer sticky resolves before inner. (`systems.rs`, `tests/layout_sticky.rs`)
7. **will-change SC trigger former** — both will-change headings as ONE `forms_stacking_context` edit; an SC-forming `WillChangeProperty` forms a `StackingContext`. The **layer-promotion half stays deferred** (no composition-layer/`RenderLayers` concept exists in render/) — flip only the SC-former half in follow-ups.md. (`systems.rs`, `tests/layout_stacking.rs`)
8. **Non-px translate units in `compose_transform`** — resolve percent translate against the entity's own resolved box; `Cq*` translate split out as a residual note. (`systems.rs`, `tests/layout_transforms.rs`)

### Track RENDER — **sequential** (shared PackedInstance contract), branch `followups-render`

> **Hard constraint (review MAJOR):** the two slices are NOT parallel-safe despite disjoint files. UiTransform grows `PackedInstance` (+affine) and degraded-groups re-tints alpha at float index 7. UiTransform lands FIRST; its affine columns are appended **after** the existing 13 floats so alpha stays at index 7; the color/alpha offset is read from a **named const** (not a literal). Degraded-groups rebases onto the new layout.

1. **UiTransform affine paint** — extract the full affine; `PackedInstance` basis columns appended after the existing floats; quad/shadow WGSL applies the affine about `TransformOrigin` (default 50% 50%) before the logical→clip map; GPU rotate reftest. **Scope = transform-paint only**; PAINT-clip is already done (`clip.rs:196`), perspective/`Preserve3d`/`BackfaceVisibility` stay C-tier deferred (narrow the follow-up, don't close it). (`extract.rs`, `instance.rs`, `pipeline.rs`, `shader.wgsl`, tests)
2. **Degraded effect groups forward-composite flat** — re-route degraded group ranges into the flat draw with per-instance opacity folded in (spec § 2.3 mandates forward-compositing; skip-as-degradation contradicts the landed spec). **Design note first:** fold opacity via **re-tint-in-place** in `prepare_effect_groups` (lower risk than reordering `prepare_effect_groups` ahead of `prepare_buiy_instances`). Re-tint reads the alpha offset from the named const. (`compositor.rs`, `node.rs`, `prepare.rs`, tests)

### Track TEXT-EDITING — disjoint files (impl serialized in-worktree for FS safety), branch `followups-text`

1. **BiDi split caret secondary indicator** — `secondary_caret_rect_for` from glyph-level affinity geometry (`cursor_from_glyph_left/right`), `SecondaryCaretVisual` seat, extract a second solid stamp. The primary caret is already correct; this adds the mixed-direction secondary indicator only. (`text/edit/caret.rs`, `text/components.rs`, `text/extract.rs`, tests)
2. **Compose-over-selection** — `delete_selection` (as one undo unit, paired with the composition group per § 6.2c) before the first preedit splice, then re-anchor the span. **Revised contract:** emit `TextChanged` when the pre-splice delete removed text. (`text/edit/ime.rs`, `text/edit/state.rs`, tests)
3. **HTML + image clipboard flavors** — `ClipboardProvider` html get/set; image behind a `clipboard-image` cargo feature. arboard 3.6.1 `Get::html()` is on the cross-platform base builder (OQ#3 resolved) — sanity-confirm the locked version first. (`text/edit/clipboard.rs`, `input.rs` copy/cut/paste block, `command.rs`, `Cargo.toml` ×2, tests)

---

## Deferred — documented, not built (speculative / unused-now)

These are flagged because building them now is the speculative work the dev guidelines warn against. Each follow-ups.md entry is updated with the reason + a concrete re-open trigger.

- **`position_try_max_depth` cap** — both spec § 3.5 and README § 5 gate it on "if profiling surfaces a hot path"; no profiling evidence exists. An unused knob. **Re-open trigger:** a measured deeply-nested-fallback hot path.
- **Multi-reference reftest aggregation** — `reftests.md` notes no current pairing needs it; tested-but-unused machinery. **Re-open trigger:** a real logical-vs-physical (≥2 reference) pairing appears.
- **`golden-prune` advisory bin** — corpus is 2 cells; nothing to prune. **Re-open trigger:** golden corpus grows enough that stale positives are plausible.

## Doc-flip → LANDED (already done in code, follow-up entry stale)

- **Anchor target IS sticky/table/multicol** — closed by Phase 7 Task 9 (`systems.rs:1757` reads `PostTaffyPositionOverrides.by_entity` first; tested in `layout_sticky.rs::anchor_target_is_sticky_anchored_tracks_displaced_position`).
- **Node-draw model: per-entity clip + composite passes** — superseded by Option C (R8b per-instance fragment-discard clip) + R9 effect-compositor GPU orchestration (`c0a5fe0`).

## Confirmed blocked — stay deferred (gated on unbuilt subsystems / renderer features)

- Cross-window anchor targets, per-window top layer → `buiy-window-and-surface-design` (not chartered).
- Sticky em/rem/Vh/Vw/Vmin/Vmax insets → those `Length` variants don't exist (Phase 10 viewport units + a font phase).
- `clear_warned_once_on_exit` wire-up → `BuiyState`/`BuiyExit` lifecycle states don't exist.
- R11 forced-colors BoxShadow draw-skip, shadow-blur residue golden, color-emoji residue golden, FC BoxShadow visual reftest → no real BoxShadow/color-emoji extract+draw pipeline.
- Multi-range selection *behavior* → needs an N-editor/N-selection design decision; also reopens the caret.rs surface BiDi-caret owns.
- Object-store golden migration → scale trigger (>50 MB / >500 positives) not fired; `goldens.md` forbids building it now.

---

## Done criteria (per slice)

1. New/changed tests written test-first and green at the lowest tier (headless `tests/*.rs`; GPU tests `#[ignore]`).
2. Full gate green: `xvfb-run -a cargo test --workspace` + `cargo fmt --check` + `cargo clippy -D warnings` + `cargo doc`; GPU `--ignored` lane for slices with GPU tests (RX 6700 XT / lavapipe).
3. Spec touchpoint updated (remove the deferral / close the open question).
4. The follow-ups.md entry flipped to **LANDED** with a one-paragraph "as landed" note.
