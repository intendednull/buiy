# Buiy Text T6: Decoration Painting — Implementation Plan

**Date:** 2026-06-10
**Status:** landed
**Spec:** [specs/2026-06-09-buiy-text-rendering-design/decoration-and-paint.md](../specs/2026-06-09-buiy-text-rendering-design/decoration-and-paint.md) §§ 2–4, 9 + [glyph-pipeline.md](../specs/2026-06-09-buiy-text-rendering-design/glyph-pipeline.md) §§ 6.2–6.3, 10 + [verification.md](../specs/2026-06-09-buiy-text-rendering-design/verification.md) §§ 1.2–1.3
**Campaign:** [plans/2026-06-09-buiy-text-campaign.md](2026-06-09-buiy-text-campaign.md) — phase T6 (depends on T4; the implementer starts from a branch with T1–T5 merged — T5 landed @ `814ff6b`)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Text decorations become visible pixels through the already-built,
GPU-verified render primitives — nothing in the render seam is redesigned.
Land the `TextDecorations` authoring component lowered into
`Attrs.text_decoration` at `TextSync` (line bits only — color stays Buiy-side,
decision 1); the pure emission function over `LayoutRun.decorations` mirroring
upstream `render_decoration`'s math 1:1 in f32 logical px with the
physical-px min-thickness rule (§ 3.3 — upstream's `.max(1.0).ceil()` is
deliberately NOT copied); underline/overline as quad-tier instances riding the
new entity-keyed `ExtractedTextQuads` carrier, spliced into the **existing**
quad instance buffer by the § 4.6 fresh-list merge contract (per-entity order
+ effect-group partition contiguity preserved, the triple quad gate
`nodes || groups || text_quads`); line-through as a solid-stamp
`GlyphAlphaInstance` emitted after the run's glyphs (paints OVER the text,
CSS Text Decoration L3 painting order); the 1×1 solid-white `CoverageR8`
stamp as a **warmup-pinned** atlas entry — the one committed
`AtlasWarmupQueue` push of this campaign, riding the render architecture
§ 1.1 finish-ordering seam through `BuiyTextPlugin`'s guarded
`register_render_world` — whose key joins the `ResidentTextKeys` touch pass
whenever a stamp instance is live; and the `DecorationLineStyle` C-tier
reservation realized as an enum whose `Solid`/`Double` arms work and whose
`Dotted`/`Dashed`/`Wavy` arms degrade warn-once (the `TextWrap::Balance`
precedent).

**Architecture:** The cosmic-text boundary holds. `buiy_core::text` gains the
authoring component (`components.rs`), the `Attrs` lowering (`sync.rs`), the
pure emission mirror (`decoration.rs`, new), and the stamp helpers
(`stamp.rs`, new); `extract_buiy_glyphs` grows the decoration walk — one run
walk emits quads AND stamps AND glyphs, under the one § 6.2 damage decision.
The render side gains exactly one new carrier type (`TextQuad` +
`ExtractedTextQuads`, defined in `render/extract.rs` beside `ExtractedNode`,
init'd by `BuiyRenderPlugin` exactly like `ExtractedGlyphs` so prepare works
without the text plugin), one new pure pack input
(`pack_view_partitioned` grows a third parameter — the § 4.6 splice), and one
new gate term in `prepare_buiy_instances`. **No new GPU buffer, pipeline,
shader, or draw branch** — quads ride the existing 52 B quad blob, stamps
ride the existing 68 B glyph blob.

**v1 slice (decoration-and-paint §§ 2–4, verbatim):** `text-decoration-line`
(underline | overline | line-through, any combination) +
`text-decoration-color` (optional `ColorToken`; absent = `currentColor`
precedence § 3.2) + the `Double` underline paint math (§ 3.2's row — the
authoring knob ships as `DecorationLineStyle::Double`, see decision 2 /
erratum 2); per-span geometry from `DecorationSpan` (mixed-size rich text
gets per-span line geometry for free when the rich-text tier lands); the
§ 3.3 physical-px min-thickness floor with y snapped to the physical grid;
the § 4.4 emission-order table seats 3–5 (seat 2 selection and seat 6 caret
are T7).

**Where T6 ends (honesty pins — named seams, not built):**

