**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_lunex — unsolved problems, missing capabilities, integration gaps

# Open problems

This file catalogues the gaps in bevy_lunex's current capability set — both the explicitly-flagged issues and the structural absences. Most of these gaps are out-of-scope-by-design rather than bugs; the project's "worldspace game UI" framing leaves much of the application-UI feature surface untouched. The list is curated to surface what Buiy must address that bevy_lunex does not.

## Accessibility integration (AccessKit)

**Status: absent.** bevy_lunex does not depend on `accesskit`, `bevy_a11y`, or any accessibility primitive. No role announcement, no focus-ring primitive, no screen-reader path, no live regions, no keyboard-only navigation guarantees.

There is no open issue tracking this — the project has not committed to addressing it. By contrast, `bevy_ui` integrates AccessKit via `bevy_a11y` (since Bevy 0.10), `bevy_feathers` exposes accessibility props per-widget, and `bevy_ui_widgets` ships headless primitives with AccessKit wiring as a default.

For Buiy, this is the load-bearing inversion. Buiy commits to AccessKit-first with decomposed `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations` from day one per [accessibility.md](../../specs/2026-05-07-buiy-foundation/accessibility.md). bevy_lunex is the prior-art evidence for what "we'll add it later" looks like at 2.7 years in: still nothing.

## Theme system / token primitives

**Status: absent.** No theme tokens, no color palette resource, no font-style tokens, no spacing scale, no dark/light variant primitives. Styling is per-component, manual.

Bevypunk demonstrates a cohesive visual identity, but it is achieved by hand-coded styles across the project, not by a token system. Replicating the Bevypunk look requires copying styling code, not consuming a token bundle.

For Buiy: the token system is a [README goal 6](../../specs/2026-05-07-buiy-foundation/README.md) commitment. bevy_lunex's absence here is informative — without tokens, the path from primitives to a polished look requires substantial per-project investment, and the absence likely contributes to the small showcase community (see [`ecosystem.md`](ecosystem.md)).

## Animation / transition primitives

**Status: minimal.** A single `Text animation` component was added 2025-05 (commit "Polish the text animation"). There is no general transition system, no keyframe animation, no spring physics, no layout-transition primitive, no reduced-motion-gated mode.

Because UI nodes are `Transform`-positioned entities, developers can animate them with Bevy's standard tweening crates (`bevy_tweening`, etc.) — but the integration is ad-hoc, not blessed by bevy_lunex itself.

For Buiy: this is a [visuals.md](../../specs/2026-05-07-buiy-foundation/visuals.md) and [interaction.md](../../specs/2026-05-07-buiy-foundation/interaction.md) area. bevy_lunex's gap is consistent with its primitive-engine framing; Buiy aims to commit to first-party animation primitives.

## Drag-and-drop

**Status: not documented.** No DnD primitive in the API surface, no chapter in the book, no example in the repo as of 2026-05-22. Developers must implement DnD by hand against `bevy_picking` pointer events.

For Buiy: drag-and-drop is a documented [media-and-widgets.md](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md) capability. bevy_lunex's absence sets a baseline — even basic DnD requires significant per-project work in bevy_lunex.

## Text editing (IME, BiDi)

**Status: not addressed.** bevy_lunex uses `cosmic-text` for shaping but does not expose a text-input widget. There is no `TextInput` component, no IME composition handling, no BiDi cursor primitive, no selection model. Implementing a text-input field on bevy_lunex requires building it from scratch.

`cosmic-text` itself supports IME composition and BiDi at the shaping layer, so the foundation is there — but bevy_lunex does not expose it. The book's "Use case guidance" explicitly says *"No pre-built input components."*

For Buiy: [text.md](../../specs/2026-05-07-buiy-foundation/text.md) commits to IME and BiDi from the start. bevy_lunex's absence is a structural baseline.

## WASM / mobile maturity

**Status: WASM opt-in, mobile undocumented.**

