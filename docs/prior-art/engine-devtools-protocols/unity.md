**Date:** 2026-06-18
**Status:** active
**Subject:** Unity — PlayerConnection/EditorConnection byte-message bus + the UI Toolkit Debugger over the live VisualElement tree

# Unity — PlayerConnection bus + UI Toolkit Debugger

Unity (Unity Technologies, **proprietary** — the engine is closed-source; the
`UnityCsReference` GitHub mirror is published under Unity's Reference-Only /
Companion license and is **not** open-source-redistributable) exposes two
distinct surfaces relevant to an agent perception+control interface. They sit at
opposite ends of the "turnkey vs. build-it-yourself" axis:

1. **PlayerConnection / EditorConnection** — a low-level, generic, bidirectional
   socket message bus. It moves opaque bytes; any *meaning* (read this property,
   invoke that action) is something you build on top.
2. **UI Toolkit Debugger** — a browser-devtools-style inspector over the live
   retained `VisualElement` tree, addressed per `Panel`. It surfaces UXML
   structure, computed layout, and resolved USS, and supports limited live
   (non-persisted) inline-style editing.

Neither is a packaged "mutate any property / invoke any function over the wire"
API like Unreal's Remote Control (see [`unreal.md`](unreal.md)) or Godot's Remote
inspector (see [`godot.md`](godot.md)). Unity gives you the pipe and the
inspector separately; you assemble drive semantics yourself.

This file is the **remote/devtools-wire** lens on Unity. For Unity's UI *subsystem*
proper — UGUI vs. UI Toolkit, UXML/USS, VisualElement layout, theming, the
production-scale story — see the sibling folder [../unity-ui/](../unity-ui/), which
already notes the "UI Toolkit Debugger devtools shape" as a borrow. The two folders
are complementary; on widgets/styling defer to [../unity-ui/](../unity-ui/), on the
inspect/drive wire defer here.

## Surface 1: PlayerConnection / EditorConnection — the byte bus

`PlayerConnection` (runtime side, in the Player) and `EditorConnection` (Editor
side) are mirror-image endpoints of one socket-based message channel.

**The protocol shape is GUID-keyed `byte[]` messages.** Confirmed method
signatures from the Scripting API:

```csharp
// PlayerConnection (runtime)
public void Register(Guid messageId, UnityAction<MessageEventArgs> callback);
public void Send(Guid messageId, byte[] data);
public void Unregister(Guid messageId, UnityAction<MessageEventArgs> callback);
```

`EditorConnection` mirrors these on the Editor side (`Register` / `Send` /
`Unregister`, plus `Initialize`). Both ends agree on a shared `Guid messageId`;
the payload is raw `byte[]`. The callback receives a `MessageEventArgs`, whose
documented fields are a `playerId` (which connected player sent it) and the
`byte[] data` payload — so serialization/deserialization of anything structured
is entirely the caller's job.

Key traits drawn from the docs and the `UnityCsReference` mirror:

- **Symmetric, bidirectional.** Player → Editor and Editor → Player use the same
  `Register`/`Send` shape. A message ID is just a `Guid` constant both ends
  hard-code.
- **Multiple callbacks per ID.** "There can be multiple registered callbacks for
  one message ID," each individually removable via `Unregister`. This is a
  fan-out subscription model, not a single handler slot.
- **One channel, many consumers (multiplexed).** The same PlayerConnection socket
  carries Unity's own profiler stream, the console/log relay, and the managed
  debugger alongside any custom message IDs you register. The bus is a shared
  substrate; subsystems demultiplex by GUID.
- **Requires a Development build.** The connection only exists for a Player built
  with **Development Build** enabled; the profiler path additionally wants
  **Autoconnect Profiler**. Connection is established by auto-connect or by
  connecting to a host/IP — there is no single documented fixed public port
  (auto-discovery / multicast negotiation between Editor and Player). **Any
  specific Unity PlayerConnection port number is (unverified)** — the docs do not
  pin one.
- **`PlayerConnection.instance`** is a singleton, usable only from a
  `MonoBehaviour` / `ScriptableObject` / `Object`-derived class; `isConnected`
  reports Editor reachability. There is also a blocking `BlockUntilRecvMsg` and a
  `TrySend`.

**Drive semantics: build-it-yourself.** Nothing about PlayerConnection knows what
a "property" or an "action" is. To drive game state you define your own message
IDs and your own byte encoding on both ends — it is a transport, not a
property-mutation surface. This is the opposite design point from a
schema-bearing API: maximum generality, zero out-of-the-box semantic addressing.
(The transport-agnostic, layered split here rhymes with React DevTools' Bridge /
Wall separation — see [`react-devtools.md`](react-devtools.md) — except Unity
stops at the raw wall and ships no standard "operations"/"override" protocol on
top.)

