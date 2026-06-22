**Date:** 2026-06-18
**Status:** active
**Subject:** Anthropic's Agent-Computer-Interface (ACI) tool-design discipline, applied to a Buiy "drive the UI" toolset

# Agent-Computer-Interface (ACI) tool-design discipline

Two Anthropic Engineering pieces define the discipline for designing tools an LLM
agent calls: **"Building Effective Agents"** (Erik Schluntz & Barry Zhang,
2024-12-19) and **"Writing effective tools for AI agents — with agents"**
(Anthropic Engineering, 2025-09-11). The second names the lens explicitly:
**ACI — the agent-computer interface** — and argues you should *"invest in the ACI
as much as the HCI."* The term itself comes from the **SWE-agent** paper (Yang et
al., NeurIPS 2024), which showed that custom-built tool interfaces — not model
weight changes — drove a large jump on SWE-bench, framing tool design as a UX
discipline for non-human callers.

The throughline: **tools are a contract between deterministic systems and
non-deterministic agents.** The agent has limited context and reasons in
natural-language tokens, so the interface must be shaped for *how the model
thinks*, not for how the underlying API is structured.

## The principles

- **Few, semantic, consolidated tools beat 1:1 API wrapping.** *"More tools
  don't always lead to better outcomes."* The canonical example: instead of
  `list_users` + `list_events` + `create_event`, ship one `schedule_event` tool
  that finds availability and books it. Each tool should map to a *task the agent
  performs*, collapsing multi-step plumbing into one call.

- **Return compact, structured, high-signal state.** Prefer *"contextual
  relevance over flexibility."* Return human-readable fields (`name`, semantic
  role) over opaque identifiers (`uuid`, internal handles). Build in pagination,
  filtering, range selection, and truncation with sensible defaults — Claude Code
  caps tool responses at ~25,000 tokens by default. The 2025-06-18 MCP revision's
  `structuredContent` (see [mcp-protocol.md](mcp-protocol.md)) is the wire format
  for this.

- **Errors as guidance, not tracebacks.** Make error responses *"clearly
  communicate specific and actionable improvements, rather than opaque error
  codes or tracebacks."* A failed action should tell the agent what to do next.

- **Name and describe tools for the model.** Tool descriptions are
  prompt-engineered context — *"even small refinements to tool descriptions can
  yield dramatic improvements."* Treat each like *"a great docstring for a junior
  developer"*: examples, edge cases, unambiguous parameter names (`user_id`, not
  `user`). Use **namespacing** with consistent prefixes (`asana_projects_search`,
  `asana_users_search`) so the agent picks the right tool.

- **Poka-yoke (mistake-proofing).** *"Change the arguments so that it is harder
  to make mistakes."* Anthropic's SWE-bench agent required **absolute** filepaths
  instead of relative ones to prevent navigation errors. SWE-agent's analogue: a
  linter that *blocks* a syntactically-invalid edit before it lands. One
  constraint change can eliminate an entire failure class.

- **Put yourself in the model's shoes.** *"Is it obvious how to use this tool,
  based on the description and parameters, or would you need to think carefully
  about it?"* Keep formats close to *"what the model has seen naturally occurring
  in text on the internet"*; avoid formatting overhead (line counting,
  string-escaping); *"give the model enough tokens to 'think' before it writes
  itself into a corner."*

- **Evaluation-transcript-driven refinement.** Improve tools by reading agent
  transcripts and reasoning traces, not by guessing — Anthropic reports *"most of
  the advice in this post came from repeatedly optimizing our internal tool
  implementations with Claude Code."*

## Applied: a Buiy "drive the UI" toolset

Buiy already authors an AccessKit semantic tree (role + name + state + actions).
The ACI-correct toolset exposes *that tree*, not the ECS world, as a small set of
semantic verbs. A reasonable starting roster, mirroring Playwright-MCP's
accessibility-snapshot-first model (see [playwright-mcp.md](playwright-mcp.md)):