- **WASM.** A `wasm` Cargo feature exists since post-0.3. Bevypunk ships a WASM demo on itch.io with documented stutter (*"limited performance & stutter due to running on a single thread"*). Single-threaded execution is a Bevy WASM limitation that bevy_lunex inherits. AccessKit's web adapter is not integrated regardless of platform.
- **Android / iOS.** No platform-specific code in the crate. No mobile examples. No mobile-targeted documentation. Inherits Bevy's mobile maturity (iOS better than Android; Android improving but rough). Touch input through `bevy_picking`'s pointer abstraction works in principle but is not validated.

For Buiy: WASM and mobile parity are explicit Buiy goals. bevy_lunex's WASM-works-but-stutters baseline is honest; planning Buiy's WASM story should target better, and the AccessKit-web-adapter integration gap is the same one Buiy faces.

## Performance benchmarks (no published numbers)

**Status: absent.** The "blazingly fast" marketing claim is unverified. No `benches/` directory in the repo, no `criterion` setup, no `bevy-bencher` participation, no comparison numbers against `bevy_ui`, `bevy_egui`, `sickle_ui`, or `woodpecker_ui`.

The retained-mode architectural argument is plausible (no per-frame layout recomputation for static UI) but unmeasured. See [`critiques.md`](critiques.md) § "The 'blazingly fast' marketing claim."

For Buiy: this is an opportunity. Buiy could ship the first cross-Bevy-UI-kit benchmark suite. Even without it, Buiy's own [verification.md](../../specs/2026-05-07-buiy-foundation/verification.md) commits to benchmark CI; bevy_lunex's absence raises the bar.

## Hot-reload / asset reload

**Status: not implemented; tracked in open issue #11 since 2023-11-06.** The issue title is *"Hot reloading thoughts"* — it is a discussion, not a tracked work item, and has been open for over 2.5 years with no commits referencing it.

Because bevy_lunex UI is built in Rust code (not from `.scn` assets), hot-reload of UI definitions would require either (a) a recompile loop (Bevy's `dynamic_linking` feature, which works but is slow), or (b) an asset-based scene format (which doesn't exist for bevy_lunex). No design doc exists.

For Buiy: BSN-friendly scene-asset hot-reload is a known target. bevy_lunex's 2.5-year-open issue is evidence that "we'll add it later" rarely happens for design-heavy features.

## Multi-window support

**Status: not documented.** bevy_lunex depends on `bevy_window` and `bevy_winit`, so multi-window apps are nominally possible. There is no documented per-window UI pattern, no example, and no book chapter on multi-window UI.

For Buiy: multi-window support is a stated goal. bevy_lunex's gap suggests this is harder than it looks — the per-window-camera + per-window-anchor-coordinates math requires explicit support that bevy_lunex hasn't shipped.

## 1000+-node performance

**Status: unmeasured.** There is no documented stress test of a large UI tree (1000+ nodes). The retained-mode argument suggests static large trees should be fast; what happens when many nodes animate simultaneously is unknown.

The `radsort` dependency suggests z-order sorting is on the hot path; with 1000+ nodes the constant factors matter. No published numbers.

For Buiy: scale-game thinking (1000×, 10000× nodes) per [verification.md](../../specs/2026-05-07-buiy-foundation/verification.md) is a Buiy commitment. bevy_lunex sets no baseline.

## Custom shader integration

**Status: works by construction.** Because bevy_lunex renders through `bevy_sprite`, any Bevy `Material2d` can paint a UI panel. This is actually a **bevy_lunex strength** relative to `bevy_ui` (which historically struggles with custom materials). The 0.4.2 release added a `custom mesh for UI node` example demonstrating this.

For Buiy: this is one of the design wins to adopt — owning the render path enables custom materials in the same way. Buiy plans first-class material integration per architecture specs.

## WCAG 2.2 SC coverage

**Status: 0%.** Without AccessKit, without role/label/state/relation primitives, without focus-ring system, without high-contrast theme variants, and without keyboard-only navigation guarantees, bevy_lunex fails every Level A WCAG SC that requires assistive-technology integration. This is not a partial-failure-with-gaps; it is structural absence.

A bevy_lunex UI **cannot** conform to WCAG 2.2 at any level (A, AA, AAA) without external accessibility tooling. The project is functionally inaccessible to screen-reader users.

