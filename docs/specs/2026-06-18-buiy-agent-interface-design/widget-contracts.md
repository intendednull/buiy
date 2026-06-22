# Widget contracts — `A11yContract` authoring + per-widget APG contracts

**Date:** 2026-06-18
**Status:** draft

The **authoring surface** every interactive widget implements, and the concrete per-widget contracts for the Phase-1 catalog. ONE declaration per role drives BOTH the outbound `add_action` advertisement AND the inbound dispatch path — accessibility and agent-control cannot drift.

Read first: [semantic-tree.md](semantic-tree.md) (the `A11yState*`/`A11yRelations` components), [action-router.md](action-router.md) (`route_action_requests`/`dispatch_action_request`, the live filter, the `EditCommand::SetSelection` path), [verification.md](verification.md) (gates #3, #7).

Prior-art: [../../prior-art/wai-aria-apg/](../../prior-art/wai-aria-apg/) — [patterns-catalog.md](../../prior-art/wai-aria-apg/patterns-catalog.md), [keyboard-contracts.md](../../prior-art/wai-aria-apg/keyboard-contracts.md), [roles-states-properties.md](../../prior-art/wai-aria-apg/roles-states-properties.md), [name-computation.md](../../prior-art/wai-aria-apg/name-computation.md), [focus-management.md](../../prior-art/wai-aria-apg/focus-management.md), [live-regions.md](../../prior-art/wai-aria-apg/live-regions.md); the megacomponent anti-pattern [../../prior-art/bevy-a11y/component-model-incident.md](../../prior-art/bevy-a11y/component-model-incident.md); the AccessKit agent-control / no-key-bindings lessons in [../../prior-art/accesskit/capabilities.md](../../prior-art/accesskit/capabilities.md) and [../../prior-art/accesskit/lessons.md](../../prior-art/accesskit/lessons.md).

Version base: **accesskit 0.24 / Bevy 0.19-rc.3** — the current base (the BSN/0.19 bump landed, PR #70). Verify resolved 0.24 signatures via `cargo tree`/`cargo doc`.

---

## 1. The `A11yContract` trait

`A11yContract` in `crates/buiy_core/src/a11y/contract.rs`, re-exported into `buiy_widgets`. ONE impl per interactive role.

```rust
pub trait A11yContract: Send + Sync + 'static {
    fn role() -> A11yRole;
    /// Role-static advertised verbs. Emitted via `add_action`, re-validated inbound.
    /// Per-instance capability is the router's LIVE filter on top (§3).
    fn actions() -> &'static [accesskit::Action];
    /// Lower each advertised verb. Called by `dispatch_action_request` AFTER
    /// liveness + the live filter. Returns a typed `ActionError`, never panics.
    fn honor(world: &mut World, entity: Entity, action: accesskit::Action,
             data: Option<&accesskit::ActionData>) -> Result<(), ActionError>;
}
```

`ActionError` is in `action.rs`, shared with the router and the in-process API (full set: `NotFound`/`Unsupported`/`NotActionable`/`BadData` — [action-router.md](action-router.md)).

### The role → contract static registry

A static dispatch table maps `A11yRole` to its contract, consulted both directions:

- **Outbound** — `to_accesskit_node` looks up the role and `add_action`s every verb in `actions()`. The focusable-`add_action(Focus)` hardcode in `to_accesskit_node` (the `if view.focusable` block in translate.rs) is removed: every `Focusable` contributes `{Focus, Blur}`; the contract contributes the rest.
- **Inbound** — `route_action_requests` resolves `NodeId → Entity → A11yRole → contract` and (after liveness + capability) calls `honor`.

```rust
pub fn contract_for(role: A11yRole) -> Option<ContractEntry>;
pub struct ContractEntry {
    pub actions: &'static [accesskit::Action],
    pub honor: fn(&mut World, Entity, accesskit::Action, Option<&accesskit::ActionData>)
        -> Result<(), ActionError>,
}
```

**Static dispatch table, not a per-entity handle.** Simpler; avoids a boxed-handle component. It can't express two widgets sharing a role honoring differently — no Phase-1 widget needs that (Button vs Disclosure-trigger both `Role::Button` but advertise different verbs; the registry keys on the contract type, not bare role). Open question in [phasing.md](phasing.md): if same-role-different-honor under the same key ever appears, this lifts to a per-entity contract handle. Deferred.

---

## 2. The lockstep keystone

**Advertisement and honoring share one source of truth** — `actions()` is read by both directions.

- **advertise-without-honor** — a verb in `actions()` with no `honor` arm. A **gate-#7 contract bug**: the APG fixture replays the advertised verb and asserts the transition.
- **honor-without-advertise** — a `honor` arm for an un-advertised verb. **Can never fire** — the router re-reads the advertised set before calling `honor`, so it's dead code.

One list (`actions()`), read in two directions.

---

## 3. Role-static advertisement + the router's live filter {#the-static-actions-gap-and-the-live-filter}

`actions()` is `&'static` — role-level. A `Slider` always advertises `{Increment, Decrement, SetValue, Focus, Blur}`, but a specific instance may be disabled or read-only. A role-static set can't express that (the weakness in the rejected role-static-only model — [README.md](README.md#rejected)). Resolution: **role-static advertisement PLUS the router's live per-instance filter.** At dispatch, the router re-reads live state and drops verbs the instance can't honor:

- `A11yDisabled` → drop all actionable verbs (Focus still allowed; the node stays addressable).
- `A11yReadOnly` on a text role → drop `SetValue`/`ReplaceSelectedText`/`SetTextSelection`'s mutating effect (the editor also enforces this — `apply_tracked` returns a default when `ctx.read_only` — so the filter is additive defense).
- live `A11yValue` bounds → an `Increment` at `now == max` is a clamped no-op (saturated success, not error).

One-declaration-drives-both stays intact. The filter is the router's job ([action-router.md](action-router.md#live-capability-filter)); contracts don't duplicate it.

---

## 4. APG keyboard contracts are CONSUMER-SIDE

AccessKit models role/state/verbs but **not key bindings** ([../../prior-art/accesskit/capabilities.md](../../prior-art/accesskit/capabilities.md): the gap list). The APG keyboard contract is implemented **consumer-side** in Buiy's widget systems. The verification fixture tests **both Enter and Space** per widget and asserts the APG-correct outcome (Button fires on both; Checkbox toggles on Space, inert on Enter). The two paths converge: a keyboard event and an inbound `Action::Click` lower into the **same** sink — a Button's Space handler and its `Click` honor both emit `MessageWriter<OnPress>`. See [../../prior-art/wai-aria-apg/keyboard-contracts.md](../../prior-art/wai-aria-apg/keyboard-contracts.md) and [verification.md](verification.md) gate #7.

---

## 5. Per-widget contracts

Each widget in `buiy_widgets`; its bundle assembles the decomposed components and `impl A11yContract`. Every interactive widget implicitly advertises `{Focus, Blur}` via `Focusable` (router: `Focus` → set `FocusedEntity`+`FocusVisible`, `Blur` → clear if focused). Lists below name *additional* verbs.

### Button (`buiy_widgets/src/button.rs`)
- Role `Button`. Verbs `{Click, Focus, Blur}`.
- APG: **Enter AND Space** activate — closes the two `button.rs` TODOs (keyboard activation missing; press-on-mouse-up timing). Both handlers and the `Click` honor emit `MessageWriter<OnPress>(entity)` (the message `emit_on_press_on_click` consumes).
- Toggle button: carries `A11yToggled(Toggled)`; `Click` flips `Toggled` (via `set_toggled`, which unifies aria-pressed/aria-checked).
- `honor`: `Click` → emit `OnPress`; if `A11yToggled` present, advance first.

### Checkbox (new)
- Role `Checkbox`→`Role::CheckBox`. Verbs `{Click, Focus, Blur}`. State `A11yToggled` incl. **`Mixed`** (tri-state).
- APG: **Space ONLY toggles; Enter does NOT** — load-bearing asymmetry vs Button; gate-#7 asserts both.
- `honor`: `Click` advances `A11yToggled` (False→True→[Mixed→]False for tri-state; False↔True plain), the same state Space mutates.

### Switch (new)
- Role `Switch`→`Role::Switch`. Verbs `{Click, Focus, Blur}`. State `A11yToggled` binary (no `Mixed`).
- APG: Space and Enter both toggle. `honor`: `Click` flips False↔True.

### Slider (new)
- Role `Slider`→`Role::Slider`. Verbs `{Increment, Decrement, SetValue, Focus, Blur}`. State `A11yValue { now,min,max,step,jump,text }` + `A11yOrientation`.
- APG: Right/Up inc, Left/Down dec, Home→min, End→max, PageUp/Down→jump.
- `honor`: `Increment`/`Decrement` → `now ±= step` clamped `[min,max]` + value-change announcement (gate #4); `SetValue` (`NumericValue(f64)`) → clamp into `now`. Live filter clamps at-bounds to a saturated no-op.
- Multi-thumb: one `Role::Slider` node per thumb under a `Group` — no megacomponent; grouping falls out of the ECS subtree ([semantic-tree.md](semantic-tree.md)).

### TextInput (`buiy_widgets/src/text_input.rs`)
- Role **split** `TextInput`→`Role::TextInput` vs `MultilineTextInput`→`Role::MultilineTextInput`. The split IS the multiline distinction — no `A11yMultiline`, no `set_multiline`. Retires the `A11yRole::Text` stopgap at `text_input.rs:83-88`.
- Verbs `{SetValue, SetTextSelection, ReplaceSelectedText, Focus, Blur}`.
- State `A11yTextValue(String)` (synced from `TextEditState`) + `A11yPlaceholder` + optional `A11yReadOnly`/`A11yRequired`/`A11yInvalid`/`A11yAutoComplete` + `A11yRelations.error_message`.
- APG: the existing `apply_keyboard_edits` path; agent verbs lower into the same `EditCommand` pipeline.
- `honor` (all via `TextEditState::apply_tracked`, facade boundary `tests/text_facade_boundary.rs`):
  - `SetValue` (`Value(Box<str>)`) → `EditCommand::SelectAll` then `EditCommand::Insert(s)` — both variants exist today (`command.rs:21`), **no new editor work**.
  - `ReplaceSelectedText` (`Value`) → `Insert(s)` over the current selection; over a non-current range, first `SetSelection` then `Insert`.
  - `SetTextSelection` (`SetTextSelection`, an absolute range) → the **new** `EditCommand::SetSelection { anchor, focus }`. Today's vocabulary has only directional `Motion(Motion, bool)` and whole-buffer `SelectAll`, so it can't place an arbitrary range; the Motion lowering is not achievable. Specified in [action-router.md](action-router.md#text-lowering); `dispatch_action_request` assembles `&mut FontSystem + &mut EditContext` for `apply_tracked`.
- Read-only: drops `SetValue` (and the mutating effect of `ReplaceSelectedText`/`SetTextSelection`) via the live filter (§3). `SetTextSelection` still moves the selection on a read-only field (selecting/copying allowed, mutation forbidden).

### Disclosure-trigger (new)
- Role `Button`. Verbs `{Expand, Collapse, Click, Focus, Blur}`. State `A11yExpanded(bool)` + `A11yRelations.controls = [panel]`.
- APG: Enter/Space toggle. `honor`: `Expand` → `A11yExpanded(true)` + show panel; `Collapse` → `A11yExpanded(false)` + hide; `Click` toggles `A11yExpanded`, honored **in addition to** `Expand`/`Collapse`.
- Panel is `Role::Region`, a real child. Accordion = N disclosures nested as a real subtree — no megacomponent; Down/Up/Home/End roving focus is consumer-side.

### Dialog (new role wiring)
- Role `Dialog`/`AlertDialog`. State `A11yModal` + `A11yRelations.labelled_by=[title]`/`described_by=[body]`/`owns=[...]`. Invoker advertises `{Click}` + `controls=[dialog]`.
- Implicit live: `AlertDialog` derives assertive/atomic from role in the derive fold (alert⇒assertive+atomic, status⇒polite+atomic, log⇒polite), announcing correctly with no author `A11yLive` ([semantic-tree.md](semantic-tree.md), [../../prior-art/wai-aria-apg/live-regions.md](../../prior-art/wai-aria-apg/live-regions.md)).
- No AccessKit dialog verb: focus-trap/Esc/restore are Buiy's overlay state machine. AccessKit contributes the modal flag + labelling/owns. `SetSequentialFocusNavigationStartingPoint` routes into the focus model. `owns` re-parents a portalled dialog so the tree reflects ownership, not paint location.

### Tooltip-trigger (new)
- Verbs (trigger) `{ShowTooltip, HideTooltip, Focus, Blur}`. State `A11yRelations.described_by=[tooltip]`. `honor`: drive show/hide timing. Tooltip node `Role::Tooltip`, non-interactive.

### Implicit: every `Focusable`
Contributes `{Focus, Blur}` (the `Focusable` component drives it in `to_accesskit_node`, replacing the focusable-`add_action` hardcode in translate.rs).

---

## 6. The router rule (for authors)

Dispatch reaches a verb only if advertised (`actions()` → `add_action`) AND honored (`honor` arm). If you add to `actions()`, add a `honor` arm — else gate #7 fails. A `honor` arm without advertisement is dead code. Author both halves in the same change; the fixture in [verification.md](verification.md) enforces it.