Third-party tooling fills the gap: e.g. the community `UniCli` project drives the
Editor from the terminal so "both humans and AI agents" can run compiles/tests/
commands, and the `Unity-EditorConnectionExample` repo shows the bare
Register/Send loop. None of that is a first-party agent-control protocol; it is
people hand-rolling semantics over the generic bus.

## Surface 2: UI Toolkit Debugger — inspecting the live tree

Opened via **Window > UI Toolkit > Debugger** (or the ⋮ menu in UI Builder / Game
view → "UI Toolkit Debugger"). It is the browser-DevTools analog for UI Toolkit
(Unity's retained-mode UI; the runtime tree is `VisualElement` nodes, authored in
**UXML** and styled with **USS**, Unity's HTML/CSS analogs).

What it exposes, addressed per **Panel** (a `Panel` is the root container that
owns one `VisualElement` hierarchy):

- **The live `VisualElement` tree** — "a live view of your UI hierarchy," all
  child elements of the selected element. Pick a node with the **Pick Element**
  button (the inspect-cursor pattern).
- **Resolved USS** — "All USS selectors for each component of the `VisualElement`"
  and detailed per-selector info: which selectors matched and what they
  contributed (the cascade, resolved).
- **Computed layout** — the element's resolved box (Unity's layout runs on a
  Yoga/flexbox engine), surfaced alongside element state and any element-level
  errors/warnings.

**Can it drive state? Limited, non-persisted yes.** The Debugger's Style
Inspector lets you "view and **edit** the styles that are applied to your UI
elements" and see the change live. But the docs are explicit about the ceiling:

> "Editing styles in the Debugger only changes the inline styles on the live
> elements themselves and the changes aren't saved anywhere and will be lost on
> the next UI regeneration."

So the mutation path is real but narrow: **inline style overrides on existing
elements, ephemeral, no persistence back to UXML/USS assets.** This is closer to
the browser DevTools "edit element.style" experience than to a programmatic,
schema-addressed property API. There is **no documented arbitrary live mutation
of non-style `VisualElement` properties, and no node create/destroy or
function-invoke path** comparable to Godot's Remote inspector or Unreal Remote
Control. (The Debugger can mutate inline styles, so it is not purely observe-only,
but it remains overwhelmingly an inspect-oriented tool.)

The Debugger is also an **Editor-side tool over the Editor's own panels / a
connected play session**, not a remote protocol you script from outside. It is
not layered on the PlayerConnection byte bus in any documented, reusable way —
the two surfaces are independent.

## Why this matters for Buiy

Unity is the clean example of the **"generic pipe + separate inspector" split**,
and of its cost. PlayerConnection is maximally flexible but semantically empty:
every integrator re-invents message IDs and byte encodings, and there is no
shared, queryable model of "what UI nodes exist and what can I do to them." The
UI Toolkit Debugger *does* hold that model (a live, role-ish, layout-resolved
tree) but keeps it Editor-local and human-facing, with mutation limited to
ephemeral inline styles. The lesson — that the valuable asset is a single
*semantic* live tree that is both inspectable and drivable through one typed
channel, rather than a raw transport plus a detached viewer — is drawn out in
[`lessons.md`](lessons.md); cross-engine comparison lives in
[`README.md`](README.md) and the open questions in
[`open-problems.md`](open-problems.md).

## Sources

- PlayerConnection class — https://docs.unity3d.com/ScriptReference/Networking.PlayerConnection.PlayerConnection.html
- PlayerConnection.Register — https://docs.unity3d.com/ScriptReference/Networking.PlayerConnection.PlayerConnection.Register.html
- PlayerConnection.Send — https://docs.unity3d.com/ScriptReference/Networking.PlayerConnection.PlayerConnection.Send.html
- EditorConnection class — https://docs.unity3d.com/ScriptReference/Networking.PlayerConnection.EditorConnection.html
- MessageEventArgs.data — https://docs.unity3d.com/2023.2/Documentation/ScriptReference/Networking.PlayerConnection.MessageEventArgs-data.html
- UnityCsReference ConnectionApi.cs — https://github.com/Unity-Technologies/UnityCsReference/blob/master/Runtime/Export/Networking/PlayerConnection/ConnectionApi.cs
- UI Toolkit Debugger (manual) — https://docs.unity3d.com/6000.2/Documentation/Manual/UIE-ui-debugger.html
- UniCli (community CLI for humans + AI agents) — https://github.com/yucchiy/UniCli
- Unity-EditorConnectionExample (community) — https://github.com/akof1314/Unity-EditorConnectionExample
