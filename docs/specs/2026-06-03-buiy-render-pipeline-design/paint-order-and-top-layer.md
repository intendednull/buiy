# Buiy render — paint order and top layer

**Parent:** [README.md](README.md)

This file defines how render **consumes** the immutable stacking output layout
produces (README § 2 pillar 1, [`buiy-layout-design/stacking-and-top-layer.md`](../2026-05-08-buiy-layout-design/stacking-and-top-layer.md)).
Its scope is purely the *consumer side*: how the typed-primitive batched node and
the top-layer composite pass ([architecture.md](architecture.md)) walk
`StackingContext.painters_z`, how the paint walk and the hit-test walk are pinned
to be exact inverses, how top-layer members escape to the root composite pass, the
`::backdrop` model, and the `Display::None` / `Containment.content_visibility` /
`CssVisibility` skip rules.

Render contributes **nothing** to stacking. Every fact below — membership, paint
order, tiebreak, top-layer tier ordering — is decided in layout sub-pass 6f
(`stacking_context` in `crates/buiy_core/src/layout/systems.rs`) and read here
verbatim. The one cross-subsystem invariant this file *fixes* is the paint/hit-test
ordering identity; the picking backend that obeys it lives in
`buiy-input-events-design`.

> **Verified against source.** The components and the 6f algorithm cited here are
> the shipped Phase-9 types: `StackingContext { painters_z: Vec<Entity> }`
> (`crates/buiy_core/src/components.rs`), `Stacking { z_index, isolation, top_layer }`,
> `TopLayer { None | Modal | Popover | Tooltip | Fullscreen }`, and the
> `TopLayerActivation { order: VecDeque<Entity> }` resource
> (`crates/buiy_core/src/layout/systems.rs`).

## 1. Forward walk of `painters_z`

The typed-primitive batched node (README § 2 pillar 2, [architecture.md](architecture.md))
paints a stacking context by iterating its `StackingContext.painters_z` **front to
back** — index 0 is the bottom-most painter, the last index is the top-most. This is
the painter's algorithm applied to a list layout already sorted; render performs no
comparison, no sort, no tree walk of its own.

```rust
// Render-side consumption (extract / queue stage). NOT a re-sort — a read.
// `painters_z` arrives pre-ordered from sub-pass 6f.
for &painter in &stacking_context.painters_z {
    // emit this painter's typed primitives (quad / shadow / glyph / path)
    // into the per-primitive + per-layer batch (architecture.md § batching).
}
```

`painters_z` is the *paint order of every descendant within this context*, sorted by
sub-pass 6f into the spec § 2.1 tiers: negative `z_index` first, then in-flow
non-positioned (document order), floats (always empty in Buiy), in-flow positioned
with `z_index: Auto` (document order), then positive `z_index`. The render walk
neither knows nor cares about the tiers — it trusts the order.

### 1.1 Nested stacking contexts are entered atomically

