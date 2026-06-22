**Date:** 2026-06-18
**Status:** active
**Subject:** Glossary of engine-devtools / remote-inspection terms used across this prior-art folder

# Glossary — engine devtools & remote-inspection terms

Concise definitions for the vocabulary used throughout this folder. Each entry is
1–2 lines. For full evidence, follow the per-system files: Godot
([godot.md](godot.md)), Unity ([unity.md](unity.md)), Unreal
([unreal.md](unreal.md)), React DevTools ([react-devtools.md](react-devtools.md)).
The folder thesis and how each term maps to Buiy live in [README.md](README.md),
[lessons.md](lessons.md), and [open-problems.md](open-problems.md).

## Godot

**EngineDebugger** — Godot's runtime debugger core inside the running game; opens a
TCP socket to the editor's debug server and ships profiler / error / remote-tree
traffic over it. The transport behind the Remote scene tree. See [godot.md](godot.md).

**Remote scene tree** — the **"Remote"** tab that appears in the editor's Scene dock
while a game runs, showing the *live* runtime node tree (vs. the "Local" tab = the
edited scene file). Selecting a remote node loads its live state into the Inspector,
where edits mutate the running game. See [godot.md](godot.md).

**SceneTree** — Godot's runtime owner of all active `Node`s (the scene graph the
game actually runs). The Remote scene tree mirrors this object to the editor;
distinct from the editor's edited `.tscn` representation.

**Synchronize Scene Changes** — Debug-menu toggle: "any changes made to the scene in
the editor will be replicated in the running project." Editor → game push of scene
edits. (Menu string from official docs.) See [godot.md](godot.md).

**Synchronize Script Changes** — Debug-menu toggle: "any changes made to the script
in the editor will be reloaded in the running project." Live script hot-reload into
the running game. (Menu string from official docs.) See [godot.md](godot.md).

## Unity

**PlayerConnection** — Unity API on the *player* (running build) side of the
editor↔player socket. `Register(Guid, Action)` to listen, `Send(Guid, byte[])` to
emit; the same socket multiplexes profiler, console, and the managed debugger.
Requires a Development build. See [unity.md](unity.md).

**EditorConnection** — the editor-side counterpart of PlayerConnection; same
GUID-keyed `Register` / `Send(Guid, byte[])` messaging API, used by editor tooling
to talk to connected players. See [unity.md](unity.md).

**UI Toolkit Debugger** — Window > UI Toolkit > Debugger: a browser-devtools-style
live view of the VisualElement tree, showing UXML structure, computed layout, and
resolved USS styles for the selected element. Primarily inspect; mutation limited to
ephemeral inline styles, no documented arbitrary live-mutation path. See [unity.md](unity.md).

**VisualElement** — the base node of Unity's UI Toolkit retained-mode UI tree (its
analog of a DOM element / scene node). What the UI Toolkit Debugger inspects.

## Unreal

**Remote Control API** — Unreal's embedded web server (Beta) that processes HTTP
requests and WebSocket messages (JSON) to read/write any Blueprint/Python-exposed
property AND *invoke* exposed functions. Strongest "drive" surface of the four
systems. HTTP default 30010, WebSocket default 30020. See [unreal.md](unreal.md).

**Remote Control Preset** — a curated, named bundle of exposed properties/functions/
actors registered for remote access, so external clients address a stable set of
fields rather than walking the whole object graph. See [unreal.md](unreal.md).

**`-RCWebControlEnable`** — launch flag that turns on the Remote Control web server
in packaged / `-game` builds (where it is off by default), usually paired with
`-RCWebInterfaceEnable`. In-editor the server starts automatically. See [unreal.md](unreal.md).

## React DevTools

**React DevTools** — Meta's MIT-licensed inspector for React trees (Components +
Profiler panels). Standalone npm `react-devtools` latest is 7.0.1 (published
2025-10-20). The closest web analog to an engine remote inspector. See
[react-devtools.md](react-devtools.md).

**Backend / Agent / Bridge / Wall / Frontend** — React DevTools' transport-agnostic
layering: **Backend** instruments the renderer and fires events → **Agent** receives
backend events and talks to the frontend → **Bridge** is the message abstraction →
**Wall** is the raw transport the Bridge wraps (postMessage in-browser, WebSocket
for RN / standalone) → **Frontend** is the Components/Profiler UI. The split is the
main "same substrate, different client" lesson for Buiy. See
[react-devtools.md](react-devtools.md) and [lessons.md](lessons.md).

