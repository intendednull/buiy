# buiy_mcp — opt-in networked transport (Phase 2)

**Date:** 2026-06-18
**Status:** draft

`buiy_mcp` is a **new, opt-in companion crate** exposing Buiy's inspect-and-control surface to external LLM agents over a network socket using the Model Context Protocol (MCP). It is **transport only**: a thin adapter running the *exact same* in-process contract ([inprocess-api.md](./inprocess-api.md)) over a wire, never a parallel model.

> **Phase 1 ships ZERO of this.** The entire substrate ([semantic-tree.md](./semantic-tree.md), [action-router.md](./action-router.md), [widget-contracts.md](./widget-contracts.md), [inprocess-api.md](./inprocess-api.md)) lands and is fully verified ([verification.md](./verification.md)) with no transport, no security, no networking dependency. `buiy_mcp` is the Phase 2 deliverable in [phasing.md](./phasing.md), gated on user go-ahead.

> **Prior-art status.** The transport-side references in this file (MCP spec + `rmcp` SDK, Playwright-MCP, the React DevTools Bridge/Wall, the Bevy Remote Protocol) are **named in prose, not cited to `docs/prior-art/` files** — those folders do not exist yet ([README.md](./README.md) research-debt note). Run `researching-prior-art` to create them **before** this crate is built, so the transport design inherits a grounded corpus. The semantic vocabulary the wire stays aligned to IS grounded: [../../prior-art/accesskit/capabilities.md](../../prior-art/accesskit/capabilities.md), [../../prior-art/accesskit/lessons.md](../../prior-art/accesskit/lessons.md), [../../prior-art/wai-aria-apg/](../../prior-art/wai-aria-apg/).

---

## 1. Why a new crate, and why this boundary

### 1.1 New crate, consistent with the modular/opt-in principle

`buiy_mcp` is **not** in the foundation crate inventory (`docs/specs/2026-05-07-buiy-foundation/`, §2.8). Introduced here, consistent with foundation §2.8's "modular subsystems, opt-in surface area." Networking/persistence/security is foundation **non-goal #1**. A crate shipping a socket server, auth, and capability gating is exactly the surface foundation pushes to an opt-in seam. An app takes on `buiy_mcp` only when it wants a networked agent plane.

### 1.2 The Bridge/Wall split

The architecture borrows the **React DevTools Bridge/Wall split**: the inspect+control *protocol* is defined once against an abstract `Wall` (a bidirectional channel); the same `Bridge` logic runs over `postMessage`, a WebSocket, or a native socket. Buiy applies the identical inversion:

- The contract is defined once in `buiy_core::a11y::inprocess` over an **in-process channel** (`&mut World`/`&mut App`). Phase 1. See [inprocess-api.md](./inprocess-api.md).
- `buiy_mcp` is the *second* `Wall`: it runs the unchanged `snapshot`/`perform`/`get_by_role`/`wait_for`/`act_when_actionable` over a socket with an MCP envelope.

One semantic model (the decomposed AccessKit tree, [semantic-tree.md](./semantic-tree.md)) and one serialization (`SemanticTree`, [inprocess-api.md](./inprocess-api.md)). `buiy_mcp` adds a transport envelope, not a new node model, ref scheme, or action vocabulary — the explicit rejection of the flutter_driver/integration_test/MCP fragmentation. One contract, N consumers.

### 1.3 Semantics align to AccessKit; envelope aligns to MCP

- **Semantics** (what a node is, what a verb means) stay aligned to AccessKit 0.24 — the same roles/states/`Action` verbs an AT sees ([../../prior-art/accesskit/capabilities.md](../../prior-art/accesskit/capabilities.md), [../../prior-art/wai-aria-apg/](../../prior-art/wai-aria-apg/)).
- **Envelope** (wire framing) aligns to MCP: tools declared as MCP tools with JSON-Schema input; results are MCP tool results; subscriptions/settles use MCP's own mechanisms. No bespoke RPC framing.

Target: **MCP spec revision 2025-06-18**, official Rust SDK **`rmcp`**. The tool surface mirrors Playwright-MCP's role-addressed, locator-style design: snapshot-first, address-by-role+name, act-then-observe.

---

## 2. MCP tool surface

A small flat set; every tool is a *thin* projection of an `inprocess` op — one generic `perform`, the rest sugar compiling to `inprocess::perform`/`act_when_actionable`.

