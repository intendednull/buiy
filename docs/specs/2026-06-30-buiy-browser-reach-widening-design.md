# Buiy browser-reach widening — design (WebGL2 + platform-service reach)

**Date:** 2026-06-30
**Status:** draft (prototype-validated — see § Provenance)
**Supersedes:** the deferred milestones in
[`2026-06-25-buiy-wasm-browser-support-design.md` § 2-deferred / § 6](2026-06-25-buiy-wasm-browser-support-design.md)
(the WebGL2 reach + the platform-service gaps D8/D9). That spec's v1 (WebGPU-only) is
shipped (PR #85); this spec graduates its deferred work into a concrete, prototype-validated
target for **widening** reach beyond flag-gated WebGPU.

> **Provenance — prototype-validated.** A throwaway prototype (`worktree-webgl2-reach-proto`,
> off `4ddeabf`) built + RAN Buiy on WebGL2 in a real browser. Headline: the full 5-screen
> gallery **renders correctly and is interactive on WebGL2 with zero shader errors** after a
> single band-attribute fold; WebGL2 adds a **small, cargo-deny-clean `Cargo.lock` delta (7
> backend crates)**. Full keep/refine/redesign:
> [prototype retrospective](../reports/2026-06-30-browser-reach-widening-prototype-retrospective.md).

## Purpose

Define the target for running Buiy in **any modern browser without experimental flags**, and
for the platform-service reach (touch, a11y, IME/soft-keyboard, cross-app clipboard) that makes
the web target genuinely usable. The mechanism: a **WebGL2 reach backend** (unflagged
everywhere) delivered alongside the WebGPU build via a feature-detect loader, plus the shared
and web-only service shims. WebGPU stays the preferred backend where available.

## 1. Why this widens support

WebGPU (~82% global, mid-2026) is default-on in Chrome/Edge/Safari-26/Android-12+ but
**default-OFF in Firefox on every OS** (`dom.webgpu.enabled`), flag-gated on some Linux Chrome,
and absent pre-Safari-26 / on older Android. Today Buiy ships **one WebGPU artifact** (both web
examples gate `bevy features=["webgpu"]` under `cfg(target_arch="wasm32")`; buiy sets no
`Backends`), so a visitor without an enabled WebGPU adapter gets nothing. **WebGL2 runs
unflagged in every modern browser** (`downlevel_webgl2_defaults` is the universal baseline). So
the flag requirement is a property of the single chosen backend, and a WebGL2 reach build
removes it.

## 2. Decisions

### D1 — Ship a WebGL2 reach build via a two-artifact `navigator.gpu` loader
**Decision.** Add a `webgl2`-feature wasm artifact beside the `webgpu` one; a JS loader
feature-detects a usable WebGPU adapter and loads WebGPU if present, else WebGL2. **Why.**
The two bevy meta-features can't coexist in one binary (`webgpu` wins) → reach = two artifacts.
Prototype: routing verified (gpu→webgpu, no-gpu→webgl2, both paint). **Rejected.** One dual
backend binary (not possible today); WebGL2-only (loses WebGPU's perf where available).

### D2 — Band pipeline ≤16 vertex attributes via ONE affine fold (KEEP from prototype)
**Decision.** Fold `affine_col0`+`affine_col1` (loc15+16) → one `affine: vec4` (loc15) in
`band.wgsl` + `band_vertex_buffers`; `BorderBandInstance` (already `affine:[f32;4]`) and the
192 B stride are UNCHANGED. **Why.** `band.wgsl` declares 17 attributes; WebGL2 caps at 16
(verified `maxVertexAttribs==16`), and the invalid band pipeline poisons the whole `buiy_pass`
→ blank screen. Behavior-identical native/WebGPU (band GPU test green) + WebGL2 renders.
**Rejected.** The spec's radii-to-UBO over-scope — one fold reaches 16. **Guard:** a `const`/
reftest assertion so a future 17th attribute re-breaks WebGL2 loudly.

### D3 — `Rgba16Float` fallback is capability-gated across BOTH sites + the float-linear extension
**Decision.** Detect `Rgba16Float` renderability (`RenderAdapter.get_texture_format_features`
→ RENDER_ATTACHMENT allowed + filterable) once at startup; thread the chosen `EffectTargetFormat`
through the effect-group compositor target (`compositor.rs:437`, pipelines `690/702/718`,
`BYTES_PER_TEXEL`) AND the backdrop-blur scratch (`blur.rs:73`) + their specialization keys;
fall back to `Rgba8Unorm` (accept banding) when float render targets are absent. Gate the blur's
Linear sampler on `OES_texture_float_linear` (a DISTINCT extension the old spec omitted); the
effect-group composite sampler is `Nearest` and unaffected. **Why.** Float-less WebGL2 (some
mobile) can't create an `Rgba16Float` RENDER_ATTACHMENT. Prototype: the happy path renders on
both float-capable adapters; the break isn't reproducible locally → the final force-tests the
Rgba8 path. **Rejected.** Unconditional format flip (breaks HDR/linear compositing where float
IS available — degrades the common desktop case).

### D4 — CI `web-smoke` gains a FULLY-enforced WebGL2 paint/shader gate
**Decision.** Add a WebGL2 leg (`tools/web-smoke/run-webgl2.mjs`) asserting a WebGL2 context +
zero GLSL-ES compile/link errors + non-blank canvas, using **software WebGL2 (SwiftShader)** —
enforced on the hosted runner. **Why.** Unlike v1's WebGPU smoke (software WebGPU absent on
hosted runners → SKIPS), software WebGL2 works headless, so the WebGL2 conformance gate is
CI-enforceable — a strict improvement. Ignore non-render 404s. **Rejected.** Best-effort-only
(wastes the SwiftShader capability WebGL2 uniquely has).

### D5 — Touch: shared-code Part A + Part B (REDESIGN from prototype) — **LANDED (W2)**
**Decision.** (A) `sync_pointer_location_on_button` in `PickingSystems::Backend` before
`emit_picks` — applies a Press/Release's own location to `PointerLocation`. (B) touch activation:
record the press target via a `Pointer<Press>` observer (`touch_press_records_target`), then
activate on the raw `Release` `PointerInput` gated on the CURRENT `HoverMap`
(`touch_tap_activates`, a system after `PickingSystems::Hover`); the `Click` path is
suppressed for `PointerId::Touch` so it never double-fires. **Why.** Running proved Part A alone
does NOT fix a browser cold-tap. **Implementation correction (W2, found by running the fix in a
browser + a console DIAG):** the prototype's plan to observe `Pointer<Press>`+`Pointer<Release>`
does NOT work — bevy_picking's `Pointer<Release>` (like `Pointer<Click>`) targets the PREVIOUS
frame's hover map (events.rs:656), which a first-touch tap never populates, so **`Pointer<Release>`
never fires for a cold tap** (only `Pointer<Press>`, which uses the current map, does). Hence Part
B reads the RAW `Release` `PointerInput` + the current `HoverMap` instead of `Pointer<Release>`.
Shared code (native mouse keeps the `Click` path, unaffected). **Verified:** cold touch-tap
navigates in a real WebGL2 browser; headless `cold_touch_tap_activates_widget_root` +
`touch_release_off_target_does_not_activate` guard it (`pointer_events_c3b.rs`); full headless +
crosscut suites green. **Rejected.** Forking bevy_picking; Part A only (insufficient — proven);
observing `Pointer<Release>` (never fires for a tap — proven by running).

### D6 — Cross-app clipboard: sync-facade + async-fill latch (web provider)
**Decision.** A wasm-only `WebClipboard` swapped in at `text/mod.rs:311-314`. Copy/cut stay
sync (fire-and-forget `navigator.clipboard.write_text`); paste = a sync `get_text()` facade
backed by an async `read_text()`-filled latch (`spawn_local`/`JsFuture`), first paste possibly
stale. **Why.** The `ClipboardProvider` trait is sync; `read_text()` is a Promise; the latch
keeps the trait unchanged. Browser: secure-context + transient-activation gated. **Rejected.**
Making the trait async (churns the whole native path for a web-only need).

### D7 — a11y sink: Buiy-owned hidden DOM/ARIA overlay (accesskit_web does not exist)
**Decision.** A wasm-only `WebA11ySink` at the `adapter.rs:52` seam mirroring each frame's
`A11yNodeView` tree into a visually-hidden, ARIA-annotated DOM subtree next to `#buiy`
(role→`role`, name→`aria-label`, toggled→`aria-checked`, expanded→`aria-expanded`, + an
`aria-live` region), reusing `build_tree_update`'s fold. **Why.** `accesskit_web` **does not
exist** (verified — roadmap "planned, lowest priority, funding-dependent"); the data half is
already web-ready. **Verify with a REAL screen reader** (not spec-only). XL. **Rejected.**
Waiting for upstream (indefinite); claiming the built tree = working a11y (silent WCAG failure).

### D8 — IME + mobile soft-keyboard: Buiy DOM bridge outside winit
**Decision.** A wasm-only bridge: a hidden focused `<input>` sibling of `#buiy` (egui TextAgent
pattern); on editor focus `.focus()` (raises the OSK) + move to `ime_position`; register
`compositionstart/update/end` + `input` → `MessageWriter<Ime>` feeding the unchanged E5 engine
(the OUTPUT seam `ime_enabled`/`ime_position` at `ime.rs:571,585`, INPUT seam
`MessageReader<Ime>` at `ime.rs:429`). **Why.** winit#4424 is OPEN with no timeline (winit web
emits no `Ime`); the engine is winit-free + reusable. EditContext is Chromium-only → the hidden
`<input>` is the cross-browser path. **Rejected.** Waiting on winit#4424; owning a full IME
engine (already exists).

