# RD5 — Migration-cost ledger: secondary readers + ripple on the clean base

**Decision: migrate in three cost tiers. LEAF keeps `A11yToggled` as the model so
READERS are untouched (only the schedule ripples after the early drain) and only a
handful of WRITERS reroute; the MENU machine is the expensive one (deletes
`sync_menu_open`/`sync_menu_dismissed`, turns `A11yExpanded`+`CssVisibility` into
bind-projections, forces the generic light-dismiss substrate to couple to
MenuModel); the editor record-tap is non-invasive (zero secondary-reader rewire).
The one desync headless tests cannot catch is the inspector's Menu "open" cell,
hardcoded "false" and refreshed every frame.**

Confidence: **high** (base reader/writer sites verified first-hand).

---

## Tier 1 — LEAF (Checkbox/Switch): cheap, readers untouched

`A11yToggled` IS the model (prototype `leaf.rs:57-63`), so every reader keeps
reading `A11yToggled` — **NO reader contract rewire.** The cost is writers + schedule:

- **(a)** reroute the OnPress writer to enqueue (done in proto:
  `buiy_widgets/src/lib.rs:78-96` → `ToggleMsg::Toggle`).
- **(b) CLOSE THE PROTOTYPE GAP:** the gallery RUNTIME multi-writer `toggle_all_rows`
  was LEFT as a direct write — **verified base `examples/buiy_gallery/src/lib.rs:1312-1313`
  `t.0 = next`** (the prototype diff did NOT reroute it). This is the exact runtime
  multi-writer the single-writer discipline exists to kill, so the prototype's
  "single-writer proven" headline is NOT fully realized in the gallery. The FINAL
  must reroute it to `ToggleMsg::Set(next)` — elevate from port-action to
  **correctness must-fix.**
- **(c)** the 3 at-spawn seed writes (`lib.rs:1160` row completed, `:3729` modal
  register switch, `:4887` showcase switch) are legitimate model seeds — convert to
  `ToggleMsg::Set` for replay fidelity OR document explicitly as Elm-flags
  model-seeds-at-construction; do not leave as silent second writers.
- **(d) the schedule ripple:** every reader that must observe a same-frame toggle
  moves `.after(ToggleLeafSet::Drain)`. The prototype SPLIT the TodoMvcPlugin chain
  (collect→apply_intents before Picking; apply_filter/update_count/restyle_completed
  after `ToggleLeafSet::Drain`) and re-pinned ShowcasePlugin. **The ordering
  constraint `.after(ToggleLeafSet::Drain).before(A11yUpdate)` is load-bearing and
  only documented in comments — a future system added to the wrong half silently
  reintroduces one-frame staleness with no compile error.**

Readers unaffected by contract but needing `.after(drain)` scheduling:
`inspector.rs:805,848`; `lib.rs:1372,1648,5689`; `checkbox.rs:197`, `switch.rs:246`
(`Changed<A11yToggled>` visuals).

---

## Tier 2 — MENU machine: expensive, cross-cutting

`MenuModel` becomes the single source; `A11yExpanded` (button) + `CssVisibility`
(menu) + `active_descendant` become `bind_menu_model` projections;
`sync_menu_open` (`menu.rs:513`) and `sync_menu_dismissed` (`menu.rs:615`) are
DELETED; `advance_expanded_on_press` gains `Without<MenuButton>` (disclosures stay);
`close_menu`/`menu_keyboard_nav`/light-dismiss all enqueue `MenuMsg`.

**The cross-cutting cost — light-dismiss coupling:** `dismiss.rs` is a GENERIC
role-agnostic substrate (tooltip/popover/modal). Verified base
`crates/buiy_widgets/src/dismiss.rs:76-77` — `close_overlay` writes
`CssVisibility::Hidden` directly, role-agnostic. The prototype branches it on
`With<MenuModel>` and calls `enqueue::<MenuModel>` — its own comment flags this as a
layering smell needing a generic dismiss-through-the-funnel hook. (See
dismiss-uninvert pressure point — net-new design, not a mechanical port.)

### The headless-invisible desync (the one tests cannot catch)

