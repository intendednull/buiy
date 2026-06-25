# Widget visual + picking layer over the agent-interface a11y substrate — child C4 of the widget-catalog campaign

`2026-06-22` · `[draft]` · Wave 3 · realizes foundation `media-and-widgets.md §3.10` (the visible widget surface), `visuals.md §3.3` (per-state look) · **consumes** the agent-interface campaign's a11y substrate (states/roles/router/bundles) · depends on the agent-interface **P1a** (decomposed `A11yStates`) + **P1d** (APG widget bundles), C3 (`Pointer<E>` + focus-on-click signal), C1/C3 (pick-through), C6 (focus-ring paint), C7 (verification-geometry tier)

> Reads the umbrella [`README.md`](README.md). Adheres to the cross-cutting arbitrations §6 and the **coordinate-don't-cede** decision §2.7 — references shared contracts by number rather than redefining them. **Scope boundary (revised under umbrella §2.7):** this child owns the **visible-rendering + picking-integration layer** of the catalog widgets — the visible label (content-sized child `Text` scene-fn), the focus-ring hookup (with C6), the pick-through (`Pickable::IGNORE` on internal nodes, with C3), and the per-state **visual** (fill/strike/accent/hover/active) driven by reading the agent-interface a11y-state components via change-detection. It does **not** own the a11y-state component model, the `A11yRole` enum, the a11y lowering, the action router, the APG keyboard contracts, or the canonical widget bundles — **all of those are the agent-interface campaign's (P1a/P1c/P1d)** and this child consumes them.

---

## Coordination with the agent-interface campaign

