**Date:** 2026-06-18
**Status:** active
**Subject:** Lessons for Buiy from the LLM-agent-interface corpus — what it validates, what to avoid, what to borrow

# Lessons for Buiy

The decision file for `docs/prior-art/llm-agent-interface/`. The evidence sits in the
sibling files — this file turns it into **validates / avoid / borrow** calls against Buiy's
thesis: *the AccessKit semantic tree Buiy already authors (role + name + state + actions) is
the right LLM-agent perception+control surface, made bidirectional by consuming AccessKit
`ActionRequest`s through Buiy's own per-window `accesskit_winit::Adapter` /
`ActionHandler::do_action` plumbing* (the same handler a screen reader hits on a Buiy-owned
window — bevy_winit's accessibility path is not running there; see
[../accesskit/](../accesskit/) and Buiy's `architecture.md` §2.6). Evidence files stay
factual; the design judgments live here. Start from [README.md](README.md) for the map; terms
in [glossary.md](glossary.md); unresolved questions in [open-problems.md](open-problems.md).

---

## Validates — the bets the corpus supports

- **A11y-tree-first perception.** Playwright-MCP's default mode operates on the browser
  accessibility tree, not pixels, precisely because it is structured, deterministic, and
  vision-model-free (~200–400 tokens/snapshot vs thousands for DOM or screenshots). Buiy's
  bet — feed the agent the same tree screen readers consume — is the proven design, not a
  novel gamble. See [playwright-mcp.md](playwright-mcp.md).

- **Inverted lossiness — Buiy authors the tree.** For browsers and OS adapters the a11y tree
  is *reverse-engineered* from a DOM/widget hierarchy and is lossy. Buiy authors the
  AccessKit tree as the **source** of its UI semantics, so role/name/state/actions are
  first-class, not inferred. The usual "a11y tree is incomplete" critique
  ([computer-use-and-gui-agents.md](computer-use-and-gui-agents.md)) applies far less to a
  toolkit that owns the tree. See [../accesskit/](../accesskit/) and [../bevy-a11y/](../bevy-a11y/).

- **MCP as the LLM-facing transport.** MCP is the de-facto standard (introduced by Anthropic
  Nov 2024; donated to the Agentic AI Foundation under the Linux Foundation on 2025-12-09 —
  open, neutral governance, not single-vendor). An official Rust SDK, `rmcp` (crates.io max
  published **1.7.0**, 2026-05-13, **Apache-2.0**), exists in in-process-friendly form. Buiy
  speaking MCP buys interoperability with every MCP client. See [mcp-protocol.md](mcp-protocol.md).

- **In-process plugin (not separate-process bridge).** Buiy owns the AccessKit tree inside
  its own ECS world *and owns its own per-window `accesskit_winit::Adapter`*, so a Bevy
  plugin can read the tree and inject `ActionRequest`s directly through Buiy's own
  `ActionHandler` — no second process, no protocol round-trip to its own state. `rmcp`
  supports this (stdio / child-process / Streamable-HTTP transports from inside the app).
  This also *shrinks the security surface* (no untrusted third-party tool descriptions, no
  separate-process trust boundary — see [open-problems.md § 4](open-problems.md)). Contrast
  the bridge model below. See [mcp-protocol.md](mcp-protocol.md) and
  [bevy-mcp-bridges.md](bevy-mcp-bridges.md).

---

## Avoid — the traps the corpus warns against

- **Pixel-only perception as the spine.** Anthropic computer use (still **beta** as of
  2026-06-18: `computer_20251124`, header `computer-use-2025-11-24`) loops
  screenshot→coordinate-action→screenshot. It works anywhere but is slow, token-heavy,
  non-deterministic, and blind to off-screen/occluded state. Keep pixels as an opt-in *vision
  tier*, never the primary channel. See
  [computer-use-and-gui-agents.md](computer-use-and-gui-agents.md).

- **Exposing raw ECS as the UI surface (the `bevy_brp_mcp` trap).** `bevy_brp_mcp` (v0.19.0,
  natepiano `bevy_brp` monorepo, MIT OR Apache-2.0; latest 0.20.0-rc.1) bridges MCP to a
  running Bevy app via BRP — but it surfaces *components/entities/reflection*, not UI
  semantics. Its own `brp_type_guide` tool exists because the registry schema does not tell
  an agent the JSON shape spawn/insert/mutate expect, forcing trial-and-error. Buiy must
  expose **role/name/state/action** semantics, not `Transform`/`Node` internals. See
  [bevy-mcp-bridges.md](bevy-mcp-bridges.md).

- **One-tool-per-endpoint sprawl.** Anthropic's tool-writing guidance (2025-09-11) warns that
  many thin tools bloat context and confuse agents; consolidate into few high-value semantic
  tools. A Buiy tool-per-action-kind explosion would repeat the mistake. See
  [aci-tool-design.md](aci-tool-design.md).

- **Coupling to a fast-churning spec without version-pinning.** MCP's revision id is the date
  of the last *breaking* change (2024-11-05 → 2025-03-26 → 2025-06-18 → **2025-11-25**
  current; a 2026-07-28 RC exists but is **not released**). Playwright-MCP is still 0.0.x
  (v0.0.76) with a churning `--caps` roster. Pin the MCP revision and `rmcp` version; gate
  new primitives (Tasks, tool-calling-in-sampling) behind capability checks. See
  [mcp-protocol.md](mcp-protocol.md) and [playwright-mcp.md](playwright-mcp.md).

- **Treating a bare `NodeId` as a global agent ref.** AccessKit is one-tree-per-window and
  `NodeId`s are per-tree-scoped, so a bare `NodeId` collides across windows. The agent ref
  must be a `(window_id / tree_id, NodeId)` pair. Designing the ref scheme single-tree is a
  trap that breaks on the first multi-window app. See [open-problems.md § 9](open-problems.md).

---

## Borrow — patterns to lift directly

- **Snapshot → act-by-ref → snapshot-after-action.** Playwright-MCP returns a structured
  snapshot where each interactive element carries a stable `ref` (e.g. `ref=e5`); the agent
  acts by ref (`browser_click { ref }`), and the tool returns a fresh snapshot so the agent
  sees the result. Buiy should mirror this loop exactly — *with the timing model named*: the
  "frame settles" boundary for the post-action snapshot is an open question
  ([open-problems.md § 2](open-problems.md)), and long-running actions may need MCP Tasks
  rather than an inline snapshot ([open-problems.md § 8](open-problems.md)). See
  [playwright-mcp.md](playwright-mcp.md).

- **AccessKit `NodeId` as the stable ref (window-qualified).** AccessKit already *requires* a
  stable id per node and reverse-maps incoming action requests from global ids back to local
  nodes — that is precisely Playwright's `ref` contract, already present in Buiy. Use
  `NodeId` as the agent's element ref, qualified by its window/tree (see the multi-window
  caveat above); no parallel id scheme needed. See [../accesskit/](../accesskit/) and
  [glossary.md](glossary.md).

