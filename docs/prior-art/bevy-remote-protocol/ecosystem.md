**Date:** 2026-06-18
**Status:** active
**Subject:** BRP ecosystem — inspectors (human clients), MCP bridges (agent clients), and editor tooling riding the protocol

# BRP ecosystem — same substrate, different clients

BRP (`bevy_remote`) is a transport, not an app. It ships no UI of its own (see
[transports.md](./transports.md)). Everything useful is a separate process that
speaks JSON-RPC to a running Bevy app over the HTTP transport (default
`127.0.0.1:15702`). The ecosystem splits cleanly by *who the client serves*:

- **Inspectors** — the **human** client. A person browses the entity tree, reads
  components, edits values. Web app, egui panel, or editor side-view.
- **MCP bridges** — the **agent** client. An LLM coding assistant launches,
  inspects, and mutates the app through the same `world.*` / `registry.*`
  methods ([methods.md](./methods.md)). Different consumer, identical wire
  protocol.

That symmetry is the load-bearing observation: one read/write surface serves both
a human tool and an agent tool, with no protocol fork. See [lessons.md](./lessons.md)
and the sibling agent-interface folder
[../llm-agent-interface/bevy-mcp-bridges.md](../llm-agent-interface/bevy-mcp-bridges.md).

## Inspectors (human clients)

### bevy_remote_inspector — web inspector over BRP
TypeScript + WebSocket front end; an alternative to the in-engine egui
inspector. Maintainer **notmd**, repo
<https://github.com/notmd/bevy_remote_inspector>, **MIT**. Features (per repo):
entity-hierarchy tree with drag-and-drop reparenting, component
add/remove/toggle, auto-reconnect on app restart.

Maturity caveat: the workspace `Cargo.toml` on `main` still pins `bevy = "0.15"`
with the `bevy_remote` feature — i.e. it predates the 0.17 dotted method rename
(`bevy/query` → `world.query`, see [methods.md](./methods.md)). lib.rs lists
**v0.1.0 / Bevy 0.15**. Treat it as **likely stale** against modern BRP; whether
it still tracks current method names is **(unverified)**.

### bevy-inspector-egui — in-process, *not* a BRP client
The widely-used egui inspector. Maintainer **Jakob Hellermann**
(@jakobhellermann), repo <https://github.com/jakobhellermann/bevy-inspector-egui>,
**MIT OR Apache-2.0**. Latest **0.36.0** (2026-01-14), supports Bevy 0.18. It
runs *inside* the app process and reflects the live `World` directly — there is
**no BRP / remote feature**. Listed here precisely to mark the boundary: it is
the in-engine cousin of the remote inspectors, sharing the reflection substrate
([custom-methods.md](./custom-methods.md) covers the reflection tax) but not the
wire protocol.

### Editor-integrated inspectors (BRP HTTP clients)
A layer of editor extensions consume BRP directly so the inspector lives in your
code editor rather than a browser:

- **vscode-bevy-inspector** (splo) — VS Code side-view showing entities,
  components, resources, and the schema registry; can insert/modify component &
  resource values on **Bevy 0.16+** (0.15 connects but lacks resources/registry
  edits). Repo <https://github.com/splo/vscode-bevy-inspector>, on the VS Code
  Marketplace and Open VSX. Requires the app to enable `RemotePlugin` +
  `RemoteHttpPlugin`.
- **NeitherDucks/bevy-inspector** — another VS Code plugin over BRP.
- **bevy_inspector.nvim** (Lommix) — Neovim remote entity/component inspector
  using the Telescope API, repo <https://github.com/Lommix/bevy_inspector.nvim>.

These reinforce the pattern: the protocol is fixed; the human surface is
whatever the editor/host provides. (Activity/version currency of the
third-party editor plugins beyond what is cited is **(unverified)**.)

## MCP bridges (agent clients): the bevy_brp family

Maintainer **natepiano**, unified workspace repo
<https://github.com/natepiano/bevy_brp>, **MIT OR Apache-2.0**. The older split
repos `bevy_brp_mcp-ARCHIVED` / `bevy_brp_extras-ARCHIVED` are deprecated; all
development is in the one workspace. Crates: **bevy_brp_mcp**,
**bevy_brp_extras**, and **mcp_macros** (boilerplate-reduction macros within the
workspace). A wider survey of MCP↔BRP bridges (incl. `bevy_mcp`,
`bevy_debugger_mcp`) lives in the sibling
[../llm-agent-interface/bevy-mcp-bridges.md](../llm-agent-interface/bevy-mcp-bridges.md).

### bevy_brp_mcp — the MCP↔BRP bridge
An **MCP server** that lets AI coding assistants "launch, inspect, and mutate
Bevy applications via the Bevy Remote Protocol." It is a **separate stdio
process**: the assistant talks MCP to the server; the server launches and
monitors the Bevy app as an independent process and relays to it over BRP. Tool
categories (per its README):

1. **App discovery & management** — find Bevy apps in the workspace, check build
   status, launch with proper asset loading.
2. **BRP core operations** — entity spawn/despawn/query, component
   get/insert/remove/mutate, resource management, hierarchy ops (i.e. thin
   wrappers over the `world.*` methods).
3. **Real-time monitoring** — component watching (the `+watch` streams), process
   status checks.
4. **Log management** — captures the launched app's stdout/stderr to temp files
   for the agent to read.