**fiber tree** — React's internal reconciliation tree of "fiber" nodes (one per
component instance / host element). The Backend walks fibers to build the tree the
Frontend displays. Buiy's analog is the AccessKit semantic tree.

**dehydrated** — how per-node detail (props/state/hooks/context) is shipped on
selection: nested values are sent shallow, wrapped with metadata (type, name,
`unserializable`, `inspectable`, size, previews), and the deeper levels are fetched
lazily as the user expands. Avoids serializing whole object graphs. See
[react-devtools.md](react-devtools.md).

**operations message** — the compact, typed-array incremental tree-diff React emits
per commit (add/remove/re-order node ops keyed against a de-duplicated string
table), rather than re-serializing the whole tree. See [react-devtools.md](react-devtools.md).

**`overrideValueAtPath(type, path, value)`** — the unified dev-mode mutation entry:
a `type` discriminator (props/state/hooks/context) plus a path addresses one value
to override in the running app. Replaced the older `overrideProps` /
`overrideHookState` / `overrideState` / `overrideContext` (kept for back-compat);
siblings `deletePath` / `renamePath`. The "drive state" verb. See
[react-devtools.md](react-devtools.md).

## Cross-cutting

**Testing Library** — Kent C. Dodds' MIT-licensed DOM/UI testing family whose
queries address nodes by user-visible semantics, not implementation. Cited here as
the a11y-locator analog of an agent perception surface. See [lessons.md](lessons.md).

**`getByRole(role, { name })`** — Testing Library's primary query: select a node by
its ARIA **role** + accessible **name** — the exact "address a live UI node by
semantic role, not implementation" pattern an agent interface wants. (`logRoles`
enumerates available roles/names.) See [lessons.md](lessons.md).

**BRP (Bevy Remote Protocol)** — Bevy's documented JSON-RPC 2.0 surface over the
running ECS (`bevy/query`, `bevy/get`, `bevy/insert`, plus `+watch` change-push
variants). The engine-native, transport-shaped precedent for Buiy's ecosystem;
cited inline (https://docs.rs/bevy/latest/bevy/remote/index.html), no folder of its
own in this corpus. See [open-problems.md](open-problems.md).

**AccessKit ActionRequest** — AccessKit's inbound message (a role-targeted action
against a node) that would make Buiy's currently output-only AccessKit tree
bidirectional. Bevy already has the inbound path (`WinitActionRequestHandlers` in
`bevy::winit::accessibility`); wiring it to widget actions is the load-bearing Buiy
move. See [README.md](README.md) and [lessons.md](lessons.md).

**same-substrate-different-client** — this folder's organizing thesis: one live tree
(scene graph, VisualElement tree, fiber tree, or — for Buiy — the AccessKit semantic
tree) serves *both* a human inspector and an automated/agent client, observed and
driven through the same channel rather than via a bespoke side API. See
[README.md](README.md) and [lessons.md](lessons.md).

## Sources

- Godot debugging tools (Remote tree, Synchronize Scene/Script Changes menu strings): https://docs.godotengine.org/en/stable/tutorials/scripting/debug/overview_of_debugging_tools.html
- Unity PlayerConnection / EditorConnection scripting reference: https://docs.unity3d.com/ScriptReference/Networking.PlayerConnection.PlayerConnection.html
- Unity UI Toolkit Debugger manual: https://docs.unity3d.com/6000.2/Documentation/Manual/UIE-ui-debugger.html
- Unreal Remote Control for Unreal Engine: https://dev.epicgames.com/documentation/en-us/unreal-engine/remote-control-for-unreal-engine
- Unreal Remote Control WebSocket reference: https://dev.epicgames.com/documentation/en-us/unreal-engine/remote-control-api-websocket-reference-for-unreal-engine
- React DevTools architecture overview (Backend/Agent/Bridge/Wall/Frontend, inspectElement lazy fetch, operations format): https://github.com/facebook/react/blob/main/packages/react-devtools/OVERVIEW.md
- React DevTools hydration/dehydration source: https://github.com/facebook/react/blob/main/packages/react-devtools-shared/src/hydration.js
- react-devtools 7.0.1 (latest, published 2025-10-20): https://registry.npmjs.org/react-devtools/latest
- Testing Library getByRole query docs: https://testing-library.com/docs/queries/byrole/
- Bevy Remote Protocol: https://docs.rs/bevy/latest/bevy/remote/index.html
- bevy_winit inbound AccessKit ActionRequest path: https://docs.rs/bevy/latest/bevy/winit/accessibility/index.html
