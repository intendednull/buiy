# Buiy text — the Taffy measure seam (content sizing inside layout)

**Parent:** [README.md](README.md)

This file owns the seam where text content sizing plugs into the layout
pipeline: how text leaves register a measure source with Taffy, the measure
protocol (what a measure call reads and returns), the reshape-at-final-width
pass, caching and re-entrancy with the container-query re-runs, the F-tier
mapping of line-height / white-space / text-align into measure inputs, and the
baseline plumbing shape. It is the layout-side counterpart of
[glyph-pipeline.md](glyph-pipeline.md) (which consumes the finalized `Buffer`)
and depends on the engine-ownership decisions in
[architecture.md](architecture.md) (`SharedFontSystem`,
`TextBuffer` lifecycle). The render-side seam it ultimately feeds —
`BuiyAtlas` + `GlyphAlphaInstance` — is **built and GPU-verified**
([render atlas-and-text-seam.md § 3–5](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md));
nothing here redesigns it.

It maps the foundation rows
[text.md § 3.4](../2026-05-07-buiy-foundation/text.md#34-typography):
**line-height (F)**, **white-space incl. `text-wrap: wrap|nowrap` (F)**, and
the layout-side enablement of **text-align (F)** — plus the structural seam no
inventory row names but every row depends on: measure registration, the
intrinsic-sizing protocol, and re-entrancy with the cq re-runs.

Versions this file is written against: **taffy 0.10.1** (Cargo.lock:4050-4053;
all taffy refs below are to the vendored source, read, not recalled) and
**cosmic-text 0.19.0** (pinned; not yet in Cargo.lock — all cosmic-text
signatures verified against docs.rs/cosmic-text/0.19.0, fetched 2026-06-09).

---

## § 1 The seam, stated once

Mirroring [atlas-and-text-seam.md § 1](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md):
**layout owns the pipeline; text owns the buffer and the protocol.**

| Concern | Owner |
|---|---|
| `BuiyLayoutStep` chain, the three Taffy compute sites, `ResolvedLayout`, the ≤2× compute ceiling | **layout** (`2026-05-08-buiy-layout-design/architecture.md` § 3.2, § 9) |
| `LayoutTree` (now `TaffyTree<Entity>`, § 2), node GC, `by_entity` | **layout** |
| `TextBuffer` component (the cosmic-text `Buffer` + cached intrinsics) | **this file** |
| The measure protocol (width resolution, intrinsic cache, shape calls) | **this file** |
| `TextSync` / `TextCommit` pipeline steps | **this file** (inserted into layout's chain, § 4) |
| `ResolvedBaseline` output component | **this file** (§ 6) |
| `SharedFontSystem` resource type + lock discipline | [architecture.md](architecture.md) (this file only *locks* it) |
| Glyph extraction from the finalized `Buffer` → `ExtractedGlyphs` | [glyph-pipeline.md](glyph-pipeline.md) |
| Font registration, fallback stacks, `Attrs` construction | [font-assets.md](font-assets.md) |

Today's as-built baseline this file changes: Buiy uses **no measure API
anywhere** — `taffy_compute` calls plain `tree.compute_layout(root, …)` per
root (`crates/buiy_core/src/layout/systems.rs:2596-2641`), and plain
`compute_layout` is literally `compute_layout_with_measure(node, avail,
|_,_,_,_,_| Size::ZERO)` (taffy-0.10.1 `src/tree/taffy_tree.rs:925-927`) — so
a content-sized leaf currently measures **zero**. The intrinsic keywords
already exist in Buiy's `Sizing` enum but `sizing_to_dim` maps
`MinContent|MaxContent|FitContent` silently to `Dimension::auto()` with the
comment "until Phase 10 + text rendering integrate"
(`crates/buiy_core/src/layout/translate.rs:625-637`). This file is that
integration.

---

## § 2 `LayoutTree` migration — `TaffyTree<()>` → `TaffyTree<Entity>`

### § 2.1 NodeContext = `Entity`

**Decision.** `LayoutTree.tree` (`crates/buiy_core/src/layout/tree.rs:14-18`)
migrates from `TaffyTree<()>` to `TaffyTree<Entity>`. Text leaves register
their entity as the node context (`new_leaf_with_context(style, entity)`,
taffy_tree.rs:581, or `set_node_context(node, Some(entity))`,
taffy_tree.rs:642-657); the measure closure receives `Option<&mut Entity>`
straight from Taffy's leaf dispatch (taffy_tree.rs:318-326) and resolves the
entity against ECS queries it captures. O(1) dispatch, zero duplicated state:
the `Buffer` and style data stay in the ECS where editing
([editing-and-ime.md](editing-and-ime.md)) and extraction also need them, and a future
measured leaf kind (image, canvas) is component dispatch, not a `LayoutTree`
type change.

**Runner-up rejected:** a rich context enum owning the cosmic-text `Buffer`
inside the `TaffyTree` (Bevy 0.15's `NodeMeasure` shape). Rejected because
`LayoutTree` is `NonSend`: burying the `Buffer` there forces the editing
surface and `TextCommit` to route every `Buffer` access through the NonSend
resource, double-stores truth, and complicates GC (`gc_removed_nodes` would
have to own `Buffer` destruction). Also rejected: keeping `TaffyTree<()>` with
a side NodeId→Entity reverse map — it rebuilds a map Taffy already maintains
and cannot live inside `LayoutTree` (borrow conflict with `&mut self` during
compute).

### § 2.2 Context lifecycle — registration is edge-triggered

`set_node_context` **calls `mark_dirty` internally** (taffy_tree.rs:656,
verified) — so registration must happen only on `Added<Text>` /
`RemovedComponents<Text>` edges inside `TextSync` (§ 4.1), **never
per-frame**, or the O(0) steady-state invariant silently dies. The
steady-state zero-measure test (§ 8) is the tripwire. Node GC is unchanged:
`gc_removed_nodes` drops the Taffy node, which drops the context `Entity`
(a `Copy` id — nothing to destruct); the `Buffer` itself dies with the
`TextBuffer` component on despawn.

### § 2.3 Where the `Buffer` lives — `TextBuffer` component

**Decision.** `cosmic_text::Buffer` is **`Send + Sync` in 0.19** (verified on
docs.rs auto-traits; the prior-art note "FontSystem implements neither Send
nor Sync" in `docs/prior-art/cosmic-text/integration.md` is **stale** for
0.19). The buffer therefore lives in a plain component:

```rust
/// Main-world component on every text entity. Internal mutation during
/// measure/finalize goes through `Mut::bypass_change_detection` (§ 7).
#[derive(Component)]
pub struct TextBuffer {
    pub buffer: cosmic_text::Buffer,
    /// Cached intrinsic widths, keyed by content version (§ 3.2).
    intrinsics: Option<IntrinsicWidths>, // { min_content: f32, max_content: f32 }
}
```

Queryable by the measure closure, `TextCommit`, the future `Editor` surface
(the bevy_cosmic_edit `EditorBuffer` QueryData borrow pattern,
`docs/prior-art/bevy-cosmic-edit/lessons.md` Borrow #1), and despawn cleanup
is automatic. **Runner-ups rejected:** inside the Taffy context (§ 2.1's
runner-up), and a NonSend `HashMap<Entity, Buffer>` side store (re-creates the
GC problem `by_entity` solves and makes every text system NonSend for
nothing).

**The shared accessor (pinned here; consumed by
[editing-and-ime.md § 2.2a](editing-and-ime.md)).** Editable entities own
their authoritative `Buffer` inside `TextEditState` (`Editor<'static>` over
`BufferRef::Owned`), so every system that touches "the entity's buffer" goes
through one `QueryData`:

```rust
#[derive(QueryData)]
#[query_data(mutable)]
pub struct TextBufferAccess {
    display: &'static mut TextBuffer,
    edit: Option<&'static mut TextEditState>,
}
// impl on the item types: with_buffer(f) / with_buffer_mut(f) — dispatch to
// the editor-owned Buffer (via the facade's with_buffer*) when `edit` is
// Some, else to `display.buffer`. Mutable access routes through
// Mut::bypass_change_detection (§ 7) in both arms.
```

The measure closure and `TextCommit` use `TextBufferAccess`; the glyph
producer's `Extract` query uses its read-only form. Display-only and editable
entities take the same code path, and compatibility with `BufferRef::Owned`
holds by construction — closing editing-and-ime.md's open question 4.

**As landed (T3, 2026-06-11):** `TextBufferAccess` is **deferred to the
`buiy-text-editing` campaign** — the `edit` arm binds `TextEditState`, which
does not exist yet. The measure closure and `TextCommit` bind
`&mut TextBuffer` directly (and the glyph producer binds `&TextBuffer`)
until the editor lands; the swap is mechanical (T3 plan decision 12;
supersedes the seam table's "built in T3" expectation).

---

## § 3 The measure protocol

### § 3.1 The contract Taffy gives us — content-box, cached, childless-only

Three verified facts shape the protocol:

1. **Content-box.** `compute_leaf_layout` subtracts `content_box_inset`
   (padding + border + scrollbar gutter) from the available space before
   calling measure and adds it back to the returned size (taffy-0.10.1
   `src/compute/leaf.rs:111-146`). The closure **never does BoxModel math**.
2. **Cached.** The measure fn is invoked only for childless nodes, wrapped in
   `compute_cached_layout`, so identical `(known_dimensions, available_space)`
   inputs hit Taffy's per-node cache (taffy_tree.rs:297-330). Repeated
   identical probes within a frame cost nothing.
3. **Size only.** The closure returns `Size<f32>` — there is **no baseline
   channel** in taffy 0.10's measure API (the other closure slot in
   `compute_leaf_layout` is the calc resolver, leaf.rs). Baseline is
   structurally a Buiy-side output (§ 6).

### § 3.2 Width resolution and the intrinsic cache

**Decision.** The measure closure resolves a candidate width, relays out the
buffer at that width, and folds the line boxes:

```text
width = if let Some(w) = known_dimensions.width { w }
        else { match available_space.width {
            AvailableSpace::MinContent   => cached intrinsics.min_content,
            AvailableSpace::MaxContent   => cached intrinsics.max_content,
            AvailableSpace::Definite(w)  => w,
        } }
buffer.set_size(Some(width), None);          // lazy — no FontSystem
buffer.shape_until_scroll(&mut fs, false);   // height_opt=None ⇒ all lines
size = fold over buffer.layout_runs():       // (max line_w, Σ line_height)
```

The intrinsic widths are cached on `TextBuffer` per **content version** (text
/ attrs / font / metrics / wrap — recomputed in `TextSync` on change, § 4.1),
computed once via `set_size(Some(0.0), None)` → longest-word width under
`Wrap::Word` (min-content) and `set_size(None, None)` → unwrapped width
(max-content). Answering Taffy's intrinsic probes from the cache makes them
O(1); the definite-width call is cheap because cosmic-text's `BufferLine`
splits `shape_opt` (width-independent harfrust output) from `layout_opt`
(greedy line-break per width) — `set_size` invalidates only the latter
(`docs/prior-art/cosmic-text/architecture.md` "Memory layout"). That split —
not the optional `shape-run-cache` feature — is the amortization this
protocol relies on; the feature stays **OFF** in v1 (decided, review round 1
— [architecture.md § 7](architecture.md) records the decision and the
rejected ON-with-trim runner-up).

**Runner-ups rejected:** (a) re-shape from scratch on every measure call —
loses the whole shaping cost × measure-call-count on every width change;
(b) Bevy-0.15-style "precompute min/max widths, approximate height from them"
— loses correctness: wrapped height is not derivable from two widths, the
actual line-break at the candidate width is required (which is exactly why
Bevy's own measure also re-lays-out through its buffer).

### § 3.3 Intrinsic sizing keywords on text leaves

taffy 0.10.1's `Dimension` exposes **only** `length` / `percent` / `auto` /
`calc` constructors (verified, `src/style/dimension.rs:271-303`) — the
`MIN_CONTENT`/`MAX_CONTENT` tags exist on `CompactLength` but `Dimension`
cannot carry them into `Style.size`. So `Sizing::MinContent|MaxContent` on a
**text leaf** resolves *inside the measure closure*: the closure (which holds
the `Entity`) reads the entity's Buiy `Sizing` and, when an intrinsic keyword
is set on an axis, answers from the cached intrinsics regardless of the probe.
`sizing_to_dim` keeps translating the keyword to `Dimension::auto()`
(translate.rs:625-637) — the *measure answer* is what realizes the keyword.
Fidelity limit: where the parent algorithm passes a `known_dimensions` width
(e.g. cross-axis stretch resolves before measure), the known dimension
overrides the measured size in `compute_leaf_layout` (leaf.rs:143-146) and the
keyword behaves as `auto` — flagged in Open questions. Intrinsic keywords on
**container** nodes remain deferred (the Phase 10 half of the translate.rs
comment).

### § 3.4 FontSystem capture and units

The closure locks **`SharedFontSystem`** (`Arc<Mutex<FontSystem>>`, decided in
[architecture.md](architecture.md)) **once per compute call**,
not per measure invocation — the helper (§ 4.3) takes the guard for the
duration of `compute_layout_with_measure`, so per-glyph lock traffic is zero.
`FontSystem` is `Send + Sync` in 0.19, so the `Mutex` is for the RenderApp
clone (atlas-miss rasterization), not a Send workaround.

**Units: measure speaks logical px.** `Metrics` is unit-agnostic px; Buiy
pins measure (and everything in this file) to **logical px**, matching the
bridge's logical-px space (`crates/buiy_core/src/render/bridge.rs:10-14`).
Physical-px rasterization (`LayoutGlyph::physical(offset, scale)`) is
[glyph-pipeline.md](glyph-pipeline.md)'s side of the scale-factor line; the
two specs share this sentence so they cannot diverge.

---

## § 4 Pipeline integration — `TextSync` and `TextCommit`

The 11-step chain (`crates/buiy_core/src/layout/pipeline.rs:17-76`) gains two
text steps. Updated chain (text steps **bold**):

```text
RemovedNodesGc → WritingModeInherit → **TextSync** → SyncStyles → CqActivate
→ TaffyCompute → CqFlipCheck → CqFlipReRun → PostTaffyOverrides
→ WriteResolvedLayout → CqDescendantInvalidate → CqDescendantReRun
→ **TextCommit**
```

### § 4.1 `TextSync` — the dirty path (pre-SyncStyles)

**Decision.** A dedicated `text_sync_buffers` system between
`WritingModeInherit` and `SyncStyles`. The content component is
**`Text(pub String)`** — the authored UTF-8 string this step feeds (after the
§ 5.2/§ 5.4 pre-passes) to `set_text`. For entities with `Changed<Text>` or a
changed **text-style carrier** ([architecture.md § 5.1](architecture.md)'s
union — the font trio `FontFamily`/`FontSize`/`FontWeight`
([font-assets.md § 8](font-assets.md)) plus the line-height,
white-space/wrap, align, and `TextDirection` (§ 5.4) carriers) it updates the
buffer
(`set_text(&mut self, text, &Attrs, Shaping, Option<Align>)` — note the 0.19
signature takes `&Attrs` **and** an alignment param, no FontSystem;
`set_metrics`, `set_wrap` likewise FontSystem-free), runs the white-space
collapse pre-pass (§ 5.2), invalidates the cached intrinsics, and calls
`tree.mark_dirty(node_id)` (public, recursive to ancestors —
taffy_tree.rs:873, verified). `Added<Text>` / `RemovedComponents<Text>` drive
`set_node_context` registration/unregistration (§ 2.2).

**Why a dedicated step.** Text content changing without any style change must
still invalidate Taffy's leaf cache, and the only lever is `mark_dirty` —
`set_style` won't be called for it. **Runner-up rejected:** widening
`sync_styles`' `Changed<>` Or-filter and doing buffer work inside
`translate_one_entity` — three strikes: the filter is already at Bevy's
15-tuple cap (`systems.rs:1971-2043`), `translate_one_entity` is shared
verbatim with both cq re-run paths (`systems.rs:2282-2292` documents why) and
must not gain buffer side-effects that would double-apply mid-frame, and
buffer sync wants to run once-per-frame-before-layout, not once-per-
translation. Also rejected: relying on `set_style` being called anyway —
simply incorrect. Ordering before `SyncStyles` lets the same frame pick up a
text-driven style insert (new leaf) cleanly.

### § 4.2 `TextCommit` — reshape at final width (post-CqDescendantReRun)

**Decision.** The new **last** layout step. The buffer's last measured width
can differ from the laid-out width (stretch / grow / percent), so for each
text entity, compare `buffer.size()` to the final Taffy **content-box** size;
if different, `set_size(Some(w), Some(h))` + `shape_until_scroll`; apply
`Align` (§ 5.3); write `ResolvedBaseline` and `ComputedTextLayout`
(§ 6) — **idempotent-insert**, copying `write_resolved_layout`'s discipline
(`systems.rs:2657-2691`). Steady-state cost is O(0): the `buffer.size()`
equality short-circuits.

**Why last, not folded into step 7.** `cq_descendant_rerun` (step 9) re-runs
compute and **rewrites `ResolvedLayout` after** `write_resolved_layout` (step
7) (the `cq_descendant_rerun` system is its own compute site — system names
pin the reference per the T3 erratum; line numbers drift) — folding finalize
into step 7
would reshape against sizes step 9 may immediately invalidate, producing
one-frame-stale glyphs on exactly the cq-cascade frames the layout spec worked
hardest to make same-frame-correct. **Runner-up rejected:** a separate system
in a post-layout set (`BuiySet::Style` or a new `BuiySet::Text`) — works
mechanically but scatters the layout↔text contract across sets and leaves the
step unprotected by `tests/layout_pipeline_order.rs`; finalize is the text
half of the layout handshake and belongs in the asserted chain.

**As landed (T3, 2026-06-11):** the trigger row for this step
([architecture.md § 5.1](architecture.md): "the TextSync-dirty set ∪
`Changed<ResolvedLayout>`") misses measure-touched buffers whose resolved
size did not change — an ancestor resize re-probes the leaf at a probe
width, the leaf's own resolved size holds, neither trigger fires, and the
buffer would be left at the probe width. As built, commit iterates **all**
text entities behind a cheap reconcile guard, with measure's
`height_opt = None` as the catch-all signal: measure never sets a height,
commit always does, so a probe-left buffer can never compare equal (T3 plan
decision 7).

**As landed (T3, 2026-06-11):** `set_size(Some(w), Some(h))` keeps cosmic's
height windowing, so `overflow: visible` text taller than its box does not
lay out past the content height (`shape_until_scroll` stops at `scroll_end`;
`LayoutRunIter` also cuts at `height_opt`) — such lines are absent from
`ComputedTextLayout` and from the glyph producer's emission until the
overflow seam is revisited with overflow painting (T3 plan decision 9).

### § 4.3 One compute helper, three sites — re-entrancy and the ≤2× ceiling

**Decision.** One helper — `compute_roots_with_text_measure(tree, fonts,
text_query, window_size, …)` — replaces plain `compute_layout` at **all
three** compute sites: the `taffy_compute`, `cq_flip_rerun`, and
`cq_descendant_rerun` systems (system names, not line numbers, are the
pinned references — the charter's `systems.rs` line refs drifted before T3
even landed and keep drifting; T3 erratum, edited in place). The closure is
rebuilt per call from current world state, holds no cross-call state, and
never issues `Commands`. Re-entrancy is then free: a flip frame's second
compute re-measures only nodes the flip dirtied (Taffy's cache covers the
rest), and buffer mutation is idempotent (`set_size` to the same width is a
no-op relayout). Measure adds **zero** extra Taffy passes — it rides whatever
passes layout already runs, preserving the architecture.md § 9 "never more
than twice" commitment verbatim; `LayoutTaffyComputeCount`
(`systems.rs:96-107`) semantics are unchanged.

**Runner-up rejected (correctness bug, not an option):** migrating only
`taffy_compute` and leaving the cq paths on plain `compute_layout` — a cq flip
that changes a text ancestor's width would re-lay-out the text leaf with the
**zero** measure (taffy_tree.rs:925-927), collapsing it mid-frame. This is the
same drift failure the shared `translate_one_entity` helper already guards
against on the style side. Also rejected: a third text-driven re-run pass —
text needs no extra pass.

---

## § 5 F-tier property mapping

### § 5.1 line-height → `Metrics` **F**

`line-height` and `font-size` map to the buffer's `Metrics { font_size,
line_height }` (`Metrics::new(font_size_px, line_height_px)`; CSS number
values via `Metrics::relative(font_size, scale)`). `Metrics` feeds straight
into measure — `line_height` is the Σ term of measured height. **Runner-up
rejected:** pushing line-height per-span through `Attrs` — that is the C-tier
rich-text path, the wrong layer for the F-tier single-style paragraph, and it
complicates the intrinsic cache key.

### § 5.2 white-space / text-wrap → `Wrap` + collapse pre-pass **F**

`text-wrap: wrap` → `Wrap::Word`; `nowrap` → `Wrap::None` (verified 0.19
variants: `None | Glyph | Word | WordOrGlyph`; C-tier `overflow-wrap` later
flips to `WordOrGlyph`/`Glyph`). **The whitespace-collapse pre-pass is
mandatory, not optional:** cosmic-text lays out the source string verbatim, so
CSS-default collapsing must happen before `set_text` or measured widths
include literal runs of spaces — same pre-pass family as the documented
text-transform and bidi-control insertions
(`docs/prior-art/cosmic-text/lessons.md`). `text-wrap: balance|pretty|stable`
parse and **degrade to greedy `Word` wrap with a warn-once** — no engine
support (cosmic-text and Parley both lack balancing per the prior-art); the
foundation F-row is satisfiable by accepting the keywords with documented
greedy behavior, promotable later without resharpening this seam. **Runner-up
rejected:** implementing balancing above cosmic-text in v1.

**The `white-space` value table (normative).** Each value resolves to a
(collapse mode × `cosmic_text::Wrap`) pair:

| `white-space` | Collapse mode (pre-pass) | `Wrap` |
|---|---|---|
| `normal` | collapse | `Word` |
| `nowrap` | collapse | `None` |
| `pre` | preserve | `None` |
| `pre-wrap` | preserve | `Word` |
| `pre-line` | preserve-breaks | `Word` |

`text-wrap` composes where CSS says it does: `text-wrap: nowrap` forces
`Wrap::None` over the table's wrap column; `wrap` keeps it.

**The collapse pre-pass (spec level).** A pure `&str → Cow<str>` transform
run inside `TextSync` (§ 4.1) immediately before `set_text`, parameterized by
the collapse mode; the mode is part of the intrinsic-cache content version
(§ 3.2). Rules, per CSS Text Level 3 § 4.1 phase I:

- **collapse** (`normal` / `nowrap`): segment breaks (LF, CR, CRLF —
  normalized first) and tabs each become a collapsible space; runs of
  collapsible spaces collapse to one; leading and trailing collapsible spaces
  are trimmed. The result reaches cosmic-text as one logical line — soft
  wrapping, if any, is `Wrap`'s job.
- **preserve** (`pre` / `pre-wrap`): nothing collapses; segment breaks become
  hard line breaks (cosmic-text buffer lines); tabs pass through untouched to
  cosmic-text's tab stops — `Buffer::set_tab_width(u16)` (verified 0.19, lazy
  like the other setters), set to **8** (the CSS `tab-size` initial value) at
  `TextSync`; the C-tier `tab-size` property later drives the same setter.
- **preserve-breaks** (`pre-line`): segment breaks become hard line breaks;
  spaces and tabs collapse as in collapse mode within each segment.

### § 5.3 text-align — a finalize concern, not a measure concern **F**

Cosmic `Align` needs the final line width to position runs, so alignment is
applied in `TextCommit` (via `set_text`'s `Option<Align>` param / per-line
align), never during measure. This file owns the *enablement* (the finalize
pass that gives `Align` its width); the align **value** surface is the styling
area's.

**The value table (normative).** CSS `text-align` → `cosmic_text::Align`
(0.19 variants: `Left | Right | Center | Justified | End`):

| `text-align` | Mapping | Degradation note |
|---|---|---|
| `start` | `None` (no per-line align) | cosmic-text's unaligned default follows the line's BiDi direction — exactly CSS `start` |
| `end` | `Align::End` | — |
| `left` / `right` / `center` | `Align::Left` / `Align::Right` / `Align::Center` | physical, as CSS specifies |
| `justify` | `Align::Justified` | — |
| `justify-all` | `Align::Justified` + warn-once | last-line justification is not exposed upstream; degrades to `justify`, promotable without reshaping this seam |
| `match-parent` | resolved at style time: the parent's computed value with its `start`/`end` lowered against the parent's direction (per CSS), then mapped by this table | a style-resolution concern; never reaches cosmic-text as a distinct value |

### § 5.4 per-node direction (`dir` analogue) **F**

**`TextDirection { Ltr, Rtl, Auto }`** — the foundation `dir`-analogue F-row
(text.md § 3.4 "Bidirectional text"). Lowered entirely in the `TextSync`
pre-pass: after the § 5.2 collapse transform, each buffer line gets a **strong
direction mark prepended** — `Ltr` → LRM (U+200E), `Rtl` → RLM (U+200F),
`Auto` → nothing (cosmic-text's per-paragraph first-strong default IS the CSS
`dir=auto` behavior; absent component = `Auto`). UAX #9 P2 then finds the mark
as the line's first strong character and forces the paragraph level, so the
base direction drives everything downstream: reordering, the unaligned
`start` default (`rtl → Align::Right`), `Align::End`'s resolution, and the
`LayoutRun.rtl` flag the editing campaign's caret model consumes.

**Why NOT a whole-line isolate wrap (the rejected runner-up):** wrapping the
content in `LRI`/`RLI`/`FSI` … `PDI` forces reordering *inside* the isolate
but can never set the paragraph level — UAX #9 P2 explicitly **skips strong
characters between an isolate initiator and its matching PDI** when
determining the paragraph level (verified in the vendored
`unicode-bidi-0.3.18/src/lib.rs:381-399`; cosmic-text derives the line's
`rtl` from that level, `shape.rs:1349-1352`, and the unaligned default from
`rtl`, `shape.rs:2764`). A fully-wrapped line therefore always resolves LTR —
`dir=rtl` would never right-align and plain RTL text would regress. The web
platform lowers *block-level* `dir` the same way this section does — as a
higher-level-protocol paragraph level (UAX #9 HL1 / CSS `direction`);
isolates are the lowering for *inline* span direction (`<bdi>`, inline
`dir`), which stays the named seam for the rich-text tier. Also rejected: a
per-`Buffer` direction override API — cosmic-text 0.19 exposes none.

Ordering: the prepend runs AFTER collapse, so the trim sees the authored
leading/trailing spaces, not the mark (the marks are `Cf`, not
`White_Space`, but prepending first would still offset the trim's view of
the line start). The direction value joins the § 3.2 intrinsics-cache
content version (a `dir` flip is a content change). **Editing consequence
(successor campaign):** the prepended mark shifts the line's byte offsets by
its UTF-8 length (3 bytes); hit-testing and cursor↔source mapping must map
through the same pre-pass offset table as the § 5.2 collapse transform.

**As landed (T5, 2026-06-11):** marks are prepended per **non-empty** line
only — a shaped mark on an empty line could grow a phantom glyph and flip
T3's glyphs-keyed `ResolvedBaseline` for `Text("")` (§ 6's as-landed note).
Empty-line caret direction is the editing campaign's offset-table seam.

### § 5.5 Named C-tier deferrals

`overflow-wrap` (Wrap variant flip), `text-indent` (a measure-input offset on
the first line), `vertical-align` (consumes § 6's baseline), per-span metrics
(rich-text `AttrsList`) — seams named here, designed in their own areas.

---

## § 6 Outputs and baseline plumbing

**Decision.** `TextCommit` writes dedicated output components —
`ComputedTextLayout` (the settled line/run geometry,
[architecture.md § 3.3](architecture.md)) and the baseline carrier below —
idempotent-insert; `ResolvedLayout` is untouched:

```rust
/// Offsets from the node's content-box top, from the first/last
/// LayoutRun.line_y ("Y offset to baseline of line", verified 0.19 doc).
#[derive(Component, PartialEq)]
pub struct ResolvedBaseline { pub first: f32, pub last: f32 }
```

Taffy 0.10's measure API cannot carry a baseline (§ 3.1 fact 3), so baseline
is a Buiy-side output regardless; the only question was the carrier.
**Runner-up rejected:** extending `ResolvedLayout` with an `Option<f32>`
baseline — `ResolvedLayout`'s change-tick is load-bearing three ways
(`sync_styles`' `Changed<ResolvedLayout>` cascade trigger,
`cq_descendant_invalidate` seeding, render-bridge damage seeding —
`bridge.rs:24-42`); a frequently-rewritten field risks spurious cascade
wakeups and forces every idempotence check to compare it. Also rejected:
recomputing from the `Buffer` at each consumer — future inline baseline
alignment, AccessKit text geometry, and devtools shouldn't need `Buffer`
access or a FontSystem lock to read a number layout already computed.

**As landed (T3, 2026-06-11):** a "no laid-out runs" removal condition for
`ResolvedBaseline` never matches literally — cosmic-text synthesizes a
glyph-less `LayoutLine` for every empty `BufferLine` (shape.rs:3025–3051,
"create a visual line for empty lines"), so `Text("")` still yields one run.
As built, baseline presence keys on **glyphs**: the synthetic line's
`line_y` is the centering artifact of a zero-ascent strut, not a baseline —
while the synthetic line stays in `ComputedTextLayout` as real
`line_top`/`line_height` geometry (caret math and the height fold both
count it) (T3 plan decision 15).

The commit→render handoff: `TextCommit` leaves the buffer shaped at its
final size; the glyph producer ([glyph-pipeline.md](glyph-pipeline.md))
reads it at extract (`extract_buiy_glyphs` in `ExtractSchedule` —
[architecture.md § 4.4](architecture.md)) and fills
the render-world `ExtractedGlyphs`
(`crates/buiy_core/src/render/prepare.rs:47-51`) under the extract damage
discipline (rebuild only on change probes, else retain — render
architecture.md § 3.1 + the 2026-06-07 retention design). Damage keys on the
commit **output** components (`ResolvedBaseline`, `ComputedTextLayout`),
never on `TextBuffer` ticks (§ 7).

---

## § 7 Change-detection contract — protecting O(0)

**Decision.** The measure closure and `TextCommit` access `TextBuffer` via
`Mut::bypass_change_detection()`. A width probe is not a semantic change —
letting it tick `TextBuffer` would mirror the exact bug
`write_resolved_layout`'s idempotent-insert exists to prevent (perpetual
`Changed<>` → `sync_styles` iterating every node every frame; the failure mode
is documented at `systems.rs:2643-2656`). Author intent rides `Changed<Text>` and
the pinned text-style carriers (`FontFamily`/`FontSize`/`FontWeight`,
font-assets § 8; the white-space/wrap/align/`TextDirection` carriers, § 5)
(consumed by `TextSync`, § 4.1);
`Changed<TextBuffer>` is reserved for nothing. **Runner-up rejected:** letting
`Changed<TextBuffer>` fire on every measure and filtering downstream — every
consumer would re-derive "did anything really change"; the O(0) invariant dies
by a thousand filters.

The pinning instrument: a `TextMeasureCallCount` resource (the
`SyncStylesIterCount` precedent, `systems.rs:109-117`) asserting **zero
measure invocations on a no-change frame** (Taffy's cache holds, § 2.2's
edge-triggered registration holds). This is the contract that keeps text from
regressing layout's flagship invariant (layout architecture.md § 9).

---

## § 8 Verification

Headless (no GPU, the standard gate):

1. **Pipeline order** — `tests/layout_pipeline_order.rs` extends to assert
   `TextSync` before `SyncStyles` and `TextCommit` after
   `CqDescendantReRun`.
2. **Content sizing** — a text leaf auto-sizes to its content in a flex
   column; wraps when the parent width shrinks and `ResolvedLayout` height
   grows accordingly.
3. **Intrinsic keywords** — `Sizing::MinContent`/`MaxContent` resolve on text
   leaves (§ 3.3), incl. min-content = longest-word width under `Wrap::Word`
   (CJK / no-break / tab fixtures — risk 4).
4. **Steady state** — a no-change frame performs zero measure calls
   (`TextMeasureCallCount == 0`) and zero buffer relayouts.
5. **Re-entrancy** — a cq activation flip that resizes a text ancestor
   re-measures same-frame and `LayoutTaffyComputeCount` stays ≤ 2× roots.
6. **`shape_until_scroll` total-height semantics** — a unit test pins that
   `height_opt = None` + `prune = false` shapes/lays out **all** lines (risk
   2: verified by signature + prior-art convention, not by 0.19 doc text;
   fallback is the per-line `BufferLine` shape/layout API).
7. **Perf fixture** — 1000-paragraph ancestor-width resize, bounding the
   per-frame cost to line-breaking (`layout_opt`) only, before the
   verification area signs off ([verification.md](verification.md)).

GPU-lane (golden) interaction: fractional line heights (16px × 1.2 = 19.2)
meet Taffy's `use_rounding` whole-px rounding (taffy_tree.rs:915-919) — golden
fixtures carry an explicit rounding-policy note rather than ad-hoc epsilons.

---

## Open questions

1. **FontSystem resource shape — blueprint vs. engine pin.** This area's
   exploration recommended a plain `Resource BuiyFontSystem(FontSystem)`
   (FontSystem is verified `Send + Sync` in 0.19, so `ResMut` already
   serializes main-world access), but the engine-ownership decision (area 0,
   [architecture.md](architecture.md)) pins
   `SharedFontSystem(Arc<Mutex<FontSystem>>)` because the RenderApp needs a
   clone for atlas-miss rasterization. This file follows the pin (§ 3.4) and
   flags rather than relitigates; if the render world ever stops needing the
   `FontSystem` (e.g. pre-rasterized warmup only), the plain-Resource shape is
   the simplification to revisit.
2. **Intrinsic-keyword fidelity under stretch.** Because taffy 0.10.1's
   `Dimension` cannot express min-/max-content (verified, § 3.3), keyword
   resolution lives in the measure closure and loses to a parent-resolved
   `known_dimensions` (cross-axis stretch). The blueprint's "makes
   `Sizing::MinContent/MaxContent` real" therefore holds for the main-axis /
   shrink-to-fit cases the F-tier needs, with the stretch case documented as
   `auto`-equivalent until a Taffy version carries the keywords natively.
3. **Multi-window measuring.** `taffy_compute` sizes roots against the first
   window (`systems.rs:2611-2615`) — a pre-existing limitation text inherits;
   text measured against the wrong viewport on secondary windows is out of
   scope here but named so it is not mistaken for a text bug.
4. **Per-buffer memory.** One `Buffer` per text node retains `shape_opt` +
   `layout_opt` caches; thousands of nodes may need a trim policy. (The
   SwashCache stays empty by construction — glyph-pipeline § 3.2's uncached
   path; `BufferLine` caches are this seam's.)

## Sources

- docs.rs/cosmic-text/0.19.0 — `struct.Buffer.html` (`new`, `set_text`,
  `set_size`, `set_wrap`, `set_metrics`, `set_metrics_and_size`,
  `shape_until_scroll`, `layout_runs`, `size`; `Send + Sync` auto-traits),
  `struct.LayoutRun.html` (`line_y` = "Y offset to baseline of line",
  `line_top`, `line_height`, `line_w`), `struct.Metrics.html`
  (`new`/`relative`), `enum.Wrap.html`, `struct.FontSystem.html`
  (`Send + Sync`) — fetched 2026-06-09.
