**Date:** 2026-06-18
**Status:** active
**Subject:** Bevy-ecosystem MCP bridges (bevy_brp_mcp / bevy_mcp / bevy_debugger_mcp) — how they bridge MCP↔BRP as a separate process exposing raw ECS, and why a Buiy agent surface should be a dedicated in-process plugin over the AccessKit semantic tree instead.

# Bevy MCP bridges — BRP-vs-dedicated-plugin

This file surveys the existing way agents drive a *Bevy* app today (an MCP server in front of the Bevy Remote Protocol), and contrasts that architecture with the surface Buiy already has. For the MCP wire protocol itself see [mcp-protocol.md](mcp-protocol.md). The Bevy Remote Protocol (BRP) is the transport these bridges sit on; it is Bevy's built-in `RemotePlugin`/`RemoteHttpPlugin` JSON-RPC-over-HTTP interface (a dedicated BRP prior-art folder does not exist yet, so BRP is documented inline here from the Bevy docs in Sources).

## The existing bridges

All of these are *external MCP servers* that translate MCP tool calls into BRP requests against a separately-running Bevy app.

- **bevy_brp_mcp** — the established one. Crate `bevy_brp_mcp` on crates.io, **v0.19.0** (2026-03-23), dual-licensed **MIT OR Apache-2.0**, maintainer **natepiano**. The original standalone repo `natepiano/bevy_brp_mcp` is **ARCHIVED**; live code now lives in the monorepo **https://github.com/natepiano/bevy_brp** (crate at `mcp/`). It pairs with a companion in-app plugin **`bevy_brp_extras`** (`BrpExtrasPlugin`), which registers extra BRP methods on top of Bevy's own `RemotePlugin`. (Staleness note: crates.io now lists **0.20.0-rc.1** (2026-05-24) as the latest version; the repo README anticipates a Bevy 0.19 / bevy_brp_mcp 0.20.0 pairing under independent versioning. The 0.19.0 facts here remain correct; bump the "latest" note as the 0.20.0 line stabilizes.)
- **bevy_mcp** — https://github.com/Nub/bevy_mcp (maintainer "Nub"). A smaller, separate MCP↔BRP bridge ("MCP server for Bevy Remote Protocol"). Version/license **(unverified)**.
- **bevy_debugger_mcp** — https://github.com/ladvien/bevy_debugger_mcp, crate `bevy_debugger_mcp` (seen at **v0.1.2**, license **(unverified)**). Framed as "a Bevy agentic debugger": observe entities/components/resources, run experiments with automatic rollback, performance/anomaly analysis, session recording, screenshots. Debug-oriented rather than a UI-control surface.

There is also an unrelated `rltvty/bevy-mcp` that helps an LLM *edit Bevy source code* (a coding assistant), not drive a running app — out of scope here.

## How the architecture actually works

The shape is the same across bevy_brp_mcp / bevy_mcp:

```
Agent (MCP client) --stdio--> MCP server (separate OS process)
                                   |  translates tool call -> JSON-RPC
                                   v
                          HTTP POST :15702  (BRP, JSON-RPC over HTTP)
                                   |
                                   v
                          Running Bevy app + RemotePlugin / RemoteHttpPlugin
```

Two process boundaries, two protocol hops. The agent speaks MCP over stdio to the bridge; the bridge speaks **BRP** — JSON-RPC 2.0 over HTTP, default port **15702** — to the Bevy app, which must have Bevy's `RemotePlugin` + `RemoteHttpPlugin` added. BRP is request/response and client-initiated; the app only answers. The bridge does *not* live inside the app; it is a standalone binary you launch alongside it (bevy_brp_mcp can even discover and launch the app for you, capturing logs to a temp dir).

## What the agent sees: raw ECS, not a UI model

The tools these bridges expose are the BRP method set, i.e. the **raw ECS world**: `bevy/query` (entities by component filter), `bevy/get` / `bevy/list` components, `bevy/spawn`, `bevy/destroy`, `bevy/insert`, `bevy/remove`, `bevy/mutate_component`, resource get/insert/list/mutate, `bevy/reparent`, plus watch (streaming) variants. `bevy_brp_extras` adds input/IO custom methods (screenshots, keyboard/mouse injection, window management, graceful shutdown, FPS diagnostics) — still expressed as raw remote calls. bevy_brp_mcp wraps these in tools like `bevy_query`, `bevy_get`, `world_mutate_components`, `brp_type_guide` (which exists specifically because agents otherwise cannot guess the JSON shape of a reflected component).

The consequence is the central trap of this approach:

