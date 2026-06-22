**Date:** 2026-06-18
**Status:** report (research input to a future `buiy-agent-interface-design` spec)

# Agent ↔ running-app interaction surface — research

> Can LLM agents drive UI state transitions and inspect state on a *running* Buiy
> app instance, and what would "native LLM interfacing" look like as a first-class
> capability? This report is the **research** stage output: an internal capability
> audit (cited to `file:line`), an external prior-art survey, and a recommended
> layered architecture with rejected alternatives. It does **not** yet decide the
> spec — it feeds one.

Produced by a 14-agent research workflow (6-way internal audit + 6-way external
prior-art sweep → synthesis → adversarial code-verified review). The review
verdict was **sound-with-corrections**; every load-bearing internal claim below
was re-checked against the actual code, and the corrections are folded in. See
*Provenance* at the end.

---

## TL;DR

1. **Buiy is unusually well-positioned for this — better than a browser.** It
   already *authors* an AccessKit semantic tree (`A11yRole`/`A11yLabel`/
   `A11yDescription` → `A11yTreeBuilder` → per-window `accesskit_winit` adapter).
   That role + name + state + supported-actions tree is the *exact* surface
   Playwright-MCP, computer-use hybrids, Flutter (`SemanticsNode`/`SemanticsAction`),
   and Jetpack Compose (`SemanticsProperties`/`SemanticsActions`) all converged on
   as the agent perception+control surface. The usual knock on accessibility-tree
   automation — "the AX tree is a lossy projection of opaque pixels" — is
   **inverted** for Buiy: the tree is the *authored source of truth*, not an
   after-the-fact derivation. Most UI frameworks would have to *build* this tree to
   become agent-drivable; Buiy gets it from work already landed.

2. **The single load-bearing gap is direction.** The tree is verified
   **output-only**. Buiy declares exactly one action
   (`node.add_action(Action::Focus)`, `a11y/translate.rs:38-39`), pushes via
   `update_if_active` (`a11y/adapter.rs`), and reads **zero** inbound
   `ActionRequest`s — *even the `Focus` action it declares is never honored back*.
   Crucially, the inbound plumbing **already exists one layer down**: `bevy_winit`
   forwards OS/AT/agent actions onto `MessageWriter<ActionRequestWrapper>`
   (`bevy_winit .../accessibility.rs`), and Buiy reads that channel nowhere.
   Making the tree bidirectional is a **Buiy-side router + per-widget action
   vocabulary + per-state node enrichment** task — *not* a from-scratch protocol
   build, and *no new dependency* (`accesskit`/`accesskit_winit` are already direct
   deps).

