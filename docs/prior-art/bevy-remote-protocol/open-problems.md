**Date:** 2026-06-18
**Status:** active
**Subject:** Bevy Remote Protocol — what BRP does not solve (gaps, taxes, and bypasses), marked inherent vs fixable

# Open problems — what BRP does not solve

BRP (the `bevy_remote` crate, landed Bevy 0.15) is a strong *world-introspection*
protocol: a JSON-RPC surface onto the live ECS — query entities, read/write
components and resources, spawn/despawn, reparent, mutate single reflection
fields, watch for changes. See [`methods.md`](./methods.md) for the full method
set. But several gaps matter directly to anyone evaluating BRP as an **agent
perception+control surface** — which is the frame Buiy cares about (an LLM agent
that perceives and acts on a UI). This file catalogs those gaps and marks each
**inherent** (a consequence of the design) vs **fixable** (an omission that could
be closed without changing the model). Design takeaways live in
[`lessons.md`](./lessons.md); this file is evidence.

## 1. Raw ECS components are not a semantic UI model — INHERENT

BRP exposes the ECS as it is: a bag of typed components per entity. There is no
notion of "this is a button labeled Save, currently disabled." To recover UI
*meaning*, an agent must:

- **Walk the hierarchy itself.** Parent/child relationships are just components
  (`ChildOf` / `Children` in modern Bevy). BRP returns them as data; it does not
  hand back a pre-walked tree. The client reconstructs structure by following
  those component values entity-by-entity.
