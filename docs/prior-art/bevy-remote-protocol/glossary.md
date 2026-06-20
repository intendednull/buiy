**Date:** 2026-06-18
**Status:** active
**Subject:** Bevy Remote Protocol — glossary of BRP terms, types, methods, and ecosystem names

# Glossary — BRP terms

Concise definitions for the terms used across this folder. One to two lines each.
See [README.md](./README.md) for orientation, [methods.md](./methods.md) for the
full method catalog, [transports.md](./transports.md) for the channel/HTTP layer,
[custom-methods.md](./custom-methods.md) for extension, and
[ecosystem.md](./ecosystem.md) for the external tools named below.

## Core protocol

- **BRP (Bevy Remote Protocol)** — Bevy's built-in protocol for inspecting and
  mutating a running app's ECS world from an external process. Landed in Bevy
  0.15.0 (released 2024-11-29; initial impl PR #14880 merged 2024-09-23).
  Request/response shape is JSON-RPC 2.0.

- **`bevy_remote`** — the engine crate (`crates/bevy_remote/` in the bevyengine
  monorepo, MIT OR Apache-2.0) implementing BRP: request types, built-in method
  handlers, and the plugins below. See [README.md](./README.md).

- **JSON-RPC (2.0)** — the wire format BRP rides on. Each request carries `id`,
  `method`, and optional `params`; the response carries `id` plus `result` or
  `error` (an object with `code` / `message` / optional `data`). The module doc
  states "The Bevy Remote Protocol is based on the JSON-RPC 2.0 protocol." The
  HTTP transport also accepts JSON-RPC **batch** arrays. See
  [transports.md](./transports.md) for the error-code bands.

- **`RemotePlugin`** — the core plugin. Wires BRP method handlers and processes
  requests over an **in-process channel**; adds **no transport** by itself. Also
  the builder you call `.with_method(...)` on. See [transports.md](./transports.md).

- **`RemoteHttpPlugin`** — `bevy_remote::http::RemoteHttpPlugin`, behind the
  `http` cargo feature (gated `not(target_family = "wasm")`). Exposes BRP over
  HTTP; default bind **127.0.0.1:15702**. The transport most clients use. See
  [transports.md](./transports.md).

## Built-in methods (current dotted form, Bevy 0.17+)

Methods were renamed from the original `bevy/` prefix to a dotted namespace in
Bevy 0.17 (PR #19377, merged 2025-07-29). Full catalog + old→new mapping in
[methods.md](./methods.md).

- **`world.query`** — query entities by component filters; returns matching
  entities plus requested component data. Was `bevy/query` (0.15).

- **`world.get_components`** — fetch named components from one entity (entity id +
  list of fully-qualified component type names). Was `bevy/get` (0.15).

- **`world.mutate_components`** — set a **single field within a component** by
  reflection path: entity id + component type name + field path + new value.
  Was `bevy/mutate`.

- **`registry.schema`** — returns JSON-Schema-shaped descriptions of
  Reflect-registered types from the `AppTypeRegistry`. Added Bevy 0.16 as
  `registry/schema` (tracking issue #16745); dotted in 0.17.

- **`rpc.discover`** — OpenRPC service-discovery method: lists the methods the
  server exposes. Name unchanged across the 0.17 rename.

- **`+watch` (e.g. `world.get_components+watch`, `world.list_components+watch`)**
  — streaming variants that keep the connection open and push a per-tick diff
  whenever the watched data changes. Over HTTP this rides a Server-Sent Events
  response (`Content-Type: text/event-stream`). See
  [methods.md](./methods.md) and [transports.md](./transports.md).

## Types and internals

- **`BrpResult`** — the result type a BRP method handler returns: `Ok(Value)` on
  success or a `BrpError` on failure. The unit a custom handler produces. See
  [custom-methods.md](./custom-methods.md).

- **`BrpError`** — the error object a failed method returns: `code` (integer),
  `message` (human-readable), optional `data`. Standard JSON-RPC codes sit in the
  `-326xx` band; BRP-specific ECS errors in the `-232xx`/`-234xx` band (exact
  numeric values version-tracked, (unverified)). See [transports.md](./transports.md).

- **`RemoteWatchingRequests`** — the resource holding currently-active `+watch`
  subscriptions; a Bevy system iterates it each frame and pushes a response per
  watcher when its data changed (closing the channel on error).

- **custom method** — a non-built-in BRP method registered by the app: a name
  string mapped to a handler. Lets an app expose app-specific remote operations.
  See [custom-methods.md](./custom-methods.md).

- **`with_method`** — builder call on `RemotePlugin` that registers a custom
  method: `RemotePlugin::default().with_method(name, handler)`. The doc shows the
  handler shape `fn handler(In(params): In<Option<Value>>, world: &mut World)
  -> BrpResult` — a system-convertible fn taking optional JSON params and `&mut
  World`. (Exact `In<Option<Value>>` param type in the newest release is
  (unverified).) See [custom-methods.md](./custom-methods.md).

- **`RemoteMethods`** — the resource holding all registered method handlers;
  allows inserting custom methods **at runtime** (not just at plugin build via
  `with_method`). Existence confirmed across 0.15–0.18; API stability across
  those versions is (unverified).

## Reflection layer (the "reflection tax")

BRP only sees what reflection exposes. See [open-problems.md](./open-problems.md)
and [lessons.md](./lessons.md) for why this matters to Buiy.

- **`AppTypeRegistry`** — the ECS resource wrapping Bevy's `TypeRegistry` for the
  whole app. BRP resolves component/resource type names and their (de)serializers
  through it; a type absent from the registry is invisible over BRP.

- **`Reflect`** — Bevy's reflection trait. A component/resource must be
  Reflect-registered (`app.register_type::<T>()`) to be queryable, readable, or
  mutatable over BRP at all.

- **`ReflectSerialize`** (and its pair `ReflectDeserialize`) — type-data entries
  in the registry that let BRP serialize a reflected value to JSON (and parse it
  back). Without them a registered type may still be invisible/uneditable over
  BRP. The exact required-trait list stated by the community is (unverified)
  against current source.

- **reflection path** — the dotted/indexed field path used by
  `world.mutate_components` to address one field inside a component (e.g.
  `translation.x`), interpreted via `Reflect`.

## Hierarchy components

- **`ChildOf` / `Children`** — Bevy's parent/child relationship pair (the 0.16
  rename of `Parent`→`ChildOf`, PR #17427). `ChildOf(parent)` on an entity is the
  source of truth; the parent automatically gains a `Children` list. Relevant to
  BRP because reparenting (`world.reparent_entities`, formerly `bevy/reparent`)
  manipulates this relationship.

## Ecosystem (external tools)

Full detail in [ecosystem.md](./ecosystem.md).

- **`bevy_brp_mcp`** — an MCP server (maintainer natepiano, MIT OR Apache-2.0)
  that lets AI assistants launch, inspect, and mutate Bevy apps over BRP. Latest
  **0.20.0-rc.1** (2026-05-24, Bevy 0.19-rc); latest stable **0.19.0**
  (2026-03-23, Bevy 0.18).

- **`bevy_brp_extras`** — companion plugin (same workspace) adding extra BRP
  methods, including a `screenshot` method (needs Bevy's `png` feature). Port
  priority `BRP_EXTRAS_PORT` env > `with_port()` > default 15702. Latest
  **0.20.0-rc.1** (2026-05-24, Bevy 0.19-rc); latest stable **0.19.0**
  (2026-03-23, Bevy 0.18).

## Sources

- bevy_remote module docs (RemotePlugin, RemoteHttpPlugin, RemoteMethods,
  BrpResult, RemoteWatchingRequests, with_method signature, JSON-RPC 2.0 basis):
  https://docs.rs/bevy/latest/bevy/remote/index.html
- bevy_remote error codes (BrpError / code bands): https://docs.rs/bevy/latest/bevy/remote/error_codes/index.html
- RemotePlugin: https://docs.rs/bevy/latest/bevy/remote/struct.RemotePlugin.html
- AppTypeRegistry: https://docs.rs/bevy/latest/bevy/ecs/reflect/struct.AppTypeRegistry.html
- ReflectSerializer / reflect (de)serialization type data:
  https://docs.rs/bevy/latest/bevy/reflect/serde/struct.ReflectSerializer.html
- BRP registry JSON schema endpoint (PR #16882) and resource methods (PR #17423):
  https://github.com/bevyengine/bevy/pull/16882 ;
  https://github.com/bevyengine/bevy/pull/17423
- BRP watch_id / unwatch (PR #16407): https://github.com/bevyengine/bevy/pull/16407
- Method rename to dotted form (PR #19377): https://github.com/bevyengine/bevy/pull/19377
- ChildOf / Children rename (Parent→ChildOf, PR #17427; 0.15→0.16 migration):
  https://github.com/bevyengine/bevy/pull/17427 ;
  https://bevy.org/learn/migration-guides/0-15-to-0-16/
- ChildOf component: https://docs.rs/bevy/latest/bevy/ecs/hierarchy/struct.ChildOf.html
- bevy_brp_mcp: https://crates.io/crates/bevy_brp_mcp
- bevy_brp_extras: https://crates.io/crates/bevy_brp_extras
