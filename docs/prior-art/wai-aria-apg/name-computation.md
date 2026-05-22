**Date:** 2026-05-22
**Status:** active
**Subject:** ACCNAME 1.2 — the accessible name and accessible description computation algorithm Buiy implements in `buiy_core`; precedence rules, recursive descent, labelling chain, hidden-subtree exclusion

# Accessible name and description computation (ACCNAME 1.2)

The Accessible Name and Description Computation specification (current draft: **W3C Working Draft, 20 May 2026**, at <https://www.w3.org/TR/accname-1.2/>) defines the precise algorithm that browsers and AT use to derive the **accessible name** and **accessible description** of every node in the accessibility tree. The Buiy foundation [`accessibility.md § 3.11`](../../specs/2026-05-07-buiy-foundation/accessibility.md) pins ACCNAME 1.2 implementation in `buiy_core`. AccessKit deliberately does **not** compute names — it holds the references (`set_label`, `set_description`, `set_labelled_by`, `set_described_by`) and trusts the consuming toolkit to walk them per ACCNAME 1.2 ([`accesskit/lessons.md § Validates`](../accesskit/lessons.md)).

This file is the algorithm Buiy implementers consult; it's normative for any widget whose name source is non-trivial. **It's a Working Draft, not yet a Recommendation** — the Buiy implementation should track ACCNAME 1.2 (current draft) and re-verify whenever the spec advances toward CR.

## The precedence (high to low)

For **name**:

1. **`aria-labelledby`** — resolves to a space-concatenated walk of the referenced subtree(s) in DOM order
2. **`aria-label`** — direct string label
3. **Host-language label** — for HTML / Buiy's equivalent: `<label for>` or wrapping `<label>`, `alt` on images, `<caption>` on tables, the `title` attribute, `<figcaption>` on figures, etc.
4. **Name from content** — for elements that support it (`button`, `link`, `cell`, `heading`, `menuitem`, ...), recursively walk descendants and concatenate text (including pseudo-element generated content like `::before`, `::after`, `::marker`)
5. **`title` attribute** — last-resort fallback

For **description**, the same five-step pattern using `aria-describedby` → `aria-description` → host-language description → `title`.

The spec phrases the algorithm as a sequential walk with numbered steps; the labels (2A, 2B, 2C, 2D, 2E, 2F, 2G, 2H, 2I) appear in the older ACCNAME 1.1 phrasing. Implementers reference the steps by name (not by number) since numbering has shifted between drafts.

## Detailed step-by-step

### Step A — Hidden Not Referenced

If the current node is **hidden** (e.g. `display: none`, `visibility: hidden`, `aria-hidden="true"`, `inert`, or in a hidden subtree) **and** the node is NOT being computed because it was referenced from `aria-labelledby` / `aria-describedby` of a different node, **skip** the node (return empty string).

This is **critical for Buiy**: a hidden label can still contribute to an accessible name when referenced. A common pattern is `<label for=foo class="visually-hidden">Search</label>` — the label is visually hidden but its text becomes the accessible name.

### Step B — `aria-labelledby`

If the node has `aria-labelledby`, resolve every id reference in the order listed, recursively compute each referenced node's text alternative (with the `traverseReferenced` flag set so hidden subtrees are NOT excluded), concatenate with spaces, return the result.

**Cycle protection.** Implementations must guard against `aria-labelledby` cycles. ACCNAME requires that during one name computation, no node is visited twice. Buiy maintains a "visited" set keyed by `Entity` during the descent.

### Step C — Embedded Control

If the current node is **embedded inside another node's name computation** AND is itself a form control with a current value, return the control's current value (e.g. `<input>` returns its value, `<select>` returns the selected option's text).

This rule exists because forms like `<label>Name: <input value="Alice"></label>` should yield the label "Name: Alice".

### Step D — `aria-label`

If the node has a non-empty `aria-label`, return its trimmed value.

### Step E — Host-language label

Walk host-language labelling mechanisms in spec-defined order. For HTML:

- `<label for>` or `<label>` wrapping
- `alt` attribute on `<img>` and `<area>`
- `<caption>` for `<table>`
- `<figcaption>` for `<figure>`
- `<legend>` for `<fieldset>`
- The `title` attribute (as a last resort within this step)
- Implicit labels from other relationships specified by the host language

For Buiy, the "host language" is Buiy's component model. The Buiy mapping is documented in [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md): `Image.alt`, `Heading` content, `Label` component association via `LabelFor` relation, etc.

### Step F — Name from Content

For roles in the **"name from content"** set (`button`, `link`, `cell`, `columnheader`, `rowheader`, `heading`, `menuitem`, `menuitemcheckbox`, `menuitemradio`, `option`, `radio`, `tab`, `treeitem`, `tooltip`, etc.), recursively walk the node's descendants and concatenate their text alternatives.

**Pseudo-element generated content** counts. ACCNAME 1.2 explicitly includes CSS-generated content from `::before`, `::after`, and `::marker`. For Buiy, equivalent generated-content sources (e.g. icon glyph in a `Button`, an animated checkmark in a `Checkbox`) must be excluded or included per their semantic intent. Buiy convention: decorative content is marked `Decoration` and excluded; semantic glyphs (e.g. an actual `Image` with an `alt`) are included.

### Step G — Text Node

If the node is a text node, return its text.

### Step H — Recursive Name from Content

Continue descending into children for the "name from content" case, applying the algorithm recursively to each child.

### Step I — Tooltip (last resort)

If no name has been determined by any earlier step, the `title` attribute (or equivalent tooltip text) is used as a last-resort name. APG strongly discourages relying on `title` because it's not exposed reliably across platforms / pointer types.

## Description computation

The description algorithm mirrors the name algorithm with these sources, in precedence order:

1. `aria-describedby` — recursive walk of referenced subtrees
2. `aria-description` (string, ARIA 1.2 / 1.3)
3. Host-language description (e.g. HTML `title` attribute, `<figcaption>`)
4. `title` attribute (last resort)

**`aria-describedby` vs `aria-details` distinction.** `aria-describedby` produces a flat text string that the AT announces alongside the name (e.g. "Username, must be 8–32 characters"). `aria-details` is a relationship to a richer structured block — the AT exposes a separate "details" affordance, not a concatenated string. Buiy's name computation does NOT walk `aria-details` for the description; instead, AccessKit's `details: Vec<NodeId>` relation surfaces the relationship for AT-side navigation.

## Hidden-subtree exclusion rules

| Source of name | Hidden node included? |
|---|---|
| `aria-labelledby` reference walk | **YES** — referenced hidden subtrees contribute |
| `aria-describedby` reference walk | **YES** — same reason |
| Recursive descent for name-from-content | **NO** — hidden descendants are excluded |
| Host-language label (e.g. `<label for>`) | **YES** if explicit association, **NO** if implicit-by-DOM-proximity |

Buiy's name computation tracks a `traverse_referenced: bool` flag (set true when entering a `labelledby` / `describedby` walk, false otherwise) and uses it to decide hidden inclusion at each node.

## Trimming and whitespace

ACCNAME 1.2:

- Collapses sequences of whitespace within a node's contribution to a single space
- Concatenates child contributions with a single space between them
- Trims leading and trailing whitespace from the final string

Buiy's implementation uses Rust's `str::split_whitespace().collect::<Vec<_>>().join(" ")` pattern for the normalization step.

## Locale and language

ACCNAME does not directly compute language-tagged names — that's the host language's job. Buiy's [`accessibility.md § 3.11 Live regions and announcements`](../../specs/2026-05-07-buiy-foundation/accessibility.md) lists **locale-aware accessible-name composition** (number / date formatting; `lang`-switching mid-string in `aria-labelledby` chains) as a tier-C feature.

The recursive walk needs to remember the inherited `lang` for each contributed substring so the AT can announce in the right language (e.g. `<span lang="fr">bonjour</span> world` should be announced with French pronunciation for "bonjour"). AccessKit supports per-node language tags via `Node::set_language(&str)`; Buiy emits the resolved language per AccessKit node.

## ARIA roles that PROHIBIT a name

Some roles (`caption`, `code`, `deletion`, `emphasis`, `generic`, `insertion`, `mark`, `none`, `paragraph`, `presentation`, `strong`, `subscript`, `superscript`, `suggestion`, `term`, `time`) **prohibit** an accessible name per ARIA 1.2 § 5.2.7.5. If `aria-label` / `aria-labelledby` are set on these roles, AT should ignore them. Buiy's emission layer warns when it encounters this combination and elides the name.

## Buiy implementation notes

The ACCNAME 1.2 implementation lives in `buiy_core`. Pinned design:

- Algorithm is a **single function** `compute_accessible_name(entity: Entity, world: &World, references: &mut Visited) -> String` recursively called during `BuiySet::A11yUpdate`
- Memoized per-frame (a frame-local cache keyed by `Entity` — recomputation is skipped if no relevant component changed)
- The `references: &mut Visited` set is the cycle-protection visited set
- The output is written to AccessKit's `Node::set_label(...)` and `set_description(...)` per node
- Per-widget overrides exist for widgets whose name-from-content semantics differ (e.g. an `Image` with an empty `alt` has a different rule than a `Button` with an empty content)

## Verification

The verification harness ([`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)) gate 3 (AccessKit tree snapshots) captures the resolved name + description for every focusable node. Test cases for the name-computation logic specifically:

- A11yLabel direct (Step D)
- A11yRelations.labelled_by chain (Step B), single and multi
- A11yRelations.labelled_by cycle — must terminate
- A11yRelations.labelled_by to hidden subtree — must include text
- Name from content on a `button` with mixed text + icon (Step F)
- Hidden visually-hidden label included via `labelled_by`
- ARIA roles that prohibit name — warn + elide
- Whitespace collapse and trimming
- Description via `described_by`
- `aria-details` does NOT contribute to description string

## Open questions

- **CSS `content` from `::before` / `::after` / `::marker`.** ACCNAME 1.2 includes this; Buiy has no `::before` / `::after` pseudo-element analogue yet (foundation §3 is silent). When Buiy ships generated content, the name computation needs to mirror.
- **`alt=""` on decorative images.** ACCNAME treats empty `alt` as "image is decorative; exclude from name from content". Buiy's `Image.alt: Option<String>` plus the `Decoration` marker covers this; verify the algorithm correctly excludes.
- **Form value embedding (Step C) for Buiy editable widgets.** When a `Textbox` is referenced inside a `Label`, the Textbox's current value should contribute. The Buiy form-state machine must surface "current value" to the name walker.

## Sources

- ACCNAME 1.2 Working Draft (20 May 2026): <https://www.w3.org/TR/accname-1.2/>
- ACCNAME 1.1 (the previous Recommendation): <https://www.w3.org/TR/accname-1.1/>
- ARIA 1.2 § 5.2.7 Name and Description: <https://www.w3.org/TR/wai-aria-1.2/#namecalculation>
- Buiy foundation ACCNAME commitment: [`docs/specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- AccessKit name fields (set_label / set_description / set_labelled_by / set_described_by): <https://docs.rs/accesskit/0.24.0/accesskit/struct.Node.html>
- Sibling files: [`roles-states-properties.md`](roles-states-properties.md), [`patterns-catalog.md`](patterns-catalog.md), [`live-regions.md`](live-regions.md)