A nested stacking context appears in its parent's `painters_z` as a **single entry**
(the nested context's root entity), sorted into the parent's order by the nested
root's own `z_index`. When the forward walk reaches such an entry, render **descends
into that entity's own `StackingContext.painters_z` as a unit** — it paints the
entire nested context (recursively, by the same rule) before returning to the parent
list. It never flattens the nested list into the parent's, and never re-sorts the
two together.

This is the structural guarantee that makes the "layout writes, render reads"
contract hold under nesting: sub-pass 6f's `painters_of` walk stops descending the
moment it hits a child that itself forms a context (`if !forming.contains(&node)` in
`stacking_context`), so each `painters_z` is exactly one context's slice. Render's
recursion mirrors that boundary. If render flattened or re-sorted across the
boundary it would reintroduce the per-frame layer-tree re-walk that the immutable
handoff exists to eliminate (the Blink re-walk Buiy explicitly rejected —
[blink/stacking-and-paint.md § 2](../../prior-art/blink/stacking-and-paint.md)).

The recursion terminates because the context tree is a finite DAG over distinct
entities, and an entity is excluded from its own `painters_z` (sub-pass 6f builds a
context's list from its *descendants*, never itself).

### 1.2 Paint-order determinism is layout-owned

Within one tier, ties break by **document order** — the depth-first `Children`-order
index. Bevy entities have no inherent document order (archetype iteration is not a
Buiy ordering guarantee, [blink/stacking-and-paint.md § 2.1](../../prior-art/blink/stacking-and-paint.md)),
so sub-pass 6f establishes the order itself: it walks `Children` depth-first into
the list, then applies a **stable** sort keyed on the paint tier (`paint_key` +
`sort_by_cached_key` in `stacking_context`), so equal-tier entries keep their
document-order positions.

**Render ASSUMES this tiebreak is already pinned and must not introduce its own
ordering.** It must not, for example, fall back to `Entity` id order, GPU buffer
insertion order, or `z`-from-`GlobalTransform.translation.z` to disambiguate equal-z
painters — any of those would make paint order depend on a value 6f did not author,
silently diverging from CSS document order and (worse) from the hit-test walk that
reads the same list (§ 2). The batching stage (architecture.md) may reorder *draw
calls* for GPU-state coalescing **only when the reorder is paint-order-neutral**
(two batches that do not overlap, or that are separated by a context boundary);
overlapping painters within one context retain their `painters_z` index as the
compositing order. This is the one place the render side could accidentally
reintroduce nondeterminism, so it is called out as a hard constraint, not a
guideline.

## 2. The paint / hit-test ordering identity

**Hit-test order = `painters_z` reversed.** The entity painted last (top-most) is
the first hit-test candidate; picking walks the list back to front and returns the
first painter whose bounds contain the point and that is pickable. This is the
single invariant this spec fixes across the layout↔render↔input boundary, and it is
non-negotiable: paint and pick read the *same* `painters_z`, in opposite directions,
so they **cannot diverge**. There is no second sorted structure for picking to drift
against.

```text
paint:    painters_z[0]  ── front-to-back ──▶  painters_z[n-1]   (bottom → top)
hit-test: painters_z[n-1] ── back-to-front ──▶ painters_z[0]     (top → bottom, first hit wins)
```

This is the rule Servo settled with its `build_display_list` /
`hit_test.rs` pair — "the forward version of the reversed stacking-context walk"
([servo-stylo/rendering.md § hit-test order](../../prior-art/servo-stylo/rendering.md)) —
and the rule Blink's reverse paint-order hit test follows
([blink/stacking-and-paint.md § 5](../../prior-art/blink/stacking-and-paint.md)).
Front-loading the sort in 6f pays off twice: render walks `painters_z` forward, the
interaction pass walks it (or its per-context slices) in reverse.

### 2.1 The pickability flag

A second input is needed during the back-to-front walk: a per-entity **pickability /
pointer-events flag**. `pointer-events: none` is a *hit-test-only* concept — it
changes neither paint, visibility, nor stacking; it makes picking fall through to the
painter underneath ([blink/stacking-and-paint.md § 5](../../prior-art/blink/stacking-and-paint.md)).
So the reverse walk skips a painter whose flag says "transparent to input" and
continues to the next, rather than treating it as a layout or stacking concern.

Buiy's picking substrate is `bevy_picking` 0.18, whose `Pickable` component already
carries the two orthogonal bits this needs: `should_block_lower` (does a hit here
stop the walk descending to lower painters?) and `is_hoverable` (is this entity an
input target at all?). The mapping of `pointer-events` keywords onto `Pickable` —
and the backend that performs the reverse-`painters_z` walk honoring it — is owned
by **[`buiy-input-events-design`](../2026-05-07-buiy-foundation/README.md)**, not
this spec. This file fixes only the ordering identity (hit-test = reversed
`painters_z`, with a pickability skip); the mechanism lives there.

The top layer (§ 3) participates in this identity automatically: top-layer members
are appended to the **end** of the root context's `painters_z` (they paint last), so
they are the **first** hit-test candidates — which is exactly why a modal is modal,
its backdrop intercepting clicks bound for content beneath it
([blink/stacking-and-paint.md § 5](../../prior-art/blink/stacking-and-paint.md)).

## 3. Top-layer compositing at the root

Top-layer members — entities whose `Stacking.top_layer != TopLayer::None` — escape
their parent stacking context and composite at the **root**, in the per-window
top-layer composite pass (README § 2 pillar 2, [architecture.md](architecture.md)).
Layout has already done the escape: sub-pass 6f excludes top-layer entities from
their parent's `painters_z` (`if top_layer_of(node) != TopLayer::None { continue; }`
in `painters_of`) and re-attaches them to their **root-ancestor** context's
`painters_z`, appended after all in-flow painters. Render therefore finds them
already at the tail of the root context's list and composites them there — it does
not hunt the tree for top-layer members.

### 3.1 The four-tier ordering, materialized into the `painters_z` tail

Render's **only** ordering input is the `painters_z` tail. `TopLayerActivation` is
**layout's** input to sub-pass 6f, *not* render's — render never reads it. 6f consumes
the activation stack and emits the resulting tier order *into* the root context's
`painters_z` tail, which is the single ordered structure render walks.

Within the top layer, order is the **four non-None tiers** (`None` is not a tier —
it means the entity is not in the top layer at all):

```text
Fullscreen  <  Tooltip  <  Popover  <  Modal        (tier, bottom → top)
                then, within a tier, activation recency (most recent on top)
```

Sub-pass 6f produces this order: it stable-sorts the `TopLayerActivation.order`
`VecDeque` (already in activation order, oldest → newest) by `tier_rank`, so the
tier is the primary key and activation recency is the within-tier tiebreak. The
concrete rank values are owned by layout's `tier_rank`
(`crates/buiy_core/src/layout/systems.rs`); render consumes the resulting order
without depending on the numbering. The `VecDeque` is the LIFO activation stack
([blink/stacking-and-paint.md § 4](../../prior-art/blink/stacking-and-paint.md))
maintained whenever `top_layer` flips `None → non-None`. Render reads the resulting
tail-of-`painters_z` order verbatim.

The `Tooltip` tier and the strict tier ordering are a deliberate Buiy divergence
from Blink's pure-recency single list (Blink would let a popover opened after a modal
paint above it; Buiy's tier rule keeps it below) — recorded in
[blink/stacking-and-paint.md § 4](../../prior-art/blink/stacking-and-paint.md). It is
a *layout* decision; render only consumes its result.

> **`v1` simplification — single global top layer (README § 5 #1).** Phase 9 ships
> one global `TopLayerActivation` reading the primary window; true per-window
> routing is owned by `buiy-window-and-surface-design`. The render side reserves the
> per-window node-group structure so the composite pass is *already* keyed by window
> ([architecture.md](architecture.md)); the global-activation read is the tracked
> dependency, not a final decision. Multiple root trees are handled correctly today
> (6f attaches each escapee to its own `root_ancestor`), which is the seam per-window
> routing slots into.

### 3.2 Escape from clip, but not from the AccessKit tree

A top-layer member's effective clip is the **window viewport**, not any ancestor's
`ClipRect`. It is not clipped by an ancestor whose `Overflow` is
`OverflowMode::Hidden`/`Clip` ([`buiy-layout-design/stacking-and-top-layer.md` § 4.3](../2026-05-08-buiy-layout-design/stacking-and-top-layer.md)).
Because the top-layer composite pass runs against the root, not the escapee's
ancestor clip chain, this falls out for free: render simply does not intersect a
top-layer member's `ClipRect` with its ancestors. The `WriteClipRects` pass
([clip-and-transform.md](clip-and-transform.md)) is the owner of *not* folding
ancestor clips into a top-layer member's `ClipRect` — modeling clip escape as
"effective clip = window viewport" rather than a per-entity skip-clip hack, matching
Blink's clip-tree re-parenting ([blink/stacking-and-paint.md § 4](../../prior-art/blink/stacking-and-paint.md)).

**Escaping clip must not escape the accessibility tree.** A top-layer member leaves
its parent's *paint* context but keeps its layout-tree position and its place in
document order; `buiy-accessibility-design` builds the AccessKit tree from layout
topological order ([`buiy-layout-design/architecture.md` § 5](../2026-05-08-buiy-layout-design/architecture.md)),
which is unchanged by top-layer promotion. So a modal that paints at the root is
**still a child of its `ChildOf` parent in the AccessKit tree**. The render-side
escape is a paint/compositing re-parent only; it touches neither `ChildOf`, nor
`GlobalTransform` propagation ([clip-and-transform.md](clip-and-transform.md)), nor
the a11y tree. This is the one place a naive "move the entity to the root" shortcut
would corrupt accessibility, so the boundary is explicit: **only `painters_z`
membership and the clip rect change; hierarchy and a11y order are invariant.**

## 4. The `::backdrop` model — OPEN

Modal/popover dimming (the CSS `::backdrop` pseudo-element) is **not yet decided**
(README § 5 #3). Each top-layer entry in CSS gets a viewport-sized box rendered
*immediately beneath* it, which is how a modal opened over a popover dims the popover
too ([blink/stacking-and-paint.md § 4](../../prior-art/blink/stacking-and-paint.md)).
Two models are on the table; this spec **proposes** the first and marks the choice
open against README § 5 #3.

- **(proposed) Render-synthesized backdrop.** Render synthesizes the backdrop
  *primitive* per top-layer entry, but it would own **no ordering input** for it: the
  backdrop's slot — **immediately below its owner** in the top-layer paint order — would be
  materialized **layout-side into `painters_z`** as the synthesized `owner_index − ε`
  position, so render never carries a second sort key that could reintroduce the
  pillar-1 layer-tree drift. The `owner_index − ε` materialization is **part of this
  proposed (not-yet-committed) model**, not settled v1: it would be a **layout (sub-pass
  6f) deliverable *if* this model is accepted**, and nothing emits it today. This is
  not a new ordering concept, though: the
  `TopLayerActivation` `VecDeque` is *already* the LIFO order a backdrop nests into,
  so 6f *would* materialize the backdrop as "owner's index − ε" in the `painters_z` tail, not
  a second list ([blink/stacking-and-paint.md § 4](../../prior-art/blink/stacking-and-paint.md)).
  It would composite with the owner as a unit, so a stack of modal+popover dims correctly
  in LIFO order with no app involvement, and the backdrop never appears as a sibling
  in normal stacking. It would carry no entity in the layout tree (it is a render
  artifact), so it does not perturb document order or the AccessKit tree (§ 3.2).
- **(runner-up) App-spawned scrim.** Modal dimming is expressed by the app spawning
  a full-window scrim entity at the appropriate `top_layer` tier. Simpler for the
  render side (no synthesized primitive), but pushes a correctness-sensitive
  concern — getting the scrim's tier and recency to nest under exactly the right
  owner — onto every app, and the scrim *does* occupy the layout/a11y tree.

The proposed model is preferred because it keeps `::backdrop` faithful (LIFO nesting
beneath the owner) without an app-tree entity, but the choice is **confirmed against
`buiy-window-and-surface-design`** before it cements — per-window backdrop routing
and the scrim-vs-synthesized boundary interact with that spec. Until then this is
**OPEN (README § 5 #3)** and no `::backdrop` primitive is committed in v1.

**v1 behavior during the open period: no dimming backdrop ships.** Until the model
above cements, v1 paints top-layer members with **no synthesized dimming box** beneath
them — a modal opened over a popover does not dim the popover. The four-tier ordering
(§ 3.1) and the clip/a11y escape (§ 3.2) ship; only the `::backdrop` dimming primitive
is withheld. Consequently the modal-over-popover golden (§ 6) asserts **tier ORDER
only** (Fullscreen < Tooltip < Popover < Modal, plus within-tier recency), **not
backdrop nesting** — there is no backdrop to nest in v1. This keeps the open `::backdrop`
decision from leaking an uncommitted primitive into the shipped golden.

## 5. Skip rules

The forward walk skips four categories. The first three skip a subtree *entirely*
(no box, no descendants) and their predicates are layout-owned; render reads their
result and emits nothing. The fourth — `CssVisibility::Hidden` — skips the subtree's
*paint* while **keeping its layout box**, and is render-owned.

### 5.1 `Display::None` — skipped entirely

A `Display::None` entity is invisible to layout: it is never given a Taffy node, gets
no `ResolvedLayout`, and is excluded from every `painters_z` (sub-pass 6f's
`display_none(node)` guard drops it before it can be pushed). So it never reaches
render — no paint, no clip, no stacking participation. There is no render-side
`Display::None` check to write; the absence from `painters_z` *is* the skip.

### 5.2 `Containment.content_visibility == Hidden` — subtree skipped

A `Containment.content_visibility == Hidden` entity prunes its **descendants** from layout entirely
(`content_visibility_skip(...) → SkipKind::HiddenPrune`, `crates/buiy_core/src/layout/systems.rs`):
Taffy never lays the subtree out, so the descendants have no geometry and appear in
no `painters_z`. Per CSS the Hidden entity *itself* still lays out and resolves its
own box ([`buiy-layout-design`, Phase 11](../../plans/follow-ups.md)) — so render
paints the Hidden entity's own box (background/border/etc.) but its subtree is
absent. This is a layout-side prune (paint + clip + stacking traversal all skipped
for the descendants because they were never laid out); render inherits it for free.

### 5.3 `Containment.content_visibility == Auto` off-screen — skip paint (render-owned half)

`Containment.content_visibility == Auto` is the one skip with a **render-owned half**, completing
layout's Phase 11. Layout owns the *layout* half: an `Auto` entity that is off-screen
**and** carries a `ContainIntrinsicSize` hint gets the Taffy size-sentinel and its
descendants detached (`SkipKind::AutoSentinel`,
[follow-ups.md](../../plans/follow-ups.md) — landed). But the *paint* skip for an
off-screen `Auto` entity is explicitly **a render concern Phase 11 does not own**
(follow-ups.md verbatim: "Auto's off-screen *paint* skip … remains a render concern
Phase 11 does not own").

This spec owns it: **render skips painting an off-screen `Containment.content_visibility == Auto`
subtree.** Render reuses layout's already-computed off-screen determination rather
than recomputing visibility — the same hysteresis-expanded-viewport test
(`is_off_screen` against the `ContentVisibilityMargin`-expanded rect, default 200px,
`crates/buiy_core/src/layout/systems.rs`). The carrier is **`OffscreenAuto`** (F-tier),
a layout-written, render-read marker component placed on each off-screen `Auto`
entity by the same off-screen test layout already runs inline in `sync_styles`
(`is_off_screen`, `crates/buiy_core/src/layout/systems.rs`; component-model § 12.2).
That inline test persists nothing render can read today — emitting
`OffscreenAuto` from it is the layout-side deliverable this rule depends on. Render
extract reads `OffscreenAuto` to drop the subtree's primitives, keeping render a thin
consumer (README § 2 pillar 1) rather than a second visibility engine. The component
is catalogued in [component-model.md § 12](component-model.md) and
[README § 3.1](README.md); this file fixes the behavior (off-screen `Auto` → no
paint) and the source of truth (layout's off-screen test, surfaced as `OffscreenAuto`,
not a render recompute).

The layout off-screen test (and therefore the future `OffscreenAuto` emission) is
**primary-window-only today**: layout builds the comparison viewport from
`Query<&Window, With<PrimaryWindow>>` (`crates/buiy_core/src/layout/systems.rs`), so a
second window's viewport never enters the off-screen determination. This is a tracked
per-window-layout dependency, mirroring the global-`TopLayerActivation` primary-window
read (§ 3.1, the D2 flag): both are seams that per-window layout/window routing
(`buiy-window-and-surface-design`) slots into, not final decisions.

The asymmetry is deliberate and matches CSS: `Auto` *off-screen without an intrinsic
hint* is the residual case where layout cannot skip the layout work (no placeholder
size), but render can and should still skip the paint — which is why the paint skip
is render-owned and geometry-light, gated only on the off-screen flag, not on the
hint.

### 5.4 `CssVisibility::Hidden` — skip paint, keep the box

Unlike the three layout-owned skips above, `CssVisibility` is **render-owned** (F-tier,
catalogued in [component-model.md § 12](component-model.md)): an
`enum CssVisibility { Visible, Hidden, Collapse }` whose `Hidden` variant means *paint
nothing for this entity and its subtree, but keep the layout box*. (The render-owned
CSS-visibility enum is deliberately **not** named `Visibility`: `bevy::prelude::Visibility`
exists with different variants/semantics and its own visibility systems — reusing the
name would collide, so the CSS property gets its own `CssVisibility` type, mirroring
the `Transform`-reuse name-collision rationale in [component-model.md § 12](component-model.md).)
This is CSS `visibility: hidden`: the subtree still occupies space (its `ResolvedLayout`
box is unchanged and its descendants stay in `painters_z`), but render emits no primitives
for it. So a `CssVisibility::Hidden` subtree differs from `Display::None` (which has no
box at all) and from `Containment.content_visibility == Hidden` (which has the parent box
but prunes descendants from *layout*): here the geometry is fully laid out and present in
`painters_z`, and the skip is purely a paint suppression.

Render's extract drops the `CssVisibility::Hidden` entity *and its descendants* from
primitive emission — the same subtree-scoped paint skip as the `OffscreenAuto` case
(§ 5.3), just keyed on `CssVisibility::Hidden` instead of the off-screen marker.

> **v1 implementation status (R5).** R5's `extract_buiy_nodes` implements the
> per-entity **leaf** skip only: `node_skip_reason` drops the entity that carries
> `CssVisibility::Hidden` (or `OffscreenAuto`), but does **not** yet drop its
> descendants — Buiy has no visibility cascade, so a `Visible`/default child of a
> `Hidden` parent stays in `painters_z` and would still paint. The subtree-scoped
> suppression this section mandates is a tracked follow-up
> ([follow-ups.md](../../plans/follow-ups.md) → "Render — subtree visibility
> suppression"; design fork in
> [2026-06-06-render-subtree-visibility-suppression-design.md](2026-06-06-render-subtree-visibility-suppression-design.md)).
> Deferred deliberately: R5 emits only the `Changed`-gated set and leaves full-set
> assembly + the persistent unchanged-painter cache to R6/R8, and a cache-coherent
> descendant drop must coordinate with that cache (or land as a separate render-prep
> visibility-propagation pass). The leaf skip is sufficient today because no v1 code
> sets `CssVisibility::Hidden` on a non-leaf entity and layout does not yet emit
> `OffscreenAuto`.

Because
the box is retained, the hit-test interaction is **not** decided here: per CSS,
`visibility: hidden` also removes the subtree from hit-testing, but that picking skip
is owned by [`buiy-input-events-design`](../2026-05-07-buiy-foundation/README.md)
alongside the `pointer-events` mapping (§ 2.1), not by this paint rule. The `Collapse`
variant (table-row / flex-item collapse) is a **deferred marker** — named in the
component model but not painted-differently in v1; v1 ships only the `Hidden`
paint-skip.

## 6. Verification

Per README § 7 ([verification.md](verification.md)), these consumption rules are
proven where they are cheapest:

- **Paint-order identity (headless).** Given a fixture's `StackingContext.painters_z`
  (computed by layout, asserted in the layout suite), a unit test asserts the render
  extract emits primitives in `painters_z` index order and the picking-order
  helper yields the exact reverse — pinning the § 2 identity without a GPU. The
  ordering identity is a pure-data property, so it does not need the golden-image
  harness.
- **Nested-context atomicity (headless).** A fixture with a nested SC inside a
  parent asserts the render walk descends the nested `painters_z` as a unit and
  emits no interleaving with parent painters between the nested entry and its
  completion.
- **Top-layer compositing (gate #2 visual-regression + gate #10 hit-target).** A
  modal-over-popover golden image proves the four-tier **order only** (tier ranking +
  within-tier recency, § 3.1) on the canonical CI GPU — **not** backdrop nesting, since
  v1 ships no dimming `::backdrop` (§ 4, OPEN); the hit-target gate proves the modal
  intercepts input first (§ 2, § 3).
- **Clip escape vs. a11y retention (headless).** A top-layer member under an
  `Overflow::Hidden` ancestor asserts (a) its `ClipRect` is the viewport, not the
  ancestor box ([clip-and-transform.md](clip-and-transform.md), gate #5
  layout-snapshot), and (b) it remains its `ChildOf` parent's child in the AccessKit
  tree (§ 3.2).
- **Skip rules (headless).** `Display::None` / `Containment.content_visibility == Hidden`
  subtrees emit no primitives (their absence from `painters_z` is asserted layout-side; the
  render half asserts no extract output); an off-screen `Containment.content_visibility == Auto`
  subtree emits no paint while its on-screen sibling does (§ 5.3); and a
  `CssVisibility::Hidden` subtree emits no primitives **while retaining its layout box
  and `painters_z` membership** (§ 5.4) — the render-owned paint skip, sharing the
  `OffscreenAuto` subtree-scoped mechanism (§ 5.3) and distinguished from `Display::None`
  by the kept box.

---

*Consumer-side spec for layout's stacking output. The ordering identity (§ 2) is the
cross-subsystem invariant this file fixes; its mechanism lives in
`buiy-input-events-design`. The `::backdrop` model (§ 4) is OPEN against README § 5
#3, pending `buiy-window-and-surface-design`.*
