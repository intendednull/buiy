# Verification follow-ups campaign — implementation plan

**Date:** 2026-07-01
**Realizes:** [`docs/specs/2026-07-01-verification-followups-campaign-design.md`](../specs/2026-07-01-verification-followups-campaign-design.md)
**Branch/worktree:** `worktree-verify-followups-campaign` (off `origin/main` `f37c6fa`)

## Execution model (and why)

Slices are **sequenced within one worktree, not fanned out in parallel for
implementation.** Reason: the BUILD slices share one `cargo` target dir and several
touch adjacent files (`buiy_core/tests/render/*`, `follow-ups.md`), so concurrent editing
+ concurrent `cargo test` would race. Parallel fan-out (under `reliable-agent-fleet`) is
reserved for the **review gates** (read-only, safe to parallelize) after each wave.
`follow-ups.md` + `docs/README.md` edits are **consolidated into one tracker slice per
wave** (S5 / G5) rather than scattered across code slices, to avoid same-file conflicts
and to let the tracker reflect what actually landed.

Gate discipline: after each wave, fresh-context reviewers verify **by running**, not just
reading. Commit per verified slice. Push/PR only at wave end, and **stop for explicit go
before merge**.

Baseline gate commands (from CLAUDE.md):
- **Headless (W1):** `cargo fmt --all -- --check` · `cargo clippy --workspace --all-targets --locked -- -D warnings` · `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` · `cargo test --workspace --locked` (or nextest).
- **GPU (W2):** `cargo test -p buiy_core -j 2 -- --ignored --test-threads=1` **and** `cargo test -p buiy_verify -j 2 -- --ignored --test-threads=1` (both legs).

---

## Wave 1 — headless hardening + tracker truth (PR #1)

### S1 — text-capable `build_app` (V14 + V13, one coherent change)

**Files:** `crates/buiy_verify/src/coverage/enroll.rs`, `crates/buiy_verify/fixtures/button/resting.rs`,
`crates/buiy_verify/tests/verify_headless/content_presence.rs`, the 12± `button.resting.*.snap`.

1. **Make `build_app` text-capable.** Add `BuiyTextPlugin { system_fonts: false }` (that is
   the *whole* literal — `system_fonts` is the only field) to the `build_app` stack. **That is
   the entire change** — no Ahem staging, no fixture edit. The text plugin seeds
   `SharedFontSystem` (which the Taffy text-measure requires), and `system_fonts:false` keeps
   host fonts out so the metrics come only from the embedded default font (Fira Sans). Pure-CPU/
   no-adapter invariant holds: `BuiyTextPlugin`'s render half is guarded on `RenderApp` (absent
   under `MinimalPlugins`) and its asset-loader reg on `AssetServer` (also absent).
   *(Superseded during execution: an earlier draft of this step staged Ahem via
   `determinism::stage_ahem`; the W1 code review proved it inert for this fixture — Fira Sans
   wins resolution — and removed it. Ahem's em-box only matters for host-stable rasterized
   pixels, which the CPU metric tiers don't produce. See the spec's V14 deviation note.)*
2. **Point the fixture label at Ahem** in `fixtures/button/resting.rs` so it resolves
   deterministically; update the fixture docstring (drop the "content-width-empty" note).
3. **Re-bless** the button structured snapshots. **Do not pre-trust "12"** — run the
   headless snapshot tests, review each `.snap` diff (label `0×0` → measured; button may
   re-center its child; assert *no other field moves*), accept via `cargo insta` /
   `INSTA_UPDATE` per house practice, and record the actual count.
4. **V13 driver:** add `enroll_content_presence` + a catalog-wide test in
   `content_presence.rs` iterating `sorted_catalog() × Matrix::cpu_snapshots`, `update`
   once, asserting `content_is_present` for every `scene_is_text_bearing` cell. It runs on
   the *same* text-capable `build_app` — no separate stack.
