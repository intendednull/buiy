# Buiy WebGPU / browser-rendering support — design

**Date:** 2026-06-25
**Revised:** 2026-06-26 — re-decided after a throwaway prototype built and ran Buiy in a real browser (see § 7 Provenance).
**Status:** draft

Graduates the **web** target out of the [foundation roadmap](2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap) into its own design spec, and resolves the web portion of the foundation's **§ 5 "Platform support staging"** open question. Aligns with — does not contradict — [architecture.md § 2.9 "Platform support — staged"](2026-05-07-buiy-foundation/architecture.md), which classifies web as a deferred, manual-release-gate platform; this spec gives that a concrete **v1 target** (WebGPU-only) and entry conditions.

> **Provenance — prototype-validated.** The original (2026-06-25) version of this spec was written from a file:line-verified feasibility study, headless. A throwaway WASM prototype (branch `wasm-proto`; `PROTOTYPE-JOURNAL.md` + retrospective) then **built and ran** Buiy in a real browser, which corrected several load-bearing assumptions this revision folds in. **Headline: the original claim that "the render code needs zero changes for the WebGPU MVP" was FALSE** — buiy's WGSL is not WebGPU-conformant (a uniformity violation the native GPU lane can't see); with one small, native-safe shader fix (D2) a styled, texted widget paints. Full keep/refine/redesign in § 7 and the prototype retrospective.

Research substrate (read these first): the [WASM/browser feasibility report](../reports/2026-06-25-wasm-browser-support-feasibility.md), the [`web-rendering` prior-art folder](../prior-art/web-rendering/) (`lessons.md` is the decision file), and the **prototype retrospective** (`wasm-proto:PROTOTYPE-JOURNAL.md`).

## Purpose

Define the target shape of running Buiy in a web browser: a Buiy app compiled to `wasm32-unknown-unknown`, rendering its custom Bevy render-graph pipeline into an HTML `<canvas>` on the **WebGPU** backend. v1 is **render-and-reach on WebGPU** — the goal is that Buiy *paints and is interactive* in a WebGPU browser, prototype-validated. The platform-service gaps (browser a11y, IME, mobile keyboard, cross-app clipboard) and **WebGL2 reach** are **named and staged**, not silently shipped as working. The step-by-step migration belongs in a later `docs/plans/` entry.

## 1. Scope

**v1 target = WebGPU-only.** The prototype validated that a single WebGPU artifact paints; WebGL2 reach turned out to be a materially larger lift (B2 poisons the whole pass — § 2-deferred) and is split into its own later milestone.

**In scope (v1):**

- A `wasm32-unknown-unknown` build of Buiy that compiles, links, and renders a widget into a `<canvas>` on the **WebGPU** backend (**one** artifact).
- The **WGSL uniformity fix** (D2) — the hard render prerequisite.
- The `arboard` compile-gate (D4); the **lean WebGPU feature wiring** without the 3D stack (D5); a deliberate **`Cargo.lock` + supply-chain** update for the backend deps (D6); a real `examples/` web crate + canvas shell.
- A **headless-browser WebGPU CI smoke** lane (§ 4) — the only gate that catches the shader-conformance class — plus a native behavior-identical test for the shader change.
- A standard size profile + the lean feature set; **measure + document** the real shippable size.

**Out of scope (deferred; designs preserved in § 2-deferred / § 6):**

- **WebGL2 reach** — the band-≤16 repack, the `Rgba16Float` fallback, the `navigator.gpu` two-artifact loader, and end-to-end WebGL2 validation. (Its own milestone.)
- IME composition, mobile soft-keyboard, browser a11y sink, cross-app async clipboard, loading-screen / streaming.
- WASM **without** Bevy, and SSR — a foundation non-goal.
- Changing Buiy's native targets or behavior. The web target is **purely additive** — every change is `cfg(target_arch = "wasm32")`-gated or a new wasm-only build target, and **the one shared-code change (D2) is behavior-identical on native** (a `#[ignore]` GPU test asserts it).

