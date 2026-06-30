**Date:** 2026-06-26
**Status:** report (empirical findings from the hot-reload prototype experiment; updates the matrix in [`2026-06-26-hot-reload-feasibility-research.md`](2026-06-26-hot-reload-feasibility-research.md))

# Hot-reload for Buiy — experiment findings + verified solutions

> The [feasibility report](2026-06-26-hot-reload-feasibility-research.md) was the *research*
> stage (a 5-dimension landscape + a layered recommendation, argued on paper). This report is
> the *experiment* stage: a throwaway prototype (`examples/hot_reload_lab`, built prototype-first
> on branch `worktree-hot-reload-prototype`, **unmerged**) **measured** each dimension by RUNNING
> probes, then **attacked** every problem it found with a running solution probe. It supersedes
> the feasibility report's *estimated* matrix with *measured* verdicts, and records which problems
> are solvable on the current codebase versus blocked. The detailed backing — the wave-by-wave
> journal, the keep/refine/redesign retrospective, and the `hot_reload_lab` harness — lives on
> the unmerged prototype branch `worktree-hot-reload-prototype`
> (`docs/prototypes/2026-06-26-hot-reload-experiment-{journal,RETROSPECTIVE}.md`); this report is
> the self-contained synthesis.

The prototype is a headless `StateSnapshot` (a 13-field "survival set" — caret, text, selection,
scroll, focus + liveness, an MVU model, theme token, entity bits) + a field-by-field `diff`. Each
probe drove a non-default UI state, triggered a reload, and diffed before/after. Every result
below was RUN and independently re-verified (exit 0).

---

## TL;DR

1. **The decisive variable is whether the reload RESPAWNS entities.** *In-place* reloads (asset,
   theme) preserve the entire live state for free; *respawn* reloads (scene re-apply, full-tree,
   anything minting new `Entity` ids) destroy **all** state unless it is `Reflect`-serializable
   and keyed by a **stable id**.
2. **The state-preservation crux is already solved by Buiy's MVU substrate.** The keystone probe
   reused the MVU record/replay path *verbatim* (`TypedReflectSerializer` → RON → `FromReflect`,
   keyed by `LogicalId`) as a reload state-migration path: a modeled value survived a full
   despawn/respawn onto a brand-new entity, while un-modeled editor state died.
3. **Every state-and-scene problem the experiment found is solvable TODAY** with running solution
   probes, **without a `buiy_core` API change** — and every solution keys on `LogicalId`, i.e.
   they are the hot-reload consumers of the (in-progress) MVU-as-core substrate.
4. **The only true blocker is the upstream `.bsn` text loader.** `bsn!` is compile-time only;
   `apply_scene` re-apply is destructive (below). The live code-patch and visual-repaint gaps are
   env/verification, not codebase.

---

## Part 1 — Measured results matrix (supersedes the feasibility estimate)

