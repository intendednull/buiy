# Buiy render — verification

**Parent:** [README.md](README.md)

How render correctness is proven. The render subsystem has one fact about its
test environment that shapes the whole strategy: **CI runners have no wgpu
adapter, so the GPU paint path cannot execute headless.** This file states that
constraint, lays out the layered proof structure that works around it (each
render property proven at the lowest layer that can see it), maps the
render-relevant CI gates from [foundation/verification.md § 3.15](../2026-05-07-buiy-foundation/verification.md#315-verification-pipeline)
onto the pillars of [README.md § 2](README.md#2-architectural-pillars-one-line-summaries),
and specifies the e2e golden-image harness that closes the pixel-correctness gap.

It owns the *mechanism* of render verification. Concrete per-fixture numbers
(perf budgets, leak thresholds, golden tolerances) are owned by
`buiy-verification-design` and tracked in [README.md § 5](README.md#5-open-questions)
open question #4.

---

## 1. The hard constraint: no GPU on CI runners

Buiy's check command runs tests under `xvfb-run -a` on Linux
([CLAUDE.md § Build & Test](../../../CLAUDE.md)). `xvfb` is a *virtual X server* —
it provides a display surface so windowed code does not abort on `DISPLAY`
being unset, but it provides **no GPU and no wgpu adapter**. The CI matrix in
[`.github/workflows/ci.yml`](../../../.github/workflows/ci.yml) installs
`libwayland-dev` / `libxkbcommon-dev` / `xvfb` — never a Vulkan ICD or a
software rasterizer.

This is fatal for any test that touches the paint path, because Bevy's
`RenderPlugin::build` does `block_on(initialize_renderer(...))`, which
`expect()`s a real adapter. With no adapter the process panics *before any Buiy
render code runs*. The Phase-0 smoke suite already encodes this reality
([`crates/buiy_core/tests/render_smoke.rs`](../../../crates/buiy_core/tests/render_smoke.rs)):
the two tests that add `bevy::render::RenderPlugin` are marked

```rust
#[ignore = "needs a wgpu adapter (real GPU or lavapipe); covered by Task 19 e2e harness"]
```

and run only with `--ignored` locally or on the GPU-provisioned e2e runner. The
single non-ignored smoke test (`render_plugin_loads_without_panic`) deliberately
adds `BuiyRenderPlugin` *without* `RenderPlugin` and relies on the plugin's
early-return guard — `let Some(render_app) = app.get_sub_app_mut(RenderApp) else { return; }`
in [`render/mod.rs`](../../../crates/buiy_core/src/render/mod.rs) — so it
exercises `build` without ever provisioning a device.

The whole verification design is a response to this single fact: **prove every
render property at the lowest layer that can observe it without a device, and
reserve the GPU only for the one property that genuinely requires pixels.**

> **Why not provision a GPU for the whole suite?** A software rasterizer
> (`lavapipe` / `llvmpipe`) *can* supply an adapter, but it is slow, its output
> is not bit-identical to the canonical CI GPU class the goldens are captured on
> ([foundation gate #2](../2026-05-07-buiy-foundation/verification.md#315-verification-pipeline)),
> and pulling it into the unit/integration suite would make every PR pay GPU
> setup cost for properties that need no GPU. The split below keeps the common
> path device-free and fast.

---

## 2. The proof layers

Four layers, ordered cheapest-and-most-headless first. A render claim is proven
at the **lowest** layer that can see it; only claims that reduce to "the pixels
are right" reach layer 4.

### 2.1 Layer 1 — smoke / registration tests (headless, no device) — **F**

These assert the render graph is *wired*, not that it *paints*. They run on
every PR with no adapter.

- **Plugin builds without a device.** `BuiyRenderPlugin::build` is a no-op when
  `RenderApp` is absent; adding it to a `MinimalPlugins` app must not panic.
  (`render_smoke::render_plugin_loads_without_panic`, already green.)
- **Pipeline is queued.** With a device present (e2e runner / `--ignored`), the
  `BuiyPipeline` resource exists in the `RenderApp` world after `build`
  (`render_smoke::pipeline_registers_in_render_app`). The pipeline is queued via
  `PipelineCache::queue_render_pipeline` in
  [`render/pipeline.rs`](../../../crates/buiy_core/src/render/pipeline.rs); the
  test asserts the resource, the e2e harness (layer 4) asserts the *compiled*
  pipeline by actually drawing.
- **Graph node + edges exist per window.** *Separate PRESENT from TARGET here.*
  **Present (Phase 0):** `render_smoke::render_graph_node_inserted_after_main_2d_pass`
  asserts **only that the `BuiyRenderLabel` node is present** in the `Core2d`
  sub-graph (`sub.get_node_state(BuiyRenderLabel).is_ok()`); the wiring it stands
  over today is the single edge
  `add_render_graph_edge(Core2d, Node2d::EndMainPass, BuiyRenderLabel)` in
  [`render/node.rs`](../../../crates/buiy_core/src/render/node.rs) — an
  `EndMainPass → BuiyRenderLabel` after-anchor, with **no** `Tonemapping` edge and
  **no** `StartMainPassPostProcessing` anchor. This is a Phase-0 seed, not
  target-state proof.
  **Target:** the edges of [architecture.md § 1.3](architecture.md) —
  the after-anchor `StartMainPassPostProcessing → BuiyRenderLabel` and the
  load-bearing `BuiyRenderLabel → Tonemapping` — are what this test must be
  **extended to assert** (the present test does not yet check either edge; it must
  grow from node-presence to edge-chain assertions). As the spec further grows the
  node into the per-window `BuiyRenderLabel` node group of
  [pillar 2 / architecture.md](architecture.md), the same test extends to assert the
  group's internal node→node edges and the top-layer composite node's placement
  after the typed-primitive batched node — still purely a graph-shape assertion,
  but **not** device-free: **registering the render-graph node and its edges
  requires the `RenderApp`, and the `RenderApp` is only created when
  `RenderPlugin::build` succeeds in acquiring a wgpu adapter.** So the
  node/edge-registration assertions are part of the GPU e2e path (they carry the
  `#[ignore]` adapter caveat and run on the e2e runner / `--ignored`), not the
  every-PR headless path. The headless path covers the component model + `ClipRect`
  geometry + instance/coordinate math + the **main-world** system
  schedule-membership/ordering (the `WriteClipRects` / `WriteEffectGroups`
  render-prep passes are main-world systems, so their schedule placement is
  inspectable with no `RenderApp`) — never the graph wiring, which has no
  `RenderApp` to register into without a device.

> Registration tests that need `RenderPlugin` (pipeline resource, graph node)
> carry the `#[ignore]` adapter caveat and run on the e2e runner; tests that
> only touch `build`'s control flow run on every PR. The boundary is "does this
> assertion require a device to *exist*," not "does it require a frame to be
> *drawn*."

**Proves:** pillar 2 (the node group + composite pass are registered in the
right place), pillar 4 to the extent that `WriteClipRects` is *scheduled* in the
render-prep set (the *values* it writes are layer 3).

### 2.2 Layer 2 — CPU unit tests of the render math (headless, no device) — **F**

Everything the GPU consumes is *produced* on the CPU, and that producer is a
pure function testable with no adapter. This is the layer that already carries
the most weight ([`crates/buiy_core/tests/render_instance.rs`](../../../crates/buiy_core/tests/render_instance.rs))
and the one this spec grows the most as the component model lands.

- **Instance-data packing vs the pipeline descriptor.** The single most
  important invariant: `size_of::<InstanceData>()` must equal the per-instance
  `array_stride` declared in the `RenderPipelineDescriptor`
  (`render_instance::instance_data_layout_matches_pipeline_descriptor`,
  pinning stride = 36 today). A drift between the Rust struct and the WGSL
  vertex layout is a silent corruption the compiler cannot catch; this test is
  the guard. **PRESENT vs TARGET:** the `array_stride = 36` literal and the
  `to_instance_*` packing assertions are **Phase-0 BASELINE** tests pinning the
  *current* per-vertex-instance layout; the pillar-3 hybrid handoff
  ([architecture.md](architecture.md)) **REPLACES** that per-instance vertex
  layout with persistent storage buffers + a view uniform, so these specific
  stride/packing assertions are migrated, not target-state proof. What carries to
  the target is the **rule**, not the number: as [component-model.md](component-model.md)
  adds `Background` / `Border` / `BoxShadow` packing, **each new instance/uniform
  struct gets its own stride-vs-descriptor assertion** against its own layout —
  the invariant is "no `#[repr(C)]` GPU struct without a matching layout test,"
  and *that* is the target-state proof, with the literal `36` retired when the
  hybrid handoff lands.
- **sRGB → linear conversion.** `to_instance` converts the authored sRGB color
  to `LinearRgba` so the linear-light pipeline (pillar referenced in
  [color-and-forced-colors.md](color-and-forced-colors.md)) blends correctly and
  the `Rgba8UnormSrgb` target re-encodes on write. Asserted against Bevy's own
  `LinearRgba::from(Color)` (`render_instance::to_instance_packs_color_in_linear_rgba`).
- **Coordinate / view math.** The px→clip mapping (origin centering, the single
  y-flip carried in `rect_size`, the `2.0 / min(window)` radius scale) is locked
  by `to_instance_centers_origin_at_window_center`,
  `to_instance_offsets_position_to_clip`, and `to_instance_radius_uses_min_window_dim`.
  **PRESENT vs TARGET:** these three `to_instance_*` assertions are **Phase-0
  BASELINE** — they pin the *current* CPU-side per-instance conversion that the
  hybrid handoff **REPLACES**. When pillar 3's hybrid handoff moves this conversion
  into a **view uniform**, the assertions are migrated to a CPU port of the uniform
  math, and *that* port is the target-state proof. The durable property —
  "a px rect lands at the right clip coordinates" — is layer-2 testable whether the
  arithmetic lives on the (Phase-0) CPU `to_instance` path or the (target)
  view-uniform-fed vertex stage; only the seed assertions are retired.
- **SDF semantics via a CPU port.** The fragment SDF is mirrored 1:1 in Rust
  (`sdf_rounded_rect` / `shader_half_size` in `render_instance.rs`) and walked at
  known sample points, proving "inside the box is filled, outside is empty"
  and pinning the `abs(rect_size)` half-extent fix
  (`shader_sdf_inside_is_filled_outside_is_empty`,
  `signed_rect_size_breaks_sdf_without_abs`). Because the SDF uses only
  `abs`/`length`/`min`/`max` — no platform intrinsics — the CPU port is exact,
  so shader-logic regressions are caught headless **before** the GPU golden ever
  runs. As the typed-primitive set grows (shadow, glyph, path —
  [README § 2 pillar 2](README.md#2-architectural-pillars-one-line-summaries)),
  each SDF gets the same CPU-port treatment.

**Proves:** pillar 3 (handoff packing correctness — the bytes the GPU reads are
the bytes layout produced), the color half of
[color-and-forced-colors.md](color-and-forced-colors.md) (linear-light),
and the geometry that the SDF turns into coverage. These are the properties most
prone to silent corruption and least in need of a GPU — so they live here.

### 2.3 Layer 3 — layout-snapshot gate on `ClipRect` (headless, no device) — **F**

`ClipRect` is computed by the `WriteClipRects` render-prep pass
([clip-and-transform.md](clip-and-transform.md)) from layout-owned inputs only,
and written as a component holding **resolved scalar values** (the clip
rectangle, plus the `Changed<ScrollOffset>` recompute result). Because it is
plain resolved geometry — not pixels — it is verified exactly like layout's own
output: **foundation gate #5 (layout snapshots)** snapshots `ClipRect` per
fixture and diffs the resolved values.

This is the structural payoff of pillar 4: by computing the clip in a render-prep
pass that reads only layout output (rather than walking ancestors inside render
extract), the clip geometry is *visible to the layout-snapshot gate* and cannot
silently drift across the render boundary. A fixture asserts, e.g., that a child
of an `overflow: hidden` scroller with a `ScrollOffset` has the intersected,
scroll-translated clip rect the spec prescribes — all as numbers, no device.

**Proves:** pillar 4 (clip geometry correctness), and the `Containment` PAINT /
`Overflow` + `ScrollOffset` consumption claims in
[clip-and-transform.md](clip-and-transform.md) and
[paint-order-and-top-layer.md](paint-order-and-top-layer.md), at the resolved-value
layer. The *pixels* that result from clipping are layer 4.

### 2.4 Layer 4 — e2e golden-image harness (needs a device) — **F**

The one property that reduces to "the pixels are right" — and only that — is
proven by rendering on a real device and perceptually diffing against a curated
golden. This is **foundation gate #2 (visual regression)**. Section 4 specifies
it in full.

**Proves:** the end of every pillar's chain — that the queued pipeline actually
compiles and draws (pillar 2/3), that the SDF coverage and linear→sRGB output
land as the right pixels (layer-2 math made visible), that clipping/top-layer
composite/effect-group opacity composite *to the right image* (pillars 4/6), and
forced-colors output (gate #11, § 3). It is the most expensive and least
granular layer, so it carries only what the lower layers cannot.

---

## 3. The render-relevant CI gate map

Of the fifteen CI gates in
[foundation/verification.md § 3.15](../2026-05-07-buiy-foundation/verification.md#315-verification-pipeline),
seven bear on render. Each row names the gate, the proof layer above that
realizes it, and the pillar claim it discharges.

| Gate | Layer | What it proves for render | Pillar / file |
|---|---|---|---|
| **#2 Visual regression** | 4 (device) | Rendered output matches golden per widget × state × theme × viewport. The *only* gate that proves pixels. | Pillars 2, 3, 6 end-to-end; [effect-compositor.md](effect-compositor.md), [color-and-forced-colors.md](color-and-forced-colors.md) |
| **#5 Layout snapshots** | 3 (headless) | `ClipRect` resolved geometry (the box, ancestor intersection, scroll-translated viewport). | Pillar 4; [clip-and-transform.md](clip-and-transform.md) |
| **#10 Hit-target ≥24×24** | 3 (headless) | A **layout/picking-time** geometric gate, not render-owned: every interactive widget's picking hit-rect is ≥24×24 at every fixture viewport. Render aligns to it for free — it shares one `ClipRect` and paint order = hit-test order reversed (pillar 1) — so the geometry the gate checks is the geometry render paints. | Pillar 1 (ordering identity); [paint-order-and-top-layer.md](paint-order-and-top-layer.md) |
| **#11 Forced-colors** | 3 + 4 | (a) token-flow analysis: no widget paints a color outside the system-color token set under `forced-colors: active` (headless, on the resolved `Background`/`Border`/`Outline` tokens); (b) golden visual diff under forced-colors (device) — **blocked on `buiy-theme-tokens-design` delivering the forced-colors system-color map (or a minimal v1 stub map): the goldens cannot be captured until the resolved palette exists, so #11(b) is a tracked-dependency gate, not silently un-runnable at v1.** | [color-and-forced-colors.md](color-and-forced-colors.md) |
| **#14 Perf regression** | 4 (device) | The **combined** layout + render + a11y per-frame time per fixture vs main-branch baseline on the fixed runner — the ±10% default slack and the per-fixture budget belong to the *whole* gate, not to render alone. This spec owns and keeps-satisfiable only the **render-time component** of that measure; it is not a render-only gate. **Mechanism committed here; per-fixture numbers owned by `buiy-verification-design`.** | Pillar 3 (the hybrid handoff exists to keep render's component of this gate satisfiable); README § 5 #4 |
| **#15 Memory leak** | 4 (device) | RSS slope < 1 MB/min after warmup and atlas-entry count returns within ε of baseline after a ~10-min scripted fixture. **Mechanism committed; numbers owned by `buiy-verification-design`.** | [atlas-and-text-seam.md](atlas-and-text-seam.md), [effect-compositor.md](effect-compositor.md) (RT pooling) |
| **#1 Unit tests** | 1, 2 | The smoke/registration and CPU-math layers above run as ordinary `cargo test`. | Pillars 2, 3, 4 (registration + packing) |

Two clarifications the table compresses:

- **#10 is a layout/picking-time gate render aligns to, not a render-owned
  gate.** It checks resolved hit-rect geometry at picking time; render does not
  own it and does not get a separate measurement. Render aligns to it for free
  because pillar 1 fixes hit-test order = paint order reversed and the single
  `ClipRect` is read by *both* render and picking
  ([README § 3.2](README.md#32-render-owned-this-spec-introduces)). So gate #10's
  hit-rect check operates on exactly the geometry render paints; the two cannot
  diverge because there is one `ClipRect`, not two.
- **#14 / #15 are mechanism-only here.** This spec defines atlas
  eviction/warmup ([atlas-and-text-seam.md](atlas-and-text-seam.md)), effect-RT
  pooling ([effect-compositor.md](effect-compositor.md)), and the persistent-buffer
  handoff (pillar 3) **so that the gates are satisfiable** — i.e. so there is a
  steady state to regress against. Note also that #14 is the *combined*
  layout + render + a11y per-frame measure: its ±10% slack and per-fixture budget
  belong to the whole gate, and this spec owns only render's *component* of it,
  never a render-only gate. The pass/fail thresholds are
  [README § 5](README.md#5-open-questions) open question #4, owned by
  `buiy-verification-design`. Do not encode threshold numbers in this spec.

Gates #3, #4, #6, #7, #8, #9, #12, #13 are not render gates (a11y trees,
input replay, WCAG SCs, contrast linter, property tests, hot-reload); #9
(contrast linter) and #13 (hot-reload, "no atlas leaks") touch render data but
are owned by `buiy-verification-design` and the asset pipeline respectively.
This spec contributes the atlas-leak *predicate* #13 depends on
([atlas-and-text-seam.md](atlas-and-text-seam.md)).

---

## 4. The e2e golden-image harness (gate #2)

Gate #2 is the only proof of pixels, so its reliability is load-bearing. The
harness has four committed properties.

### 4.1 Canonical CI GPU class

Goldens are captured and diffed on a **single canonical CI GPU class** — one
self-hosted runner profile, one driver, one wgpu backend — never the
heterogeneous PR matrix. Pixel output is not bit-identical across GPUs/drivers,
so a single class is what makes a stored golden meaningful. (This mirrors the
"per-platform goldens on a single canonical CI GPU class" decision in
[foundation gate #2](../2026-05-07-buiy-foundation/verification.md#315-verification-pipeline).)
Buiy's own widget tests render via Bevy's screenshot system into an offscreen
target on this runner.

### 4.2 Perceptual diff with an explicit tolerance budget

Comparison is **perceptual**, not exact byte equality — a per-pixel
perceptual-difference metric with an explicit, per-fixture tolerance budget
(owned by `buiy-verification-design`; this spec commits to *having* a budget,
not its value). Exact equality is rejected because even on a fixed GPU,
floating-point rounding in the SDF and in linear→sRGB encoding produces
sub-LSB jitter that is invisible but not bit-stable. The budget is the line
between "jitter" and "regression."

### 4.3 The flake-mitigation triad

Three sources of nondeterminism are pinned before any frame is captured
(matching [foundation gate #2](../2026-05-07-buiy-foundation/verification.md#315-verification-pipeline)):

1. **Fixed clock.** Time is driven by a fixed/virtual clock, not wall time, so
   any time-dependent visual (a focus ring's animated phase, a transition's
   midpoint) is captured at a deterministic instant. Animation is owned by
   `buiy-animation-design`; the harness only fixes the clock the render reads.
2. **Font-load sync.** The capture blocks until every font the fixture
   references is fully loaded and its glyphs are resident — a half-loaded font
   paints fallback or blank glyphs and flips the diff. (Glyph shaping is owned
   by `buiy-text-rendering-design`; the harness consumes its load-complete
   signal via the shared `TextureAtlas`, [atlas-and-text-seam.md](atlas-and-text-seam.md).)
3. **Atlas warmup.** The texture atlas ([atlas-and-text-seam.md](atlas-and-text-seam.md))
   is warmed — glyphs/icons/gradients the fixture needs are uploaded — before
   capture, so first-frame upload latency and lazy-allocation ordering do not
   perturb the image. Warmup also establishes the steady-state baseline gate #15
   measures against.

These three are *necessary together*: each removes one independent flake source,
and a golden captured without all three is not reproducible.

### 4.4 Human-curated `--accept` workflow

A failing #2 means one of two things: a regression (fix the code) or an
**intentional** visual change (update the golden). Golden updates go through a
human-reviewed `--accept` workflow, never an automatic overwrite. This is the
one place the foundation's "no human approval gate" CI policy is explicitly
narrowed: the *test outcome* gate has no human approval (green = mergeable), but
*updating a golden* is a curated PR-review step
([foundation gate #2 + CI policy](../2026-05-07-buiy-foundation/verification.md#315-verification-pipeline)).
Without this, a render regression could be laundered into the baseline by a
blanket re-accept; the curation step is what keeps the golden trustworthy.

### 4.5 Per-window

Gate #2, like all applicable gates, runs **per-window** where multi-window
fixtures exist (foundation § Multi-window verification): top-layer compositing
and node-group ownership are per-window by construction
([README goal #4](README.md#1-goals-and-non-goals) / [pillar 2](README.md#2-architectural-pillars-one-line-summaries) / [architecture.md § 4](architecture.md#4-per-window-node-group-ownership-cross-cutting-318-f-tier)),
so multi-window goldens verify each window's stack independently. The v1
single-global-`TopLayerActivation` simplification ([README § 5](README.md#5-open-questions)
open question #1) bounds what these fixtures can assert about per-window
top-layer routing until `buiy-window-and-surface-design` lands true routing.

---

## 5. What each pillar's claim rests on

A reverse index — for each [README § 2](README.md#2-architectural-pillars-one-line-summaries)
pillar, the gate/layer that proves it cannot have silently broken.

| Pillar | Claim | Proven by |
|---|---|---|
| 1 — immutable consumer | paint order = hit-test order reversed; geometry never recomputed | Gate #10 — a layout/picking-time geometric gate render aligns to (one `ClipRect`, one order, shared by render + picking), not render-owned; plus the layer-1 graph-shape test that render reads `painters_z` forward |
| 2 — typed-primitive node + top-layer pass | node group + composite pass registered in `Core2d` after `StartMainPassPostProcessing`, with the load-bearing `BuiyRenderLabel → Tonemapping` edge | Layer 1 (graph node + edges); gate #2 (the pass actually composites) |
| 3 — hybrid handoff | persistent buffers + view uniform pack correctly; no per-frame realloc waste | Layer 2 (stride-vs-descriptor + view-uniform math); gate #14 (the handoff keeps render time in budget) |
| 4 — `ClipRect` in render-prep | clip computed from layout output, testable as resolved values | Gate #5 (layout snapshots over `ClipRect`); layer 1 (the pass is scheduled in render-prep) |
| 5 — layout owns transform tree | render reads `GlobalTransform`; no re-implemented propagation | Bevy's own `TransformSystems::Propagate` tests + a Buiy bridge test that `ResolvedLayout` + `ResolvedTransform` compose into the expected `Transform` (layer 2); gate #2 confirms the painted result |
| 6 — off-screen effect compositor | group opacity + isolation composite correctly via off-screen RT | Gate #2 (overlapping-children group-opacity golden — the exact case the forward-pass approximation was rejected for); gate #15 (RT pool returns to baseline) |
| 7 — no property trees | render paints current frame's resolved values; relies on ECS change-detection | No dedicated gate — this is an *absence*; the revisit trigger (animating opacity/transform without re-running layout) is what would add one. Until then, layer-2/3 tests over resolved values are the whole proof. |

Pillar 6's row is the sharpest tie between a gate and a settled design decision:
the user rejected a v1 forward-pass approximation of group opacity *because it is
subtly wrong over overlapping children*
([README § 2 pillar 6](README.md#2-architectural-pillars-one-line-summaries)).
Gate #2 must therefore include a fixture with overlapping children under a group
`Opacity < 1` and an `isolation` boundary — that golden is the standing proof the
correct off-screen-composite path stayed correct.

A wording precision on this row's "isolation": **isolation is the layout-owned
`Stacking.isolation` FIELD, not a render SC-trigger component.** The render-owned
SC-trigger component count is **THREE** — `Opacity` / `Filter` / `MixBlendMode`
([effect-compositor.md § 1](effect-compositor.md), [component-model.md](component-model.md));
isolation rides the layout `Stacking` bundle's `isolation` field, and the
compositor reads that field (it does not introduce a fourth render-side trigger
component). The pillar-6 golden still exercises the isolation boundary, but the
boundary is materialized from layout's `Stacking.isolation`, not from a render
component this spec defines.

---

## 6. Verification

Concretely, "render verification works" means:

- **Headless (every PR, `xvfb-run`, no adapter):** layers 1–3 are ordinary
  `cargo test --workspace`. Registration, instance-packing/stride, sRGB→linear,
  coordinate/SDF math, and `ClipRect` snapshots all pass with no device. The
  Phase-0 suites [`render_smoke.rs`](../../../crates/buiy_core/tests/render_smoke.rs)
  and [`render_instance.rs`](../../../crates/buiy_core/tests/render_instance.rs)
  are the seed; each new primitive/component extends layer 2 with its own
  stride + SDF tests, and each new clip input extends gate #5.
- **GPU (e2e runner, canonical CI GPU class):** the `#[ignore]`-gated
  registration tests plus gate #2 goldens (with the flake triad), gate #11(b)
  forced-colors goldens, gate #14 render-time, and gate #15 leak run. These are
  the only tests that touch the device.
- **The boundary is enforced, not conventional:** a GPU-needing test that is not
  `#[ignore]`-marked will panic the headless CI run at adapter init — so the
  marker is load-bearing and the no-adapter constraint is self-policing. The
  layer split is not a guideline; it is what keeps the PR suite green without a
  GPU.

A render property that *can only* be proven by pixels lives at gate #2 behind
the `--accept` curation; everything else is proven headless at the layer that
can see it. That division is the whole answer to "how do we verify a GPU
renderer on runners with no GPU."

---

*Target state as of 2026-06-03. Mechanism owned here; per-fixture tolerance,
perf, and leak numbers owned by `buiy-verification-design` ([README § 5](README.md#5-open-questions) #4).*
