# In-Process Inspect + Control API

**Date:** 2026-06-18
**Status:** draft

The transport-agnostic in-process inspect+control contract — Phase 1's headline deliverable and Buiy's own headless test driver. In `crates/buiy_core/src/a11y/inprocess.rs`, defined ONCE over `&mut World` / `&mut App`, NO winit and NO GPU, the substrate [mcp-companion.md](./mcp-companion.md) later wraps in a socket envelope without changing a line.

Target-state API: `snapshot`, the one generic `perform`, the ergonomic wrappers + `get_by_role`, the `act_when_actionable`/`wait_for` actionability gates, the new lowest `buiy_verify` tier, the off-by-one ref fix, and the `accesskit_consumer` dependency. Tree shape defined here at the boundary, structurally in [semantic-tree.md](./semantic-tree.md); inbound dispatch in [action-router.md](./action-router.md); gates in [verification.md](./verification.md). Index + locked decisions: [README.md](./README.md).

Prior-art: [../../prior-art/accesskit/tree-model.md](../../prior-art/accesskit/tree-model.md) + [../../prior-art/accesskit/lessons.md](../../prior-art/accesskit/lessons.md) (one canonical retained tree, N consumers — ATs and agents read the same nodes). The act-then-observe round-trip, the actionability gate-then-act loop, and the define-the-contract-once-over-an-in-process-channel discipline are borrowed from Playwright/Playwright-MCP, browser-automation actionability, and the React DevTools Bridge/Wall — external references named in prose here; per the [README.md](./README.md) research-debt note, those `docs/prior-art/` folders are not yet written and must be created (`researching-prior-art`) before Phase 2 transport lands.

---

## 1. Why an in-process seam exists

AccessKit's outbound push is built for live ATs: `build_tree` → `TreeUpdate` → `accesskit_winit::Adapter` ships it via the per-window `ACCESS_KIT_ADAPTERS` thread-local. In a headless test (no winit, no display, no GPU — the gate [verification.md](./verification.md) keeps green without an adapter) that thread-local is **empty**. Tests, `buiy_verify`, and later the MCP companion still need the same tree an AT sees and to drive the same inbound channel.

