**Date:** 2026-05-22
**Status:** active
**Subject:** Godot Control — accessibility: pre-4.5 gap (~11 years), AccessKit landed in 4.5 (September 2025) as experimental, comparison to Buiy's AccessKit-first stance

# Accessibility

Godot's accessibility story is **two distinct eras**: a ~11-year void from Godot 1.0 (January 2014) through Godot 4.4 (March 2025), and an AccessKit-integrated era starting with Godot 4.5 (September 2025). Both eras are load-bearing for Buiy's lessons.

## Era 1: pre-4.5 — no formal a11y

For the first eleven years of Godot's life, the engine had **no screen-reader support, no platform accessibility tree, no ARIA-equivalent semantics on Controls.** Specifically:

- **No AccessKit integration.** AccessKit (the cross-platform Rust a11y bridge that abstracts UIA / NSAccessibility / AT-SPI) shipped in 2022 and was adopted by Bevy in March 2023 (Bevy 0.10); Godot did not adopt it until 4.5.
- **No native a11y APIs wired.** Godot did not register UIA providers on Windows, NSAccessibility roles on macOS, or AT-SPI services on Linux.
- **Orca on Linux: unofficial workarounds.** The Orca screen reader (the standard Linux a11y client) could detect Godot's *window* but could not enumerate Controls inside it. Blind users reported the editor (and games) as completely opaque. A handful of community workarounds existed (custom event hooks, external scripts that read state via Godot's remote-inspector debug protocol) but nothing the project officially supported.
- **No `aria-label` / `aria-describedby` analogues.** Controls had `name` (scene-tree path) and `tooltip_text` (mouse-hover hint); neither was wired to assistive tech.
- **No role / state / value taxonomy.** A Button was not announced as "button"; a CheckBox was not announced as "checkbox, checked." The widgets had no accessibility identity.
- **No focus-visible / focus-not-obscured / keyboard-trap contracts.** Focus rings existed via theme (a StyleBox for the `focus` theme item) but per-widget keyboard contracts followed each widget's `_gui_input()` implementation, not a shared APG-style spec.
- **No live regions, no global announcer.** Toast notifications and status messages couldn't reach screen readers.

This was a known weakness, surfaced repeatedly in `godot-proposals` and Hacker News commentary, but it persisted because:

- Game engines have historically deprioritized a11y (industry-wide gap, not Godot-specific).
- AccessKit didn't exist for most of Godot's history (the substrate matured 2022+).
- Adding a11y required deep refactors of the Control class hierarchy + TextServer + Theme system — none of which was scoped in 3.x or earlier 4.x releases.

## Era 2: Godot 4.5 — AccessKit lands (September 2025)