**This child CONSUMES, does not redefine, the agent-interface a11y substrate.** Per umbrella §2.7 (coordinate, don't cede) and the agent-interface staged design (`semantic-tree.md`, `widget-contracts.md`, `phasing.md` P1a/P1d, landed `a11y/mod.rs`), the following are **owned by the agent-interface campaign** and this child only reads/extends them:

**Consumes (never redefines):**
- The **decomposed a11y-state components** (`a11y/states.rs`, agent-interface P1a): `A11yToggled(Toggled)` — tri-state incl. `Mixed`, role-disambiguated; `A11ySelected(bool)`; `A11yExpanded(bool)`; the `A11yDisabled` marker; `A11yValue`/`A11yTextValue`/`A11yPlaceholder` for the valued/text widgets. C4 reads these (change-detection) to drive the **visual** only. There is **no** competing `ToggleState`/`Checked`/`Pressed`/`Selected`/`InteractionDisabled` type in `buiy_widgets` or `buiy_core` from this campaign.
- The **`A11yRole` enum** (landed P0; expanded *there*): `Checkbox`/`Switch`/`Slider`/`TextInput`/`MultilineTextInput`/`Region`/`Group`. C4's visuals key off these by the **landed names** (e.g. `TextInput`, not `TextField`). C4 does **not** add roles, touch `role_to_accesskit`, or touch the verify-side `role_to_str`/`KNOWN_ROLES` — those are agent-interface + C7.
- The **one-way lowering** (`build_tree` → `A11yNodeView` derive fold → `translate.rs`): the agent-interface campaign owns the widened `A11yNodeView`, the `build_tree` query, and the `to_accesskit_node` setter fold. C4 emits **no** a11y data and edits **none** of these files.
- The **inbound action router** + the **`A11yContract`** + the **APG keyboard contracts** (agent-interface P1c/P1d): activation flows through the existing **`OnPress`** message and the router's `Action::Click`→`OnPress` path. There is **no `Activate` event** and **no `Action::Click`→`Activate` bridge** in this campaign (the umbrella §2.7 explicitly removes the competing `Activate`).
- The **canonical APG widget bundles** in `buiy_widgets` (agent-interface P1d): Checkbox/Switch/Slider/Disclosure/Dialog/Tooltip/TextInput — each is the bundle that assembles the decomposed components and `impl A11yContract`. C4 does **not** construct these bundles; it **extends** them.

**Owns (the visual + picking layer these bundles need but the agent-interface campaign does not build):**
- The visible **label** rendering — a content-sized, centered child `Text` entity carrying the foreground token, authored via a mergeable scene-fn (§2.3). (The agent-interface bundle carries the *accessible* name via `A11yLabel`; C4 supplies the *pixels*.)
- The **focus-ring hookup** (with C6) — opting the widget into the `:focus-visible` signal so C6's ring paint fires.
- The **pick-through** (with C1/C3) — `Pickable::IGNORE` on the widget's internal/label nodes so a hit resolves to the widget root the router addresses (delivers the stacking-aware `hit_test` the agent-interface campaign deferred — its follow-up #3 — via C1+C3).
- The **per-state visual** — reading `A11yToggled` (tri-state: `Off`/`On`/`Mixed`) / `A11ySelected` / `A11yExpanded` / `A11yDisabled` via `Changed<…>` change-detection to drive fill/checkmark/strike/accent, plus pointer hover/active visual states (§2.2).

**Depends on (hard, must be live before C4 code lands):** agent-interface **P1a** (the state components exist + register) and **P1d** (the widget bundles exist to extend), plus C3 (pointer + focus-on-click signal), C1/C3 (pick-through), C6 (ring paint). **Meeting point is per-widget:** agent-interface P1d builds the bundle + `A11yContract` + APG keyboard; C4 adds rendering + picking on top. Coordinate per-widget so neither campaign rebuilds the other's layer.

---

## 1. Problem & current state (file:line evidence)

The agent-interface campaign realizes the **a11y substrate**: the decomposed state components (`a11y/states.rs`), the `A11yRole` taxonomy (already landed on `main`: `Checkbox`/`Switch`/`Slider`/`TextInput`/`MultilineTextInput`/`Region`/`Group` at the landed `a11y/mod.rs:41-47`), the derive-fold lowering, the action router, and the canonical APG widget bundles. **What that substrate does not produce is a *visible, pickable* widget** — and that gap is C4's.

**The catalog widgets render nothing themselves.** The agent-interface bundle carries `A11yRole` + the state components + `A11yLabel` (the AT name) + the `A11yContract`, but no on-screen pixels: no visible label, no checkmark/fill, no hover/active feedback, no focus ring. A sighted user sees an unstyled box; a screen-reader user gets the full semantic. C4 supplies the visual.

**The visible label is not decoupled from the accessible name.** The agent-interface bundle carries `A11yLabel(String)` (the AT name; ACCNAME 1.2 computes the effective name in `accname.rs`). There is no child `Text` rendering those pixels, and no contract for how a click on the label-text reaches the widget root the router addresses.

**Picking does not yet route through internal nodes.** A widget built as `Widget [ Text(label) ]` (the bevy_ui_widgets `children![]` shape) needs the child `Text` to be pick-transparent so a click resolves to the widget root (the entity the router's `NodeId → Entity` lookup targets). That pick-through is C1's coordinate-space fix + C3's `Pickable` propagation; C4 declares the dependency and authors the internal nodes `Pickable::IGNORE`.

**Visual state is not yet wired to the a11y state.** Nothing reads `A11yToggled`/`A11ySelected`/`A11yExpanded`/`A11yDisabled` to repaint. The visual must follow the substrate's state via change-detection so that a toggle (whether from a pointer click, a keyboard Space, or an inbound AT `Action::Click`, all converging in the agent-interface router) repaints the checkmark/fill exactly once.

**Focus is structurally invisible (WCAG 2.4.7).** The umbrella's audit confirms keyboard focus has no visible ring. C6 owns the ring **paint**; C4 owns each widget **opting in** to the `:focus-visible` signal so the ring fires on the right entity.

What the prototype's anti-patterns teach (audit W2, the umbrella's §7 retains the lesson): the `A11yToggled(bool)` conflation, the duplicate-component insert-after-spawn panic, the co-located-`Text` label, and mouse-down activation are all **solved in the agent-interface substrate** (tri-state `A11yToggled`, bsn!-clean bundles, the router's release-timed `OnPress`). C4 inherits those solutions; it does not re-litigate them. C4's only carry-forward is the **visual** discipline (the `Changed`-gated repaint) and the **child-`Text` label decoupling** (now the rendering half of the agent-interface bundle's `A11yLabel`).

---

## 2. Target design

### 2.1 What C4 builds: the visual + picking layer, per widget

For each agent-interface P1d widget bundle (Checkbox/Switch/Slider/Disclosure/Dialog/Tooltip/TextInput) C4 adds, in `buiy_widgets`, a **rendering + picking extension** that:

1. spawns a visible **child `Text`** for the label (§2.3),
2. opts the widget into **focus-on-click** + the **`:focus-visible`** signal (§2.4, with C3/C6),
3. marks internal nodes **`Pickable::IGNORE`** so the pick resolves to the widget root (§2.5, with C1/C3),
4. runs a **`Changed`-gated visual system** reading the a11y-state component(s) the widget carries, to drive fill/checkmark/strike/accent + pointer hover/active (§2.2).

C4 does **not** define the widget marker, the bundle, the `#[require]` set, the `A11yContract`, or the keyboard handlers — those are the agent-interface bundle (P1d). C4's deliverable per widget is the **scene-fn + the visual system + the pick-through marker**, layered on top of the existing bundle.

### 2.2 Per-state visual driven by the agent-interface state components

The visual systems read the agent-interface state components (`a11y/states.rs`) via change-detection and never write them back (the lowering is the agent-interface campaign's one-way path; C4 reads for paint only):

| Reads (agent-interface component) | Drives the visual |
|---|---|
| `A11yToggled(Toggled)` — `{False, True, Mixed}` | Checkbox/Switch/toggle-Button fill + checkmark/dash/thumb position. `Mixed` → the dash glyph; `True` → check; `False` → empty. |
| `A11ySelected(bool)` | Listbox-option / row selected accent (background + foreground token swap). |
| `A11yExpanded(bool)` | Disclosure caret rotation / panel-shown affordance. |
| `A11yDisabled` (marker) | dimmed foreground + suppressed hover/active feedback (the marker also prunes focus/hit-test/announce — those walks are owned by C5/C3/agent-interface respectively). |

Because the agent-interface state components are `Reflect` + `Default` + change-detected (`semantic-tree.md §1`: "each is independently `Default`-able, change-detected, and BSN-patchable"), C4's visual systems gate on `Changed<A11yToggled>` / `Changed<A11ySelected>` / etc. A toggle from **any** source — pointer, keyboard Space, or an inbound AT `Action::Click` — flows through the agent-interface router into the **same** `A11yToggled` write, and the `Changed` filter fires the repaint exactly once. There is no per-state component owned by this campaign; the single source of truth is the agent-interface state.

**The one-frame visual lag** (the `Changed`-gated repaint may trail the state flip by a frame) matches the prototype's documented, imperceptible lag and the agent-interface lowering's own one-frame inbound latency (`phasing.md` risk 2). No AT-visible regression, since the AT reads the substrate directly, not C4's paint.

**Pointer hover/active visual.** C4 adds the hover/active styling driven by C3's `Pointer<Over>`/`Pointer<Out>`/`Pointer<Press>`/`Pointer<Release>` over the widget root. This is purely visual feedback (token swap on hover, depressed look on press); it is not the activation path (that is the agent-interface router's `OnPress`).

### 2.3 Visible label as a child `Text` scene-fn

The accessible name (`A11yLabel`, agent-interface) and the **visible** label are decoupled. The agent-interface bundle carries `A11yLabel` (the AT name; ACCNAME 1.2 effective name computed in `accname.rs`). C4 authors the **visible** label as a **child `Text` entity** via a scene-fn — the bevy_ui_widgets/`children![]` pattern — **not** co-located on the widget root (the prototype hack, audit §7). The widget root keeps `A11yLabel`; the child `Text` carries the pixels.

The scene-fn (e.g. `checkbox(label)`) spells the agent-interface bundle plus the child `Text` as mergeable `bsn!` field-patches, mirroring the existing `button()` scene-fn (`scene.rs:53-71`). It composes onto the agent-interface bundle — it does **not** redefine the bundle's `#[require]` set. Concretely the scene-fn layers `[ Text(label) ]` (and the `Pickable::IGNORE` of §2.5) onto whatever the P1d bundle already requires.

### 2.4 Focus-on-click + the `:focus-visible` opt-in (with C3/C6)

C4 consumes C3's focus-on-click signal (umbrella §6.4) and C6's ring paint. Concretely:
- **focus-on-click**: the widget opts into setting `FocusedEntity` on the chosen press event over the widget root, mirroring the existing `text_input::focus_on_click` (`text_input.rs:133-149`) shape. C3 owns *whether* this is a `Pointer<Press>` observer or a system; C4 owns *that each catalog widget opts in* (the prototype's gap, audit W13).
- **`:focus-visible`**: C4 ensures the widget root carries whatever focus-visible signal C6's ring-lowering keys off (component shape pinned by C3/C5/C6 per umbrella §6.6). C4 emits the signal; C6 paints the ring.

Note: the **keyboard activation** (Button Enter+Space, Checkbox Space-only, etc.) is the agent-interface APG keyboard contract (P1d, `widget-contracts.md §4-5`), **not** C4. C4's keyboard concern is limited to the focus-visible decay heuristic that distinguishes keyboard focus from pointer focus, coordinated with C3/C6.

### 2.5 Pick-through: `Pickable::IGNORE` on internal nodes (with C1/C3)

A widget rendered as `Widget [ Text(label) ]` must hit-test **through** the child `Text` to the widget root (the entity the agent-interface router resolves from `NodeId`). C4 authors the internal nodes (the child `Text`, any inner visual layers) as `Pickable::IGNORE`-or-bubbles per C3's `Pickable` propagation contract, on C1's fixed coordinate basis.

**This is where C4 delivers the agent-interface campaign's deferred dependency.** The agent-interface campaign's `HitTargetable` ("not obscured") needs a **stacking-aware `hit_test`** (its follow-up #3: `picking::hit_test` is currently smallest-AABB and stacking/top-layer unaware, `phasing.md` follow-up #3). This campaign's C1 (coordinate basis) + C3 (`painters_z`/stacking pick-depth) **build that stacking-aware hit-test**, which the pick-through here rides on. C4 declares the dependency on C1+C3 and supplies the per-widget `Pickable::IGNORE` markers; C3 supplies the stacking-aware resolution.

---

## 3. Decisions & rejected alternatives

**D1 — C4 consumes the agent-interface a11y-state components; it defines no parallel state types** (resolves the §2.3-superseded "Checked/Pressed/Selected representation" question under umbrella §2.7). The visual systems read `A11yToggled`/`A11ySelected`/`A11yExpanded`/`A11yDisabled` from `a11y/states.rs`. **Runner-up — define `buiy_widgets`-local `Checked`/`Pressed`/`Selected`/`InteractionDisabled` and lower them into the a11y layer** (the pre-§2.7 design): rejected because the agent-interface campaign **owns** the a11y-state substrate (umbrella §2.7, LOCKED #5), the substrate's tri-state `A11yToggled` (incl. `Mixed`, role-disambiguated) is exactly the fix the prototype's `A11yToggled(bool)` needed, and two parallel state models would mean two lowering paths and a golden re-bless war. The umbrella's coordinate-don't-cede call stands: one state model, owned there, consumed here for visuals.

**D2 — Activation flows through `OnPress` + the agent-interface router; no `Activate` event** (resolves "event vocabulary" under umbrella §2.7). The agent-interface router maps an inbound `Action::Click` → `OnPress` (`widget-contracts.md §4`: "a keyboard event and an inbound `Action::Click` lower into the same sink — `MessageWriter<OnPress>`"). C4 emits no activation event and adds no AT bridge. **Runner-up — a Buiy-native `Activate` `EntityEvent` + an `Action::Click`→`Activate` bridge owned by C4** (the pre-§2.7 design): rejected — it competes with the agent-interface router's `OnPress` convergence point, and umbrella §2.7 explicitly removes the competing `Activate`. C4's pointer hover/active is purely visual; activation is the substrate's.

**D3 — Adopt the landed role names (`TextInput`/`MultilineTextInput`/`Region`/`Group`); do not add roles** (resolves the "TextField relabel + role expansion" the pre-§2.7 draft owned). The role names are the **landed** agent-interface ones (`a11y/mod.rs:41-47`): `TextInput` (not `TextField`), `MultilineTextInput` (not `MultilineTextField`), `Region`, `Group`. C4's visuals key off these names. **Runner-up — C4 owns a `TextField`/`Searchbox`/`Option`/`MenuItem`/`Status`/`Alert` expansion + `role_to_accesskit` + `KNOWN_ROLES`** (the pre-§2.7 draft): rejected — the `A11yRole` enum is the agent-interface campaign's (it adds roles *there*, with both stringifiers in lockstep, `semantic-tree.md §4`). C4 must not touch the enum or either stringifier; if a C4 visual needs a role the substrate has not landed, C4 raises it with the agent-interface campaign rather than adding it here.

> **Widget-name mapping (pin for C2/C8 readers):** where this campaign's specs say **`TextField`** (C2's controlled-`value`/`ValueChange<String>` lifecycle, C8's S1 screen), it denotes **the visible, controlled-value *rendering* layer this campaign supplies over the agent-interface `TextInput` bundle** — *not* a competing widget or `A11yRole`. The bundle + `A11yContract` + APG keyboard + the `A11yRole::TextInput`/`MultilineTextInput` role are the agent-interface campaign's (P1d); this campaign renders that bundle (the field box, caret/selection paint via the text stack, focus ring, placeholder visual) and reads its `A11yTextValue`/`A11yPlaceholder` for display. Read `TextField` as "the styled `TextInput`," never as a C4-owned role/bundle.

**D4 — The visible label is a child `Text`; the accessible name stays `A11yLabel` on the root** (unchanged from the pre-§2.7 design — this half is squarely C4's). The decoupling is the bevy_ui_widgets pattern and fixes the prototype's co-located-`Text` hack (audit §7). **Runner-up — co-locate the visible text on the widget root** (the prototype): rejected — it conflates the AT name source with the rendered pixels and breaks pick-through. C4 owns the child `Text`; the agent-interface bundle owns `A11yLabel`.

**D5 — The visual reads state via `Changed<…>` change-detection, one-way** (unchanged in spirit; now reading the agent-interface components). The substrate's components are change-detected (`semantic-tree.md §1`); C4 gates repaints on `Changed`. **Runner-up — poll every frame** (no change-detection): rejected for the needless per-frame repaint cost; the agent-interface components are designed for `Changed`-gating.

**Removed/ceded vs the pre-§2.7 draft (now the agent-interface campaign's):** the separate `ToggleState`/`Checked`/`Pressed`/`Selected`/`InteractionDisabled` component **types**; the `A11yRole` expansion + `role_to_accesskit` + the verify `KNOWN_ROLES`/`role_to_str` change; the one-way `build_tree`→`A11yNodeView`→`translate.rs` lowering; the per-widget **APG keyboard contracts**; the canonical widget **bundles** (Checkbox/Switch/Radio/TextField construction + `#[require]`); the `Activate`/`ValueChange<T>`/`Set<X>` event vocabulary + the `Action::Click`→`Activate` bridge. These are agent-interface **P1a/P1c/P1d** (`semantic-tree.md`, `widget-contracts.md`, `phasing.md`).

---

## 4. Contracts & interfaces

**Consumed from the agent-interface campaign (do not redefine):**
- `buiy_core::a11y::states::{A11yToggled(Toggled), A11ySelected(bool), A11yExpanded(bool), A11yDisabled, A11yValue, A11yTextValue, A11yPlaceholder, …}` — read for visuals via change-detection (`semantic-tree.md §2`, P1a).
- `buiy_core::a11y::A11yRole` — landed names `Checkbox`/`Switch`/`Slider`/`TextInput`/`MultilineTextInput`/`Region`/`Group` (`a11y/mod.rs:41-47`); C4 keys visuals off these, adds none.
- The one-way lowering (`build_tree` → `A11yNodeView` derive fold → `translate.rs`) and the `A11yContract` + action router (`Action::Click`→`OnPress`) — agent-interface P1a/P1b/P1c.
- The canonical APG widget **bundles** in `buiy_widgets` (Checkbox/Switch/Slider/Disclosure/Dialog/Tooltip/TextInput) + their **APG keyboard** — agent-interface P1d (`widget-contracts.md §5`, `phasing.md` Phase 1d).
- `OnPress` (the existing message) — the activation sink the router + keyboard converge on.

**Shared contracts referenced (umbrella §6, do not redefine):**
- **§6.4 Focus** — C3 provides focus-on-click + the `:focus-visible` signal; C5 owns the focus tree; C6 owns the ring paint; **C4 opts each widget in** to focus-on-click + emits the focus-visible signal. The agent-interface `A11yDisabled` marker gates focus traversal (C5), AccessKit prune (agent-interface a11y), and hit-test (C3) — C4 reads it for the dimmed visual only.
- **§6.6 Focus-visible component shape** — produced by C3/C5, consumed by C6's ring-lowering; C4 emits the signal on the widget root.
- **C1 / C3 pick-through** — the child `Text` (§2.3) is `Pickable::IGNORE` per C3's `Pickable` propagation on C1's fixed coordinate basis. C1+C3 build the stacking-aware `hit_test` the agent-interface campaign deferred (its follow-up #3); C4's pick-through rides on it.

**Own contracts defined here (precise):**
- A per-widget **scene-fn** in `buiy_widgets` (e.g. `checkbox(label)`/`switch(label)`/…) layering a child `Text(label)` (+ `Pickable::IGNORE`) onto the agent-interface P1d bundle — §2.3.
- A per-widget **visual system** reading the relevant agent-interface state component(s) via `Changed<…>` to drive fill/checkmark/strike/accent + pointer hover/active — §2.2.
- The per-widget **focus-on-click opt-in** + **`:focus-visible` signal emission** on the widget root — §2.4.
- The per-widget **`Pickable::IGNORE`** internal-node markers — §2.5.

---

## 5. Migration / build steps (ordered; blast radius noted)

All steps re-derive on the rebased `main` per umbrella §8 (re-confirm file:line first) and land **after** the agent-interface **P1a** (state components) + **P1d** (widget bundles) for the touched widgets. Sequenced so each lands green.

1. **Visual-system scaffolding** — add the `buiy_widgets` visual systems that read the agent-interface state components (`Changed<A11yToggled>`/`Changed<A11ySelected>`/`Changed<A11yExpanded>`/`A11yDisabled`) and drive fill/checkmark/strike/accent. *Blast:* new systems in `buiy_widgets`; registration in `WidgetsPlugin`. Additive. **Test:** flipping `A11yToggled` On→Mixed→Off repaints the checkmark/dash/empty (C7 reftest or display-list snapshot).

2. **Label scene-fns** — add the per-widget scene-fns layering a child `Text(label)` (+ `Pickable::IGNORE`) onto each agent-interface P1d bundle, mirroring `button()` (`scene.rs:53-71`). *Blast:* `scene.rs`/per-widget files; prelude export of the scene-fns. **Test:** a `checkbox("Done")` spawns a child `Text` carrying the pixels while the root keeps `A11yLabel`.

3. **Focus-on-click + `:focus-visible` opt-in** — each catalog widget opts into setting `FocusedEntity` on the press event (mirroring `text_input::focus_on_click`, `text_input.rs:133-149`) and emits the focus-visible signal C6 reads. *Blast:* per-widget; consumes C3's signal. **Test:** clicking a widget sets `FocusedEntity`; the focus-visible signal appears on the root (the ring paint is C6's gate).

4. **Pick-through markers** — mark internal nodes `Pickable::IGNORE` so a hit on the label resolves to the widget root the router addresses. *Blast:* per-widget. **Test (C7 Tier-A geometry):** a click on the label-text pixel hits the widget root, not the child `Text` (rides C1+C3's stacking-aware hit-test).

5. **Pointer hover/active visual** — token swap on `Pointer<Over>`/`Out`, depressed look on `Pointer<Press>`/`Release`. *Blast:* per-widget visual systems. **Test:** hover/press change the rendered token (C7 reftest); not the activation path.

6. **Prelude expansion** — export the scene-fns from `buiy::prelude` (the app-author surface; the state components + bundles are exported by the agent-interface campaign). *Blast:* `buiy/src/lib.rs`.

---

## 6. Verification (how C7 gates this — the visual/geometry tier)

C7 owns the picking-geometry + render-content verification tier (umbrella §2.7); the agent-interface campaign owns the a11y semantic gates (#3/#4/#6/#7/#12) over its in-process driver. C4's verification rides the **C7** tier (visual + picking), not the a11y gates. Everything below **lands RED-first** (umbrella §8 risk 5).

- **Per-state visual (C7 reftest / display-list snapshot)** — flip `A11yToggled` Off→On→Mixed and assert the rendered checkmark/check/dash; flip `A11ySelected` and assert the selected accent; `A11yExpanded` and assert the caret; `A11yDisabled` and assert the dimmed foreground + suppressed hover. RED-first: before C4's visual systems, the flip repaints nothing.
- **Label rendering (C7 content-presence + reftest)** — `checkbox("Done")` renders the label pixels in a child `Text` (glyph_count>0 per C7's content-presence invariant) with the root keeping `A11yLabel`.
- **Pick-through (C7 Tier-A geometry)** — a click on a label-text pixel resolves to the widget root, exercising C1+C3's stacking-aware hit-test (the agent-interface campaign's deferred follow-up #3, delivered here). RED-first: the pre-fix smallest-AABB path or a non-`IGNORE` child mis-resolves.
- **Focus-on-click + focus-visible (C7 Tier-A)** — clicking sets `FocusedEntity`; the focus-visible signal appears (the ring paint is C6's gate).
- **No a11y-gate duplication** — the role/state/action lowering, the APG keyboard (Checkbox Space-only, Button Enter+Space), and the AT-replay are the **agent-interface gates** (#3/#4/#6/#7) over its in-process driver; C4 does **not** re-author them. C4's gates assert the *visible + pickable* layer the agent-interface gates do not exercise.

---

## 7. Open questions deferred + dependencies

**Hard dependencies (must be live before C4 code lands):**
- **Agent-interface P1a** — the decomposed a11y-state components (`a11y/states.rs`) must exist + register; C4's visual systems read them.
- **Agent-interface P1d** — the canonical APG widget bundles (Checkbox/Switch/Slider/Disclosure/Dialog/Tooltip/TextInput) must exist; C4 extends them with rendering + picking. Coordinate per-widget so neither campaign rebuilds the other's layer.
- **C3** — the `Pointer<E>` model, the focus-on-click signal, and `Pickable` propagation for the label pick-through. C4 consumes these.
- **C1** — the coordinate-space fix underlying the pick-through hit-test.
- **C6** — the focus-ring paint + per-state styling tokens C4's visual systems use; C4 emits the focus-visible signal + the state-derived style intent, C6 paints.

**Deferred / coordinated (not this child):**
- **The a11y substrate itself** — state components, roles, lowering, router, `A11yContract`, APG keyboard, the canonical bundles — owned by the agent-interface campaign (P1a/P1c/P1d). C4 consumes; it does not build any of it.
- **Slider value visual** (thumb position from `A11yValue`, orientation) — C4 supplies the thumb/track rendering keyed off `A11yValue` once the agent-interface Slider bundle (P1d) lands; the value semantics + `Increment`/`Decrement`/`SetValue` honoring are the agent-interface campaign's.
- **Dialog/Tooltip/Menu container positioning + focus-trap** — the overlay/scroll/modal *container* geometry is **C5**; C4 supplies only the leaf widget visuals (label, state, hover) for the widgets that sit in those containers.
- **Listbox/Option selection visual** — C4 paints the `A11ySelected` accent; the selection *semantics* (single/multi-select, roving) are the agent-interface + C5 (focus tree) concern.

**Soft (parallel in Wave 3):**
- **C6** — consumes C4's focus-visible signal + the state-derived style intent for the ring lowering + per-state styling. C4 emits the visual intent; C6 paints it.
- **C8** — composes the full widget set (the agent-interface bundles + C4's visuals + C5's containers) into the gallery exemplar.
