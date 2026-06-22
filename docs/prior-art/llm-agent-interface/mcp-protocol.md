**Date:** 2026-06-18
**Status:** active
**Subject:** Model Context Protocol — JSON-RPC base, server/client primitives, transports, capability negotiation & lifecycle, revision history, rmcp (Rust SDK)

# Model Context Protocol (MCP)

MCP is an open protocol that standardizes how applications expose context and capabilities to LLMs. It is relevant prior art for Buiy because it already separates a long-lived, app-controlled *perception* surface (resources + subscriptions) from a model-controlled *act* surface (tools + result snapshot) — the same split a bidirectional AccessKit tree would need. This file documents the protocol itself. For the consumer end (a browser agent driven over its accessibility tree) see [playwright-mcp.md](playwright-mcp.md); for tool-shape design see [aci-tool-design.md](aci-tool-design.md); for the Bevy↔MCP bridges see [bevy-mcp-bridges.md](bevy-mcp-bridges.md).

## Base: JSON-RPC 2.0

All MCP messages are **JSON-RPC 2.0**, UTF-8 encoded — requests (with `id`), responses, and notifications (no `id`, no reply). JSON-RPC batching was added in 2025-03-26 and **removed** in 2025-06-18. The default JSON Schema dialect is **2020-12** (a tool may override per-schema with a `$schema` field, e.g. draft-07). MCP layers a connection lifecycle and a set of typed methods on top of this base.

## Three server primitives

The three server-side primitives are deliberately keyed to *who drives them*:

- **Tools** — **model-controlled**. The LLM discovers (`tools/list`) and invokes (`tools/call`) tools from context. A tool definition carries a `name`, human-readable `description`, an **`inputSchema`** (JSON Schema object, required, not `null`), an optional **`outputSchema`**, and optional `annotations` (behavior hints) and `icons`. `tools/call` returns a **`CallToolResult`**: unstructured `content` (text / image / audio / `resource_link` / embedded `resource`) and/or **`structuredContent`** (a JSON object; when an `outputSchema` is declared the server MUST conform and clients SHOULD validate). The result carries an **`isError`** flag: tool-execution errors set `isError: true` *inside* the result so the model sees them and can self-correct, distinct from JSON-RPC **protocol errors** (unknown tool, malformed request) which are reported as JSON-RPC errors. If the tool list changes the server emits `notifications/tools/list_changed` (gated by the `tools.listChanged` capability).
  - **Tool annotations** (2025-03-26) are hints: `readOnlyHint`, `destructiveHint`, `idempotentHint`, `openWorldHint`, plus `title`. The spec is explicit that clients **MUST consider tool annotations untrusted unless they come from trusted servers**. These annotations are the wire-level hint a host uses to decide which verbs need confirmation gating — see [open-problems.md § 4](open-problems.md) for where that gate should live.
  - **Tool names** SHOULD be 1–128 chars, case-sensitive, `[A-Za-z0-9_.-]` only (standardized in 2025-11-25, SEP-986).

- **Resources** — **application-driven** (host decides how/whether to feed them to the model; *not* model-controlled). Each resource is identified by a **URI** (`file://`, `https://`, `git://`, or custom schemes per RFC 3986; URI *templates* per RFC 6570 for parameterized resources). Methods: `resources/list` (paginated), `resources/templates/list`, `resources/read` (returns `text` or base64 `blob` contents). Two optional sub-capabilities:
  - **`subscribe`** — client calls `resources/subscribe` for a specific URI; the server then pushes **`notifications/resources/updated`** (carrying the changed `uri`) whenever that resource changes, prompting a fresh `resources/read`.
  - **`listChanged`** — server emits **`notifications/resources/list_changed`** when the *set* of resources changes.
  Resources, templates, and content blocks support `annotations` (`audience: ["user"|"assistant"]`, `priority` 0.0–1.0, `lastModified` ISO-8601). Note: whether the **2024-11-05** initial revision shipped resource subscriptions as a named feature at launch vs. added shortly after is *(unverified)* — subscriptions exist in the spec; launch-date attribution is unconfirmed.

- **Prompts** — **user-controlled**: parameterized message templates a user explicitly selects (e.g. slash-commands), discovered via `prompts/list`, fetched via `prompts/get`; `notifications/prompts/list_changed` under `prompts.listChanged`.

## Client features

The client (host) can also expose capabilities back to the server, inverting the usual direction:

- **Sampling** — the server asks the client to run an LLM completion (`sampling/createMessage`), so a server can use the model without holding its own API key; the human stays in the loop. 2025-11-25 added **tool calling in sampling** (`tools`/`toolChoice`), enabling server-driven agent loops (SEP-1577).
- **Elicitation** (added **2025-06-18**) — the server requests structured input from the user mid-session via `elicitation/create` + a JSON schema; 2025-11-25 added a **URL mode**. This is the protocol-level analogue of a server asking the UI to pop a form, and the natural wire mechanism for a confirmation gate before a destructive action.
- **Roots** — the client advertises filesystem (or URI) **roots** that scope what the server should operate on, with `notifications/roots/list_changed`.

