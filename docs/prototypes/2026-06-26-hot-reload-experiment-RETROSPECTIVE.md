# Buiy hot-reload experiment — RETROSPECTIVE (the prototype's deliverable)

> The prototype's product is LEARNING. This retrospective synthesizes the
> [journal](2026-06-26-hot-reload-experiment-journal.md) into keep / refine / redesign
> for the FINAL (a future `buiy-asset-pipeline-design` / hot-reload spec), and updates
> the [feasibility report](../reports/2026-06-26-hot-reload-feasibility-research.md)'s
> recommendations with EMPIRICAL verdicts. The code (`examples/hot_reload_lab/`, untracked)
> is an unmerged reference. **DO NOT MERGE.**

Built prototype-first: a headless `hot_reload_lab` harness (`StateSnapshot` over a 13-field
survival set + a field `diff`) drove 7 probes across all 5 hot-reload dimensions, each RUN
and verified. The feasibility report was the input spec; this experiment tested its claims.

---

## Verdict — is hot-reload achievable for Buiy, and how close did we get?

**Yes, dimension by dimension, and the report's hardest dimension (state-preservation) is
already solved by an existing subsystem.** Empirically:

- The decisive variable is **whether the reload respawns entities.** *In-place* reloads
  (asset/font, theme) preserve the **entire** live state for free (measured: 12–13/13
  survived). *Respawn* reloads (scene, full-tree, anything minting new `Entity` ids) destroy
  **all** state — UNLESS it is a `Reflect`-serializable model keyed by a stable id.
- **The state-preservation crux is solved by Buiy's MVU subsystem.** The keystone probe took
  the MVU record/replay path (`TypedReflectSerializer` → RON → `FromReflect`, keyed by
  `LogicalId`) **verbatim** and used it as a reload state-migration path: a `Counter` model's
  value survived a full despawn/respawn onto a brand-new entity, while the un-modeled editor
  text died. Model-is-data + a stable key = state-preserving reload.
- Reached running code for 4 of 5 dimensions; dim-4 (Rust code-patch) reached
  feature-integration + a served binary but the live patch was build-time-gated (below).

---

## Validated — KEEP (report recommendations the experiment CONFIRMED)

- **L2 "ship theme reload first, safe-by-construction"** — CONFIRMED: a theme-token mutation
  changed only `theme_token`; the other 12 fields survived (tree never rebuilt).
- **The asset/font beachhead preserves state in-place** — CONFIRMED: a `FontsGeneration` bump
  reshaped with ALL 13 fields surviving (no respawn).
- **`apply_scene` is an overlay that spawns children** — CONFIRMED exactly as the report cited
  (root identity kept; only scene-named components written; children spawned fresh).
- **Code-patch is dev-only / native-only / binary-tip-crate-only** — CONFIRMED in source
  (`connect_subsecond` is `#[cfg(not(wasm))]`; `HotPatchPlugin` only in `DefaultPlugins`).
- **The layered ship-order** (asset+theme now → scene later → migration) holds.

## REFINE / REDESIGN (where the experiment changed the picture)

1. **REDESIGN — couple hot-reload's state layer to the MVU subsystem; don't reinvent it.**
   The report framed L4 (author-stable key) and L6 (Reflect migration) as NEW hot-reload
   machinery to build. The experiment shows the **MVU subsystem already provides both**:
   `LogicalId` IS the stable key, and the Reflect replay log IS the migration. The hot-reload
   spec should be a **sibling of / co-designed with** `2026-06-26-buiy-state-management-design`,
   sharing one substrate. The report's "one key, three consumers" (reconcile + a11y + agent)
   becomes **one key, FOUR consumers** — all `LogicalId`.
2. **REFINE — `apply_scene` is WORSE than "duplicates": it LEAKS.** Old children are orphaned
   from the root but left ALIVE (still carrying runtime state), so a reload loop leaks the
   whole prior subtree each pass. The scene-reload design must **explicitly despawn or
   reconcile** the old subtree by key — `linked_spawn` does NOT clean them up on `Children`
   overwrite.
3. **REFINE — raw `Entity` is the wrong cross-reload currency.** The focus-dangle finding:
   `FocusedEntity` kept identical bits after respawn while pointing at a despawned entity — a
   bits-only diff couldn't even SEE the dangle (needed a liveness check). On any respawn, ALL
   entity-keyed refs (focus, anchor maps, modal stack) must be **remapped by `LogicalId`** — or
   avoid respawn (in-place patch).
4. **REFINE — name the two reload CLASSES explicitly in the spec:** *in-place* (asset/theme —
   state-safe, no key needed) vs *respawn* (scene / code-with-layout-change — needs key-based
   restore). They have opposite state-preservation profiles and must not be conflated.
5. **REFINE — `HotPatchPlugin` is manual outside `DefaultPlugins`** — a concrete wiring detail
   for any future `dev-hotpatch` feature on a Buiy app.

## Framework / system sharp edges the prototype surfaced

- **`bevy_scene` `apply_scene` re-apply LEAKS the prior child subtree** (orphaned, alive, not
  despawned). Upstream behavior, not a Buiy bug — but load-bearing: a Buiy scene-reload MUST
  despawn/reconcile, and this is worth an upstream note.
- **`FocusedEntity` holds a raw `Entity` with no liveness guard** — silently dangles after a
  respawn. A real robustness gap the moment Buiy respawns any subtree (which scene reload does).
- **`FocusPlugin` needs `InputPlugin`** (`ButtonInput<KeyCode>`) even headless — minor harness
  ergonomics (buiy_core's headless text support omits `FocusPlugin` to dodge it).
- **`cosmic_text::Motion` isn't re-exported by `buiy_core`** (and `Node` isn't in
  `bevy::prelude`) — minor authoring papercuts.