### D9 — Web target stays purely additive
Every change is `cfg(target_arch="wasm32")`-gated or a new build target, EXCEPT the shared-code
D2 (band fold, behavior-identical) and D5 (touch, native-safe + test-guarded). Native targets/
behavior unchanged; `--workspace` enables no backend feature.

## 3. Phasing (→ the plan)

Render reach is proven and lands first; platform-service stages behind it, each independently
mergeable + gated:

1. **W1 Render reach** (D2 band fold + D1 feature plumbing + `build-web.sh` loader + D4 CI
   webgl2 leg). The flag-removal deliverable — **fully verified here** (renders + CI-gated on
   SwiftShader). **D3 (`Rgba16Float` capability gate) is deliberately NOT in W1:** the float-less
   break targets older mobile and is not reproducible on desktop/SwiftShader (both expose the
   float extensions), so shipping it in the "verified core" would ship unverified code. It moves
   to W2 (mobile hardening), where a float-less rig or a forced-fallback test can exercise it.
2. **W2 Mobile hardening** (D5 touch Part A + Part B + headless cold-tap/touch test; **D3**
   `Rgba16Float` capability gate + forced-Rgba8 test) — the mobile population W1 reaches.
3. **W3 Clipboard** (D6 WebClipboard) — verifiable in-browser.
4. **W4 a11y overlay** (D7) — XL, real-screen-reader-gated.
5. **W5 IME/soft-keyboard** (D8) — CJK-browser-gated.

