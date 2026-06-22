**Date:** 2026-06-18
**Status:** active
**Subject:** Open problems — what LLM-driven UI control does not yet reliably solve, and which gaps are inherent vs tooling-immaturity

# Open problems

This file collects the honest failure modes of "LLM drives a live UI through a semantic
tree / tool layer." It is the counterweight to the rest of the folder, which mostly
describes what works. Each problem is tagged **[inherent]** (a property of the approach
that better engineering reduces but cannot remove) or **[immaturity]** (a gap that is
plausibly closed as tools/specs mature). The Buiy thesis — AccessKit semantic tree as the
agent's perception+control surface (see [README.md](README.md)) — inherits most of these;
the design lessons drawn from them live in [lessons.md](lessons.md).

## 1. Closed action vocabulary vs arbitrary app transitions — [inherent]

A semantic/accessibility tree exposes a fixed vocabulary of actions (AccessKit `Action`:
`Click`, `Focus`, `SetValue`, `ScrollIntoView`, `Increment`, …; Playwright-MCP: `click`,
`type`, `press_key`, `fill_form`, …; see [aci-tool-design.md](aci-tool-design.md)). Real
applications have transitions that no element-level action names: a drag-to-reorder, a
canvas marquee-select, a chord shortcut, a long-press, a gesture, a "draw here" affordance.
When the needed transition is outside the vocabulary, the agent either cannot perform it or
falls back to coordinate-level pixel control (computer use; see
[computer-use-and-gui-agents.md](computer-use-and-gui-agents.md)) and loses every benefit
of the structured surface. This is **inherent**: any structured action set is a projection
of the app's true interaction space, and the projection is lossy. Mitigation is to widen
the vocabulary (custom actions, `browser_evaluate`-style escape hatches) — but each widening
re-introduces brittleness or pixel dependence. Buiy can add custom AccessKit actions per
widget, but the escape-hatch problem does not disappear.

## 2. Snapshot scaling, freshness, and the re-serialization timing model — [partly inherent]

The accessibility/structured snapshot is the agent's eyes, and it is expensive. Reported
figures: a single tree snapshot "can mean thousands of tokens per step"; "complex
e-commerce pages or dashboards often contain thousands of elements," and "even a single
snapshot can exceed the optimal context window for efficient reasoning." Playwright-MCP
"dumps full accessibility trees on every action," so "agents drown in DOM before finishing
tasks." Production browser-agent stacks respond with hard caps (e.g. a ~50,000-character
snapshot ceiling with "intelligent trimming"), keeping only the most-recent snapshot in
context, and explicit truncation indicators. Tools like Vercel `agent-browser` and
`browser-use` compress the tree to the most relevant nodes.

The token *budget* per step is tooling-addressable (compression, diffs, viewport-scoping,
stable IDs). The deeper problem — that a large UI has more semantic state than fits a
reasoning budget, forcing lossy selection of what the agent sees — is **inherent**: trimming
can hide the very element the task needs.

For Buiy this connects to a concrete, *unnamed* timing question. The folder asserts MCP
`resources/subscribe` + ECS change-detection is a natural fit (see [lessons.md](lessons.md)
and [mcp-protocol.md](mcp-protocol.md)), but the mechanism is undecided: **how often is the
AccessKit tree re-serialized for the agent?** Buiy already builds a diff-style `TreeUpdate`
per frame when an AT is active (the producer pushes only changed nodes — see
[../accesskit/](../accesskit/)), so the natural answer is to reuse that change-tracked diff
rather than re-serialize the whole tree on every snapshot. But the **"frame settles"
boundary** for snapshot-after-action — when an `ActionRequest` has been applied and the
resulting `ResolvedLayout`/a11y state is stable enough to serialize — is not defined.
[playwright-mcp.md](playwright-mcp.md) hand-waves "after the ECS frame settles"; a real
design must name it (one frame? wait until no `Changed` a11y components for a frame?
schedule-ordered after `BuiySet::A11yUpdate`?). Naming the timing model — even as an open
question with a default — is a prerequisite for the snapshot-after-action loop being
deterministic.

## 3. Stale-ref / element-replaced-between-read-and-act — [inherent to async UIs]

