# MVU-as-core research — a11y / agent-interface write-side

> Research-stage artifact for **prototype-3** (MVU as the CORE primary interface).
> Area: **a11y-agent-interface-writeside** — map the agent-interface / a11y
> subsystem and how a core Msg write-log UNIFIES it (the write-side dual of the
> semantic read-tree). Code claims are against **current `origin/main`** in this
> worktree (`/mnt/storage/projects/buiy/.claude/worktrees/mvu-core`). Proto-1/2
> citations are the seed worktree (`…/state-mgmt-elm-prototype`).
>
> Charter inputs hit: signal **#2** (agent actions through `update`) and the
> re-decide item **"`LogicalId` unified with the agent-interface test-id space."**
> Mandate: re-decide every choice, do not rubber-stamp the charter.

---

## TL;DR (the load-bearing findings)

1. **There is no `LogicalId` and no test-id in current core.** The grep is empty
   (`grep -rn "LogicalId|test_id|TestId" crates` → 0 hits). The *only* identity in
   core is the AccessKit `NodeId` derived from `Entity`:
   `node_id_for(entity) = NodeId(entity.to_bits()+1)` (`a11y/translate.rs:17-21`),
   inverted by `entity_for_node_id` (`:29-34`). It is **session-stable, not
   human-stable, not cross-session-stable** — the translate doc says so
   (`:14-16`), and the spec lists author test-ids as a *Phase-2 follow-up*
   (`phasing.md:121,146`; `inprocess-api.md:79`; `mcp-companion.md:55,103`).
   `LogicalId` exists only in proto-2 (`examples/mvu_native/src/runtime.rs:95`).

2. **The inbound action seam already exists and is already the single ingress**,
   but it is **NOT a funnel** — it lowers each verb into a *different, direct*
   sink. `dispatch_action_request(&mut World, &ActionRequest)` (`a11y/action.rs:151`)
   is the locked headless act seam. Its dispatch (`:223-305`) fans out:
   `Focus/Blur → FocusedEntity` resource; `Expand/Collapse → A11yExpanded`
   component; `ShowTooltip/HideTooltip → CssVisibility`; `Click → contract honor →
   Messages<OnPress>`; slider `Increment/Decrement/SetValue → A11yValue` mutated
   in-place (`contract.rs:384-411`); text `SetValue → TextEditState::apply`
   (`contract.rs:450-495`). **Only `Click` is a recorded message; everything else
   is a direct world write.**

3. **The `ActionRequest` stream is NOT a complete input log.** Pointer and button-
   keyboard activation write `OnPress` *directly* (`picking/activation.rs:66`,
   `a11y/action.rs:432`), bypassing `ActionRequest` entirely. `ActionRequest` is
   minted only by the in-process driver (`inprocess.rs:381`) and slider-keyboard
   (`action.rs:575`). So "record the ActionRequest stream" captures **agent/AT
   actions only**, not real-user pointer/keyboard input.

4. **The write-side dual is the thing missing from core.** The read-tree
   (`build_tree` → `build_tree_update` → `accesskit_consumer::Tree`) is fully core
   and verified. Its inverse — *one recorded write-log keyed by one stable id that
   every modality funnels through* — is exactly what proto-2 built
   (`runtime.rs`: `MsgLog` + `LogicalId` + single ordered `Drain`) and what the
   optional placement cannot host without a dependency inversion.

5. **The charter's "unify `LogicalId` with the test-id space" is correct and
   underspecified.** There are two *would-be* stable-id systems on a collision
   course — the agent-interface's planned author test-id and the MVU log's
   `LogicalId` — both wanting the same thing (a session/cross-session-stable,
   author-assigned handle on an entity). They should be **one** space. But which
   id becomes the *AccessKit wire ref* is a real, risk-bearing decision (the AT
   path is mature and golden-keyed on `bits+1`).

---

## (a) How the read-tree is built and where actions/inputs enter

### Outbound (read) path — fully core, verified

