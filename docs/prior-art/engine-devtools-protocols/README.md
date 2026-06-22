**Date:** 2026-06-18
**Status:** active
**Subject:** Engine remote/devtools protocols — folder overview, key-facts table, reading order, and the "same substrate, different client" thesis

# Engine remote/devtools protocols

This folder documents how **game engines and React expose a running UI/scene tree to an external client** — a debugger, an inspector, a remote-control API — and, crucially, whether that client can only *observe* the live tree or can also *drive* it (mutate properties, invoke functions). These are the nearest-domain precedents for what a Buiy **agent interface** wants: attach to a live, running UI, read its semantic structure, and act on it.

This folder is the **remote/devtools-protocol lens** on three engines that already have their own UI-subsystem folders here: Godot UI → [`../godot-control/`](../godot-control/), Unity UI → [`../unity-ui/`](../unity-ui/), Unreal UI → [`../unreal-slate-umg/`](../unreal-slate-umg/). Those folders study the *widget/layout/theming subsystem*; this one studies the separate question of the *wire that surfaces a live instance of that tree to an outside client*. Read them as complementary, not duplicate — when in doubt, the per-engine UI folder is authoritative on widgets and layout, this folder on the remote/inspect/drive protocol.

The unifying theme is **"same substrate, different client."** Godot's Remote tab, Unity's UI Toolkit Debugger, Unreal's Remote Control API, and React DevTools all attach a *different client* to the *same live tree* the engine already maintains for its own purposes. The engine does not build a parallel "debug model"; it surfaces the tree it already has, and the human inspector and (potentially) an automated client are interchangeable consumers of that one surface. For Buiy the analog is exact: the **AccessKit semantic tree** (role + name + state + actions) that Buiy already authors for screen readers is the substrate; a human devtools inspector and an LLM agent are two clients of it. (Bevy's transport-shaped analog is **BRP** — the Bevy Remote Protocol, a JSON-RPC surface over the running ECS; the semantic surface for Buiy is the AccessKit tree, not raw ECS components.)

This is an **index, not a deep dive.** Per-system detail lives in the sibling files below; design conclusions for Buiy live in [`lessons.md`](lessons.md), not here.

## Key facts (verified 2026-06-18 via web)

| System | Inspects | Drives? (mutate / invoke) | Transport / port | Maintainer / license |
|---|---|---|---|---|
| **Godot** — Remote scene tree + EngineDebugger ([`godot.md`](godot.md)) | Live runtime **node** tree (Scene dock "Remote" tab); selected node's live state in Inspector | **YES** — Inspector property edits on the Remote tree mutate the running game live | TCP, localhost (127.0.0.1). Community-cited ports: **6007** live editor sync, **6006** script debugger *(forum/issue-sourced, not the docs page)* | Godot Engine project / **Godot Foundation**. **MIT**. Stable **4.7** (2026-06-18) |
| **Unity** — PlayerConnection / EditorConnection + UI Toolkit Debugger ([`unity.md`](unity.md)) | Live **VisualElement** tree (UXML structure, computed layout, resolved USS) | **PARTIAL** — PlayerConnection is a generic bidirectional byte-message bus (build your own drive semantics); UI Toolkit Debugger is inspect-primarily, with ephemeral inline-style edits only | Socket negotiated between editor and player; **no single fixed public port** documented (auto-discovery) *(unverified)* | **Unity Technologies**. **Proprietary** (the `UnityCsReference` mirror is reference-only, *not* open source) |
| **Unreal** — Remote Control API ([`unreal.md`](unreal.md)) | Any property/function exposed to Blueprint/Python or a Remote Control Preset | **YES — strongest of the four** — reads, writes properties, AND invokes exposed functions | HTTP default **30010**, WebSocket default **30020** (configurable; bind localhost) | **Epic Games**. **Unreal Engine EULA** (proprietary). Remote Control marked **Beta** |
| **React DevTools** ([`react-devtools.md`](react-devtools.md)) | Live React component tree (incremental "operations" messages; per-node detail fetched lazily, dehydrated) | **YES** — dev-mode prop/state/hook/context override (`overrideValueAtPath`); cannot create/destroy components arbitrarily | Transport-agnostic: `window.postMessage` in-browser; **WebSocket** relay (default port 8097) for React Native / standalone | **Meta (Facebook)**. **MIT**. npm `react-devtools` **7.0.1** (2025-10-20) |

