**Date:** 2026-06-18
**Status:** active
**Subject:** Bevy Remote Protocol — registering custom BRP methods (`with_method` / `RemoteMethods`), a worked handler example, and the Reflect/serde visibility tax that gates what BRP can see

# Custom methods + the Reflect/serde visibility tax

This is the load-bearing seam for any framework — like a UI toolkit — that wants
its own domain verbs over BRP instead of forcing clients to drive raw component
writes. Two facts govern whether that works:

1. **You can register arbitrary handlers** with full `&mut World` access, so
   semantic verbs (`ui.click`, `ui.snapshot_accessibility_tree`) are expressible.
2. **But the built-in component/resource methods only see Reflect-registered,
   serde-serializable types** — and a type that misses those requirements is
   *silently invisible*, with no surface error. The two facts interact: custom
   methods are the escape hatch *because* the reflection tax is real.

See [`methods.md`](./methods.md) for the built-in verb set this extends, and
[`lessons.md`](./lessons.md) for the validates/avoid/borrow take.

---

## Registering a custom method

Two registration paths.

**At init (builder):** `RemotePlugin::default().with_method(name, handler)`.
The verified signature (docs.rs, bevy 0.18.1):

```rust
pub fn with_method<M>(
    self,
    name: impl Into<String>,
    handler: impl IntoSystem<In<Option<Value>>, Result<Value, BrpError>, M>,
) -> RemotePlugin
```

`BrpResult` is the alias `Result<Value, BrpError>`. The docs state the handler is
"a system-convertible function which takes optional JSON parameters as input and
returns a `BrpResult`," with the canonical shape:

```rust
fn handler(In(params): In<Option<Value>>, world: &mut World) -> BrpResult
```

Two properties make this powerful for a framework:

- **Exclusive `World` access** — handlers "will always run with exclusive
  `World` access," so a verb can read or mutate any part of app state, not just
  one component on one entity.
- **Arbitrary system params** — the docs note handlers may use "arbitrary system
  parameters in conjunction with the optional `Value` input" (e.g. `Commands`,
  `Res<T>`, a `Query`), since the handler is any `IntoSystem`. `bevy_brp_extras`
  uses exactly this to add a `screenshot` method (see [`ecosystem.md`](./ecosystem.md)).

The handler decodes its own params out of the `Option<Value>` JSON blob and
encodes its own `Value` result — BRP does not type-check the payload for custom
methods; the schema contract lives inside the handler.

### Worked example: a custom verb returning a `Value`

A minimal handler that takes `{ "entity": <u64> }` and returns
`{ "child_count": <n> }` — illustrating param-decode, `&mut World` read, and the
`Ok(Value)` / `Err(BrpError)` split:

```rust
use bevy::remote::{BrpError, BrpResult, error_codes};
use serde_json::{json, Value};

fn count_children(In(params): In<Option<Value>>, world: &mut World) -> BrpResult {
    // Decode our own params out of the opaque JSON blob.
    let entity_bits = params
        .as_ref()
        .and_then(|v| v.get("entity"))
        .and_then(Value::as_u64)
        .ok_or_else(|| BrpError {
            code: error_codes::INVALID_PARAMS, // reuse the standard -32602
            message: "expected { entity: u64 }".into(),
            data: None,
        })?;
    let entity = Entity::from_bits(entity_bits);

    let n = world
        .get::<Children>(entity)
        .map(|c| c.len())
        .unwrap_or(0);

    // Encode our own result; BRP does not type-check it.
    Ok(json!({ "child_count": n }))
}

// Registered at startup:
app.add_plugins(RemotePlugin::default().with_method("ui.count_children", count_children));
```

Registered under `ui.count_children`, this is now enumerable via `rpc.discover`
and callable like any built-in. The exact `error_codes` constant names are
read off the crate (`-32602` is the standard JSON-RPC invalid-params code; see
[`transports.md`](./transports.md) for the error bands); treat the precise
constant spelling as version-tracked. `count_children` is trivial, but the same
shape is what lets a real handler dispatch through the framework's *own* event
path rather than poking a component (see below).

### Why a UI framework wants this

Without custom methods, an agent driving a UI over BRP must translate intent into
raw component writes: to "click a button" it would `world.mutate_components` some
interaction-state field by reflection path, which couples the client to internal
component layout and bypasses the framework's own event/observer flow. A custom
`ui.click` verb instead runs *inside* the framework's `&mut World`, dispatching
through the same code path a real pointer event would — semantics stay in the
framework, not smeared across the wire format. For Buiy specifically, the
analogous bidirectional surface is AccessKit `ActionRequest` consumption rather
than a bespoke BRP verb; the BRP custom-method mechanism is the *evidence* that
"register your own domain verbs" is a viable pattern, not a prescription to copy
the transport. That distinction lives in [`lessons.md`](./lessons.md) and in
[`../llm-agent-interface/bevy-mcp-bridges.md`](../llm-agent-interface/bevy-mcp-bridges.md).