Godot 4.5 introduced **AccessKit-based screen-reader support**, contributed by [Pāvels Nadtočajevs (@bruvzg)](https://github.com/bruvzg) — the same contributor who shipped the TextServer overhaul in 4.0. From the [Godot 4.5 release notes](https://godotengine.org/releases/4.5/):

> "A feature often overlooked that is a must-have in computer software is screen reader support."

What 4.5 ships:

- **AccessKit producer integration** on Windows (UIA), macOS (NSAccessibility), and Linux (AT-SPI) via `accesskit_winit`-equivalent platform bridges adapted for Godot's own windowing.
- **Screen-reader-aware Control properties** — base `Control` exposes accessibility role + name + description + state that AccessKit serializes to the platform tree.
- **`accessibility_*` properties on Control** — `accessibility_name`, `accessibility_description`, `accessibility_live` (live-region politeness: off / polite / assertive), plus widget-type-specific value / state.
- **Bindings for Node-level customization** — non-Control nodes can also surface to the a11y tree via custom bindings, useful for in-game UI that doesn't use the standard Control hierarchy.
- **Editor coverage: partial.**
  - **Complete:** the Project Manager + the standard Control widgets in user projects.
  - **Partial:** the Inspector dock (many fields work; others don't yet).
  - **Not yet complete:** the full Godot Editor (Scene dock, FileSystem dock, asset browser, script editor — Godot 4.5 explicitly notes editor coverage is incomplete).
- **Status: "experimental."** The 4.5 announcement is explicit: this is the first release with AccessKit, and rough edges are expected. Pre-4.6 fixes are landing as 4.5.x patches.

## What is *not* covered (as of late 2025 / early 2026)

- **No drag-and-drop keyboard alternative contracts** (WCAG 2.5.7). Drag-and-drop in Godot is the user-written `_get_drag_data` / `_can_drop_data` / `_drop_data` API — there is no engine-level requirement that drag-driven widgets expose a keyboard equivalent or AT-discoverable move action.
- **No ACCNAME 1.2-conforming accessible-name computation.** The `accessibility_name` property is consulted; the algorithm for falling back to `aria-labelledby` chains, visible text, `title` is not formalized as an engine contract per widget.
- **No live-region implementation across the catalog.** `accessibility_live` is exposed on Control, but per-widget integration (e.g., toast notifications emitting `role=status`, alert dialogs emitting `role=alert` automatically) is still being wired.
- **No `aria-pressed` / `aria-checked` mixed-state semantics formalized** at the contract level (Godot's CheckBox has a third "indeterminate" visual state but the a11y emission is not yet specified).
- **Editor not fully usable** by blind developers. The Project Manager works; the script editor + Inspector tabs do not (yet).
- **No braille-display-specific properties** (`aria-braillelabel`, `aria-brailleroledescription`).
- **No forced-colors / prefers-contrast / prefers-reduced-motion** as first-class user-preference inputs. The OS dark/light preference is read, but the broader WCAG 2.2 user-preference surface is not honored automatically.
- **WCAG 2.5.8 target size** (24×24 minimum). Default theme button hit-targets are sometimes below this; no enforcement gate.

## Comparison to Buiy's AccessKit-first stance

Buiy's foundation [`accessibility.md § 3.11`](../../specs/2026-05-07-buiy-foundation/accessibility.md) commits to:

- **AccessKit-first from v1.** Buiy is v1 with AccessKit shipping; Godot waited 11 years and shipped it as "experimental" in 4.5.
- **Decomposed `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations` components.** Godot stuffs all a11y properties on the base Control class, including the editor's own widgets. This is BSN-hostile in the same shape as `bevy_a11y::AccessibilityNode` (issue [bevy/#17644](https://github.com/bevyengine/bevy/issues/17644)); see [`/home/user/buiy/docs/prior-art/bevy-ui/lessons.md`](../bevy-ui/lessons.md).
- **WCAG 2.2 AA as floor, full Level A + AA SC enumeration.** Buiy's spec enumerates every SC with an enforcement strategy (CI / RT / LR / DC). Godot's 4.5 work surfaces accessibility but doesn't claim WCAG-grade coverage.
- **APG keyboard contract per interactive widget.** Buiy's `buiy-widget-catalog-design` will spec keyboard contracts per WAI-ARIA APG; Godot's keyboard handling is per-widget in `_gui_input()` with no central contract.
- **ACCNAME 1.2 name computation.** Buiy commits to the full ACCNAME 1.2 algorithm in `buiy_core`. Godot consults `accessibility_name` but does not implement ACCNAME chains.
- **WCAG 2.5.7 dragging-movement alternative contract for every drag widget.** Buiy spec'd this in `buiy-input-events-design`. Godot has no such contract.
- **Live regions + global announcer service.** Buiy ships this from v1. Godot 4.5 has the *primitive* (`accessibility_live`); the *service* and per-widget integration are still in progress.
- **OS preference plumbing** (`prefers-color-scheme`, `prefers-reduced-motion`, `prefers-contrast`, `forced-colors`). Buiy commits to all of these as first-class. Godot reads dark/light, less for the rest.

## Implications for Buiy

- **Don't defer a11y past v1.** The single sharpest lesson from Godot's history: an 11-year a11y vacuum is hard to retrofit, even with AccessKit as the substrate. Godot 4.5's "experimental" status reflects the cost of late retrofit on a mature widget catalog.
- **Validate the AccessKit choice across game engines.** Both Buiy (via Bevy) and Godot now commit to AccessKit. This is a strong signal that AccessKit's producer model fits game-engine widget hierarchies — the substrate is the right bet even for ECS-shaped engines (Bevy/Buiy) and scene-tree-shaped engines (Godot).
- **Borrow:** the `accessibility_*` property naming on Control is clean. Buiy's `A11yLabel` / `A11yDescription` / `A11yLive` components map directly onto Godot's `accessibility_name` / `accessibility_description` / `accessibility_live` — verify Buiy's component names against Godot's property names for consistency where it doesn't cost anything.
- **Borrow:** the editor-eats-its-own-dog-food principle. Godot 4.5's biggest test of its a11y story is **the editor itself becoming usable to blind developers**. Buiy's verification harness (foundation [`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)) should run the inspector / devtools against AccessKit too — if Buiy's own tooling isn't AT-accessible, the framework's a11y claim is hollow.
- **Avoid:** stuffing all a11y into the base Node class. Godot Control has `accessibility_*` properties on the base class; this is the same megacomponent shape Bevy fell into. Decomposed components per [`/home/user/buiy/docs/prior-art/bevy-ui/lessons.md`](../bevy-ui/lessons.md) "Avoid" row 1.
- **Avoid:** shipping a11y as "experimental." Buiy's v1 commits to AccessKit-tested-in-CI; Godot's experimental status is a fair pre-1.0 marker for *their* a11y but Buiy is starting from green field and should commit to AA-from-v1, not "experimental-from-v1."

## Sources

- Godot 4.5 release notes — https://godotengine.org/releases/4.5/
- AccessKit project — https://accesskit.dev
- Pāvels Nadtočajevs (@bruvzg) — AccessKit + TextServer contributor — https://github.com/bruvzg
- Buiy foundation accessibility spec — [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- bevy-ui lessons (megacomponent + per-window AccessKit ownership) — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
- WAI-ARIA APG — https://www.w3.org/WAI/ARIA/apg/
- WCAG 2.2 — https://www.w3.org/TR/WCAG22/
- ACCNAME 1.2 — https://www.w3.org/TR/accname-1.2/