- **Selection + caret (T7).** `ExtractedTextQuads` seat 2 (selection rects
  splice BEFORE an entity's decoration quads), the
  `CaretVisual`/`SelectionVisual` probe-union members, and the stamp's caret
  consumer — the producer's § 6.2 ledger comments are updated to mark T6's
  seats landed and T7's still open.
- **`DecorationLineStyle` dotted/dashed/wavy** — enum variants exist, degrade
  to `Solid` with a warn-once (the `TextWrap::Balance` precedent). Dotted/
  dashed are pure future Buiy emission patterns (segmented quads); wavy's
  unblock path of record is **upstream-PR-first** (`UnderlineStyle`'s literal
  `// TODO: Wavy`, verified in 0.19) — the Buiy fallback must not get baked
  into the F-tier types (decoration-and-paint § 9).
- **`-thickness` / `text-underline-offset` / `-position`** — pure transforms
  of `DecorationMetrics` at emission time (§ 2.2's metrics-as-data payoff);
  no carriers, not built.
- **`text-decoration-skip-ink`** — named only (needs per-glyph ink boxes).
- **`TextDecorations` removal** is NOT a trigger-union member — like every
  other style carrier (the T2-erratum-1 precedent), a removed component
  resyncs on the next *other* trigger; a stale decoration can persist until
  then. Documented in the sync ledger, not special-cased.
- **A color-only `TextDecorations` edit pays one reshape** — the TextSync
  union is component-granular, and the line bits live in `Attrs`, so the sync
  re-applies the lazy setters even when only `color` changed. Accepted:
  decoration edits are rare, and splitting the component contradicts the
  spec's single-component shape.
- **Glyph stamps in effect groups (T8).** Underline/overline quads ride
  `pack_view_partitioned`, so they DIM inside an `Opacity(0.5)` group;
  line-through stamps ride the flat glyph draw and do NOT — the § 4.5
  asymmetry, pinned by a GPU test here and flipped by T8's glyph-buffer
  partition. No editable-text-in-effect-group fixture may be claimed correct
  before T8.
- **Cross-node layering** — all quads still draw before all glyphs globally
  (glyph-pipeline § 11.3); within-node decoration order is correct by
  primitive rank + § 4.4 emission order; cross-node overlap artifacts remain
  the render paint-order phase's dependency (same caveat as T4).

**Tech stack:** cosmic-text 0.19.0 (default features — no change), `bitflags`
2.11.1 (already a direct `buiy_core` dependency — `ContainFlags`/
`EffectReason` precedent), `smallvec` (workspace dep — `AtlasKey` precedent).
**No new dependencies** — if a task appears to need one, STOP: that
contradicts the charter. (`cargo deny check` is not required: no dep
changes.)

**Test reality:** the component, the lowering, the emission math, the carrier
retention, the pack splice, and the stamp warmup/touch mechanics are all
headless (the pack is a pure function; the producer runs on the adapterless
extract harness; `BuiyAtlas` is device-free by design). The GPU lane carries
one new file: four decoration goldens (underline, double, overline,
line-through-over-glyphs) plus the gate-term regressions and the § 4.5
asymmetry pin. Every GPU test keeps `#[ignore]` and builds on
`tests/support/mod.rs`.

---

## The gate (run BOTH lanes at every task boundary)

T6 ships render-world prepare/pack changes and GPU tests, so the per-task
gate is the headless gate **plus the GPU lane** (this host has the RX 6700
XT / RADV; Vulkan render-to-texture needs no display):

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  xvfb-run -a cargo test --workspace -j 2
```

```sh
cargo test -p buiy_core -j 2 -- --ignored --test-threads=1
```

Expected: both green. The headless gate must stay green **independently** (CI
has no adapter); the GPU lane is additive and must pass on this host before
the phase merges. A GPU-needing test without `#[ignore]` panics the headless
run at adapter init — self-policing.

---

## Orientation: verified facts this plan builds on

Source-verified against the vendored crate
(`~/.cargo/registry/src/*/cosmic-text-0.19.0/`) and the as-built T1–T5 tree
at `814ff6b`. **The emission math below is THE normative mirror target — the
exact-number tests in Task 2 pin it.**

### 1. The cosmic-text 0.19 decoration surface (attrs.rs, layout.rs)

```rust
// attrs.rs:227–243
pub enum UnderlineStyle { #[default] None, Single, Double /* TODO: Wavy */ }
pub struct TextDecoration {            // Eq + Hash + Default, all fields pub
    pub underline: UnderlineStyle,
    pub underline_color_opt: Option<Color>,
    pub strikethrough: bool,
    pub strikethrough_color_opt: Option<Color>,
    pub overline: bool,
    pub overline_color_opt: Option<Color>,
}
// attrs.rs:262–281 — EM units; all fields pub, hand-constructible in tests
pub struct DecorationMetrics { pub offset: f32, pub thickness: f32 }
pub struct GlyphDecorationData {
    pub text_decoration: TextDecoration,
    pub underline_metrics: DecorationMetrics,
    pub strikethrough_metrics: DecorationMetrics,
    pub ascent: f32,                   // ascent / upem — overline positioning
}
// layout.rs:67–77 — all fields pub
pub struct DecorationSpan {
    pub glyph_range: Range<usize>,     // indices into LayoutLine::glyphs
    pub data: GlyphDecorationData,
    pub color_opt: Option<Color>,      // "Fallback color from the FIRST GLYPH's color_opt"
    pub font_size: f32,                // first glyph's — scales the EM metrics
}
```

Builders on `Attrs` (attrs.rs:382–409): `.underline(UnderlineStyle)`,
`.underline_color(Color)`, `.strikethrough()`, `.strikethrough_color(Color)`,
`.overline()`, `.overline_color(Color)`. None of the types is
`#[non_exhaustive]` — Task 2's fixtures hand-build `GlyphDecorationData`.

**`LayoutRun.glyphs` and `glyph_range` index the same slice** — verified at
buffer.rs:271–281: `LayoutRunIter` yields one run per `LayoutLine` with
`glyphs: &layout_line.glyphs, decorations: &layout_line.decorations`, so a
span's `glyph_range` indexes `run.glyphs` directly. `LayoutRun` carries
`line_y` (baseline), `line_top`, `line_height`, `line_w`, `rtl`.

### 2. Span production (shape.rs:1098–1180, 2842–2985)

- `has_decoration()` **gates span creation**: `decoration_spans` are built
  during shaping only when the line's default attrs OR any attrs span has a
  decoration. Adding/removing the line bits therefore **changes shaping
  output** — `TextDecorations` must be a TextSync trigger (Task 1).
- Metrics come from `decoration_metrics(font)` (shape.rs:686–705): the
  font's `post` underline / `OS/2` strikeout tables normalized by upem, with
  fallbacks when a table is absent — underline offset **−0.125 em**,
  strikeout offset **0.3 em**, thickness **1/14 em**, and `(default,
  default, 0.0)` when upem is 0.
- During layout, adjacent glyphs with **equal `GlyphDecorationData`** merge
  into one `DecorationSpan` (shape.rs:2960–2975); `span.color_opt` is copied
  from the first glyph's `color_opt`, `span.font_size` from the first
  glyph's `font_size`.

### 3. `render_decoration` — the math the pure function mirrors (render.rs)

Per span, with `glyphs = &run.glyphs[span.glyph_range]`, `fs = span.font_size`,
`m = span.data`:

- **X extent:** `x_min = min(g.x)`, `x_max = max(g.x + g.w)` over the span's
  glyphs — min/max, NOT first/last, "because RTL paragraphs store glyphs in
  right-to-left order" (upstream's own comment). Skip when the slice is
  empty or `width <= 0`.
- **Underline `Single`:** `thickness = (m.underline_metrics.thickness * fs).max(1.0).ceil()`;
  `y = run.line_y - m.underline_metrics.offset * fs` (the `post` offset is
  negative-down — a typical negative offset lands the line BELOW the
  baseline in y-down screen coords).
- **Underline `Double`:** same thickness + y; `gap = thickness`; second rect
  at `y + thickness + gap` (= `y + 2 × thickness`).
- **Strikethrough:** `thickness` from `strikethrough_metrics` (same
  `.max(1.0).ceil()`); `y = run.line_y - m.strikethrough_metrics.offset * fs`
  (the `OS/2` offset is positive — above the baseline).
- **Overline:** **reuses the underline thickness**;
  `y = (run.line_y - m.ascent * fs).max(run.line_top)` — clamped so it never
  escapes the line box.
- **Color precedence, per kind:**
  `td.<kind>_color_opt.or(span.color_opt).unwrap_or(default_color)` — the
  per-kind `-color` property, then the span's TEXT color, then the caller's
  default (`currentColor`).
- Buiy does **not** copy `.max(1.0).ceil()` (it floors at 1 *logical* px in
  upstream's integer-pixel space — 1.5 physical px at scale 1.5, the exact
  AA blur § 3.3 prevents) and does **not** quantize through
  `Renderer::rectangle(i32, i32, u32, u32)` (§ 3.1's rejected runner-up).

### 4. The § 3.3 rule, exact form

```text
thickness_phys    = max(1, round(thickness_logical_raw × scale_factor))   // whole physical px
thickness_logical = thickness_phys / scale_factor                          // back to logical
y_phys            = round(y_logical_raw × scale_factor)                    // snap to the grid
y_logical         = y_phys / scale_factor
```

x is NOT snapped (f32 logical end-to-end — the subpixel discipline; upstream's
`x as i32` cast is part of what § 3.1 rejects). For `Double`, the second rect
sits at `y + 2 × thickness_logical` — since `thickness_phys` is integral and
`y` is grid-snapped, the second rect is grid-aligned by construction (no
second snap).

### 5. The as-built quad path (the splice target)

- `ExtractedNode { entity, position, size, color, clip, group }`
  (extract.rs:66–93) — paint order is the implicit `Vec` order of
  `ExtractedNodes.nodes`; **extract emits a record for every painted entity
  even with no background** (`Color::NONE`), and only the PACK skips the
  quad (buckets.rs `pack_view`/`pack_view_partitioned` `continue` on
  `Color::NONE`) — § 4.6 fact (a): every text entity has a node record to
  splice after.
- `pack_view_partitioned(nodes, group_count) -> PackedPartition`
  (buckets.rs:212–284): walks `nodes` in order, pushes `[f32; 13]` instances
  (`packed_to_raw(pack_extracted(node))`), tracks per-group contiguous
  `group_ranges` + complement `flat_ranges`, with the contiguity
  `debug_assert_eq!(r.end, idx, …)` tripwire. Callers:
  `prepare.rs:177` + six sites in `tests/render_buckets.rs`.
- `prepare_buiy_instances` (prepare.rs:148–216) gates the quad buffer on
  `nodes.is_changed() || groups.is_changed()` and the glyph buffer
  **independently** on `glyphs.is_changed()` — T6 adds the third quad term,
  never collapses the two gates.
- `pack_extracted` (instance.rs:82–98): `LinearRgba::from(node.color)`,
  radius 0, clip packed with the `±INFINITY` sentinel. `pack_text_quad`
  (Task 3) mirrors it exactly.
- `ExtractedGlyphs` is `init_resource`'d by **`BuiyRenderPlugin`**
  (render/mod.rs ~:250) "so the prepare gate works even if the text plugin
  is absent" — `ExtractedTextQuads` copies that placement and rationale.

### 6. The as-built producer (the emission site)

`extract_buiy_glyphs` (text/extract.rs) currently binds **15 system params**
(the 16-param cap was already hit once — the `removed` streams are tupled).
T6 tuples the two carriers (`(ResMut<ExtractedGlyphs>,
ResMut<ExtractedTextQuads>)`) so the count stays 15 and T7 keeps headroom.
The per-entity walk already computes everything decoration emission needs:
`origin = gt.translation().truncate() + computed.content_offset` (logical
px), `effective_clip(…)` (self-inclusive, § 8), the resolved entity color,
the `PendingFontBlock` zero-alpha arm, and `scale_factor`. The § 6.2 union
ledger comment names this exact join ("`ExtractedTextQuads` rebuilt
alongside `ExtractedGlyphs` (T6, same producer, same probe, one damage
decision)").

### 7. The stamp substrate

- `AtlasEntryKind::Mask` is the reserved coverage kind — key byte `3`,
  `CoverageR8` format, "sampled exactly like a glyph (a `GlyphAlphaInstance`
  with a mask key)" (atlas/types.rs:83–117). The leading kind byte
  partitions the key space, so a Mask-kind stamp key can never alias a glyph
  key.
- `AtlasWarmupQueue` is a render-world resource drained by the private
  `warmup_atlas` system before `maintain_atlas` (atlas/mod.rs:103);
  `BuiyAtlas::drain_warmup` is `pub` (atlas.rs:81) so headless tests drain
  manually.
- **The finish-ordering seam, as built:** `BuiyPlugin::build` (documented
  "add AFTER `DefaultPlugins`") adds `BuiyTextPlugin` with the `RenderApp`
  already live, and `BuiyTextPlugin::build` reaches it through the guarded
  `app.get_sub_app_mut(RenderApp)` → `register_render_world` (text/mod.rs:178–205,
  as built since T1). The stamp push lands inside that guard.
  `SubApp::init_resource` inserts only if absent, so the text plugin
  init'ing `AtlasWarmupQueue` before `atlas::register`'s own
  `init_resource` (plugin order: text before render in both `BuiyPlugin`
  and the GPU harness) is clobber-free in both orders.
- The coverage shader interpolates
  `atlas_uv = mix(uv_rect.xy, uv_rect.zw, v.uv)` (coverage.wgsl:65); the
  atlas sampler is the pinned **Nearest**/ClampToEdge (atlas/gpu.rs:62–73).
  Emitting the stamp's `uv_rect` as the cell's **midpoint replicated**
  (`min == max == center`) makes every fragment sample the center texel —
  exact under ANY filter (see erratum 3).
- Eviction does not clear texels; a later insert reuses the cell
  (glyph-pipeline § 6.3's hazard) — hence the stamp key joins the
  `ResidentTextKeys` touch pass whenever a stamp instance is live.

### 8. Reflection precedent for bitflags

`bitflags!` doesn't compose with `#[derive(Reflect)]`;
`layout/types.rs:1147–1178` registers `ContainFlags` via
`impl_reflect_opaque!((in path) Type(Default, PartialEq))`.
`DecorationLines` copies that exact pattern.

---

## Decisions (with runner-ups) — read before implementing

1. **Line bits ride `Attrs`; the `-color` property does NOT.** TextSync
   lowers `TextDecorations.line`/`.style` into `Attrs.text_decoration` via
   the `.underline()`/`.strikethrough()`/`.overline()` builders — this is
   what makes cosmic-text BUILD `DecorationSpan`s (`has_decoration()` gates
   span creation, Orientation § 2). The `.{kind}_color()` builders are
   **never called**: `TextDecorations.color` is a `ColorToken`, and tokens
   resolve at extract against the live `Theme` (the `TextColor` model) — the
   producer applies it as precedence tier 1 at emission.
   **Rejected runner-up:** bake the resolved color into
   `Attrs.*_color_opt` at sync — a theme swap would leave stale colors
   (`theme.is_changed()` is in the extract union, NOT the sync union) or
   force a full reshape per retheme; both violate the "retheme = re-emit,
   never reshape" discipline the § 12 retint contract pins. Corollary: the
   pure mirror still implements upstream's full three-tier precedence
   (Task 2 unit-tests it with hand-built spans) even though
   `td.*_color_opt` is always `None` as-built — the drift guard mirrors
   upstream, not Buiy's subset.

2. **`TextDecorations` gains a `style: DecorationLineStyle` field**
   (= erratum 2 against the spec's two-field § 2.2 pin). The campaign's T6
   test surface demands a `double` GPU golden, and the spec's § 3.2 table
   specifies Double's paint math — but the two-field shape
   `{ line, color }` cannot AUTHOR Double. As built:
   `TextDecorations { line: DecorationLines, style: DecorationLineStyle,
   color: Option<ColorToken> }` where
   `DecorationLineStyle { Solid (default), Double, Dotted, Dashed, Wavy }` —
   `Solid`/`Double` lower to `UnderlineStyle::Single`/`Double`;
   `Dotted`/`Dashed`/`Wavy` degrade to `Solid` with a warn-once. This IS the
   § 9 "`DecorationLineStyle` strategy enum at the quad-emission seam"
   reservation, realized as the enum + match arms § 9 says C-tier lands as.
   Per CSS, `text-decoration-style` applies to all line kinds; upstream only
   carries Double for underline, so overline/line-through stay single-line
   under `Double` (documented on the field).
   **Rejected runner-ups:** (a) keep the two-field shape — cannot author the
   campaign-mandated double golden; a test-only `Attrs` back door would be a
   test-as-producer hack T4 just deleted. (b) Mirror cosmic's
   `TextDecoration` shape as the component — conflates `-line` and `-style`
   into one non-CSS-shaped surface and bakes upstream's
   underline-only-Double asymmetry into Buiy's authoring tier.

3. **`DecorationLines` is a `bitflags` set** (`UNDERLINE | OVERLINE |
   LINE_THROUGH`, `u8`), reflected via `impl_reflect_opaque!` — the
   `ContainFlags` precedent verbatim; `bitflags` is already a direct dep.
   **Rejected runner-up:** a three-bool struct — derives `Reflect`
   field-wise, but diverges from the spec's pinned "bitflag set" and from
   the two existing flag types for zero gain.

4. **`TextQuad` + `ExtractedTextQuads` live in `render/extract.rs`,**
   beside `ExtractedNode`, and are `init_resource`'d by `BuiyRenderPlugin`
   (the `ExtractedGlyphs` rationale: the prepare gate must work when the
   text plugin is absent). The producer is `text::extract_buiy_glyphs` —
   exactly the `ExtractedGlyphs` producer/consumer split.
   **Rejected runner-ups:** (a) define in `prepare.rs` — `buckets.rs` (the
   pack) would import from `prepare.rs` which imports `buckets.rs`: a
   gratuitous module cycle. (b) Define in `buiy_core::text` — puts a render
   carrier behind the text plugin and hands a cosmic-boundary module a
   render-seam type it doesn't own. `TextQuad` carries NO cosmic type
   (`Entity`/`Vec2`/`Color`/`ClipRect`) — the seam contract test stays
   green.

5. **`pack_view_partitioned` grows a third parameter** (`text_quads:
   &[TextQuad]`) — one true pack path, callers updated (prepare + six test
   sites pass `&[]`).
   **Rejected runner-up:** a sibling `pack_view_partitioned_with_text` —
   two pack paths that must never drift, for the cost of seven mechanical
   call-site edits. (`pack_view`/`pack_extracted_nodes` — the un-partitioned
   legacy pair used only by older tests — deliberately do NOT grow the
   parameter; the partitioned pack is the one production path,
   prepare.rs:177.)

6. **The splice derives ALL ordering from the fresh node list, every pack**
   (the spec's round-2 contract, restated as code): walk `nodes` in `Vec`
   order; immediately after each node's own instance (or its `Color::NONE`
   skip), splice that entity's text quads from an entity→contiguous-range
   lookup built over the flat carrier per pack. Effect-group membership
   comes from the node record being spliced after (`node.group`), so a text
   quad's partition placement can never disagree with its entity's, and the
   contiguity `debug_assert` holds by construction.
   **Rejected runner-up (the spec's round-1 bug, re-rejected here):** a
   paint-order index ("`painters_z` merge key") recorded into `TextQuad` at
   emission — `ExtractedNode` carries no such field, and a recorded index
   goes stale whenever the node walk rebuilds from a trigger outside the
   text probe union while text quads are retained (e.g.
   `Changed<StackingContext>` on a non-text sibling) — retained quads would
   splice at wrong positions and trip the partition assert. Task 3's
   reorder test pins this.

7. **The carrier is entity-grouped, and the pack debug_asserts it.** The
   producer emits each entity's quads contiguously (it walks entities one at
   a time), so the lookup is `HashMap<Entity, Range<usize>>`; a non-adjacent
   second run for an entity trips a `debug_assert` naming the producer
   contract. Quads whose entity has no node record this pack are skipped
   silently (both unions fire on every entity-set change — spawn via the
   `Added` fan, despawn via `RemovedComponents<ResolvedLayout>`, hide/show
   via the `ComputedPaintSkip` add/remove pair — so a miss is a transient
   impossibility, not an error path worth a panic).

8. **The stamp key is `[AtlasEntryKind::Mask.key_byte(), 0]`** — a 2-byte
   key under the Mask kind (the "sampled exactly like a glyph" reservation),
   sub-discriminant `0` reserved for the solid stamp (future
   `clip-path`/`mask-image` keys carry longer content-derived payloads and
   can never collide with the 2-byte sentinel). Helpers live in
   `text/stamp.rs` (text owns the sentinel per decoration-and-paint § 4.3;
   T7's caret reuses them).
   **Rejected runner-up:** a Glyph-kind sentinel (e.g. font seat
   `u32::MAX`) — semantically wrong (it is not a glyph) and structurally
   aliasable with a real 19 B glyph key in principle; the kind byte exists
   precisely to partition producers.

9. **The stamp's `uv_rect` is the cell midpoint replicated**
   (`[c.x, c.y, c.x, c.y]`) — the interpolated `atlas_uv` is then constant,
   so every fragment samples the center texel exactly, under the pinned
   Nearest sampler or any future filter (supersedes § 4.3's bilinear
   reasoning — erratum 3).
   **Rejected runner-up:** the entry's full `[min, max]` rect — correct in
   the interior but samples on the cell boundary at quad edges, where
   Nearest could flip to a neighboring texel by f32 rounding; midpoint
   costs nothing and is unconditionally exact.

10. **The warmup push lives in `register_render_world`** (build-time,
    inside the existing `RenderApp` guard): `init_resource::<AtlasWarmupQueue>()`
    (idempotent in both plugin orders, Orientation § 7) + one
    `push(solid_stamp_warmup_request())`. The first `warmup_atlas` run
    drains it before any first paint — gate-#2 structural determinism, and
    the existing `wait_for_text_ready` predicate (queue empty + keys
    resident) covers it with no harness change. The producer ALSO
    `get_or_insert`s the stamp on use, so a grace-evicted stamp (idle, no
    live stamp instance) self-heals on the next line-through — § 4.3's
    "re-inserted on miss like any content-addressed entry".
    **Rejected runner-up:** a `RenderStartup` one-shot system — works, but
    adds a schedule entry for a single push with zero codebase precedent;
    every other registration-time render-world insert is done directly in
    `build`/`register_render_world`.

11. **Blocked text zero-alphas its decorations; transparent ones skip** —
    the glyph arm's rule applied uniformly: `PendingFontBlock` present →
    emit quads and stamps with alpha 0 (layout-identical, paint-invisible,
    buffers warm); resolved alpha 0 without a block → emit nothing. One
    rule, one test.

12. **Wholesale carrier publication under the one damage decision** — on
    any dirty frame the producer rebuilds and republishes BOTH
    `ExtractedGlyphs` and `ExtractedTextQuads` (spec-pinned: "same producer,
    same probe, one damage decision"), even when only one would differ; on a
    steady frame it touches NEITHER. Per-resource diffing is the same
    deferred optimization as per-entity patching (glyph-pipeline § 11.4).

---

## File structure

```
crates/buiy_core/src/
├── text/
│   ├── components.rs        M  TextDecorations + DecorationLines + DecorationLineStyle (Task 1)
│   ├── sync.rs              M  Attrs lowering + trigger-union growth (Task 1)
│   ├── decoration.rs        C  the pure render_decoration mirror (Task 2)
│   ├── stamp.rs             C  solid-stamp key/bitmap/uv/warmup helpers (Task 4)
│   ├── extract.rs           M  quad + stamp emission, fan/union growth, carrier tuple (Tasks 3–4)
│   └── mod.rs               M  module decls, exports, register_type, warmup push (Tasks 1–4)
├── render/
│   ├── extract.rs           M  TextQuad + ExtractedTextQuads (Task 3)
│   ├── instance.rs          M  pack_text_quad (Task 3)
│   ├── buckets.rs           M  pack_view_partitioned splice (Task 3)
│   ├── prepare.rs           M  triple quad gate (Task 3)
│   └── mod.rs               M  init_resource::<ExtractedTextQuads> + re-export (Task 3)
crates/buiy_core/tests/
├── text_decoration.rs       C  component/lowering/math/drift-guard/stamp-warmup (Tasks 1, 2, 4)
├── render_text_quads.rs     C  the § 4.6 splice contract over the pure pack (Task 3)
├── text_sync.rs             M  TextDecorations trigger row (Task 1)
├── text_extract.rs          M  carrier retention/rebuild + stamp touch/evict (Tasks 3–4)
├── render_buckets.rs        M  six call sites grow `&[]` (Task 3)
├── support/extract_harness.rs M  ExtractedTextQuads + quad change log (Task 3)
└── text_decoration_gpu.rs   C  goldens + gate-term regressions, #[ignore] (Task 5)
docs/
├── README.md                M  T6 catalog entry (Task 6)
└── plans/2026-06-09-buiy-text-campaign.md  M  status flip + T6 errata (Task 6)
```

---

### Task 1: `TextDecorations` + the TextSync `Attrs` lowering (headless)

The authoring surface and the dataflow's input half: the component, its
lowering into `Attrs.text_decoration` (line bits only — decision 1), and the
trigger-union growth (`has_decoration` gates span creation, so a line-bit
change MUST reshape — Orientation § 2).

**Files:**
- Create: `crates/buiy_core/tests/text_decoration.rs`
- Modify: `crates/buiy_core/src/text/components.rs`
- Modify: `crates/buiy_core/src/text/sync.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`
- Modify: `crates/buiy_core/tests/text_sync.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/buiy_core/tests/text_decoration.rs` (this file grows across
Tasks 1, 2, and 4 — start it with the Task-1 sections):

```rust
//! T6 decoration painting — the authoring component, the TextSync `Attrs`
//! lowering, the pure emission mirror (Task 2), and the solid stamp
//! (Task 4). Spec: decoration-and-paint.md §§ 2–4, 9.

mod support;

use bevy::prelude::*;
use buiy_core::text::{
    DecorationLineStyle, DecorationLines, SharedFontSystem, Text, TextBuffer, TextDecorations,
};

/// Minimal headless app: the T2/T3 text pipeline (TextSync + measure +
/// TextCommit need LayoutPlugin's step sets), no render half.
fn text_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::theme::ThemePlugin)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(buiy_core::layout::LayoutPlugin)
        .add_plugins(buiy_core::text::BuiyTextPlugin::default());
    app
}

fn settle(app: &mut App) {
    for _ in 0..3 {
        app.update();
    }
}

// --- the lowering: line bits reach cosmic, spans come back -----------------

#[test]
fn underline_component_produces_decoration_spans() {
    let mut app = text_app();
    let e = app
        .world_mut()
        .spawn((
            buiy_core::Node,
            buiy_core::layout::Style::default(),
            Text("Hi there".into()),
            TextDecorations {
                line: DecorationLines::UNDERLINE,
                ..Default::default()
            },
        ))
        .id();
    settle(&mut app);
    let buffer = app.world().get::<TextBuffer>(e).expect("TextBuffer");
    let run = buffer.buffer.layout_runs().next().expect("one run");
    // Uniform attrs across the line → upstream merges into ONE span
    // covering every glyph (Orientation § 2).
    assert_eq!(run.decorations.len(), 1, "one merged span");
    let span = &run.decorations[0];
    assert_eq!(span.glyph_range, 0..run.glyphs.len());
    assert_eq!(
        span.data.text_decoration.underline,
        cosmic_text::UnderlineStyle::Single
    );
    assert!(!span.data.text_decoration.strikethrough);
    assert!(!span.data.text_decoration.overline);
    // Decision 1: the color builders are NEVER called — tokens resolve at
    // extract, so the Attrs tier stays None.
    assert_eq!(span.data.text_decoration.underline_color_opt, None);
    // span.color_opt mirrors the first glyph's Attrs color — Buiy never
    // sets Attrs.color_opt (TextColor resolves at extract), so tier 2 is
    // structurally None in v1 (decision 1 corollary).
    assert_eq!(span.color_opt, None);
    assert_eq!(span.font_size, 16.0, "span font_size = the FontSize default");
}

#[test]
fn no_decoration_means_no_spans() {
    // has_decoration() gates span creation upstream — the zero-cost path.
    let mut app = text_app();
    let e = app
        .world_mut()
        .spawn((buiy_core::Node, buiy_core::layout::Style::default(), Text("Hi".into())))
        .id();
    settle(&mut app);
    let buffer = app.world().get::<TextBuffer>(e).expect("TextBuffer");
    let run = buffer.buffer.layout_runs().next().expect("one run");
    assert!(run.decorations.is_empty());
}

#[test]
fn all_three_lines_and_double_style_lower_together() {
    let mut app = text_app();
    let e = app
        .world_mut()
        .spawn((
            buiy_core::Node,
            buiy_core::layout::Style::default(),
            Text("Hi".into()),
            TextDecorations {
                line: DecorationLines::UNDERLINE
                    | DecorationLines::OVERLINE
                    | DecorationLines::LINE_THROUGH,
                style: DecorationLineStyle::Double,
                color: None,
            },
        ))
        .id();
    settle(&mut app);
    let buffer = app.world().get::<TextBuffer>(e).expect("TextBuffer");
    let run = buffer.buffer.layout_runs().next().expect("one run");
    let td = &run.decorations[0].data.text_decoration;
    assert_eq!(td.underline, cosmic_text::UnderlineStyle::Double);
    assert!(td.strikethrough);
    assert!(td.overline);
}

#[test]
fn dotted_dashed_wavy_degrade_to_solid() {
    // Decision 2: the § 9 reservation arms degrade warn-once (the
    // TextWrap::Balance precedent) — assert the lowered VALUE; the warn
    // fires once process-wide and is not asserted (AtomicBool precedent).
    for style in [
        DecorationLineStyle::Dotted,
        DecorationLineStyle::Dashed,
        DecorationLineStyle::Wavy,
    ] {
        assert_eq!(
            style.to_cosmic_underline(),
            cosmic_text::UnderlineStyle::Single,
            "{style:?} degrades to Single"
        );
    }
    assert_eq!(
        DecorationLineStyle::Double.to_cosmic_underline(),
        cosmic_text::UnderlineStyle::Double
    );
    assert_eq!(
        DecorationLineStyle::Solid.to_cosmic_underline(),
        cosmic_text::UnderlineStyle::Single
    );
}

#[test]
fn decoration_lines_default_is_empty() {
    assert!(DecorationLines::default().is_empty());
    assert!(!TextDecorations::default().line.contains(DecorationLines::UNDERLINE));
}
```

Add the trigger row to `crates/buiy_core/tests/text_sync.rs` (follow the
file's existing per-trigger pattern — one focused test mirroring how the
T3/T5 carriers assert `TextSyncAppliedCount`):

```rust
#[test]
fn text_decorations_change_triggers_exactly_one_resync() {
    // § 5.1 union growth (T6): the line bits live in Attrs and
    // has_decoration() gates span creation upstream, so a TextDecorations
    // edit must resync like any other text-style change.
    let mut app = sync_app(); // the file's existing builder
    let e = spawn_text(&mut app); // the file's existing fixture helper
    settle(&mut app);
    assert_applied(&app, 0); // steady

    app.world_mut().entity_mut(e).insert(TextDecorations {
        line: DecorationLines::UNDERLINE,
        ..Default::default()
    });
    app.update();
    assert_applied(&app, 1); // Changed (== Added) fired once

    // Mutating the existing component re-fires.
    app.world_mut()
        .get_mut::<TextDecorations>(e)
        .unwrap()
        .line
        .insert(DecorationLines::LINE_THROUGH);
    app.update();
    assert_applied(&app, 1);
    app.update();
    assert_applied(&app, 0); // back to steady
}
```

(Adapt helper names to the file's actual idioms — read it first; the
assertion currency is `TextSyncAppliedCount`.)

- [ ] **Step 2: Run the tests, confirm they fail** (`TextDecorations` does
  not exist → compile failure counts as RED for the new file; keep the
  existing suites green).

- [ ] **Step 3: Implement**

In `crates/buiy_core/src/text/components.rs` (new section after
`TextDirection`; add `use bevy::reflect::impl_reflect_opaque;` and
`use crate::render::color::ColorToken;` to the imports):

```rust
bitflags::bitflags! {
    /// CSS `text-decoration-line` value set (decoration-and-paint § 2.2;
    /// text.md:51, F). A bitflag set: any combination of the three lines.
    /// The plural component name (`TextDecorations`) deliberately avoids
    /// colliding with `cosmic_text::TextDecoration`, which the sync
    /// lowering binds.
    #[derive(Clone, Copy, Default, PartialEq, Eq, Debug)]
    pub struct DecorationLines: u8 {
        /// Painted UNDER the text (quad tier, § 4.2).
        const UNDERLINE    = 1 << 0;
        /// Painted UNDER the text (quad tier, § 4.2).
        const OVERLINE     = 1 << 1;
        /// Painted OVER the text (solid-stamp glyph tier, § 4.2) — the CSS
        /// Text Decoration L3 painting-order requirement.
        const LINE_THROUGH = 1 << 2;
    }
}

// `bitflags!` doesn't compose with `#[derive(Reflect)]` — register the
// opaque type manually (the layout ContainFlags precedent).
impl_reflect_opaque!((in crate::text::components) DecorationLines(Default, PartialEq));

/// `text-decoration-style`, the § 9 strategy enum at the quad-emission seam
/// — realized with its working arms (decoration-and-paint § 9; T6 plan
/// decision 2): `Solid`/`Double` lower to cosmic's
/// `UnderlineStyle::Single`/`Double`; `Dotted`/`Dashed`/`Wavy` parse and
/// DEGRADE to `Solid` with a warn-once (the `TextWrap::Balance` precedent).
/// Dotted/dashed are future Buiy emission patterns (segmented quads); the
/// wavy unblock path of record is upstream-PR-first (the literal
/// `// TODO: Wavy` in 0.19's enum) — never bake the fallback into F-tier
/// types. Upstream carries `Double` for the underline only, so overline and
/// line-through stay single-line under `Double` (the upstream asymmetry,
/// documented not hidden).
#[derive(Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
pub enum DecorationLineStyle {
    #[default]
    Solid,
    Double,
    Dotted,
    Dashed,
    Wavy,
}

impl DecorationLineStyle {
    /// The cosmic `UnderlineStyle` this style lowers to (the sync mapping).
    pub fn to_cosmic_underline(self) -> cosmic_text::UnderlineStyle {
        match self {
            DecorationLineStyle::Solid => cosmic_text::UnderlineStyle::Single,
            DecorationLineStyle::Double => cosmic_text::UnderlineStyle::Double,
            DecorationLineStyle::Dotted | DecorationLineStyle::Dashed | DecorationLineStyle::Wavy => {
                warn_once_decoration_style_degrades();
                cosmic_text::UnderlineStyle::Single
            }
        }
    }
}

/// CSS `text-decoration` (decoration-and-paint § 2.2; text.md:51, F): which
/// lines to draw, their style, and the optional `text-decoration-color`
/// override. `color: None` = `currentColor` — the § 3.2 precedence, resolved
/// AT EXTRACT against the live theme (decision 1: line bits ride
/// `Attrs.text_decoration`; the color token never does — a theme swap
/// re-emits instances, never reshapes).
///
/// The `style` field supersedes the spec's two-field § 2.2 pin (T6 erratum
/// 2): § 3.2 specifies Double's paint math and the campaign demands its
/// golden, so the knob ships here rather than behind a C-tier follow-up.
///
/// Component REMOVAL is not a resync trigger (the T2-erratum-1 carrier
/// precedent): a removed `TextDecorations` resyncs on the next other
/// trigger.
#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct TextDecorations {
    /// Which decoration lines to draw (any combination).
    pub line: DecorationLines,
    /// Line style; `Solid` default. See [`DecorationLineStyle`].
    pub style: DecorationLineStyle,
    /// `text-decoration-color`; `None` = `currentColor` (§ 3.2 tier 3 via
    /// the span/entity fallbacks).
    pub color: Option<ColorToken>,
}

static WARNED_DECORATION_STYLE: AtomicBool = AtomicBool::new(false);

fn warn_once_decoration_style_degrades() {
    if !WARNED_DECORATION_STYLE.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: text-decoration-style dotted/dashed/wavy are not built; \
             degrading to solid (decoration-and-paint § 9 — wavy's unblock \
             path is upstream-PR-first; warned once)"
        );
    }
}
```

In `crates/buiy_core/src/text/sync.rs`:

1. Grow the trigger union (after the T5 `Changed<TextDirection>` arm) and
   update the module-doc ledger ("joined in T6: `TextDecorations` — the line
   bits live in `Attrs`, `has_decoration` gates span creation"):

```rust
    // T6 carrier (decoration-and-paint § 2.2): the line bits ride
    // Attrs.text_decoration and gate upstream span creation, so a
    // decoration edit must reshape. (A color-only edit also lands here —
    // component-granular; accepted, see the T6 plan's honesty pins.)
    Changed<TextDecorations>,
```

2. Grow `SyncedText`/`SyncedTextItem`/`unsynced` with
   `Option<&TextDecorations>`, thread it through `sync_one` and the creation
   loop into `AuthoredStyle::resolve`.

3. Grow `AuthoredStyle` with the two `Copy` halves (the `ColorToken` never
   reaches sync — decision 1):

```rust
struct AuthoredStyle<'a> {
    // … existing fields …
    /// T6: the decoration LINE bits + style (decision 1: only these lower
    /// into `Attrs`; the color token resolves at extract).
    deco_lines: DecorationLines,
    deco_style: DecorationLineStyle,
}
```

with `resolve` taking `decorations: Option<&TextDecorations>` and filling
`deco_lines: decorations.map_or(DecorationLines::empty(), |d| d.line)` /
`deco_style: decorations.map_or_else(Default::default, |d| d.style)`.

4. Lower in BOTH attrs constructors — `attrs()` (the `set_text` /
   empty-text / `set_rich_text` default) and `span_attrs` (every resolved
   span), via one shared helper so the two can never diverge:

```rust
impl<'a> AuthoredStyle<'a> {
    /// T6: apply the decoration line bits (decision 1 — bits only, never
    /// the `*_color` builders; tokens resolve at extract).
    fn decorated<'b>(&self, attrs: Attrs<'b>) -> Attrs<'b> {
        let mut attrs = attrs;
        if self.deco_lines.contains(DecorationLines::UNDERLINE) {
            attrs = attrs.underline(self.deco_style.to_cosmic_underline());
        }
        if self.deco_lines.contains(DecorationLines::LINE_THROUGH) {
            attrs = attrs.strikethrough();
        }
        if self.deco_lines.contains(DecorationLines::OVERLINE) {
            attrs = attrs.overline();
        }
        attrs
    }
}
```

`attrs()` returns `self.decorated(Attrs::new().family(…).weight(…))`;
`span_attrs` becomes a method (or takes `&AuthoredStyle` as today) and wraps
its result the same way. Every span of a decorated node carries the bits, so
upstream merges them back into whole-line spans (Orientation § 2).

In `crates/buiy_core/src/text/mod.rs`: export `TextDecorations`,
`DecorationLines`, `DecorationLineStyle` from `components`; add
`.register_type::<TextDecorations>()` and
`.register_type::<DecorationLineStyle>()` (and `DecorationLines` via its
opaque registration — `register_type::<DecorationLines>()` after the
`impl_reflect_opaque!`) to the author-set registration block.

- [ ] **Step 4: Run the new tests — green.**
- [ ] **Step 5: Run BOTH gate lanes — green.** Commit:
  `feat(text): T6.1 — TextDecorations component + Attrs lowering`.

---

### Task 2: The pure emission mirror — `text/decoration.rs` (headless)

The math half: `render_decoration`'s semantics 1:1 (Orientation § 3) in f32
logical px with the § 3.3 physical-px floor (Orientation § 4). Pure
functions, exact-number fixtures — the upstream-drift guard.

**Files:**
- Create: `crates/buiy_core/src/text/decoration.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`
- Modify: `crates/buiy_core/tests/text_decoration.rs`

- [ ] **Step 1: Write the failing tests**

Append to `crates/buiy_core/tests/text_decoration.rs`:

```rust
use bevy::math::Vec2;
use buiy_core::text::{
    DecorationKind, snap_thickness, snap_y, span_decoration_rects, span_x_extent,
};
use cosmic_text::{DecorationMetrics, GlyphDecorationData, TextDecoration, UnderlineStyle};

/// Hand fixture: underline-only data with round-number EM metrics.
/// offset −0.1 em (post-table convention: negative = below baseline in
/// y-down screen space after the `line_y − offset×fs` mirror), thickness
/// 0.05 em, ascent 0.75 em.
fn underline_data(style: UnderlineStyle) -> GlyphDecorationData {
    GlyphDecorationData {
        text_decoration: TextDecoration {
            underline: style,
            ..TextDecoration::new()
        },
        underline_metrics: DecorationMetrics { offset: -0.1, thickness: 0.05 },
        strikethrough_metrics: DecorationMetrics { offset: 0.3, thickness: 0.05 },
        ascent: 0.75,
    }
}

const FS: f32 = 16.0; // span font_size: metrics × 16 → offset −1.6, thickness 0.8

// --- § 3.3: the snap helpers, in isolation --------------------------------

#[test]
fn thickness_floors_at_one_whole_physical_pixel() {
    // raw 0.8 logical @1.0 → 0.8 phys → round 1 → 1.0 logical
    assert_eq!(snap_thickness(0.8, 1.0), 1.0);
    // raw 0.8 @1.25 → 1.0 phys → exactly 1 → 0.8 logical (already integral)
    assert_eq!(snap_thickness(0.8, 1.25), 0.8);
    // raw 0.3 @2.0 → 0.6 phys → max(1, round) = 1 → 0.5 logical
    assert_eq!(snap_thickness(0.3, 2.0), 0.5);
    // THE § 3.3 pin: at scale 1.5 a 1-logical-px line must become 2 whole
    // physical px (2/1.5 logical), NOT upstream's verbatim `.max(1).ceil()`
    // = 1 logical px = 1.5 physical px (the AA blur the rule prevents).
    assert_eq!(snap_thickness(1.0, 1.5), 2.0 / 1.5);
}

#[test]
fn y_snaps_to_the_physical_grid() {
    assert_eq!(snap_y(10.3, 1.0), 10.0);
    assert_eq!(snap_y(10.5, 1.0), 11.0); // round-half-up at .5 (f32 round)
    // 10.3 @1.25 → 12.875 phys → round 13 → 10.4 logical
    assert_eq!(snap_y(10.3, 1.25), 13.0 / 1.25);
}

// --- the mirrored placement math, exact numbers ----------------------------

#[test]
fn single_underline_exact_rect() {
    // origin (10.0, 20.0), line_y 12.0, scale 1.0:
    //   y_raw = 20 + 12 − (−0.1 × 16) = 33.6 → snap 34.0
    //   t     = 0.05 × 16 = 0.8        → floor 1.0
    let rects = span_decoration_rects(
        Vec2::new(10.0, 20.0),
        12.0,        // line_y
        4.0,         // line_top
        3.0,         // x_start (run-local)
        50.0,        // width
        &underline_data(UnderlineStyle::Single),
        FS,
        None,        // span color
        1.0,         // scale
    );
    assert_eq!(rects.len(), 1);
    let r = &rects[0];
    assert_eq!(r.kind, DecorationKind::Underline);
    assert_eq!(r.rect, [13.0, 34.0, 50.0, 1.0]); // x = origin.x + x_start (UNsnapped)
    assert_eq!(r.color_opt, None);
}

#[test]
fn single_underline_fractional_scale_exact_rect() {
    // scale 1.25: y_raw 33.6 → 42 phys → exactly 33.6 logical;
    //             t_raw 0.8 → 1.0 phys → 0.8 logical.
    let rects = span_decoration_rects(
        Vec2::new(10.0, 20.0), 12.0, 4.0, 3.0, 50.0,
        &underline_data(UnderlineStyle::Single), FS, None, 1.25,
    );
    assert_eq!(rects[0].rect, [13.0, 33.6, 50.0, 0.8]);
}

#[test]
fn double_underline_gap_equals_thickness() {
    // § 3.2: two rects, gap = thickness ⇒ rect2.y = rect1.y + 2 × t.
    // scale 1.0 → t = 1.0 (floored), y = 34.0 ⇒ second at 36.0.
    let rects = span_decoration_rects(
        Vec2::new(10.0, 20.0), 12.0, 4.0, 3.0, 50.0,
        &underline_data(UnderlineStyle::Double), FS, None, 1.0,
    );
    assert_eq!(rects.len(), 2);
    assert_eq!(rects[0].rect, [13.0, 34.0, 50.0, 1.0]);
    assert_eq!(rects[1].rect, [13.0, 36.0, 50.0, 1.0]);
    assert_eq!(rects[1].kind, DecorationKind::Underline);
}

#[test]
fn line_through_uses_strikeout_metrics() {
    // y_raw = 20 + 12 − (0.3 × 16) = 27.2 → snap 27.0; t = 0.8 → 1.0.
    let mut data = underline_data(UnderlineStyle::None);
    data.text_decoration.strikethrough = true;
    let rects = span_decoration_rects(
        Vec2::new(10.0, 20.0), 12.0, 4.0, 3.0, 50.0, &data, FS, None, 1.0,
    );
    assert_eq!(rects.len(), 1);
    assert_eq!(rects[0].kind, DecorationKind::LineThrough);
    assert_eq!(rects[0].rect, [13.0, 27.0, 50.0, 1.0]);
}

#[test]
fn overline_clamps_to_line_top_and_reuses_underline_thickness() {
    let mut data = underline_data(UnderlineStyle::None);
    data.text_decoration.overline = true;
    // Unclamped: y_raw = 20 + (12 − 0.75 × 16) = 20.0 → snap 20.0.
    let rects = span_decoration_rects(
        Vec2::new(10.0, 20.0), 12.0, 4.0, 3.0, 50.0, &data, FS, None, 1.0,
    );
    assert_eq!(rects[0].kind, DecorationKind::Overline);
    assert_eq!(rects[0].rect, [13.0, 20.0, 50.0, 1.0]); // underline t reused

    // Clamped: ascent 0.9 em → line_y − 14.4 = −2.4 < line_top 4.0 →
    // y = origin.y + line_top = 24.0 (clamp BEFORE the origin fold + snap).
    data.ascent = 0.9;
    let rects = span_decoration_rects(
        Vec2::new(10.0, 20.0), 12.0, 4.0, 3.0, 50.0, &data, FS, None, 1.0,
    );
    assert_eq!(rects[0].rect[1], 24.0);
}

#[test]
fn color_precedence_mirrors_upstream_per_kind() {
    use cosmic_text::Color as CColor;
    let span_color = CColor::rgb(1, 2, 3);
    let kind_color = CColor::rgb(9, 9, 9);

    // Tier 2: span text color when the -color tier is empty.
    let rects = span_decoration_rects(
        Vec2::ZERO, 12.0, 4.0, 0.0, 10.0,
        &underline_data(UnderlineStyle::Single), FS, Some(span_color), 1.0,
    );
    assert_eq!(rects[0].color_opt, Some(span_color));

    // Tier 1: the per-kind *_color_opt wins over the span color.
    let mut data = underline_data(UnderlineStyle::Single);
    data.text_decoration.underline_color_opt = Some(kind_color);
    let rects = span_decoration_rects(
        Vec2::ZERO, 12.0, 4.0, 0.0, 10.0, &data, FS, Some(span_color), 1.0,
    );
    assert_eq!(rects[0].color_opt, Some(kind_color));

    // Tier 3 (both None) → None: the CALLER falls back to the resolved
    // entity foreground (currentColor) — asserted at the producer in Task 3.
    let rects = span_decoration_rects(
        Vec2::ZERO, 12.0, 4.0, 0.0, 10.0,
        &underline_data(UnderlineStyle::Single), FS, None, 1.0,
    );
    assert_eq!(rects[0].color_opt, None);
}

#[test]
fn zero_width_spans_emit_nothing() {
    let rects = span_decoration_rects(
        Vec2::ZERO, 12.0, 4.0, 0.0, 0.0,
        &underline_data(UnderlineStyle::Single), FS, None, 1.0,
    );
    assert!(rects.is_empty());
    // And the extent helper refuses empty/degenerate input upstream of it.
    assert_eq!(span_x_extent(&[], &(0..0)), None);
}

// --- the upstream-drift guard: real shaping, pinned numbers ----------------

#[test]
fn drift_guard_real_spans_from_the_embedded_font() {
    // Mirror-not-call (§ 3.1) means a cosmic-text bump that changes span
    // production or decoration_metrics must fail HERE, loudly, instead of
    // silently shifting goldens. Shape real text against the committed
    // embedded font (deterministic — registered_fonts_db only) and pin the
    // span's metric values as literals.
    //
    // IMPLEMENTER: capture the four pinned constants by running this test
    // once with `dbg!(span.data)` and hard-coding the printed values, each
    // with a derivation comment (the T5 shaping-snapshot precedent). They
    // are pure functions of the committed font bytes (post/OS2 tables ÷
    // upem), so they are bit-stable until either the font artifact or
    // upstream's decoration_metrics changes — exactly the two drifts this
    // test exists to catch.
    use cosmic_text::{Attrs, Metrics, Shaping};
    let fonts = SharedFontSystem::new();
    let mut fs = fonts.lock();
    let mut buffer = cosmic_text::Buffer::new_empty(Metrics::new(16.0, 19.2));
    buffer.set_size(&mut fs, Some(400.0), Some(100.0));
    buffer.set_text(
        "Hi there",
        &Attrs::new()
            .underline(cosmic_text::UnderlineStyle::Single)
            .strikethrough()
            .overline(),
        Shaping::Advanced,
        None,
    );
    buffer.shape_until_scroll(&mut fs, false);
    let run = buffer.layout_runs().next().expect("one run");
    assert_eq!(run.decorations.len(), 1, "uniform attrs merge to one span");
    let span = &run.decorations[0];
    assert_eq!(span.glyph_range, 0..run.glyphs.len());
    assert_eq!(span.font_size, 16.0);

    const EPS: f32 = 1e-6;
    let m = &span.data;
    // PIN(capture): Fira Sans latin subset, post.underlinePosition/upem etc.
    assert!((m.underline_metrics.offset - PINNED_UNDERLINE_OFFSET).abs() < EPS);
    assert!((m.underline_metrics.thickness - PINNED_UNDERLINE_THICKNESS).abs() < EPS);
    assert!((m.strikethrough_metrics.offset - PINNED_STRIKEOUT_OFFSET).abs() < EPS);
    assert!((m.ascent - PINNED_ASCENT).abs() < EPS);

    // And the x-extent helper against the real glyph slice (the RTL-safe
    // min/max walk): equals the run's own extremes.
    let (x, w) = span_x_extent(run.glyphs, &span.glyph_range).expect("non-empty");
    let min = run.glyphs.iter().map(|g| g.x).fold(f32::INFINITY, f32::min);
    let max = run.glyphs.iter().map(|g| g.x + g.w).fold(f32::NEG_INFINITY, f32::max);
    assert!((x - min).abs() < EPS && (w - (max - min)).abs() < EPS);
}
```

(Also add an RTL extent case: shape an Arabic fixture-font string — the T5
corpus idiom, `register_fixture_font` + `support::fixture_font_bytes` — and
assert `span_x_extent` returns the min/max envelope, not first/last glyph.)

- [ ] **Step 2: Run — RED** (module does not exist).

- [ ] **Step 3: Implement**

Create `crates/buiy_core/src/text/decoration.rs`:

```rust
//! The pure decoration-emission mirror (decoration-and-paint § 3): walk
//! `LayoutRun.decorations` ourselves and emit f32 logical-px rects,
//! mirroring upstream `render_decoration`'s semantics 1:1
//! (cosmic-text 0.19.0 src/render.rs, source-verified) — with two
//! deliberate substitutions pinned by the § 3.1/§ 3.3 decisions:
//!
//! 1. No `Renderer::rectangle(i32, i32, u32, u32)` quantization — f32
//!    logical px end-to-end (fractional scale factors survive).
//! 2. The thickness floor is `max(1, round(t × scale))` WHOLE PHYSICAL px
//!    (converted back to logical), with y snapped to the same grid —
//!    upstream's `.max(1.0).ceil()` floors at 1 LOGICAL px, which is 1.5
//!    physical px at scale 1.5: the exact AA blur this rule prevents.
//!    x is never snapped (the subpixel discipline).
//!
//! Color precedence mirrors upstream exactly, per kind:
//! `td.<kind>_color_opt → span color → None` (the caller's `currentColor`
//! fallback). As-built tier 1 is structurally `None` (decision 1: the
//! `-color` property is a Buiy `ColorToken` resolved at extract, never an
//! `Attrs` color) and tier 2 is `None` until the rich-text tier sets
//! `Attrs.color_opt` — the mirror still implements all three tiers because
//! it mirrors UPSTREAM, and the drift guard pins upstream.
//!
//! Pure functions — no ECS, no GPU, no FontSystem: unit-testable with
//! hand-built `GlyphDecorationData` (every cosmic type here is plain-pub).

use bevy::math::Vec2;
use cosmic_text::{Color, GlyphDecorationData, LayoutGlyph, UnderlineStyle};
use smallvec::SmallVec;
use std::ops::Range;

/// Which decoration line a rect belongs to — the § 4.2 seat router:
/// `Underline`/`Overline` are quad-tier (`ExtractedTextQuads`),
/// `LineThrough` is a solid-stamp glyph-tier instance emitted after the
/// run's glyphs.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DecorationKind {
    Underline,
    Overline,
    LineThrough,
}

/// One emitted decoration rect, in WORLD logical px (the `origin` fold is
/// done here, before the § 3.3 y-snap, so the snap lands on the real
/// physical grid — the same reason `physical()` folds the origin before
/// binning, glyph-pipeline § 5.1).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct DecorationRect {
    pub kind: DecorationKind,
    /// `[x, y, w, h]`, logical px, window space.
    pub rect: [f32; 4],
    /// Upstream tiers 1–2 (`td.*_color_opt` else the span text color);
    /// `None` = the caller applies `currentColor` (the resolved entity
    /// foreground), after the Buiy `TextDecorations.color` token override.
    pub color_opt: Option<Color>,
}

