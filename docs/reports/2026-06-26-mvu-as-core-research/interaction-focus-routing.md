# MVU-as-core research — interaction / press / focus routing

> Research stage of `/staged-development` for prototype-3 (MVU as the CORE/primary
> interface). This file maps how Buiy handles **interaction, press, and focus today**
> (current `origin/main` in the `mvu-core` worktree) and exactly how a core-level Msg
> funnel would intercept/replace it. Read-only analysis; no production code changed.
>
> Charter rule honored: re-decide every choice, inherit nothing blindly. Where the
> evidence cuts against the charter, it says so (see Risk 1, Risk 4, and the "escape
> hatch" finding).

All `file:line` citations are to the current-main tree under
`/mnt/storage/projects/buiy/.claude/worktrees/mvu-core` unless prefixed `proto-2:`
(the `state-mgmt-elm-prototype` worktree).

---

## 0. TL;DR for the spec author

1. **Buiy already has a one-sink activation funnel** (`OnPress`, a buffered
   `Message`) that pointer + keyboard + AT all converge on
   (`interaction.rs:29`, three producers, several consumers). It is a proto-MVU bus
   missing only (a) the ordered drain + record tap and (b) coverage of the
   *non-activation* mutation paths.
2. **The editor already has a command vocabulary** — `EditCommand`
   (`text/edit/command.rs:21`) — lowered to cosmic actions at exactly one site
   (`apply_tracked`, `input.rs:97`). This is the escape from the named
   `TextEditState`-is-not-`Reflect` crux: **log/replay the command stream, not the
   state.** This is the single most important interaction finding.
3. **`OnPress` covers activation only.** Focus, hover, drag-select, scroll, slider
   value, text editing, IME, menu nav, dialog lifecycle, and light-dismiss each have
   their **own** direct-mutation path. A *complete* core funnel must absorb all of
   them — that is the real migration surface, not just `OnPress`.
4. **Recommended routing:** keep proto-2's propagating `EntityEvent` + one global
   observer per model (nearest-ancestor enqueue), but **collapse the `OnPress→Routed`
   two-hop** the prototype used into a single producer-triggered routed event to kill
   the 1–2-frame latency (proto-2 REFINE #2). The **drain stays a system** in a
   pinned set; never an observer.
5. **Funnel lives in `buiy_core`** (new `buiy_core::mvu` module). An opt-in crate
   cannot make core's *own* consumers (`advance_toggle_on_press`, the editor apply,
   the focus writers) enqueue instead of mutate — the dependency points the wrong way
   (charter signal #2). That is the load-bearing reason this must be core.

---

## 1. The current activation architecture (the "grounding loop")

### 1.1 One sink: `OnPress`

`OnPress(Entity)` is a **buffered `Message`** (not an observer `Event`), defined and
registered in core:

- `crates/buiy_core/src/interaction.rs:29-30` — `#[derive(Message, Debug, Clone, Copy)] pub struct OnPress(pub Entity);`
- `interaction.rs:37-43` — `InteractionPlugin` does `app.add_message::<OnPress>()`.
- `lib.rs:100` — `CorePlugin` adds `InteractionPlugin`, so `Messages<OnPress>` exists
  for in-core producers **regardless of whether `buiy_widgets` is present**
  (`interaction.rs:10-18` documents exactly this: the P1c router lives in core and
  cannot depend on `buiy_widgets`, so the sink must be in core).

The module doc is explicit that this is a deliberately single sink: "There is
deliberately **no** competing `Activate` event — a second sink would fork the
activation grounding loop" (`interaction.rs:16-17`). This is the co-drive SC-1
contract.

### 1.2 Three producers converge on `OnPress`

| Modality | Mechanism | Site | What it does |
|---|---|---|---|
| Pointer | **observer** on `Pointer<Click>` | `picking/activation.rs:54` (`pointer_click_emits_on_press`), registered `picking/mod.rs:73` | `if is_activatable_role(role) { writer.write(OnPress(target)) }` |
| Keyboard | **system** in `BuiySet::Input` | `a11y/action.rs:398` (`keyboard_activation`), registered `a11y/mod.rs:324` | per-role APG keymap (`activation_keys`, `action.rs:366`): Button=Enter/Space, Checkbox=Space, Switch=Space/Enter → `writer.write(OnPress(focused))` |
| AT / agent | role contract `honor(Click)` via the inbound router | `a11y/action.rs:151` (`dispatch_action_request`) → contract writes `OnPress`; driven by `route_action_requests` (`action.rs:323`) or the in-process driver directly | `Action::Click` → `OnPress` |

The pointer producer is an **observer**; the keyboard/AT producers are **systems**
(they read buffered `KeyboardInput` / drain the `ActionRequestWrapper` channel). All
three write the *same* `Messages<OnPress>` buffer.

`is_activatable_role` (`activation.rs:40-45`) gates pointer activation to
`Button | Checkbox | Switch` — so a click on a plain node or text input does **not**
spuriously activate. This is the boundary that keeps pure hover/move *off* the
activation path (load-bearing for Risk 1).

### 1.3 Consumers read `OnPress` and **mutate directly**

This is the funnel-interception point. The mutation happens in the consumer:

- `advance_toggle_on_press` (`buiy_widgets/src/lib.rs:78-96`) — `Query<(&A11yRole, &mut A11yToggled)>`; on each `OnPress(e)` advances `A11yToggled` **in place** (`toggled.advance_checkbox()` / `toggled.toggle_switch()`).
- `advance_expanded_on_press` (`buiy_widgets/src/lib.rs:118-127`) — `Query<&mut A11yExpanded>`; flips `expanded.0 = !expanded.0`.
- `open_dialog_on_invoker_press` (`dialog.rs:364`), `close_dialog_on_button` (`dialog.rs:546`) — read `OnPress`, mutate dialog visibility.
- **App-level handlers** (the canonical end-user pattern): `route_nav_press` (`examples/buiy_gallery/src/shell.rs:1480`), and `for OnPress(e) in reader.read()` loops at `examples/buiy_gallery/src/lib.rs:1158, 3067, 3085, 4139, 5515` and `inspector.rs:992`. Each matches `e` against known button entities and mutates app state / writes an app-domain message.

**There is no user-facing Button callback in current main.** `button.rs:132-145`
states the Button "needs no activation system of its own"; it only emits `OnPress`.
The "fires its own callback elsewhere" phrasing in `lib.rs:73` is aspirational — the
*actual* end-user contract is "register a `MessageReader<OnPress>` system and match by
entity" (`examples/hello_button/src/main.rs:26-30`).

> **Implication for MVU-core.** The existing `OnPress` bus is essentially a primitive
> message log with N independent readers, each owning a cursor and reaching directly
> into ECS to mutate. MVU-core's job is to insert a **single ordered drain + record
> tap** between the `OnPress` read and the mutation, and to make the consumers
> *enqueue* a `Msg` rather than mutate. The shape is already 80% there for activation.

### 1.4 End-to-end path for a pointer button press (the (a) answer)

1. winit cursor → `PointerInput` (bevy_picking `PointerInputPlugin`, added by the
   windowed `BuiyPlugin`, not `PickingPlugin` — `picking/mod.rs:46-50`).
2. `emit_picks` (`picking/backend.rs:53`) runs in `PreUpdate` /
   `PickingSystems::Backend` (`backend.rs:48`) and writes `PointerHits` ranked by
   `global_paint_order` (`picking/depth.rs:43`), honoring `ComputedPaintSkip`
   (pick-set == paint-set, `backend.rs:112`) and the `Pickable` occlusion rule
   (`depth.rs:116` `resolve_picks`).
3. bevy_picking `InteractionPlugin` (`picking/mod.rs:64`) diffs the hovermap and
   fires the `Pointer<Press/Release/Click/...>` taxonomy with capture→target→bubble.
4. `pointer_click_emits_on_press` observer (`activation.rs:54`) fires on
   `Pointer<Click>` (which only fires when press+release share a target — the
   drag-cancel semantics fall out for free, `activation.rs:8-17`), checks role, writes
   `OnPress(target)`.
5. Same frame: `focus_on_click` observer (`focus.rs:274`) fires on `Pointer<Press>`,
   walks `ChildOf` to the nearest `Focusable` (`nearest_focusable`, `focus.rs:294`),
   sets `FocusedEntity = Some(target)` + `FocusVisible(false)` (`focus.rs:287-289`).
6. Same frame, `BuiySet::Input`: consumers read `OnPress`
   (`advance_toggle_on_press` mutates `A11yToggled`, or the app handler mutates app
   state).
7. `Changed<A11yToggled>` → C4 visual systems repaint (`buiy_widgets/src/lib.rs:196-203`),
   then `BuiySet::A11yUpdate` `build_tree` re-publishes the semantic tree.

The **mutation site is step 6 (the consumer)**. The funnel must sit between the
`OnPress` read and that mutation.

---

## 2. Where interaction / focus / press state lives (the (b) answer)

| State | Kind | Site | `Reflect`? |
|---|---|---|---|
| `OnPress` activation sink | `Message` buffer | `interaction.rs:29` | n/a (transient) |
| Hover / press | **none of Buiy's** — C3c retired the `Hovered` resource | `picking/mod.rs:36-45` | lives in bevy_picking `Pointer<E>` + `DirectlyHovered`/`Hovered` |
| Focus target | `Resource FocusedEntity(Option<Entity>)` | `focus.rs:109-111` | yes (`#[reflect(Resource)]`) |
| Focus-visible heuristic | `Resource FocusVisible(bool)` | `focus.rs:116-118` | yes |
| Toggle state | `Component A11yToggled` | `a11y/states.rs` | yes |
| Expanded state | `Component A11yExpanded` | `a11y/states.rs` | yes |
| Slider value | `Component A11yValue` | `a11y/states.rs` | yes |
| **Editor state** | `Component TextEditState` | `text/edit/state.rs:92` | **NO** (`#[derive(Component)]` only; wraps `cosmic_text::Editor`, `state.rs:88-94`) |

**How routing already reaches into core (charter signal #1).** Proto-2's routing
layer had to import the core sink: `proto-2: examples/mvu_native/src/routing.rs:13`
`use buiy_core::interaction::OnPress;`. Its `bridge_press` system
(`proto-2:routing.rs:32-45`) reads `Messages<OnPress>` and re-triggers a bubbling
`Routed<M>` EntityEvent; the `route_observer` (`proto-2:routing.rs:51-62`) enqueues
at the nearest model owner. An **opt-in** crate *can* read the sink — but the
*producers* (`pointer_click_emits_on_press`, `keyboard_activation`, the AT router) and
the *existing consumers* (`advance_toggle_on_press`, the editor apply, the focus
writers) all live in `buiy_core`/`buiy_widgets` and mutate **outside** any opt-in
funnel. So an opt-in log is structurally incomplete: it sees the app-domain `Msg` a
user maps `OnPress` to, but never the widget-internal toggle/expand/edit/focus
mutations. That is exactly the charter's case for making the substrate core.

---

## 3. Observers / `EntityEvent` usage today (the (c) answer)

Observers (`On<...>`) are pervasive and are the idiomatic pointer-handling surface:

- `pointer_click_emits_on_press` (`Pointer<Click>`), `derive_multi_click`
  (`Pointer<Click>`) — `picking/mod.rs:73-74`.
- `focus_on_click` (`Pointer<Press>`) — `focus.rs:241`.
- `editor_pointer_press` (`Pointer<Press>`, `pointer.rs:146`), `editor_pointer_drag`
  (`Pointer<Drag>`, `pointer.rs:182`) — **mutate `TextEditState` directly inside the
  observer** (`pointer.rs:170` `state.apply_pointer_gesture(...)`).
- `light_dismiss_on_press` (`Pointer<Press>`) — `buiy_widgets/src/dismiss.rs:99`.
- `menu_item_click_emits_on_press` (`Pointer<Click>`) — `buiy_widgets/src/menu.rs:442`;
  plus inline menu observers `menu.rs:407-412`.

Custom `EntityEvent` types:

- `MultiClick` (`picking/gesture.rs:37-47`) — a propagating
  (`#[entity_event(propagate, auto_propagate)]`) double/triple-click signal, derived
  from the editor's `ClickTracker`. The one bespoke EntityEvent in core.
- The whole `Pointer<E>` taxonomy is bevy `EntityEvent` (bubbles capture→target→bubble).
- Proto-2 added `Routed<M>` (`proto-2:routing.rs:23-28`) as a propagating EntityEvent.

**Critical nuance — observers fire synchronously, `OnPress` is buffered.** Today
there is a *mix*: pointer producers are observers that write a buffered `Message`;
keyboard/AT producers are systems; consumers are systems in `BuiySet::Input`. The
**editor bypasses `OnPress` entirely** and mutates `TextEditState` directly in
observers (`editor_pointer_press`) and a system (`apply_keyboard_edits`,
`input.rs:562`, in `BuiySet::Input`). So "all interaction" is not one path today — it
is `OnPress` (activation) **plus** a parallel editor command path **plus** direct
focus/value/dialog mutations. A core funnel must unify these.

---

## 4. Routing ALL interaction through a Msg funnel at the core level (the (d) answer)

The hard rule is "handlers ENQUEUE, never mutate." Concretely, every direct-mutation
site below must become an enqueue into the funnel; a single ordered drain (one per
model type, or one type-erased drain) folds them:

### 4.1 Inventory of mutation sites that must convert to enqueue

| Site | File:line | Mutates today | Funnel form |
|---|---|---|---|
| `advance_toggle_on_press` | `widgets/lib.rs:78` | `&mut A11yToggled` | enqueue `Toggle` Msg to the widget model |
| `advance_expanded_on_press` | `widgets/lib.rs:118` | `&mut A11yExpanded` | enqueue `ToggleExpanded` |
| Dialog open/close | `dialog.rs:364, 546` | dialog `CssVisibility` | enqueue dialog-lifecycle Msg |
| `editor_pointer_press` | `text/edit/pointer.rs:170` | `TextEditState` (caret place) | enqueue an `EditCommand`/pointer-gesture Msg |
| `editor_pointer_drag` | `text/edit/pointer.rs:182` | `TextEditState` (selection extend) | enqueue drag-extend Msg |
| `apply_keyboard_edits` | `text/edit/input.rs:562` | `TextEditState` via `EditCommand` | enqueue `EditCommand`; drain applies via `apply_tracked` |
| IME splice/commit | `text/edit/ime.rs` | `TextEditState.preedit` | enqueue IME Msgs |
| `slider_keyboard` → `honor` | `a11y/action.rs:537` | `A11yValue` | enqueue value Msg |
| `focus_on_click` | `focus.rs:287` | `FocusedEntity`/`FocusVisible` | enqueue focus Msg |
| `handle_tab` | `focus.rs:370` | `FocusedEntity`/`FocusVisible` | enqueue focus Msg |
| AT `Focus/Blur/Expand/Collapse/ShowTooltip` | `a11y/action.rs:231-294` | resources/components | enqueue the matching Msg |
| `menu_keyboard_nav`, `sync_menu_open` | `menu.rs` | menu state | enqueue menu Msgs |
| App `OnPress` handlers | gallery `lib.rs`/`shell.rs`/`inspector.rs` | app state | enqueue app `Msg` |

### 4.2 The two structural facts that make this tractable

1. **Activation is already one sink.** Pointer/keyboard/AT already converge on
   `OnPress` (§1.2). The funnel re-targets the *consumers* from "mutate" to "enqueue";
   the producers barely change (write a routed activation event instead of, or in
   addition to, `OnPress`).
2. **The editor is already command-shaped.** `EditCommand` (`command.rs:21-51`) is a
   Buiy-owned verb enum (`Insert`, `Backspace`, `Motion`, `Cut/Copy/Paste`,
   `Undo/Redo`, `SelectAll`, `Submit`, ...). `apply_keyboard_edits` (`input.rs:562`)
   already *produces* an `EditCommand` from the keymap and *applies* it inline via
   `apply_tracked` (`input.rs:97`, "the one place edits are recorded as undo units").
   Splitting produce/apply into **enqueue (`EditCommand`) + drain (`apply_tracked`)**
   is the MVU shape and — crucially — makes the edit history loggable as a **command
   stream**, sidestepping the `TextEditState`-is-not-`Reflect` crux entirely (Risk 2).

### 4.3 What `OnPress` does NOT cover (the real scope)

`OnPress` is activation-only. The funnel must also absorb: focus changes
(`FocusedEntity`/`FocusVisible`, ≥4 writers), text editing + IME (the `EditCommand`
path), slider/value changes (the a11y-router `honor` path, *not* `OnPress` —
`widgets/lib.rs:280` notes the slider value is changed "NOT through the `OnPress`
toggle sink"), menu roving nav, dialog lifecycle, scroll (`scroll.rs:218`
`keyboard_scroll`), and light-dismiss. Each is its own path today. **This breadth —
not the `OnPress` re-point — is the migration cost.**

---

## 5. The escape hatch: can raw-ECS interaction coexist? (the (e) answer)

**Yes for reads and for app-owned state; with a caveat for funneled state.**

- Users can still `add_observer` / `add_systems` freely; nothing structurally blocks
  a raw-ECS interaction system.
- **Caveat (a real limit):** if a raw user system mutates *funneled* state directly
  (e.g. `Query<&mut A11yToggled>` or writing `FocusedEntity`), that mutation is
  invisible to the record tap → **breaks the complete-log / replay guarantee
  silently.** The `PureEnv` allowlist (`proto-2:runtime.rs:201`) constrains
  *reducers* (it refuses `Commands`, `ResMut`, `Query<&mut>`), but it does **not**
  and cannot constrain arbitrary user systems the app registers.
- Therefore the honest contract is: *raw ECS is unrestricted for state the funnel does
  not own; for funneled state you must enqueue.* This cannot be fully enforced by the
  type system at the app-system boundary. Options:
  - **(a) Convention + docs** — list the funneled-state component/resource set; accept
    that completeness holds "iff you don't raw-mutate funneled state." Cheap; leaks.
  - **(b) Encapsulation** — make funneled state's fields `pub(crate)` and mutable only
    through the drain (the `TextEditState` precedent — its `editor` is already
    `pub(crate)`, `state.rs:96`). Closes the leak for the framework's own types but is
    a large migration and does nothing for *user* model state.
  - Recommend **(b) for the framework's funneled components** (toggle/expanded/value/
    focus) + **(a) documented contract for user state**. This makes the framework's
    own completeness guarantee robust while keeping the escape hatch genuinely open.

---

## 6. Recommendations

### 6.1 Routing mechanism

**Recommend:** keep proto-2's propagating `EntityEvent` + one global observer per
model (nearest-ancestor enqueue, `proto-2:routing.rs`), because it compiled cleanly,
matches the existing `Pointer<E>` / `MultiClick` bubbling idiom (`gesture.rs:37`), and
uses the engine's `set_event_target` rewrite for nearest-ancestor semantics for free.
**But collapse the `OnPress → Routed<M>` two-hop**: have the activation *producer*
trigger the routed event directly (or make activation itself the propagating
`EntityEvent`), so there is no buffered-`Message` middle hop adding a frame.

**Runner-up (rejected):** keep `OnPress` as a buffered `Message` and bridge it into
`Routed<M>` exactly as proto-2 did (`bridge_press`). Rejected because it keeps two
parallel mechanisms and reintroduces the proto-2 REFINE #2 latency (the
bridge→trigger→observer→drain chain spanned 1–2 frames,
`proto-2:RETROSPECTIVE:87-92`).

**Tension to preserve:** `OnPress`-as-`Message` is what lets three producers converge
on one consumer without ordering them. If activation becomes a propagating
`EntityEvent`, the keyboard/AT producers (which are `&mut World` systems) must
`commands.trigger` instead of `writer.write` — workable, and the bubbling *helps*
routing (nearest activatable ancestor). Net: the EntityEvent path is better for
*routing*, but the spec must consciously re-home the convergence guarantee.

### 6.2 Where the funnel lives

**Recommend:** a new `buiy_core::mvu` module (Model trait, `Envelope`/inbox, the
single drain, `MsgLog` record tap, `MvuSet`, `enqueue`, sealed `PureEnv`). **Rationale:**
the producers (pointer observer, keyboard system, AT router) already live in core, and
the *consumers that mutate* (`advance_toggle_on_press` in widgets, the editor apply in
core, the focus writers in core) also live in core/widgets. Only a **core-resident**
funnel can make those *own* consumers enqueue instead of mutate; an opt-in crate cannot
(core can't depend on it — charter signal #2). The AccessKit read-tree is already core
(`a11y/`); the Msg write-log is its dual and wants to be co-located.

**Runner-up (rejected):** keep the runtime in an opt-in `buiy_mvu` and only funnel the
app boundary (the proto-1/2 locked decision). Rejected for this bet precisely because
it leaves widget-internal toggle/expand/edit/focus mutations out of the log (§2, §4.3)
— the incompleteness the charter exists to fix.

### 6.3 Observer vs system

- **Producers / routing (enqueue):** observers for pointer-derived events (they
  already ride the picking bubble and carry the picked target — `activation.rs:54`,
  `focus.rs:274`). Systems for keyboard / AT (they read buffered input / drain the
  `ActionRequestWrapper` channel — `action.rs:398, 323`).
- **Drain (fold): MUST be a system** in a pinned set (`MvuSet::Drain`), never an
  observer. Observers run at unpredictable command-flush points and re-entrantly; a
  single ordered drain system is what gives deterministic fold order + the record tap.
  Proto-2 already splits it this way (routing observers in `MvuSet::Enqueue`, drain
  system in `MvuSet::Drain`, `proto-2:runtime.rs:300-334`). Keep that split.
- **Pin the sets against `BuiySet`:** `MvuSet::Enqueue ⊆ BuiySet::Input` (after the
  producers), `MvuSet::Drain` after `Input` and before `Animate`/`Picking`/
  `A11yUpdate` (so a folded toggle settles before the C4 visuals and `build_tree`),
  `MvuSet::Bind` after `Drain` and before `Render`. With explicit command-flush points
  so latency is **one designed frame**, not the emergent 1–2 (proto-2 REFINE #2).
  `BuiySet` order today is Layout → Style → Input → Animate → Picking → A11yUpdate →
  Render (`lib.rs:79-87, 104-116`).

---

## 7. Risks (interaction / focus specific)

### Risk 1 — Per-frame pointer volume vs the 60 Hz floor (HIGH)
**Evidence:** `emit_picks` writes `PointerHits` **every frame** a pointer targets a
Buiy window, even with an empty pick list (`backend.rs:140`, "ALWAYS emit"). The
`Pointer<Over/Out/Move/Drag/Scroll>` taxonomy fires continuously. If *every* pointer
event funnels (enqueue + drain), the drain runs hot on hover alone.
**This cuts against a naive "route ALL interaction through the funnel" reading.**
**Mitigation:** funnel only **state-changing** events (Click/activate, `EditCommand`,
value-change, focus-change, dialog/menu lifecycle); leave pure hover/move/drag-track
as raw ECS — they do not mutate funneled state. The existing `OnPress` boundary already
does exactly this (only `Pointer<Click>` lowers to `OnPress`, not `Move` —
`activation.rs:8-17`). Make that the explicit funnel boundary. Drag-*select* is the
edge case (it mutates `TextEditState` per move, `pointer.rs:182`): batch it as one
"drag session" command or sample it.

### Risk 2 — `TextEditState` is not `Reflect` and wraps `cosmic_text::Editor` (HIGH — the named crux)
**Evidence:** `state.rs:88-94` — `#[derive(Component)]` only; doc: "NOT
reflect-registered (it carries a `cosmic_text::Editor`...)". It holds `Editor<'static>`,
`UndoStack`, `Option<PreeditSpan>`, etc. A `Reflect`-snapshot funnel cannot serialize
it. **Mitigation:** route editing through the existing `EditCommand` vocabulary
(`command.rs:21`) and **log/replay the command stream, not the state.** The editor
already has the single apply site (`apply_tracked`, `input.rs:97`); make the drain call
it. Replay = re-fold `EditCommand`s from a known seed buffer. This sidesteps `Reflect`
for the editor entirely and is the cleanest resolution of the crux the charter (signal
#3) and the hot-reload research both flagged. (Caret-blink phase and the projected
`TextSelection`/`intrinsics` are derived state recomputed each frame —
`state.rs:107-121` — so they need not be logged.)

### Risk 3 — Enqueue→drain flush latency across the observer/system edge (MEDIUM)
**Evidence:** proto-2 RETROSPECTIVE:87-92 — bridge→`commands.trigger` flush→observer
enqueue flush→`Drain` spanned a couple frames depending on where Bevy inserts
`apply_deferred`; the GUI tolerated ≤2 frames but it was emergent.
**Mitigation:** pin `MvuSet::{Enqueue,Drain,Bind}` against `BuiySet` with explicit
sync points (§6.3); collapse the `OnPress→Routed` two-hop (§6.1). Target one designed
frame.

### Risk 4 — Escape-hatch leak: raw `Query<&mut>` on funneled state silently breaks the log (MEDIUM)
**Evidence:** nothing prevents a user system from `Query<&mut A11yToggled>` or writing
`FocusedEntity`; `PureEnv` (`proto-2:runtime.rs:201`) only constrains *reducers*, not
app systems. **This is a genuine boundary of the "complete log" claim — it holds by
contract, not by construction, at the user-system edge.**
**Mitigation:** encapsulate the framework's funneled components (make fields
`pub(crate)`, mutate only via the drain — the `TextEditState.editor` precedent,
`state.rs:96`) + document the funneled-state contract for user model state (§5).

### Risk 5 — Migration breadth: the whole interaction surface must convert (HIGH)
**Evidence:** the §4.1 inventory — at least 13 framework mutation sites
(`advance_toggle_on_press`, `advance_expanded_on_press`, dialog open/close,
`editor_pointer_press`, `editor_pointer_drag`, `apply_keyboard_edits`, IME splice/
commit, `slider_keyboard` honor, `focus_on_click`, `handle_tab`, the AT router's
Focus/Blur/Expand/Collapse/Tooltip arms, `menu_keyboard_nav`/`sync_menu_open`,
`light_dismiss`) plus every app `OnPress` handler — each verified, all mature and
test-covered. **Mitigation:** stage it. (1) Activation first (`OnPress` consumers →
enqueue) since it is already one sink. (2) Editor next (the `EditCommand` path already
exists). (3) Focus as a singleton actor (Risk 6). (4) Leave hover/move raw. **No
big-bang** — the charter itself warns a big-bang rewrite is likely wrong.

### Risk 6 — Focus is global-resource state, not per-entity, so it resists "every widget is an actor" (MEDIUM)
**Evidence:** `FocusedEntity`/`FocusVisible` are **resources** (`focus.rs:111, 118`)
mutated from ≥4 sites (`focus_on_click` `focus.rs:287`, `handle_tab` `focus.rs:370`,
AT `Focus/Blur` `action.rs:231-248`, plus dialog focus-restore via `FocusReturn`
`focus.rs:104`). A per-widget model does not fit. **Mitigation:** model focus as **one
root/app-level focus actor** (a singleton `Model`); all focus changes enqueue focus
`Msg`s it folds. This is clean *and* puts focus changes in the log — closing the focus
half of the replay crux. **Runner-up (rejected):** keep focus as a raw resource with a
side-channel record tap — rejected because it forks the log (focus events recorded by
a different mechanism than widget Msgs) and re-opens the incompleteness.

---

## 8. Answers to the charter's explicit sub-questions

- **(a) press path:** §1.4 — pointer → backend `emit_picks` → bevy_picking
  `Pointer<Click>` → `pointer_click_emits_on_press` observer writes `OnPress` →
  `BuiySet::Input` consumer (`advance_toggle_on_press` / app handler) **mutates
  directly**. Mutation site = the consumer.
- **(b) where state lives / how routing reaches core:** §2 — `OnPress` sink + focus
  resources in core; toggle/expanded/value as `A11y*` components; editor in the
  non-`Reflect` `TextEditState`. Proto-2 routing imported `buiy_core::interaction::OnPress`
  (signal #1); an opt-in funnel can read the sink but not re-home the in-core
  consumers' mutations.
- **(c) observers/EntityEvent:** §3 — observers are the pointer idiom
  (`activation.rs`, `focus.rs`, `pointer.rs`, `dismiss.rs`, `menu.rs`); the one bespoke
  EntityEvent is `MultiClick` (`gesture.rs:37`); `Pointer<E>` and proto-2 `Routed<M>`
  are propagating EntityEvents. Observers fire synchronously; `OnPress` is buffered —
  and the editor bypasses `OnPress` entirely.
- **(d) full-funnel cost:** §4 — convert the §4.1 inventory of direct-mutation sites
  to enqueue; the two enablers are the existing one-sink `OnPress` and the existing
  `EditCommand` vocabulary; the real scope is the *non-activation* paths `OnPress`
  doesn't cover (§4.3).
- **(e) escape hatch:** §5 — coexists for reads and app-owned state; raw mutation of
  *funneled* state is the leak; close it with `pub(crate)` encapsulation for framework
  types + a documented contract for user state.

---

## 9. Cross-references for the spec / plan stages

- Activation sink + producers: `interaction.rs`, `picking/activation.rs`,
  `a11y/action.rs` (`keyboard_activation`, `dispatch_action_request`).
- Funnel placement argument (must-be-core): §2, §6.2 + charter signal #2.
- Editor command stream (the `Reflect` escape): `text/edit/command.rs`,
  `text/edit/input.rs:97` (`apply_tracked`), `text/edit/state.rs:92`.
- Routing mechanism precedent to port: `proto-2:examples/mvu_native/src/routing.rs`;
  drain/set discipline: `proto-2:examples/mvu_native/src/runtime.rs:300-418`.
- Focus-as-actor: `focus.rs` (`FocusedEntity`, `handle_tab`, `focus_on_click`).
- Scheduling anchor: `BuiySet` chain `lib.rs:79-116`; a11y router ordering
  `a11y/mod.rs:315-333`.