The in-process API feeds the **same** `TreeUpdate` (`build_tree` then `build_tree_update(builder.snapshot(), focus)`) into an `accesskit_consumer::Tree`, and calls `dispatch_action_request(&mut World, &ActionRequest)` directly (the headless seam, LOCKED #6). One contract, two transports: in-process now, socket later. A parallel test driver beside the production protocol fragments (the flutter_driver/integration_test/MCP split); Buiy takes the single-contract path.

---

## 2. Snapshot (inspect)

```rust
pub enum TreeView { Unmerged, Merged }
impl Default for TreeView { fn default() -> Self { TreeView::Unmerged } }
pub fn snapshot(world: &mut World, view: TreeView) -> SemanticTree;
```

`Unmerged` = canonical structural tree (the default; ATs self-merge, most diffable). `Merged` = read-time projection collapsing `A11yMergeChildren` subtrees.

`snapshot` runs the **production** translate path: (1) `build_tree(world)`; (2) `build_tree_update(builder.snapshot(), focus)` — the same `TreeUpdate` the adapters consume; (3) fed into an `accesskit_consumer::Tree`, and `SemanticTree` serialized **from the consumer view** — the same path that resolves `labelled_by`/`active_descendant`, applies role-implied defaults, and exposes announcement fields, so gates #3/#4/#7 observe the same tree a real AT would, not a Buiy-private shortcut. `TreeView` selects the projection at read time over the ONE canonical tree (two stored trees rejected in [semantic-tree.md](./semantic-tree.md)).

`snapshot` never dumps the raw `World`. The AccessKit tree **is** the pre-filtered allowlist (pure-layout, `A11yHidden`, inert nodes already excluded by `build_tree`) — the exposure boundary the MCP companion inherits ([mcp-companion.md](./mcp-companion.md) §5.2).

### 2.1 `SemanticTree` shape

Deterministic, diffable. Per node: `role` (`role_to_str`, lockstep with `translate::role_to_accesskit`), `name` (`compute_accessible_name`), `state` (decomposed, **present-only**: toggled/expanded/selected/disabled/read_only/required/busy/modal/invalid/value/text_value/placeholder/orientation/has_popup/auto_complete/level/live/pos_in_set/set_size — a key appears only when its component is present), `actions` (advertised verbs the router can honor), `relations` (each as the target's `ref`), `ref` (`node_id_for(entity).0` = `entity.to_bits()+1`), nested `children` (document order). A button+checkbox fixture serializes to ~2–5 KB — vs thousands of vision tokens, no antialiasing flakiness. That size + determinism makes it the new lowest tier (§4).

### 2.2 The off-by-one ref fix (LOCKED #4)

Today `buiy_verify::a11y::snapshot_tree` (`crates/buiy_verify/src/a11y.rs:47`) emits raw `n.entity.to_bits()`, while `node_id_for` (`crates/buiy_core/src/a11y/translate.rs`) returns `to_bits().saturating_add(1)`. The serializer's `ref` is **one below** the addressed NodeId — a latent bug the moment a snapshot drives actions. Fix (Phase 0, [phasing.md](./phasing.md)): emit `node_id_for(n.entity).0`; **re-bless affected goldens in the same change** (the `ref` values shift +1; nothing else moves). After the fix `ref` round-trips: `perform(world, action, snapshot_node.ref, data)` addresses the named entity. The inverse `entity_for_node_id(NodeId)` = `(id.0 != 0).then(|| Entity::from_bits(id.0 - 1))` lives next to `node_id_for` and is shared with the router ([action-router.md](./action-router.md)).

---

## 3. Perform (control) — one primitive, act-then-observe

```rust
pub fn perform(world: &mut World, action: accesskit::Action,
               target: accesskit::NodeId, data: Option<accesskit::ActionData>)
    -> Result<SemanticTree, ActionError>;
```

The single control primitive: (1) builds `ActionRequest { action, target, data }` and calls `dispatch_action_request(world, &req)` directly — the headless seam ([action-router.md](./action-router.md)): same liveness + live capability re-check, same per-`Action` dispatch into the real Focus/OnPress/EditCommand/slider/expanded/tooltip sinks, sidestepping the one-frame winit latency; (2) ticks the schedule until the frame settles; (3) auto-re-snapshots and returns the post-action `SemanticTree` inline. **Act-then-observe in one round-trip** — the consequence comes back in the same call. Failure is typed and loud (`NotFound`/`Unsupported`/`NotActionable`/`BadData` from [action-router.md](./action-router.md)) — never a silent no-op.

### 3.1 Ergonomic wrappers — thin sugar over the ONE primitive

Each constructs the right `(action, data)` and calls `perform`. No parallel routing.

```rust
pub fn click(world, target)        -> Result<SemanticTree, ActionError>;
pub fn set_value(world, target, s) -> Result<SemanticTree, ActionError>; // SetValue + Value
pub fn focus(world, target)        -> Result<SemanticTree, ActionError>;
pub fn increment(world, target)    -> Result<SemanticTree, ActionError>;
pub fn expand(world, target)       -> Result<SemanticTree, ActionError>;
pub fn set_selection(world, target, sel: accesskit::TextSelection)
                                   -> Result<SemanticTree, ActionError>; // SetTextSelection
```

`set_selection` caveat: `Action::SetTextSelection` (and `ReplaceSelectedText` over an arbitrary range) cannot be expressed by today's `EditCommand` (`crates/buiy_core/src/text/edit/command.rs:21`) — `Motion(Motion, bool)` is directional and `SelectAll` selects the whole buffer; **no variant places an absolute anchor+focus**. Honoring them needs the **new** `EditCommand::SetSelection { anchor, focus }` (and `dispatch_action_request` assembling `&mut FontSystem + &mut EditContext` for `apply_tracked`). That's the router's contract ([action-router.md](./action-router.md#text-lowering)); `set_selection`'s existence here is **gated on it landing** ([phasing.md](./phasing.md)), not assumed. (`set_value` on text lowers via `SelectAll` + `Insert`, both of which exist today — no new work.)

### 3.2 `get_by_role` — addressing above bare NodeId

```rust
pub fn get_by_role(world: &mut World, role: A11yRole, name: Option<&str>,
                   state: Option<&StateQuery>) -> Result<accesskit::NodeId, ActionError>;
```

**Strict single-match**: zero or >1 matches is a loud `ActionError::NotFound` ("ambiguous: N matched") — not first-match, not a retry (the Playwright strict-locator rule). Ambiguity is a *test* bug. The future home of author-supplied test-id matching (a named Phase-2 follow-up, [phasing.md](./phasing.md)); Phase 1 errors loudly. `StateQuery` is a present-only predicate (e.g. `expanded: Some(true)`) matched against the same decomposed state the snapshot exposes.

---

## 4. The new lowest `buiy_verify` tier: semantic-tree snapshot

```
semantic-tree snapshot   ← NEW, lowest: role+name+state+actions+relations+ref, ~2–5 KB, NO rasterizing
  └─ layout snapshot └─ display-list snapshot └─ invariant └─ reftest └─ golden (residue only)
```

The lowest-tier rule ([verification.md](./verification.md), `using-buiy-verification`) now bottoms out here. A role regression, a missing name, a `toggled` that didn't flip, a wrong `labelled_by`, an un-advertised action — all observable here **without rasterizing a pixel**. Only genuine rasterization residue belongs at the golden tier. Cheap, deterministic, no GPU — runs in the headless gate, never the `#[ignore]` lane. `buiy_verify::a11y` gains `semantic_tree(app, view) -> String` (calls `inprocess::snapshot`, serializes insta-compatible); gates #3/#4/#6/#7 drive this tier. The #12 invariants (no orphans, every focusable named, focus reachable) are owned in the a11y subsystem (properties of the semantic contract), driven by `buiy_verify` as a tier.

---

## 5. Actionability — frame-loop polling, not sleeps

Driving a widget the moment its ref resolves is wrong (may not be laid out, mid-animation, obscured, disabled). Gate each action behind a frame-loop poll (the attached/stable/visible/enabled discipline of browser-automation actionability):

```rust
pub struct ActionabilityOpts { pub timeout_frames: u32, pub force: bool }
pub fn act_when_actionable(app: &mut App, target: accesskit::NodeId,
    action: accesskit::Action, data: Option<accesskit::ActionData>,
    opts: ActionabilityOpts) -> Result<SemanticTree, ActionError>;
```

Each frame **re-resolves** the target (NodeId stable, liveness not) and checks, in order:

| Gate | Condition | Source |
|---|---|---|
| Attached | `entity_for_node_id(target)` resolves + exists | World |
| LaidOut | has `ResolvedLayout`, non-empty bounds, not hidden/inert | layout |
| Stable | resolved bounds unchanged across two frames *(caveat)* | layout (open question) |
| HitTargetable | `picking::hit_test(world, center)` returns this entity *(caveat)* | picking (follow-up) |
| Enabled | `A11yDisabled` clear | a11y state |

Between polls it runs `app.update()` — real frames, no sleep — until all gates hold or `timeout_frames` is exhausted (`NotActionable`). Not-yet-ready is retried **silently**; unsupported/stale/ambiguous is surfaced **loudly**. `force = true` bypasses *actionability* (escape hatch) but the action still validates the ref through `dispatch_action_request` — `force` skips actionability, not correctness.

### 5.1 Honest caveats

- **HitTargetable depends on stacking-aware hit-testing — a NAMED FOLLOW-UP.** `picking::hit_test` (`crates/buiy_core/src/picking/mod.rs:37`) returns the smallest-AABB entity — z-order/stacking/top-layer UNAWARE; it cannot tell whether a node is obscured. Phase 1 ships `HitTargetable` as **AABB-only** with the explicit limitation; the not-obscured-by-an-overlay semantics are a follow-up ([phasing.md](./phasing.md)) gated on a stacking-aware `hit_test`. The existing function is NOT presented as already answering "obscured."
- **Stable is an OPEN QUESTION vs Buiy's layout-dirty machinery.** "Same bounds across two frames" is a DOM sampling heuristic. The implementation **must verify** whether the layout pipeline already exposes a *settled*/not-dirty signal to read directly (cleaner than sampling). If so, Stable reads it; otherwise it falls back to two-frame sampling. Flagged, not assumed.

### 5.2 `wait_for` — block on a semantic condition

```rust
pub fn wait_for(app: &mut App, cond: impl Fn(&SemanticTree) -> bool,
                timeout_frames: u32) -> Result<SemanticTree, ActionError>;
```

Blocks on a **semantic** condition (node appears, state settles, name changes) against the `SemanticTree`, never a pixel diff. Steps real frames between checks. The building block for async/animated flows; the MCP companion exposes the same `wait_for` over the wire.

---

## 6. Dependency: `accesskit_consumer`

The snapshot path feeds its `TreeUpdate` into an `accesskit_consumer::Tree`, which is **not currently a declared dependency** (root `Cargo.toml` lists only `accesskit`/`accesskit_winit`). Add it to `crates/buiy_core/Cargo.toml`, version **0.36/0.37** — pinned to the release matching the resolved `accesskit` 0.24 (verify the compatible pair via `cargo tree`/`cargo doc`, not docs.rs; `Cargo.lock` is gitignored here). The 0.24 vocabulary is the current base — the Bevy 0.19-rc.3 bump landed (PR #70, main @ `3b3b0ba`) and this branch is rebased onto it ([phasing.md](./phasing.md)). Run `cargo deny check` when adding the dependency. The dependency is `buiy_core`'s; `buiy_verify`/`buiy_mcp` consume it transitively.

---

## 7. What this API is and is not

- **Is**: the one transport-agnostic inspect (`snapshot`) + control (`perform`) contract over `&mut World`, the strict resolver, the sugar, the no-sleep loop — all headless, all on Buiy's real schedule, all reading/driving the same AccessKit tree an AT and an agent see.
- **Is not** a transport: no socket, auth, capability tiers, or wire format — the MCP companion's concern in Phase 2 ([mcp-companion.md](./mcp-companion.md)), riding this contract unchanged (React DevTools Bridge/Wall).
- **Is not** a parallel test driver: every control path funnels through `perform` → `dispatch_action_request`; every inspect through `snapshot` → `build_tree_update` → `accesskit_consumer::Tree`.

See [README.md](./README.md) for the index, [widget-contracts.md](./widget-contracts.md) for the per-widget verbs, [phasing.md](./phasing.md) for Phase 0 → 1c sequencing.
