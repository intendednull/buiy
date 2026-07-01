# Browser-reach-widening prototype — retrospective (seeds the final)

**Date:** 2026-06-30
**Prototype worktree:** `worktree-webgl2-reach-proto`, off `origin/main` `4ddeabf`
(DO NOT MERGE the prototype code; this retrospective + the journal are the deliverable).
**Journal:** `PROTOTYPE-JOURNAL.md` (in the prototype worktree).
**Charter:** widen Buiy browser support beyond the flag-gated WebGPU-only v1 (PR #85);
"everything, one big push" (render reach + platform-service reach), prototype-first.

## Verdict

**The target is achievable and the core is proven by running it.** Buiy's full
5-screen gallery **renders correctly and is interactive on WebGL2** (real AMD RX 6700 XT
via ANGLE-on-Vulkan, headless Chrome 149), with **zero GLSL-ES shader errors** and only a
**small, cargo-deny-clean `Cargo.lock` delta (7 backend crates)** (corrected in the final —
see below). WebGL2 is the lever that removes the "enable experimental
flags" requirement (unflagged in every modern browser; WebGPU is still default-off in
Firefox all-OS, flag-gated on some Linux, absent pre-Safari-26 / older Android).

The single render blocker was exactly the one the survey predicted — the band pipeline's
17th vertex attribute — fixed by one attribute fold. Everything else (the "end-to-end
WebGL2 validation pending" risk in the WASM spec § 2-deferred) is now retired for the
non-effect-group path, and the effect-group happy path is validated too.

## Validated — KEEP (port as-is; re-derive the rationale in the final)

- **Band ≤16 repack via ONE affine fold.** Collapse `affine_col0`+`affine_col1`
  (loc15+16, two `Float32x2`) → one `affine: vec4` (loc15). The `BorderBandInstance.affine`
  field is already a contiguous `[f32;4]`, so the Rust struct is UNCHANGED (192 B stride
  preserved) — only `band.wgsl` (struct + `mat2x2(i.affine.xy, i.affine.zw)`) and the
  `VertexBufferLayout` change. **Rejected the spec's over-scope:** do NOT move per-corner
  radii to a UBO; one fold reaches 16. Verified: native band GPU test green + WebGL2 renders.
- **Feature-plumbed build axis.** `[features] webgpu / webgl2` on `buiy_web` + `gallery_web`,
  NEITHER default → native `--workspace` unaffected; backend chosen via `trunk --features`.
  `trunk 0.21.14` supports `--features`.
- **Two-artifact `navigator.gpu` loader.** Feature-detect a usable WebGPU adapter → load the
  WebGPU build, else the WebGL2 build. Routing verified (3 branches). Both bevy meta-features
  can't coexist in one binary, so reach = two artifacts + this JS switch.
- **Inherit-Bevy-bootstrap, embedded fonts, single-threaded, MemClipboard-on-wasm** — all
  still hold on WebGL2 (unchanged from v1).
- **Touch Part A** (`sync_pointer_location_on_button`) — a correct, necessary, non-regressing
  shared-code fix (29 picking tests green). Port it; it is half the touch fix.

## REFINE / REDESIGN (the final does these differently — full-picture reasons)

- **`Rgba16Float` fallback is capability-GATED, not format-flipped, and covers TWO sites +
  a distinct extension.** The effect-group compositor target (`compositor.rs:437` + pipelines
  `690/702/718`, `BYTES_PER_TEXEL` `199`) AND the backdrop-blur scratch (`blur.rs:73`) are
  both `Rgba16Float` RENDER_ATTACHMENTs; the blur sampler does Linear over it (`blur.rs`
  ~254) needing `OES_texture_float_linear` — a DISTINCT extension the spec omits. On the
  adapters testable here (AMD + SwiftShader) BOTH expose `EXT_color_buffer_float` +
  `OES_texture_float_linear`, so the happy path renders — but the float-less break (some
  mobile) is NOT reproducible here. **Final:** detect renderability via
  `RenderAdapter.get_texture_format_features(Rgba16Float)` (RENDER_ATTACHMENT allowed?) +
  the filterable flag, thread a chosen `EffectTargetFormat` through both sites + the pipeline
  specialization keys, fall back to `Rgba8Unorm` (accept banding) when absent. Force-test the
  Rgba8 path (the prototype could not naturally trigger it).
- **Touch needs Part A + Part B.** Running proved Part A alone does NOT fix a browser
  cold-tap (touchstart+touchend fall in the `PointerId::Touch` pointer's short lifespan;
  `Pointer<Click>` derives from the PREVIOUS frame's hover_map → never populated). **Final:**
  add Part B — a Buiy press-based activation emitting the widget action from
  `Pointer<Press>`+`Pointer<Release>` on the CURRENT hover map — and a headless cold-tap +
  `PointerId::Touch` Started/Ended test in `buiy_verify/src/pointer.rs` (blind today: every
  method settles a hover first). The WASM spec § 6 "one-frame hover lag" conflates this with
  the §3.3 transform-lag; the real click-miss is causes (2)+(3) in upstream bevy_picking.
- **Loader from a template, not hashed literals.** Generate the loader `index.html` +
  per-backend subdirs at build time (stable filenames), wired into trunk + CI.
- **CI `web-smoke` gains a WebGL2 leg that is FULLY enforced.** Unlike v1's WebGPU smoke
  (software WebGPU absent on hosted runners → shader/paint check SKIPS), software WebGL2
  (SwiftShader) works headless — so the WebGL2 paint/shader-conformance gate can be
  CI-ENFORCED. Reuse `tools/web-smoke/run-webgl2.mjs` (prototype); don't fail on non-render
  404s. This is a strict CI improvement the WebGL2 milestone buys.

## Framework/system findings surfaced by running

- **WebGL2 adds a small, cargo-deny-clean `Cargo.lock` delta — 7 backend crates**
  (`gl_generator`, `glow`, `glutin_wgl_sys`, `khronos-egl`, `khronos_api`, `wgpu-core-deps-wasm`,
  `xml-rs`). **Correction:** the prototype worktree reported "zero churn" because its lock was
  pre-polluted by an earlier webgl2 build; on a clean base (the final, off `e9d639c`) the delta
  is real. So WebGL2 DOES need a deliberate lock commit + `cargo deny` re-run — a smaller-but-real
  D6-style step (WebGPU's D6 delta was larger). `cargo deny check` is clean (advisories/bans/
  licenses/sources all ok).
- **`maxVertexAttribs == 16`** on both adapters — the band repack was strictly necessary and
  now sits exactly at the cap. Any future band attribute addition re-breaks WebGL2 → add a
  compile-time/`const` assert or a reftest guard.
- **Interactivity works on WebGL2** (picking hit-test → router → re-render), empirically —
  not assumed.

## Residual gaps for the final to close (from the P5–P8 research fleet)

- **Clipboard (P6, medium):** sync `ClipboardProvider`; swap at `text/mod.rs:311-314` (wasm
  arm → `WebClipboard`). Copy stays sync (fire-and-forget `write_text`); paste = sync-facade +
  async-fill latch (`read_text()`→Promise; first paste may be stale). Browser needs secure
  context + transient activation; bevy `Update` runs on rAF not inside the gesture handler.
  Verifiable in-browser.
- **a11y sink (P7, HIGH / XL):** `accesskit_web` **does not exist** (verified — roadmap
  "planned, lowest priority, funding-dependent"). Only actionable path = a Buiy-owned hidden
  DOM/ARIA overlay mirroring `A11yNodeView`→ARIA attrs at the `adapter.rs:52` seam; MUST be
  verified with a REAL screen reader (not spec-only). Decision locked: build the overlay, do
  not wait for upstream.
- **IME / soft-keyboard (P8, medium):** native IME engine is winit-free + reusable verbatim
  (`ime.rs` E5); seams = `MessageReader<Ime>` (input) + `ime_enabled`/`ime_position` (output).
  **winit#4424 OPEN, no timeline.** Actionable path = a Buiy DOM bridge OUTSIDE winit (hidden
  focused `<input>` sibling, egui TextAgent pattern; `compositionstart/update/end` →
  `MessageWriter<Ime>`; `.focus()` raises the OSK). EditContext Chromium-only. Buildable now.

## Build strategy for the final (hybrid port)

Cut a fresh `webgl2-reach-final` worktree off the SAME base `4ddeabf`. **Port the KEEP work**
(band repack, feature plumbing, loader, touch Part A — cherry-pickable, shared base) and
**implement the REFINE/REDESIGN cleanly** (Rgba16Float capability gate, touch Part B, CI
webgl2 leg, loader templating). Stage platform-service (clipboard → a11y overlay → IME) as
gated waves behind the proven render reach. Merge-gate on human review; do not self-merge.

## Verification status (what was actually RUN)

| Item | Verified how | Result |
|---|---|---|
| Band repack native-safe | `render` GPU test on AMD | ✅ green |
| WebGL2 renders (5 screens) | headless Chrome + screenshots | ✅ correct |
| WebGL2 interactivity | synthetic nav clicks | ✅ navigates |
| Effect groups / blur on WebGL2 | header BackdropFilter + opacity active, 0 errors | ✅ happy path |
| Float extensions | probe both adapters | ✅ all present (break not reproducible here) |
| Two-artifact loader routing | 3 branches | ✅ PASS |
| Touch cold-tap (RED) | browser `touchscreen.tap` | ✅ reproduced |
| Touch Part A | rebuild + retest | ⚠️ necessary, NOT sufficient → Part B required |
| Lock delta | `Cargo.lock` diff + `cargo deny` | ✅ 7 backend crates, audit-clean (proto's "zero" was lock pollution) |
| Platform-service P6–P8 | research fleet (code + upstream) | seams + status resolved (not built) |
