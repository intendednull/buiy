# Widget-Catalog × Agent-Interface Co-Drive — cross-campaign sequencing & contracts

`[active]` — created 2026-06-22. The master coordination plan for driving the
**widget-catalog** campaign (the app) and the **agent-interface** campaign (the
inspection tools) together, as one swarm, so the tools stay grounded against a
real application and evolve to be actually useful.

Both campaign doc trees reference **this** doc for sequencing, scope, and the
shared contracts:

- App: [`docs/specs/2026-06-22-buiy-widget-catalog-design/`](../specs/2026-06-22-buiy-widget-catalog-design/README.md) (umbrella C0 + children C1–C8)
- Tools: [`docs/specs/2026-06-18-buiy-agent-interface-design/`](../specs/2026-06-18-buiy-agent-interface-design/README.md) (P0 landed; P1a–P2 remaining) + its [campaign plan](2026-06-18-buiy-agent-interface-campaign.md)

---

## 0. Locked decisions (owner, 2026-06-22)

1. **Tight co-development.** The tool spec *bends to what the app proves it needs*
   (grounding) rather than being driven to its as-written phase boundaries.
   Consequences, applied throughout: **no AABB-only `HitTargetable` stopgap is
   ever written** — C1+C3 deliver the real stacking-aware `hit_test`, while the
   agent-interface `HitTargetable` *gate* that would consume it stays deferred
   (§3.2); if/when un-deferred it reads this `hit_test` directly, never an AABB
   shim. **Pin the drift-prone shared contracts up front** (§5); **build each
   widget bundle-then-pixels in one pass**
   (P1d bundle immediately followed by its C4 visual). The app-side correctness
   work (C1/C2/C7) runs *in parallel* with the tool substrate (P1a/P1b) from day
   one — they share no code.

2. **Demand-pulled tool scope.** We drive the inspection tools **only as far as
   the widget-catalog consumes them.** Everything else stays un-built and is
   recorded in the deferral ledger (§3) so it can be picked up later. The tools
   are **not** driven to spec completion. Each tool phase's "done" includes
   updating the ledger.

