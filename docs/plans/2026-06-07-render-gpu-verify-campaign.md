# Render GPU-Verification Campaign

> **For agentic workers:** this is a *campaign* plan, not a bite-sized TDD plan.
> The bug-fix work is exploratory — the exact bugs are discovered by running the
> GPU suite, then each fix is driven TDD-style as it surfaces. Phases are run as
> sequential Workflows; the orchestrator stays in the loop between them.

**Goal:** Verify and fix the render-pipeline GPU path (R6–R11) on real hardware,
then build the GPU orchestration that was deferred while the host was believed to
have no adapter — all with runtime verification, and add a GPU test lane to the
gate.

**Architecture:** The render pipeline (R1–R11) is merged to `main`
([[buiy-render-pipeline-progress]]). Every GPU-runtime assertion was parked behind
`#[ignore]` on the premise that this host + CI have *no wgpu adapter*. That premise
is **false** — the host has an AMD Radeon RX 6700 XT (RADV/Vulkan), and Vulkan
render-to-texture needs no X server. So the ~42 ignored tests can run here and the
GPU code can finally be verified.

**Branch:** `render-gpu-verify` (off `origin/main` @ 5148686).

**Gate invariant:** the headless `cargo test --workspace` gate (NO `--ignored`)
must stay green throughout. The GPU lane is **additive** — `cargo test -- --ignored`.

---

## The finding (2026-06-07)

`vulkaninfo` confirms a real Vulkan adapter. The `#[ignore = "needs a wgpu adapter
(real GPU or lavapipe)"]` reason on all 42 tests was a never-falsified assumption:
the headless gate runs without `--ignored`, so it never instantiated the adapter,
so the GPU path's bugs stayed hidden. Two were already found + fixed (below).

## GPU test inventory (42 `#[ignore]`, all `buiy_core`)

| File | # | Drives a frame? |
|------|---|-----------------|
| `tests/render_smoke.rs` | 13 | mixed — some inspect resources after build, some `update()` |
| `tests/render_compositor_gpu.rs` | 7 | yes |
| `tests/render_prepare.rs` | 5 | yes |
| `tests/atlas_gpu.rs` | 5 | yes |
| `tests/render_specialize_gpu.rs` | 2 | resource inspect |
| `tests/render_golden_harness.rs` | 2 | yes (readback) |
| `tests/atlas_register.rs` | 1 ignored (+1 headless) | resource inspect |
| `tests/render_shader_wgsl.rs` | 1 | naga/device validate |
| `tests/render_primitive_dedup.rs` | 1 | resource inspect |
| `src/render/compositor.rs` | 2 | unit (in-crate) |
| `src/render/prepare.rs` | 1 | unit (in-crate) |
| `src/render/golden.rs` | 1 | unit (in-crate) |

Run: `cargo test -p buiy_core --test <name> -- --ignored --nocapture` (single
binary, `--test-threads=1` safe). `cargo test --workspace` link-OOMs under full
mold parallelism → use `-j 2`.

## Bugs already found + fixed (uncommitted on `render-gpu-verify`)

- **BUG #1 — shaders loaded into the wrong world.** `pipeline::register` inserted
  the WGSL into the *render* world's `Assets<Shader>`, which does not exist there
  (`AssetPlugin` owns it in the *main* world; the render world gets only the
  extracted GPU mirror). Fix: `load_internal_asset!` the two shaders into the main
  world in `BuiyRenderPlugin::build`, guarded by RenderApp + `Assets<Shader>`
  presence so the headless gate is unaffected.
- **BUG #2 — device-dependent registration ran in `build`.** `pipeline::register`
  needs `RenderDevice`/`PipelineCache`, which `RenderPlugin` only inserts during
  *its* `finish` (async `initialize_renderer`). Fix: split the plugin — device-free
  work stays in `build`; `pipeline::register` moved to a new
  `BuiyRenderPlugin::finish`.

Verified: `cargo build -p buiy_core` clean; the pipeline now *creates* on the GPU
(the `pipeline.rs` panic is gone); headless render tests still green.

---

## Phases

### Phase 1 — Canonical headless-GPU harness (keystone)

The blocker: a full render frame (`app.update()` after `finish`+`cleanup`) panics
with a system-param validation error — "Parameter `…messages` failed validation:
Message not initialized" (Bevy 0.18 renamed Events→Messages). Some system's
`Messages<T>` is not registered by a minimal probe plugin set.

**Deliverable:** one reusable test-support helper that constructs the *minimal
complete* plugin set to drive a full Buiy render frame headless on this GPU, with
a smoke assertion that passes under `--ignored`, plus the root cause of the Message
panic. Approach: a parallel 3-angle panel (minimal-render-set + `add_message`;
Bevy headless-renderer example pattern; `DefaultPlugins` minus winit/window), each
verified by a real `cargo test --ignored` run; pick the winner.