- **Decode each component's shape.** "Is this widget interactable? what's its
  label? is it focused?" is not a BRP concept — it is whatever components a given
  UI framework happens to attach (`Text`, `Interaction`, `Node`, plus
  framework-specific marker/state components), each with its own field layout the
  client must know how to read. BRP identifies components **by fully-qualified
  type name**, not by any semantic role (this name-keying is explicit in the
  protocol — see issue [#18138](https://github.com/bevyengine/bevy/issues/18138)).

This is **inherent**: BRP is deliberately a generic world protocol, not a UI
protocol. It has no role/name/state/action vocabulary because the ECS has none.
The semantic layer — if you want one — lives above BRP, in the client, and must
be re-derived from raw components for every framework it talks to. Contrast with
an accessibility tree (AccessKit/AT-SPI/UIA), which *is* a role+name+state+action
model by construction. (This contrast is the central lesson for Buiy; it lives in
[`lessons.md`](./lessons.md), not here.)

## 2. The Reflect + serde visibility tax — INHERENT, partly silent

BRP can only see and edit a component/resource if it is **`Reflect`-registered in
the `AppTypeRegistry`** and carries the reflection type-data needed for
(de)serialization (community docs cite `ReflectSerialize` / `ReflectDeserialize`;
the exact required trait-data list is **(unverified)** against current source).
Types lacking this are **invisible and uneditable** over BRP — not errored, just
absent from results.

This tax has a *silent* failure mode that recurs in the Bevy tracker:

- Types that miss `ReflectSerialize` registration fail only at runtime, and the
  symptom is a confusing "did not register ReflectSerialize" error or silent
  omission, not a compile error — e.g. discussions
  [#12063](https://github.com/bevyengine/bevy/discussions/12063) (`Instant`),
  [#14281](https://github.com/bevyengine/bevy/discussions/14281)
  (`StrongHandle`), and `#[reflect(opaque)]` remote-type registration failing at
  runtime, issue [#19017](https://github.com/bevyengine/bevy/issues/19017).
- **Dynamic / runtime-defined components are largely outside BRP.** Ser/de for
  dynamic components is blocked on dynamic-type registration; BRP can sometimes
  list them but not round-trip their values (issue
  [#18138](https://github.com/bevyengine/bevy/issues/18138), open, labeled
  C-Bug). The protocol's name-keying compounds this: no stable name, no edit.

**Inherent** in that BRP is reflection-mediated by design — that is how it stays
schema-agnostic and language-neutral. **Fixable** at the margin: better error
surfacing and broader default registrations reduce the *silent* part, but the
fundamental "only reflected+serializable state is visible" constraint stays.

## 3. No screenshot / paint truth in core BRP — FIXABLE (and bolted on downstream)

Core `bevy_remote` exposes **ECS state only**. There is no method to retrieve
what was actually rendered — no framebuffer, no rasterized pixels, no
paint-order/visual-output truth. An agent using core BRP perceives the *model*,
never the *picture*; it cannot tell whether a widget the model says is visible was
actually painted, occluded, or off-screen.

This gap was filled **outside** the engine: `bevy_brp_extras` (natepiano
workspace, MIT OR Apache-2.0; latest **0.20.0-rc.1** (2026-05-24, Bevy 0.19-rc),
latest stable **0.19.0** (2026-03-23, Bevy 0.18)) adds a
**`brp_extras/screenshot`** method that captures the **primary window** to a file
path — and requires the Bevy **`png`** feature, or it writes a 0-byte file. See
[`ecosystem.md`](./ecosystem.md) and [`custom-methods.md`](./custom-methods.md)
for how extras plugs in.

**Fixable**: screenshot is just another custom method (BRP's `with_method` makes
this straightforward). But its absence from core means: (a) pixel truth is
opt-in, third-party, and version-tracked separately from the engine, and (b) the
extras screenshot is *window-scoped*, not the per-target render-to-texture path a
headless agent or a render-truth check would want (cf. Bevy's own
"screenshots from any render target" gap, issue
[#12478](https://github.com/bevyengine/bevy/issues/12478)).

## 4. Raw component writes can bypass framework systems and invariants — INHERENT

`world.insert_components`, `world.remove_components`, `world.spawn_entity`,
`world.despawn_entity`, `world.reparent_entities`, and especially
`world.mutate_components` (set one field by reflection path) write **directly into
the World**. They do not necessarily route through the systems a UI framework uses
to maintain its own invariants. Concretely, a remote write can:

- Set a state field a framework expected only its own change-detection/validation
  systems to set, leaving derived state (layout, styling, focus bookkeeping)
  stale until — or unless — the framework's systems happen to reconcile it.
- Spawn/reparent entities into a tree without the framework's
  construction/teardown logic running, producing structurally valid but
  semantically half-formed UI.

There is community awareness that BRP's relationship to system ordering is
delicate — issue [#16042](https://github.com/bevyengine/bevy/issues/16042) tracks
"system ordering around BRP." The protocol gives a client a write primitive at the
component level; honoring framework invariants is the framework's problem, not the
protocol's. **Inherent**: a generic World-mutation protocol cannot know any
particular framework's invariants. The mitigation is to expose *intent-level*
actions (do-this) rather than *state-level* writes (set-this-field), so the
framework's own code runs — but that is a design choice the client/framework must
make on top of BRP, not something BRP provides.

## 5. Method-name churn across versions — FIXABLE (a migration cost, now mostly paid)

The method namespace was renamed wholesale from the **`bevy/*`** prefix to dotted
**`world.*` / `registry.*`** forms in Bevy **0.17** (PR
[#19377](https://github.com/bevyengine/bevy/pull/19377), merged 2025-07-29) —
including a semantic rename (`bevy/destroy` → `world.despawn_entity`). Any client
written against 0.15/0.16 breaks against 0.17+. The full mapping is in
[`methods.md`](./methods.md). This is a one-time **fixable** migration tax (the
method set is now more self-describing), but it is a real interop cost for tools
straddling Bevy versions, and a reminder that BRP's method surface is **not** a
frozen contract — it tracks engine refactors. (For how Buiy should version its
*own* verbs to avoid repeating this churn, see [`lessons.md`](./lessons.md).)

## 6. HTTP-only default, no wasm — INHERENT for the default transport

The only transport shipped in-tree is `RemoteHttpPlugin`, behind the **`http`**
cargo feature, and it is gated **`not(target_family = "wasm")`** — so the default
transport **does not run in the browser**. The docs.rs `bevy::remote::http` page
confirms the wasm exclusion and the default bind **127.0.0.1:15702**. The core
`RemotePlugin` itself is transport-agnostic (it processes requests over an
in-process channel), so a wasm-compatible transport *could* be written — see
[`transports.md`](./transports.md) — but none ships in-tree. **Inherent** to the
shipped default; **fixable** in principle via a custom transport.

## 7. Security / auth: an open loopback port with no authentication — INHERENT (by omission)

Core BRP ships **no authentication, authorization, or encryption**. Adding
`RemoteHttpPlugin` opens an HTTP port (default **127.0.0.1:15702**) that accepts
any JSON-RPC request — including arbitrary component writes, spawns, despawns, and
single-field mutations — from anything that can reach the socket. The docs.rs
pages for both `bevy::remote` and `bevy::remote::http` (v0.18.1) contain **no**
authentication guidance and **no** "trusted-network only" warning; the security
model is implicitly "bind to loopback and trust the local machine."

Implications:

- Bind address is configurable, so a careless `0.0.0.0` bind exposes full World
  read/write to the network with no credential check.
- Even on loopback, any local process (or a browser page via a permissive setup)
  can drive the World. There is no notion of a read-only client, a scoped client,
  or an audit trail.

**Inherent** to the current design (the protocol has no auth concept); **fixable**
only by layering auth/transport security on top, which BRP does not do for you.
For an agent-control surface this is the single sharpest production gap: a remote
write primitive with no gate.

## 8. No stated scale/cost ceilings — FIXABLE (unspecified, not unsolvable)

Neither the docs nor the protocol name a cost ceiling for the load-bearing paths,
so a client must discover them empirically:

- **`world.query` / `registry.schema` payload size.** A broad query, or a
  `registry.schema` dump over a large type registry, serializes a lot of JSON
  per call; there is no documented pagination or result cap.
- **`+watch` per-tick diff cost.** The diff is computed every Bevy tick from
  change-detection; a watcher on a high-churn entity, or many concurrent
  watchers, multiplies that work per frame with no documented backpressure.
- **No batch-size limit is stated** for JSON-RPC batch arrays.

**Fixable** in the sense that none of this is intrinsic to the model — caps,
pagination, and backpressure are addable — but as shipped the scaling envelope is
unspecified. For Buiy this matters because the analogous surfaces (semantic-tree
snapshot, `+watch`-style tree-update stream) carry the same costs; the scaling
assumption must be named explicitly rather than inherited silently — see
[`lessons.md`](./lessons.md).

---

### Inherent vs fixable — at a glance

| # | Gap | Class |
|---|-----|-------|
| 1 | Raw components ≠ semantic UI model | **Inherent** |
| 2 | Reflect+serde visibility tax (silent omissions) | **Inherent** (silence is fixable) |
| 3 | No screenshot/paint truth in core | **Fixable** (bolted on by extras) |
| 4 | Raw writes bypass framework invariants | **Inherent** |
| 5 | Method-name churn (`bevy/*` → `world.*`) | **Fixable** (one-time migration) |
| 6 | HTTP-only default, no wasm | **Inherent** to default transport |
| 7 | Open port, no auth | **Inherent** by omission |
| 8 | No stated scale/cost ceilings | **Fixable** (unspecified) |

How these gaps shape Buiy's choice of perception+control surface — and which to
**borrow**, **avoid**, or treat as **validated** — is argued in
[`lessons.md`](./lessons.md). Term definitions: [`glossary.md`](./glossary.md).

## Sources

- bevy_remote (docs.rs, v0.18.1): https://docs.rs/bevy/latest/bevy/remote/index.html
- bevy_remote::http (docs.rs, v0.18.1): https://docs.rs/bevy/latest/bevy/remote/http/index.html
- Issue #18138 — bevy_remote doesn't work with dynamic components (component name-keying; ser/de blocked): https://github.com/bevyengine/bevy/issues/18138
- Issue #16042 — System ordering around BRP: https://github.com/bevyengine/bevy/issues/16042
- PR #19377 — Renamed BRP methods to be more explicit (`bevy/*` → `world.*`/`registry.*`): https://github.com/bevyengine/bevy/pull/19377
- Issue #19017 — `#[reflect(opaque)]` remote type fails to register at runtime, making ReflectSerializer unusable: https://github.com/bevyengine/bevy/issues/19017
- Discussion #12063 — Type 'bevy_utils::Instant' did not register ReflectSerialize: https://github.com/bevyengine/bevy/discussions/12063
- Discussion #14281 — StrongHandle did not register ReflectSerialize: https://github.com/bevyengine/bevy/discussions/14281
- bevy_brp_extras (docs.rs, screenshot method; png feature requirement): https://docs.rs/bevy_brp_extras
- bevy_brp_extras crates.io versions (0.20.0-rc.1 latest, 0.19.0 stable): https://crates.io/api/v1/crates/bevy_brp_extras
- Issue #12478 — Screenshots from any render target not just windows: https://github.com/bevyengine/bevy/issues/12478
