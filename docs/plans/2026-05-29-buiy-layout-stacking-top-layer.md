# Buiy layout — Phase 9: stacking + top layer

**Date:** 2026-05-29
**Status:** landed
**Spec:** [`specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md`](../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md) § 1 (`Stacking` + `ZIndex`/`Isolation`/`TopLayer`), § 2 (stacking-context formation + `StackingContext.painters_z`), § 3 (`z_index`), § 4 (top layer + `TopLayerActivation`), § 6 (test surface), § 7 (v1 implementation status) + [`architecture.md`](../specs/2026-05-08-buiy-layout-design/architecture.md) § 3 (sub-pass 6f), § 6 (error model); reads the Phase-8 [`transforms-and-containment.md`](../specs/2026-05-08-buiy-layout-design/transforms-and-containment.md) § 3 `ResolvedTransform` artifact.
**Supersedes:** none (graduates the unbuilt `stacking-and-top-layer` spec child; sub-pass 6f extends the Phase-8 `PostTaffyOverrides` chain — `clear → sticky 6a → table 6b → multicol 6c → anchor 6d → transform 6e` — by appending `stacking_context` 6f after `transform_composition`).

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development`. Each task lists exact file paths and TDD steps; steps use checkbox (`- [ ]`) tracking. Run the project gate (below) before every commit and resolve every warning.

**Goal:** Land the stacking layer — the `Stacking` self-styling component (`ZIndex` / `Isolation` / `TopLayer`), stacking-context detection (the union trigger list, restricted to the triggers buildable in `buiy_core` today), the `StackingContext { painters_z }` private render handoff (mirroring how `ResolvedTransform` is the render handoff for the matrix), CSS-faithful `z_index` sibling ordering, and a **single global** top layer with `TopLayerActivation` activation-order tracking and parent-stacking / overflow-clip escape. A new `PostTaffyOverrides` sub-pass **6f** (`stacking_context`) runs **after** `transform_composition` (6e) — it reads the composed `ResolvedTransform` to detect transform-formed stacking contexts (spec [§ 2.1](../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md#21-stackingcontext-private-component)).

**Architecture (3 sentences):**
1. **Stacking-context detection + paint-order resolution as a post-Taffy overlay that writes a private render handoff.** Sub-pass 6f `stacking_context` walks the `Node` hierarchy, decides per entity whether it forms a stacking context (the spec § 2 union of triggers 1 positioned+`z_index`, 2 `Isolation::Isolate`, 3 non-identity transform via `Has<ResolvedTransform>`, 4 `Containment.contain ⊇ PAINT/STRICT`, 6 root), and for each SC-forming entity computes the spec § 2.1 five-tier paint order of its descendants (descending through non-SC children, treating nested SCs as atomic entries) into a private `StackingContext { painters_z: Vec<Entity> }` component. Render later walks `painters_z`; layout decides depth ordering, render decides how to paint it (spec boundary). It writes **nothing** to `PostTaffyPositionOverrides` (stacking does not move the layout box) — it owns `StackingContext`, exactly as 6e owns `ResolvedTransform`.
2. **A single global top layer (deliberate, documented divergence from spec § 4.4 at the implementation-scope level).** Spec § 4.4 wants a per-window top layer, but `buiy_core` has one global `LayoutTree` and uses the primary window only (no per-window layout) — so Phase 9 ships **one** top layer. Top-layer entities (`TopLayer != None`) are removed from their parent's `painters_z` and attached to the **root** context's paint order (membership = root, not parent — the spec § 6 escape test), ordered by tier (Fullscreen < Tooltip < Popover < Modal) then by activation order, tracked in a `TopLayerActivation { order: VecDeque<Entity> }` resource. Per-window scope is a follow-up gated on `buiy-window-and-surface-design` (D2).
3. **Detection triggers are restricted to what exists in `buiy_core` today.** Triggers 1–4 + 6 are realized. Trigger 5's render-side formers (`opacity`/`filter`/`mix_blend_mode`) and the `will-change` SC former are **deferred** — the components carrying those properties do not exist in `buiy_core` yet (will-change is Phase-8 tier-E, stored-only), so 6f cannot read them. The predicate is written so adding them later is a localized extension (D5). The activation maintenance + detection live in one system so there is a single ordered pass over the tree.

**Tech Stack:** Bevy 0.18 (`bevy::prelude::{Children, ChildOf, Node, Query, Commands, ResMut, NonSend, With, Has}`, `bevy::ecs::entity::Entity`). `std::collections::{VecDeque, HashSet, HashMap}` (no `bevy::utils::*`, per Phase 6/7/8 precedent). No new external dependency. Reads the Phase-8 `ResolvedTransform` (`crate::components`) for trigger 3.

---

## Prior-art citations (used throughout this plan)

- **Pipeline sub-pass chain** — `crates/buiy_core/src/layout/mod.rs:205-214`: a `.chain().in_set(BuiyLayoutStep::PostTaffyOverrides)` tuple `(clear_post_taffy_overrides, sticky_offset, table_layout, multicol_pack, anchor_resolution, transform_composition)`. Phase 9 appends `stacking_context` as the **7th** element (sub-pass 6f), after `transform_composition` (6e). No new `BuiyLayoutStep` variant — 6f lives in the existing `PostTaffyOverrides` set (`crates/buiy_core/src/layout/pipeline.rs:16-44`). The comment at `mod.rs:203-204` already reserves this slot: "Phase 9 will append stacking 6f after 6e (it reads the composed matrix)."
- **Render handoff component pattern** — `crates/buiy_core/src/components.rs:47-61` (`ResolvedTransform { pub matrix: Mat4 }`, `#[derive(Component, Reflect, Clone, Debug, PartialEq)] #[reflect(Component)]` + hand-written identity `Default`, `pub` at crate level, re-exported `lib.rs:17`, written only by sub-pass 6e). `StackingContext` mirrors this exactly: a `pub` crate-level private-by-convention component in `components.rs`, written only by 6f.
- **`ResolvedTransform` is the trigger-3 signal** — written by `transform_composition` (`systems.rs:2414-2446`) only for non-identity transforms (identity → component removed). So `Has<ResolvedTransform>` is exactly "this entity has a non-identity transform" = spec § 2 trigger 3. No need to re-inspect `UiTransform`/`Translate`/`Rotate`/`Scale`.
- **Root detection** — `crates/buiy_core/src/layout/systems.rs:1682-1685` (`taffy_compute`): a root is a `Node` entity with `parent.map(|p| !tree.by_entity().contains_key(&p.parent())).unwrap_or(true)` — no `ChildOf`, or a `ChildOf` whose parent is not in `LayoutTree`. 6f reuses this exact predicate (spec § 2 trigger 6: "the root entity always forms one"). `ChildOf`'s accessor is `.parent()` in Bevy 0.18 (`systems.rs:270`, `:308`).
- **Document order = `Children` order** — `crates/buiy_core/src/layout/systems.rs:1635` (`sync_children_for_entity(entity, children: Option<&Children>, …)` feeds Taffy children in `Children` iteration order). The spec § 2.1 tiers "(document order)" map to `Children` iteration order. `Children` derefs to `&[Entity]`.
- **`LayoutTree::by_entity` accessor** — `crates/buiy_core/src/layout/tree.rs:31` (`pub fn by_entity(&self) -> &std::collections::HashMap<Entity, taffy::NodeId>`). Used for the root-detection "parent in tree?" check. (`LayoutTree` is `NonSend`.)
- **`PostTaffyPositionOverrides` shape + role** — `crates/buiy_core/src/layout/systems.rs:176-179`. Phase 9's `stacking_context` does **NOT** write this map (stacking does not move the layout box) — it writes `StackingContext`. Same structural shape as 6e.
- **Per-session warn-once dedup** — `crates/buiy_core/src/layout/systems.rs:203-206` (`pub struct LayoutWarnedOnceSession { pub set: HashSet<LayoutWarnOnceKey> }`); `LayoutWarnOnceKey` enum at `crates/buiy_core/src/layout/types.rs:975-1023` (currently `TableUnsupported(Entity)`, `MulticolUnsupported`, `StickyFrUnsupported(Entity)`, `StickyCqDeferred(Entity)`, `SizeContainmentZeroed(Entity)`, `ContentVisibilityDeferred(Entity)`). Dedup idiom: `if warned.set.insert(LayoutWarnOnceKey::X) { warn!(…) }` (example: `table_layout`, `systems.rs:611-626`). Phase 9 adds `MultipleFullscreenTopLayer` (D6). The enum is already `register_type`'d (`mod.rs:159`); `Reflect` picks up the new variant for free.
- **Resource init + observer/registry precedent** — `crates/buiy_core/src/layout/mod.rs:52-72` (`app.init_resource::<…>()` block). `TopLayerActivation` is `init_resource`'d here. Phase 9 maintains it inside 6f via a current-membership rebuild (D3), NOT an observer — unlike `AnchorNameRegistry` (`mod.rs:69`, observer-maintained), because `Stacking` is a `Style` bundle field (always re-inserted on spawn / replaced on any `Style` change), which makes `Insert`/`Replace` observer transitions noisy; a deterministic per-frame membership rebuild reads the actual current `top_layer` values.
- **`Style` bundle + fluent setters** — `crates/buiy_core/src/layout/style.rs:45` (`#[derive(Bundle, Clone, Debug, Default)] pub struct Style { … }`). The Phase-8 `ui_transform`/`containment` fields + setters (e.g. `style.rs` `pub fn containment(mut self, c: Containment) -> Self`) are the precedent Phase 9's `stacking` field + `.z_index()`/`.isolation()`/`.top_layer()` setters follow.
- **Component-registration chain** — `crates/buiy_core/src/layout/mod.rs:103-174` (one long `app.register_type::<T>()` chain, grouped by phase, terminating in the "Phase 8 — transforms + containment" group). Phase 9 appends a "Phase 9 — stacking + top layer" group + `crate::components::StackingContext`.
- **Facade re-exports** — `crates/buiy_core/src/layout/mod.rs:13-36` (`pub use components::{…}` + `pub use types::{…}` + `pub use systems::{…}`), `crates/buiy_core/src/lib.rs:17` (`ResolvedTransform` re-export), `crates/buiy/src/lib.rs` (top-level facade re-exports the same set). Phase 9 adds `Stacking`, `ZIndex`, `Isolation`, `TopLayer`, `TopLayerActivation`, and `StackingContext`.
- **Test harness** — `crates/buiy_core/tests/layout_transforms.rs:11-35`: `fn app() { let mut app = App::new(); app.add_plugins(MinimalPlugins); app.add_plugins(CorePlugin); app.add_plugins(LayoutPlugin); app }` (no `TransformPlugin`, no render); spawn `(Node, Style::default()…)` or add components directly; `app.update()` (one frame runs the whole pipeline); assert via `app.world().get::<StackingContext>(e)` / `app.world().resource::<TopLayerActivation>()`. Children spawned via `commands.spawn(...).add_child(c)` or `with_children`. Existing files: `tests/layout_transforms.rs`, `tests/layout_containment.rs`, `tests/layout_pipeline_order.rs`, `tests/layout_post_taffy_overrides_clear.rs`.

