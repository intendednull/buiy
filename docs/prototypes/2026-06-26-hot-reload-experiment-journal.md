# Buiy hot-reload — Prototype Dev Journal

> PROTOTYPE — exploratory, **DO NOT MERGE**. The deliverable is this journal + the
> retrospective. The code (`examples/hot_reload_lab/`) is an unmerged reference.

**Goal:** empirically map the *boundary of the possible* for hot-reloading a running
Buiy app — every dimension — by building probes and RUNNING them.
**Worktree:** `worktree-hot-reload-prototype`, off base `origin/main @ 5c0da9f`.
**Reference:** feasibility report `docs/reports/2026-06-26-hot-reload-feasibility-research.md`
(on `worktree-hot-reload-feasibility-research`).

---

## The experimental space (established + sign-off'd)

Every experiment is one cell of **(reload UNIT) × (survival SET) → verdict**.

**Reload units:** U1 font · U2 image · U3 theme token · U4 style value · U5 `bsn`
subtree restyle · U6 `bsn` subtree *structure* (add/remove/reorder) · U7 system body ·
U8 widget/observer body · U9 component data value · U10 component **shape** (the wall).

**Survival set:** caret · text content · selection · scroll offset · focus ·
hover/press · blink/animation phase · atlas residency · **entity identity** (keystone) ·
layout tree · modal stack · IME preedit.

**Mechanisms:** M1 `AssetEvent::Modified` · M2 Resource-mutate + `is_changed()`
re-resolve · M3 `apply_scene`/`ScenePatch` re-apply (+interim loader) · M4 Bevy
`hotpatching`/subsecond (code bodies) · M5 Reflect serialize→migrate→respawn · M6
identity-preserving in-place patch (keyed reconcile).