/// § 3.3: thickness floored to whole physical px, minimum one, expressed
/// in logical px.
pub fn snap_thickness(thickness_logical: f32, scale_factor: f32) -> f32 {
    (thickness_logical * scale_factor).round().max(1.0) / scale_factor
}

/// § 3.3: y snapped to the physical pixel grid, expressed in logical px.
pub fn snap_y(y_logical: f32, scale_factor: f32) -> f32 {
    (y_logical * scale_factor).round() / scale_factor
}

/// The span's horizontal extent `(x_start, width)` in run-local logical px:
/// min/max over the span's glyphs — NOT first/last, because RTL paragraphs
/// store glyphs in right-to-left order (upstream's own comment, mirrored).
/// `None` when the range is empty/out-of-bounds or the width is ≤ 0
/// (upstream's early-outs).
pub fn span_x_extent(glyphs: &[LayoutGlyph], range: &Range<usize>) -> Option<(f32, f32)> {
    let span_glyphs = glyphs.get(range.clone())?;
    if span_glyphs.is_empty() {
        return None;
    }
    let mut x_min = f32::INFINITY;
    let mut x_max = f32::NEG_INFINITY;
    for g in span_glyphs {
        x_min = x_min.min(g.x);
        x_max = x_max.max(g.x + g.w);
    }
    let width = x_max - x_min;
    (width > 0.0).then_some((x_min, width))
}

