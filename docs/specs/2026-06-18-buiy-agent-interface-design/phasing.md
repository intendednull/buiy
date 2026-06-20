# Phasing, Sequencing, Follow-ups, Open Questions, and Risks

**Date:** 2026-06-18
**Status:** draft

Sequences the work into review-gated phases, names the deferred follow-ups, surfaces the open questions, and records the load-bearing risks. The migration narrative for the target state in the sibling files; they describe *what* is built, this describes *the order it lands and what we defer*.

Siblings: [README.md](./README.md) (index + scope/non-goals + rejected alternatives), [semantic-tree.md](./semantic-tree.md), [action-router.md](./action-router.md), [widget-contracts.md](./widget-contracts.md), [inprocess-api.md](./inprocess-api.md), [mcp-companion.md](./mcp-companion.md), [verification.md](./verification.md).

Prior-art grounding: [../../prior-art/accesskit/capabilities.md](../../prior-art/accesskit/capabilities.md), [../../prior-art/accesskit/lessons.md](../../prior-art/accesskit/lessons.md) (the agent-control posture, decomposed components, no key bindings), [../../prior-art/accesskit/tree-model.md](../../prior-art/accesskit/tree-model.md) (one canonical retained tree), [../../prior-art/bevy-a11y/component-model-incident.md](../../prior-art/bevy-a11y/component-model-incident.md) (#17644 this spec inverts), [../../prior-art/wai-aria-apg/](../../prior-art/wai-aria-apg/). The transport-side references (MCP, Playwright-MCP, React DevTools Bridge/Wall, BRP) are not yet backed by `docs/prior-art/` folders — see the [README.md](./README.md) research-debt note; the `researching-prior-art` pass that creates them is a prerequisite of Phase 2.

---

## Dependency gate: accesskit 0.24 / Bevy 0.19-rc.3 (rides the BSN bump)

This entire spec targets **accesskit 0.24** and **Bevy 0.19-rc.3** (wgpu 29, accesskit_winit matching). Main is on **accesskit 0.21 / Bevy 0.18** (`Cargo.toml:71` pins `accesskit = "0.21"`; translate.rs carries the 0.21 `set_label` doc note). The 0.24 vocabulary — `set_toggled`, `set_expanded(bool)`, `set_selected(bool)`, `set_live_atomic` (NOT `set_atomic`, which doesn't exist in 0.24 — [semantic-tree.md](./semantic-tree.md)), the closed 22-variant `Action` enum, `CustomAction(i32)`, the `Role::TextInput`/`Role::MultilineTextInput` split — **rides the in-flight BSN/Bevy 0.19-rc.3 bump campaign** (separate work, needs user go-ahead).

**This spec is sequenced AFTER that bump lands.** Two hard rules:

1. **Verify every 0.24 setter signature against `Cargo.lock`, not docs.rs.** The review caught one stale assumption (`set_atomic` → `set_live_atomic`, accesskit-0.24.1 lib.rs:1806); treat the whole fold the same way.
2. **If the bump slips, write the derive fold against 0.21 shapes and migrate.** The fold is the single emission point ([semantic-tree.md](./semantic-tree.md)); writing it 0.21-first is mechanical to migrate (per-component `if let Some(x) = view.field { node.set_x(x) }`), but several setters differ and the role split is 0.24-only. Keep the fold isolated so the migration is one file.

`accesskit_consumer` ([inprocess-api.md](./inprocess-api.md), [verification.md](./verification.md)) is **not currently a declared dependency**. Add it to `crates/buiy_core/Cargo.toml` (0.36/0.37 in the registry, version matching the bump) and run `cargo deny check` at Phase 1a/1c. Version-skew is a hard check.

---

## Phase sequence

Review gates between phases: a fresh-context review (logic, spec-alignment, quality) + the relevant gate (#3/#4/#6/#7/#12) green headless before carrying work forward.

### Phase 0 — addressing + serializer fix (no behavior change)

Unblocks everything (LOCKED #4). The only phase that can land *before* the 0.24 bump for the first two deliverables; the role additions referencing 0.24-only roles are gated on the bump.

- Add `entity_for_node_id(NodeId) -> Option<Entity>` next to `node_id_for` in `a11y/translate.rs`: `(id.0 != 0).then(|| Entity::from_bits(id.0 - 1))`. Reused by the router and the serializer fix.
- Fix `buiy_verify::a11y::snapshot_tree` (`a11y.rs`, the `entity: n.entity.to_bits()` map at line 47) to emit `node_id_for(n.entity).0`. **Re-bless affected residue goldens in the same change** (the `ref` values shift +1).
- Extend the `A11yRole` `#[non_exhaustive]` enum (`Checkbox`/`Switch`/`Slider`/`TextInput`/`MultilineTextInput`/`Region`/`Group`), **updating BOTH stringifiers**: `translate::role_to_accesskit` AND `buiy_verify::a11y::role_to_str` (the `_ => "Unknown"` wildcard masks a half-update). Gate #3 stays green.

Verification: gate #3 stays green; goldens re-blessed; no functional change.

### Phase 1a — decomposed component surface (outbound)

Make the tree richly correct per widget × state. No inbound yet.

- `a11y/states.rs` (NEW): `A11yToggled`, `A11yExpanded`, `A11ySelected`, markers `A11yDisabled`/`ReadOnly`/`Required`/`Busy`/`Modal`/`Hidden`, `A11yInvalid`, `A11yValue`, `A11yTextValue`, `A11yPlaceholder`, `A11yOrientation`, `A11yHasPopup`, `A11yAutoComplete`, `A11yLevel`, `A11yLive`, `A11yPosInSet`, `A11ySetSize` (~21 state concepts). `a11y/relations.rs` (NEW): `A11yRelations`. All `#[derive(Component, Reflect, FromReflect, Default, Clone)]` `#[reflect(Component)]`, `register_type`'d in `A11yPlugin::build`. Shapes in [semantic-tree.md](./semantic-tree.md).
- Widen `A11yNodeView` (mod.rs:57-62) from 5 flat fields to the full winit-free snapshot, and widen the `build_tree` query tuple. Rewrite `to_accesskit_node` as the flat ordered **0.24-setter derive fold**. **Fold the corrections in:** `A11yLive{atomic}` → `set_live_atomic`, and the fold **derives implicit Live from role** (alert→assertive/atomic, status→polite/atomic, log→polite) — [semantic-tree.md](./semantic-tree.md), [../../prior-art/wai-aria-apg/live-regions.md](../../prior-art/wai-aria-apg/live-regions.md).
- ACCNAME 1.2 `compute_accessible_name` fn in `a11y/accname.rs` (NEW) — a function, NOT a component.
- **Add `accesskit_consumer` to `crates/buiy_core/Cargo.toml`**, run `cargo deny check`.

Verification: gate #3 over the in-process `accesskit_consumer` path (the new lowest tier). Every new state component and role variant ships its #3 fixture.

### Phase 1b — real nesting

Replace the synthetic flat single-root with the ECS tree + relation overlay.

- `build_tree` reads `Option<&ChildOf>` + `Option<&Children>`: DEFAULT = the `ChildOf`/`Children` tree filtered to a11y-bearing entities via `nearest_a11y_ancestor`; ROOT keys off the window entity (`node_id_for(window_entity)`, retiring hardcoded `ROOT_NODE_ID`-as-constant); OVERRIDE = `A11yRelations.owns` re-parents LAST (owned entities removed from their layout parent; cycle/duplicate-parent logged once and dropped); PRUNE = `A11yHidden`/inert + descendants emit no node. `build_tree_update` rewritten to push_child resolved children in document order and emit relations as setters. Full rules in [semantic-tree.md](./semantic-tree.md).
- Merged/unmerged projection knob (`TreeView::{Unmerged,Merged}`, default Unmerged); an `A11yMergeChildren` marker drives the collapse. Read-time over the ONE canonical tree.
- **Land the gate #12 invariants** as proptests owned by the a11y subsystem (in `inprocess.rs`): no orphans, focus reachable, every focusable named. See [verification.md](./verification.md).

Verification: gate #3 over the nested tree; gate #12 green.

### Phase 1c — inbound router + `A11yContract` + in-process driver

Wire bidirectionality; light the input-replay + APG gates headless.

- `a11y/contract.rs` (NEW): the `A11yContract` trait + the static role→contract registry. Drives BOTH outbound `add_action` and inbound dispatch — the lockstep keystone. See [widget-contracts.md](./widget-contracts.md).
- `a11y/action.rs` (NEW): `route_action_requests` draining the EXISTING `MessageReader<ActionRequestWrapper>` (LOCKED #6); the liveness + live-capability guard; `dispatch_action_request(world: &mut World, req: &ActionRequest) -> Result<(), ActionError>` dispatching every 0.24 Action into the real Focus/OnPress/EditCommand/slider/expanded/tooltip sinks. **Corrections folded in:**
  - `route_action_requests` gets **EXPLICIT intra-`BuiySet::Input` ordering** — `.before(emit_on_press_on_click).before(handle_tab).before(apply_keyboard_edits)` (or a first-in-Input sub-set). Intra-set ordering is NOT automatic in Bevy. (Cross-set ordering is already fine — the `BuiySet`s `Layout → Style → Input → Animate → Picking → A11yUpdate → Render` are `.chain()`ed at lib.rs:87-93, so `Input` precedes `A11yUpdate`.)
  - The new **`EditCommand::SetSelection { anchor, focus }`** is added to `text/edit/command.rs` (today's `Motion(Motion, bool)`/`Insert`/`Backspace`/`Delete`/`Enter`/`Cut`/`Copy`/`Paste`/`Undo`/`Redo`/`SelectAll`/`Escape`/`Submit` set — `command.rs:21` — cannot place an absolute range). `SetTextSelection` → `SetSelection`; `ReplaceSelectedText` → `SetSelection` then `Insert(s)` — both via `apply_tracked`, which the dispatch fn assembles with `&mut FontSystem + &mut EditContext` (editor-level read-only enforcement already lives in `apply_tracked`, `text/edit/input.rs`; the filter is additive). Shape in [action-router.md](./action-router.md#text-lowering). If this editor work can't pull into 1c, scope `SetTextSelection`/`ReplaceSelectedText` out of Phase 1 (follow-up #6) — do NOT ship the not-achievable Motion lowering. (`SetValue`-text lowers fine now via `SelectAll` + `Insert`, both existing.)
- Button keyboard activation (Enter + Space → `OnPress`), closing the two `buiy_widgets/src/button.rs` TODOs (mouse-up timing; keyboard activation) and giving `Click` a non-mouse route.
- `a11y/inprocess.rs` (NEW): `snapshot`/`perform`/`get_by_role`/`act_when_actionable`/`wait_for` + `TreeView` + `SemanticTree` + the headless injection seam feeding an `accesskit_consumer::Tree`. See [inprocess-api.md](./inprocess-api.md). **`HitTargetable` scoped honestly:** `picking::hit_test` (picking/mod.rs:37) is smallest-AABB and stacking/top-layer UNAWARE, so it cannot answer "not obscured"; Phase 1c gates AABB-only with the limitation noted; stacking-aware `hit_test` is follow-up #3.

Verification: gates #3, #4 (announcements incl. role-implied live), #6 (input replay through the headless seam), #7 (APG conformance — Button=Enter+Space, asymmetries enforced). The read-only-rejects-`SetValue` re-check is verified by replaying `SetValue` against a read-only TextInput and asserting the guidance error/no-op.

### Phase 1d — the APG widget catalog

Real APG widgets in `buiy_widgets` (each: bundle assembles the decomposed components, `impl A11yContract`):

- **Checkbox** (Checkbox; `{Click, Focus, Blur}`; `A11yToggled` incl. Mixed).
- **Switch** (Switch; `{Click, Focus, Blur}`; binary `A11yToggled`).
- **Slider** (Slider; `{Increment, Decrement, SetValue, Focus, Blur}`; `A11yValue` + `A11yOrientation`; multi-thumb = one Slider node per thumb under a Group).
- **Disclosure/Accordion** (trigger Button + `{Expand, Collapse, Click, Focus, Blur}` + `A11yExpanded` + `controls=[panel]`; panel Region; accordion = N disclosures nested as a real subtree).
- **Dialog** (Dialog/AlertDialog; `A11yModal` + `labelled_by`/`described_by`; invoker advertises `{Click}` + `controls=[dialog]`; focus-trap/Esc/restore are Buiy's overlay state machine).
- **Tooltip-trigger** (`{ShowTooltip, HideTooltip, Focus, Blur}` + `described_by(tooltip)`; tooltip node Role::Tooltip).
- **TextInput** off the `A11yRole::Text` stopgap (`buiy_widgets/src/text_input.rs:83-88`): role split + `A11yTextValue` (synced from `TextEditState`) + `A11yPlaceholder` + optional `A11yReadOnly`/`Required`/`Invalid`/`AutoComplete` + `error_message`; actions `{SetValue, SetTextSelection, ReplaceSelectedText, Focus, Blur}` lowering to `EditCommand` via `apply_tracked`.

Each widget ships its **#3 fixture** (role+name+state+actions+ref) and **#7 fixture** (every documented key, both Enter AND Space, asserting the APG-correct transition). See [widget-contracts.md](./widget-contracts.md), [verification.md](./verification.md).

### Phase 2 — opt-in transport (`buiy_mcp`)

Networked agent surface over the unchanged in-process contract (LOCKED #1). Foundation non-goal #1 — an opt-in companion crate.

- `buiy_mcp` crate: socket transport (React DevTools Bridge/Wall over the exact `a11y::inprocess` contract), MCP tool envelope (`snapshot`/`perform`/`click`/`type`/`set_value`/`focus`/`expand`/`wait_for`/`get_by_role`), capability-tier gating, versioned handshake, auth, push tree-deltas via Bevy change detection, and the structured-app-verb RPC lane escaping `CustomAction`'s i32 limit. See [mcp-companion.md](./mcp-companion.md).
- **Prerequisite:** create the transport `docs/prior-art/` folders via `researching-prior-art` (MCP/`rmcp`, Playwright-MCP, React DevTools Bridge/Wall, BRP) before building — [README.md](./README.md) research-debt note.
- Several **named follow-ups** layer here.

---

## Named follow-ups (deferred, not in Phase 1)

Most land in or alongside Phase 2.

1. **Author-supplied test-ids over the NodeId ref.** Phase 1 addresses by canonical NodeId (`entity.to_bits()+1`) — session-stable, not human-stable. Test-ids sit in `get_by_role`'s tie-break slot ([inprocess-api.md](./inprocess-api.md)).
2. **Multi-window per-WindowId tree keying.** `ROOT_NODE_ID` has no window discriminator and `node_id_for` has no per-WindowId keying. Phase 1 is single-window-scoped ([semantic-tree.md](./semantic-tree.md) §7.2).
3. **Stacking-aware `hit_test` for `HitTargetable`.** `picking::hit_test` is smallest-AABB, not stacking/top-layer aware; new stacking-aware hit-testing is required before `HitTargetable` means "not obscured." Until then AABB-only with the limitation documented ([inprocess-api.md](./inprocess-api.md)).
4. **Lazy `TreeUpdate` diffing gated on `AccessibilityRequested`.** Phase 1 keeps rebuild-each-frame; pull forward only if per-frame rebuild cost is unacceptable (open question 2).
5. **Richer `owns`-edge cases.** The once-logged drop on cycle/duplicate-parent is the Phase 1 guard; richer portalled-dialog/accordion interactions harden later.
6. **The `EditCommand::SetSelection` editor work, if not pulled into Phase 1c.** If it can't land in 1c, `SetTextSelection`/`ReplaceSelectedText` are scoped out of Phase 1 (the other TextInput actions still ship). Do not ship a non-achievable lowering.

---

## Open questions (resolve at implementation time)

1. **Actionability `Stable` signal vs layout internals.** "Same bounds across two frames" is DOM-borrowed. Verify whether `ResolvedLayout`/the layout pipeline exposes a "settled" signal to read directly (cleaner than sampling). Check; do not assume.
2. **Per-frame rebuild cost vs lazy diffing.** `build_tree` rebuilds every frame and widens. Confirm the cost is acceptable for target sizes, or pull follow-up #4 forward.
3. **Multi-window keying.** Confirm no Phase-1 fixture needs multi-window before relying on single-window scoping (follow-up #2).
4. **`owns` acyclicity edge cases.** Validate the once-logged-drop guard covers realistic accordion/dialog/portal interactions.
5. **`get_by_role` tie-break + test-id disambiguator.** Strict single-match errors loudly on two same-role+name nodes. Confirm the error UX is good enough for Phase-1 authoring until test-ids (follow-up #1).
6. **Static registry vs per-entity contract handle.** The registry is a static table; cannot express two widgets sharing a role with different honoring. Confirm no Phase-1 widget needs that; if one does, it becomes a per-entity contract-handle component.

---

## Risks {#risks}

1. **Wide `build_tree` query tuple / arity.** ~22 components widen the tuple and `A11yNodeView`; Bevy query-arity limits and `#[allow(clippy::type_complexity)]` pressure grow. Mitigated by `states.rs`/`relations.rs` grouping, a registration helper, possibly a `QueryData`-derive struct — but the wide tuple is real maintenance surface.
2. **One-frame inbound latency (winit path).** `poll_receivers` writes in the previous frame's PostUpdate; the router reads at Input start. In-process tests must tick deterministically; the direct `dispatch_action_request` fn sidesteps this for tests, but the latency must be documented so winit-path agents don't mistake it for a bug.
3. **Same-frame despawn races + loud `ActionError`.** A request can race a same-frame despawn under the winit latency; the liveness guard yields a soft "stale ref." The typed `ActionError` (`NotFound`/`Unsupported`/`NotActionable`/`BadData`) must propagate **loudly** through the in-process API and (later) MCP, or the agent silently no-ops.
4. **NodeId not human-stable until test-ids.** Phase-1 agents address by `entity.to_bits()+1` — session-stable, not human-stable. Acceptable for tests, awkward for hand-written scripts until follow-up #1.
5. **0.24 / BSN-bump coupling.** The 0.24 setter/Action vocabulary rides the in-flight Bevy 0.19-rc.3 bump (separate campaign, user go-ahead). If it slips, the `translate.rs` derive fold must be written against 0.21 and migrated. Verify against `Cargo.lock`, not docs.rs (the `set_live_atomic` correction is the canonical example). Keep the fold isolated so the migration is one file.
6. **Supersede-don't-contradict docs hygiene.** Folding the `buiy-accessibility-design` roadmap slot into this spec (LOCKED #5) means this spec must **supersede** that slot, not leave a parallel unfilled entry. `docs/README.md`'s index and any cross-references must be updated in the same change (per `organizing-buiy-docs`), or readers see two parallel a11y entries. Because the slot was only ever a forward reference (foundation `accessibility.md` + code comments), never a catalog entry, the index change is a fresh **add** that claims the a11y territory. The a11y module keeps a clean `buiy_core/src/a11y/` boundary so the future `buiy_a11y` crate lift stays mechanical.