Two adjacent precedents are referenced but not given their own file: **Testing Library `getByRole(role, { name })`** (Kent C. Dodds / Testing Library org, MIT) — selecting a live UI node by **ARIA role + accessible name**, the same "address by semantics, not implementation" pattern an agent wants; and **Bevy's BRP** as the transport-shaped analog. Both are discussed in [`lessons.md`](lessons.md) and [`glossary.md`](glossary.md).

## How to use this folder

Read this README for the landscape and the one load-bearing lesson ("same substrate, different client"). Then read the per-system file for whichever precedent is closest to the decision in front of you:

- Designing the **read surface** (how the agent perceives the tree) → React DevTools' incremental-operations + lazy-dehydrated-detail protocol ([`react-devtools.md`](react-devtools.md)) is the most refined.
- Designing the **write/drive surface** (how the agent mutates or invokes) → Unreal's Remote Control ([`unreal.md`](unreal.md), property read/write **plus** function invoke) and Godot's live Inspector mutation ([`godot.md`](godot.md)) are the two ends of the spectrum.
- Designing **transport and enablement** (port, handshake, dev-build gating, exposure risk) → all four files; Unreal and Godot have the clearest port/enable-flag stories.
- Cross-cutting design conclusions for Buiy → [`lessons.md`](lessons.md); unresolved questions → [`open-problems.md`](open-problems.md).

## Framing disclosure

These files are written from Buiy's **AccessKit-semantic-tree-first, agent-surface** stance: Buiy already authors an AccessKit tree (role + name + state + actions) but only as *output*, and the thesis running through this folder is that this same tree is the right LLM-agent perception+control surface — made bidirectional by consuming AccessKit `ActionRequest`s through the existing `bevy_winit` inbound path. This is a **learn-from artifact, not a neutral catalog**: the per-system *evidence* files stay factual and source-cited, but the selection of systems, the inspect-vs-drive axis they are measured on, and every "Implications for Buiy" passage are organized to interrogate that one bet. Design decisions live only in [`lessons.md`](lessons.md) as validates / borrow / avoid; the evidence files do not bake them in. Read accordingly — a neutral survey of engine devtools would weight these systems differently.

## Folder contents

| File | Purpose |
|---|---|
| [`README.md`](README.md) | This file — overview, key-facts table, reading order, glossary stub, framing disclosure, the "same substrate, different client" thesis. |
| [`godot.md`](godot.md) | Godot's Remote scene-tree tab + EngineDebugger: editor-as-server / game-as-client, TCP socket, Synchronize Scene/Script Changes, live Inspector mutation of the running tree. |
| [`unity.md`](unity.md) | Unity's PlayerConnection / EditorConnection byte-message bus (GUID-keyed, `byte[]` payloads, development-build gate) and the UI Toolkit Debugger (inspect-primarily view of the VisualElement / UXML / USS tree, ephemeral inline-style edits). |
| [`unreal.md`](unreal.md) | Unreal's Remote Control API: embedded HTTP (REST-like) + WebSocket (JSON pub/sub) server; read/write properties AND invoke functions; ports 30010 / 30020; enable flags; Epic's exposure warning. |
| [`react-devtools.md`](react-devtools.md) | React DevTools' Backend → Agent → Bridge → Wall → Frontend layering; transport-agnostic Wall; incremental "operations" tree protocol; lazy dehydrated per-node detail; versioned handshake; `overrideValueAtPath` mutation path. |
| [`lessons.md`](lessons.md) | Cross-cutting conclusions for Buiy, as validates / borrow / avoid — "same substrate, different client," read vs. drive split, lazy detail, versioned handshake, the role+name locator pattern. |
| [`open-problems.md`](open-problems.md) | Unresolved questions these precedents raise for an agent surface: security/exposure, mutation safety, partial-vs-full drive, poll cadence vs. push, schema versioning, and the socket-transport auth gap. |
| [`glossary.md`](glossary.md) | Shared vocabulary: remote tree, EngineDebugger, PlayerConnection, Remote Control Preset, Bridge/Wall, dehydration, operations message, `getByRole`, BRP, ActionRequest. |

