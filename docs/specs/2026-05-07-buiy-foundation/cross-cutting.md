# Feature inventory — cross-cutting

**Parent:** [README.md](README.md)

Tier legend: **F** = foundation, **C** = core, **E** = extended, **O** = out (excluded, with reason). See [README.md § Tier legend](README.md#tier-legend).

Cross-cutting categories that don't fit a single subsystem: i18n / locale, state / data / reactivity, theming, devtools, 3D-anchored UI (deferred subsystem), and compatibility / coexistence with bevy_ui.

## 3.12 Internationalization and locale

- `lang` analogue per Buiy text; plumbed to AccessKit for AT pronunciation. **F**
- `dir` LTR/RTL with full UI mirroring (scrollbars, sliders, progress bars, icons). **F**
- Logical CSS properties (start/end vs left/right). **F**
- BiDi UAX #9, full implementation. **F**
- Vertical writing modes (CJK, Mongolian). **C**
- ICU MessageFormat 2.0 for translation strings (placeholders, plurals, gender, select, ordinals). **C**
- Locale-aware formatters: NumberFormat, DateTimeFormat, RelativeTimeFormat, PluralRules, ListFormat, Collator, Segmenter, DisplayNames, DurationFormat. **C**
- Calendar systems (Gregorian, Hebrew, Islamic, Buddhist, Japanese, Persian). **E**
- Numbering systems (Latin, Arabic-Indic, others). **E**
- Pseudolocale support for QA. **E**

## 3.13 State, data, reactivity

- Two-way binding for input value / checked / selected. **F**
- Form state machine. **F**
- Validation state propagation. **F**
- Selection state (single / multi / range with shift-click, ctrl/cmd-toggle). **F**
- Drag state. **C**
- Pseudo-class state surface ([interaction.md § 3.7](interaction.md)). **F**
- Reactivity primitive: Bevy observers + change detection. **F**
- Signal / computed / effect layer. **O** — explicitly excluded from the foundation. May return as a follow-up sub-spec if usage demands it; the spec does not commit to that.
- **Out:** History API / URL routing, `localStorage` / `sessionStorage` / `IndexedDB`. UI does not own persistence or routing. **O**

## 3.14 Theming and user preferences

(Most of this is restated from [architecture.md § 2.5](architecture.md) for catalog completeness.)

- Semantic tokens. **F**
- Theme assets (hot-reloadable). **F**
- Variants (light / dark / high-contrast / custom). **F**
- Scales (color, spacing, typography, motion, radius, elevation). **F**
- Theme inheritance — subtree can carry its own `Theme` component to override. **C**
- All `prefers-*` queries surfaced as `UserPreferences` resource. **F**
- Forced-colors mode → token palette swap with system colors. **F**
- Reduced-motion → animation short-circuit. **F**
- Color-scheme → variant swap. **F**
- Color-gamut detection (`(color-gamut: srgb | p3 | rec2020)`). **C**
- Pointer / hover media features (`(pointer: none|coarse|fine)`, `(hover: none|hover)`). **C**
- Dynamic-range detection (`(dynamic-range: standard | high)`). **E**
- CSS-flavored stylesheet — out of this spec; future sub-spec if needed.

## 3.16 Devtools / DX

- Inspector (entity / component view of Buiy hierarchy). **C**
- Layout overlay (Flexbox / Grid lines, box-model boundaries). **C**
- AccessKit tree viewer. **C**
- Contrast checker (live, against current theme). **C**
- Focus order visualizer (numbered overlay showing tab order). **C**
- Performance profiler (per-frame layout / render / AccessKit timing). **C**
- Theme editor (live token tweaking). **C**
- BSN hot-reload indicator. **C**
- Verification harness CLI for downstream Buiy users. **C**

## 3.17 3D-anchored UI (deferred subsystem)

- UI panels as billboards in 3D space. **C**
- UI panels on curved or arbitrary surfaces. **E**
- Worldspace UI hit-testing through the 3D scene. **C**
- Diegetic UI (UI that lives "in" the game world — terminals, screens, holograms). **C**
- Render-to-texture for UI applied to 3D meshes. **C**

This subsystem gets its own design spec (`buiy-3d-anchored-ui-design`). No `UiTransform` / `Transform` divergence to bridge — Buiy nodes use Bevy's general `Transform`.

## 3.18 Compatibility and coexistence

Coexistence with bevy_ui is **per-window**, not per-app-shared-window. AccessKit allows exactly one tree per window adapter; one window cannot host both bevy_ui and Buiy a11y trees simultaneously without a coordinator. The supported model is:

- An app may have multiple windows. Each window is **owned by one stack**: either Buiy or bevy_ui.
- On a Buiy-owned window: Buiy owns the `accesskit_winit::Adapter`, the render-graph nodes, the `bevy_picking` backend(s), the focus model, the IME consumer. `bevy_a11y` is suppressed for that window. bevy_ui's own systems do not render or interact on that window.
- On a bevy_ui-owned window: bevy_ui retains its current behavior. Buiy is absent.
- Inside a single Buiy tree, you do not mix raw `bevy_ui::Node` content. The component models are independent.
- Migration from a bevy_ui window to a Buiy window is by replacement of the window's UI tree, not by extending bevy_ui components.

**Why per-window, not coordinator-merged.** A merge coordinator (single AccessKit adapter, both stacks pushing subtrees under one root) is theoretically possible but adds a coordination crate, ID-space rules, focus-arbitration rules, and IME-routing rules — meaningful complexity for a use case (mixing Buiy and bevy_ui in one window) the spec does not need. If demand arises later, that becomes a follow-up sub-spec (`buiy-coexistence-design`).

**Coexistence rules — committed:**
- One stack per window. **F**
- Window stack assignment is fixed at window creation; **no runtime stack switching** for an existing window in v1. **F**
- Buiy components do not extend `bevy_ui::Node`. **F**
- Migration from bevy_ui → Buiy is by per-window replacement. **F**
- Per-window state keyed by winit `WindowId`: AccessKit adapter, IME consumer, focus tree root, render-graph node group, `bevy_picking` backend filter. **F**
- Render-graph node ordering, `bevy_picking` backend priority, IME consumer selection, focus arbitration: per-window stack owns these unilaterally on its own window. **F**
- **`UiPickingPlugin` interaction:** when a Buiy window is present, Buiy's picking backend is filtered to that window via `bevy_picking`'s window-filter. Bevy's default `UiPickingPlugin` (added by `DefaultPlugins`) operates on bevy_ui-owned windows only. If an app is Buiy-only and adds `DefaultPlugins`, `UiPickingPlugin` runs on no windows and is a no-op. **F**

**Excluded entirely**
- Networking, fetch, XHR, WebSocket, WebRTC, WebTransport. **O**
- Cookies, localStorage, IndexedDB, service workers, web app manifest, install. **O**
- File System Access, Web Bluetooth, USB, Serial, MIDI, NFC, geolocation. **O**
- WebAuthn, Payment Request. **O**
- Speech recognition / synthesis (game audio, not UI). **O**
- DRM / EME. **O**
- `data:` / `blob:` URL schemes. **O**
- Native iframe / sandbox / cross-origin. **O**
- Same-origin policy, CORS, CSP. **O**
- HTML parser quirks mode, document.write, mutation events (deprecated). **O**
- Print stylesheet (`@media print`, `@page`). **O** — Bevy is not a print target.
- SMIL animation. **O** — superseded by CSS / WAAPI analogue.
- Legacy presentational HTML attributes. **O**