3. **Long-lived integration branch.** All waves accumulate on the integration
   branch (`worktree-todomvc-reimpl-research2`, currently one commit above
   `origin/main` @ `e54cf0c`). PR to `main` **at wave boundaries**; never
   self-merge (PR → green CI → owner's go). Review gate between every wave.

**Gate dissolution.** The widget-catalog umbrella §8 gate ("no widget-catalog
code lands until agent-interface P1a/P1c/P1d **merge**") was written assuming a
*separate team* owned the tool. One swarm now owns both, so the gate is reread:
**the widget-touching waves wait for the substrate they consume; the non-widget
app-correctness work does not.** §8 is updated to this reading in the spec
reconciliation pass.

---

## 1. Division of labor

Established in widget-catalog umbrella §2.7 ("coordinate, don't cede"); unchanged
by the co-drive except that *one swarm now owns both columns*.

| Concern | Owner |
|---|---|
| AccessKit semantic tree: roles, decomposed state/relation components, ACCNAME, the translate derive-fold | **agent-interface** (P1a) |
| Real ECS nesting, `A11yHidden` prune, gate-#12 invariants | **agent-interface** (P1b) |
| Inbound `Action` router, `A11yContract` registry, in-process driver, `EditCommand` surface | **agent-interface** (P1c) |
| Canonical APG widget **bundles** (Checkbox/Switch/Slider/Disclosure/Dialog/Tooltip/TextInput) + their contracts + APG keyboard | **agent-interface** (P1d) |
| Opt-in MCP transport | **agent-interface** (P2 — out of scope, see §3) |
| Coordinate-space correctness (absolute basis via `GlobalTransform`, clip) | **widget-catalog** (C1) |
| Editor text-integrity (the Bug-2+3 single fix) | **widget-catalog** (C2) |
| `Pointer<E>` input model + **stacking-aware `hit_test`** | **widget-catalog** (C3) |
| Widget **visual + picking layer** over the P1d bundles (label, ring, pick-through, state-driven paint) | **widget-catalog** (C4) |
| Scroll / overlay / popover / menu / modal **geometry** + focus-trap | **widget-catalog** (C5) |
| F-tier styling (shadow / border-sides / outline / focus ring into extract + shaders) | **widget-catalog** (C6) |
| Real-input verification harness (`PointerHarness`, font-reload survival) | **widget-catalog** (C7) |
| Widget-gallery exemplar | **widget-catalog** (C8) |

**Meeting points** (where one swarm hands off to itself, in one pass): C4 extends
each P1d bundle (§4 Wave 3); C5 containers compose P1d Dialog/Tooltip/Menu
bundles (Wave 4); C7 *consumes* the P1a/P1c snapshot+driver (never re-implements
it); C1+C3 *deliver* the stacking-aware `hit_test` P1c's actionability deferred.

---

## 2. The dependency DAG

Nodes: agent-interface `{P0✓, P1a, P1b, P1c, P1d, P2}` ∪ widget-catalog
`{C1…C8}`. Edges are "X must precede Y" with the consumed artifact.

**Internal — agent-interface:** `P0→P1a→P1b→P1c→P1d→P2` (each phase consumes the
prior's component surface / nesting / router).

**Internal — widget-catalog:** `C7→C1` (C7's Tier-A harness is the RED-first
regression gate that *proves* C1) · `C1→C3` (C3 hit-tests in C1's absolute basis;
hardest edge) · `C7→C3` (C7's Tier-A is the green-gate for C3's ~18-file
`Hovered` migration — RED-first in Wave 1, GREEN as C3 lands in Wave 2) ·
`C7→C2` (C7 owns the font-reload-survival RED tests C2 un-ignores) · `C1,C3→C5` ·
`C3,C4→C5` · `C1,C3→C6` · `C7→C6` (goldens) · `C3,C4,C5,C6,C7→C8`.

**Cross-campaign — app delivers to tool:**
- **`C1 + C3 → (P1c deferred follow-up #3)`:** C1's absolute basis + C3's
  `painters_z`/stacking pick-depth **are** the stacking-aware `hit_test` the
  agent-interface campaign deferred (follow-up #3, `inprocess-api.md §5.1`).
  *This is a deferred-consumer edge, not a Wave-2 build dependency:* the consumer
  — P1c's `HitTargetable` actionability gate — is itself deferred under demand-pull
  (§3.2). C1+C3 still deliver the `hit_test` in Wave 2 because C3's own picking and
  C5's overlays (Wave 4) consume it; if/when `HitTargetable` is un-deferred it
  reads this, never an AABB shim (§5 SC-3).

**Cross-campaign — tool delivers to app:**
- `P1a → C4` (C4 reads `A11yToggled`/`A11ySelected`/`A11yExpanded`/`A11yDisabled`
  for visuals — defines no state itself) · `P1d → C4` (C4 extends the bundles) ·
  `P1c → C3,C4` (keyboard/AT producers into the shared `OnPress` sink; no
  competing `Activate`) · `P1a → C5` (C5 populates `A11yLive`/`A11yModal`/
  `active_descendant` + the scroll wire fields) · `P1b → C5` (C5 reads the
  `A11yHidden` prune marker) · `P1d → C5` (containers compose the bundles) ·
  `P1a/P1c/P1d → C8` (the gallery composes bundles, reads state, activates via
  the router) · `P0/P1a → C7` (C7 consumes `semantic_tree`/`snapshot`).

**Critical path** (≈ depth 8 through the *merged* DAG, not the longer of two
spines): `P0→P1a→P1b→P1c→P1d→C4→C5→C8` — the tool spine's P1d feeds C4 (Wave 3)
which feeds C5 (Wave 4), so the cross-spine hand-offs make the true longest chain
longer than either single spine; compressed into 5 waves by the parallel-eligible
nodes. **Two co-critical serialization points:** **C3** (the ~18-file `Hovered`
migration — highest single-node blast radius, umbrella §9 risk #1) and **P1d**
(where both spines must complete before Wave-3 C4 can start). Everything visual
stands on **P1a** (state substrate) + **C1/C7** (coordinate correctness + its gate).

---

## 3. Demand-pulled scope & deferral ledger

Derived backward from app demand (C8 gallery screens S1–S5 + C5 containers + C4
visuals + C7 assertions) against the full agent-interface spec. **Default:
DEFER** anything with no concrete gallery consumer.

### 3.1 BUILD set (the minimal substrate)

- **P1d widgets — all 8 bundles BUILD** (the gallery was designed to exercise the
  whole set), but **single-role / single-variant**: TextInput (single-line),
  Checkbox, Button (+Enter/Space), Switch, Slider (single-thumb), Disclosure
  (single), Dialog (plain), Tooltip-trigger.
- **P1a state — 12 of ~21 components BUILD:** `A11yToggled` (tri-state),
  `A11yExpanded`, `A11ySelected`, `A11yDisabled`, `A11yValue`, `A11yTextValue`,
  `A11yPlaceholder`, `A11yModal`, `A11yHidden`, `A11yLive`, `A11yOrientation`,
  `A11yHasPopup` (S3's MenuButton + the gallery screen-switcher populate it).
- **P1a relations — 4 of 8 fields BUILD:** `labelled_by`, `described_by`,
  `controls`, `active_descendant`. (The `A11yRelations` *struct* carries all 8 —
  `Reflect` is cheap — but the deferred fields get no populate-side system and no
  fold-arm test.)
- **P1a `A11yNodeView` widening — adds the SC-4 scroll wire fields** (scroll
  offset + content/viewport extent + a scrollable flag): the single coordinated
  wire-format change. P1a owns re-blessing the existing a11y snapshots when it
  widens (new fields appear as defaults); C5 (Wave 4) only populates them.
- **P1a `A11yRole` additions:** `A11yLive`'s role-implied derivation
  (`resolve_live`) needs the `Status` role (S1's "N items left" live-region) —
  P0's enum stops at `Group`. P1a adds `Status` (+ `Alert`/`Log` for the complete
  role-implied-live set) + both stringifiers + the P0 `KNOWN_ROLES` forcing
  function. (Surfaced verifying the P1a plan against resolved accesskit 0.24:
  `set_live_atomic` is also a no-arg marker, not `set_live_atomic(bool)`.)
- **P1b — BUILD:** default `ChildOf`/`Children` nesting + `nearest_a11y_ancestor`
  collapse, window-entity root, the **`A11yHidden` prune** (most load-bearing for
  S4), and the **three named gate-#12 invariants** (no-orphans / focus-reachable /
  every-focusable-named) applied over the gallery fixtures.
- **P1c actions — BUILD:** `Focus`/`Blur`, `Click`, `Increment`/`Decrement`,
  `SetValue` (lowers via existing `SelectAll`+`Insert` — no new editor work),
  `Expand`/`Collapse`, `ShowTooltip`/`HideTooltip`.
- **P1c driver — BUILD (minimal):** `snapshot(Unmerged)` + `perform` + the thin
  sugar C7 calls (`click`/`set_value`/`focus`/`get_by_role`/`increment`/`expand`)
  + `wait_for` (a standalone semantic-condition frame-poll — independent of the
  deferred actionability *gates*; S3 tooltip-timing / the live-region update may
  block on it) + the `ref`/off-by-one fix.

### 3.2 Deferral ledger (pick up later — no gallery consumer)

Maintained here (authoritative) and mirrored into the agent-interface
[`phasing.md`](../specs/2026-06-18-buiy-agent-interface-design/phasing.md) during
the spec-reconciliation pass. Each phase's "done" updates it.

- **P1a state:** `A11yReadOnly`, `A11yRequired`, `A11yBusy`, `A11yInvalid`,
  `A11yAutoComplete`, `A11yLevel`, `A11yPosInSet`, `A11ySetSize` — no gallery
  screen reads or populates them. (`A11yHasPopup` was moved to BUILD — §3.1.)
- **P1a relations:** `owns`, `flow_to`, `details`, `error_message` — no consumer
  (`owns` re-parent only matters for a portalled dialog; S4 is in-place).
- **P1b:** the `owns` re-parent edge; `TreeView::Merged` + `A11yMergeChildren`
  (C7 snapshots Unmerged only); the **exhaustive #12 proptest generators** (build
  the three named invariants over the gallery corpus, defer the random-tree fuzz).
- **P1c — the single cleanest cut: the entire `EditCommand::SetSelection`
  editor-work slice** + `Action::SetTextSelection` + `Action::ReplaceSelectedText`
  + `inprocess::set_selection` — the only gallery editor (S1's single-line field)
  needs only type + submit + clear + SetValue, which all lower via existing
  `SelectAll`+`Insert`. Also defer: `SetSequentialFocusNavigationStartingPoint`
  (S4 trap is driven by C5's focus machine), `CustomAction(i32)` + registry,
  `Scroll*` (already router-deferred; S2 scroll is `Pointer<Scroll>`), and the
  actionability **gates** (`act_when_actionable` / `HitTargetable` gate /
  `Stable` gate) — C7 brings its own condition-based frame-settling loop, so these
  have no consumer. (`wait_for` is **not** in this defer — it is a standalone
  semantic-condition poll, built per §3.1.)
- **P1d variants:** `MultilineTextInput` role, `AlertDialog` role, multi-thumb
  Slider, Accordion (multi-Disclosure subtree).
- **P2 (entirely out):** `buiy_mcp` transport, MCP tool envelope, capability
  tiers, auth, tree-delta push, structured-verb RPC; the transport prior-art
  folders; the P2 named follow-ups (test-ids, multi-window keying, lazy
  `TreeUpdate` diffing, richer `owns` edge cases).

### 3.3 Demand-pull assumptions (revisit when C8 screens are pinned)

These BUILD/DEFER calls rest on a current reading of C8 that is not yet
pixel-final. Each flips cheaply if a screen design changes; revisit at Wave 3/4
when C8's screens are concretely authored:

- **Q-A** single Disclosure + single-line field on S5 → `MultilineTextInput` /
  Accordion deferred. (Flip if S5 adds a textarea or accordion.)
- **Q-B** S4 Dialog authored in-place → `owns` re-parent deferred. (Flip if S4
  portals the dialog subtree to a top-layer root.)
- **Q-C** S4 is a plain `Dialog`, not `AlertDialog`.
- **Q-D** RESOLVED → BUILD: S3's MenuButton and the gallery screen-switcher
  (itself a MenuButton) **populate** `A11yHasPopup`, so it is built now — this is
  not screen-design-contingent (a MenuButton without `haspopup` is an APG defect).
- **Q-E** no list-position semantics asserted → `A11yPosInSet`/`SetSize` deferred.
- **Q-F** only the three named #12 invariants over the gallery corpus, not the
  full proptest fuzz suite.
- **Q-G** C7's Tier-A uses its own settle loop → the driver actionability loop
  deferred.

---

## 4. The interleaved wave plan

Each wave is a parallel set; review gate + (at boundaries) a PR to `main` between
waves. "Tool scope" is the demand-pulled subset from §3.

> **Wave 0 — establish the co-drive foundation.** ✅ rebased onto current `main`;
> integration branch live. Remaining: author this doc (done), pin the four shared
> contracts (§5), reconcile the spec set (tight co-dev + demand-pulled + §8
> reread), scope+plan P1a, reconcile the C1/C2/C7 plans. Front-loads the
> drift-prone seams while there's no code to churn. → kicks off P1a.

> **Wave 1 — app-correctness gate ‖ tool-substrate outbound (max concurrency).**
> App: **C7** (RED-first, first-in-wave) → **C1** + **C2** in parallel. Tool:
> **P1a → P1b** (demand-pulled: the 12 components + 4 relations + scroll fields +
> nesting + `A11yHidden` prune + 3 invariants). Two streams sharing **no edited
> code**: C7's *picking-geometry* (Tier-A) is fully P1a-independent (it is the C1
> gate); C7's *a11y-state* assertions read P1a's `semantic_tree` tier and so lag
> P1a within the wave. *Grounding:* none yet — deliberately the independent
> foundations.

> **Wave 2 — the two linchpins, coordinating on one sink.** **Step 0 (shared
> micro-gate, single-owned):** relocate `OnPress` to `buiy_core` per SC-1; C3 and
> P1c both branch from it. Then **C3** (gated on C1+C7) ‖ **P1c** (gated on P1b;
> demand-pulled actions + minimal driver, **no** `SetSelection`, **no**
> actionability gates). Disjoint code; both write the shared `OnPress` sink and
> agree on the `hit_test` semantics (SC-3). *On `hit_test`:* C1+C3 deliver the
> real stacking-aware `hit_test` here — consumed by C3's own picking now and C5's
> overlays in Wave 4; the agent-interface `HitTargetable` *gate* that would also
> consume it stays deferred (§3.2), so **no AABB stopgap is ever written**.
> *Grounding loop activated:* activation (the hit-test loop's tool-side consumer,
> `HitTargetable`, lands only if/when un-deferred; its app-side consumers exercise
> the hit_test from Wave 4).

> **Wave 3 — widgets + visual + render, per-widget meeting point.** **P1d** leads
> (the 8 bundles) → **C4** lags per-widget (label + ring + pick-through + state
> paint) ‖ **C6** (render/extract, disjoint). Sequence each P1d widget
> immediately before its C4 extension (bundle-then-pixels in one pass).
> *Grounding:* the widget meeting-point loop.

> **Wave 4 — containers.** **C5** (gated on C1,C3,C4,C7 + P1a/P1b/P1d). Scroll +
> `FocusScope` start as soon as P1a's wire surface lands; Dialog/Menu/Tooltip
> slices follow their P1d bundle. C3+C5 co-author the inert `emit_picks` filter
> in one pass. *Grounding:* the obscuring geometry (modals/menus/tooltips) that
> the stacking-aware `hit_test` must resolve now exists — C5's overlays exercise
> C3's pick-through + light-dismiss, validating the hit_test against real
> occlusion (and would validate `HitTargetable` directly if it is un-deferred).

> **Wave 5 — capstone (‖ P2 only if green-lit).** **C8** (gallery; composes
> everything, surfaces the idle-CPU + 1000-row scale investigations). **P2 stays
> deferred** (§3) unless the owner explicitly green-lights the MCP transport.
> *Grounding:* the full semantic-tree validation loop.

**Why interleaved, not tool-first:** (1) Waves 1–2 run the two campaigns'
independent foundations fully in parallel — the biggest schedule compression;
(2) it activates the grounding loops at the earliest point both halves exist
(Wave 2), so the tool is validated against real geometry, not synthetic fixtures;
(3) owning both sides collapses the `HitTargetable` AABB stopgap and the
inert-filter TODO into correct-first implementations. Runner-up "fully land
P1a→P1d, then start C*" loses on all three: it serializes genuinely-parallel
work, forces the known-wrong stopgap-then-rework, and defers every grounding loop
to after P1d.

---

## 5. Shared contracts (pinned once)

The four seams both campaigns touch. Pinned **here**; specs reference them by ID
(e.g. "per co-drive SC-1") and must **not** redefine them.

### SC-1 — the `OnPress` activation sink
- **Shape:** `OnPress(pub Entity)`, a Bevy `Message` (read via `MessageReader`).
- **Move:** relocate the type **and its `add_message::<OnPress>()` registration**
  from `buiy_widgets::button` / `WidgetsPlugin` to a **`buiy_core`** plugin (the
  router's plugin), re-exported from `buiy_widgets` for source-compat. The P1c
  action router lives in `buiy_core` and **cannot** depend on `buiy_widgets`, so
  the sink must live in `buiy_core` (verified: `buiy_widgets`→`buiy_core` is the
  only edge; the lone writer is `emit_on_press_on_click`, lone reader a button
  test, both preserved by the re-export). *Sequencing:* the relocate is **Wave-2
  step 0** — a single-owned micro-gate that lands before C3 or P1c body work; both
  branch from it (not "whichever lands first").
- **Producers:** C3 (pointer `Click` → `OnPress`); P1c (`Action::Click` → `OnPress`,
  and Button Enter/Space → `OnPress`). **No competing `Activate` event** — a
  second sink would fork the activation grounding loop (§6).
- **Consumers:** widget logic (Checkbox advances `A11yToggled`, Button fires its
  callback, etc.).

### SC-2 — the `:focus-visible` signal
- **Shape:** the existing pair of `buiy_core` resources —
  `FocusedEntity(Option<Entity>)` (`focus.rs:38`) + `FocusVisible(bool)`
  (`focus.rs:45`). **No new per-entity focus component** is in demand-pulled scope
  (no screen needs per-entity focus-visible) — this drops the spec's "v1 reads
  resources, swap to a component later" two-step.
- **Producers:** keyboard focus (Tab) sets `FocusVisible = true` (exists); **C3
  adds the missing pointer-clear path** (focus-on-click sets `FocusVisible =
  false` — today it "is never reset to false", `focus.rs:16`); C5 traversal
  updates `FocusedEntity`.
- **Consumers:** C6 paints the focus ring iff `entity == FocusedEntity.0 &&
  FocusVisible.0`; C4 may read it for opt-in focus visuals.
- **Scaling assumption (load-bearing):** the pair models a *single global focus in
  a single window* — `FocusVisible` is one bool, not per-entity. Sufficient for
  the demand-pulled scope; revisit (a per-entity component, or per-window keying)
  only when multi-window or remembered-per-entity focus-visible is actually
  needed. Named here so the "drop the two-step" decision stays auditable.

### SC-3 — the stacking-aware `hit_test` (pick-through convention)
- **Signature (unchanged, stays `pub`):** `pub fn hit_test(world: &World, point:
  Vec2) -> Option<Entity>` (`picking/mod.rs:37`). C3 also exposes
  `pub fn global_paint_order(...) -> Vec<Entity>` (`picking/depth.rs`).
- **Semantics:** C1 makes it read the **absolute** basis (non-optional
  `GlobalTransform`); C3 replaces the smallest-area depth (`backend.rs:50`) with
  **stacking/paint-order** (topmost-painted wins). This is a depth-rule
  replacement **plus net-new `Pickable`-query surface**: today neither `hit_test`
  (`mod.rs:38-48`) nor the backend `emit_picks` (`backend.rs:27-46`) reads
  `Pickable` at all, so honoring `Pickable::IGNORE` pass-through +
  `should_block_lower` + clip bounds is new filtering C3 adds. `Pickable::IGNORE`
  is confirmed available in the resolved `bevy_picking 0.19.0`.
- **Two paths must agree (correctness trap):** the area/depth logic is currently
  **duplicated** in the free `hit_test` (`mod.rs:42-46`) and the backend
  `emit_picks` (`backend.rs:42-53`). `emit_picks` drives `Hovered`, which fires
  `OnPress` (`button.rs:124`); the a11y `HitTargetable` reads the free `hit_test`.
  C3 must land the stacking rule in **both** — or, preferred, unify them on shared
  depth logic — so pointer activation and AT actionability can never disagree.
- **`Pickable::IGNORE` convention:** decorative widget internals (a label, an
  icon) carry `Pickable::IGNORE` so a hit resolves to the **widget-root** entity
  the a11y router addresses by `NodeId`. C3 asserts the convention; C4 / P1d
  author it per widget.
- **Tool consumption:** P1c's `a11y/inprocess.rs` reads this `pub` hit_test for
  its `HitTargetable` actionability gate so it can mean "not obscured" — the
  deferred follow-up #3. **Under tight co-dev, P1c ships no AABB-only stopgap;**
  `HitTargetable` (if/when built) reads this. (Per §3 the actionability *gate* is
  itself deferred — but if it lands, it lands on this, not on an AABB shim.)

### SC-4 — the scroll wire fields on `A11yNodeView`
- **Where:** P1a's widening of `A11yNodeView` (`a11y/mod.rs:66`) is the **single
  coordinated wire-format change** that adds the scroll fields (scroll offset +
  content/viewport extent + a scrollable flag).
- **Producer:** C5 populates them on scroll containers. **Consumer:** C7 may
  assert them; AT exposure rides them.
- **Pin timing:** the exact field set (names/types) is finalized **with the P1a
  plan** (Wave 1) from `scroll-overlay-modal.md §Coordination`, *before* the
  widening lands — so C5 (Wave 4) populates a schema that already exists. C5 adds
  **no** competing scroll component to the a11y view.

---

## 6. Grounding loops (why co-develop)

1. **Hit-test loop (headline — partially deferred).** C1 (basis) + C3 (paint-order
   pick-depth) *build* the stacking-aware `hit_test`; C5's overlays/modals *are*
   the obscuring geometry that *exercises* it (Wave 4); C8 drives it under real
   input. The tool-side consumer — P1c's `HitTargetable` "not obscured" gate — is
   **deferred** under demand-pull (no gallery path calls it), so this loop closes
   on the app side now and on the tool side only if/when `HitTargetable` is
   un-deferred; either way it will read this real hit_test, never an AABB shim.
   Co-development means the `hit_test` the tool deferred gets *built and exercised*
   regardless.
2. **Activation loop.** C3 emits the pointer producer into `OnPress`; P1c emits
   the keyboard+AT producers into the *same* sink; C4 reads the resulting
   `A11yToggled` flip; C7 asserts the flip regardless of producer. One sink (SC-1,
   no competing `Activate`) lets one C7 test validate all three input paths.
3. **Semantic-tree validation loop.** C7 *causes* a bulk op via real synthetic
   clicks; the agent-interface `semantic_tree` tier *observes* the resulting tree;
   the #3/#12 assertion lives at the tool gate but the causation is C7's geometry.
4. **Widget meeting-point loop.** P1d builds bundle+contract+APG-keyboard; C4 adds
   pixels; C5 puts it in a container; C8 runs it under real input — surfacing
   whether the three actually compose.
5. **Text set-channel loop.** C2 proves typed/seeded content survives a font load;
   P1c lowers `SetValue` through the *same* `SelectAll`+`Insert` channel — an
   AT-set and an app-set traverse identical code.

---

## 7. Delivery & process

- **Integration branch.** `worktree-todomvc-reimpl-research2` is the long-lived
  integration branch; all waves accumulate here. PR to `main` at wave boundaries.
  **Never self-merge** — PR → green CI → owner's go (standing rule). Keep the
  branch rebased on `origin/main` between waves.
- **Verification.** Headless gate (`cargo fmt`/`clippy`/`doc`/`nextest`, xvfb on
  Linux) must stay green per wave; the GPU `--ignored` lane (`buiy_core` +
  `buiy_verify`, pinned lavapipe) is additive for any render/golden work (C6/C8).
  C7 *extends* the landed nextest config + the two consolidated test binaries —
  never a parallel rig.
- **Fan-out discipline.** Every parallel-agent wave runs under
  `reliable-agent-fleet`: structured-output contracts, count returns vs spawns,
  retry the gaps, no synthesis from partial coverage. Review gate (fresh-context
  agents) after research, after the spec/plan reconciliation, and after each
  implementation wave.
- **Deferral-ledger maintenance.** §3.2 is authoritative and mirrored into the
  agent-interface `phasing.md`. When a phase lands, mark what it built and move
  anything newly-touched between BUILD/DEFER, so the "what's left of the
  inspection tools" picture stays honest for a later pickup.
- **Demand-pull revisits.** The §3.3 assumptions are revisited when C8's screens
  are pixel-pinned (Wave 3/4); a flip is a cheap one-component / one-arm addition.