/// Emit one span's decoration rects (§ 3.2's table, exactly upstream's
/// placement with the § 3.3 substitutions — module doc):
///
/// | line | y (pre-snap, world) | thickness (pre-floor) |
/// |---|---|---|
/// | underline | `origin.y + line_y − underline.offset × fs` | `underline.thickness × fs` |
/// | underline Double | + a second rect at `y + 2 × t` (gap = t) | same |
/// | line-through | `origin.y + line_y − strikeout.offset × fs` | `strikeout.thickness × fs` |
/// | overline | `origin.y + max(line_y − ascent × fs, line_top)` | underline's |
///
/// `line_y`/`line_top` are run-local (`LayoutRun` fields); `x_start`/`width`
/// from [`span_x_extent`]. ≤ 4 rects (Double underline + strike + over).
#[allow(clippy::too_many_arguments)]
pub fn span_decoration_rects(
    origin: Vec2,
    line_y: f32,
    line_top: f32,
    x_start: f32,
    width: f32,
    data: &GlyphDecorationData,
    font_size: f32,
    span_color_opt: Option<Color>,
    scale_factor: f32,
) -> SmallVec<[DecorationRect; 4]> {
    let mut out = SmallVec::new();
    if width <= 0.0 {
        return out;
    }
    let td = &data.text_decoration;
    let x = origin.x + x_start;

    // Underline (Single | Double) — and the thickness overline reuses.
    let underline_t = snap_thickness(data.underline_metrics.thickness * font_size, scale_factor);
    if td.underline != UnderlineStyle::None {
        let color_opt = td.underline_color_opt.or(span_color_opt);
        let y = snap_y(
            origin.y + line_y - data.underline_metrics.offset * font_size,
            scale_factor,
        );
        out.push(DecorationRect {
            kind: DecorationKind::Underline,
            rect: [x, y, width, underline_t],
            color_opt,
        });
        if td.underline == UnderlineStyle::Double {
            // gap = thickness; t is physically integral and y grid-snapped,
            // so the second rect is grid-aligned by construction.
            out.push(DecorationRect {
                kind: DecorationKind::Underline,
                rect: [x, y + 2.0 * underline_t, width, underline_t],
                color_opt,
            });
        }
    }

    // Line-through (its own font table).
    if td.strikethrough {
        let t = snap_thickness(data.strikethrough_metrics.thickness * font_size, scale_factor);
        let y = snap_y(
            origin.y + line_y - data.strikethrough_metrics.offset * font_size,
            scale_factor,
        );
        out.push(DecorationRect {
            kind: DecorationKind::LineThrough,
            rect: [x, y, width, t],
            color_opt: td.strikethrough_color_opt.or(span_color_opt),
        });
    }

    // Overline — clamped to the line box (run-local, BEFORE the origin
    // fold, mirroring upstream's `(line_y − ascent×fs).max(line_top)`).
    if td.overline {
        let y_local = (line_y - data.ascent * font_size).max(line_top);
        let y = snap_y(origin.y + y_local, scale_factor);
        out.push(DecorationRect {
            kind: DecorationKind::Overline,
            rect: [x, y, width, underline_t],
            color_opt: td.overline_color_opt.or(span_color_opt),
        });
    }
    out
}
```

In `text/mod.rs`: `mod decoration;` +
`pub use decoration::{DecorationKind, DecorationRect, snap_thickness, snap_y, span_decoration_rects, span_x_extent};`.

- [ ] **Step 4: Capture the drift-guard pins** (run once with `dbg!`,
  hard-code with derivation comments), then run the new tests — green.
- [ ] **Step 5: Run BOTH gate lanes — green.** Commit:
  `feat(text): T6.2 — the pure render_decoration mirror (§ 3.3 floor)`.

---

### Task 3: `ExtractedTextQuads` + the § 4.6 splice + the triple gate + underline/overline emission (headless)

The carrier, the merge contract, the gate term, and the producer's quad-tier
half — the load-bearing task. **CRITICAL CONTRACT (decision 6):** the splice
derives all ordering from the FRESH `nodes.nodes` list on every pack;
`TextQuad` carries NO order or group field; partition contiguity is
preserved by adoption; the quad gate is
`nodes.is_changed() || groups.is_changed() || text_quads.is_changed()`; quad
and glyph buffers stay independently gated.

**Files:**
- Modify: `crates/buiy_core/src/render/extract.rs`
- Modify: `crates/buiy_core/src/render/instance.rs`
- Modify: `crates/buiy_core/src/render/buckets.rs`
- Modify: `crates/buiy_core/src/render/prepare.rs`
- Modify: `crates/buiy_core/src/render/mod.rs`
- Modify: `crates/buiy_core/src/text/extract.rs`
- Modify: `crates/buiy_core/tests/render_buckets.rs` (six call sites)
- Modify: `crates/buiy_core/tests/support/extract_harness.rs`
- Modify: `crates/buiy_core/tests/text_extract.rs`
- Create: `crates/buiy_core/tests/render_text_quads.rs`

- [ ] **Step 1: Write the failing tests**

Create `crates/buiy_core/tests/render_text_quads.rs` — the § 4.6 contract
over the pure pack (no ECS, no GPU):

```rust
//! The § 4.6 splice-merge contract (decoration-and-paint.md): text quads
//! merge into the EXISTING quad instance blob, ordered by the FRESH node
//! list every pack, partition contiguity preserved. These tests are the
//! headless half of the contract; the GPU regressions live in
//! tests/text_decoration_gpu.rs.

