**Date:** 2026-06-18
**Status:** active
**Subject:** Glossary — MCP, the Rust SDK, browser/GUI-agent control, and Bevy-bridge terms used across this folder

Concise definitions for terms recurring across [`README.md`](README.md) and the
evidence files. One- to two-line entries; deep treatment lives in the linked
sibling. "Current" = as of 2026-06-18.

## Protocol: MCP and its primitives

- **MCP (Model Context Protocol)** — open JSON-RPC protocol standardizing how LLM
  apps connect to external context and capabilities; introduced by Anthropic
  Nov 2024, donated to the Agentic AI Foundation (a directed fund under the Linux
  Foundation, co-founded by Anthropic, Block, and OpenAI) on 2025-12-09. Full
  treatment: [`mcp-protocol.md`](mcp-protocol.md).
- **Tool** — a server-exposed function the model can *call* (model-controlled);
  has a name, JSON-Schema input, and (since 2025-06-18) optional `structuredContent`
  output. The action primitive. See [`aci-tool-design.md`](aci-tool-design.md).
- **Tool annotations** — behavior hints on a tool (`readOnlyHint`,
  `destructiveHint`, `idempotentHint`, `openWorldHint`; 2025-03-26). Untrusted
  unless from a trusted server; `destructiveHint` is the natural signal for
  routing a verb through a confirmation gate.
- **Resource** — server-exposed *read-only context* (file, record, snapshot)
  addressed by URI; application- or user-controlled, not invoked like a tool.
  The perception primitive.
- **Prompt** — a reusable, parameterized message template the server offers the
  client (e.g. a slash-command), typically user-controlled.
- **Sampling** — server asks the *client's* LLM to generate a completion
  (`sampling/createMessage`), inverting the usual direction; 2025-11-25 added
  `tools`/`toolChoice` for server-side agent loops (SEP-1577).
- **Elicitation** — server pauses mid-call to ask the *user* for structured input
  via `elicitation/create` + a JSON schema; added 2025-06-18 (URL-mode 2025-11-25).
  The wire mechanism for a confirmation gate.
- **Roots** — client-declared filesystem/URI boundaries telling a server which
  locations it may operate within; a scoping/safety mechanism.
- **Tasks** — experimental 2025-11-25 primitive (SEP-1686) for durable/async
  "call-now, fetch-later" requests, decoupling long work from the request timeout;
  the shape a long-running UI action (animation/async load) would map to (see
  [`open-problems.md`](open-problems.md) § 8).

## Transports

- **stdio transport** — server runs as a child process; JSON-RPC framed over its
  stdin/stdout. The default for local servers; simplest and most common. `rmcp`
  feature `transport-io`.
- **Streamable HTTP** — the 2025-03-26 HTTP transport (replaced HTTP+SSE): a
  single HTTP endpoint that can upgrade to Server-Sent-Events streaming. Current
  remote transport. In `rmcp`, Server-Sent Events are folded into the
  `transport-streamable-http-server` / `transport-streamable-http-client`
  features (there is no standalone SSE-transport feature).
- **SSE (Server-Sent Events)** — the original 2024-11-05 "HTTP+SSE" remote
  transport (two endpoints, one an SSE stream); superseded by Streamable HTTP. In
  current MCP/`rmcp`, SSE survives only as the streaming *mechanism inside*
  Streamable HTTP, not as a separate transport.

## Rust SDK

- **rmcp** — the official Rust SDK for MCP (`modelcontextprotocol/rust-sdk`),
  Apache-2.0; crates.io max published version **1.7.0** (2026-05-13). Provides
  `#[tool]`, `#[tool_router]`, `#[prompt]`, `#[task_handler]` macros with auto
  JSON-Schema. Transport feature flags (per `crates/rmcp/Cargo.toml`):
  `transport-io` (stdio), `transport-child-process` (`TokioChildProcess`),
  `transport-streamable-http-server` / `transport-streamable-http-client`
  (Streamable HTTP, SSE folded in). See [`mcp-protocol.md`](mcp-protocol.md).

## Browser / GUI-agent control

- **Playwright-MCP** — Microsoft's MCP server (`@playwright/mcp`, Apache-2.0,
  v0.0.76, 2026-06-10) that drives a browser via tools like
  `browser_click`/`browser_type`. Accessibility-snapshot-first, not pixel-based.
  See [`playwright-mcp.md`](playwright-mcp.md).
- **Accessibility snapshot** — a structured, text serialization of the page's
  accessibility tree (roles, names, states), used as the model's perception
  surface instead of a screenshot. The same tree screen readers consume.