5. **Enhanced capabilities** — optional, only when the target app also embeds
   `bevy_brp_extras` (below).

Versions: latest **0.20.0-rc.1** (2026-05-24, targets Bevy 0.19-rc); latest
stable **0.19.0** (2026-03-23, targets Bevy 0.18). The version line diverged from
Bevy's at the 0.19 mark — earlier point releases include 0.18.6–0.18.8
(Feb–Mar 2026). Explicit Claude Code config example in the README. Adoption is
modest (~3k total crates.io downloads as of June 2026) — useful and active, but a
small project, not a foundation-blessed tool.

### bevy_brp_extras — extra BRP methods for the agent
An **optional plugin you add to your own Bevy app** that registers additional
BRP methods (custom methods in the sense of [custom-methods.md](./custom-methods.md)),
all under the **`brp_extras/`** prefix. The crate is best known for its
**`screenshot`** method, but the surface is broader than that — input synthesis
so an agent can *drive* the app, not just read it:

- **App lifecycle:** `screenshot`, `shutdown`, `set_window_title`, `get_diagnostics`
- **Keyboard:** `send_keys`, `type_text`
- **Mouse:** `click_mouse`, `double_click_mouse`, `send_mouse_button`,
  `move_mouse`, `drag_mouse`, `scroll_mouse`
- **Trackpad gestures (macOS):** `double_tap_gesture`, `pinch_gesture`,
  `rotation_gesture`

Gotchas worth noting:
- **`screenshot` needs Bevy's `png` feature.** Without it the file is created but
  is 0 bytes — a silent-ish failure.
- **Port priority:** `BRP_EXTRAS_PORT` env var > `with_port()` builder > default
  `15702`. This lets you point the agent at a non-default port without recompiling.

Versions: latest **0.20.0-rc.1** (2026-05-24, targets Bevy 0.19-rc); latest
stable **0.19.0** (2026-03-23, targets Bevy 0.18), with 0.18.6–0.18.8 preceding
it. Both the `mcp` and `extras` crates publish in lockstep — the 0.20.0-rc.1
pair was published the same day (2026-05-24) tracking Bevy 0.19-rc.

### Is there a standalone BRP CLI?
**No standalone `brp` / `cargo-brp` binary** was found in the natepiano
workspace — the tooling is MCP-server-based, not a hand-run CLI. A separate BRP
CLI's existence elsewhere is **(unverified)**; absence is not definitively
proven. In practice anyone can hit BRP with `curl` posting JSON-RPC, so a
dedicated CLI is low-value.

## The pattern, restated for Buiy

BRP is the **transport**; inspectors are the **human client** and MCP bridges
are the **agent client**, both on the *same substrate*. Crucially, the agent
side needed `bevy_brp_extras` to bolt on *control* (input synthesis, screenshots)
because core BRP is overwhelmingly a *data* protocol over reflected
components/resources — it has no first-class notion of "perform this UI action."

For Buiy the relevant contrast (developed in [lessons.md](./lessons.md)): Buiy
already authors an AccessKit semantic tree (role + name + state + **actions**).
That tree carries the *action* vocabulary BRP lacks, and it is the same tree
screen readers consume — so the human-assistive client and the agent client
could share not just a transport but a semantic, action-aware model, rather than
the reflection-of-raw-components view BRP exposes. The bevy_brp_extras "we had to
add an input-synthesis side-channel" story is evidence for *why* a raw-ECS
read/write surface is insufficient as an agent control plane on its own.

## Sources

- bevy_remote_inspector repo: <https://github.com/notmd/bevy_remote_inspector>
- bevy_remote_inspector workspace `Cargo.toml` (Bevy 0.15 pin): <https://raw.githubusercontent.com/notmd/bevy_remote_inspector/main/Cargo.toml>
- bevy_remote_inspector on lib.rs: <https://lib.rs/crates/bevy_remote_inspector>
- bevy-inspector-egui repo: <https://github.com/jakobhellermann/bevy-inspector-egui>
- bevy-inspector-egui crates.io versions (0.36.0, 2026-01-14): <https://crates.io/api/v1/crates/bevy-inspector-egui>
- vscode-bevy-inspector (splo): <https://github.com/splo/vscode-bevy-inspector>
- bevy-inspector (NeitherDucks): <https://github.com/NeitherDucks/bevy-inspector>
- bevy_inspector.nvim (Lommix): <https://github.com/Lommix/bevy_inspector.nvim>
- bevy_brp unified workspace: <https://github.com/natepiano/bevy_brp>
- bevy_brp_mcp README (tool categories, separate stdio process, Claude Code): <https://raw.githubusercontent.com/natepiano/bevy_brp/main/mcp/README.md>
- bevy_brp_extras README (method list, BRP_EXTRAS_PORT priority, png feature): <https://raw.githubusercontent.com/natepiano/bevy_brp/main/extras/README.md>
- bevy_brp_mcp crates.io versions (0.20.0-rc.1, 2026-05-24; stable 0.19.0, 2026-03-23): <https://crates.io/api/v1/crates/bevy_brp_mcp>
- bevy_brp_extras crates.io versions (0.20.0-rc.1, 2026-05-24; stable 0.19.0, 2026-03-23): <https://crates.io/api/v1/crates/bevy_brp_extras>
- bevy_brp_mcp-ARCHIVED (deprecated): <https://github.com/natepiano/bevy_brp_mcp-ARCHIVED>
