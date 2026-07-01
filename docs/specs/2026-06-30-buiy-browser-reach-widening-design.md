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

### D6 — Cross-app clipboard: sync-facade + async-fill latch (web provider) — **LANDED (W3)**
**Decision.** A wasm-only `WebClipboard` swapped in at `text/mod.rs` (the wasm arm). Copy/cut stay
sync (fire-and-forget `navigator.clipboard.write_text`); paste = a sync `get_text()` facade
backed by an async `read_text()`-filled latch (`spawn_local`/`JsFuture`), falling back to the
in-app copy where the OS read is denied. **Why.** The `ClipboardProvider` trait is sync;
`read_text()` is a Promise; the latch keeps the trait unchanged. The web-sys Clipboard API is
behind `--cfg=web_sys_unstable_apis` (wired in `.cargo/config.toml` + the web-smoke CI job, since
the workflow-global `RUSTFLAGS` shadows the config). **Verified (W3):** cross-app COPY works
end-to-end — typing in the gallery input + Ctrl+A/Ctrl+C put the text on the real OS clipboard
via `writeText` (headless Chrome, clipboard permissions granted). **Known limitation:** cross-app
PASTE is BEST-EFFORT — `read_text()` needs the clipboard-read permission + transient activation,
and Bevy's `Update` runs on the rAF tick (not inside the paste gesture), so where the OS read is
denied `get_text` falls back to the in-app copy (in-app copy/paste always works). **A guaranteed
cross-app paste needs a DOM `paste`-event bridge** (reads `clipboardData` inside the gesture) —
a named follow-up. Lock delta: 4 dependency EDGES on `buiy_core` (js-sys/wasm-bindgen/
wasm-bindgen-futures/web-sys, already in the graph), no new crates; `cargo deny` clean.
**Rejected.** Making the trait async (churns the whole native path for a web-only need); a full
DOM-event bridge in v1 (defers to the follow-up — the writeText path already delivers copy).

### D7 — a11y sink: Buiy-owned hidden DOM/ARIA overlay (accesskit_web does not exist) — **LANDED (W4, read-only v1)**
**Decision.** A wasm-only `WebA11ySinkPlugin` (`a11y/web_sink.rs`), registered by `A11yPlugin` on
wasm alongside the (inert-on-web) winit adapter. It mirrors each frame's `A11yNodeView` snapshot
(`builder.snapshot()` — the SAME data `build_tree_update` consumes) into a visually-hidden
(clip pattern, NOT `display:none`/`aria-hidden`), ARIA-annotated DOM subtree (`#buiy-a11y-tree`)
next to the canvas: role→`role` (21 roles mapped), name→`aria-label`, description→`aria-description`,
toggled→`aria-checked`, expanded→`aria-expanded`, selected→`aria-selected`, disabled→`aria-disabled`,
hidden→`aria-hidden`; nested per the a11y parent/children; rebuilt only on a change signature (a
stable AX tree). Every DOM call is fallible-swallowed (an a11y sink must not crash the app).
**Why.** `accesskit_web` **does not exist** (verified). **Verified (W4):** the browser AX tree (CDP
`Accessibility.getFullAXTree` — what a screen reader consumes) contains the gallery's widgets with
correct roles + accessible names (application + 23 named buttons + 3 checkboxes + a switch + 2
textboxes + dialog/heading/status). Lock delta: web-sys Document/Element/HtmlElement/Node features
(no new crates); `cargo deny` clean; native unaffected (the module is `cfg(wasm32)`). **Scope
(v1):** OUTBOUND/read-only. **Follow-ups (named):** (1) INBOUND — a screen-reader click/focus routed
BACK into the app via the existing `ActionRequest` path (`data-buiy-entity` handle is already
emitted); (2) a real-AT pass (NVDA/VoiceOver) beyond the AX-tree assertion; (3) `aria-activedescendant`/
`focus()` for live focus (the `data-buiy-focused` marker is AX-observable now). **Rejected.**
Waiting for upstream (indefinite); claiming the built tree = working a11y without verifying the AX
tree (silent WCAG failure — this wave verifies the AX tree).

### D8 — IME + mobile soft-keyboard: Buiy DOM bridge outside winit — **LANDED (W5, prototype-first)**
**Decision.** A wasm-only bridge: a hidden focused `<input>` sibling of `#buiy` (egui TextAgent
pattern); on editor focus `.focus()` (raises the OSK) + move to `ime_position`; register
`compositionstart/update/end` + `input` → `MessageWriter<Ime>` feeding the unchanged E5 engine
(the OUTPUT seam `ime_enabled`/`ime_position` at `ime.rs:571,585`, INPUT seam
`MessageReader<Ime>` at `ime.rs:429`). **Why.** winit#4424 is OPEN with no timeline (winit web
emits no `Ime`); the engine is winit-free + reusable. EditContext is Chromium-only → the hidden
`<input>` is the cross-browser path.