use bevy::prelude::*;
use buiy_core::render::buckets::pack_view_partitioned;
use buiy_core::render::extract::{ExtractedNode, TextQuad};

fn node(entity: Entity, x: f32, color: Color, group: Option<usize>) -> ExtractedNode {
    ExtractedNode {
        entity,
        position: Vec2::new(x, 0.0),
        size: Vec2::splat(10.0),
        color,
        clip: None,
        group,
    }
}

fn quad(entity: Entity, x: f32) -> TextQuad {
    TextQuad {
        entity,
        position: Vec2::new(x, 100.0),
        size: Vec2::new(5.0, 1.0),
        color: Color::WHITE,
        clip: None,
    }
}

fn e(i: u32) -> Entity {
    Entity::from_raw(i) // adapt to the constructor the other render tests use
}

/// Instance x-positions, the order fingerprint.
fn xs(p: &buiy_core::render::buckets::PackedPartition) -> Vec<f32> {
    p.instances.iter().map(|i| i[0]).collect()
}

#[test]
fn quads_splice_immediately_after_their_entity() {
    let nodes = [node(e(1), 1.0, Color::WHITE, None), node(e(2), 2.0, Color::WHITE, None)];
    let quads = [quad(e(1), 11.0), quad(e(1), 12.0), quad(e(2), 22.0)];
    let p = pack_view_partitioned(&nodes, 0, &quads);
    // node1, its two quads IN CARRIER ORDER, node2, its quad — § 4.4 holds
    // by construction (background < decorations, per entity).
    assert_eq!(xs(&p), vec![1.0, 11.0, 12.0, 2.0, 22.0]);
}