## 2. Decisions (v1)

Each decision names the choice, the reason, and the rejected alternative(s). Claims are verified in the [feasibility report](../reports/2026-06-25-wasm-browser-support-feasibility.md) and/or by the prototype (§ 7). Code line-numbers are from the prototype base (`fdb8dda`/`8917852`) — **re-verify during implementation** (origin/main has since advanced).

### D1 — WebGPU-only for v1; WebGL2 reach is a separate later milestone

**Decision.** v1-web ships **one WebGPU artifact**. WebGL2 reach (the second backend) is deferred to its own milestone (§ 2-deferred).

**Why.** The prototype validated the WebGPU MVP renders (with D2). WebGL2 is a materially larger and partly-unproven lift: B2 is now known to be a **hard, all-or-nothing blocker** (the invalid band pipeline poisons the whole render pass → blank screen, not just missing borders — § 2-deferred), plus the `Rgba16Float` fallback, a two-artifact `navigator.gpu` loader, and full end-to-end validation. WebGPU reaches current Chrome/Edge (113+), Firefox (Windows 141+/macOS 145+), and Safari 26+ — sufficient for a first web target that the foundation already stages as manual-release-gate.

**Rejected.** *WebGPU + WebGL2 in v1* — doubles the build/CI/binary surface for reach we can add later; the prototype showed WebGL2 needs the band repack before anything paints. *Wait for single-binary dual-backend (bevy#13168)* — open upstream, no timeline.

### D2 — Fix the WGSL uniformity violation (the hard render prerequisite)

**Decision.** Restructure the three buiy fragment shaders that call a derivative builtin after an early-`return` clip-discard — `shader.wgsl` (Quad, `fwidth`), `coverage.wgsl` (Glyph, `textureSample`), `band.wgsl` (`fwidth`). Compute the derivative **unconditionally** (uniform control flow) and apply the clip as an **alpha mask** (`select(1.0, 0.0, clipped)`) instead of returning early.

**Why.** Chrome's **Tint strictly enforces** WGSL's uniformity rule (derivative builtins — `fwidth`, `textureSample` — must be in uniform control flow); native **naga is lenient**. The early-return-then-derivative pattern makes every buiy shader module **fail to compile on WebGPU → every pipeline fails → blank screen** (prototype: verified, then fixed → the widget paints). The native GPU lane (Vulkan/naga) is green and **stays green** — this class is invisible to it. The fix is **behavior-identical on native** (a clipped fragment outputs alpha 0 either way), so it is safe to land on the shared render path; **recommended to land first**, with a native `#[ignore]` GPU test asserting the masked output matches the early-return output. (Prototype commit `8c017e1`.)

**Rejected.** A wasm-only shader variant — needless divergence; the fix is native-safe and a genuine portability/correctness improvement. Leaving the shaders and hoping — nothing paints on WebGPU.

### D3 — Inherit Bevy's browser bootstrap; add zero bootstrap code

**Decision.** Buiy writes **no** browser-bootstrap code; it rides Bevy's `DefaultPlugins` (non-blocking `spawn_app` runner, canvas-by-CSS-selector binding, surface-from-`RawWindowHandle::Web`, async device init via `RenderPlugin::finish()`). The only Buiy-side surface is *configuration*: the web example sets `Window.canvas` + the backend feature.

**Why.** Buiy constructs no `Window`, sets no `WgpuSettings`/`Backends`, never requests an adapter/device — it only *reads* `Res<RenderDevice>` in `finish()`/prepare systems, where Bevy guarantees the async device exists. Prototype-confirmed: `DefaultPlugins.set(WindowPlugin{canvas,…})` + `BuiyPlugin` + `Camera2d` + `Button::new` paints with zero bootstrap code. See [`prior-art/web-rendering/bevy-bootstrap.md`](../prior-art/web-rendering/bevy-bootstrap.md).

**Rejected.** Any Buiy-owned surface/adapter/event-loop management — duplicates Bevy and reintroduces the synchronous-adapter blocker Buiy avoids.

### D4 — Compile-gate: arboard off wasm + MemClipboard default

**Decision.** Move `arboard` under `[target.'cfg(not(target_arch = "wasm32"))'.dependencies]`, cfg-gate `ArboardClipboard` + its re-exports, and default the `Clipboard` resource to the existing pure-Rust `MemClipboard` on wasm. (The `x11`/`wayland` bevy features are **inert** on wasm — the prototype compiled `buiy_core` for wasm32 with them still enabled — so gating them is **optional cleanup, not required** to compile.)

**Why.** `arboard` has **no wasm backend** and is an unconditional `buiy_core` dep instantiated as the default `Clipboard` — it fails to compile for wasm before any render code is reached, and the prototype confirmed it is the **sole** core compile blocker. The `ClipboardProvider` facade makes the swap a one-site change.

**Rejected.** A web clipboard backend in v1 (D9 defers it). Gating x11/wayland as a *required* step — the prototype proved it unnecessary for compilation.

### D5 — Lean WebGPU feature wiring (drop the 3D stack)

**Decision.** Enable the WebGPU backend with the **minimal** bevy feature set, explicitly avoiding the 3D crates. The bevy **meta** crate feature is **`webgpu`** (→ `bevy_internal/webgpu` → `wgpu/webgpu`); naïvely enabling it pulls `bevy_pbr` + `bevy_mikktspace` (3D) and other crates a 2D UI lib never uses. Investigate the leanest enable (e.g. a curated feature list, or `bevy_render/webgpu` more surgically) that keeps the binary and the `cargo deny` surface minimal. Add a release size profile (`opt-level="z"`, `lto="fat"`, `codegen-units=1`, `strip`, `wasm-opt -Oz`, brotli) and **measure + document** the shippable size. (Loading-screen / streaming deferred.)

**Why.** The prototype's debug wasm was ~100 MB, bloated by the 3D crates the `webgpu` meta-feature drags in. A 2D UI lib needs none of it; trimming is both a size win and just-correct.

**Rejected.** Shipping the `webgpu` meta-feature as-is (avoidable bloat + deny surface). A loading-screen/streaming investment in v1 (web is manual-release-gate; defer the polish).

### D6 — Deliberate Cargo.lock + supply-chain update for the backend deps

**Decision.** Treat the lock update as an explicit P0 step: enabling the WebGPU backend feature **adds crates to `Cargo.lock`** (`bevy_anti_alias`, `bevy_dev_tools`, `bevy_post_process`, `bevy_feathers`, `bevy_pbr`, `bevy_mikktspace`, …) that the committed `--locked` lock never had. Refresh the index, update the lock in a dedicated commit, and **re-run `cargo deny` + the MSRV check** on the new crates. (getrandom needs **no** action — verified absent from the wasm production graph; only relevant if a future dep re-activates it.)

**Why.** The research's "wasm-layer deps already in the lock" premise is **true for wasm-bindgen/web-sys/web-time but false for the backend-feature crates** — under `--locked` + a stale index, resolution *fails* until the lock is deliberately updated (prototype-confirmed). This is real supply-chain surface, not a no-op.

**Rejected.** Assuming the lock is unaffected (it isn't). Skipping `cargo deny` on the new crates (they're shipped code).

### D7 — Single-threaded on web

**Decision.** Run Bevy's single-threaded scheduler on web (the current default — `multi_threaded` off). No wasm threads (atomics + COOP/COEP + `wasm-bindgen-rayon` + nightly) in v1. **Zero change required.**

**Why.** Bevy 0.19 doesn't run its multithreaded scheduler on web regardless; a latency-bound UI lib gains little; cross-origin isolation restricts embedding.

**Rejected.** wasm threads in v1 — high cost, low payoff, deployment-restricting.

### D8 — Accessibility: tree is web-ready, sink is staged and disclosed

**Decision.** On web, Buiy builds its AccessKit `TreeUpdate` as usual (`a11y/translate.rs` is winit-free pure data) but reaches **no browser assistive technology** — AccessKit ships no web/canvas adapter. **Staged and disclosed** in every web milestone, matching foundation § 2.9. A future web a11y sink (hidden-DOM/ARIA overlay driven off the same `build_tree_update`, or upstream `accesskit_web`) swaps only the sink.

**Why.** The data/sink split already exists; the in-process driver already consumes the tree with no winit adapter, proving the data layer is web-ready. Only the platform sink is missing.

**Rejected.** Claiming web a11y "works" because the tree builds (it reaches nothing — a silent WCAG failure). Blocking v1 on a DOM sink (an XL effort).

### D9 — IME, mobile keyboard, cross-app clipboard: deferred behind named seams

**Decision.** v1-web supports **desktop hardware-keyboard** Latin input only. IME composition (winit emits no `Ime::Preedit`/`Commit` on web — winit#4424), the mobile soft-keyboard (no on-screen keyboard without a focused DOM input/`EditContext`), and cross-app clipboard (async `navigator.clipboard` vs the sync `ClipboardProvider` trait) are **deferred**, each behind a named seam. In-app copy/paste works via `MemClipboard`.

**Why.** Functional gaps, not compile blockers; they degrade safely. The shims live outside winit and are separable work.

**Rejected.** Owning the IME/keyboard shim in v1 — scope the render/reach target first.

### D10 — Fonts: embedded by default; assets via WebAssetPlugin or embed

**Decision.** Rely on Buiy's embedded default font (`include_bytes!` Fira Sans) for v1; system-font scanning stays opt-in/off (a no-op on wasm). Fetched assets go through Bevy's `WebAssetPlugin` (added before `AssetPlugin`); non-Latin coverage is a font-*supply* task. (Prototype: the "Save" button text rendered with zero font work.)

**Why.** The browser exposes no system fonts; cosmic-text + swash + fontdb run on wasm with in-memory `Source::Binary`.

**Rejected.** System-font scanning on web (dead path). woff2-in-engine (a separate seam if needed).

## 2-deferred. Deferred decisions (the WebGL2 reach milestone)

Preserved here so the later milestone re-decides from a written design, not from scratch.

- **Band pipeline ≤16 vertex attributes (was D4 / report B2).** `band.wgsl` declares **17** attributes (`@location 0..=16`); `wgpu`'s `downlevel_webgl2_defaults()` caps `max_vertex_attributes = 16`. **Prototype sharpened the impact:** the invalid band pipeline is set inside `buiy_pass`, so an invalid pipeline makes the whole `CommandEncoder` fail to finish → **the entire pass is dropped → NOTHING paints** (not the "missing borders/focus-rings" the original framing described). So on WebGL2 this is a **hard, all-or-nothing prerequisite**. Fix: repack `BorderBandInstance` + `band.wgsl` to ≤16 (fold the two clip `vec2`s into one `vec4`, the affine into one `vec4`, and/or move per-corner radii into an instance-indexed UBO). It is also WebGPU **spec-baseline insurance** (baseline is 16; desktop Dawn's ~30 masks it today — a conservative/mobile WebGPU adapter would also fail), so it may be pulled forward into the WebGPU track if mobile WebGPU is targeted.
- **`Rgba16Float` effect-compositor fallback (was D5 / report W1).** On WebGL2, gate the float effect targets on `EXT_color_buffer_float` (per the `wgpu-hal-29` capability model that one extension suffices for rgba16f render+blend; verify end-to-end) or substitute `Rgba8Unorm`. **Not exercised by the prototype** (the simple button has no effect group) — needs an opacity<1 fixture to validate. WebGPU-core, so v1-WebGPU is unaffected.
- **Two-artifact `navigator.gpu` loader.** A JS feature-detect that loads the WebGPU build when `navigator.gpu` is present, else the WebGL2 build. The bevy `webgpu`/`webgl2` features don't coexist in one binary (`webgpu` takes precedence), so reach = two artifacts.
- **End-to-end WebGL2 validation.** The fixed Quad/Glyph WGSL **does** translate + validate via naga→GLSL ES (prototype: only the band failed, on attribute count, not translation) — so the old "naga translation unverified" risk is largely **resolved**; what remains is validating the full pipeline (incl. the Bevy core_pipeline passes) end-to-end once the band fits ≤16.

## 3. Target architecture

The v1 web target is a thin additive shell over the native architecture:

1. **Build axis.** The web build sets the bevy **`webgpu`** meta-feature (lean variant — D5), as one wasm artifact. Native builds add nothing.
2. **Shared-code change (D2).** The three-shader uniformity fix — the only change to shared (native + web) code, behavior-identical on native.
3. **Dependency cfg-gating (D4) + lock (D6).** `arboard` moves under a non-wasm target table; `buiy_core` compiles for wasm32; the lock gains the backend crates (deliberate update + `cargo deny`/MSRV).
4. **Bootstrap inheritance (D3).** A web example crate configures `Window { canvas, fit_canvas_to_parent, prevent_default_event_handling }` + `console_error_panic_hook` + console tracing; trunk serves an `index.html` with the canvas. No Buiy bootstrap code.
5. **Platform-service posture (D8/D9), staged.** Clipboard → `MemClipboard`; IME/keyboard → deferred shim; a11y → tree builds, sink deferred — each disclosed.
6. **Correctness wiring (verify-then-fix).** sRGB-encode on the negotiated surface (**prototype: looked correct out of the box** — likely a non-issue, still confirm vs a native capture), DPR × logical-size clamp to `max_texture_dimension_2d` (the prototype saw a DPR/sizing oddity), runtime (not `cfg!`) macOS-modifier detection, `Msaa::Off` evaluation, canvas `prevent_default` for wheel/touch.

## 4. Verification strategy

Per foundation § 2.9, web is a **manual-release-gate** platform; v1 makes the **compile + paint** dimension automated:

- **Compile gate (CI).** A `cargo build --target wasm32-unknown-unknown` lane for the web build + `wasm32-unknown-unknown` added to `deny.toml` `graph.targets` so the web dependency graph is audited.
- **Headless-browser WebGPU smoke gate (CI) — the load-bearing new lane.** Productionizes the prototype harness: build the wasm web example → load in **headless Chrome (WebGPU)** → assert **zero shader/pipeline errors + a non-blank canvas**. This is the **only** gate that exercises the real **Tint** compiler, so the only one that catches the D2 uniformity class and prevents its silent regression (the native GPU lane can't — naga is lenient by design). `panic = abort` on wasm also makes this lane catch render-extract panics the headless gate misses.
- **Native shader-equivalence test.** A `#[ignore]` GPU test asserting the D2 masked-clip output is pixel-identical to the early-return output, so the fix can't silently alter native rendering.
- **Visual gate (manual / future).** In-browser visual diffing needs a different harness than `buiy_verify`'s native readback — deferred; keeps web at manual-release-gate for *visuals*.
- **A11y gate (blocked upstream).** Real browser-AT verification waits on a web a11y sink (D8).

## 5. Phasing (pointer)

v1 (WebGPU) sequence, basis for the implementation **plan** (`docs/plans/`): **(1)** D2 shader fix + the native-equivalence test (land first — native-safe correctness fix); **(2)** D4 arboard gate; **(3)** D5 lean WebGPU feature wiring + the web example crate + canvas shell; **(4)** D6 lock + `cargo deny`/MSRV; **(5)** the headless-browser WebGPU smoke lane; **(6)** size profile + measure. The **WebGL2 reach milestone** (§ 2-deferred) follows as separate work. This spec commits to the *target* and *decisions*; the plan commits to the *order*.

## 6. Risks & open questions

- **sRGB / gamma (downgraded).** Original "highest-risk" item; the prototype rendered **correct colors out of the box** on WebGPU. Likely a non-issue — still confirm vs a native side-by-side capture; a final-pass encode shader is the fallback if a surface ever lacks sRGB-encode-on-write.
- **Shader-conformance regression (the real ongoing risk).** The D2 class is invisible to the native GPU lane forever. Mitigated by the § 4 headless-browser smoke lane — **this lane is load-bearing, not optional.**
- **Binary size.** Even lean + `wasm-opt`, a Bevy wasm app is large (~15-30 MB class). v1 measures + documents; loading-screen deferred.
- **DPR / canvas sizing.** The prototype saw a physical-vs-CSS size mismatch — pin during implementation (clamp to `max_texture_dimension_2d`).
- **WebGL2 milestone unknowns** — see § 2-deferred (W1 not yet exercised; end-to-end validation pending the band repack).

## 7. Provenance: what the prototype changed

The throwaway prototype (branch `wasm-proto`; journal + retrospective) re-decided the headless spec:

- **REDESIGN:** "zero render changes for the WebGPU MVP" → **one shader-uniformity fix (D2) is a hard prerequisite**; then it paints.
- **REDESIGN:** B2 on WebGL2 = "missing borders" → **poisons the whole pass → blank screen** (§ 2-deferred); a hard all-or-nothing WebGL2 blocker.
- **REFINE:** the bevy cargo feature is the **meta** `webgpu`/`webgl2`, not `webgl` (that's the internal feature).
- **REFINE:** "wasm deps already in the lock" → **false for the backend crates** (D6); the web build needs a deliberate lock + `cargo deny`/MSRV update, and the `webgpu` meta-feature pulls 3D crates (D5).
- **REFINE:** `x11`/`wayland` gating is **optional, not required** (D4) — inert on wasm.
- **DOWNGRADE:** sRGB risk (looked correct); naga→GLSL translation (the fixed non-band shaders translate fine — § 2-deferred).
- **KEEP (validated):** WebGPU-first, inherit-Bevy-bootstrap, arboard-gate, embedded fonts, single-threaded, the a11y data/sink split — all confirmed by running it.
- **VALIDATED:** with D2 + D4 + D5 + D6, Buiy renders a styled, texted widget in Chrome on WebGPU (real AMD RDNA2 adapter).

## 8. References

- Research: [feasibility report](../reports/2026-06-25-wasm-browser-support-feasibility.md); [`web-rendering` prior-art](../prior-art/web-rendering/) (esp. [`lessons.md`](../prior-art/web-rendering/lessons.md), [`bevy-bootstrap.md`](../prior-art/web-rendering/bevy-bootstrap.md)); **prototype retrospective** (`wasm-proto:PROTOTYPE-JOURNAL.md`, unmerged reference).
- Foundation: [README § 5 open questions](2026-05-07-buiy-foundation/README.md#5-open-questions), [architecture § 2.9](2026-05-07-buiy-foundation/architecture.md).
- Code anchors (re-verify during implementation): `crates/buiy_core/Cargo.toml` (arboard dep), `crates/buiy_core/src/text/mod.rs` (default `Clipboard` insertion), `crates/buiy_core/src/render/{shader,coverage,band}.wgsl` (D2 uniformity fix), `crates/buiy_core/src/render/instance.rs` (`BorderBandInstance`, deferred), `crates/buiy_core/src/render/compositor.rs` (`Rgba16Float`, deferred), `crates/buiy_core/src/text/font_system.rs` (embedded font), `crates/buiy_core/src/a11y/{translate,adapter}.rs` (a11y data/sink split).
