# Buiy layout — Phase 6: anchor positioning

**Date:** 2026-05-21
**Status:** active
**Spec:** [`specs/2026-05-08-buiy-layout-design/display-and-positioning.md`](../specs/2026-05-08-buiy-layout-design/display-and-positioning.md) § 3 (anchor positioning) + [`architecture.md`](../specs/2026-05-08-buiy-layout-design/architecture.md) § 3 (sub-pass 6d), § 6 (error model).
**Supersedes:** none (graduates from "anchor positioning" sub-pass 6d stub in Phase 5 `BuiyLayoutStep::PostTaffyOverrides`).

> **For agentic workers:** REQUIRED SUB-SKILL — use `superpowers:subagent-driven-development`. Each task lists exact file paths and TDD steps; steps use checkbox (`- [ ]`) tracking.

**Goal:** Add CSS anchor positioning to Buiy layout — `Anchor` component, `AnchorNameRegistry` resource maintained by Bevy 0.18 observers, and a sub-pass 6d `anchor_resolution` system that Kahn-sorts anchored entities, evaluates each one's `position_try` fallback chain against this frame's Taffy output, and writes the resolved position to a transient `AnchorOverrides` resource that `write_resolved_layout` (step 7) merges with Taffy's per-node layout. Anchor cycles are broken deterministically by dropping the edge from the *most-recently-inserted* anchored entity in the cycle (tracked via a monotonic epoch in the registry). Per-frame warn dedup gates all anchor-failure log lines via a `HashSet<(Entity, AnchorErrorKind)>` resource cleared at the top of `anchor_resolution`.