`build_tree` (`a11y/mod.rs:445-637`) runs in `BuiySet::A11yUpdate`, scanning a
single wide `#[derive(QueryData)] A11yNodeQuery` (`mod.rs:352-388`) over every
entity, projecting each a11y-bearing entity into an `A11yNodeView`
(`mod.rs:149-258`), pruning `A11yHidden` subtrees, and collapsing presentational
wrappers into real parent/child edges. The result list lives in the
`A11yTreeBuilder` resource (`mod.rs:261-270`). `translate::build_tree_update`
(`translate.rs:284-328`) folds that list into an `accesskit::TreeUpdate` (one
synthetic `Role::Window` root over the top-level nodes), which both the live
`accesskit_winit::Adapter` and the in-process consumer (`inprocess.rs:229-233`)
ingest. The translation is winit-free and `Entity`-free at the seam — relations
and nesting are resolved `Entity → NodeId` inside `build_tree`/`build_tree_update`
(`mod.rs:496-504`, `translate.rs:301-306`).

### Inbound (action/input) entry points

| Ingress | Location | Lowers into |
|---|---|---|
| `route_action_requests` (system) | `action.rs:323-344` | drains `Messages<ActionRequestWrapper>` (the winit a11y channel) → `dispatch_action_request` per request |
| `dispatch_action_request` (headless seam) | `action.rs:151-306` | the per-verb sinks below — **the single act primitive** |
| `inprocess::perform` (driver) | `inprocess.rs:375-389` | builds an `ActionRequest`, calls `dispatch_action_request`, re-snapshots (act-then-observe) |
| `keyboard_activation` (system) | `action.rs:398-435` | **writes `OnPress` directly** (APG Enter/Space keymap) — bypasses the seam |
| `slider_keyboard` (system) | `action.rs:537-588` | mints an `ActionRequest`, routes *through* `dispatch_action_request` |
| pointer click producer | `picking/activation.rs:57-66` | **writes `OnPress` directly** — bypasses the seam |

The per-verb dispatch in `dispatch_action_request` (`action.rs:223-305`):

```
Focus/Blur          → set/clear FocusedEntity resource         (action.rs:231-249)
Expand/Collapse     → set A11yExpanded.0 component bool         (action.rs:259-271)
ShowTooltip/Hide    → write CssVisibility on described_by node  (action.rs:283-294)
_ (Click, Slider…)  → contract_for(role).honor(world, …)       (action.rs:296-304)
   Button/Checkbox/Switch.honor(Click) → emit_on_press → Messages<OnPress>   (contract.rs:179-183)
   Slider.honor(Inc/Dec/Set)            → A11yValue mutated in place          (contract.rs:384-411)
   TextInput.honor(SetValue)            → TextEditState::apply via EditCommand (contract.rs:450-495)
```

**Key structural fact:** the convergence the design advertises ("pointer, keyboard,
AT-Click all converge on one `OnPress` route", `interaction.rs:3-10`,
`contract.rs:171-183`) is **real but narrow** — it holds only for *activation*
(`Click`). Value, text, focus, expand, and tooltip verbs each terminate in their
own direct world write. There is no single mutation point and no log.

---

## (b) The identity space(s) — and whether they can be ONE

### What exists today (three things, none of them a stable author id)

1. **AccessKit `NodeId` = `entity.to_bits()+1`** (`translate.rs:17-21`). The
   canonical addressing ref everywhere: the in-process `SemanticNode.r#ref`
   (`inprocess.rs:162-185,279-282`), the inbound `ActionRequest.target_node`
   (`action.rs:152-159`), and the planned MCP wire `ref` (`mcp-companion.md:55`).
   **Session-stable only** — entity bits depend on spawn/generation order, so the
   same logical widget gets a *different* ref in a fresh process. The agent report
   confirms (`reports/2026-06-18-agent-interaction-surface-research.md:88`) and
   even records an off-by-one where `buiy_verify::a11y::snapshot_tree` emitted raw
   `to_bits()` while the router used `bits+1` — *two serializers disagreeing on the
   key* (`:89`), the exact failure mode a split id space invites.

