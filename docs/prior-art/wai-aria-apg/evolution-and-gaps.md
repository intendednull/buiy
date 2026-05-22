**Date:** 2026-05-22
**Status:** active
**Subject:** WAI-ARIA APG — version history (ARIA 1.0 → 1.1 → 1.2 → 1.3 draft), what's coming, and the documented gaps relevant to Buiy (game UIs, gamepad input, 3D / diegetic surfaces, complex visualisations)

# Evolution and gaps

## Version history

### ARIA 1.0 (2014)

W3C Recommendation 20 March 2014. The original specification. Introduced the role taxonomy, state and property attributes, the accessible name and description model. APG existed as informal authoring guidance contemporaneously.

### ARIA 1.1 (2017)

W3C Recommendation 14 December 2017. Notable additions:
- New roles: `feed`, `figure`, `none` (synonym for `presentation`), `searchbox`, `switch`, `term`
- `aria-modal` replaces deprecated dialog-modality conventions
- `aria-orientation` accepts `undefined` (was just horizontal/vertical)
- `aria-haspopup` accepts the popup type (`menu` / `listbox` / `tree` / `grid` / `dialog`)
- ACCNAME 1.1 algorithm refined and pinned

### ARIA 1.2 (2023)

W3C Recommendation **6 June 2023**. The current version Buiy targets. Notable additions:

- **New roles:** `blockquote`, `caption`, `code`, `deletion`, `emphasis`, `generic`, `insertion`, `mark`, `meter`, `paragraph`, `strong`, `subscript`, `superscript`, `suggestion`, `time`
- **New properties:** `aria-braillelabel`, `aria-brailleroledescription`, `aria-colindextext`, `aria-description`, `aria-rowindextext`
- `aria-checked` now formally documented on `switch`
- `aria-details` clarified vs `aria-describedby` (the rich-vs-flat distinction)
- `directory` role **deprecated**
- `aria-grabbed`, `aria-dropeffect` **deprecated** (replaced by host-language drag-and-drop semantics + WCAG 2.5.7 keyboard alternative)
- Stronger DPub ARIA integration
- ACCNAME 1.2 spec'd separately as a Working Draft

### ARIA 1.3 (Working Draft, in progress)

W3C Working Draft at <https://www.w3.org/TR/wai-aria-1.3/>. Anticipated additions:
- `aria-actions` — declare named actions
- New roles for emerging patterns
- Clarifications across roles + properties

**Buiy stance.** Target ARIA 1.2 today; track 1.3 drafts; revisit when 1.3 advances to CR. AccessKit's `Role` enum extension is the gating factor — Buiy can't emit a role AccessKit doesn't yet have ([`accesskit/lessons.md § Avoid`](../accesskit/lessons.md): "Role enum is closed; new ARIA roles wait for an AccessKit bump").

### APG version history

APG is not formally versioned the way ARIA spec is — it's a living document. Major content shifts:

- Carousel pattern added ~2019
- Feed pattern added ~2018
- Combobox patterns reorganised across 1.1 / 1.2 to clarify the three combobox variants (`aria-autocomplete` `none` / `list` / `both` / `inline`)
- Treegrid added
- Date Picker example added (still labelled "example" not "pattern")

