**Date:** 2026-06-18
**Status:** active
**Subject:** Unreal Engine Remote Control API — embedded HTTP + WebSocket server that reads, writes, AND invokes functions on live objects; the strongest remote-drive surface in this cluster

# Unreal Remote Control API

Unreal's **Remote Control** is an embedded web server running *inside* a live engine
instance. It is the only system in this cluster that exposes a true remote
command/RPC surface: not just reading and writing property values, but **calling any
function** exposed to Blueprint/Python on a live object. For the Buiy thesis — that
the semantic tree should become a bidirectional perception + control surface — Unreal
is the closest existing proof that "observe + mutate + invoke" over one wire is a
shippable design. (`godot.md` and `unity.md` are the weaker observe / observe-or-
mutate cousins; the Buiy thesis itself lives in [lessons.md](lessons.md).)

This file is the **remote-control-wire** lens on Unreal. For Unreal's UI *subsystem* —
Slate, UMG, Widget Blueprints, the AAA-scale UI story — see the sibling folder
[../unreal-slate-umg/](../unreal-slate-umg/). Remote Control is engine-wide (it
addresses any exposed UObject, not only UI), so the overlap with that folder is
small, but they are complementary lenses on the same engine.

- **Maintainer:** Epic Games.
- **License:** Unreal Engine EULA (proprietary; engine source on GitHub is gated
  behind EULA acceptance). Remote Control is marked **Beta**.
- **Docs:** https://dev.epicgames.com/documentation/en-us/unreal-engine/remote-control-for-unreal-engine
  (current docs track **UE 5.8**, released at State of Unreal 2026 / 2026-06-17, the
  final UE5 feature release before UE6).
- **WebSocket reference:** https://dev.epicgames.com/documentation/en-us/unreal-engine/remote-control-api-websocket-reference-for-unreal-engine

## Mechanism: one embedded server, two transports

The docs describe "running a web server inside the Unreal Engine that services
WebSocket messages and HTTP requests made by remote web applications through a
**REST-like API**." Two transports share that server:

- **HTTP** (default port **30010**) — REST-like request/response. Routes target
  presets, properties, and functions, e.g.
  `PUT http://localhost:30010/remote/preset/MyPreset/function/Print Text`, which
  "returns a JSON payload with any return values from the function."
- **WebSocket** (default port **30020**) — JSON messages: "All WebSocket messages
  sent to the server must be JSON objects." It plays two roles: (a) it can **tunnel
  an HTTP route** — an HTTP-typed message carries a URL, verb, and body so a single
  WebSocket connection can drive the same routes as the REST API; and (b) it carries
  the **change-push / pub-sub** stream (below). Both ports are configurable under
  Project Settings > Plugins > Remote Control.

A third, separate piece is the **Remote Control Web Interface** — a bundled no-code
web UI served on default port **7000** (`127.0.0.1:7000`). It is a *client* of the
HTTP/WebSocket API, not part of the API itself.

## The three verbs: read, write, invoke

From the overview, Remote Control lets you:

- "read and write the values of any properties exposed to Blueprint and Python or a
  Remote Control Preset", and
- "call any function that is exposed to Blueprint and Python."

That last verb is what sets Unreal apart from Godot's Remote inspector (property
edits only) and Unity's PlayerConnection (a raw byte-message bus you build semantics
on). Invoking a function is a genuine remote method call against a live UObject —
the analog an agent wants when "set property X" is not enough and it needs to trigger
an action (the AccessKit-action analog for Buiy).

## Change-push: pub-sub over the WebSocket

The WebSocket reference defines a publish–subscribe model keyed on a preset. A client
sends **`Preset.Register`** to subscribe (and `Preset.Unregister` to stop); the server
then pushes notifications:

- **`PresetFieldsChanged`** — "sent any time someone modifies the value of a property
  exposed in the preset" (the live observe-the-mutation channel),
- **`PresetFieldsAdded`** — "sent any time a property is exposed to the preset",
- **`PresetFieldsRemoved`** — "sent any time a property or function is removed",
- **`PresetFieldsRenamed`** — "sent any time a property or function is renamed."

This is the same shape as React DevTools' incremental operations stream
([react-devtools.md](react-devtools.md)): the tree/exposure set is diffed and pushed
rather than polled. Notably the push is **scoped to the preset**, not the whole
object graph — you only get events for what you explicitly exposed.

## The Remote Control Preset = a curated exposure allowlist