2. **`get_by_role(role, name, state)`** (`inprocess.rs:463-487`) — a *content
   locator*, Playwright strict-single-match, **not an id**. Ambiguity (0 or >1) is
   a typed `NotFound`, by design a test bug (`inprocess-api.md:72-79`). This is the
   "test-id space" the charter names, but it is a *query over content*, fragile to
   i18n/duplication, and explicitly slotted to *host* a future author test-id as
   its tie-break (`phasing.md:136`).

3. **Author-supplied test-ids** — **do not exist.** Named Phase-2 follow-up #1 in
   four places (`phasing.md:121,136`; `inprocess-api.md:79`; `mcp-companion.md:103`;
   `README.md:36`): "a human-stable layer above the session-stable NodeId."

And in proto-2, **`LogicalId(u64)`** (`runtime.rs:91-99`): a session-stable logical
identity the `MsgLog` keys on (`runtime.rs:316-317`) so a replay in a *fresh
process* with different `Entity` allocation still lands correctly — proven
byte-identical cross-process in `bin/replay_harness.rs`. Its own doc comment says
it is "aligned (by intent) with the agent-interface test-id space"
(`runtime.rs:91-93`). The draft spec makes the alignment explicit:
"Identity = a stable `LogicalId` aligned to the agent-interface test-id space"
(`2026-06-26-buiy-state-management-design.md:139`).

### Can they be ONE space?

**Yes — and they should be.** All three "stable id" needs are the same need:

- the agent/AT needs a ref that round-trips a recorded action across sessions
  (today's `bits+1` cannot);
- `get_by_role` needs a tie-break that is author-stable, not content-derived
  (the named follow-up);
- the Msg log needs a key that survives respawn (proto-2's `LogicalId`).

A single author-assignable `LogicalId` keyed to an entity satisfies all three. The
existing `Entity ↔ NodeId` bijection (`translate.rs`) already proves the seam; the
only change is to anchor the stable end of that bijection to an **author id +
deterministic fallback** rather than raw entity bits. The off-by-one bug (`:89`)
is positive evidence that *two* id derivations over the same entities drift in
practice; collapsing to one is a simplification cascade, not a feature.

**The non-trivial decision is *which* id becomes the AccessKit wire ref.** Two
viable shapes (see Decision 1).

---

## (c) Why "action lowering through `update`" makes agent-driving reproducible — and why an optional top-layer cannot do it

### Reproducibility = a complete, replayable write-log over a stable id

Proto-2's invariant (`runtime.rs:11`, `:151-162`): **the single ordered `Drain` is
the only place a model changes**; observers/callbacks/press handlers may *only*
`enqueue` (`runtime.rs:172-178`). The drain records every `(LogicalId, Msg, seq)`
into `MsgLog` (`runtime.rs:300-333`) before folding. Replay re-folds the log keyed
by `LogicalId` in a fresh process → byte-identical state (`bin/replay_harness.rs`).
Reproducibility has two requirements, and *both* are write-side:

1. **A single mutation point** so replay re-running the log reconstructs state
   deterministically (no out-of-band writes to miss). Today this is violated: the
   inbound seam writes five different sinks directly (finding 2), and pointer/
   keyboard write `OnPress`/edits without ever minting an `ActionRequest`
   (finding 3). "Record the `ActionRequest` stream" is therefore **incomplete** —
   it omits all real-user input and all non-`Click` state transitions.

2. **A session-independent id** so a recorded action re-targets the same logical
   widget next session. Today this is violated: the ref is `entity.to_bits()+1`.

"Action lowering through `update`" fixes both at once: the inbound seam, instead of
writing a sink directly, **enqueues a `Msg` onto the target model's inbox**, so the
verb and its downstream model transition both flow through the one recorded drain,
keyed by the one stable id. The read-tree (outbound) and the write-log (inbound)
become exact duals over a single identity space.

### Why an optional top-layer structurally cannot

**Dependency direction.** `buiy_core` sits at the bottom of the graph: it depends
on `accesskit`/`accesskit_consumer` and nothing upward (`crates/buiy_core/Cargo.toml`);
`buiy_widgets`, `buiy`, `buiy_bsn`, `buiy_verify` all depend *down* on
`buiy_core`. The act seam `dispatch_action_request` lives in core
(`a11y/action.rs`) and lowers into core sinks. For agent actions to lower *through*
the funnel, the funnel must be reachable **from core** — i.e. at or below
`buiy_core`. An optional `buiy_mvu` crate *above* core cannot be called by core
without inverting the dependency (a cycle).

Proto-2 demonstrates exactly the half that an optional layer *can* do and the half
it cannot: `routing.rs:13` literally `use buiy_core::interaction::OnPress;` — it
bridges core's `OnPress` *up* into its own funnel via an observer
(`routing.rs:32-45`). That works because it depends *down*. But it can only catch
verbs that already reach a message channel (`OnPress`). The direct-sink verbs —
`Focus`, `Expand`, slider `A11yValue`, text `TextEditState` — never hit a channel,
so an optional bridge **structurally cannot record them**. Worse, a second bridge
reading the `ActionRequest` channel would be a *competing ingress*, violating the
spec's LOCKED #6 "single inbound ingress" (`action.rs:14-18`). This is precisely
charter signal #2's "dependency points the wrong way," confirmed in code.

---

## (d) What co-locating the Msg write-log with the AccessKit read-tree in core buys

1. **One identity space, enforced by construction.** `node_id_for`/`entity_for_node_id`
   already mediate `Entity ↔ NodeId` in core; the log key, the addressing ref, and
   the `get_by_role` tie-break collapse onto that *one* bijection instead of three
   parallel derivations (the off-by-one at `:89` is the cost of not doing this).

2. **A complete write-log.** With the inbound seam lowering through the drain
   (Decision 2) *and* the pointer/keyboard producers enqueuing instead of writing
   `OnPress`/edits directly (open question), **every** state transition —
   agent-, AT-, pointer-, keyboard-originated — is one recorded `Msg` over one id.
   This closes the `TextEditState` crux on the write side (charter signal #3):
   widget-internal focus/edit/selection/value transitions become first-class log
   entries, so whole-UI replay/time-travel/hot-reload reconstruct them.

3. **The read/write duality lives in one place.** `inprocess::snapshot` already
   reads the canonical tree back through the production consumer (`inprocess.rs:325-357`);
   a co-located log makes `perform` *record* on the same id the snapshot *reads* —
   act-then-observe and record-then-replay share one vocabulary. The MCP transport
   stays an opt-in crate above core that wraps the unchanged driver
   (`inprocess.rs:1-3`, `mcp-companion.md`) — genuinely optional, because it only
   *consumes* the core driver; it never needs core to depend on it.

4. **No competing ingress.** Keeping the seam and the funnel co-located preserves
   LOCKED #6 (one ingress): the single core seam *is* the funnel entry, rather than
   an optional layer racing the core seam to the same world.

---

## Recommendations

### Decision 1 — Unify identity into ONE author-assignable stable id (layered ref)

**Recommend.** Introduce one core `LogicalId` (name TBD — "NodeKey"/"Ref") that is
simultaneously (i) the Msg-log key, (ii) the `get_by_role` tie-break / addressing
id used by the in-process driver + MCP, and (iii) author-assignable with a
**deterministic structural fallback** (parent id + local key, so unlabeled widgets
still get a session-stable id and dynamic lists get keyed-reconcile ids).
**Layer it:** keep `node_id_for(entity)=bits+1` as the *AT-facing winit wire
NodeId* (ATs never compare across sessions — `translate.rs:14`), and resolve the
stable `LogicalId` to `Entity` through a registry resource for the agent/test/log
path. One resolver, two faces, no destabilization of the mature AT goldens.

- *Rationale.* Two stable-id systems are already converging (agent test-id
  follow-up + MVU `LogicalId`); both want an author-assigned, respawn-surviving
  handle. One space removes the drift class the `:89` off-by-one exemplifies and
  makes a recorded action re-target the same widget next session.
- *Runner-up — unify into NodeId itself* (make `LogicalId` *be* the `NodeId`, drop
  `bits+1`). Cleaner conceptually (one u64), but it rewrites the AccessKit wire ref
  the entire verified AT path + every a11y golden is keyed on, and turns the
  pure-math inverse into a registry lookup on the hot AT path. Higher risk for
  marginal gain; revisit post-migration.
- *Runner-up — keep them separate* (status quo: `bits+1` ref, private MVU
  `LogicalId`, content-only locators). Rejected: cross-session record/replay,
  time-travel across respawn, and hot-reload are impossible when the ref is entity
  bits, and two id mappings must be hand-synced (the `:89` failure mode at scale).

### Decision 2 — Lower model-backed verbs through `update`; leaves stay direct

**Recommend.** For a **model-backed** widget, `dispatch_action_request` (and the
`A11yContract::honor` arms) lower the verb to an **`enqueue(Msg)`** onto that
widget's MVU inbox — the recorded drain is the only mutation point — schedule the
drain later in the *same* frame (proto-2's `Enqueue → Drain → Bind` chain,
`runtime.rs:411-418`) so the same-frame outbound reflection the router guarantees
(`mod.rs:298-323`) is preserved. A widget with **no model** (imperative leaf)
keeps today's direct-sink honor (the escape hatch).

- *Rationale.* Today only `Click→OnPress` is recorded; `Focus`/`Expand`/slider
  `A11yValue`/text `TextEditState` are direct writes (finding 2). Routing
  model-backed verbs through the drain makes the verb *and* its model transition
  both recorded and replayable — the write-side dual of the read-tree.
- *Runner-up — record-and-replay the `ActionRequest` stream, no funnel change.*
  Rejected: it captures agent/AT actions only (pointer/keyboard write `OnPress`
  directly, never an `ActionRequest` — finding 3), and replay re-dispatches against
  live world state, inheriting every direct-write nondeterminism the funnel exists
  to eliminate.
- *Caveat (perf, must surface).* High-frequency verbs (slider drag, per-keystroke
  text) through enqueue+drain+Reflect-record threaten the 60 Hz floor. The a11y
  *seam* is not the bottleneck (it is already one exclusive `&mut World` drain);
  the cost is the funnel's Reflect-serialize. Mitigate with per-verb record
  sampling/opt-out (mutate the model in-drain without serializing hot verbs every
  frame) — charter explicitly calls for this; gate with iai-callgrind. **Owned by
  the perf-area research; flagged here as the binding constraint on this decision.**

### Decision 3 — Log + driver in core; MCP transport opt-in above

**Recommend.** Keep the in-process driver in core (already at `a11y/inprocess.rs`)
and add the Msg-log resource + ordered drain to core (proto-2's `MsgLog` +
`MvuSet::Drain`). Keep the MCP companion an opt-in crate *above* core that wraps
the driver unchanged.

- *Rationale.* Dependency direction (finding c): core's single seam must reach the
  funnel, so the funnel is ≤ core; the transport only consumes the driver, so it is
  > core. This is the only layering that gives a complete log without a cycle and
  without a competing ingress.
- *Runner-up — log in an optional `buiy_mvu`, bridge via an observer* (proto-2's
  shape). Rejected for the inbound seam: an upward bridge can only catch
  channel-borne verbs (`OnPress`); it cannot record the direct-sink verbs, and a
  second `ActionRequest`-reading bridge violates LOCKED #6.

---

## Risks

| # | Severity | Risk | Evidence | Candidate mitigation |
|---|---|---|---|---|
| R1 | **HIGH** | Changing the AccessKit wire ref from `bits+1` to a registry-resolved stable id destabilizes the mature, golden-keyed AT path. | `translate.rs:14-21` pure-math round-trip; `reports/…2026-06-18…:88-89` off-by-one already bit; every a11y snapshot/golden keyed on `bits+1`. | Layer (Decision 1): keep `bits+1` as the AT wire NodeId; add `LogicalId` as the agent/test/log id via a resolver registry; unify the *tie-break + log key* first, defer touching the AT wire ref. |
| R2 | **HIGH** | High-frequency verbs (slider drag, per-keystroke text) through enqueue→drain→Reflect-record breach the 60 Hz floor. | `contract.rs:384-411` (slider) & `:450-495` (text) mutate directly today *for same-frame reasons*; charter PERFORMANCE constraint. | Per-verb record sampling/opt-out; mutate model in-drain without serializing hot verbs every frame; iai-callgrind gate. Coordinate with perf-area research. |
| R3 | **MED** | Escape-hatch leaves that mutate their own state outside the funnel reopen the incomplete-log gap. | Charter wants both MVU-primary *and* raw-ECS escape; finding 2 shows direct sinks are the current norm. | Document the boundary: the log is complete only for model-backed widgets. Offer an opt-in Reflect-snapshot diff for imperative leaves that still want replay — a *different* mechanism; surface as open question. |
| R4 | **MED** | A stable author-id space needs uniqueness; auto-derived structural fallback collides on dynamic lists (N rows). | proto-2 hand-assigned `LogicalId(1)/(2)` (`main.rs:55`, `lib.rs`); draft spec names keyed-reconcile of dynamic children. | Hierarchical ids (parent id + local key) + the draft spec's keyed-reconcile; reuse `get_by_role`'s "ambiguity-is-a-bug" stance (`inprocess.rs:480-485`). |
| R5 | **MED** | WASM: in-memory log + registry add no new obstacle, but proto-2's *cross-process* replay writes a RON file — no FS on web. | `bin/replay_harness.rs` persists RON; charter WASM "zero new obstacles". | Keep the log an in-memory `Vec<LoggedEntry>` (it already is, `runtime.rs:111-117`); make persistence a separate, transport-agnostic concern (serialize over postMessage/IndexedDB, not `std::fs`). |
| R6 | LOW–MED | "MCP wraps the driver unchanged" is unproven — `buiy_mcp` is 0% built. | `docs/README.md:208` (agent-interface P2/`buiy_mcp` = 0%); crate absent from `crates/`. | Low risk to *these* decisions (driver/log are core regardless). Keep the in-process driver API stable; validate the wrap when the transport is specced. |

---

## Open questions

1. **Which id is the AccessKit wire ref** — layered (`bits+1` for AT + `LogicalId`
   for agent/log, Decision 1 primary) or unified (`LogicalId` *is* the `NodeId`,
   runner-up)? Needs a resolver-registry perf + golden-migration spike.
2. **Do pointer/keyboard producers also enqueue** (so the write-log is complete for
   real-user input), or does the funnel additionally record `OnPress`? Today they
   write `OnPress`/edits directly (`picking/activation.rs:66`, `action.rs:432`) and
   never mint an `ActionRequest`. Completeness of the log hinges on this.
3. **Granularity of global state.** Focus is a single `FocusedEntity` resource
   (`focus.rs:111`), not a per-widget model. Does it become a focus-*actor* model
   the funnel writes, or stay a resource the drain mutates? Same question for
   `A11yExpanded`/`CssVisibility`/tooltip state. (The charter "widget granularity"
   re-decide.)
4. **`Cmd` algebra in core.** Compound verbs (slider `SetValue` = clamp; text
   `SetValue` = `SelectAll`+`Insert`) — do they lower to one `Msg` or a `Cmd`
   sequence on the core drain? Bears on `Cmd` re-integration (`task`/`batch`/
   dead-letter) and on replay determinism.
5. **Is `TextEditState` reflectable.** Charter signal #3 + hot-reload research name
   it as un-reflected; if so, even a complete *Msg* log cannot reconstruct editor
   internals without either reflecting the state or making every editor transition
   a logged `Msg`. Confirm and decide which.