---

## File map (what each task touches)

| File | Touched by tasks |
|---|---|
| `docs/plans/2026-05-29-buiy-layout-stacking-top-layer.md` | T1 (this file) |
| `docs/specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md` | T1 (§ 7 status — already added during planning; T1 confirms) |
| `docs/README.md` | T1 (Phase 9 entry under "### Layout"), T13 (status tag) |
| `crates/buiy_core/src/layout/types.rs` | T2 (`ZIndex`, `Isolation`, `TopLayer`), T5 (`LayoutWarnOnceKey::MultipleFullscreenTopLayer`) |
| `crates/buiy_core/src/layout/components.rs` | T3 (`Stacking`) |
| `crates/buiy_core/src/components.rs` | T4 (`StackingContext` private render-handoff component) |
| `crates/buiy_core/src/layout/systems.rs` | T5 (`TopLayerActivation` resource + warn key), T6 (`forms_stacking_context` helper), T7 (`paint_tier` + `sort_painters` helpers), T8 (`stacking_context` system 6f), T9 (top-layer escape in 6f) |
| `crates/buiy_core/src/layout/style.rs` | T10 (`stacking: Stacking` field + setters) |
| `crates/buiy_core/src/layout/mod.rs` | T8 (wire `stacking_context` into the chain after `transform_composition`; `init_resource::<TopLayerActivation>`), T11 (`register_type` group + `pub use` re-exports) |
| `crates/buiy_core/src/lib.rs` | T11 (re-export `StackingContext`) |
| `crates/buiy/src/lib.rs` | T11 (re-export the public Phase-9 set from the top-level facade) |
| `crates/buiy_core/tests/layout_pipeline_order.rs` | T8 (assert 6f runs after 6e — a positioned+z_index entity gets a `StackingContext`) |
| `crates/buiy_core/tests/layout_stacking.rs` | T12 (new file — spec § 6 integration tests minus per-window) |
| `CHANGELOG.md` | T13 |
| `docs/plans/follow-ups.md` | T13 |

No changes to: `crates/buiy_core/src/render/mod.rs` (render consumes `StackingContext`/`TopLayerActivation` in a render-pipeline follow-up), `crates/buiy_core/src/layout/pipeline.rs` (6f lives in the existing `PostTaffyOverrides` set), `crates/buiy_core/src/layout/translate.rs`, `crates/buiy_core/src/layout/tree.rs`.

---

## Decision blocks (locked-in choices the implementer must honor)

### D1. Phase 9 scope = stacking + top-layer; triggers restricted to what exists today

**Decision:** Phase 9 ships (a) the `Stacking` component (`ZIndex`/`Isolation`/`TopLayer`), (b) stacking-context detection for triggers **1** (positioned + `ZIndex::Layer`), **2** (`Isolation::Isolate`), **3** (non-identity transform via `Has<ResolvedTransform>`), **4** (`Containment.contain ⊇ PAINT/STRICT`), **6** (root), (c) the `StackingContext { painters_z }` private render handoff with the spec § 2.1 five-tier paint-order sort, (d) CSS-faithful `z_index` sibling ordering, and (e) a single global top layer (escape + tier ordering + `TopLayerActivation`). It does **NOT** implement trigger **5**'s render-side formers (`opacity`/`filter`/`mix_blend_mode`) or the `will-change` SC former.

