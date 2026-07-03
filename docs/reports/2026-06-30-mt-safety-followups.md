# MT-safety audit — deferred follow-ups

Date: 2026-06-30
Companion to: `docs/specs/2026-06-30-mt-safety-design.md`

The MT-safety campaign (make Buiy correct under Bevy's `multi_threaded` executor —
which downstream consumers already enable via feature unification) ran the full
headless suite under the MT executor and a multi-agent static sweep of all crates.
The bugs fixed in the campaign PR are in the spec (D2–D7). This report records the
hazards the audit surfaced that were **deliberately NOT fixed** in that PR, with
enough detail to action later. Scope discipline: the campaign was "MT-safety"; these
are either latent (no current trigger), out of MT scope, or dead code.

## H6 — `prepare_effect_groups` re-derives the dirty bits `prepare_buiy_instances` used (latent)

**Actioned 2026-07-03** (Stage 0 of the glyph partial re-extract change): prepare publishes `PreparedDamage`; compositor reads it.

- **Where:** `crates/buiy_core/src/render/compositor.rs:635-636` vs `render/prepare.rs:330-331`.
- **What:** `prepare_effect_groups` recomputes `quad_dirty`/`glyph_dirty` from `is_changed()`
  to mirror the per-tier repack signals `prepare_buiy_instances` used, then gates an in-place
  alpha-fold + range-merge on the shared `ResMut<BuiyInstanceBuffers>`. Correctness needs the
  two `is_changed()` evaluations to agree.
- **Why deferred:** Correct TODAY — the two are order-pinned (`prepare_effect_groups
  .after(prepare_buiy_instances)` + the `ResMut<BuiyInstanceBuffers>` access conflict) and
  both run unconditionally so change-ticks advance in lockstep. It is a *fragility*, not a
  live bug: inserting a system that touches those resources between them, adding a `run_if`,
  or dropping the `.after` would silently desync the gates — and only under buffer-budget
  degradation (never with the 64 MiB default), so it would escape steady-state tests.
- **Fix when actioned:** have `prepare_buiy_instances` publish the per-tier dirty bits it
  actually used into a small render resource; `prepare_effect_groups` reads that. One source
  of truth, immune to reorder/`run_if` drift.

## H7 — `clear_warned_once_on_exit` will need explicit ordering when wired (dead code)

- **Where:** `crates/buiy_core/src/layout/systems.rs:531`; note at `layout/mod.rs:76-78`.
- **What:** Full-`clear()`s `LayoutWarnedOnceSession`. Currently UNWIRED (no BuiyState/BuiyExit
  lifecycle yet — plan D7), so no hazard today; the five `.insert()` writers all take `ResMut`
  (serialized, no UB).
- **Why deferred:** dead code — nothing schedules it.
- **Fix when actioned:** when the exit/teardown lifecycle (plan D7) wires it, place it in a
  dedicated teardown set ordered `.after` the whole `BuiySet::Layout` chain, or gate it on a
  state-exit run condition — so it can't wipe warnings mid-session or no-op depending on order.

## H8 — `PlaceholderBuffer` excluded from the `FontsGeneration` reshape sweep (NOT an MT bug)

- **Where:** `crates/buiy_core/src/text/edit/placeholder.rs:97-121` vs the sweep at
  `crates/buiy_core/src/text/sync.rs:263-273`.
- **What:** `text_sync_buffers` reshapes every editor/display buffer on a `FontsGeneration`
  bump, but `sync_placeholder` reshapes its `PlaceholderBuffer` only when the placeholder
  *string* changes — it never consults `FontsGeneration`/`FontDbLineage`. After a fresh-lineage
  system-font scan, the placeholder's `layout_runs()` still carry old `cache_key.font_id`s →
  wrong/missing glyphs (tofu/blank placeholder) until the next placeholder-string edit.
- **Why deferred:** **this is a single-threaded bug, not an MT-correctness issue** — out of
  this campaign's scope. It rides the same `FontsGeneration` machinery the audit examined, so
  it was surfaced here, but it reproduces single-threaded. Low severity: `system_fonts` is off
  by default (fresh-lineage swap is opt-in), requires an empty editor showing a placeholder at
  the swap instant, and self-heals on the next edit.
- **Fix when actioned:** in `sync_placeholder`, also reshape when `Res<FontsGeneration>
  .is_changed()`, mirroring the `text_sync_buffers` sweep arm. (Belongs in a text/font-handling
  change, not an MT PR.)

## Minor — stale doc comment

`crates/buiy_core/src/text/font_system.rs:56` says the font mutex has "exactly three lock
sites"; there are ~13 now (all sound — single non-reentrant mutex, no nesting, no lock-order
surface). The campaign PR corrects this comment (it was encountered during the audit); noted
here for traceability.
