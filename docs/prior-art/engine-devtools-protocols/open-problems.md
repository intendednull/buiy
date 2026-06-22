**Date:** 2026-06-18
**Status:** active
**Subject:** Engine devtools protocols — honest gaps an agent interface inherits, marked inherent vs. fixable, including the socket-transport auth gap the in-process answer leaves open

# Open problems — gaps in engine devtools protocols

The four reference systems ([godot.md](godot.md), [unity.md](unity.md),
[unreal.md](unreal.md), [react-devtools.md](react-devtools.md)) all prove the same
thesis: one live tree, two clients — a human inspector and, in principle, a
programmatic agent. But every one of them was built for the human client first. The
gaps below are the cost an LLM agent pays for that. Each is tagged **inherent** (a
consequence of what the system is for) or **fixable** (an accident of how it was built
that a clean design can avoid). The consumer-side takeaways live in
[lessons.md](lessons.md) as validates / borrow / avoid.

---

## 1. Editor/GUI-coupled framing — inspect+control welded to a panel

**Godot** and **Unity** never expose a clean programmatic API for the live tree.
The wire exists, but it is the *editor's* private channel:

- Godot's running game is a debug **client** of the editor **server** over a TCP
  socket; the live runtime tree appears as the Scene dock's **"Remote"** tab, and
  property mutation happens by editing the Inspector. The protocol is the
  `EngineDebugger` / `remote_debugger.cpp` internal format — there is no published,
  stable, documented message schema. An agent wanting the same drive capability
  must reverse-engineer the debugger wire from source
  (`core/debugger/remote_debugger.cpp`, `engine_debugger.h`), a format free to
  change between versions.
- Unity's **UI Toolkit Debugger** is a Window-menu panel that observes the live
  `VisualElement` tree (UXML structure, computed layout, resolved USS), with only
  ephemeral inline-style edits as mutation. No documented arbitrary live-mutation
  API sits behind it comparable to Godot's Remote inspector (unverified whether one
  exists at all). The general bus underneath — `PlayerConnection`/`EditorConnection`
  — is a raw GUID-keyed `byte[]` message bus: you must *invent and implement* your
  own drive semantics on top before an agent can do anything.

**Inherent or fixable?** *Fixable.* Nothing about live inspection requires a GUI
as the only client. The coupling is a product decision (ship an editor, not an
API). React DevTools already shows the split is possible: its **Bridge/Wall**
layering is transport-agnostic and the backend has no idea a GUI is on the other
end. Buiy's relevant move: make the semantic tree a first-class programmatic
surface, never reachable only through a panel.

---

## 2. Default-off / flag-gated posture — absent exactly when you want headless control

**Unreal Remote Control** is the strongest driver of the four (reads, writes,
*and* invokes exposed functions over HTTP `30010` / WebSocket `30020`) — and it is
**disabled by default in packaged / `-game` builds**. Per Epic's docs, "Remote
Control is disabled by default in packaged projects or in `-game` to accommodate
virtual production workflows." You re-enable it only by launching with
**`-RCWebControlEnable -RCWebInterfaceEnable`** on the command line (community
threads confirm this is the standard packaged-build dance).

The posture is security-by-disablement: the safe default is *no remote surface at
all*. For an autonomous agent that is precisely the wrong default — a headless
game process has no human to flip the flag, and a packaged build the agent didn't
launch itself simply has no port open. The control surface is absent exactly in
the deployment shape (shipped, headless) where programmatic control is most
wanted.

**Inherent or fixable?** *Partly inherent.* Exposing a mutate/invoke surface to
the network is a real attack surface; defaulting it off is defensible. The
*fixable* part is the binary framing — "all off" vs. "all on, bound to a port."
A capability-scoped, in-process surface (the agent shares the address space, no
socket to the open internet) sidesteps the dilemma: the same channel a screen
reader already uses doesn't need a `-Enable` flag because it isn't a new
remotely-reachable server. See [lessons.md](lessons.md) (avoid: don't make control
a gated opt-in network server). **But see gap 6** — the in-process answer only
holds for the in-process transport; the socket/MCP transport [lessons.md](lessons.md)
also wants reopens the auth question.

---

## 3. Manual per-object exposure ceremony — nothing reachable until a human curates it

Unreal's reach is gated a second time, below the flag: the **Remote Control
Preset**. By default *nothing* is addressable. A human opens the Remote Control
Panel, enables Edit Mode, and clicks the per-property **eye icon** (the
three-dot settings menu) to expose each property; only then is it "also exposed
to the Remote Control API." Functions and actors are exposed the same way, one
at a time. There is a Blueprint library to script the population, but the
default path is manual curation, per object, by a person.

