# Buiy Agent-Interface Phase 1a — Decomposed State Surface (Outbound) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> `superpowers:subagent-driven-development` (recommended) or
> `superpowers:executing-plans` to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

> Part of the [co-drive](2026-06-22-widget-catalog-agent-interface-codrive.md);
> Wave 1 — the authoritative sequencing / scope / shared-contract source
> (§3.1 the P1a BUILD set, §3.2 DEFER ledger, §5 SC-4, §6 grounding loops). This
> plan sequences the design in
> [`semantic-tree.md`](../specs/2026-06-18-buiy-agent-interface-design/semantic-tree.md)
> §§2–6 into bite-sized RED-first tasks; it does **not** redefine scope or
> contracts — read the co-drive doc for those.

**Date:** 2026-06-22
**Status:** active
**Spec:** docs/specs/2026-06-18-buiy-agent-interface-design/semantic-tree.md (the design); phasing.md Phase 1a (the order)
**Campaign:** docs/plans/2026-06-18-buiy-agent-interface-campaign.md
**Predecessor:** docs/plans/2026-06-18-buiy-agent-interface-p0-addressing.md (LANDED on `main` via PR #79 @ `a1ab149`)

**Goal:** Make the AccessKit tree richly correct per widget × state by landing the
**demand-pulled** decomposed-component substrate: the **12 state components** + the
**`A11yRelations` struct (8 fields, only 4 wired)** + a pure ACCNAME function +
the **widened `A11yNodeView`** (state `Option`s, resolved relations, and the SC-4
scroll wire fields) + the rewritten **`to_accesskit_node` derive fold** (the single
0.24-setter emission point). No inbound routing, no nesting — those are P1c / P1b.

**Architecture:** Three new files under `crates/buiy_core/src/a11y/`
(`states.rs`, `relations.rs`, `accname.rs`), a widened `A11yNodeView` + `build_tree`
query in `mod.rs`, and a rewritten `to_accesskit_node` in `translate.rs`. The fold is
the **single emission point** against the resolved accesskit 0.24 setters — every
setter signature is confirmed in that one file (standing rule §0.2). Each component is
small, public-fielded, `Reflect`-derived, and `register_type`'d in `A11yPlugin::build`
(the [semantic-tree.md §1](../specs/2026-06-18-buiy-agent-interface-design/semantic-tree.md)
megacomponent inversion). A new **gate-#3 tier** over the in-process
`accesskit_consumer` path is the lowest verification rung; C7 (widget-catalog) and
P1c consume the **same** `semantic_tree`/snapshot surface, so the consumer helper has
**one home** (`buiy_verify::a11y`), never a fork.

**Tech Stack:** Rust, Bevy 0.19.0-rc.3 ECS (`Component`/`Reflect`/`Query`/`register_type`),
`accesskit` 0.24.1 (`Node`, `Toggled`/`Live`/`Orientation`/`HasPopup` enums, the
`set_*` fold), `accesskit_consumer` 0.36.0 (the new in-process read tier),
`buiy_verify::a11y` (the snapshot serializer + the new consumer helper).

**Wave / dependencies:** Wave 1, tool-substrate spine. Depends on **P0** (the
`entity_for_node_id` inverse + the 7 new `A11yRole` variants + the canonical-ref
snapshot serializer — all LANDED #79). P1a **precedes P1b** (nesting reads the
widened `A11yNodeView`), **P1c** (the router reads this state for the live filter),
and is **consumed cross-campaign by** C4 (reads `A11yToggled`/`A11ySelected`/
`A11yExpanded`/`A11yDisabled` for visuals — co-drive §2), C5 (populates
`A11yLive`/`A11yModal`/`active_descendant` + the SC-4 scroll fields), C7 (asserts over
`semantic_tree`/snapshot), C8 (the gallery). **P1a owns re-blessing the existing a11y
snapshots when `A11yNodeView` widens** (new fields appear as defaults).

---

## § 0. Standing rules for this plan (read first)

These two rules from
[phasing.md §"Base: accesskit 0.24"](../specs/2026-06-18-buiy-agent-interface-design/phasing.md)
govern **every** task below — they are not one-time checks:

1. **Setter-verification discipline (recurring).** Every `set_*` line in the fold is
   verified against the **resolved accesskit 0.24.1**, read from the **committed
   `Cargo.lock`** (PR #78 committed it; CI runs `--locked`), **not docs.rs**. Use
   `cargo tree -p accesskit` (confirm `0.24.1`) and read the unpacked source
   (`~/.cargo/registry/src/.../accesskit-0.24.1/src/lib.rs`) or `cargo doc -p accesskit
   --open`. The canonical trap: **`set_live_atomic`, NOT `set_atomic`** — `set_atomic`
   does not exist in 0.24 (`accesskit-0.24.1/src/lib.rs:1806`:
   `(LiveAtomic, is_live_atomic, set_live_atomic, clear_live_atomic)`). Each fold-arm
   task re-states the exact signature it depends on; if a signature differs from what
   this plan asserts, **stop and fix the plan**, do not guess.
2. **Keep the derive fold isolated.** `to_accesskit_node` in `translate.rs` is the
   **single emission point** — the one file where every 0.24 setter signature is
   confirmed. No setter is called anywhere else. A new ARIA concept = one tiny
   component + one `A11yNodeView` field + one fold arm + (for roles) one `role_to_str`
   arm. Nothing leaks.

**Repo commit policy:** end every commit message with the repo trailer
(`Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>`). The
per-task `cargo test -p buiy_core …` commands are the fast inner loop; run the full
gate (§ "Phase 1a done") before the final wave-boundary push. **Never self-merge** —
PR → green CI → owner's go (co-drive §7).

**Verified-against-resolved-deps facts (confirmed 2026-06-22 on this branch — re-confirm
in Phase 0):**

| Fact | Resolved value | Source |
|---|---|---|
| `accesskit` | `0.24.1` | `Cargo.lock` |
| `accesskit_consumer` to **add** | **`0.36.0`** (the version the resolved `accesskit_winit 0.32.2` → `accesskit_unix 0.21.1` → `accesskit_atspi_common 0.18.1` stack already pulls; 0.35/0.37 are orphan lock entries with no reverse-dep) | `cargo tree -i accesskit_consumer@0.36.0` |
| markers (`set_disabled`/`set_modal`/`set_live_atomic`) | **no argument** (`flag_methods!`, `lib.rs:1087`) | source |
| `set_expanded`/`set_selected` | take `bool` (`bool_property_methods!`, `lib.rs:2117`) | source |
| `set_toggled`/`set_orientation`/`set_has_popup`/`set_live` | take the enum (`unique_enum_property_methods!`, `lib.rs:2137`) | source |
| `set_value`/`set_placeholder`/`set_label`/`set_description` | take `impl Into<Box<str>>` (`string_property_methods!`) | source |
| `set_numeric_value`/`set_min_numeric_value`/… | take `f64` (`f64_property_methods!`) | source |
| relation vecs (`set_labelled_by`/`set_described_by`/`set_controls`) | take `impl Into<Vec<NodeId>>` (`node_id_vec_property_methods!`) | source |
| `set_active_descendant` | takes `NodeId` (single; `node_id_property_methods!`, `lib.rs:1898`) | source |
| **scroll (SC-4 target setters)** | `set_scroll_x`/`set_scroll_x_min`/`set_scroll_x_max`/`set_scroll_y`/`set_scroll_y_min`/`set_scroll_y_max` all take `f64` (`f64_property_methods!`, `lib.rs:1971`) | source |
| `Toggled` | `{False, True, Mixed}` + `impl From<bool>` | `lib.rs:585` |
| `Live` | `{Off, Polite, Assertive}` | `lib.rs:584` |
| `Orientation` | `{Horizontal, Vertical}` | `lib.rs:417` |
| `HasPopup` | `{Menu, Listbox, Tree, Grid, Dialog}` | `lib.rs:605` |
| consumer read path | `accesskit_consumer::Tree::new(TreeUpdate, bool) → .state() → .node_by_id(NodeId) → Node` with getters `toggled()`/`is_disabled()`/`is_selected()`/`label()`/`value()`/`placeholder()`/`orientation()`/`has_popup()`/`is_modal()`/`is_live_atomic()`/`live()`/`numeric_value()`/`supports_action()` | `accesskit_consumer-0.36.0/src/{tree,node}.rs` |

> **Consumer-getter gap (impl-time flag).** The consumer `Node` (0.36) exposes
> `is_selected() -> Option<bool>` but **no public `is_expanded()` getter** was found in
> `node.rs`; the **producer** `accesskit::Node` *does* (`is_expanded() -> bool`,
> generated by `bool_property_methods!`). So: assert `A11yExpanded` at the **producer
> tier** (`to_accesskit_node` → `node.is_expanded()`); reserve the consumer `Tree` tier
> for the components the consumer surfaces directly (toggled / selected / disabled /
> modal / value / placeholder / orientation / has_popup / live / numeric). Each task
> below names which tier it asserts at. **Re-confirm the consumer getter set in Phase
> 0** — if 0.36 grew an `is_expanded()`, prefer the consumer tier uniformly.

---

## PHASE 0 (Task 0): Re-confirm anchors + add `accesskit_consumer` + stand up the gate-#3 consumer tier

**This plan's code blocks were written against the rebased base `e54cf0c` (PR #77
testing-audit + #78 CI-hardening + #79 a11y P0). Re-confirm every anchor before the
first edit.** P0 is LANDED (the 7 `A11yRole` variants, `entity_for_node_id`, the
canonical-ref serializer), so the working surface is the post-#79 `a11y/` module.

**Files**
- Read: `/mnt/storage/projects/buiy/CLAUDE.md` (Build & Test), `crates/buiy_core/src/a11y/mod.rs`, `crates/buiy_core/src/a11y/translate.rs`, `crates/buiy_core/src/a11y/adapter.rs`, `crates/buiy_core/Cargo.toml`, `Cargo.toml` (workspace pins), `crates/buiy_verify/src/a11y.rs`, `crates/buiy_core/tests/crosscut.rs`, `crates/buiy_core/tests/crosscut/a11y_translate.rs`, `crates/buiy_verify/tests/verify_headless.rs`, `crates/buiy_verify/tests/verify_headless/a11y.rs`, `deny.toml`
- Modify: `crates/buiy_core/Cargo.toml` (add `accesskit_consumer`), `crates/buiy_verify/Cargo.toml` (add `accesskit_consumer` as a dep for the new helper — confirm where the helper lives)

**Steps**
- [ ] **Confirm the base.** `git fetch --all --prune`; `git log --oneline -1 origin/main` → expect `e54cf0c`. Confirm `crates/buiy_core/src/a11y/` contains **only** `mod.rs`, `translate.rs`, `adapter.rs` — **no `states.rs`/`relations.rs`/`accname.rs` yet** (this plan creates them). Confirm P0 landed: `grep -n "entity_for_node_id" crates/buiy_core/src/a11y/translate.rs` and the 7 new `A11yRole` variants in `mod.rs` (`Checkbox`…`Group`).
- [ ] **Re-grep the anchors this plan cites and fix drift:**
  - `grep -n "pub struct A11yNodeView" crates/buiy_core/src/a11y/mod.rs` → the 5-field struct at **`mod.rs:66`** (`entity`/`role`/`name`/`description`/`focusable`).
  - `grep -n "pub(crate) fn build_tree" crates/buiy_core/src/a11y/mod.rs` → `build_tree` at **`mod.rs:99`**; its query tuple at **`mod.rs:101-107`**.
  - `grep -n "fn build\b" crates/buiy_core/src/a11y/mod.rs` → `A11yPlugin::build` at **`mod.rs:88`** (the `register_type` chain at `90-94`).
  - `grep -n "pub fn to_accesskit_node" crates/buiy_core/src/a11y/translate.rs` → **`translate.rs:40`**; `role_to_accesskit` at **`translate.rs:57`**; `build_tree_update` at **`translate.rs:81`**.
  - `grep -n "fn role_to_str\|KNOWN_ROLES" crates/buiy_verify/src/a11y.rs` → `role_to_str` at **`a11y.rs:31`**, `WireNode` at **`a11y.rs:11`**, `KNOWN_ROLES` at **`a11y.rs:103`**.
- [ ] **Verify the accesskit 0.24 setter signatures (standing rule § 0.1).** Run `cargo tree -p accesskit` → expect `0.24.1`. Spot-check the canonical trap and the SC-4 scroll setters against the unpacked source:
  ```sh
  AK="$HOME/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/accesskit-0.24.1/src/lib.rs"
  grep -nE 'set_live_atomic|set_atomic|set_toggled|set_expanded|set_selected|set_scroll_x\b|set_scroll_y\b' "$AK"
  ```
  Expected: `set_live_atomic` present (`lib.rs:1806`), `set_atomic` **absent**; `set_scroll_x`/`set_scroll_y` present. If the registry path differs, find it: `find ~/.cargo/registry/src -type d -name 'accesskit-0.24.1'`.
- [ ] **Add `accesskit_consumer` to `crates/buiy_core/Cargo.toml`** under `[dependencies]` (the producer crate needs it only if the consumer helper lands in `buiy_core`; **decide the helper's home in the next step** — if it lands in `buiy_verify`, add it there instead). Pin the resolved version:
  ```toml
  # In-process AccessKit consumer — the gate-#3 read tier (semantic-tree.md §2,
  # inprocess-api.md). Version matches the resolved accesskit 0.24.1: the
  # accesskit_winit 0.32.2 stack already pulls accesskit_consumer 0.36.0
  # (cargo tree -i accesskit_consumer@0.36.0), so this adds NO new graph node.
  accesskit_consumer = "0.36"
  ```
  Add the workspace pin to the root `Cargo.toml` `[workspace.dependencies]` if the project pins accesskit centrally (it pins `accesskit = "0.24"` / `accesskit_winit = "0.32"` there — mirror with `accesskit_consumer = "0.36"` and reference `accesskit_consumer.workspace = true`).
- [ ] **Decide the consumer-helper home (no fork — co-drive C7 contract).** C7, P1c, and P1a all read the **same** semantic-tree snapshot surface. The snapshot serializer already lives in `buiy_verify::a11y` (`snapshot_tree`/`diff_snapshots`). **Put the new consumer helper there too** — a single `buiy_verify::a11y::consume(views: &[A11yNodeView], focused: Option<NodeId>) -> accesskit_consumer::Tree` (or a thin `assert_node` helper that builds the `TreeUpdate` via `build_tree_update`, wraps it in `accesskit_consumer::Tree::new(update, true)`, and returns a node-by-id accessor). This means `accesskit_consumer` is a **`buiy_verify` dep**, not (necessarily) a `buiy_core` one — confirm which crate actually constructs the consumer `Tree` and add the dep there. Rationale recorded inline: "C7 (widget-catalog) and P1c consume this same helper; it has one home so the three never fork — co-drive §1, §6 loop 3."
- [ ] **`cargo deny check` (supply-chain gate, CLAUDE.md).** Run it after the manifest edit:
  ```sh
  cargo deny check
  ```
  Expected: clean (no new advisory / license / ban for `accesskit_consumer 0.36.0` — it is already in the graph transitively). If `cargo deny` flags a duplicate `accesskit_consumer` (the 0.35/0.37 orphan lock entries), that is a **pre-existing** multi-version note, not introduced here — record it, do not "fix" it inline (scope discipline).
- [ ] **Stand up the gate-#3 consumer tier (the new lowest rung).** Add ONE foundational fixture proving the round-trip producer→consumer works for an existing P0 component, so every later component task just adds an assertion. In `crates/buiy_verify/tests/verify_headless/a11y.rs` (the established a11y test module; reached via the `verify_headless` group binary), add:
  ```rust
  #[test]
  fn consumer_reads_back_a_button_label() {
      // The gate-#3 in-process tier: build the real TreeUpdate via the fold,
      // hand it to accesskit_consumer, and read the node back the way an AT does.
      // This is the rung every P1a component fixture stands on.
      let views = vec![A11yNodeView {
          entity: entity(1),
          role: A11yRole::Button,
          name: "Save".into(),
          // ... remaining A11yNodeView fields default after the Task-4 widening ...
          ..A11yNodeView::default()   // see Task 4: A11yNodeView gains Default
      }];
      let tree = buiy_verify::a11y::consume(&views, None);
      let node = tree.state().node_by_id(
          buiy_core::a11y::translate::node_id_for(entity(1))
      ).expect("node present");
      assert_eq!(node.label().as_deref(), Some("Save"));
  }
  ```
  > **Sequencing note:** this fixture references the widened `A11yNodeView`
  > (`..A11yNodeView::default()`) and the `consume` helper, both of which land in Task 4.
  > Write the helper + this fixture **as part of Task 4** (or land Task 4 first); Phase 0
  > only *registers the dep and decides the home*. The bullet here pins the shape so Task 4
  > implements it, not a stray early file. If you prefer, defer this exact fixture into
  > Task 4 and let Phase 0 end at "dep added, home decided, `cargo deny` green."
- [ ] **Baseline gate green.** Confirm the existing a11y tests pass on the clean base before any component lands:
  ```sh
  cargo test -p buiy_core --test crosscut a11y
  cargo test -p buiy_verify --test verify_headless a11y
  ```
  Expected: all existing a11y_translate + verify a11y tests PASS. Record the count.
- [ ] **Commit:** `build(a11y): add accesskit_consumer 0.36 + cargo deny green (P1a Phase 0)`

---

## § How each component task is shaped (the bite-sized RED-first template)

Tasks 1–12 each add **one** state component. Every one follows the identical shape so
the wave can fan out per-component under `subagent-driven-development`:

1. **RED — gate-#3 fixture first.** In `crates/buiy_verify/tests/verify_headless/a11y.rs`
   (or the producer-tier `crates/buiy_core/tests/crosscut/a11y_translate.rs` when the
   consumer lacks the getter — see § 0 gap), write a fixture that builds an
   `A11yNodeView` carrying the new state and **asserts the setter's observable output**
   through the consumer `Tree` (or producer `Node`). Run it — expect a **compile RED**
   (the `A11yNodeView` field / the component doesn't exist yet).
2. **GREEN part A — the component.** Add the `#[derive(Component, Reflect, FromReflect,
   Default, Clone, Debug, PartialEq)] #[reflect(Component)]` struct to `states.rs` with the
   exact shape from [semantic-tree.md §2](../specs/2026-06-18-buiy-agent-interface-design/semantic-tree.md);
   `register_type` it in `A11yPlugin::build`.
3. **GREEN part B — the `A11yNodeView` field + the `build_tree` read + the fold arm.**
   Add the `Option<…>` field to `A11yNodeView`, widen the `build_tree` query tuple to read
   `Option<&A11yX>` and populate it, and add the **one fold arm** to `to_accesskit_node`
   against the § 0-verified setter signature.
4. **GREEN — run the fixture, expect PASS.** Then run the whole a11y suite; re-bless any
   widened-struct snapshot (Task 4 owns the first re-bless).
5. **Commit** (one component per commit).

> Tasks 1–12 may be split across parallel agents **after Task 4 lands** (the
> `A11yNodeView` widening + `Default` + the `build_tree` query skeleton are the shared
> spine they all edit). Run them under `reliable-agent-fleet`: each agent owns one
> component, returns its fixture name + the fold-arm line, and the coordinator counts 12
> returns before the wave-close gate. Until Task 4 lands, Tasks 1–12 conflict on
> `mod.rs`/`translate.rs` and must serialize.

---

## Task 1: `A11yValue` — the valued-range component (do this first; it is the only multi-field one)

Land the five-numeric-field valued range first because it is the structurally hardest
fold arm (it sets six setters) and the only § 1 "surgical exception" group — getting it
right de-risks the simpler markers. [semantic-tree.md §2](../specs/2026-06-18-buiy-agent-interface-design/semantic-tree.md):
`A11yValue { now, min, max: f64, step, jump: Option<f64>, text: Option<String> }`.

> **Ordering caveat:** Task 1's fixture references the widened `A11yNodeView`. If you
> follow the strict template, **land Task 4 (the widening + `Default`) before Task 1's
> GREEN** — or fold Task 4's `A11yNodeView` skeleton into Task 1. The plan lists Task 4
> separately for clarity; the implementer may merge the widening into the first component
> task. The remaining 11 components then only *add a field*, never reshape the struct.

**Files**
- New: `crates/buiy_core/src/a11y/states.rs` (create; add `A11yValue`)
- Modify: `crates/buiy_core/src/a11y/mod.rs` (`pub mod states;` + `A11yNodeView.value: Option<A11yValue-projection>` + query + register), `crates/buiy_core/src/a11y/translate.rs` (fold arm)
- Test: `crates/buiy_verify/tests/verify_headless/a11y.rs` (gate-#3 fixture, consumer tier)

**Steps**
- [ ] **RED — fixture first.** Assert a slider's numeric value round-trips through the
  consumer (`node.numeric_value() == Some(0.5)`, min/max likewise):
  ```rust
  #[test]
  fn consumer_reads_slider_numeric_value() {
      let views = vec![A11yNodeView {
          entity: entity(1),
          role: A11yRole::Slider,
          name: "Volume".into(),
          value: Some(A11yValue { now: 0.5, min: 0.0, max: 1.0, step: Some(0.1), jump: None, text: None }),
          ..A11yNodeView::default()
      }];
      let tree = buiy_verify::a11y::consume(&views, None);
      let node = tree.state().node_by_id(node_id_for(entity(1))).unwrap();
      assert_eq!(node.numeric_value(), Some(0.5));
      assert_eq!(node.min_numeric_value(), Some(0.0));
      assert_eq!(node.max_numeric_value(), Some(1.0));
  }
  ```
  Run: `cargo test -p buiy_verify --test verify_headless a11y::consumer_reads_slider_numeric_value` → **RED** (compile: no `A11yValue`, no `value` field).
- [ ] **GREEN A — the component.** In the new `states.rs`:
  ```rust
  #[derive(Component, Reflect, FromReflect, Default, Clone, Debug, PartialEq)]
  #[reflect(Component)]
  pub struct A11yValue {
      pub now: f64,
      pub min: f64,
      pub max: f64,
      pub step: Option<f64>,
      pub jump: Option<f64>,
      pub text: Option<String>,
  }
  ```
  `register_type::<A11yValue>()` in `A11yPlugin::build`; `pub mod states;` + `pub use states::*;` in `mod.rs`.
- [ ] **GREEN B — view field + query + fold arm.** Add `pub value: Option<A11yValue>` to
  `A11yNodeView`; read `Option<&A11yValue>` in `build_tree` and `.cloned()` it into the view.
  Fold arm in `to_accesskit_node` (§ 0-verified `f64` + `set_value` string setters):
  ```rust
  if let Some(v) = &view.value {
      node.set_numeric_value(v.now);
      node.set_min_numeric_value(v.min);
      node.set_max_numeric_value(v.max);
      if let Some(s) = v.step { node.set_numeric_value_step(s); }
      if let Some(j) = v.jump { node.set_numeric_value_jump(j); }
      if let Some(t) = &v.text { node.set_value(t.clone()); }
  }
  ```
- [ ] **GREEN — run, expect PASS:** `cargo test -p buiy_verify --test verify_headless a11y::consumer_reads_slider_numeric_value`.
- [ ] **Commit:** `feat(a11y): A11yValue valued-range component + numeric fold arm (P1a)`

---

## Task 2: `A11yTextValue` — the single-line text value

[semantic-tree.md §2](../specs/2026-06-18-buiy-agent-interface-design/semantic-tree.md):
`A11yTextValue(pub String)` → `set_value` (role disambiguates vs `A11yValue.text`).
Consumer getter: `node.value() -> Option<String>`.

**Files:** `states.rs` (+`A11yTextValue`), `mod.rs` (field/query/register), `translate.rs` (fold arm), test in `verify_headless/a11y.rs`.

**Steps**
- [ ] **RED — fixture.** A `TextInput` with `text_value: Some("hello".into())` → `node.value() == Some("hello")`. Run → RED (no field).
- [ ] **GREEN A:** `pub struct A11yTextValue(pub String);` (full derive set) + register.
- [ ] **GREEN B:** `pub text_value: Option<String>` on the view; query `Option<&A11yTextValue>`; fold arm `if let Some(s) = &view.text_value { node.set_value(s.clone()); }`.
  > **Order note:** the fold sets `A11yValue.text` *then* `A11yTextValue` (§5 fold order). Both call `set_value`; a node carrying both is a contract error a widget never authors (role split), but the fold's last-writer is `A11yTextValue` by §5 ordering — keep that order.
- [ ] **GREEN — PASS;** commit: `feat(a11y): A11yTextValue + set_value fold arm (P1a)`

---

## Task 3: `A11yPlaceholder`

`A11yPlaceholder(pub String)` → `set_placeholder` (`impl Into<Box<str>>`). Consumer getter `node.placeholder() -> Option<&str>`.

**Files:** `states.rs`, `mod.rs`, `translate.rs`, test.

**Steps**
- [ ] **RED:** `placeholder: Some("Search…".into())` → `node.placeholder() == Some("Search…")`. RED (no field).
- [ ] **GREEN A:** `pub struct A11yPlaceholder(pub String);` + register.
- [ ] **GREEN B:** `pub placeholder: Option<String>`; query; fold `if let Some(p) = &view.placeholder { node.set_placeholder(p.clone()); }`.
- [ ] **PASS;** commit: `feat(a11y): A11yPlaceholder + set_placeholder fold arm (P1a)`

---

## Task 4: Widen `A11yNodeView` + `Default` + the `build_tree` query skeleton + FIRST snapshot re-bless (the shared spine)

**This is the load-bearing structural task** — it widens `A11yNodeView` from 5 flat
fields to the full winit-free snapshot, derives `Default` (so component tasks can use
`..A11yNodeView::default()`), widens the `build_tree` query once, and **re-blesses the
existing a11y snapshots** since the struct grew. After this lands, Tasks 1–3 and 5–12
only *add a field + a query term + a fold arm*. **P1a owns this re-bless** (co-drive §3.1).

**Files**
- Modify: `crates/buiy_core/src/a11y/mod.rs` (`A11yNodeView` widening + `Default` + `build_tree` query/populate), `crates/buiy_verify/src/a11y.rs` (the `consume` helper — co-drive single-home)
- Re-bless: any existing a11y snapshot/golden that captured the struct (the `verify_headless/a11y.rs` JSON assertions are field-targeted, so they likely survive; confirm — see step)

**Steps**
- [ ] **Widen `A11yNodeView`** ([semantic-tree.md §7](../specs/2026-06-18-buiy-agent-interface-design/semantic-tree.md)
  snapshot shape) to the full set. Concrete field proposal (impl-time names; `parent`/`children`
  land in **P1b**, not here — only the **state + relations + SC-4 scroll** fields are P1a):
  ```rust
  #[derive(Clone, Debug, PartialEq, Default)]
  pub struct A11yNodeView {
      // Existing (P0):
      pub entity: Entity,            // Default = Entity::PLACEHOLDER (confirm Default impl)
      pub role: A11yRole,
      pub name: String,
      pub description: String,
      pub focusable: bool,
      // State Options (Tasks 1–12; absent = not-applicable):
      pub toggled: Option<Toggled>,          // Task 5  (accesskit::Toggled)
      pub expanded: Option<bool>,            // Task 6
      pub selected: Option<bool>,            // Task 7
      pub disabled: bool,                    // Task 8  (marker → bool flag in the view)
      pub modal: bool,                       // Task 9  (marker)
      pub hidden: bool,                      // Task 10 (marker; P1a sets the flag, P1b does the PRUNE)
      pub live: Option<A11yLive>,            // Task 11 (politeness + atomic)
      pub orientation: Option<Orientation>,  // Task 12 (accesskit::Orientation)
      pub has_popup: Option<HasPopup>,       // Task 12-b? — folded into Task 12 group (accesskit::HasPopup)
      pub value: Option<A11yValue>,          // Task 1
      pub text_value: Option<String>,        // Task 2
      pub placeholder: Option<String>,       // Task 3
      // Relations resolved to NodeId at build time (Entity never leaks — §3):
      pub labelled_by: Vec<NodeId>,          // Task 13 (wired)
      pub described_by: Vec<NodeId>,         // Task 13 (wired)
      pub controls: Vec<NodeId>,             // Task 13 (wired)
      pub active_descendant: Option<NodeId>, // Task 13 (wired)
      // SC-4 scroll wire fields (Task 14; default/None now, C5 populates in Wave 4):
      pub scroll: Option<A11yScrollView>,
  }
  ```
  > **`Toggled`/`Live`/`Orientation`/`HasPopup` are `accesskit` types** — import them in
  > `mod.rs` (`use accesskit::{Toggled, Live, Orientation, HasPopup};`). This keeps the
  > view's enum fields one-to-one with the setters, so the fold arm is trivial. (The
  > component wrappers `A11yToggled`/`A11yLive`/`A11yOrientation`/`A11yHasPopup` in
  > `states.rs` newtype these — see the per-task shapes. The view stores the *projected*
  > accesskit enum, the component stores the *authored* wrapper; `build_tree` projects.)
  > `Entity`'s `Default` is `Entity::PLACEHOLDER` in Bevy 0.19 — confirm; if not, give
  > `A11yNodeView` a manual `Default` setting `entity: Entity::PLACEHOLDER`.
- [ ] **Derive `Default`** (above). Confirm `Entity: Default`; if absent, hand-write `Default`.
- [ ] **Widen the `build_tree` query tuple ONCE** to read every `Option<&A11yX>` the
  component tasks need. Add the terms incrementally per task, but establish the
  `#[allow(clippy::type_complexity)]` + the populate skeleton here. Relations resolution
  (`node_id_for` at build time) is wired in Task 13; scroll in Task 14.
  > **Risk (phasing.md §Risks #1): wide query arity.** ~16 `Option<&_>` terms approach
  > Bevy's tuple-arity ceiling. If the tuple won't compile, extract a `#[derive(QueryData)]`
  > struct (`A11yQuery`) — named in the spec as the mitigation. Prefer the `QueryData`
  > struct from the start if the term count is already near the limit on this Bevy.
- [ ] **Write the `consume` helper** in `buiy_verify::a11y` (co-drive single-home):
  ```rust
  /// Build the real TreeUpdate via the isolated fold and wrap it in an
  /// in-process accesskit_consumer::Tree — the gate-#3 read tier shared by
  /// P1a fixtures, C7, and P1c (co-drive §1, no fork).
  pub fn consume(views: &[A11yNodeView], focused: Option<accesskit::NodeId>) -> accesskit_consumer::Tree {
      let update = buiy_core::a11y::translate::build_tree_update(views, focused);
      accesskit_consumer::Tree::new(update, true)
  }
  ```
  Confirm the exact `Tree::new` signature (`new(initial_state: TreeUpdate, is_host_focused: bool)`, `tree.rs:602`) and the `state().node_by_id` path against 0.36.
- [ ] **Re-bless the a11y snapshots.** Run the a11y suites; the field-targeted JSON
  assertions in `verify_headless/a11y.rs` (`json.contains("\"role\":\"Button\"")`) are
  unaffected by new struct fields **unless** the serializer's `WireNode` widened — it did
  **not** in P1a (the SC-4 scroll fields ride `A11yNodeView`, but `WireNode` is the
  *serialized* subset; widening it is a separate decision). Confirm zero golden churn;
  if any snapshot captured the full struct via `Debug`, re-bless it and note it.
  ```sh
  cargo test -p buiy_core --test crosscut a11y
  cargo test -p buiy_verify --test verify_headless a11y
  ```
  Expected: green; record any re-bless. **If a golden churns, P1a owns the re-bless** —
  do it here, in this commit, with a one-line note in the commit body.
- [ ] **Land the Phase-0 `consumer_reads_back_a_button_label` fixture here** (it depends on
  `consume` + `Default`). Run → PASS.
- [ ] **Commit:** `feat(a11y): widen A11yNodeView to the decomposed snapshot + consumer gate-#3 tier (P1a)`

---

## Task 5: `A11yToggled` (tri-state) — `set_toggled`

[semantic-tree.md §2](../specs/2026-06-18-buiy-agent-interface-design/semantic-tree.md):
`A11yToggled(pub Toggled) {False, True, Mixed}` → `set_toggled`. `Mixed` is **never
collapsed**. Consumer getter `node.toggled() -> Option<Toggled>`.

**Steps**
- [ ] **RED:** a Checkbox with `toggled: Some(Toggled::Mixed)` → `node.toggled() == Some(Toggled::Mixed)`. Also a `Toggled::True` case. RED (no field).
- [ ] **GREEN A:** `pub struct A11yToggled(pub accesskit::Toggled);` — implement `Default` as `Toggled::False` (the wrapper isn't `Default`-derivable since `Toggled` may not be `Default`; confirm — if `Toggled: Default` is absent, `#[derive(Default)]` fails, so hand-write `impl Default`). Register.
- [ ] **GREEN B:** view field `toggled: Option<Toggled>`; `build_tree` projects `Option<&A11yToggled>` → `.map(|t| t.0)`; fold arm `if let Some(t) = view.toggled { node.set_toggled(t); }`.
- [ ] **PASS;** commit: `feat(a11y): A11yToggled tri-state + set_toggled fold arm (P1a)`

---

## Task 6: `A11yExpanded` — `set_expanded(bool)`

`A11yExpanded(pub bool)` → `set_expanded(b)`; absence ⇒ `clear_expanded` (i.e. omit the
arm). **Producer-tier assertion** (consumer 0.36 has no `is_expanded()` getter — § 0 gap;
re-confirm in Phase 0). Assert via `to_accesskit_node(&view).is_expanded()`.

**Steps**
- [ ] **RED (producer tier, in `crosscut/a11y_translate.rs`):**
  ```rust
  #[test]
  fn expanded_view_sets_expanded() {
      let view = A11yNodeView { role: A11yRole::Button, expanded: Some(true), ..Default::default() };
      assert!(to_accesskit_node(&view).is_expanded());
      let collapsed = A11yNodeView { expanded: Some(false), ..Default::default() };
      assert!(!to_accesskit_node(&collapsed).is_expanded());
  }
  ```
  RED (no `expanded` field). *(If Phase 0 finds a consumer `is_expanded()` in 0.36, write the consumer-tier fixture instead, for uniformity.)*
- [ ] **GREEN A:** `pub struct A11yExpanded(pub bool);` + register.
- [ ] **GREEN B:** view `expanded: Option<bool>`; query `Option<&A11yExpanded>` → `.map(|e| e.0)`; fold `if let Some(b) = view.expanded { node.set_expanded(b); }`.
- [ ] **PASS;** commit: `feat(a11y): A11yExpanded + set_expanded fold arm (P1a)`

---

## Task 7: `A11ySelected` — `set_selected(bool)`

`A11ySelected(pub bool)` → `set_selected(b)`; absence ⇒ `clear_selected`. Consumer getter
`node.is_selected() -> Option<bool>`.

**Steps**
- [ ] **RED:** `selected: Some(true)` → `node.is_selected() == Some(true)`. RED.
- [ ] **GREEN A:** `pub struct A11ySelected(pub bool);` + register.
- [ ] **GREEN B:** view `selected: Option<bool>`; query; fold `if let Some(b) = view.selected { node.set_selected(b); }`.
- [ ] **PASS;** commit: `feat(a11y): A11ySelected + set_selected fold arm (P1a)`

---

## Task 8: `A11yDisabled` (marker) — `set_disabled()`

Marker component (no fields) → `set_disabled()` (**no argument**, `flag_methods!`).
Consumer getter `node.is_disabled() -> bool`.

**Steps**
- [ ] **RED:** view `disabled: true` → `node.is_disabled() == true`; a `disabled: false` node → `false`. RED.
- [ ] **GREEN A:** `#[derive(Component, Reflect, FromReflect, Default, Clone, Debug, PartialEq)] #[reflect(Component)] pub struct A11yDisabled;` + register.
- [ ] **GREEN B:** view `disabled: bool`; `build_tree` reads `Option<&A11yDisabled>` → `.is_some()`; fold `if view.disabled { node.set_disabled(); }`.
- [ ] **PASS;** commit: `feat(a11y): A11yDisabled marker + set_disabled fold arm (P1a)`

---

## Task 9: `A11yModal` (marker) — `set_modal()`

Marker → `set_modal()`. Consumer getter `node.is_modal() -> bool`. (S4 dialog populates it.)

**Steps**
- [ ] **RED:** `modal: true` → `node.is_modal() == true`. RED.
- [ ] **GREEN A:** `pub struct A11yModal;` (marker derive set) + register.
- [ ] **GREEN B:** view `modal: bool`; query `Option<&A11yModal>` → `.is_some()`; fold `if view.modal { node.set_modal(); }`.
- [ ] **PASS;** commit: `feat(a11y): A11yModal marker + set_modal fold arm (P1a)`

---

## Task 10: `A11yHidden` (marker) — set the flag NOW; the PRUNE is P1b

[semantic-tree.md §2/§7.4](../specs/2026-06-18-buiy-agent-interface-design/semantic-tree.md):
`A11yHidden` is **NOT a node flag in the final design** — it **prunes** the entity+subtree
(P1b §7.4). **In P1a there is no nesting yet**, so P1a carries the marker + the view flag
**only**; it emits the accesskit `set_hidden()` flag as a stopgap so the component is
testable, and **P1b replaces that with the prune**.

> **Deliberate scope seam (state it):** P1a sets `node.set_hidden()` for `A11yHidden`
> nodes; P1b (nesting) **removes that fold arm** and instead excludes the entity+subtree
> from `build_tree`. This is a known two-step — recorded so P1b knows to retire the arm,
> not duplicate it. The marker + view flag are P1a's deliverable; the *semantics* finalize
> in P1b. (If the implementer prefers, carry the marker + view flag with **no fold arm**
> in P1a and let P1b add the prune — that is cleaner but leaves `A11yHidden` unobservable
> at gate #3 until P1b. The plan picks the stopgap-flag so P1a has a green fixture; flag
> this choice for the P1b author.)

**Steps**
- [ ] **RED:** `hidden: true` → `to_accesskit_node(&view).is_hidden()` (producer tier; consumer prunes hidden nodes from its filtered tree, so assert producer-side). RED.
- [ ] **GREEN A:** `pub struct A11yHidden;` + register.
- [ ] **GREEN B:** view `hidden: bool`; query `Option<&A11yHidden>` → `.is_some()`; fold arm `if view.hidden { node.set_hidden(); } // STOPGAP — P1b replaces with the §7.4 prune`.
- [ ] **PASS;** commit: `feat(a11y): A11yHidden marker (P1a flag stopgap; P1b prunes) (P1a)`

---

## Task 11: `A11yLive` — `set_live` + `set_live_atomic` + role-implied derivation (`resolve_live`)

The must-fix arm. [semantic-tree.md §2/§5](../specs/2026-06-18-buiy-agent-interface-design/semantic-tree.md):
`A11yLive { politeness: Live, atomic: bool }` → `set_live(politeness)` **and**
`set_live_atomic()` (the marker — **NOT `set_atomic`, which doesn't exist** in 0.24).
**Plus** the role-implied derivation: `resolve_live(role, explicit)` derives
politeness/atomic from the role when no explicit `A11yLive`, then the explicit component
overrides — `Role::Alert ⇒ Assertive+atomic`, `Role::Status ⇒ Polite+atomic`,
`Role::Log ⇒ Polite`. Without this, gate #4 is wrong for an alert with no author `A11yLive`.

> **A11yRole coverage caveat:** the P0 `A11yRole` enum does **not** include `Alert`/
> `Status`/`Log` variants (it stops at `Group`). So `resolve_live` in P1a can only key off
> roles that exist. **Two readings — pick at impl-time and record:** (a) add the
> `Alert`/`Status`/`Log` `A11yRole` variants **here** (they have AccessKit equivalents and
> the live-region grounding needs them) — this is a small, in-scope extension since
> `resolve_live` is meaningless without them; or (b) write `resolve_live` keyed on the
> existing roles (returning `(None, false)` for all current ones, i.e. the explicit
> component is the only source) and defer the role-implied arms with the role variants to
> the phase that adds them. **Recommendation: (a)** — add `Alert`/`Status`/`Log` to
> `A11yRole` + both stringifiers (the P0 KNOWN_ROLES forcing function) in this task, since
> the spec's `resolve_live` is explicitly role-keyed and the live-region loop (co-drive
> grounding) needs a role to imply from. Flag this as a scope note in the PR.

**Files:** `states.rs` (+`A11yLive`), `mod.rs` (field/query/register; possibly +3 `A11yRole` variants), `translate.rs` (the `resolve_live` fn + the fold arm; +3 `role_to_accesskit` arms), `buiy_verify/src/a11y.rs` (+3 `role_to_str` arms + `KNOWN_ROLES`).

**Steps**
- [ ] **RED — two fixtures.** (1) Explicit: `live: Some(A11yLive { politeness: Live::Assertive, atomic: true })` → `node.live() == Live::Assertive` AND `node.is_live_atomic() == true`. (2) Role-implied (if reading (a)): a node `role: A11yRole::Status` with `live: None` → `node.live() == Live::Polite` AND atomic. RED.
- [ ] **GREEN A:** `pub struct A11yLive { pub politeness: Live, pub atomic: bool }` — `Live` is `accesskit::Live`; hand-write `Default` if `Live: Default` is absent (it defaults to `Polite` per the enum-property macro default). Register. (Optionally add `Alert`/`Status`/`Log` to `A11yRole` + both stringifiers per reading (a).)
- [ ] **GREEN B — `resolve_live` + fold arm.** Add to `translate.rs`:
  ```rust
  /// Role-implied live politeness/atomic, overridden by an explicit A11yLive.
  /// (semantic-tree.md §5; wai-aria-apg/live-regions.md)
  fn resolve_live(role: A11yRole, explicit: Option<A11yLive-projection>) -> (Option<Live>, bool) {
      if let Some(l) = explicit { return (Some(l.politeness), l.atomic); }
      match role {
          A11yRole::Alert  => (Some(Live::Assertive), true),
          A11yRole::Status => (Some(Live::Polite), true),
          A11yRole::Log    => (Some(Live::Polite), false),
          _ => (None, false),
      }
  }
  ```
  Fold arm (against § 0-verified `set_live(Live)` + the `set_live_atomic()` **marker**):
  ```rust
  let (politeness, atomic) = resolve_live(view.role, view.live);
  if let Some(p) = politeness {
      node.set_live(p);
      if atomic { node.set_live_atomic(); }   // marker, NO argument — NOT set_atomic
  }
  ```
  > **§ 0 trap restated:** `set_live_atomic` takes **no argument** (it's a `flag_methods!`
  > marker, `lib.rs:1806`). `node.set_live_atomic(atomic)` would not compile. The boolean
  > gates *whether to call it*, it is not an argument.
- [ ] **PASS;** commit: `feat(a11y): A11yLive + resolve_live role-implied derivation + set_live/set_live_atomic (P1a)`

---

## Task 12: `A11yOrientation` + `A11yHasPopup` — the two enum-property markers

Group these two (both `unique_enum_property_methods!`, both single-field enum wrappers).
[semantic-tree.md §2](../specs/2026-06-18-buiy-agent-interface-design/semantic-tree.md):
`A11yOrientation` → `set_orientation(Orientation)`; `A11yHasPopup` → `set_has_popup(HasPopup)`.
`A11yHasPopup` is BUILD (co-drive §3.1 Q-D RESOLVED — S3's MenuButton + the gallery
screen-switcher populate it). Consumer getters `node.orientation() -> Option<Orientation>`,
`node.has_popup() -> Option<HasPopup>`.

**Steps**
- [ ] **RED — two fixtures.** `orientation: Some(Orientation::Horizontal)` → `node.orientation() == Some(Orientation::Horizontal)`; `has_popup: Some(HasPopup::Menu)` → `node.has_popup() == Some(HasPopup::Menu)`. RED.
- [ ] **GREEN A:** `pub struct A11yOrientation(pub accesskit::Orientation);` + `pub struct A11yHasPopup(pub accesskit::HasPopup);` (hand-write `Default` if the inner enums aren't `Default` — `Orientation`/`HasPopup` enum-property defaults are `Vertical`/`Menu`). Register both.
- [ ] **GREEN B:** view `orientation: Option<Orientation>` + `has_popup: Option<HasPopup>`; query both; fold:
  ```rust
  if let Some(o) = view.orientation { node.set_orientation(o); }
  if let Some(h) = view.has_popup   { node.set_has_popup(h); }
  ```
- [ ] **PASS;** commit: `feat(a11y): A11yOrientation + A11yHasPopup + enum fold arms (P1a)`

---

## Task 13: `A11yRelations` — 8 fields carried, only 4 wired

[semantic-tree.md §3](../specs/2026-06-18-buiy-agent-interface-design/semantic-tree.md):
the struct carries **all 8** `Entity`-ref fields (`Reflect` is cheap, BSN-patchable
per-field), but **only 4 are wired** in P1a: `labelled_by`, `described_by`, `controls`,
`active_descendant`. The other 4 (`owns`, `flow_to`, `details`, `error_message`) are
**carried-but-unwired** — **deliberately deferred** (co-drive §3.2: no gallery consumer;
`owns` re-parent only matters for a portalled dialog, S4 is in-place). They get **no
`build_tree` resolution and no fold arm** in P1a. Storage is `Entity`; resolution to
`NodeId` happens at **translate/build time** via `node_id_for`, so `Entity` never leaks
past the seam (§3).

**Files:** `relations.rs` (NEW — the full 8-field struct), `mod.rs` (view fields for the 4 wired + the `build_tree` resolution + register), `translate.rs` (4 fold arms).

**Steps**
- [ ] **RED — fixture (consumer tier where possible).** Build two nodes A (entity 1) and B
  (entity 2) where A `labelled_by: vec![B-entity]`; assert the consumer resolves the
  relation — `node_a.labelled_by()` yields the node for B (consumer getter
  `labelled_by(...)` returns an iterator of `Node`; assert it contains B's `NodeId`). Also
  assert `active_descendant` round-trips (`node.active_descendant() -> Option<Node>`). RED
  (no relation fields / no struct).
  > **Resolution timing:** the view stores **`Vec<NodeId>`** (already resolved), so the
  > fixture builds the `A11yNodeView` with `labelled_by: vec![node_id_for(entity(2))]`
  > directly — `build_tree`'s job (resolving `Entity`→`NodeId`) is exercised by the
  > integration `build_tree` test, not this translate-level fixture. Add a `build_tree`
  > integration test (in `crosscut/a11y_translate.rs` or a new `crosscut/a11y_build.rs`)
  > that spawns two entities with `A11yRelations { labelled_by: vec![b], .. }` and asserts
  > the built view's `labelled_by == vec![node_id_for(b)]`.
- [ ] **GREEN A — the full struct** ([semantic-tree.md §3](../specs/2026-06-18-buiy-agent-interface-design/semantic-tree.md) verbatim):
  ```rust
  #[derive(Component, Reflect, FromReflect, Default, Clone, Debug, PartialEq)]
  #[reflect(Component)]
  pub struct A11yRelations {
      pub labelled_by: Vec<Entity>,      // WIRED
      pub described_by: Vec<Entity>,     // WIRED
      pub controls: Vec<Entity>,         // WIRED
      pub owns: Vec<Entity>,             // carried, UNWIRED (deferred — co-drive §3.2)
      pub flow_to: Vec<Entity>,          // carried, UNWIRED
      pub details: Vec<Entity>,          // carried, UNWIRED
      pub active_descendant: Option<Entity>, // WIRED
      pub error_message: Option<Entity>, // carried, UNWIRED
  }
  ```
  Register. **Doc-comment the 4 unwired fields** as "carried for BSN-patchability +
  forward-compat; no populate-side resolution or fold arm until un-deferred (co-drive §3.2)."
- [ ] **GREEN B — resolve the 4 wired in `build_tree` + 4 fold arms.** In `build_tree`,
  read `Option<&A11yRelations>` and resolve the 4 wired refs to `NodeId` via `node_id_for`
  into the view's `labelled_by`/`described_by`/`controls`/`active_descendant`. Fold arms
  (§ 0-verified: vec setters take `impl Into<Vec<NodeId>>`, `set_active_descendant` takes
  `NodeId`):
  ```rust
  if !view.labelled_by.is_empty()  { node.set_labelled_by(view.labelled_by.clone()); }
  if !view.described_by.is_empty() { node.set_described_by(view.described_by.clone()); }
  if !view.controls.is_empty()     { node.set_controls(view.controls.clone()); }
  if let Some(id) = view.active_descendant { node.set_active_descendant(id); }
  // owns / flow_to / details / error_message: deferred — no arm (co-drive §3.2).
  ```
- [ ] **PASS** (both the translate fixture and the `build_tree` resolution test); commit:
  `feat(a11y): A11yRelations (8 carried, 4 wired) + relation fold arms (P1a)`

---

## Task 14: SC-4 — the scroll wire fields on `A11yNodeView` (schema only; C5 populates)

[co-drive §5 SC-4](2026-06-22-widget-catalog-agent-interface-codrive.md): the **single
coordinated wire-format change** adding the scroll fields to `A11yNodeView` — scroll
offset + content/viewport extent + a scrollable flag. **P1a lands the schema (default/None);
C5 (Wave 4) populates it on scroll containers.** The exact field set is finalized **with
this plan** from
[scroll-overlay-modal.md §Coordination](../specs/2026-06-22-buiy-widget-catalog-design/README.md)
**before** the widening lands — so C5 populates a schema that already exists. C5 adds **no**
competing scroll component to the view.

**Field proposal (concrete names/types — finalize against scroll-overlay-modal.md §Coordination):**
```rust
/// Scroll geometry for a scroll container, exposed to AT. P1a lands this as the
/// A11yNodeView schema (SC-4, the single coordinated wire-format change); C5
/// (Wave 4) populates it on scroll containers. AccessKit exposure rides the
/// f64 scroll setters (set_scroll_x / set_scroll_x_min / set_scroll_x_max /
/// set_scroll_y / set_scroll_y_min / set_scroll_y_max — verified in 0.24.1).
#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct A11yScrollView {
    pub offset: Vec2,           // current scroll offset (logical px)
    pub content_extent: Vec2,   // scrollable content size
    pub viewport_extent: Vec2,  // visible viewport size
    pub scrollable: bool,       // true iff content_extent > viewport_extent on either axis
}
```
On `A11yNodeView`: `pub scroll: Option<A11yScrollView>` (None = not a scroll container).

> **AccessKit mapping (the fold arm — lands behaviorally in C5, schema here):** the spec's
> `A11yNodeView` carries the geometry; the **fold maps it to the 0.24 f64 scroll setters**
> — `scroll_x = offset.x`, `scroll_x_min = 0.0`, `scroll_x_max = content_extent.x −
> viewport_extent.x` (clamped ≥ 0), same for y. **P1a's choice:** land the **field +ONE
> fold arm now** (so the schema is exercised end-to-end and the setter signatures are
> confirmed in the isolated fold per § 0.2), populated by `None` everywhere until C5. This
> keeps SC-4 a *true* coordinated wire change — both the view field AND its single emission
> point exist; C5 only flips `None`→`Some` on containers. (Runner-up: land the field with
> **no** fold arm and let C5 add both. Rejected: it splits the SC-4 wire change across two
> waves and re-opens the setter-signature question in C5, violating "fold is the single
> emission point, confirmed once.")

**Steps**
- [ ] **RED — fixture.** A node with `scroll: Some(A11yScrollView { offset: vec2(0.0, 40.0), content_extent: vec2(100.0, 300.0), viewport_extent: vec2(100.0, 100.0), scrollable: true })` → consumer/producer `node.scroll_y() == Some(40.0)`, `node.scroll_y_max() == Some(200.0)`. RED (no field, no arm).
  > Confirm `accesskit_consumer` exposes `scroll_y()` getters; if not, assert at the producer tier (`to_accesskit_node(&view).scroll_y()` — the producer `Node` has the f64 getters via `f64_property_methods!`).
- [ ] **GREEN A:** add `A11yScrollView` + the `scroll` field to `A11yNodeView` (already
  stubbed in Task 4's struct; this task fills in the type + the arm). No component needed
  in P1a — `A11yScrollView` is a *view-only projection*; the **C5 scroll component**
  (widget-catalog) populates `build_tree`'s read later. P1a's `build_tree` sets
  `scroll: None` for now (no scroll component exists to read yet).
- [ ] **GREEN B — the single fold arm:**
  ```rust
  if let Some(s) = &view.scroll {
      node.set_scroll_x(s.offset.x);
      node.set_scroll_x_min(0.0);
      node.set_scroll_x_max((s.content_extent.x - s.viewport_extent.x).max(0.0));
      node.set_scroll_y(s.offset.y);
      node.set_scroll_y_min(0.0);
      node.set_scroll_y_max((s.content_extent.y - s.viewport_extent.y).max(0.0));
  }
  ```
- [ ] **Re-bless any widened snapshot** (P1a owns it). Confirm the `WireNode` serializer is
  unchanged (scroll is not in the snapshot wire format unless C7 asks for it later).
- [ ] **PASS;** commit: `feat(a11y): SC-4 scroll wire fields on A11yNodeView + scroll fold arm (schema; C5 populates) (P1a)`

---

## Task 15: ACCNAME — `compute_accessible_name` (a function, not a component)

[semantic-tree.md §6](../specs/2026-06-18-buiy-agent-interface-design/semantic-tree.md):
ACCNAME 1.2 (`labelledby > label > host > content > title`, hidden-subtree exclusion) as
a pure `compute_accessible_name(...) -> String` in `accname.rs`, feeding `A11yLabel`'s
`set_label` string. **Derived every build, never stored.** A function, NOT a component.

> **Scope honesty (impl-time decision — record it).** Full ACCNAME 1.2 with the `labelledby`
> walk + hidden-subtree exclusion needs the **nesting/`labelled_by`-resolution that lands in
> P1b**. In P1a (no nesting yet) the realizable subset is the **precedence among
> locally-available sources**: explicit `A11yLabel` (host) > content fallback. The
> `labelledby` arm (highest precedence) resolves references that only become tree-walkable
> in P1b. **Two readings — pick one and flag it:** (a) land `compute_accessible_name` in
> P1a as the **function skeleton with the precedence order + the locally-resolvable arms
> (label/host/content)**, with the `labelledby` arm stubbed to "P1b wires the walk" (a
> documented TODO with a RED-ignored fixture C7/P1b un-ignores); or (b) defer the whole
> function to P1b where the tree walk exists. **Recommendation: (a)** — the function's
> *home* + precedence skeleton + the local arms are genuinely P1a (the spec lists accname
> under Phase 1a), and the `labelledby` walk is the one arm that legitimately needs P1b.
> This keeps the ACCNAME *contract* in P1a and lets P1b fill the one tree-dependent arm.

**Files:** `accname.rs` (NEW), `mod.rs` (`pub mod accname;` + wire into `build_tree`'s `name` derivation — `compute_accessible_name` replaces the raw `A11yLabel.0` read), test in `crosscut/a11y_translate.rs` or a new `crosscut/a11y_accname.rs`.

**Steps**
- [ ] **RED — fixture(s) for the local arms.** (1) host precedence: an entity with
  `A11yLabel("Save")` and content "ignored" → `compute_accessible_name(...) == "Save"`.
  (2) content fallback: no `A11yLabel`, content "Click me" → `"Click me"`. Write a
  **`#[ignore = "P1b wires the labelledby walk"]`** fixture for the `labelledby` arm so
  the deferral is visible and un-ignored by P1b. RED on the local arms (no fn).
- [ ] **GREEN — the function + precedence skeleton.** In `accname.rs`:
  ```rust
  /// ACCNAME 1.2 accessible-name computation (semantic-tree.md §6,
  /// wai-aria-apg/name-computation.md). Pure; derived every build, never stored.
  /// Precedence: labelledby > label(host) > content > title.
  /// P1a realizes label/host/content; the labelledby WALK lands in P1b (needs nesting).
  pub fn compute_accessible_name(/* the locally-available inputs: label, content, … */) -> String {
      // 1. labelledby — P1b (tree walk). 2. explicit label. 3. content. 4. title.
      // ...precedence among the locally-available sources...
  }
  ```
  Wire it into `build_tree`'s `name` field (replace the raw `label.map(|l| l.0.clone())`).
- [ ] **PASS** (local arms green; `labelledby` arm `#[ignore]`); commit:
  `feat(a11y): compute_accessible_name ACCNAME function (local arms; P1b wires labelledby) (P1a)`

---

## Task 16: Wave-close — full a11y fold audit + gate-#3 sweep + the full project gate

Confirm the fold is the single emission point, every BUILD component has a gate-#3
fixture, the DEFER set has none, and the workspace gate is green.

**Files:** verify-only (no edits beyond doc-note fixes).

**Steps**
- [ ] **Fold-isolation audit (standing rule § 0.2).** Grep that **no `set_*` accesskit
  setter is called outside `translate.rs`**:
  ```sh
  grep -rn "\.set_\(toggled\|expanded\|selected\|disabled\|modal\|hidden\|live\|live_atomic\|orientation\|has_popup\|value\|placeholder\|numeric_value\|min_numeric_value\|max_numeric_value\|numeric_value_step\|numeric_value_jump\|labelled_by\|described_by\|controls\|active_descendant\|scroll_x\|scroll_y\)" crates/buiy_core/src crates/buiy_widgets/src
  ```
  Expected: matches **only** in `crates/buiy_core/src/a11y/translate.rs`. Any other hit is a leak — fix it (move the call into the fold).
- [ ] **BUILD-set coverage.** Confirm a gate-#3 (or producer-tier) fixture exists for each
  of the **12** components + the 4 wired relations + the SC-4 scroll arm + ACCNAME local
  arms. Confirm the **DEFER** components (`A11yReadOnly`/`Required`/`Busy`/`Invalid`/
  `AutoComplete`/`Level`/`PosInSet`/`SetSize`) and the **4 unwired relation fields** have
  **no fold arm and no fixture** (they don't exist as components in P1a — only the
  `A11yRelations` struct carries the 4 unwired *fields*).
- [ ] **Setter-signature final sweep.** Re-read every fold arm against the § 0 table one
  last time (the standing discipline). Confirm `set_live_atomic()` is called with **no
  argument**, markers (`set_disabled`/`set_modal`/`set_hidden`) with **no argument**,
  enum setters with the accesskit enum, vec setters with `Vec<NodeId>`.
- [ ] **`cargo deny check`** once more (clean).
- [ ] **Full project gate** (CLAUDE.md Build & Test; Linux host → `xvfb-run -a`; add `-j 2`
  if the test link OOMs):
  ```sh
  cargo fmt --all -- --check && \
    cargo clippy --workspace --all-targets -- -D warnings && \
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
    xvfb-run -a cargo test --workspace -j 2
  ```
  Expected: all green. The GPU `--ignored` lane is **not** required for P1a (it touches no
  GPU path); the headless gate must stay green (CI has no adapter).
- [ ] **Update the deferral ledger (co-drive §3.2 + phasing.md).** Mark P1a's BUILD set as
  **landed** in both
  [`2026-06-22-widget-catalog-agent-interface-codrive.md` §3.1](2026-06-22-widget-catalog-agent-interface-codrive.md)
  and
  [`phasing.md`](../specs/2026-06-18-buiy-agent-interface-design/phasing.md) (the mirrored
  ledger). Note any BUILD/DEFER item that moved (e.g. if reading (a) added `Alert`/`Status`/
  `Log` roles, or if ACCNAME's `labelledby` arm was deferred to P1b). "Docs updated" is part
  of done (CLAUDE.md).
- [ ] **Update `docs/README.md`** if P1a's landing changes the catalog status line for the
  agent-interface campaign (per `organizing-buiy-docs`).
- [ ] **Commit:** `chore(a11y): P1a wave-close — fold audit, gate-#3 sweep, ledger update (P1a)`

---

## Out of scope for P1a (deferred — see ledger)

These are **deferred, not cancelled** — no gallery consumer (co-drive
[§3.2](2026-06-22-widget-catalog-agent-interface-codrive.md) /
[phasing.md](../specs/2026-06-18-buiy-agent-interface-design/phasing.md), authoritative).
P1a builds **none** of them: no component, no fold arm, no fixture.

- **State components (8 deferred):** `A11yReadOnly`, `A11yRequired`, `A11yBusy`,
  `A11yInvalid`, `A11yAutoComplete`, `A11yLevel`, `A11yPosInSet`, `A11ySetSize` — no
  gallery screen reads or populates them. (The setters exist in 0.24 —
  `set_read_only`/`set_required`/`set_busy`/`set_invalid`/`set_auto_complete`/`set_level`/
  `set_position_in_set`/`set_size_of_set` — but P1a emits none.)
- **Relation fields (4 carried-but-unwired):** `owns`, `flow_to`, `details`,
  `error_message` — the `A11yRelations` **struct carries all 8** (Task 13), but these 4 get
  **no `build_tree` resolution and no fold arm** (co-drive §3.2: `owns` re-parent only
  matters for a portalled dialog; S4 is in-place).
- **Nesting (all of P1b):** `build_tree` reading `ChildOf`/`Children`,
  `nearest_a11y_ancestor` collapse, the window-entity root, the **`A11yHidden` prune**
  (P1a sets the flag as a stopgap — Task 10 — P1b replaces it), the `owns` re-parent edge,
  `TreeView::Merged`/`A11yMergeChildren`, the gate-#12 invariants. The widened
  `A11yNodeView` reserves `parent`/`children` fields for P1b (Task 4 does **not** add them).
- **The `labelledby` ACCNAME walk** (Task 15 reading (a) defers it to P1b — the one arm
  that needs the tree walk; the function home + local arms are P1a).
- **Inbound (all of P1c):** the `Action` router, `A11yContract`, the in-process driver,
  `EditCommand::SetSelection`, the actionability gates.
- **Widgets (all of P1d):** the 8 APG bundles.
- **Transport (all of P2):** `buiy_mcp`.

---

## Done criteria (P1a acceptance)

- [ ] `accesskit_consumer 0.36` is a declared dep; `cargo deny check` green; the gate-#3
  in-process consumer tier (`buiy_verify::a11y::consume`) exists with **one home** (no fork
  — C7/P1c share it).
- [ ] `states.rs` carries **exactly the 12 BUILD components** (`A11yToggled`, `A11yExpanded`,
  `A11ySelected`, `A11yDisabled`, `A11yValue`, `A11yTextValue`, `A11yPlaceholder`,
  `A11yModal`, `A11yHidden`, `A11yLive`, `A11yOrientation`, `A11yHasPopup`) — each with the
  full derive set, `register_type`'d, and a gate-#3 (or producer-tier) RED-first fixture.
  **None** of the 8 deferred state components exists.
- [ ] `relations.rs` carries the `A11yRelations` struct with **all 8 fields** but only the
  **4 wired** ones (`labelled_by`/`described_by`/`controls`/`active_descendant`) have a
  `build_tree` resolution + a fold arm; the other 4 are carried-but-unwired (documented).
- [ ] `accname.rs` carries `compute_accessible_name` (a function, not a component) with the
  precedence skeleton + local arms; the `labelledby` walk is the one documented P1b deferral.
- [ ] `A11yNodeView` is widened to the decomposed snapshot (state `Option`s + resolved
  relation `Vec<NodeId>`/`Option<NodeId>` + the **SC-4 scroll fields**, default/None now);
  `build_tree`'s query is widened; **P1a re-blessed any snapshot that churned**.
- [ ] `to_accesskit_node` is the rewritten **ordered 0.24-setter derive fold** — one arm per
  built component, `resolve_live` role-implied derivation included — and is the **single
  emission point** (the Task-16 grep finds setters only in `translate.rs`).
- [ ] **Standing setter-verification held:** `set_live_atomic` (not `set_atomic`), markers
  with no argument, enum setters with the accesskit enum, all confirmed against the
  committed `Cargo.lock` / resolved accesskit 0.24.1.
- [ ] The deferral ledger (co-drive §3.2 + phasing.md) + `docs/README.md` are updated;
  full workspace headless gate green.