## Glossary stub

Full definitions in [`glossary.md`](glossary.md). The terms that recur across files:

- **Same substrate, different client** — the engine surfaces the *one* live tree it already maintains; the human inspector and an automated client are interchangeable consumers of it.
- **Inspect vs. drive** — *inspect* = read-only observation of the live tree; *drive* = mutate properties and/or invoke functions on it. The key axis of the table above.
- **Remote tree** — a live, running-instance tree exposed to an external client (Godot's "Remote" tab; React DevTools' component tree).
- **Dehydration** — sending only shallow per-node detail with nested values wrapped as fetch-on-demand markers (React DevTools).
- **Operations message** — a compact, incremental tree-diff message rather than a full re-send (React DevTools).
- **Remote Control Preset** — Unreal's curated set of exposed properties/functions an external client may read/write/invoke.
- **BRP** — Bevy Remote Protocol, Bevy's transport-shaped JSON-RPC analog over the running ECS (the engine-native precedent for Buiy's ecosystem).
- **AccessKit ActionRequest** — the inbound message type (role-targeted action) that would make Buiy's currently output-only AccessKit tree bidirectional.

## Why this matters for Buiy

These four systems are the **nearest-domain precedents** for combining *live inspection* with *remote control* of a running UI tree, and every one of them validates the same move: build **one** authoritative live tree and let multiple clients attach. Buiy already authors an AccessKit semantic tree (role + name + state + actions) — the very tree screen readers consume — but it is currently **output-only**. The precedents here show the missing half is not a new model but a *new client plus an inbound channel*: Godot/Unreal mutate the live tree, React DevTools overrides live values, all through a client distinct from the engine's own UI. Buiy's analog is to make the AccessKit tree bidirectional by consuming AccessKit **ActionRequests** through the existing `bevy_winit` inbound path — which already exists (`WinitActionRequestHandlers`, the `bevy::winit::accessibility` module; landed with AccessKit integration PR #6874) and today drops those requests into an event channel rather than driving widgets. Same substrate, a second (agent) client alongside the screen-reader one. See [`open-problems.md`](open-problems.md) for what this inbound path does and does not yet give, and [`lessons.md`](lessons.md) for the full accounting.

## Sources

- Godot 4.7-stable release (2026-06-18) — official archive: https://godotengine.org/download/archive/ ; GitHub releases (4.7-stable tag): https://github.com/godotengine/godot/releases ; coverage: https://www.phoronix.com/news/Godot-4.7-Released ; https://www.linuxcompatible.org/story/godot-47-release-brings-hdr-output-faster-asset-store-and-smoother-mobile-development-to-your-projects
- Godot debugging tools overview: https://docs.godotengine.org/en/stable/tutorials/scripting/debug/overview_of_debugging_tools.html
- Unity PlayerConnection / EditorConnection: https://docs.unity3d.com/ScriptReference/Networking.PlayerConnection.PlayerConnection.html ; Unity UI Toolkit Debugger: https://docs.unity3d.com/6000.2/Documentation/Manual/UIE-ui-debugger.html
- Unreal Remote Control: https://dev.epicgames.com/documentation/en-us/unreal-engine/remote-control-for-unreal-engine ; WebSocket reference: https://dev.epicgames.com/documentation/en-us/unreal-engine/remote-control-api-websocket-reference-for-unreal-engine
- React DevTools overview: https://github.com/facebook/react/blob/main/packages/react-devtools/OVERVIEW.md ; npm `react-devtools` 7.0.1 (latest, published 2025-10-20): https://registry.npmjs.org/react-devtools/latest ; https://www.npmjs.com/package/react-devtools
- Testing Library `getByRole`: https://testing-library.com/docs/queries/byrole/
- Bevy Remote Protocol (JSON-RPC over the running ECS): https://docs.rs/bevy/latest/bevy/remote/index.html
- bevy_winit inbound AccessKit ActionRequest path (`WinitActionRequestHandlers`): https://docs.rs/bevy/latest/bevy/winit/accessibility/index.html ; AccessKit integration PR #6874: https://github.com/bevyengine/bevy/pull/6874