For an agent this is a discovery wall: it can only see and drive the slice a
human already decided to expose. The world is full of state the agent cannot
reach because no one added it to a Preset. React DevTools has the opposite
posture — it instruments the *whole* renderer and every commit emits an
operations message for every relevant node; nothing needs manual registration.

**Inherent or fixable?** *Fixable, and it's a design fork.* Preset-style
opt-in exposure trades completeness for a curated, intentional surface
(virtual-production operators want exactly the 12 knobs on their panel). An
agent wants the dual: the *whole* tree visible by default, with semantics
attached. A tree that is authored for accessibility (role + name + state +
actions on every node) is exposed-by-construction — the curation already
happened, for a different consumer. That is the borrow in [lessons.md](lessons.md).

---

## 4. Bespoke per-engine wire formats — no interoperation

Every system speaks its own protocol over its own transport:

| System | Transport | Format |
|---|---|---|
| Godot | TCP socket (editor↔game) | internal `EngineDebugger` messages (undocumented) |
| Unity | negotiated socket (no fixed public port, unverified) | GUID-keyed `byte[]` payloads |
| Unreal | HTTP `30010` + WebSocket `30020` | JSON request/response + pub-sub |
| React DevTools | `postMessage` / WebSocket relay | versioned "operations" + dehydrated detail |

A tool written against one of these transfers nothing to the next. There is no
shared schema for "a UI node has a role, a name, a state, and a set of
invocable actions" — each project re-derives it. React's protocol is the most
disciplined (versioned and **checked at the backend/frontend handshake**, with
"Unsupported backend version" / "Unsupported Bridge operation" errors), but it
is still React-specific.

**Inherent or fixable?** *Fixable, but only by adopting an existing standard
instead of inventing another bespoke one.* The standard already exists for UI
semantics: the accessibility tree (ARIA roles in the browser; the platform a11y
APIs that AccessKit abstracts). `getByRole(role, { name })` (see
[react-devtools.md](react-devtools.md) and [lessons.md](lessons.md)) addresses a
node by ARIA role + accessible name — the same "name the node by what it *is*, not
how it's built" addressing an agent wants, and it is cross-framework precisely
because it rides a standard rather than a per-engine wire.

---

## 5. Polling vs. push — the agent re-asks instead of being told

React DevTools, having selected an element, **re-requests that element's detail
on a ~1/sec interval** (cited from the overview; verify the exact interval
against current `react-devtools-shared` if it's load-bearing). Detail is fetched
**lazily on selection** and sent dehydrated. The tree-shape operations stream
*is* push (every commit emits one), but the deep per-node state an inspector
shows is pull, on a timer.

For a human at 1 Hz this is invisible. For an agent it is latency plus wasted
round-trips: you learn a value changed up to a second late, and only for the one
node you happened to be "selecting." Unreal does better here — its WebSocket
channel carries change-push / pub-sub, so subscribed properties notify rather
than needing re-reads.

**Inherent or fixable?** *Fixable.* Polling is a fallback for the absence of a
change-notification primitive. An engine that already tracks per-field change
detection can push diffs instead. This is **Bevy's structural advantage** (next
section).

---

## 6. Auth for the out-of-process transport — the gap the in-process answer leaves

Gap 2's answer — "run the agent surface in-process, sharing the address space, so
there is no socket to secure" — is real but partial. [lessons.md](lessons.md)
itself argues Buiy should support **both** an in-process channel **and** a socket /
MCP transport (so an out-of-process inspector or an MCP-speaking agent can attach).
The moment a socket exists, the in-process argument no longer covers it, and the
field offers no good answer to copy:

- **Unreal** has *no built-in auth/token layer at all* — its trust model is
  "bind localhost + the preset allowlist + don't expose the port," and Epic's own
  docs warn against opening it to the internet. A surface that can invoke arbitrary
  exposed functions with zero authentication is the explicit anti-pattern.
- **Godot** and **Unity** bind localhost and rely on the dev-build / editor-session
  framing; neither documents authentication for a remote attach.
- **React DevTools** versions and handshakes the protocol but does not authenticate
  the peer — the WebSocket relay trusts whoever connects on `localhost:8097`.

