**Date:** 2026-06-18
**Status:** active
**Subject:** Bevy Remote Protocol (BRP) — transports: JSON-RPC 2.0 envelope, worked request/response example, error model, the transport-agnostic in-process channel, the HTTP plugin, and SSE watch streaming

# BRP transports

BRP separates three concerns: the **wire format** (JSON-RPC 2.0), **request
dispatch** (a transport-agnostic in-process channel), and the **transport**
itself (HTTP, or anything you layer on). This file covers all three. For the
method catalog see [methods.md](methods.md); for the tools that ride these
transports see [ecosystem.md](ecosystem.md).

## JSON-RPC 2.0 envelope

BRP is JSON-RPC 2.0. A request object carries:

- `method` — a string naming a BRP method (e.g. `world.query`; pre-0.17 `bevy/query` — see [methods.md](methods.md) for the dotted-name rename).
- `params` — method-specific parameter data; optional for some methods.
- `id` — "arbitrary JSON data" that is echoed back in the response; the server otherwise ignores it.

A response carries the echoed `id` plus exactly one of:

- `result` — present if the request succeeded.
- `error` — present if it failed; an object with `code` (integer), `message` (a one-sentence human-readable description), and an optional `data` field of arbitrary type.

These shapes are documented on the crate's request/response/error types
(docs.rs `bevy/latest/bevy/remote`).

### Worked example: `world.query`

A request that asks for the `Name` and `Transform` of every entity that also has
a `Transform` (the data/filter split mirrors a Bevy `Query<D, F>`):

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "method": "world.query",
  "params": {
    "data": {
      "components": [
        "bevy_core::name::Name",
        "bevy_transform::components::transform::Transform"
      ]
    },
    "filter": {
      "with": ["bevy_transform::components::transform::Transform"]
    }
  }
}
```

A successful response — one object per matching entity, components keyed by
their fully-qualified type name (the only key BRP has; there is no semantic role
— see [open-problems.md](open-problems.md) §1):

```json
{
  "jsonrpc": "2.0",
  "id": 1,
  "result": [
    {
      "entity": 4294967297,
      "components": {
        "bevy_core::name::Name": "Player",
        "bevy_transform::components::transform::Transform": {
          "translation": [0.0, 1.0, 0.0],
          "rotation": [0.0, 0.0, 0.0, 1.0],
          "scale": [1.0, 1.0, 1.0]
        }
      }
    }
  ]
}
```

The exact `params` field names (`data`/`filter`, `components`/`with`) are the
BRP `BrpQueryParams` shape; treat the precise key spelling as version-tracked
(the values above match the 0.17+ dotted-method era) and confirm against
`registry.schema` / `rpc.discover` rather than hard-coding — see the
churn caution in [methods.md](methods.md).

### Error model

On failure the response carries an `error` object instead of `result`:

```json
{ "jsonrpc": "2.0", "id": 1,
  "error": { "code": -23402, "message": "Component `Foo` isn't registered", "data": null } }
