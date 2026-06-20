**Date:** 2026-06-18
**Status:** active
**Subject:** Bevy Remote Protocol — lessons for Buiy (validates / borrow / avoid); the decision file

# Lessons for Buiy — Bevy Remote Protocol

This is the decision file. BRP is a real, shipping, Bevy-native pattern for live
inspect/mutate of a running ECS app over a socket. It validates some of Buiy's
instincts and supplies concrete pieces to borrow — but its *raw-ECS surface* is
the wrong agent UI plane for an AccessKit-first framework. Net thesis below.

For the Buiy framing (AccessKit semantic tree as the agent perception+control
surface), see [`../accesskit/`](../accesskit/) and the MCP-bridge survey in
[`../llm-agent-interface/`](../llm-agent-interface/). For the BRP facts these
lessons rest on, see [`methods.md`](./methods.md),
[`transports.md`](./transports.md), [`custom-methods.md`](./custom-methods.md),
and [`open-problems.md`](./open-problems.md).

## Validates

- **In-process server over a socket is a proven Bevy-native pattern.** BRP runs
  as a plugin (`RemotePlugin`) processing JSON-RPC over an in-process channel,
  with HTTP added by `RemoteHttpPlugin` — no separate daemon, no out-of-band
  state. Buiy can host its own agent endpoint the same way. See
  [`transports.md`](./transports.md).
- **The custom-method seam is the right place for UI-semantic verbs.**
  `with_method(name, handler)` lets a project add domain verbs that run with
  `&mut World` access, so a verb like "activate the button named Save" is
  expressible as a first-class method rather than a raw component edit. This is
  exactly where Buiy's AccessKit-action verbs belong. See
  [`custom-methods.md`](./custom-methods.md).
- **Self-description is expected of this class of endpoint.** `registry.schema`
  (Bevy 0.16, issue #16745) and `rpc.discover` (OpenRPC) let a client enumerate
  types and methods at runtime. An agent endpoint should likewise be
  introspectable — list available roles/actions, not require a hard-coded
  client. See [`methods.md`](./methods.md).
- **Live observation via streaming works.** The `+watch` variants
  (`world.get_components+watch`, `world.list_components+watch`) stream change
  notifications over an SSE response (`text/event-stream`). Live observation of a
  UI tree is a solved shape, not a research problem. See [`methods.md`](./methods.md)
  and [`transports.md`](./transports.md).
- **Granular mutation by reflection path is viable.** `world.mutate_components`
  sets a single field inside a component by `GetPath`-style path (entity +
  component type + field path + value), rather than replacing whole components.
  Fine-grained, targeted mutation over a wire is a shipped capability. See
  [`methods.md`](./methods.md).

## Borrow

- **The custom-method seam as a debug-tier raw-ECS hatch.** Adopt
  `with_method` (or an equivalent) so Buiy can expose raw-ECS inspect/mutate for
  *debugging the framework itself* — distinct from, and beneath, the agent's
  semantic plane. See [`custom-methods.md`](./custom-methods.md).
- **The `127.0.0.1:15702` port convention.** BRP's default bind is loopback
  `127.0.0.1:15702`. Reusing it lets existing BRP tooling (inspectors,
  `bevy_brp_mcp`) co-exist with / point at a Buiy app for low-level debugging.
  See [`transports.md`](./transports.md) and [`ecosystem.md`](./ecosystem.md).
- **`registry.schema` / `rpc.discover` self-description.** Borrow the
  introspection endpoints so a client can discover Buiy's exposed types and
  verbs without a baked-in contract.
- **The `+watch` streaming model for live observation.** Borrow the SSE streaming
  shape for an agent watching the semantic tree change (focus moved, a node's
  state flipped) rather than polling. *Name the scaling assumption*: the diff is
  recomputed per Bevy tick per subscription (BRP states no backpressure or cap —
  [`open-problems.md`](./open-problems.md) §8), so Buiy's tree-update stream must
  bound work by coalescing per-frame deltas and capping concurrent watchers, not
  inherit an unbounded per-tick cost silently.
- **Reflection-path granular mutation, where a raw edit is genuinely needed.**
  When a debug verb must poke one field, the `mutate_components` path model is
  the right granularity — not whole-component replacement.
- **Versioned, enumerable verbs — `rpc.discover` over a frozen vocabulary.**
  BRP's `bevy/*` → `world.*` churn (no alias shim; clients silently broke —
  [`open-problems.md`](./open-problems.md) §5) is the cautionary tale. Concrete
  recommendation for Buiy's own endpoint: (a) carry an explicit protocol/version
  field in the envelope or expose it via a discovery method; (b) make the verb
  set **enumerable** (a `discover`-equivalent) so clients query the vocabulary
  rather than hard-coding it; (c) when a verb's name or shape must change,
  *add the new verb and keep the old one as a documented alias for at least one
  release* rather than renaming in place. The AccessKit action vocabulary
  (`Default`, `Focus`, `SetValue`, …) is itself a stable, externally-defined
  enum, which sidesteps most ad-hoc verb churn — lean on it instead of inventing
  parallel verbs.