So none of the four models a real auth story for a remotely-reachable control
surface; all four lean on network isolation. For Buiy's socket/MCP path that is
**an unsolved problem this corpus does not answer** — the in-process AccessKit
channel sidesteps it for the embedded case, but a token/capability/handshake-auth
design for the socket case still has to be invented (or borrowed from outside this
folder — MCP's own transport-security guidance is the obvious place to look).

**Inherent or fixable?** *Fixable, but out of scope for the precedents here* —
flagged so a Buiy spec does not mistake "in-process sidesteps auth" for "auth is
solved." It is solved only for the transport that doesn't need it.

---

## Bevy's advantage (the contrast)

Bevy starts from the opposite defaults on the two gaps that hurt agents most. Note
there is **no `bevy-remote-protocol` folder in this corpus**; the BRP facts below are
cited inline to their canonical sources rather than to a sibling file.

- **First-class programmatic API, not a GUI side-channel.** The **Bevy Remote
  Protocol (BRP)** is a documented **JSON-RPC 2.0** surface for inspecting and
  *editing* entities and components of a running app (methods like `bevy/query`,
  `bevy/get`, `bevy/insert`) — the API is the product, not the editor's private
  debugger wire. No reverse-engineering a panel's socket (contrast gap 1). (BRP
  itself is still a network server, so gap 2's posture question — and gap 6's auth
  question — stay live; see [lessons.md](lessons.md) for why an in-process AccessKit
  ActionRequest channel sidesteps the posture half.)
- **Native change-detection push, not polling.** Bevy's ECS already tracks
  per-component change detection, and BRP exposes **`+watch` methods**
  (`bevy/get+watch`, `bevy/list+watch`) that stream updates as components change —
  push built on the change detection the engine computes anyway, not a 1 Hz re-read
  loop (contrast gap 5). (Note: the server-owned `watch_id` refinement PR #16407 was
  closed pending a broader stream-transport redesign — the `+watch` capability
  ships; the id/cancel ergonomics are still settling.)

What Bevy/BRP does **not** yet solve, and what Buiy adds: a *semantic* surface.
BRP speaks ECS components (raw types), not role + name + state + actions. Buiy
already authors an AccessKit semantic tree — the same tree screen readers consume
— which is exposed-by-construction (gap 3) and rides a cross-framework standard
(gap 4). The remaining move is making it bidirectional by consuming AccessKit
`ActionRequest`s through the existing `bevy_winit` inbound path — which already
exists (`WinitActionRequestHandlers`, the `bevy::winit::accessibility` module;
landed with AccessKit integration PR #6874) and today forwards those requests into
an event channel rather than driving widgets. Wiring that channel to widget actions
turns the output-only perception surface into a control surface without standing up
a new gated network server (gaps 1, 2, 3, 5 addressed at once; gap 6 still applies
to any socket transport Buiy *additionally* exposes). The full validates / avoid /
borrow accounting is in [lessons.md](lessons.md).

---

## Sources

- Godot remote debugger source: https://github.com/godotengine/godot/blob/master/core/debugger/remote_debugger.cpp
- Godot EngineDebugger class docs: https://docs.godotengine.org/en/stable/classes/class_enginedebugger.html
- Godot debugging overview: https://docs.godotengine.org/en/stable/tutorials/scripting/debug/overview_of_debugging_tools.html
- Unity UI Toolkit Debugger: https://docs.unity3d.com/6000.2/Documentation/Manual/UIE-ui-debugger.html
- Unity PlayerConnection scripting ref: https://docs.unity3d.com/ScriptReference/Networking.PlayerConnection.PlayerConnection.html
- Unreal Remote Control (enable flags / packaged disablement / no built-in auth): https://dev.epicgames.com/documentation/en-us/unreal-engine/remote-control-for-unreal-engine
- Unreal Remote Control Presets / Expose Property (eye-icon ceremony): https://dev.epicgames.com/documentation/en-us/unreal-engine/remote-control-presets-and-web-application-for-unreal-engine
- Unreal WebSocket reference: https://dev.epicgames.com/documentation/en-us/unreal-engine/remote-control-api-websocket-reference-for-unreal-engine
- Unreal forum (packaged-build -RCWebControlEnable): https://forums.unrealengine.com/t/web-remote-control-is-disabled-by-default-when-running-outside-the-editor/490586
- React DevTools overview (operations, dehydration, versioned bridge, poll): https://github.com/facebook/react/blob/main/packages/react-devtools/OVERVIEW.md
- Bevy Remote Protocol (JSON-RPC, `bevy/get`, `+watch`): https://docs.rs/bevy/latest/bevy/remote/index.html
- Bevy Remote Protocol watch / unwatch PR #16407: https://github.com/bevyengine/bevy/pull/16407
- Bevy 0.15 release notes (BRP introduction): https://bevy.org/news/bevy-0-15/
- bevy_winit inbound AccessKit ActionRequest path (`WinitActionRequestHandlers`): https://docs.rs/bevy/latest/bevy/winit/accessibility/index.html ; AccessKit integration PR #6874: https://github.com/bevyengine/bevy/pull/6874
- Testing Library getByRole: https://testing-library.com/docs/queries/byrole/