#[test]
fn transparent_nodes_still_anchor_their_quads() {
    // § 4.6 fact (a): extract emits Color::NONE records and only the pack
    // skips the BACKGROUND quad — the text quads still splice at the
    // entity's paint position.
    let nodes = [
        node(e(1), 1.0, Color::NONE, None),
        node(e(2), 2.0, Color::WHITE, None),
    ];
    let quads = [quad(e(1), 11.0)];
    let p = pack_view_partitioned(&nodes, 0, &quads);
    assert_eq!(xs(&p), vec![11.0, 2.0]);
}

#[test]
fn order_derives_from_the_fresh_node_list_every_pack() {
    // THE round-2 contract (the rejected painters_z merge key): the SAME
    // retained carrier lands correctly when the node walk rebuilt in a NEW
    // order for a non-text reason.
    let quads = [quad(e(1), 11.0), quad(e(2), 22.0)];
    let before = [node(e(1), 1.0, Color::WHITE, None), node(e(2), 2.0, Color::WHITE, None)];
    let after = [node(e(2), 2.0, Color::WHITE, None), node(e(1), 1.0, Color::WHITE, None)];
    assert_eq!(xs(&pack_view_partitioned(&before, 0, &quads)), vec![1.0, 11.0, 2.0, 22.0]);
    assert_eq!(xs(&pack_view_partitioned(&after, 0, &quads)), vec![2.0, 22.0, 1.0, 11.0]);
}

#[test]
fn quads_adopt_their_entitys_group_and_keep_contiguity() {
    // Group membership comes from the node record being spliced after —
    // a text quad's partition placement can never disagree with its
    // entity's. debug_assert contiguity must NOT fire (debug test builds).
    let nodes = [
        node(e(1), 1.0, Color::WHITE, None),
        node(e(2), 2.0, Color::WHITE, Some(0)),
        node(e(3), 3.0, Color::WHITE, Some(0)),
        node(e(4), 4.0, Color::WHITE, None),
    ];
    let quads = [quad(e(2), 22.0), quad(e(3), 33.0)];
    let p = pack_view_partitioned(&nodes, 1, &quads);
    assert_eq!(xs(&p), vec![1.0, 2.0, 22.0, 3.0, 33.0, 4.0]);
    // Group 0 = instances 1..5 (node2, quad, node3, quad) — one contiguous
    // range including the spliced quads.
    assert_eq!(p.group_ranges, vec![1..5]);
    assert_eq!(p.flat_ranges, vec![0..1, 5..6]);
}

#[test]
fn group_member_with_only_quads_still_extends_the_group() {
    // A transparent group member contributes ONLY its text quads — they
    // must still carry the group (the § 4.5 underline-dims contract).
    let nodes = [
        node(e(1), 1.0, Color::WHITE, Some(0)),
        node(e(2), 2.0, Color::NONE, Some(0)),
    ];
    let quads = [quad(e(2), 22.0)];
    let p = pack_view_partitioned(&nodes, 1, &quads);
    assert_eq!(p.group_ranges, vec![0..2]);
    assert!(p.flat_ranges.is_empty());
}

#[test]
fn unknown_entities_are_skipped() {
    // Decision 7: a quad whose entity has no node record this pack is
    // dropped (transient impossibility — both unions fire on entity-set
    // changes), never panicked on.
    let nodes = [node(e(1), 1.0, Color::WHITE, None)];
    let quads = [quad(e(9), 99.0)];
    let p = pack_view_partitioned(&nodes, 0, &quads);
    assert_eq!(xs(&p), vec![1.0]);
}

#[test]
fn transparent_text_quads_are_skipped_like_node_quads() {
    let nodes = [node(e(1), 1.0, Color::WHITE, None)];
    let mut q = quad(e(1), 11.0);
    q.color = Color::NONE;
    let p = pack_view_partitioned(&nodes, 0, &[q]);
    assert_eq!(xs(&p), vec![1.0]);
}

#[test]
fn empty_carrier_is_byte_identical_to_the_old_pack() {
    // The no-text regression: with no text quads the partition must be
    // exactly the pre-T6 output (the compositor's flat path stays
    // byte-for-byte).
    let nodes = [
        node(e(1), 1.0, Color::WHITE, None),
        node(e(2), 2.0, Color::WHITE, Some(0)),
        node(e(3), 3.0, Color::NONE, None),
    ];
    let p = pack_view_partitioned(&nodes, 1, &[]);
    assert_eq!(xs(&p), vec![1.0, 2.0]);
    assert_eq!(p.group_ranges, vec![1..2]);
    assert_eq!(p.flat_ranges, vec![0..1]);
}

#[test]
fn text_quad_packs_like_a_node_quad() {
    // pack_text_quad mirrors pack_extracted: linearized color, radius 0,
    // clip sentinel.
    use buiy_core::render::instance::pack_text_quad;
    let q = quad(e(1), 11.0);
    let p = pack_text_quad(&q);
    assert_eq!(p.rect_pos, [11.0, 100.0]);
    assert_eq!(p.rect_size, [5.0, 1.0]);
    assert_eq!(p.radius, 0.0);
    let lin = LinearRgba::from(Color::WHITE);
    assert_eq!(p.color, [lin.red, lin.green, lin.blue, lin.alpha]);
    assert_eq!(p.clip_min, [f32::NEG_INFINITY; 2]);
}
```

Harness growth (`tests/support/extract_harness.rs`): mirror `GlyphChangeLog`
with a quad log, init the carrier, add accessors:

```rust
use buiy_core::render::extract::ExtractedTextQuads;

#[derive(Resource, Default)]
pub struct TextQuadChangeLog {
    pub changed_frames: usize,
}

fn log_text_quad_changes(quads: Res<ExtractedTextQuads>, mut log: ResMut<TextQuadChangeLog>) {
    if quads.is_changed() {
        log.changed_frames += 1;
    }
}
// in with_atlas_config: render.init_resource::<ExtractedTextQuads>();
//                       render.init_resource::<TextQuadChangeLog>();
// schedule: (maintain_atlas, extract_buiy_glyphs, log_glyph_changes,
//            log_text_quad_changes).chain()
// accessors:
pub fn text_quads(&self) -> &ExtractedTextQuads { self.render.resource() }
pub fn quad_changed_frames(&self) -> usize { self.render.resource::<TextQuadChangeLog>().changed_frames }
```

Producer-side tests in `tests/text_extract.rs` (use the file's
`spawn_text`/harness idioms; spawn with
`TextDecorations { line: UNDERLINE | OVERLINE, .. }` where noted):

```rust
#[test]
fn decorated_text_emits_quads_alongside_glyphs() {
    // One run walk emits both (§ 4.6): underline + overline = 2 quads,
    // entity-keyed, world logical px, the entity's self-inclusive clip.
}

#[test]
fn steady_state_retains_both_carriers_untouched() {
    // After settle: N frames with no trigger → glyph AND quad changed
    // counts both stay flat (the O(0) contract extends to the new carrier).
}

#[test]
fn text_decorations_change_rebuilds_both_carriers() {
    // Mutate TextDecorations (e.g. add LINE_THROUGH... use color for the
    // pure-extract path: a color change reaches extract via
    // Changed<TextDecorations> even though ComputedTextLayout is idempotent)
    // → exactly one rebuild frame for both logs; the quad colors re-resolve.
}

#[test]
fn decoration_color_precedence_at_the_producer() {
    // (a) TextDecorations.color = Some(token) → quad color == the resolved
    //     token (tier 1).
    // (b) color = None → quad color == the entity's resolved TextColor
    //     (tier 3, currentColor — tier 2 is structurally None in v1).
    // (c) theme token swap → theme.is_changed() rebuild → new color
    //     (retheme = re-emit, decision 1).
}

#[test]
fn scale_change_refloors_decoration_thickness() {
    // set_scale(2.0) → rebuild; assert a quad's height equals the § 3.3
    // floor at the new scale (exact number from the drift-guard pins).
}

#[test]
fn blocked_text_zero_alphas_decorations() {
    // The PendingFontBlock arm (decision 11): register a Block-display
    // family (the text_extract.rs Block idiom from T5), assert quads emit
    // with alpha exactly 0.0, then lift → full alpha.
}

#[test]
fn undecorated_text_emits_no_quads() {
    // The carrier stays empty (and is still PUBLISHED on rebuild frames —
    // wholesale, decision 12).
}
```

- [ ] **Step 2: Run — RED.**

- [ ] **Step 3: Implement**

`render/extract.rs` — the carrier (after `ExtractedNodes`):

```rust
/// One text quad-tier visual (decoration-and-paint § 4.6): selection rects
/// (T7) and underline/overline (T6), keyed by the SOURCE entity. A flat
/// `Copy` record — deliberately NO order and NO group field: paint order is
/// the implicit `Vec` order of `ExtractedNodes.nodes`, and BOTH derive from
/// the fresh node list at pack time (a recorded index would go stale
/// whenever the node walk rebuilds while text quads are retained — the
/// spec's rejected round-1 design). Carries no cosmic-text type (the seam
/// contract).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TextQuad {
    /// The source main-world entity — the splice key.
    pub entity: Entity,
    /// Painted top-left, logical px, window space (origin-folded by the
    /// producer; § 3.3 y already snapped).
    pub position: Vec2,
    /// Quad size, logical px (height = the § 3.3 floored thickness).
    pub size: Vec2,
    /// Resolved paint color (§ 3.2 precedence applied at extract);
    /// `Color::NONE` = skip at pack (mirrors `ExtractedNode.color`).
    pub color: Color,
    /// The entity's SELF-INCLUSIVE clip (same resolution as its glyphs,
    /// glyph-pipeline § 8); `None` = the full-view sentinel.
    pub clip: Option<ClipRect>,
}

/// Render-world carrier for text's quad-tier visuals (decoration-and-paint
/// § 4.6). Producer: `text::extract_buiy_glyphs` — rebuilt alongside
/// `ExtractedGlyphs` under the same § 6.2 probe union (one damage decision),
/// retained untouched on steady frames so `is_changed()` is the third quad
/// gate term in `prepare_buiy_instances`. ENTITY-GROUPED: each entity's
/// quads are contiguous, in § 4.4 emission order (the pack debug_asserts
/// the grouping).
#[derive(Resource, Default, Clone, Debug)]
pub struct ExtractedTextQuads {
    pub quads: Vec<TextQuad>,
}
```

`render/instance.rs`:

```rust
/// Pack one [`TextQuad`] (decoration-and-paint § 4.6) exactly like a node
/// quad: CPU-linearized color, radius 0, clip sentinel. Same blob, same
/// pipeline, no new GPU anything.
pub fn pack_text_quad(quad: &TextQuad) -> PackedInstance {
    let lin = LinearRgba::from(quad.color);
    let (clip_min, clip_max) = match quad.clip {
        Some(c) => ([c.min.x, c.min.y], [c.max.x, c.max.y]),
        None => (CLIP_SENTINEL_MIN, CLIP_SENTINEL_MAX),
    };
    PackedInstance {
        rect_pos: [quad.position.x, quad.position.y],
        rect_size: [quad.size.x, quad.size.y],
        color: [lin.red, lin.green, lin.blue, lin.alpha],
        radius: 0.0,
        clip_min,
        clip_max,
    }
}
```

`render/buckets.rs` — grow `pack_view_partitioned` (the body refactor:
hoist the group/flat run bookkeeping out of the node loop into a small
helper so the node instance and its spliced quads share it):

```rust
pub fn pack_view_partitioned(
    nodes: &[ExtractedNode],
    group_count: usize,
    text_quads: &[TextQuad],
) -> PackedPartition {
    // Entity → contiguous carrier range (§ 4.6's "entity→quads lookup over
    // the flat carrier"), rebuilt per pack — all ordering derives from the
    // FRESH node walk below, so retained quads land correctly even on
    // frames where the node list rebuilt for non-text reasons (fact (b)).
    let mut quads_by_entity: std::collections::HashMap<Entity, std::ops::Range<usize>> =
        std::collections::HashMap::new();
    for (i, q) in text_quads.iter().enumerate() {
        match quads_by_entity.entry(q.entity) {
            std::collections::hash_map::Entry::Vacant(slot) => {
                slot.insert(i..i + 1);
            }
            std::collections::hash_map::Entry::Occupied(mut range) => {
                debug_assert_eq!(
                    range.get().end,
                    i,
                    "ExtractedTextQuads must be entity-grouped (the producer \
                     emits each entity's quads contiguously — § 4.6)"
                );
                range.get_mut().end = i + 1;
            }
        }
    }

    let mut p = Partitioner::new(nodes.len() + text_quads.len(), group_count);
    for node in nodes {
        let g = node.group.filter(|&g| g < group_count);
        if node.color != Color::NONE {
            p.push(packed_to_raw(&pack_extracted(node)), g);
        }
        // § 4.6: splice the entity's text quads IMMEDIATELY after its node
        // record, adopting the node's group — partition placement can never
        // disagree with the entity's, so contiguity holds by construction.
        if let Some(range) = quads_by_entity.get(&node.entity) {
            for quad in &text_quads[range.clone()] {
                if quad.color == Color::NONE {
                    continue;
                }
                p.push(packed_to_raw(&pack_text_quad(quad)), g);
            }
        }
    }
    p.finish()
}
```

…where `Partitioner` is the extracted bookkeeping (the existing `match g`
block verbatim — instances vec, `group_ranges` with the contiguity
`debug_assert_eq!` and its full message, `flat_ranges`, `run_group`), moved
into a private struct with `push(instance, group)` + `finish()`. Behavior
for node-only input is byte-identical (the `empty_carrier…` test pins it).
Import `pack_text_quad` + `TextQuad`.

`render/prepare.rs`:

```rust
    text_quads: Res<ExtractedTextQuads>,
