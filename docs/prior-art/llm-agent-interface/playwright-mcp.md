**Date:** 2026-06-18
**Status:** active
**Subject:** Playwright-MCP — Microsoft's accessibility-tree-first MCP browser-control server; the closest existing analog to a Buiy agent server, and the per-tool template for a `buiy_snapshot`/`buiy_click{ref}` surface over AccessKit

# Playwright-MCP

Playwright-MCP is the single closest shipping analog to what a Buiy agent server would be: an MCP server whose **primary perception surface is a structured accessibility tree, not pixels**, and whose actions target elements by **stable refs read out of that tree** rather than by coordinates. Almost every design decision in it maps one-to-one onto a Buiy server built over the AccessKit semantic tree Buiy already authors (role + name + state + actions). This file documents the mechanics, then maps each idea to a Buiy equivalent.

For the protocol it speaks, see [mcp-protocol.md](mcp-protocol.md). For the AccessKit tree it would mirror — including the `accesskit` 0.24 / `accesskit_winit` 0.33 line whose `NodeId`/`Action`/`ActionRequest` types this file leans on — see [../accesskit/](../accesskit/). For the pixel-based alternative it deliberately rejects as default, see [computer-use-and-gui-agents.md](computer-use-and-gui-agents.md).

## What it is

- **Repo:** https://github.com/microsoft/playwright-mcp ; npm `@playwright/mcp`.
- **Maintainer:** Microsoft. **License:** Apache-2.0.
- **Version:** **v0.0.76** (npm publish timestamp `2026-06-10T00:16:09Z`) — still pre-1.0 (0.0.x); the tool roster and `--caps` set churn between releases, so treat specifics here as a snapshot, not a frozen contract. (Date note: the GitHub releases page shows v0.0.76 as "released 10 Jun" with the year omitted because it is the current year; the npm registry's explicit publish timestamp pins it to 2026-06-10.)
- **Core model:** drives a real browser (Chromium/Firefox/WebKit via Playwright) on behalf of an LLM. The default mode ("Snapshot Mode") perceives the page through **Playwright's accessibility tree** — "Uses Playwright's accessibility tree, not pixel-based input"; "No vision models needed, operates purely on structured data." Vision Mode (screenshots + coordinate clicks) is opt-in behind a flag.

## The snapshot → analyze → act-by-ref loop

The central loop, which Buiy would replicate verbatim with `buiy_snapshot` in place of `browser_snapshot`:

1. **Snapshot.** `browser_snapshot` returns the accessibility tree as **structured text**, each interactive node carrying a unique `ref`. Example from the docs:

   ```
   - heading "todos" [level=1]
   - textbox "What needs to be done?" [ref=e5]
   - listitem:
     - checkbox "Toggle Todo" [ref=e10]
     - text: "Buy groceries"
   - contentinfo:
     - text: "1 item left"
   ```

   Each interactive element gets "a unique ref for deterministic interaction."
2. **Analyze.** The model reads the text tree, finds the element it wants by role + name (e.g. the `textbox` at `ref=e5`), and picks its `ref`.
3. **Act by ref.** The model calls a mutating tool passing that ref. No coordinates, no pixel guessing.
4. **Re-snapshot.** After a state-mutating action the server captures a **fresh snapshot and appends it to the tool result**, so the model always sees the post-action tree without a separate `browser_snapshot` call. (The introduction docs show this sequential-snapshot behavior; the v0.0.76 README phrases the tool result as carrying the updated page state.)

This closed loop — perceive-as-text, act-by-ref, auto-re-perceive — is exactly the bidirectional shape Buiy needs: snapshot the AccessKit tree, act on a node, consume the resulting `ActionRequest` through Buiy's own `accesskit_winit::Adapter` / `ActionHandler::do_action` plumbing (the same handler a screen reader hits on a Buiy-owned window), re-snapshot.

## The toolset (v0.0.76)

A note on naming churn: in earlier 0.0.x releases the element-targeting parameter was literally `ref`; **as of v0.0.76 the parameter is `target`** ("Exact target element reference from the page snapshot, or a unique element selector"), and most action tools also take an optional human-readable `element` string "used to obtain permission to interact with the element." The conceptual model is unchanged — you still target by a ref pulled from the snapshot — but the field is now `target` and also accepts a CSS/text selector, loosening the older accessibility-ref-only constraint.

Primary tools:

- **`browser_snapshot`** — "Capture accessibility snapshot of the current page, this is better than screenshot." Optional `target`, `depth` (limit tree depth), `boxes` (include each element's bounding box as `[box=x,y,width,height]`), `filename`.
- **`browser_click`** — `target` (ref/selector), optional `element`, `doubleClick`, `button`, `modifiers`.
- **`browser_type`** — `target`, `text`, optional `submit` (press Enter after), `slowly` (char-by-char), `element`.
- **`browser_press_key`** — `key` ("such as `ArrowLeft` or `a`").
- **`browser_hover`** — `target`, optional `element`.
- **`browser_select_option`** — `target`, `values` (array), optional `element`.
- **`browser_fill_form`** — `fields` (array): fill multiple form fields in one call. This is the token-saving consolidation pattern [aci-tool-design.md](aci-tool-design.md) calls out — one round-trip for a whole form instead of N type calls.
- **`browser_navigate`** / `browser_navigate_back` — load a URL / go back.
- **`browser_wait_for`** — wait until `text` appears, `textGone` disappears, or `time` seconds pass. The structured analog of a vision agent's screenshot-poll-loop: wait on a **semantic condition**, not a pixel diff.

Plus a long tail (`browser_evaluate`, `browser_take_screenshot`, `browser_tabs`, `browser_file_upload`, `browser_handle_dialog`, console/network/storage/devtools families).

## Capability tiers gated by flags (`--caps`)

The server ships a small default surface and gates riskier or heavier capabilities behind `--caps`. As of v0.0.76 the flags include: **`vision`** (coordinate mouse tools — `browser_mouse_click_xy`, `browser_mouse_move_xy`, `browser_mouse_drag_xy`, wheel/up/down), **`pdf`** (`browser_pdf_save`), **`devtools`**, **`storage`** (cookie/localStorage/sessionStorage), **`network`** (routing/mocking), **`testing`** (verify_* assertions, locator generation), and **`config`**. (Re-verify this exact set against the current README — it changes between 0.0.x releases.)

The load-bearing point for Buiy: **the accessibility-tree tools are the always-on core; pixel/coordinate tools are an opt-in escape hatch.** Vision is for the cases the a11y tree genuinely cannot describe (canvas-rendered content). A Buiy server would mirror this gating: AccessKit role/name/action tools as the default tier, a pixel/coordinate tier off by default for the rare unannotated-canvas case.

## The token economics: ~2–5 KB tree vs thousands of vision tokens

The reason snapshot-first is the default, not just an option, is cost and reliability. Reported figures (secondary sources, order-of-magnitude — treat as illustrative not benchmarked):

- A structured accessibility snapshot is roughly **~200–400 tokens** for a simple page, single-digit-KB for typical pages.
- The equivalent screenshot consumes **thousands** of vision tokens; one cited comparison: a run consuming **~50,000 vision tokens drops to ~5,000 text tokens** with the accessibility tree — a **~10×** reduction, and larger on dense pages.

Beyond raw token count, the text tree is **deterministic and parseable**: the model targets a `ref` that maps to a concrete DOM node, instead of inferring an `(x, y)` from a rasterized image and hoping the layout didn't shift. This is the same correctness argument the Buiy thesis rests on — the AccessKit node has a stable `NodeId`, a known role, and a declared action set, so acting on it is exact, not a pixel gamble.

## Implications for Buiy

Playwright-MCP is, almost line for line, the template for a Buiy agent server — with one structural advantage in Buiy's favor: **Buiy already authors the tree.** Playwright-MCP has to extract an accessibility tree out of a browser whose primary contract is the DOM; Buiy's AccessKit tree (role + name + state + actions per node) is the framework's own first-class output, currently only flowing **outward** to the platform a11y layer. Making it bidirectional is the whole move.

The mapping (these are framing notes; the validates/avoid/borrow decisions live in [lessons.md](lessons.md), not here):

| Playwright-MCP | Buiy equivalent |
|---|---|
| `browser_snapshot` → text a11y tree with `ref`s | `buiy_snapshot` → serialize the AccessKit tree, role + name + state + supported actions per node |
| `target` = ref pulled from snapshot | `ref` = the AccessKit **`NodeId`** — already stable, already unique, already what screen readers address. *Caveat:* AccessKit `NodeId`s are scoped **per-tree (per-window)**, so a multi-window Buiy app has overlapping NodeId spaces; the agent ref must really be a `(window_id/tree_id, NodeId)` pair, not a bare `NodeId` (see [open-problems.md § 9](open-problems.md)). |
| `browser_click{target}` | `buiy_click{ref}` → emit an AccessKit `Action::Click` `ActionRequest` for that `NodeId`, dispatched through Buiy's own `accesskit_winit::Adapter` / `ActionHandler::do_action` for the owning window |
| `browser_type{target,text}` | `buiy_type{ref,text}` → `ActionRequest` with `Action::SetValue` / focus + text |
| `browser_press_key{key}` | a key-injection tool into the same winit event stream Buiy already owns |
| snapshot auto-appended after mutation | re-serialize the AccessKit tree after the ECS frame settles and append to the tool result (the "frame settles" boundary is itself an open timing question — see [open-problems.md § 2](open-problems.md)) |
| `--caps vision` opt-in pixel tier | a pixel/coordinate tier off by default; Buiy can offer its GPU readback path only when the a11y tree can't describe a node |
| `browser_fill_form{fields}` | a batched multi-field action to save round-trips on forms |

The key insight Buiy should take: **the ref is not a synthetic handle you invent — it is the AccessKit `NodeId` you already mint** (paired with the window/tree it belongs to). Playwright-MCP had to graft refs onto browser nodes; Buiy gets them for free from the tree it already builds. The only missing half is *consuming* AccessKit `ActionRequest`s rather than only emitting the tree, which AccessKit + winit already define a channel for — and which on a Buiy-owned window is Buiy's own `ActionHandler`, not bevy_winit's (see [../accesskit/](../accesskit/)).

Unflattering caveats worth carrying forward:

- **It's 0.0.x.** Microsoft has not committed to a stable tool contract; tool names and `--caps` move between releases. A Buiy server should expect to version its tool surface and not treat any single roster as permanent.
- **The a11y tree is not always sufficient.** The existence of Vision Mode is an admission: canvas/WebGL content and visually-encoded state (color-only signals, custom-painted widgets without ARIA) are invisible to the tree. Buiy inherits the analog: any node that paints meaning without putting it in its AccessKit role/name/state is invisible to the agent. The fix is the same as the screen-reader fix — annotate the node — which is a feature, not a workaround.
- **`target` now also accepts raw CSS/text selectors**, not just refs. That convenience reintroduces brittleness (selectors break on markup change) that pure ref-targeting avoided. Buiy should be cautious about offering a non-`NodeId` targeting path for the same reason.

## Sources

- https://github.com/microsoft/playwright-mcp — repo, README, tool list, `--caps`, v0.0.76
- https://github.com/microsoft/playwright-mcp/blob/main/README.md — per-tool parameter schemas (`target`, `element`, `text`, …)
- https://www.npmjs.com/package/@playwright/mcp — npm publish timestamp for v0.0.76 (2026-06-10)
- https://playwright.dev/mcp/introduction — snapshot→act-by-ref loop, accessibility-snapshot example with `[ref=e5]`, ref model
- https://playwright.dev/docs/getting-started-mcp — Playwright MCP getting-started / Snapshot vs Vision mode
- https://www.morphllm.com/playwright-mcp — secondary: tool count, token-economics figures
- https://mcp.directory/blog/playwright-browser-mcp-guide-2026 — secondary: accessibility-tree-vs-pixels framing, ~50k→~5k token comparison
