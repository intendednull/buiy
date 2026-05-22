**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui — what's structurally missing or unfinished as of 0.18.1 / 0.19-rc.1

# Open problems

What bevy_ui does not yet do, or does only at a stub level, as of 2026-05-22 (Bevy 0.18.1 stable, 0.19.0-rc.1 in pre-release). Each entry is what a Buiy designer or downstream consumer would hit if they tried to use bevy_ui for the matching scenario. Cross-link to [critiques.md](critiques.md) for the criticism layer and to [history.md](history.md) for the timeline.

## Renderer feature gaps

Per [critiques.md § renderer feature gaps](critiques.md#the-renderer-feature-gaps) — the master list. Restating for skimmability:

- **Non-rectangular clipping.** Only axis-aligned rects. Path / mask-image / clip-path shapes are not supported. Issue #9381 open since 2023-08.
- **Rounded-corner clipping bug.** Issue #13093 open since 2024-04 — rounding shader runs after rect clip.
- **Transform-aware clipping.** Scaled UI nodes clip incorrectly; rotated UI nodes do not clip at all.
- **`backdrop-filter`.** No support; no tracking issue found.
- **`mix-blend-mode` and CSS `isolation`.** No support; no tracking issue found.
- **True top-layer compositing.** No analog to CSS `:popover` / `<dialog showModal>` top-layer. The `Popover` widget added in 0.18 does floating-UI-style positioning but does not render to a true top layer above all stacking contexts.

Each of these is a hard *renderer-level* limit. Buiy's response: implement them in Buiy's own render passes (foundation architecture.md § 2.3). Cannot be added by a layered crate sitting on top of bevy_ui.

## Accessibility tree completeness

- **`bevy_a11y` is post-#17644 but still maturing.** PR #24308 broke up `AccessNode` into more BSN-friendly pieces, but the full decomposed-component set Buiy ships (foundation architecture.md § 2.6 — `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations`) is *not* what bevy_a11y exposes today. Buiy will replace, not layer.
- **Live regions** (polite / assertive). Not built into bevy_a11y as of 0.18; Buiy provides a global announcer resource (foundation architecture.md § 2.3).
- **ACCNAME 1.2 name computation.** Not in bevy_ui; Buiy implements it in `buiy_core` (architecture.md § 2.6).
- **AccessKit web adapter.** Not yet shipped upstream in AccessKit (per its own roadmap), so bevy_ui (and Buiy) cannot offer first-class a11y on WASM. iOS adapter is in-progress upstream. Foundation README § 5 flags this as an open question.

## BSN ergonomics maturity (post-#17644)

- **BSN itself isn't merged.** PR #20158 is draft / closed-unmerged as of 2026-05-22. The orchestrator pre-amble's claim that BSN "landed in 0.18" was wrong (see [history.md](history.md) for the correction).
- **The decomposition lesson generalizes beyond bevy_a11y.** Every existing megacomponent in the Bevy workspace will need to be decomposed before BSN can patch its properties. As of 0.18 this audit is incomplete.
- **`Construct` trait (cited in #14437) is not merged.** This is the proposed mechanism for components that need `World` state (asset handles, OS-pref resources) at construction time, since Required Components currently demand `Default`.

## Text editing maturity

- **IME composition.** bevy_ui has *partial* IME support via cosmic-text's preedit buffer; full IME composition (pre-edit overlay, candidate window positioning, commit semantics) is incomplete. Issue tracker has multiple open IME-related bugs.
- **Multi-line text input.** No first-class multi-line text editor widget in `bevy_ui_widgets`; bevy_cosmic_edit (third-party) is the de-facto solution.
- **BiDi caret + selection.** cosmic-text supports BiDi visually; caret movement across BiDi runs in bevy_ui is buggy / incomplete.
- **Undo / redo.** Not built-in; consumers roll their own per text input.

Buiy's `buiy_text` crate (foundation architecture.md § 2.8) owns this in full.

## Spatial / gamepad navigation

- **`AutoDirectionalNavigation`** landed in 0.18, providing automatic directional nav for UI elements via arrow keys / gamepad. This is recent — for most of bevy_ui's history, gamepad nav was rolled by hand per project.
- **Trap / restoration / roving-tabindex / `aria-activedescendant` / sequential-focus-navigation-starting-point.** These APG / WAI-ARIA focus concepts are not built into bevy_ui as of 0.18. Buiy bakes them into its focus model (foundation architecture.md § 2.3).

## Animation / transition primitives

- **`TryStableInterpolate`** in 0.18 enables interpolation of `Val` and colors that can fail. This is the substrate for animation, not animation itself.
- **No CSS-style `transition` / `keyframes` / spring primitives** built into bevy_ui. Bevy has a generic `bevy_animation` crate, but it is curve-based and not designed for UI transitions.
- **Layout transitions.** Not supported. Animating a layout-changing property (e.g., `flex-direction`) does not produce smooth interpolation.

Buiy plans a dedicated `buiy_animation` crate (foundation architecture.md § 2.8) with reduced-motion gating.

## Theming / token system

- **No built-in design-token primitive.** bevy_ui does not ship a semantic-token concept (`color.surface.primary`, `space.4`, etc.). bevy_feathers ships *its own* opinionated theme tightly coupled to the editor look-and-feel.
- **No OS-preference plumbing.** `prefers-color-scheme`, `prefers-contrast`, `forced-colors`, `prefers-reduced-motion`, `prefers-reduced-transparency` are not surfaced. Buiy provides a `UserPreferences` resource (foundation architecture.md § 2.5).
- **No contrast linter.** WCAG contrast ratios are not checked anywhere in the stack.
- **No hot-reloadable theme assets.** bevy_feathers themes are code-defined.

## Hot-reload of components

- **`.bsn` asset reload is part of the BSN proposal, but BSN is unmerged.** Hot-reload of components is therefore not currently a bevy_ui feature.
- Foundation README § 5 lists "hot-reload of components (not just themes)" as an open question for Buiy.

## 3D-anchored UI

- **bevy_ui does not provide first-class 3D-anchored UI.** UI nodes have `Transform` but are clamped to a screen-space camera. Worldspace-anchored UI (health bars over characters, name tags) is a roll-your-own exercise in bevy_ui — usually done by computing screen positions per frame.
- bevy_lunex's transform-based approach (see [comparisons.md](comparisons.md)) is the in-ecosystem answer; Buiy's `buiy_3d` crate (architecture.md § 2.3, 2.8) is Buiy's.

## Multi-window UI / per-window theming

- **Multi-window is supported by Bevy** (multiple windows can have their own cameras / render targets).
- **Per-window theming** is not a built-in concept. Each window must wire its own UI hierarchy; theme overrides per window are not factored.

## Scroll containers

- **`Overflow::Scroll`** landed in Bevy 0.16 (issue #8074 tracked it; PR #20093 polished it in 0.17). Scrollbar appearance is configurable via the node's `scrollbar_width` field.
- **`scroll-snap` (CSS Scroll Snap)** is not implemented. Taffy itself does not expose scroll-snap as of its current roadmap — see below.
- **`overflow-anchor`** is not implemented.

Buiy's overflow + scrolling is specified at `docs/specs/2026-05-08-buiy-layout-design/` and the foundation layout plans series (`docs/plans/2026-05-08-buiy-layout-overflow-and-scrolling.md`).

## Anchor positioning

- **CSS anchor positioning** (`anchor-name`, `position-anchor`, `position-try`, `anchor()`) is **listed in Taffy's roadmap (issue #345)** but is not implemented in Taffy as of the 0.6 series. bevy_ui therefore does not expose it.
- The `Popover` widget added in 0.18 implements floating-UI-style placement (a similar concept) but as widget-internal logic, not as a generic layout primitive.

Buiy plans to *extend Taffy itself* with anchor positioning rather than wait for upstream (foundation architecture.md § 2.3).

## Container queries

- **Container queries** are in Taffy's roadmap (issue #345) but not implemented as of Taffy 0.6. bevy_ui therefore does not expose them.
- Buiy has *already shipped* a container-queries-on-top-of-Taffy implementation in `docs/plans/2026-05-21-buiy-layout-container-queries.md` (landed). This is one of the concrete validations of the Buiy-extends-Taffy bet.

## Writing modes

- **Vertical writing modes (`writing-mode: vertical-rl`, `vertical-lr`, `sideways-rl`, etc.) are not in Taffy.** bevy_ui therefore does not expose them.
- Buiy has shipped a `WritingMode` component + `LogicalBoxModel` / `LogicalInset` builders in `docs/plans/2026-05-10-buiy-layout-writing-modes.md` (landed), with sideways variants stubbed.

## Subgrid

- **Subgrid is in Taffy's roadmap** but not implemented. bevy_ui therefore exposes flat CSS Grid only (per the Taffy 0.6 upgrade in PR #15844).
- Buiy ships a Subgrid warn-once stub today; full implementation tracks Taffy upstream (`docs/plans/2026-05-09-buiy-layout-grid.md`).

## Color management

- **Linear RGB only.** bevy_ui works in linear RGB; CSS color spaces (Display-P3, Rec2020, OKLab, OKLCH, color()) are not modeled as authoring primitives.
- Buiy's render-pipeline sub-spec is the planned home for gradient + color-space work (foundation README § 4).

## Verifiable behavior

- **No AccessKit-tree snapshot harness in CI.** bevy_ui is not regression-tested for its accessibility output.
- **No visual-regression snapshot harness in CI.** Visual changes are caught by human review.
- **No APG conformance suite.** Widget keyboard contracts are not auto-verified against APG.

This is the verification gap the Buiy foundation spec's "Goal 7 — verifiable" addresses (foundation README § 1.7, verification.md).

## Sources

- Issue #8074 scrollable UI — `https://github.com/bevyengine/bevy/issues/8074`.
- PR #15844 Taffy 0.6 upgrade — `https://github.com/bevyengine/bevy/pull/15844`.
- PR #20093 scrolling polish — `https://github.com/bevyengine/bevy/pull/20093`.
- Taffy roadmap issue #345 — `https://github.com/DioxusLabs/taffy/issues/345`.
- Taffy README — `https://github.com/DioxusLabs/taffy`.
- bevy_ui_anchor third-party — `https://github.com/TotalKrill/bevy_ui_anchor`.
- Bevy 0.16 scroll example — `https://bevy.org/examples/ui-user-interface/scroll/`.
- Bevy 0.18 release notes (AutoDirectionalNavigation, Popover, TryStableInterpolate) — `https://bevy.org/news/bevy-0-18/`.
- AccessKit project (web adapter status) — `https://accesskit.dev`.
- Buiy layout plans series — `../../plans/2026-05-08-buiy-layout-overflow-and-scrolling.md`, `../../plans/2026-05-09-buiy-layout-grid.md`, `../../plans/2026-05-10-buiy-layout-writing-modes.md`, `../../plans/2026-05-21-buiy-layout-container-queries.md`.
