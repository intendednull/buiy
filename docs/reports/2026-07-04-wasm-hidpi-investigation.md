# HiDPI / web scale-factor overflow (Dooduel F9) — root cause: a headless-emulation artifact, not a real-device bug

Date: 2026-07-04
Status: report (one-shot investigation)
Realizes: the [Dooduel FINAL design](../specs/2026-07-03-dooduel-final-design.md) §2.10 (F9) + the [execution plan](../plans/2026-07-03-dooduel-final.md) wave F9.
Reproduces + resolves: the prototype's top mobile residual, journaled in [PROTO1 journal](../prototypes/2026-07-02-dooduel-PROTO1-journal.md) W7 ("framework-bug — HiDPI web scaling").
Evidence: [assets](2026-07-04-wasm-hidpi-investigation-assets/) (two screenshots). Reproduction tooling committed at `tools/web-smoke/hidpi-check.mjs`; the headless CI proxy at `crates/buiy_core/tests/layout/layout_hidpi.rs`.

## The reported bug

From the prototype's W7 (the first time the app was driven in a real browser): on
wasm at `devicePixelRatio > 1` (every real phone / retina display), the whole UI
rendered "~dpr× too large and overflowed" — correct at dsf=1, ~2× too large at
dsf=2, ~3× at dsf=3 (card + content spilling past the viewport, top/right
clipped). The journal classified it a `framework-bug` needing "a focused
`buiy_core` HiDPI/web scale-factor investigation," and the FINAL plan slotted F9
into the serial `buiy_core` render line on the assumption it edits the
scale-factor seam.

## Verdict (TL;DR)

**There is no dpr bug in Buiy, bevy, or winit's real-device path — and F9 must
NOT edit the `buiy_core` scale-factor seam.** The seam is correct.

The "2× too large" is a **headless-Chromium dpr-emulation artifact.** It appears
**only** when the two device-pixel-ratio signals the web stack reads are made
*inconsistent* — which Chromium's single-knob emulation does, but a real device
never does. When both signals agree (as they always do on a physical HiDPI
screen), Buiy renders at the correct logical size, crisp. Reproduced across dsf
1/2/3, both directions of the artifact, and the correct consistent case.

What ships instead of a render change:

1. A headless CI proxy (`layout_hidpi.rs`) proving Buiy's layout is
   **scale-factor-invariant** — the regression guard the spec (§2.10, finding M4)
   asked for.
2. A **faithful HiDPI web-verification harness** (`tools/web-smoke/hidpi-check.mjs`)
   that emulates a real device (both dpr signals set consistently) and asserts the
   sizing invariants — so the acceptance gate can't reproduce the false artifact.
3. This report, documenting the mechanism and the corrected verification recipe.

## The scale-factor code path (the chain, verified `file:line`)

Buiy defines all layout + geometry in **logical pixels**; device scale factor
enters once, at the render boundary. The chain from the browser to a pixel:

1. **winit** (web backend, `winit-0.30.13/src/platform_impl/web`):
   - `scale_factor = window.devicePixelRatio` — `web_sys/mod.rs:52`.
   - physical `inner_size` = the **`DevicePixelContentBox`** ResizeObserver's
     reported size (true device pixels) — `web_sys/resize_scaling.rs:143,239`
     (falls back to `content-box logical × scale_factor`, `resize_scaling.rs:190`,
     when the browser lacks device-pixel-content-box — also dpr-consistent).