> **Empirical finding (W5 probe, 2026-07-01 — why W5 is its own prototype-first wave, not a quick
> shim).** A no-rebuild browser probe + the winit-0.30.13 source establish that focusing the hidden
> input **starves winit's text keyboard**: winit's `WindowEvent::KeyboardInput` (the path bevy's
> editor consumes) is attached to the **canvas** (`web/web_sys/canvas.rs:301`); with the input
> focused, canvas `keydown` fires **0/3** (probe) while `window` `keydown` still fires 3/3 by
> bubbling — but the window-level winit listener (`event_loop/runner.rs:347`) only emits raw
> `DeviceEvent::Key` (device-events-gated), which the editor does NOT use for text. So the
> hidden-input bridge must **fully replace** keyboard for the focused editor (the egui TextAgent
> model): route ALL input from the input back into bevy — text via `input`/composition → `Ime`,
> and **every non-text key (arrows, Enter, Backspace, Tab, Esc, Ctrl/Cmd shortcuts) via `keydown`
> → synthesized `bevy::input::keyboard::KeyboardInput`**, needing a DOM-`code`→bevy-`KeyCode`
> mapping table. Plus focus-sync (focus/blur on `ime_enabled`), double-insertion avoidance, and a
> touch-vs-desktop policy. The probe scoped W5 as its own prototype-first wave (not a shim).

**Implementation (`text/edit/web_ime.rs`, wasm-only).** `WebImePlugin` (registered by `BuiyTextPlugin`
on wasm) creates a hidden off-screen `<input>` and, on `Window.ime_enabled`, `.focus()`es it (raising
the OSK + capturing composition). Its `keydown`/`keyup` (non-composing, `preventDefault`'d) → synthesized
`KeyboardInput` (logical `Key` from `event.key` — the editor classifies on `logical_key`; `KeyCode` from
`event.code` — mainly so `ButtonInput<KeyCode>` reflects modifiers); its `compositionupdate`/`end` →
`Ime::Preedit`/`Commit`. On `ime_enabled=false` the input blurs and winit's canvas keyboard resumes.
**Verified (W5, real WebGL2 browser):** clicking the editor focuses the hidden input; **typing** lands
in the editor, **arrow navigation + Backspace + Enter** work, **CJK IME composition commits** (synthetic
`compositionstart→update→end("你好")` inserted 你好), and **modifier shortcuts** (Ctrl+A + Ctrl+C → OS
clipboard) work — all through the bridge, no panics. Native unaffected (`cfg(wasm32)` module); web-sys
HtmlInputElement/KeyboardEvent/CompositionEvent features (no new lock crates); `cargo deny` clean.
**Follow-ups (named):** `ime_position` tracking (position the input at the caret for the IME candidate
window); a touch-only focus policy; the full `KeyCode` table (v1 covers letters/digits/modifiers/nav —
the editor uses `logical_key`, so the rest only affects `ButtonInput<KeyCode>` completeness).

**Rejected.** Waiting on winit#4424; owning a full IME engine (already exists); a naive
focus-the-input shim (proven to starve the editor's keyboard — a regression, not a feature).

### D9 — Web target stays purely additive
Every change is `cfg(target_arch="wasm32")`-gated or a new build target, EXCEPT the shared-code
D2 (band fold, behavior-identical) and D5 (touch, native-safe + test-guarded). Native targets/
behavior unchanged; `--workspace` enables no backend feature.

## 3. Phasing (→ the plan)

**Status (2026-07-01): 4 waves LANDED + verified on `main`; 2 items remain.**

1. **W1 Render reach** — **✅ LANDED (#94).** D2 band fold + D1 feature plumbing + `build-web.sh`
   loader + D4 CI webgl2 leg. The flag-removal deliverable — verified (renders + CI-gated on
   SwiftShader). **D3 (`Rgba16Float` float-less fallback) was deliberately excluded** (not
   reproducible on desktop/SwiftShader — both expose the float extensions), and **remains
   DEFERRED** to a float-less-rig / forced-fallback wave.
2. **W2 Touch** — **✅ LANDED (#98).** D5 Part A + Part B + headless cold-tap/touch tests; verified
   in a real WebGL2 browser. (D3 was NOT bundled here — it needs a float-less rig it lacks.)
3. **W3 Clipboard** — **✅ LANDED (#99).** D6 WebClipboard; cross-app copy verified end-to-end
   (best-effort paste documented).
4. **W4 a11y overlay** — **✅ LANDED (#100).** D7 WebA11ySink; the AX tree verified via CDP
   (read-only v1; inbound/real-AT follow-ups named).
5. **W5 IME/soft-keyboard** — **✅ LANDED (prototype-first).** D8 `WebImePlugin`
   (`text/edit/web_ime.rs`) — a hidden `<input>` fully bridges keyboard+IME for the focused editor.
   Verified in a real WebGL2 browser: typing, arrow nav, Backspace, Enter, **CJK IME composition
   commit**, and **Ctrl+A/Ctrl+C shortcuts** all work through the bridge; native unaffected. (The
   probe first scoped it as prototype-first; building + running it proved it out.)
6. **D3 `Rgba16Float` float-less fallback** — **⏳ DEFERRED (only remaining item).** Needs a
   float-less WebGL2 rig (or a forced-`Rgba8` test) that the dev/CI adapters can't provide (all
   expose the float extensions); the desktop/common-mobile path already works (both float
   extensions present), so this only hardens older/low-end mobile.

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
