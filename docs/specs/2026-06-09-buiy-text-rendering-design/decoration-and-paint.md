# Buiy text — decoration, selection, and caret painting

**Parent:** [README.md](README.md)

This file owns the painting of every non-glyph text visual: `text-decoration`
lines, `::selection` rectangles and selected-text color, the caret
(geometry-to-pixels, `caret-color`, blink, reduced-motion), `::placeholder`,
and the preedit-underline painting primitive the IME flow consumes. Foundation
rows mapped ([text.md](../2026-05-07-buiy-foundation/text.md#34-typography)):
`text-decoration-line` / `-color` (**F**, :51); `::selection` + `::placeholder`
(**F**, :73); visual selection rectangles for mixed-direction lines (**F**, :89);
caret color/style with reduced-motion blink + `caret-color` (**F**, :90–91,
painting half); placeholder text (**F**, :83, painting half); preedit rendering
(**F**, :96, painting primitive only).

Everything here is a *producer* of already-built, GPU-verified render
primitives — nothing in the render seam is redesigned: quads ride the quad
bucket ([buckets.rs:30–53](../../../crates/buiy_core/src/render/buckets.rs)),
coverage stamps ride `GlyphAlphaInstance` (68 B,
[atlas/primitive.rs:28–48](../../../crates/buiy_core/src/render/atlas/primitive.rs))
into `ExtractedGlyphs`
([prepare.rs:39–51](../../../crates/buiy_core/src/render/prepare.rs)).

---

## 1. Scope and seams

| Concern | Owner |
|---|---|
| Decoration / selection / caret / placeholder / preedit **painting** (geometry → instances, paint-order seats, color resolution) | **this file** |
| Caret + selection **model** (cursor logic, motion, multi-range selection, IME state machine, undo) | [editing-and-ime.md](editing-and-ime.md) (model) + the successor `buiy-text-editing-design` campaign |
| Glyph shaping, run geometry (`layout_runs`), rasterization, atlas insertion, glyph-instance emission | [glyph-pipeline.md](glyph-pipeline.md) |
| `FontSystem` ownership, `Buffer` lifecycle, schedule placement, damage discipline | [architecture.md](architecture.md) |
| Atlas machinery, `GlyphAlphaInstance` / `IconInstance` shapes, paint-rank enum | built — [render atlas-and-text-seam.md](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md) |
| Token values (`color.selection.bg/.fg`, caret token) | `buiy-theme-tokens-design` (this file fixes the *resolution order*, not the palette) |

This file consumes run geometry and the selection/caret positions; it produces
quad-bucket instances and solid-coverage glyph stamps. No cosmic-text type
crosses into the render crate (the seam contract,
[atlas-and-text-seam.md § 5](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#5-where-text-plugs-in-the-seam-mechanically)).

---

## 2. The decoration dataflow — inherit cosmic-text's spans

### 2.1 The 0.19 surface

cosmic-text 0.19 carries decoration intent *into* shaping and decoration
geometry *out of* layout (verified against docs.rs 0.19.0 + the 0.19.0 tag
source; the [prior-art capabilities table](../../prior-art/cosmic-text/capabilities.md)
already stanced "Inherit" for these rows):

- **In:** `Attrs.text_decoration: TextDecoration` with builders `.underline(UnderlineStyle)`,
  `.underline_color(Color)`, `.strikethrough()` / `.strikethrough_color(Color)`,
  `.overline()` / `.overline_color(Color)`.
  `UnderlineStyle { None, Single, Double /* TODO: Wavy */ }`.
- **Out:** `LayoutRun.decorations: &[DecorationSpan]` where
  `DecorationSpan { glyph_range: Range<usize>, data: GlyphDecorationData, color_opt: Option<Color>, font_size: f32 }`
  and `GlyphDecorationData { text_decoration, underline_metrics: DecorationMetrics, strikethrough_metrics: DecorationMetrics, ascent: f32 }`,
  with `DecorationMetrics { offset: f32, thickness: f32 }` in **EM units**.
- **Metric source:** `decoration_metrics(font)` (src/shape.rs:686–705 @0.19.0)
  reads the font's underline (`post`) and strikeout (`OS/2`) tables, normalized
  by upem, with explicit fallbacks when a table is absent: underline offset
  −0.125 em, strikeout offset 0.3 em, thickness 1/14 em.

### 2.2 Decision: inherit the spans, do not recompute

**Decision.** Buiy maps its node-level decoration component into
`Attrs.text_decoration` and reads geometry back from `LayoutRun.decorations`.
The authoring component is
**`TextDecorations { line: DecorationLines, color: Option<ColorToken> }`** —
`DecorationLines` a bitflag set (`UNDERLINE | OVERLINE | LINE_THROUGH`, the
`text-decoration-line` values) and `color` the optional
`text-decoration-color` override (`None` = `currentColor`, § 3.2's
precedence); the plural name deliberately avoids colliding with
`cosmic_text::TextDecoration`, which § 2.1 binds. Buiy never reads font
decoration tables itself. **F**

**Rationale.** 0.19 added exactly this surface — decoration intent in, per-run
spans with font-table metrics out, merged across attrs boundaries and
font-fallback clusters by upstream. Crucially, `DecorationMetrics` is *data*,
not baked pixels: the C-tier knobs (`-thickness`, `text-underline-offset` /
`-position`) layer above as emission-time transforms (§ 9), so inheriting
forfeits nothing.

**Rejected runner-up:** Buiy reads the font tables itself (swash/skrifa) and
computes spans from `LayoutGlyph` ranges. Loses: duplicates upstream's
span-merging and per-cluster fallback handling, and buys no capability the
metrics-as-data path doesn't already give.

### 2.3 Metric semantics

A decoration line's resolved logical-px geometry is
`DecorationMetrics × DecorationSpan.font_size` (EM × px-per-EM). The span's
`font_size` — not the node's — is the multiplier, so a mixed-size rich-text run
gets per-span line geometry, matching upstream. The horizontal extent is the
union of the glyphs in `glyph_range` (their x extents in the run), in f32
logical px end-to-end — the same coordinate space as every other instance
([instance.rs](../../../crates/buiy_core/src/render/instance.rs) `PackedInstance`).

---

## 3. Emission — borrow the math, own the instances

### 3.1 Decision: do not implement `cosmic_text::Renderer`

**Decision.** Buiy walks `LayoutRun.decorations` itself with a pure emission
function (per-run spans → decoration instances), mirroring upstream
`render_decoration`'s semantics 1:1, and emits f32 logical-px instances into
Buiy's own buckets. **F**

**Rationale.** Upstream's reference painter (`render_decoration<R: Renderer>`,
src/render.rs @0.19.0) quantizes through
`Renderer::rectangle(x: i32, y: i32, w: u32, h: u32, color)` — integer pixels.
That breaks fractional scale factors, forfeits Buiy's subpixel discipline, and
gives no control over which paint-order seat a rect lands in (every rect
arrives as an undifferentiated callback). Owning emission keeps f32 logical px
end-to-end, applies min-thickness in *physical* px (§ 3.3), and chooses the
seat per decoration kind (§ 4).

**Rejected runner-up:** implement `Renderer { rectangle, glyph }` and call
`render_decoration`. Loses on quantization and ordering control.

**Drift guard.** Because we mirror rather than call, upstream's math is pinned
by exact-number headless tests ([verification.md § 1.2](verification.md#12-the-headless-inventory)):
a cosmic-text bump that changes decoration semantics fails loudly instead of
silently shifting goldens.

### 3.2 The mirrored math

Per span, with `m = metrics × span.font_size` (logical px) and `line_y` the
run's baseline:

| Line | Vertical placement | Thickness |
|---|---|---|
| underline (Single) | `y = line_y − m.underline.offset` (offset is negative-down per the font convention upstream applies) | `m.underline.thickness`, § 3.3 floor |
| underline (Double) | two rects, gap = thickness | each rect `m.underline.thickness` |
| line-through | `y = line_y − m.strikeout.offset` | `m.strikeout.thickness`, § 3.3 floor |
| overline | `y = max(line_y − ascent × font_size, line_top)` — clamped so it never escapes the line box | reuses underline thickness |

Color precedence, exactly upstream's:
`DecorationSpan.color_opt` (the `-color` property) **or** the span's text color
**or** the node's resolved foreground (`currentColor`). The resolved color is
pre-linearized straight-alpha, like every instance color
([atlas/primitive.rs:35–42](../../../crates/buiy_core/src/render/atlas/primitive.rs)).

### 3.3 The physical-px minimum-thickness rule

Glyph bitmaps are subpixel-bucketed at raster time
([glyph-pipeline.md](glyph-pipeline.md)); decoration rects are analytic. At a
fractional scale factor a 1-logical-px line lands on a half-physical-pixel
boundary and AA-blurs. The rule:
`thickness_phys = max(1, round(thickness_logical × scale_factor))` — **whole
physical pixels, minimum one** — converted back to logical px for the
instance, with y snapped to the same physical grid. Upstream's
`.max(1.0).ceil()` operates in its own integer-pixel space and must **not** be
copied verbatim: at `scale_factor = 1.5` it would floor at 1 *logical* px
= 1.5 physical px, the exact blur this rule prevents. **F**

---

## 4. Paint-order seats

### 4.1 The constraint

CSS Text Decoration Level 3 § "Painting Order of Text Decorations" requires
underline and overline painted **under** the text and line-through **over**
it. Buiy's primitive paint rank is fixed and built:
`shadow 0 < quad 1 < glyph 2 < path 3`
([buckets.rs:42–53](../../../crates/buiy_core/src/render/buckets.rs)), and
within a batch, instance order = emission order
([buckets.rs:84–114](../../../crates/buiy_core/src/render/buckets.rs)). So a
quad can never paint over a glyph in the same layer — the seats below are
forced by that rank, not chosen against it.

### 4.2 Decision: quads under, solid-coverage stamps over

**Decision.** Selection rectangles, underline, and overline are **quad-tier**
instances (emitted after the node's background). Line-through and the caret
are **`GlyphAlphaInstance`s sampling a 1×1 solid-white `CoverageR8` atlas
stamp**, emitted after the run's glyph instances so they paint over the text.
**F**

**Rationale.** The glyph-alpha primitive is explicitly "not text-specific — any
single-channel coverage stamp uses it"
([atlas/primitive.rs:13–17](../../../crates/buiy_core/src/render/atlas/primitive.rs),
[seam spec § 4.1](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#41-the-glyph-alpha-primitive--the-alpha-as-color-trick)).
A 1×1 white R8 texel stretched by `rect` is an over-glyph tinted rectangle with
zero new pipeline, riding existing batching and the existing draw branch. The
deciding bonus for the caret: it rides the **independent glyph damage gate**
([prepare.rs:157–216](../../../crates/buiy_core/src/render/prepare.rs)) — a
caret blink re-uploads only the glyph buffer, never the quad buffer (§ 6.3).

**Rejected runner-ups.** (1) *All decorations as quads* — a spec violation
outright: line-through would paint under the glyphs. (2) *A new over-text
primitive kind* — a fourth pipeline, a specialization variant, and a node draw
branch for what one warm atlas texel already does. (3) *A quad-tier caret* —
cannot paint over glyphs at all (§ 4.1) and would dirty the entire quad buffer
every blink.

### 4.3 The solid stamp

One reserved Buiy-internal sentinel `AtlasKey` (opaque to the atlas per
[seam § 3](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#3-the-insertlookup-api--the-only-handle-the-seam-touches))
maps to a 1×1 `CoverageR8` bitmap of value 255. It is **warmup-pinned**: pushed
into `AtlasWarmupQueue`
([warmup.rs:19–44](../../../crates/buiy_core/src/render/atlas/warmup.rs)) at
text-plugin startup so it is resident before any first paint, and re-inserted
on miss like any content-addressed entry. Residency must also be
*maintained*: the stamp's key joins the glyph producer's `ResidentTextKeys`
touch pass ([glyph-pipeline.md § 6.3](glyph-pipeline.md)) whenever any stamp
instance (caret, line-through — any § 4.2 stamp seat) is live in
`ExtractedGlyphs` — otherwise 60 retained idle frames (`eviction_grace`)
evict the cell, a later insert reuses it, and the retained stamp instances'
UVs sample someone else's bitmap. Stretched `rect`s sample its UV
midpoint; bilinear filtering of a uniform texel is exact, so the stamp is
resolution-independent. **F**

### 4.4 Emission order within a node

Within-bucket instance order = emission order, so one ordering rule covers
everything:

| # | What | Bucket / primitive |
|---|---|---|
| 1 | node background / border | quad |
| 2 | `::selection` rects | quad |
| 3 | underline, overline | quad |
| 4 | the run's glyphs | glyph (`GlyphAlphaInstance`, atlas coverage) |
| 5 | line-through | glyph (solid stamp) |
| 6 | caret | glyph (solid stamp) |

### 4.5 Named dependencies (not owned here)

- **Cross-node layering.** v1 buckets route everything to `(primitive, layer 0)`
  — "real layers are the paint-order phase's job"
  ([buckets.rs:9–11, 146–153](../../../crates/buiy_core/src/render/buckets.rs)).
  Within-node decoration ordering (§ 4.4) is correct today; cross-*node*
  ordering (an overlay panel's background over another node's text) depends on
  the render paint-order layer work landing. This spec names that dependency
  rather than assuming it.
- **Glyph stamps in effect groups — landed (T8,
  [2026-06-11-buiy-text-t8-glyphs-in-effect-groups.md](../../plans/2026-06-11-buiy-text-t8-glyphs-in-effect-groups.md)).**
  The glyph buffer IS partitioned by effect-group ranges like the quad path
  (per-entity `entity_runs` + fresh-node-list group derivation at prepare —
  the § 4.6 discipline applied to the glyph buffer; the
  [follow-ups.md "glyphs bypass effect-group compositing"](../../plans/follow-ups.md)
  entry is closed): underline, line-through, caret, and the glyph ink all dim
  together inside an `Opacity(0.5)` card — the
  editable-text-in-effect-group fixture constraint is lifted
  ([verification.md § 4](verification.md#4-campaign-gates)).

### 4.6 The quad carrier — `ExtractedTextQuads`

**Decision.** Text's quad-tier visuals (selection rects, underline, overline —
§ 4.4 seats 2–3) ride a dedicated render-world resource,
`ExtractedTextQuads(Vec<TextQuad>)`, where `TextQuad` is a flat `Copy` record
`{ entity, rect, color, clip }` — the quad-instance payload keyed by the
**source entity**. (The as-built `ExtractedNode` is
`{ entity, position, size, color, clip, group }`, extract.rs:66–93 — paint
order is the *implicit* `Vec` order of `ExtractedNodes.nodes`; there is no
explicit paint-order field to copy, and `TextQuad` deliberately carries no
`group` either — both derive from the node record at pack time, below.) The
as-built quad path packs exactly one background quad per entity from
`ExtractedNode`; text needs N rects per node, so it gets its own flat
carrier rather than a per-entity payload. **Producer:** `extract_buiy_glyphs`
([glyph-pipeline.md § 6](glyph-pipeline.md)) writes it at the same emission
site as `ExtractedGlyphs` — one run walk emits both — rebuilt-on-damage /
retained-on-steady-state under the same normative probe union
(glyph-pipeline § 6.2; one damage decision covers both resources).
**Packer — the merge contract:** the prepare quad pack
(`pack_view_partitioned`'s caller) takes `ExtractedTextQuads` as a second
input and merges it into the **existing** quad instance buffer — no new GPU
buffer, pipeline, or draw. All ordering derives from the **fresh** node list
on every pack: walk `nodes.nodes` in its `Vec` order and, immediately after
each node record, splice that entity's text quads (an entity→quads lookup
over the flat carrier). Effect-group membership comes from the node record
being spliced after, so a text quad's partition placement can never disagree
with its entity's. Two as-built facts make the splice total and stale-proof:
(a) `ExtractedNodes.nodes` retains a record for **every** painted entity even
when it has no background — extract emits `Color::NONE` records and only the
*pack* skips the quad (buckets.rs:167/219) — so every text entity has a node
record to splice after; (b) because positions are re-derived from the fresh
list each pack, retained text quads land correctly even on frames where the
node walk rebuilt for non-text reasons. The § 4.4 emission-order table holds
by construction: background < selection < underline/overline (< glyphs, in
the later glyph batch). The quad-buffer damage gate keys on
`nodes.is_changed() || groups.is_changed() || text_quads.is_changed()` (the
as-built gate already unions nodes and groups — prepare.rs:169; text quads
join it); quad and glyph buffers stay **independently** gated, so a caret
blink still re-uploads glyphs only (§ 6.3). **F**

**Rejected runner-up (review round 2 — the round-1 text's bug):** keying the
merge on a paint-order index ("`painters_z`") recorded into `TextQuad` at
emission. Twice wrong: the as-built `ExtractedNode` carries no such field
(the claim that text would reuse "the same partition keys the node records
carry" was false — order is implicit `Vec` order), and a recorded index goes
**stale** whenever the node walk rebuilds from a trigger outside the text
probe union (e.g. `Changed<StackingContext>` on a non-text sibling reorders
the list) while text quads are retained — retained quads would splice at
wrong positions and break the partition-contiguity `debug_assert`
`pack_view_partitioned` relies on. The entity key + fresh-walk derivation is
immune by construction.

**Rejected runner-up:** extending `ExtractedNode` with a per-entity
`Vec<DecorationRect>` — a `Vec` field breaks the flat `Copy` record the node
extract's retention and packing discipline is built on, and variable-length
payloads inside the node list break the partition contiguity
`pack_view_partitioned` relies on. A parallel flat entity-keyed list composes
instead of intruding.

---

## 5. `::selection`

### 5.1 Rectangles via `LayoutRun::highlight`

**Decision.** Selection rectangles come from
`LayoutRun::highlight(cursor_start, cursor_end) -> impl Iterator<Item = (f32, f32)>`
— per-run `(x_left, width)` spans — with the selection endpoints from
`Editor::selection_bounds() -> Option<(Cursor, Cursor)>`
([editing-and-ime.md](editing-and-ime.md) owns the cursor model). Buiy unions each span with
the run's `(line_top, line_height)` to form a rect, colors it with the
`color.selection.bg` token, and emits it as a quad-tier instance (§ 4.4 seat 2)
into `ExtractedTextQuads` (§ 4.6). The endpoints reach the producer through
the render-prep-written `SelectionVisual` state (§ 6.3's window), so emission
itself stays in the `ExtractSchedule` producer. **F**

**Rationale.** Verified in 0.19 source (src/buffer.rs:58–113): `highlight` is
grapheme-accurate (it splits ligature clusters proportionally) and yields
**multiple disjoint spans** for mixed-BiDi runs — exactly the F-tier "visual
selection rectangles correct for mixed-direction lines" row (text.md:89). No
embedder BiDi math.

**Rejected runner-up:** a hand-rolled sweep over `LayoutGlyph.level` runs (the
stance in [prior-art capabilities.md "Selection rendering"](../../prior-art/cosmic-text/capabilities.md),
which predates this API). Loses: reimplements upstream's grapheme math; the
prior-art row is stale.

**Erratum (supersedes prior art).** The prior-art folder cites
`Editor::with_selection_bounds(|rects|)` — **no such method exists in 0.19**;
the real contract is `selection_bounds()` + per-run `highlight`. Errata
ledger: [verification.md § 5](verification.md#5-prior-art-errata-ledger).

### 5.2 Selected-text color: re-tint at emission

**Decision.** Glyph instances whose cluster intersects the selection are
emitted with the selected-text token (`color.selection.fg`) instead of the
resolved foreground. The atlas is never touched. **F**

**Rationale.** Alpha-as-color makes re-tint free — `color` is per-instance
([atlas/primitive.rs:35–42](../../../crates/buiy_core/src/render/atlas/primitive.rs)),
and the retint-byte-identity GPU test
([2026-06-07-render-gpu-verify-campaign.md](../../plans/2026-06-07-render-gpu-verify-campaign.md)
Phase 4) already proves on this exact pipeline that re-tinting leaves the atlas
byte-identical. Upstream's own `Editor::draw(…, text_color, cursor_color,
selection_color, selected_text_color, …)` confirms re-tint is the canonical
model. Granularity is the glyph cluster: a partially selected ligature re-tints
whole while its selection *rect* stays grapheme-accurate — the accepted,
upstream-matching tradeoff.

**Rejected runner-up:** a second glyph overlay pass clipped to the selection
rects. Loses: double-draws coverage (visible darkening on AA edges where the
clip bisects a glyph) and needs per-instance clip gymnastics that per-cluster
re-tint sidesteps.

---

## 6. The caret

### 6.1 Geometry

The caret position comes from the editing model
(`Editor::cursor_position() -> Option<(i32, i32)>` in buffer-local coordinates,
plus the run's line metrics — [editing-and-ime.md](editing-and-ime.md) owns logical→visual
cursor resolution, including the BiDi split caret). This file owns the pixels:
the caret is one solid-stamp `GlyphAlphaInstance` (§ 4.3) with
`rect = (caret_x, line_top, caret_w, line_height)`, where `caret_w` is
1 logical px floored/rounded to whole physical px by the § 3.3 rule. The rect
travels to the producer in `CaretVisual` (§ 6.3). Emitted
last (§ 4.4 seat 6), so it paints over glyphs and line-through. **F**

### 6.2 `caret-color`

Resolution order: explicit `caret-color` → the theme caret token → the node's
resolved foreground (`currentColor`), mirroring CSS's `caret-color: auto`. The
value lands in the stamp's per-instance `color`; changing it is a re-tint,
never an atlas mutation. Token values are `buiy-theme-tokens-design`'s. **F**

### 6.3 Blink, the virtual clock, reduced-motion, damage

**Decision — state in render-prep, emission in extract.** A main-world
component carries the editor's visual state for the producer:
`CaretVisual { visible: bool, rect: Rect }` (and its sibling
`SelectionVisual` — the § 5 rect list plus re-tint ranges), written by a
render-prep system in the `.after(Animate).before(Picking)` window
([architecture.md § 4](architecture.md)). The blink clock is evaluated
**there**: caret visibility is a square-wave function of the app clock, and
the render-prep system writes `CaretVisual` **only on an edge** (idempotent —
an unconditional per-frame write would keep `Changed<CaretVisual>`
perpetually hot and kill extract retention). Emission stays where all
emission lives: the `ExtractSchedule` producer reads
`CaretVisual`/`SelectionVisual` through the glyph-pipeline § 6.2 probe union,
so a blink edge rebuilds `ExtractedGlyphs` (and `ExtractedTextQuads`) that
frame and a steady phase rebuilds nothing. Because the square wave reads the
Bevy clock, the golden harness's `fixed_clock`
([golden.rs:19–20](../../../crates/buiy_core/src/render/golden.rs)) pins the
phase deterministically — the blink-pair golden captures both phases at chosen
virtual instants ([verification.md § 3.1](verification.md#31-fixed_clock--the-virtual-clock-drives-blink)).
Under `prefers-reduced-motion` the caret is **steady** (always visible, no
blink), per text.md:90. The blink period is a theme/animation value, not
pinned here. **F**

The damage property (the reason the caret is a glyph-tier stamp, § 4.2): a
blink frame changes only `ExtractedGlyphs`, so the quad buffer is retained —
structural per the independently-gated uploads in
[prepare.rs:230–270](../../../crates/buiy_core/src/render/prepare.rs), asserted by a
dedicated test
([verification.md § 1.3](verification.md#13-the-gpu-ignore-inventory)).
Since T8 the property is also pinned at the GPU lane: `BufferUploadStats`
(the observable render-world instrument prepare records its per-buffer
`write_buffer` calls into) lets
`caret_blink_reuploads_the_glyph_buffer_only`
(tests/text_selection_caret_gpu.rs) assert a blink frame issues exactly one
glyph upload and zero quad uploads.

---

## 7. `::placeholder`

When an editable node's value is empty, the placeholder string is shaped, laid
out, and painted through the *same* pipeline as real text — same `Buffer`
machinery, same glyph producer, same decoration seats — with exactly one
difference: the foreground resolves to the placeholder token instead of the
node's text color. No layout difference, no caret or selection interaction (a
placeholder is never selectable). **F**

**Rejected runner-up:** a dedicated placeholder paint path. Loses: a second
text pipeline to keep correct for zero benefit; a placeholder is just text
with a different tint.

---

## 8. Preedit underline — the painting seam

The IME preedit span paints as a **forced `Single` underline over the preedit
range** plus the standard caret, using §§ 2–4's machinery unchanged — a
preedit underline is not a new decoration kind, only an underline whose range
comes from IME state instead of `Attrs`. This file reserves that painting
entry point; the preedit state machine (winit events → spans, display splice,
commit/undo grouping) belongs to the editing campaign
([editing-and-ime.md](editing-and-ime.md)). **F (painting primitive); consumed by the editing
campaign.**

---

## 9. C-tier reservations

Deferred per the foundation tiers (text.md:52) — reserved as seams, not built:

- **`text-decoration-style` (wavy / dotted / dashed).** Reserved as a
  `DecorationLineStyle` strategy enum at the quad-emission seam: dotted/dashed
  are pure Buiy emission patterns (segmented quads — no upstream change); wavy
  wants either the upstream `UnderlineStyle::Wavy` variant (a literal
  `// TODO: Wavy` sits in 0.19's enum) or a Buiy tiled `CoverageR8` stamp.
  C-tier lands as new match arms, not a re-architecture; the unblock path of
  record for wavy is **upstream-PR-first**, and the Buiy emission fallback
  must not get baked into the F-tier types. Rejected runner-up: build wavy now
  (speculative scope; the upstream TODO may land it for free).
- **`-thickness` / `text-underline-offset` / `-position`.** Pure transforms of
  `DecorationMetrics` at emission time (§ 2.2's metrics-as-data payoff).
- **`Double` exposure.** The upstream variant exists and § 3.2 specifies its
  paint math; the CSS *knob* exposing it is C-tier with `-style`.
- **`text-decoration-skip-ink`.** Named only; needs per-glyph ink-box queries.

---

## Open questions

1. **Caret seat conflict with [editing-and-ime.md § 5](editing-and-ime.md).**
   That file pins the caret as "a 1–2 px **quad** … emitted on the next layer"
   and blink as "a CPU-side visibility toggle on the emitted quad." Two
   problems: (a) per-layer interleave does not exist — v1 routes everything to
   `layer 0` and "real layers are the paint-order phase's job"
   ([buckets.rs:9–11, 146–153](../../../crates/buiy_core/src/render/buckets.rs)),
   so with the fixed rank `quad 1 < glyph 2` (buckets.rs:42–53) a quad caret
   paints **under** glyphs today, and a "next layer" hack would misuse the
   `painters_z` stacking index for a within-node ordering concern; (b) a quad
   caret rides the quad damage gate, so every blink would re-upload the whole
   quad buffer — the glyph-tier stamp blinks through the independent glyph
   gate alone ([prepare.rs:157–216](../../../crates/buiy_core/src/render/prepare.rs)).
   This file's § 4.2 seats (caret + line-through = glyph-tier solid stamps)
   stand on those two as-built facts; editing-and-ime.md § 5's caret bullet
   should be revised to consume them. Same applies to its split caret (= two
   stamps, not two quads).
   *Resolved in review round 1 (2026-06-09): editing-and-ime.md § 5 was
   rewritten to this file's § 4.2 stamp model — caret = glyph-tier solid stamp
   over text, blink = a `CaretVisual` edge through the independent glyph
   damage gate (§ 6.3), split caret = two stamps, selection = quad-tier under
   text.*
2. **Sibling file-name divergence.** [editing-and-ime.md](editing-and-ime.md)
   linked to a `decoration-and-selection.md` that does not exist — this file
   (`decoration-and-paint.md`) is the painting owner it means. For the
   synthesizer/README pass to reconcile. *Resolved in the 2026-06-09 synthesis
   pass: every sibling link now targets `decoration-and-paint.md`.*

---

## Sources

- CSS Text Decoration Level 3, § "Painting Order of Text Decorations" — <https://www.w3.org/TR/css-text-decor-3/>
- cosmic-text 0.19.0 API — <https://docs.rs/cosmic-text/0.19.0/> (`Attrs`, `TextDecoration`, `UnderlineStyle`, `DecorationSpan`, `DecorationMetrics`, `LayoutRun`, `Editor`) (verified 2026-06-09)
- cosmic-text 0.19.0 source — <https://github.com/pop-os/cosmic-text/tree/0.19.0> (`src/attrs.rs`; `src/shape.rs:686–705` `decoration_metrics`; `src/render.rs` `render_decoration` + `Renderer`; `src/buffer.rs:58–113` `LayoutRun::highlight`)