A **Preset** is a no-code asset that exposes selected project content (properties and
functions) and binds them to web-app widgets without writing code. Crucially it is
also a **security and surface boundary**: only what a designer has exposed into the
preset is reachable by name through the preset routes and appears in the pub-sub
stream. Everything else stays invisible to the remote client.

This is the same allowlist pattern Buiy gets *for free* from AccessKit: the semantic
tree only contains nodes/actions a widget chose to publish, so the exposure boundary
is intrinsic rather than a separate curated asset. Unreal had to invent the Preset
because its object graph is otherwise fully reflective; an AccessKit-first framework
starts with the curated surface. (Caveat: Remote Control can *also* address arbitrary
objects/properties exposed to Blueprint/Python directly, not only via a preset — the
preset is the curated convenience + binding layer, not the only door.)

## Default-off in packaged builds — the lesson

In-editor, the HTTP server starts automatically when the plugin is enabled. But in
**packaged / `-game` builds the system is disabled by default**; you must launch with
**`-RCWebControlEnable -RCWebInterfaceEnable`** to turn the web control server and
web interface on, respectively.

The posture is defensible (don't ship an open control port by accident) but the
lesson generalizes: **the remote-control capability is absent exactly in the headless,
shipped configuration where an agent would most want to drive the app.** A control
surface that only exists in the editor isn't a control surface for autonomous
clients — it's an authoring convenience. For Buiy this argues for the agent surface
being on the *same* AccessKit channel that ships in every build (screen readers need
it at runtime too), rather than a separate dev-only server that gets compiled out.
See [lessons.md](lessons.md).

## Security warnings

The server binds localhost by default and Epic warns explicitly against exposing it:
"Do not attempt to open the hostname and port of your Unreal Engine application to the
open Internet, as this may make your Project and your computer vulnerable to malicious
actions from third parties. Expect to use the Web Remote Control system only within
your Local Area Network (LAN) or over a secure Virtual Private Network (VPN)." To
reach the API from another machine you must change `DefaultBindAddress` in
`DefaultEngine.ini`. There is no built-in auth/token layer documented — the trust
model is network isolation + the preset allowlist, nothing more. A surface that can
*invoke arbitrary exposed functions* with no authentication is a sharp reminder that
remote-drive power and an auth story must ship together.

## Version / status caveats

- Remote Control is documented as **Beta**; the exact engine-version mapping of that
  Beta label was not pinned (current docs track UE 5.8). (unverified) whether it has
  graduated past Beta in any 5.x release.
- Ports **30010** (HTTP) and **30020** (WebSocket) are confirmed from the docs; **7000**
  is the separate Web Interface UI port.

## Implications for Buiy

- **Invoke is the differentiator.** Read + write alone (Godot) is insufficient for an
  agent; the ability to *call a function / trigger an action* is what makes a surface
  agentic. Buiy already authors AccessKit `actions` per node — consuming
  `ActionRequest`s back through the bevy_winit channel is the direct analog of
  Unreal's "call any exposed function," without a bespoke RPC server.
- **Curated exposure is mandatory, and Buiy gets it intrinsically.** Unreal needed
  the Preset allowlist; the AccessKit tree *is* the allowlist.
- **Ship the surface in every build, with auth.** Unreal's editor-only-by-default
  posture and missing auth layer are the two anti-patterns to avoid.

Detailed framing lives in [lessons.md](lessons.md) (validates / avoid / borrow); see
also [open-problems.md](open-problems.md) for the auth + scoping questions this raises.

## Sources

- Remote Control for Unreal Engine (overview, capabilities, enable flags, Beta): https://dev.epicgames.com/documentation/en-us/unreal-engine/remote-control-for-unreal-engine
- Remote Control API WebSocket Reference (port 30020, JSON, HTTP-tunnel message, Preset.Register pub-sub, PresetFieldsChanged/Added/Removed/Renamed): https://dev.epicgames.com/documentation/en-us/unreal-engine/remote-control-api-websocket-reference-for-unreal-engine
- Remote Control Preset API HTTP Reference (port 30010, function-call route): https://docs.unrealengine.com/5.1/en-US/remote-control-preset-api-http-reference-for-unreal-engine/
- Remote Control Web Application / Web Interface (port 7000, security warning, DefaultBindAddress): https://dev.epicgames.com/documentation/unreal-engine/remote-control-web-application-for-unreal-engine
- Unreal Engine 5.8 release (State of Unreal 2026, 2026-06-17): https://www.unrealengine.com/news/unreal-engine-5-8-is-now-available
