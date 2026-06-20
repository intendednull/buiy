**Date:** 2026-06-18
**Status:** active
**Subject:** Godot — EngineDebugger + the editor's Remote scene tree: a live mirror of the running SceneTree where Inspector property edits land in the running game.

# Godot — remote debugger, remote scene tree, live property mutation

Godot is the one MIT-licensed, shipping-at-scale game engine whose editor mirrors a
**live** scene tree out of the running game and lets you **edit** a selected node's
properties so the change lands in the running process. The other three engines here
are proprietary (see [unity.md](unity.md), [unreal.md](unreal.md)); Godot is the
open-source reference point for "same tree, two clients."

This file is about Godot's **remote/devtools wire**. For Godot's UI *subsystem* —
`Control` nodes, anchor+offset layout, the `Theme` resource, the widget catalog —
see the sibling folder [../godot-control/](../godot-control/); the two are
complementary lenses on the same engine.

## Maintainer / license / version

- **Maintainer:** Godot Engine project / the **Godot Foundation**. Original authors
  Juan Linietsky & Ariel Manzur.
- **License:** MIT (engine and editor; the editor is itself a Godot project).
- **Latest stable:** **Godot 4.7-stable, released 2026-06-18.** Confirmed against the
  official download archive (`godotengine.org/download/archive/`, which lists
  4.7-stable at the top with "Current state: stable") and the GitHub releases page
  (a `4.7-stable` tag dated 2026-06-18, the successor to 4.6.3-stable / 2026-05-20).
  4.7 is a feature release (HDR output, faster Asset Store, mobile improvements); the
  debugger model below is unchanged from 4.x and is **not** version-load-bearing for
  any lesson in this folder.
- **Repo:** https://github.com/godotengine/godot
- **Docs:** https://docs.godotengine.org/en/stable/tutorials/scripting/debug/overview_of_debugging_tools.html

## The mechanism: editor = debug server, game = debug client

When you run a project from the editor, the editor hosts a **debug server** and the
launched game connects back to it as a **client** over a TCP socket. The
`EngineDebugger` core singleton is "active in the running game" and, per the class
reference, "exposes the internal debugger, handling the communication between the
editor and the running game." It carries arbitrary named messages between the two
ends — `register_message_capture(name, callable)` registers a handler so that
messages prefixed `name:` (e.g. `scene:`, `core:`, `profiler:`) are dispatched to
that callable. The remote scene tree and the remote inspector are built on top of
this message bus, not on a bespoke protocol.

A key consequence: the debugger is a **general bidirectional message channel** that
the editor's higher-level tools (scene tree mirror, inspector, profiler, the
print/error console) all multiplex over. Compare React DevTools' Bridge/Wall split
in [react-devtools.md](react-devtools.md), where the transport is likewise an
abstract message wall under tree + inspect + mutate protocols.

## The Remote scene tree tab

While the game runs, the editor's **Scene dock** grows two toggle tabs: **Local**
(the `.tscn` you authored) and **Remote**. The Remote tab shows the **live runtime
node tree** — rooted at the running game's root `Window`/viewport and descending
through every node currently in the `SceneTree`, **including nodes instantiated at
runtime** (spawned enemies, dynamically `add_child`-ed UI, etc.) that never existed
in any `.tscn`. Selecting a remote node populates the **Inspector** with that node's
**live** property set read out of the running game.

The docs state plainly: "While using **Remote** you can inspect or change the nodes'
parameters in the running project." That is the load-bearing fact for this corpus —
the Remote tree is not observe-only.

## DRIVES state? YES — Inspector edits mutate the running game

Editing a property in the Inspector while a remote node is selected pushes the new
value down the debugger socket into the running process and applies it to the live
object. This is the strongest open-source instance of an out-of-process inspector
that **writes** into a running scene tree, not just reads it. (Unreal's Remote
Control, [unreal.md](unreal.md), goes further by also **invoking functions**, but is
proprietary and HTTP/WebSocket-based, not a debugger socket.)

**Caveat — persistence.** Values set on the Remote tree mutate the *running
instance only*. They do **not** write back to the `.tscn` on disk unless you
explicitly save; closing the game discards them. This observe-vs-author boundary —
the live tree is a separate surface from the persisted document — recurs across all
four engines and is drawn out in [lessons.md](lessons.md).

## Synchronize Scene / Script Changes (editor → game push)

The Remote tree is game → editor (the editor mirrors what the game is doing). Two
**Debug menu** toggles run the other direction, editor → game (exact menu strings
and descriptions quoted from the official docs):