> **Raw components are not a semantic UI model.** What comes back is a flat soup of `Transform`, `Node`, `BackgroundColor`, `Text`, `Interaction`, plus every gameplay component, keyed by opaque `Entity` ids. Nothing says "this is a button labeled *Save*, enabled, that responds to a press." To recover a *role*, the agent must reverse-engineer it — walk `ChildOf`/`Children` to find the text under a clickable node, infer that `Interaction` + a `Button` marker means clickable, infer disabled-ness from the absence or value of some component, and guess at the accessible name. That inference is brittle, version-specific, and re-derived on every call. The `brp_type_guide` tool is itself evidence of the gap: the model can't even serialize a component correctly without a generated schema.

This is the same complaint [playwright-mcp.md](playwright-mcp.md) answers for the web: Playwright-MCP feeds the agent the **accessibility snapshot** (role + name + state, the semantic tree), *not* the raw DOM, precisely because raw nodes force the model to reconstruct meaning. BRP hands the agent the "DOM" equivalent and stops there.

## Why Buiy should NOT layer over BRP

Buiy is unusual: it **already authors an AccessKit semantic tree** (role + name + value + state + supported actions) for every widget — the same tree a screen reader consumes — today output-only. That changes the calculus completely versus a generic Bevy app:

- **Semantic, not raw.** The AccessKit node *is* the role/name/state model the BRP approach has to reconstruct by hand. A Buiy agent surface starts from meaning, not from `Entity` + component soup. No `ChildOf` walking, no `Interaction`-marker inference, no per-version component schemas.
- **In-process, no HTTP hop, no second process.** On a Buiy-owned window Buiy owns its **own** `accesskit_winit::Adapter` (keyed by winit `WindowId`); incoming `ActionRequest`s are routed to Buiy entities through Buiy's own `ActionHandler::do_action` plumbing — bevy_winit's accessibility path is not running on those windows (see [../accesskit/](../accesskit/) and Buiy's `architecture.md` §2.6). That same `ActionHandler` is how a screen reader clicks a button today. A dedicated in-process plugin reuses that channel for the agent — zero added processes, zero `:15702` HTTP round-trip, zero JSON-RPC-over-HTTP serialization tax, and the action executes through the *same* code path real assistive tech uses (so it's exercised, not a bypass).
- **One surface for AT and agents.** Screen readers and LLM agents become the same kind of client of the same tree. Perception (read the tree) and control (dispatch an `ActionRequest` through Buiy's `ActionHandler`) are unified, instead of bolting a parallel raw-ECS control plane onto the side.

**Keep BRP — but only as a debug hatch.** BRP / custom methods remain genuinely useful for *developer* debugging of a Buiy app (inspect arbitrary components, mutate state, drive the experiment-with-rollback workflows bevy_debugger_mcp pioneered). The recommendation is a tiering, not a rejection: the **agent UI-control surface** is the dedicated AccessKit plugin (semantic, in-process); **raw BRP** stays available as a lower, opt-in *debug tier* for when you deliberately need to poke the ECS, not as the primary way an agent perceives or operates the UI. Mixing the two tiers — letting the agent mutate `BackgroundColor` directly instead of invoking the button's action — is exactly the abstraction leak to avoid.

The lessons file records this as a borrow/avoid: **borrow** Playwright-MCP's accessibility-snapshot-first model and BRP's debug-tier `type_guide` idea; **avoid** the raw-ECS-as-UI-model architecture and the separate-process HTTP bridge. See [lessons.md](lessons.md) for the validates/avoid/borrow breakdown and [aci-tool-design.md](aci-tool-design.md) for how the tool roster over the semantic tree should be shaped.

## Sources

- https://github.com/natepiano/bevy_brp — live bevy_brp monorepo (mcp/ crate), maintainer, license, version, architecture
- https://crates.io/crates/bevy_brp_mcp/versions — crate versions (0.19.0 2026-03-23; latest 0.20.0-rc.1 2026-05-24; MIT OR Apache-2.0)
- https://github.com/Nub/bevy_mcp — bevy_mcp bridge
- https://github.com/ladvien/bevy_debugger_mcp — bevy_debugger_mcp agentic debugger
- https://crates.io/crates/bevy_debugger_mcp/0.1.2 — bevy_debugger_mcp version
- https://github.com/rltvty/bevy-mcp — unrelated code-editing MCP (scoped out)
- https://docs.rs/bevy/latest/bevy/remote/index.html — Bevy Remote Protocol (RemotePlugin, JSON-RPC over HTTP)
- https://docs.rs/bevy/latest/bevy/remote/http/index.html — RemoteHttpPlugin, default port 15702
- https://github.com/bevyengine/bevy/blob/main/crates/bevy_remote/src/lib.rs — BRP method definitions (bevy/query, bevy/get, mutate, etc.)