5. **Non-vacuity proof (RED-first):** temporarily blank the button label → the V13 check
   must RED; restore → GREEN. Keep the control assertion that makes vacuous-pass impossible.

**Blast-radius check:** run *all* button-enrolling tiers — `coverage_layout`,
`coverage_display_list`, `coverage_dpr_invariance`, `coverage_invariants`,
`snapshot_*`. Only the snapshot tiers may rebless; the property/invariant tiers must stay
green unchanged. If a property tier breaks, stop and root-cause (do not rebless a property).

**Verify:** full headless gate green; the reblessed diffs reviewed; V13 RED-first proven.

### S2 — SDF oracle DRY (V10)

**Files:** new `pub fn sdf_rounded_rect` in `crates/buiy_core/src/render/` (co-locate with
the SDF/shader module; confirm the right home), `crates/buiy_verify/src/reftest.rs`,
`crates/buiy_core/tests/render/render_instance.rs`, `crates/buiy_core/tests/render/render_border_sdf.rs`.

1. Hoist one canonical Rust `sdf_rounded_rect` into `buiy_core::render` (pub, documented as
   the CPU twin of `shader.wgsl`'s `sdf_rounded_rect`).
2. Replace the three duplicate Rust copies with imports; delete the local defs.
3. **Do NOT** add a WGSL-text-twin guard test (spec: net-negative). Leave the CI lavapipe
   cross-check as the real Rust↔WGSL pin.

**Verify:** `cargo test -p buiy_core` + `cargo test -p buiy_verify` headless green; the SDF
reftest/oracle numbers unchanged (pure refactor — identical values).

### S3 — render-test hygiene (H1)

**Files:** `crates/buiy_core/tests/render/render_smoke.rs`, the three `.snap` with stale
`source: tests/render_instance.rs` headers.

1. **Stride is 68** (confirmed: `render_instance.rs:115` `packed_instance_stride_is_68`,
   "17 f32 = 68 B"; `instance.rs:31`; `shadow.wgsl:3`). Rename the fn
   `clip_aabb_pipeline_registers_with_stride_52` → `..._stride_68` and fix **all ~8 stale
   "52" mentions** in `render_smoke.rs` (~lines 253, 256, 263, 273, 420, 425, 443, 447 — the
   fn name, comments, and two `.expect()` strings), not just the fn name.
2. Fix the stale insta `source:` headers to the post-consolidation path (coordinate with S2
   if S2 moved `render_instance.rs`'s SDF fn — the header path itself is just the test file
   path; re-point only if stale).

**Verify:** `cargo test -p buiy_core` green; the renamed test still asserts the true stride.

### S4 — V17 tracking-gap doc (part 1 only)

**Files:** `docs/plans/follow-ups.md`.

Record *time-boxed-ignore* + *flaky-auto-ignore* in the deferred-golden-primitives list
(closes the Task-4.7 tracking gap). **Machinery stays deferred.** (Folded into S5's single
`follow-ups.md` edit to avoid a same-file conflict.)

### S5 — tracker truth (consolidated `follow-ups.md` + `docs/README.md`)

**Files:** `docs/plans/follow-ups.md`, `docs/README.md`.

One agent/pass owns all W1 doc-truth edits, after S1–S4 land:
- **PRUNE (mark resolved w/ evidence + commit refs):** V8 (`4c1acbb`), V9 (`52d9194`/`ae42b96`),
  V11 (byte-current, EXACT on lavapipe), V15 (`4c1acbb` #13). V12 kept as-is (tidy the stale
  bless command on its line; forward-watch note stays).
- **DEFER (dated re-verify note + gradation):** V2, V3, V18 (no consumer — re-verified
  2026-07-01); V16 → point at the live C8 entry (~L1644), not a bare "deferred."
- **CONFIRM-BLOCKED (refresh note w/ dated evidence):** V5 (color-glyph leg; soften the
  IconInstance wording per the gate), V7 (590 B ≪ trigger), V20 (HashMap tokens), V21
  (single global LayoutTree).
- **CLOSE the BUILT items' notes:** V14, V13, V10, H1, V17 → mark done/landed in this PR.
- **`docs/README.md`:** add index entries for the campaign spec + this plan (sibling to the
  `2026-06-15-buiy-verification-design` entry).

**Verify:** `cargo doc` (doc links), and a read-back that no follow-up note now contradicts
the code state.

### W1 gate + PR

Fresh-context fan-out (≥2 reviewers): (a) correctness/verification — re-run the headless
gate, confirm V13 non-vacuity, confirm S2 is behavior-preserving, confirm the rebless diffs
are label-only; (b) tracker-truth — every follow-up note now matches reality. Fix findings.
Open **PR #1**, wait for green CI, **stop and ask before merge**.

---

## Wave 2 — GPU verification (PR #2)

Order matters: **G1 (V19) first** — it fixes a possibly-live-panicking RX test, so the GPU
lane must be green before goldens are added.

### G1 — adapter-robust ink detection (V19)

**Files:** `crates/buiy_core/tests/text_edit/text_caret_selection_e3_gpu.rs`,
`text_selection_caret_gpu.rs`, `text_ime_preedit_gpu.rs`.

**Scope (plan-gate B1):** the fix is **all five** absolute-`>=180`-family predicates across
the three files, not just `is_white_ink`:
- `text_caret_selection_e3_gpu.rs:70` `is_white_ink` (`p>=180` all ch)
- `text_selection_caret_gpu.rs:78` `is_white_ink` **and** `:72` `is_blue_ink` (`p[2]>=180 && p[0]<=150`)
- `text_ime_preedit_gpu.rs:51` `is_white_ink` **and** `:46` colored-ink (`p[2]>=180 && p[0]<=80 && p[1]<=140`)

Fixing only `is_white_ink` leaves the selection-band + preedit-color asserts equally
RADV-brittle. And a *scalar* "brighter than background" helper is wrong — `text_selection_caret_gpu`
asserts blue-selection vs. white-ink **separately**, so the helper must preserve per-channel
discrimination.

1. **Empirically confirm first:** run the three tests on the RX
   (`cargo test -p buiy_core -j 2 -- --ignored --test-threads=1 <names>`). Record which of the
   five predicates actually empty their `cols_where()` → `.expect`/`assert!` fail on RADV.
   Decides framing (live-bug vs. portability) but not the fix.
2. Replace all five with a shared **channel-parametric** helper — "channel *c* meaningfully
   above the black background" — so `is_white_ink` = all three channels up, `is_blue_ink` =
   blue up **and** red not up, preedit-color = its own channel signature, each defined
   *relative to the measured background* rather than an absolute 180. Preserve every test's
   semantic geometric assertion (caret right of ink; blue selection band distinct from white
   ink; preedit ink present). Push back on the note's "route through `buiy_verify::golden`"
   framing (a whole-frame golden over-captures for geometry).
3. Re-verify green on the RX; sanity-check the helper is adapter-robust (holds at lavapipe's
   dimmer coverage *and* RADV's).

**Verify:** all three GPU tests pass on the RX; semantic asserts intact.

### G2 — forced-colors BoxShadow visual reftest (V1)

**Files:** `crates/buiy_verify/tests/verify_gpu/coverage_forced_colors.rs` (replace the
`boxshadow_visual_reftest_is_blocked` placeholder), `reftest_cases_gpu.rs` (pattern),
`crates/buiy_verify/src/coverage/forced_colors.rs` (stale "BLOCKED" doc block).

Real Tier-4 reftest: `test` = bordered box + BoxShadow under `prefs.forced_colors=true`;
`reference` = same box, no shadow; `reftest!(match, forced_colors_boxshadow_suppressed, …)`.
Adapter-agnostic → verify on the RX (no lavapipe needed).

**Verify:** the reftest passes on the RX; confirm it *can fail* (temporarily disable the
forced-colors suppression → reftest REDs), then restore.

### G3 — shadow-blur-kernel golden (V4)  ·  needs pinned lavapipe to bless

**Files:** `crates/buiy_verify/tests/verify_gpu/goldens.rs`, new
`crates/buiy_verify/tests/goldens/shadow/…` (blessed PNG + `.toml`).

1. Add a `box_shadow` fixture + `golden_shadow_blur_kernel` modeled on `golden_sdf_corner`
   (deterministic capture; **non-vacuous paint check on every adapter**; adapter-gated
   `assert_golden` under `on_pinned_lavapipe()`).
2. **Self-validate on the RX:** the non-vacuous paint check + double-capture stability run
   on RADV (the `assert_golden` skip-as-pends off lavapipe — expected).
3. **Bless on pinned lavapipe** (Mesa 24.3.4, per `.github/actions/install-mesa`): set up the
   pinned lavapipe ICD locally, `BUIY_BLESS=1` the run, review the Gaussian falloff, commit
   the `.png` + `.toml`. **If pinned lavapipe cannot be reconstructed here → this is the
   human/CI-attention point** (see §risks); land the code, flag the bless for the CI GPU host.

**Verify:** self-check green on RX; blessed cell compares EXACT on lavapipe.

### G4 — bless forced-colors-safe residue cells (V6)  ·  needs pinned lavapipe to bless

**Files:** `crates/buiy_verify/tests/verify_gpu/coverage_golden.rs`, `tests/goldens/button/resting/…`.

1. Make `matrix_goldens` honor `fx.snapshots_cell(&cell)` — skip un-paintable cells in
   **both** the assert and the `BUIY_BLESS` capture paths (so blessing can't bake the magenta
   sentinel). Mirror the CPU-tier skip.
2. **Bless the 12 `theme==ForcedColors` button cells on pinned lavapipe**; commit. The 12
   light-theme cells stay skip-as-pending (widget-catalog owns making the default button
   forced-colors-safe).
3. Confirm `matrix_goldens` flips `asserted==0` → `asserted>0` (12 compared) on lavapipe.

**Verify:** non-vacuity guard now sees `asserted>0`; the 12 cells compare EXACT on lavapipe.
**RX-observability (plan-gate A3):** there are currently **zero** committed button golden
PNGs, so G4's headline (`asserted 0→>0`) is **unobservable on the RX** — only the
`snapshots_cell`-honoring skip change (which cells iterate) is RX-verifiable here. G4's
real outcome is lavapipe-gated. Contrast G3, whose `golden_shadow_blur_kernel` genuinely
self-validates on RX (non-vacuous paint check + double-capture). So the lavapipe-attention
risk bites hardest on **G4** (and the blessing half of G3).

### G5 — W2 tracker truth

Flip V19, V1, V4, V6 notes to landed (with the empirical V19 finding recorded); note V4/V6
blessing provenance (which adapter blessed).

### W2 gate + PR

Fresh-context fan-out: verify both GPU legs pass on the RX (and V4/V6 on lavapipe if
blessed here); confirm each new test *can fail*; confirm no headless regression. Open
**PR #2**, wait for green CI (incl. the CI GPU lavapipe lane), **stop and ask before merge.**

---

## Risks & the one human/CI-attention point

- **Pinned-lavapipe blessing (G3/G4).** This host is RADV, not lavapipe; CI goldens are
  lavapipe-tagged. If a Mesa-24.3.4 lavapipe ICD can be reconstructed locally, bless here
  (as the widget-catalog campaign did). If not, V4/V6 **code + RX self-validation** land, but
  the blessed-PNG commit must be produced on the CI GPU host — I will flag this explicitly
  rather than commit an RX-tagged golden that would silently adapter-skip in CI. V19 + V1 are
  unaffected (no blessing).
- **V14 rebless portability.** If Ahem is not forced sole-family / `system_fonts` not off,
  the reblessed CPU snapshots become host-dependent and fail CI. S1 step 1 pins this.
- **V13 vacuity.** Guarded by the RED-first proof (S1 step 5).
