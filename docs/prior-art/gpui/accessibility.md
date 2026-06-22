**Date:** 2026-05-22
**Status:** active
**Subject:** GPUI — accessibility posture: no AccessKit, no screen-reader support, 2.5+ years open discussion. The cost of deferring a11y at the framework layer.

# Accessibility

**GPUI has no accessibility.** No AccessKit integration. No platform accessibility tree wired up (no AX API on macOS, no UIAutomation on Windows, no AT-SPI on Linux). No screen reader support. No structural ARIA model. No keyboard-only navigation contract enforced at the framework level.

This is the single largest disqualifier from copying GPUI's architecture wholesale into Buiy. It is also the most informative data point in this corpus about what happens when a UI framework defers accessibility past v1.

## The 2023-present open discussion

[Discussion #6576](https://github.com/zed-industries/zed/discussions/6576) — "Accessibility (a11y) in Zed" — was opened in June 2023. As of May 2026 it remains open with no concrete shipped solution.

Quoted from the team response captured in that thread (paraphrased per the WebFetch summary):

> Accessibility in Zed will be a join[t] effort between things on the Zed side, and building out features in GPUI. [It] will be a long project ... likely lasting far beyond 1.0.

Zed 1.0 shipped October 2025. Accessibility did not. The Windows release (also 2025) — per the team's own admission in [the Zed-on-Windows discourse](https://zed.dev/windows) and accessibility-discussion threads — has "zero practical accessibility with screen readers (other than system-provided dialogs like file/folder pickers)."

A separate tracking issue for VoiceOver on macOS exists (#7895). No commit landed against it.

## Why GPUI lacks accessibility

The structural reasons GPUI hasn't shipped a11y, derived from the discussion threads and the architecture:

1. **GPUI was built for Zed first.** Zed's primary users are sighted developers; accessibility was deferred at the framework's birth and the deferral compounded as the editor grew.
2. **No platform a11y API was integrated when each backend was built.** The macOS backend uses Cocoa directly without `NSAccessibility` wiring. The Windows backend uses Win32 directly without UIAutomation provider implementation. The Linux backend has no AT-SPI integration.
3. **Custom rendering means custom a11y.** Because GPUI paints its own pixels (no native widgets, no DOM), the OS has no view into the UI structure. The framework would need to actively expose an accessibility tree — exactly the job AccessKit does. But integrating AccessKit retroactively requires touching every element type to emit semantic information; nothing in `Div`'s API today carries roles, names, or states.
4. **No incentive prioritization.** Zed Industries is venture-funded; their roadmap is driven by developer-tool product growth. Accessibility is downstream of all current strategic priorities. The Feb-2026 community-deprioritization announcement ([HN 47003569](https://news.ycombinator.com/item?id=47003569)) makes this explicit — GPUI maintenance is for Zed's needs, and Zed's needs don't currently include a11y.

The recommendation that surfaced in the a11y discussion thread — "integrate AccessKit into GPUI, reference egui's AccessKit integration for the model" — is correct. It just hasn't been done.

## Cost analysis: what 2.5 years of a11y debt looks like

GPUI provides the **clearest production-scale data point** on the cost of deferring AccessKit. Specifically:

- **Affected users.** Anyone needing screen-reader interaction with Zed cannot use it as a primary editor. That includes blind and low-vision developers (a real, measurable population in software engineering).
- **Affected enterprise sales.** Enterprises with WCAG / Section 508 / EN 301 549 procurement requirements cannot buy Zed for accessibility-mandated workflows. The Series B + Sequoia investor profile suggests Zed wants to sell into enterprise; this is a real friction.
- **Affected ecosystem.** Every third-party tool that adopts GPUI (Longbridge Pro, anything built on `gpui-component`) inherits the a11y gap.
- **Affected fork — `gpui-ce` does not have a11y either.** The community fork has not addressed it; the lift remains "do it from scratch."

## What an AccessKit retrofit would require

Reverse-engineering from Buiy's own AccessKit-first design (foundation §2.6, [`docs/prior-art/accesskit/integration.md`](../accesskit/integration.md)), retrofitting AccessKit into GPUI would need:

1. **Per-window adapter ownership.** Each `Window` needs to own an `accesskit_winit::Adapter` (or platform-specific equivalent, since GPUI doesn't use winit — it would need its own `accesskit_macos::Adapter`, `accesskit_windows::Adapter`, `accesskit_unix::Adapter` wiring).
2. **Element-level semantic data.** Every `Element` would need to provide role, name, value, state. The cleanest path is decomposed accessibility components attached via the `Styled` trait — same shape as `.bg(color)` but `.aria_role(Role::Button).aria_label("Save")`.
3. **`TreeUpdate` emission on view notifications.** When `cx.notify()` fires, the affected subtree needs to push a `TreeUpdate` diff to the adapter.
4. **Stable `NodeId`s.** Currently elements get fresh IDs each frame (the immediate-mode reality inside the retained-mode shell). AccessKit needs stable IDs across frames — this requires a hash strategy keyed on (`element_path`, `key`) or similar.
5. **`ActionRequest` dispatch.** When the OS AT triggers an action (click, focus, set value), the request needs to route into the action system to fire the equivalent of a user-driven event.
6. **ACCNAME 1.2 computation.** Accessible name resolution from label / labelled-by / text-content fallbacks.
7. **Live regions.** Polite/assertive announcement queue with a global announcer.

This is genuinely a multi-quarter effort. Each step is well-understood (Buiy is doing it from scratch in `buiy-accessibility-design`), but the GPUI codebase doesn't decompose along these axes today — `Div`'s API has no slot for ARIA-shaped semantics. **The retrofit is roughly the cost of rebuilding AccessKit-integration from scratch in a new codebase**, which is what `egui_accesskit` did in eframe, what `bevy_a11y` did in Bevy, and what Buiy is doing in `buiy_core`.

## Lesson for Buiy: accessibility-first is the cheap path

The GPUI evidence makes Buiy's foundation §2.6 commitment ("AccessKit-first") unambiguously correct. The retrofit cost grows superlinearly with codebase age and adoption — the longer you wait, the harder it becomes to redesign component APIs to carry semantic data, and the larger the surface of widget code that needs updating.

**Three concrete inheritances from GPUI's negative example:**

1. **Decomposed a11y components from day one.** Buiy's `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations` (foundation §2.6) live on every widget at construction. No "add ARIA later" path.
2. **Stable NodeIds derived from Bevy Entity.** Buiy decisions (foundation §2.6) — `NodeId` from `Entity::index()` — give stable identifiers across frames without the hash-key dance GPUI would need.
3. **Lazy adapter activation.** AccessKit's `update_if_active` gates a11y work on AT-attached state. Buiy honors the same gate via `AccessibilityRequested` (foundation §2.6); idle apps pay nothing. GPUI could adopt this trivially today and gain "zero-cost a11y when no AT attached" as a sales point.

## Could Buiy hypothetically depend on GPUI for rendering and bolt a11y on top?

No. Two structural blockers:

1. **GPUI does not surface element semantics.** There's no `Element::role()` or `Element::accessible_name()` to read from. To wire AccessKit, you'd need to modify GPUI's source — the patches would be substantial and would land in a Zed-controlled Apache-2.0 codebase with no community contribution channel post-Feb-2026.
2. **GPUI's element-tree-rebuilt-per-frame model needs stable IDs for AccessKit.** This is solvable but requires a hash-key strategy added to `Element`/`Div`.

Even if those were resolved, GPUI's lack of AT-API integration on each platform means **Buiy would be writing the `accesskit_macos::Adapter` integration, the `accesskit_windows::Adapter` integration, and the `accesskit_unix::Adapter` integration into GPUI itself**. That's the bulk of the a11y work. There's no shortcut.

The right answer for Buiy is the answer foundation §2.6 already commits to: own the AccessKit integration in `buiy_core`, build trees from Buiy's decomposed components, talk to `accesskit_winit::Adapter` directly. GPUI is reference for what _not_ to do here.

## Sources

- Accessibility discussion #6576: https://github.com/zed-industries/zed/discussions/6576
- VoiceOver tracking #8146: https://github.com/zed-industries/zed/discussions/8146
- HN: Zed deprioritizing GPUI community work: https://news.ycombinator.com/item?id=47003569
- Zed on Windows (accessibility-gap admission context): https://zed.dev/windows
- Cross-link: AccessKit lessons file (names GPUI as verified-false AccessKit adopter): [`docs/prior-art/accesskit/lessons.md`](../accesskit/lessons.md)
- Cross-link: bevy_a11y BSN issue #17644 (the megacomponent problem Buiy is structured to avoid): https://github.com/bevyengine/bevy/issues/17644