## 4. Verification

Per foundation § 2.9 web is manual-release-gate; this spec makes the **WebGL2 render dimension
CI-enforced** (D4, the SwiftShader win). Native gate stays green with no adapter. Render reach +
touch + clipboard are headless/browser-verifiable here; a11y + IME require real AT / real IME
and are verified on a dev host before their wave merges. `wasm32-unknown-unknown` stays in
`deny.toml graph.targets` (WebGL2 adds no new crates, but audit the graph regardless).

## 5. References
- Prototype: [retrospective](../reports/2026-06-30-browser-reach-widening-prototype-retrospective.md)
  + `PROTOTYPE-JOURNAL.md`.
- Predecessor: [`2026-06-25-buiy-wasm-browser-support-design.md`](2026-06-25-buiy-wasm-browser-support-design.md)
  (v1 WebGPU, PR #85).
- Code anchors: `crates/buiy_core/src/render/{band.wgsl,primitive.rs,instance.rs,compositor.rs,blur.rs}`,
  `crates/buiy_core/src/picking/backend.rs`, `crates/buiy_core/src/text/{mod.rs,edit/clipboard.rs,edit/ime.rs}`,
  `crates/buiy_core/src/a11y/{translate.rs,adapter.rs}`, `examples/{buiy_web,gallery_web}`,
  `tools/web-smoke/`, `.github/workflows/ci.yml` (`web-smoke`).
