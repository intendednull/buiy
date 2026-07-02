# Plan — glyph/icon affine transform + transform-origin

Implements `docs/specs/2026-07-01-glyph-affine-transform-design.md` (gated by two
fresh-context reviews). Branch `fix/gallery-stepper-and-glyph-affine` (the stepper
fix already landed at `9d8945f`). Each wave: RED→GREEN, its own commit, gated.

Shared-target build: `env CARGO_TARGET_DIR=/mnt/storage/projects/buiy/target`
(the worktree reuses the warm bevy artifacts).

## Wave order & dependency

`W1 (6e origin + meter)` and `W2 (carrier, identity)` are independent and each
tree-green on their own; `W3 (producers)` needs both (the pivot from W1, the
carrier from W2) and is where rotation actually paints. Then `W-verify`.

---

## W1 — 6e transform-origin (center pivot) + preserve the meter fill

**Goal:** `compose_transform` honors `ui.origin` (default center); the meter fill
opts back into left-edge so its left-anchored `Scale` is byte-identical.

1. `layout/systems.rs` `compose_transform`: resolve
   `O = (resolve(origin.x, box_size.x), resolve(origin.y, box_size.y), 0)`
   (default `50% 50%` → `box_size/2`) and return
   `Translate(O) · (t·r·s·m_transform) · Translate(-O)`. Keep the
   `m == IDENTITY` gate + stale-removal (identity is bit-exact). Add a comment:
   the pivot is baked here so every downstream consumer (box extract + coverage
   producers) gets it; picking is a translation-AABB that never modeled rotation
   and is safe only because rotated elements are `Pickable::IGNORE`.
2. `buiy_widgets/src/composites.rs` `meter()`: set the fill's `TransformOrigin`
   to the left edge (`x = 0%`) and rewrite the `:206-211` comment (which currently
   claims "no transform-origin needed / corner pivot is what we want").
3. Spec formula: update
   `docs/specs/2026-05-08-buiy-layout-design/transforms-and-containment.md §1/§1.1`
   to the origin-conjugated form.
4. **Tests (headless, RED→GREEN):** unit-assert `compose_transform` with a
   non-zero box — rotate about center (translation column = `(I−L)·O`), scale about
   center; identity → `IDENTITY`; translate-only unchanged. Add a note/comment at
   the `mat4_is_pure_scale` invariant predicate (box=ZERO today; a non-zero box +
   center-pivot scale would leak a translation column).
5. **Verify:** headless workspace gate green. Meter render byte-identical (checked
   in W-verify against a pre-change capture).

**Commit:** `fix(layout): honor transform-origin in 6e (center pivot) + pin meter fill left`

---

## W2 — glyph/icon affine carrier + coverage shader (identity, byte-stable)

**Goal:** the coverage carrier + shader can apply an affine; nothing feeds a
non-identity one yet, so output is byte-identical.

