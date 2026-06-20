**Date:** 2026-06-18
**Status:** active
**Subject:** Bevy Remote Protocol (BRP) — folder overview, key facts, reading order

# Bevy Remote Protocol (BRP)

This folder documents **`bevy_remote`**, the in-engine JSON-RPC 2.0 remote
control surface that ships inside the Bevy engine monorepo. BRP lets an external
process — a debugger, an inspector, an MCP server, an AI assistant — read and
mutate a running Bevy app's ECS state over a transport (an in-process channel by
default; HTTP behind a feature). It is the closest thing Bevy has to a built-in
"introspect and drive the running app from outside" protocol, which is exactly
the capability Buiy is weighing for agent perception/control. This folder is an
**index, not a deep dive**; each sibling file owns one slice. It is a sibling to
[`../bevy-ui/`](../bevy-ui/) — BRP is how you'd remotely poke the `bevy_ui`
world that Buiy parallels — and to [`../llm-agent-interface/`](../llm-agent-interface/),
which surveys the MCP bridges that ride BRP today.

## Framing disclosure

These documents are written from Buiy's **AccessKit-semantic-tree-first**
agent-surface stance: the thesis that the AccessKit tree (role + name + state +
actions — the same tree screen readers consume) is the right LLM-agent
perception+control surface, made bidirectional by consuming AccessKit
`ActionRequest`s through the existing `bevy_winit` channel. This folder is a
**learn-from artifact, not a neutral catalog** of BRP: it documents BRP
faithfully and cites primary sources, but it selects and frames what it covers
to answer one question — *what does BRP validate, lend, or warn against for
Buiy's agent surface?* The evidence files (`methods.md`, `transports.md`,
`custom-methods.md`, `ecosystem.md`, `open-problems.md`, `glossary.md`) stay
factual and do not bake in Buiy design decisions; the Buiy verdicts
(validates / borrow / avoid) live only in [`lessons.md`](lessons.md). A neutral
BRP reference would weight transport ergonomics, client libraries, and
debugging workflows differently than this folder does.

All facts here are web-verified as of June 2026; anything we could not source is
marked `(unverified)`.

## Key facts