```

BRP follows JSON-RPC 2.0's reserved code ranges and then adds its own
BRP-specific codes. Per the crate's error constants:

- **Standard JSON-RPC** (`-32700` parse error, `-32600` invalid request,
  `-32601` method not found, `-32602` invalid params, `-32603` internal error)
  — the reserved `-326xx` band.
- **BRP-specific** codes in the `-232xx` / `-234xx` band cover ECS-level
  failures (e.g. entity not found, component not registered/reflected,
  resource not present). The crate exposes these as named constants
  (`error_codes::*`); the exact numeric assignments are version-tracked — read
  them from the `bevy_remote` source rather than memorizing. *(exact numeric
  values per release: (unverified) — treat the bands, not the digits, as
  stable.)*

For **custom** methods, a handler returns `Err(BrpError)` to signal failure; the
`BrpError { code, message, data }` it constructs becomes the `error` object
verbatim. So a custom endpoint chooses its own codes — by convention reuse the
standard `-32602` (invalid params) for a malformed payload and pick an
app-specific code outside the reserved bands for domain failures. See
[custom-methods.md](custom-methods.md).

### Batches

Requests "may occur on their own or in batches" — a JSON array of
request objects yields a JSON array of responses. Internally this is the
`BrpBatch` enum (`Single` vs `Batch(requests)`); parsing of individual entries
is deferred so malformed entries can be reported individually. One restriction
the HTTP source states outright: *"Streaming can not be used in batch
requests"* — a `+watch` method cannot appear inside a batch array, because it
needs a long-lived connection (see below).

## The transport-agnostic core: an in-process channel

The core `RemotePlugin` "set[s] up everything needed without starting any
transports." It wires up an in-process channel and processes whatever requests
arrive on it; *you* (or a transport plugin) feed the channel. Concretely the
plugin installs two resources:

- `BrpSender` — the sending half; a transport hands deserialized `BrpMessage`s to it.
- `BrpReceiver` — the receiving half, drained each tick by a Bevy system that runs the matching method handler against the `World` and returns a `BrpResult`.

Because the contract is "push a `BrpMessage` onto the sender, get a result back,"
the core is genuinely transport-agnostic: HTTP is just one producer of
`BrpMessage`s. Anything that can deserialize a JSON-RPC request and own a
response channel can drive BRP — see the stdio note below. The crate-level docs
call BRP "a transport-agnostic and serialization-agnostic protocol."

## HTTP transport: `RemoteHttpPlugin`

The one transport Bevy ships is `bevy_remote::http::RemoteHttpPlugin`, gated
behind the **`http`** cargo feature **and** `not(target_family = "wasm")` — it
does **not** compile for wasm targets. It requires `RemotePlugin` to be added
first (it only supplies the network front end; the core supplies dispatch).

Default bind is **`127.0.0.1:15702`** (constants `DEFAULT_ADDR` /
`DEFAULT_PORT`; backing resources `HostAddress` / `HostPort`). Clients "are
expected to `POST` JSON requests to the root URL." Builder methods (verified on
docs.rs):

- `with_address(self, address: impl Into<IpAddr>) -> RemoteHttpPlugin` — sets the bind IP.
- `with_port(self, port: u16) -> RemoteHttpPlugin` — sets the listen port.
- `with_headers(self, headers: Headers) -> RemoteHttpPlugin` — extra response headers (bulk).
- `with_header(self, name: impl TryInto<HeaderName>, value: impl TryInto<HeaderValue>) -> RemoteHttpPlugin` — adds one response header.

The default loopback bind matters for the threat model: out of the box BRP is
reachable only from the same host, which is why the agent-tooling that talks to
it (MCP servers, inspectors) runs locally alongside the app. Binding to a
routable address with `with_address` exposes an unauthenticated read/write ECS
surface — BRP itself ships no auth. (See [open-problems.md](open-problems.md).)

WebSocket support for `RemoteHttpPlugin` was proposed/added via PR #16403
(villor) — requests sent as JSON text frames in the same JSON-RPC shape; treat
the exact shipped version as (unverified) here.

## Watch methods: per-tick diffs over an SSE response

The `+watch` methods (`world.get_components+watch`,
`world.list_components+watch`; pre-0.17 `bevy/get+watch`, `bevy/list+watch`)
turn a one-shot request into a subscription: each Bevy tick the server pushes a
diff. For `get_components+watch` the payload is `components` (a map of
components added or changed last tick) and `removed` (fully-qualified type names
of components removed). Outstanding subscriptions live in the
`RemoteWatchingRequests` resource, which "holds the `BrpMessage`s of all ongoing
watching requests along with their handlers."

Over HTTP the diffs stream on a single long-lived response implemented as
**Server-Sent Events**: the response sets `Content-Type: text/event-stream`, and
the `BrpStream` body emits each tick's result as an SSE `data:` frame —

```rust
let bytes = Bytes::from(format!("data: {serialized}\n\n").as_bytes().to_owned());
let frame = Frame::data(bytes);
```

The shape is: one request, an unbounded sequence of framed JSON diffs until the
client disconnects. A known wart (raised in PR #16403 discussion): with SSE
there is no in-band *unwatch* — closing the connection is the only cancel; a
bidirectional transport like WebSocket would need an explicit unwatch verb.
(See [open-problems.md](open-problems.md).)

## stdio: not core, but trivially layered

BRP ships no stdio transport. None is needed: because the core consumes
`BrpMessage`s off a channel, a process can read newline-delimited JSON-RPC from
stdin, feed the `BrpSender`, and write responses to stdout. This is the seam
the MCP bridges exploit — an MCP server speaks MCP (over stdio) to the agent
and BRP (over HTTP, default `127.0.0.1:15702`) to the running app, translating
between the two. So the agent never speaks BRP directly; the bridge does. (See
[ecosystem.md](ecosystem.md) and [../llm-agent-interface/](../llm-agent-interface/)
for `bevy_brp_mcp` and friends.)

## Implications for Buiy

The load-bearing lesson is the transport/dispatch/format split, not HTTP. A
perception+control surface built on Buiy's existing AccessKit tree would want
the same separation: a transport-agnostic request channel (Buiy already owns
the `bevy_winit` channel that delivers AccessKit `ActionRequest`s), a stable
request envelope, and a streaming diff for "what changed this tick" — the
AccessKit tree-update stream is already a per-tick diff, structurally analogous
to SSE `+watch`. The design takeaways live in [lessons.md](lessons.md) as
validates/borrow/avoid; this file stays factual.

## Sources

- bevy_remote crate docs (transport-agnostic design, JSON-RPC envelope, batch, watch payloads, default methods): https://docs.rs/bevy/latest/bevy/remote/index.html
- bevy_remote error codes / BrpError type: https://docs.rs/bevy/latest/bevy/remote/error_codes/index.html
- bevy_remote::http module docs (default 127.0.0.1:15702, `http` + non-wasm gating, POST to root URL): https://docs.rs/bevy/latest/bevy/remote/http/index.html
- RemoteHttpPlugin builder methods (with_address / with_port / with_headers / with_header): https://docs.rs/bevy/latest/bevy/remote/http/struct.RemoteHttpPlugin.html
- RemotePlugin docs (sets up the channel, starts no transport): https://docs.rs/bevy/latest/bevy/remote/struct.RemotePlugin.html
- bevy_remote source (lib.rs — BrpSender/BrpReceiver, BrpBatch, RemoteWatchingRequests, error_codes): https://github.com/bevyengine/bevy/blob/main/crates/bevy_remote/src/lib.rs
- bevy_remote HTTP source (SSE text/event-stream streaming, BrpStream, no-streaming-in-batch): https://github.com/bevyengine/bevy/blob/main/crates/bevy_remote/src/http.rs
- PR #14880 — Initial implementation of the Bevy Remote Protocol (Adopted): https://github.com/bevyengine/bevy/pull/14880
- PR #16403 — Add WebSocket support for RemoteHttpPlugin (unwatch discussion): https://github.com/bevyengine/bevy/pull/16403