The inspector Menu "open" cell is **hardcoded "false" and refreshed every frame** —
verified base `examples/buiy_gallery/src/inspector.rs:725`
`("open", LiveCell::new("false", "color.text.dim"))`. Any headless assertion sees
"false" and passes whether the menu is open or not; only running the GUI (or a
live-interaction test that opens the menu then reads the cell text) exposes it. The
**same hardcoded-resting pattern exists for the Modal screen** (`inspector.rs:737`)
— a latent identical trap if the dialog is ever MVU-ified. FIX: rewire `:725` to
read `MenuModel.open` live + add a live-interaction test.

**Machine drain-slot caveat (gap, see synthesis):** the prototype menu KEEPS the
late `MvuSet::Drain` pinned `.after(BuiySet::A11yUpdate)` — verified prototype
`mvu/mod.rs:764-766` and `widgets/lib.rs:225` ("The MACHINE/Menu tier keeps the late
`MvuSet::Drain`"). So a keyboard-nav/pointer-driven `MenuMsg` open lags the
A11yUpdate-built a11y tree by one frame. RD1's inline mini-drain fixes only the
AT-action-originated path; the non-AT machine path is an undecided spec question.

Menu open-state test assertions on `A11yExpanded`/`CssVisibility` keep passing but
now observe a bind-projection one frame behind `MenuModel`; tests that toggle then
assert in the same frame may flake unless they pump an extra `update()` (~20
assertion sites: `menu_dismiss_c5c.rs`, `widgets/tests/menu.rs`, `interaction.rs`).

---

## Tier 3 — EDITOR: non-invasive, additive

A purely additive record tap at the one apply site (prototype
`input.rs:571-697`, `ime.rs` +52) — `TextEditState` stays the state, `EditCommand`
vocabulary unchanged, **ZERO secondary-reader rewire** (`text_input.rs:175`,
`a11y/contract.rs:474`, gallery `:1171,:1341`, `visual.rs:83` all untouched).

**One gap:** `set_value` (gallery `lib.rs:1256,1332`) seeds editor buffers OUTSIDE
the tapped apply site, so those mutations are not in the recorded edit stream —
whole-UI replay would have to re-seed them as initial conditions, not re-fold.
Decide: route through the tapped apply path, or declare an out-of-stream
initial-condition seed in the replay scope spec.

---

## Residual open-for-spec

- Fix the inspector Modal-screen hardcoded cells (`inspector.rs:737`) preemptively,
  or only the Menu cell this migration touches? (The dialog is not in this FINAL's
  MVU scope.)
- The generic dismiss-through-the-funnel hook (the single largest design decision in
  the menu ripple) — design it, or accept the one-branch coupling with a logged
  follow-up?
- Does whole-UI replay capture gallery `set_value` editor seeds, or re-apply them as
  initial conditions outside the recorded stream?
- Do the at-spawn `A11yToggled` seeds (1160/3729/4887) need `ToggleMsg::Set` for
  record/replay fidelity, or is the initial component value the replay seed?
- Re-point the menu open-state tests at `MenuModel` (authoritative) vs the
  `A11yExpanded`/`CssVisibility` projections (user-observable, bind-lag risk)?

## Key evidence (base, verified first-hand)

- `examples/buiy_gallery/src/lib.rs:1295-1316` (`toggle_all_rows` `t.0 = next` at
  :1312-1313 — the unrerouted runtime multi-writer); `:1160,:3729,:4887` (at-spawn
  seeds); `:1256,:1332` (`set_value` editor seeds outside the tap).
- `examples/buiy_gallery/src/inspector.rs:725` (Menu "open" hardcoded "false"),
  `:737` (Modal "open" same pattern).
- `crates/buiy_widgets/src/dismiss.rs:76-77` (role-agnostic `close_overlay` →
  `CssVisibility::Hidden`).
- prototype `leaf.rs:57-63,90-106`; `widgets/lib.rs:78-96,219-249`;
  `mvu/mod.rs:764-766`, `widgets/lib.rs:225` (machine keeps late drain);
  `menu.rs:513,615` (deleted sync systems); `input.rs:571-697`, `record.rs:1-46`
  (additive editor tap).
- Readers: `inspector.rs:805,848`; `lib.rs:1372,1648,5689`; `checkbox.rs:197`,
  `switch.rs:246`; `text_input.rs:175`, `a11y/contract.rs:474`, `visual.rs:83`.