2. **bevy_winit** (`bevy_winit-0.19.0/src/system.rs:87`):
   `resolution.set_scale_factor_and_apply_to_physical_size(winit_window.scale_factor())`
   — bevy's `WindowResolution` stores winit's physical size + scale_factor, and
   `resolution.size()` returns **logical** = physical / scale_factor. (On web,
   `fit_canvas_to_parent` only sets the canvas CSS `width/height: 100%`,
   `system.rs:103` — the backing store + scale are winit's.)
3. **Buiy layout** reads `primary_window.resolution.width()/height()` (logical) as
   the root available space — `crates/buiy_core/src/layout/systems.rs:900, 2047,
   2581, 3394, 4106`.
4. **Buiy render extract** fills the per-view `logical_size =
   resolution.size()` + `scale_factor = resolution.scale_factor()` —
   `crates/buiy_core/src/render/extract.rs:1705-1706`.
5. **Buiy view uniform** builds the logical→clip affine from `logical_size`
   (`BuiyViewUniform::for_view`, `crates/buiy_core/src/render/view_uniform.rs:60`);
   the vertex stage maps logical-px positions across the physical framebuffer.
   `scale_factor` is carried only for SDF / corner-radius math, never to rescale
   geometry.

Every step is in logical px and internally consistent. The single load-bearing
identity is at step 2: **`logical = physical / scale_factor`.**

## Root cause: inconsistent dpr signals under emulation

`logical = physical / scale_factor` is correct **iff** the two inputs come from
the same physical dpr:

- `scale_factor` ← `window.devicePixelRatio` (winit `mod.rs:52`)
- `physical` ← the compositor's device-pixel-content-box (winit `resize_scaling.rs`)

On a **real device** these are two views of one number, so
`physical / scale_factor = CSS size` exactly, and layout/render get the true
logical viewport. **No bug.**

Chromium's headless dpr *emulation* drives the two knobs **independently**:

- `--force-device-scale-factor=N` (process) scales the *compositor / content-box*
  by N but leaves `window.devicePixelRatio` at 1.
- Playwright/CDP per-context `deviceScaleFactor=N` sets `window.devicePixelRatio`
  to N for JS but does **not** supersample the device-pixel-content-box in
  headless SwiftShader.

Set only one and the identity breaks in opposite directions:

| Run | signals set | JS `devicePixelRatio` | canvas backing (px) | canvas CSS (px) | derived logical = backing / dpr | on-screen result |
|-----|-------------|-----------------------|---------------------|-----------------|---------------------------------|------------------|
| baseline | none | 1 | 390×844 | 390×844 | **390** = CSS | correct |
| context `deviceScaleFactor=2` only | JS dpr only | 2 | 390×844 | 390×844 | **195** = ½ CSS | **2× too LARGE** (overflow, top/right clipped) |
| process `--force-device-scale-factor=2` only | content-box only | 1 | 780×1688 | 390×844 | **780** = 2× CSS | 2× too small |
| **BOTH = 2** (a real device) | both | 2 | 780×1688 | 390×844 | **390** = CSS | **correct, crisp** |
| **BOTH = 3** | both | 3 | 1236×2745 | 412×915 | **412** = CSS | **correct, crisp** |

The render is correct **iff `backing / devicePixelRatio == CSS size`** — i.e. iff
the two dpr signals agree. The prototype's "2× too large" is exactly row 2: JS
`devicePixelRatio=2` fed winit's `scale_factor=2` while the un-supersampled
content-box fed `physical=390`, so winit computed `logical = 390/2 = 195` and the
app laid out for a 195-px viewport that was then mapped across the full 390-CSS
display — every fixed-size element 2× oversized. Row 4 (both signals = 2, the
real-device case) renders identically to the dsf=1 baseline, just crisper (780
backing). See the two screenshots in
[assets](2026-07-04-wasm-hidpi-investigation-assets/): `artifact-context-dsf2.png`
(row 2, the false 2×) vs `correct-consistent-dsf2.png` (row 4, right size).

The artifact is **backend-agnostic** (it lives in the winit/DOM canvas-sizing
layer, above wgpu) — reproduced on WebGL2; the WebGPU backend shares the same
canvas/window path.

## Why the prototype (and the journal) saw it

The prototype's mobile verification used Chromium context-`deviceScaleFactor`
emulation with "a fresh browser context per size" — i.e. row 2 — and never tested
a physical HiDPI device (the journal hedges "This **WOULD** affect a real phone").
It reproduced the same emulation artifact this investigation did, and reasonably
(but incorrectly) attributed it to a `buiy_core` scale-factor bug. The one detail
that doesn't fit a real bug — the journal's note that `is_mobile`/`card_w` "read
the logical `window.width()` (390/412)" — is itself a symptom of the artifact: on
a real device `window.width()` would read the true logical 390, but the layout
and the view uniform both read the *same* `resolution.size()`, so any correct read
of 390 forces a correct render. A render that is 2× off while the app reads 390 is
only possible if the window's logical size is *not* actually 390 (it was 195 under
row-2 emulation) — which is the emulation inconsistency, not a Buiy seam.