| Tool | Inputs | Lowers to | Notes |
|---|---|---|---|
| `snapshot` | `view: "unmerged"\|"merged"` (default unmerged) | `snapshot(world, TreeView)` | compact role+name+state+actions+ref tree |
| `perform` | `ref`, `action`, `data?` | `perform(world, action, ref, data)` | the single generic primitive |
| `click` | `ref` | `perform(.., Click, ..)` | sugar |
| `type` | `ref`, `text` | `perform(.., SetValue, Value(text))` | sugar; lowers through `EditCommand` |
| `set_value` | `ref`, `value` | `perform(.., SetValue, ..)` | sugar; numeric/text role-dispatched |
| `focus` | `ref` | `perform(.., Focus, ..)` | sugar |
| `expand` | `ref`, `expanded` | `perform(.., Expand\|Collapse, ..)` | sugar |
| `get_by_role` | `role`, `name?`, `state?` | `get_by_role(..)` | strict single-match; ambiguity is a loud error |
| `wait_for` | `condition` | `wait_for(app, condition)` | semantic, never a pixel diff |
| `increment`/`decrement`/`set_selection` | `ref` (+data) | `perform(.., Increment\|Decrement\|SetTextSelection, ..)` | added 1:1 as the sugar set grows |

Control tools carry the **post-action `SemanticTree`** inline (auto-re-snapshot, [inprocess-api.md](./inprocess-api.md)) — act-then-observe in one round-trip. `ref` on the wire is the canonical AccessKit `NodeId` (`entity.to_bits()+1`), identical to the in-process `ref` and to what `buiy_verify::a11y::snapshot_tree` emits after the Phase 0 off-by-one fix. No new addressing. Author-supplied test-ids over the ref are a Phase-2 follow-up (§6), layered on top.

### 2.1 Errors

The typed `ActionError` ([action-router.md](./action-router.md): `NotFound`/`Unsupported`/`NotActionable`/`BadData`) maps to MCP tool errors with the guidance string intact. Stale-ref/not-actionable/ambiguous is loud; a not-yet-ready actionability condition is retried silently by `act_when_actionable` before any error. The MCP layer never invents an outcome the in-process contract didn't produce.

---

## 3. Change-detection push, not polling

Naïve MCP re-`snapshot`s on a timer. `buiy_mcp` **pushes tree deltas** to subscribed clients via Bevy change detection — the React DevTools Bridge model (the app emits transitions; the frontend doesn't poll). A plugin system runs after `BuiySet::A11yUpdate` ([action-router.md](./action-router.md) §7), observing `Changed<A11y*>` (or diffing the just-built `TreeUpdate`), enqueuing a delta onto each subscriber's channel. The delta reuses the same `SemanticTree` serialization, scoped to the changed subtree. A subscription is an MCP capability opted into at handshake (§4). Lines up with the foundation's eventual lazy-`TreeUpdate`-diff gated on `AccessibilityRequested`.

---

## 4. MCP Tasks for async / animated settles

Some actions don't settle in one frame (animated disclosure, layout transition, multi-frame `wait_for`). `buiy_mcp` uses **MCP Tasks** (call-now/fetch-later): a tool that may not settle promptly returns a task handle; the app keeps stepping (`act_when_actionable`/`wait_for` over real frames, condition-polling, never a sleep); the client fetches the settled post-state `SemanticTree` when it completes. Maps Buiy's frame-loop settling to MCP's async primitive rather than holding a synchronous socket across an animation.

---

## 5. What this crate owns — everything the foundation excludes

The home for all of foundation non-goal #1 (networking/persistence/security).

### 5.1 Networking
`rmcp`-backed MCP server, listener, framing. Transport bindings (WebSocket/WebTransport/WebRTC) behind crate features. None in `buiy_core` — it stays winit-free *and* transport-free.

### 5.2 Capability-tier gating
A `--caps`-style tier model (the Playwright-MCP capability-flags analog) enforced at the MCP boundary **before** any call reaches `inprocess`:

- **Safe default (always on):** role/name/state **inspect** + subscribe, plus the **APG verbs** a widget advertises through its `A11yContract` ([widget-contracts.md](./widget-contracts.md)) — `Click`, `Focus`, `Blur`, `Expand`/`Collapse`, `Increment`/`Decrement`, `SetValue` on enabled editable nodes. Safe because it's exactly what an AT may already do.
- **Opt-in (off by default):** pixel readback (the only path to pixels — the semantic tree never carries them); raw-key (synthesizing raw key events instead of the advertised APG verb); destructive actions (app-flagged verbs, §5.5).

The tier gate is **on top of** the substrate's two-layer capability model (role-static advertisement + the router's live per-instance filter re-reading `A11yDisabled`/`A11yReadOnly`/`A11yValue`, [action-router.md](./action-router.md) §3). The substrate already refuses `SetValue` against a now-read-only field; the MCP tier gate is a coarser deployment-level policy in front of that — defense in depth.