…
    // The quad gate (§ 4.6): nodes OR groups OR text quads — text's
    // quad-tier visuals ride the SAME buffer, so a decoration-only frame
    // (e.g. a TextDecorations color edit: text probe fires, node probe
    // doesn't) must re-pack it. Quad and glyph buffers stay INDEPENDENTLY
    // gated — a caret blink (T7) re-uploads glyphs only.
    if nodes.is_changed() || groups.is_changed() || text_quads.is_changed() {
        let partition =
            pack_view_partitioned(&nodes.0.nodes, groups.0.len(), &text_quads.quads);
```

(`pack_extracted_nodes`/`pack_view` stay untouched — decision 5 note.)

`render/mod.rs`: `.init_resource::<extract::ExtractedTextQuads>()` in the
render-branch chain, with the `ExtractedGlyphs`-style comment ("filled by
`text::extract_buiy_glyphs` (T6); init'd here so the prepare gate works even
if the text plugin is absent"). Re-export `TextQuad`/`ExtractedTextQuads`
wherever `ExtractedNodes` is re-exported.

`text/extract.rs` — the producer:

1. Tuple the carriers (param count stays 15 — Orientation § 6):
   `mut carriers: (ResMut<ExtractedGlyphs>, ResMut<ExtractedTextQuads>)`,
   destructured at the top. Update the vanished-window arm to clear BOTH
   (once), the steady-state arm to touch NEITHER, and the publish site:

```rust
    glyphs.glyphs = new_glyphs;
    text_quads.quads = new_quads;
```

2. Grow the fan with `Option<&TextDecorations>` and the union with
   `Changed<TextDecorations>` (comment: "T6 — the decoration carrier: line
   bits change shaping via TextSync; the COLOR tier resolves here, so a
   color-only edit must re-emit even though `ComputedTextLayout` is
   idempotent"). Update the § 6.2 ledger comment: the
   `ExtractedTextQuads` join is now REAL; `Changed<CaretVisual>` /
   `Changed<SelectionVisual>` remain the T7 seats.

3. Per entity, before the run loop:

```rust
        let resolved_entity_color = resolve_token(&color.unwrap_or(&default_color).0, theme);
        let entity_color = linear_color(resolved_entity_color);
        let eff_clip = effective_clip(stacking, clip_rect, ancestor_clip);
        let clip = pack_clip(eff_clip.as_ref());
        // § 3.2 tier 1: the -color token, resolved at extract (decision 1).
        let deco_override: Option<Color> =
            decorations.and_then(|d| d.color.as_ref()).map(|t| resolve_token(t, theme));
```

4. Per run, BEFORE the glyph loop — the decoration walk (one run walk emits
   both; line-through rects are buffered for Task 4, where the stamp lands —
   in THIS task, route only `Underline`/`Overline` and leave a
   `// T6 Task 4: LineThrough → solid stamp, after the glyph loop` seam):

```rust
            for span in run.decorations {
                let Some((x_start, width)) = span_x_extent(run.glyphs, &span.glyph_range)
                else {
                    continue;
                };
                for deco in span_decoration_rects(
                    origin, run.line_y, run.line_top, x_start, width,
                    &span.data, span.font_size, span.color_opt, scale_factor,
                ) {
                    // § 3.2 precedence: token override → upstream tiers
                    // (per-kind / span color) → currentColor.
                    let mut color = deco_override
                        .or(deco.color_opt.map(cosmic_color))
                        .unwrap_or(resolved_entity_color);
                    if blocked {
                        color = color.with_alpha(0.0); // § 7 Block: paint-invisible, layout-identical
                    } else if color.alpha() == 0.0 {
                        continue;
                    }
                    match deco.kind {
                        DecorationKind::Underline | DecorationKind::Overline => {
                            new_quads.push(TextQuad {
                                entity,
                                position: Vec2::new(deco.rect[0], deco.rect[1]),
                                size: Vec2::new(deco.rect[2], deco.rect[3]),
                                color,
                                clip: eff_clip,
                            });
                        }
                        DecorationKind::LineThrough => { /* Task 4 */ }
                    }
                }
            }
```

with `fn cosmic_color(c: cosmic_text::Color) -> Color {
Color::srgba_u8(c.r(), c.g(), c.b(), c.a()) }` extracted so `span_color`
becomes `linear_color(cosmic_color(c))` (one conversion, two sinks).

5. `new_quads: Vec<TextQuad>` declared beside `new_glyphs`. Entity-grouping
   holds by construction (the walk is per-entity); note it where the vec is
   declared.

Update the six `tests/render_buckets.rs` call sites (`, &[]`) and the
harness per Step 1.

- [ ] **Step 4: Run the new tests — green.** Also re-run
  `render_compositor*`/`render_group_contiguity_gpu` on the GPU lane — the
  partition refactor must not disturb the compositor.
- [ ] **Step 5: Run BOTH gate lanes — green.** Commit:
  `feat(render,text): T6.3 — ExtractedTextQuads + the § 4.6 splice + triple gate`.

---

### Task 4: The solid stamp + line-through over the text (headless)

The over-text half: the 1×1 warmup-pinned `CoverageR8` stamp (§ 4.3), the
finish-ordering push, the touch-pass join (glyph-pipeline § 6.3), and the
line-through emission after the run's glyphs (§ 4.4 seat 5).

**Files:**
- Create: `crates/buiy_core/src/text/stamp.rs`
- Modify: `crates/buiy_core/src/text/extract.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`
- Modify: `crates/buiy_core/tests/text_decoration.rs`
- Modify: `crates/buiy_core/tests/text_extract.rs`

- [ ] **Step 1: Write the failing tests**

Append to `tests/text_decoration.rs`:

```rust
use buiy_core::render::atlas::{AtlasConfig, AtlasFormat, BuiyAtlas};
use buiy_core::text::{solid_stamp_bitmap, solid_stamp_key, stamp_uv};

#[test]
fn stamp_key_is_mask_kind_and_aliases_nothing() {
    let key = solid_stamp_key();
    assert_eq!(key.0.as_slice(), &[3u8, 0u8], "Mask kind byte + sub-id 0");
    assert_ne!(key.0.len(), buiy_core::text::GLYPH_KEY_LEN, "can never alias a glyph key");
}

#[test]
fn stamp_bitmap_is_one_solid_white_texel() {
    let bmp = solid_stamp_bitmap();
    assert_eq!(bmp.size, bevy::math::UVec2::ONE);
    assert!(matches!(bmp.format, AtlasFormat::CoverageR8));
    assert_eq!(bmp.data, vec![255u8]);
}

#[test]
fn stamp_uv_is_the_cell_midpoint_replicated() {
    // Decision 9: constant interpolated uv → every fragment samples the
    // center texel — exact under the pinned Nearest sampler (and any other).
    let mut atlas = BuiyAtlas::new(AtlasConfig::default());
    let entry = atlas.get_or_insert(solid_stamp_key(), AtlasFormat::CoverageR8, solid_stamp_bitmap);
    let uv = stamp_uv(&entry);
    let c = entry.uv.center();
    assert_eq!(uv, [c.x, c.y, c.x, c.y]);
}

#[test]
fn register_render_world_pushes_the_warmup_request() {
    // The finish-ordering seam, headless form (decision 10): a bare SubApp
    // (the register_render_world test idiom) receives the queue + exactly
    // one stamp request; draining it makes the stamp resident pre-paint.
    use bevy::app::SubApp;
    use buiy_core::render::atlas::AtlasWarmupQueue;
    use buiy_core::text::{SharedFontSystem, register_render_world};
    let mut render_app = SubApp::new();
    let fonts = SharedFontSystem::new();
    register_render_world(&mut render_app, &fonts);
    let mut queue = render_app
        .world_mut()
        .remove_resource::<AtlasWarmupQueue>()
        .expect("queue init'd by the text plugin half");
    assert_eq!(queue.len(), 1, "exactly one push: the solid stamp");
    let mut atlas = BuiyAtlas::new(AtlasConfig::default());
    atlas.drain_warmup(&mut queue);
    let entry = atlas.get(&solid_stamp_key()).expect("warmup-pinned");
    assert_eq!(entry.px.size(), bevy::math::UVec2::ONE);
}
```

Append to `tests/text_extract.rs` (harness-based; line-through fixture =
`TextDecorations { line: DecorationLines::LINE_THROUGH, .. }`):

```rust
#[test]
fn line_through_emits_a_stamp_instance_after_the_runs_glyphs() {
    // The § 4.4 seat-5 order: the LAST instance(s) of the entity's glyph
    // emission are stamps — uv min == max (the midpoint signature
    // distinguishes a stamp from a real glyph), color = the § 3.2 resolved
    // decoration color, rect height = the § 3.3 floored strikeout
    // thickness. resident_keys gains one solid_stamp_key() entry PER stamp
    // instance (the one-key-per-instance invariant).
}

#[test]
fn live_stamp_survives_eviction_grace_via_the_touch_pass() {
    // glyph-pipeline § 6.3's join: with_atlas_config(eviction_grace: 5),
    // settle a line-through fixture, idle 3× grace frames (steady,
    // retained) → atlas.get(&solid_stamp_key()) still resident.
}

#[test]
fn idle_stamp_evicts_and_reinserts_on_miss() {
    // § 4.3 "re-inserted on miss": no decoration → after grace the stamp
    // drains (gate #15 — warmup-pinned is not pin-forever; note: the
    // harness never runs warmup_atlas, so this exercises the
    // get_or_insert-on-miss path end-to-end). Then ADD line-through →
    // rebuild re-inserts; instance uvs are valid for the NEW entry.
}

#[test]
fn blocked_text_zero_alphas_the_stamp_too() {
    // Decision 11, stamp half: Block → stamp instances emit at alpha 0
    // (the stamp stays resident + touched), lift → full alpha.
}
```

- [ ] **Step 2: Run — RED.**

- [ ] **Step 3: Implement**

Create `crates/buiy_core/src/text/stamp.rs`:

```rust
//! The 1×1 solid-white `CoverageR8` stamp (decoration-and-paint § 4.3):
//! one reserved Buiy-internal sentinel `AtlasKey` whose cell, stretched by
//! `GlyphAlphaInstance.rect`, is an over-glyph tinted rectangle with ZERO
//! new pipeline — line-through (T6) and the caret (T7) ride it, emitted
//! after the run's glyphs so they paint over the text (the CSS Text
//! Decoration L3 painting order; quads can never paint over glyphs —
//! § 4.1's fixed rank).
//!
//! Residency: warmup-pinned at plugin startup (`register_render_world`
//! pushes [`solid_stamp_warmup_request`] — the render architecture § 1.1
//! finish-ordering seam delivers a live `RenderApp`), re-inserted on miss
//! like any content-addressed entry (the producer's `get_or_insert`), and
//! TOUCH-MAINTAINED while any stamp instance is live (the key joins
//! `ResidentTextKeys` per instance — glyph-pipeline § 6.3: an idle-but-
//! painted stamp past `eviction_grace` would otherwise lose its cell to
//! reuse and sample someone else's bitmap).

use bevy::math::UVec2;

use crate::render::atlas::{
    AtlasBitmap, AtlasEntry, AtlasEntryKind, AtlasFormat, AtlasKey, AtlasWarmupRequest,
};

/// The sentinel key: `[Mask kind byte, 0]` — the Mask kind is the reserved
/// "sampled exactly like a glyph" coverage kind (types.rs), the leading
/// kind byte makes glyph-key aliasing structurally impossible, and the
/// 2-byte length + sub-id 0 are reserved for THIS stamp (future
/// clip-path/mask-image keys carry longer content-derived payloads).
pub fn solid_stamp_key() -> AtlasKey {
    AtlasKey::from_bytes(&[AtlasEntryKind::Mask.key_byte(), 0])
}

/// The stamp bitmap: one full-coverage texel. Value 255 ⇒ the sampled
/// coverage is exactly 1.0 and the instance tint passes through unchanged
/// (alpha-as-color, § 4.1).
pub fn solid_stamp_bitmap() -> AtlasBitmap {
    AtlasBitmap {
        size: UVec2::ONE,
        format: AtlasFormat::CoverageR8,
        data: vec![255],
    }
}

/// The startup warmup push (§ 4.3 "warmup-pinned"): resident before any
/// first paint, so a first-frame caret/line-through never races a cold
/// atlas (gate #2).
pub fn solid_stamp_warmup_request() -> AtlasWarmupRequest {
    AtlasWarmupRequest {
        key: solid_stamp_key(),
        format: AtlasFormat::CoverageR8,
        bitmap: solid_stamp_bitmap(),
    }
}

/// The instance `uv_rect` for a stamp: the cell MIDPOINT replicated, so the
/// interpolated `atlas_uv` (coverage.wgsl `mix(min, max, v.uv)`) is
/// constant and every fragment samples the center texel — exact under the
/// pinned Nearest sampler or any future filter (T6 decision 9; supersedes
/// § 4.3's bilinear note — erratum 3).
pub fn stamp_uv(entry: &AtlasEntry) -> [f32; 4] {
    let c = entry.uv.center();
    [c.x, c.y, c.x, c.y]
}
```

`text/mod.rs`: `mod stamp;` + `pub use stamp::{solid_stamp_bitmap,
solid_stamp_key, solid_stamp_warmup_request, stamp_uv};` and in
`register_render_world` (with the decision-10 comment):

```rust
    // T6 (decoration-and-paint § 4.3): the warmup-pinned solid stamp — the
    // one committed AtlasWarmupQueue push of the text campaign. This runs
    // inside the live-RenderApp guard (the render architecture § 1.1
    // finish-ordering seam: BuiyPlugin adds this plugin after
    // DefaultPlugins, so the sub-app exists). init_resource is insert-if-
    // absent, so plugin order vs atlas::register is irrelevant.
    render_app.init_resource::<crate::render::atlas::AtlasWarmupQueue>();
    render_app
        .world_mut()
        .resource_mut::<crate::render::atlas::AtlasWarmupQueue>()
        .push(stamp::solid_stamp_warmup_request());
```

`text/extract.rs` — line-through emission. Hoist a per-frame
`let mut stamp_entry: Option<AtlasEntry> = None;` beside `font_guard` (one
residency probe per frame, lock-free — the stamp closure never touches the
`FontSystem`). In the Task-3 decoration walk, buffer line-through rects per
run (`SmallVec<[([f32; 4], [f32; 4]); 2]>` of (rect, linear color) — the
linear color via `linear_color(color)` after the same precedence/Block
handling, except `blocked` sets `color[3] = 0.0` post-linearize like the
glyph arm). After the run's glyph loop:

```rust
            // § 4.4 seat 5: line-through paints OVER the run's glyphs —
            // solid-stamp GlyphAlphaInstances appended after them.
            if !strikes.is_empty() {
                let entry = *stamp_entry.get_or_insert_with(|| {
                    atlas.get_or_insert(solid_stamp_key(), AtlasFormat::CoverageR8, solid_stamp_bitmap)
                });
                if entry.page > 0 {
                    warn_once_page_overflow();
                }
                for (rect, color) in strikes.drain(..) {
                    new_glyphs.push(GlyphAlphaInstance {
                        rect,
                        uv: stamp_uv(&entry),
                        color,
                        clip,
                        page: entry.page as u32,
                    });
                    // § 6.3: one key per instance — the un-gated touch pass
                    // keeps a live stamp LRU-warm through retained frames.
                    new_keys.push(solid_stamp_key());
                }
            }
```

(`GlyphMetaCache` is untouched — stamps need no bearings; its residency
prune is key-agnostic and keeps working.)

- [ ] **Step 4: Run the new tests — green.** Also re-run
  `text_touch_pass.rs` (the seam-contract test must stay green —
  `TextQuad`/stamp helpers cross no cosmic type into the render seam).
- [ ] **Step 5: Run BOTH gate lanes — green.** Commit:
  `feat(text): T6.4 — the warmup-pinned solid stamp + line-through over text`.

---

### Task 5: GPU lane — one golden per decoration kind + the gate-term regressions (`#[ignore]`)

Pixels. Four kind goldens (the campaign's test surface), the three quad-gate
terms regression-pinned end-to-end, and the § 4.5 group asymmetry pinned as
EXPECTED (so T8 flips a failing assertion, not a silent behavior).

**Files:**
- Create: `crates/buiy_core/tests/text_decoration_gpu.rs`

- [ ] **Step 1: Write the tests** (all `#[ignore = "needs a wgpu adapter; …"]`,
  built on `support::gpu_render_app`/`render_to_image`/
  `spawn_capture_camera`/`finish_and_run`/`wait_for_text_ready`/
  `readback_rgba` — the text_gpu.rs idioms; W=128, H=64; text `"Hi"` at
  40 px — no descenders, so glyph ink never crosses below the baseline and
  band classification is unambiguous; TEXT token = white, DECO token = pure
  red):

```rust
//! GPU end-to-end decoration tests (T6): real entities through TextSync →
//! TextCommit → extract (quads + stamps) → the § 4.6 splice → pixels.
//! decoration-and-paint §§ 3–4; one golden per kind (campaign T6 surface).
//! Run: cargo test -p buiy_core --test text_decoration_gpu -- --ignored --test-threads=1
```

Shared helpers:

- `spawn_decorated_fixture(app, deco: TextDecorations)` — the
  `text_gpu.rs::spawn_text_fixture` shape plus the component and a second
  theme token for the decoration color.
- `red_bands(pixels) -> Vec<Range<u32>>` — scan rows; a row is "red" when
  any pixel within ±4/channel of
  `expected_full_coverage_srgb(linear red)`; coalesce adjacent rows into
  bands. (Band classification, not stored PNGs — the established
  "re-capture IS the golden" idiom; determinism is asserted with a fresh
  re-capture + `perceptual_diff < 1e-4` like `hello_text`.)
- `white_rows(pixels) -> Range<u32>` — the glyph-ink row envelope.

Tests:

1. `underline_paints_one_band_below_the_glyphs` — exactly one red band,
   entirely BELOW `white_rows().end`; band height = the § 3.3 floored
   thickness in physical px; re-capture determinism.
2. `double_underline_paints_two_bands_with_a_thickness_gap` —
   `style: Double`: exactly two red bands, equal heights, gap rows == band
   height (gap = thickness, § 3.2).
3. `overline_paints_above_the_glyphs` — one red band entirely ABOVE
   `white_rows().start`.
4. `line_through_paints_over_the_glyph_ink` — THE seat test: one red band
   INTERSECTING `white_rows()`; at the band's rows, columns that are white
   directly above and below the band read RED inside it — the stamp painted
   over the glyph coverage (a quad-tier line-through would read white here:
   quads draw under glyphs).
5. `decoration_recolor_repacks_the_quad_buffer` — **the third gate term.**
   Capture (red underline) → mutate `TextDecorations.color` to a second
   token (blue) — this fires `Changed<TextDecorations>` in the TEXT probe
   union only (no `extract_buiy_nodes` union member fires; `ResolvedLayout`
   / `ComputedTextLayout` writes are idempotent) → drive frames → capture:
   the band is now blue, no red remains. Without
   `text_quads.is_changed()` in the prepare gate this test fails with a
   stale red underline.
6. `sibling_background_change_resplices_retained_quads` — **the nodes term
   + § 4.6 fact (b).** Capture (underline) → change a SIBLING node's
   `Background` (nodes rebuild; the text union does NOT fire — text quads
   retained) → capture: the underline band is still present, identical rows
   (the retained carrier landed correctly through the fresh-list walk; a
   stale-index merge would misplace or drop it).
7. `opacity_group_dims_the_underline_but_not_the_line_through` — **the
   groups term + the § 4.5 asymmetry pin.** Wrap the text in an
   `Opacity(0.5)` card (an `EffectGroup` former): the underline's red band
   reads at composite-dimmed intensity (it rode the group's partition range
   — group adoption end-to-end), while the line-through band reads at FULL
   intensity (glyph draws bypass effect groups until T8). Comment loudly:
   `// T8 flips the second assertion — keep them adjacent so it fails HERE.`

- [ ] **Step 2: Run the GPU lane — RED on the new file** (until Tasks 1–4
  are merged in this branch, then green).
- [ ] **Step 3: Run BOTH gate lanes — green.** Commit:
  `test(text): T6.5 — decoration GPU goldens + quad-gate regressions`.

---

### Task 6: Docs flip + errata + self-review

**Files:**
- Modify: `docs/README.md`
- Modify: `docs/plans/2026-06-09-buiy-text-campaign.md`

- [ ] **Step 1: Campaign plan** — flip the phase-status table row
  `T6 | Decoration painting | proposed → landed`, and append the errata
  block to the T6 section (the established "T*n* errata for the spec edit
  pass" pattern — superseding context, not a silent contradiction),
  seeded from this plan + anything further found while implementing:

  1. *decoration-and-paint § 3.2's "`DecorationSpan.color_opt` (the
     `-color` property)"* misattributes the field: source-verified in
     0.19.0, the `-color` property is the per-kind
     `TextDecoration.{underline,strikethrough,overline}_color_opt` inside
     `span.data`, while `DecorationSpan.color_opt` is the span's TEXT color
     ("Fallback color from the first glyph's `color_opt`", layout.rs:73) —
     precedence tier 2, not tier 1. The precedence ORDER as specced is
     correct; as built, tier 1 is Buiy's `TextDecorations.color` token
     resolved at extract (the line bits ride `Attrs`, the color never does
     — a theme swap re-emits, never reshapes).
  2. *§ 2.2's two-field `TextDecorations { line, color }`* cannot author
     the `Double` row whose paint math § 3.2 specifies and whose golden the
     campaign mandates — as built the component carries
     `style: DecorationLineStyle { Solid, Double, Dotted, Dashed, Wavy }`
     (dotted/dashed/wavy degrade to solid warn-once): the § 9 reservation
     realized as the enum + match arms, not a fourth component.
  3. *§ 4.3's "bilinear filtering of a uniform texel is exact"* — the
     as-built atlas sampler is the pinned **Nearest** (atlas/gpu.rs); as
     built the stamp instance's `uv_rect` is the cell midpoint replicated,
     which is exact under any filter and immune to edge-texel selection.
  4. *decoration color tiers 1–2 are structurally dormant in v1* — Buiy
     never sets `Attrs.*_color_opt` (erratum 1) nor `Attrs.color_opt`
     (rich-text spans are C-tier), so upstream's first two tiers are always
     `None` as-built; the pure mirror implements and unit-tests all three
     anyway (it mirrors upstream — the drift guard's job).

- [ ] **Step 2: docs/README.md** — add the T6 plan line to the text-plans
  catalog block (after the T5 line, same format):
  `- [Buiy text T6 — Decoration painting](plans/2026-06-10-buiy-text-t6-decoration-painting.md) — `TextDecorations` (`DecorationLines` bitflags + `DecorationLineStyle` Solid/Double, dotted/dashed/wavy degrade warn-once) lowered to `Attrs.text_decoration` at TextSync (line bits only; the `-color` token resolves at extract), the pure `render_decoration` mirror (f32 logical px, the § 3.3 physical-px min-thickness floor + y-snap, upstream's int-px `.max(1).ceil()` deliberately not copied), underline/overline → the entity-keyed `ExtractedTextQuads` carrier spliced into the existing quad buffer by the § 4.6 fresh-node-list walk (group adoption, contiguity by construction, triple gate `nodes||groups||text_quads`), line-through → 1×1 warmup-pinned `CoverageR8` solid-stamp glyph instance after the run's glyphs (midpoint uv; key joins the touch pass), GPU goldens per kind + gate-term regressions + the § 4.5 dim-asymmetry pin (T8 flips it). Selection/caret = T7 seats. `[landed]``
- [ ] **Step 3: Self-review.** Re-read the diff against decoration-and-paint
  §§ 2–4/9 and glyph-pipeline §§ 6.2–6.3/10: every **F** marker in §§ 2–4
  either landed or is a named T7/T8 seat; the § 4.6 contract paragraphs map
  1:1 onto `pack_view_partitioned` + the producer; no cosmic type crossed
  the seam; the § 6.2 ledger comments name exactly the T7 joins. Confirm
  the campaign's T6 test surface is fully covered (exact-number y /
  thickness / fractional floor / double gap / color precedence headless;
  underline / double / overline / line-through GPU). Dispatch a
  fresh-context review subagent over the full T6 diff (the
  requesting-code-review skill).
- [ ] **Step 4: Run BOTH gate lanes one final time — green.** Commit:
  `docs(text): T6 — campaign status flip + errata`.