## Independent confirmation the core is scale-correct

Two pre-existing, orthogonal pieces of evidence agree that Buiy's scale handling
is correct when the window is set up consistently:

- **Layout is scale-invariant** — the new headless proxy
  (`crates/buiy_core/tests/layout/layout_hidpi.rs`) builds the same tree at
  `scale_factor` 1 / 2 / 3 and asserts the full `ResolvedLayout` tree is
  byte-identical, and that a viewport-filling shell fits the logical viewport at
  dsf=2. **Passes.**
- **Render is scale-correct** — `render::golden::capture_app_scaled` already
  renders headless at arbitrary DPRs (asserting the DPR pin at the capture
  boundary, `golden.rs:278`) and its goldens hold at multiple scale factors.

## The dynamic-resize sub-case (secondary, unverified)

The journal also noted a related symptom: "a dynamic window resize also mis-sizes
the surface — the render confines to a sub-region," worked around with a fresh
browser context per size. This is a distinct concern (a possible surface-reconfigure
transient) and was **not** reproduced or root-caused here; the steady-state probes
(16 s settle) were always correct. The spec already classes live-resize as
secondary to the mobile-at-load criterion (plan risk R2). Recommend it be checked
on the real-device acceptance gate; if it reproduces there it is a separate,
narrowly-scoped follow-up, not part of F9.

## What ships (and what deliberately does not)

- **Ships:** `layout_hidpi.rs` (the per-wave CI proxy / scale-invariance guard) +
  `tools/web-smoke/hidpi-check.mjs` (the faithful, consistent-dpr HiDPI web gate) +
  this report. **SG only** — no render/scale-factor code is touched.
- **Does NOT ship:** any change to the `buiy_core` render scale-factor seam. The
  plan's premise that F9 edits `extract.rs`/`view_uniform.rs`/`node.rs` scale
  handling is **retired by this finding** — the seam is correct, and a "fix" there
  would regress real devices (it would re-introduce the very dpr× mis-scale in the
  opposite direction). F9 therefore does **not** serialize with the render line and
  imposes no byte-stability risk on F1/F3/F4a/F4b.

## Recommendations

1. **Merge F9 as a test + verification-harness + docs PR** (SG). No render change.
2. **Update the FINAL spec §2.10 / plan wave F9** to record: root cause =
   emulation artifact; deliverable = the proxy + `hidpi-check.mjs`; F9 removed from
   the serial render line (it edits no render file).
3. **The HiDPI acceptance gate must use consistent dpr** — either
   `tools/web-smoke/hidpi-check.mjs` (both signals set) or a physical HiDPI device.
   A single-knob `deviceScaleFactor` emulation is **not** a valid HiDPI check; it
   manufactures false overflow (and its inverse) and is what misled the prototype.
4. **Real-device milestone check remains** (spec §4.4) — all available evidence,
   including the most faithful emulation reachable without hardware, says a real
   HiDPI phone renders correctly; the physical check is the final confirmation and
   the place to also verify the dynamic-resize sub-case.

## Residual risk

The only leg not exercisable in this headless environment is a physical HiDPI
display. The winit source (correct-for-consistent-dpr), the golden captures
(scale-correct render), the layout proxy (scale-invariant layout), and the
consistent-dpr browser runs (correct at dsf 2 and 3) all converge on "no
real-device bug." The real-device gate is confirmation, not open risk.