### Phase 2 — Fix + run the 42-test suite

Apply the canonical harness across all ignored tests (most need only `.finish()`
added — they inspect render-world resources that now materialize in `finish`).
Run each file under `--ignored`, collect the real failures (distinguishing
broken-harness from real-GPU-bug), and queue the genuine bugs.

### Phase 3 — Root-cause + fix the real GPU bugs (R6–R11)

For each genuine failure: debugging skill → minimal repro → root-cause → TDD fix.
Likely surfaces: bind-group correctness, the actual draw, tonemapping-LUT setup
for the Core2d node test, atlas upload/sampling, specialize variants.

### Phase 4 — Build the deferred GPU orchestration (verified)

The four render GPU deferrals tracked in `follow-ups.md`:
1. **Effect-compositor GPU orchestration** — fill `prepare_effect_groups`; add the
   extract→prepare effect-group data flow (R5's flat extract carries no group
   membership); wire the `BuiyNode::run` composite loops; exclude group-member
   ranges from the flat draw.
2. **Subtree-visibility suppression** render-prep pass (design note
   `2026-06-06-render-subtree-visibility-suppression-design.md`).
3. **Atlas** `AtlasPage.texture` upload + sampling.
4. **R11 §3.3** forced-colors `BoxShadow` draw-skip in extract.

Each lands with a GPU `#[ignore]`-lane golden/readback test that now actually runs.

### Phase 5 — Add the GPU lane to the gate

Document + script a `cargo test --workspace -- --ignored` (or per-crate `--ignored`)
lane alongside the headless gate, so GPU regressions are caught going forward.
Update `CLAUDE.md` Build & Test and the campaign memory.

---

## Status

- [x] Finding confirmed; campaign chartered (user-approved "Full GPU campaign").
- [x] BUG #1 + #2 fixed + verified. Committed `f79635b`.
- [x] **Phase 1 — canonical harness.** `tests/support/mod.rs::gpu_test_app` + the
  keystone smoke `tests/render_gpu_harness.rs`. The "Message not initialized"
  panic was a pure Bevy-stack gap (not a Buiy bug); fixed by adding the correct
  owning plugins (WindowPlugin, CameraPlugin, ThemePlugin) + `init_asset::<Mesh>()`.
  Committed `f79635b`.
- [x] **Phase 2 — triage + fix the suite.** The "42" was inflated by doc-comment
  mentions; **22 real ignored tests**. Result: **0 real bugs** — 11 broken
  harnesses fixed (missing `finish()`/`ImagePlugin`, dead `System::name()`
  membership idiom → count-delta, stale `BUIY_EXTRACT_SYSTEM_COUNT` 2→3), 1
  pass-already, 10 empty stubs deferred to Phase 4. GPU lane 25 pass / 0 fail;
  headless gate 656 pass / 0 fail / 26 ignored. Committed `f21bb6d`.
- [x] **Phase 3 — real GPU bugs: NONE.** Triage found zero. The only real
  production bugs were #1 + #2 (Phase 1). The production GPU path (pipeline
  registration, graph topology, extract/prepare wiring, specialization cache,
  atlas resources) is correct. Phase 3 closes empty.
- [ ] **Phase 4 — deferred orchestration** (the 10 empty stubs = the 4
  follow-ups.md render deferrals). Build order by dependency:
  1. Buffer-upload round-trip (prepare → `BuiyInstanceBuffers` for a real node):
     `prepare_uploads_persistent_buffers`, `node_draws_persistent_buffers_with_view_uniform`.
  2. Golden capture/readback harness (render-to-texture + readback + `perceptual_diff`)
     — the infra ALL pixel-asserting stubs reuse: `overlapping_semitransparent_fills_match_golden`.
  3. Node actually paints + top-layer composite order: `top_layer_composites_last_over_in_flow`.
  4. Atlas upload + sampling: `warmed_glyph_uploads_and_samples_with_tint`,
     `retint_same_glyph_leaves_atlas_byte_identical`, `warmup_makes_first_frame_match_golden`,
     `gate15_atlas_entries_return_to_baseline_after_idle`.
  5. Effect-compositor GPU orchestration (extract→prepare effect-group dataflow +
     off-screen RT composite): `group_opacity_overlap_is_single_layer_at_half`,
     `rt_pool_returns_to_baseline_after_idle`.
  Also the two non-test deferrals: subtree-visibility suppression pass; R11 §3.3
  BoxShadow forced-colors draw-skip. Each lands with its now-real GPU `#[ignore]` test.
- [ ] Phase 5 — GPU gate lane.
