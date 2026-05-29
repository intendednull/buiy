**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui — first-party companion crates + third-party alternative UI stacks

# Ecosystem

bevy_ui sits at the center of a layered ecosystem: first-party companion crates inside the Bevy workspace, plus a thriving set of third-party UI stacks that either build *on top of* bevy_ui or stand *parallel to* it (the same architectural choice Buiy makes — see [comparisons.md](comparisons.md)). This file catalogs the neighborhood. Each system will get its own prior-art folder later under `docs/prior-art/<system>/`; cross-link to those when they exist.

## First-party (in the `bevyengine/bevy` workspace)

### bevy_feathers — official widget kit

Bevy's opinionated widget toolkit, targeted at the planned **Bevy Editor**, World Inspector, and other dev tooling. **Introduced in Bevy 0.17** (2025-09-30) via PR #19730 (ickshonpe et al.). Gated behind the `experimental_bevy_feathers` feature flag. Provides Button, Slider, Checkbox, MenuButton, virtual keyboard, and (added in 0.18) a `ColorPlane` 2D color picker. Builds on top of **bevy_ui_widgets** (below) for headless behavior and adds Bevy-Editor-flavored styling and theming.

**Buiy relationship:** Feathers is an opinionated tooling widget set; Buiy is a general-purpose UI library. They overlap in widget catalog but diverge in styling philosophy (Feathers is tightly themed for editor look-and-feel; Buiy is token-driven and WCAG-floor-by-default). Per foundation README § 5 open question, whether Buiy ships migration adapters *from* Feathers components is undecided.

Orchestrator pre-amble correction: pre-amble said "ships in Bevy 0.16+"; actual is **0.17+**.

→ *Future folder:* `docs/prior-art/bevy-feathers/`.

### bevy_ui_widgets — headless widget primitives

The headless / unstyled widget primitives, also **introduced in Bevy 0.17**. Output of discussion #16900 (Standard Headless Widgets, viridia, December 2024). Widgets: `CoreButton`, `CoreSlider`, `CoreScrollbar`, `CoreCheckbox`, `CoreRadioButton`. Most widgets use *external state management* — they emit change events rather than mutating internal state, so apps update widget state in response. Still experimental; API expected to change.

**Buiy relationship:** Conceptually closest to what Buiy's `buiy_widgets` crate will provide (foundation architecture.md § 2.8). Buiy's widget catalog will *include APG keyboard contracts and a11y wiring as part of the widget contract* — bevy_ui_widgets does not enforce APG patterns at the same depth.

→ *Future folder:* `docs/prior-art/bevy-ui-widgets/`.

### bevy_a11y — AccessKit bridge

The crate that wires Bevy entities to AccessKit's accessibility tree. **Introduced in Bevy 0.10** (2023-03-06) via PR #6874 (Nolan Darilek). Centerpiece component: `AccessibilityNode` — the megacomponent that issue #17644 flagged as BSN-incompatible. **Buiy explicitly does not layer over `bevy_a11y`; it replaces it** on any window where Buiy is present (foundation architecture.md § 2.6). See [critiques.md](critiques.md) for the design-debt argument.

→ *Future folder:* `docs/prior-art/bevy-a11y/`.

### bevy_picking — hit-testing primitive

Bevy's pointer / picking system. UI-, sprite-, and mesh-picking are all backends to a common picking abstraction. bevy_ui registers a picking backend (when `bevy_picking` feature is on). **Buiy registers its own picking backend** in parallel to bevy_ui's (foundation architecture.md § 2.9).

→ *Future folder:* `docs/prior-art/bevy-picking/`.

### bevy_input_focus — focus model

Focus management primitive. Introduced in **Bevy 0.16** (April 2025) — the `InputFocus` resource replaced the old `Focus` resource and added pluggable nav strategies (`Tab`, 2D-spatial). bevy_ui consumes it. **Buiy owns its own focus model** (focus rings, traps, restoration, inert subtrees, roving tabindex, spatial gamepad nav — foundation architecture.md § 2.3).

### bevy_text — cosmic-text wrapper

Wrapper around cosmic-text for shaping, layout, and atlas management. Decoupled from bevy_ui — also used by 2D `Text2d`. Provides `TextLayoutInfo`, font handles, font registration. **Buiy uses cosmic-text directly** rather than going through `bevy_text` (foundation architecture.md § 2.3).

## Third-party UI stacks (alternatives & complements)

These are the projects that sit at the same architectural layer as bevy_ui — they are either *replacements* or *layers* on it. They are the closest analog of what Buiy is: a parallel UI stack.

### bevy_lunex — parallel transform-based UI

