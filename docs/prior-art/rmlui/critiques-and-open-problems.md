**Date:** 2026-05-22
**Status:** active
**Subject:** RmlUi — third-party critiques, structural gaps, open problems

# Critiques and open problems

This file surfaces what RmlUi structurally does not solve as of 6.2 (2026-01-11), drawn from the project's own changelog and docs, the gap analysis vs CSS / HTML / modern web platform, and the open-source-project shape (single maintainer, no foundation, no funded a11y / platform work). It is the honest enumeration that the marketing-style "RmlUi is lightweight and performant" framing does not capture.

## Critiques

### 1. Bus factor 1 (governance)

The single largest structural risk. RmlUi is **fundamentally a single-maintainer project** (`mikke89` / Michael Ragazzon). 8 years of durable stewardship is genuinely impressive, but there is no documented succession plan. If `mikke89` stops working on the project, the path is:

- Existing users continue using current release.
- Community PRs accumulate without merge.
- A new fork emerges with a new name (the libRocket → RmlUi pattern repeats).
- Several years pass before the new fork is production-trustworthy.

The 2014–2018 libRocket dormancy is the precedent. The risk is not theoretical.

**No corporate steward, no foundation, no co-maintainer, no documented governance bylaws, no contributor RFC process.** These are the structural mitigations available; none are in place.

### 2. C++-embedder integration friction

For non-C++ engines (Unity / Unreal Blueprint / Godot GDScript / Rust / Go / Python), every embedder must:

- Write or maintain a binding layer to the C++ API.
- Manage C++ lifetime semantics (`UniquePtr<Element>`, parent-borrow, `Rml::Initialise` / `Rml::Shutdown` lifecycle).
- Translate engine asset I/O through `FileInterface`.
- Bridge engine renderer through `RenderInterface` (which was redesigned in 6.0, requiring all embedders to port).
- Bridge engine input source through `Context::Process*` methods.

The friction is real and tracks the broader pattern of C++ library integration cost. Unity / Unreal plugins exist but are third-party-maintained, lag RmlUi releases, and re-implement binding work for each engine separately. **No first-party bindings to non-C++ languages.**

For Buiy this is moot (Buiy is Rust + Bevy ECS native, no embedder integration cost), but for any Buiy reader considering "could we just use RmlUi from Rust?" — the answer is "yes, via FFI, with all the C++ FFI complexity that entails."

### 3. CSS coverage gaps (technical scope)

Enumerated thoroughly in [`rml-rcss-coverage.md`](rml-rcss-coverage.md). Headline gaps as of 6.2:

- **No CSS Grid.** The single largest feature gap. No subgrid, no masonry. 15 years and a from-scratch layout engine still has not delivered Grid.
- **No container queries.** No `@container`, no `cqw/cqh/cqi/cqb` units.
- **No anchor positioning.** No `anchor-name`, no `position-anchor`, no `anchor()`, no `position-try`.
- **No logical properties.** `inline-size`, `block-size`, `padding-inline-*`, `margin-block-*` absent.
- **No modern color spaces.** `lab()`, `lch()`, `oklab()`, `oklch()`, `color()` profiles, `color-mix()` absent.
- **No `clip-path` (non-rect masks)**, no `backdrop-filter`, no `mix-blend-mode`, no `isolation`, no CSS top layer.
- **No CSS Nesting**, no `:has()`, no `:is()`, no `:where()`, no `:focus-visible`.
- **No scroll-driven animations.** `animation-timeline`, `scroll-timeline`, `view-timeline` absent.
- **No system color keywords** (`Canvas`, `CanvasText`, `LinkText`, etc.) → no forced-colors mode.
- **No OS-preference media queries** (`prefers-color-scheme`, `prefers-reduced-motion`, `prefers-contrast`, etc.).

The pattern: **whatever has been added to CSS in the last decade** (Grid 2017, container queries 2022, anchor positioning 2024, logical properties since 2017, modern color 2022, CSS Nesting 2023) is not in RmlUi. The project's own README acknowledges this: *"We do not aim to be fully compliant with CSS or HTML."*

### 4. Accessibility absence (structural scope)

Documented in [`accessibility.md`](accessibility.md). The most critical critique. After 15+ years of cumulative shipping, RmlUi has no AT bridge, no ARIA, no accessible name computation, no `:focus-visible`, no focus traps, no live regions, no forced-colors / reduced-motion support. Spatial controller navigation is the only accessibility-adjacent feature — and it is a *gameplay* feature, not a WCAG conformance feature.

The structural reason: accessibility requires a tree representation (AccessKit), per-widget APG keyboard contracts, name-computation algorithms, per-platform AT-adapter integration, and verification harness — none of which RmlUi has roadmapped. Adding any of them now would require an architectural commitment of years of work, with no obvious funded contributor to drive it.

### 5. BiDi paragraph algorithm + complex-script shaping

