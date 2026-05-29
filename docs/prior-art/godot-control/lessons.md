**Date:** 2026-05-22
**Status:** active
**Subject:** Godot Control — lessons for Buiy: which design choices Godot's experience validates, which to avoid, which primitives to borrow

# Lessons for Buiy

This is the consult-this-when-designing decision file. Other files in this corpus are evidence; this file is the synthesis. Three sections — **Validates** (Buiy choices Godot's experience confirms), **Avoid** (pitfalls to mitigate), **Borrow** (primitives worth studying).

Godot Control is the **most directly comparable open-source game-engine UI** to Buiy: same MIT-permissive license posture, same general "UI as part of a scene-tree-shaped game engine" framing, and (uniquely) **source we can read end-to-end**. Twelve years of production shipping (1.0 in January 2014 through 4.6 in 2026) is the longest-running comparable corpus. The architecture's stability across 12 years is itself a strength worth understanding.

## Top-of-file finding: open-source MIT game-engine UI ships at scale

Godot is the **existence proof** that an MIT-licensed, open-source-end-to-end, game-engine-shaped UI library can ship indie commercial games at million-copy scale. **Brotato** (Blobfish, 2022 — ~1M+ Steam copies), **Buckshot Roulette** (Mike Klubnika, 2024 — ~1M+ copies), **Cassette Beasts** (Bytten Studio, 2023 — ~250k+ copies), **Dome Keeper** (Bippinbits, 2022 — ~200k+ copies) are the empirical data.

The Godot Foundation's twelve-year run validates that:

- **MIT permissive scales commercially** — see [`distribution-and-governance.md`](distribution-and-governance.md). Buiy's MIT-or-Apache permissive posture is in the same family.
- **Foundation governance works** — Stichting form, donations + partnerships + grants, board-led. Buiy doesn't need this today (we're a Bevy plugin under Bevy's foundation) but the model is proven.
- **Engine-and-editor-share-one-UI dogfood pays off** — bugs are felt by maintainers immediately. Buiy's `buiy_devtools` should follow the same pattern.

The lessons that follow are not "Godot got things wrong"; Godot is excellent at its own goals. They are "Godot's choices diverge from Buiy's in specific named places, and Buiy should learn from the divergence."

## Validates

These Buiy design choices are confirmed by Godot's production experience:

- **MIT-permissive licensing for a UI library.** Twelve years of MIT did not block any commercial use (W4 Games, Meta, Microsoft engagements all happened under MIT). Foundation [`README.md § 1`](../../specs/2026-05-07-buiy-foundation/README.md) commits to permissive licensing; Godot is the long-running validation.
- **Owning the renderer.** Godot owns its renderer end-to-end (Vulkan + OpenGL + D3D12 backends, no wgpu) and ships across desktop / mobile / web / consoles. Foundation [`architecture.md § 2.3`](../../specs/2026-05-07-buiy-foundation/architecture.md) commits to owning the render pipeline (atop Bevy's wgpu); Godot is the existence proof that owning rendering is shippable at this scale.
- **Theme as a resource pattern.** Godot's Theme resource (typed map keyed by `(type, item)`, hot-reloadable, per-Control inheritance, local override) is the longest-running production precedent for asset-driven UI theming. Foundation [`architecture.md § 2.5`](../../specs/2026-05-07-buiy-foundation/architecture.md) commits to token assets with hot-reload; the *shape* matches Godot's, with Buiy adding semantic-token naming + OS-pref binding + contrast linting.
- **Container subtree-overrides-its-children layout pattern.** Godot's Container subclasses overwrite child anchors + offsets; the child's authored values are silently ignored under Container parents. The pattern is clean and ergonomic. Buiy's Taffy-driven layout uses the same conceptual pattern (parent layout drives child resolved geometry), just with a generic solver instead of per-Container C++ classes.
- **AccessKit as the cross-platform a11y substrate.** Both Bevy and Godot (4.5+) commit to AccessKit. Two independent game engines arriving at the same producer interface is a strong signal AccessKit is the right substrate. Foundation [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md) commits to AccessKit-first.
- **Engine eats its own dog food via the editor.** Godot's editor is written in Godot Controls. The maintainers feel framework bugs immediately. Foundation [`architecture.md § 2.3`](../../specs/2026-05-07-buiy-foundation/architecture.md) names `buiy_devtools` as a Buiy-implementation crate; sticking to this principle (no separate UI stack for devtools) is the dogfood validation.
- **Reactivity through signals + change detection, not React-style hooks.** Godot uses Object-level signals + manual property setters; Bevy uses observers + change detection. Both are non-React; both ship. Foundation §2.7 commits to "observers + change detection only in v1"; Godot validates that this level of reactivity is sufficient for serious UI.
- **Decoupled Font / FontSize.** Godot 4.0 split Font (typeface resource) from FontSize (theme item integer); Buiy's foundation [`text.md`](../../specs/2026-05-07-buiy-foundation/text.md) commits to the same decomposition (typeface assets + size as a style property).
- **Per-window AccessKit adapter ownership.** Buiy's foundation §2.6 commits to per-window adapter ownership; Godot 4.5 follows the same pattern (one adapter per Godot Window). Two engines agreeing on the per-window key is independent validation.

## Avoid

Pitfalls drawn from Godot's experience, with Buiy's mitigation.

| Pitfall | Source in this corpus | Buiy mitigation |
|---|---|---|
| **Anchor + margin layout as the primary positioning model.** Non-obvious for CSS / web developers; container-overwrites-anchors gotcha; no flexbox / grid mental model; no logical properties; rigid one-Container-per-algorithm. | [`layout-anchors-margins.md`](layout-anchors-margins.md), [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "Layout: anchor + offset is non-intuitive vs CSS" | Commit to CSS box model + Flex + Grid + Block via Taffy. Foundation [`visuals.md § 3.2`](../../specs/2026-05-07-buiy-foundation/visuals.md). Logical properties (`inline-size`, `padding-inline-*`) at F-tier. Optional "screen-anchor" affordance for game HUDs is fine; the *default* model is web-shaped. |
| **Accessibility as afterthought (11-year gap).** Godot 1.0 → 4.4 shipped without screen-reader support, ARIA-equivalent semantics, or AT-tree producer integration. AccessKit landed in 4.5 (September 2025) as experimental. The retrofit cost is real. | [`accessibility.md`](accessibility.md), [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "Accessibility: added 11 years late, still 'experimental'" | AccessKit-first from v1. Foundation [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md): WCAG 2.2 AA floor, decomposed `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations` components, APG keyboard contracts per widget, ACCNAME 1.2 in `buiy_core`, drag-and-drop WCAG 2.5.7 keyboard alternative contract enforced. |
| **BiDi added 9 years late (4.0).** Godot 1.0 → 3.x had no BiDi, no complex graphemes, no font fallback, no IME. The TextServer rewrite in 4.0 fixed it but took multi-year coordinated effort. | [`text-and-input.md`](text-and-input.md) § "What landed in Godot 4.0", [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "Text: BiDi + complex scripts added 9 years late" | BiDi + complex graphemes + multi-level font fallback + IME at v1 via cosmic-text. Foundation [`text.md`](../../specs/2026-05-07-buiy-foundation/text.md). Don't repeat the 9-year delay. |
| **RichTextLabel + BBCode as the rich-text model.** BBCode is divergent from web norms (HTML / Markdown); RichTextLabel is display-only (no editor); no semantic elements; no CSS-style stylesheet; no content security; a11y of `[url]` and `[table]` unclear. | [`text-and-input.md`](text-and-input.md) § "RichTextLabel and BBCode", [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "RichTextLabel: BBCode is divergent" | Don't pick BBCode. When Buiy ships a rich-text widget (per `buiy-text-editing-design`), use HTML-or-Markdown-shaped markup so a11y bridges recognize structure. Rich-text **editing** is a v1 commitment if the foundation deems it core. |
| **Visual properties stuffed onto the base Control class.** Background color, border, font, theme overrides, and (in 4.5) accessibility properties all live on `Control`. This is the same megacomponent shape Bevy's `AccessibilityNode` had (issue [bevy/#17644](https://github.com/bevyengine/bevy/issues/17644)) — BSN-hostile. | [`architecture.md`](architecture.md) § "Control class", [`accessibility.md`](accessibility.md) | Decomposed small public-fielded components from day one. Foundation [`architecture.md § 2.4`](../../specs/2026-05-07-buiy-foundation/architecture.md). `BackgroundColor`, `BorderColor`, `Outline`, `BoxShadow`, `A11yRole`, `A11yLabel`, `A11yDescription`, `A11yStates`, `A11yRelations` are separate. |
| **Per-Container-as-a-C++-class layout.** Each algorithm (HBox, VBox, Grid, Flow, Margin, Center, AspectRatio) is its own subclass with its own `_notification(SORT_CHILDREN)`. Adding a new layout means new C++ class; third parties cannot extend the layout solver. | [`layout-anchors-margins.md`](layout-anchors-margins.md) § "Each container is a C++ class with its own algorithm" | One Style builder + Taffy. New layout features (subgrid, container queries, anchor positioning) extend the existing layout pass, not new component types. Foundation [`visuals.md § 3.2`](../../specs/2026-05-07-buiy-foundation/visuals.md). |
| **Per-Control-type theme item keys.** Godot's `("font_color", "Button")` keys produce N×M item explosions across Control types × color slots. Type variations help but don't solve the underlying matrix. | [`theme-and-styling.md`](theme-and-styling.md) § "What Theme does *not* do" | Semantic tokens (`color.surface.primary`, `space.4`, `radius.md`). Foundation [`architecture.md § 2.5`](../../specs/2026-05-07-buiy-foundation/architecture.md). Widgets consume tokens; tokens collapse the matrix. |
| **No OS-preference binding to theme variants.** Godot reads dark/light but does not auto-bind `prefers-reduced-motion`, `prefers-contrast`, `forced-colors`, `prefers-reduced-transparency`. | [`theme-and-styling.md`](theme-and-styling.md) § "What Theme does *not* do" | `UserPreferences` resource bound to theme variants automatically. Foundation [`architecture.md § 2.5`](../../specs/2026-05-07-buiy-foundation/architecture.md). |
| **No contrast linter.** Default theme is "reasonable" but no enforced WCAG 2.2 AA gate. Custom themes can ship at any contrast ratio. | [`theme-and-styling.md`](theme-and-styling.md) § "What Theme does *not* do" | Default theme passes WCAG 2.2 AA by construction. Contrast linter validates custom themes at load + CI. Foundation [`architecture.md § 2.5`](../../specs/2026-05-07-buiy-foundation/architecture.md), [`accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md). |
| **No virtualization in ItemList / Tree.** Render all items eagerly; large lists (10,000+ rows) stutter; user-built virtualization is necessary. | [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "Performance at scale" | Virtualization is a first-class widget concern in [`buiy-widget-catalog-design`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md). Borrow from [GPUI's `UniformList` / `List`](../gpui/lessons.md) for the implementation shape. |
| **Three-way scripting fragmentation (GDScript / C# / GDExtension).** Each has different ergonomics; library / addon authors choose one and lose two. | [`distribution-and-governance.md`](distribution-and-governance.md), [`critiques-and-open-problems.md`](critiques-and-open-problems.md) § "Three-way scripting fragmentation" | Buiy is Rust-only. Fragmentation removed. Plugins consume Buiy via Bevy's plugin trait + crates.io; no parallel script-language story. |
| **GDExtension ABI churn.** Rust plugins (via `gdext`) recompile per Godot minor. C++ source API drifts within 4.x. | [`distribution-and-governance.md`](distribution-and-governance.md) | Bevy minor releases are migration events for Buiy already (foundation §2.9); same model. Rust crates.io stable API per Bevy minor, no parallel ABI. |
| **Lightweight RFC process.** Design state across `godot-proposals` + Discord + recorded GodotCon talks. Lossy. | [`distribution-and-governance.md`](distribution-and-governance.md) § "Contributor base" | `docs/specs/` + `docs/plans/` + `docs/reports/` + `docs/prior-art/` discipline. Per CLAUDE.md ([`/home/user/buiy/CLAUDE.md`](../../../CLAUDE.md)). |

## Borrow

Concrete primitives worth studying and adapting:

1. **Theme resource shape: typed map keyed by `(type, item) → value` with `(local_override, control_theme, ancestor_themes, project_default, engine_fallback)` lookup chain.** Buiy commits to semantic tokens (`color.surface.primary` style) but the *storage* and *lookup chain* shape can mirror Godot's. See [`theme-and-styling.md`](theme-and-styling.md) § "Theme lookup chain." The lookup chain is well-defined, well-tested in production, and ergonomic.

2. **Type variations for theme variants.** Godot's "FlatButton is a variation of Button with overrides" pattern is clean and avoids the CSS-class-selector complexity. Buiy's APG widget catalog can use the same primitive for variant names (e.g., `Button.primary`, `Button.secondary`) without inventing a separate selector grammar. See [`theme-and-styling.md`](theme-and-styling.md) § "Type variations."

3. **`Range`-abstract-base for "anything with min / max / value."** ProgressBar, HSlider, VSlider, SpinBox, ScrollBar all inherit from Range, sharing the min / max / value / step / page / allow_greater / allow_lesser API. Buiy's widget catalog should consider a similar shared abstract for min/max/value-bearing widgets. See [`control-hierarchy.md`](control-hierarchy.md) § "Progress / range / sliders."

4. **`size_flags_horizontal/vertical` enum + `stretch_ratio` as the child→parent layout contract.** Cleaner than raw CSS `flex-grow` / `flex-shrink` / `flex-basis` for game-developer ergonomics. Buiy can layer this as convenience values on top of the Taffy primitives. See [`layout-anchors-margins.md`](layout-anchors-margins.md) § "Containers override anchors."

5. **`CodeEdit` feature set as the "code-editor-grade text widget" reference.** Syntax-highlighter resource interface, code completion popups, line folding, multi-caret, gutters, bookmarks, breakpoints. Buiy doesn't ship a code editor at v1, but `buiy-text-editing-design`'s "complete" reference target is CodeEdit's surface. See [`control-hierarchy.md`](control-hierarchy.md) § "Text editing."

6. **Container naming conventions: HBoxContainer / VBoxContainer / GridContainer / FlowContainer / MarginContainer / CenterContainer / AspectRatioContainer / ScrollContainer / PanelContainer / TabContainer / SplitContainer.** Buiy uses one Style builder, not many container classes, but the *names* for layout patterns (Box / Grid / Flow / Margin / Center / AspectRatio / Scroll / Panel / Tab / Split) are clear and battle-tested. Borrow the labels where the underlying mechanism differs.

7. **`SubViewportContainer` for render-to-texture UI surfaces.** Embeds a sub-viewport (rasterized scene) into a Control rect. Buiy's foundation [§2.3 "render-to-texture surfaces"](../../specs/2026-05-07-buiy-foundation/architecture.md) commits to this pattern; Godot's API is the precedent for the exposed-to-game-author shape.

8. **The `_can_drop_data` / `_drop_data` callback pair on every Control.** Drag-and-drop is opt-in per-Control with predicate-style "can-this-accept" + handler "what-to-do" methods. Clean, Rust-translatable. Buiy's `buiy-input-events-design` for drag-and-drop should consider the same shape (with the additional WCAG 2.5.7 keyboard-alternative contract Godot lacks).

9. **`accessibility_*` property naming on Control (Godot 4.5+).** `accessibility_name`, `accessibility_description`, `accessibility_live`. The naming is clear and consistent. Buiy's components (`A11yLabel`, `A11yDescription`, `A11yLive`) can align with the same vocabulary even where the component decomposition differs.

10. **The MIT-permissive open-source posture.** Twelve years of MIT shipping at indie commercial scale validates the choice. Buiy commits to dual MIT-or-Apache; the *spirit* of "no friction for any commercial use" matches Godot's posture. See [`distribution-and-governance.md`](distribution-and-governance.md).

11. **Hot-reload of Theme assets.** Godot's editor reloads `.tres` themes on disk change; live UI updates immediately. Buiy commits to hot-reload of theme assets — Godot is the precedent that this delivers real developer ergonomics.

12. **The engine-and-editor-share-one-UI dogfood principle.** Godot's editor is written in Godot Controls. Buiy's `buiy_devtools` should be written in Buiy. If Buiy's own devtools are not AT-accessible and WCAG-compliant, the framework's a11y claim is hollow.

## How to use this file

When designing a Buiy feature:

1. **Find the row in `Avoid`** that names a pitfall close to your design. Read the linked file for the original incident.
2. **Find the entry in `Borrow`** that names a primitive close to what you're designing. Read the linked file to understand Godot's shape, then adapt for Buiy's component model (decomposed, public-fielded, observable, reflection-registered).
3. **Promote any decision into a Buiy spec** under `docs/specs/` — this file captures what we learn from Godot, not Buiy's own decisions.

## Cross-corpus reading

- [`/home/user/buiy/docs/prior-art/bevy-ui/lessons.md`](../bevy-ui/lessons.md) — the sister "game-engine UI" lessons file Buiy is parallel to. Read alongside this file when designing render pipeline, component decomposition, layout integration, or focus model.
- [`/home/user/buiy/docs/prior-art/accesskit/`](../accesskit/) — the a11y bridge both Bevy and Godot adopt. Two-engine validation of the substrate.
- [`/home/user/buiy/docs/prior-art/taffy/`](../taffy/) — the layout engine Buiy commits to. Read alongside [`layout-anchors-margins.md`](layout-anchors-margins.md) for the contrast.
- [`/home/user/buiy/docs/prior-art/cosmic-text/`](../cosmic-text/) — the text shaper Buiy commits to. Read alongside [`text-and-input.md`](text-and-input.md).

## Sources

- This folder's evidence files: [`README.md`](README.md), [`architecture.md`](architecture.md), [`control-hierarchy.md`](control-hierarchy.md), [`theme-and-styling.md`](theme-and-styling.md), [`layout-anchors-margins.md`](layout-anchors-margins.md), [`text-and-input.md`](text-and-input.md), [`accessibility.md`](accessibility.md), [`history.md`](history.md), [`distribution-and-governance.md`](distribution-and-governance.md), [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md), [`critiques-and-open-problems.md`](critiques-and-open-problems.md), [`glossary.md`](glossary.md)
- Buiy foundation README — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Buiy foundation accessibility — [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Buiy foundation visuals — [`../../specs/2026-05-07-buiy-foundation/visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md)
- bevy-ui sibling lessons — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
- Bevy issue #17644 (megacomponent canonical case) — https://github.com/bevyengine/bevy/issues/17644