## Avoid

- **Do NOT make raw ECS the agent's UI surface.** Over BRP an agent sees
  component soup — it must reverse-engineer "this is a button", "this is its
  label", "this is focused" from arbitrary component types and field names.
  Buiy already *has* the answer: an AccessKit semantic tree of
  role + name + state + actions, the same tree screen readers consume. Exposing
  raw ECS would force the agent to re-derive semantics Buiy authored on purpose.
  See [`../accesskit/`](../accesskit/) and
  [`../llm-agent-interface/`](../llm-agent-interface/).
- **Do NOT rely on BRP for text content.** BRP only sees components that are
  `Reflect`-registered and serde-serializable in the `AppTypeRegistry`; types
  lacking that are invisible (the "reflection tax"). Buiy's text/editor internals
  are deliberately un-reflected, so an agent reading the ECS would not see what
  the user sees. The painted text lives in the semantic tree's `name`/`value`,
  not in a queryable component. See [`open-problems.md`](./open-problems.md).
- **Do NOT assume a query reflects what is painted.** A `world.query` returns
  ECS state, which can diverge from the rendered/laid-out result — components set
  this frame but not yet laid out, retained paint state, GPU residue. "What the
  ECS holds" is not "what is on screen." The semantic tree (built from the
  computed UI) is the surface that tracks the painted result; BRP queries are a
  snapshot of intent, not output. See [`open-problems.md`](./open-problems.md).
- **Do NOT treat resource/event reflection as a free win for agents.** Resource
  methods and `world.trigger_event` (Bevy 0.18, PR #21798) are powerful but
  raw-ECS-shaped and gated by the same reflection tax; they belong in the debug
  tier, not the agent's perception loop.

## Net

**BRP is a debug-tier escape hatch, not the primary agent UI plane.** Buiy
should host a BRP-shaped endpoint (loopback, JSON-RPC, custom-method seam,
self-description, `+watch`) for *framework debugging* and co-existence with
existing Bevy tooling — and it should keep that plane firmly beneath the
AccessKit semantic tree. The agent perceives and controls Buiy through the
semantic tree (role/name/state/actions), made bidirectional by consuming
AccessKit `ActionRequest`s through the existing `bevy_winit` channel; it drops
to raw ECS only to debug the framework, never to drive the UI. BRP shows the
pattern is sound and hands Buiy reusable parts — but the raw-ECS *surface* is
exactly what an AccessKit-first framework exists to replace.

## Sources

- https://docs.rs/bevy/latest/bevy/remote/index.html
- https://github.com/bevyengine/bevy/pull/14880
- https://github.com/bevyengine/bevy/pull/19377
- https://github.com/bevyengine/bevy/issues/16745
- https://gist.github.com/coreh/1baf6f255d7e86e4be29874d00137d1d
- https://docs.rs/bevy/latest/bevy/ecs/prelude/struct.AppTypeRegistry.html
- https://bevy.org/learn/migration-guides/0-16-to-0-17/