| Tool | Role |
|------|------|
| `snapshot` | Return the current AccessKit tree (roles, names, states, available actions, stable node refs). The agent's perception surface. |
| `click` | Invoke the default action on a node by ref — maps to an AccessKit `ActionRequest` dispatched through Buiy's own `ActionHandler`. |
| `type` / `set_value` | Enter text / set a value on a focusable node. |
| `focus` | Move focus to a node. |
| `press_key` | Send a key chord (Tab, Enter, Escape, arrows). |
| `wait_for` | Block until a condition holds (node appears, text present, state settles) — replaces arbitrary sleeps. |

**Auto-snapshot-after-action.** Each mutating tool (`click`, `type`,
`set_value`, `press_key`) should return the *resulting* snapshot inline, so the
agent perceives the new state without a separate round-trip. This is the
consolidation principle: fold "act then observe" into one call, the way
`schedule_event` folds find-then-book. Playwright-MCP does exactly this. (The
catch: it assumes the action *settles synchronously* within the response — an
animation or async load that completes later breaks the assumption. That async
case is where MCP Tasks (call-now/fetch-later) belong; see
[open-problems.md § 8](open-problems.md).)

**Stable refs are poka-yoke.** Acting by semantic node ref (from the last
snapshot) rather than by pixel coordinate or raw `Entity` id makes a whole class
of mistakes impossible — there is no "click at (x,y) and miss." A stale ref should
return a guidance error ("node gone; re-snapshot"), not a panic. Note the ref must
carry its window/tree: AccessKit `NodeId`s are per-tree-scoped, so in a
multi-window app the ref is a `(window_id, NodeId)` pair (see
[open-problems.md § 9](open-problems.md)).

**Capability tiers + the confirmation gate.** Mirror Playwright-MCP's `--caps`
gating: a default safe tier (snapshot + click + type + focus + wait_for) and
opt-in tiers for riskier surfaces (raw key injection, value coercion,
screenshots/vision, raw-ECS/BRP debug). MCP's `2025-03-26` destructive/read-only
**tool annotations** (see [mcp-protocol.md](mcp-protocol.md)) let the host surface
which verbs mutate, and `destructiveHint` is the natural per-action signal for
routing a verb through a human confirmation gate (via MCP elicitation, or a Buiy
confirmation widget). *Where* that gate lives — per-action annotation, capability
tier, or a dedicated confirmation widget — is a real design question treated in
[open-problems.md § 4](open-problems.md), not settled here.

## What NOT to do

- **One-tool-per-endpoint sprawl.** Do not generate a tool per widget type or
  per ECS system. That is the `list_users`/`list_events` anti-pattern at UI
  scale — it floods context and forces the agent to assemble multi-step plans the
  toolset should have consolidated.

- **Raw-ECS dumps.** Do not return the full component graph or a `World` dump as
  the "snapshot." It is the opposite of high-signal: enormous, full of opaque
  identifiers and engine-internal state the agent must filter. The AccessKit
  tree is the pre-filtered, semantic, screen-reader-grade view — use it. This is
  the central caution against generic Bevy-MCP/BRP bridges that expose ECS
  queries directly (see [bevy-mcp-bridges.md](bevy-mcp-bridges.md)).

- **Pixel-coordinate actions as the primary path.** Coordinate clicking
  (computer-use style) is a fallback, not the interface; it discards the
  structure Buiy already has and reintroduces miss-the-target failures.

The validates/avoid/borrow distillation of these points for Buiy lives in
[lessons.md](lessons.md).

## Sources

- https://www.anthropic.com/engineering/writing-tools-for-agents
- https://www.anthropic.com/research/building-effective-agents
- https://arxiv.org/abs/2405.15793 (SWE-agent: Agent-Computer Interfaces Enable Automated Software Engineering)
- https://github.com/SWE-agent/SWE-agent/blob/main/docs/background/aci.md
- https://github.com/microsoft/playwright-mcp
- https://modelcontextprotocol.io/specification/2025-03-26 (tool annotations)