## Problems ATTACKED — solutions built + verified (current codebase)

Each problem the experiment found was attacked with a running solution probe
(`examples/hot_reload_lab/src/solutions.rs`). Verdict per problem:

| Problem | Solution (verified) | Solvable on current codebase? |
|---|---|---|
| **P1a** scene reload duplicates children | **keyed reconciler** (PATCH-by-key / SPAWN-new / DESPAWN-absent) | ✅ YES, user code — 3 specs→3 live→3 total (vs apply_scene 2→5) |
| **P1b** old children leak | reconciler despawns absent keys | ✅ YES — dropped key despawned, no orphan |
| **P1c** `apply_scene` clobbers runtime state | **description-vs-runtime ownership split** (never author runtime comps) | ✅ YES — `RuntimeTicks=77` survived (vs apply_scene 99→0) |
| **P2** un-modeled editor state dies on respawn | **carve-out → respawn → rebuild** (`{text,caret,selection}` plain data) | ✅ YES on the public API (ASCII single-line); the existing `restore_cursor` makes the production seam tiny |
| **P3** entity-ref dangle (focus) | **`LogicalId` resolver remap** (`HashMap<lid,Entity>` → repoint `FocusedEntity`) | ✅ YES, user code — `is_live` false→true, points at the new entity |
| **P1d** no runtime `.bsn` loader | a Buiy text→`Scene` interpreter (the interim loader is the proof-of-shape) | ⛔ BLOCKED on upstream `.bsn` loader → improve-support (build interpreter, or wait) |
| **P4** live code-patch un-observed | `dx serve --hot-patch` on an interactive dev host + a `dev-hotpatch` feature wiring (`HotPatchPlugin` manual) | 🔧 improve-support (env/toolchain, not codebase; mechanism source-confirmed) |
| **P5** visual repaint un-observed | a GPU-lane reload test (theme/font re-resolve → pixels) | 🔧 improve-support (a verification test to add) |

**The decisive result: every state-and-scene problem is solvable TODAY without a `buiy_core`
API change** — and every solution keys on **`LogicalId`**, i.e. they are the hot-reload
consumers of the proto-3 MVU-as-core substrate. The only true blocker is the upstream `.bsn`
text loader.

### "Improve support" — the small, designed changes the production versions want

These are not blockers; they are the clean production form of the verified prototypes:

1. **Expose `TextEditState::restore_cursor` publicly + a `TextEditSnapshot` `{text, caret,
   anchor, active}`** (the proto-3 H5 type). The prototype proved the public Insert+Motion
   replay works for ASCII but has two sharp edges a direct seam removes: **byte-vs-grapheme**
   (`Motion` steps graphemes; the snapshot stores bytes) and **replay direction** (must extend
   toward the active end). The seam wraps code that ALREADY EXISTS — a ~tiny addition that also
   handles non-ASCII/IME robustly. **Owned by the MVU-as-core text-edit work (W3), not new.**
2. **A keyed child reconciler as a Buiy scene-reload layer** (replacing `apply_scene`'s
   spawn-fresh child handling with PATCH/SPAWN/DESPAWN keyed on `bsn` `#Name` / `LogicalId`) +
   the description-vs-runtime component-ownership classification. This is the report's L5; the
   prototype proved the shape end-to-end.
3. **Build the `LogicalId` ↔ `Entity` resolver registry** (proto-3 D7, design-only today) so the
   remap generalizes beyond focus to anchors + the modal stack + the reconciler — one registry,
   the same "four consumers" key.
4. **(Optional) a `FocusedEntity` liveness guard** — clear/repoint focus when its target is
   despawned — cheap robustness even absent a full reload feature.

## Residual gaps for the FINAL to close

(The scene reconciler and the editor carve-out are no longer gaps — both were attacked and
verified above; see *Problems ATTACKED*. What genuinely remains:)

- **The upstream `.bsn` text loader** (P1d) — the only true BLOCKER. `bsn!` is compile-time
  only, so a runtime file reload needs the deferred upstream loader or a Buiy text→`Scene`
  interpreter (the interim loader is the working proof-of-shape).
- **Live code-patch (P4) un-observed** — `dx serve --hot-patch` did a cold instrumented
  full-tree rebuild that didn't finish in the bounded window, so the app never ran under dx.
  Re-run on an interactive dev host (~10–15 min build, use `info!`/tracing not `println!`).
  Env/toolchain, not a codebase gap; mechanism is source-confirmed.
- **Visual repaint/reshape (P5) un-observed** — the experiment proved STATE survival headless;
  the actual paint re-resolve (theme `is_changed()` gate) and glyph reshape are GPU-gated.
  Add a GPU-lane reload test to confirm pixels update.
- **Productionize the verified prototypes** — the `TextEditSnapshot`/`restore` public seam, the
  keyed reconciler as a Buiy scene-reload layer, and the `LogicalId`↔`Entity` resolver registry
  (all detailed under *Improve support* above) — small, designed, owned mostly by MVU-as-core.

## Build strategy (for the final)

- **Co-design the hot-reload spec with the MVU state-mgmt spec** (shared `LogicalId` + Reflect
  substrate). This is the single biggest learning — state-preservation is not new machinery.
- **Ship order (empirically grounded):** theme/token asset reload → generalized native asset
  watch → a `dev-hotpatch` feature flag (native, binary-crate) → scene reload (gated on the
  upstream `.bsn` loader + the keyed reconciler + ownership split) → cross-shape migration.
- **The prototype code stays throwaway** (DO NOT MERGE). Its product is this retrospective + the
  journal's filled matrix, which update the feasibility report with measured verdicts.
