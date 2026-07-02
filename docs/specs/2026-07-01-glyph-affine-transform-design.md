# Glyph/Icon affine transform + transform-origin — design

**Status:** proposed (2026-07-01)
**Area:** render pipeline (coverage/glyph path) + layout sub-pass 6e (transform composition)
**Origin:** the 2026-07-01 gallery widget audit — the Showcase disclosure chevron
never rotates on expand. Root-caused to two stacked, pre-existing defects.
**Supersedes / resolves:** `docs/plans/follow-ups.md` → "Render — `UiTransform`
paint …" **Residual A** (transform-origin never honored) and closes the
glyph/icon leg of the R1 affine-paint work.

## 1. Problem

The disclosure chevron (an `Icon`) and the core `Disclosure` caret (a text glyph
`▶`) are given a `Rotate` longhand on expand, but they never rotate on screen —
only their colour changes. Two independent defects stack:

1. **The glyph/icon coverage render path drops the 2×2 affine entirely.** The
   box/quad path threads the composed affine (`extracted_node_for` reads
   `GlobalTransform.affine().matrix3`; `PackedInstance.affine:[f32;4]`;
   `shader.wgsl` multiplies it). The **coverage** path (text glyphs + icons, both
   `GlyphAlphaInstance` on the one coverage pipeline) keeps only
   `gt.translation()` — `icon_producer.rs:303-305`, `text/extract.rs:450` — and
   `coverage.wgsl` builds an axis-aligned quad. So `Rotate`/`Scale` on any text or
   icon entity is silently discarded at paint. (Verified in the audit: colour
   updates via `Changed<Icon>`, rotation never applies.)

2. **`transform-origin` is never honored (the pivot bug).** `compose_transform`
   (layout sub-pass 6e, `layout/systems.rs`) builds `M = t·r·s·m_transform` and
   never reads `ui.origin`. `TransformOrigin` defaults to `50% 50%` (center) and
   is present on every `Node` (via `#[require(UiTransform)]`), but is ignored, so
   the composed matrix's fixed point is the box-local **top-left**. Even once
   defect 1 is fixed, a −90° chevron would rotate about its box corner and
   **swing ~one box-width away** instead of spinning in place.

Both are missed by the current suite: interaction tests assert ECS state, not
pixels; the only committed glyph golden is the unrotated `text-ahem`; there is no
rotate/scale fixture anywhere in `buiy_verify`.

## 2. Goals / non-goals