3. **Recommended shape: semantic-tree-first, MCP-primary / BRP-debug-tier,
   two-tier transport.** Build the agent surface on the AccessKit tree Buiy already
   authors — not on raw ECS. Make it bidirectional through the existing
   `bevy_winit` channel (one router system, no second `ActionHandler`). Define
   **one** transport-agnostic inspect+control protocol over the live tree and
   expose it as (a) a fast **in-process Rust API** — the lowest tier, which Buiy's
   own tests/devtools drive directly with no serialization — and (b) a serialized
   **MCP** surface for out-of-process LLM agents, with **BRP** (Bevy's built-in
   JSON-RPC) as an opt-in raw-ECS *debug* escape hatch. The human devtools
   inspector, the in-process test driver, and the out-of-process LLM agent become
   **three clients of one substrate** (the Flutter cautionary tale is fragmenting
   into `flutter_driver` vs `integration_test` vs MCP — don't).

4. **Scope correction (from the review):** exposing a *shipped* Buiy app to
   *end-user* LLM agents over a network/MCP transport is an **app-layer** concern,
   not a foundation-library responsibility (foundation README non-goals: "data and
   transport are the consuming app's concern"). Buiy should **ship the substrate**
   (the bidirectional semantic tree + the in-process inspect/control protocol); the
   transport, auth, and the decision to expose end-user agents belong to the app
   built on Buiy or an **opt-in companion crate**. Treat `buiy_mcp` as spec-only /
   open-question for now, not a reserved foundation crate.

---

## Part 1 — What Buiy has today (internal capability inventory)

Buiy is a Bevy **0.18** ECS UI framework (a 0.19-rc.3 / accesskit-0.24 bump is in
flight on `worktree-bsn-support`), parallel to `bevy_ui`, with a retained widget
tree, CSS-subset layout (Taffy), cosmic-text, focus, a `bevy_picking` backend, a
full text-edit command pipeline, and a structured verification harness. Every
claim below was code-verified by the review.

### The semantic tree (perception) — already exists, output-only

| Capability | Where | Notes |
|---|---|---|
| Role vocabulary (9 roles, `#[non_exhaustive]`) | `a11y/mod.rs:25-40` | `Generic/Button/Link/Image/Text/Heading/Dialog/AlertDialog/Tooltip`; full ~38-role ARIA taxonomy deferred to `buiy-accessibility-design`. |
| Accessible name / description | `a11y/mod.rs:44-51` | `A11yLabel`/`A11yDescription` literal-string fast path. |
| Per-frame node-view snapshot | `a11y/mod.rs:90-114` | `build_tree` (Update, `BuiySet::A11yUpdate`) **clears** `A11yTreeBuilder.nodes` (`:100`) and rebuilds from a component query each frame. `snapshot()` exposes `&[A11yNodeView]` (`:72`). |
| Pure Buiy→AccessKit translation | `a11y/translate.rs:27-85` | `to_accesskit_node` sets role/label/description and **the only action ever attached**: `add_action(Action::Focus)` on focusable nodes (`:38-39`). |
| Stable, invertible node id | `a11y/translate.rs:17-21` | `node_id_for = NodeId(entity.to_bits().saturating_add(1))`; `ROOT_NODE_ID = NodeId(0)`. Invert: `Entity::from_bits(n-1)`. Pure, session-stable. |
| Machine-readable serialization | `buiy_verify/src/a11y.rs:43-76` | `snapshot_tree` → stable serde JSON. **Off-by-one caveat:** it emits raw `entity.to_bits()` (`:47`), **not** the `NodeId` (`bits+1`). The two serialization paths disagree on the key. |
| Per-frame push into winit adapters | `a11y/adapter.rs:51-72` | Strictly **outbound** — `update_if_active` per window; never reads anything back. |

### The driving substrate (control) — exists, not wired for external driving

| Capability | Where | Notes |
|---|---|---|
| **Inbound action channel (already populated!)** | `bevy_winit .../accessibility.rs` (`poll_receivers` → `MessageWriter<ActionRequestWrapper>`) | `bevy_winit` owns the per-window adapter and **already forwards inbound `ActionRequest`s** onto a Bevy message channel. Buiy reads it **nowhere** (`grep do_action/ActionRequest/ActionHandler/MessageReader<ActionRequestWrapper>` across `crates/` = **0** consumers). |
| Focus source of truth | `focus.rs:36-38` | `FocusedEntity(Option<Entity>)` resource (Reflect); written by `advance_focus` (`:82`) and text pointer/click only — *never* by an action. (Focus model is a Phase-0 stub.) |
| Activation path | `buiy_widgets/.../button.rs:27,71` | `OnPress(Entity)` + `emit_on_press_on_click` → `MessageWriter<OnPress>`. Keyboard activation is a TODO (`button.rs:69`). |
| Hit-test / actionability | `picking/mod.rs:37,51` | `hit_test(world, point) -> Option<Entity>` over `(Entity, &ResolvedLayout)`; `point_in_aabb`. Gives the "is this target hittable" check. |
| Text command sink | `text/edit/command.rs:21`, `text/edit/input.rs:67` | `EditCommand` = 13-variant pub enum (`Insert/Backspace/Delete/Enter/Motion/SelectAll/Escape/Submit/Cut/Copy/Paste/Undo/Redo`); `TextEditState::apply(&mut FontSystem, cmd, …)` is a clean keymap-bypassing seam — but it's a per-entity method consumed inside one focus-gated system (`apply_keyboard_edits`, `input.rs:459`), **not** a message bus. |

### Structured + visual inspection — reusable, observation-only

| Capability | Where | Notes |
|---|---|---|
| Layout / display-list dumps | `buiy_verify/src/snapshot.rs` (`layout_dump`, `display_list_dump`, `extract_nodes_from_world`, `NameLookup`) | Pure builders, reusable. But `LayoutEntry` + `collect_layout_entries` are **private** (`snapshot.rs:204/216`), and core render structs (`ExtractedNode`/`ExtractedNodes`/`PackedInstance`/`LayoutEntry`) derive `Debug` but **not** `Serialize` — an agent gets text dumps, not typed JSON. Only `A11yNodeView`/`GoldenKey` have a serde path. |
| Offscreen render-to-texture + GPU readback | **`buiy_core::render::golden.rs:248` (`capture_to_image`), `:431` (`readback_rgba_into`)** | *Correction:* the capture primitive lives in **`buiy_core`**, not `buiy_verify`. `buiy_verify::determinism::DeterministicApp::capture` (`determinism.rs:168`) is a thin wrapper. Vulkan RTT needs no X server → headless screenshots work on any GPU host. |
| a11y JSON snapshot + diff | `buiy_verify/src/a11y.rs:43,28` | `snapshot_tree` / `diff_snapshots` / `role_to_str` — the existing diffable serialization of semantic state. |

### Transport / reflection readiness

- **BRP is not wired.** No `bevy_remote`/`RemotePlugin`/`brp` in any `Cargo.toml`
  or `Cargo.lock`. **122** `register_type` calls in `buiy_core/src` give field-level
  Reflect over layout/render-style/core-marker/text-style components +
  `Theme`/focus/picking resources, so BRP would be near-free **for a debug tier**
  — but the two most agent-relevant payloads, **text content**
  (`TextEditState` `state.rs:72-73`, `TextBuffer` `components.rs:548-549`), are
  deliberately `#[derive(Component)]`-only / **un-reflected**, so raw BRP literally
  cannot read or type field text.
- **No `buiy_mcp` crate / no agent-interface anywhere in docs.**
  `architecture.md:70-83` lists `buiy_core` / `buiy_devtools` (human overlays) /
  `buiy_verify` (dev-dep harness). `grep` across `docs/` for
  `agent-interface|bevy_remote|brp|mcp` = **0 hits** — the question has not even
  been posed in the foundation open-questions.

---

## Part 2 — External prior art: everyone converged on the semantic tree

Six external clusters were surveyed. The throughline is striking: **every mature
"drive a running UI from outside" system addresses UI by a stable semantic node id
+ a closed action verb-set, with pixels as a fallback.** Buiy already has the tree
they each had to invent.

- **Bevy Remote Protocol (BRP)** — `bevy_remote` (JSON-RPC 2.0; HTTP on
  `127.0.0.1:15702` via `RemoteHttpPlugin`). Built-in verbs `world.query/get/list/
  insert/remove/spawn/despawn/reparent/mutate_components` (the `bevy/*` prefix was
  renamed to `world.*`/`registry.*`/`rpc.*` across 0.15→0.18) + `+watch` streaming
  diffs. **The load-bearing seam is custom methods:** `RemotePlugin::with_method`
  registers domain verbs with `&mut World` access. **Borrow:** custom-method seam,
  the fixed-loopback-port convention, `+watch` push, `registry.schema`/`rpc.discover`
  self-description. **Avoid:** treating raw components as the agent's UI model
  (the trap existing `bevy_brp_mcp` bridges fall into); the silent Reflect+serde
  visibility tax; no screenshot truth in core BRP.
  Sources: `docs.rs/bevy/latest/bevy/remote/`, the `bevy_remote` lib.

- **Accessibility tree as a control surface** — AccessKit
  `ActionRequest{action, target: NodeId, data}` → `ActionHandler::do_action`, with
  per-node advertised actions via `Node::add_action`. The same shape across
  platforms: Windows **UIA** control patterns (`Invoke`/`Toggle`/`ExpandCollapse`/
  `RangeValue`), Linux **AT-SPI** `do_action`, macOS **AX** `AXUIElementPerformAction`
  — which Playwright/WinAppDriver/Appium/dogtail already drive native apps through.
  **Borrow:** the exact inbound seam; action-set-as-affordance-menu;
  set-of-marks/semantic-locator selection by role+name+ref. **Avoid:** assuming the
  closed 22-verb enum covers every transition; assuming the screen-reader-lossiness
  story (it's inverted for an *authoring* producer — but it *does* bite any Buiy
  content drawn straight to GPU without emitting nodes); per-frame-rebuild target
  instability (`xa11y`'s "element replaced between query and action").
  Sources: `docs.rs/accesskit` `Action`, `docs.rs/bevy/latest/bevy/winit/accessibility/`.

- **Browser protocols (CDP / WebDriver / WebDriver-BiDi / Playwright)** — the
  decade-deep precedent. CDP `Accessibility.getFullAXTree`/`queryAXTree`,
  `Input.dispatchMouse/KeyEvent` (synthetic input through the *real* event path),
  `Page.captureScreenshot`, `Overlay.highlightNode`; one identity
  (`backendDOMNodeId`/`AXNodeId`) threaded across inspect and control. Playwright
  lifts this to **accessibility-tree-first locators** (`getByRole`/`getByLabel`),
  the **ARIA snapshot** (diffable YAML), and **actionability auto-waiting**
  (locate → wait-until-attached/visible/stable/hit-targetable/enabled → dispatch,
  with strict single-match + `force`). **Borrow:** stable opaque node ids,
  semantic locators, lazy re-resolved-at-action-time locators, actionability
  auto-waiting (maps onto Buiy's frame loop), synthetic input through the real
  pipeline. **Avoid:** CDP's domain sprawl + enable/disable statefulness;
  Chromium/version coupling; the two-tree (DOM + a11y) join (Buiy has one tree).

- **MCP + LLM-native UI control** — the user's core interest. MCP = JSON-RPC 2.0
  over stdio or Streamable-HTTP+SSE; **tools** (model-driven, typed I/O) +
  **resources** (URI-addressed, `subscribe` → `notifications/resources/updated`).
  The official **Rust SDK is `rmcp`** (`#[tool]` macros, stdio + HTTP/SSE).
  **Playwright-MCP is the template:** a small semantic toolset
  (`browser_snapshot`, `browser_click{ref}`, `browser_type{ref,text}`,
  `browser_press_key`, `browser_wait_for`), targets by `ref` from the latest
  **accessibility** snapshot (~2-5 KB structured text, reportedly **10-100×**
  cheaper than screenshots), and **auto-appends a fresh snapshot to every mutating
  tool's result**. Capability tiers gate pixel/`vision` and other escape hatches.
  **Computer-use** (pixel + coordinate clicks) is the fallback for genuinely visual
  content only. **ACI tool-design discipline:** few, semantic, consolidated tools;
  errors as guidance. **Avoid:** pixel-only perception as the spine; exposing raw
  ECS as the UI surface; one-tool-per-endpoint sprawl.
  Sources: `modelcontextprotocol.io/specification/2025-11-25`,
  `github.com/microsoft/playwright-mcp`.

- **Retained-mode framework semantics trees (Flutter + Compose)** — the closest
  analogs. Flutter's `SemanticsAction` (`tap`/`scrollX`/`increase`/`setText`/
  `setSelection`/`copy`/…/`customAction`) and Compose's `SemanticsActions`
  (`performClick`/`performTextInput`/`performScrollTo`/`performSemanticsAction`) are
  the AccessKit `Action` analog. **One tree, N consumers:** the semantics tree is
  *intentionally* the a11y surface **and** the test surface **and** the tooling
  surface **and** now the agent surface. Two transport tiers: **in-process** direct
  calls for tests (`SemanticsController`/`ComposeTestRule`, no serialization) +
  **out-of-process** RPC (Flutter VM Service JSON-RPC/WebSocket; Compose via
  `AccessibilityNodeInfo`) with a thin MCP shim. **Borrow:** one-tree-N-consumers; a
  generic `perform_action(node_id, Action, data)` primitive with ergonomic wrappers;
  author-supplied stable `testTag`/`ValueKey` locators decoupled from i18n text; a
  rich matcher set. **Avoid:** gating the agent surface behind a debug-only build
  (Flutter's friction — Buiy's a11y tree is *always live*); fragmenting into
  parallel automation stacks.
  Sources: `api.flutter.dev/.../SemanticsController-class.html`, `api.flutter.dev/.../semantics/`.

- **Engine remote/devtools + React DevTools** — Godot (remote scene tree +
  live Inspector property mutation), Unreal Remote Control (read **+ write +
  invoke functions** on live objects, HTTP+WebSocket `:30020`, flag-gated), Unity
  PlayerConnection (generic bus), React DevTools (transport-agnostic **Bridge/Wall**
  protocol, compact-incremental-tree + lazy-per-node detail, `overrideValueAtPath`,
  versioned handshake) + Testing Library `getByRole`. **The central lesson:** an
  agent interface and a human devtools inspector are the **same substrate, different
  clients**. **Borrow:** in-process-server-over-socket with the inspector as a
  *client*; React's transport-agnostic protocol layer; function-invocation not just
  field-set; an explicit exposure allowlist (Unreal Preset) as a security boundary.
  **Avoid:** editor/GUI-coupled framing (make the protocol first-class, not a
  reverse-engineered debugger wire); default-off/flag-gated posture (decide security
  deliberately); polling where Bevy change-detection can push.
  Sources: `github.com/facebook/react/.../react-devtools/OVERVIEW.md`.

---

## Part 3 — Recommended architecture (a 5-layer stack)

Each layer names the **existing Buiy substrate** it reuses and the **gap** to close.
Corrections from the adversarial review are folded in.

### L0 — Raw ECS introspection (debug escape hatch only)
- **Role:** whole-World read/mutate by reflected type path over JSON-RPC; the
  fallback when something an agent needs isn't in the semantic tree, and the
  framework-developer debugging surface.
- **Builds on:** 122 `register_type` calls; `ResolvedLayout{position,size}` is
  reflected.
- **Gap / honest framing:** `bevy_remote` absent; **text content un-reflected**, so
  this is "BRP-ready for the *debug tier* only," never the agent's primary plane.
  Raw component writes bypass Buiy's own systems/invariants.

### L1 — Semantic UI model = the AccessKit tree (primary perception + addressing)
- **Role:** the agent's perception surface and locator space — role + name + state +
  supported-actions (the affordance menu), keyed by a stable `ref`.
  `snapshot → act-by-ref`, `getByRole`-style.
- **Builds on:** `A11yTreeBuilder` (rebuilt each frame from components),
  `A11yNodeView`, `snapshot_tree` serde JSON, the pure invertible `node_id_for`.
- **Gap:** nodes carry only role/label/description + `Focus` action — **no
  state setters** (checked/toggled/expanded/selected/disabled/value); the tree is
  **flat** (every node under one synthetic `Window` root, `translate.rs:73-78`) — a
  *list*, not a navigable hierarchy with `labelledby`/`controls`/`active-descendant`;
  ~9 roles; **no author-supplied stable test id** decoupled from i18n text; no
  out-of-process read channel yet.

### L2 — Action + event injection through the REAL pipelines (drive transitions)
- **Role:** turn an intent-level command (`Action` + target `ref`, or a high-level
  click/type/focus) into a transition through the **same** focus/picking/input/edit
  systems a human drives — never a shadow control plane.
- **Builds on:** the **already-populated** `bevy_winit`
  `MessageWriter<ActionRequestWrapper>` channel; `FocusedEntity`;
  `OnPress`/activation; `EditCommand`/`TextEditState::apply`; `hit_test` +
  `ResolvedLayout` for actionability.
- **Gap (definitive):** **no inbound router exists.** Even the declared `Focus` is
  never honored. `Click` isn't declared on any node, so "press this button via a11y"
  is unrepresentable. No per-widget action vocabulary; no actionability auto-wait; a
  stability guard is needed (the per-frame `nodes.clear()` means `do_action` must
  tolerate a despawned/moved target across the read→act gap).

### L3 — Structured + visual inspection (verification / hybrid fallback)
- **Role:** beyond the a11y snapshot — a where-is-everything layout dump, a
  what-will-be-painted display-list dump, an ARIA-snapshot-style diffable
  serialization, and an on-demand GPU screenshot (full + per-node clip) for
  custom-painted content the tree can't express. Pixels are the fallback, not the
  spine.
- **Builds on:** `buiy_verify` pure builders; **`buiy_core::render::golden`'s**
  headless RTT + readback (`capture_to_image`/`readback_rgba_into`), wrapped by
  `buiy_verify::determinism::capture`.
- **Gap:** observation-only, no driving API; `assert_*` wrappers are
  insta-coupled/panic-on-mismatch (agents must call the underlying pure fns); core
  structs aren't `Serialize`; `LayoutEntry`/`collect_layout_entries` are private; the
  screenshot path needs a real wgpu adapter (`#[ignore]` GPU lane); no
  highlight/overlay.

### L4 — LLM-facing transport = MCP (a11y-snapshot-first); BRP as the L0 hatch
- **Role:** adapt L1–L3 into a small, ACI-shaped toolset:
  `buiy_snapshot{subtree?, depth?, mode}`, `buiy_click{ref}`, `buiy_focus{ref}`,
  `buiy_type{ref,text}`, `buiy_press_key`, `buiy_set_value{ref,value}`,
  `buiy_wait_for`; every mutating tool auto-returns a fresh snapshot; the live
  tree/focus as subscribable resources (`buiy://tree`, `buiy://focus`); pixel/vision
  + raw-BRP behind opt-in capability tiers.
- **Builds on:** `accesskit`/`accesskit_winit` already direct deps; `rmcp` as the
  candidate Rust SDK (**pending `cargo deny check` + supply-chain audit — do not
  assert it as "the fit" before the gate the project mandates**).
- **Gap:** no `buiy_mcp` crate / transport / tool schemas / subscribable projection;
  no depth-limit/pagination/concise-mode (unbounded-snapshot risk); **no security
  model**; no docs slot.

---

## The recommendation

1. **Semantic-tree-first, not raw-ECS-first.** Flutter, Compose, Playwright-MCP,
   and the platform AX substrates all converged here; Buiy's inverted-lossiness
   position makes it *more* agent-ready than a browser. Raw BRP hands the agent the
   World and forces it to reverse-engineer UI meaning from component soup — the trap
   `bevy_brp_mcp` bridges fall into. **Buiy already has the meaning.**

2. **Make the tree bidirectional via the existing channel, not a new handler.** Add
   **one** Buiy system in/after `BuiySet::A11yUpdate` that drains `bevy_winit`'s
   `MessageReader<ActionRequestWrapper>`, inverts `node_id_for`
   (`NodeId(n) → Entity::from_bits(n-1)`), and dispatches `accesskit::Action` into
   the real pipelines: `Focus → FocusedEntity`; `Click → OnPress`/activation;
   `SetValue/SetTextSelection/ReplaceSelectedText → TextEditState::apply`. **Do not**
   register a competing `ActionHandler` (the adapter slot is single-occupant). In
   parallel, have each widget **advertise its real action vocabulary** in
   `to_accesskit_node` and enrich nodes with state setters
   (toggled/expanded/selected/disabled/value) backed by new components — this is the
   APG-per-widget contract re-expressed as AX actions, and it simultaneously serves
   real screen readers.

3. **One protocol, two tiers, three clients.** Define a transport-agnostic
   inspect+control protocol over the live tree (React's Bridge/Wall lesson) and
   expose it as (a) an in-process Rust API (the lowest tier — Buiy's tests &
   `buiy_devtools` drive it directly, à la `SemanticsController`/`ComposeTestRule`)
   and (b) a serialized MCP/socket surface for external agents. `buiy_devtools`
   (human) + the in-process test driver + `buiy_mcp` (agent) share **one core**.

4. **MCP primary, BRP debug-tier — both, layered.** A dedicated **in-process**
   `buiy_mcp` plugin (NOT a layer over BRP) because the agent's surface must be the
   semantic tree BRP doesn't model, and in-process avoids the HTTP hop + second
   process the separate bridges pay. Keep BRP's `with_method` for a `debug`-tier
   raw-ECS hatch that composes cleanly.

5. **Borrow Playwright actionability** implemented over the frame loop (poll until
   laid-out/visible/stable/hit-targetable/enabled via `ResolvedLayout` + `hit_test`),
   strict single-match + `force` bypass.

### Rejected alternatives

- **Raw-ECS / BRP-only.** Nearly free, but gives the World not a UI model; text
  content is un-reflected (can't read/type field text); raw writes bypass invariants.
  Keep only as the L0 debug hatch.
- **Pixel / computer-use-only.** Slow, token-heavy, brittle; ~10-100× costlier than
  a tree snapshot; the lossiness critique that would justify it doesn't apply to an
  *authoring* producer. Keep as the L3 fallback for genuinely visual content only.
- **A bespoke non-a11y inspector protocol.** Every engine surveyed minted its own
  wire format and none interoperate. Align with the already-authored AccessKit tree
  + the AccessKit Action contract + established MCP/BRP transports.
- **A competing `ActionHandler` / synthesized OS input.** `bevy_winit` owns the
  single-occupant adapter and already forwards inbound requests; consume that
  channel.
- **Extend `buiy_verify` into the runtime driver.** It's a dev-dependency,
  observation-only, insta-coupled harness — wrong tier. Reuse its pure builders as
  L3 primitives; the driving surface belongs in a new crate sharing the in-process
  core.

---

## Use-case fork (and the library-boundary correction)

The user named two things; they share ~80% of the substrate but pull the
security/default-posture in opposite directions.

- **Dev tooling** (highest-confidence near-term value, least security-sensitive):
  agents *and Buiy's own tests* drive the app during dev/CI to exercise widgets,
  assert transitions, triage visual bugs. Wants maximum reach — the in-process Rust
  API tier, the L0/BRP hatch, full L3 inspection. This is where `buiy_verify`'s
  documented input-replay vision (verification gate #6) and the `accesskit_consumer`-
  driven gates (#3/#4/#7) finally get a driver.
- **Product feature** (the strategic differentiator — but scoped carefully): apps
  *built with* Buiy are agent-drivable out of the box because they ship the
  AccessKit tree. **Library-boundary correction (review):** the foundation README
  non-goals state "data and transport are the consuming app's concern" and exclude
  non-Bevy frontends. So Buiy should **ship the bidirectional semantic tree + the
  in-process inspect/control substrate**; the **transport, auth, and the decision to
  expose end-user agents** belong to the *app* (or an **opt-in companion crate**),
  not foundation. Keep `buiy_mcp` as spec-only / open-question until the security
  model + `cargo deny` audit are done.
- **Shared substrate:** one bidirectional AccessKit tree + one transport-agnostic
  inspect+control protocol. Perception = serialized snapshot (+ optional screenshot);
  locator = the canonical `ref`; control = inbound `Action` routed through the real
  pipelines; actionability over the frame loop. Build the substrate once; fork only
  at transport + capability-gating + security.

---

## Design tensions

- **Generic vs semantic.** Raw ECS is fully general but meaningless to an LLM; the
  AccessKit tree is intent-level but only as complete as the nodes Buiy authors. The
  closed **22-verb** Action enum won't express every app-level transition (open
  *this* panel, run *this* command, drag-reorder) — forcing them through
  `CustomAction` recreates an unstructured surface. Decide per-transition which are
  first-class AX actions vs a richer side channel.
- **Dev-tool vs shipped feature** — pulls security + default-on/off in opposite
  directions (see the fork above).
- **Coupling the agent surface to a11y correctness.** Every a11y gap (flat tree, ~9
  roles, missing state setters, GPU-drawn content with no nodes) is *also* an
  agent-blindness gap. Mostly virtuous (one investment serves both) but agent
  pressure could distort a11y design, and any custom-rendered content is invisible to
  both.
- **Per-frame rebuild instability.** `build_tree` clears nodes each frame; an
  `ActionRequest` targets a `NodeId` captured earlier. `NodeId` stability holds, but
  the handler must tolerate a moved/despawned target (the `xa11y` failure mode).
  Actionability auto-waiting is the mitigation.
- **Multi-window (review — first-class, not deferred).** Today the adapter pushes
  **one shared** `TreeUpdate` (`ROOT_NODE_ID = NodeId(0)`) to **every** window
  (`adapter.rs:66-71`), and `node_id_for` carries **no window discriminator**
  (`translate.rs:11` flags multi-window as v0.x). A ref/snapshot scheme that ignores
  which window a node belongs to breaks the moment window-and-surface design lands —
  design window-aware or explicitly scope to single-window v0.x.
- **Headless asymmetry (review — affects the dev/CI fork most).** The **entire**
  inbound `ActionRequestWrapper` path requires `bevy_winit`'s per-window adapter
  (`ACCESS_KIT_ADAPTERS` thread-local, populated only with real windows). Under
  `MinimalPlugins`/headless — the CI/test fork rated highest-confidence — the channel
  is **never populated**, so a headless driver **cannot** use it and needs a separate
  **in-process injection point** (write `FocusedEntity` / emit `OnPress` / call
  `TextEditState::apply` directly). The two-tier protocol's in-process tier must
  **not** depend on the winit channel.
- **Security / trust of an external driver** — an agent that can
  focus/click/type/set-value (and via L0 mutate arbitrary components) is a powerful
  capability. Needs a deliberate model (loopback-only, auth, capability-allowlist,
  debug-tier gating for L0), not off-by-default disablement nor wide-open BRP.
  Load-bearing for the in/out-of-process split, not a later detail.
- **0.18 → 0.19-rc.3 churn (review — retarget the design base).** The BSN bump
  (wgpu 27→29, **accesskit 0.21→0.24**, +`bevy_scene`) is in flight. *Good news,
  verified:* `bevy_winit-0.19.0-rc.3`'s `poll_receivers` still forwards
  `MessageWriter<ActionRequestWrapper>` — **the inbound seam survives**. But
  **accesskit 0.24's `Action` enum + node setters are the design target**, not a
  post-hoc "re-validate" step; verify each verb
  (`Increment`/`Decrement`/`Expand`/`Collapse`/`SetValue`/`ReplaceSelectedText`)
  exists in the pinned version before listing it as the vocabulary. BRP method names
  already churned (`bevy/*` → `world.*`) — pin and version any Buiy namespace.
- **In-process latency vs out-of-process decoupling** — resolved by the two-tier
  protocol, but design it as one protocol over two transports from day one.

---

## Open questions (for the spec stage)

1. Which transitions are first-class AccessKit Actions vs a separate richer command
   channel? (`CustomAction` with structured data? a generalized `EditCommand`-style
   typed message bus app-wide?)
2. Is `NodeId` (`bits+1`) sufficient as the `ref`, or does Buiy need an
   author-supplied stable **test id** (`testTag`/`ValueKey`) surviving
   i18n/layout/despawn churn? **And reconcile the off-by-one:** `snapshot_tree`
   emits raw `bits` (`a11y.rs:47`) while the inbound `NodeId` is `bits+1` — pick one
   canonical ref and make both emitters agree (a concrete first task).
3. When does the flat tree need real nesting + relations for navigation — and is
   that the same work as the deferred `buiy-accessibility-design` nesting? (Likely
   yes — one investment.)
4. Security/trust model: loopback-only? auth? capability allowlist? debug-tier
   gating for L0/BRP? default-on for the semantic tier on shipped apps, or opt-in?
5. Transport concretely: `rmcp` in-process plugin (stdio + HTTP/SSE) — the
   ECS↔background-task channel-bridge design; honor the BRP port convention
   (`127.0.0.1:15702`) for the debug tier so existing tooling co-exists?
6. Actionability semantics over the frame loop: exact conditions, default timeout,
   strict-single-match, force-bypass, and despawn tolerance.
7. Snapshot scaling: depth-limit / subtree-scope / pagination / concise-vs-detailed
   — needed day one to avoid blowing the context budget on a large UI.
8. Widget-state components (`Pressed`/`Checked`/`Expanded`/`Selected`/`Disabled`/
   `Value`) feeding both AX node setters and inbound handlers — this spec's scope or
   `buiy-accessibility-design`'s? (Coupled.)
9. Re-validate the inbound seam + Action vocabulary + node setters against
   0.19-rc.3 / accesskit-0.24.
10. Does the agent surface need a non-a11y structural locator (CSS-subset selector
    escape hatch) at all, given Buiy's single retained tree?

---

## Recommended next steps

1. **Land a thin vertical spike proving bidirectionality** *before* the full spec:
   one Buiy system reading `MessageReader<ActionRequestWrapper>`, inverting
   `node_id_for`, and honoring exactly `Action::Focus` by writing `FocusedEntity` —
   closing the one action Buiy already *declares* but never honors. **Write the test
   against in-process injection** (push an `ActionRequestWrapper` message directly),
   since headless harnesses have no winit adapter to originate one. De-risks the
   whole router with ~one system.
2. **Second spike:** advertise `Action::Click` on `Button` nodes and route inbound
   `Click` to the existing `OnPress`/activation path; assert the `OnPress` stream
   fires.
3. **Pose the gating decision in the foundation open-questions** (README §5): "Does
   Buiy expose a runtime programmatic introspect+drive surface, and via what protocol
   (MCP / BRP / a11y-tree-over-IPC)?" — currently absent.
4. **Spawn the prior-art folders** (below) so a spec has launchpads.
5. **Write `docs/specs/2026-06-18-buiy-agent-interface-design/`** (run
   `brainstorming` first; review-gate with fresh agents), cementing:
   semantic-tree-first, MCP-primary/BRP-debug-tier, two-tier transport, the
   bidirectional AccessKit router, the per-widget action vocabulary + widget-state
   components, multi-window/headless handling, and the security model.
6. **Re-validate** the inbound seam against the 0.19-rc.3 / accesskit-0.24 branch
   before freezing the router; run `cargo deny check` on `rmcp` before committing to
   it.
7. **Document the verification strategy** (synthetic `ActionRequest` → assert state
   transition) so the `accesskit_consumer`-driven gates inherit it.

---

## Prior-art gaps to promote (per `using-prior-art`)

The corpus has **40 folders but none** on remote/agent/automation protocols. All
six external clusters are load-bearing for the future spec and have no folder.
**Suggested action: spawn `researching-prior-art` for these after the user triages
direction** (not blocking; this report already captures first-pass findings + sources):

- `docs/prior-art/bevy-remote-protocol/` — BRP method catalog, the `bevy/*`→`world.*`
  rename history, port/custom-method seam, the Reflect+serde tax, the inspector/MCP
  ecosystem. The "why not just BRP + custom methods?" contrast. Sibling to `bevy-ui`.
- `docs/prior-art/browser-automation/` — CDP + WebDriver(classic+BiDi) + Playwright:
  stable node ids, a11y-tree-first locators, actionability auto-waiting, ARIA
  snapshots, synthetic-input-through-the-real-path. The decade-deep precedent.
- `docs/prior-art/llm-agent-interface/` — MCP + Playwright-MCP + computer-use +
  `bevy_brp_mcp` bridges + Anthropic ACI discipline. **The user's core interest.**
  *(Note: the 2025-26 MCP layer is fast-moving — capture now, flag time-sensitive.)*
- `docs/prior-art/engine-devtools-protocols/` — Godot remote scene tree, Unity
  PlayerConnection + UI Toolkit Debugger, Unreal Remote Control (read+write+invoke),
  React DevTools (Bridge/Wall, versioned protocol) + Testing Library `getByRole`.
  Anchors the "same substrate, different client" thesis.
- `docs/prior-art/retained-mode-semantics-automation/` — Flutter `SemanticsNode`/
  `SemanticsAction` + Compose `SemanticsProperties`/`SemanticsActions`: the
  one-tree-N-consumers proof, `performAction`/`performSemanticsAction` as the
  AccessKit-`ActionRequest` analog, `testTag`/`ValueKey` locators, the two-tier
  transport, the don't-fragment / don't-gate-behind-debug lessons.
- **Extend** (not create) `docs/prior-art/accesskit/` with `agent-control.md` — the
  bidirectional `ActionRequest` seam, the agent-surface framing (Playwright-MCP /
  xa11y, 10-100× cheaper than screenshots), platform AX as an *automation* (not just
  AT) substrate, the Bevy `bevy_winit` inbound path, and the inverted-lossiness
  insight + honest limits.

---

## Provenance

Research workflow `wf_36c3e416-26d` (14 agents; ~978K subagent tokens). Phases:
6-way internal capability audit (cited to `file:line`) + 6-way external prior-art
sweep (web-researched, sourced) → synthesis → adversarial review with full repo
access. Review verdict **sound-with-corrections**; all 15 load-bearing internal
claims re-verified against the code, corrections folded into this report (crate
attribution of the capture path, the `NodeId`/`bits` off-by-one, the
register_type count 122-not-125, the headless-channel asymmetry, multi-window as
first-class, the 0.19/accesskit-0.24 retarget, and the library-boundary scoping of
the product fork).
