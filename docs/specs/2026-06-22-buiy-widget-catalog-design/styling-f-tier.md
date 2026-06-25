# Styling F-tier — feed BoxShadow / Border / Outline / focus-ring into the live draw spine — child C6 of the widget-catalog campaign

`2026-06-22` · `[active]` · Wave 3 · realizes foundation `visuals.md §3.3` (box-shadow / per-side border / border-radius / outline) + `color-and-forced-colors` (forced-colors draw-skip) · depends on C0, C1, C4, C7

> **Landed.** C6-a (Outline band channel + focus-ring lowering — WCAG 2.4.7) and C6-b (per-side `Border` via the band channel AT the box edge + `BoxShadow` via the `(Shadow, layer)` bucket + the forced-colors shadow-suppression branch) are built and verified: headless extract/display-list tier + the GPU `--ignored` lane (programmatic readback, not pixel goldens — per-side border colors at the edge + the shadow paints offset behind the box). Inset box-shadow + dashed/dotted/double border `LineStyle`s remain the deferred C-tier fast-follows (§ 3.1 / § 7). Completes C6 + Wave 3.

> Scope is decision §2.4 (umbrella): wire `BoxShadow` / per-side `Border` / `Outline` (+ the focus ring) into the live `extract → pack → bucket → specialize → draw` spine, and add the forced-colors draw-time `BoxShadow`-suppression branch. The four F-tier styling channels exist as components, the shadow shader and primitive kind exist, and a CPU border-band oracle exists — but **none of them paint**: the live `ExtractedNode → pack_extracted → (Quad,0) → buiy_pass` path emits only a solid-fill rounded-rect with a hardcoded radius `0`. This child closes that wiring without touching the R1/R2-frozen 68 B quad stride (umbrella §6.7).
>
> **Not in scope (C-tier, explicitly):** gradients / background-image / multiple background layers (atlas seam); `filter` / `backdrop-filter` / `mix-blend-mode` shaders (reserved, already `EffectGroup`-marked); `ClipRadius` rounded-clip-corner clipping; `clip-path` / path-primitive shapes; `text-shadow`; the `Groove`/`Ridge`/`Inset`/`Outset` bevel `LineStyle`s (render as `Solid` until the C bevel shader); color-space / HDR / wide-gamut management; the *contents* of the forced-colors system-color map (owned by `buiy-theme-tokens-design` — this child only wires the suppression branch + confirms the stub already on `main`). The per-widget "which states must visually differ" roster is C4/C8's, not this child's.

---

## 1. Problem & current state