**Goals**
- Text glyphs and icons paint their composed `Rotate`/`Scale` (rigid-body about
  the element's transform-origin), matching the box/quad path and CSS semantics.
- `transform-origin` honored at 6e (default center), so render **==** picking by
  construction (both consume the same `ResolvedTransform`/`GlobalTransform`).
- The unrotated corpus stays byte-identical (identity fast-path).
- WebGL2 stays ≤16 vertex attributes.

**Non-goals (explicit deferrals, logged in follow-ups)**
- **Text-quad decorations** — **underline / overline / selection / preedit** →
  the separate `PackedInstance` text-quad carrier (`pack_text_quad`,
  `render/instance.rs:149-165`), which already *has* an affine slot but packs
  `IDENTITY_AFFINE`. Same root cause, different carrier, **no current consumer
  rotates a decorated text run** (verified: the only rotated text is the caret
  glyph `▶`, undecorated; the only `TextDecorations` user is the todo strikethrough,
  never rotated). Deferred; noted as a follow-up. **Note:** *strike/line-through
  is NOT in this deferral* — it is emitted as a **coverage-carrier
  `GlyphAlphaInstance` solid stamp** (`text/extract.rs:690-712`), so it rides the
  glyph fix and rotates in v1.
- **Hinted glyph re-rasterization at `Scale`** — v1 stretches the coverage cell
  (matches the box path + web behavior). A crisper rescale is a later concern.
- perspective / `Preserve3d` / backface / skew — unchanged (already C-tier).

## 3. Design

Three coordinated changes; the render change is inert until the producers feed a
non-identity affine, and the whole thing is a no-op under identity.

### 3.1 Layout 6e — bake transform-origin into `ResolvedTransform` (the pivot)

In `compose_transform`, resolve `ui.origin` to px against the current-frame box
(`box_size`, already read there) — default center → `O = box_size/2` — and
conjugate the product:

```
M = Translate(O) · (t·r·s·m_transform) · Translate(-O)
```

- **Identity-safe (bit-exact):** `Translate(O)·I·Translate(-O)` has linear part
  `I` and translation `O + (−O) = 0` (exact in IEEE), so `M == Mat4::IDENTITY`
  holds bit-exactly and the existing `m == Mat4::IDENTITY` gate (and the "remove
  stale `ResolvedTransform`") is unchanged.
- **Translate-only unchanged (for representable values):** translation commutes
  with the origin conjugation; the translation column is computed `(O + v) − O`,
  which equals `v` for the half-pixel/integer values in use (not a *universal*
  byte guarantee, but exact for every current case).
- **Only rotate/scale gain the center pivot.** `GlobalTransform` then carries the
  pivot compensation `(I − L)·O` in its translation column, applied once.
- **Spawn-frame pop (benign):** an element rotated *at spawn* would use `O = 0`
  for the first frame (the ZERO-box Taffy fallback, `systems.rs:4619-4628`) then
  self-heal to center next frame — mirrors the existing percent-translate
  self-heal. Harmless for the caret/chevron (they rotate only *after* an expand,
  well past spawn).
- Update the spec formula in
  `docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md §1/§1.1`
  to the origin-conjugated form (it currently omits it).

**Picking safety (corrected — load-bearing).** Picking does NOT invert the affine:
`emit_picks`/`hit_test` are a **translation-anchored AABB** over the *unrotated*
`layout.size` (`picking/backend.rs:145-155`, `picking/mod.rs:149-154,190-193`; no
inverse/`transform_point` in the module). So render already ≠ picking for rotated
boxes *today*, and baking the pivot **shifts** a rotated element's pick AABB by ~a
box dimension. This is safe **only because every rotated/scaled element is
`Pickable::IGNORE`** — verified census: meter fill (`composites.rs:213`),
disclosure caret (`disclosure.rs:252`), showcase chevron (via `icon_box`,
`lib.rs:401`); translate-only thumbs commute (AABB unmoved). A hit over such an
element falls through to its unrotated pickable ancestor (the disclosure header /
widget root) — the intended target anyway. **Future guard:** a *pickable* rotated
element (e.g. a rotated drag handle) would pick wrong — a pre-existing gap
(picking never modeled rotation), not introduced here; logged as a follow-up. A
one-line comment at the 6e change records this invariant.

### 3.2 Render coverage path — carry + apply the affine (mirror the band fold)

- `GlyphAlphaInstance` (`render/atlas/primitive.rs`): append `affine:[f32;4]`
  **after `page`** (the last field, offset 68) → stride **68→84**, align 4, no
  padding. Bump `GLYPH_ALPHA_INSTANCE_STRIDE_BYTES` and **both** size const-asserts
  — the auto-following `size == STRIDE` one *and* the literal `size == 4*4*4+4`
  one (→ `4*4*4 + 4 + 4*4` = 84, message "5 vec4 + u32"). `GLYPH_ALPHA_FLOAT_OFFSET
  = 11` is **preserved** (appended after `page`, so `color[3]` stays at float idx
  11) → the R2 degraded-group re-tint (typed `.color[3]`) is unaffected.
- Vertex layout (`glyph_vertex_buffers`, **`render/primitive.rs:168`** — *not*
  `prepare.rs`, which only holds the auto-sizing `RawBufferVec` and needs no
  change): `array_stride 68→84`; one `VertexAttribute { Float32x4, offset 68,
  shader_location 7 }`. Also switch the hard-coded `68` literal at
  `render/primitive.rs:187` to `GLYPH_ALPHA_INSTANCE_STRIDE_BYTES`. Glyph pipeline
  goes **7→8 attrs** (WebGPU + WebGL2 both cap at 16) — **no fold required**; the
  single-`vec4` form *is* the band's fold. Add a `band_attr_cap`-style regression
  test (in `render/primitive.rs`) asserting the coverage layout stays ≤16 attrs /
  max location ≤15.
- `coverage.wgsl`: add `@location(7) affine: vec4<f32>` and multiply, exactly like
  `shader.wgsl:53-54` / `band.wgsl:76`:
  ```wgsl
  let logical = i.rect.xy + mat2x2<f32>(i.affine.xy, i.affine.zw) * (v.uv * i.rect.zw);
  ```
  Fragment shader unchanged (`frag_pos = logical` already). Identity `[1,0,0,1]`
  ⇒ `logical = rect.xy + v.uv*size` = today, byte-for-byte.

### 3.3 Producers — emit the pre-pivoted origin + the affine

The **pivot is handled CPU-side via `transform_point`**, so the shader stays the
plain `rect_pos + affine*local` form (no separate pivot attribute — the chosen
architecture; see §4). For each emitted glyph/icon instance:

- `affine` = `gt.affine().matrix3` xy columns (same code as `extract.rs:510-511`).
- `rect.xy` = `gt.transform_point(box_local_topleft.extend(0.0)).truncate()`
  **instead of** `gt.translation() + offset`, where `box_local_topleft` is the
  glyph/icon's box-local origin:
  - **icons** (`icon_producer.rs`): `(layout.size - icon_size) * 0.5` (the
    existing centering offset). Icon atlas keys are position-independent, so this
    is bit-identical under identity (`transform_point(offset) == translation()+offset`).
  - **text glyphs** (`text/extract.rs`, sites: `emit_glyph` `:984`, strike stamp
    `:702`, caret primary/secondary `:786`/`:799`): here the subpixel **atlas bin**
    (cosmic-text `CacheKey`) depends on the *fractional part* of the FULL-origin
    physical offset. **Do NOT recompute `physical()` with translation dropped** —
    that shifts the bin and breaks the `text-ahem` golden for any non-integer box
    position. Instead: keep the existing full-origin `physical()` call unchanged
    (binning identical to today), then derive the box-local top-left by
    **subtracting the translation from the already-computed window rect**:
    `box_local_tl = rect_window.xy − gt.translation().truncate()`. At UI
    magnitudes `|rect| ≈ |translation|` so the subtraction is Sterbenz-exact, and
    the identity roundtrip `(rect − t) + t` is bit-exact. `emit_glyph`'s signature
    must grow to receive `gt` (and the precomputed affine); today it takes only
    `phys/color/clip`.

Because `transform_point(tl) + affine·(v.uv·size) = transform_point(tl + v.uv·size)`,
every corner is the fully-transformed box-local corner → the whole run/icon
rotates rigidly about the (6e-baked) transform-origin. Under identity,
`transform_point(offset) == translation()+offset` → unchanged. (Crux algebra
independently verified: translation enters exactly once, no double-apply.)

### 3.4 Preserve the meter fill (the one `Scale` consumer that wants a corner)

`buiy_widgets::composites::meter` grows its fill with `Scale(pct,1,1)` and
**relies on top-left/left-edge anchoring** ("grows from the LEFT edge"). Once 6e
honors the default center origin, a center pivot would grow it from the middle —
a regression. Fix: set the fill's `TransformOrigin` to the **left edge**
(`x = 0%`; y irrelevant since `scale.y == 1`). This reproduces today's left-anchored
fill exactly and updates the meter's doc comment to say so.

## 4. Alternatives considered

- **Icon-swap band-aid** (render a down-chevron path when open, no transform):
  rejected — leaves the framework root cause (all rotated glyphs/icons) and the
  core `Disclosure` caret broken; violates the project's no-band-aid rule.
- **Producer-only center pivot for icons** (bake the center pivot in
  `icon_producer`, leave 6e alone): rejected — a render-only special case that
  fixes neither the core caret nor the box/quad path, and hides the pivot from the
  one place (`ResolvedTransform`) every downstream consumer already reads. 6e is
  the correct single layer because both the box/quad extract and the coverage
  producers derive from `GlobalTransform`. (Note: this rejection is *not* about
  render==picking — picking never modeled rotation; see §3.1 picking-safety.)
- **Shader-side pivot attribute** (carry `affine` + a separate `pivot` vec2;
  `logical = pivot + affine*(corner - pivot)`): correct, but adds an attribute and
  a shader form that diverges from the quad/band. **Chosen instead:** CPU
  `transform_point` (§3.3) — 1 attribute, shader identical to quad/band, and the
  pivot lives in 6e where picking already consumes it.
- **Global default pivot flip to center without per-element origin**: rejected —
  breaks the meter's left-anchored scale; per-element `transform-origin` (which
  already exists on `UiTransform`) is the right mechanism.

## 5. Blast radius & verification

**Touched:** `layout/systems.rs` (6e), `render/atlas/primitive.rs` (struct +
both const-asserts), `render/primitive.rs:168/187` (glyph vertex layout — the real
file; `prepare.rs` needs **no** change), `render/coverage.wgsl`,
`render/icon_producer.rs`, `render/text/extract.rs` (incl. `emit_glyph` signature),
`buiy_widgets/src/composites.rs` (meter origin **+ the `:206-211` comment**), the
layout transform spec. `GlyphAlphaInstance` constructors → identity affine: **5
production** (`icon_producer.rs:321`; `text/extract.rs:702/786/799/984`) **+ test
sites**, including two hard breaks — `tests/crosscut/atlas_primitive.rs:10`
(`size_of == 68` → `84`) and `tests/render/render_compositor.rs:561`
(`cast_ref::<_, [f32;17]>` → `[f32;21]`).

**Verification (lowest tier that observes each leg; RED before, GREEN after):**

1. **Tier 2 — headless (primary gate, every-PR):**
   - Extend `tests/support/extract_harness.rs`; assert a rotated/scaled **text**
     entity's emitted `GlyphAlphaInstance.affine == gt.affine().matrix3` (not
     identity) and that `rect.xy == transform_point(origin)`.
   - Add a small adapterless **icon** extract harness (analogue) and the same
     assertion for a rotated/scaled `Icon`.
   - Identity byte-stability: an unrotated entity's packed glyph buffer is
     unchanged vs. today.
   - `compose_transform` origin: unit-assert `M` conjugation (center default;
     translate-only unchanged; identity unchanged).