**Architecture (3 sentences):**
1. **Types-first decomposition.** `Anchor` lives in `components.rs` alongside `Position`; `AnchorName` / `AnchorRef` / `PositionTry` / `TryCondition` live in `types.rs` alongside `PositionKind` / `Inset`; `LayoutAnchorBroken` is a unit-struct marker in `components.rs` alongside `ContainerQueryActive`. The decomposed-only convention applies: `Anchor` is not folded into `Style` per spec [`architecture.md § 2.4`](../specs/2026-05-08-buiy-layout-design/architecture.md#24-child-side-components-decomposed-only) ("anchored elements are typically rare ... the decomposed-only convention keeps `Style`'s authoring surface focused on the 95% case"). The Phase 5 Container precedent for unconditional Bundle fields therefore does *not* apply to Anchor; authors spawn `(Style::default(), Anchor { ... })`.
2. **Observer-maintained registry.** `AnchorNameRegistry` is a `Resource` populated synchronously by three observers (`On<Insert, Anchor>`, `On<Replace, Anchor>`, `On<Remove, Anchor>`) registered via `app.add_observer(...)` in `LayoutPlugin::build`. The registry stores `HashMap<String, Vec<(Entity, u64)>>` (most-recently-inserted-wins via `last_insert_wins`-style append) plus `HashMap<Entity, u64>` (entity → insertion epoch) plus a monotonic `u64 next_epoch` counter. Multiple entities declaring the same name produce a `warn!` once per (name, frame), deduplicated via the per-frame warn resource.
3. **Sub-pass 6d as a normal Bevy system with `tree.tree.layout()` reads.** The pass runs after `cq_flip_rerun` and before `write_resolved_layout`. It (a) clears `AnchorOverrides.by_entity` and ALL variants of `LayoutAnchorWarnedThisFrame.set` (observers do not contribute to this set — see D11); (b) builds the in-degree map; pre-populates external target nodes (entities pointed at by an `AnchorRef::Entity(e)` that themselves have no `Anchor` component) as edge-map keys with `None` outgoing so Kahn's termination check is well-defined (D10); Kahn-sorts the anchored→anchor DAG; (c) on cycle detection, finds the cycle node with the highest insertion epoch and drops its outgoing edge, re-running Kahn from scratch (bounded re-runs: each iteration removes one edge from a finite graph); (d) for each anchored entity in topological order, resolves the target via `AnchorRef::Entity(e)` or `AnchorRef::Name(name) → registry lookup`, looks up the target's `Display` component via a separate `Query<&Display>` and treats `Display::None` as `TargetMissing` (D9 — `sync_styles` does NOT remove `Display::None` entities from `tree.by_entity`; it sets `taffy::Display::None` on the existing node so `tree.tree.layout()` would return a zero-size box, NOT a `None`), reads the anchor's box from `tree.tree.layout(anchor_taffy_node_id)` (the same prior-art pattern as Phase 5 `cq_flip_check` — see `architecture.md § 3.2` and `crates/buiy_core/src/layout/systems.rs:857-880`); (e) iterates `position_try` in order, evaluates each `TryCondition` against this frame's resolved sizes + the primary window's viewport size, and writes the *first passing try's* resolved position into `AnchorOverrides.by_entity` while idempotently removing `LayoutAnchorBroken`; (f) on broken resolution (target missing, target `Display::None`, every try fails, or in a cycle), writes `Vec2::ZERO` to `AnchorOverrides.by_entity`, idempotently inserts `LayoutAnchorBroken` (per D8 — when an edge is dropped due to cycle detection, *both* endpoints of the dropped edge get `LayoutAnchorBroken`, matching spec § 3.4 line 229), and emits one `warn!` per (entity, kind) via the per-frame dedup gate; detects `DuplicateName` by scanning `AnchorNameRegistry.by_name` buckets for `len > 1` and recording one warn per (late-inserter-entity, DuplicateName) per frame (D11). `write_resolved_layout` (step 7) consults `AnchorOverrides.by_entity` per entity and uses the override position (with size still from `tree.tree.layout()`) when present.

**Tech Stack:** Bevy 0.18 (observers via `app.add_observer(|t: On<Insert, Anchor>, ...| { ... })` per `/home/intendednull/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_ecs-0.18.1/src/observer/mod.rs:54`), Taffy 0.10 (`tree.tree.layout(node_id) → TaffyResult<&Layout>` for reading the anchor target's resolved rect — same call site shape as Phase 5 `cq_flip_check` at `crates/buiy_core/src/layout/systems.rs:691-704`), `std::collections::{HashMap, HashSet, VecDeque}` (no `bevy::utils::HashMap` in the codebase). No new external dep — Kahn sort is hand-rolled (graph size is O(anchored entities), typically <20, so the constant factor saved by `petgraph` is not worth the dep).

---

## Prior-art citations (used throughout this plan)

Each task below references these. Quoting the file + line here once so individual tasks stay tight.

- **Idempotent insert pattern** — `crates/buiy_core/src/layout/systems.rs:471-495` (`write_resolved_layout` compares `cur.position == new.position && cur.size == new.size` before `commands.entity(e).insert(new)`); `crates/buiy_core/src/layout/systems.rs:687-704` (`cq_activate`'s `was_active.is_none() && active` flip dance for `ContainerQueryActive`/`Inactive`). Mirror this exactly for `LayoutAnchorBroken`.
- **Memoized ancestor walk** — `crates/buiy_core/src/layout/systems.rs:514-568` (`inherit_writing_mode` allocates `HashMap<Entity, WritingMode>` once per call and uses it as a per-call cache via `resolve_writing_mode`). Phase 6's Kahn sort uses a similar per-call cache: `HashMap<Entity, AnchorNodeState>` for in-degree tracking.
- **Per-session warn-once dedup (AtomicBool gates)** — `crates/buiy_core/src/layout/translate.rs:9-80` (`WARNED_CQ_NO_ANCESTOR`, `WARNED_CQB_AGAINST_INLINE`). Phase 6 introduces a *per-frame* variant for anchor errors (spec § 3.2 step 4: "A `warn!` fires once per (entity, frame)") — the new pattern is a `Resource` holding `HashSet<(Entity, AnchorErrorKind)>` cleared at the top of `anchor_resolution`.
- **Style Bundle fluent setters / Container precedent** — `crates/buiy_core/src/layout/style.rs:44-55` (Bundle field list), `crates/buiy_core/src/layout/style.rs:414-447` (Container fluent setters), `crates/buiy_core/src/layout/components.rs:306-311` (Container's sentinel default). Phase 6 deviates: Anchor stays decomposed-only (no Bundle field, no fluent setter) per spec § 2.4 — see "Architecture" sentence 1 above.
- **Phase 5 nested `Or<>` filter widening** — `crates/buiy_core/src/layout/systems.rs:148-195` (outer `Or<>` at 15 entries with one nested inner `Or<(4)>`). Phase 6 adds `Changed<Anchor>` and `Changed<LayoutAnchorBroken>` — the inner nested `Or` is rebalanced to keep the outer ≤15. *Rationale: see Task 10.*
- **Resource + reset semantics** — `crates/buiy_core/src/layout/systems.rs:52-63` (`LayoutTaffyComputeCount`, `SyncStylesIterCount`); `crates/buiy_core/src/layout/systems.rs:421` (reset at start of `taffy_compute`). Phase 6 mirrors: `AnchorOverrides`, `LayoutAnchorWarnedThisFrame` cleared at top of `anchor_resolution`.
- **Pipeline step + attach point** — `crates/buiy_core/src/layout/pipeline.rs:17-44` (`BuiyLayoutStep::PostTaffyOverrides` enum slot, currently empty), `crates/buiy_core/src/layout/mod.rs:95-107` (`.in_set(BuiyLayoutStep::*)` chain — Phase 6 attaches `anchor_resolution.in_set(BuiyLayoutStep::PostTaffyOverrides)`).
- **Style components-only convention; not in Bundle** — spec [`architecture.md § 2.4`](../specs/2026-05-08-buiy-layout-design/architecture.md#24-child-side-components-decomposed-only): "anchored elements are typically rare (tooltips, popovers, dropdowns) and each carries a non-trivial `position_try` chain. The decomposed-only convention keeps `Style`'s authoring surface focused on the 95% case."
- **Reading anchor box from `tree.tree.layout()` not `ResolvedLayout`** — Phase 5 `cq_flip_check` precedent. `crates/buiy_core/src/layout/systems.rs:857-880` reads `let layout = tree.tree.layout(*node_id);` because at sub-pass 6 time, `ResolvedLayout` is *stale* (it's written in step 7, after step 6). Spec [`architecture.md § 3.2`](../specs/2026-05-08-buiy-layout-design/architecture.md#32-container-query-re-layout): "the size source is `tree.layout(node_id)` ... it is *not* the entity-side `ResolvedLayout` (that's written in step 7 and stale at this point in the chain)." Phase 6 anchor pass follows the same convention; the spec text "read its `ResolvedLayout`" in display-and-positioning.md § 3.2 is shorthand for "read the anchor's resolved box" — the *source* is Taffy.
- **Bevy 0.18 observer API** — `app.add_observer(|t: On<Insert, Anchor>, q: Query<&Anchor>, mut reg: ResMut<AnchorNameRegistry>| { ... });` per `/home/intendednull/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_ecs-0.18.1/src/observer/mod.rs:54`. `Insert` is a lifecycle event struct (`pub struct Insert { pub entity: Entity }`) per `bevy_ecs-0.18.1/src/lifecycle.rs:348`. The `On` type exposes `event() -> &E`, so `t.event().entity` is the inserted entity; reading the Anchor value requires `q.get(t.event().entity)`. **Plan v2 decision (D12): observers register as CLOSURES**, not named `fn` items — the `On<'w, 't, E, B>` type has two lifetime parameters with no defaults, and Rust's elision rules for multi-lifetime structs in named-fn signatures are subtle. Closures inherit lifetimes from `add_observer`'s `IntoObserverSystem` impl and compile cleanly. The closure body forwards to a private helper (`handle_anchor_insert`) for testability.
- **`SmolStr` vs `String`** — codebase uses `String` for area names (`crates/buiy_core/src/layout/types.rs:394` — "Spec uses `SmolStr` for area names; Phase 3 uses `String` to avoid a new direct dep."). Phase 6 follows the same precedent: `AnchorName::Named(String)` and `AnchorRef::Name(String)`. **Plan-wide invariant:** all anchor-name fields are `String`.

---

## File map (what each task touches)

| File | Touched by tasks |
|---|---|
| `crates/buiy_core/src/layout/types.rs` | T1 (add `AnchorName`, `AnchorRef`, `PositionTry`, `TryCondition`, `AnchorErrorKind`) |
| `crates/buiy_core/src/layout/components.rs` | T2 (add `Anchor`, `LayoutAnchorBroken`) |
| `crates/buiy_core/src/layout/systems.rs` | T4 (resources), T5 (observer fns), T6 (Kahn helper), T7 (`anchor_resolution` system), T9 (widen `write_resolved_layout`), T10 (widen `sync_styles` filter) |
| `crates/buiy_core/src/layout/mod.rs` | T8 (`register_type`, `init_resource`, `add_observer`, `add_systems`) |
| `crates/buiy_core/src/lib.rs` | T8 (re-export new public types) |
| `crates/buiy/src/lib.rs` | T8 (re-export new public types from the top-level facade crate) |
| `crates/buiy_core/tests/layout_pipeline_order.rs` | T11 (augment fixture with anchored+anchor pair) |
| `crates/buiy_core/tests/layout_anchor_positioning.rs` | T11 (new file — 7 integration tests covering basic, fallback chain, cycle, missing target, idempotent, broken-marker-clear, observer-registry) |
| `CHANGELOG.md` | T12 (post-merge — separate from this plan) |

No changes to: `translate.rs` (anchor pass reads Taffy directly, no per-property translation), `pipeline.rs` (step `PostTaffyOverrides` is already declared in Phase 1 — Phase 6 just attaches a system to it), `style.rs` (Anchor stays decomposed-only per spec § 2.4).

---

## Decision blocks (locked-in choices the implementer must honor)

### D1. AnchorName / AnchorRef payload type — `String`, not `SmolStr`

The spec text uses `SmolStr` (display-and-positioning.md § 3.1 lines 174-183). The codebase uses `String` everywhere similar names appear (e.g. `GridAreas` named-area lookup in `crates/buiy_core/src/layout/types.rs:394` — "Phase 3 uses `String` to avoid a new direct dep"). **Decision: use `String` in Phase 6.** Adding `smol_str` to the workspace just for anchor-name interning is not justified at the size of typical anchor graphs (<20 names per app). When the rest of the codebase migrates to `SmolStr`, a one-line type swap covers it. Spec-side: this is an implementation-detail divergence, not a behavioral one — both types compare structurally for HashMap lookup.

### D2. Anchor stays decomposed-only (no Bundle field in Style)

Spec [`architecture.md § 2.4`](../specs/2026-05-08-buiy-layout-design/architecture.md#24-child-side-components-decomposed-only) is explicit: "The *child side* — properties that only make sense on a child of a particular container ... and `Anchor` (which describes a relationship to another entity) live as decomposed components only. They are spawned alongside `Style`". **Decision: do NOT add `pub anchor: Anchor` to `Style`. Do NOT add fluent setters.** Authoring example: `commands.spawn((Style::default(), Anchor { position_anchor: Some(AnchorRef::Name("submit-btn".into())), position_try: vec![...], ..default() }));`. This is intentionally less ergonomic than Container (which is in the Style Bundle) — the trade-off is keeping Style's surface bounded.

### D3. AnchorOverrides as a frame-local `Resource`, not a private component

Three options were considered:
- **A.** Private `AnchorComputedPosition(Vec2)` component, inserted by anchor pass, consulted by `write_resolved_layout`.
- **B.** `Resource AnchorOverrides { by_entity: HashMap<Entity, Vec2> }`, cleared at top of anchor pass, consulted by `write_resolved_layout`.
- **C.** anchor pass writes `ResolvedLayout` directly; `write_resolved_layout` filters out anchored entities.

**Decision: B.** Component-style overrides (A) would churn `Changed<AnchorComputedPosition>` every frame, violating the O(0) steady-state contract (Phase 2 invariant). C couples step 7's filter to a per-entity component lookup and complicates the existing idempotent-insert pattern. B is a transient per-frame resource with no Changed<> impact and a simple HashMap consult inside step 7. Implementation: `anchor_resolution` clears the map at the top, populates per successful resolution + per broken case (writing `Vec2::ZERO` for broken). `write_resolved_layout` reads `overrides.by_entity.get(&entity)` and uses the override position with size from `tree.tree.layout()`.

### D4. Cycle-edge drop algorithm — re-run Kahn after each drop, not incremental update

When Kahn terminates with `sorted.len() < total_nodes`, any node still having `in_degree > 0` is part of a cycle (or has a cycle-only ancestor). To break the cycle:
1. Collect nodes with `in_degree > 0` (cycle candidates).
2. Among them, find the node with the **highest** insertion epoch (most recently inserted).
3. Drop that node's outgoing edge (the (cycle-node, anchor) edge).
4. Re-run Kahn from scratch on the modified graph.

**Decision: re-run from scratch, not incremental.** Re-running is O(V+E) per iteration. The graph is small (typically <20 anchored entities). Worst case: K cycles requires K Kahn iterations, each O(V+E) = O(K × V × E). With V ≤ 20 and K ≤ V, total is ≤ O(20³) = 8000 ops — negligible per frame. Incremental in-degree updates would save a constant factor but introduce subtle correctness bugs (must carefully decrement only outgoing edges of the dropped source). Re-running is simpler to test and matches what the spec terms "deterministic". The cycle nodes' epochs are read from `AnchorNameRegistry.entity_epochs` and an analogous map for direct `AnchorRef::Entity` cases (any `Anchor`-bearing entity has an entry; the observer in T5 populates this on every `On<Insert, Anchor>` trigger).

### D5. Per-frame warn dedup pattern (new in Phase 6) — RESOLVES SPEC CONFLICT

**Spec conflict:** architecture.md § 6 (lines 232-234) says "warn once" semantics are "per-(entity, error-kind) pair, deduplicated via a `HashSet` resource cleared on `BuiyExit`" — i.e., per-session. display-and-positioning.md § 3.2 step 4 (line 203) says anchor `warn!` fires "once per (entity, frame)" — i.e., per-frame. These conflict.

**Decision: anchor errors use PER-FRAME dedup.** Justification: the user-facing contract in spec § 3.2 step 4 is more specific (it names the error category, not just the general error model) and is the authoring-surface contract. The architecture-level "cleared on BuiyExit" applies to other error kinds (Taffy `Err`, missing parent in step 1, etc.) where the same error reproducing every frame is genuinely a permanent bug. Anchor errors are different: an anchor can become broken (target moves off-screen, viewport resizes) and then unbroken without code change. Per-frame warns let the *author* see when the state is currently broken; once-per-session would warn only on the first frame and then go silent even though the broken state persists.

**Implementation:**

```rust
pub enum AnchorErrorKind {
    TargetMissing,         // AnchorRef points at an entity that doesn't exist / is despawned / Display::None
    AllFallbacksFailed,    // Every PositionTry's conditions evaluated false
    InCycle,               // This entity's edge was dropped due to cycle detection
    DuplicateName,         // A second entity declared the same anchor_name
    AnchorSizeUsed,        // anchor-size() in a PositionTry inset (deferred to v1.x)
}

#[derive(Resource, Default, Debug)]
pub struct LayoutAnchorWarnedThisFrame {
    pub set: HashSet<(Entity, AnchorErrorKind)>,
}
```

Cleared at the top of `anchor_resolution` (set ← `HashSet::new()`). **The observers do NOT contribute to this set** (D11 resolution): they only update the registry. `DuplicateName` is *re-detected* by `anchor_resolution` each frame by scanning `AnchorNameRegistry.by_name` for buckets with `bucket.len() > 1` — the late-inserter (last entry) is the warn target. This guarantees the warn is emitted every frame the duplicate persists, not only on the frame of the duplicate insert.

**Divergence from arch.md § 6 documented:** Phase 6 introduces a per-frame variant. Other Phase 1-5 warn paths (Taffy `Err` in `taffy_compute`, missing parent in `sync_styles`, `Length::Fr` outside grid in `translate.rs`) continue to use the per-session AtomicBool gates. A future cleanup may unify both behind a single `LayoutWarnLog` resource with per-kind dedup policies, but that is out of Phase 6 scope.

### D6. Anchor declaration AND target on the same entity

The spec defines `Anchor` with both `anchor_name` (declares this entity AS an anchor) and `position_anchor` (declares this entity anchors TO another). A single entity can have both fields set: it's both an anchor for others AND anchored to a third party. The Kahn DAG handles this naturally — the entity has an outgoing edge (its `position_anchor`) and is the target of incoming edges (from entities pointing at its `anchor_name`). The "most-recently-inserted in cycle" tiebreaker is per-entity, not per-edge; if such an entity is in a cycle, dropping its outgoing edge breaks the cycle the same way as any other anchored node.

### D7. `tree.tree.layout(anchor_taffy_node_id)` access pattern

Phase 5 reads `tree.tree.layout(*node_id)` from `cq_flip_check` (systems.rs:691-704) — the receiver is `NonSendMut<LayoutTree>` and the lookup is via `tree.by_entity.get(&entity).copied()`. **Decision: Phase 6 uses the identical pattern.** The signature for `anchor_resolution` includes `tree: NonSendMut<LayoutTree>` (mutable because `write_resolved_layout` also takes `NonSend<LayoutTree>`, and `cq_flip_rerun` takes `NonSendMut` — Phase 6 takes `NonSend` since it does not mutate the tree, only reads). Actually — the system *only* reads tree.tree.layout, so `NonSend<LayoutTree>` is correct (Phase 6 does not call `compute_layout` or mutate the tree).

### D8. Idempotent `LayoutAnchorBroken` marker — insert if missing on broken case, remove if present on resolved case; BOTH cycle endpoints get the marker

`LayoutAnchorBroken` is a unit-struct marker (Phase 5 ContainerQueryActive/Inactive precedent at `components.rs:348-356`). Phase 6 idempotent-insert pattern matches systems.rs:687-704:

```rust
if broken && existing_broken.is_none() {
    commands.entity(entity).insert(LayoutAnchorBroken);
} else if !broken && existing_broken.is_some() {
    commands.entity(entity).remove::<LayoutAnchorBroken>();
}
```

No-op when state is already correct. Avoids `Changed<LayoutAnchorBroken>` churn.

**Cycle handling — both endpoints get LayoutAnchorBroken (spec § 3.4 line 229).** When Kahn drops the (source, target) edge of a cycle, the spec mandates *both* endpoints get the marker. Implementation: maintain a `broken_set: HashSet<Entity>` populated with (a) the dropped source AND (b) the target Entity the dropped edge pointed at (read from the pre-drop edges map). Both go into broken_set, both get `LayoutAnchorBroken` via idempotent insert. The target may not have an `Anchor` component (could be a plain `Node`) — Phase 6 still inserts the marker on it; this is the spec's documented behavior. Test 3 (cycle) must assert both endpoints have `LayoutAnchorBroken`. The marker is cleared via the same idempotent dance on the next frame's resolution if the cycle dissolves.

### D9. `Display::None` target detection — explicit `Display` query (not via tree.by_entity absence)

**Critical correction from the v1 plan.** `Display::None` does NOT remove the entity from `tree.by_entity`. Reading `crates/buiy_core/src/layout/translate.rs:447`: `Display::None` maps to `taffy::Display::None` — the entity remains a node in the Taffy tree with `display: none` style (Taffy zeroes its size + skips layout, but the node ID is still mapped). So `tree.by_entity.get(&hidden_anchor)` returns `Some(node_id)`, and `tree.tree.layout(node_id)` returns `Ok(Layout { size: (0,0), ... })`, not `Err`.

**Decision: `anchor_resolution` queries `Query<&Display>` separately to detect `Display::None` on the target.** Algorithm:
1. After resolving the target Entity via registry or direct ref, look up `display_q.get(target_entity)`.
2. If the result is `Ok(&Display::None)`, treat as `TargetMissing`: write `Vec2::ZERO`, mark broken, warn.
3. Otherwise proceed to read `tree.tree.layout(target_taffy_id)`.

Adds a second query parameter to `anchor_resolution`: `display_query: Query<&Display>`. This is read-only and conflict-free with `anchored_query: Query<(Entity, &Anchor, Option<&LayoutAnchorBroken>), With<Node>>` because the borrowed component sets are disjoint (Display vs Anchor).

### D10. Kahn pre-pass for external target nodes

**Critical correction.** Kahn's algorithm increments `in_degree[t]` for every edge `(s → t)`. If `t` is *not* a source in the edges map (i.e., `t` is an anchor target Entity that has no `Anchor` component itself), `t` ends up with `in_degree[t] = 1` but is never dequeued (Kahn only dequeues from `current_edges.keys()`). On termination, `t` has `in_degree > 0` and is flagged as a cycle node — but `t` has no outgoing edge to drop. The plan's `current_edges.get_mut(&drop_from)` does nothing for an external `t`; the loop re-runs unchanged and goes infinite.

**Decision: pre-populate external targets as edge-map keys with `None` outgoing.**

```rust
// Add this BEFORE in_degree is built (in kahn_anchor_sort, after copying input edges into current_edges):
let external_targets: Vec<Entity> = current_edges
    .values()
    .filter_map(|t| t.as_ref().copied())
    .filter(|t| !current_edges.contains_key(t))
    .collect();
for t in external_targets {
    current_edges.insert(t, None);
}
```

External targets now have `in_degree[t]` correctly incremented by their incoming edge, dequeued as soon as their in-degree reaches 0, and contribute to `order`. Kahn terminates correctly. The cycle-node identification (in_degree > 0 post-Kahn) becomes accurate.

### D11. `DuplicateName` is detected by `anchor_resolution`, not by observers

**Critical correction.** The v1 plan had the `On<Insert, Anchor>` observer insert `(entity, DuplicateName)` into `warned.set` directly. But `anchor_resolution.set.clear()` at the top of the system wipes that entry before any warn line is emitted — net behavior: silent loss.

**Decision: observers do NOT touch `warned.set`. `anchor_resolution` re-detects duplicates each frame** by scanning `AnchorNameRegistry.by_name`:

```rust
for (_name, bucket) in reg.by_name.iter() {
    if bucket.len() > 1 {
        // The last entry is the most-recent (current winner); earlier
        // entries are shadowed. The late inserter is the warn target.
        if let Some(&(late_entity, _)) = bucket.last() {
            new_warns.push((late_entity, AnchorErrorKind::DuplicateName));
        }
    }
}
```

This is O(named entries) per frame — small. Warns persist as long as the duplicate state persists. The `by_name` map is private; `anchor_resolution` accesses it via a new pub-crate accessor `AnchorNameRegistry::iter_buckets(&self) -> impl Iterator<Item = (&str, &[(Entity, u64)])>`.

### D12. Observer registration uses CLOSURES, not named `fn` items

`On<'w, 't, E, B>` has two lifetime parameters without defaults. Bevy 0.18's lifetime elision in named-`fn` signatures handles single-lifetime structs cleanly (e.g., `fn foo(x: &Bar)`) but is subtle for multi-lifetime structs. Closures inherit lifetimes from `add_observer`'s `IntoObserverSystem<E, B, M>` impl and compile cleanly without explicit annotations. Phase 6 registers all three observers as closures:

```rust
// In LayoutPlugin::build:
app.add_observer(|trigger: On<Insert, Anchor>, q: Query<&Anchor>, mut reg: ResMut<AnchorNameRegistry>| {
    systems::handle_anchor_insert(trigger.event().entity, &q, &mut reg);
});
app.add_observer(|trigger: On<Replace, Anchor>, mut reg: ResMut<AnchorNameRegistry>| {
    reg.remove(trigger.event().entity);
});
app.add_observer(|trigger: On<Remove, Anchor>, mut reg: ResMut<AnchorNameRegistry>| {
    reg.remove(trigger.event().entity);
});
```

The closure bodies forward to private helpers in `systems.rs` for testability — `handle_anchor_insert(entity, &q, &mut reg)`. Tests directly invoke the helper to verify registry mutations.

---

## Deferred from Phase 6 (explicit divergences from spec)

| Feature | Spec location | Why deferred |
|---|---|---|
| `anchor-size()` in `PositionTry::inset` | display-and-positioning.md § 3.4 line 231 | "tier-C feature deferred to v1.x" per spec. Phase 6 implementation: when an inset contains an anchor-size term (Phase 6 doesn't introduce one yet, but the type system allows extension via a future `Length::AnchorSize(_)`), `length_to_px_with_anchor` returns `0.0` and emits a `warn!` via `AnchorErrorKind::AnchorSizeUsed`. Phase 6 ships the `AnchorErrorKind::AnchorSizeUsed` variant with a stub returning 0 so the future `Length::AnchorSize` extension lands without churn. |
| `position_try_max_depth` resource cap | README.md § 5 + display-and-positioning.md § 3.5 | "Open: whether to cap depth via a `position_try_max_depth` resource if profiling surfaces deeply-nested fallback hot paths." Phase 6 evaluates the full chain linearly; no cap. Tracked in `docs/plans/follow-ups.md` (which will be created in T12 if it doesn't already exist; it was created during Phase 5). |
| Cross-window anchor cycles | display-and-positioning.md § 3.4 + README.md § 5 | Spec is silent on cross-window anchors. Phase 6 implementation: `AnchorRef::Entity(e)` resolves only if `e`'s `ResolvedLayout` exists; cross-window entities have separate layout trees, and (since the anchored entity's parent must share the layout tree) cross-window anchors emit `AnchorErrorKind::TargetMissing` and broken. Tracked: follow-ups.md. |
| Performance: Kahn re-runs on K cycles | D4 above | Bounded but unoptimized. Profiling-driven optimization deferred. Tracked: follow-ups.md. |
| Anchor target IS itself a sticky/table/multicol entity | architecture.md § 3 (sub-passes 6a-6c) | Phase 6 reads anchor target position from `tree.tree.layout()`, which is Taffy's *pre-correction* position. If the target is sticky-displaced (6a), packed by multi-column layout (6c), or repositioned by the table algorithm (6b), the anchor pass uses the un-corrected position. Phase 6 does not detect this — when 6a/6b/6c land in Phase 7+, they must store their corrections in a side-channel that anchor_resolution can consult, OR anchor_resolution must move to run AFTER all 6a-6c corrections are applied. Tracked: follow-ups.md. |
| `LayoutAnchorBroken` on a non-`Anchor` target | spec § 3.4 line 229 ("both endpoints") | Spec mandates both cycle endpoints get the marker even if the target is a plain `Node`. Phase 6 implements this; the marker on a non-`Anchor` entity has no behavioral side-effect today but exists for devtools surfacing. |
| Sub-pass ordering within `PostTaffyOverrides` | architecture.md § 3 (lines 153-162 declare 6a→6b→6c→6d order) | Phase 6 attaches `anchor_resolution` as the sole system in `PostTaffyOverrides`, so no intra-set ordering is needed. Future phases (Phase 7 — sticky 6a, table 6b, multicol 6c) must add `.before(anchor_resolution)` constraints to preserve the declared 6a→6b→6c→6d order. The plan's Task 8 includes a *forward-looking comment* in `mod.rs` noting this expectation. |
| Steady-state cost framing | architecture.md § 9 line 265 | Anchor pass is O(anchored entities), not O(0). The plan's "O(0) steady-state" claim in self-review applies to `sync_styles` and to the absence of `Changed<ResolvedLayout>` churn caused by anchor pass — NOT to `anchor_resolution`'s own work, which is always-O(anchored) per the architectural cost model. Wording is corrected in the Self-Review section. |

---

## Task list (11 implementation tasks + 1 closeout)

The tasks below are organized so each one is a reviewable chunk with clear file boundaries. Each ends with a `git commit`. Run `cargo test --workspace` after each task's final step.

---

### Task 1: Add anchor-positioning types

**Files:**
- Modify: `crates/buiy_core/src/layout/types.rs:980` (append at end, before the existing `#[cfg(test)] mod tests {...}` block)

**Context for implementer:**
We're adding the value-type layer of anchor positioning: `AnchorName`, `AnchorRef`, `PositionTry`, `TryCondition`, and `AnchorErrorKind`. These mirror spec § 3.1 lines 161-183 with the `String` payload per Decision D1 (instead of the spec's `SmolStr`). `AnchorErrorKind` is per Decision D5. All types derive `Reflect, Clone, Debug, PartialEq, Default` (where defaultable); `AnchorErrorKind` adds `Copy, Eq, Hash` because it's hashed in `LayoutAnchorWarnedThisFrame`.

- [ ] **Step 1: Write the failing tests** at the bottom of `crates/buiy_core/src/layout/types.rs` inside the existing `#[cfg(test)] mod tests` block:

```rust
#[test]
fn anchor_name_named_round_trips() {
    let n = AnchorName::Named("tooltip-anchor".into());
    let copy = n.clone();
    assert_eq!(n, copy);
}

#[test]
fn anchor_name_implicit_vs_named_are_distinct() {
    assert_ne!(AnchorName::Implicit, AnchorName::Named("x".into()));
}

#[test]
fn anchor_ref_entity_and_name_are_distinct() {
    let e = AnchorRef::Entity(bevy::prelude::Entity::PLACEHOLDER);
    let n = AnchorRef::Name("x".into());
    assert_ne!(e, n);
}

#[test]
fn position_try_default_is_empty() {
    let p = PositionTry::default();
    assert_eq!(p.inset, Inset::default());
    assert!(p.conditions.is_empty());
}

#[test]
fn try_condition_fits_in_container_carries_ref() {
    let c = TryCondition::FitsInContainer(AnchorRef::Name("parent".into()));
    let copy = c.clone();
    assert_eq!(c, copy);
}

#[test]
fn try_condition_variants_are_distinct() {
    assert_ne!(TryCondition::FitsInViewport, TryCondition::AnchorVisible);
}

#[test]
fn anchor_error_kind_hashes_and_compares() {
    use std::collections::HashSet;
    let mut s = HashSet::new();
    s.insert(AnchorErrorKind::TargetMissing);
    s.insert(AnchorErrorKind::AllFallbacksFailed);
    s.insert(AnchorErrorKind::TargetMissing);
    assert_eq!(s.len(), 2);
}
```

- [ ] **Step 2: Run the tests and verify they fail to compile.**

```bash
cargo test -p buiy_core --lib layout::types::tests::anchor 2>&1 | tail -20
```

Expected: compile errors like `cannot find type AnchorName in this scope`.

- [ ] **Step 3: Add the types.** Append to `crates/buiy_core/src/layout/types.rs`, *before* the `#[cfg(test)] mod tests {...}` block (look for the existing block; insert immediately above it):

```rust
/// CSS anchor name. `Implicit` means "referenced by `Entity` ID alone" —
/// no name lookup, the anchor target is identified directly. `Named(_)`
/// participates in the `AnchorNameRegistry` lookup.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.1.
///
/// Spec uses `SmolStr` for the named payload; Phase 6 follows the
/// Phase 3 `GridAreas` precedent and uses `String` to avoid a new direct
/// dep (`crates/buiy_core/src/layout/types.rs:394`).
#[derive(Reflect, Clone, Debug, PartialEq, Eq, Default)]
pub enum AnchorName {
    #[default]
    Implicit,
    Named(String),
}

/// A reference to an anchor target — either a direct `Entity` handle or
/// a name lookup against the `AnchorNameRegistry`.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.1.
#[derive(Reflect, Clone, Debug, PartialEq, Eq)]
pub enum AnchorRef {
    Entity(bevy::prelude::Entity),
    Name(String),
}

/// One entry in an `Anchor.position_try` fallback chain. The first
/// `PositionTry` whose `conditions` all evaluate true is applied.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.1.
#[derive(Reflect, Clone, Debug, PartialEq, Default)]
pub struct PositionTry {
    /// The offset relative to the anchor's resolved box for this try.
    pub inset: Inset,
    /// All conditions must pass for this try to apply.
    pub conditions: Vec<TryCondition>,
}

/// A condition guarding a `PositionTry`. All conditions on a try must
/// pass simultaneously for the try to apply.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.1.
#[derive(Reflect, Clone, Debug, PartialEq)]
pub enum TryCondition {
    /// The anchored entity's would-be box does not overflow the viewport.
    FitsInViewport,
    /// The anchored entity's would-be box fits inside the referenced
    /// container's resolved box. The container is identified the same
    /// way as `Anchor.position_anchor` — by `Entity` or by registered
    /// name.
    FitsInContainer(AnchorRef),
    /// The anchor's resolved box intersects the viewport.
    AnchorVisible,
}

/// Per-frame anchor-error category for the warn-dedup `HashSet` in
/// `LayoutAnchorWarnedThisFrame`. Spec § 3.2 step 4: "warn fires once
/// per (entity, frame)".
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.2.
#[derive(Reflect, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AnchorErrorKind {
    /// The anchor target was missing, despawned, or carried `Display::None`.
    TargetMissing,
    /// Every `PositionTry` in the chain failed its conditions.
    AllFallbacksFailed,
    /// The entity was in an anchor cycle; its edge was dropped.
    InCycle,
    /// Two entities declared the same `anchor_name`; the later wins.
    /// Reported on the *late* insert. Distinct from spec's "warn once
    /// per (name, frame)" only in that the per-entity gate also avoids
    /// repeat warns if the same entity re-inserts within the same frame.
    DuplicateName,
    /// `anchor-size()` used in a `PositionTry::inset` term. Tier-C
    /// deferred to v1.x; the term resolves to zero with a warn.
    AnchorSizeUsed,
}
```

- [ ] **Step 4: Run the tests, verify they pass.**

```bash
cargo test -p buiy_core --lib layout::types::tests::anchor 2>&1 | tail -10
```

Expected: 7 passing tests in this group.

- [ ] **Step 5: Commit.**

```bash
git add crates/buiy_core/src/layout/types.rs
git commit -m "feat(buiy_core): add Anchor positioning value types

AnchorName, AnchorRef, PositionTry, TryCondition, AnchorErrorKind.
Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.1.

String payload (not SmolStr) per Phase 3 GridAreas precedent
(types.rs:394 — 'avoid a new direct dep')."
```

---

### Task 2: Add `Anchor` component and `LayoutAnchorBroken` marker

**Files:**
- Modify: `crates/buiy_core/src/layout/components.rs` (append after the existing `ContainerQueryInactive` marker — search for `pub struct ContainerQueryInactive`)

**Context for implementer:**
The `Anchor` component is decomposed-only (D2: not folded into `Style` per spec § 2.4). It derives `Component, Reflect, Default, Clone, Debug, PartialEq` to enable the idempotent-insert dance from Phase 5 (a comparison must be possible). Default = `Anchor { anchor_name: None, position_anchor: None, position_try: vec![] }`. `LayoutAnchorBroken` is a unit-struct marker following the Phase 5 ContainerQueryActive/Inactive pattern (`crates/buiy_core/src/layout/components.rs:340-360`).

- [ ] **Step 1: Write the failing tests** at the bottom of `crates/buiy_core/src/layout/components.rs` inside the existing `#[cfg(test)] mod tests` block:

```rust
#[test]
fn anchor_default_is_empty() {
    let a = Anchor::default();
    assert_eq!(a.anchor_name, None);
    assert_eq!(a.position_anchor, None);
    assert!(a.position_try.is_empty());
}

#[test]
fn anchor_full_round_trips_partial_eq() {
    let a = Anchor {
        anchor_name: Some(AnchorName::Named("btn".into())),
        position_anchor: Some(AnchorRef::Name("other".into())),
        position_try: vec![PositionTry::default()],
    };
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn anchor_differs_when_position_try_diverges() {
    let a = Anchor { position_try: vec![PositionTry::default()], ..default() };
    let b = Anchor { position_try: vec![], ..default() };
    assert_ne!(a, b);
}

#[test]
fn layout_anchor_broken_is_unit_marker() {
    let _m = LayoutAnchorBroken;
    // existence + Default suffice; the marker carries no data.
    let _d = LayoutAnchorBroken;
}
```

- [ ] **Step 2: Run the tests and verify they fail to compile.**

```bash
cargo test -p buiy_core --lib layout::components 2>&1 | tail -10
```

Expected: `cannot find type Anchor in this scope`.

- [ ] **Step 3: Add the component + marker.** Append to `crates/buiy_core/src/layout/components.rs` near the bottom (just before `#[cfg(test)] mod tests`):

```rust
/// CSS anchor positioning — declares this entity as an anchor target
/// (via `anchor_name`) and/or anchors this entity TO another (via
/// `position_anchor`). When `position_anchor.is_some()`, the
/// `anchor_resolution` system (sub-pass 6d) overrides this entity's
/// `ResolvedLayout.position` by walking the `position_try` chain and
/// applying the first try whose conditions all pass.
///
/// Decomposed-only by spec § 2.4: not folded into the `Style` Bundle
/// because anchored elements are rare (tooltips, popovers) and each
/// carries a non-trivial `position_try` chain. Spawn alongside `Style`:
///
/// ```ignore
/// commands.spawn((
///     Style::default(),
///     Anchor {
///         position_anchor: Some(AnchorRef::Name("submit-btn".into())),
///         position_try: vec![PositionTry { /* ... */ }],
///         ..default()
///     },
/// ));
/// ```
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.1.
#[derive(Component, Reflect, Default, Clone, Debug, PartialEq)]
#[reflect(Component, Default)]
pub struct Anchor {
    /// Names this entity AS an anchor (so other entities can reference
    /// it via `AnchorRef::Name`). `None` means the entity is not a
    /// named anchor target (but can still be a target via direct
    /// `AnchorRef::Entity(_)` references).
    pub anchor_name: Option<AnchorName>,
    /// Declares that this entity is anchored TO another. `None` means
    /// the entity participates in normal layout. `Some(_)` triggers
    /// the anchor-resolution pass for this entity.
    pub position_anchor: Option<AnchorRef>,
    /// Ordered fallback chain. The first try whose `conditions` all
    /// pass wins; if every try fails, the entity gets a
    /// `LayoutAnchorBroken` marker and `ResolvedLayout.position`
    /// defaults to `(0, 0)`.
    pub position_try: Vec<PositionTry>,
}

/// Devtools marker — present when this entity's anchor resolution
/// failed this frame (target missing, every fallback failed, or in a
/// cycle whose edge was dropped). Idempotent: present iff broken,
/// absent iff resolved. Authors observe `With<LayoutAnchorBroken>` to
/// surface broken anchors in inspectors.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.2 step 4.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct LayoutAnchorBroken;
```

Also confirm the import line at the top of `components.rs` includes the new types from `super::types`. If the existing line is `use super::types::{...}`, add `Anchor*` imports as needed. The new types referenced by `Anchor` are `AnchorName`, `AnchorRef`, `PositionTry`. The implementer should update the `use super::types::{...}` line to include these three.

- [ ] **Step 4: Run the tests, verify they pass.**

```bash
cargo test -p buiy_core --lib layout::components 2>&1 | tail -10
```

Expected: 4 new tests pass alongside existing.

- [ ] **Step 5: Commit.**

```bash
git add crates/buiy_core/src/layout/components.rs
git commit -m "feat(buiy_core): add Anchor component + LayoutAnchorBroken marker

Anchor is decomposed-only (NOT a Style Bundle field) per spec § 2.4:
'anchored elements are typically rare ... the decomposed-only
convention keeps Style's authoring surface focused on the 95% case.'

LayoutAnchorBroken follows the Phase 5 ContainerQueryActive/Inactive
unit-marker pattern."
```

---

### Task 3: Add `AnchorNameRegistry` resource and `AnchorOverrides` resource

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs:50-65` (insert new resources right after `SyncStylesIterCount`)

**Context for implementer:**
Two new `Resource` definitions, both private (`pub(crate)` only what mod.rs needs).

`AnchorNameRegistry` is per Decision D4 / D5: stores `HashMap<String, Vec<(Entity, u64)>>` (named → all entities claiming that name, in insertion order — last is current winner) plus `HashMap<Entity, u64>` (entity → epoch — used by Kahn cycle-resolution to find the most-recently-inserted node) plus a monotonic `u64 next_epoch` counter. Inserts and removes are handled by observers (Task 5). The registry is `pub` because the public test surface needs to construct + inspect it; but the lookup helper `find_entity_by_name(&self, name: &str) -> Option<Entity>` returns the *current winner* (last in the Vec).

`AnchorOverrides` is per Decision D3: `pub by_entity: HashMap<Entity, Vec2>`. Cleared at top of `anchor_resolution`. Consulted by `write_resolved_layout` (Task 9 widens that system's signature to include `Res<AnchorOverrides>`).

`LayoutAnchorWarnedThisFrame` is per Decision D5: `pub set: HashSet<(Entity, AnchorErrorKind)>`. Cleared at top of `anchor_resolution`. Inserted into by both the observer (for `DuplicateName`) and `anchor_resolution` (for the other variants).

- [ ] **Step 1: Write the failing tests** at the bottom of `crates/buiy_core/src/layout/systems.rs` inside the existing `#[cfg(test)] mod tests` block (if there is one; if not, add one):

```rust
#[test]
fn anchor_name_registry_lookup_returns_most_recent() {
    let mut r = AnchorNameRegistry::default();
    let e1 = bevy::prelude::Entity::from_raw(1);
    let e2 = bevy::prelude::Entity::from_raw(2);
    r.insert("foo".into(), e1);
    r.insert("foo".into(), e2);
    assert_eq!(r.find_entity_by_name("foo"), Some(e2));
}

#[test]
fn anchor_name_registry_remove_falls_back_to_prior() {
    let mut r = AnchorNameRegistry::default();
    let e1 = bevy::prelude::Entity::from_raw(1);
    let e2 = bevy::prelude::Entity::from_raw(2);
    r.insert("foo".into(), e1);
    r.insert("foo".into(), e2);
    r.remove(e2);
    assert_eq!(r.find_entity_by_name("foo"), Some(e1));
}

#[test]
fn anchor_name_registry_remove_unknown_is_noop() {
    let mut r = AnchorNameRegistry::default();
    r.remove(bevy::prelude::Entity::from_raw(99)); // does not panic
}

#[test]
fn anchor_name_registry_epoch_monotonic() {
    let mut r = AnchorNameRegistry::default();
    let e1 = bevy::prelude::Entity::from_raw(1);
    let e2 = bevy::prelude::Entity::from_raw(2);
    r.insert("a".into(), e1);
    r.insert("b".into(), e2);
    assert!(r.entity_epoch(e2) > r.entity_epoch(e1));
}

#[test]
fn anchor_overrides_default_empty() {
    let o = AnchorOverrides::default();
    assert!(o.by_entity.is_empty());
}

#[test]
fn layout_anchor_warned_default_empty() {
    let w = LayoutAnchorWarnedThisFrame::default();
    assert!(w.set.is_empty());
}
```

- [ ] **Step 2: Run the tests, verify they fail to compile.**

```bash
cargo test -p buiy_core --lib layout::systems::tests::anchor 2>&1 | tail -10
```

- [ ] **Step 3: Add the resources** right after `pub struct SyncStylesIterCount(pub usize);` (which is around `systems.rs:55-60`):

```rust
/// Phase 6 — anchor-name lookup table maintained by observers on
/// `On<Insert, Anchor>` / `On<Replace, Anchor>` / `On<Remove, Anchor>`.
///
/// Storage:
/// - `by_name`: anchor name → ordered `Vec<(Entity, u64)>`. Last entry
///   is the current winner (spec: "most-recently-inserted wins").
/// - `entity_epochs`: every `Anchor`-bearing entity's monotonic insertion
///   epoch. Used by `anchor_resolution`'s Kahn-cycle-edge-drop algorithm
///   to identify the most-recently-inserted entity in a cycle.
/// - `next_epoch`: monotonic counter bumped on every observer-driven
///   insert. Never decrements.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.1.
#[derive(Resource, Default, Debug)]
pub struct AnchorNameRegistry {
    by_name: std::collections::HashMap<String, Vec<(Entity, u64)>>,
    entity_epochs: std::collections::HashMap<Entity, u64>,
    next_epoch: u64,
}

impl AnchorNameRegistry {
    /// Insert an entity under a name, bumping the epoch. If the same
    /// `(name, entity)` pair already exists, this is a *re*-insert
    /// (e.g. component replaced) — the epoch bumps so the cycle tiebreaker
    /// considers this entry the most recent.
    ///
    /// Use [`track_epoch`] for unnamed anchors — `insert` is for the
    /// named case only.
    pub fn insert(&mut self, name: String, entity: Entity) {
        let epoch = self.bump_epoch_for(entity);
        let bucket = self.by_name.entry(name).or_default();
        bucket.retain(|(e, _)| *e != entity);
        bucket.push((entity, epoch));
    }

    /// Track the entity's insertion epoch without inserting into any
    /// name bucket. Used by the `On<Insert, Anchor>` observer for
    /// `Anchor.anchor_name == None` cases — the entity still needs an
    /// epoch entry (for the Kahn cycle-edge-drop tiebreaker) but should
    /// NOT pollute `by_name` with sentinel buckets.
    pub fn track_epoch(&mut self, entity: Entity) {
        let _ = self.bump_epoch_for(entity);
    }

    fn bump_epoch_for(&mut self, entity: Entity) -> u64 {
        let epoch = self.next_epoch;
        self.next_epoch += 1;
        self.entity_epochs.insert(entity, epoch);
        epoch
    }

    /// Remove every entry for this entity from every name bucket and
    /// from `entity_epochs`. Called on `On<Remove, Anchor>` and
    /// `On<Replace, Anchor>` (the replace path removes then re-inserts
    /// using the new anchor_name).
    pub fn remove(&mut self, entity: Entity) {
        for bucket in self.by_name.values_mut() {
            bucket.retain(|(e, _)| *e != entity);
        }
        // Drop emptied buckets to avoid unbounded growth.
        self.by_name.retain(|_, bucket| !bucket.is_empty());
        self.entity_epochs.remove(&entity);
    }

    /// Most-recently-inserted entity claiming this name (spec § 3.1
    /// last-wins semantics), or `None` if no entity claims it.
    pub fn find_entity_by_name(&self, name: &str) -> Option<Entity> {
        self.by_name.get(name)?.last().map(|(e, _)| *e)
    }

    /// Entity's most-recent insertion epoch. Used by the Kahn
    /// cycle-edge-drop algorithm.
    pub fn entity_epoch(&self, entity: Entity) -> u64 {
        self.entity_epochs.get(&entity).copied().unwrap_or(0)
    }

    /// Iterate `(name, bucket)` pairs for `DuplicateName` detection
    /// (D11). `bucket.len() > 1` means duplicate; the last entry is
    /// the late-inserter / warn target.
    pub(super) fn iter_buckets(&self) -> impl Iterator<Item = (&str, &[(Entity, u64)])> {
        self.by_name.iter().map(|(k, v)| (k.as_str(), v.as_slice()))
    }
}

/// Phase 6 — frame-local map of anchor-resolution position overrides.
/// `anchor_resolution` clears this at the top of each call and populates
/// it for every entity with `Anchor.position_anchor.is_some()`. Step 7
/// (`write_resolved_layout`) consults the map per entity and uses the
/// override position (with size still from `tree.tree.layout()`) when
/// present.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.2.
#[derive(Resource, Default, Debug)]
pub struct AnchorOverrides {
    pub by_entity: std::collections::HashMap<Entity, Vec2>,
}

/// Phase 6 — per-frame warn-dedup set. Cleared at the top of
/// `anchor_resolution` (for the `TargetMissing`, `AllFallbacksFailed`,
/// `InCycle`, `AnchorSizeUsed` kinds) and populated by both observer
/// closures (for the `DuplicateName` kind) and `anchor_resolution`
/// itself. Spec § 3.2 step 4: "warn fires once per (entity, frame)".
#[derive(Resource, Default, Debug)]
pub struct LayoutAnchorWarnedThisFrame {
    pub set: std::collections::HashSet<(Entity, AnchorErrorKind)>,
}
```

Also confirm `AnchorErrorKind` is imported at the top of `systems.rs` — add to the existing `use super::types::{...}` line.

- [ ] **Step 4: Run the tests, verify they pass.**

```bash
cargo test -p buiy_core --lib layout::systems::tests::anchor 2>&1 | tail -15
```

- [ ] **Step 5: Commit.**

```bash
git add crates/buiy_core/src/layout/systems.rs
git commit -m "feat(buiy_core): add AnchorNameRegistry + AnchorOverrides + warn-dedup resources

Three new Resources for Phase 6 anchor positioning:
- AnchorNameRegistry: name → entity with monotonic insertion epochs
  for the Kahn cycle-edge-drop tiebreaker.
- AnchorOverrides: frame-local position-override map consulted by
  write_resolved_layout (step 7).
- LayoutAnchorWarnedThisFrame: per-frame warn-dedup set covering
  (Entity, AnchorErrorKind).

Decisions D3/D5 from the plan: per-frame dedup is new in Phase 6
(Phases 1-5 used per-session AtomicBool gates only)."
```

---

### Task 4: Add registry observer helper functions (registered via closures in T8)

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add the private helper `handle_anchor_insert` plus observer test scaffolding)

**Context for implementer:**
Per Decision D12, observers register as CLOSURES in `mod.rs::LayoutPlugin::build` (not as named `fn` items). The closures forward to a single private helper `handle_anchor_insert(entity: Entity, q: &Query<&Anchor>, reg: &mut AnchorNameRegistry)` for testability. Per Decision D11, observers do NOT touch `LayoutAnchorWarnedThisFrame` — duplicate-name detection happens inside `anchor_resolution` via `reg.iter_buckets()` scanning. Per Decision D9, `Display::None` is detected by `anchor_resolution` via a separate `Query<&Display>` — not by the observer.

Per `bevy_ecs-0.18.1/src/lifecycle.rs`:
- `Insert` — fires every time `insert` is called (first-add OR replace). New value is queryable when the observer fires.
- `Replace` — fires when an existing component is being replaced (immediately before the old value drops). Old value is queryable.
- `Remove` — fires on actual remove (no replacement). Old value is queryable.

Sequence for `commands.entity(e).insert(NewAnchor)` on an entity that already has Anchor:
1. `Replace` fires (old value queryable).
2. Old value dropped, new value installed.
3. `Insert` fires (new value queryable).

Sequence for `commands.entity(e).remove::<Anchor>()`:
1. `Remove` fires (old value queryable).
2. Old value dropped.

**Decision: register 3 closures — `Insert` (add to registry), `Replace` (pre-replace cleanup), `Remove` (post-remove cleanup).** The Insert helper does NOT detect duplicates; that's anchor_resolution's job.

- [ ] **Step 1: Write the failing observer tests** in a *new* `#[cfg(test)] mod observer_tests` at the bottom of `systems.rs`:

```rust
#[cfg(test)]
mod observer_tests {
    use super::*;
    use crate::components::Node;
    use crate::layout::components::Anchor;
    use crate::layout::types::AnchorName;
    use bevy::prelude::*;

    fn app_with_observers() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        app.init_resource::<AnchorNameRegistry>();
        app.init_resource::<LayoutAnchorWarnedThisFrame>();
        app.add_observer(
            |trigger: On<bevy::ecs::lifecycle::Insert, Anchor>,
             q: Query<&Anchor>,
             mut reg: ResMut<AnchorNameRegistry>| {
                super::handle_anchor_insert(trigger.event().entity, &q, &mut reg);
            },
        );
        app.add_observer(
            |trigger: On<bevy::ecs::lifecycle::Replace, Anchor>,
             mut reg: ResMut<AnchorNameRegistry>| {
                reg.remove(trigger.event().entity);
            },
        );
        app.add_observer(
            |trigger: On<bevy::ecs::lifecycle::Remove, Anchor>,
             mut reg: ResMut<AnchorNameRegistry>| {
                reg.remove(trigger.event().entity);
            },
        );
        app
    }

    #[test]
    fn observer_insert_registers_named_anchor() {
        let mut app = app_with_observers();
        let e = app
            .world_mut()
            .spawn(Anchor {
                anchor_name: Some(AnchorName::Named("foo".into())),
                ..default()
            })
            .id();
        // Observers fire synchronously on `spawn`, so the registry
        // reflects the new entry immediately.
        let reg = app.world().resource::<AnchorNameRegistry>();
        assert_eq!(reg.find_entity_by_name("foo"), Some(e));
    }

    #[test]
    fn observer_remove_cleans_registry() {
        let mut app = app_with_observers();
        let e = app
            .world_mut()
            .spawn(Anchor {
                anchor_name: Some(AnchorName::Named("foo".into())),
                ..default()
            })
            .id();
        app.world_mut().entity_mut(e).remove::<Anchor>();
        let reg = app.world().resource::<AnchorNameRegistry>();
        assert_eq!(reg.find_entity_by_name("foo"), None);
    }

    #[test]
    fn observer_replace_removes_then_reinserts() {
        let mut app = app_with_observers();
        let e = app
            .world_mut()
            .spawn(Anchor {
                anchor_name: Some(AnchorName::Named("old".into())),
                ..default()
            })
            .id();
        app.world_mut().entity_mut(e).insert(Anchor {
            anchor_name: Some(AnchorName::Named("new".into())),
            ..default()
        });
        let reg = app.world().resource::<AnchorNameRegistry>();
        assert_eq!(reg.find_entity_by_name("old"), None);
        assert_eq!(reg.find_entity_by_name("new"), Some(e));
    }

    #[test]
    fn observer_anchor_without_name_is_tracked_by_epoch_only() {
        let mut app = app_with_observers();
        let e = app.world_mut().spawn(Anchor::default()).id();
        let reg = app.world().resource::<AnchorNameRegistry>();
        // No named entry — but the entity is in entity_epochs (for
        // cycle-resolution lookups that don't go through `by_name`).
        assert!(reg.entity_epoch(e) > 0);
        // The empty-string bucket should NOT contain the entity.
        // (regression test for the v1 plan's empty-string side-channel).
        assert_eq!(reg.find_entity_by_name(""), None);
    }

    // DuplicateName detection moved to anchor_resolution (D11) — the
    // observer no longer touches LayoutAnchorWarnedThisFrame. Test
    // coverage for duplicate-name warns lives in the integration tests
    // (tests/layout_anchor_positioning.rs).
}
```

- [ ] **Step 2: Run the tests, verify they fail to compile.**

```bash
cargo test -p buiy_core --lib layout::systems::observer_tests 2>&1 | tail -10
```

- [ ] **Step 3: Add the private `handle_anchor_insert` helper** to `systems.rs` (after the resources from Task 3). Note: there is NO `on_anchor_insert` / `on_anchor_replace` / `on_anchor_remove` named function — the observers are closures registered in `mod.rs` (per Decision D12). The `handle_anchor_insert` helper is the testable extracted body for the Insert closure; Replace and Remove are trivial one-liners (`reg.remove(entity)`) and don't need an extracted helper.

```rust
/// Private helper invoked by the `On<Insert, Anchor>` observer closure
/// registered in `LayoutPlugin::build` (D12). Adds the entity to the
/// registry under its `anchor_name` if any; otherwise tracks just the
/// epoch (D11/B2 — no empty-string sentinel bucket).
///
/// Duplicate-name detection is NOT done here; it happens in
/// `anchor_resolution` via `reg.iter_buckets()` (D11). Observers run
/// between frames; clearing `LayoutAnchorWarnedThisFrame` at the top
/// of `anchor_resolution` would otherwise lose any observer-recorded
/// warns.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.1.
pub(super) fn handle_anchor_insert(
    entity: Entity,
    q: &Query<&Anchor>,
    reg: &mut AnchorNameRegistry,
) {
    let Ok(anchor) = q.get(entity) else {
        return; // entity may have been despawned mid-flush
    };
    match &anchor.anchor_name {
        Some(AnchorName::Named(name)) => {
            reg.insert(name.clone(), entity);
        }
        Some(AnchorName::Implicit) | None => {
            // Track the epoch only — D11/B2 — never put unnamed
            // anchors into `by_name` (would pollute the registry
            // and corrupt `find_entity_by_name("")` semantics).
            reg.track_epoch(entity);
        }
    }
}
```

Replace and Remove observers (registered in T8) call `reg.remove(entity)` directly — no helper needed.

- [ ] **Step 4: Run the tests, verify they pass.**

```bash
cargo test -p buiy_core --lib layout::systems::observer_tests 2>&1 | tail -15
```

- [ ] **Step 5: Commit.**

```bash
git add crates/buiy_core/src/layout/systems.rs
git commit -m "feat(buiy_core): handle_anchor_insert helper for registry maintenance

Observers are registered as closures in LayoutPlugin::build (T8) per
D12 — On<'w,'t,E,B>'s two lifetimes are subtle in named-fn signatures
so closures are the safer pattern.

handle_anchor_insert: bump epoch + add name (or epoch-only via
track_epoch if unnamed, never an empty-string bucket — D11/B2).
Replace + Remove observers are one-line reg.remove(entity) closures
with no extracted helper.

DuplicateName detection moved to anchor_resolution (D11) — observers
no longer touch LayoutAnchorWarnedThisFrame because the warned set
is cleared at the top of anchor_resolution and observers run between
frames."
```

---

### Task 5: Kahn topological sort helper + cycle-edge-drop

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add helpers after Task 4's observers, before `anchor_resolution` in Task 6)

**Context for implementer:**
Per Decision D4: hand-rolled Kahn, re-run from scratch on cycle. Graph is `HashMap<Entity, Option<Entity>>` (anchored → anchor target; `None` means the entity has no outgoing edge, i.e., it's only an anchor target, not anchored). Edge dropped on cycle: the (cycle-node, anchor_target) edge for the cycle-node with the highest insertion epoch.

The helper returns a `Vec<Entity>` in topological order plus a `HashSet<Entity>` of entities whose edges were dropped (for warn dispatch + LayoutAnchorBroken marker insertion).

Helper signature:
```rust
fn kahn_anchor_sort(
    edges: &HashMap<Entity, Option<Entity>>,
    epochs: &dyn Fn(Entity) -> u64,
) -> (Vec<Entity>, HashSet<Entity>) // (topo_order, edges_dropped_from)
```

- [ ] **Step 1: Write the failing tests** in the existing `#[cfg(test)] mod tests` block of `systems.rs`:

```rust
#[test]
fn kahn_sort_orders_simple_chain() {
    // a → b → c
    let mut edges = std::collections::HashMap::new();
    let a = Entity::from_raw(1);
    let b = Entity::from_raw(2);
    let c = Entity::from_raw(3);
    edges.insert(a, Some(b));
    edges.insert(b, Some(c));
    edges.insert(c, None);
    let (order, dropped) = kahn_anchor_sort(&edges, &|_| 0);
    // anchor targets come BEFORE anchored entities: c, b, a
    let ci = order.iter().position(|&e| e == c).unwrap();
    let bi = order.iter().position(|&e| e == b).unwrap();
    let ai = order.iter().position(|&e| e == a).unwrap();
    assert!(ci < bi);
    assert!(bi < ai);
    assert!(dropped.is_empty());
}

#[test]
fn kahn_sort_breaks_2_node_cycle_at_higher_epoch() {
    // a → b, b → a; epoch(b) > epoch(a)
    let mut edges = std::collections::HashMap::new();
    let a = Entity::from_raw(1);
    let b = Entity::from_raw(2);
    edges.insert(a, Some(b));
    edges.insert(b, Some(a));
    let epochs = move |e: Entity| if e == b { 10 } else { 5 };
    let (order, dropped) = kahn_anchor_sort(&edges, &epochs);
    assert_eq!(dropped.len(), 1);
    assert!(dropped.contains(&b)); // b's edge (b → a) was dropped
    assert_eq!(order.len(), 2);
}

#[test]
fn kahn_sort_breaks_3_node_cycle_at_highest_epoch() {
    // a → b → c → a (cycle); epoch(c) > epoch(b) > epoch(a)
    let mut edges = std::collections::HashMap::new();
    let a = Entity::from_raw(1);
    let b = Entity::from_raw(2);
    let c = Entity::from_raw(3);
    edges.insert(a, Some(b));
    edges.insert(b, Some(c));
    edges.insert(c, Some(a));
    let epochs = move |e: Entity| match e {
        x if x == c => 30,
        x if x == b => 20,
        _ => 10,
    };
    let (order, dropped) = kahn_anchor_sort(&edges, &epochs);
    assert_eq!(dropped.len(), 1);
    assert!(dropped.contains(&c));
    assert_eq!(order.len(), 3);
}

#[test]
fn kahn_sort_handles_two_independent_cycles() {
    // (a → b → a) + (c → d → c); each cycle drops its higher-epoch node
    let mut edges = std::collections::HashMap::new();
    let a = Entity::from_raw(1);
    let b = Entity::from_raw(2);
    let c = Entity::from_raw(3);
    let d = Entity::from_raw(4);
    edges.insert(a, Some(b));
    edges.insert(b, Some(a));
    edges.insert(c, Some(d));
    edges.insert(d, Some(c));
    let epochs = move |e: Entity| match e {
        x if x == b => 20,
        x if x == d => 40,
        _ => 10,
    };
    let (order, dropped) = kahn_anchor_sort(&edges, &epochs);
    assert_eq!(dropped.len(), 2);
    assert!(dropped.contains(&b));
    assert!(dropped.contains(&d));
    assert_eq!(order.len(), 4);
}

#[test]
fn kahn_sort_empty_input_is_empty_output() {
    let edges = std::collections::HashMap::new();
    let (order, dropped) = kahn_anchor_sort(&edges, &|_| 0);
    assert!(order.is_empty());
    assert!(dropped.is_empty());
}

#[test]
fn kahn_sort_only_targets_no_anchored() {
    // a (no outgoing), b (no outgoing) — both should appear, no edges
    let mut edges = std::collections::HashMap::new();
    let a = Entity::from_raw(1);
    let b = Entity::from_raw(2);
    edges.insert(a, None);
    edges.insert(b, None);
    let (order, dropped) = kahn_anchor_sort(&edges, &|_| 0);
    assert_eq!(order.len(), 2);
    assert!(dropped.is_empty());
}

#[test]
fn kahn_sort_external_target_no_anchor_doesnt_loop() {
    // a → b, but b is NOT in edges (it's a plain Node target).
    // D10 pre-pass should add b as `b → None`, Kahn terminates cleanly.
    let mut edges = std::collections::HashMap::new();
    let a = Entity::from_raw(1);
    let b = Entity::from_raw(2);
    edges.insert(a, Some(b));
    // NOT inserting b.
    let (order, dropped) = kahn_anchor_sort(&edges, &|_| 0);
    assert_eq!(order.len(), 2);
    let ai = order.iter().position(|&e| e == a).unwrap();
    let bi = order.iter().position(|&e| e == b).unwrap();
    assert!(bi < ai); // b is the target — comes first
    assert!(dropped.is_empty());
}
```

- [ ] **Step 2: Run the tests, verify they fail to compile.**

```bash
cargo test -p buiy_core --lib layout::systems::tests::kahn 2>&1 | tail -10
```

- [ ] **Step 3: Add the helper** to `systems.rs` (after the observer fns, before `anchor_resolution`):

```rust
/// Kahn topological sort over the (anchored → anchor) DAG. Returns the
/// resolved topological order (anchor targets first, anchored last) and
/// the set of entities whose outgoing edge was dropped to break a cycle.
///
/// On cycle: identifies the remaining cycle-bound nodes (post-Kahn nodes
/// with in_degree > 0), finds the one with the highest insertion epoch
/// via `epochs(entity)`, drops its outgoing edge, and re-runs Kahn from
/// scratch. Repeats until all nodes are placed.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.4.
///
/// `edges`: anchored entity → `Some(anchor_target)` or `None` (entity is
/// only an anchor target, no outgoing edge). Returns: (order, dropped).
fn kahn_anchor_sort(
    edges: &std::collections::HashMap<Entity, Option<Entity>>,
    epochs: &dyn Fn(Entity) -> u64,
) -> (Vec<Entity>, std::collections::HashSet<Entity>) {
    let mut current_edges: std::collections::HashMap<Entity, Option<Entity>> = edges.clone();
    let mut dropped: std::collections::HashSet<Entity> = std::collections::HashSet::new();

    // D10 — pre-pass: ensure every target of a `Some(t)` edge is also a
    // key in `current_edges` (with `None` outgoing). Without this, a
    // target Entity that has no Anchor component (e.g. a plain Node
    // pointed at via AnchorRef::Entity(e)) ends up with in_degree > 0
    // but is never dequeued — Kahn flags it as a cycle node and the
    // edge-drop is a no-op, looping forever. Pre-populating these
    // "external target" nodes gives the algorithm a well-defined
    // termination check.
    let external_targets: Vec<Entity> = current_edges
        .values()
        .filter_map(|t| t.as_ref().copied())
        .filter(|t| !current_edges.contains_key(t))
        .collect();
    for t in external_targets {
        current_edges.insert(t, None);
    }

    loop {
        // Build in_degree map: number of edges ending at each node.
        let mut in_degree: std::collections::HashMap<Entity, usize> = std::collections::HashMap::new();
        for &e in current_edges.keys() {
            in_degree.entry(e).or_insert(0);
        }
        for (_, target) in &current_edges {
            if let Some(t) = target {
                *in_degree.entry(*t).or_insert(0) += 1;
            }
        }

        // Queue of zero-in-degree nodes.
        let mut queue: std::collections::VecDeque<Entity> = in_degree
            .iter()
            .filter(|(_, &d)| d == 0)
            .map(|(e, _)| *e)
            .collect();

        let mut order: Vec<Entity> = Vec::with_capacity(current_edges.len());
        while let Some(n) = queue.pop_front() {
            order.push(n);
            // Decrement in_degree of the node n points at (if any).
            if let Some(Some(target)) = current_edges.get(&n).copied() {
                let d = in_degree.entry(target).or_insert(1);
                *d = d.saturating_sub(1);
                if *d == 0 {
                    queue.push_back(target);
                }
            }
        }

        if order.len() == current_edges.len() {
            return (order, dropped);
        }

        // Cycle detected. Find the remaining cycle-bound nodes
        // (in_degree > 0 at termination), pick the one with the highest
        // epoch, drop its outgoing edge, re-run.
        let cycle_nodes: Vec<Entity> = in_degree
            .iter()
            .filter(|(_, &d)| d > 0)
            .map(|(e, _)| *e)
            .collect();

        if cycle_nodes.is_empty() {
            // Defensive: should not happen if order.len() != edges.len()
            return (order, dropped);
        }

        let &drop_from = cycle_nodes
            .iter()
            .max_by_key(|&&e| epochs(e))
            .expect("cycle_nodes non-empty");

        // Drop the outgoing edge from this node.
        if let Some(entry) = current_edges.get_mut(&drop_from) {
            *entry = None;
        }
        dropped.insert(drop_from);
    }
}
```

- [ ] **Step 4: Run the tests, verify they pass.**

```bash
cargo test -p buiy_core --lib layout::systems::tests::kahn 2>&1 | tail -15
```

Expected: 6 passing tests.

- [ ] **Step 5: Commit.**

```bash
git add crates/buiy_core/src/layout/systems.rs
git commit -m "feat(buiy_core): hand-rolled Kahn sort for Anchor DAG with cycle-edge drop

Hand-rolled (no petgraph dep) — graph is O(anchored entities), typically
small. Cycle handling: identify post-Kahn cycle-bound nodes (in_degree>0),
pick the highest-epoch one, drop its edge, re-run from scratch.

Bounded re-runs: each iteration drops one edge from a finite graph.
Worst case O(V^3) — negligible for V <= 20.

Spec § 3.4: 'edges that would close a cycle are dropped — the dropped
edge is (child anchored, anchor) for the most-recently-inserted
anchored entity in the cycle.'"
```

---

### Task 6: `anchor_resolution` system + helpers

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs` (add after Kahn helper from Task 5)

**Context for implementer:**
The main system. Signature:

```rust
pub(super) fn anchor_resolution(
    mut commands: Commands,
    tree: NonSend<LayoutTree>,
    anchored_query: Query<(Entity, &Anchor, Option<&LayoutAnchorBroken>), With<Node>>,
    reg: Res<AnchorNameRegistry>,
    primary_window: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    mut overrides: ResMut<AnchorOverrides>,
    mut warned: ResMut<LayoutAnchorWarnedThisFrame>,
)
```

Algorithm:
1. Clear `overrides.by_entity` and clear the per-frame variants of `warned.set` (everything except `DuplicateName`, which is owned by the observer and should NOT be cleared by anchor_resolution — the observer alone manages it). **Correction:** all variants are cleared at top — even `DuplicateName` — because the warned set is per-frame and observers run between frames. The observer re-inserts `DuplicateName` if the duplicate persists. The clear-at-top + insert-by-observer + insert-by-resolution pattern keeps the contract simple.
2. Build the edge map: `HashMap<Entity, Option<Entity>>` from `anchored_query`. For each `(entity, anchor, _)`:
   - If `anchor.position_anchor.is_some()`, resolve the target Entity: `AnchorRef::Entity(e) → e` or `AnchorRef::Name(n) → reg.find_entity_by_name(&n)`. If the target is unresolvable (None or doesn't exist), insert into the edge map as `entity → None` and record `TargetMissing` in `warned`.
   - If `anchor.position_anchor.is_none()`, the entity is just an anchor target (or unnamed-and-untargeted, harmless). Insert into the edge map as `entity → None`.
3. Run `kahn_anchor_sort`. For each `drop_entity` in `dropped`, record `InCycle` in `warned` and prepare to emit the warn line.
4. Walk the topological order. For each `anchored_entity` (entity whose `Anchor.position_anchor.is_some()`):
   - If `anchored_entity ∈ dropped`: write `Vec2::ZERO` to `overrides`, idempotently insert `LayoutAnchorBroken`, log if not already warned this frame.
   - Else: look up the target's box from `tree.tree.layout(target_taffy_id)` (use `tree.by_entity.get(&target)`). If the target Entity has no Taffy node, fall through to broken (same as `TargetMissing`).
   - With the anchor's box (`location + size`), iterate `position_try` in order: for each `PositionTry`, resolve `inset` into a `Vec2` offset (top/left edges → `(left, top)`, with negative values from `right`/`bottom` representing "place to the right/below the anchor"; Phase 6 uses a simple `try_anchored_position(anchor_box: Rect, inset: &Inset)` helper). Evaluate every `TryCondition`:
     - `FitsInViewport`: anchored box rect must be entirely within viewport rect (window primary).
     - `FitsInContainer(ref)`: resolve `ref` to an Entity, look up its taffy box, check containment.
     - `AnchorVisible`: anchor's box must intersect the viewport rect.
   - First passing try wins: write its position to `overrides`, idempotently remove `LayoutAnchorBroken`.
   - If no try passes: write `Vec2::ZERO` to `overrides`, idempotently insert `LayoutAnchorBroken`, record `AllFallbacksFailed`.
5. Emit warns at the end (one log line per unique `(entity, kind)` in `warned.set` for the variants this frame added — but only if they weren't already in `warned.set` *before* this system started). Implementation: track newly-added warns via a local `Vec<(Entity, AnchorErrorKind)>` and emit `warn!` per entry.

Helper: `try_anchored_position(anchor_box: TaffyLayoutBox, inset: &Inset, viewport: Vec2) -> Vec2` — pure function computing the anchored entity's would-be position from the anchor's resolved box and the try's inset. Phase 6 uses the spec's simple semantics: `inset.top` is the offset from the anchor's bottom edge downward (i.e., "below the anchor by this much"); `inset.bottom` similarly from the top edge upward (above); `inset.left` from the right edge rightward (to the right of the anchor); `inset.right` from the left edge leftward (to the left of the anchor). When two opposing edges are both set, the implementation picks the side opposite the "above/below/left/right" inset that's non-zero; when both are zero, the anchored position equals the anchor's top-left.

Actually, to match CSS anchor positioning semantics more cleanly, Phase 6 uses the convention: `inset` is interpreted *relative to the anchor's box*, with `top`/`right`/`bottom`/`left` being absolute offsets from the corresponding anchor-box edge. A try with `inset.top = Sizing::Px(8.0)` means "8px above the anchor's top edge". This is the convention shown in the spec's authoring example (display-and-positioning.md § 3.3 uses `Inset::above(Length::px(8.0))` — so Phase 6 will need `Inset::above`, `Inset::below`, `Inset::left_of`, `Inset::right_of` convenience constructors as part of this task).

**Convenience constructors on `Inset`** (add to `types.rs` as part of this task):

```rust
impl Inset {
    /// Above the anchor: anchored box's bottom edge is `dist` above the anchor's top.
    pub fn above(dist: Length) -> Self { Self { bottom: Sizing::Length(dist), ..default() } }
    /// Below the anchor: anchored box's top edge is `dist` below the anchor's bottom.
    pub fn below(dist: Length) -> Self { Self { top: Sizing::Length(dist), ..default() } }
    /// Left of the anchor: anchored box's right edge is `dist` left of the anchor's left.
    pub fn left_of(dist: Length) -> Self { Self { right: Sizing::Length(dist), ..default() } }
    /// Right of the anchor: anchored box's left edge is `dist` right of the anchor's right.
    pub fn right_of(dist: Length) -> Self { Self { left: Sizing::Length(dist), ..default() } }
}
```

And the position-from-inset helper:

```rust
/// Compute the anchored entity's would-be top-left from the anchor's
/// resolved box and the try's inset.
///
/// Convention: `inset` is interpreted relative to the anchor's box edges.
/// - `inset.top != 0`: place anchored entity BELOW anchor (anchored.top = anchor.bottom + top).
/// - `inset.bottom != 0`: place anchored entity ABOVE anchor (anchored.bottom = anchor.top - bottom).
/// - `inset.left != 0`: place anchored entity to the RIGHT of anchor (anchored.left = anchor.right + left).
/// - `inset.right != 0`: place anchored entity to the LEFT of anchor (anchored.right = anchor.left - right).
///
/// When `top == bottom == 0`, anchored.top = anchor.top (vertically aligned).
/// When `left == right == 0`, anchored.left = anchor.left (horizontally aligned).
///
/// Sizing::Auto → 0.0 (no offset). Sizing::Length(_) → resolve via length_to_px.
fn try_anchored_position(
    anchor_pos: Vec2,
    anchor_size: Vec2,
    anchored_size: Vec2,
    inset: &Inset,
    viewport: Vec2,
) -> Vec2 {
    let to_px = |s: &Sizing, axis: f32| -> f32 {
        match s {
            Sizing::Auto => 0.0,
            Sizing::Length(l) => length_inset_to_px(*l, axis, viewport),
            // B4: FitContent is a tuple variant FitContent(Length); the
            // wildcard `(_)` discards the inner Length (no fit-content
            // semantics in inset position resolution).
            Sizing::MinContent | Sizing::MaxContent | Sizing::FitContent(_) => 0.0,
        }
    };
    let top = to_px(&inset.top, anchor_size.y);
    let bottom = to_px(&inset.bottom, anchor_size.y);
    let left = to_px(&inset.left, anchor_size.x);
    let right = to_px(&inset.right, anchor_size.x);

    let x = if right > 0.0 {
        anchor_pos.x - right - anchored_size.x
    } else if left > 0.0 {
        anchor_pos.x + anchor_size.x + left
    } else {
        anchor_pos.x
    };
    let y = if bottom > 0.0 {
        anchor_pos.y - bottom - anchored_size.y
    } else if top > 0.0 {
        anchor_pos.y + anchor_size.y + top
    } else {
        anchor_pos.y
    };
    Vec2::new(x, y)
}

/// Resolve a `Length` for inset use. `Px` → its value; `Percent` → percent
/// of the relevant axis. `Fr` → 0 (warn-once). `Cq*` → 0 (warn-once);
/// container units in inset are tier-C deferred and tracked in follow-ups.
fn length_inset_to_px(l: Length, axis: f32, _viewport: Vec2) -> f32 {
    match l {
        Length::Px(v) => v,
        Length::Percent(p) => axis * (p / 100.0),
        Length::Fr(_) => 0.0,
        Length::Cqw(_) | Length::Cqh(_) | Length::Cqi(_) |
        Length::Cqb(_) | Length::Cqmin(_) | Length::Cqmax(_) => 0.0,
    }
}
```

The cycle-warn line for `InCycle` is per-cycle per-frame: collect each unique cycle (set of nodes), pick the dropped entity to name in the message, emit once per dropped-entity-as-key. Since `kahn_anchor_sort` returns the set of dropped entities (one per cycle), iterate the set and emit one warn per dropped entity. This satisfies "one warn per cycle per frame".

- [ ] **Step 1: Write failing integration test** in `crates/buiy_core/tests/layout_anchor_positioning.rs` (create the file):

```rust
//! Phase 6 integration: anchor-resolution end-to-end.
//!
//! Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3 + § 4.

use bevy::prelude::*;
use buiy_core::components::{Node, ResolvedLayout};
use buiy_core::layout::{
    Anchor, AnchorName, AnchorRef, BoxModel, Inset, LayoutAnchorBroken,
    LayoutPlugin, Length, PositionTry, Sizing, Style, TryCondition,
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins).add_plugins(LayoutPlugin);
    app
}

#[test]
fn anchor_basic_positions_below_anchor() {
    let mut app = app();
    // Anchor: 100x50 at (50, 50)
    let anchor = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(100.0).height_px(50.0),
            Anchor {
                anchor_name: Some(AnchorName::Named("btn".into())),
                ..default()
            },
        ))
        .id();
    let _ = anchor; // anchor referenced by name; the registry resolves it

    // Anchored: 80x20, placed 10px below the anchor.
    let anchored = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(80.0).height_px(20.0),
            Anchor {
                position_anchor: Some(AnchorRef::Name("btn".into())),
                position_try: vec![PositionTry {
                    inset: Inset::below(Length::Px(10.0)),
                    conditions: vec![TryCondition::FitsInViewport],
                }],
                ..default()
            },
        ))
        .id();

    // Run a couple of frames to let Taffy resolve sizes + anchor pass apply.
    app.update();
    app.update();

    let anchored_rl = app.world().get::<ResolvedLayout>(anchored).unwrap();
    // anchored's y should be at anchor.y + anchor.size.y + 10
    // (anchor at default 0,0 in Taffy with size 100,50 → anchored at y=60)
    assert_eq!(anchored_rl.position.y, 60.0);
}
```

(Don't add the other 6 integration tests yet — those land in Task 11. This task just needs one end-to-end test to drive `anchor_resolution`'s implementation.)

- [ ] **Step 2: Wire up the system in mod.rs** *just enough to make this test compile*. Add (Task 8 will refine this):

```rust
// in crates/buiy_core/src/layout/mod.rs build():
app.init_resource::<systems::AnchorNameRegistry>();
app.init_resource::<systems::AnchorOverrides>();
app.init_resource::<systems::LayoutAnchorWarnedThisFrame>();

// Observers are registered as CLOSURES per D12 — On<'w,'t,E,B>'s
// two lifetimes are subtle in named-fn signatures.
app.add_observer(
    |trigger: On<bevy::ecs::lifecycle::Insert, Anchor>,
     q: Query<&Anchor>,
     mut reg: ResMut<systems::AnchorNameRegistry>| {
        systems::handle_anchor_insert(trigger.event().entity, &q, &mut reg);
    },
);
app.add_observer(
    |trigger: On<bevy::ecs::lifecycle::Replace, Anchor>,
     mut reg: ResMut<systems::AnchorNameRegistry>| {
        reg.remove(trigger.event().entity);
    },
);
app.add_observer(
    |trigger: On<bevy::ecs::lifecycle::Remove, Anchor>,
     mut reg: ResMut<systems::AnchorNameRegistry>| {
        reg.remove(trigger.event().entity);
    },
);

app.add_systems(
    Update,
    systems::anchor_resolution.in_set(BuiyLayoutStep::PostTaffyOverrides),
);

// also re-export new types:
pub use components::{ /* ... existing ... */, Anchor, LayoutAnchorBroken};
pub use types::{ /* ... existing ... */, AnchorName, AnchorRef, PositionTry, TryCondition, AnchorErrorKind};
pub use systems::{ /* ... existing ... */, AnchorNameRegistry, AnchorOverrides, LayoutAnchorWarnedThisFrame};
```

(Task 8 will finalize this; this task adds it minimally to allow the test to run.)

- [ ] **Step 3: Run the test, verify it fails (no anchor_resolution system).**

```bash
cargo test -p buiy_core --test layout_anchor_positioning anchor_basic_positions_below_anchor 2>&1 | tail -15
```

Expected: fail (either compile or runtime — `assert_eq!(60.0, 0.0)` if the anchor pass is a no-op).

- [ ] **Step 4: Implement `anchor_resolution` + `Inset::above/below/left_of/right_of` + helpers.**

In `types.rs`, add the four `Inset::above/below/left_of/right_of` constructors (placement: right after the existing `Inset` impl block).

In `systems.rs`, add the system + helpers. Skeleton:

```rust
/// Step 6 sub-pass 6d — anchor resolution.
///
/// Spec: docs/specs/2026-05-08-buiy-layout-design/display-and-positioning.md § 3.2 + § 3.4.
pub(super) fn anchor_resolution(
    mut commands: Commands,
    tree: NonSend<LayoutTree>,
    anchored_query: Query<(Entity, &Anchor, Option<&LayoutAnchorBroken>), With<Node>>,
    display_query: Query<&Display>,
    broken_query: Query<(Entity, Option<&LayoutAnchorBroken>)>,
    reg: Res<AnchorNameRegistry>,
    primary_window: Query<&bevy::window::Window, With<bevy::window::PrimaryWindow>>,
    mut overrides: ResMut<AnchorOverrides>,
    mut warned: ResMut<LayoutAnchorWarnedThisFrame>,
) {
    // 1. Clear frame-local state. Observers do NOT contribute to
    // `warned.set` (D11) — they only update the registry. Duplicates
    // are re-detected from the registry below.
    overrides.by_entity.clear();
    warned.set.clear();

    let viewport = primary_window
        .single()
        .ok()
        .map(|w| Vec2::new(w.resolution.width(), w.resolution.height()))
        .unwrap_or(Vec2::ZERO);

    // 2. Build edge map. The Kahn helper does its own pre-pass for
    // external target nodes (D10), so we don't need to insert plain-Node
    // targets here.
    let mut edges: std::collections::HashMap<Entity, Option<Entity>> =
        std::collections::HashMap::new();
    let mut new_warns: Vec<(Entity, AnchorErrorKind)> = Vec::new();
    // Helper: target resolution honoring Display::None (D9). Returns
    // Some(entity) only when the target is name-resolvable AND not
    // Display::None.
    let resolve_target = |r: &AnchorRef| -> Option<Entity> {
        let candidate = match r {
            AnchorRef::Entity(t) => Some(*t),
            AnchorRef::Name(n) => reg.find_entity_by_name(n),
        }?;
        if let Ok(Display::None) = display_query.get(candidate) {
            return None;
        }
        Some(candidate)
    };
    for (e, anchor, _) in anchored_query.iter() {
        let target = anchor.position_anchor.as_ref().and_then(&resolve_target);
        edges.insert(e, target);
        if anchor.position_anchor.is_some() && target.is_none() {
            new_warns.push((e, AnchorErrorKind::TargetMissing));
        }
    }

    // 3. Kahn sort. The helper handles external target pre-pass and
    // cycle-edge dropping.
    let entity_epochs_fn = |e: Entity| reg.entity_epoch(e);
    let (order, dropped) = kahn_anchor_sort(&edges, &entity_epochs_fn);
    // D8 — both endpoints of a dropped cycle edge get LayoutAnchorBroken.
    // dropped_targets: the target Entity at the other end of each
    // dropped edge (read from the pre-drop edges map).
    let mut dropped_targets: std::collections::HashSet<Entity> =
        std::collections::HashSet::new();
    for d in &dropped {
        new_warns.push((*d, AnchorErrorKind::InCycle));
        if let Some(Some(target)) = edges.get(d).copied() {
            dropped_targets.insert(target);
        }
    }

    // 4. DuplicateName detection (D11). Scan registry buckets; bucket.len > 1
    // means duplicate; the last entry is the late-inserter / warn target.
    for (_name, bucket) in reg.iter_buckets() {
        if bucket.len() > 1 {
            if let Some(&(late_entity, _)) = bucket.last() {
                new_warns.push((late_entity, AnchorErrorKind::DuplicateName));
            }
        }
    }

    // 5. Walk topological order. Resolve position-try chain per entity.
    let mut broken_set: std::collections::HashSet<Entity> = std::collections::HashSet::new();
    // Cycle endpoints are broken regardless of try-chain outcome.
    for d in &dropped { broken_set.insert(*d); }
    for t in &dropped_targets { broken_set.insert(*t); }

    for &e in &order {
        let anchored = anchored_query.get(e).ok();
        let Some((_, anchor, _existing_broken)) = anchored else { continue };
        let Some(_) = anchor.position_anchor.as_ref() else { continue };

        if dropped.contains(&e) {
            overrides.by_entity.insert(e, Vec2::ZERO);
            // broken_set already contains e.
            continue;
        }

        let target = edges.get(&e).copied().flatten();
        let Some(target_entity) = target else {
            overrides.by_entity.insert(e, Vec2::ZERO);
            broken_set.insert(e);
            continue;
        };

        // Read anchor target's box from Taffy.
        let Some(target_taffy) = tree.by_entity.get(&target_entity).copied() else {
            overrides.by_entity.insert(e, Vec2::ZERO);
            broken_set.insert(e);
            new_warns.push((e, AnchorErrorKind::TargetMissing));
            continue;
        };
        let Ok(target_layout) = tree.tree.layout(target_taffy) else {
            overrides.by_entity.insert(e, Vec2::ZERO);
            broken_set.insert(e);
            new_warns.push((e, AnchorErrorKind::TargetMissing));
            continue;
        };
        let anchor_pos = Vec2::new(target_layout.location.x, target_layout.location.y);
        let anchor_size = Vec2::new(target_layout.size.width, target_layout.size.height);

        // Anchored entity's own size (from Taffy).
        let anchored_size = tree.by_entity.get(&e)
            .copied()
            .and_then(|id| tree.tree.layout(id).ok())
            .map(|l| Vec2::new(l.size.width, l.size.height))
            .unwrap_or(Vec2::ZERO);

        // Iterate position_try; first passing wins.
        let mut resolved_position: Option<Vec2> = None;
        for try_ in &anchor.position_try {
            let candidate = try_anchored_position(anchor_pos, anchor_size, anchored_size, &try_.inset, viewport);
            let candidate_rect = (candidate, anchored_size);
            let anchor_rect = (anchor_pos, anchor_size);
            if try_conditions_pass(&try_.conditions, candidate_rect, anchor_rect, viewport, &tree, &reg, &display_query) {
                resolved_position = Some(candidate);
                break;
            }
        }

        match resolved_position {
            Some(pos) => {
                overrides.by_entity.insert(e, pos);
                // broken_set does NOT contain e — write_resolved_layout's
                // idempotent remove fires.
            }
            None => {
                overrides.by_entity.insert(e, Vec2::ZERO);
                broken_set.insert(e);
                new_warns.push((e, AnchorErrorKind::AllFallbacksFailed));
            }
        }
    }

    // 6. Idempotent LayoutAnchorBroken marker management. Iterate over
    // EVERY entity that could currently have or need the marker —
    // anchored entities (anchored_query) AND dropped_targets (which may
    // be plain Nodes without Anchor). Use broken_query to read the
    // current marker state for the non-anchored set.
    for (e, _, existing_broken) in anchored_query.iter() {
        let is_broken = broken_set.contains(&e);
        if is_broken && existing_broken.is_none() {
            commands.entity(e).insert(LayoutAnchorBroken);
        } else if !is_broken && existing_broken.is_some() {
            commands.entity(e).remove::<LayoutAnchorBroken>();
        }
    }
    // Also handle plain-Node targets in dropped_targets (they may not
    // be in anchored_query but still need the marker per D8).
    for &t in &dropped_targets {
        if let Ok((_, existing_broken)) = broken_query.get(t) {
            if existing_broken.is_none() {
                commands.entity(t).insert(LayoutAnchorBroken);
            }
        }
    }
    // Cleanup: remove LayoutAnchorBroken from entities NOT in broken_set
    // but currently carrying the marker AND not in anchored_query
    // (anchored_query case handled above). This covers the case where
    // a previously cycle-broken plain-Node target becomes un-broken.
    for (t, existing_broken) in broken_query.iter() {
        if existing_broken.is_some()
            && !broken_set.contains(&t)
            && anchored_query.get(t).is_err()
        {
            commands.entity(t).remove::<LayoutAnchorBroken>();
        }
    }

    // 7. Emit warns (one per unique (entity, kind) per frame).
    for (entity, kind) in new_warns {
        if warned.set.insert((entity, kind)) {
            match kind {
                AnchorErrorKind::TargetMissing => {
                    warn!(?entity, "buiy: anchor target missing or has Display::None");
                }
                AnchorErrorKind::AllFallbacksFailed => {
                    warn!(?entity, "buiy: every position_try fallback failed");
                }
                AnchorErrorKind::InCycle => {
                    warn!(?entity, "buiy: anchor cycle detected; dropped this entity's outgoing edge (both cycle endpoints marked LayoutAnchorBroken)");
                }
                AnchorErrorKind::DuplicateName => {
                    warn!(?entity, "buiy: duplicate anchor_name — late inserter wins, shadowed entries lose name lookup");
                }
                AnchorErrorKind::AnchorSizeUsed => {
                    warn!(?entity, "buiy: anchor-size() in PositionTry::inset is deferred to v1.x; resolving to 0");
                }
            }
        }
    }
}

fn try_conditions_pass(
    conditions: &[TryCondition],
    anchored_rect: (Vec2, Vec2), // (pos, size)
    anchor_rect: (Vec2, Vec2),
    viewport: Vec2,
    tree: &LayoutTree,
    reg: &AnchorNameRegistry,
    display_query: &Query<&Display>,
) -> bool {
    conditions.iter().all(|c| match c {
        TryCondition::FitsInViewport => {
            let (pos, size) = anchored_rect;
            pos.x >= 0.0 && pos.y >= 0.0
                && pos.x + size.x <= viewport.x
                && pos.y + size.y <= viewport.y
        }
        TryCondition::FitsInContainer(r) => {
            let container = match r {
                AnchorRef::Entity(e) => Some(*e),
                AnchorRef::Name(n) => reg.find_entity_by_name(n),
            };
            let Some(c) = container else { return false };
            // D9 — Display::None containers fail the condition.
            if let Ok(Display::None) = display_query.get(c) { return false }
            let Some(taffy) = tree.by_entity.get(&c).copied() else { return false };
            let Ok(layout) = tree.tree.layout(taffy) else { return false };
            let cpos = Vec2::new(layout.location.x, layout.location.y);
            let csize = Vec2::new(layout.size.width, layout.size.height);
            let (apos, asize) = anchored_rect;
            apos.x >= cpos.x && apos.y >= cpos.y
                && apos.x + asize.x <= cpos.x + csize.x
                && apos.y + asize.y <= cpos.y + csize.y
        }
        TryCondition::AnchorVisible => {
            let (pos, size) = anchor_rect;
            // Intersection of anchor rect with viewport rect (0,0,viewport.x,viewport.y).
            pos.x + size.x > 0.0 && pos.y + size.y > 0.0
                && pos.x < viewport.x && pos.y < viewport.y
        }
    })
}
```

- [ ] **Step 5: Modify `write_resolved_layout`** to consult `AnchorOverrides`. This is part of Task 9 in full, but the integration test needs at least the read path. For this task, add a one-line lookup in `write_resolved_layout`:

```rust
// In write_resolved_layout (systems.rs:471-495), modify the construction of `new`:
let position = overrides.by_entity.get(&entity)
    .copied()
    .unwrap_or_else(|| Vec2::new(layout.location.x, layout.location.y));
let new = ResolvedLayout {
    position,
    size: Vec2::new(layout.size.width, layout.size.height),
};
```

And add `overrides: Res<AnchorOverrides>` to `write_resolved_layout`'s signature.

- [ ] **Step 6: Run the integration test, verify it passes.**

```bash
cargo test -p buiy_core --test layout_anchor_positioning 2>&1 | tail -15
```

- [ ] **Step 7: Commit.**

```bash
git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/src/layout/types.rs crates/buiy_core/src/layout/mod.rs crates/buiy_core/tests/layout_anchor_positioning.rs
git commit -m "feat(buiy_core): anchor_resolution system + Inset convenience constructors

Sub-pass 6d of BuiyLayoutStep::PostTaffyOverrides (spec architecture.md § 3).

Algorithm: clear frame-local state -> build (anchored->anchor) edge map
-> Kahn topological sort with cycle-edge drop -> for each anchored entity
in topo order, evaluate position_try chain against this frame's Taffy
output (tree.tree.layout) and viewport -> write resolved position to
AnchorOverrides resource (or Vec2::ZERO on broken) -> idempotent
LayoutAnchorBroken marker management -> per-frame deduped warn emission.

write_resolved_layout (step 7) consults AnchorOverrides; the override
position wins over Taffy's location when present, size still from Taffy.

Inset gains above/below/left_of/right_of convenience constructors per
spec authoring example (§ 3.3)."
```

---

### Task 7: Widen `write_resolved_layout` to consult `AnchorOverrides`

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs:471-495` (the `write_resolved_layout` function — Task 6 made a minimal change; this task makes it explicit + tested)

**Context for implementer:**
Task 6 added a one-line `overrides.by_entity.get(&entity)` lookup in `write_resolved_layout`. This task adds the comment + targeted test confirming the override semantics. If Task 6 already finalized the change, this task is just the test.

- [ ] **Step 1: Confirm the change from Task 6 is in place.** Read `crates/buiy_core/src/layout/systems.rs:471-495`. The signature should include `overrides: Res<AnchorOverrides>`. The construction of `new` should use `overrides.by_entity.get(&entity).copied().unwrap_or_else(...)`. If not, apply Task 6's Step 5 changes.

- [ ] **Step 2: Add targeted unit test** in `tests/layout_anchor_positioning.rs`:

```rust
#[test]
fn write_resolved_layout_prefers_anchor_override_over_taffy_position() {
    use buiy_core::layout::AnchorOverrides;
    let mut app = app();

    // Spawn an entity with NO Anchor — just a normal layout node.
    let plain = app.world_mut().spawn((Node, Style::default().width_px(50.0).height_px(50.0))).id();

    // Spawn a normal-anchor pair: anchor at (10,10) size 100x100, anchored 5px below.
    let anchor_e = app
        .world_mut()
        .spawn((Node, Style::default().width_px(100.0).height_px(100.0),
                Anchor { anchor_name: Some(AnchorName::Named("a".into())), ..default() }))
        .id();
    let anchored_e = app
        .world_mut()
        .spawn((Node, Style::default().width_px(20.0).height_px(20.0),
                Anchor { position_anchor: Some(AnchorRef::Name("a".into())),
                          position_try: vec![PositionTry {
                              inset: Inset::below(Length::Px(5.0)),
                              conditions: vec![],  // no conditions = always passes
                          }], ..default() }))
        .id();

    app.update();
    app.update();

    // Plain entity: position comes from Taffy.
    let plain_rl = app.world().get::<ResolvedLayout>(plain).unwrap();
    assert!(plain_rl.position.x == 0.0); // first child in root: Taffy places at 0,0

    // Anchored entity: position comes from override (anchor.y + anchor.size.y + 5 = 0 + 100 + 5 = 105)
    let anchored_rl = app.world().get::<ResolvedLayout>(anchored_e).unwrap();
    assert_eq!(anchored_rl.position.y, 105.0);

    // Confirm via AnchorOverrides resource directly.
    let overrides = app.world().resource::<AnchorOverrides>();
    assert!(overrides.by_entity.contains_key(&anchored_e));
    assert!(!overrides.by_entity.contains_key(&plain));
    assert!(!overrides.by_entity.contains_key(&anchor_e)); // anchor target, not anchored
}
```

- [ ] **Step 3: Run the test, verify it passes.**

```bash
cargo test -p buiy_core --test layout_anchor_positioning write_resolved_layout_prefers 2>&1 | tail -10
```

- [ ] **Step 4: Commit.**

```bash
git add crates/buiy_core/tests/layout_anchor_positioning.rs
git commit -m "test(buiy_core): write_resolved_layout prefers AnchorOverrides over Taffy

Phase 6 invariant: anchor override position wins; size still from Taffy."
```

---

### Task 8: Final mod.rs + re-exports + register_type

**Files:**
- Modify: `crates/buiy_core/src/layout/mod.rs` (final integration)
- Modify: `crates/buiy_core/src/lib.rs` (re-exports)
- Modify: `crates/buiy/src/lib.rs` (re-exports — top-level facade crate)

**Context for implementer:**
Task 6 wired the minimal subset to make the basic test pass. This task finalizes mod.rs:
1. All three observers registered via `app.add_observer(...)`.
2. All three resources init.
3. `anchor_resolution` attached in `PostTaffyOverrides`.
4. New types (`Anchor`, `LayoutAnchorBroken`, `AnchorName`, `AnchorRef`, `PositionTry`, `TryCondition`, `AnchorErrorKind`) registered via `register_type::<T>()`.
5. All public types re-exported from `mod.rs` AND from the top-level `crates/buiy_core/src/lib.rs` (so external consumers can `use buiy_core::Anchor` etc.) AND from `crates/buiy/src/lib.rs` (facade crate's re-exports).

- [ ] **Step 1: Update mod.rs.** Open `crates/buiy_core/src/layout/mod.rs`, walk through these touchpoints:

a. **Re-exports.** Add to the `pub use components::{...}`:

```rust
pub use components::{
    Anchor, BoxModel, Container, ContainerQuery, ContainerQueryActive, ContainerQueryInactive,
    Display, FlexItem, FlexParams, GridItem, GridParams, LayoutAnchorBroken, Overflow, Position,
    Scroll, ScrollOffset, ScrollSnapItem, WritingMode, WritingModeResolved,
};
```

Add to the `pub use types::{...}` (Phase 6 types):

```rust
pub use types::{
    AlignContent, AlignItems, AnchorErrorKind, AnchorName, AnchorRef, AspectRatio, BoxSizing,
    ContainerType, Direction, Edges, FlexAxis, FlexGap, FlexWrap, GridAreas, GridAutoFlow,
    GridLine, Inset, JustifyContent, JustifyItems, Length, LogicalEdges, NamedArea, Orientation,
    OverflowMode, OverscrollBehavior, PositionKind, PositionTry, QueryCondition, RepeatCount,
    ScrollBehavior, ScrollbarColor, ScrollbarGutter, ScrollbarWidth, Sizing, SnapAlign, SnapStop,
    SnapType, TextOrientation, TrackSize, TryCondition, UnicodeBidi, WritingModeKind,
};
```

Add to `pub use systems::{...}`:

```rust
pub use systems::{
    AnchorNameRegistry, AnchorOverrides, LayoutAnchorWarnedThisFrame, LayoutTaffyComputeCount,
    SyncStylesIterCount,
};
```

b. **In `LayoutPlugin::build`**, init the three new resources (alongside existing):

```rust
app.init_resource::<systems::CqReRunRequested>();
app.init_resource::<systems::LayoutTaffyComputeCount>();
app.init_resource::<systems::SyncStylesIterCount>();
// Phase 6 — anchor positioning.
app.init_resource::<systems::AnchorNameRegistry>();
app.init_resource::<systems::AnchorOverrides>();
app.init_resource::<systems::LayoutAnchorWarnedThisFrame>();
```

c. **In `LayoutPlugin::build`**, register the three observers (alongside existing systems but BEFORE `add_systems`) per Decision D12:

```rust
// Phase 6 — anchor lifecycle observers. Register before the
// anchor_resolution system so they're live for any Anchor inserts/
// removes that happen during the first frame. Closure form per D12
// (named-fn observer signatures with multi-lifetime `On<'w,'t,E,B>`
// have subtle elision rules; closures avoid the issue).
app.add_observer(
    |trigger: On<bevy::ecs::lifecycle::Insert, Anchor>,
     q: Query<&Anchor>,
     mut reg: ResMut<systems::AnchorNameRegistry>| {
        systems::handle_anchor_insert(trigger.event().entity, &q, &mut reg);
    },
);
app.add_observer(
    |trigger: On<bevy::ecs::lifecycle::Replace, Anchor>,
     mut reg: ResMut<systems::AnchorNameRegistry>| {
        reg.remove(trigger.event().entity);
    },
);
app.add_observer(
    |trigger: On<bevy::ecs::lifecycle::Remove, Anchor>,
     mut reg: ResMut<systems::AnchorNameRegistry>| {
        reg.remove(trigger.event().entity);
    },
);

// Forward note for Phase 7: when sticky / table / multicol sub-passes
// land in PostTaffyOverrides, they must add .before(systems::anchor_resolution)
// to preserve the spec's declared 6a -> 6b -> 6c -> 6d sub-pass order
// (architecture.md § 3 lines 153-162).
```

d. **Register_type for new types** at the end of the existing `register_type` chain in `LayoutPlugin::build`. Add a `// Phase 6 — anchor positioning.` block:

```rust
            // Phase 6 — anchor positioning.
            .register_type::<Anchor>()
            .register_type::<LayoutAnchorBroken>()
            .register_type::<AnchorName>()
            .register_type::<AnchorRef>()
            .register_type::<PositionTry>()
            .register_type::<TryCondition>()
            .register_type::<AnchorErrorKind>();
```

e. **In `add_systems`**, attach `anchor_resolution` in `PostTaffyOverrides`:

```rust
app.add_systems(
    Update,
    (
        systems::gc_removed_nodes.in_set(BuiyLayoutStep::RemovedNodesGc),
        systems::inherit_writing_mode.in_set(BuiyLayoutStep::WritingModeInherit),
        systems::sync_styles.in_set(BuiyLayoutStep::SyncStyles),
        systems::cq_activate.in_set(BuiyLayoutStep::CqActivate),
        systems::taffy_compute.in_set(BuiyLayoutStep::TaffyCompute),
        systems::cq_flip_check.in_set(BuiyLayoutStep::CqFlipCheck),
        systems::cq_flip_rerun.in_set(BuiyLayoutStep::CqFlipReRun),
        systems::anchor_resolution.in_set(BuiyLayoutStep::PostTaffyOverrides),
        systems::write_resolved_layout.in_set(BuiyLayoutStep::WriteResolvedLayout),
    ),
);
```

- [ ] **Step 2: Update `crates/buiy_core/src/lib.rs`** — add re-exports for the new public types. Search for the existing Phase 5 re-export block (likely `pub use crate::layout::{Container, ContainerQuery, ...};` or similar). Add:

```rust
pub use crate::layout::{
    Anchor, AnchorErrorKind, AnchorName, AnchorRef, LayoutAnchorBroken, PositionTry, TryCondition,
};
```

(Place this alongside the existing Phase 5 re-exports.)

- [ ] **Step 3: Update `crates/buiy/src/lib.rs`** — add re-exports for the new public types. The facade crate mirrors `buiy_core` for public types. Search for the existing Phase 5 re-export block. Add:

```rust
pub use buiy_core::{
    Anchor, AnchorErrorKind, AnchorName, AnchorRef, LayoutAnchorBroken, PositionTry, TryCondition,
};
```

- [ ] **Step 4: Run full workspace tests, verify no regressions.**

```bash
cargo test --workspace 2>&1 | tail -30
```

Expected: all Phase 1-5 tests still pass, plus Phase 6 tests.

- [ ] **Step 5: Run `cargo clippy --workspace --all-features -- -D warnings`.**

Expected: no warnings.

- [ ] **Step 6: Commit.**

```bash
git add crates/buiy_core/src/layout/mod.rs crates/buiy_core/src/lib.rs crates/buiy/src/lib.rs
git commit -m "feat(buiy_core, buiy): register and re-export Phase 6 anchor types

LayoutPlugin gains:
- 3 init_resource calls (AnchorNameRegistry, AnchorOverrides,
  LayoutAnchorWarnedThisFrame).
- 3 add_observer calls (on_anchor_insert/replace/remove).
- 7 register_type calls for reflection (Anchor, LayoutAnchorBroken,
  AnchorName, AnchorRef, PositionTry, TryCondition, AnchorErrorKind).
- anchor_resolution attached in BuiyLayoutStep::PostTaffyOverrides.

buiy_core and buiy facade re-export the 7 new public types."
```

---

### Task 9: Widen `sync_styles` Or<> filter to include `Changed<Anchor>`

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs:148-195` (the `Or<>` block in `sync_styles`'s query filter)

**Context for implementer:**
The Phase 5 filter is at 15 outer entries with a nested inner `Or<(4)>` (Container, ContainerQuery, ContainerQueryActive, ContainerQueryInactive). Adding `Changed<Anchor>` and `Changed<LayoutAnchorBroken>` requires either:
- **Rebalancing the inner Or** to fit Anchor + LayoutAnchorBroken (would need an inner-Or with 6 entries, still under 15).
- **Adding a second inner Or** (the outer Or would stay at 15, with two nested inner Ors).

**Decision: rebalance.** The inner Or becomes 6 entries: Container, ContainerQuery, ContainerQueryActive, ContainerQueryInactive, Anchor, LayoutAnchorBroken. The outer Or stays at 15 entries. Rationale: grouping all "marker-style/identity-style" components together (CQ markers + anchor markers) keeps the filter readable.

Actually — `Anchor` isn't a marker; it's a value-bearing component with a `PositionTry` Vec field. But it's still "Phase 5+ extension components" grouped logically. The inner Or grows to 6 to accommodate.

But wait — actually adding to the nested Or is cleaner if it groups by SEMANTIC ROLE not by phase. Let me re-examine. The outer Or has:
- 14 individual `Changed<X>` for core layout components (Display, BoxModel, Position, etc.)
- 1 nested Or for "Phase 5+ extension components"

Phase 6's `Anchor` is a core layout component (not an extension marker). So it could either:
(a) Become a top-level entry (push the outer Or to 16 — overflows Bevy's 15-cap).
(b) Move INTO the nested Or alongside the CQ entries (inner Or grows from 4 to 6, outer Or stays at 15).
(c) Replace an existing top-level entry with a new nested Or.

Option (a) overflows. Option (b) is cleanest. Option (c) is unnecessarily disruptive. **Go with (b).**

`LayoutAnchorBroken` is a marker (idempotent unit struct). It doesn't NEED to be in the filter — `sync_styles` doesn't read it. But adding it means `sync_styles` re-translates an entity when its broken state flips. Since `LayoutAnchorBroken` doesn't affect Taffy translation (it's just a marker for devtools), **omit it from the filter**. Only add `Changed<Anchor>` to the inner Or.

Final inner Or: 5 entries (Container, ContainerQuery, ContainerQueryActive, ContainerQueryInactive, Anchor).

- [ ] **Step 1: Read `systems.rs:148-195`.** Confirm the current filter shape matches the prior-art audit (14 top-level + 1 nested Or<(4)>).

- [ ] **Step 2: Modify the filter.** Change the inner `Or<(4 entries)>` to `Or<(5 entries)>` adding `Changed<Anchor>` as the 5th. The outer Or stays at 15. Update the comment block to mention Phase 6.

- [ ] **Step 3: Write test** confirming `sync_styles` re-runs when `Anchor` changes. Add to `tests/layout_anchor_positioning.rs`:

```rust
#[test]
fn sync_styles_reruns_when_anchor_changes() {
    use buiy_core::layout::SyncStylesIterCount;
    let mut app = app();

    // Spawn one entity with default Style (no Anchor).
    let e = app.world_mut().spawn((Node, Style::default())).id();
    app.update();
    let count_before = app.world().resource::<SyncStylesIterCount>().0;

    // Insert Anchor — Changed<Anchor> fires.
    app.world_mut().entity_mut(e).insert(Anchor {
        anchor_name: Some(AnchorName::Named("x".into())),
        ..default()
    });
    app.update();
    let count_after = app.world().resource::<SyncStylesIterCount>().0;

    // The entity should have been re-translated. After steady-state, the
    // count drops back to zero. SyncStylesIterCount measures THIS frame's
    // matched count, so count_after >= 1 immediately after the Anchor insert.
    assert!(count_after >= 1);

    // After a second update, the entity is no longer Changed; count drops.
    app.update();
    let count_steady = app.world().resource::<SyncStylesIterCount>().0;
    assert_eq!(count_steady, 0);
}
```

- [ ] **Step 4: Run the test.**

```bash
cargo test -p buiy_core --test layout_anchor_positioning sync_styles_reruns_when_anchor 2>&1 | tail -10
```

- [ ] **Step 5: Commit.**

```bash
git add crates/buiy_core/src/layout/systems.rs crates/buiy_core/tests/layout_anchor_positioning.rs
git commit -m "feat(buiy_core): include Changed<Anchor> in sync_styles filter

Inner nested Or grows from 4 to 5 (adds Anchor alongside Container/CQ
entries). Outer Or stays at 15 entries — under the Bevy 0.18 tuple cap.

LayoutAnchorBroken is intentionally omitted from the filter: it's a
devtools marker that doesn't affect Taffy translation."
```

---

### Task 10: Update pipeline-order test with anchor fixture

**Files:**
- Modify: `crates/buiy_core/tests/layout_pipeline_order.rs` (augment the fixture)

**Context for implementer:**
The pipeline-order test asserts the 9-step chain runs in declared order. Phase 5 augmented its fixture with a (Container + ContainerQuery + Cqw descendant) to exercise CqActivate / CqFlipCheck / CqFlipReRun. Phase 6 augments it with an (anchor + anchored) pair so PostTaffyOverrides's anchor_resolution is exercised when the test runs.

- [ ] **Step 1: Read the existing fixture.** It probably spawns one or two entities and runs `app.update()`. Add to the spawn block (right before `app.update()`):

```rust
// Phase 6 — anchor fixture so PostTaffyOverrides has work to do.
let anchor_target = app
    .world_mut()
    .spawn((
        Node,
        Style::default().width_px(50.0).height_px(50.0),
        Anchor {
            anchor_name: Some(AnchorName::Named("test-anchor".into())),
            ..default()
        },
    ))
    .id();
let _ = anchor_target;

let _anchored = app
    .world_mut()
    .spawn((
        Node,
        Style::default().width_px(30.0).height_px(20.0),
        Anchor {
            position_anchor: Some(AnchorRef::Name("test-anchor".into())),
            position_try: vec![PositionTry {
                inset: Inset::below(Length::Px(5.0)),
                conditions: vec![],
            }],
            ..default()
        },
    ))
    .id();
```

Add to the imports at the top of the test file:

```rust
use buiy_core::layout::{Anchor, AnchorName, AnchorRef, Inset, Length, PositionTry};
```

- [ ] **Step 2: Run the augmented test, verify it still passes.**

```bash
cargo test -p buiy_core --test layout_pipeline_order 2>&1 | tail -15
```

The 9-step order assertion should be unchanged (PostTaffyOverrides slot is now populated, not just defined). The test verifies the ORDER, not the content.

- [ ] **Step 3: Commit.**

```bash
git add crates/buiy_core/tests/layout_pipeline_order.rs
git commit -m "test(buiy_core): pipeline-order test exercises anchor sub-pass 6d

Fixture adds an anchor target + anchored entity so PostTaffyOverrides's
anchor_resolution has work each frame. The 9-step order assertion is
unchanged — Phase 6 populates the PostTaffyOverrides slot that Phases 1-5
declared but left empty."
```

---

### Task 11: Anchor integration tests (full coverage)

**Files:**
- Modify: `crates/buiy_core/tests/layout_anchor_positioning.rs` (add the remaining tests)

**Context for implementer:**
The basic anchor test (Task 6) + override-vs-Taffy test (Task 7) + sync_styles rerun (Task 9) already exist. This task adds the remaining 7 tests per spec § 4:

1. Anchor basic — Task 6 already.
2. Anchor fallback chain (first fails, second wins).
3. Anchor cycle detection (A→B, B→A; assert one resolves, one broken, one warn).
4. Anchor missing target (broken + warn).
5. LayoutAnchorBroken marker is removed when resolution succeeds (idempotent).
6. Steady-state O(0): no re-iteration after anchor resolves stably.
7. Anchor observer maintains registry across spawn/despawn.

- [ ] **Step 1: Add the 7 tests** to `tests/layout_anchor_positioning.rs`. Each is described in detail below.

**Test 2: Fallback chain — first fails, second wins.**

```rust
#[test]
fn anchor_fallback_chain_second_wins_when_first_overflows_viewport() {
    let mut app = app();

    // Set viewport to 200x200 implicit via no Window plugin — but tests
    // use MinimalPlugins which has no Window. The primary_window query
    // returns None, viewport = (0,0). For this test, we need a non-zero
    // viewport. Add a minimal window resource.
    // Per Phase 5 prior art (tests/layout_container_queries.rs),
    // synthesize a PrimaryWindow with a manual spawn:
    use bevy::window::{PrimaryWindow, Window, WindowResolution};
    app.world_mut().spawn((
        Window {
            resolution: WindowResolution::new(200.0, 200.0),
            ..default()
        },
        PrimaryWindow,
    ));

    // Anchor near top-right corner: pos (150, 10), size 50x50.
    // To position the anchor near the top-right we wrap it in a flex
    // root that pushes it to (150, 10). Simplest: spawn a parent with
    // padding-left:150 padding-top:10.
    // Actually, simpler: spawn a parent of width:200 height:200 with
    // a single child positioned via flex. But Phase 6 doesn't need to
    // be that complex — we can just place the anchor at a known
    // position via flexbox layout.

    let root = app.world_mut().spawn((Node, Style::default().width_px(200.0).height_px(200.0))).id();
    let anchor = app.world_mut().spawn((
        Node,
        Style::default().width_px(50.0).height_px(50.0),
        Anchor { anchor_name: Some(AnchorName::Named("a".into())), ..default() },
    )).id();
    app.world_mut().entity_mut(root).add_children(&[anchor]);

    // Anchored: prefer ABOVE the anchor (would put it at y = -20, fails
    // FitsInViewport since y < 0). Falls back to BELOW the anchor.
    let anchored = app.world_mut().spawn((
        Node,
        Style::default().width_px(20.0).height_px(20.0),
        Anchor {
            position_anchor: Some(AnchorRef::Name("a".into())),
            position_try: vec![
                PositionTry {
                    inset: Inset::above(Length::Px(10.0)),
                    conditions: vec![TryCondition::FitsInViewport],
                },
                PositionTry {
                    inset: Inset::below(Length::Px(10.0)),
                    conditions: vec![TryCondition::FitsInViewport],
                },
            ],
            ..default()
        },
    )).id();

    app.update();
    app.update();

    let rl = app.world().get::<ResolvedLayout>(anchored).unwrap();
    // The "below" fallback should win: y = anchor.y + anchor.h + 10 = 0 + 50 + 10 = 60.
    assert_eq!(rl.position.y, 60.0);
    // LayoutAnchorBroken should NOT be present.
    assert!(app.world().get::<LayoutAnchorBroken>(anchored).is_none());
}
```

**Test 3: Cycle detection — both endpoints broken (spec § 3.4).**

```rust
#[test]
fn anchor_cycle_marks_both_endpoints_broken() {
    let mut app = app();
    let a = app.world_mut().spawn((Node, Style::default().width_px(50.0).height_px(50.0),
        Anchor { anchor_name: Some(AnchorName::Named("a".into())),
                 position_anchor: Some(AnchorRef::Name("b".into())),
                 position_try: vec![PositionTry { inset: Inset::below(Length::Px(5.0)), conditions: vec![] }],
                 ..default() })).id();
    // b is spawned AFTER a; its epoch is higher.
    let b = app.world_mut().spawn((Node, Style::default().width_px(50.0).height_px(50.0),
        Anchor { anchor_name: Some(AnchorName::Named("b".into())),
                 position_anchor: Some(AnchorRef::Name("a".into())),
                 position_try: vec![PositionTry { inset: Inset::below(Length::Px(5.0)), conditions: vec![] }],
                 ..default() })).id();
    app.update();
    app.update();

    // Spec § 3.4 line 229: "Both endpoints get LayoutAnchorBroken markers."
    // b has the higher epoch — its edge is dropped. Both a (target of
    // dropped edge) and b (source of dropped edge) get the marker.
    assert!(app.world().get::<LayoutAnchorBroken>(b).is_some(),
            "spec § 3.4: cycle source (dropped edge) must be marked");
    assert!(app.world().get::<LayoutAnchorBroken>(a).is_some(),
            "spec § 3.4: cycle target (other endpoint of dropped edge) must be marked");

    // Verify exactly one InCycle warn was emitted (per cycle per frame).
    use buiy_core::layout::{AnchorErrorKind, LayoutAnchorWarnedThisFrame};
    let warned = app.world().resource::<LayoutAnchorWarnedThisFrame>();
    let in_cycle_count = warned.set.iter().filter(|(_, k)| *k == AnchorErrorKind::InCycle).count();
    assert_eq!(in_cycle_count, 1);
}

#[test]
fn anchor_duplicate_name_warns_each_frame_dupe_persists() {
    let mut app = app();
    // First entity claims "dupe".
    let _e1 = app.world_mut().spawn(Anchor {
        anchor_name: Some(AnchorName::Named("dupe".into())),
        ..default()
    }).id();
    // Second entity also claims "dupe" — e2 is the late inserter.
    let e2 = app.world_mut().spawn(Anchor {
        anchor_name: Some(AnchorName::Named("dupe".into())),
        ..default()
    }).id();
    app.update();

    use buiy_core::layout::{AnchorErrorKind, LayoutAnchorWarnedThisFrame};
    let warned = app.world().resource::<LayoutAnchorWarnedThisFrame>();
    assert!(warned.set.contains(&(e2, AnchorErrorKind::DuplicateName)));

    // After a second update, the duplicate persists — warn should still fire.
    app.update();
    let warned = app.world().resource::<LayoutAnchorWarnedThisFrame>();
    assert!(warned.set.contains(&(e2, AnchorErrorKind::DuplicateName)));
}
```

**Test 4: Missing target.**

```rust
#[test]
fn anchor_missing_target_marks_broken_and_warns_once_per_frame() {
    let mut app = app();
    let e = app.world_mut().spawn((Node, Style::default().width_px(20.0).height_px(20.0),
        Anchor {
            position_anchor: Some(AnchorRef::Name("nonexistent".into())),
            position_try: vec![PositionTry { inset: Inset::below(Length::Px(0.0)), conditions: vec![] }],
            ..default()
        })).id();
    app.update();
    app.update();

    assert!(app.world().get::<LayoutAnchorBroken>(e).is_some());
    let rl = app.world().get::<ResolvedLayout>(e).unwrap();
    assert_eq!(rl.position, Vec2::ZERO);

    use buiy_core::layout::{AnchorErrorKind, LayoutAnchorWarnedThisFrame};
    let warned = app.world().resource::<LayoutAnchorWarnedThisFrame>();
    assert!(warned.set.contains(&(e, AnchorErrorKind::TargetMissing)));
}
```

**Test 5: Idempotent broken marker (clear on resolution success).**

```rust
#[test]
fn layout_anchor_broken_clears_when_resolution_succeeds() {
    let mut app = app();

    // Start with a missing target → broken.
    let anchored = app.world_mut().spawn((Node, Style::default().width_px(20.0).height_px(20.0),
        Anchor {
            position_anchor: Some(AnchorRef::Name("late".into())),
            position_try: vec![PositionTry { inset: Inset::below(Length::Px(0.0)), conditions: vec![] }],
            ..default()
        })).id();
    app.update();
    app.update();
    assert!(app.world().get::<LayoutAnchorBroken>(anchored).is_some());

    // Now spawn the target.
    let _target = app.world_mut().spawn((Node, Style::default().width_px(50.0).height_px(50.0),
        Anchor { anchor_name: Some(AnchorName::Named("late".into())), ..default() })).id();
    app.update();
    app.update();

    // Broken marker should be removed.
    assert!(app.world().get::<LayoutAnchorBroken>(anchored).is_none());
}
```

**Test 6: Steady-state O(0) — no needless re-iteration.**

```rust
#[test]
fn anchor_steady_state_no_extra_sync_styles_iter() {
    use buiy_core::layout::SyncStylesIterCount;
    let mut app = app();
    let _anchor = app.world_mut().spawn((Node, Style::default().width_px(50.0).height_px(50.0),
        Anchor { anchor_name: Some(AnchorName::Named("a".into())), ..default() })).id();
    let _anchored = app.world_mut().spawn((Node, Style::default().width_px(20.0).height_px(20.0),
        Anchor {
            position_anchor: Some(AnchorRef::Name("a".into())),
            position_try: vec![PositionTry { inset: Inset::below(Length::Px(5.0)), conditions: vec![] }],
            ..default()
        })).id();

    // Run several frames to reach steady state.
    for _ in 0..5 { app.update(); }

    // Steady-state: sync_styles iter count should be 0 (no Changed<>
    // for any tracked component on this frame).
    let count = app.world().resource::<SyncStylesIterCount>().0;
    assert_eq!(count, 0);
}
```

**Test 7: Observer maintains registry across spawn/despawn.**

```rust
#[test]
fn anchor_observer_cleans_registry_on_despawn() {
    use buiy_core::layout::AnchorNameRegistry;
    let mut app = app();
    let e = app.world_mut().spawn(Anchor {
        anchor_name: Some(AnchorName::Named("ephemeral".into())),
        ..default()
    }).id();

    {
        let reg = app.world().resource::<AnchorNameRegistry>();
        assert_eq!(reg.find_entity_by_name("ephemeral"), Some(e));
    }

    app.world_mut().entity_mut(e).despawn();

    // Despawn fires On<Remove, Anchor> which calls reg.remove(e).
    let reg = app.world().resource::<AnchorNameRegistry>();
    assert_eq!(reg.find_entity_by_name("ephemeral"), None);
}
```

**Test 8: Anchor target with `Display::None` is treated as missing.**

```rust
#[test]
fn anchor_target_with_display_none_is_treated_as_missing() {
    let mut app = app();
    let _hidden_anchor = app.world_mut().spawn((Node,
        Style::default().width_px(50.0).height_px(50.0).display(Display::None),
        Anchor { anchor_name: Some(AnchorName::Named("hidden".into())), ..default() })).id();
    let anchored = app.world_mut().spawn((Node, Style::default().width_px(20.0).height_px(20.0),
        Anchor {
            position_anchor: Some(AnchorRef::Name("hidden".into())),
            position_try: vec![PositionTry { inset: Inset::below(Length::Px(0.0)), conditions: vec![] }],
            ..default()
        })).id();
    app.update();
    app.update();

    // Display::None target → anchored is broken.
    assert!(app.world().get::<LayoutAnchorBroken>(anchored).is_some());
}
```

Note: `Display::None` removes the entity from the Taffy tree (spec § 1.1). So `tree.by_entity.get(&hidden_anchor)` returns `None` → `anchor_resolution` falls into `TargetMissing`. This is exactly the spec's "carries `Display::None` → treated as missing" semantics.

- [ ] **Step 2: Run all 7 tests.** Some may need a synthetic `PrimaryWindow` to satisfy the `primary_window` query — see Test 2's example.

```bash
cargo test -p buiy_core --test layout_anchor_positioning 2>&1 | tail -30
```

Expected: all tests pass.

- [ ] **Step 3: Commit.**

```bash
git add crates/buiy_core/tests/layout_anchor_positioning.rs
git commit -m "test(buiy_core): anchor positioning end-to-end coverage

7 integration tests per spec § 4 + Phase 6 additions:
- fallback chain (first fails viewport, second wins)
- cycle detection (higher epoch entity's edge dropped)
- missing target (broken + warn-once-per-frame)
- idempotent broken marker (clears on resolution success)
- steady-state O(0) (no needless sync_styles re-iteration)
- observer registry cleanup on despawn
- Display::None target treated as missing (spec § 3.2 step 1)"
```

---

### Task 12: Final whole-branch review + CHANGELOG + PR

Per the established workflow:

1. Dispatch `code-reviewer` agent on the full branch diff against main.
2. Add CHANGELOG.md entries under `## [Unreleased]`:
   - `### Added (Phase 6 — layout anchor positioning)`
   - `### Changed (Phase 6)`
   - `### Deferred / divergences from spec (Phase 6)`
3. Append to `docs/plans/follow-ups.md`:
   - `anchor-size()` deferred (tier-C, v1.x)
   - `position_try_max_depth` resource cap (open in spec)
   - Cross-window anchor resolution (silent in spec)
4. Open the PR; wait for the 6 CI gates (Lint / Doc / Deny / Test ubuntu-macos-windows).
5. Merge.
6. Flip plan + `docs/README.md` to `[landed]`.
7. Clean up worktree, local branch, stale remote ref.

---

## Self-review checklist (for me, the planner) — v2 post-review

- [x] Spec coverage: every numbered spec section in § 3 has a task or decision block.
  - § 3.1 Anchor + AnchorName + AnchorRef + PositionTry + TryCondition + AnchorNameRegistry → T1 + T2 + T3 + T4
  - § 3.2 Resolution algorithm (steps 1-4 + broken marker + warn-once-per-frame) → T6 + T11
  - § 3.3 Authoring example (`Inset::above`, etc.) → T6
  - § 3.4 Cycle handling (both endpoints broken — D8) + performance contract + anchor-size() deferral → T5 + T6 + deferred section
  - § 3.5 Position-try chain depth (open question) → deferred section
  - § 4 Test surface (anchor basic, fallback chain, cycle detection) → T11; non-anchor § 4 tests (containing block, sticky, Display::Contents, Display::None vs Visibility::Hidden) PREDATE Phase 6 — not added in this phase. Verified to exist as Phase 0/1/2 tests in the `tests/` directory; Phase 6 augments only with anchor-specific coverage.
  - architecture.md § 3 (pipeline sub-pass 6d) → T8 + deferred entry for sub-pass ordering with future 6a-6c
  - architecture.md § 6 (warn-once error model) → T4 + T6 (per-frame variant introduced + explicit divergence in D5)
- [x] Type consistency: `Anchor`, `AnchorName`, `AnchorRef`, `PositionTry`, `TryCondition`, `AnchorErrorKind` names match between tasks.
- [x] Decision blocks D1-D12 are honored by every task that touches them.
- [x] No placeholders / TBD.
- [x] Prior-art citations cite exact file:line.
- [x] Each task ends with a git commit.
- [x] Tasks fit subagent-driven-development: implementer + spec-reviewer + code-quality-reviewer per task.
- [x] **Steady-state cost framing corrected**: `anchor_resolution` is O(anchored entities), NOT O(0). The plan's tests assert O(0) for `sync_styles` (no `Changed<>` cascade from anchor pass), which is the actual Phase 2 invariant preserved by Phase 6. `anchor_resolution`'s own work is always proportional to the anchored set; spec architecture.md § 9 line 265 explicitly carves out "steps 0, 6, 7 are `O(roots + anchored)`" as the cost contract for sub-pass 6d.
- [x] **All BLOCKERs from plan v1 review addressed**:
  - B-spec-1: both cycle endpoints broken (D8 + Test 3) ✓
  - B-spec-2: per-frame vs per-BuiyExit warn dedup conflict resolved (D5) ✓
  - B-prior-1: observer registration is closure form (D12) ✓
  - B-prior-2: empty-string bucket → `track_epoch` helper (D11/B2 fix) ✓
  - B-prior-3: `Sizing::FitContent(_)` tuple variant pattern fixed in Task 6 ✓
  - B-feas-1: `DuplicateName` discarded fix — moved into `anchor_resolution` (D11) ✓
  - B-feas-2: Kahn external target pre-pass (D10) ✓
  - B-feas-3: `Display::None` explicit query (D9) ✓
  - B-prior-4: `bevy::MinimalPlugins` → `MinimalPlugins` under `use bevy::prelude::*` ✓
  - B-prior-5: citation correction systems.rs:857-880 (was 691-704) ✓
- [x] **Forward note for Phase 7** (sticky/table/multicol attaching to PostTaffyOverrides): documented in deferred section as a constraint future phases must honor (`.before(anchor_resolution)`).

---

## Diff stat estimate (plan v2)

| Touchpoint | Lines added (approx) |
|---|---|
| `types.rs` (T1) | +160 (types + Inset convenience + tests) |
| `components.rs` (T2) | +80 (component + marker + tests) |
| `systems.rs` (T3-T7, T9) | +700 (resources + handle_anchor_insert + Kahn with pre-pass + anchor_resolution with Display query + DuplicateName scan + both-endpoints broken handling + helpers + tests) |
| `mod.rs` (T8) | +60 (init + 3 closure observers + register + system attach + forward note for Phase 7) |
| `lib.rs` (T8) | +6 (re-exports) |
| `crates/buiy/src/lib.rs` (T8) | +6 (re-exports) |
| `tests/layout_anchor_positioning.rs` (T6 + T7 + T11) | +500 (≈10 integration tests including both-endpoints cycle assertion + duplicate-name persists-across-frames) |
| `tests/layout_pipeline_order.rs` (T10) | +25 (anchor fixture) |
| `CHANGELOG.md` (T12) | +40 (added per-frame warn dedup divergence note) |
| `docs/plans/follow-ups.md` (T12) | +50 (anchor-on-sticky/table/multicol target invisibility + sub-pass ordering + Kahn perf) |
| **Total** | **≈+1627 lines** |

Still comfortably under Phase 5's ≈+4600 lines.

---

**End of plan.**
