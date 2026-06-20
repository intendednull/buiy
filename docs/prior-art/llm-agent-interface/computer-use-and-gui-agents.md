**Date:** 2026-06-18
**Status:** active
**Subject:** Anthropic computer use + OS-level GUI agents — the pixel-vs-semantic perception tradeoff, set-of-marks, and the a11y-tree-first hybrid frontier

# Computer use + GUI agents — pixel vs semantic perception

This file covers the *other* end of the perception spectrum from
[playwright-mcp.md](playwright-mcp.md): agents that drive a GUI by looking at
pixels and emitting raw mouse/keyboard actions. The contrast is the central
argument for why a tree-authoring framework should lead with the tree.

## Anthropic computer use

Computer use gives Claude a screenshot of the screen plus mouse/keyboard/
coordinate control. The loop is: **screenshot → model emits an action (click at
(x,y), type, key, scroll, drag, …) → executor runs it → screenshot again**.
There is no structured representation of the UI in the loop — the model
perceives the interface only as an image and must ground every action to pixel
coordinates itself.

**Tool versions, beta headers, and supported models** (verified against the live
platform.claude.com computer-use-tool docs on 2026-06-18 — the doc enumerates the
roster exactly as below; model rosters move, so re-verify before citing):

- `computer_20251124` — beta header **`computer-use-2025-11-24`** — listed for
  **Claude Opus 4.8, Claude Opus 4.7, Claude Opus 4.6, Claude Sonnet 4.6, and
  Claude Opus 4.5**. Adds an optional `enable_zoom` parameter and a `zoom`
  action taking a region `[x1, y1, x2, y2]` — a coping mechanism for the
  coordinate-grounding problem (let the model magnify a region before clicking).
  Both the tool id and the zoom feature are confirmed in the live docs.
- `computer_20250124` — beta header **`computer-use-2025-01-24`** — Sonnet 4.5,
  Haiku 4.5, Opus 4.1 (deprecated), Sonnet 4 (retired except Bedrock/Vertex),
  Opus 4 (retired except Vertex). Model statuses churn — re-verify against the
  deprecations page.
- `computer_20241022` — the original public beta (Oct 22, 2024, Claude 3.5
  Sonnet).

**Status: still BETA, not GA.** It has required a beta header continuously
since October 2024. That longevity-in-beta is itself a signal about how hard
reliable pixel control is. The tool ships paired with a reference Docker
container (a virtual desktop the model drives); Anthropic's docs note it
achieves SOTA single-agent results on WebArena but caution it remains
error-prone.

## The core tradeoff: semantic-tree vs pixel perception

GUI-agent perception research (OSWorld and the GUI-agent surveys below)
converges on three perception modalities:

1. **Accessibility tree (semantic)** — a structured representation of UI
   elements with role, name, value, state, and hierarchy, *including elements
   not currently visible* (scrolled-off, occluded). This is the same tree
   screen readers consume and the same tree Playwright-MCP snapshots.
2. **Screenshot (pixel)** — a direct visual image of the current viewport,
   nothing more. General (works on anything that renders) but blind to
   structure, state, and off-screen content.
3. **Set-of-marks (SoM)** — a hybrid: overlay numbered bounding boxes onto
   interactive elements in a screenshot (boxes derived from the a11y tree or a
   detector), so the model picks an element by *index* instead of predicting
   raw coordinates. Sidesteps the hardest part of pixel grounding.

The practical asymmetry:

| | Semantic tree | Pixel / screenshot |
|---|---|---|
| Cost per step | cheap (text; truncated to ~10k tokens in OSWorld) | expensive (~a full image every step) |
| Reliability | high — references stable element identity | brittle — coordinate prediction fails, repeats actions |
| Generality | needs an *authored* tree to exist | works on anything that draws pixels |
| State / off-screen | exposed (value, checked, scrolled-off nodes) | invisible unless on screen |
| Custom-drawn widgets | invisible if not in the tree | visible (it's just pixels) |

OSWorld's authors report that VLM agents "struggle to ground screenshots to
predict precise coordinates," "tend to predict repetitive actions," and show
"limited knowledge of basic GUI interactions." Pixel perception is
especially weak on exactly the controls whose *meaning* lives in structure:
dropdowns/comboboxes (the open popup is a separate layer), scrollbars (drag
geometry the model must infer), sliders, and off-screen list items. Those are
precisely the cases the a11y tree handles natively (role=combobox + expanded
state + child options).

The tree's weakness is the mirror image: it only works if a tree *exists and
is accurate*. Canvas-drawn UIs, games, video, image content, and apps that
never wired up accessibility are invisible or under-described to a
tree-only agent. That is the one place pixels are genuinely irreplaceable.

## The hybrid frontier: tree-primary, screenshot-on-demand

The 2025–2026 GUI-agent surveys describe the emerging consensus as a hybrid:
**accessibility/DOM tree first, screenshot as fallback.** From the surveys
(*GUI Agents: A Survey*; *Towards Trustworthy GUI Agents: A Survey*): hybrid
designs "combine visual and structural cues for improved robustness"; the
execution layer "provides a fresh accessibility tree snapshot after each
interaction, and when agents encounter elements not exposed in the
accessibility tree or requiring visual understanding, they can invoke
screenshot tools, with this delegation pattern keeping most interactions fast
by operating on text-based snapshots while providing fallback for visually
rendered content."

This is the same shape Playwright-MCP ships in production: snapshot mode
(a11y tree) is the default, vision mode (screenshots) is opt-in behind a
capability flag — see [playwright-mcp.md](playwright-mcp.md). Dual-grounding
agents like AppAgent (labeled screenshot + XML of interactive elements) and
OSCAR (Windows UIA a11y tree + descriptive labels) sit in the same family:
the tree supplies identity and state, the pixels supply the genuinely-visual
residue, and set-of-marks is the bridge that lets a model reference tree
elements without coordinate math.

## Implications for Buiy

Buiy already authors an AccessKit semantic tree (role + name + state +
actions) for every widget — the same tree screen readers and the agents above
want. That puts Buiy on the cheap, reliable side of the tradeoff *by
construction*: an agent driving a Buiy app via that tree never has to predict
a pixel coordinate, never loses a scrolled-off list item, and reads combobox
/ checkbox / slider state directly. The pixel path — expensive, brittle,
beta-since-October-2024 — is what tree-less frameworks are forced into.

The honest caveat is the tree's known blind spot: custom-painted content with
no accessible representation. The hybrid frontier says the right answer is not
"tree *or* pixels" but "tree first, screenshot as a deliberate fallback for
genuinely visual content." The design conclusions — lead with the tree, keep
pixels as an escape hatch, and consider set-of-marks for any visual fallback
— are recorded as validates/borrow/avoid entries in
[lessons.md](lessons.md), not here.

See also [aci-tool-design.md](aci-tool-design.md) (why a clean action
surface matters more than raw capability) and [open-problems.md](open-problems.md)
(the custom-widget / canvas blind spot as an open question).

## Sources

- Computer use tool docs (versions, beta headers, model roster, zoom action) — verified 2026-06-18: https://platform.claude.com/docs/en/agents-and-tools/tool-use/computer-use-tool
- OSWorld (observation modalities: screenshot + a11y tree; coordinate-grounding brittleness): https://proceedings.neurips.cc/paper_files/paper/2024/file/5d413e48f84dc61244b6be550f1cd8f5-Paper-Datasets_and_Benchmarks_Track.pdf
- OSWorld-Human (token cost of a11y tree, 10k cutoff; efficiency): https://arxiv.org/html/2506.16042v1
- GUI Agents: A Survey (perception modalities, set-of-marks, dual grounding, AppAgent/OSCAR): https://arxiv.org/html/2412.13501v2
- Towards Trustworthy GUI Agents: A Survey (hybrid tree-first + screenshot-fallback): https://arxiv.org/html/2503.23434v2
- Large Language Model-Brained GUI Agents: A Survey (set-of-marks, accessibility-tree perception): https://arxiv.org/html/2411.18279v2
- Agent S (computer-use agent framework, perception design): https://arxiv.org/html/2410.08164