For Buiy: per-widget WCAG SC mapping per [accessibility.md](../../specs/2026-05-07-buiy-foundation/accessibility.md) is the right inversion.

## bevy_picking integration: how it actually works

**Status: integrated since 0.3.** This is a **bevy_lunex strength** worth highlighting: the project does not roll its own hit-testing — it integrates with the official `bevy_picking` system.

From the book's Interactivity chapter, bevy_lunex exposes three event types via observers:

- `Pointer<Click>`
- `Pointer<Over>`
- `Pointer<Out>`

The `Pointer<T>` events carry their `bevy_picking` metadata (which mouse button, modifier keys, world position). The hit-test backend is bevy_lunex's own, registered as a picking backend (per the 0.3.0 release notes: *"migrated Lunex picking backend"*).

What's **missing** from the picking story:

- **No documented keyboard navigation.** Picking is pointer-focused; keyboard-only users have no canonical pattern.
- **No documented focus management.** There is no `Focused` resource or component model exposed.
- **No documented gamepad navigation** beyond the `GamepadCursor` marker added in 0.2.2 (which simulates a pointer with a gamepad, not the same as native gamepad navigation).
- **No `Pointer<Down>` / `Pointer<Up>`** documented in the book interactivity chapter — only the click / hover events are surfaced. Implementing drag requires using lower-level `bevy_picking` events directly.

For Buiy: the picking integration design is the prior-art to adopt; the keyboard/focus/gamepad gaps are the ones to close.

## 3D-anchored UI vs 2D UI: ergonomics balance

**Status: imbalanced toward 3D.** The project's distinctive strength — worldspace UI — is well-supported and documented. The project's underlying primitives (anchored positioning) work less well for dense 2D UI, and the book says so:

- *"Not optimized for rapid development iteration."*
- *"No pre-built input components."*
- *"Poor fit for desktop application UIs."*
- *"Lacks flexbox-like layout functionality."*

This imbalance is a design choice, not a bug. It is also the reason `sickle_ui` and `bevy_feathers` (Taffy-based, screen-space-first) coexist with bevy_lunex rather than competing directly — they serve the 2D-UI case bevy_lunex declines.

For Buiy: serving both 2D and 3D UI ergonomically is one of Buiy's distinguishing goals. The lesson from bevy_lunex is that this is hard — the primitive choices that make 3D easy make 2D awkward, and vice versa. Buiy's bet (Taffy + general `Transform` + screen-space-by-default) is technically harder than either of bevy_lunex's or `bevy_ui`'s choices in isolation. Plan the integration carefully.

## Sources

- bevy_lunex open issues — `https://github.com/bytestring-net/bevy-lunex/issues`.
- Issue #10 (DSL thoughts, open since 2023-11-05) — `https://github.com/bytestring-net/bevy-lunex/issues/10`.
- Issue #11 (Hot reloading thoughts, open since 2023-11-06) — `https://github.com/bytestring-net/bevy-lunex/issues/11`.
- Issue #53 (FillPortion/Flex Unit feature request) — `https://github.com/bytestring-net/bevy-lunex/issues/53`.
- Issue #58 (Advanced navigation) — `https://github.com/bytestring-net/bevy-lunex/issues/58`.
- Issue #102 (SystemCursor on Linux) — `https://github.com/bytestring-net/bevy-lunex/issues/102`.
- Bevy Lunex book — `https://bytestring-net.github.io/bevy_lunex/`.
- Interactivity chapter — `https://bytestring-net.github.io/bevy_lunex/chapters/interactivity.html`.
- bevy_lunex Cargo.toml — `https://raw.githubusercontent.com/bytestring-net/bevy-lunex/main/crate/Cargo.toml`.
- 0.3.0 release notes (picking backend migration) — `https://github.com/bytestring-net/bevy_lunex/releases`.
- Bevypunk WASM caveats — `https://idedary.itch.io/bevypunk`.
- AccessKit + Bevy integration article — `https://accesskit.dev/accesskit-integration-makes-bevy-the-first-general-purpose-game-engine-with-built-in-accessibility-support/`.