## Transports

Two standard transports (the protocol is otherwise transport-agnostic; custom transports are allowed):

- **stdio** — the **client launches the server as a subprocess** and exchanges newline-delimited JSON-RPC over the child's `stdin`/`stdout`; `stderr` is free-form logging. Clients SHOULD support stdio whenever possible. This is the natural fit for a local app embedding a server in-process or as a child.
- **Streamable HTTP** (added **2025-03-26**, replacing the old HTTP+SSE transport) — the server exposes a **single MCP endpoint** handling POST (client→server messages) and GET. The server MAY upgrade a response to **Server-Sent Events** (`text/event-stream`) to stream multiple messages and to push **server→client requests/notifications** (e.g. `resources/updated`). Stateful **sessions** are carried by an **`MCP-Session-Id`** header assigned at `initialize` and echoed on every later request; SSE event IDs + `Last-Event-ID` give resumability. Security: servers MUST validate `Origin` (DNS-rebinding defense) and SHOULD bind to localhost when local. Over HTTP the client MUST send `MCP-Protocol-Version` on every request after init.

The SSE/notification direction of Streamable HTTP is what makes a *changing* surface (resource updates, list-changed) expressible as push rather than poll — the property Buiy's perception surface would want if it ever spoke MCP.

## Capability negotiation & lifecycle

Three phases: **initialization → operation → shutdown**. The client opens with an `initialize` request carrying its `protocolVersion`, `capabilities`, and `clientInfo`; the server replies with *its* `protocolVersion`, `capabilities`, `serverInfo`, and optional `instructions`; the client then sends `notifications/initialized`. **Version negotiation:** client sends its latest supported revision; if unsupported the server replies with one it does support, and the client disconnects if it can't match.

**Capabilities** gate every optional feature — both sides may only use what was negotiated. Client side: `roots`, `sampling`, `elicitation`, `tasks`, `experimental`. Server side: `prompts`, `resources`, `tools`, `logging`, `completions`, `tasks`, `experimental`. Sub-capabilities: `listChanged` (prompts/resources/tools) and `subscribe` (resources only). **Shutdown** has no dedicated message — for stdio the client closes the child's stdin then SIGTERM/SIGKILL; for HTTP it closes the connection (or sends `DELETE` with the session id).

## Revision history

The revision id is a date `YYYY-MM-DD` = the last day a **backwards-incompatible** change landed (it is *not* bumped for compatible changes).

