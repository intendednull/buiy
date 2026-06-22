**Date:** 2026-06-18
**Status:** active
**Subject:** Bevy Remote Protocol (BRP) — the built-in JSON-RPC method catalog (reads, writes, schema/discovery, `+watch` streaming) and the `bevy/*` → `world.*` rename history.

# BRP methods — the verb catalog

BRP is a JSON-RPC 2.0 surface over a Bevy `World`. The transport layer (see
[transports.md](transports.md)) only moves bytes; the *meaning* lives in the
method names below. Every built-in method is a thin wrapper over reflection and
ECS commands — it touches only data that is `Reflect`-registered in the
`AppTypeRegistry` (the "reflection tax"; see [open-problems.md](open-problems.md)).

Method names below are the **current (Bevy 0.17+)** dotted form. The pre-0.17
`bevy/*` names and the rename mapping are in the
[history section](#namespace-history-bevy--world).

## Reads

| Method | What it returns |
|---|---|
| `world.query` | Searches the ECS for entities matching a filter, returning component values. Filter params: `components` (fetch these), `option` (fetch if present), `has` (boolean presence checks), `with` (require), `without` (exclude). The data/filter split mirrors a Bevy `Query<D, F>`. |
| `world.get_components` | Fetch one or more named components from **one** entity (entity id + list of type names). |
| `world.list_components` | List all registered component types, or those present on a given entity. |
| `world.get_resources` | Fetch a single resource value by type name. |
| `world.list_resources` | List all resource types that have a `ReflectResource` in the registry. |
| `registry.schema` | Retrieve JSON-Schema-style type schemas for registered types — the machine-readable shape an external editor needs to construct valid component/resource/event payloads. |
| `rpc.discover` | OpenRPC service-discovery: advertises the available methods + server info. Lets a client enumerate the surface at runtime instead of hard-coding names. Name is unchanged across all versions. |

### Strict vs. lenient queries

Since Bevy 0.16, `world.query` **skips missing or invalid (non-reflectable)
components by default** instead of erroring. A `strict` boolean on the query
params flips this back to fail-fast: when `strict: true`, a component that can't
be reflected or found aborts the whole request. The lenient default makes
broad introspective queries ("give me everything you can") robust against the
fact that not every type is BRP-visible.

## Writes

| Method | Effect |
|---|---|
| `world.spawn_entity` | Create a new entity from a map of components; returns the new entity id. |
| `world.despawn_entity` | Remove an entity from the world. (Renamed from `destroy` — see history.) |
| `world.insert_components` | Add/overwrite components on an existing entity. |
| `world.remove_components` | Delete named components from an entity. |
| `world.mutate_components` | Set a **single field within one component** by reflection path: entity id + component type name + field path + new value. This is the fine-grained editor poke — change `Transform.translation.x` without resending the whole component. |
| `world.reparent_entities` | Re-assign the parent of one or more child entities (or detach to root). |
| `world.insert_resources` | Add or update a resource value. |
| `world.remove_resources` | Delete a resource. |
| `world.mutate_resources` | Field-path set within a resource (resource analogue of `mutate_components`). |
| `world.trigger_event` | Fire an `Event` by fully-qualified type name + value, so remote observers run. Added in Bevy **0.18** (PR [#21798](https://github.com/bevyengine/bevy/pull/21798), merged 2025-12-08). Requires the event type be reflect-registered with `#[reflect(Event)]` (a new `ReflectEvent` reflection type); without it the event is invisible/un-triggerable over BRP. Works for plain events and `EntityEvent`s. |

Note the asymmetry that matters for an agent-control surface: reads/writes above
mutate *state*, but only `world.trigger_event` injects into the *event/observer*
flow — i.e. it is the one built-in method that drives behavior rather than data.
For Buiy's AccessKit thesis the analogue is an `ActionRequest` (Default, Focus,
SetValue, …): an action verb fired at a node that runs the widget's handler,
not a field poke. See [lessons.md](lessons.md).

## `+watch` streaming variants

Two read methods have streaming siblings that emit a **per-tick diff** instead of
a one-shot snapshot. They require a streaming transport — over HTTP this is
**Server-Sent Events** (`Content-Type: text/event-stream`); see
[transports.md](transports.md) for the wire shape.

- **`world.get_components+watch`** — watches named components on one entity. Each
  tick's message carries:
  - `components`: map of components **added or changed** that tick,
  - `removed`: type names **removed** that tick,
  - `errors`: (lenient mode only) per-type error map.
- **`world.list_components+watch`** — watches the component *set* on one entity:
  - `added`: type names added that tick,
  - `removed`: type names removed that tick.

The diff is computed from Bevy's change-detection ticks, so a client gets
added/changed/removed deltas rather than re-polling full state. There is no
built-in `world.query+watch` — watching is per-entity, not per-query.

## Namespace history: `bevy/*` → `world.*`

- **0.15 (original, released 2024-11-29; initial impl PR
  [#14880](https://github.com/bevyengine/bevy/pull/14880) merged 2024-09-23):**
  all methods used the **`bevy/`** prefix — `bevy/query`, `bevy/get`,
  `bevy/spawn`, `bevy/insert`, `bevy/remove`, `bevy/destroy`, `bevy/reparent`,
  `bevy/list`, plus streaming `bevy/get+watch` and `bevy/list+watch`.
  (`bevy/mutate` arrived in this lineage via PR
  [#16940](https://github.com/bevyengine/bevy/pull/16940).)
- **0.16 (released 2025-04):** added the **resource methods**
  (`bevy/get_resource`, `bevy/insert_resource`, `bevy/remove_resource`,
  `bevy/list_resources`, `bevy/mutate_resource`; PR
  [#17423](https://github.com/bevyengine/bevy/pull/17423)) and the
  **`registry/schema`** endpoint. The `strict` query flag (lenient-by-default
  `world.query`) also landed in 0.16. `rpc.discover` is present.
- **0.17 (PR [#19377](https://github.com/bevyengine/bevy/pull/19377), AlephCubed,
  merged 2025-07-29):** renamed every method to the **dotted, explicit** form.
  Full mapping:

  | 0.15/0.16 (`bevy/*`) | 0.17+ (`world.*` / `registry.*`) |
  |---|---|
  | `bevy/query` | `world.query` |
  | `bevy/spawn` | `world.spawn_entity` |
  | `bevy/destroy` | `world.despawn_entity` *(note: destroy → despawn)* |
  | `bevy/reparent` | `world.reparent_entities` |
  | `bevy/get` | `world.get_components` |
  | `bevy/insert` | `world.insert_components` |
  | `bevy/remove` | `world.remove_components` |
  | `bevy/list` | `world.list_components` |
  | `bevy/mutate` | `world.mutate_components` |
  | `bevy/get+watch` | `world.get_components+watch` |
  | `bevy/list+watch` | `world.list_components+watch` |
  | `bevy/get_resource` | `world.get_resources` |
  | `bevy/insert_resource` | `world.insert_resources` |
  | `bevy/remove_resource` | `world.remove_resources` |
  | `bevy/mutate_resource` | `world.mutate_resources` |
  | `bevy/list_resources` | `world.list_resources` |
  | `registry/schema` | `registry.schema` |

  Rationale per the PR: align entity removal with `EntityCommands::despawn`
  (so `destroy` → `despawn`), and move from slash-prefixed `bevy/` to
  dot-separated hierarchical namespaces (`world.`, `registry.`, `rpc.`).
  `rpc.discover` was already dotted and is unchanged.
- **0.18 (current stable, docs.rs latest 0.18.1):** added `world.trigger_event`
  (above). Bevy **0.19** is in release-candidate stage as of June 2026; treat
  any 0.19 BRP changes as in-flight/(unverified).

This is a breaking rename with no built-in alias shim: a 0.15/0.16 client that
hard-codes `bevy/query` silently fails against a 0.17+ server. That fragility is
exactly what `rpc.discover` is meant to mitigate (enumerate, don't hard-code),
and it is a cautionary note for Buiy on freezing a verb vocabulary early —
see [lessons.md](lessons.md) and [open-problems.md](open-problems.md).

## Extending the catalog

The built-in set is not closed: `RemotePlugin::with_method(name, handler)` and
the `RemoteMethods` resource let an app register custom verbs (e.g.
`bevy_brp_extras`'s `screenshot`). That mechanism — and how it interacts with
`rpc.discover` — is covered in [custom-methods.md](custom-methods.md).

For how these data-shaped verbs compare to a semantic-action surface (AccessKit
role/name/state/action vs. component-type/field-path), and why Buiy's existing
AccessKit tree may be the better LLM-agent control surface than a reflect-typed
world API, see the bevy_ui prior-art folder [../bevy-ui/](../bevy-ui/),
the MCP-bridge survey in [../llm-agent-interface/](../llm-agent-interface/),
and [lessons.md](lessons.md).

## Sources

- bevy_remote 0.18.1 API docs: https://docs.rs/bevy_remote/0.18.1/bevy_remote/
- bevy::remote module docs (latest): https://docs.rs/bevy/latest/bevy/remote/index.html
- PR #14880 "Initial implementation of the Bevy Remote Protocol" (0.15): https://github.com/bevyengine/bevy/pull/14880
- PR #19377 "Renamed BRP methods to be more explicit" (rename, 0.17): https://github.com/bevyengine/bevy/pull/19377
- PR #16940 "Add BRP method to mutate a component": https://github.com/bevyengine/bevy/pull/16940
- PR #17423 "BRP resource methods" (0.16): https://github.com/bevyengine/bevy/pull/17423
- PR #21798 "world.trigger_event" (0.18): https://github.com/bevyengine/bevy/pull/21798
- Bevy 0.16 release / migration notes: https://bevy.org/learn/migration-guides/0-15-to-0-16/
- This Week in Bevy, 0.16 release: https://thisweekinbevy.com/issue/2025-04-28-bevy-016-is-out-now
