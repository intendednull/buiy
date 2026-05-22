**Date:** 2026-05-22
**Status:** active
**Subject:** WAI-ARIA APG — glossary of accessibility terms used across this folder; cross-linked to where each term is load-bearing

# Glossary

Terms used in this folder. Cross-link to where each term is load-bearing.

## A

**ACCNAME** — Accessible Name and Description Computation. The W3C specification (currently <https://www.w3.org/TR/accname-1.2/>, Working Draft 20 May 2026) defining the precise algorithm for deriving the accessible name and description of any node. See [`name-computation.md`](name-computation.md).

**Accessible name** — The string an AT (screen reader) announces to identify a widget. Computed per ACCNAME from `aria-labelledby` > `aria-label` > host-language label > content > title.

**Accessible description** — Supplementary text announced by AT after (or on demand) the name. Computed per ACCNAME from `aria-describedby` > `aria-description` > host-language description > title.

**APG** — WAI-ARIA Authoring Practices Guide. The non-normative companion to ARIA that documents widget design patterns. Maintained by the W3C ARIA Working Group at <https://www.w3.org/WAI/ARIA/apg/>. See [`README.md`](README.md).

**ARIA** — Accessible Rich Internet Applications. The W3C specification (currently <https://www.w3.org/TR/wai-aria-1.2/>) defining roles, states, and properties for making web content accessible. Companion to APG. See [`roles-states-properties.md`](roles-states-properties.md).

**`aria-*` attribute** — A property or state in the ARIA vocabulary. Examples: `aria-label`, `aria-labelledby`, `aria-expanded`, `aria-controls`. See [`roles-states-properties.md`](roles-states-properties.md).

**`aria-activedescendant`** — Property naming the "logical focus" descendant of a composite widget while DOM focus stays on the widget root. Alternative to roving tabindex. See [`focus-management.md`](focus-management.md).

**`aria-labelledby`** / **`aria-describedby`** — Properties referencing other nodes whose text becomes the labelled/described node's name/description. Note: British spelling on AccessKit side (`labelled_by`). See [`name-computation.md`](name-computation.md).

**`aria-live`** — Property declaring a region as "live" — content updates are announced to AT asynchronously. Values: `off` / `polite` / `assertive`. See [`live-regions.md`](live-regions.md).

**AT (assistive technology)** — Software or hardware that translates the platform a11y API into a user-experience modality: screen reader (NVDA, JAWS, VoiceOver, TalkBack, Orca, Narrator), braille display, voice control, switch input, etc. See [`platform-bindings.md`](platform-bindings.md).

**AT-SPI** — Assistive Technology Service Provider Interface. The Linux platform a11y API, consumed by Orca and other Linux AT. AccessKit uses `accesskit_unix` to translate. See [`platform-bindings.md`](platform-bindings.md).

## C

**Composite widget** — An ARIA widget with focusable descendants — Listbox, Menu, Tree, Tabs, Grid, etc. Tab enters the composite at one descendant; arrow keys navigate within. Two focus patterns: roving tabindex or `aria-activedescendant`. See [`focus-management.md`](focus-management.md).

**Core-AAM** — Core Accessibility API Mappings. The W3C spec (<https://www.w3.org/TR/core-aam-1.2/>) defining how ARIA roles + states + properties map onto platform a11y APIs. See [`platform-bindings.md`](platform-bindings.md).

## F

**Focus trap** — A focus-management pattern where focus is constrained within a container (e.g. modal dialog). Tab from the last focusable wraps to the first; Shift+Tab from the first wraps to the last. Buiy uses `inert` on the rest of the window. See [`focus-management.md`](focus-management.md).

**`:focus-visible`** — A CSS pseudo-class that styles only the focus state triggered by keyboard / AT (not pointer). See [`focus-management.md`](focus-management.md).

## H

**HTML-AAM** — HTML Accessibility API Mappings. The W3C spec (<https://www.w3.org/TR/html-aam-1.0/>) defining how HTML elements map onto platform a11y APIs. See [`platform-bindings.md`](platform-bindings.md).

## I

**Inert** — HTML attribute (and ARIA convention) marking a subtree as excluded from focus + AT + hit-testing. Used by modal dialogs to suppress the rest of the document. See [`focus-management.md`](focus-management.md).

## L

**Landmark** — An ARIA role identifying a major page region: `banner`, `navigation`, `main`, `complementary`, `contentinfo`, `region`, `search`, `form`. AT users navigate by landmark. See [`roles-states-properties.md`](roles-states-properties.md).

**Live region** — A subtree marked with `aria-live` (or implicitly via `role=alert` / `status` / `log` / `timer`) that announces updates to AT asynchronously. See [`live-regions.md`](live-regions.md).

## N

**Name from content** — An ACCNAME rule where the accessible name is computed by recursively concatenating descendant text. Applies to certain roles (`button`, `link`, `cell`, `heading`, `menuitem`, ...). See [`name-computation.md`](name-computation.md).

**NSAccessibility** — The macOS platform a11y API, consumed by VoiceOver. AccessKit uses `accesskit_macos`. See [`platform-bindings.md`](platform-bindings.md).

## P

**Property** — In ARIA, an attribute whose value is **less likely to change** than a state. Describes the widget's role-essential attributes. Examples: `aria-haspopup`, `aria-orientation`, `aria-valuemax`. See [`roles-states-properties.md`](roles-states-properties.md).

## R

**Role** — An ARIA attribute that identifies the widget kind: `button`, `link`, `combobox`, `dialog`, etc. The role determines which states and properties are valid + which keyboard contract applies. See [`roles-states-properties.md`](roles-states-properties.md).

**Roving tabindex** — A composite-widget focus pattern where exactly one descendant has `tabindex="0"` (in tab sequence) at any time; all others have `tabindex="-1"`. Arrow keys move both `tabindex="0"` and DOM focus from one descendant to another. Alternative to `aria-activedescendant`. See [`focus-management.md`](focus-management.md).

## S

**SC (success criterion)** — A WCAG conformance requirement at Level A, AA, or AAA. WCAG 2.2 has 50 Level A + AA SCs plus AAA. See [`wcag-22-aa-mapping.md`](wcag-22-aa-mapping.md).

**Screen reader** — An AT that reads on-screen content aloud. Major examples: NVDA, JAWS, Narrator (Windows); VoiceOver (macOS, iOS); Orca (Linux); TalkBack (Android). See [`platform-bindings.md`](platform-bindings.md).

**Sequential Focus Navigation Starting Point** — An AccessKit action (`Action::SetSequentialFocusNavigationStartingPoint`) that tells the focus model "Tab from here next time the user presses Tab". Used for AT virtual-cursor positioning and skip-links. See [`focus-management.md`](focus-management.md).

**State** — In ARIA, an attribute whose value is **expected to change** in response to user interaction. Examples: `aria-checked`, `aria-expanded`, `aria-pressed`. See [`roles-states-properties.md`](roles-states-properties.md).

## T

**Tabindex** — HTML attribute controlling whether and where an element appears in the tab sequence. `0` = in sequence at its natural position; `-1` = focusable programmatically but not in tab sequence; positive values are an anti-pattern. See [`focus-management.md`](focus-management.md).

**Type-ahead** — A keyboard pattern in Menu, Listbox, Combobox, Tree where typing letters moves focus to the next item whose name starts with that letter. Multi-char prefix supported with debounce. See [`keyboard-contracts.md`](keyboard-contracts.md).

## U

**UIA (UI Automation)** — The Windows platform a11y API, consumed by NVDA, JAWS, Narrator. AccessKit uses `accesskit_windows`. See [`platform-bindings.md`](platform-bindings.md).

**UIAccessibility** — The iOS platform a11y API, consumed by VoiceOver. AccessKit uses `accesskit_ios` (shipped 2026-05-11, alpha). See [`platform-bindings.md`](platform-bindings.md).

## W

**WAI** — Web Accessibility Initiative. The parent W3C group under which the ARIA Working Group operates. <https://www.w3.org/WAI/>.

**WAI-ARIA** — Same as ARIA. Full name: "Accessible Rich Internet Applications". See [`roles-states-properties.md`](roles-states-properties.md).

**WCAG** — Web Content Accessibility Guidelines. The W3C spec (currently <https://www.w3.org/TR/WCAG22/>, version 2.2, Recommendation 5 October 2023) defining accessibility success criteria at Level A / AA / AAA. See [`wcag-22-aa-mapping.md`](wcag-22-aa-mapping.md).

**Widget** — In APG, a focusable interactive UI control or composite. See [`patterns-catalog.md`](patterns-catalog.md).

## Sources

- W3C glossary: <https://www.w3.org/glossary/>
- WCAG 2.2 glossary: <https://www.w3.org/TR/WCAG22/#glossary>
- ARIA 1.2 important terms: <https://www.w3.org/TR/wai-aria-1.2/#important_terms>