- **ref** — a stable per-element identifier emitted in an accessibility snapshot
  so the model can name a specific node without coordinates. In a Buiy mapping
  the ref is the AccessKit `NodeId`, which is per-tree-scoped — so across multiple
  windows the real ref is a `(window_id, NodeId)` pair (see
  [`open-problems.md`](open-problems.md) § 9).
- **act-by-ref** — the interaction pattern where the model targets an element by
  its `ref` (e.g. `browser_click(ref=...)`) rather than by pixel coordinate;
  deterministic and vision-free. Contrast set-of-marks / computer use.
- **Capability tier (`--caps`)** — Playwright-MCP's opt-in tool groups (e.g.
  `vision`, `pdf`, `devtools`, `network`; set churns per release) that gate
  higher-risk or heavier tools behind explicit enablement.
- **Snapshot-after-action** — the loop discipline of returning a fresh
  accessibility snapshot (or screenshot) *after* each action so the model always
  reasons over post-mutation state; central to both Playwright-MCP and computer
  use. The "when is the post-state settled" boundary is an open question for an
  ECS-backed UI. See [`playwright-mcp.md`](playwright-mcp.md), [`open-problems.md`](open-problems.md).

## Vision-based control

- **Computer use** — Anthropic's beta capability where Claude perceives a
  screenshot and emits raw mouse/keyboard/coordinate actions in a
  screenshot → act → screenshot loop; tool `computer_20251124`, still beta
  (not GA). See [`computer-use-and-gui-agents.md`](computer-use-and-gui-agents.md).
- **Set-of-marks (SoM)** — visual-prompting technique (Yang et al., arXiv
  2310.11441) that overlays numbered/boxed marks on interactive regions of a
  screenshot so the model can refer to elements by mark id instead of raw pixels;
  bridges vision models toward act-by-ref. See [`computer-use-and-gui-agents.md`](computer-use-and-gui-agents.md).

## Tool / interface design

- **ACI (agent-computer interface)** — term from the SWE-agent paper (arXiv
  2405.15793): the commands an agent uses plus the format of feedback it gets;
  the agent-facing analogue of an HCI/UI. Anthropic's guidance: invest in the ACI
  as much as the HCI. See [`aci-tool-design.md`](aci-tool-design.md).
- **Confused deputy / lethal trifecta** — the security failure mode of an agent
  with privilege being steered by attacker-controlled tokens (confused deputy);
  the *lethal trifecta* is the toxic mix of private data + untrusted instructions
  + an exfiltration vector. See [`open-problems.md`](open-problems.md) § 4.

## Bevy bridges

- **BRP bridge** — an MCP server that exposes a running Bevy app to MCP clients
  via the Bevy Remote Protocol (BRP), letting agents query/mutate ECS state
  out-of-process (e.g. `bevy_brp_mcp`). See [`bevy-mcp-bridges.md`](bevy-mcp-bridges.md).
- **BRP (Bevy Remote Protocol)** — Bevy's built-in JSON-RPC remote interface
  (`RemotePlugin`) for inspecting/mutating entities and components in a live app;
  the channel the Bevy MCP bridges sit on top of.

## AccessKit terms (the action model)

- **AccessKit `ActionRequest`** — the `{ action, target: NodeId, data }` struct an
  adapter hands the producer when an AT (or, in Buiy's thesis, an agent) asks to
  perform an action. On a Buiy-owned window it is delivered to Buiy's own
  `ActionHandler::do_action`, not bevy_winit's. Pinned to the `accesskit` 0.24
  line; detail in [../accesskit/](../accesskit/).
- **One-tree-per-window** — AccessKit enforces a single tree per
  `accesskit_winit::Adapter` per window, keyed by winit `WindowId`; `NodeId`s are
  per-tree-scoped (can overlap across windows). The reason an agent ref must carry
  the window. See [../accesskit/](../accesskit/), [`open-problems.md`](open-problems.md) § 9.

For how these terms map onto Buiy's AccessKit-first thesis (the semantic tree as
the perception+control surface), see [`lessons.md`](lessons.md).

## Sources

- https://modelcontextprotocol.io/specification/versioning
- https://modelcontextprotocol.io/specification/2025-11-25/changelog
- https://blog.modelcontextprotocol.io/posts/2025-12-09-mcp-joins-agentic-ai-foundation/
- https://github.com/modelcontextprotocol/rust-sdk
- https://crates.io/crates/rmcp
- https://github.com/microsoft/playwright-mcp
- https://github.com/natepiano/bevy_brp
- https://arxiv.org/abs/2310.11441
- https://arxiv.org/abs/2405.15793
- https://github.com/SWE-agent/SWE-agent/blob/main/docs/background/aci.md
- https://www.anthropic.com/engineering/writing-tools-for-agents