- **Capability tiers + a real confirmation gate.** Playwright-MCP gates power behind `--caps`
  (vision, pdf, devtools, storage, network, testing, config — roster churns). Buiy should
  default to the read-snapshot + safe-actions tier and gate vision/screenshot, raw-ECS/BRP
  introspection, and destructive actions behind explicit opt-in caps. Destructive actions
  additionally need a human gate; *where* it lives (per-action `destructiveHint` annotation,
  capability tier, or a Buiy confirmation widget via MCP elicitation) and *how it composes
  with AccessKit's fire-and-forget `do_action`* is a design question, not settled here — see
  [open-problems.md § 4](open-problems.md) and [aci-tool-design.md](aci-tool-design.md). See
  [playwright-mcp.md](playwright-mcp.md).

- **ACI few-semantic-tools discipline.** The agent-computer interface (term from SWE-agent;
  Anthropic: "invest in ACI as much as HCI") favors a small set of semantic tools refined
  against evaluation transcripts, with token-efficient responses and clear namespacing.
  Design Buiy's tools as `snapshot / find / invoke-action / set-value / wait-for`, not a tool
  per AccessKit action. See [aci-tool-design.md](aci-tool-design.md).

- **MCP resources + subscriptions for live tree streaming, over ECS change-detection.** MCP
  `resources/subscribe` + `notifications/resources/updated` lets a server push a lightweight
  "changed" signal and let the client re-pull. Expose the AccessKit tree (or subtrees) as MCP
  resources so agents get live UI state without polling — a natural fit for Buiy's *existing*
  diff-style per-frame `TreeUpdate` (the producer already pushes only changed nodes when an
  AT is active). The unresolved part is the re-serialization cadence and the "settled"
  boundary — see [open-problems.md § 2](open-problems.md). See [mcp-protocol.md](mcp-protocol.md).

- **Elicitation / structured output for mid-action prompts.** MCP elicitation
  (`elicitation/create`, 2025-06-18) and `structuredContent` give typed request/response — use
  them for confirmation gates on destructive UI actions (keeping the human-in-the-loop inside
  the app's own UI) and to return typed snapshots. See [mcp-protocol.md](mcp-protocol.md).

---

## Sources

- https://github.com/microsoft/playwright-mcp
- https://playwright.dev/mcp/snapshots
- https://accesskit.dev/how-it-works/
- https://github.com/AccessKit/accesskit
- https://modelcontextprotocol.io/specification/2025-11-25/changelog
- https://modelcontextprotocol.io/legacy/concepts/resources
- https://github.com/modelcontextprotocol/rust-sdk
- https://crates.io/crates/rmcp
- https://blog.modelcontextprotocol.io/posts/2025-12-09-mcp-joins-agentic-ai-foundation/
- https://github.com/natepiano/bevy_brp
- https://crates.io/crates/bevy_brp_mcp
- https://www.anthropic.com/engineering/writing-tools-for-agents
- https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool
