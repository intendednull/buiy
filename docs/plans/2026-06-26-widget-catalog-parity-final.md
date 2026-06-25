# Widget Catalog Parity — FINAL Implementation Plan (Phase 3)

From `docs/specs/2026-06-26-widget-catalog-parity-final-design.md`. On
`parity-final` (off `main` @ `fdb8dda`). Strategy: **port the validated prototype
work + land the re-decided refinements.** `parity-final` shares the base
`fdb8dda` with `parity-prototype`, so prototype commits are reachable for
cherry-pick. **DO NOT MERGE — human-review-gated.**

## Execution model
- Port a layer (cherry-pick its prototype commits, or apply the layer's diff +
  reword), resolving conflicts, then land that layer's **refinements** as
  deliberate commits. Gate each wave (fmt/clippy/doc/test; GPU lanes for render).
- Sequential (the layers depend: theme/anim → render → shell). Each wave runs in
  the warm worktree; verify by **running the GUI** + the headless gate.
- After every wave, a fresh review agent checks the port + refinements.
- Prototype reference: `/mnt/storage/projects/buiy/.claude/worktrees/parity-prototype`
  (journal, code, the per-wave commit SHAs).

## Prototype commit map (the validated work to port)
- **A (foundations):** theme `755ef50` · animation `4ee58d6` · fonts+LetterSpacing
  `9908fbd` · fmt `3d248ce`.
- **B (render caps):** gradients `e3866a5` · dotted-grid `96ea8e6` · icons
  `d494e63` · backdrop-blur `272a4ef`.
- **Render fixes:** M1+M6 cross_root_rank `61ea95c` · D-polish (M5/⌘/blink/search)
  `eb60a7b` · capstone (Display::None paint-skip + inspector nuances) `00f034e`.
- **C (shell+screens):** C1 shell+router `d2b6a3c` · C2 composites `eec9609` ·
  TodoMVC `2ec7f0d` · scroll `886e21a` · menu `04cc07c` · modal `32626af` ·
  showcase `2f0fbbc` · LetterSpacing fix `80ad82f` · C4 inspector+theming `cfa0f69`.
- Skip the prototype's doc/journal/snapshot-only commits (the final has its own docs).

## Waves

### Wave 1 — API/prelude + extract-query partition DESIGN (design-only)
- Design `buiy::prelude` + the promotions list (spec §2 REFINE).
- Design the **nested extract-query** partition (`extract_buiy_base/colors/effects/
  gradients/icons` over a shared `ExtractedNodes` map) — the seam doc the render
  port (Wave 3) builds to, so the refactor lands as render caps are ported, not
  after. Output: a short `docs/specs/.../extract-partition.md` seam note.
- Review + lock. *(No code yet.)*

### Wave 2 — Port A (theme + animation + fonts) + refinements
- Port `755ef50` `4ee58d6` `9908fbd` `3d248ce` onto parity-final.
- **Refine:** `AnimatedBackgroundColor` auto-composites in `resolve_background_color`;
  add the prelude promotions for these types; confirm **framework default stays
  LIGHT** + the gallery will insert `default_dark_theme()` explicitly (spec §2
  resolution 2); confirm **LetterSpacing px contract** (spec §2 resolution 1 — keep
  `px/font_size` lowering; do NOT switch to em).
- Gate: fmt/clippy/doc + the A-wave tests (~1340 headless).

### Wave 3 — Port B (gradients/dotted/icons/blur) + render fixes + extract refactor
- Port `e3866a5` `96ea8e6` `d494e63` `272a4ef` + `61ea95c` (cross_root_rank) +
  the render parts of `eb60a7b` (M5) + `00f034e` (Display::None paint-skip).
- **Redesign as ported:** apply the Wave-1 nested extract-query partition (don't
  reproduce the monolithic query + its mid-flight patch).
- Gate: ≥1800 headless + the buiy_core render/GPU smoke; defer lavapipe goldens
  to Wave 6.

### Wave 4 — Port C1 shell + ScreenRouter + the 5 screens
- Port `d2b6a3c` (shell+router) + `2ec7f0d` `886e21a` `04cc07c` `32626af`
  `2f0fbbc` (the 5 screens) + `80ad82f` (LetterSpacing fix — already px).
- Gallery inserts `default_dark_theme()` explicitly at boot (the dark-default
  resolution). Standardize the inspector/footer "mounted" wording.
- Gate: shell_layout + router + per-screen snapshots; **run the GUI**, screenshot
  all 5 screens.

### Wave 5 — Port C2/C4 (composites + inspector) + promotions + headless plugin
- Port `eec9609` (composites) + `cfa0f69` (inspector + live accent-swap) + the
  inspector-nuance fixes from `00f034e`.
- **Refine:** promote the general composites (`meter`, `table_row`/`table_header`,
  `search_input`, `kbd`, `status_dot`, `pulse_blink`) to `buiy_widgets`; build the
  **`BuiyPlugin::headless()`/`BuiyHeadlessPlugin`** + repoint the capture path to it.
- Gate: inspector state-sync + live accent-swap + composite unit tests.

### Wave 6 — Parity audit + lavapipe goldens + full gate + human-review prep
- Re-capture all 5 screens + the accent-swap; compare to values.md; audit the
  resolved bugs (M1-M6) + any new finding.
- **Bless CI goldens on pinned lavapipe** (logo gradient, dotted bg, icons, blur,
  caret) per the campaign's reconstruction technique; wire the dual-path
  verification (headless always + GPU goldens) into `buiy_verify`/CI.
- Full workspace gate (fmt/clippy/doc/check/test + both GPU lanes) green.
- Write `FINAL-DECISIONS.md` (the prototype→final narrative + the resolved
  decisions) + the docs index updates. **Open the PR → STOP at the human-review
  gate** (no self-merge).

## Risks
Cherry-pick conflicts on shared files (lib.rs/render) across layers (resolve in
order); the extract refactor interacting with ported render caps (Wave 1 seam
de-risks it); lavapipe golden recalibration (campaign technique exists); keeping
the commit series reviewable.