| Component | What it is | Version / date | Maintainer | License | Repo |
|---|---|---|---|---|---|
| `bevy_remote` | Core BRP plugin (`RemotePlugin`) + JSON-RPC method registry | Landed Bevy **0.15.0** (released 2024-11-29; PR [#14880](https://github.com/bevyengine/bevy/pull/14880) merged 2024-09-23); docs.rs latest **0.18.1** | Bevy Foundation / bevyengine org | MIT OR Apache-2.0 | [bevyengine/bevy](https://github.com/bevyengine/bevy) (`crates/bevy_remote/`) |
| `RemoteHttpPlugin` | HTTP transport (`http` feature, `not(wasm)`); default bind `127.0.0.1:15702` | ships with `bevy_remote` | same | MIT OR Apache-2.0 | same crate (`http` module) |
| bevy_remote_inspector | Web (TS + WebSocket) remote inspector, BRP client | lib.rs shows **v0.1.0 / Bevy 0.15** (likely stale; current status (unverified)) | notmd | MIT | [notmd/bevy_remote_inspector](https://github.com/notmd/bevy_remote_inspector) |
| bevy-inspector-egui | In-engine egui inspector — **in-process, NOT a BRP client** | **0.36.0** (2026-01-14, supports Bevy 0.18) | Jakob Hellermann | MIT OR Apache-2.0 | [jakobhellermann/bevy-inspector-egui](https://github.com/jakobhellermann/bevy-inspector-egui) |
| bevy_brp_mcp | MCP server: AI assistants launch/inspect/mutate Bevy apps over BRP | latest **0.20.0-rc.1** (2026-05-24, Bevy 0.19-rc); latest stable **0.19.0** (2026-03-23, Bevy 0.18) | natepiano | MIT OR Apache-2.0 | [natepiano/bevy_brp](https://github.com/natepiano/bevy_brp) |
| bevy_brp_extras | Plugin adding extra BRP methods incl. `screenshot` | latest **0.20.0-rc.1** (2026-05-24, Bevy 0.19-rc); latest stable **0.19.0** (2026-03-23, Bevy 0.18) | natepiano | MIT OR Apache-2.0 | [natepiano/bevy_brp](https://github.com/natepiano/bevy_brp) |

Bevy **0.19** is in release-candidate stage as of June 2026; treat any
0.19-specific BRP behavior as in-flight `(unverified)`. The `bevy_brp_mcp` and
`bevy_brp_extras` `0.20.0-rc.1` releases track that Bevy 0.19-rc; their
`0.19.0` releases are the latest builds that target stable Bevy 0.18.

## How to use this folder

Read this README for the lay of the land, then jump to the sibling that matches
your question (see the table of contents below). The evidence files
(`methods.md`, `transports.md`, `custom-methods.md`, `ecosystem.md`,
`open-problems.md`, `glossary.md`) are deliberately **factual** — they describe
what BRP *is*, not what Buiy should do. Buiy-specific judgments (what to
validate, borrow, or avoid) live in [`lessons.md`](lessons.md). When a fact is
load-bearing for a decision, trace it back to the cited source rather than
trusting the summary.

## Table of contents

- [`methods.md`](methods.md) — the JSON-RPC method namespace: `world.*`,
  resource methods, `registry.schema`, `+watch` streaming, the 0.15 `bevy/` →
  0.17 dotted-form rename, and per-method semantics.
- [`transports.md`](transports.md) — `RemotePlugin` in-process channel,
  `RemoteHttpPlugin`, the `http` feature gate, default `127.0.0.1:15702`,
  JSON-RPC batch + error model, worked request/response examples, SSE `+watch`.
- [`custom-methods.md`](custom-methods.md) — `with_method` / `RemoteMethods`,
  the handler signature, a worked custom-handler example, and the seam for
  app-defined endpoints. **The high-value path for Buiy.**
- [`ecosystem.md`](ecosystem.md) — inspectors, the `bevy_brp` MCP family,
  archived repos, and what is/isn't a BRP client.
- [`open-problems.md`](open-problems.md) — the reflection tax, raw-ECS vs
  semantic-model gap, security/auth posture, schema discovery limits.
- [`lessons.md`](lessons.md) — implications for Buiy (validates / borrow /
  avoid).
- [`glossary.md`](glossary.md) — BRP / JSON-RPC / reflection terms.

## Glossary stub

Full definitions live in [`glossary.md`](glossary.md). Quick anchors:

- **BRP** — Bevy Remote Protocol; JSON-RPC 2.0 over a transport into a running
  Bevy app.
- **`RemotePlugin`** — core plugin; registers methods, processes requests on an
  in-process channel, adds no transport.
- **Reflection tax** — only `Reflect`-registered, serde-serializable types in
  the `AppTypeRegistry` are visible/editable over BRP `(exact reflect-trait
  list unverified)`.
- **`registry.schema`** — endpoint exposing the type registry's JSON schema.

## Canonical reading order

1. **README.md** (this file) — orientation + key facts.
2. [`methods.md`](methods.md) — what BRP can do today.
3. [`transports.md`](transports.md) — how requests get in.
4. [`custom-methods.md`](custom-methods.md) — the extension seam.
5. [`open-problems.md`](open-problems.md) — where raw-ECS BRP falls short.
6. [`ecosystem.md`](ecosystem.md) — who builds on it.
7. [`lessons.md`](lessons.md) — what it means for Buiy.
8. [`glossary.md`](glossary.md) — reference as needed.

## Why this matters for Buiy

Buiy is a Bevy 0.18 ECS UI framework parallel to `bevy_ui`. Because BRP is an
in-engine plugin, Buiy could enable a **DEBUG-tier** remote surface nearly for
free by adding `RemotePlugin` (+ `RemoteHttpPlugin`). But the default methods
expose **raw ECS components** — entity ids, `Reflect`-gated component blobs —
not a semantic UI model an agent can reason over. The reflection tax also means
half of Buiy's state may be invisible until annotated. The high-value seam is
**custom methods** (see [`custom-methods.md`](custom-methods.md)) layered over
Buiy's existing **AccessKit** semantic tree (role + name + state + actions) —
the same tree screen readers consume — rather than over raw components. That
distinction is the thread [`lessons.md`](lessons.md) develops, alongside the
MCP-bridge survey in [`../llm-agent-interface/`](../llm-agent-interface/).

## Sources

- https://docs.rs/bevy/latest/bevy/remote/index.html
- https://docs.rs/bevy/latest/bevy/remote/struct.RemotePlugin.html
- https://github.com/bevyengine/bevy/pull/14880
- https://bevy.org/news/bevy-0-18/
- https://github.com/notmd/bevy_remote_inspector
- https://github.com/jakobhellermann/bevy-inspector-egui
- https://crates.io/crates/bevy-inspector-egui
- https://github.com/natepiano/bevy_brp
- https://crates.io/crates/bevy_brp_mcp
- https://crates.io/crates/bevy_brp_extras
