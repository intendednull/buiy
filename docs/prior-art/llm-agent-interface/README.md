**Date:** 2026-06-18
**Status:** active
**Subject:** MCP + LLM-native UI control — folder index, key facts, and reading order for the agent-perception/control prior art

# LLM agent interface — MCP + LLM-native UI control

This folder collects the external prior art on **how LLM agents perceive and drive a user interface**: the Model Context Protocol (MCP) as the LLM-facing transport for tools/resources/prompts, the two competing UI-control paradigms (structured **accessibility-tree-first** control vs. pixel-level **computer use**), the Bevy-specific MCP↔app bridges, and Anthropic's tool/agent-design guidance (the ACI framing). It exists to answer one question for Buiy: *if the AccessKit semantic tree Buiy already authors were made bidirectional, what would the LLM-agent perception+control surface look like, and what does the field already know about building it?* The evidence files stay factual and version-pinned; the Buiy-specific verdicts live only in [`lessons.md`](lessons.md).

## Key facts

| Subject | What it is | Version / date | Maintainer | License | Repo |
|---|---|---|---|---|---|
| **MCP spec** | LLM-facing protocol: resources, prompts, tools, sampling, elicitation, tasks | rev **`2025-11-25`** (current); a `2026-07-28` RC exists but is **not released** | Anthropic → **AAIF** under the Linux Foundation (2025-12-09) | spec text open | [spec](https://modelcontextprotocol.io/specification/2025-11-25/changelog) · [repo](https://github.com/modelcontextprotocol/modelcontextprotocol) |
| **`rmcp`** | Official Rust SDK for MCP (`#[tool]` macros, stdio/child-process/HTTP transports) | crates.io max published **1.7.0** (2026-05-13), matching tag `rmcp-v1.7.0` | `modelcontextprotocol` org | **Apache-2.0** | [rust-sdk](https://github.com/modelcontextprotocol/rust-sdk) |
| **Playwright-MCP** | Accessibility-snapshot-first browser-control MCP server (no vision model by default) | **v0.0.76** (2026-06-10, npm publish timestamp), pre-1.0 | Microsoft | Apache-2.0 | [playwright-mcp](https://github.com/microsoft/playwright-mcp) |
| **Anthropic computer use** | Pixel-level GUI agent: screenshot → mouse/keyboard actions → screenshot | tool `computer_20251124`, header `computer-use-2025-11-24`; **still BETA** | Anthropic | proprietary API | [docs](https://platform.claude.com/docs) |
| **bevy_brp_mcp** | Separate-process MCP server bridging MCP clients ↔ a running Bevy app via the Bevy Remote Protocol (BRP) | **0.19.0** (2026-03-23); crates.io latest is **0.20.0-rc.1** (2026-05-24) | natepiano | MIT OR Apache-2.0 | [bevy_brp](https://github.com/natepiano/bevy_brp) |
| **AccessKit** | The cross-platform a11y tree Buiy authors (role+name+state+actions) — the linchpin of the whole thesis | pinned in the sibling folder: `accesskit` 0.24.0 / `accesskit_winit` 0.33.0 / `accesskit_consumer` 0.36.0 | Pneuma Solutions | MIT OR Apache-2.0 | [accesskit/](../accesskit/) |
| **ACI guidance** | Anthropic's "agent-computer interface" framing + tool-writing playbook | "Building Effective Agents" 2024-12-19; "Writing tools for agents" 2025-09-11 | Anthropic | articles | [research](https://www.anthropic.com/research/building-effective-agents) |

Versions/dates in this area move fast; "current" = as of **2026-06-18**. Each evidence file re-flags anything that may have gone stale. AccessKit specifics (NodeId = u64, the `Action` vocabulary, `ActionRequest{action,target,data}`) refer to the **0.24 line** pinned above; detail lives in [../accesskit/](../accesskit/), not here.

## How to use this folder

_Framing disclosure (added at finalize): this folder was assembled as prior-art research for Buiy's agent-interface design, written from Buiy's AccessKit-semantic-tree-first agent-surface stance. It is a learn-from artifact, not a neutral catalog: evidence files describe external systems as they are, but the selection of what to document and the "Implications for Buiy" / "Why this matters" framings are oriented around the thesis that Buiy's already-authored AccessKit tree is the right bidirectional LLM-agent perception+control surface. Buiy-specific judgments are isolated in [`lessons.md`](lessons.md) as validates/borrow/avoid, and Buiy design decisions are NOT baked into the evidence files._

Start at this README, then follow the reading order below. Use the [glossary](glossary.md) for unfamiliar terms (MCP primitive names, ACI, BRP, elicitation, Streamable HTTP). When consulting this folder during spec/plan/review work, treat each evidence file as a launchpad for fresh online research rather than a frozen snapshot — re-verify any version number before citing it.

## Table of contents

- [`README.md`](README.md) — this index.
- [`mcp-protocol.md`](mcp-protocol.md) — the MCP spec: primitives (resources/prompts/tools/sampling/elicitation/tasks), transports, revision history, governance, and the `rmcp` Rust SDK.
- [`playwright-mcp.md`](playwright-mcp.md) — Microsoft's accessibility-snapshot-first browser-control server: the snapshot→act-by-ref loop, tool roster, and capability tiers.
- [`computer-use-and-gui-agents.md`](computer-use-and-gui-agents.md) — the pixel-level paradigm (Anthropic computer use + the broader GUI-agent line) and why structured trees beat screenshots when a tree exists.
- [`bevy-mcp-bridges.md`](bevy-mcp-bridges.md) — `bevy_brp_mcp`, `bevy_mcp`, `bevy_debugger_mcp`: how the Bevy ecosystem already exposes a running app to MCP clients via BRP.
- [`aci-tool-design.md`](aci-tool-design.md) — Anthropic's agent-computer-interface and tool-design guidance: tools as contracts, evaluation-driven refinement, token-efficient responses.
- [`open-problems.md`](open-problems.md) — unsettled questions: tree-snapshot freshness/staleness, action acknowledgement, security/confused-deputy, confirmation-gating destructive actions, multi-window node addressing, async/long-running actions, latency.
- [`lessons.md`](lessons.md) — **Buiy-facing** verdicts: what this prior art validates, what to borrow, what to avoid.
- [`glossary.md`](glossary.md) — terms used across the folder.

## Glossary

Term definitions (MCP primitives, ACI, BRP, elicitation, sampling, Streamable HTTP, snapshot mode vs. vision mode, act-by-ref) live in [`glossary.md`](glossary.md).

## Canonical reading order

1. [`mcp-protocol.md`](mcp-protocol.md) — establishes the transport and primitive vocabulary everything else assumes.
2. [`playwright-mcp.md`](playwright-mcp.md) — the closest existing analogue to the Buiy thesis: a structured accessibility tree as the agent's perception+control surface.
3. [`computer-use-and-gui-agents.md`](computer-use-and-gui-agents.md) — the contrast paradigm (pixels) that motivates choosing the tree.
4. [`bevy-mcp-bridges.md`](bevy-mcp-bridges.md) — what already exists in Bevy's own ecosystem.
5. [`aci-tool-design.md`](aci-tool-design.md) — how to shape the tools/responses once the surface exists.
6. [`open-problems.md`](open-problems.md) — the hard parts still unsolved.
7. [`lessons.md`](lessons.md) — pull it together for Buiy.

## Why this matters for Buiy

Buiy already authors an AccessKit semantic tree — role + name + state + supported actions, per node — but today it is **output-only**, pushed to screen readers via Buiy's own per-window `accesskit_winit::Adapter` (keyed by winit `WindowId`). The thesis these files serve: *that same tree is the right LLM-agent perception+control surface.* Playwright-MCP is the live proof point — it drives the web by an accessibility snapshot, not pixels, and Anthropic's own guidance says to invest in the ACI as much as the HCI. The missing half is making the tree **bidirectional**: an agent reads a structured snapshot, then acts by node reference (`Click`/`Focus`/`SetValue` on a stable id), and the resulting AccessKit `ActionRequest` is consumed through **Buiy's own `accesskit_winit::Adapter` / `ActionHandler::do_action` plumbing** — the same `ActionHandler` path a screen reader already uses on a Buiy-owned window (bevy_winit's accessibility path is not running on those windows; see [../accesskit/](../accesskit/) §architecture and Buiy's `architecture.md` §2.6). MCP is the natural LLM-facing transport to wrap that snapshot→act-by-ref loop. Whether Buiy adopts this — and how — is decided in [`lessons.md`](lessons.md), not here.

## Sources

- https://modelcontextprotocol.io/specification/2025-11-25/changelog
- https://modelcontextprotocol.io/specification/versioning
- https://github.com/modelcontextprotocol/modelcontextprotocol
- https://github.com/modelcontextprotocol/rust-sdk
- https://crates.io/crates/rmcp
- https://github.com/microsoft/playwright-mcp
- https://www.npmjs.com/package/@playwright/mcp
- https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool
- https://github.com/natepiano/bevy_brp
- https://crates.io/crates/bevy_brp_mcp
- https://www.anthropic.com/research/building-effective-agents
- https://www.anthropic.com/engineering/writing-tools-for-agents
- https://blog.modelcontextprotocol.io/posts/2025-12-09-mcp-joins-agentic-ai-foundation/