[`text-and-input.md`](text-and-input.md). FreeType-only default; HarfBuzz is a sample; UAX #9 BiDi algorithm is not in the core. RTL + complex scripts (Arabic / Hindi / Thai / Khmer / Burmese) require embedder work. Compare: cosmic-text gives Buiy BiDi + complex scripts out of the box.

### 6. IME platform fragility

[`text-and-input.md`](text-and-input.md). Win32 backend has IME; other platforms have empty defaults. Mobile / macOS / Linux IME is the embedder's problem. The library cannot honestly claim "productivity-app-grade text input" — it can claim "Win32 IME-shaped game text input."

### 7. Performance characterization is sparse

The README claims *"light-weight and performant"* but there are **no published benchmarks** for node counts at productivity-app scale (1000s of elements). The single-pass layout model + docs guidance ("avoid content-based sizing, prefer `flex: <number>` with definite dimensions") suggests there are real performance cliffs that the author knows about; the magnitude is not publicly characterized.

This is the same critique that applies to bevy_ui (see [`../bevy-ui/critiques.md`](../bevy-ui/critiques.md) § "Performance critiques"). Game UI libraries broadly don't publish productivity-app-scale benchmarks; Buiy's verification harness ([`../../specs/2026-05-07-buiy-foundation/verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)) explicitly includes 1000+-node fixtures to avoid this trap.

### 8. The decorator divergence locks RmlUi out of CSS ecosystem tooling

[`rml-rcss-coverage.md`](rml-rcss-coverage.md) § "Decorators." Because RCSS replaces CSS `background-image` / `background-position` / `background-repeat` / `border-image` with the **decorator** primitive, every CSS-ecosystem tool (Stylelint, designer-friendly CSS editors, MDN reference docs, AI-generated CSS) does not apply to RmlUi authoring. Authors learn a custom shape. This is a permanent ecosystem-divergence cost.

### 9. No first-party visual editor / designer tool

The proprietary cousins (NoesisGUI with Microsoft Blend compatibility; Unity UI Builder for UI Toolkit; Unreal UMG editor) ship visual designers. RmlUi authors hand-write RML + RCSS in a text editor. The visual debugger added in libRocket is for **runtime** inspection, not authoring.

### 10. Slow major-release cadence

[`history.md`](history.md). 5.0 (Flexbox) was 3 years after 4.0; 6.0 (effects) was 2 years after 5.0. 6.0 → 6.1: 8 months. 6.1 → 6.2: 9 months. Pace is slowing. Single-maintainer + no foundation funding consistent with this trajectory.

## Open problems (what RmlUi structurally doesn't solve)

Distinct from critiques: these are problems the project would need to formally address to close gaps vs the modern web platform / vs accessibility-first competitors.

### Open problem 1: How does RmlUi adopt AccessKit?

Adopting AccessKit would require:

- A new tree representation parallel to the element tree.
- Stable NodeIds derived from C++ element pointers (or a separate ID space).
- AccessKit producer logic per element type — `<button>`, `<input>`, `<select>`, `<tabset>`, etc.
- ACCNAME 1.2 name-computation walking RML + RCSS.
- Focus model overhaul (`:focus-visible`, traps, restoration, inert).
- Per-platform AccessKit adapter integration (Windows UIA, macOS NSAccessibility, Linux AT-SPI, etc.).
- AccessKit version-pinning policy.
- Test harness for AccessKit tree snapshots.

**No public commitment, no tracking issue, no roadmap entry.** Until this is on the roadmap, RmlUi's accessibility absence is permanent.

### Open problem 2: How does RmlUi add CSS Grid?

Adding Grid to RmlUi's own layout engine would be the largest single layout-engine extension in the project's history. Taffy / Yoga / browsers have all shipped Grid; the engineering scope is well-understood. The project's CSS Grid absence is not because Grid is too hard to implement, it is because the single maintainer has not prioritized it.

A Grid implementation would touch:

- Track sizing algorithm (intrinsic + content + fr).
- Named line + named area resolution.
- Auto-placement algorithm.
- Subgrid pass.

This is ~3–12 months of work for one focused contributor. **No tracking issue, no public design.**

### Open problem 3: Modern CSS effects (`backdrop-filter`, `mix-blend-mode`, top layer)

The 6.0 render-interface redesign added filters + box-shadow + masks but stopped short of `backdrop-filter`, `mix-blend-mode`, `isolation`, and CSS top layer. The render-interface architecture can grow to add these but they are not on the roadmap.

True top layer in particular is required for proper modal `<dialog>` / popover behavior; without it, modals are always layered via `position: absolute` + high `z-index` and can be clipped by scrolling containers — a known bug shape.

### Open problem 4: First-party HarfBuzz integration + BiDi

Moving HarfBuzz from sample to built-in font engine, plus adding a UAX #9 BiDi algorithm to the core, would close the largest text-quality gap. Requires:

- HarfBuzz as a non-optional dependency or a robust opt-in path.
- BiDi algorithm implementation (or bundling a library like `icu4c` / `fribidi`).
- Mark-cluster handling for complex scripts.
- Run shaping pipeline + bidi-resolved-run-segmentation pipeline.

The 6.1 / 6.2 changelog improvements to the HarfBuzz sample are encouraging but stop short of first-party adoption.

### Open problem 5: First-party non-C++ language bindings

Python + Lua bindings were dropped in the RmlUi era. No first-party Rust / Go / C# / JavaScript bindings. The C++ binding surface is awkward for FFI (`UniquePtr<Element>`, parent-borrow, lifecycle management).

If non-C++ adoption is a project goal (unclear), this would require either:

- A more FFI-friendly C API layer (a `cffi`-style header).
- First-party Rust / C# bindings.

Currently the third-party Unity / Unreal plugins fill this gap, but they lag RmlUi releases.

### Open problem 6: Performance characterization at productivity-app scale

The "lightweight + performant" claim should be backed by published benchmarks at 1000s of nodes for the productivity-app use case. Currently absent. Without this, sizing the library for non-game scenarios (desktop apps, productivity tools, complex menus) is guesswork.

### Open problem 7: Hot reload / DevTools beyond runtime inspector

The runtime visual debugger (libRocket era) is meaningful but distant from browser DevTools or Unity Profiler. Hot-reload of RML + RCSS files exists; hot-reload of bound C++ data + decorators is sparse. A modern devtools experience (element tree inspector, computed-style panel, layout-tree visualizer, performance flame graph) would close a productivity gap.

### Open problem 8: Touch + mobile + console as first-class

6.2 added native touch input + inertial scrolling — encouraging — but the broader mobile / console story (gesture vocabulary, virtual-keyboard hints, console TRC compliance for input prompts, controller-button glyph rendering) remains thin compared to Unity / Unreal.

## Implications for Buiy

- **Pick your structural commitments knowingly.** RmlUi's bus factor 1 + no-foundation + no-CLA + permissive-license is one coherent shape. Buiy's Bevy Foundation + multi-maintainer + decomposed-components + AccessKit-first is another. Each shape has costs; the cost of RmlUi's shape is bus-factor risk + slow feature pace + permanent CSS-spec divergence.
- **Substrate borrows beat own-built engines.** RmlUi's own layout engine has not delivered Grid, container queries, anchor positioning, subgrid, masonry, logical properties — every modern CSS-layout feature is a multi-quarter project. Buiy's commitment to Taffy means these arrive when Taffy ships them.
- **`backdrop-filter` / `mix-blend-mode` / top layer must be designed in from day one.** RmlUi's 6.0 render-interface redesign was breaking; the next round (for the effects it still doesn't have) will be breaking again. Buiy's `buiy-render-pipeline-design` sub-spec must commit to the full effects vocabulary upfront.
- **Accessibility absence is recoverable but expensive.** RmlUi's 15-year a11y absence is not a moral judgment; it is a structural consequence of a small project without funded contributors. Buiy commits to AccessKit-first as a foundation-tier requirement specifically because retrofitting is the path RmlUi did not take.
- **Performance characterization must be designed in, not asserted.** Buiy's verification harness ([`../../specs/2026-05-07-buiy-foundation/verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)) commits to productivity-app fixtures + 1000+-node tests in CI. RmlUi's absence of public benches is a cautionary tale for "lightweight and performant" rhetoric.
- **Decorator divergence is the cautionary tale for any future Buiy-CSS-stylesheet sub-spec.** If Buiy ever ships a CSS-flavored stylesheet (foundation [`README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 5 open question), it must commit to standard CSS semantics for `background-image` / `background-position` / `background-repeat` rather than inventing a custom primitive — otherwise it inherits RmlUi's permanent ecosystem-divergence cost.

## Sources

- RmlUi changelog (feature gaps, 6.0 render redesign, 6.2 native touch) — https://github.com/mikke89/RmlUi/blob/master/changelog.md
- RmlUi documentation (no a11y section, RCSS coverage limits) — https://mikke89.github.io/RmlUiDoc/
- RmlUi releases (cadence) — https://github.com/mikke89/RmlUi/releases
- RmlUi README ("We do not aim to be fully compliant with CSS or HTML") — https://github.com/mikke89/RmlUi
- libRocket dormancy precedent — https://github.com/libRocket/libRocket
- Buiy foundation README (web-platform parity, WCAG 2.2 AA) — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Buiy foundation accessibility — [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Buiy foundation verification — [`../../specs/2026-05-07-buiy-foundation/verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)
- bevy_ui critiques (performance characterization) — [`../bevy-ui/critiques.md`](../bevy-ui/critiques.md)