- **2024-11-05** — initial revision (Anthropic; David Soria Parra & Justin Spahr-Summers). Core primitives resources, prompts, tools, sampling; stdio + HTTP+SSE transports.
- **2025-03-26** — OAuth 2.1 authorization; **Streamable HTTP** (replaces HTTP+SSE); **tool annotations**; audio content; JSON-RPC batching (later removed).
- **2025-06-18** — **elicitation**; **structured tool output** (`structuredContent`); MCP servers as OAuth 2.0 Resource Servers (RFC 8707); **removed** JSON-RPC batching.
- **2025-11-25** — **CURRENT** (MCP's 1-year anniversary). Experimental **Tasks** (durable async "call-now, fetch-later", SEP-1686); **tool calling in sampling** (SEP-1577); `icons` metadata (SEP-973); OIDC Discovery + OAuth Client ID Metadata Documents; URL-mode elicitation; standardized tool-name guidance (SEP-986); JSON Schema 2020-12 default.

A **2026-07-28** release candidate exists (RC locked ~May 2026) but is dated *after* today (2026-06-18) and is **not released** — its contents are not current and are not cited here.

**Governance:** introduced by **Anthropic** (Nov 2024). On **2025-12-09 Anthropic donated MCP to the Agentic AI Foundation (AAIF)** — a directed fund under the Linux Foundation, co-founded by Anthropic, Block, and OpenAI (with support from Google, Microsoft, AWS, Cloudflare, and Bloomberg). MCP joined alongside Block's `goose` and OpenAI's `AGENTS.md` as the three founding projects. This puts MCP under neutral, community governance rather than single-vendor stewardship. (Confirmed first-party: the MCP blog post, the Linux Foundation press release, and Anthropic's own news post, all dated 2025-12-09.)

## SDKs — `rmcp` (official Rust SDK)

- **Repo:** https://github.com/modelcontextprotocol/rust-sdk (crate **`rmcp`**, description "Rust SDK for Model Context Protocol"). **License Apache-2.0.** Maintained by the `modelcontextprotocol` org.
- **Version:** crates.io is authoritative — the maximum published version is **1.7.0** (2026-05-13), matching release tag `rmcp-v1.7.0`, license Apache-2.0. (The repo README's quickstart snippet still shows an older `0.16.0` in its example `Cargo.toml` line; that is stale copy in the README, not the published version. Do not cite 0.16.0 as current.)
- **Transports (verified against `crates/rmcp/Cargo.toml`):** the transport feature flags are `transport-io` (stdio), `transport-child-process` (`TokioChildProcess`), and `transport-streamable-http-server` / `transport-streamable-http-client` (Streamable HTTP — Server-Sent Events streaming is folded into the HTTP transport; there is no longer a standalone `transport-sse-*` feature). So stdio, child-process, and Streamable-HTTP (incl. its SSE direction) are all first-class.
- **Macros:** `#[tool]`, `#[tool_router]`, `#[tool_handler]`, `#[prompt]`/`#[prompt_router]`/`#[prompt_handler]`, `#[task_handler]` — auto-generate JSON-Schema from typed Rust handler signatures, so a server author writes ordinary `async fn`s with `schemars`-derived argument structs.

Official SDKs also exist for TypeScript, Python, Kotlin, C#, Go, Swift, Java, and Ruby (the TS and Python SDKs are the reference implementations); only `rmcp` is load-bearing for a Rust/Bevy project.

## Why this split maps onto Buiy

MCP's primitive taxonomy is the cleanest articulation of the perception/act split Buiy needs (decisions live in [lessons.md](lessons.md), not here):

- **Resources + `subscribe` → `notifications/resources/updated`** model a *long-lived, changing* surface as addressable state with push-on-change. That is structurally what a live AccessKit semantic tree is: a stable set of URI-addressable nodes whose name/state/value mutate over frames. The subscription→updated→re-read loop is exactly how a screen reader (or an agent) tracks a UI that keeps changing under it. (The *timing* — how often Buiy would re-serialize the tree, and whether it diffs — is an open design question; see [open-problems.md § 2](open-problems.md).)
- **Tools + `CallToolResult` (with `isError` + a result snapshot)** model the *tight act loop*: invoke an action, get back a self-contained snapshot of what happened, retry on `isError`. AccessKit `ActionRequest`s (the bidirectional channel Buiy would add) are the same shape — a typed action in, an observable post-state out. The handler that receives them on a Buiy-owned window is Buiy's own `ActionHandler::do_action`, not bevy_winit's.
- **Tool vs protocol error distinction** is the same discipline an action-dispatch surface needs: malformed requests fail loudly to the caller, but "the button refused" should come back as observable state the model can react to, not a transport fault.
- **Tasks (2025-11-25, experimental)** model the *async* case the tight act loop cannot: a long-running UI action (an animation completing, an async data load) whose result is fetched later rather than inline. A Buiy `wait_for`-style or animation-completing action is a candidate for this shape; see [open-problems.md § 8](open-problems.md).

The takeaway is the *separation*, not adopting MCP wholesale: MCP is JSON-RPC-over-a-pipe between *processes*, whereas Buiy's tree lives in-process in the ECS. See [aci-tool-design.md](aci-tool-design.md) for how tool/result shape affects agent success, and [open-problems.md](open-problems.md) for the unresolved questions (snapshot granularity, subscription churn, action acknowledgement, confirmation gating, multi-window addressing).

## Sources

- MCP spec — Resources (2025-11-25): https://modelcontextprotocol.io/specification/2025-11-25/server/resources
- MCP spec — Tools (2025-11-25): https://modelcontextprotocol.io/specification/2025-11-25/server/tools
- MCP spec — Lifecycle (2025-11-25): https://modelcontextprotocol.io/specification/2025-11-25/basic/lifecycle
- MCP spec — Transports (2025-11-25): https://modelcontextprotocol.io/specification/2025-11-25/basic/transports
- MCP spec — Versioning: https://modelcontextprotocol.io/specification/versioning
- MCP spec — 2025-11-25 changelog: https://modelcontextprotocol.io/specification/2025-11-25/changelog
- rmcp crate (crates.io, max published 1.7.0): https://crates.io/crates/rmcp
- rmcp repo + `crates/rmcp/Cargo.toml` (transport feature flags): https://github.com/modelcontextprotocol/rust-sdk
- MCP joins the Agentic AI Foundation (2025-12-09): https://blog.modelcontextprotocol.io/posts/2025-12-09-mcp-joins-agentic-ai-foundation/
- Linux Foundation — AAIF formation press release: https://www.linuxfoundation.org/press/linux-foundation-announces-the-formation-of-the-agentic-ai-foundation
- Anthropic — donating MCP / establishing the AAIF: https://www.anthropic.com/news/donating-the-model-context-protocol-and-establishing-of-the-agentic-ai-foundation