The read→act split is racy. Playwright-MCP assigns each node a ref (`ref=e5`); "refs are
stable within a single snapshot … until the page changes. After navigation or DOM updates,
the tool returns a fresh snapshot with new refs. Between snapshots the page can re-render
and the refs go stale … If the agent reuses old refs you'll see 'element not found'
errors." The mitigation is procedural ("always re-snapshot after a navigation, click, or
any action that might change the DOM"), and most tools auto-return a snapshot after each
action — but a re-render *between* a snapshot and the next act still invalidates the ref,
and an SPA can swap a node the agent already decided to click. This is **inherent** to any
async, externally-mutating UI: the world moves between perceive and act. Buiy's ECS gives a
potential edge here (a `NodeId` derived from a stable `Entity` identity is durable, not a
positional ref, and an `ActionRequest` can be validated against current component state at
apply time inside the schedule) — but the agent's *plan* can still target a node whose
semantics changed since the snapshot it reasoned over. Stable identity reduces "wrong
element," not "stale intent."

## 4. Security / authz of an agent driving a live app — [inherent + immaturity]

An LLM that can both read app state and take actions is a confused-deputy waiting to happen.

**The MCP / separate-process case.** A 2025 study of 1,899 open-source MCP servers found
**5.5% exhibited tool-poisoning vulnerabilities** (malicious or altered tool descriptions,
injected responses, redirected data flows). Tools can mutate their own definitions
post-install ("You approve a safe-looking tool on Day 1, and by Day 7 it's quietly rerouted
your API keys"). Simon Willison's framing is the canonical one:

> "LLMs will trust anything that can send them convincing sounding tokens, making them
> extremely vulnerable to confused deputy attacks."

> "Mixing together private data, untrusted instructions and exfiltration vectors is the
> other toxic combination" (the *lethal trifecta*).

> "With multiple servers connected to the same agent, a malicious one can override or
> intercept calls made to a trusted one."

MCP added OAuth 2.1 (2025-03-26), Resource-Server classification (2025-06-18), and tool
annotations, which harden the transport/authz layer — that part is **immaturity** being
closed.

**The in-process case Buiy actually recommends has a *different* threat model.** The
architecture [lessons.md](lessons.md) recommends is an **in-process Buiy plugin** that reads
the AccessKit tree and injects `ActionRequest`s through Buiy's own `ActionHandler` — not a
separate MCP server with untrusted tool descriptions. That removes two of the MCP-specific
threats outright: there are **no untrusted tool descriptions** to poison (the tool roster is
Buiy's own code, not third-party server metadata), and there is **no separate-process trust
boundary** for a malicious co-resident server to intercept. What remains is the residual,
irreducible risk: **injection via rendered app content** — a malicious label, a poisoned
text field, attacker-controlled strings the app *displays* and the agent *reads as part of
the tree*. Prompt injection has no known complete defense (Willison: these vulnerabilities
are "present any time we provide tools to an LLM that can potentially be exposed to
untrusted inputs"), so even the clean in-process design cannot assume the names/values in
its own tree are trustworthy when the app renders third-party content. The in-process design
shrinks the attack surface; it does not eliminate it.

**Where the confirmation gate lives.** Whichever architecture, a destructive UI action
(delete, submit-payment, irreversible state change) needs a human gate, and this is the
first hard question for a UI-control surface. The candidate placements, none yet chosen:

- **Per-action annotation** — mark individual AccessKit actions / tools `destructiveHint`
  (MCP tool annotations, 2025-03-26) and gate any annotated verb. Fine-grained, but relies
  on every widget author correctly annotating.
- **Capability tier** — put all mutating verbs in an opt-in tier (Playwright-MCP `--caps`
  style), so destructive power is off by default. Coarse but simple.
- **A confirmation widget** — surface the pending action to the user as a Buiy modal /
  confirmation widget (rendered through Buiy's own UI), the protocol analogue of MCP
  **elicitation** (`elicitation/create`). This keeps the human-in-the-loop *inside the
  app's own UI* rather than in the agent transport.

How the gate interacts with **AccessKit's action model** is the load-bearing detail:
AccessKit's `ActionHandler::do_action` is fire-and-forget (the producer receives an
`ActionRequest` and acts), so a gate cannot live *inside* `do_action` as a blocking prompt —
it must sit *before* the `ActionRequest` is synthesized (the agent plugin holds the request,
elicits confirmation, then dispatches) or the action must be modeled as a two-phase
propose→confirm→commit at the Buiy layer above AccessKit. Naming this is a prerequisite for
any bidirectional-tree spec; it is unsettled here.

## 5. Fast-moving MCP spec as a stability risk — [immaturity]

If Buiy bets on MCP as the wire protocol, it bets on a moving target. The revision history
(see [mcp-protocol.md](mcp-protocol.md)) shows four backwards-incompatible revisions in
~13 months: 2024-11-05, 2025-03-26, 2025-06-18, 2025-11-25, with churn that *removed*
features mid-stream (JSON-RPC batching added 2025-03-26, removed 2025-06-18) and *replaced*
a whole transport (HTTP+SSE → Streamable HTTP). A further RC dated **2026-07-28** is locked
but **not yet released as of 2026-06-18** — do not build against it as current. Governance
moved from single-vendor (Anthropic) to the Agentic AI Foundation under the Linux Foundation
on **2025-12-09** (confirmed first-party — see [mcp-protocol.md](mcp-protocol.md)), which
should slow breaking changes long-term but is itself a recent transition. Net: this is
**immaturity**, expected to settle, but today it argues for keeping any MCP binding behind
an adapter rather than threading MCP types through Buiy's core. The internal surface
(AccessKit `ActionRequest`s consumed through Buiy's own `ActionHandler`) is the stable
contract; MCP is one optional outward projection of it.

## 6. Custom-rendered / canvas content invisible to a semantic tree — [inherent]

The semantic-tree approach sees only what is *in* the tree. "Content rendered on a canvas
element doesn't appear in the accessibility (AX) tree unless the application explicitly
provides fallback content or ARIA markup, making games, data visualizations, and custom
renderers that draw directly to canvas invisible to AX queries." The same is true of any
GPU-rasterized custom widget: flattening UI to pixels "negates all of the interactivity and
accessibility features that are native to DOM elements." Workarounds are parallel-DOM
mirrors, ARIA fallbacks, or the new (Google I/O 2026) HTML-in-Canvas API — all of which mean
*manually re-authoring* the semantics the renderer destroyed.

This is **inherent** but it cuts *toward* Buiy's thesis, not against it. Buiy is itself a
custom GPU renderer (it draws its own widgets; there is no DOM behind them). The reason it is
not invisible is precisely that it is **AccessKit-first** and already authors a semantic tree
(role+name+state+actions) as a first-class output — the canvas-accessibility tax is paid up
front by construction. The open problem reframed for Buiy: any widget that paints meaning
*only* into pixels (a custom chart, a game viewport) and forgets to populate its AccessKit
node is invisible to an agent for the same reason it is invisible to a screen reader. The
fix and the failure mode are identical to the canvas case — there is no agent-only shortcut.

## 7. Eval / reliability of GUI agents — [immaturity, with an inherent floor]

Even granting a clean perception+control surface, end-to-end task reliability is unsettled.
On the OSWorld benchmark (369 real desktop/web tasks), humans succeed on **72.36%**; the
early-2024 multimodal baseline (GPT-4V) was **~12.24%**, with the deficit concentrated in
GUI grounding and multi-app workflow reasoning. By late 2025 agentic frameworks had closed
much of that gap (Simular's *Agent S* reports **72.6%**, edging the human baseline; OpenAI's
*Operator* reports ~**38%** on OSWorld, 58% on WebArena, 87% on a JS-heavy web subset). Two
honest readings: (a) rapid progress is real; (b) "72% on a benchmark" still means roughly
one task in four fails, often silently, and these numbers are *grounding-and-pixel* agents,
not structured-tree agents — the structured-tree path (Playwright-MCP, BRP bridges) has far
less published head-to-head eval. The benchmark churn (OSWorld, WorldGUI, OSUniverse,
MMBench-GUI, all 2024-2025) signals the field has not agreed how to measure reliability.
This is mostly **immaturity** (better agents, better evals coming), over an **inherent**
floor: a non-deterministic agent on a stateful UI cannot be made 100% reliable, which is why
[aci-tool-design.md](aci-tool-design.md)'s evaluation-transcript-driven tool refinement and
human-gating for destructive actions (§4) matter regardless of model quality.

## 8. Long-running / async actions vs the synchronous-settle assumption — [immaturity]

The snapshot-after-action loop ([playwright-mcp.md](playwright-mcp.md),
[aci-tool-design.md](aci-tool-design.md)) assumes an action *settles synchronously*: act,
the frame settles, return the post-state snapshot inline. Real UI actions are often **not**
synchronous — an animation completing, a content-visibility off-screen subtree rendering, an
async data load, a network-backed list populating. If the tool returns its snapshot before
the effect lands, the agent reasons over a stale post-state; if it blocks waiting, it can
stall the request past a timeout. MCP's experimental **Tasks** primitive (2025-11-25,
SEP-1686 — durable "call-now, fetch-later"; see [mcp-protocol.md](mcp-protocol.md)) is the
shape that fits this: a long-running UI action returns a task handle, and the agent fetches
the settled snapshot later. Neither [lessons.md](lessons.md) nor
[aci-tool-design.md](aci-tool-design.md) yet considers which Buiy actions should map to a
Task vs an inline result; `wait_for` and animation-completing actions are the obvious
candidates. This is **immaturity** (Tasks is experimental and the mapping is undesigned),
but it is a real gap the synchronous loop papers over.

## 9. Multi-window / multi-tree node addressing — [inherent to the AccessKit model]

The "ref = NodeId" borrow in [lessons.md](lessons.md) and [playwright-mcp.md](playwright-mcp.md)
silently assumes a **single tree**. AccessKit is **one-tree-per-window**: a Buiy app with
multiple windows has multiple `accesskit_winit::Adapter`s, each owning one tree keyed by
winit `WindowId`, and **`NodeId`s are scoped per-tree** — different windows may carry
overlapping `NodeId` numeric values without conflict (confirmed in the sibling
[../accesskit/](../accesskit/) and [../bevy-a11y/](../bevy-a11y/) folders: "an
`accesskit_winit::Adapter` accepts exactly one tree per window"). A bare `NodeId` is
therefore **not** a unique agent ref across a real multi-window app. The agent ref scheme
actually needs to be a **`(tree_id / window_id, NodeId)`** pair, and the snapshot must label
which window/tree each subtree belongs to. This is a concrete design hole: the
single-tree mental model the folder borrows from Playwright (one page, one tree) does not
survive contact with multi-window Buiy. It is **inherent** to AccessKit's per-window tree
model, not a tooling gap — the addressing scheme must carry the window dimension from day
one.

## Cross-links

- Design responses to these problems (validate / avoid / borrow): [lessons.md](lessons.md)
- Protocol churn, governance, Tasks, and tool annotations: [mcp-protocol.md](mcp-protocol.md)
- Snapshot/ref model and the snapshot-after-action timing these problems attack: [playwright-mcp.md](playwright-mcp.md)
- Pixel-fallback alternative and its own costs: [computer-use-and-gui-agents.md](computer-use-and-gui-agents.md)
- Action-vocabulary, confirmation-gate placement, and tool-contract framing: [aci-tool-design.md](aci-tool-design.md)
- AccessKit one-tree-per-window + NodeId scoping: [../accesskit/](../accesskit/)
- Terms (AX tree, ref, confused deputy, lethal trifecta): [glossary.md](glossary.md)

## Sources

- OSWorld benchmark / human vs agent gap: https://www.emergentmind.com/topics/osworld-benchmark
- Agent S / human-baseline claim: https://www.simular.ai/articles/simulars-computer-use-agent-outperforms-humans
- OSWorld leaderboard (Operator figures): https://leaderboard.steel.dev/registry/benchmarks/osworld
- GUI-agent benchmark landscape: https://github.com/OSU-NLP-Group/GUI-Agents-Paper-List
- Playwright-MCP snapshots / stale-ref behavior: https://playwright.dev/mcp/snapshots
- Off-screen / non-viewport snapshot noise (bug): https://github.com/microsoft/playwright/issues/39955
- Accessibility-tree token cost in browser MCPs: https://dev.to/kuroko1t/how-accessibility-tree-formatting-affects-token-cost-in-browser-mcps-n2a
- Snapshot caps / compression (agent-web-interface): https://github.com/lespaceman/agent-web-interface
- Browser-agent architecture/security survey: https://arxiv.org/html/2511.19477v1
- Canvas/WebGL invisible to AX tree: https://annekagoss.medium.com/accessible-webgl-43d15f9caa21
- Canvas accessibility fallback/ARIA: https://pauljadam.com/demos/canvas.html
- HTML-in-Canvas API (Google I/O 2026): https://dev.to/manikant92/google-io-2026-quietly-ended-a-20-year-old-web-problem-meet-the-html-in-canvas-api-4h9d
- MCP prompt injection (Simon Willison): https://simonwillison.net/2025/Apr/9/mcp-prompt-injection/
- MCP tool poisoning (OWASP): https://owasp.org/www-community/attacks/MCP_Tool_Poisoning
- MCPTox benchmark (tool-poisoning study): https://arxiv.org/pdf/2508.14925
- Confused-deputy / blast-radius (Aptible): https://www.aptible.com/mcp-security/mcp-prompt-injection
- MCP Tasks (SEP-1686) / 2025-11-25 changelog: https://modelcontextprotocol.io/specification/2025-11-25/changelog
- MCP versioning (revision dates): https://modelcontextprotocol.io/specification/versioning
