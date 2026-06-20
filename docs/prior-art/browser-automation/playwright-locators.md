**Date:** 2026-06-18
**Status:** active
**Subject:** Playwright — accessibility-tree-first locators (getByRole/getByLabel/getByText) and the ARIA snapshot as a diffable role+name+state serialization

# Playwright — accessibility-tree-first locators

Playwright (Microsoft; v1.61.0, Apache-2.0, npm `playwright` — re-check the
version, it bumps roughly weekly) drives Chromium, Firefox, and WebKit through
one API. The part relevant to Buiy is not the wire protocol but the *addressing
model*: Playwright's recommended way to find an element is by its **role +
accessible name**, the same semantic identity a screen reader announces — not by
CSS path or XPath. This is the closest mainstream validation of "the
accessibility tree is the control surface."

## Why semantic locators are the default

Playwright's locator docs state the recommendation directly: *"We recommend prioritizing user-facing attributes and explicit contracts such as `page.getByRole()`."* CSS/XPath are explicitly demoted: *"CSS and XPath are not recommended as the DOM can often change leading to non resilient tests."*

The recommended ordering (most-preferred first):

1. `getByRole(role, { name })` — resolves against ARIA roles + accessible name; the primary locator
2. `getByText(...)` — non-interactive content by visible text
3. `getByLabel(...)` — form controls via their associated `<label>`
4. `getByPlaceholder(...)`
5. `getByAltText(...)` — images by `alt`
6. `getByTitle(...)`
7. `getByTestId(...)` — explicit `data-testid` contract; the **escape hatch**

CSS/XPath selectors sit below even `getByTestId` — available via `page.locator('css=…')` / `xpath=…` but discouraged. The argument is resilience: a CSS path like `div.panel > ul > li:nth-child(3) > button` encodes incidental DOM structure that breaks on re-layout; `getByRole('button', { name: 'Save' })` encodes the *contract the user perceives* and survives refactors that don't change what the button is or what it says. Role + accessible name is an **implementation-independent address** — it names the element by the semantics already computed for assistive tech, so test code and screen reader converge on the same tree.

`getByRole` resolves against ARIA role + the computed accessible name (see [../wai-aria-apg/](../wai-aria-apg/) for the role taxonomy and the accessible-name algorithm that produces the `name`). Options narrow further by ARIA state: `{ checked, pressed, expanded, selected, disabled, level, exact }`. The `name` option does substring/normalized matching by default; `{ exact: true }` or a `/regex/` makes it precise.

## Lazy, re-resolved-at-action-time locators

A `Locator` is not a captured element handle — it is a *description* that re-resolves every time it is used. Playwright resolves it at **action time**, not creation time, so a locator created before a re-render still finds the current element after it. Combined with **strict mode** (an action throws if the locator matches more than one element), this gives two properties Buiy should note: (1) addresses are values that can be stored and replayed across frames; (2) ambiguity is an error surfaced to the caller, not silently resolved to "the first match." Auto-waiting/actionability (visible, stable, receives events, enabled) layers on top — see [actionability.md](actionability.md).

## The ARIA snapshot

`page.ariaSnapshot()` / `locator.ariaSnapshot()` serialize the accessibility subtree to **YAML**: a stable, human-readable, diffable dump of role + accessible name + relevant ARIA state. Per-node syntax:

```yaml
- role "accessible name" [attribute=value]
```

where `role` is the ARIA/HTML role (`heading`, `button`, `list`, `textbox`, …), the quoted string is the accessible name, and bracketed attributes carry state (`checked`, `disabled`, `expanded`, `level`, `pressed`, `selected`, `invalid`). Children nest by indentation, mirroring tree structure. Example:

```yaml
- heading "Sign in" [level=1]
- textbox "Email"
- button "Continue" [disabled]
```

The snapshot is asserted with `expect(locator).toMatchAriaSnapshot(template)`. Matching is **partial and tolerant by design**:

- **Exact name** — a quoted string must match the accessible name verbatim.
- **Regex name** — `/pattern/` matches dynamic text, e.g. `- heading /Issues \d+/`.
- **Omission = wildcard** — leave out a node's name or an attribute and only the present fields are checked; omit whole nodes and only the listed structure must be present.
- **`/children` mode** controls child strictness: `contain` (default — listed children present, in order), `equal` (exactly this list, in order), `deep-equal` (exact including nested).

The template is generated from the live tree: codegen's "Assert snapshot" action, `toMatchAriaSnapshot('')` (empty → fills in on first run), or `npx playwright test --update-snapshots` to re-bless. v1.60 added `ariaSnapshot({ boxes: true })`, which appends each node's bounding box as `[box=x,y,width,height]` in viewport coordinates — so an agent can read *what* an element is and *where* it sits without a screenshot + vision pass; the docs frame this as a machine-readable alternative to screenshot navigation for AI agents / Playwright MCP. An AI-oriented `ariaSnapshot({ mode: 'ai' })` was also introduced around v1.59–1.60 (re-check the exact version on the release notes).

## Implications for Buiy

Buiy already authors an AccessKit tree (role + name + state + actions) per node. Two artifacts map onto Playwright's model almost directly:

- **A `getByRole`-over-AccessKit query.** Buiy can expose a query that walks the AccessKit tree and resolves a node by `role` + accessible `name` (+ state filters), returning the node id — the same address space the agent perceives. Strict-mode semantics (error on >1 match) and lazy re-resolution (resolve against the *current* tree each turn, since the ECS tree is rebuilt per frame) port cleanly: an AccessKit `NodeId` is the action target, re-resolved per turn rather than a stale handle.
- **An ARIA-snapshot-style dump.** A deterministic YAML (or similar) serialization of the AccessKit subtree — role + name + state, nested by tree structure — gives a diffable assertion that slots beside `buiy_verify`'s existing snapshot tiers (layout snapshot → display-list snapshot → invariant → reftest → golden; see the project's verification design). A semantic-tree snapshot would be a *new lowest-tier* assertion that observes role/name/state regressions without rasterizing — the analogue of Playwright asserting the a11y tree instead of a pixel screenshot. Whether to adopt it is a decision for [lessons.md](lessons.md), not this evidence file.

Note Playwright reads the browser-computed accessibility tree (via the engine's a11y APIs; the Chromium path is CDP's Accessibility domain — see [cdp.md](cdp.md)). Buiy *is* the source of that tree, so it skips the recompute-from-DOM step Playwright depends on: the role+name+state are first-class outputs of Buiy's widget code, not reverse-engineered from markup. See [../accesskit/](../accesskit/) for the tree Buiy authors and [../wai-aria-apg/](../wai-aria-apg/) for the role + accessible-name semantics both Playwright and AccessKit inherit.

## Sources

- Playwright locators guide: https://playwright.dev/docs/locators
- Playwright ARIA snapshots guide: https://playwright.dev/docs/aria-snapshots
- Playwright release notes (v1.59–1.61, ariaSnapshot boxes/ai): https://playwright.dev/docs/release-notes
- Playwright v1.60.0 release: https://github.com/microsoft/playwright/releases/tag/v1.60.0
- npm `playwright` (version, license): https://registry.npmjs.org/playwright/latest