2. **Tier 4/5 — GPU `#[ignore]` (local RX 6700 XT + CI GPU lane):**
   - Extend `render_transform_paint_gpu.rs` with off-axis pixel probes for a
     rotated **glyph** (asymmetric multi-glyph Ahem or scale-x, never a 90°
     single-em no-op) and a rotated **icon** (the asymmetric chevron).
   - **Recalibrate BOTH existing box probes** to the new center pivot:
     `rotated_fill_paints_off_axis` (`:100-167`, scanned `y≈32` near the old
     corner pivot → moves to ≈`(34,47)`) **and** `scaled_fill_paints_beyond_unscaled_box`
     (`:22-90`, samples `(30,30)`; under center pivot the 2× box becomes `[11,31)`
     so `(30,30)` is 1px from the rim → re-pick a mid-fill sample).
   - Optional mismatch reftest: `rotated_text != unrotated_text` (fuzz floor 0).
3. **Re-verify:** the `transform_roundtrips` invariant (feeds `box=ZERO` → pivot
   never engages, stays green; note the `mat4_is_pure_scale` predicate would reject
   a center-pivot scale with a non-zero box — a landmine if anyone later feeds real
   box sizes, worth a comment); `layout_transforms.rs` (translate/position-only →
   survives); the `text-ahem` golden on **pinned lavapipe** (identity fast-path +
   preserved subpixel binning should keep it byte-identical; if a sub-pixel rim
   shifts, re-bless with a documented `FuzzBudget`/`BlessLedger` reason — never
   widen silently). Picking is unaffected (all rotated elements `Pickable::IGNORE`;
   see §3.1) — a smoke check that the disclosure header still toggles on click
   suffices.
4. **End-to-end:** re-render the gallery Showcase (RX 6700 XT) and confirm the
   chevron spins to point down on expand and the meter fill still grows from the
   left.

## 6. Rollout

Single feature branch, one PR ("fix the two audit findings"): the trivial stepper
fix (already landed on the branch) + this glyph/icon-affine + 6e origin work, in
gated waves (each RED→GREEN, committed separately). CI runs the headless gate +
both GPU legs. Merge when green.