### 5.3 Versioned handshake
Client/server agree at connect on the MCP revision (2025-06-18), the semantic-tree schema version, and the granted tiers. Mismatch rejected at the handshake. The in-process contract has no version wire — it's a direct call.

### 5.4 Authentication
Client auth on the socket — wholly `buiy_mcp`'s. The substrate has no principal; in-process callers (the test driver, `buiy_verify`) are trusted by construction. Auth exists only because the transport opens the contract to untrusted clients.

### 5.5 The structured app-verb RPC lane
The substrate's app-verb channel is `Action::CustomAction(i32)` via a `CustomActionRegistry` ([action-router.md](./action-router.md)). 0.24 `CustomAction` carries **only an i32 index** — an honest ceiling. `buiy_mcp` escapes it with a **structured-app-verb RPC lane**: an MCP tool carrying real **name + typed args** (a JSON object), resolved server-side to the app's verb handler. The *only* place structured app verbs exist (forcing name+args through the i32 schema would be dishonest). Gated by the destructive-actions tier (§5.2) when flagged destructive; shares the same act-then-observe contract.

---

## 6. Named Phase-2 follow-ups carried by this crate

Per [phasing.md](./phasing.md): **author-supplied test-ids over the NodeId ref** (a human-stable layer above the session-stable NodeId, the answer to `get_by_role`'s ambiguity error); **multi-window per-`WindowId` tree keying** (Phase 1 is single-window-scoped — `ROOT_NODE_ID` has no window discriminator); **richer `owns`-edge cases** and **lazy `TreeUpdate` diffing gated on `AccessibilityRequested`** (the latter feeds the push-delta layer §3).

---

## 7. The Bevy Remote Protocol is a debug hatch, not the agent plane

Bevy ships its own **Bevy Remote Protocol (BRP)** — a JSON-RPC surface for querying/mutating ECS components/entities remotely. Tempting to route agents through it; we **do not**. BRP operates on **raw ECS components by reflection** — an engine-internal debug surface with no semantic model, no role/name/state, no APG contract, no capability tiering. An agent over BRP would poke implementation components, couple every script to Buiy's internal layout, and bypass the live filter. BRP's role here is a **debug-tier hatch** (inspect/mutate raw ECS while debugging the substrate/transport), explicitly **not the agent plane**. The agent plane is the semantic tree through `buiy_mcp`, which never dumps the raw `World` (the AccessKit tree IS the pre-filtered allowlist, [inprocess-api.md](./inprocess-api.md)). If a deployment enables both, they are two surfaces for two audiences, configured independently.

---

## 8. References

Grounded prior-art (real folders): [../../prior-art/accesskit/capabilities.md](../../prior-art/accesskit/capabilities.md), [../../prior-art/accesskit/lessons.md](../../prior-art/accesskit/lessons.md), [../../prior-art/accesskit/tree-model.md](../../prior-art/accesskit/tree-model.md), [../../prior-art/wai-aria-apg/](../../prior-art/wai-aria-apg/) — the semantic vocabulary the wire stays aligned to.

Transport references (named in prose; `docs/prior-art/` folders to be created via `researching-prior-art` before this crate is built, per [README.md](./README.md)): the MCP spec revision 2025-06-18 + the `rmcp` Rust SDK + MCP Tasks; Playwright-MCP (snapshot-first, role+name addressing, `--caps`, act-then-observe, re-run-the-locator retry); the React DevTools Bridge/Wall split (contract once, run over any channel; push deltas not poll); the Bevy Remote Protocol (engine-internal debug over raw ECS, NOT an agent plane).

Sibling spec files: [README.md](./README.md) · [semantic-tree.md](./semantic-tree.md) · [action-router.md](./action-router.md) · [widget-contracts.md](./widget-contracts.md) · [inprocess-api.md](./inprocess-api.md) · [verification.md](./verification.md) · [phasing.md](./phasing.md).
