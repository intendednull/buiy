**Date:** 2026-05-22
**Status:** active
**Subject:** Unreal Slate + UMG — accessibility coverage in 2026 (and the gap to Buiy's AccessKit-first stance)

# Accessibility

**Bottom line:** Unreal's accessibility story is limited and late. UE only began shipping a serious screen-reader integration in UE 4.22 (2019); only added mobile a11y in 4.23; still has no AT-SPI/Linux a11y story; has no AccessKit; has no first-class focus model abstraction matching WCAG's `:focus-visible` + roving tabindex + `aria-activedescendant` triad; has no WCAG conformance gate. Coverage is widget-list-limited and per-platform-uneven. This is the single biggest design gap between Slate/UMG and Buiy — and the most explicit "borrow the shape, avoid the implementation" lesson in this folder.

## What ships in 2026

Per the official Epic docs ([Supporting Screen Readers in Unreal Engine](https://dev.epicgames.com/documentation/en-us/unreal-engine/supporting-screen-readers-in-unreal-engine)):

- **Windows** — bridges to third-party screen readers (NVDA, JAWS) via UE's `IAccessibleWidget` interface + Windows IAccessible / UIA bridge in `WindowsApplicationMisc`. **Not** native MSAA/UIA tree publication; UE owns a parallel tree that is queried on demand.
- **macOS** — limited; `NSAccessibility` bridge exists but is sparsely populated.
- **iOS** — **VoiceOver** bridge. Built-in, works for common UMG widgets.
- **Android** — **TalkBack** bridge. Built-in, works for common UMG widgets.
- **Linux** — **none.** No AT-SPI integration ships. (Cross-link: AccessKit's Linux adapter is the substrate Buiy will use.)
- **Web (HTML5/WASM)** — none. Unreal's HTML5 target was deprecated in UE 4.24 (2019) and Unreal does not have an active web a11y story.

Widgets that support screen-reader text out of the box (per the Epic docs):

- `UTextBlock`
- `UEditableTextBox`
- `USlider`
- `UButton`
- `UCheckBox`

That's the **whole** documented list. Other widgets — `UComboBox`, `UScrollBox`, `UProgressBar`, `UTreeView`, `UListView`, `URichTextBlock` — require manual `IAccessibleWidget` overrides per project.

## The UMG accessibility surface

UMG exposes a small per-widget accessibility section in the Details panel:

- **`AccessibleBehavior`** — `NotAccessible` / `Auto` / `Summary` / `Custom`.
- **`AccessibleSummaryBehavior`** — same enum for the summary text (e.g. "Form with 4 fields, 1 invalid").
- **`AccessibleText`** (when behavior is `Custom`) — explicit accessible name.
- **`CanChildrenBeAccessible`** — whether descendants participate.

Toggled by **`Override Accessible Defaults`** in the Details panel. Defaults are off-by-default for most widget classes — the author must opt in per widget.

There is **no** equivalent for:

- ARIA roles (no `role="dialog"` / `role="tabpanel"` / `role="menubar"` analog).
- Live regions (`aria-live` / `role="status"` / `role="alert"`).
- Relationships (`aria-controls`, `aria-describedby`, `aria-labelledby` — `AccessibleText` only takes a literal string).
- States (`aria-expanded`, `aria-checked`, `aria-pressed`, `aria-selected` — derived implicitly from widget type, not author-settable).
- Properties (`aria-keyshortcuts`, `aria-haspopup`, etc.).

## Focus model

Slate has a focus model — `FSlateApplication::Get().SetUserFocus()` plus per-widget `bSupportsKeyboardFocus`. Tab navigation walks via the focus path. UMG exposes `IsFocusable`, `OnFocusReceived`, `OnFocusLost`.

**What's missing:**

- No `:focus-visible` distinction (keyboard-only focus indicator vs mouse-induced focus). Focus rings draw the same on click and on Tab.
- No focus traps for modal popovers — `SWindow` modal mode handles top-level modals, but in-page focus traps (a typical "open dropdown, Tab cycles inside it") are project-by-project hand-rolled.
- No focus restoration (returning focus to the trigger when a popover closes).
- No `inert` subtree primitive (blocking focus/picking/AT into a region).
- No roving tabindex / `aria-activedescendant` composite-widget pattern. Slate composite widgets (`SListView`, `STreeView`) implement their own keyboard navigation but the abstraction isn't shared and isn't a contract.

CommonUI adds spatial-navigation primitives (cardinal direction navigation for gamepads, focus groups) on top of Slate's focus — this is the de-facto modern focus stack for shipped UE games, but it is **gamepad-first**, not screen-reader-first.

## Why the gap exists

Three structural reasons.

**(1) The Editor doesn't need it.** Slate's killer use case was the Unreal Editor — a tool used internally by Epic and by professional game developers, virtually none of whom were a11y consumers in 2010-2014. The editor still has essentially no a11y coverage; the Blueprint graph editor, the level editor, the content browser are all unusable with a screen reader.

**(2) Game UI is a niche-by-niche accessibility story.** Unlike a web page, a shipped game's UI is bespoke per title — there's no "default" UI that benefits from cross-cutting a11y improvements. Each shipped UE title adds its own a11y layer (Naughty Dog's The Last of Us Part II is the famous example; it doesn't use UMG but illustrates the cost). Epic ships the *primitives*; titles are expected to do the work.

**(3) Cross-platform AT integration is hard.** AccessKit (which Buiy adopts) was started at Mozilla in 2020 and matured 2022-2026 specifically to amortize that cost across many UI frameworks. Unreal predates AccessKit by a decade; rolling its own per-platform bridge has produced uneven coverage.

## Comparison to Buiy's stance

[Foundation architecture § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md) commits Buiy to:

- **AccessKit as the source of truth.** Tree built lazily (gated on `AccessibilityRequested`), pushed as `TreeUpdate` diffs.
- **Decomposed a11y components.** `A11yRole`, `A11yLabel`, `A11yDescription`, `A11yStates`, `A11yRelations` — every WAI-ARIA concept has its own ECS component, BSN-authorable, hot-reloadable.
- **WCAG 2.2 AA at the floor.** Default theme contrast, focus ring, motion sensitivity, all gated.
- **Single focus model.** `:focus-visible` semantics, traps, restoration, `inert` subtrees, roving tabindex, `aria-activedescendant`, sequential-focus-navigation-starting-point, spatial gamepad nav.
- **Live-region announcer** as a first-class Buiy resource.

In short: every gap enumerated above is a Buiy-spec requirement. The contrast isn't accidental — Unreal's a11y story is the canonical "accessibility-as-an-afterthought-in-a-AAA-engine" case study that justifies Buiy's investment.

## Borrowable shape (despite the gaps)

A handful of Slate/UMG patterns are still worth borrowing:

- **`AccessibleBehavior` enum.** The 4-state opt-in (`NotAccessible` / `Auto` / `Summary` / `Custom`) is a clean DX pattern — pick the right shape per widget class without forcing the author to spell out a full ARIA contract every time. Buiy's `A11yRole` derives the right defaults for most widget kinds.
- **`CanChildrenBeAccessible` toggle.** A subtree-level "stop publishing accessibility from here downward" maps cleanly onto Buiy's `inert`-style subtree marker.
- **Editor-side preview of a11y output.** UMG has a Widget Reflector that can dump the accessibility tree; Buiy's devtools spec ([foundation cross-cutting](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)) includes an AccessKit tree viewer for the same reason — designers need to *see* what AT will hear.

## Sources

- Supporting Screen Readers in Unreal Engine — https://dev.epicgames.com/documentation/en-us/unreal-engine/supporting-screen-readers-in-unreal-engine
- Blind Accessibility Features Overview — https://dev.epicgames.com/documentation/en-us/unreal-engine/blind-accessibility-features-overview-in-unreal-engine
- How to directly access accessibility features (Epic forums) — https://forums.unrealengine.com/t/how-to-directly-access-accessibility-features/2674355
- Breaking Barriers Accessibility Features in Unreal Engine (MoldStud) — https://moldstud.com/articles/p-breaking-barriers-accessibility-features-in-unreal-engine
- Accessibility Game Engines (AccessForge) — https://accessforge.com/accessibility-game-engines
- Buiy foundation accessibility — [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Sibling stack: [`../bevy-a11y/`](../bevy-a11y/), [`../accesskit/`](../accesskit/)
