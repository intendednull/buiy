# Coordinate-space + picking/clip correctness — child C1 of the widget-catalog campaign

`2026-06-22` · `[draft]` · Wave 1 · realizes foundation `architecture` (pillar-5 transform bridge), `cross-cutting` (§3.18 picking backend) · depends on C0

> **Scope (umbrella §4 C1, decision §2.5).** Keep `ResolvedLayout.position`
> parent-**local**. Route every *absolute* consumer — picking (`emit_picks` /
> `hit_test` / `point_in_aabb`), the clip producer (`render/clip.rs`
> `write_clip_rects`), outline ink-bounds, and overlays — through
> `GlobalTransform`, made **non-optional** (drop the prototype's
> `unwrap_or(layout.position)` fallback that silently mixed two spaces). Fix the
> lying `components.rs:65` "window-relative" doc comment. **Preserve** the
> `bridge.rs:138` `base = position − acc` invariant (rejected alt-b double-
> corrects and breaks the scroll fold). This child owns the **coordinate basis**
> only — *not* the depth/event model (`emit_picks`'s smallest-area depth, camera
> ref, no-hit emission, `Pickable`), which is **C3** per umbrella §6.1/§6.2.

---

## 1. Problem & current state

Bug 1 (audit §2) is a coordinate-space **class** bug: a consumer reads
`ResolvedLayout.position` and treats it as window-absolute, but the field is
**parent-relative** (Taffy's per-node `location`, written verbatim). The two
spaces coincide only when every ancestor sits at the window origin — exactly the
"top-left layout makes relative ≈ absolute" trap.

**The single writer is layout-local by construction.**
`write_resolved_layout` (`crates/buiy_core/src/layout/systems.rs:2959-2993`)
writes `position = overrides.by_entity.get(&entity) … .unwrap_or_else(|| Vec2::new(layout.location.x, layout.location.y))`
(systems.rs:2972-2976). `layout.location` is Taffy's **rel-to-parent** offset;
the only substitution is `PostTaffyPositionOverrides` (sticky/table/multicol/
anchor — *not* a general accumulation pass). So `ResolvedLayout.position` is and
stays parent-local. The doc comment at `crates/buiy_core/src/components.rs:65`
calls it `"window-relative"` — the **lie** the audit (§1 MISSED #2, §6 #2)
names as the structural enabler of every relative-as-absolute consumer.

**The render/extract path already reads the right thing.** The transform bridge
(`crates/buiy_core/src/render/bridge.rs`) is the sole writer of each node's Bevy
`Transform.translation`: it folds `position − acc` (rel position minus
accumulated ancestor scroll) into a `Transform`, and Bevy propagation finalizes
`GlobalTransform` (bridge.rs:58-172). Render extract reads that absolute value —
`render/mod.rs:435` `let position = global.translation().truncate();` (pillar 5,
documented mod.rs:350-351, 378-387); `ExtractedNode.position` is explicitly
"`GlobalTransform.translation.xy` … not `ResolvedLayout.position`"
(`render/extract.rs:69-72`); text extract, the editor caret, and IME geometry
likewise already use `GlobalTransform` (`render/extract.rs:351`,
`text/extract.rs:181`, `text/edit/pointer.rs:72`, `text/edit/ime.rs:559`). So the
absolute basis is established and battle-tested for everything **except** the two
consumers below.

**The two live offenders (the complete set on `main`, grep-verified):**

1. **Picking — fires.** `point_in_aabb` (`crates/buiy_core/src/picking/mod.rs:51-57`)
   AABB-tests `layout.position .. layout.position + layout.size`. Both the free
   `hit_test` (mod.rs:37-49) and the backend `emit_picks`
   (`crates/buiy_core/src/picking/backend.rs:41-46`) call it. On the first
   centered/offset widget the pick rect is wrong by the ancestor offset. This is
   the bug the prototype hit on its first card.

2. **Clip producer — latent (same class).** `write_clip_rects`'s `walk`
   (`render/clip.rs:284-298`) builds `let own = Aabb::from_box(rl.position, rl.size)`
   (clip.rs:286) and intersects it with the running `ancestor: Option<Aabb>`
   AABB that has been folded **down the tree in absolute space**. Because
   `rl.position` is parent-local and `ancestor` is absolute, the intersection is
   geometrically meaningless once **any** clipper is offset from the window
   origin. It is *latent* (umbrella §9, audit App.B #2): a `ClipRect` is emitted
   only when an ancestor actually clips, and **every** existing clip test keeps
   its outer clipper at the window origin — `nested_overflow_hidden_intersects_to_tighter_box`
   nests two clippers but both at origin (`tests/render_clip_rects.rs:138-177`).
   No test offsets a clipper, so the bug has never fired. The gallery's scroll
   areas, menus, and modals (C5/C8) are exactly offset clippers → it goes live.

**The verifier is structurally blind (umbrella §9.5 carried risk).** Both picking
tests hand-write `ResolvedLayout` with **no** `GlobalTransform` and **no** bridge:
`tests/picking_backend.rs:36-45` and `tests/picking.rs:15-24` spawn a node at
`(10,10)` and assert a hit at `(50,30)`. Because they never run layout→bridge,
the hand-set `.position` *is* the absolute position by coincidence — the test
passes whether picking reads `.position` or `GlobalTransform`. It cannot observe
Bug 1. C7's Tier-A harness (the real layout→bridge→`GlobalTransform` path) is the
gate that can; this child lands RED against it.

**The fallback to delete.** The prototype kept `Option<&GlobalTransform>` +
`unwrap_or(layout.position)` to keep bridge-less unit tests green (audit §2 Bug-1
counterpoint; App.B #5). It silently mixes two coordinate spaces and masks the
"GlobalTransform should always be present" invariant. Render hard-requires
`&GlobalTransform` non-optionally (`render/mod.rs:421`, `render/extract.rs:351`);
picking and clip must match it. There is no such fallback on `main` yet (the
prototype is unmerged) — the task is to **not introduce one** when wiring the
non-optional query, and to fix the tests instead (C7).

---

## 2. Target design

One rule, stated once and obeyed by every absolute consumer:

> **Absolute window-logical position = `GlobalTransform.translation().truncate()`.**
> `ResolvedLayout.position` is parent-local and is read only by the bridge (the
> sole accumulator) and by layout-internal passes. No consumer outside the bridge
> reads `ResolvedLayout.position` for an absolute coordinate.

### 2.1 Picking — `point_in_aabb` / `hit_test` / `emit_picks` read `GlobalTransform`

Replace the `ResolvedLayout.position`-based AABB with a `GlobalTransform`-based
one. The hit-test geometry becomes a pure function of an absolute top-left + a
size:

```rust
// picking/mod.rs — coordinate basis only (C3 owns depth/camera/no-hit/Pickable)
pub(crate) fn point_in_aabb(point: Vec2, abs_pos: Vec2, size: Vec2) -> bool {
    let max = abs_pos + size;
    point.x >= abs_pos.x && point.x <= max.x
        && point.y >= abs_pos.y && point.y <= max.y
}
```

- `hit_test` (the `&World` free fn) queries `(Entity, &ResolvedLayout, &GlobalTransform)`
  **non-optionally** and calls `point_in_aabb(point, gt.translation().truncate(), rl.size)`.
  A node without a `GlobalTransform` (never went through the bridge) is simply
  absent from the query — the same drop render already accepts (render/mod.rs
  doc 383-387). No fallback.
- `emit_picks` (`backend.rs`) widens its `nodes` query from `(Entity, &ResolvedLayout)`
  to `(Entity, &ResolvedLayout, &GlobalTransform)` and hit-tests in absolute
  space the same way.

**Boundary with C3 (umbrella §6.1, §6.2, §9 carried risk #1).** This child changes
*only the coordinate the AABB is computed in*. It does **not** touch: the
smallest-area depth tiebreak (`backend.rs:50-72`), `Entity::PLACEHOLDER` camera
(backend.rs:65), `order = 0.0` (backend.rs:74), no-hit emission, `painters_z`
pick-depth, or `Pickable`. Those are C3's `emit_picks` rewrite. C1 lands first
(Wave 1) so that when C3 rewrites depth/event flow (Wave 2) the geometry it reads
is already correct. Mechanically: C1 edits the `point_in_aabb` call site + the
query tuple; C3 rewrites the body around it.

### 2.2 Clip producer — `walk` reads `GlobalTransform`

`Aabb::from_box` already takes a `position` argument (clip.rs:37-42); the fix is
to **pass the absolute position** instead of `rl.position`. The `walk` gains a
per-node `GlobalTransform` read; `ClipNodeData` carries it as a **required** term
(non-optional like render), keeping `ResolvedLayout` optional for the existing
"clear stale clip on a node without a resolved box" arm (clip.rs:294-297):

```rust
// render/clip.rs — walk arm (paraphrase)
match (rl, gt) {
    (Some(rl), Some(gt)) => {
        let abs = gt.translation().truncate();          // C1: absolute basis
        let own = Aabb::from_box(abs, rl.size);          // was Aabb::from_box(rl.position, rl.size)
        let clip = ancestor.map(|a| a.intersect(own));
        reconcile(entity, clip, ancestor, commands, existing);
        let contribution = clip_contribution(own, box_model, overflow, containment);
        intersect_opt(ancestor, contribution)
    }
    _ => { reconcile(entity, None, None, commands, existing); ancestor }
}
```

The emitted `ClipRect` / `AncestorClip` stay **window-relative** (their documented
contract, clip-and-transform.md §A.2 lines 74/110) — now *correctly* so, because
the own box is finally in window space, matching the absolute `ancestor` AABB it
intersects with and the absolute `GlobalTransform` the render consumer
(`clip_for_primitive` → `scissor_rect`) pairs it against. `scissor_rect` is
unchanged (it already maps a window-relative `ClipRect` to a physical scissor,
clip.rs:112-129).

**Scheduling — the load-bearing new constraint (own contract §4.2).** Today
`write_clip_rects` reads `ResolvedLayout` and is scheduled
`Update, .after(BuiySet::Animate).before(BuiySet::Picking)` (render/mod.rs:119-124)
— the **same** window as the bridge's transform-composition + propagation chain
(`seed_scroll_dirty`, `write_buiy_transform`, `mark_dirty_trees`,
`propagate_parent_transforms`, `sync_simple_transforms`, chained
`.after(Animate).before(Picking)`, lib.rs:108-129). There is **no** ordering edge
between them today, which is harmless while clip reads `ResolvedLayout`. Once clip
reads `GlobalTransform`, it **must** run *after* the propagation chain or it sees
a stale/absent transform. The fix adds `write_clip_rects.after(sync_simple_transforms)`
(the last propagation system), keeping `.before(BuiySet::Picking)`. Picking
already runs after the chain (the chain is `.before(BuiySet::Picking)` and picking
is in `BuiySet::Picking`), so picking needs no new edge — but C1 documents that it,
like clip, now depends on the chain having run.

### 2.3 Outline ink-bounds & overlays — already absolute; pinned, not changed

The umbrella names outline ink-bounds and overlays as C1-routed absolute consumers.
On `main` both already read `GlobalTransform`:

- **Outline ink-bounds / effect-group bounds** (`render/extract.rs:586-587, 654-655`)
  expand around `n.position`, where `n` is `ExtractedNode` whose `position` is the
  `GlobalTransform`-derived value (extract.rs:69-72, query at 351). Already absolute.
- **Overlays / top-layer**: top-layer membership forces `clip = None` (the full-view
  sentinel) at extract (extract.rs:362-366) and does not read `ResolvedLayout.position`.
  Overlay **positioning** (popover anchor math) is **C5's** to build and will, by this
  child's rule, consume `GlobalTransform` — C1 establishes the rule; it does not pre-build
  C5's anchors.

C1's deliverable for these is the **doc rule** (§2 top) + the `components.rs:65`
fix that makes the rule honest, so the next consumer (C5 overlays, future devtools)
cannot re-fall into the trap. No code change to extract.

### 2.4 The doc-comment fix (audit §1 MISSED #2, supersede #4)

`components.rs:65` becomes:

```rust
/// Top-left position in logical pixels, **parent-relative** (Taffy's
/// per-node `location`; only `PostTaffyPositionOverrides` substitutes it —
/// sticky/table/multicol/anchor — never a general accumulation). This is NOT
/// an absolute coordinate: the transform bridge (`render/bridge.rs`) is the
/// sole accumulator (`position − ancestor_scroll` → `Transform` →
/// `GlobalTransform`). Absolute consumers (picking, clip, render extract,
/// overlays) MUST read `GlobalTransform.translation().truncate()`, never this
/// field. See docs/specs/2026-06-22-buiy-widget-catalog-design/coordinate-space-correctness.md.
pub position: Vec2,
```

### 2.5 Preserve the `bridge.rs:138` invariant (decision §2.5; do NOT touch)

`bridge.rs:138` `let base = Mat4::from_translation((resolved.position - acc).extend(0.0));`
**requires** `resolved.position` to be parent-local: `acc` is the accumulated
ancestor `ScrollOffset` summed down the walk (bridge.rs:135-162), and
`position − acc` yields the per-node translation Bevy propagation then folds into
the absolute `GlobalTransform`. If `ResolvedLayout.position` were made absolute
(rejected alt-b), this subtraction **double-corrects** and breaks the
scroll→`GlobalTransform` fold (the proof that picked alt-a over alt-b, umbrella
§2.5, audit §2 Bug-1 alt (a) refutation). C1 reads `GlobalTransform` *downstream*
of this composition; it never alters the field the composition consumes. This is
umbrella §6.2's "`bridge.rs:138` is an invariant to PRESERVE, not fix."

---

## 3. Decisions & rejected alternatives

**D1 — Layout-local `ResolvedLayout` + non-optional `GlobalTransform` consumers
(realizes umbrella §2.5 / audit Tier-0 #1).** Decided at the campaign level; C1
records the implementation consequence. *Runner-up — alt-b "make `.position`
absolute via accumulation":* one honest field for all consumers, and the
`world_position` helper (systems.rs:413-441) already accumulates for sticky — but
(i) it **breaks `bridge.rs:138`** (the decisive evidence), and (ii) large blast
radius (every layout golden re-blessed; two absolute sources risk re-diverging).
Rejected on the bridge evidence — already settled by §2.5; not re-litigated here.

**D2 — Non-optional `GlobalTransform`, no fallback (resolves audit App.B #5).**
Query `&GlobalTransform` as a required term in `hit_test`, `emit_picks`, and the
clip `walk`. A node that never went through the bridge is dropped (absent from the
query), matching render exactly. *Runner-up — keep `Option<&GlobalTransform>` +
`unwrap_or(layout.position)`:* keeps bridge-less unit tests green with zero test
churn, but it is precisely the foot-gun that "silently mixes two coordinate
spaces" (audit §2 counterpoint) and re-arms the trap the doc fix is meant to
disarm. Rejected; the test churn it avoids is paid once by C7's Tier-A migration,
which we want anyway. **`ResolvedLayout` stays optional in the clip `walk`** (only
to keep the existing "clear stale clip on a box-less node" arm, clip.rs:294-297) —
the *position read* is gated on `GlobalTransform` being present, not on `ResolvedLayout`.

**D3 — Clip runs after the propagation chain (resolves the new ordering question).**
Add `write_clip_rects.after(sync_simple_transforms)`. *Runner-up — a shared
`BuiySet::RenderPrep`-style set ordering clip after the bridge:* cleaner long-term
and is hinted at by render/mod.rs comments, but introducing a new set + migrating
the three sibling prep passes (`write_effect_groups`, `write_paint_skip`) is scope
beyond C1's coordinate fix and risks reordering passes that currently rely only on
`.after(Animate).before(Picking)`. C1 adds the **minimal** explicit edge
(`.after(sync_simple_transforms)`) and leaves the set-refactor as a noted follow-up.
The edge is correct and sufficient; the sibling prep passes still read
`ResolvedLayout`/markers (not `GlobalTransform`) so they need no new edge.

**D4 — One-frame-stale `GlobalTransform` is acceptable + documented (resolves audit
§2 Bug-1 counterpoint).** `emit_picks` runs in `PreUpdate` (`PickingSystems::Backend`,
backend.rs:23) while the bridge propagates in `Update` — so `emit_picks` reads the
**previous** frame's `GlobalTransform`. This matches the documented `Hovered` lag
and the editor's own one-frame posture (`text/edit/pointer.rs` doc). `hit_test`
(the `&World` free fn, used by tests/library) reads whatever is current when called.
`write_clip_rects` runs in the same `Update` as the bridge and (with D3) **after**
it — so clip is **not** one-frame-stale; only the `PreUpdate` backend is. C1
documents the asymmetry; it does not try to eliminate the backend lag (that is C3's
scheduling call, umbrella §10). *Runner-up — move `emit_picks` to `Update` after the
chain:* removes the lag but is an `emit_picks` scheduling change, which is C3's
territory; deferred to C3.

**D5 — Scope the fix to the two live offenders; pin (not edit) the already-absolute
consumers.** Verified by grep (§1): only `picking/mod.rs:51-57` and `clip.rs:286`
read `ResolvedLayout.position` as absolute among non-test, non-bridge code. Outline
ink-bounds and overlays already read `GlobalTransform`. *Runner-up — proactively
re-audit/rewrite every `.position` read:* unnecessary churn; the doc rule (§2.4) +
C7's invariant is the durable guard against future offenders, not a blanket sweep.

---

## 4. Contracts & interfaces

### 4.1 Shared contracts referenced (umbrella §6 — referenced, not redefined)

- **§6.1 Pick-depth from `painters_z`** — owned by **C3**. C1 does **not** touch
  depth; it leaves `emit_picks`'s smallest-area tiebreak and `HitData` untouched,
  changing only the AABB coordinate.
- **§6.2 Coordinate space (this child) gates picking (C3), clip (C6 outline + the
  `clip.rs:286` bug), and overlays (C5).** C1 strictly precedes all three.
  **`bridge.rs:138` is an invariant to PRESERVE** (see §2.5).
- **§6.4 Focus / §6.7 R1/R2 byte-stability** — untouched by C1 (C1 changes no
  `PackedInstance` field, no paint; `ClipRect`/`AncestorClip` *values* become
  correct but their byte layout is unchanged).

### 4.2 Own contract — the absolute-coordinate rule (C1 defines)

1. **Absolute basis = `GlobalTransform.translation().truncate()`.** Every consumer
   needing a node's window position reads it from `GlobalTransform`, non-optionally.
   `ResolvedLayout.position` is parent-local and read only by the bridge + layout
   internals. (Codified in the `components.rs:65` doc, §2.4.)
2. **Clip-after-bridge ordering.** Any render-prep pass that reads `GlobalTransform`
   (now: `write_clip_rects`) must be scheduled `.after(sync_simple_transforms)`
   (the last propagation system, lib.rs:124) and `.before(BuiySet::Picking)`.
3. **Producer outputs stay window-relative.** `ClipRect`/`AncestorClip` remain in
   logical-px y-down window space (their existing contract); C1 only makes the
   producer compute them in that space correctly. `point_in_aabb`'s new signature
   `(point, abs_pos, size)` is the C1 seam C3 builds its `emit_picks` rewrite on.

### 4.3 Supersedes (record, don't silently contradict)

- **`render-pipeline-design/clip-and-transform.md §A.2`** describes the producer
  reading `Rect::from(resolved.position, resolved.size)` (spec line ~152). C1
  corrects the producer to read `GlobalTransform`-derived position; the spec §A.2
  prose must gain a note (flipped when C1 lands) that the own box is computed from
  `GlobalTransform.translation().truncate()`, not `ResolvedLayout.position`,
  consistent with pillar 5 §B.5 (which already says render/picking read
  `GlobalTransform`). Do not delete the §A.2 text; annotate the supersede.
- **`components.rs:65` "window-relative" doc lie** — corrected by C1 (umbrella
  supersede #4).

---

## 5. Migration / build steps (ordered; blast radius noted)

Each step's first action at implementation time is the umbrella §8 rebase +
re-confirm of these file:line anchors against the then-current `origin/main`.

1. **Doc-comment fix.** `components.rs:65` → the parent-local text (§2.4).
   *Blast radius:* doc only; no behavior. Lands first so the rule is written down
   before the code obeys it.

2. **Picking coordinate basis.** `picking/mod.rs`: change `point_in_aabb` to
   `(point, abs_pos, size)`; `hit_test` queries `(Entity, &ResolvedLayout, &GlobalTransform)`
   and passes `gt.translation().truncate()`. `picking/backend.rs`: widen
   `emit_picks`'s `nodes` query to include `&GlobalTransform` and hit-test in
   absolute space — **touching only the AABB call**, leaving depth/camera/order/
   no-hit for C3. *Blast radius:* `picking/mod.rs`, `picking/backend.rs`;
   `tests/picking.rs` + `tests/picking_backend.rs` must move to the bridge path
   (they currently spawn bare `ResolvedLayout` with no `GlobalTransform` and would
   now drop out of the query) — **co-delivered with C7 Tier-A**, which provides the
   layout→bridge→`GlobalTransform` harness those tests migrate onto.

3. **Clip coordinate basis + ordering.** `render/clip.rs`: `ClipNodeData` gains
   `Option<&GlobalTransform>`; the `walk` match reads the absolute position for
   `Aabb::from_box` (§2.2). `render/mod.rs:119-124`: add
   `.after(crate::render::bridge::sync_simple_transforms)` (re-export the symbol if
   not already reachable) to `write_clip_rects`. *Blast radius:* `render/clip.rs`,
   `render/mod.rs` (one `.after`); the existing `tests/render_clip_rects.rs` suite —
   **every value assertion there stays GREEN** because all its clippers are at the
   window origin (so `GlobalTransform`-derived == `ResolvedLayout.position` there);
   the suite needs the app to build `GlobalTransform` (it already adds
   `LayoutPlugin` + `BuiyRenderPlugin` + `CorePlugin`, so the bridge runs — verify
   the propagation chain populates `GlobalTransform` under `MinimalPlugins`; if a
   node lacks it, add the bridge to the harness rather than reintroducing a fallback).

4. **Add the nested + OFFSET overflow-clip test (RED-first, the C1-specific gate).**
   New test in `tests/render_clip_rects.rs`: an outer `overflow:hidden` clipper
   positioned **away from the origin** (e.g. via a positioned ancestor or padding/
   margin chain that gives it a non-zero `GlobalTransform`), a child overflowing it.
   Assert the child's `ClipRect`/`AncestorClip` is the clipper's **absolute**
   padding box, not the origin-anchored box the old code produced. Prove it **RED**
   against pre-fix `clip.rs` (it must fail with the relative-position bug) then GREEN
   after step 3. This is the test class the audit (§1 MISSED #1, App.B #2) says is
   missing — none today offsets a clipper.

5. **Pin the already-absolute consumers (doc only).** Confirm via grep that no new
   `ResolvedLayout.position`-as-absolute read crept in (extract ink-bounds, overlays
   stay `GlobalTransform`-sourced). Add the §A.2 supersede note to
   `clip-and-transform.md`. *Blast radius:* docs.

**Net code blast radius:** 4 source files (`components.rs`, `picking/mod.rs`,
`picking/backend.rs`, `render/clip.rs`) + 1 scheduling line (`render/mod.rs`).
**Tests:** 2 picking tests re-homed onto C7's harness; 1 new offset-clip test;
the existing clip suite stays green. **Snapshots/goldens:** **none** — C1 changes
no `ResolvedLayout` value (so layout snapshots are byte-stable), no paint, and no
`PackedInstance` byte layout (umbrella §6.7 untouched). Picking does not write
`ResolvedLayout`.

---

## 6. Verification (how C7 gates this; what must be RED-first)

C1 is the reason C7's **Tier A** exists in Wave 1 (umbrella §5, §4 C7). The gate:

- **Tier A — real-input picking on an OFFSET widget (the Bug-1 regression gate).**
  C7's `PointerHarness` builds a real non-origin widget tree, runs
  layout→bridge→`GlobalTransform` + `InteractionPlugin` + the Buiy backend, injects
  a synthetic `PointerId`/`PointerInput`, and asserts the hit lands on the offset
  widget. **Proof of teeth (umbrella §9.5):** with C1 reverted, Tier A must go **RED**
  on an offset widget (the pick rect is wrong by the ancestor offset). The existing
  `tests/picking_backend.rs` / `tests/picking.rs` cannot serve as this gate — they
  hand-write `ResolvedLayout` with no bridge, so `.position` == absolute by
  coincidence and they pass either way (§1). C1's step 2 migrates them onto Tier A's
  harness; the new offset assertion is what gives the gate teeth.

- **Clip offset test (step 4 above) — RED-first.** The nested + offset
  overflow-clip test in `tests/render_clip_rects.rs` must fail against pre-fix
  `clip.rs` (the latent bug, never previously triggered) and pass after. This is
  the lowest-tier observer for the clip half of Bug 1 (a display-list/clip-geometry
  assertion, not a golden — per the verification skill's "lowest tier that can
  observe the bug": layout/clip geometry, not rasterization).

- **Ordering regression.** A headless test asserting clip reads the **post-bridge**
  `GlobalTransform`: place a scroll-offset / positioned ancestor, advance one frame,
  and assert the child's `ClipRect` reflects the absolute (transform-folded)
  position — proving the `.after(sync_simple_transforms)` edge (D3) holds and clip
  is not one-frame-stale. If clip ran before the chain, the `ClipRect` would lag.

- **No-fallback assertion.** A test that a `Node` carrying `ResolvedLayout` but **no**
  `GlobalTransform` (never bridged) is **absent** from picking/clip results (dropped,
  not silently placed at `ResolvedLayout.position`) — pins D2 so a future change
  cannot quietly reintroduce the fallback.

- **Snapshot stability check.** Run the existing layout + clip suites unchanged to
  confirm zero golden re-bless (the §5 "no snapshots touched" claim) — any diff means
  a value moved and the layout-local invariant was violated.

All four predicates land **RED-before-GREEN** (umbrella §5, §9.5). C7 owns the
harness; C1 supplies the offset-widget fixture + the offset-clip test and the
revert-to-RED demonstration.

---

## 7. Open questions deferred + dependencies

**Dependencies (incoming):** only **C0** (the umbrella decisions). C1 is Wave-1,
lowest blast radius, independent of C2.

**Dependents (C1 strictly precedes; umbrella §6.2):**
- **C3** (input model) — `emit_picks`'s depth/camera/no-hit/`Pickable` rewrite
  reads the absolute AABB C1 establishes. C3 must not land its `emit_picks` rewrite
  before C1's coordinate basis (umbrella §5 Wave 2 gate).
- **C5** (overlays/scroll) — popover/anchor positioning consumes the §4.2 rule;
  scroll areas are the first **offset clippers** that exercise the clip fix.
- **C6** (styling) — the `Outline`/focus-ring ink-bounds and the `clip.rs:286`
  bug are both downstream of C1 (umbrella §6.2); C6's outline survives
  `overflow:hidden` via the now-correct `AncestorClip`.

**Deferred (genuinely depends on un-built work):**
- **`emit_picks` PreUpdate one-frame lag** — whether to keep the backend in
  `PreUpdate` (accepting D4's documented one-frame stale `GlobalTransform`) or move
  it to `Update` after the chain is a **scheduling decision C3 owns** (it rewrites
  `emit_picks`); C1 documents the lag and leaves the schedule to C3. Deferred to C3
  because it is meaningless until C3's no-hit/depth rewrite settles the backend's
  shape (umbrella §10).
- **`BuiySet::RenderPrep` set refactor** (D3 runner-up) — grouping clip/effect/
  paint-skip/bridge into one ordered set is a clean-up that touches passes outside
  C1's scope; recorded as a follow-up, not done here. Deferred because it is not
  required for correctness and would expand C1's blast radius.
- **Overlay anchor coordinate math** — C5 builds it; C1 only pins the rule it must
  follow. Deferred to C5 because the anchor algorithm does not exist on `main`.