The patterns roster at folder-write (32 patterns per <https://www.w3.org/WAI/ARIA/apg/patterns/>) is the current cut.

## ACCNAME version history

- ACCNAME 1.1 — W3C Recommendation 18 December 2018
- ACCNAME 1.2 — W3C **Working Draft, 20 May 2026** at <https://www.w3.org/TR/accname-1.2/>

Buiy implements ACCNAME 1.2 ([`accessibility.md § ACCNAME 1.2`](../../specs/2026-05-07-buiy-foundation/accessibility.md)). The draft status is worth noting: future-draft revisions may slightly change the algorithm. Re-verify whenever the spec advances toward CR / Recommendation.

## WCAG version history

- WCAG 2.0 (2008) — original A / AA / AAA level taxonomy
- WCAG 2.1 (2018) — mobile, low-vision, cognitive accessibility additions
- **WCAG 2.2** (5 October 2023) — Buiy's target
- WCAG 3.0 (in draft, multi-year horizon) — fundamentally different conformance model ("Bronze / Silver / Gold" scoring); no near-term Buiy commitment

WCAG 2.2's new SCs Buiy gates on: 2.4.11 Focus Not Obscured (Minimum), 2.5.7 Dragging Movements, 2.5.8 Target Size (Minimum), 3.3.7 Redundant Entry, 3.3.8 Accessible Authentication. See [`wcag-22-aa-mapping.md`](wcag-22-aa-mapping.md).

## Gaps in APG (documented and structural)

### Game UI

**APG does not cover game-specific UI patterns.** Pattern roster is web-content-oriented: forms, navigation, dialogs, structured data. Things APG is silent on:

- HUD layouts (health bar, mini-map, ammo counter overlay)
- Skill trees (interactive radial / tree-shaped widget for ability upgrades)
- Inventory grids with stacking, dragging, and equip slots
- Dialogue choice trees (branching narrative interfaces)
- Quest logs with collapsible / filterable categories
- Achievement / trophy lists with progress and lock states
- Player status overlays (buffs / debuffs with stack counts and durations)

Buiy's implications: each game-UI pattern needs a per-widget contract that combines APG patterns. A skill tree is likely `Tree` + `Button` + custom spatial-navigation overlay; an inventory grid is `Grid` + drag-and-drop with WCAG 2.5.7 keyboard alternative.

### Gamepad / TV remote / D-pad input

**APG focuses on keyboard.** Tab / arrow keys / Enter / Space / Esc / Home / End / PgUp / PgDn. **Gamepad input is not in APG.** Buiy must extend:

- D-pad → arrow key analogue, with spatial-nav fallback when no widget claims arrow-key semantics
- A button (Xbox) / X button (PlayStation) → Enter equivalent
- B button (Xbox) / O button (PlayStation) → Esc equivalent
- Y / triangle → context menu / submenu
- Bumpers → tab traversal between top-level regions
- Analog stick → spatial-nav with rate-limiting / dead-zone
- Trigger → modifier for chord input

See [`focus-management.md § Spatial focus navigation`](focus-management.md) for the Buiy extension. Cross-references: [`prior-art/unreal-slate-umg/`](../unreal-slate-umg/) CommonUI cardinal navigation, [`prior-art/rmlui/`](../rmlui/) `nav-up`/`nav-down` annotations.

### 3D-anchored / diegetic UI

**APG assumes 2D screen layouts.** A widget in 3D world space (e.g. a control panel on a spaceship console you walk up to in a first-person game) has no APG pattern. The Buiy `buiy_3d` subsystem ([`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)) puts widgets in 3D space; what does Tab order even mean? APG offers no guidance.

Buiy's tentative approach: a 3D-anchored widget participates in the focus tree of its containing **interaction context** (e.g. "while reading the console, the panel's focus tree is active"); Tab order is determined by spatial layout when the player is reading.

### Complex visualisations

APG covers Grid and Treegrid (tabular), and provides graphics-ARIA for SVG (separate spec at <https://www.w3.org/TR/graphics-aria-1.0/>). **Not covered:**

- Interactive charts (bar / line / scatter with zoom / pan / data-point selection)
- Maps with regions / pins / layers / spatial selection
- Timeline / Gantt views
- Node-link diagrams (network graphs, mind maps, dependency graphs)
- Custom visualisations driven by per-pixel canvas operations

Buiy's [`media-and-widgets.md § Programmatic rendering surfaces`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md) includes a Canvas2D analogue for custom widgets that paint procedurally. For accessibility, the convention should be: every custom visualisation must provide an alternative accessible structure (a data table for charts; a list of points-of-interest for maps; a flattened sequence for timelines) reachable via an alternative-content slot.

### Window / OS-level interactions

APG covers `dialog` and `alertdialog`. It does NOT cover:
- Multi-window applications (window switcher, window menu)
- Native menu bar (vs ARIA `menubar` role for application menubars)
- Notifications outside the app's window
- System tray / dock interactions

Buiy delegates these to platform AT via OS-native windowing (Bevy / winit). Application-internal menubars use ARIA `menubar`; OS menubars are out of scope.

### Rich text editing

APG covers `textbox` (single + multi-line) but not rich-text editing. Things APG is silent on:
- Inline formatting toolbars (Bold / Italic / Link / List)
- Selection-based commands (highlight a word, apply formatting)
- Suggestions / autocomplete in long-form text
- Track-changes / annotations / comments

ARIA 1.2 added `suggestion`, `mark`, `insertion`, `deletion`, `emphasis`, `strong` roles to support some of these, but the interaction model is undefined. Buiy's text-editor widget contract inherits this limitation — see [`accesskit/lessons.md § Pushing rich-text through AccessKit as structured runs`](../accesskit/lessons.md): rich text flattens to single `value` strings.

### Drag-and-drop

ARIA 1.2 deprecated `aria-grabbed` / `aria-dropeffect`. The replacement is **host-language drag-and-drop semantics + WCAG 2.5.7 keyboard alternative**. APG does NOT specify the exact ARIA emission for drag-and-drop; each widget that supports drag must announce drag-start, drop-target-hover, drop, and cancel via polite live-region announcements.

Buiy [`accessibility.md § 3.11 Drag/drop ARIA`](../../specs/2026-05-07-buiy-foundation/accessibility.md) commits to: AccessKit Move-to action + keyboard alternative + polite announcements. Per-widget contracts spec'd in `buiy-input-events-design`.

## What Buiy gets from tracking the spec

- **ARIA 1.3** drafts surface new patterns (e.g. action declarations); track for early-adopter widget catalog additions
- **WCAG 3.0** drafts surface new conformance models; track for the multi-year horizon
- **APG additions** surface new design patterns; each new pattern is a candidate widget for Buiy's tier-C / tier-E roster

## Authority

The W3C ARIA Working Group at <https://www.w3.org/WAI/ARIA/> is the authoritative steward. The Web Accessibility Initiative (WAI) is the parent group. Contributions accepted via the working group's public review process (GitHub at <https://github.com/w3c/aria>).

Buiy authors who want to file an APG issue: the patterns repo is at <https://github.com/w3c/aria-practices>.

## Sources

- ARIA 1.0 (2014): <https://www.w3.org/TR/wai-aria-1.0/>
- ARIA 1.1 (2017): <https://www.w3.org/TR/wai-aria-1.1/>
- ARIA 1.2 (2023): <https://www.w3.org/TR/wai-aria-1.2/>
- ARIA 1.3 Working Draft: <https://www.w3.org/TR/wai-aria-1.3/>
- ACCNAME 1.1: <https://www.w3.org/TR/accname-1.1/>
- ACCNAME 1.2 Working Draft (20 May 2026): <https://www.w3.org/TR/accname-1.2/>
- WCAG 2.0 / 2.1 / 2.2: <https://www.w3.org/WAI/standards-guidelines/wcag/>
- WCAG 3.0 draft: <https://www.w3.org/TR/wcag-3.0/>
- Graphics ARIA 1.0: <https://www.w3.org/TR/graphics-aria-1.0/>
- DPub ARIA: <https://www.w3.org/TR/dpub-aria-1.1/>
- APG home: <https://www.w3.org/WAI/ARIA/apg/>
- APG repo: <https://github.com/w3c/aria-practices>
- Buiy 3D-anchored UI commitment: [`docs/specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Sibling files: [`patterns-catalog.md`](patterns-catalog.md), [`lessons.md`](lessons.md), [`focus-management.md`](focus-management.md)