**At runtime:** the `RemoteMethods` resource allows inserting methods after the
app is built (insert into the resource's method map). Its existence is confirmed
across 0.15→0.18; whether its exact API is unchanged across those releases is
(unverified). The init-time `with_method` path is the documented norm.

---

## The Reflect/serde visibility tax

The built-in component and resource methods (`world.get_components`,
`world.query`, `world.insert_components`, `world.mutate_components`, the resource
variants — see [`methods.md`](./methods.md)) operate entirely through reflection.
A type is reachable over those methods only if **all** of:

1. it derives `Reflect` and is registered in the **`AppTypeRegistry`**
   (`app.register_type::<T>()`, or auto-registration where applicable);
2. it carries the serde reflect data — community guidance is
   `#[derive(Serialize, Deserialize)]` plus `#[reflect(Serialize, Deserialize)]`,
   which registers **`ReflectSerialize` / `ReflectDeserialize`** so the BRP
   serializer can round-trip values; and
3. for component access specifically, `#[reflect(Component)]` registers
   `ReflectComponent` so the type can be located on an entity.

The exact reflect-trait list above is community-sourced and not confirmed verbatim
against current `bevy_remote` source — treat the precise trait names as
(unverified). The *shape* of the requirement (Reflect + serde data + registry)
is stated consistently across Bevy docs and the reflection guides.

### The unflattering part: invisible, not errored

Missing the tax does not produce a clear "type not registered" error at the point
you'd expect one. BRP's `world.query` **skips missing or invalid components by
default** rather than failing: a `strict` boolean was added to `BrpQueryParams`
(PR [#16725](https://github.com/bevyengine/bevy/pull/16725), default `false`) to
*opt into* failing on unknown components, and a follow-up bug
([#18869](https://github.com/bevyengine/bevy/issues/18869) / fix
[#18871](https://github.com/bevyengine/bevy/pull/18871)) showed that silent
skipping could even make a query "match all entities" because the unrecognized
component left the query with no data/filter configured. So the default-path
failure mode is: **a component your framework manages but never reflect-registered
simply does not appear in query results, and no error tells the client it was
dropped.**

For a UI framework this is the dangerous case. If text content, layout state, or
an accessibility role were stored in an un-reflected type, an agent inspecting
the app over the built-in BRP methods would see a structurally-plausible but
*incomplete* world — the button exists, but its label is missing — with nothing
flagging the gap. The reflection tax is therefore not just a registration chore;
it is a **correctness boundary for the agent's perception**, and one that fails
open (silently) rather than closed (loudly).

### How custom methods sidestep the tax

A custom handler reads the `World` directly in Rust, so it can observe types that
are *not* reflect-registered or *not* serde-serializable — it just has to encode
whatever it found into the `Value` it returns. This is why "snapshot the
accessibility tree" or "dump the resolved text run" is naturally a custom verb: it
lets the framework project its own *already-structured* state out to the client
without first making every internal type satisfy the reflection/serde contract.
The cost is that the verb's payload schema is hand-maintained inside the handler
instead of derived from the type registry.

---

## Cross-links

- [`methods.md`](./methods.md) — the built-in verbs these handlers extend and the
  reflection-driven component/resource API subject to the tax.
- [`transports.md`](./transports.md) — the JSON-RPC envelope, the error bands a
  custom handler signals through `BrpError`, and worked request/response shapes.
- [`lessons.md`](./lessons.md) — validates/avoid/borrow framing for Buiy
  (custom verbs vs. AccessKit `ActionRequest`; perception that fails silently).
- [`../llm-agent-interface/bevy-mcp-bridges.md`](../llm-agent-interface/bevy-mcp-bridges.md)
  — how MCP bridges lean on custom methods (e.g. `bevy_brp_extras`' `screenshot`).
- [`ecosystem.md`](./ecosystem.md) — `bevy_brp_extras` as a concrete custom-method
  plugin.

## Sources

- https://docs.rs/bevy/latest/bevy/remote/struct.RemotePlugin.html — `with_method` signature, exclusive-World / arbitrary-system-param handler semantics.
- https://docs.rs/bevy/latest/bevy/remote/index.html — handler shape `fn(In<Option<Value>>, &mut World) -> BrpResult`; reflection-based access.
- https://docs.rs/bevy/latest/bevy/remote/error_codes/index.html — BrpError code constants (INVALID_PARAMS etc.).
- https://github.com/bevyengine/bevy/blob/main/crates/bevy_remote/src/lib.rs — `bevy_remote` source (RemoteMethods, BrpResult/BrpError).
- https://github.com/bevyengine/bevy/pull/16725 — adds `strict` field to BRP query params (default false = skip unknown components).
- https://github.com/bevyengine/bevy/issues/18869 — BRP query fails / silently mis-behaves on missing/invalid components.
- https://github.com/bevyengine/bevy/pull/18871 — fix for silent-skip query bug.
- https://taintedcoders.com/bevy/reflection — `#[derive(... Serialize, Deserialize, Reflect)]` + `#[reflect(Component, Serialize, Deserialize)]` registering ReflectSerialize/ReflectDeserialize.
- https://docs.rs/bevy/latest/bevy/ecs/reflect/struct.AppTypeRegistry.html — AppTypeRegistry as the registry the BRP serializer consults.