1. `render/atlas/primitive.rs`: append `affine:[f32;4]` after `page` (offset 68,
   stride 84). Bump `GLYPH_ALPHA_INSTANCE_STRIDE_BYTES`; update **both** size
   const-asserts (auto `size==STRIDE` + literal `4*4*4+4` → `+4*4`, msg "5 vec4 +
   u32"). `GLYPH_ALPHA_FLOAT_OFFSET = 11` unchanged.
2. `render/primitive.rs:168` `glyph_vertex_buffers`: `array_stride 68→84` (use the
   const, replace the `:187` literal `68`); add `VertexAttribute { Float32x4, 68,
   loc 7 }`; update the doc-table.
3. `render/coverage.wgsl`: add `@location(7) affine: vec4<f32>`; vertex build →
   `let logical = i.rect.xy + mat2x2<f32>(i.affine.xy, i.affine.zw) * (v.uv*i.rect.zw);`
   Fragment unchanged.
4. All `GlyphAlphaInstance` constructors → `affine: IDENTITY` (`[1,0,0,1]`):
   `icon_producer.rs:321`, `text/extract.rs:702/786/799/984`, and the test sites.
   Fix the two hard breaks: `tests/crosscut/atlas_primitive.rs:10` (`68→84`),
   `tests/render/render_compositor.rs:561` (`[f32;17]→[f32;21]`).
5. **Test (RED→GREEN):** add a `band_attr_cap`-style test in `render/primitive.rs`
   asserting the coverage layout ≤16 attrs / max loc ≤15.
6. **Verify:** headless gate green; identity byte-stability (an unrotated glyph
   buffer unchanged vs. today — assert in a small extract test).

**Commit:** `feat(render): affine slot on the glyph/icon coverage carrier + shader (identity no-op)`

---

## W3 — producers feed the affine (rotation paints) + Tier-2 gate

**Goal:** icons + text glyphs emit the composed affine about the entity origin →
the chevron/caret rotate about center.

1. `render/icon_producer.rs`: `affine = gt.affine().matrix3` cols;
   `rect.xy = gt.transform_point(((layout.size - icon_size)*0.5).extend(0)).truncate()`.
2. `render/text/extract.rs`: keep the full-origin `physical()` (binning preserved);
   derive `box_local_tl = rect_window.xy − gt.translation().truncate()`; emit
   `gt.transform_point(box_local_tl.extend(0)).truncate()` + the affine. Grow
   `emit_glyph`'s signature to take `gt` + affine; apply at `emit_glyph` + the
   strike/caret stamp sites (`:702/786/799`).
3. **Tests (Tier-2 headless, RED→GREEN — the primary gate):**
   - Extend `tests/support/extract_harness.rs`; assert a rotated (θ=0.5rad) and a
     scaled (2×) **text** entity's emitted glyph `affine == gt.affine().matrix3`
     (not identity) and `rect.xy == transform_point(origin)`.
   - Add a small adapterless **icon** extract harness; same assertions for a
     rotated/scaled `Icon`.
   - Identity byte-stability reaffirmed (unrotated unchanged).
4. **Verify:** headless workspace gate green.

**Commit:** `fix(render): thread the composed affine through the glyph/icon producers — rotation paints`

---

## W-verify — GPU lane, goldens, end-to-end, docs

1. **GPU `#[ignore]` (both legs, RX 6700 XT):**
   `cargo test -p buiy_core -j2 -- --ignored --test-threads=1` and
   `cargo test -p buiy_verify -j2 -- --ignored --test-threads=1`.
   - Extend `render_transform_paint_gpu.rs`: off-axis pixel probes for a rotated
     **glyph** (asymmetric) + rotated **icon** (chevron); **recalibrate BOTH**
     `rotated_fill_paints_off_axis` and `scaled_fill_paints_beyond_unscaled_box`
     to center pivot.
   - Optional mismatch reftest `rotated_text != unrotated_text` (fuzz floor 0).
   - Re-bless `text-ahem` only if lavapipe shows a sub-pixel shift (documented
     `BlessLedger` reason); expect byte-identical.
2. **End-to-end (RX 6700 XT):** rebuild the throwaway probe → the Showcase chevron
   spins to point down on expand; the meter fill still grows from the left; stepper
   reads the incremented value. (Then remove the throwaway probe bin.)
3. **Full headless gate:** `cargo fmt --all -- --check`; `cargo clippy --workspace
   --all-targets --locked -D warnings`; `RUSTDOCFLAGS=-D warnings cargo doc`;
   headless `cargo test --workspace`.
4. **Docs:** close follow-ups.md Residual A (transform-origin now honored);
   add the two new deferrals (text-quad decoration affine; rotated-*pickable*
   element picking); add the spec/plan to `docs/README.md`; note the local GPU
   results.
5. **PR + merge:** push, open PR, CI green (3-OS headless + both GPU legs + MSRV +
   web-smoke + deny), then merge (user pre-authorized "merge when ready").

---

## Gates

After **each** wave: fresh-context review (correctness + spec alignment) + the
headless gate. After W-verify: the GPU legs + the end-to-end gallery render. No
wave advances on unreviewed/unverified work.
