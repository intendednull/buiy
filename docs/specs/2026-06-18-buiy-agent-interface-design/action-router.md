# Action Router — the single inbound Action ingress

**Date:** 2026-06-18
**Status:** draft

The **one inbound channel** by which screen readers, in-process agents, and Buiy's own headless test driver drive the running app. The consumer half of one-tree/N-consumers: the same nodes [semantic-tree.md](./semantic-tree.md) publishes outbound are the targets here. Inbound dispatch is a single ECS system plus a headless free function; **no parallel agent driver** — one ingress, one `perform`, wrappers are sugar ([../../prior-art/accesskit/capabilities.md](../../prior-art/accesskit/capabilities.md): Actions are AccessKit's inbound verb channel; [../../prior-art/accesskit/lessons.md](../../prior-art/accesskit/lessons.md): the agent-control posture).

See also [widget-contracts.md](./widget-contracts.md) (the advertise-and-honor surface) and [inprocess-api.md](./inprocess-api.md) (the headless caller). [README.md](./README.md#rejected-alternatives) records the rejected competing-`ActionHandler` alternative and the risk register.

> **Version note.** Every type/`Action`/`ActionData` here is **accesskit 0.24** (rides the Bevy 0.19-rc.3/wgpu-29 bump; main is 0.21/0.18). Forward-looking. Verify against `Cargo.lock`, not docs.rs.

---

## 1. One ingress, no competing handler

The seam is the **existing** `bevy_winit` channel: `bevy_winit::accessibility` installs a single-occupant `WinitActionHandler` via `accesskit_winit::Adapter::with_direct_handlers`, and `poll_receivers` (PostUpdate, `AccessibilitySystems::Update`) forwards every `accesskit::ActionRequest` onto a `MessageWriter<ActionRequestWrapper>` (verified in `bevy_winit` 0.18.1 and 0.19.0-rc.3 `src/accessibility.rs`). The adapter is **structurally single-occupant** ([../../prior-art/accesskit/lessons.md](../../prior-art/accesskit/lessons.md): per-window ownership is structural, the slot single-occupant). A second handler is impossible/wrong. Buiy adds **one reader system** draining the channel bevy_winit fills:

```rust
// crates/buiy_core/src/a11y/action.rs
fn route_action_requests(
    mut requests: MessageReader<ActionRequestWrapper>,
    world: /* &mut World via exclusive system, §5 */,
) { /* … */ }
```

Single ingress for screen readers AND agents AND tests. (Rejected: a competing handler — [README.md](./README.md#rejected-alternatives).)

---

## 2. NodeId ↔ Entity inversion

The ref is the AccessKit `NodeId` = `entity.to_bits() + 1` (`node_id_for` in `a11y/translate.rs`; `+1` reserves `NodeId(0)` for the synthetic root). The shared inverse lives **next to `node_id_for`**:

```rust
pub fn entity_for_node_id(id: NodeId) -> Option<Entity> {
    (id.0 != 0).then(|| Entity::from_bits(id.0 - 1))
}
```

Three call sites: (1) `route_action_requests`; (2) `dispatch_action_request` (§5); (3) the **buiy_verify serializer off-by-one fix** — `buiy_verify::a11y::snapshot_tree` emits raw `n.entity.to_bits()` (a11y.rs:47) today, one short of the addressed NodeId; it changes to `node_id_for(n.entity).0`. Detailed in [inprocess-api.md](./inprocess-api.md); both directions stay in one place to never drift. `NodeId` is **exact** — no fuzzy matching; the only failure modes are liveness and capability (§3).

---

## 3. Per-request guard: liveness + LIVE per-instance capability {#live-capability-filter}

`build_tree` rebuilds every frame; a `NodeId` is stable but its entity may despawn/move/change state between read and act. Re-validate **every request, every frame**:

1. **Liveness.** `entity_for_node_id(req.target)`; entity must exist this frame. Else a soft typed `ActionError::NotFound` ("stale ref") — never a panic.
2. **Capability re-check (live filter).** Re-read at dispatch: the role's `A11yContract::actions()` advertised set (verb must be advertised at all → else `Unsupported`), PLUS live `A11yDisabled`/`A11yReadOnly` markers and live `A11yValue { min,max,step }` bounds. The fix to the static-`actions()` gap ([widget-contracts.md](./widget-contracts.md#the-static-actions-gap-and-the-live-filter)). A `SetValue` against a read-only-this-frame field → typed `ActionError::NotActionable`, never applied blindly (Compose's read-only-`TextField`-rejects-`SetText`; [../../prior-art/accesskit/lessons.md](../../prior-art/accesskit/lessons.md)).
3. **Dispatch** only after (1)+(2) pass.

```rust
pub enum ActionError {
    NotFound { target: NodeId },
    Unsupported { target: NodeId, action: Action },
    NotActionable { target: NodeId, action: Action, reason: NotActionableReason },
    BadData { target: NodeId, action: Action },
}
```

All four arms propagate **loudly** through the in-process API (and later MCP) so an agent never silently no-ops — see [README.md](./README.md#risks).

---

## 4. Per-Action dispatch table {#text-lowering}

The 0.24 `Action` enum is a **closed 22-variant set** — no `Toggle`/`Press`; activation is `Click`. Every variant lowers into a **real Buiy sink**, never a shadow path.

| Action | Lowering |
|---|---|
| `Focus` | `FocusedEntity.0 = Some(e)` + `FocusVisible.0 = true`. |
| `Blur` | Clear `FocusedEntity` iff target is focused. |
| `Click` | Role-dispatched. Button → `MessageWriter<OnPress>(e)` (same message the pointer path emits — the route the two `buiy_widgets/src/button.rs` TODOs flag). Checkbox/Switch → advance `A11yToggled`. Disclosure → toggle `A11yExpanded`. Toggle button → flip `A11yToggled`. |
| `Expand`/`Collapse` | Set `A11yExpanded(true/false)` + drive panel; honored in addition to `Click`. |
| `Increment`/`Decrement` | No data. `A11yValue.now ±= step`, clamped `[min,max]`; emits value-change announcement. |
| `SetValue` | Role-dispatched on `ActionData`. Slider (`NumericValue(f64)`) → clamp into `now`. Text (`Value(Box<str>)`) → `EditCommand::SelectAll` then `EditCommand::Insert(s)` via `apply_tracked` (both variants exist today — `SelectAll` is `command.rs:61`, so this lowers fine now). |
| `SetTextSelection` | `ActionData::SetTextSelection` (absolute anchor+focus) → the new `EditCommand::SetSelection { anchor, focus }` (§4.1) via `apply_tracked`. |
| `ReplaceSelectedText` | `Value(Box<str>)` → `EditCommand::SetSelection { anchor, focus }` (when a range is carried) then `Insert(s)`, via `apply_tracked`. |
| `ShowTooltip`/`HideTooltip` | Tooltip trigger show/hide timing. |
| `SetSequentialFocusNavigationStartingPoint` | Focus model's "tab from here" anchor. |
| `CustomAction(i32)` | Opt-in `CustomActionRegistry` (`i32` → app verb). Unmapped → `Unsupported`. 0.24 `CustomAction` is an `i32` index only — no name+args; structured verbs defer to [mcp-companion.md](./mcp-companion.md). |
| `Scroll*` | Deferred; no-op + guidance `Unsupported` until scroll containers ship. |

Any verb the contract doesn't advertise is rejected at §3.

### 4.1 New editor work: `EditCommand::SetSelection { anchor, focus }`

**Real new editor work**, not a thin lowering. Today's `EditCommand` (`crates/buiy_core/src/text/edit/command.rs:21`) is `Motion(Motion, bool) / Insert(String) / Backspace / Delete / Enter / Cut / Copy / Paste / Undo / Redo / SelectAll / Escape / Submit`. `Motion` is directional (with an `extend` bool); `SelectAll` selects the whole buffer; **no existing variant places an arbitrary absolute anchor/focus range**, which `SetTextSelection` (and `ReplaceSelectedText` over an arbitrary range) require. The "Motion-with-extend" idea is **not achievable** and is abandoned.

Phase 1c adds:

```rust
pub enum EditCommand { /* … existing … */ SetSelection { anchor: usize, focus: usize } }
```

`apply_tracked` gains its arm; its signature needs `&mut FontSystem` + `&mut EditContext` assembled in `dispatch_action_request` (§5) — named, not assumed. `apply_tracked` already rejects mutations when `ctx.read_only` (returns `EditOutcome::default`, in `text/edit/input.rs`); the §3 filter is **additive** defense.

### 4.2 Facade boundary

Text actions emit **Buiy's `EditCommand`**, never cosmic `Action` directly. The seam is `EditCommand → apply_tracked`. Enforced by `tests/text_facade_boundary.rs`.

---

## 5. Headless dispatch seam

```rust
pub fn dispatch_action_request(world: &mut World, req: &ActionRequest) -> Result<(), ActionError>;
```

Runs with **no winit adapter, no GPU**. `route_action_requests` is a thin wrapper: drain the `MessageReader`, call this per request. The in-process API and test driver call it directly (or mint `ActionRequestWrapper` messages), exercising inbound → sink → outbound headless ([inprocess-api.md](./inprocess-api.md)). LOCKED #6.

---

## 6. Sink resource discipline

Every sink (`FocusedEntity`, `FocusVisible`, `Clipboard`, shared `FontSystem`, `CustomActionRegistry`, …) is owned by a sibling plugin, accessed as `Option<Res/ResMut>` (or `get_resource_mut` over `&mut World`). The Input-set Option-discipline (`emit_on_press_on_click` precedent): no-op gracefully under a partial/headless harness, never panic.

---

## 7. Scheduling — EXPLICIT intra-`BuiySet::Input` ordering

The top-level `BuiySet`s are `.chain()`ed in the **real order `Layout → Style → Input → Animate → Picking → A11yUpdate → Render`** (variants at lib.rs:65-72; `.chain()` at lib.rs:87-93). Because `Input` precedes `A11yUpdate`, an action consumed in `Input` reflects outbound in the **same frame's** `A11yUpdate` — cross-set ordering is already correct.

**But within `BuiySet::Input`, Bevy does NOT order systems without an explicit constraint.** "Runs at the start of Input" is not automatic. `route_action_requests` MUST carry explicit ordering — either:

```rust
route_action_requests
    .in_set(BuiySet::Input)
    .before(emit_on_press_on_click)
    .before(handle_tab)
    .before(apply_keyboard_edits)
    .before(focus_on_click)
```

or an ordered sub-set placed first within `Input` (`BuiyInputSub::ActionIngress` before `BuiyInputSub::Handlers`, `.chain()`ed). With this, synthesized `OnPress`/`FocusedEntity`/`EditCommand`/value effects are consumed the same frame and reflected outbound in `A11yUpdate`. Without it the guarantee silently fails. The sub-set form is preferred as the surface grows.

---

## 8. One-frame inbound latency (winit path)

`poll_receivers` writes the message in the **previous frame's PostUpdate**; `route_action_requests` reads at the next frame's `Input` start — a **documented one-frame inbound latency**. Document it so it isn't mistaken for a bug. The headless seam (§5) sidesteps it entirely: `dispatch_action_request` runs synchronously against `&mut World`, so in-process tests see the effect within the same `app.update()`. The [inprocess-api.md](./inprocess-api.md) act-then-observe round-trip relies on this.

---

## 9. Out-of-scope dependency: stacking-aware actionability {#actionability}

The in-process `HitTargetable` gate ([inprocess-api.md](./inprocess-api.md#actionability)) wants "not obscured by a modal/top-layer/tooltip overlay." `picking::hit_test(world, point)` (`picking/mod.rs:37`) returns the **smallest-AABB** entity containing the point — **z-order/stacking/top-layer UNAWARE**. It does not answer "obscured." Making `HitTargetable` reject an obscuring overlay requires **stacking-aware hit-testing first** — new work, a named follow-up, not provided today. Until then `HitTargetable` is AABB-only with the documented limitation. The router itself dispatches by exact `NodeId` and does not depend on this gate.

---

## 10. Phase placement

Phase 1c (inbound router + `A11yContract` + in-process driver), after 1a (outbound components) and 1b (real nesting). The `accesskit_consumer` dependency is added the same phase window, pinned to the 0.24-matching version, run through `cargo deny`. Full sequence in [phasing.md](./phasing.md).
