# Verification: picking-geometry real-input tier + content-presence guard — child C7 of the widget-catalog campaign

`2026-06-22` · `[draft]` · Wave 1 · realizes foundation `verification.md` (gates #5, #6, #11) · depends on C0; gates C1, C2, C3 (the *regression gate*, lands RED-first) · **complements** the agent-interface a11y semantic-tree tier + gates #3/#4/#6/#7/#12 (does not duplicate them) · **builds on** the incoming test infrastructure (cargo-nextest + the 162→7 consolidated harnesses, PR #77)

> **What this child is.** The campaign's **geometry/render-content** verification deliverable: the missing **picking-geometry real-input tier** (the pointer half of gate #6 "synthesized input replay" — committed on paper, never realized for *pointer*; the semantic/AT half is the agent-interface in-process driver) and a **content-presence guard** so a semantically-blank widget cannot be blessed. It is pulled into Wave 1 *deliberately ahead* of the code it verifies: Tier A is the RED-first regression gate that proves C1's coordinate-space fix and C3's input rework; Tier B is C2's content-survival gate. This spec designs **how the picking/coordinate-geometry + render-content tiers drive and assert**; it does **not** define the picking event model (C3), the coordinate fix (C1), or the text fixes (C2) — it consumes those and builds the tests that prove them.
>
> **It does NOT own the a11y wire-format.** The decomposed-a11y-state snapshot, the tri-state `WireNode`/`A11yNodeView` widen, the role serialization, the a11y gates #3/#4/#6/#7/#12, and the in-process semantic driver are the **agent-interface campaign's** (P0 + P1a + P1c). C7 *references and uses* that tier as the a11y lowest tier; C7's picking-geometry tier sits **alongside** it (it exercises the Bug-1 picking divergence the semantic driver does not reach). See [§Coordination with the agent-interface campaign + the incoming test infrastructure](#coordination-with-the-agent-interface-campaign--the-incoming-test-infrastructure).

---

## 1. Problem & current state

The foundation commits a CI-gated verification pyramid (`verification.md`), and the `2026-06-15-buiy-verification-design` harness landed five tiers of it. But gate **#6 "Synthesized input replay"** (`verification.md:22` — "Keyboard, pointer, touch, gamepad events injected as Bevy events; assert resulting state") is **on paper only for pointer**, and two adjacent gates are structurally blind. The audit (§4 "Verification blind spot"; §6.6, §6.7; Appendix-A.6; W18/W19) confirmed this against current `main`. Verified file:line evidence:

**1.1 No real-input tier — every interactive test hand-sets state, bypassing the production pipeline.**
- The closest thing on main, `crates/buiy_core/tests/picking_backend.rs:36-45`, spawns `ResolvedLayout { position: Vec2::new(10.0, 10.0), … }` **by hand** at an absolute position, bypassing the `layout → write_buiy_transform → GlobalTransform` chain. Because Bug 1 (picking reads parent-relative `layout.position` as absolute, `backend.rs:42` via `point_in_aabb` at `mod.rs:51-56`) only diverges when an ancestor is *off the origin*, a hand-written single-node `ResolvedLayout` is **structurally incapable of observing the bug** (audit Appendix-A.6, top-risk #6). It also never runs an interaction layer: no observers, no event propagation.
- The focus tests (`crates/buiy_core/tests/focus.rs`) and widget tests (`crates/buiy_widgets/tests/button.rs`) drive `ButtonInput<KeyCode>` / `MouseButton` directly, or hand-set `Hovered`/`FocusedEntity`. None drives `emit_picks` end-to-end from a synthetic pointer over a real laid-out tree.

**1.2 The picking/input code under test is the Phase-0 stopgap C3 replaces.**
- `update_hovered` (`picking/mod.rs:59`) is the single-`Hovered`-resource layer; `emit_picks` (`backend.rs:27`) reads `layout.position` as absolute (`backend.rs:42`), uses `Entity::PLACEHOLDER` as the camera (`backend.rs:65`), and ranks depth by smallest area (`backend.rs:50-53`). `InteractionPlugin` / `PointerInputPlugin` / `Pointer<E>` **do not exist on main** (`grep` for `InteractionPlugin`/`Pointer<` over `crates/buiy_core/src` + `crates/buiy_widgets/src` returns nothing). C3 introduces them; **Tier A must be designed against C3's new model and land RED-first, before that model exists**.

**1.3 Tier 2's CPU display-list builder is blind to glyph instances.**
- `extract_nodes_from_world` (`snapshot.rs:532`) calls `extracted_node_for(e, &gt, layout, bg, None, &theme)` — it passes `None` for glyphs and returns `ExtractedNodes { nodes, ..Default::default() }` (`snapshot.rs:553,567`). So Tier 2 **cannot** assert "this label emitted >0 glyph instances." A widget whose text silently failed to shape (the exact Bug-2 release-mode silent-no-paint failure mode, audit Bug 2) renders an empty box that every existing tier accepts as valid.

**1.4 `text_buffers_shaped` (W19's referenced module) does not exist on main.**
- `grep` for `text_buffers_shaped` / `shape_stale` over `crates/` returns **nothing** (verified). These were prototype artifacts on the bevy-0.18 branch. W19's instruction to "wire `text_buffers_shaped` into a retained gate" therefore **resolves to: build a content-presence invariant on the production `extract_buiy_glyphs` path**, which is the real shape-truth, rather than reviving a dead example-only module that would rot again.

**1.5 The a11y wire format serializes none of the new widget state — and that gap is the agent-interface campaign's to close, not C7's.**
- `buiy_verify::a11y::WireNode` (`a11y.rs:11-17`) is `entity / role / name / description / focusable` only — no decomposed state, no tri-state, no `skip_serializing_if` discipline; `role_to_str` (`a11y.rs:28-41`) has no extended-role arm; `A11yNodeView` (`a11y/mod.rs:57-63`) carries no state. **By the 2026-06-22 coordinate-don't-cede decision (umbrella §2.7), closing this gap is owned by the agent-interface campaign**, which decomposes the a11y state model and widens the snapshot: P0 fixes the `snapshot_tree` ref off-by-one + extends `A11yRole` and **both** stringifiers (`translate::role_to_accesskit` **and** `buiy_verify::a11y::role_to_str`); P1a widens `A11yNodeView` + `build_tree` + rewrites `to_accesskit_node` as the 0.24-setter derive fold and exposes the decomposed state through its `semantic_tree(app, view)` snapshot tier. C7 **does not** define, widen, or re-bless the a11y wire format — it *uses* that tier (the agent-interface "new lowest tier") for any a11y assertion it needs. This is a hard reversal of an earlier C7 draft that owned the `WireNode` extension; see the [Coordination](#coordination-with-the-agent-interface-campaign--the-incoming-test-infrastructure) section. The blind spot C7 *does* own is the **picking/coordinate-geometry** one (§1.1, §1.2) and the **render-content** one (§1.3, §1.6) — neither of which the semantic-tree tier observes.

**1.6 The FontsGeneration-bump path (Bugs 2/3) has no isolating test at any tier.**
- The prototype's `text_commit_font_reload.rs` auto-heals via `mark_dirty`→re-measure and passes pre-fix (audit Bug 2, §7) — it guards nothing. There is **no** content-survival test (audit Bug 3: "zero coverage today"). The enabler exists: `crates/buiy_core/tests/support/extract_harness.rs` (`TextExtractHarness`) drives the production `extract_buiy_glyphs` **adapterless** (no wgpu, manual `ExtractSchedule` over a bare render `World`) and exposes `glyph_count()` (`extract_harness.rs:154`); `text_extract.rs:80-92` already asserts "Hi!" → exactly 3 glyph instances. That is the ready substrate for both the content-presence invariant and the headless FontsGeneration-bump injection.

The net gap C7 owns: picking, focus-on-click, the post-C3 event model, overlay/modal hit-blocking are all **unverified by CI at the coordinate/geometry level** (the picking divergence the semantic-tree tier cannot reach); and a zero-glyph widget can be blessed (the render-content hole). The a11y-state snapshot home and a11y-tree-under-bulk-ops are the **agent-interface** campaign's semantic-tree tier + gates #3/#4/#7/#12 — C7 consumes that tier, it does not build a parallel one.

---

## 2. Target design

Three picking-geometry/render-content real-input tiers layered by cost/fidelity, plus the content-presence guard, all reusing **the existing and incoming** harness substrates (the consolidated cargo-nextest harnesses from PR #77 and any further test infra on `main`; C7 *extends* the runner/harness surface, it never stands up a parallel rig — see §2.0). Everything headless rides the existing workspace gate (no `--ignored`); the one winit smoke is `#[ignore]` on the GPU lane. The **a11y assertions** C7 needs ride the agent-interface **semantic-tree snapshot tier** (`buiy_verify::a11y::semantic_tree(app, view)` + gates #3/#4/#6/#7/#12) — C7 does not define an a11y tier, it consumes that one.

### 2.0 Build on the then-current test-infra surface (Phase-0 re-confirm, mandatory)

The project's test infrastructure is **actively growing on `main`** — cargo-nextest + the 162→7 test-binary consolidation (PR #77), CI-hardening (#78), the agent-interface in-process driver + `accesskit_consumer` semantic-tree tier + gates, and more. C7 **extends** that runner/harness/gate surface; it never forks it. The c7 plan's **Phase 0** is the forcing function: before any C7 code, re-fetch + re-branch from the then-current `origin/main`, confirm the consolidated nextest runner (`cargo nextest run`) is the inner runner, and re-confirm every harness/seam C7 reuses (`TextExtractHarness`, the `coverage`/`golden` machinery, the agent-interface `inprocess`/`semantic_tree` tier) is present and on its current API before depending on it. The tiers below name the substrates as of authoring; Phase 0 reconciles any drift.

### 2.1 Tier A — headless synthetic-Pointer harness (PRIMARY CI gate)

A `PointerHarness` in `buiy_verify` (new module `buiy_verify::pointer`) that fixes `picking_backend.rs`'s blind spot: it spawns a **real, non-origin** widget tree and lets the production layout→bridge→transform-propagation chain produce `GlobalTransform`, then injects synthetic pointer input through the **sanctioned bevy_picking path** and asserts on durable state + a thin observer capture.

```rust
// crates/buiy_verify/src/pointer.rs
pub struct PointerHarness {
    app: App,
    pointer: Entity,        // the synthetic PointerId entity (spawned once)
}

impl PointerHarness {
    /// MinimalPlugins + TransformPlugin + CorePlugin + LayoutPlugin
    /// + bevy::picking::PickingPlugin + the Buiy interaction stack
    /// (BuiyPickingBackendPlugin + InteractionPlugin [C3] + FocusPlugin
    /// + the widget/state plugins [C4]). NO RenderPlugin, NO winit, NO
    /// AssetPlugin — InteractionPlugin runs in PreUpdate as pure ECS
    /// (bevy-picking architecture.md:19,32), so the full Pointer<E>
    /// taxonomy + bubbling + propagate(false) is headless-CI-runnable.
    pub fn new() -> Self;

    /// Spawn the fixture scene under a root placed at a NON-ORIGIN offset
    /// (the regression seam for Bug 1). Runs `update()` until layout +
    /// bridge + transform propagation have produced GlobalTransform on
    /// every node (condition-based, not a fixed frame count).
    pub fn spawn_offset_tree(&mut self, offset: Vec2, scene: impl FnOnce(&mut World)) -> &mut Self;

    /// Move the synthetic pointer to a WINDOW-space position and run one
    /// update. Writes PointerLocation directly (the lessons-sanctioned
    /// synthetic path; see §3.3). Subsequent press/release/click reuse it.
    pub fn move_to(&mut self, window_pos: Vec2) -> &mut Self;
    pub fn press(&mut self) -> &mut Self;     // writes PointerInput Press, updates
    pub fn release(&mut self) -> &mut Self;   // writes PointerInput Release, updates
    pub fn click(&mut self) -> &mut Self;     // press + release at the current pos

    /// Assertion surface (see §3.1 — decided: state-flip is PRIMARY).
    pub fn world(&self) -> &World;            // assert A11yToggled/A11ySelected/FocusedEntity (read for VISUAL/state flips)
    pub fn captured<E: ...>(&self) -> &[CapturedEvent<E>];  // thin observer log (propagation tests)
    // For any a11y-tree assertion, read the AGENT-INTERFACE semantic-tree tier
    // — `buiy_verify::a11y::semantic_tree(self.app_mut(), TreeView::Unmerged)`
    // (its `inprocess::snapshot` over the canonical tree). C7 does NOT add an
    // `a11y_tree()` accessor that re-derives a parallel a11y view.
    pub fn app_mut(&mut self) -> &mut App;    // hand the running app to the agent-interface semantic_tree() reader
}
```

The harness installs a **capture observer** resource: a tiny `#[derive(Resource, Default)] struct CapturedEvents(Vec<(Entity, Phase)>)` that an observer pushes into, so propagation/bubbling/`propagate(false)` tests can read which entities saw an event and in what order. This is the only test-only wiring; it observes the production events C3 defines, it does not replace them.

**What Tier A catches (the cross-tier map, §6):** Bug 1 (offset widget → absolute-consumer divergence — the regression test *for* C1); the entire post-C3 event model (`Click`/`DoubleClick`/`Press`/`Release`/`Drag`, observers, capture→target→bubble, `Pickable::IGNORE` composite-widget hit-through); focus-on-click (C4); pick-depth/stacking paint-order (C3's `painters_z` primitive); overlay/modal hit-blocking + `should_block_lower` (C3/C5). This is the **picking/coordinate-geometry** divergence the agent-interface semantic-tree tier does **not** exercise (its in-process driver dispatches `ActionRequest`s over the canonical AccessKit tree — it never injects a pointer at a window coordinate, so it is structurally blind to Bug 1; that is precisely why C7's tier sits alongside it). **Cannot** catch: async-font timing (→ Tier B), winit coordinate/scale-factor conversion + real camera ref (→ Tier C), and a11y-tree state/structure — **a11y-tree consistency after bulk ops** (toggle-all / clear / filter — audit §6.7) is the agent-interface **gate #3/#12** territory, asserted via its `semantic_tree(app, view)` snapshot tier, not a C7-owned `A11yTreeBuilder` read. Where Tier A drives a bulk op, it *reads* that tier; it does not redefine it.

### 2.2 Tier B — headless FontsGeneration-bump (co-delivered with C2)

Fold into the existing **adapterless** `TextExtractHarness` (`extract_harness.rs`) — **not** a new `DefaultPlugins+AssetPlugin` app. Add helpers that register a font / call `apply_font_registry` mid-sequence while an editor buffer holds typed content, then assert the two Bug-2/Bug-3 properties. This is the isolating test the audit says is missing, with deterministic injection and zero winit.

```rust
// added to crates/buiy_core/tests/support/extract_harness.rs
impl TextExtractHarness {
    /// Inject the FontsGeneration bump deterministically: bump the
    /// `FontsGeneration` resource the way `apply_font_registry`
    /// (registry.rs:543) does on a runtime add_font, then run one frame.
    /// This is the trigger for Bugs 2/3 (audit Bug 3: the sweep fires on
    /// EVERY runtime add_font, not just startup) — C2 must confirm this
    /// reproduces the clobber identically to the async loader path.
    pub fn bump_fonts_generation(&mut self) -> &mut Self;
}
```

Tests (in a new `crates/buiy_core/tests/text_font_reload_survival.rs`, replacing the dead auto-healing `text_commit_font_reload.rs` pattern):
- **content-survival (Bug 3):** spawn a `TextEditState` editor, seed it via the real path, type content so the editor-owned buffer ≠ display `Text`, `settle()`, `bump_fonts_generation()`, `settle()`; assert the editor buffer **still holds the typed content** (string equality on the editor value, not just size-parity — the audit Appendix-B.6 flags the prototype substrate test only checked size).
- **reshape / shape-guard (Bug 2):** after the bump, assert `glyph_count() == expected` and `changed_frames` shows the buffer **reshaped** (did not stay unshaped → silent-no-paint).
- **empty-editor 0-vs-1:** an empty editor with no placeholder emits 0 glyphs after the bump and does **not** crash/assert-fire; the empty case is the one the prototype's "complete" fix accidentally relied on (audit Bug 2 critical refutation).
- **preedit-survival:** set an IME preedit, bump, assert composition survives (a mid-composition `set_text` destroys it — audit Bug 3, §2.6).

### 2.3 Tier C — #[ignore] GPU/winit smoke (additive, cannot gate PRs)

One end-to-end `#[ignore]` fixture under real `DefaultPlugins` on the GPU lane (`cargo test … -- --ignored --test-threads=1`, CLAUDE.md GPU lane), exercising what (A)+(B) cannot: the **winit coordinate/scale-factor conversion** and the **real camera ref** (replacing `Entity::PLACEHOLDER`, `backend.rs:65`). Decided minimal (§3.5): spawn a button at a non-origin, non-1.0-scale-factor position; drive a winit cursor-moved + click at the OS-space coordinate; assert the same `A11yToggled`/state flip Tier A asserts at the logical coordinate. **No new pixel surface** — it asserts state, not pixels, so it cannot add a second flaky golden.

### 2.4 Content-presence invariant + bless-guard (W19, §6.7)

A Tier-3-style invariant `content_is_present` on the production extract path, enrolled via `coverage`, plus a bless-guard in the golden path.

```rust
// crates/buiy_verify/src/invariant.rs  (new predicate, joins the Tier-3 family)
/// For a fixture declared text-bearing, the production extract path
/// (extract_buiy_glyphs, via the adapterless TextExtractHarness) MUST
/// emit > 0 glyph instances. A zero-glyph text-bearing widget is the
/// silent-no-paint failure (Bug 2 release mode).
pub fn content_is_present(app: &App) -> Result<(), Violation>;
```

- **Mechanism:** run the fixture's spawned scene through the adapterless extract step (the `TextExtractHarness` machinery, reused — `text_extract.rs` already proves `glyph_count()` on the production producer), assert `glyph_count > 0` for text-bearing fixtures. Enrolled by `coverage::enroll_all` so **every** label-bearing fixture is auto-checked headlessly.
- **Bless-guard:** in `golden::assert_golden` / the `BUIY_BLESS` path, a fixture declared text-bearing **cannot be blessed when its `glyph_count == 0`** — closes the silent-no-paint hole at the corpus boundary (the bless-guard refuses, loudly, rather than recording a blank baseline). Reuses the existing `CoverageKey`/`Backend` schema — **no new key field** (skia-gold/wpt lesson: fixing the schema before generating goldens; a field add re-baselines the corpus, §6.7).
- **Text-bearing scope:** **infer from component presence at spawn** (`Text` or `TextEditState` on any spawned entity), not a per-fixture flag (§3.4). The placeholder-editor edge case is handled explicitly: an empty editor showing a `Placeholder` with `PlaceholderActive` emits placeholder glyphs and so is text-bearing-positive; an empty editor with no active placeholder is **not** text-bearing for the guard (so `glyph_count == 0` is legal there).

### 2.5 A11y wire format / semantic-tree tier — CEDED to the agent-interface campaign (C7 consumes it)

**This section is intentionally a non-deliverable for C7.** An earlier draft had C7 extend `buiy_verify::a11y::WireNode` + `role_to_str` + `KNOWN_ROLES` and co-land the `A11yNodeView`/`build_tree` widen. Under the 2026-06-22 coordinate-don't-cede decision (umbrella §2.7), **the a11y wire format and its tri-state/role serialization are owned by the agent-interface campaign**, which already designs exactly this and more, on the canonical AccessKit tree read through `accesskit_consumer`:

- **P0 (landed/landing there):** fix `buiy_verify::a11y::snapshot_tree` to emit `node_id_for(n.entity).0` (the ref off-by-one, re-blessing affected goldens in that same change); extend the `A11yRole` enum and update **both** stringifiers (`translate::role_to_accesskit` **and** `buiy_verify::a11y::role_to_str`, the `_ => "Unknown"` wildcard being the half-update tripwire).
- **P1a (there):** widen `A11yNodeView` (today 5 flat fields) + the `build_tree` query tuple to the full winit-free decomposed-state snapshot; rewrite `to_accesskit_node` as the 0.24-setter derive fold (tri-state `set_toggled` incl. `Mixed`, `set_selected`, `set_expanded`, `A11yValue`/`A11yTextValue`, `set_live_atomic` + role-implied live, `active_descendant`, …). This **is** the "tri-state, role-disambiguated, present-only, `skip_serializing_if`" wire format the audit praised in the prototype — rebuilt on 0.24 **there**, not here.
- **The semantic-tree snapshot tier** (`buiy_verify::a11y::semantic_tree(app, view) -> String`, the agent-interface "new lowest tier") + gates **#3** (role/name/state/relations/actions/ref), **#4** (announcements), **#6** (synthesized `ActionRequest` replay), **#7** (APG keyboard), **#12** (proptest invariants) are the agent-interface deliverable.

**C7's relationship to that tier:** *consumer only.* Where a C7 test needs to observe a11y state (e.g. after a Tier-A `click()` confirm the checkbox lowered to `"toggled": "on"`, or after a bulk op confirm the tree shape), it calls the agent-interface `semantic_tree(app, view)` over the same running app. C7 adds **no** `WireNode` field, **no** `role_to_str`/`KNOWN_ROLES` arm, and triggers **no** a11y-golden re-bless — those re-blesses happen once, in the agent-interface P0/P1a change, governed by that campaign's "every new role/state ships its #3 fixture in the same change" coverage rule. The earlier §6.5 single-coordinated-change-with-C4 framing, the `active_descendant`/`live`/`modal`/`scroll_*` field declarations, and the Mixed-serializes-`"mixed"` mutation/control pair are all **superseded** by — and now live in — the agent-interface gates #3/#4 (it owns the same mutation discipline).

### 2.6 Catalog-widget fixture enrollment plan (coverage-by-construction)

Every catalog widget ships as a `fixtures/<widget>/<state>.rs` via the `fixture!` macro (one `#[path] mod` line in `coverage/mod.rs`, SKILL.md:69-82), so it auto-enrolls across layout/display-list/invariant/`content_is_present`/forced-colors/golden + the 24-cell Matrix. C7 owns the **plan + discipline**; C8 authors the fixtures, C4/C5/C6 supply the widgets. Fixtures MUST be:
- **forced-colors-safe** — system-color tokens (mirror the existing `fixtures/button/resting.rs:53-67`); the default `Button::new()` paints the magenta sentinel under forced-colors (coverage.md § Honest deviation), so catalog fixtures insert the forced-colors-safe target paint explicitly.
- **color-asymmetric** — never `#ffffffff`/`#ff00ffff` only (both invariant under R↔B swap, SKILL.md:187), so a color mutation is observable.
- **`Name`-tagged roots** — every dump keys by `Name`, never `Entity` bits (SKILL.md:184).

Enrollment targets for the catalog (one fixture set per state): `button` (resting/hover/focus/pressed/disabled), `checkbox` (off/on/**mixed**/disabled), `switch` (off/on), `radio`+`radiogroup`, `textfield` (empty/placeholder/filled/focus), one selection widget, `scrolllist`, `menu`/`overlay`, `modal`. The new fixtures grow the 24-cell-per-fixture corpus; `verify_cell_count_under_ceiling` (`CELL_CEILING_PER_FIXTURE = 32`, coverage.md:316) is a deliberate budget tripwire, not a surprise.

**C7 signs off on C8's reduced-matrix constructor living in the C7-owned `matrix.rs`.** The gallery's full-screen capstone fixture (C8) would blow the per-fixture cell budget if it ran the full 24-cell Matrix on every screen, so C8 needs a reduced matrix — a `Matrix::gallery_screen()` constructor that selects only the load-bearing axes for a whole-screen capture. Because the Matrix type is C7-owned verification infrastructure, that constructor **lives in the C7-owned `crates/buiy_verify/src/matrix.rs`**, not in C8's gallery crate; it lands **coordinated with C7** (C7 reviews/owns the axis-reduction so it stays a principled subset of the full Matrix, not an ad-hoc escape hatch around the ceiling). C7 explicitly signs off on this here so it is not scope bleed when C8 lands.

---

## 3. Decisions & rejected alternatives

### 3.1 Tier-A assertion surface: state-flip PRIMARY, observer-capture for propagation only — *decided*

Assert on the **durable widget-state component** — the agent-interface-owned `A11yToggled` (tri-state, incl. `Mixed`) / `A11ySelected` / `A11yExpanded` / `FocusedEntity` (umbrella §2.7; C7 *reads* these for the visual/state flip, it does not define them) — as the primary surface; use a thin observer-capture log **only** for propagation/bubbling/`propagate(false)` tests that have no state side-effect to observe.

**Why:** state-flip assertions are durable across C3's event-API churn (the prior-art warns of one bevy_picking rename per minor, lessons.md:60-64) — a test asserting "after `click()` the `A11yToggled` flipped to `On`" survives a `Press`→`Down` rename, whereas a test asserting "the `Pointer<Click>` observer fired" breaks. The decomposed a11y-state model (agent-interface P1a `a11y/states.rs`) exists precisely to give a stable, queryable seam; lean on it. Propagation order, by contrast, has *no* durable state proxy — that genuinely needs the capture log, so keep it for those cases only.

**Rejected — observer-capture as the primary surface:** directly mirrors the new model, but couples every interaction assertion to event names that the prior-art guarantees will churn, and makes the gate brittle exactly where it must be most stable (it gates C3's own migration of ~18 hand-set-`Hovered` test files, umbrella §5 Wave 2). Rejected.

**Rejected — both, everywhere:** doubles the assertion surface and the churn-exposure for no added coverage on the durable cases. Use both only where each is load-bearing.

### 3.2 InteractionPlugin reads direct `PointerLocation`/`PointerInput` injection without `PointerInputPlugin` — *decided (with a build-step verification gate)*

Tier A writes `PointerLocation` + `PointerInput` **directly** onto the synthetic pointer entity and does **not** add bevy_picking's `PointerInputPlugin`. This is the lessons-sanctioned synthetic path (lessons.md:36-40, "Custom pointer ID lets synthetic input pass through the normal pipeline"; :90-92).

**Why:** direct injection is more deterministic (no winit input-gathering, no frame-coupled bookkeeping) and is the explicitly-sanctioned test-replay path. `PointerInputPlugin`'s job is to *translate winit events into* `PointerInput` — which is exactly the layer we are replacing with synthetic input.

**Caveat carried to the build steps (not a deferral):** whether `InteractionPlugin` (C3) reads what direct injection writes **without** `PointerInputPlugin`'s per-frame `PointerLocation` maintenance must be confirmed against C3's actual wiring in Migration step 3. If C3's `InteractionPlugin` depends on `PointerInputPlugin` running first to populate `HoverMap` inputs, Tier A adds `PointerInputPlugin` and feeds it `PointerInput` rather than hand-maintaining `PointerLocation`. The *decision* (prefer direct injection) is fixed; the one-line confirmation is a build step because it depends on C3's exact plugin graph, which is co-designed.

**Rejected — drive winit synthetic events:** reintroduces the winit dependency Tier A exists to avoid, is non-deterministic, and is what Tier C is *for*. Rejected for the headless tier.

### 3.3 Bug-2/3 verified on the adapterless harness, not a real async loader — *decided (CI gate); real-async deferred to C2 confirmation*

Tier B's `bump_fonts_generation` on `TextExtractHarness` is the CI gate. A real `DefaultPlugins+AssetPlugin` async-loader variant is **not** built unless C2 surfaces a loader-timing bug class the synchronous bump cannot reproduce.

**Why:** the bump is the same `FontsGeneration` resource change the async path produces (audit Bug 3: `apply_font_registry`, `registry.rs:543`, fires the all-buffers sweep on every runtime `add_font`); injecting it deterministically isolates Bugs 2/3 with zero winit/async flakiness, on a substrate that already drives the production `extract_buiy_glyphs`. The prototype's real-async test was flaky and auto-healed (audit §7) — exactly the failure mode to avoid.

**Gate handed to C2 (§7):** C2 must confirm the bump reproduces the clobber **identically** to the async path. If it does not, the real-async variant becomes a Tier-C `#[ignore]` smoke (one fixture), never a per-PR gate.

**Rejected — full `DefaultPlugins+AssetPlugin` as the CI gate:** higher fidelity but reintroduces async timing the harness deliberately removes (the Ahem-font determinism lesson: the synchronous font is *why* no current tier exercises the async path — flutter-golden lesson). It would be flaky and slow on every PR. Rejected as the gate; reserved as a conditional smoke.

### 3.4 Bless-guard text-bearing scope: infer from component presence, with explicit placeholder handling — *decided*

A fixture is "text-bearing" if any spawned entity carries `Text` or `TextEditState`. The placeholder edge: an editor showing an **active** `Placeholder` (`PlaceholderActive` present) is text-bearing-positive (placeholder glyphs are real paint); an empty editor with **no** active placeholder is not text-bearing (legal `glyph_count == 0`).

**Why:** inference is less error-prone than a hand-set per-fixture flag (a flag drifts the moment someone adds a label and forgets the flag — exactly how the prototype's `text_buffers_shaped` rotted, §1.4). Reading the live component is the same "observe real paint, not a stale descriptor" discipline coverage's forced-colors producer already follows (coverage.md verification #4).

**Rejected — per-fixture `text_bearing: bool` flag:** explicit but desync-prone and adds a `Fixture` field that re-baselines the macro; the inference reads the *same* live tree every other tier reads. Rejected.

**Rejected — treat the placeholder as never-text-bearing:** would let a broken placeholder render blank silently — the placeholder IS visible text and must be guarded. Rejected; placeholder-active is positive.

### 3.5 Tier C is a single state-asserting smoke, not a pixel test — *decided*

One `#[ignore]` fixture, asserting the winit-path state flip (not pixels), at a non-1.0 scale factor and non-origin position.

**Why:** Tier C exists only to cover the winit coordinate/scale-factor conversion + real camera ref that (A)+(B) cannot reach (`Entity::PLACEHOLDER`, `backend.rs:65`). Asserting state (the same `A11yToggled` flip Tier A checks, but routed through real winit coordinates) proves the conversion without opening a second flaky pixel surface — the existing golden tier already owns rasterization residue.

**Rejected — a visual golden in Tier C:** would duplicate Tier 5's job and add flake; the coordinate-conversion bug is a *state* bug (wrong entity picked), observable without pixels. Rejected.

### 3.6 a11y-tree-under-bulk-ops rides the agent-interface semantic-tree tier — *decided (ownership ceded)*

a11y-tree-under-bulk-ops (toggle-all / clear / filter; audit §6.7) is an **agent-interface gate #3/#12 concern**, asserted through its **semantic-tree snapshot tier** (`semantic_tree(app, view)` → `inprocess::snapshot`), **not** a C7-owned `A11yTreeBuilder` read. Where C7's Tier A drives the bulk op (it has the running headless app and the synthetic-pointer driver to *cause* a toggle-all/clear via real clicks), it hands the app to that tier and asserts on the returned `SemanticTree`.

**Why:** the agent-interface in-process driver runs `snapshot(world, view)` through the production `build_tree` → `build_tree_update` → `accesskit_consumer::Tree` path — the same tree a real AT sees — entirely headless (no winit, no adapter). Duplicating that with a C7-private `A11yTreeBuilder` read would fork the a11y view the umbrella §2.7 forbids. The audit's open-Q #8 ("does `Display::None` remove a row from the accesskit tree headlessly?") is answerable on that tier; whether `Display::None` itself prunes or C5's `Inert` marker prunes explicitly is **C5's** pruning design + the agent-interface gate's call, not C7's.

**Division:** C7 may *cause* the bulk op through Tier A's real-input driver (that exercises the picking/event geometry end-to-end), then *read* the agent-interface tier to assert the resulting a11y shape. The assertion lives at the agent-interface gate; the causation is C7's pointer geometry.

---

## 4. Contracts & interfaces

### Shared contracts referenced (per umbrella §6 — NOT redefined here)
- **§6.1 pick-depth from `painters_z`** — C3 owns the derivation; Tier A *asserts* pick-order == paint-order on a stacked/overlay fixture, consuming whatever `painters_z` C3 produces. C7 does not define the primitive.
- **§6.2 coordinate space (C1)** — Tier A's offset-tree design is the regression test *for* C1; it asserts absolute consumers read `GlobalTransform`. `bridge.rs:138` is an invariant to **preserve** (verified: `base = Mat4::from_translation((resolved.position - acc)…)`), so Tier A spawns trees that exercise the bridge fold rather than hand-writing `ResolvedLayout`.
- **§6.3 `Pointer<Scroll>`** — C3 owns the entry; Tier A asserts the scroll event reaches the nearest container (C5's routing) on a scroll fixture.
- **§6.4 focus** — Tier A asserts C3's focus-on-click signal and reads `FocusedEntity` (C4 consumes); the focus *tree* (C5) is asserted via `compute_next_focus` scope behavior in Tier A's offset tree.
- **§6.5 a11y wire format** — **owned by the agent-interface campaign**, not C7 (umbrella §2.7). The `WireNode`/`A11yNodeView` widen, `role_to_str`/`KNOWN_ROLES`, `skip_serializing_if` discipline, and the single coordinated re-bless are the agent-interface P0 + P1a change + its gate #3. C7 *consumes* the resulting `semantic_tree(app, view)` tier; it neither defines fields nor owns the re-bless (§2.5).
- **§6.7 R1/R2 byte-stability** — **C7 owns the byte-assertion tests** (`assert_instance_hex_snapshot`, the no-border quad pixel-stability check); the content-presence guard reuses the existing `CoverageKey`/`Backend` schema with **no field add**.
- **§6.9 activation parity** — Tier A's accessibility-parity assert: a widget reaches the **same durable state flip** whether activated by `Pointer<Click>` (C3 pointer geometry) **or** by the agent-interface router (an inbound `Action::Click` `ActionRequest` → `OnPress`/`Focus`/`EditCommand`, per the agent-interface action-router). There is **no competing Buiy-native `Activate` event** — activation flows through the existing `OnPress` / the agent-interface router (umbrella §2.7). C7 asserts the pointer path and the AT/keyboard path (driven through the agent-interface in-process driver's `perform`/`click`) converge on the same state.

### Own contracts (defined here)
- `buiy_verify::pointer::PointerHarness` (§2.1) — the synthetic-pointer real-input harness. Asserts on durable state (§3.1: reads `A11yToggled`/`A11ySelected`/`FocusedEntity`) + an optional `CapturedEvents` observer log for propagation.
- `buiy_verify::invariant::content_is_present(&mut App) -> Result<(), Violation>` (§2.4) — the content-presence predicate; text-bearing inferred from `Text`/`TextEditState` presence (§3.4).
- The bless-guard in `golden::bless_guard_check` at the `assert_golden` / `BUIY_BLESS` call site (§2.4) — refuses a zero-glyph text-bearing cell; no key-schema change.
- `TextExtractHarness::bump_fonts_generation` (§2.2) — deterministic Bug-2/3 trigger on the adapterless harness.
- `buiy_verify::matrix::Matrix::gallery_screen()` (§2.6) — the reduced-axis Matrix constructor C8's whole-screen capstone fixture uses; lives in the C7-owned `matrix.rs` and lands coordinated with C7 (C7 signs off the axis reduction so it stays a principled subset, not an ad-hoc ceiling escape).

### NOT a C7 contract (owned by the agent-interface campaign — §2.5, umbrella §2.7)
- `WireNode` fields / `A11yNodeView` widen / `role_to_str` / `KNOWN_ROLES` / the tri-state serialization / the a11y-golden re-bless — **agent-interface P0 + P1a**. C7 consumes `buiy_verify::a11y::semantic_tree(app, view)` and the gates #3/#4/#6/#7/#12, it does not define them.

---

## 5. Migration / build steps (ordered; blast radius noted)

Wave-1 ordering: C7's Tier-A harness skeleton + content-presence + a11y extension land first (RED against the not-yet-built C1/C3/C4), then go GREEN as those land. **Every step proves RED before GREEN (§6).**

1. **Content-presence invariant + bless-guard** — add `content_is_present` to `invariant.rs`; wire into `coverage::enroll_all`; add the bless-guard to `golden.rs`. *Blast:* new predicate + one enrollment body line + golden-path branch. **RED proof:** a zero-glyph label fixture (text shaping forced to drop) → `content_is_present` RED + bless-guard refuses. *Touches:* `invariant.rs`, `golden.rs`, `coverage/enroll.rs`, one `#[cfg(test)]` mutation fixture.
2. **a11y wire-format / semantic-tree tier — NOT a C7 step (consumed from the agent-interface campaign).** The `WireNode`/`A11yNodeView` widen, the role serialization, `KNOWN_ROLES`, the tri-state `skip_serializing_if` discipline, and the one-time a11y-golden re-bless are the **agent-interface P0 + P1a** change (§2.5, umbrella §2.7). C7 does not touch `a11y.rs`'s `WireNode`/`role_to_str`/`KNOWN_ROLES`; it consumes the resulting `semantic_tree(app, view)` tier + gates #3/#4/#6/#7/#12. *This step is deliberately a no-op for C7* — listed so the deletion is explicit, not silent.
3. **Tier-A `PointerHarness` skeleton (against C3's model)** — build the harness module; confirm the §3.2 direct-injection-vs-`PointerInputPlugin` question against C3's plugin graph (one-line confirm). *Blast:* new `buiy_verify::pointer` module; the existing `picking_backend.rs` is **kept** as the unit-level backend test, annotated as superseded-for-integration-coverage by Tier A (audit §7's "keep the nested+offset regression test" is subsumed by Tier A's offset-tree design). **RED proof (the load-bearing one):** with C1 reverted, Tier A on an **offset** widget goes RED (picks the wrong entity / no hit) — this is the regression gate that proves C1. *Touches:* new module, `picking_backend.rs` doc note.
4. **Tier-A behavior assertions** — focus-on-click, pick-depth/stacking, overlay/modal hit-blocking, `Pickable::IGNORE` composite hit-through, activation-parity (pointer `Click` vs the agent-interface router's `Action::Click`→`OnPress`). Any a11y-tree assertion (incl. a11y-tree-under-bulk-ops) reads the **agent-interface** `semantic_tree(app, view)` tier — C7 causes the bulk op via Tier-A real input, the agent-interface gate #3/#12 owns the assertion. *Blast:* test bodies in the new module; depends on C3 components being live + the agent-interface state components/router landed. **RED proof:** each assertion injected-bug-tested (e.g. revert focus-on-click → RED; collapse `painters_z` to area → overlay pick RED).
5. **Tier-B FontsGeneration-bump** — add `bump_fonts_generation` to `extract_harness.rs`; write `text_font_reload_survival.rs` (content-survival, reshape, empty-editor 0-vs-1, preedit); delete the dead auto-healing `text_commit_font_reload.rs` pattern. *Blast:* one harness method + one test file; co-sequenced with C2. **RED proof:** with C2's TextSync fix reverted, content-survival goes RED (the editor buffer is clobbered to `""`).
6. **Tier-C winit smoke** — one `#[ignore]` `DefaultPlugins` fixture asserting the winit-coordinate state flip at non-1.0 DPR. *Blast:* one GPU-lane test; cannot gate PRs. **RED proof:** revert the real-camera-ref wiring (`Entity::PLACEHOLDER`) → the back-projection picks nothing → RED.
7. **Catalog fixture enrollment** — as C4/C5/C6/C8 land widgets, author `fixtures/<widget>/<state>.rs` (forced-colors-safe, asymmetric, `Name`-tagged) + the `#[path] mod` line. *Blast:* grows the corpus by 24 cells/fixture (watch `CELL_CEILING_PER_FIXTURE`); new layout/display-list `.snap` per cell. **RED proof inherited** from coverage's `broken_fixture_produces_violation` discipline.

---

## 6. Verification (how C7 gates the campaign; what must be proven RED-first)

C7 **is** the gate, so its own teeth are the deliverable. The harness is load-bearing test infrastructure — a property that never fails is worse than none (verification-design README § Verification of the harness; SKILL.md:204). The vacuous-green risk is the top risk (umbrella §9.5): the existing `picking_backend.rs` hand-writes `ResolvedLayout` and is **structurally blind** to Bug 1, so it must **not** be trusted as the gate. Every new predicate is proven RED-before-GREEN:

- **Tier A ↔ C1 (the load-bearing RED proof):** revert C1's coordinate fix → Tier A on an **offset** widget goes RED (the synthetic pointer over the visually-correct position picks the wrong entity or no entity). This is the single most important proof in the campaign — it is what makes Tier A a real gate rather than a green-by-construction rubber stamp.
- **Tier A ↔ C3:** revert focus-on-click → focus-on-click assert RED; collapse `painters_z` depth to smallest-area → overlay/modal hit-blocking assert RED; remove `propagate(false)` → bubbling-halt assert RED (via the `CapturedEvents` log).
- **Tier A ↔ C4 (visual/state read):** a Mixed checkbox under `click()` flips the agent-interface `A11yToggled` to the correct tri-state; the activation-parity assert goes RED if the pointer `Click` path and the agent-interface router (`Action::Click`→`OnPress`) path diverge on the resulting state.
- **Tier B ↔ C2:** revert the TextSync editor-clobber fix → content-survival RED (buffer clobbered to `""`); revert the shape-guard → reshape/`glyph_count` RED (silent-no-paint).
- **content-presence ↔ Bug 2:** a zero-glyph text-bearing fixture → `content_is_present` RED + the bless-guard **refuses to bless** (proven via a `#[cfg(test)]` zero-glyph mutation fixture excluded from the real catalog, mirroring coverage's `broken_fixture_produces_violation`).
- **a11y wire format / tri-state serialization mutation pair:** **owned by the agent-interface gate #3/#4** (the Mixed-serializes-`"mixed"` / control-omits-`checked` discipline lives there) — not a C7 RED proof.

Specific fixtures/tests: the `PointerHarness` offset-tree cases (`buiy_verify::pointer` + `crates/buiy_verify/tests/` integration tests); `crates/buiy_core/tests/text_font_reload_survival.rs` (Tier B); `content_is_present` in `crates/buiy_verify/tests/content_presence.rs`; the Tier-C `#[ignore]` smoke on the GPU lane. All headless tiers run under the workspace gate (`cargo nextest run --workspace` per the incoming PR #77 runner — see §0); Tier C runs under the GPU `--ignored` lane (CLAUDE.md). The a11y semantic-tree gates run in the same headless lane but are the agent-interface deliverable.

---

## 7. Open questions deferred + dependencies

**Resolved here** (see §3): Tier-A assertion surface (state-flip primary on the agent-interface `A11yToggled`/`A11ySelected`, §3.1); direct injection vs `PointerInputPlugin` (direct, with a build-step confirm, §3.2); adapterless-bump vs real-async for Bugs 2/3 (adapterless gate, §3.3); bless-guard text-bearing scope incl. placeholder (infer + placeholder-active-positive, §3.4); Tier-C single state smoke (§3.5); a11y-tree-under-bulk-ops reads the agent-interface semantic-tree tier (§3.6).

**Deferred — genuinely depend on un-built children's exact shape (confirmed at build time, not re-decided):**
- **§3.2 build-step confirm:** does C3's `InteractionPlugin` read direct `PointerLocation`/`PointerInput` injection without `PointerInputPlugin`? Depends on C3's plugin graph (Wave 2). The decision (prefer direct) is fixed; the confirm is a one-liner against C3's wiring.
- **§3.6 (now an agent-interface concern):** whether `Display::None` / `Inert` prunes a row from the canonical tree headlessly is the agent-interface gate #3/#12 + C5 pruning question, read on `semantic_tree(app, view)`; C7's only role is *causing* the bulk op via Tier-A real input.
- **§3.3 hand-off to C2:** C2 must confirm `bump_fonts_generation` reproduces the clobber identically to the async loader path; if not, a Tier-C async smoke is added (not a per-PR gate).

**Dependencies (sequencing):**
- **C0** (umbrella) — anchors the cross-cutting arbitrations C7 references (§6.1–6.9). Done.
- **C1** (coordinate space) — Tier A is the regression test *for* it; Tier A lands RED-first in Wave 1 and goes GREEN when C1 lands. Co-sequenced.
- **C2** (text fixes) — Tier B is co-delivered (Wave 1); the survival/reshape tests gate C2's fix.
- **C3** (input model) — Tier A's event-model assertions (Wave 2) consume C3's `Pointer<E>`/`painters_z` (activation routes through `OnPress`/the agent-interface router, not a competing `Activate`, umbrella §2.7); the harness skeleton lands ahead (Wave 1) and the behavior asserts fill in as C3 lands.
- **Agent-interface campaign** (a11y substrate + semantic-tree tier) — C7 consumes its `A11yToggled`/`A11ySelected`/`A11yExpanded` state components (state-flip reads), its `semantic_tree(app, view)` snapshot tier + gates #3/#4/#6/#7/#12 (all a11y assertions), its action router (activation-parity), and its in-process driver (`perform`/`click` for the AT-path parity). C7 builds the **stacking-aware `hit_test`** (via C1+C3) that the agent-interface actionability gate's `HitTargetable` deferred (its follow-up #3) and depends on.
- **C4** (visual/picking extension of the agent-interface widget bundles) — the state-flip assertions consume the agent-interface `A11yToggled`/`A11ySelected`/`A11yExpanded`; the a11y wire/role extension is the **agent-interface** P0/P1a change (§2.5), not a C4+C7 coordinated change here.
- **C5** (scroll/overlay/modal/focus) — the `Pointer<Scroll>` routing, overlay/modal hit-blocking, and `Inert`-prune assertions consume C5's containers (Wave 4).
- **C6** (styling) — the content-presence/display-list/golden enrollment must cover the newly-fed shadow/border/outline instances once they reach `ExtractedNodes` (today `extract_buiy_nodes` has no `BoxShadow` branch, coverage.md:353) — enrolled here, fed by C6.
- **C8** (gallery) — supplies the fixtures this tier enrolls; coverage-by-construction only pays off if the gallery widgets are authored as `buiy_verify` fixtures (the campaign's stated direction).

---

## Coordination with the agent-interface campaign + the incoming test infrastructure

Per umbrella §2.7 + §8 (the user's 2026-06-22 "coordinate, don't cede" decision), C7 **complements** the agent-interface verification deliverable and **builds on** the incoming test infrastructure — it never stands up a parallel a11y tier or a parallel test rig.

**Consumes (does NOT define / re-implement):**
- **The a11y wire format + semantic-tree tier** — `buiy_verify::a11y::WireNode`/`role_to_str`/`KNOWN_ROLES` (P0 extension), the widened `A11yNodeView`/`build_tree`/`to_accesskit_node` derive fold (P1a), and the `semantic_tree(app, view)` snapshot tier. C7 makes **every** a11y assertion through that tier; it adds no field, no role arm, and triggers no a11y-golden re-bless. (Was C7's §2.5 — now ceded.)
- **The decomposed a11y-state components** — `A11yToggled` (tri-state, incl. `Mixed`, role-disambiguated), `A11ySelected`, `A11yExpanded`, `A11yValue`, … from `a11y/states.rs` (P1a). Tier A/Tier C read these for the durable state-flip assertion (§3.1).
- **The `A11yRole` enum** (P0: Checkbox/Switch/Slider/TextInput/MultilineTextInput/Region/Group; more added there) — C7's fixtures reference roles; it adds none.
- **The inbound action router + in-process driver** (`route_action_requests`, `a11y/inprocess.rs`: `snapshot`/`perform`/`click`/`get_by_role`/`wait_for` + `accesskit_consumer`) — C7 drives the AT/keyboard activation-parity path through `perform`/`click`; activation routes via `OnPress`/the router, **no competing `Activate`**.
- **The a11y gates #3/#4/#6/#7/#12** — role/name/state/relations/actions/ref (#3), announcements (#4), synthesized `ActionRequest` replay (#6), APG keyboard (#7), proptest invariants (#12). a11y-tree-under-bulk-ops is gate #3/#12 (§3.6).
- **The incoming test infrastructure** — cargo-nextest + the 162→7 consolidated harnesses (PR #77), CI-hardening (#78), and any further test infra on `main`. C7 **extends** the project's runner/harnesses/gates; it never forks them. The c7 plan's Phase 0 re-confirms the then-current runner/harness surface (it is actively growing on `main`).

**Owns here (the geometry/render-content layer the agent-interface campaign does not build):**
- **Tier-A `PointerHarness`** — the picking/coordinate-**geometry** real-input tier. It injects a synthetic pointer at a window coordinate over a real non-origin laid-out tree and asserts the hit/state — proving the **Bug-1 picking divergence** the semantic in-process driver (which dispatches `ActionRequest`s over the canonical tree, never a pointer-at-a-coordinate) is structurally blind to. It sits **alongside** the agent-interface a11y tier, not on top of it.
- **Tier-B FontsGeneration-bump content-survival** (C2's gate) on the adapterless `TextExtractHarness`.
- **The content-presence invariant + bless-guard** (render-content: no glyph ⇒ no bless) on the production `extract_buiy_glyphs` path.
- **Tier-C `#[ignore]` GPU/winit smoke** (the winit coordinate/scale-factor + real-camera-ref path).
- **The catalog fixture-enrollment plan + `Matrix::gallery_screen()`** (coverage-by-construction; C8 authors the fixtures).
- **The stacking-aware `hit_test`** delivered by C1+C3, which the agent-interface actionability gate's `HitTargetable` deferred (its follow-up #3) and depends on.

**Removed / ceded vs the earlier draft:** the a11y `WireNode` tri-state + role serialization extension (§2.5, formerly a C7 deliverable); the `active_descendant`/`live`/`modal`/`scroll_*` field declarations; the Mixed-serializes-`"mixed"` mutation/control pair; the single-coordinated-change-with-C4 re-bless sequencing; any C7-owned `A11yTreeBuilder` read or `a11y_tree()` accessor; and any Buiy-native `Activate` parity assert (replaced by router/`OnPress` parity). All now live in the agent-interface campaign.