The live draw path paints **only** a solid-fill rounded-rect quad with a **hardcoded radius `0`**. Every F-tier styling channel beyond background fill is unfed, despite the foundation tiering all four **F** (`visuals.md:128-129,133,145`) and the render-pipeline spec designing the concrete pass shape. The audit confirms this and flags the invisible focus ring as the single **CRITICAL** bug (`2026-06-21-todomvc-prototype-audit.md:41,142,159,239`, finding #1; WCAG 2.4.7).

The verified gap, channel by channel (file:line on current `main`):

- **Extract fan reads only `Background`.** `extract_buiy_nodes` (`crates/buiy_core/src/render/extract.rs:347-381`) binds `Option<&Background>` for paint, plus the clip/stacking/effect inputs — it does **not** bind `Border`, `BoxShadow`, `Outline`, or `Extract<Res<UserPreferences>>`. The damage `Or`-set (`extract.rs:391-419`) likewise omits `Changed<Border/BoxShadow/Outline>`. `ExtractedNode` (`extract.rs:65-107`) carries `position/size/affine/color/clip/group` — no shadow / border / outline / radius fields; the doc at `extract.rs:64` literally says "shadow/border/glyph fields are added by their tier."

- **Pack hardcodes `radius: 0.0`.** `pack_extracted` (`instance.rs:117-142`) sets `radius: 0.0` with the comment "per-node corner radius is not yet on the extract record, so v1 packs square quads." `PackedInstance` (`instance.rs:67-91`) carries one `radius: f32` (slot 5, the byte the shadow shader reinterprets as blur), the clip AABB, and the 4-float affine — **no** blur, **no** spread, **no** per-side border widths, **no** per-side colors, **no** per-corner radius. Stride is frozen at 68 B = `[f32;17]` (`instance.rs:33`, `PACKED_INSTANCE_STRIDE_BYTES`), pinned by `packed_raw_stride_agrees()` (`instance.rs:168-171`).

- **Buckets only ever emit `(Quad, layer 0)`.** `pack_view` / `pack_view_partitioned` (`buckets.rs:176-189,235-286`) route every node to the single `Quad` batch. `BuiyPrimitiveKind` reserves `Shadow/Quad/Glyph/Path` with `paint_order()` `shadow<quad<glyph<path` (`buckets.rs:30-54`), and `PrimitiveBatchKey`'s `Ord` is layer-then-paint-order (`buckets.rs:65-78`) — so emitting `Shadow` into its bucket already draws it behind the quad **for free** — but no shadow / border-band / outline instance is ever produced.

- **Shaders: shadow exists and is unfed; quad has no band.** `shadow.wgsl` exists, shares the **exact** 68 B instance layout, reinterprets slot 5 (`@location(5) blur`) as the effective blur sigma (`shadow.wgsl:5,31`), and implements the Abramowitz-Stegun erf closed-form Gaussian-blurred box (`shadow.wgsl:74-109`). The `Shadow` pipeline is *buildable* — `primitive.rs:228` maps `Shadow → shadow_shader_handle()`, `quad_family_vertex_buffers()` works for it, the UUID octet `..02` is reserved (`pipeline.rs:46-55`) — but it is **never specialized**: `prepare_buiy_view_pipelines` (`pipeline.rs:300-327`) builds only `Quad` + `Glyph`, `BuiyViewPipelines` (`pipeline.rs:282-288`) carries only those two ids, `BuiyInstanceBuffers` (`prepare.rs:88-143`) has only quad + glyph buffers/counts, and `buiy_pass` (`node.rs:72-435`) draws only quad ranges + glyph ranges — **no shadow draw anywhere**. `shader.wgsl` (`shader.wgsl:68-91`) has only the **outer** rounded-rect SDF — no outer-minus-inner band, no per-side color. The CPU band oracle (`tests/render_border_sdf.rs`) is present and unused by any shader.

- **Forced-colors draw-skip is absent.** The static analyzer (`forced_colors_analyzer.rs`) and the main-world `Theme` swap (`forced_colors.rs::apply_forced_colors_theme`) exist and are correct; the **16-key system-color stub map already exists** on `main` (`theme.rs:110-145` `forced_colors_theme()` + `SystemColorKeyword::ALL`). But the **draw-time `BoxShadow`-suppression branch** ("skip the Shadow bucket when `UserPreferences.forced_colors`") does **not** exist — extract never reads `UserPreferences`. `color-and-forced-colors.md:162` requires this as "one branch in extract."

- **Focus ring is read by no paint system.** Neither `FocusVisible` nor `Outline` is read by any extract/paint path (grep-confirmed; audit:41,142). `FocusVisible` is today a **global `Resource`** (`focus.rs:45`), `FocusVisible(pub bool)`, set true on Tab and never reset (Phase-0 keyboard-only). `FocusedEntity(pub Option<Entity>)` (`focus.rs:38`) names *which* entity. There is no per-entity "focused-and-keyboard-visible" marker yet — the umbrella §6.6 pins that C6 confirms this component's shape with C3/C5 before designing the lowering. See §3.6.

- **The render-pipeline status note overstates F-tier paint.** `render-pipeline-design/README.md:7` claims "R1–R11 plus the GPU-verify campaign all landed — the §3.2 component model and **every pass** described here exist in `render/*.rs` and are verified on real hardware," and README:11 lists box-shadow / borders / outlines as in-scope/covered. This is **false for paint**: no shadow / border-band / outline pass exists in the live `ExtractedNode → bucket → draw` path. The audit (`audit:141`) confirms "the prototype's flat-render table is more truthful than the spec's status note." Umbrella §3 supersede-item 3 requires C6 to **flip** the note (not delete it) when C6 lands.

**What is already correct and reused (do not rebuild):** the `Border`/`BorderSide`/`Corners`/`Radius`/`BoxShadow`/`Shadow`/`Outline`/`LineStyle` components (`components.rs:94-249`); `BoxModel.border: Edges` as the layout-owned border *width* (`layout/components.rs:45`) — `ResolvedLayout.size` is the **border box**, so width measures inward; `clip_for_primitive` + `AncestorClip` (delivered by C1, `clip.rs:138`, wired in extract at `extract.rs:306-317`); `px_or_zero` for paint-`Length` resolution (`clip.rs:93`); the closed-form shadow shader; the band oracle; the forced-colors stub + main-world swap + analyzer.

## 2. Target design

The migration is **additive** to the hot extract/pack/draw spine, in the sequence the grounding pins (§5). The keystone decision (umbrella §6.7, resolved §3.3): **shadow reuses the frozen 68 B quad layout** (radius slot → blur sigma) with **zero stride change**; **border + outline ride a distinct quad-variant record** (its own `RawBufferVec` + `VertexBufferLayout`, the precedent `GlyphAlphaInstance` set) so `PackedInstance` stays byte-identical `[f32;17]`.

### 2.1 Extract: grow the fan + the damage union

Add to the `extract_buiy_nodes` fan (`extract.rs:347`) and the damage `Or`-set (`extract.rs:391`) in lockstep (the file's own "FAN:" / "Or-set in lockstep" comments mark the exact insertion points):

```rust
// added to the `nodes` Query tuple:
Option<&Border>, Option<&BoxShadow>, Option<&Outline>,
// added to the system params:
prefs: Extract<Res<UserPreferences>>,           // forced-colors (architecture.md:91 — F-tier, v1 fan)
box_models: Extract<Query<&BoxModel, With<Node>>>, // border WIDTH (layout-owned Edges)
// added to the damage Or-set:
Changed<Border>, Changed<BoxShadow>, Changed<Outline>,
// (BoxModel.border change already re-runs layout → Changed<ResolvedLayout>; add
//  Changed<BoxModel> only if a width-only edit can occur without a re-layout — it cannot,
//  border width is a Taffy input, so it is NOT added: see §3.5 risk note.)
```

`UserPreferences.forced_colors` is *not* a per-entity `Changed` term; it rides the existing global `theme.is_changed()` re-extract edge (the forced-colors swap mutates `Res<Theme>`, `forced_colors.rs`), so a forced-colors flip already re-extracts every node — the new draw-skip branch then reads the flag. (Add `Changed<UserPreferences>` to the gate only if a future path can flip `forced_colors` without swapping the theme; today the swap is the only producer, so it is unnecessary.)

`ExtractedNode` (`extract.rs:65`) grows three optional sub-records, all **resolved at extract time** (tokens → linear `Color` via `color::resolve_token`, the canonical resolver — no second mapping):

```rust
pub struct ExtractedNode {
    // ...existing: entity, position, size, affine, color, clip, group...
    /// Per-corner elliptical radius in logical px, resolved from `Border.radius`
    /// (None == square, the byte-identical fast path). Folds into BOTH the quad
    /// fill AND the band; carried on the border-variant record (§2.3).
    pub radius: CornerRadiiPx,                       // [[f32;2];4] = ((rx,ry) × 4 corners)
    /// Per-side border paint: resolved color + width px + style, one per side.
    /// `None` == no border band (the no-border quad path stays pixel-stable).
    pub border: Option<ExtractedBorder>,
    /// Resolved outline (color/width/offset px). `None` == no outline.
    pub outline: Option<ExtractedOutline>,
    /// Resolved, spread/offset-expanded, blur-sigma'd shadow terms, in CSS
    /// list order (index 0 frontmost). Empty == no shadow. Suppressed at the
    /// PRODUCER when forced-colors is active (§2.5) — the vec is then empty.
    pub shadows: Vec<ExtractedShadow>,
}
```

The per-corner radius **completes the broken uniform-radius migration**: the legacy `uniform_radius_px` (`mod.rs:340`) read `Border.radius.top_left.x` only (px-only, loses 3 corners + both y-radii); the live `pack_extracted` is worse (hardcodes `0`). The fix carries all 4 corners × (x,y) = 8 floats, each `px_or_zero`-resolved, clamped to `≤ min(half_w, half_h)` per CSS overlap rules.

### 2.2 Shadow: pack into the reserved `(Shadow, layer)` bucket — zero stride change

Shadow needs **no** quad-stride bump. `shadow.wgsl` already shares the identical 68 B layout with `radius` (slot 5) reinterpreted as blur sigma. Add a `pack_shadow(&ExtractedShadow) -> PackedInstance` that produces a `PackedInstance` whose:

- `rect_pos`/`rect_size` = the **spread-and-offset-expanded** box (border box grown by `spread`, translated by `(offset_x, offset_y)`), CPU-computed — one `PackedInstance` per shadow entry (`architecture.md:174` "one draw per shadow"; `component-model.md:256-260`);
- `radius` field = the **effective blur sigma** (§3.4 — the CSS-blur→sigma factor decision);
- `color` = the resolved, CPU-linearized shadow color;
- `clip`/`affine` = the same fields the node already carries.

`pack_view` / `pack_view_partitioned` (`buckets.rs`) gain a Shadow pass **before** the quad push: for each node, push each `ExtractedShadow` to `(Shadow, layer)`, then push the node's quad to `(Quad, layer)`. Because `PrimitiveBatchKey::Ord` is layer-then-paint-order and `Shadow.paint_order() = 0 < Quad = 1`, the `BTreeMap` iteration already orders shadow-before-quad-within-layer — no draw-loop re-sort. (v1 ships **outset** shadows; inset deferred — §3.1.)

`BuiyInstanceBuffers` (`prepare.rs:88`) grows a `shadow: RawBufferVec<[f32;17]>` + `shadow_count: u32` + the shadow group/flat range partition (the shadow primitive participates in effect-group partitioning identically to the quad — the `RangePartitioner` is already primitive-agnostic, `buckets.rs:329`). `prepare_buiy_instances` packs and uploads it under the same quad-dirty gate (`prepare.rs:230` — shadows ride the node walk, so the quad gate covers them).

`prepare_buiy_view_pipelines` (`pipeline.rs:300`) specializes a third id, `shadow: CachedRenderPipelineId`, via `BuiyPrimitiveKey { kind: Shadow, format, samples }` — exactly the quad/glyph idiom; `BuiyViewPipelines` (`pipeline.rs:282`) grows the field; `register` (`pipeline.rs:244`) adds the eager `Shadow@Rgba8UnormSrgb@1x` baseline. `buiy_pass` (`node.rs:303`) adds a **shadow draw loop before the quad loop**, mirroring the glyph loop after — same `flat_ranges`/`group_ranges` exclusion, same view-uniform `@group(0)`.

### 2.3 Border: outer-minus-inner SDF band in a DISTINCT quad-variant record

Border pressures the quad stride (per-side widths + per-side colors + inner radius). To protect R1/R2 byte-stability (umbrella §6.7), border + outline get a **distinct instance record** — its own `RawBufferVec` + `VertexBufferLayout` — exactly as `GlyphAlphaInstance` is a separate 68 B layout sharing the quad pipeline machinery (`primitive.rs:147-217`):

```rust
/// The border/outline quad-variant instance. A DISTINCT record from
/// `PackedInstance` (NOT a stride bump): own RawBufferVec, own VertexBufferLayout,
/// own shader (`band.wgsl`, octet ..06). Painted through a `BorderBand`
/// quad-variant pipeline. The no-border quad path is UNTOUCHED — pixel-stable.
#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
pub struct BorderBandInstance {
    pub rect_pos: [f32; 2],            // border box top-left, logical px
    pub rect_size: [f32; 2],
    pub color_top: [f32; 4],           // per-side resolved linear color
    pub color_right: [f32; 4],
    pub color_bottom: [f32; 4],
    pub color_left: [f32; 4],
    pub width: [f32; 4],               // [top, right, bottom, left] px
    pub outer_radius: [f32; 8],        // per-corner (rx, ry) × 4
    pub inner_radius: [f32; 8],        // outer shrunk by adjacent widths (oracle rule)
    pub clip_min: [f32; 2],
    pub clip_max: [f32; 2],
    pub affine: [f32; 4],
}
```

The band fragment is the `render_border_sdf.rs` oracle made GPU: a fragment is painted iff `inside(outer_rounded_rect) AND NOT inside(inner_rounded_rect)`, where `inner_half = outer_half - width` and `inner_r = max(outer_r - adjacent_width, 0)` (the oracle's load-bearing shrink, `render_border_sdf.rs:66-101`). Per-side color selects on the fragment's dominant edge (quadrant by `sign(local)` + which axis the band touches). The band routes to `(Quad, layer)` as a **variant within the quad batch's draw position** — it draws in the same per-layer slot as the fill so border paints over fill (CSS: border paints after background within a box). v1 simplest-correct: emit the band record only when at least one side has non-`None` style and non-zero width; the radius alone (no border) rounds the **fill** via the existing `PackedInstance.radius` (now fed, §2.1) through `shader.wgsl`'s existing outer SDF — so a rounded-but-borderless box needs **no** band record (it rides the cheaper quad path with a real radius). This is why the per-corner radius lives on `PackedInstance.radius` *and* on the band record: the borderless rounded box uses the former, the bordered box uses the latter.

> **The two-records decision (umbrella §6.7 / open Q resolved §3.3).** v1 keeps **two records**: the byte-stable `[f32;17]` `PackedInstance` for fill (now with a real radius) + the distinct `BorderBandInstance` for border/outline. It does NOT migrate every quad to a unified wide record. Rationale + rejected alternative in §3.3.

### 2.4 Outline: clip-suppressed quad-variant via `AncestorClip`

`Outline` reuses the **same `BorderBandInstance` + band pipeline** (it is a band: a stroked ring outside the border box). The differences are pure CPU-side geometry + clip selection:

- box = border box grown by `width + offset` on all sides; `outer_radius` = `Border.radius` grown by `(width + offset)` (CSS-faithful outline rounding, `component-model.md:362`); `inner_radius` = the original outer radius / inner edge so the ring is `width` thick;
- clip = the entity's **`AncestorClip`**, never its own `ClipRect` (`component-model.md:348-359`) — so a focus ring survives an `overflow:hidden` ancestor (WCAG 2.4.7 / 2.4.11). `extract.rs` already resolves `AncestorClip` (`extract.rs:306`); the outline record packs `AncestorClip` (not the own-box clip) into its `clip_min`/`clip_max`. Top-layer members force `None` (full-view) exactly like the fill (`extract.rs:311`).

Outline draws in the **top of its layer** (after fill + border + glyph within the box) so it sits on top; routing it to `(Quad, layer)` after the band keeps the order. Because the box is `width + offset` outside the fill, the outline's painted bounds extend the node's ink box — the bounds expansion is folded through C1's non-optional `GlobalTransform` (umbrella §6.2; the affine carried on the record).

### 2.5 Forced-colors: `Extract<Res<UserPreferences>>` + skip the Shadow bucket

Add `prefs: Extract<Res<UserPreferences>>` to the fan (§2.1). When `prefs.forced_colors` is set, the **shadow producer emits no `ExtractedShadow`** (the `shadows` vec stays empty) — one branch at the producer, so the suppression is structural, not per-widget (`color-and-forced-colors.md:160-164`). Border / Outline / Background **survive** untouched: their colors already resolve through the forced-colors `Theme` variant the main-world swap installed (`forced_colors.rs`), remapping to `ButtonBorder` / `CanvasText` / `Highlight` via the existing token edge — **no extra branch** for them. This makes the §3.2 structural-cue guarantee self-enforcing (a shadow-only state affordance becomes invisible under forced-colors, so it must have a non-shadow cue — the analyzer + golden prove it). The 16-key stub map this resolves against **already exists** on `main` (`theme.rs:110`); C6 confirms it (it does not re-ship it) — umbrella §6.8.

### 2.6 Focus ring: main-world lowering `FocusVisible/Focused` → `Outline`

A small **main-world** system lowers keyboard-visible focus into an `Outline` carrying a forced-colors-safe `Highlight` token, so the ring is **just an `Outline`** — no new render path (§2.4 paints it). The system (in `buiy_core`, scheduled in `BuiySet::Style` before `BuiySet::Render`, so the `Outline` is present when extract runs):

```rust
fn lower_focus_ring(
    focused: Res<FocusedEntity>, visible: Res<FocusVisible>,
    mut commands: Commands,
    rings: Query<Entity, With<FocusRingMarker>>,     // entities we own a ring on
) {
    // remove the ring from any entity that is no longer the visibly-focused one;
    // insert Outline{ color: SystemColor(Highlight), style: Solid, width: 2px, offset: 2px }
    //   on focused.0 when visible.0 is true (WCAG: >=2px, >=3:1 — Highlight satisfies it).
}
```

`width ≥ 2px` and the `Highlight` token (≥3:1 vs `Canvas`) satisfy WCAG 2.4.11. The ring `Outline` is a **framework-owned** component the lowering inserts/removes — never author-set — so an author's own `Outline` is distinguished by the `FocusRingMarker` (the lowering only touches rings it owns).

> **Component-shape confirmation with C3/C5 (umbrella §6.6, resolved §3.6).** Today `FocusVisible` is a global `Resource(bool)` + `FocusedEntity(Option<Entity>)` (`focus.rs:38-45`). C3 provides focus-on-click + the `:focus-visible` decay *signal*; C5 owns the focus *tree*. **C6 reads whatever single signal "this entity is keyboard-focused-and-visible" C3/C5 settle on** and does not invent its own. v1 reads the *resource pair* that exists today (`FocusedEntity` + `FocusVisible`); if C3/C5 promote it to a per-entity `FocusVisible` marker component, the lowering switches its query with no change to the ring `Outline` or the paint path. This is a one-line read-site change, deliberately isolated.
>
> **No collision with C5's focus-tree writes (confirmed against umbrella §6.6 / §6.4).** The split is read-only on C6's side: `lower_focus_ring` only **reads** the settled `FocusVisible`/`FocusedEntity` signal (owned by C3/C5) and only **inserts/removes a paint-only `Outline`** (gated by `FocusRingMarker`, framework-owned) on the focused entity. It writes **no** focus-tree state — not `FocusedEntity`, not `FocusVisible`, not the scope/trap/roving/restoration/inert machinery C5 owns. So the lowering cannot race or collide with C5's focus-tree writes: C3/C5 are the sole producers of the focus signal; C6 is a pure paint-side consumer. This is exactly the §6.4 partition (C3/C5 own the signal/tree, C6 owns the ring paint) made explicit at the write boundary.

### 2.7 Reconcile the render-pipeline status note (flip, don't delete)

When C6 lands, edit `render-pipeline-design/README.md:7` from "every pass … exist[s] … and [is] verified on real hardware" to the truthful split: "component model + clip/effect/atlas/text passes landed; **F-tier shadow / border-band / outline PAINT landed via the widget-catalog C6 child** (or, pre-landing, *reserved-but-unfed, tracked here*)." Update README:11 / `component-model.md §5/§7` status prose the same way. This is umbrella §3 supersede-item 3 and is part of "done" for this child.

## 3. Decisions & rejected alternatives

### 3.1 Inset shadow + per-side color in v1 vs fast-follow — **RESOLVED: ship outset + full per-side color in v1; defer inset to a fast-follow**

- **Outset shadow: v1.** It is the common case, the closed-form Gaussian-box shader already handles it, and cards/buttons/modals in the C8 gallery need it. Ships now.
- **Inset shadow: deferred fast-follow.** `component-model.md:233` says inset paints **above** the fill **clipped to the padding box** — that needs a padding-box clip and a different SDF sense (the separable-product approximation in `shadow.wgsl:101-108` does not handle the inner-glow case cleanly). The `Shadow.inset: bool` field already exists (`components.rs:208`); v1 packs only `!inset` terms and `warn!`s once on an `inset` term, exactly mirroring layout's warn-and-fallback idiom. Adding inset later is purely additive (a second SDF branch + the padding-box clip) — no stride or record change. *Rejected — "ship inset too":* the padding-box clip + corrected inner SDF is a meaningful shader+geometry slice the gallery does not need, and shipping it half-correct (separable product) would bless a wrong golden.
- **Per-side border color: v1 (full).** The distinct `BorderBandInstance` record (§2.3) already has room for 4 colors + 4 widths at no R1/R2 cost — once we pay for the distinct record, per-side color is *free* (4 extra `vec4`s). *Rejected — "single border color in v1, per-side fast-follow":* it would only save 12 floats on a record we are minting anyway, and would force a second migration of the same record when per-side lands. The foundation tiers per-side longhands **F** (`visuals.md:128`); ship them. (Per-side *style* — dashed/dotted — still renders as `Solid` in v1; the dash shader is C-tier, `components.rs:96`.)

### 3.2 The CSS blur-radius → Gaussian-sigma factor — **RESOLVED: `sigma = blur / 2`**

CSS `box-shadow` blur radius is defined (CSS Backgrounds 3 §7.1) such that a blur of `B` produces a shadow whose edge transitions over roughly `B` px, which corresponds to a Gaussian standard deviation of **`sigma = B / 2`**. This is the factor Chromium/WebKit/Firefox use and what gpui's closed-form shadow assumes. `shadow.wgsl:5` says "maps `BoxShadow.blur` into it" but leaves the factor unpinned; **pin it at the producer** (`pack_shadow`): `let sigma = px_or_zero(shadow.blur) * 0.5;` (clamped to a `≥ 1e-4` floor, which the shader already guards, `shadow.wgsl:87`). Pinning it CPU-side (not in the shader) keeps the shader a pure consumer and makes the factor a single, testable constant the goldens reproduce. *Rejected — "`sigma = blur`":* over-blurs by 2× vs every browser, so any cross-referenced visual would diverge; `blur/2` is the de-facto standard.

### 3.3 Two records vs migrate-every-quad — **RESOLVED: two records (byte-stable `PackedInstance` fill + distinct `BorderBandInstance`)**

Keep the R1/R2-frozen `[f32;17]` `PackedInstance` for fill + shadow (shadow reuses it, radius→sigma) and add a **distinct** `BorderBandInstance` for border + outline. *Rejected — "migrate every quad to one wide record" (the open-Q alternative):* it is cleaner conceptually (one record, per-corner radius + per-side border everywhere) but it is a **byte-stability event** of exactly the kind umbrella §6.7 forbids — it would move `COLOR_FLOAT_OFFSET`/`ALPHA_FLOAT_OFFSET` (`instance.rs:40,46`), break the R2 degraded-group re-tint that indexes alpha at offset 7, force `packed_raw_stride_agrees()` + `render_instance.rs` + every CPU buffer-byte assertion to re-bless, and risk perturbing AA on every existing quad golden. The borderless rounded box still gets correct corners (via the now-fed `PackedInstance.radius` + the existing outer SDF), so the *only* thing the wide record buys is collapsing two pipelines into one — not worth the byte event. The distinct-record precedent is already proven by `GlyphAlphaInstance` (`primitive.rs:147`, a separate 68 B layout on the same machinery).

### 3.4 Band pipeline as a new `BuiyPrimitiveKind` vs a quad-pipeline variant — **RESOLVED: a distinct band pipeline keyed by record, NOT a new `BuiyPrimitiveKind`**

`BuiyPrimitiveKind` (`buckets.rs:30`) is the authoritative `Shadow/Quad/Glyph/Path` set and its doc explicitly says "`Border` is **not** a distinct primitive — it folds into `Quad` … `Outline` is a `Quad` variant." Respect that: the band is a **variant** that draws within the `(Quad, layer)` slot, with its own pipeline (own shader `band.wgsl` octet `..06`, own vertex layout) selected by the *record type*, not a new enum discriminant. This keeps the paint-order taxonomy intact (`paint_order()` unchanged) while honoring the byte-distinct record. *Rejected — "add `Border` to `BuiyPrimitiveKind`":* it would contradict the spec's primitive set and complicate the `paint_order` ranking (border draws *between* fill and glyph within a box, not as a separate global layer).

### 3.5 Border width source — **RESOLVED: read `BoxModel.border` (layout-owned); do NOT add a width to the render `Border`**

Border *width* affects box sizing, so it lives in `BoxModel.border: Edges` (`layout/components.rs:45`, `component-model.md:148-153`) and is a Taffy input. The render `Border` component carries only paint (color/style/radius). Extract reads `BoxModel.border` per node (resolved px via `px_or_zero`) for the band's `width` field. A width-only change re-runs layout (it is a Taffy input) → `Changed<ResolvedLayout>` → re-extract; so `Changed<BoxModel>` is **not** added to the damage gate (it would be redundant; the §2.1 comment records this). *Rejected — "duplicate width onto render `Border`":* it would create a second source of truth for a layout-affecting value and risk fill/band/layout desync.

### 3.6 Focus-ring source component — **RESOLVED: read the C3/C5 focus-visible signal; v1 reads today's `FocusedEntity` + `FocusVisible` resources**

Resolved in §2.6. Per umbrella §6.6, C6 does not own the focus-visible component shape — it consumes it. v1 reads the resource pair on `main`; a promotion to a per-entity marker is a one-line read-site swap. **Confirmed against umbrella §6.6:** the lowering only **reads** the settled `FocusVisible`/`FocusedEntity` signal and only **inserts/removes a paint-only `Outline`** (gated by `FocusRingMarker`); it writes **no** focus-tree state, so it does not collide with C5's focus-tree writes — C6 stays a pure paint-side consumer (the §6.4 partition at the write boundary, §2.6). *Rejected — "C6 mints its own per-entity focus-visible component":* it would duplicate C3/C5's signal and risk drift (the very conflation §6.4/§6.6 partition away).

## 4. Contracts & interfaces

**Shared contracts referenced (umbrella §6 — referenced, not redefined):**
- **§6.7 R1/R2 byte-stability** — the 68 B `PackedInstance` stride is frozen; shadow reuses it (radius→sigma); border/outline get a distinct `BorderBandInstance` record; the no-border quad path stays pixel-stable; **C7 owns the byte-assertion tests** (`packed_raw_stride_agrees()` must still pass unchanged). This child adds a parallel `border_band_stride_agrees()` for the new record.
- **§6.8 forced-colors 16-key stub map** — already on `main` (`theme.rs:110`); C6 confirms + wires the suppression branch against it, does not re-ship it; the full map stays `buiy-theme-tokens-design`'s.
- **§6.6 focus-visible component shape** — produced by C3/C5, consumed by C6's ring lowering (§2.6 / §3.6); C6 reads the settled signal.
- **§6.2 coordinate space (C1)** — shadow/outline ink-bounds + the `AncestorClip` the outline uses fold through non-optional `GlobalTransform`; C1 strictly precedes this child's outline work. `bridge.rs:138` is an invariant to preserve.
- **§6.4 focus split** — C6 owns the ring **paint**; C3/C5 own the signal/tree; C4 consumes `FocusedEntity`.
- **§6.1 pick-depth / paint-order** — C6 emits Shadow/Quad/band/Outline into the existing `painters_z`-ordered buckets; it never re-sorts (pillar 1).

**Own contracts this child defines precisely:**
- `BorderBandInstance` (§2.3): the distinct 68-B-class border/outline record, its `RawBufferVec`, its `VertexBufferLayout` (octet `..06` band shader), and the rule "fill uses `PackedInstance.radius`; bordered box uses the band record; outline uses the band record clipped by `AncestorClip`."
- `pack_shadow` (§2.2): `PackedInstance` with `radius := sigma = blur/2`, `rect := border-box ⊕ spread ⊕ offset`, routed to `(Shadow, layer)`.
- The forced-colors **producer-side** shadow-suppression branch (§2.5): `if prefs.forced_colors { shadows = [] }`.
- The `lower_focus_ring` main-world system + `FocusRingMarker` (§2.6): framework-owned `Outline{ Highlight, Solid, 2px, 2px }` on the visibly-focused entity.
- The `BuiyViewPipelines.shadow` + `.band` ids and the `BuiyInstanceBuffers.shadow`/`.border_band` buffers + counts + partitions (§2.2/§2.3).

## 5. Migration / build steps (ordered, with blast radius)

The grounding's eight-step sequence, each step RED-first gated by C7 (§6). **Blast radius is moderate-to-large, mostly additive, on the hot extract/pack/draw spine.**

1. **Extract fan + damage union grow** (`extract.rs:347,391`): add `Border`/`BoxShadow`/`Outline`/`UserPreferences`/`BoxModel` reads + the 3 `Changed` terms; grow `ExtractedNode` with `radius`/`border`/`outline`/`shadows`, resolved at extract. *Touches the most-tested system; existing extract tests (`render_extract*.rs`) stay green (additive `Option` terms) but new fields need new assertions.*
2. **Per-corner radius onto `PackedInstance.radius`** for the fill (`instance.rs:117`): replace `radius: 0.0` with the resolved uniform-or-min radius; completes the broken `uniform_radius_px` migration. *Existing radius-0 quad goldens must stay pixel-stable for unstyled nodes (radius still 0 when no `Border`).*
3. **`pack_shadow` + `(Shadow, layer)` bucket + shadow pipeline + shadow buffer/count + `node.rs` shadow draw loop** (`buckets.rs`, `instance.rs`, `pipeline.rs:244,300`, `prepare.rs:88`, `node.rs:303`). *Additive; the shadow pipeline is buildable today, only never specialized.*
4. **Quad-variant band: `BorderBandInstance` + `band.wgsl` + band pipeline + band buffer + `node.rs` band draw** (`instance.rs`, `primitive.rs`, `pipeline.rs`, `prepare.rs`, `node.rs`), validated against `render_border_sdf.rs`. *New shader; the existing solid-fill fast path must stay byte-identical for radius-0 no-border quads so existing quad goldens don't move.*
5. **Outline via the band pipeline using `AncestorClip`** (geometry + clip selection in extract/pack). *Reuses step 4's pipeline; no new shader.*
6. **Forced-colors shadow-skip at the producer** (`extract.rs` shadow producer). *One branch; `render_forced_colors_swap.rs` covers the theme swap, the new skip needs its own test (`prefs.forced_colors → shadows empty`).*
7. **Focus-ring lowering** (`buiy_core` new `lower_focus_ring` system + `FocusRingMarker`, scheduled `BuiySet::Style`). *Main-world only; reads the C3/C5 signal.*
8. **Flip the render-pipeline status note** (`render-pipeline-design/README.md:7,11`, `component-model.md §5/§7`) + update `docs/plans/follow-ups.md` + the docs catalog. *Doc-only; part of done.*

**Goldens (GPU `--ignored` Tier-5 lane):** NEW goldens for shadow / border-band / outline / focus-ring; the forced-colors golden **re-blessed** (shadows now suppressed where before nothing was drawn). Per `CLAUDE.md` the GPU lane is EXACT-stable on the RX 6700 XT but residue goldens were re-blessed on CI lavapipe — new goldens must be blessed on a real adapter and may need a lavapipe re-bless. **The byte-stable assertions (`packed_raw_stride_agrees`, `render_instance.rs`) must still pass unchanged** — the trap is bumping the quad stride; the distinct record avoids it.

**Specs touched:** this file (`[draft]→[active]` when landed), the render-pipeline README/component-model status notes (flipped), `follow-ups.md`, the docs catalog.

## 6. Verification (how C7 gates this; RED-first)

C7 (Tier-A real-input harness) is *not* this child's gate — C6's gates are the **render verification tiers** (the GPU `--ignored` lane, Tiers 4-5, `buiy_verify`), plus headless byte/skip asserts. Every new predicate lands **RED-first** (proven to fail before the channel is wired), per umbrella §9 risk #5:

- **Tier-4 reftest + SDF cross-check (headless-buildable logic, `#[ignore]` GPU):**
  - **Border band:** wire the existing `render_border_sdf.rs` oracle as the CPU↔GPU SDF cross-check for `band.wgsl` (the oracle currently validates nothing wired — §1). RED: no band shader exists, so the cross-check has no GPU side to compare.
  - **Focus ring survives `overflow:hidden`:** a reftest asserting the outline (clipped by `AncestorClip`) is painted outside a clipping ancestor; RED before §2.4 (no outline paints at all).
  - **Shadow presence:** a reftest asserting a node with `BoxShadow` emits a `(Shadow, layer)` instance ahead of its quad; RED before §2.2.
- **Tier-5 GPU goldens (`#[ignore]`, real adapter):**
  - shadow blur (the `blur/2` sigma reproduced); **inset deferred** (warn-once asserted instead);
  - per-side border band + per-corner elliptical radius;
  - outline-survives-clip (visual confirmation of the reftest);
  - focus-ring golden (≥2px `Highlight` ring on a focused widget);
  - **forced-colors golden re-blessed with shadows suppressed** — the §3.2 structural-cue half: a focused/pressed widget still shows a non-shadow delta (border/fill/outline) under forced-colors.
- **Headless byte/skip asserts (no GPU):**
  - `packed_raw_stride_agrees()` still passes **unchanged** (§6.7) + a new `border_band_stride_agrees()` for `BorderBandInstance`;
  - `prefs.forced_colors` true ⟹ extract produces an empty `shadows` vec (forced-colors draw-skip); RED before §2.5;
  - the per-corner radius migration: a `Border{radius: Corners}` with distinct per-corner radii produces 8 distinct resolved floats (not `top_left.x` × everything) — RED against the legacy `uniform_radius_px`;
  - the static forced-colors analyzer (`forced_colors_analyzer.rs`) over the C8 catalog asserts every state has a non-shadow cue (already green for the analyzer; the *fixture* is C8's).

**The vacuous-green trap (umbrella §9 #5):** the band oracle and shadow shader exist *unused* today — a test that merely calls the oracle proves nothing about the GPU path. Each predicate must fail with the channel unwired (the shadow draw loop absent, the band pipeline unspecialized, the forced-colors branch absent) and pass once wired — proven by reverting the wiring and re-running.

## 7. Open questions deferred + dependencies

**Deferred (genuinely depend on un-built work):**
- **Inset shadow** — deferred to a fast-follow (§3.1); needs the padding-box clip + corrected inner SDF the C8 gallery does not require. The `Shadow.inset` field exists; v1 warns-once.
- **Per-side dashed/dotted/double border render** — `LineStyle` beyond `Solid` is C-tier (`components.rs:96`); renders as `Solid` until the bevel/dash shader.
- **`currentColor` on border/outline** — the inherited-text-color carrier is owned by `buiy-text-rendering-design`; v1 falls back to `color.text.primary` / `CanvasText` (`color-and-forced-colors.md:83`). Not blocking — the fallback resolves today.

**Dependencies (must land first / coordinate):**
- **C1** (coordinate space): non-optional `GlobalTransform` + the `clip.rs:286` fix + `AncestorClip` (already present) gate the outline ink-bounds + clip selection. C1 strictly precedes C6's outline work (umbrella §6.2).
- **C3/C5** (focus signal): the focus-visible component shape the ring lowering reads (§2.6 / §3.6); v1 reads today's `FocusedEntity`+`FocusVisible` resources, swaps to whatever C3/C5 settle on in one read-site change (umbrella §6.6).
- **C4** (widget state): which channel (border/fill/outline) signals each state (pressed/checked/selected) — C6 paints whatever channel C4 chooses; the forced-colors §3.2 structural-cue check asserts each state has a non-shadow delta, so C4 and C6 must agree the state cue is not shadow-only. The per-widget roster is C4/C8's.
- **C7** (verification): the GPU `--ignored` lane hosts C6's shadow/border/outline/focus-ring goldens + reftests; C6 adds fixtures there and depends on the `buiy_verify` harness.
- **buiy-theme-tokens-design** (cross-spec): the *full* forced-colors system-color map; C6 uses the in-campaign 16-key stub (umbrella §6.8) which already exists on `main`.
