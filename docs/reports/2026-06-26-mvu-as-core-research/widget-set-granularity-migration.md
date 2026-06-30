# MVU-as-core research — Widget-set inventory, MVU granularity, and migration model

> Research-stage artifact for proto-3 (`/staged-development`). READ + ANALYZE only;
> no production code changed. Code claims = current `origin/main` worktree
> (`/mnt/storage/projects/buiy/.claude/worktrees/mvu-core`). Charter rule honored:
> every choice re-decided; the charter is challenged where the evidence cuts against it.
>
> Scope (my slice): inventory every widget's owned state + self-update + exposed
> surface (file:line); the double-write / flicker race and how MVU-through-funnel
> fixes it; THE granularity decision (every widget a Model+reducer vs leaf widgets
> route); the migration model (incremental vs rewrite, phased).

---

## 0. TL;DR

1. **Granularity: Model-per-*stateful*-widget, NOT Model-per-entity.** Leaf controls
   (Button/Checkbox/Switch/Slider/Disclosure) keep their existing tiny state
   *component* (`A11yToggled` / `A11yValue` / `A11yExpanded`) and only **route** an
   activation Msg into the funnel; the **drain becomes the single writer** of that
   component. Genuine multi-entity state machines (**Menu, Dialog**) become real
   `Model`+reducers — they are where "widget = actor" actually pays for itself.
   Composites (`meter`, `table_row`) stay imperative (the escape-hatch exemplar).
   The literal charter framing ("*every* widget is an actor", advantage #2) is the
   **runner-up** — rejected because Button has no state and the hot-path
   Reflect-serialize cost on trivial folds threatens the 60 Hz floor.

2. **The double-write/flicker risk is REAL and already costing complexity in
   `main`** — it is not hypothetical. The strongest evidence is
   `menu::sync_menu_dismissed` (`crates/buiy_widgets/src/menu.rs:615`), an entire
   reconciliation system that exists *only* because two writers
   (`MenuButton.A11yExpanded` and `Menu.CssVisibility`) update the same logical
   "menu-open" state independently. MVU-through-funnel (single drain writer) deletes
   this class of bug — but the cure is "single writer," which the **tiered** model
   delivers; it does **not** require "every widget is a Model."

3. **Migration = INCREMENTAL, never big-bang rewrite.** The state components already
   exist and are already single-source-of-truth read by `Changed`-gated visuals; the
   migration mostly **reroutes the writers** through one drain and makes activation a
   recordable Msg. `#[require]` contracts, the visuals, and the a11y fold are
   untouched. The mature, verified surface (11,374 LOC widget src + 3,905 LOC widget
   tests + the gallery) is preserved, not re-derived.

4. **The `TextEditState` crux is structurally unrecordable as a Reflect Model**
   (`crates/buiy_core/src/text/edit/state.rs:89-92` — *not* reflect-registered; it
   owns a `cosmic_text::Editor`). The "complete recordable stream" thesis (charter
   advantage #1) only closes here via **command-sourcing**: record the `EditCommand`
   vocabulary, replay by re-folding commands. Do **not** try to serialize the editor.

---

## 1. Widget-set inventory (state owned / self-update / exposed surface)

Granularity legend:
- **Router** = owns no mutable model state; converges activation on the shared
  `OnPress` sink. Already MVU-shaped.
- **Leaf-stateful** = owns exactly one small ARIA-state component that is *already*
  the single source of truth (the visual reads it via `Changed`). Self-updates via a
  shared, role-keyed consumer system.
- **Machine** = a genuine multi-entity / multi-component / multi-system state machine
  with focus + visibility + relation juggling. The double-write hotspots.
- **Imperative** = a `fn(&mut World) -> Entity` builder with imperative setters; no
  contract, no funnel.

| Widget (file) | State it OWNS | How it SELF-UPDATES | What it EXPOSES | Granularity |
|---|---|---|---|---|
| **Button** `button.rs:59` | none (only the activation sink) | activation lowers via `buiy_core::picking::pointer_click_emits_on_press` (`button.rs:132-145`); keyboard + AT producers also write `OnPress`. No widget system. | `Messages<OnPress>(entity)` (`interaction.rs:29`) | **Router** |
| **Checkbox** `checkbox.rs:78` | `A11yToggled` (tri-state `{False,True,Mixed}`, `checkbox.rs:74`) | shared `advance_toggle_on_press` (`lib.rs:78-96`) reads `OnPress`, calls `A11yToggled::advance_checkbox` (`states.rs:46`). Visual `update_checkbox_visual` reads `Changed<A11yToggled>` (`checkbox.rs:196`). | `A11yToggled` component | **Leaf-stateful** |
| **Switch** `switch.rs:81` | `A11yToggled` (binary) | SAME `advance_toggle_on_press` → `A11yToggled::toggle_switch` (`states.rs:60`). Visual `update_switch_visual` reads `Changed<A11yToggled>`, slides thumb (`switch.rs:245`). | `A11yToggled` | **Leaf-stateful** |
| **Slider** `slider.rs:94` | `A11yValue` (co-varying `now/min/max/step/jump/text`, `states.rs:157`) | NOT via `OnPress` — the contract `honor` + `slider_keyboard` (in `buiy_core`) mutate `A11yValue` directly (`slider.rs:9-16`; `A11yValue::increment` etc `states.rs:204-237`). Visual `update_slider_visual` reads `Changed<A11yValue>` (`slider.rs:286`). | `A11yValue` | **Leaf-stateful** |
| **Disclosure** `disclosure.rs:108` | `A11yExpanded` (`states.rs:72`) | shared `advance_expanded_on_press` (`lib.rs:118-127`) flips `!expanded.0`; ALSO router-generic `Expand`/`Collapse`. Visual `update_disclosure_visual` reads `Changed<A11yExpanded>`, rotates caret + flips panel `CssVisibility` (`disclosure.rs:338`). Post-spawn `wire_disclosure_controls` (`Added<Children>`, `disclosure.rs:298`). | `A11yExpanded`, controlled `DisclosurePanel` | **Leaf-stateful (+1 relation seam)** |
| **TextInput** `text_input.rs:90` | `TextEditState` (cosmic `Editor`, **un-Reflectable**, `state.rs:92`) + `Placeholder` | the `buiy_core` editor mechanism (input/keymap/IME) mutates the editor; `sync_text_input_a11y` (`text_input.rs:172`) mirrors `editor.value()` → `A11yTextValue` every frame (difference-guarded — **cannot** use `Changed` because the editor mutates behind `&`, `text_input.rs:167-171`). | `A11yTextValue` projection; `EditSubmitted` for SingleLine | **Machine (the crux)** |
| **ScrollArea** `scroll_area.rs:56` | `ScrollOffset` + `ScrollExtent` + `A11yScroll` | wheel/keyboard handlers in `buiy_core::scroll`; `update_a11y_scroll` syncs `A11yScroll` from offset+extent. No activation contract. | scroll position via `A11yScroll` (`states.rs:338`) | **Leaf-stateful (container)** |
| **Menu / MenuButton / MenuItem** `menu.rs` | `MenuButton.A11yExpanded` (open) + `Menu.CssVisibility` (open) + `Menu.A11yRelations.active_descendant` + `Popover/Anchor` | a 6-system machine: `sync_menu_open` (`menu.rs:513`), `sync_menu_dismissed` (`menu.rs:615`, **reconciliation patch**), `menu_keyboard_nav` (exclusive `&mut World`, `menu.rs:672`), `wire_menu_button` (`menu.rs:360`), `guard_menu_clicks` (`menu.rs:403`), `menu_item_click_emits_on_press` (`menu.rs:441`). Writes the **global** `FocusedEntity` resource. | `OnPress(item)`; open state | **Machine** |
| **Dialog** `dialog.rs:102` | `CssVisibility` (open) + `FocusScope::trap` + `FocusReturn` + `A11yModal` + `PendingFocus` | a 5-system machine: `open_dialog_on_invoker_press` (`dialog.rs:363`), `apply_dialog_modal_state` (reacts `Changed<CssVisibility>`, `dialog.rs:411`), `close_dialog_on_escape` (`dialog.rs:516`), `close_dialog_on_button` (`dialog.rs:545`), `resolve_pending_focus` (retry-budget, `dialog.rs:476`). Writes global `FocusedEntity` + sprays `A11yHidden` on sibling roots. | open state; `FocusReturn` | **Machine** |
| **Tooltip** `tooltip.rs:86` | `A11yTooltipHost` capability + tooltip-node `CssVisibility` | router-generic `ShowTooltip`/`HideTooltip` honor flips `CssVisibility`; `wire_tooltip_described_by` (`tooltip.rs:212`) + `position_tooltip` (`tooltip.rs:309`). | show/hide | **Leaf-stateful (capability) + overlay** |
| **Popover** `popover.rs:145` | `anchor`/`positions`/`window_margin` + `Anchor` + `Stacking` + `LightDismiss` + `CssVisibility` | `position_popover` lowers to the layout `Anchor` each frame (`popover.rs:229`), enforces top-layer + trigger-exempt dismiss. | open state; positioning substrate | **Machine (substrate)** |
| **LightDismiss** `dismiss.rs:51` | (cross-cutting) | `light_dismiss_on_press` observer (`dismiss.rs:98`) + `escape_dismiss` (`dismiss.rs:152`) write `CssVisibility::Hidden` **directly** on the top-most overlay. | overlay-close channel | **Machine (cross-cutting)** |
| **Composites** `composites.rs` | imperative markers (`MeterFill`, `TableRow`, `RowSelBar`) | imperative setters `set_meter` (`composites.rs:242`), `set_table_row_selected` (`composites.rs:762`). No contract, no funnel. | `(track, fill)` / `Entity` | **Imperative (escape hatch)** |

**Two structural observations from the inventory:**

- **Most "widget state" is already one tiny single-source-of-truth component read by
  a `Changed`-gated visual.** Checkbox/Switch/Slider/Disclosure are *already* a
  primitive MVU loop: state component (Model) + visual bind (`Changed<State>` →
  view). What is missing is only that the **writer is not unique** (see §2). This is
  the load-bearing insight for both granularity and migration.

- **The complexity is concentrated in the Machines.** Menu (810 LOC) and Dialog
  (632 LOC) hold ~70% of the stateful complexity and 100% of the reconciliation
  pathology. The leaves are cheap; the machines are where a real Model+reducer earns
  its keep.

---

## 2. The double-write / flicker race, and the funnel fix

### 2.1 The race is documented AND live in current `main`

Three independent pieces of evidence — the third is the decisive one.

1. **Proto-1 REFINE #5 (the named case).** The real `Checkbox` self-advances
   `A11yToggled` in `advance_toggle_on_press` *before* any app/parent logic sees the
   activation. If a controlling parent's model rejects the toggle, the visual has
   **already** flipped (the `Changed<A11yToggled>` gate fired) → a one-frame flicker.
   The draft spec's mitigation is a *suppression flag*: "suppress
   `advance_toggle_on_press` when a controlled marker (`OnPressMsg<M>`) is present"
   (draft spec § 12; retrospective REFINE #5).

2. **The gallery hand-rolls control around the self-update.** TodoMVC seeds completed
   rows by writing `A11yToggled::True` **directly** onto the checkbox
   (`examples/buiy_gallery/src/lib.rs:1129`), racing the widget's own
   `advance_toggle_on_press`; and it reconciles user toggles through an `intents`
   resource + apply systems (`collect_button_press` `lib.rs:1157`, `collect_*`
   `lib.rs:1135+`). That is an **app-level MVU layer hand-built on top of
   self-updating widgets** — exactly the duplication proto-3 wants to delete.

3. **`sync_menu_dismissed` is the double-write pathology, in production, with its
   reason-for-existing in the docstring** (`crates/buiy_widgets/src/menu.rs:598-635`):

   > "The C5-b light-dismiss (`dismiss.rs`) closes the open menu … by flipping the
   > menu's `CssVisibility` to `Hidden` **directly** — it does not know about the
   > button. Without this, the button's `A11yExpanded` would stay `true` after a
   > light-dismiss, desyncing the `aria-expanded` state and breaking re-open."

   Two writers (`MenuButton.A11yExpanded` via the button path; `Menu.CssVisibility`
   via the dismiss path) own one logical fact ("menu open?"), so a third system
   exists purely to keep them in lock-step — "idempotent … so it does not ping-pong
   with `sync_menu_open`." This is the canonical multi-writer reconciliation smell.
   The memory log even records that a related "anti-clobber" change once killed the
   editor Text-seed and was caught only by the GPU lane — multi-writer state is
   already drawing blood.

### 2.2 How MVU-through-funnel removes it

The funnel's invariant (proto-2 `runtime.rs:148-178`) is: **the single ordered drain
is the only place a model changes; every other system may only `enqueue`.** Apply
that to the state components:

- Light-dismiss, the button press, Escape, and AT `Collapse` all **enqueue** a
  `MenuMsg::Close` (or write `OnPress`/an intent) instead of writing a visibility/
  expanded component. The **one** reducer folds them and writes the **one** open-state
  field. There is exactly one writer ⇒ no desync ⇒ `sync_menu_dismissed` **deletes
  itself**. The `Changed<State>` visual fires once, after the fold.

- For Checkbox: "controlled" vs "self-updating" **collapses into one model**. A
  controlled checkbox is just one whose reducer-owner is an ancestor; an uncontrolled
  one folds locally. There is no `OnPressMsg<M>`-presence suppression flag to get
  right — the flicker cannot occur because the visual only ever observes a *folded*
  value. (This is strictly simpler than the draft spec's suppression mechanism; it is
  a point where the core bet genuinely simplifies the widget layer.)

**Caveat that the charter under-states:** the cure is *single writer through one
drain*, which the **tiered** model (§3) already provides. "Every widget is a Model"
is sufficient but not necessary — and it imposes the hot-path cost §4 warns about.

---

## 3. THE decision — every widget a Model+reducer, or leaves route?

### Recommendation: **TIERED. Model-per-stateful-widget; leaves route; composites stay imperative.**

Mapping the inventory tiers onto the runtime:

| Tier | Widgets | Runtime treatment |
|---|---|---|
| **Router** | Button | No model. Already converges on `OnPress`. The activation Msg is *recorded at the sink*; the widget owns no state to fold. |
| **Leaf-stateful** | Checkbox, Switch, Slider, Disclosure, ScrollArea, Tooltip-host | **Keep the existing state component.** Reframe the shared consumer (`advance_toggle_on_press`, `advance_expanded_on_press`, `slider_keyboard`, the scroll/honor writers) as the **sole drain writer** of that component. The `Changed`-gated visual is the bind. *No per-entity Model trait, no per-entity mailbox* — one shared reducer keyed by Msg/role type, the shape `advance_toggle_on_press` already proves at `O(widgets)` with one system. |
| **Machine** | Menu, Dialog, Popover | **Real `Model` + reducer.** One model state holds open/active-descendant/focus-return as ONE struct; the reducer folds open/close/dismiss/keyboard/AT as Msgs. Collapses `sync_menu_open` + `sync_menu_dismissed` + `close_menu` + the focus juggling (and the Dialog open/close/escape/pending-focus quartet) into one reducer with one writer. |
| **Imperative** | composites (`meter`, `table_row`, …), raw `World` spawns | **Stay imperative.** The documented escape hatch. |

### Why tiered (rationale)

- **The leaf state is already MVU-shaped; "make it a Model" buys nothing the
  component+bind doesn't already give.** Checkbox's `A11yToggled` is already the
  single source the visual reads via `Changed` (`checkbox.rs:196`). The only defect
  is multiple *writers*; centralizing the writer is the whole fix. Wrapping each
  checkbox in a `Model` trait + `Envelope<Checkbox>` inbox adds ceremony without
  changing the data model.

- **The transport scales per-TYPE, so the cost of "every widget a Model" is not the
  inbox — it is the per-entity model conceptual tax + the record hot path.** Proto-2's
  inbox is `Messages<Envelope<M>>` keyed by `target: Entity` (`runtime.rs:48-63`), so
  10,000 checkboxes share **one** buffer + **one** drain system (`runtime.rs:300-336`).
  Transport scaling is fine either way. What "every widget a Model" actually adds is
  (a) forcing the **stateless Button** through model ceremony, and (b)
  Reflect-serializing **every** trivial toggle fold on the record tap
  (`runtime.rs:127-145`) — the §4 hot-path threat.

- **The Machines are exactly where one owning Model deletes code.** A Menu model that
  owns "open + active item + focus-return" as one value, folded by one reducer,
  removes the `A11yExpanded`↔`CssVisibility` two-writer desync **by construction** —
  `sync_menu_dismissed` ceases to exist. This is the charter's advantage #2 realized
  where it has the most leverage.

### Runner-up: "every widget is a `Model`+reducer" (the literal charter, "each widget is an actor")

Rejected, with reasons:
- **Button has no state** — a Model+reducer for it is pure ceremony.
- **Trivial-fold record cost.** Most leaf folds are a 1-line enum advance; routing
  each through a Reflect-serialized log entry is the wrong default for a slider drag
  or a 10k-row list (§4, §6 risk).
- **It conflates "recordable" with "modeled."** You can record the **activation Msg**
  (one log entry: `OnPress(lid)` / `Toggle(lid)`) without making the widget a model —
  the fold is deterministic from the component + Msg, so replay re-derives the state.
  The tiered model keeps the log complete (the charter's actual goal) without the
  per-leaf model.

The charter's framing is the right *spirit* (one Msg log, complete stream) but the
literal "every widget" is the wrong *granularity knob*. Record at the Msg sink;
model only what has a multi-field machine.

---

## 4. Performance reading (the load-bearing risk)

- **Transport: fine.** Per-type inbox + one drain → `O(widgets)` not `O(widgets²)`;
  no per-entity system. The `advance_toggle_on_press` precedent already runs one
  system over all toggles.

- **Record tap: the real threat.** `MsgLog::record` Reflect-serializes each folded
  Msg to RON on every fold (`runtime.rs:127-145`). For a **slider drag** or
  **scroll** (continuous, every-frame `A11yValue`/`ScrollOffset` mutation) or a
  **10k-leaf** screen, serialize-every-fold on the hot path is precisely what the
  perf campaign's 60 Hz hard floor forbids. Mitigation (charter already names it):
  **recording opt-out / sampling for hot paths**, and don't record derived/animation
  folds at all. The tiered model helps here too: leaf folds can be recorded as a
  compact `Toggle(lid)` Msg, not a full model snapshot.

- **`Changed`-gated binds: already cheap and already in place** (every visual is
  `Changed<State>`-filtered). No new per-frame cost; the funnel just makes the writer
  unique.

- **WASM: no new obstacle if `Reflect`-on-hot-path is opt-out.** The serialize step is
  the only wasm-relevant cost (RON/`Reflect` are already transitive deps per proto-2);
  sampling/opt-out keeps it off the hot path. Flagged for the spec, not a blocker.

---

## 5. Migration model — INCREMENTAL, phased (not a rewrite)

A big-bang rewrite is the wrong call: 11,374 LOC of widget src + 3,905 LOC of widget
tests + the gallery encode verified APG/a11y/visual behavior, and Menu alone is a
810-LOC, 6-system machine with a GPU-lane regression history. The incremental path is
available **because the state components already exist and are already
single-source-of-truth** — the migration reroutes *writers*, it does not redesign
*data*.

**Phase 0 — land the funnel in `buiy_core`, behind the existing `OnPress` sink.**
`OnPress` is already a proto-funnel: one sink, N producers (`interaction.rs:5-17`),
role-keyed consumers. Add the `Messages` inbox + single drain + record tap +
`LogicalId` (proto-2 KEEP shapes). Reframe `advance_toggle_on_press` /
`advance_expanded_on_press` as drain consumers; record `OnPress` as the Msg log.
**Zero widget API change; zero test churn.** Gate: existing widget + gallery suites
stay green.

**Phase 1 — make leaf-control state writes flow through the drain.**
Checkbox/Switch/Slider/Disclosure/ScrollArea: their components are untouched; only the
**writer** is centralized into the drain. The `Changed`-gated visuals already are
binds. This is where the per-widget "controlled vs self-updating" suppression flag is
*deleted* (the single writer makes it unnecessary). Gate: widget unit tests +
gallery interaction tests.

**Phase 2 — convert the Machines (Menu, Dialog, Popover) to single Models.** Highest
payoff (deletes `sync_menu_dismissed`, collapses the focus juggling), highest risk
(most verified behavior, GPU-lane-sensitive). Do this **last among widgets** and one
machine at a time, each behind its own gate (incl. the live-interaction test tier the
parity campaign added). Resolve the **global `FocusedEntity`/`A11yHidden` writes**
here (see open question — these are resource writes outside any model; proto-1
REDESIGN #1 already flagged `set_focus`/`set_value` writing outside the Msg path).

**Phase 3 — the `TextEditState` crux via command-sourcing.** The editor is
un-Reflectable (`state.rs:89-92`), so it cannot be a Reflect Model. Record the
**`EditCommand` stream** (already the editor's input vocabulary) as the Msg log;
replay = re-fold commands from `init`. The editor state is *derived*, never
serialized. This is the single most important migration decision and the only path
that closes the charter's advantage-#1 ("complete recordable stream / whole-UI
replay") for text.

**Phase 4 — the escape hatch, made explicit.** Composites + raw `World` spawns stay
imperative; document the boundary ("MVU-primary, raw-ECS-permitted"). The composites
module (`composites.rs`) is the ready-made exemplar — keep it imperative as proof the
paradigm is not a trap (charter ESCAPE-HATCH constraint).

---

## 6. Risks (severity · evidence · mitigation)

1. **Migration cost of a mature, verified widget set — HIGH.** Evidence: 11,374 LOC
   widget src across 14 files + 3,905 LOC widget tests + the gallery; Menu (810 LOC,
   6 interacting systems incl. the `sync_menu_dismissed` reconciliation patch) and
   Dialog (632 LOC, 5 systems) carry the bulk. Mitigation: incremental (state
   components survive; only writers reroute), `#[require]` contracts + `Changed`
   visuals untouched, Machines converted last and one at a time behind per-widget
   gates incl. the live-interaction tier.

2. **`TextEditState` is structurally unrecordable as a Reflect Model — HIGH.**
   Evidence: `crates/buiy_core/src/text/edit/state.rs:89-92` — NOT reflect-registered,
   owns `cosmic_text::Editor`; `sync_text_input_a11y` already cannot even `Changed`-gate
   it (`text_input.rs:167-171`). The "complete recordable stream" thesis breaks here
   unless the Msg stream is the `EditCommand` vocabulary. Mitigation: command-sourcing
   (record commands, replay by re-fold) — Phase 3.

3. **Hot-path Reflect-serialize on the record tap blows the 60 Hz floor — MEDIUM/HIGH.**
   Evidence: `MsgLog::record` RON-serializes every fold (`runtime.rs:127-145`);
   slider-drag / scroll / 10k-leaf screens fold continuously. Mitigation: recording
   opt-out + sampling for hot paths (charter-named); compact leaf Msgs not model
   snapshots; never record derived/animation folds; hw-independent iai-callgrind gates.

4. **Global-resource writes escape the funnel — MEDIUM.** Evidence: Menu/Dialog write
   the global `FocusedEntity` resource and spray `A11yHidden` markers
   (`menu.rs:524`, `dialog.rs:416-437`) directly; proto-1 REDESIGN #1 flagged
   `set_focus`/`set_value` writing outside the Msg path. A model that folds "open" but
   leaves focus as an out-of-band write yields non-byte-identical replay. Mitigation:
   model focus moves as `Cmd`s/Msgs (focus is part of the recorded stream), decided in
   Phase 2; or seed focus as initial conditions for replay.

5. **Per-leaf granularity over-reach (the runner-up) — MEDIUM, avoided by the
   recommendation.** Evidence: forcing stateless Button + 10k homogeneous leaves into
   per-entity models adds ceremony + record cost with no data-model gain (§3, §4).
   Mitigation: the tiered recommendation — record at the Msg sink, model only
   Machines.

6. **Reconciliation regressions during Machine conversion — MEDIUM.** Evidence: the
   widget-catalog campaign's memory note — an "anti-clobber" change once killed the
   editor Text-seed and was caught only by the GPU lavapipe lane on PR. Multi-writer
   state is already fragile; rewiring it is where a regression hides. Mitigation:
   convert one Machine per gated PR; rely on the live-interaction test tier + GPU lane
   on every Machine PR (not just at campaign end).

---

## 7. Open questions for the spec stage

- **Focus as model state or out-of-band?** Dialog/Menu write the global
  `FocusedEntity` resource directly; for byte-identical replay this must either be
  folded as a Msg/`Cmd` or seeded as an initial condition. Decide in Phase 2.
- **Record granularity for continuous streams** (slider drag, scroll, IME preedit):
  sample every Nth, coalesce, or record only terminal commits? Drives the §4 cost.
- **`LogicalId` allocation for dynamically-spawned leaves** (todo rows, list items)
  aligned to the agent-interface test-id space — who allocates, and is it stable
  across reorder/insert/delete (the keyed-reconcile-by-domain-id property)?
- **Does the leaf tier need the `Model` trait at all, or just "drain is sole writer of
  component X"?** If the latter, the runtime's `Model` trait applies only to Machines —
  a meaningful simplification of the public API surface worth settling early.
- **Tooltip/Popover/LightDismiss are cross-cutting overlay logic, not per-widget.**
  Do they fold into the owning Machine's model (Menu-owns-its-popover) or stay a
  shared overlay subsystem that the funnel drives? Affects how `CssVisibility` writes
  are unified.