- **Synchronize Scene Changes** — "When this option is enabled, any changes made to
  the scene in the editor will be replicated in the running project. When used
  remotely on a device, this is more efficient when the network filesystem option is
  enabled." Edit a node in the *Local* (authored) scene and the running game updates
  to match, without a restart.
- **Synchronize Script Changes** — "When this option is enabled, any changes made to
  the script in the editor will be reloaded in the running project." This is
  **hot-reload** of GDScript: save the script, the running game swaps it in.

Together these make the editor a bidirectional control surface: the game streams its
live tree up, and authored scene/script edits stream down into the live game.

## Transport / ports

- **Transport:** TCP socket, editor (server) ⇄ game (client). Default bind is
  localhost (`127.0.0.1`).
- **Ports:** Conventionally **6007** for the live editor sync channel and **6006**
  for the script debugger (print/errors). The 6007 figure appears in the exported
  remote-debug invocation — `mygame --remote-debug tcp://<dev-ip>:6007` — and in
  forum/issue threads ("Error listening on port 6007"). The exact ports are **not**
  stated on the official debugging-tools docs page itself; treat as
  community-confirmed, not docs-authoritative.
- **Remote devices / standalone clients:** the editor's Debug menu has **Keep Debug
  Server Open** — "When this option is enabled, the editor debug server will stay
  open and listen for new sessions started outside of the editor itself." An exported
  build launched with `--remote-debug tcp://<dev-ip>:6007` can then attach to the
  running editor, so the Remote tree / live inspect / hot-reload work against a build
  running on a phone or another machine, not just an in-editor run.

## Implications for Buiy

Godot is the closest engine precedent for the Buiy thesis: an out-of-process client
that reads a live in-process tree **and writes back into it**. Two transferable
shapes:

1. **One tree, two clients.** Godot's Remote tab and the game's own scene graph are
   the same tree surfaced to a second consumer over a socket. Buiy already authors an
   AccessKit semantic tree (role + name + state + actions); making it the agent's
   perception surface is the same move, with the screen reader and the LLM agent as
   the "second client."
2. **Mutation as a reverse channel on an existing bus.** Godot did not invent a
   write protocol; Inspector edits ride back down the *same* EngineDebugger socket
   the tree streams up. The Buiy analog: AccessKit `ActionRequest`s flowing back
   through the existing `bevy_winit` channel — reuse the output channel's return
   path rather than standing up a parallel control plane.

The persistence caveat is also a direct lesson: Godot's live edits don't touch the
`.tscn` unless saved. A Buiy agent driving the live tree mutates runtime ECS state,
not the authored scene/source — the same observe-vs-author boundary, recorded in
[lessons.md](lessons.md). For the ECS-family equivalent of a documented remote-tree
wire, the precedent is **Bevy's own Remote Protocol (BRP)** — a JSON-RPC surface
over the running ECS (https://docs.rs/bevy/latest/bevy/remote/index.html) — which,
unlike Godot's editor-private debugger socket, is published API rather than a panel's
internal format. That contrast (first-class API vs. editor side-channel) is developed
in [open-problems.md](open-problems.md).

## Sources

- Godot debugging tools (Remote tab, Synchronize Scene/Script Changes, Keep Debug Server Open): https://docs.godotengine.org/en/stable/tutorials/scripting/debug/overview_of_debugging_tools.html
- EngineDebugger class reference (server↔game communication, register_message_capture): https://docs.godotengine.org/en/stable/classes/class_enginedebugger.html
- godot repo: https://github.com/godotengine/godot
- core/debugger/engine_debugger.h: https://github.com/godotengine/godot/blob/master/core/debugger/engine_debugger.h
- Godot 4.7-stable (download archive; "Current state: stable"): https://godotengine.org/download/archive/
- Godot GitHub releases (4.7-stable tag, 2026-06-18): https://github.com/godotengine/godot/releases
- Godot 4.7 release coverage (Phoronix): https://www.phoronix.com/news/Godot-4.7-Released
- Remote-debug invocation + port 6007 (forum/issue thread): https://forum.godotengine.org/t/error-listening-on-port-6007/9920
- godot-docs issue requesting a remote-debugging guide (port/transport discussion): https://github.com/godotengine/godot-docs/issues/11245
- Bevy Remote Protocol (ECS-family remote-tree contrast): https://docs.rs/bevy/latest/bevy/remote/index.html