**Decisions (sign-off'd):** interim `.bsn` loader = BUILD it (real file reload) ·
MVU = FIRST-CLASS arm · harness = one `hot_reload_lab` + micro-binaries for M4.

### The MVU arm — the keystone hypothesis (H8)

The MVU prototype's record→replay path **is** a state-preserving-reload path. Three
ingredients (all present in the copied substrate):
- **Model-is-data** — `#[derive(Component, Reflect)]` pure data (`runtime.rs` `Model` trait).
- **`LogicalId(u64)`** — stable key decoupled from `Entity` bits (`runtime.rs:94-99`),
  "aligned by intent to the agent-interface test-id space" = the report's L4 key.
- **`Reflect` serialization** — `collect_state` snapshots `Query<(&LogicalId, &Model)>`.

**Thesis to test:** snapshot Models by `LogicalId` → reload (despawn+respawn, minting
new `Entity` ids) → restore each Model onto its new entity by `LogicalId` → measure
byte-identical survival. If it holds, MVU *is* Buiy's answer to the un-reflected
state-island problem. Known gaps to probe: `#[reflect(ignore)]` callback fields
re-default on respawn; child reconciliation; in-flight `Cmd::task`.

### Waves
- **W0** harness: `StateSnapshot` + `snapshot_diff` + representative UI (exercises the whole survival set).
- **W1** assets + theme (M1/M2).
- **W2** Rust code body patch (M4) — `dx serve --hot-patch`; the stale-cache + World-survival probes.
- **W3** interim `.bsn` loader + `apply_scene` reconcile-vs-duplicate (M3/M6).
- **W4** state migration + the shape wall (M5/M10) **+ the MVU arm (H8)**.

### Hypotheses
H0 does the `dx`/subsecond toolchain drive a Buiy binary at all · H1 hotpatch preserves
the World on a body edit · **H2 cached shaping/layout/extract go stale → need a
`HotPatched` re-sync (key unknown)** · H3 theme reload preserves the tree by
construction · H4 whole-tree `apply_scene` duplicates children + clobbers runtime text ·
H5 respawn churns Entity ids → breaks Entity-keyed maps · H6 un-reflected
`TextEditState`/`TextBuffer` dropped by serialize-respawn · H7 a shape change crashes
subsecond — locate the wall · **H8 MVU model-is-data + `LogicalId` makes
state-preserving reload trivial.**

### Success
The filled `unit × survival` matrix + framework bugs surfaced by running it + a
retrospective re-deciding every choice for the final. NOT a shipped feature.

---

## Running log

### 2026-06-26 — W0 (in progress): harness scaffold + MVU base-drift check

- Built: scaffolded `examples/hot_reload_lab` (lib + bin + workspace member); copied the
  MVU substrate (`runtime.rs`/`routing.rs`/`callback.rs`, ~558 LOC) verbatim from the
  untracked `mvu_native` prototype-2; wrote a smoke `main` (a `Counter` MVU actor folded
  through the real drain) to validate the substrate compiles + folds on base `5c0da9f`
  (the substrate was built/verified on the older base `59cd50e` = #81).
- Ran the artifact → **PASS.** `cargo run -p hot_reload_lab` printed
  `LogicalId(1) Counter.value = 6` (Add(5)+Inc), zero warnings. The MVU substrate
  compiles AND folds correctly on `5c0da9f` despite being built on `59cd50e` —
  **base-drift risk eliminated** (the substrate is Bevy-native, depends only on
  `buiy_core::interaction::OnPress` which is stable). The MVU arm's foundation is live.
- Surprised by / friction: none yet — the copy-onto-newer-base "just worked", matching
  the Explore agent's low-drift prediction.
- If we did this again: validate the riskiest copied dependency with a smoke binary
  *first* (before any UI) — cheap de-risk, done.

### 2026-06-26 — W0 (DONE): StateSnapshot + diff machinery + representative UI

- Built: `examples/hot_reload_lab/{ui,snapshot}.rs` + a probe-runner `main`. The
  representative headless UI exercises the logical survival set — an editor
  (`TextEditState`, text "Hello world", caret `(0,6)`, selection `((0,6),(0,8))`), a
  scroll container (`ScrollOffset.y = 42`), focus, and the **MVU arm seed** (a
  `Counter{value:6}` + `LogicalId(1)`). `StateSnapshot::from_world` captures 12 fields
  incl. **entity bits** (the churn detector); `diff` classifies each `Survived`/`Changed`.
- Ran the artifact → **PASS** (verified independently, exit 0): correct state semantics;
  **no-op baseline diff = ALL 12 SURVIVED** → the machinery itself introduces zero drift.
- Findings (API corrections = real experimental data, headed for the retrospective):
  1. **Headless stack needs `bevy::input::InputPlugin` before `FocusPlugin`** — `FocusPlugin`'s
     `handle_tab` reads `Res<ButtonInput<KeyCode>>`; `MinimalPlugins` omits it → first-update
     panic. (buiy_core's own headless *text* support omits FocusPlugin to dodge this.)
  2. `cosmic_text::Motion` is **not** re-exported by `buiy_core` → the example needs a direct
     `cosmic-text = "0.19"` dep to name `Motion::Left` for `EditCommand::Motion`.
  3. `Node` is `buiy_core::Node`, not in `bevy::prelude`.
  4. `from_world(&World)` **cannot** use `World::query_filtered` (it needs `&mut World`) →
     used `iter_entities()` + `EntityRef::{contains,get,id}`.
  5. Enqueue from outside a system: `enqueue` needs `&mut Commands` → write
     `Messages<Envelope<Counter>>` directly (what `enqueue`'s deferred closure does).
  6. `TextEditState::for_font_size(16.0)` is the metrics-free seam (= `Metrics::new(16.0,19.2)`).
- Methodology lessons:
  - **`blink_origin_ms` is per-process nondeterministic** (stamped from `Time<Virtual>`) but
    STABLE within a process → all reload diffs must be **within one process** (this harness is).
  - The editor's `TextSync` `is_editor` branch never calls `set_text`, so the empty
    `Text(String::new())` companion does **not** clobber the editor buffer — inserted text
    survives every settle. (Good: text survival is a clean signal.)
  - **Headless cannot observe render-world state** (atlas residency, modal stack, pixels) —
    those need the GPU lane. But the **state-preservation axis (the survival set) IS fully
    observable headless**, and visual repaint/reshape is the secondary, GPU-gated axis. The
    experiment's primary question (does state survive?) is answerable here; the "did pixels
    update?" question is deferred to a GPU run.

### 2026-06-26 — KEYSTONE (dim-5 state-preservation + the MVU arm, H8): the central result

Drove the state-preservation dimension first (highest learning, fully headless-observable).
Two probes in `src/probes.rs`, both run + verified independently (exit 0):

- **R1 — naive respawn** (despawn+respawn the UI, NO restore): 10/13 fields lost.
  Entity bits churn (editor 205→202, scroll 204→201, counter 203→200); editor
  text/caret/selection, scroll offset, and counter value all reset to defaults;
  focus dangles. **H5 (entity-id churn) + H6 (un-reflected state destroyed on respawn)
  CONFIRMED.** This is hot-RESTART masquerading as hot-reload.
- **R2 — MVU `LogicalId`+`Reflect` restore** (checkpoint Models by `LogicalId` via the
  PROVEN replay serialization `TypedReflectSerializer`→RON→`FromReflect`, respawn, restore
  by id): **`counter_value` SURVIVED 6→6 onto a NEW entity** (bits churned 203→200), while
  the un-modeled editor text STILL LOST. **H8 CONFIRMED.**

**The headline finding (the report's thesis, now empirical):** the *modeled* island
(an MVU `Counter`, `#[derive(Reflect)]` + `LogicalId`) survives a full despawn/respawn
reload through the unmodified record/replay machinery; the *un-modeled* island (the
editor's `cosmic_text`-backed `TextEditState`) does not. **Model-is-data + a stable key
= state-preserving reload; raw component state on a churned `Entity` = lost.** The MVU
subsystem's replay path IS a hot-reload state-migration path, reused verbatim.

**Methodology finding (sharpened the harness):** a raw `Entity` reference cannot be
diff-detected as stale — after respawn, `FocusedEntity` kept identical *bits* (the
resource was never mutated) so a bits-only diff reads "SURVIVED" while the target is
despawned. Added `focused_entity_is_live` (does the focused entity exist?) → the dangle
now shows `true → false`. **Raw `Entity` is the wrong cross-reload currency; only a
stable author key (`LogicalId`) can both restore model state AND remap entity-keyed
references (focus, anchors, the modal stack).** This is exactly the report's L4
"author-stable key, one key / three consumers" — empirically motivated here.

**Surprise:** Bevy did NOT recycle the freed entity indices in-frame (despawned
205/204/203 → respawns took 202/201/200, not reused), so the dangling focus pointer
names *no* live entity at all — a concrete, not just theoretical, stale reference.

Matrix cells filled: (respawn × every survival element) = LOST without a key;
(respawn × MVU-model-value) = SURVIVED with `LogicalId`+`Reflect`.

### 2026-06-26 — W1 (DONE): theme (dim-2) + font/asset (dim-1) — the non-respawn reloads

Two probes added + run + verified independently:

- **Probe T — theme reload (H3):** mutate `Theme.colors["color.text.primary"]` (the
  `Changed<Theme>` signal a `Modified` asset-reload observer would raise). Diff =
  **1 CHANGED (`theme_token`), 12 SURVIVED** — same entity ids, caret/scroll/focus/text/
  counter all intact. **H3 state-preservation CONFIRMED:** a theme reload preserves the
  whole logical tree BY CONSTRUCTION (never rebuilt). Caveat: the *visual* re-resolve
  (extract `theme.is_changed()` gate) is GPU-gated, not observed headless — but the
  `Changed` signal is correctly raised, so a GPU run would repaint.
- **Probe F — font reload (the beachhead, dim-1):** bump `FontsGeneration` (the production
  `apply_font_registry` reload signal → the `TextSync` all-buffers reshape sweep). Diff =
  **ALL 13 SURVIVED** — same editor entity, text "Hello world", caret, selection, blink
  all survive the in-place reshape. Asset reload preserves state because it mutates
  buffers in place and never respawns. Corroborated by the existing gate
  `text_font_reload_survival.rs:94`.

**The dimension contrast is now clean:** respawn-based reload (R1) loses everything;
`LogicalId`+`Reflect` restore (R2) recovers only the *modeled* island; resource/asset
reloads (theme, font) preserve the *entire* survival set — **because they never
respawn.** The single variable that decides state survival is *whether the reload
respawns entities*: in-place mutation = safe; respawn = needs a stable-key restore.

### 2026-06-26 — W3 (DONE): scene / `bsn` re-apply (dim-3, H4) — confirmed + WORSE than predicted

Researched the real rc.3 API then measured it. `EntityWorldMut::apply_scene` (`bevy_scene
spawn.rs:499`) is the re-apply entry; path `apply_scene → ScenePatch::apply →
ResolvedSceneRoot::apply`.

- **Root identity preserved** (overlay): re-applying onto a live root keeps its `Entity`
  id; only scene-named components are written, un-authored ones (a `SurvivorMarker`)
  survive → apply_scene is an OVERLAY, not a replace.
- **H4 CONFIRMED + a nastier nuance:** children DUPLICATE — `SceneEntityReferences` is
  built fresh per apply and each `bsn!` mints fresh per-call-site refs, so a re-apply can
  never find the prior children. Measured: 2 children → **5 alive** (2 old + 3 new). And
  the old children **leak** — overwriting `root.Children` orphaned them (removed from the
  root) but left them ALIVE (ids 180/181 still in the world, still carrying their runtime
  `ticks=77`). So a naive scene-reload loop leaks the whole previous subtree each time —
  worse than clean duplication.
- **Runtime state clobbered in place (Slint #8369 CONFIRMED):** `root.RuntimeState.ticks`
  99 → 0 — the scene re-authors the component at its default on top of the live value.
- **`bsn!` is compile-time only** — it expands to type-named Rust + baked call-site refs;
  there is NO runtime `String → Scene` entry, and the `.bsn` TEXT loader is deferred
  upstream (every rc.3 spawn doc says "the `.bsn` file format is not yet released"). So a
  real `.bsn` file-reload is impossible today without the upstream loader or a Buiy
  interpreter. The throwaway **interim loader** (a one-line-per-`Name=width` text format
  with a by-NAME reconciler) demonstrates the file-edit→reload loop AND the fix the native
  path lacks: by-name reconcile gives 2→3 children (NO duplication, surviving ids kept).

**Verdict:** a production Buiy scene hot-reload **cannot** be `apply_scene` alone — it
needs (a) the upstream `.bsn` text loader (or a Buiy interpreter) and (b) a **keyed child
reconciler** (key on `bsn` `#Name`) + a description-vs-runtime component-ownership split
(so re-apply patches description, never clobbers runtime state). Exactly the report's L4/L5.
The interim loader is the working proof-of-shape.

### 2026-06-26 — W2 (DONE): Rust code hot-patch (dim-4) — feature ✓, live patch un-observed

*(This entry was corrected after the W2 agent verified ground truth and caught two wrong
claims in my coordinator framing — verify-don't-trust applies to my own narration too.)*

- **Feature integration — FULLY CONFIRMED.** `cargo build -p hot_reload_lab --bin
  hotpatch_probe --features dev-hotpatch` compiles (1m45s) and the binary **runs standalone**,
  ticking `HOTPATCH tick=N patched_value=1` ~2/sec. So `bevy/hotpatching` + subsecond
  **coexist cleanly with Buiy's dep tree**. `cargo tree -i subsecond` shows `subsecond
  v0.7.9` pulled via **two** paths: `bevy_ecs/hotpatching` (instruments system execution)
  AND `dioxus-devtools v0.7.9` (the dx CLI connection).
- **`HotPatchPlugin` is MANUAL for `MinimalPlugins`** — verified it sits in `DefaultPlugins`
  ONLY (`bevy_internal/src/default_plugins.rs`, `#[cfg(feature="hotpatching")]`). It is the
  piece that calls `connect_subsecond()` + registers the `HotPatched` receiver (opens the
  socket to dx, applies incoming jump tables). A headless `MinimalPlugins` app **must add it
  manually** or it never connects/patches. (A real finding for a future dev-hotpatch wiring.)
- **H0 (does dx drive a Buiy binary?) — PARTIAL YES.** `dx serve --hot-patch --platform
  desktop ... --bin hotpatch_probe --features dev-hotpatch` accepted the non-dioxus Buiy/Bevy
  binary, resolved the package, and printed `Serving your app: hotpatch_probe! 🚀`.
- **THE WALL (cost, not a tooling rejection): H1 NOT OBSERVED.** That banner prints at
  *server start, before the build finishes*. dx then runs its **own instrumented full-tree
  rebuild** (recompiling `bevy_render` + the rest under its jump-table linker — it does NOT
  reuse the normal `cargo build` artifacts). That cold instrumented build **did not finish**
  within the bounded window; the app **never reached running state under dx** (zero
  `HOTPATCH tick=` lines under dx). So the live `1→2` body-patch + World survival is
  **neither confirmed nor refuted empirically** — the binary never ran under dx. (Re-run on
  an interactive dev host with a ~10–15 min build budget; use `info!`/tracing not `println!`
  since dx demonstrably forwards tracing.)
- **Limits — source-confirmed.** **Native-only:** `connect_subsecond()` is
  `#[cfg(not(target_family="wasm"))]` (`bevy_app/src/hotpatch.rs`) → compiled out on wasm.
  Binary/tip-crate scope + body-only (no signature/layout change; layout change = a
  documented subsecond crash) are per the report, not re-tested live.
- **Net dim-4 verdict:** the feature **integrates cleanly with Buiy and the binary runs**;
  the live-patch demo is gated by dx's heavy cold instrumented build, so it belongs on an
  interactive dev host, not a bounded headless job. World-preservation for body edits stays
  source-confirmed (only the jump table swaps). A dev-loop convenience, native-only, not a
  shipped/web capability.

---

## Results matrix (the success criterion) — 4 of 5 dimensions final

The decisive variable across every dimension turned out to be **whether the reload
RESPAWNS entities**:

| Dim | Mechanism tested | State-preservation result (measured headless) | Reload verdict |
|---|---|---|---|
| **1. Asset (font)** | in-place reshape (`FontsGeneration` bump) | **ALL 13 SURVIVED** — same entities, text/caret intact | ✅ works — in-place, never respawns (the landed beachhead) |
| **2. Style/theme** | `Theme` resource mutate + `is_changed()` | **12/13 SURVIVED**, only `theme_token` changed | ✅ works — tree never rebuilt; state-preserving by construction |
| **3. Scene/`bsn`** | `apply_scene` re-apply | root identity kept, but **children DUPLICATE+LEAK** (2→5), **runtime state CLOBBERED** (99→0) | ⚠️ needs keyed reconciler + ownership split + upstream `.bsn` loader |
| **4. Rust code** | `bevy/hotpatching` (subsecond) via `dx serve --hot-patch` | feature + subsecond compile into a Buiy binary; **dx serves it** (H0✓); live patch World-preserving *by construction* (only fn-ptrs swap) but **not observable headless** (no stdout from a windowless dx-served binary) | ◻️ dev-only, native-only, binary/tip-crate-only — live demo needs an interactive dev host |
| **5. State-preservation** | naive respawn **vs** MVU `LogicalId`+`Reflect` restore | respawn = **everything LOST**; MVU restore = **model value SURVIVED** onto a new entity | ✅ **model-is-data + stable key is THE answer** |

**The one-sentence finding:** *in-place* reloads (asset, theme) preserve the entire live
state for free; *respawn* reloads (scene, full-tree, anything that re-spawns entities) destroy
all state UNLESS it lives in a `Reflect`-serializable model keyed by a stable id
(`LogicalId`) — which is precisely what Buiy's MVU subsystem already provides, and which the
keystone probe proved works by reusing the MVU replay path verbatim as the reload path.

### 2026-06-26 — SOLUTIONS wave: attacked every problem, built + ran the fixes

Grounded against the parallel **proto-3 MVU-as-core** design (in `mvu-core` worktree), which
already plans the state substrate: a `TextEditSnapshot` projection (§3 H5) + a `LogicalId`
resolver (§3 D7) — **both design-only today** (verified: not built in mvu-core). The editor
restore primitive `TextEditState::restore_cursor` ALREADY EXISTS (`input.rs:352-365`,
`set_cursor`+`set_selection`), just `pub(crate)`. Built 3 solution probes (`src/solutions.rs`),
each RUN + verified independently (exit 0):

- **S1 — keyed reconciler + ownership split → SOLVES P1a/P1b/P1c, current public API, no seam.**
  A `Key`-keyed reconciler (PATCH existing in place / SPAWN new / DESPAWN absent) with a strict
  description-vs-runtime ownership split: 3 specs → 3 live → **3 total** `Key` entities (NO dup,
  vs apply_scene's 2→5), dropped key **despawned** (no leak), surviving child kept its Entity id
  AND `RuntimeTicks=77` (NO clobber, vs apply_scene's 99→0) while its width patched 100→150.
  All three scene-reload losses fixed at once, in user code.
- **S2 (folded into S1)** — the ownership split (never author runtime-state components in the
  scene description) is what prevents the clobber; confirmed by `RuntimeTicks` surviving.
- **S3 — editor carve-out → SOLVES P2, current public API (single-line ASCII).** Carve
  `{text, caret, anchor, active}` to plain data; the respawned editor is empty (the R1 death,
  text→`""`); rebuild via the public `apply(Insert)` + `Motion` replay → `value()`/`caret()`/
  `mirror_selection()` all match exactly, **active end included**. Two sharp edges the replay
  has (a direct seam would not): you must replay **in the selection's own direction** (anchor
  first, then extend toward active), and `Motion` steps by **grapheme** while the snapshot stores
  **byte** indices → exact only for single-byte-per-grapheme (ASCII). **Production-robust fix =
  the minimal `pub fn snapshot()/restore()` seam wrapping the existing `restore_cursor`** (=
  proto-3 H5 `TextEditSnapshot`); not required for the ASCII case, so buiy_core left untouched.
- **S4 — focus remap by LogicalId → SOLVES P3, user code given a stable id.** With the editor
  carrying `LogicalId(42)`, after respawn `FocusedEntity` dangles (`is_live: true→false`, bits
  unchanged but naming a corpse). A `HashMap<u64,Entity>` remaps `FocusedEntity.0` old-lid→new
  entity → `is_live: false→true`, pointing at the new editor. This is the proto-3 D7 resolver
  (design-only) used as a focus resolver.

**Cross-cutting result: every state/scene problem the experiment found is SOLVABLE TODAY
without a `buiy_core` API change** (S1/S4 in user code; S3 on the public API for ASCII, with a
tiny optional seam for non-ASCII robustness). The solutions all key on **`LogicalId`** — so they
*are* the hot-reload consumers of the proto-3 MVU-as-core substrate, validating its hot-reload
claim with running code. The only remaining true BLOCKER is the upstream `.bsn` **text loader**
(P1d); the live code-patch (P4) and visual-repaint (P5) gaps are env/verification, not codebase.