A retained-mode UI engine built on Bevy ECS, using Bevy's regular `Transform` hierarchy for positioning (rather than maintaining a separate UI-only transform). Supports both 2D and 3D UI elements via Bevy's `Transform`. **This is the design choice that contrasts with bevy_ui** (where UI nodes have `Transform` but consumers aren't allowed to touch them) and with Buiy (which uses Buiy nodes but adds a 3D-anchored UI subsystem). Marked "experimental & divergent" in alice's "Vision for Bevy UI."

→ *Future folder:* `docs/prior-art/bevy-lunex/`.

### sickle_ui — extends bevy_ui with widget abstractions

Extends bevy_ui rather than replacing it. Provides a **builder interface** for constructing widgets, common-widget library, and data-driven *skins*. Sits "on top of" bevy_ui and bevy_ui's Taffy layout.

→ *Future folder:* `docs/prior-art/sickle-ui/`.

### woodpecker_ui — reactive UI with Vello rendering

Successor to kayak_ui from @StarArawn. **Reactive** framework, **ECS-first** design, uses **Vello** for rendering (not Bevy's standard render-graph node). Built-in widgets. A *replacement* for bevy_ui — its own paradigm.

→ *Future folder:* `docs/prior-art/woodpecker-ui/`.

### kayak_ui — archived

The predecessor to woodpecker_ui, also by @StarArawn. Declarative UI with a custom proc macro and CSS-like style system. Last release v0.5.0 was **February 2024**; the repo is **not formally archived on GitHub** (no `archived` flag) but is described publicly as superseded by woodpecker_ui. README acknowledges "very early stages of development" and "important features are missing." Effectively abandoned in favor of woodpecker_ui.

→ *Future folder:* `docs/prior-art/kayak-ui/` (mostly for the lessons-learned).

### belly — declarative UI

ELM-flavored declarative UI library for Bevy. Active community use; less prominent than the above. Worth a folder for its declarative-API design choices.

→ *Future folder:* `docs/prior-art/belly/`.

### bevy_egui — immediate-mode wrapper

Wraps `egui` (the popular immediate-mode Rust GUI library) as a Bevy plugin. Different paradigm entirely — immediate-mode rather than retained ECS. Used heavily for dev tooling (debug overlays, world inspectors). Coexists peacefully with bevy_ui.

→ *Future folder:* `docs/prior-art/bevy-egui/`.

### bevy_flair — CSS-style stylesheets on bevy_ui

Brings CSS-like styling to bevy_ui — define appearance and layout using familiar CSS syntax, hot-reload `.css` files. **This is the precedent the Buiy foundation README § 5 cites** when it lists "CSS-flavored stylesheet" as an open question. Sits on top of bevy_ui rather than replacing it.

→ *Future folder:* `docs/prior-art/bevy-flair/`.

### bevy_cosmic_edit — text editing on cosmic-text

A standalone crate that wraps cosmic-text for **rich text editing** on top of bevy_ui — handles IME composition, cursor, selection, undo/redo. Buiy's `buiy_text` will provide its own text-editing surface (foundation architecture.md § 2.3); bevy_cosmic_edit is the precedent.

→ *Future folder:* `docs/prior-art/bevy-cosmic-edit/`.

### Other notable third-parties

- **quill** — experimental reactive UI; "experimental & divergent" per Vision document.
- **polako** — early-stage declarative UI.
- **iyes_ui_navigation** — focus-navigation primitive that pre-dated `bevy_input_focus`.

## Game studios using bevy_ui

The bar for "shipping" Bevy commercial titles is currently low. **Tiny Glade** (Pounce Light, 2024 Steam release) is the flagship — but it uses Bevy ECS only, *not* bevy_ui; the developers wrote their own renderer because Bevy's didn't meet their visual bar at the time. As of mid-2026, the Bevy Foundation cites Tiny Glade plus "a growing long tail of smaller itch.io and Steam releases" as the production footprint. I could not verify "Foresight Spar" or "Roboquest" as bevy_ui consumers via direct sources; the orchestrator's pre-amble flagged these as needs-verification and they remain unverified here.

**Net:** bevy_ui has not yet been used at scale in a flagship commercial title. The widely-cited Bevy-shipped games either bypass bevy_ui or use Bevy primarily for ECS. This is one of the strongest arguments for the existence of the parallel-UI-stack ecosystem (bevy_lunex, woodpecker_ui, Buiy) — bevy_ui itself has not been forced through full commercial-game UX requirements.

## Sources

- bevy_feathers crate — `https://crates.io/crates/bevy_feathers`.
- PR #19730 bevy_feathers — `https://github.com/bevyengine/bevy/pull/19730`.
- bevy_ui_widgets docs — `https://docs.rs/bevy/latest/bevy/ui_widgets/`.
- Bevy 0.17 release notes — `https://bevy.org/news/bevy-0-17/`.
- Discussion #16900 Standard Headless Widgets — `https://github.com/bevyengine/bevy/discussions/16900`.
- bevy_lunex — `https://github.com/bytestring-net/bevy_lunex`, `https://crates.io/crates/bevy_lunex`.
- sickle_ui — `https://github.com/UmbraLuminosa/sickle_ui`.
- woodpecker_ui — `https://github.com/StarArawn/woodpecker_ui`.
- kayak_ui — `https://github.com/StarArawn/kayak_ui`.
- bevy_egui — `https://github.com/vladbat00/bevy_egui`.
- bevy_flair — `https://github.com/eckz/bevy_flair`.
- bevy_cosmic_edit — `https://docs.rs/bevy_cosmic_edit/latest/bevy_cosmic_edit/`.
- "A Vision for Bevy UI" — `https://hackmd.io/@bevy/HkjcMkJFC`.
- Tiny Glade interview — `https://80.lv/articles/exclusive-tiny-glade-developers-discuss-bevy-proceduralism-publishers-cozy-games`.
- Bevy Engine Twitter on Tiny Glade — `https://x.com/BevyEngine/status/1838302608370602111`.
- "How do Nice UI in Bevy?!?" (deadmoney.gg) — `https://deadmoney.gg/news/articles/how-do-nice-ui-in-bevy`.
