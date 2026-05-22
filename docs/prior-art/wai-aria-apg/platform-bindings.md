**Date:** 2026-05-22
**Status:** active
**Subject:** WAI-ARIA APG — ARIA role / state / property mappings to platform accessibility APIs (UIA, NSAccessibility, AT-SPI, TalkBack, VoiceOver); gotchas Buiy inherits via AccessKit's adapters

# Platform bindings (ARIA → platform a11y APIs)

ARIA is the producer-side vocabulary; assistive technologies consume platform a11y APIs (UIA on Windows, NSAccessibility on macOS, AT-SPI on Linux, Android `AccessibilityNodeInfo`, iOS UIAccessibility). The Core Accessibility API Mappings spec (Core-AAM 1.2, <https://www.w3.org/TR/core-aam-1.2/>) defines how ARIA roles + states + properties map onto these APIs.

For Buiy, **AccessKit owns the per-platform translation** ([`prior-art/accesskit/platform-adapters.md`](../accesskit/platform-adapters.md)). Buiy emits AccessKit `Node`s; AccessKit's per-platform adapter crates (`accesskit_windows`, `accesskit_macos`, `accesskit_unix`, `accesskit_android`, `accesskit_ios`) translate to the platform vocabulary. This file documents the resulting AT-side behaviour Buiy must verify against — because while the **AccessKit tree** is Buiy's testable artifact (verification gate 3), the **end-user behaviour** depends on the platform AT consuming the translated tree.

## Platform adapters at a glance

| Platform | Adapter | API | Primary AT consumers |
|---|---|---|---|
| Windows | `accesskit_windows` | UI Automation (UIA) | NVDA, JAWS, Narrator |
| macOS | `accesskit_macos` | NSAccessibility | VoiceOver |
| Linux (X11 + Wayland) | `accesskit_unix` | AT-SPI / D-Bus | Orca |
| Android | `accesskit_android` | `AccessibilityNodeInfo` (View hierarchy) | TalkBack |
| iOS | `accesskit_ios` (shipped 2026-05-11, basic) | UIAccessibility | VoiceOver iOS |
| Web | not yet | DOM ARIA via shadow tree | Browser-resident AT |

Production maturity: Windows and macOS are first-class; Linux is production but Wayland has gaps; Android is pre-1.0 shipping; iOS is alpha as of folder-write; Web is not started. The Buiy foundation [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md) defers Android / iOS / Web to manual-release-gate.

## ARIA → UIA (Windows)

| ARIA | UIA Control Type | Notes |
|---|---|---|
| `button` | `Button` | Standard |
| `checkbox` | `CheckBox` | `aria-checked="mixed"` → IndeterminateState pattern |
| `combobox` | `ComboBox` | Plus `ValuePattern`, `ExpandCollapsePattern`, `SelectionPattern` |
| `dialog` | `Window` or `Pane` | Modal flag via `IsModal` property |
| `link` | `Hyperlink` | |
| `listbox` | `List` | Items are `ListItem`s with `SelectionItemPattern` |
| `menu` | `Menu` | Items `MenuItem` |
| `radio` | `RadioButton` | `SelectionItemPattern` |
| `slider` | `Slider` | `RangeValuePattern` |
| `spinbutton` | `Spinner` | `RangeValuePattern` |
| `switch` | `Button` with TogglePattern | UIA has no native Switch; Buiy / AccessKit emits Button + TogglePattern |
| `tab` | `TabItem` | |
| `tablist` | `Tab` | |
| `tabpanel` | `Pane` | |
| `tree` | `Tree` | |
| `treeitem` | `TreeItem` | Plus `ExpandCollapsePattern` if expandable |
| `grid` | `DataGrid` | |
| Landmarks (e.g. `main`, `navigation`) | `Group` | UIA has no landmark; AT use `LocalizedControlType` |

**Gotcha.** UIA distinguishes "control type" (the role) from "control patterns" (behaviours). A combobox is a ComboBox with `ValuePattern` + `ExpandCollapsePattern` + `SelectionPattern`. AccessKit's adapter emits the right pattern set per role.

## ARIA → NSAccessibility (macOS)

| ARIA | NSAccessibility Role |
|---|---|
| `button` | `NSAccessibilityButtonRole` |
| `checkbox` | `NSAccessibilityCheckBoxRole` |
| `combobox` | `NSAccessibilityComboBoxRole` |
| `dialog` | `NSAccessibilityWindowRole` (modal) or sheet |
| `link` | `NSAccessibilityLinkRole` |
| `listbox` | `NSAccessibilityListRole` |
| `menu` | `NSAccessibilityMenuRole` (popup) or `NSAccessibilityMenuBarRole` |
| `radio` | `NSAccessibilityRadioButtonRole` |
| `slider` | `NSAccessibilitySliderRole` |
| `switch` | `NSAccessibilityCheckBoxRole` with subrole `NSAccessibilitySwitchSubrole` |
| `tab` | `NSAccessibilityRadioButtonRole` with subrole `NSAccessibilityTabRole` (VoiceOver convention) |
| `tablist` | `NSAccessibilityTabGroupRole` |
| `tree` | `NSAccessibilityOutlineRole` (note the rename) |
| `treeitem` | `NSAccessibilityRowRole` with subrole `NSAccessibilityOutlineRowSubrole` |
| `grid` | `NSAccessibilityTableRole` |
| `gridcell` | `NSAccessibilityCellRole` |
| Landmarks | typically `NSAccessibilityGroupRole` with landmark identifier in `accessibilityRoleDescription` |

**Gotcha.** NSAccessibility uses "subrole" liberally; the Buiy author doesn't care, but verification snapshots may differ in subrole between AT versions.

**Known issue.** AccessKit [issue #520](https://github.com/AccessKit/accesskit/issues/520) — ListBox selected state not properly communicated to AT on macOS. Buiy ships a per-platform expect-fail fixture pending upstream fix. ([`accesskit/lessons.md § Avoid`](../accesskit/lessons.md))

## ARIA → AT-SPI (Linux)

| ARIA | AT-SPI Role |
|---|---|
| `button` | `ROLE_PUSH_BUTTON` |
| `checkbox` | `ROLE_CHECK_BOX` |
| `combobox` | `ROLE_COMBO_BOX` |
| `dialog` | `ROLE_DIALOG` |
| `link` | `ROLE_LINK` |
| `listbox` | `ROLE_LIST_BOX` |
| `menu` | `ROLE_MENU` |
| `menuitem` | `ROLE_MENU_ITEM` |
| `radio` | `ROLE_RADIO_BUTTON` |
| `slider` | `ROLE_SLIDER` |
| `tab` | `ROLE_PAGE_TAB` |
| `tablist` | `ROLE_PAGE_TAB_LIST` |
| `tree` | `ROLE_TREE` |
| `treeitem` | `ROLE_TREE_ITEM` |
| `grid` | `ROLE_TABLE` (no separate grid role) |
| Landmarks | `ROLE_LANDMARK` or `ROLE_SECTION` |

**Gotchas.**

- **Wayland window position is hidden.** `winit::Window::inner_position()` returns `Err` on most Wayland desktops (sandbox model). AT-SPI bounds reports may be wrong / relative. Verify on BOTH X11 and Wayland sessions — Buiy ships fixtures for both per the foundation [`architecture.md § 2.9`](../../specs/2026-05-07-buiy-foundation/architecture.md) open question.
- **`aria-activedescendant` semantics fuzzy on Orca.** Orca historically lagged on this pattern; behaviour uneven across Orca versions. The verification harness records the tree but doesn't gate on Orca-specific behaviour.
- **D-Bus async runtime required.** `accesskit_unix` requires an async runtime feature (`tokio` or `async-io`). Buiy uses `async-io`. Pin in deployment docs.

## ARIA → Android (`AccessibilityNodeInfo`)

Android's accessibility tree is built from the View hierarchy. `AccessibilityNodeInfo` exposes:
- `className` (e.g. `android.widget.Button`) — AT use this for role inference
- `contentDescription` — primary name source
- `text` — value
- `checkable` / `checked` for toggle states
- `clickable` / `longClickable` / `scrollable` / `focusable` / `focused` — action flags
- `collectionInfo` / `collectionItemInfo` for grids and lists

**Gotchas.**

- Role mapping is by `className` string — AccessKit's adapter picks reasonable Android classes per ARIA role
- `accesskit_android` is pre-1.0; coverage incomplete per the upstream README
- TalkBack consumes the tree; verification is via TalkBack manual-release-gate
- No equivalent of `aria-activedescendant`; Android uses real focus

## ARIA → iOS (UIAccessibility)

UIAccessibility is informal-by-comparison; the tree is built from the UIKit view hierarchy. Each accessible view exposes:
- `accessibilityLabel` — name
- `accessibilityHint` — description
- `accessibilityValue` — value
- `accessibilityTraits` — flags (Button, Link, Header, Selected, NotEnabled, etc.)

**Gotchas.**

- **`accesskit_ios` 0.1.0 shipped 2026-05-11** — basic, alpha. Per [`accesskit/README`](../accesskit/README.md): ~229 lifetime downloads at folder-write; no production app reported shipping on it
- iOS VoiceOver consumes the tree; verification via iOS VoiceOver manual-release-gate (deferred from CI in Buiy's foundation spec)

## ARIA → DOM (web, browser-resident AT)

The web platform is **the** original ARIA target. Browsers consume ARIA attributes from the DOM and translate to their internal accessibility tree, which is then exposed to platform AT (UIA on Windows, etc.).

**Buiy implication.** AccessKit has no `accesskit_web` adapter yet. The Buiy Bevy-on-WASM target is partial-a11y until upstream ships. The web case is architecturally different from desktop: DOM-aligned ARIA in a shadow tree, not a parallel `Tree`/`Node` push protocol.

## HTML Accessibility API Mappings (HTML-AAM)

For native HTML elements, HTML-AAM 1.0 (<https://www.w3.org/TR/html-aam-1.0/>) specifies the mapping from HTML element to platform a11y role / state / property. Buiy is NOT an HTML implementation, but where Buiy's authoring layer ([`media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)) names components after HTML elements (`Heading`, `Image`, `Label`, `Section`), the HTML-AAM mapping is the reference for what AT expects.

## ARIA in HTML

The ARIA in HTML spec (<https://www.w3.org/TR/html-aria/>) specifies which ARIA attributes are valid on which HTML elements. A11y linters (axe, Lighthouse, WAVE) enforce this; the rules are:

- Certain ARIA roles cannot override certain native HTML roles (e.g. `<button role="link">` is invalid)
- Some ARIA attributes are redundant with native HTML attributes (`aria-required` on a `<input required>`)
- Some ARIA attributes conflict with native semantics

Buiy's component model doesn't have an HTML substrate, but the same constraints apply when emitting AccessKit roles + properties. The Buiy author can't make a `Button` emit `Role::Link` — the component contract enforces the role per widget.

## Verification strategy

The Buiy verification harness ([`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)):

- **CI-gated:** AccessKit tree snapshot per widget — captures role + name + description + states + relations. Platform-API specifics (UIA `ControlType`, NSAccessibility subrole) are NOT captured in CI; AccessKit owns the translation
- **Manual-release-gate:** test against real AT — NVDA, JAWS, Narrator (Windows), VoiceOver (macOS, iOS), Orca (Linux X11 + Wayland), TalkBack (Android). Utterances drift across AT versions; gate on AccessKit tree shape, not AT speech
- **Per-platform expect-fail fixtures:** for known upstream bugs (macOS ListBox issue #520, etc.) — fixtures pass on bug-free platforms, expected-fail on affected ones

## Open issues we inherit

| Issue | Platform | Source |
|---|---|---|
| ListBox selected state | macOS | AccessKit #520 |
| `aria-activedescendant` semantics | Linux AT-SPI / Orca | AccessKit critiques |
| Wayland window position | Linux Wayland | `winit::Window::inner_position()` returns `Err` |
| Android pre-1.0 | Android | AccessKit `accesskit_android` 0.7.3 |
| iOS alpha | iOS | `accesskit_ios` 0.1.0 (2026-05-11) |
| No web adapter | Web | not started upstream |

Documented in [`prior-art/accesskit/platform-adapters.md`](../accesskit/platform-adapters.md) and [`prior-art/accesskit/critiques.md`](../accesskit/critiques.md).

## Sources

- Core-AAM 1.2: <https://www.w3.org/TR/core-aam-1.2/>
- HTML-AAM 1.0: <https://www.w3.org/TR/html-aam-1.0/>
- ARIA in HTML: <https://www.w3.org/TR/html-aria/>
- UI Automation overview (Microsoft): <https://learn.microsoft.com/en-us/windows/win32/winauto/entry-uiauto-win32>
- NSAccessibility (Apple): <https://developer.apple.com/documentation/appkit/nsaccessibility>
- AT-SPI 2 spec: <https://accessibility.linuxfoundation.org/a11yspecs/atspi/adoc/>
- Android Accessibility: <https://developer.android.com/guide/topics/ui/accessibility>
- iOS UIAccessibility: <https://developer.apple.com/documentation/uikit/accessibility_for_uikit>
- AccessKit platform adapters: [`docs/prior-art/accesskit/platform-adapters.md`](../accesskit/platform-adapters.md)
- AccessKit lessons (Avoid section): [`docs/prior-art/accesskit/lessons.md`](../accesskit/lessons.md)
- Sibling files: [`patterns-catalog.md`](patterns-catalog.md), [`roles-states-properties.md`](roles-states-properties.md), [`focus-management.md`](focus-management.md)