| Dim | Mechanism tested | Measured state-preservation result | Reload verdict |
|---|---|---|---|
| **1. Asset (font)** | in-place reshape (`FontsGeneration` bump) | **ALL 13 SURVIVED** — same entities, text/caret intact | ✅ works — in-place, never respawns (the landed beachhead) |
| **2. Style/theme** | `Theme` resource mutate + `is_changed()` | **12/13 SURVIVED**, only `theme_token` changed | ✅ works — tree never rebuilt; state-preserving by construction |
| **3. Scene/`bsn`** | `apply_scene` re-apply | root identity kept, but children **DUPLICATE + LEAK** (2→5 alive, old orphaned), runtime state **CLOBBERED** (99→0) | ⚠️ destructive as-is — needs a keyed reconciler + ownership split + a loader |
| **4. Rust code** | `bevy/hotpatching` (subsecond) via `dx` | feature + subsecond **compile + run** in a Buiy binary, `dx` serves it; live patch World-preserving *by construction* but un-observed (dx cold instrumented build didn't finish) | ◻️ dev-only, native-only, binary/tip-crate-only |
| **5. State-preservation** | naive respawn **vs** MVU `LogicalId`+`Reflect` restore | respawn = **everything LOST**; MVU restore = **model value SURVIVED** onto a new entity | ✅ model-is-data + stable key is the answer |

**Keystone detail.** Naive respawn lost 10/13 fields (entity ids churn; editor text/caret/
selection, scroll, counter all reset; focus dangles). The MVU `LogicalId`+`Reflect` restore
recovered the modeled `Counter` value (6→6) onto a new entity while the un-modeled editor text
still died — isolating the restore step as the cause and proving **the MVU replay path is a
hot-reload state-migration path, reused unmodified.**

**Methodology finding.** A raw `Entity` reference cannot be diff-detected as stale: after respawn,
`FocusedEntity` kept identical *bits* (never mutated) so a bits-only diff read "SURVIVED" while
the target was despawned. A `focused_entity_is_live` check exposed the dangle. **Raw `Entity` is
the wrong cross-reload currency; only a stable author key can both restore model state and remap
entity-keyed references.**

---

## Part 2 — Problems ATTACKED → solutions built + verified

Each problem was attacked with a running solution probe (`examples/hot_reload_lab/src/solutions.rs`).

| Problem | Solution (verified by running) | Codebase verdict |
|---|---|---|
| **P1a** scene reload duplicates children | **keyed reconciler** (PATCH-by-key / SPAWN-new / DESPAWN-absent) — 3 specs → 3 live → 3 total (vs `apply_scene` 2→5) | ✅ solvable now, user code |
| **P1b** old children leak | reconciler despawns absent keys — dropped key gone, no orphan | ✅ solvable now |
| **P1c** `apply_scene` clobbers runtime state | **description-vs-runtime ownership split** — `RuntimeTicks=77` survived (vs `apply_scene` 99→0) | ✅ solvable now |
| **P2** un-modeled editor state dies on respawn | **carve-out → respawn → rebuild** `{text, caret, selection}` — editor SURVIVES | ✅ solvable now (public API, ASCII single-line) |
| **P3** entity-ref dangle (focus) | **`LogicalId`→`Entity` remap** — `is_live` false→true, points at the new entity | ✅ solvable now, user code |
| **P1d** no runtime `.bsn` loader | a Buiy text→`Scene` interpreter (the interim loader is the proof-of-shape) | ⛔ **BLOCKED on upstream `.bsn` loader** |
| **P4** live code-patch un-observed | `dx serve --hot-patch` on an interactive dev host + a `dev-hotpatch` wiring (`HotPatchPlugin` manual outside `DefaultPlugins`) | 🔧 improve-support (env/toolchain; mechanism source-confirmed) |
| **P5** visual repaint un-observed | a GPU-lane reload test (theme/font re-resolve → pixels) | 🔧 improve-support (a verification test to add) |

**Every state-and-scene problem is solvable today without a `buiy_core` API change** — all
keying on `LogicalId`. The editor restore primitive even *already exists* in the codebase
(`TextEditState::restore_cursor`, `text/edit/input.rs:352-365` — `set_cursor` + `set_selection`),
just `pub(crate)`.

---

## Part 3 — "Improve support" (the small, designed production changes)

Not blockers — the clean production form of the verified prototypes:

1. **Expose a `TextEditSnapshot` `{text, caret, anchor, active}` + a public `restore`** wrapping
   the existing `restore_cursor`. The prototype's public Insert+`Motion` replay works for ASCII
   but has two sharp edges a direct seam removes: **byte-vs-grapheme** (`Motion` steps graphemes;
   the snapshot stores bytes) and **replay direction** (must extend toward the active end). This
   is **proto-3 MVU-as-core's already-designed `TextEditSnapshot` (§3 H5)** — not new work.
2. **A keyed child reconciler as a Buiy scene-reload layer** — replace `apply_scene`'s spawn-fresh
   child handling with PATCH/SPAWN/DESPAWN keyed on `bsn` `#Name` / `LogicalId`, plus the
   description-vs-runtime component-ownership classification (the feasibility report's L5).
3. **Build the `LogicalId` ↔ `Entity` resolver registry** (proto-3 D7, design-only today) so the
   remap generalizes beyond focus to anchors + the modal stack + the reconciler — one registry,
   the same "four consumers" key (a11y + agent + test + hot-reload-reconcile).
4. **(Optional) a `FocusedEntity` liveness guard** — clear/repoint focus when its target is
   despawned — cheap robustness even absent a full reload feature.

---

## Part 4 — Convergence with MVU-as-core (the strategic conclusion)

The parallel **proto-3 MVU-as-core** effort (in-progress on branch `worktree-mvu-core`)
*explicitly lists hot-reload as a goal* and already designs the two pieces the state problems
need — the `TextEditSnapshot` projection (§3 H5) and the `LogicalId` resolver (§3 D7) — but both
are **design-only** today. This experiment's solution probes are the **empirical validation** of
that substrate: model-is-data + `LogicalId` + `Reflect` is, with running code, Buiy's
state-preserving-reload answer.

**Recommendation:** the FINAL hot-reload spec should be **co-designed with the MVU-as-core spec**,
sharing one `LogicalId` + `Reflect` substrate — build the keyed reconciler + the editor snapshot
seam + the resolver registry *on* it, rather than reinventing them. The one piece outside that
substrate is the upstream `.bsn` text loader (a Buiy interpreter, or wait for upstream).

---

## Provenance

Built prototype-first (the `prototype-first-development` skill) on branch
`worktree-hot-reload-prototype` (off `origin/main @ 5c0da9f`). A headless `hot_reload_lab` harness
drove 7 characterization probes (W0 baseline → R1/R2 keystone → T/F → SCENE/LOADER) + 3 solution
probes (S1/S3/S4), each RUN and independently re-verified. The prototype **code is throwaway,
DO NOT MERGE** — its products are this report, the journal, and the retrospective. Solution-research
grounded against the `mvu-core` worktree's proto-3 design. Two coordinator-framing errors were
caught by verify-don't-trust (the W2 "compile error" and "headless stdout" claims) and corrected in
the journal. The live code-patch (P4) and visual repaint (P5) were not observed headless and are
deferred to a GPU/dev host.