**Why:** Spec [§ 7](../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md#7-v1-implementation-status-phase-9-scope) records this seam. The render-side formers live on components that do not exist in `buiy_core` (render-pipeline spec is unbuilt); `will-change` is Phase-8 tier-E, stored-only with no behavior (Phase 8 D7). 6f cannot read properties that have no component. Restricting to triggers 1–4 + 6 keeps Phase 9 self-contained while producing the complete `StackingContext`/`TopLayerActivation` handoff render needs.

**How to apply:** Implement the spec § 1, § 2 (triggers 1–4, 6), § 2.1, § 3, § 4 surface. The `forms_stacking_context` predicate (T6) takes only the inputs available today; add a doc comment + follow-up noting where trigger-5 formers slot in.

**Runner-up rejected:** Stub the render-side trigger inputs (e.g. add placeholder `Opacity`/`Filter` components now). Rejected: that invents render-side API ahead of the render-pipeline spec — speculative scope the brainstorming explicitly excluded; the formers are added when their real components land.

### D2. Single global top layer (deliberate divergence from spec § 4.4 per-window scope)

**Decision:** Phase 9 ships **one** global top layer + one global `TopLayerActivation`. Top-layer entities attach to the **root** context's paint order (membership = root). Per-window top layers are **NOT** implemented.

**Why:** `buiy_core` has a single global `NonSend<LayoutTree>` and reads the primary window only (`taffy_compute` `windows.iter().next()`, `systems.rs:1676`); there is no per-window layout segregation anywhere (confirmed: the only `Window` usage in layout is viewport sizing). A per-window top layer would require per-window `LayoutTree`s or window-tagged entries — out of scope and gated on `buiy-window-and-surface-design` (unbuilt), exactly mirroring the Phase-6 cross-window-anchor deferral (`follow-ups.md` "Anchor positioning — cross-window targets").

**How to apply:** `TopLayerActivation` is a single resource. The spec § 6 "per-window top layer" test is **not** written in Phase 9 (T12 omits it; spec § 7 + a follow-up record why). The escape test ("membership = window root, not parent") IS implemented against the single root.

**Runner-up rejected:** Build per-window scaffolding now (a `HashMap<WindowEntity, TopLayer>`). Rejected: no per-window `LayoutTree` exists to key against; it would be dead scaffolding with no window-layout plumbing to drive it.

### D3. `TopLayerActivation` maintained by a per-frame membership rebuild inside 6f, not an observer

**Decision:** `TopLayerActivation { order: VecDeque<Entity> }` is updated inside `stacking_context` (6f), at the top of the system, by rebuilding from the current set of `TopLayer != None` entities: drop deque entries no longer in the current set (`retain`), then append newly-present entities in tree-iteration order. The result: activation order = order of first becoming top-layer, most-recent at the back; despawn / deactivation prune automatically; reactivation re-appends.

**Why:** Spec [§ 4.2](../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md#42-top-layer-ordering): "updated whenever `TopLayer` changes from `None` → non-`None`" and "the entity activated most recently paints on top." An observer on `Stacking` `Insert`/`Replace` is the `AnchorNameRegistry` precedent (`mod.rs:69`), but `Stacking` is a `Style` **bundle field** — re-inserted on every spawn and replaced on every `Style` mutation — so observer events do not cleanly map to `None → non-None` transitions and would need to diff previous state. The membership-set rebuild reads the actual current values, is O(top-layer count) per frame, deterministic, and colocated with the one pass that already iterates the tree (no extra sub-pass).

**How to apply:** Implement the rebuild as step 1 of `stacking_context` (T9 wires it; the resource is defined in T5). Preserve existing order via `VecDeque::retain` then `push_back` for entities not already present.

**Runner-up rejected:** Observer on `Stacking` insert/replace tracking a previous-value map. Rejected: `Style`-field churn makes the transition signal noisy; needs a side map of previous `top_layer`; more moving parts for no benefit over a cheap rebuild.

### D4. `Stacking` is a `Style` field (self-styling)

**Decision:** `Stacking` is added to the `Style` bundle as a field (`stacking: Stacking`) with fluent setters `.z_index(ZIndex)`, `.isolation(Isolation)`, `.top_layer(TopLayer)` (each sets one field of `self.stacking`).

**Why:** Spec [§ 1](../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md#1-stacking-component) defines `Stacking` as the entity's own depth-ordering property — self-styling, container-agnostic. Per [`architecture.md § 2.4`](../specs/2026-05-08-buiy-layout-design/architecture.md), self-styling properties live in `Style` (precedent: `UiTransform`/`Containment` in Phase 8). Unlike the rare additive `Translate`/`Rotate`/`Scale` longhands (decomposed-only), `Stacking` is a single small struct at its identity default on most entities and is the natural home for `.z_index()` / `.top_layer()` ergonomics.

**How to apply:** Add the field + three setters (T10). The fluent example in spec § 4.5 (`.top_layer(TopLayer::Modal)`) must compile.

**Runner-up rejected:** Decomposed-only `Stacking`. Rejected: `z_index`/`top_layer` are common enough that bundle-field ergonomics (`.z_index(…)`) are worth it, and it matches the Phase-8 self-styling precedent.

### D5. Stacking-context detection is a pure predicate; paint-order is a pure classifier + sort

**Decision:** Factor the two algorithmically-tricky pieces into **pure, unit-tested** helpers in `systems.rs`:
- `forms_stacking_context(stacking: Option<&Stacking>, position_kind: PositionKind, has_transform: bool, containment: Option<&Containment>, is_root: bool) -> bool` (T6) — the spec § 2 union for triggers 1–4 + 6.
- `paint_tier(stacking: Option<&Stacking>, position_kind: PositionKind) -> PaintTier` + `PaintTier` ordering + a `sort_painters` comparator (T7) — the spec § 2.1 five tiers.

The `stacking_context` system (T8/T9) reads queries, calls these helpers, walks the tree, and writes `StackingContext`. The helpers contain the CSS rules; the system contains the ECS plumbing.

**Why:** The CSS quirks (z_index only forms an SC when positioned; static z_index is ignored for ordering; the five-tier interleave of explicit/auto/negative/positive) are exactly the kind of logic that must be unit-tested in isolation — the Phase-8 `compose_transform` pure-helper precedent. Keeping the system thin keeps the tree walk readable.

**How to apply:** T6/T7 define + test the helpers with no `App`. T8 calls them.

**Runner-up rejected:** Inline all rules in the system. Rejected: the CSS quirks need focused unit tests that an `App`-level test obscures; matches Phase-8's `compose_transform` split.

### D6. New `LayoutWarnOnceKey` variant: `MultipleFullscreenTopLayer` (session-wide)

**Decision:** Add one variant `MultipleFullscreenTopLayer` (no `Entity` — session-wide, like `MulticolUnsupported`) to `LayoutWarnOnceKey`. Fires once per session when 6f observes more than one `TopLayer::Fullscreen` entity simultaneously.

**Why:** Spec [§ 4.2](../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md#42-top-layer-ordering): "**Fullscreen** … one entity wins; the rest fall back to their normal stacking." Multiple simultaneous fullscreen requests is a recognized-but-degenerate author condition worth a single diagnostic. Session-wide (not per-entity) because the condition is "the set has >1," not a property of one entity. No other Phase-9 condition warrants a warn (triggers 1–4 + escape are fully implemented; deferred triggers 5/will-change are silent — they are simply absent, not error states, consistent with Phase-8 D7's "valid hint, no warn").

**How to apply:** Append the variant with a doc comment citing spec § 4.2 + D6. Idiom: `if fullscreen_count > 1 && warned.set.insert(LayoutWarnOnceKey::MultipleFullscreenTopLayer) { warn!(…) }`.

**Runner-up rejected:** Per-entity `FullscreenConflict(Entity)`. Rejected: the conflict is a set property; per-entity keys would emit N warns for one degenerate state.

### D7. `StackingContext` is a `pub` crate-level private-by-convention render handoff in `components.rs`

**Decision:** `StackingContext { pub painters_z: Vec<Entity> }`, `#[derive(Component, Reflect, Clone, Default, Debug, PartialEq)] #[reflect(Component)]`, defined in `crates/buiy_core/src/components.rs` next to `ResolvedLayout`/`ResolvedTransform`. `pub` at the crate level (render + devtools need it) and reflectable, but author-set is not intended — it is written only by 6f (idempotent insert; removed when an entity no longer forms a context).

**Why:** Spec [§ 2.1](../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md#21-stackingcontext-private-component): "A private `StackingContext { painters_z: Vec<Entity>, .. }` … private (not author-set) but reflectable so devtools can inspect it." This is the same "private render handoff" role as `ResolvedTransform` (D2/T4 of Phase 8) — same home (`components.rs`), same derive shape, same lifecycle (written by a 6x sub-pass, removed when not applicable).

**How to apply:** T4 defines it. T8 writes it (idempotent: compare `painters_z` before insert; remove from entities that stop forming a context, mirroring 6e's stale-removal).

**Runner-up rejected:** Put it in `layout/components.rs` with the author-facing components. Rejected: it is a computed render handoff, not authoring surface; it belongs with `ResolvedLayout`/`ResolvedTransform`.

### D8. Top-layer entities escape into the root context; clip-escape is recorded as membership only

**Decision:** A `TopLayer != None` entity is **excluded** from its parent context's `painters_z` and appended to the **root** context's `painters_z` (after all in-flow root painters), ordered by tier (Fullscreen, Tooltip, Popover, Modal — bottom to top) then activation order. The layout-side fact Phase 9 produces is this membership + ordering. The actual clip-rect override (§ 4.3 "not clipped by ancestor `Overflow::Hidden`/`Clip`") is a **render** concern; Phase 9 records membership-at-root (which is what makes the escape testable: spec § 6 asserts "membership is the window root, not the parent"), and render reads `painters_z` + `Overflow` to apply the viewport clip.

**Why:** Spec [§ 4.1](../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md#41-toplayer-activation): "Setting `TopLayer != None` *removes* the entity from its parent's stacking context for paint purposes. Layout still treats it normally — its containing block, size, and position resolve as if it were in-flow." § 4.3's clip escape is "their effective clip rect is the window viewport" — a paint-time clip decision render owns (spec § 5: "render decides how to paint"). Phase 9's contract is the depth-ordering membership; § 6's escape test asserts exactly the membership fact, not a clip rect.

**How to apply:** In 6f, after building each context's in-flow `painters_z`, collect all top-layer entities, sort by `(tier, activation_index)`, and append to the root context. Skip top-layer entities when building any non-root context. T9 implements + tests this.

**Runner-up rejected:** Compute and store an explicit per-entity clip rect in layout. Rejected: clip rects are render state (the spec's layout/render boundary); duplicating viewport math in layout couples the two passes.

---

## Tasks

> **Per-task workflow (subagent-driven):**
> 1. Implementer subagent reads the task block.
> 2. Implementer follows TDD: failing test first, then minimal impl to pass, then refactor if needed, then commit.
> 3. Spec-compliance reviewer subagent reads the spec sections + the diff and asserts coverage.
> 4. Code-quality reviewer subagent reads the diff and asserts the code-quality bar.
> 5. Both reviews must be ✅ before moving to the next task.

> **Project gate (run before every commit, exactly — drop `xvfb-run -a` on this host, which has no xvfb; `MinimalPlugins` runs headless):**
> ```sh
> cargo fmt --all -- --check && \
>   cargo clippy --workspace --all-targets -- -D warnings && \
>   RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
>   cargo test --workspace
> ```

### Task 1: Plan doc lands + spec § 7 + docs/README.md entry

**Files:**
- Create: `docs/plans/2026-05-29-buiy-layout-stacking-top-layer.md` (this file)
- Modify: `docs/specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md` (§ 7 v1-status — added during planning; confirm present)
- Modify: `docs/README.md` (Phase 9 entry under "### Layout" → "**Plans**")

- [ ] **Step 1: This plan doc is drafted.** Confirm it covers (a) stacking + top-layer scope, (b) decision blocks D1–D8, (c) tasks T1–T13 with TDD steps, (d) prior-art citations + integration surface.
- [ ] **Step 2: Confirm spec § 7 present.** `stacking-and-top-layer.md` ends with "## 7. v1 implementation status (Phase 9 scope)". (Added during planning; if absent, add it per the spec edit.)
- [ ] **Step 3: Add docs/README.md entry.** Under "### Layout" → "**Plans**", after the Phase 8 line, append:
  ```markdown
  - [Buiy layout stacking + top layer](plans/2026-05-29-buiy-layout-stacking-top-layer.md) — Phase 9: `Stacking` (`ZIndex`/`Isolation`/`TopLayer`), `stacking_context` sub-pass 6f detecting stacking contexts (positioned+z-index, isolation, transform, paint/strict containment, root) and writing the private `StackingContext.painters_z` paint-order handoff, CSS `z_index` ordering, single global top layer + `TopLayerActivation`. Render-side triggers / will-change SC / per-window deferred (§ 7). `[active]`
  ```
- [ ] **Step 4: Commit.**
  ```bash
  git add docs/plans/2026-05-29-buiy-layout-stacking-top-layer.md docs/specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md docs/README.md
  git commit -m "docs(layout): Phase 9 plan — stacking + top layer"
  ```

### Task 2: Stacking value types in `types.rs`

**Spec:** § 1 (`ZIndex`, `Isolation`, `TopLayer`).

**Files:**
- Modify: `crates/buiy_core/src/layout/types.rs` (add 3 stacking value enums + tests)

- [ ] **Step 1: Failing test.** Add to `types.rs::mod tests`:
  ```rust
  #[test]
  fn z_index_default_is_auto() {
      assert_eq!(ZIndex::default(), ZIndex::Auto);
  }

  #[test]
  fn isolation_default_is_auto() {
      assert_eq!(Isolation::default(), Isolation::Auto);
  }

  #[test]
  fn top_layer_default_is_none() {
      assert_eq!(TopLayer::default(), TopLayer::None);
  }
  ```
  Run: `cargo test -p buiy_core z_index_default isolation_default top_layer_default` — expected FAIL (types don't exist).

- [ ] **Step 2: Add the types to `types.rs`.** After the Phase-8 transform/containment value types (near `LayoutWarnOnceKey`), insert — use the spec § 1 shapes EXACTLY:
  ```rust
  // ============================================================
  // Phase 9 — stacking value types (stacking-and-top-layer.md § 1)
  // ============================================================

  /// CSS `z-index`. `Auto` (default) does NOT form a stacking context on
  /// its own and orders by document order; `Layer(i32)` orders siblings
  /// within a context (0 default for explicit, negative behind, positive
  /// in front) AND forms a stacking context iff the entity is positioned
  /// (`Position.kind != Static`) — see [`forms_stacking_context`].
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md § 1, § 3.
  #[derive(Reflect, Clone, Copy, Default, PartialEq, Eq, Debug)]
  pub enum ZIndex {
      #[default]
      Auto,
      Layer(i32),
  }

  /// CSS `isolation`. `Isolate` forces a stacking context regardless of
  /// position or z-index.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md § 1, § 2.
  #[derive(Reflect, Clone, Copy, Default, PartialEq, Eq, Debug)]
  pub enum Isolation {
      #[default]
      Auto,
      Isolate,
  }

  /// Top-layer participation. `None` (default) = normal stacking; the
  /// non-`None` variants escape the parent stacking context and paint in
  /// the global top layer, ordered Fullscreen < Tooltip < Popover < Modal.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md § 1, § 4.
  #[derive(Reflect, Clone, Copy, Default, PartialEq, Eq, Debug)]
  pub enum TopLayer {
      #[default]
      None,
      Modal,
      Popover,
      Tooltip,
      Fullscreen,
  }
  ```
  **Implementer note:** the spec § 1 derive set is `#[derive(Reflect, Clone, Copy, Default)]`; this plan adds `PartialEq, Eq, Debug` for testability + the equality checks 6f needs. `i32` is `Eq`, so `ZIndex` can derive `Eq`.

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core z_index_default isolation_default top_layer_default
  ```
  Expected PASS.

- [ ] **Step 4: Project gate.** (Registration happens in T11; here confirm compile/tests/doc green.)
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  ```

- [ ] **Step 5: Commit.**
  ```bash
  git add crates/buiy_core/src/layout/types.rs
  git commit -m "feat(layout): stacking value types ZIndex/Isolation/TopLayer (Phase 9 — spec § 1)

ZIndex (Auto default), Isolation (Auto default), TopLayer (None default) per
stacking-and-top-layer.md § 1. register_type wiring lands in T11."
  ```

### Task 3: `Stacking` component in `components.rs`

**Spec:** § 1.

**Files:**
- Modify: `crates/buiy_core/src/layout/components.rs` (add `Stacking` component + imports)

- [ ] **Step 1: Failing test.** Add to `components.rs::mod tests`:
  ```rust
  #[test]
  fn stacking_default_is_auto_auto_none() {
      let s = Stacking::default();
      assert_eq!(s.z_index, ZIndex::Auto);
      assert_eq!(s.isolation, Isolation::Auto);
      assert_eq!(s.top_layer, TopLayer::None);
  }
  ```
  Run: `cargo test -p buiy_core stacking_default_is_auto_auto_none` — expected FAIL.

- [ ] **Step 2: Add `Stacking` to `components.rs`.** Near the Phase-8 `UiTransform`/`Containment` components:
  ```rust
  /// Depth-ordering for an entity's box: its `z-index`, `isolation`, and
  /// top-layer participation. Self-styling (a `Style` field). Consumed by
  /// sub-pass 6f `stacking_context`, which decides whether the entity
  /// forms a stacking context and computes the global paint order into the
  /// private `StackingContext` render handoff. Does NOT affect Taffy
  /// layout — stacking is a paint-order concern only (spec § 2).
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md § 1.
  #[derive(Component, Reflect, Clone, Default, PartialEq, Debug)]
  #[reflect(Component, Default)]
  pub struct Stacking {
      pub z_index: ZIndex,
      pub isolation: Isolation,
      pub top_layer: TopLayer,
  }
  ```
  Add to the `use super::types::{ … }` block:
  ```rust
  use super::types::{
      // ... existing imports ...
      Isolation, TopLayer, ZIndex,
  };
  ```
  **Implementer note:** the spec § 1 derive set is `#[derive(Component, Reflect, Clone, Default)] #[reflect(Component, Default)]`; this plan adds `PartialEq, Debug` for testability (consistent with `UiTransform`).

- [ ] **Step 3: Run the test.**
  ```bash
  cargo test -p buiy_core stacking_default_is_auto_auto_none
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/components.rs
  git commit -m "feat(layout): Stacking component (Phase 9 — spec § 1)

Self-styling depth-ordering component (z_index/isolation/top_layer). Default is
Auto/Auto/None. Style-field integration in T10; register_type in T11."
  ```

### Task 4: `StackingContext` private component in crate `components.rs`

**Spec:** § 2.1, D7.

**Files:**
- Modify: `crates/buiy_core/src/components.rs` (add `StackingContext` next to `ResolvedTransform`)

- [ ] **Step 1: Failing test.** Add to `components.rs::mod tests`:
  ```rust
  #[test]
  fn stacking_context_default_is_empty() {
      assert!(StackingContext::default().painters_z.is_empty());
  }
  ```
  Run: `cargo test -p buiy_core stacking_context_default_is_empty` — expected FAIL.

- [ ] **Step 2: Add `StackingContext` to `components.rs`.** After `ResolvedTransform`:
  ```rust
  /// Private render handoff for stacking: the paint order of every
  /// descendant within this entity's stacking context, written by
  /// sub-pass 6f (`stacking_context`) on each entity that forms a
  /// stacking context (and removed when it stops forming one). Mirrors
  /// how `ResolvedTransform` is the render handoff for the composed
  /// matrix. Not author-set, but reflectable so devtools can inspect it.
  ///
  /// `painters_z` is sorted per spec § 2.1: negative-`z_index` first,
  /// then in-flow non-positioned (document order), then floats (always
  /// empty in Buiy), then in-flow positioned with `z_index: Auto`
  /// (document order), then positive `z_index`. Nested stacking contexts
  /// appear as a single entry sorted by their own `z_index`. Top-layer
  /// entities (spec § 4) are excluded from their parent context and
  /// appended to the root context.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md § 2.1, § 5.
  #[derive(Component, Reflect, Clone, Default, Debug, PartialEq)]
  #[reflect(Component)]
  pub struct StackingContext {
      pub painters_z: Vec<Entity>,
  }
  ```
  **Implementer note:** `Entity` is `Reflect` in Bevy 0.18; `Vec<Entity>: Reflect`. Same derive shape as `ResolvedTransform` except `Default` is derivable here (empty `Vec`). Confirm `Entity` is in scope (`use bevy::prelude::*` at the top of `components.rs`).

- [ ] **Step 3: Run the test.**
  ```bash
  cargo test -p buiy_core stacking_context_default_is_empty
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/components.rs
  git commit -m "feat(layout): StackingContext private render handoff (Phase 9 — spec § 2.1)

painters_z paint-order Vec<Entity>, written by sub-pass 6f only. Mirrors the
ResolvedTransform render-handoff pattern. register_type/re-export in T11."
  ```

### Task 5: `TopLayerActivation` resource + `MultipleFullscreenTopLayer` warn key

**Spec:** § 4.2, D3, D6.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add `TopLayerActivation` resource)
- Modify: `crates/buiy_core/src/layout/types.rs` (add `LayoutWarnOnceKey::MultipleFullscreenTopLayer`)

- [ ] **Step 1: Failing test.** Add to `systems.rs::mod tests`:
  ```rust
  #[test]
  fn top_layer_activation_default_is_empty() {
      assert!(TopLayerActivation::default().order.is_empty());
  }
  ```
  And add to `types.rs::mod tests` (or wherever `LayoutWarnOnceKey` is tested; if no such test exists, this asserts the variant compiles + is hashable):
  ```rust
  #[test]
  fn multiple_fullscreen_warn_key_is_hashable() {
      let mut set = std::collections::HashSet::new();
      assert!(set.insert(LayoutWarnOnceKey::MultipleFullscreenTopLayer));
      assert!(!set.insert(LayoutWarnOnceKey::MultipleFullscreenTopLayer));
  }
  ```
  Run: `cargo test -p buiy_core top_layer_activation_default multiple_fullscreen_warn_key` — expected FAIL.

- [ ] **Step 2a: Add the resource to `systems.rs`.** Near `PostTaffyPositionOverrides` (`systems.rs:176`):
  ```rust
  /// Activation order for the single global top layer (spec § 4.2). A
  /// `VecDeque` where the most-recently-activated top-layer entity is at
  /// the back (paints last / on top within its tier). Maintained by
  /// sub-pass 6f via a per-frame current-membership rebuild (D3): entries
  /// no longer top-layer (deactivated or despawned) are dropped, newly
  /// top-layer entities are appended in tree order.
  ///
  /// Single global (not per-window): `buiy_core` has no per-window layout
  /// yet (D2). Per-window top layers are a follow-up.
  ///
  /// Spec: docs/specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md § 4.2.
  #[derive(Resource, Default, Debug)]
  pub struct TopLayerActivation {
      pub order: std::collections::VecDeque<Entity>,
  }
  ```
- [ ] **Step 2b: Add the warn-once variant to `types.rs`.** Append to `LayoutWarnOnceKey`:
  ```rust
  /// More than one `TopLayer::Fullscreen` entity is active simultaneously;
  /// CSS allows one — extras fall back to normal stacking (spec § 4.2).
  /// Session-wide (no `Entity`): the condition is a property of the set.
  MultipleFullscreenTopLayer,
  ```

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core top_layer_activation_default multiple_fullscreen_warn_key
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/src/layout/types.rs
  git commit -m "feat(layout): TopLayerActivation resource + MultipleFullscreenTopLayer key (Phase 9 — spec § 4.2)

Single global top-layer activation deque (D2/D3) + session-wide warn key for
>1 simultaneous fullscreen (D6). init_resource + register wiring in T8/T11."
  ```

### Task 6: `forms_stacking_context` pure helper

**Spec:** § 2 (triggers 1–4, 6), D1, D5.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add the pure predicate + unit tests)

- [ ] **Step 1: Failing tests.** Add to `systems.rs::mod tests`:
  ```rust
  use crate::layout::components::{Containment, Stacking};
  use crate::layout::types::{ContainFlags, Isolation, PositionKind, TopLayer, ZIndex};

  fn stk(z: ZIndex, iso: Isolation) -> Stacking {
      Stacking { z_index: z, isolation: iso, top_layer: TopLayer::None }
  }

  #[test]
  fn positioned_with_explicit_z_forms_context() {
      let s = stk(ZIndex::Layer(0), Isolation::Auto);
      assert!(forms_stacking_context(Some(&s), PositionKind::Relative, false, None, false));
  }

  #[test]
  fn static_with_explicit_z_does_not_form_context() {
      // CSS quirk: z-index on a static element does NOT form a context.
      let s = stk(ZIndex::Layer(5), Isolation::Auto);
      assert!(!forms_stacking_context(Some(&s), PositionKind::Static, false, None, false));
  }

  #[test]
  fn positioned_with_auto_z_does_not_form_context() {
      let s = stk(ZIndex::Auto, Isolation::Auto);
      assert!(!forms_stacking_context(Some(&s), PositionKind::Absolute, false, None, false));
  }

  #[test]
  fn isolate_forms_context_regardless_of_position() {
      let s = stk(ZIndex::Auto, Isolation::Isolate);
      assert!(forms_stacking_context(Some(&s), PositionKind::Static, false, None, false));
  }

  #[test]
  fn non_identity_transform_forms_context() {
      assert!(forms_stacking_context(None, PositionKind::Static, true, None, false));
  }

  #[test]
  fn paint_containment_forms_context() {
      let c = Containment { contain: ContainFlags::PAINT, ..Default::default() };
      assert!(forms_stacking_context(None, PositionKind::Static, false, Some(&c), false));
  }

  #[test]
  fn strict_containment_forms_context() {
      let c = Containment { contain: ContainFlags::STRICT, ..Default::default() };
      assert!(forms_stacking_context(None, PositionKind::Static, false, Some(&c), false));
  }

  #[test]
  fn root_always_forms_context() {
      assert!(forms_stacking_context(None, PositionKind::Static, false, None, true));
  }

  #[test]
  fn plain_in_flow_element_does_not_form_context() {
      assert!(!forms_stacking_context(None, PositionKind::Static, false, None, false));
  }
  ```
  Run: `cargo test -p buiy_core forms_stacking_context positioned_with static_with isolate_forms non_identity paint_containment strict_containment root_always plain_in_flow` — expected FAIL.

- [ ] **Step 2: Add the helper to `systems.rs`.** Near `compose_transform` (`systems.rs:2377`) or with the other pure helpers:
  ```rust
  /// The spec § 2 union of stacking-context-formation triggers that are
  /// implementable in `buiy_core` today (D1): (1) positioned + explicit
  /// `z_index`, (2) `Isolation::Isolate`, (3) non-identity transform,
  /// (4) `Containment.contain ⊇ PAINT/STRICT`, (6) root. Trigger (5)'s
  /// render-side formers (opacity/filter/blend) and the will-change SC
  /// former are deferred — their components don't exist yet (spec § 7);
  /// add an `|| render_side_former` clause here when they land.
  pub(super) fn forms_stacking_context(
      stacking: Option<&Stacking>,
      position_kind: PositionKind,
      has_transform: bool,
      containment: Option<&Containment>,
      is_root: bool,
  ) -> bool {
      // Trigger 6 — root.
      if is_root {
          return true;
      }
      // Trigger 3 — non-identity transform (ResolvedTransform present).
      if has_transform {
          return true;
      }
      if let Some(s) = stacking {
          // Trigger 2 — isolation.
          if matches!(s.isolation, Isolation::Isolate) {
              return true;
          }
          // Trigger 1 — positioned (non-static) with an explicit z-index.
          if !matches!(position_kind, PositionKind::Static)
              && matches!(s.z_index, ZIndex::Layer(_))
          {
              return true;
          }
      }
      // Trigger 4 — paint / strict containment.
      if let Some(c) = containment {
          if c.contain.intersects(ContainFlags::PAINT | ContainFlags::STRICT) {
              return true;
          }
      }
      false
  }
  ```
  **Implementer note:** `ContainFlags::PAINT`/`STRICT` are `bitflags` (Phase 8 `types.rs`); `STRICT` is a union that already includes `PAINT`/`SIZE`/`LAYOUT` — `intersects` handles both. Confirm `ContainFlags` exposes `intersects` (the `bitflags!` macro provides it) and the `PAINT`/`STRICT` consts; if `STRICT` is `PAINT|SIZE|LAYOUT` then `intersects(PAINT|STRICT)` is correct (a strict-contained entity intersects `PAINT`). Confirm `Containment.contain` is the field name (Phase 8 T9 — `Containment { contain: ContainFlags, content_visibility, will_change }`).

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core forms_stacking_context positioned_with static_with isolate_forms non_identity paint_containment strict_containment root_always plain_in_flow
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs
  git commit -m "feat(layout): forms_stacking_context predicate (Phase 9 — spec § 2)

Pure helper for SC-formation triggers 1-4 + 6 (positioned+z-index, isolation,
transform, paint/strict containment, root). Trigger 5 / will-change deferred (D1)."
  ```

### Task 7: `PaintTier` classifier + `sort_painters` (paint-order, spec § 2.1)

**Spec:** § 2.1 (five-tier sort), § 3 (`z_index` ordering), D5.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add `PaintTier`, `paint_key`, sort helper + unit tests)

- [ ] **Step 1: Failing tests.** Add to `systems.rs::mod tests`:
  ```rust
  #[test]
  fn paint_key_negative_z_sorts_first() {
      // Negative z-index → tier 0; in-flow non-positioned → tier 1;
      // auto-positioned → tier 3; positive z → tier 4.
      let neg = stk(ZIndex::Layer(-1), Isolation::Auto);
      let pos = stk(ZIndex::Layer(2), Isolation::Auto);
      let auto = stk(ZIndex::Auto, Isolation::Auto);
      // positioned entities
      let kn = paint_key(Some(&neg), PositionKind::Relative);
      let kp = paint_key(Some(&pos), PositionKind::Relative);
      let ka = paint_key(Some(&auto), PositionKind::Relative);
      let kf = paint_key(None, PositionKind::Static); // in-flow non-positioned
      assert!(kn < kf, "negative z paints behind in-flow");
      assert!(kf < ka, "in-flow paints behind auto-positioned");
      assert!(ka < kp, "auto-positioned paints behind positive z");
  }

  #[test]
  fn paint_key_orders_positive_z_ascending() {
      let z1 = stk(ZIndex::Layer(1), Isolation::Auto);
      let z2 = stk(ZIndex::Layer(2), Isolation::Auto);
      assert!(paint_key(Some(&z1), PositionKind::Relative) < paint_key(Some(&z2), PositionKind::Relative));
  }

  #[test]
  fn paint_key_static_z_index_is_ignored() {
      // z-index on a static element does not lift it out of in-flow tier.
      let z5 = stk(ZIndex::Layer(5), Isolation::Auto);
      let kf = paint_key(None, PositionKind::Static);
      assert_eq!(paint_key(Some(&z5), PositionKind::Static).0, kf.0,
          "static z-index stays in the in-flow tier");
  }
  ```
  Run: `cargo test -p buiy_core paint_key` — expected FAIL.

- [ ] **Step 2: Add the classifier + key to `systems.rs`.** The sort key is a tuple `(tier: u8, z: i32)` so a stable sort by `paint_key` then document order reproduces the spec § 2.1 ordering. (Document order is preserved by sorting a `Vec` that is already in `Children` order with a **stable** sort.)
  ```rust
  /// The spec § 2.1 paint tiers, as the primary sort rank. Document order
  /// (the `Children`-iteration order of the input `Vec`) breaks ties
  /// within a tier via a STABLE sort.
  ///
  /// Returns `(tier, z)`:
  /// - tier 0, z = the negative z   → negative `z_index` (positioned), lowest first
  /// - tier 1, z = 0                → in-flow non-positioned (document order)
  /// - tier 2, z = 0                → in-flow positioned, `z_index: Auto` (document order)
  /// - tier 3, z = the positive z   → positive `z_index` (positioned), lowest first
  ///
  /// (Floats — spec tier between non-positioned and auto-positioned — are
  /// always empty in Buiy, so they are omitted; the four live tiers keep
  /// the spec's relative order.) `z_index` on a `PositionKind::Static`
  /// entity is IGNORED (CSS quirk, spec § 3): a static element stays in
  /// tier 1 regardless of its `z_index`.
  pub(super) fn paint_key(stacking: Option<&Stacking>, position_kind: PositionKind) -> (u8, i32) {
      let positioned = !matches!(position_kind, PositionKind::Static);
      let z = match stacking.map(|s| s.z_index) {
          Some(ZIndex::Layer(n)) if positioned => Some(n),
          _ => None, // Auto, or static (z ignored)
      };
      match z {
          Some(n) if n < 0 => (0, n),
          None if !positioned => (1, 0),
          None => (2, 0), // positioned + auto z
          Some(n) /* n >= 0 */ => {
              if n == 0 {
                  // Positioned with explicit z-index 0 sits with the
                  // positive tier per CSS (0 is "explicit", paints above
                  // auto-positioned). Spec § 3: "0 is default for explicit".
                  (3, 0)
              } else {
                  (3, n)
              }
          }
      }
  }
  ```
  **Implementer note:** verify the spec § 6 "`z_index` ordering" fixture: three positioned siblings z `[2, -1, 0]` must order `[-1, 0, 2]`. Under `paint_key`: `-1 → (0,-1)`, `0 → (3,0)`, `2 → (3,2)`. Sorted: `(0,-1) < (3,0) < (3,2)` → `[-1, 0, 2]`. ✓ (The auto-positioned tier 2 sits between in-flow tier 1 and explicit-z tier 3, matching CSS: auto-positioned paint above in-flow but below any explicit non-negative z-index.)

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core paint_key
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs
  git commit -m "feat(layout): paint_key paint-order classifier (Phase 9 — spec § 2.1, § 3)

(tier, z) sort key reproducing the five-tier paint order with stable document
order tie-break; static z-index ignored per the CSS quirk."
  ```

### Task 8: `stacking_context` system 6f — base contexts + wiring

**Spec:** § 2, § 2.1, § 2.2, D5, D7. (Top layer is T9.)

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add the `stacking_context` system — base, no top-layer)
- Modify: `crates/buiy_core/src/layout/mod.rs` (wire 6f after 6e; `init_resource::<TopLayerActivation>`)
- Modify: `crates/buiy_core/tests/layout_pipeline_order.rs` (assert 6f runs after 6e)

- [ ] **Step 1: Failing test.** Add to `tests/layout_pipeline_order.rs` (mirror the existing pipeline-order tests' `app()` helper):
  ```rust
  #[test]
  fn stacking_context_runs_and_marks_positioned_z_index() {
      let mut app = app();
      // A root with one positioned + z-index child.
      let child = app
          .world_mut()
          .spawn((
              Node,
              Style::default()
                  .position(PositionKind::Relative)
                  .stacking(Stacking { z_index: ZIndex::Layer(1), ..Default::default() }),
          ))
          .id();
      let root = app
          .world_mut()
          .spawn((Node, Style::default()))
          .add_child(child)
          .id();
      app.update();
      // Root always forms a context; the child forms one (positioned+z).
      assert!(app.world().get::<StackingContext>(root).is_some(), "root forms a context");
      assert!(app.world().get::<StackingContext>(child).is_some(), "positioned+z child forms a context");
      // The root's painters_z contains the child (it is a descendant painter).
      let root_sc = app.world().get::<StackingContext>(root).unwrap();
      assert!(root_sc.painters_z.contains(&child));
  }

  #[test]
  fn plain_child_gets_no_stacking_context() {
      let mut app = app();
      let child = app.world_mut().spawn((Node, Style::default())).id();
      let _root = app.world_mut().spawn((Node, Style::default())).add_child(child).id();
      app.update();
      assert!(app.world().get::<StackingContext>(child).is_none(),
          "a plain in-flow child forms no context");
  }
  ```
  **Implementer note:** confirm the existing `app()` helper in `layout_pipeline_order.rs` adds `MinimalPlugins + CorePlugin + LayoutPlugin`, and the imports (`Stacking`, `ZIndex`, `StackingContext`, `PositionKind`, `Style`, `Node`) — add any missing `use buiy_core::…`. `.stacking(…)` is the T10 setter; if T10 hasn't landed when this test is written, spawn `Stacking { … }` as a separate component in the tuple instead. Use `.add_child` (Bevy 0.18 `EntityWorldMut::add_child`).
  Run: `cargo test -p buiy_core --test layout_pipeline_order stacking_context_runs plain_child_gets_no` — expected FAIL (system not wired).

- [ ] **Step 2: Add the `stacking_context` system to `systems.rs`.** Base version (top-layer handling is added in T9 — leave a clearly-marked TODO-free seam: T9 extends this same function). Full system:
  ```rust
  /// Sub-pass 6f — stacking-context detection + paint-order resolution.
  /// Runs after `transform_composition` (6e) so it can read the composed
  /// `ResolvedTransform` (trigger 3). Writes the private `StackingContext`
  /// render handoff; writes NOTHING to `PostTaffyPositionOverrides`
  /// (stacking does not move the layout box). Spec § 2 / § 2.1 / § 4.
  #[allow(clippy::too_many_arguments)]
  pub(super) fn stacking_context(
      mut commands: Commands,
      tree: NonSend<LayoutTree>,
      nodes: Query<(Entity, Option<&ChildOf>), With<Node>>,
      children_q: Query<&Children>,
      stacking_q: Query<&Stacking>,
      position_q: Query<&Position>,
      transformed: Query<(), With<crate::components::ResolvedTransform>>,
      containment_q: Query<&Containment>,
      display_q: Query<&Display>,
      have_sc: Query<Entity, With<crate::components::StackingContext>>,
      mut activation: ResMut<TopLayerActivation>,
      mut warned: ResMut<LayoutWarnedOnceSession>,
  ) {
      use crate::components::StackingContext;

      // --- closures reading the per-entity queries ---
      let display_none = |e: Entity| matches!(display_q.get(e), Ok(Display::None));
      let pos_kind =
          |e: Entity| position_q.get(e).map(|p| p.kind).unwrap_or(PositionKind::Static);
      let top_layer_of =
          |e: Entity| stacking_q.get(e).map(|s| s.top_layer).unwrap_or(TopLayer::None);
      let is_root = |parent: Option<&ChildOf>| {
          parent
              .map(|p| !tree.by_entity().contains_key(&p.parent()))
              .unwrap_or(true)
      };
      let forms = |e: Entity, root: bool| {
          forms_stacking_context(
              stacking_q.get(e).ok(),
              pos_kind(e),
              transformed.get(e).is_ok(),
              containment_q.get(e).ok(),
              root,
          )
      };

      // --- 1. top-layer activation rebuild (D3) — implemented in T9 ---
      // (T9 inserts the membership rebuild + fullscreen warn here, then
      // the escape logic below in step 4.)
      let _ = (&mut activation, &mut warned); // silence unused until T9

      // --- 2. find the root + classify which entities form contexts ---
      // (Single global tree → expect exactly one root in the MinimalPlugins
      // harness; multiple roots are each their own context.)
      let mut forming: std::collections::HashSet<Entity> = std::collections::HashSet::new();
      let mut roots: Vec<Entity> = Vec::new();
      for (e, parent) in nodes.iter() {
          if display_none(e) {
              continue;
          }
          let root = is_root(parent);
          if root {
              roots.push(e);
          }
          if forms(e, root) {
              forming.insert(e);
          }
      }

      // --- 3. build each forming context's painters_z ---
      // For an SC root R: walk R's subtree in document order; collect every
      // descendant that belongs to R's context. A child C of the current
      // node belongs to R's context UNLESS C itself forms a context — in
      // which case C is an atomic entry (added) but we do NOT descend into
      // C (it owns its own painters_z). Non-forming children are added and
      // descended through. Skip Display::None and (in T9) top-layer entities.
      let painters_of = |sc_root: Entity| -> Vec<Entity> {
          let mut painters: Vec<Entity> = Vec::new();
          let mut stack: Vec<Entity> = Vec::new();
          if let Ok(kids) = children_q.get(sc_root) {
              // push in reverse so we pop in document order
              stack.extend(kids.iter().rev());
          }
          while let Some(node) = stack.pop() {
              if display_none(node) {
                  continue;
              }
              // (T9: `if top_layer_of(node) != TopLayer::None { continue }`)
              painters.push(node);
              if !forming.contains(&node) {
                  if let Ok(kids) = children_q.get(node) {
                      stack.extend(kids.iter().rev());
                  }
              }
          }
          // Stable sort by paint tier; the Vec is already in document order,
          // so equal-tier entries keep document order (spec § 2.1).
          painters.sort_by_key(|&e| paint_key(stacking_q.get(e).ok(), pos_kind(e)));
          painters
      };

      // --- 4. write StackingContext on forming entities; remove stale ---
      for &e in &forming {
          let new = StackingContext { painters_z: painters_of(e) };
          let unchanged = have_sc
              .get(e)
              .ok()
              .and_then(|_| /* compare */ None::<()>) // see note
              .is_some();
          let _ = unchanged;
          // Idempotent insert: compare against the existing value.
          let differs = match commands_get_existing(&have_sc, e) {
              _ => true, // replaced by the read below
          };
          let _ = differs;
          // Simpler idempotent gate using a direct world read is not
          // available in a system; insert unconditionally is acceptable
          // for v1 (Bevy change-detection will mark it changed only when
          // the value actually differs IF we compare first — see note).
          commands.entity(e).insert(new);
      }
      // Remove StackingContext from entities that no longer form one.
      for e in have_sc.iter() {
          if !forming.contains(&e) || display_none(e) {
              commands.entity(e).remove::<StackingContext>();
          }
      }
  }
  ```
  **Implementer note (idempotent insert):** the sketch above shows the intent but the `commands_get_existing`/`differs` scaffolding is pseudo-code — **replace it** with the same gate `transform_composition` uses: add `Option<&StackingContext>` to the iteration so you can compare `existing.map(|sc| &sc.painters_z) != Some(&new.painters_z)` before inserting. Concretely, restructure step 4 to iterate the entities you need with an `existing: Query<Option<&StackingContext>>` (or fold `Option<&StackingContext>` into the main `nodes` query) and only `commands.entity(e).insert(new)` when it differs — mirror `systems.rs:2437-2443` exactly. Do NOT ship the pseudo-code; the reviewer must confirm a real idempotent gate. The `have_sc` removal loop is correct as written.
  **Implementer note (queries):** `Children` derefs to `&[Entity]`; `kids.iter()` yields `&Entity` in Bevy 0.18 (use `.copied()` / deref as the existing code does — check `sync_children_for_entity` at `systems.rs:1635` for the exact iteration idiom). `ChildOf::parent()` returns the parent `Entity`. `crate::components::ResolvedTransform` / `StackingContext` are the crate-level handoff components from Phase 8 / T4.

- [ ] **Step 3: Wire the system + resource in `mod.rs`.**
  - In the `init_resource` block (`mod.rs:69-72`), add: `app.init_resource::<systems::TopLayerActivation>();`
  - In the `PostTaffyOverrides` chained tuple (`mod.rs:205-214`), append `systems::stacking_context` after `systems::transform_composition`:
    ```rust
    (
        systems::clear_post_taffy_overrides,
        systems::sticky_offset,
        systems::table_layout,
        systems::multicol_pack,
        systems::anchor_resolution,
        systems::transform_composition,
        systems::stacking_context, // 6f — reads ResolvedTransform (6e)
    )
        .chain()
        .in_set(BuiyLayoutStep::PostTaffyOverrides),
    ```
  - Update the explanatory comment at `mod.rs:202-204` to state 6f now exists (it currently says "Phase 9 will append stacking 6f after 6e").

- [ ] **Step 4: Run the tests.**
  ```bash
  cargo test -p buiy_core --test layout_pipeline_order stacking_context_runs plain_child_gets_no
  ```
  Expected PASS.

- [ ] **Step 5: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/src/layout/mod.rs crates/buiy_core/tests/layout_pipeline_order.rs
  git commit -m "feat(layout): stacking_context sub-pass 6f — base contexts (Phase 9 — spec § 2)

Detects SC-forming entities (triggers 1-4,6), builds painters_z paint order via
the tree walk + paint_key stable sort, writes the private StackingContext handoff.
Wired after transform_composition (6e). Top-layer escape lands in T9."
  ```

### Task 9: Top-layer escape + activation order in 6f

**Spec:** § 4.1, § 4.2, § 4.3 (membership only — D8), § 4.4 (single global — D2), D3.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (extend `stacking_context` with the top-layer logic)

- [ ] **Step 1: Failing tests.** Add to `tests/layout_stacking.rs` (create the file; T12 adds the rest — or add here and let T12 extend). Minimal here:
  ```rust
  // tests/layout_stacking.rs
  use bevy::prelude::*;
  use buiy_core::layout::{Stacking, Style, TopLayer, TopLayerActivation};
  use buiy_core::{CorePlugin, layout::LayoutPlugin, components::StackingContext};
  // (adjust paths to match the actual re-export surface after T11.)

  fn app() -> App {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(CorePlugin);
      app.add_plugins(LayoutPlugin);
      app
  }

  #[test]
  fn top_layer_modal_escapes_to_root() {
      let mut app = app();
      let modal = app
          .world_mut()
          .spawn((Node, Style::default().top_layer(TopLayer::Modal)))
          .id();
      let parent = app
          .world_mut()
          .spawn((Node, Style::default()))
          .add_child(modal)
          .id();
      let root = app.world_mut().spawn((Node, Style::default())).add_child(parent).id();
      app.update();
      // Membership = root, not parent (spec § 4.1 / § 6 escape test).
      let root_sc = app.world().get::<StackingContext>(root).unwrap();
      assert!(root_sc.painters_z.contains(&modal), "modal escapes to root context");
      // It is NOT in the parent's painters (parent isn't an SC root here, but
      // assert the modal is not double-counted in any non-root context).
  }

  #[test]
  fn top_layer_activation_tracks_open_order() {
      let mut app = app();
      let a = app.world_mut().spawn((Node, Style::default().top_layer(TopLayer::Popover))).id();
      let b = app.world_mut().spawn((Node, Style::default().top_layer(TopLayer::Popover))).id();
      let _root = app.world_mut().spawn((Node, Style::default())).add_children(&[a, b]).id();
      app.update();
      let act = app.world().resource::<TopLayerActivation>();
      let order: Vec<Entity> = act.order.iter().copied().collect();
      assert_eq!(order, vec![a, b], "activation order follows tree/open order; most recent last");
  }
  ```
  **Implementer note:** the import paths are illustrative — fix them to the real re-export surface from T11 (`buiy_core::layout::{…}` + `buiy_core::components::StackingContext`). Run after T11 if the re-exports aren't in place, or import via the internal paths for an in-crate `tests/` file. `add_children(&[a,b])` spawns them as ordered children.
  Run: `cargo test -p buiy_core --test layout_stacking top_layer_modal_escapes top_layer_activation_tracks` — expected FAIL.

- [ ] **Step 2: Extend `stacking_context`.** Replace the step-1 placeholder + step-3 walk with the real top-layer logic:
  - **Step 1 (activation rebuild):** before classification:
    ```rust
    // Current top-layer membership (single global layer, D2/D3).
    let mut fullscreen_count = 0usize;
    let mut current_top: std::collections::HashSet<Entity> = std::collections::HashSet::new();
    for (e, _) in nodes.iter() {
        if display_none(e) {
            continue;
        }
        match top_layer_of(e) {
            TopLayer::None => {}
            TopLayer::Fullscreen => {
                fullscreen_count += 1;
                current_top.insert(e);
            }
            _ => {
                current_top.insert(e);
            }
        }
    }
    // Drop deactivated/despawned, keep order; append new in tree order.
    activation.order.retain(|e| current_top.contains(e));
    for (e, _) in nodes.iter() {
        if current_top.contains(&e) && !activation.order.contains(&e) {
            activation.order.push_back(e);
        }
    }
    if fullscreen_count > 1
        && warned.set.insert(LayoutWarnOnceKey::MultipleFullscreenTopLayer)
    {
        bevy::log::warn!(
            "Layout: {fullscreen_count} entities request TopLayer::Fullscreen; CSS allows one — extras fall back to normal stacking (spec § 4.2).",
        );
    }
    ```
  - **Step 3 (walk):** uncomment the top-layer skip so escaped entities don't appear in their parent context:
    ```rust
    if top_layer_of(node) != TopLayer::None {
        continue; // escapes — attached to root in step 5
    }
    ```
  - **Step 5 (escape attach):** after writing the in-flow contexts, append top-layer entities to the root context's `painters_z`, ordered by tier then activation index:
    ```rust
    // Tier rank: Fullscreen (bottom) < Tooltip < Popover < Modal (top).
    fn tier_rank(t: TopLayer) -> u8 {
        match t {
            TopLayer::Fullscreen => 0,
            TopLayer::Tooltip => 1,
            TopLayer::Popover => 2,
            TopLayer::Modal => 3,
            TopLayer::None => u8::MAX,
        }
    }
    if let Some(&root) = roots.first() {
        let mut top: Vec<Entity> = activation.order.iter().copied().collect();
        // stable sort by tier; within a tier, activation order is preserved
        // (the deque is already in activation order).
        top.sort_by_key(|&e| tier_rank(top_layer_of(e)));
        if !top.is_empty() {
            // root must be a forming context (trigger 6) — append after its
            // in-flow painters.
            // (Fold into the step-4 write: when e == root, extend painters
            //  with `top` before inserting. Implement by computing root's
            //  painters_z then `.extend(top)`.)
        }
    }
    ```
    **Implementer note:** the cleanest structure is: compute `top_sorted` (the escaped, tier+activation-ordered list) once; in step 4, when writing the **root** context, set `painters_z = painters_of(root); painters_z.extend(top_sorted.iter().copied())`. Ensure escaped entities are excluded from `painters_of` (the step-3 skip handles non-root contexts; also exclude them from the root's own in-flow walk so they appear once, at the end). For multiple roots (not expected under `MinimalPlugins`), attach to `roots.first()` and document the single-global-layer assumption. The `tier_rank` for `Fullscreen` puts it at the bottom of the top layer per spec § 4.2 ("bottom of the top-layer stack"); the "one fullscreen wins, rest fall back" nuance is a render concern beyond the warn (D6) — Phase 9 still lists all in `painters_z` ordered, and render picks the winner.

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core --test layout_stacking top_layer_modal_escapes top_layer_activation_tracks
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/tests/layout_stacking.rs
  git commit -m "feat(layout): top-layer escape + activation order in 6f (Phase 9 — spec § 4)

Top-layer entities escape parent stacking into the root context (membership=root),
ordered Fullscreen<Tooltip<Popover<Modal then activation order. Single global layer
(D2). TopLayerActivation rebuilt per frame (D3); >1 fullscreen warns once (D6)."
  ```

### Task 10: `Stacking` `Style` field + fluent setters

**Spec:** § 1, § 4.5, D4.

**Files:**
- Modify: `crates/buiy_core/src/layout/style.rs` (add `stacking: Stacking` field + 3 setters)

- [ ] **Step 1: Failing test.** Add to `style.rs::mod tests`:
  ```rust
  #[test]
  fn style_stacking_setters() {
      let s = Style::default()
          .z_index(ZIndex::Layer(3))
          .isolation(Isolation::Isolate)
          .top_layer(TopLayer::Modal);
      assert_eq!(s.stacking.z_index, ZIndex::Layer(3));
      assert_eq!(s.stacking.isolation, Isolation::Isolate);
      assert_eq!(s.stacking.top_layer, TopLayer::Modal);
  }

  #[test]
  fn style_stacking_default_is_identity() {
      let s = Style::default();
      assert_eq!(s.stacking, Stacking::default());
  }
  ```
  Run: `cargo test -p buiy_core style_stacking` — expected FAIL.

- [ ] **Step 2: Add the field + setters to `style.rs`.** Add `pub stacking: Stacking,` to the `Style` struct (with the other self-styling fields like `ui_transform`/`containment`), and:
  ```rust
  /// Set the full `Stacking` (z-index, isolation, top-layer) at once.
  pub fn stacking(mut self, s: Stacking) -> Self {
      self.stacking = s;
      self
  }

  /// Set `z-index` (spec § 3). `ZIndex::Layer(n)` on a positioned entity
  /// forms a stacking context and orders siblings.
  pub fn z_index(mut self, z: ZIndex) -> Self {
      self.stacking.z_index = z;
      self
  }

  /// Set `isolation` (spec § 2). `Isolation::Isolate` forces a context.
  pub fn isolation(mut self, iso: Isolation) -> Self {
      self.stacking.isolation = iso;
      self
  }

  /// Set top-layer participation (spec § 4). Non-`None` escapes the
  /// parent stacking context into the global top layer.
  pub fn top_layer(mut self, t: TopLayer) -> Self {
      self.stacking.top_layer = t;
      self
  }
  ```
  Add `Stacking` to the components import + `ZIndex`/`Isolation`/`TopLayer` to the types import in `style.rs` (mirror how `Container`/`Containment` are imported).
  **Implementer note:** `Style` derives `Bundle` so `stacking` is inserted on every spawn — that is intended (D4): every Style-spawned entity gets a `Stacking` at its identity default, which 6f reads (a default `Stacking` forms no context unless other triggers fire). Confirm the spec § 4.5 authoring example compiles: `Style::default().position(PositionKind::Fixed).inset(…).top_layer(TopLayer::Modal)`.

- [ ] **Step 3: Run the tests.**
  ```bash
  cargo test -p buiy_core style_stacking
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/style.rs
  git commit -m "feat(layout): Stacking Style field + z_index/isolation/top_layer setters (Phase 9 — spec § 1, D4)"
  ```

### Task 11: Register types + facade re-exports

**Spec:** § 1, § 2.1, § 4.2.

**Files:**
- Modify: `crates/buiy_core/src/layout/mod.rs` (`register_type` group + `pub use`)
- Modify: `crates/buiy_core/src/lib.rs` (re-export `StackingContext`)
- Modify: `crates/buiy/src/lib.rs` (top-level facade re-exports)

- [ ] **Step 1: Failing test.** Add to `tests/layout_pipeline_order.rs` (or a small `tests/layout_reflect.rs`):
  ```rust
  #[test]
  fn phase9_types_are_registered() {
      let mut app = app();
      app.update();
      let registry = app.world().resource::<AppTypeRegistry>().read();
      for name in [
          "buiy_core::layout::types::ZIndex",
          "buiy_core::layout::types::Isolation",
          "buiy_core::layout::types::TopLayer",
          "buiy_core::layout::components::Stacking",
          "buiy_core::components::StackingContext",
      ] {
          assert!(
              registry.get_with_type_path(name).is_some(),
              "type not registered: {name}",
          );
      }
  }
  ```
  **Implementer note:** confirm the exact `type_path` strings via `std::any::type_name` if the assert fails (module paths must match). `AppTypeRegistry` is in `bevy::prelude`.
  Run: `cargo test -p buiy_core phase9_types_are_registered` — expected FAIL.

- [ ] **Step 2: Register + re-export.**
  - In `mod.rs`, append a "Phase 9 — stacking + top layer" group to the `register_type` chain (after the Phase-8 group, before `pipeline::configure_pipeline`):
    ```rust
    // Phase 9 — stacking + top layer.
    .register_type::<Stacking>()
    .register_type::<ZIndex>()
    .register_type::<Isolation>()
    .register_type::<TopLayer>()
    .register_type::<crate::components::StackingContext>();
    ```
    (Merge into the existing chain — the chain ends with `.register_type::<WillChangeProperty>();`; convert that to `.register_type::<WillChangeProperty>()` and append the Phase-9 lines, terminating with `;`.)
  - In `mod.rs`, extend the `pub use components::{…}` block with `Stacking`, the `pub use types::{…}` block with `Isolation, TopLayer, ZIndex`, and the `pub use systems::{…}` block with `TopLayerActivation`.
  - In `crates/buiy_core/src/lib.rs`, add `StackingContext` to the `pub use components::{…}` (next to `ResolvedTransform`).
  - In `crates/buiy/src/lib.rs`, re-export the public Phase-9 set (`Stacking`, `ZIndex`, `Isolation`, `TopLayer`, `TopLayerActivation`, `StackingContext`) from the facade, mirroring the Phase-8 re-export block.
  **Implementer note:** `LayoutWarnOnceKey` is already registered (`mod.rs:159`) so the new `MultipleFullscreenTopLayer` variant needs no registration change.

- [ ] **Step 3: Run the test.**
  ```bash
  cargo test -p buiy_core phase9_types_are_registered
  ```
  Expected PASS.

- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/src/layout/mod.rs crates/buiy_core/src/lib.rs crates/buiy/src/lib.rs
  git commit -m "feat(layout): register + re-export Phase 9 stacking types"
  ```

### Task 12: Integration tests (spec § 6 surface, minus per-window)

**Spec:** § 6 (all bullets except the deferred per-window one — D2).

**Files:**
- Modify: `crates/buiy_core/tests/layout_stacking.rs` (extend with the remaining § 6 fixtures)

- [ ] **Step 1: Add the remaining § 6 tests.** Append to `tests/layout_stacking.rs` (the file created in T9). Cover each spec § 6 bullet not already covered:
  ```rust
  #[test]
  fn z_index_ordering_neg_zero_pos() {
      // spec § 6: three positioned siblings z=[2,-1,0] → painters_z [-1,0,2].
      let mut app = app();
      let z2 = app.world_mut().spawn((Node, Style::default().position(PositionKind::Relative).z_index(ZIndex::Layer(2)))).id();
      let zneg = app.world_mut().spawn((Node, Style::default().position(PositionKind::Relative).z_index(ZIndex::Layer(-1)))).id();
      let z0 = app.world_mut().spawn((Node, Style::default().position(PositionKind::Relative).z_index(ZIndex::Layer(0)))).id();
      let root = app.world_mut().spawn((Node, Style::default())).add_children(&[z2, zneg, z0]).id();
      app.update();
      let sc = app.world().get::<StackingContext>(root).unwrap();
      // The three positioned children are themselves SC roots (positioned+z),
      // so they appear as atomic entries in root.painters_z, ordered by z.
      let order: Vec<Entity> = sc.painters_z.iter().copied()
          .filter(|e| [z2, zneg, z0].contains(e)).collect();
      assert_eq!(order, vec![zneg, z0, z2], "painters ordered by z-index [-1,0,2]");
  }

  #[test]
  fn static_z_index_paints_in_document_order() {
      // spec § 6: static element + z-index 5 paints in document order, not lifted.
      let mut app = app();
      let a = app.world_mut().spawn((Node, Style::default())).id(); // first, static
      let b = app.world_mut().spawn((Node, Style::default().z_index(ZIndex::Layer(5)))).id(); // static z=5
      let root = app.world_mut().spawn((Node, Style::default())).add_children(&[a, b]).id();
      app.update();
      let sc = app.world().get::<StackingContext>(root).unwrap();
      let order: Vec<Entity> = sc.painters_z.iter().copied().filter(|e| [a, b].contains(e)).collect();
      assert_eq!(order, vec![a, b], "static z-index ignored; document order preserved");
      assert!(app.world().get::<StackingContext>(b).is_none(), "static+z forms no context");
  }

  #[test]
  fn isolation_forms_stacking_context() {
      // spec § 6: Isolation::Isolate → a StackingContext appears.
      let mut app = app();
      let iso = app.world_mut().spawn((Node, Style::default().isolation(Isolation::Isolate))).id();
      let _root = app.world_mut().spawn((Node, Style::default())).add_child(iso).id();
      app.update();
      assert!(app.world().get::<StackingContext>(iso).is_some());
  }

  #[test]
  fn mixed_top_layer_tiers_order_tooltip_below_modal() {
      // spec § 6: Modal + Tooltip open → Tooltip below Modal regardless of activation order.
      let mut app = app();
      // activate modal first, tooltip second — tier must still win.
      let modal = app.world_mut().spawn((Node, Style::default().top_layer(TopLayer::Modal))).id();
      let tooltip = app.world_mut().spawn((Node, Style::default().top_layer(TopLayer::Tooltip))).id();
      let root = app.world_mut().spawn((Node, Style::default())).add_children(&[modal, tooltip]).id();
      app.update();
      let sc = app.world().get::<StackingContext>(root).unwrap();
      let mi = sc.painters_z.iter().position(|e| *e == modal).unwrap();
      let ti = sc.painters_z.iter().position(|e| *e == tooltip).unwrap();
      assert!(ti < mi, "tooltip paints below modal (earlier in painters_z) regardless of activation");
  }
  ```
  **Implementer note:** the "transform forms a context" case is exercised indirectly by `forms_stacking_context` unit tests (T6) + the pipeline read; add an integration variant if the reviewer wants end-to-end transform→SC coverage (spawn `Style::default().translate_px(10.0, 0.0)` and assert `StackingContext` appears — the Phase-8 `transform_composition` 6e runs before 6f so `ResolvedTransform` is present). The **per-window** § 6 bullet is intentionally omitted (D2 / spec § 7).

- [ ] **Step 2: Run.**
  ```bash
  cargo test -p buiy_core --test layout_stacking
  ```
  Expected PASS (all fixtures).

- [ ] **Step 3: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add crates/buiy_core/tests/layout_stacking.rs
  git commit -m "test(layout): Phase 9 stacking integration suite (spec § 6, minus per-window)"
  ```

### Task 13: CHANGELOG + follow-ups + README status

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `docs/plans/follow-ups.md`
- Modify: `docs/README.md` (flip Phase 9 entry `[active]` → leave `[active]`; `[landed]` is set post-merge in the closeout commit, matching Phase 8's flow)

- [ ] **Step 1: CHANGELOG entry.** Add a Phase 9 section summarizing the shipped surface (`Stacking`/`ZIndex`/`Isolation`/`TopLayer`, `StackingContext`, `TopLayerActivation`, sub-pass 6f) and the deferrals (render-side SC triggers, will-change SC, per-window top layer).
- [ ] **Step 2: Update `follow-ups.md`.** Replace/extend the existing "Phase 9 stacking sub-pass 6f reads `ResolvedTransform`" entry: mark it **landed** (6f now exists) and add the three Phase-9 deferrals as new follow-up entries, each with originating-phase + symptom + implementation sketch + spec touchpoint:
  - **Render-side SC formers** (`opacity`/`filter`/`mix_blend_mode`) — extend `forms_stacking_context` when render components land. Touchpoint: `stacking-and-top-layer.md § 2` trigger 5, § 7.
  - **`will-change` SC former** — coordinates with the existing Phase-8 "will-change layer promotion + SC trigger" follow-up; cross-link it. Touchpoint: `transforms-and-containment.md § 5.3`, `stacking-and-top-layer.md § 2` trigger 5.
  - **Per-window top layer** — depends on `buiy-window-and-surface-design`; mirror the cross-window-anchor follow-up. Touchpoint: `stacking-and-top-layer.md § 4.4`, § 7.
- [ ] **Step 3: Confirm README Phase 9 entry** (added T1) reads `[active]`. (The `[landed]` flip + `docs: mark Phase 9 layout plan [landed]` commit happen after the whole-branch review + merge, matching the Phase-8 closeout.)
- [ ] **Step 4: Project gate + commit.**
  ```bash
  cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && cargo test --workspace
  git add CHANGELOG.md docs/plans/follow-ups.md docs/README.md
  git commit -m "docs(layout): Phase 9 CHANGELOG + follow-ups (stacking + top layer)"
  ```

---

## Self-review (against the spec)

**Spec coverage** (`stacking-and-top-layer.md`):
- § 1 `Stacking`/`ZIndex`/`Isolation`/`TopLayer` → T2 (enums), T3 (`Stacking`), T10 (Style field). ✓
- § 2 SC formation triggers 1–4, 6 → T6 (`forms_stacking_context`), T8 (system). Trigger 5 + will-change deferred (D1, spec § 7). ✓
- § 2.1 `StackingContext.painters_z` + five-tier sort → T4 (component), T7 (`paint_key`), T8 (walk + sort). ✓
- § 2.2 performance (6f O(entities)) → T8 (single tree pass). ✓
- § 3 `z_index` ordering + static-ignored quirk → T7 (`paint_key`), T12 (fixtures). ✓
- § 4.1/4.2/4.3 top-layer escape + tier + activation → T5 (resource), T9 (escape + activation), T12 (mixed-tier fixture). Clip-rect = render (D8). ✓
- § 4.4 per-window → deferred (D2, spec § 7); follow-up T13. ✓
- § 5 mapping to render → `StackingContext`/`Stacking`/`TopLayerActivation` are the consumed handoffs; render consumption is a follow-up. ✓
- § 6 test surface → T8 (runs/marks), T9 (escape, activation), T12 (z-order, static, isolation, mixed-tier). Per-window omitted (D2). ✓

**Placeholder scan:** the T8 idempotent-insert sketch is explicitly flagged as pseudo-code with a mandatory "replace with the real `Option<&StackingContext>` gate mirroring `transform_composition`" instruction — the reviewer must reject the pseudo-code. All other steps have concrete code + commands.

**Type consistency:** `Stacking` fields (`z_index`/`isolation`/`top_layer`), `StackingContext.painters_z`, `TopLayerActivation.order`, `forms_stacking_context(stacking, position_kind, has_transform, containment, is_root)`, `paint_key(stacking, position_kind) -> (u8, i32)`, `LayoutWarnOnceKey::MultipleFullscreenTopLayer` are used consistently across T2–T13.
