**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_feathers — comparison to bevy_ui_widgets, bevy_egui, bevy_lunex, sickle_ui, woodpecker_ui, kayak_ui, Buiy

# Comparisons

`bevy_feathers` sits in the official-blessed-widget-kit slot of the Bevy UI landscape. This file places it row-by-row against its closest neighbors and against Buiy. Each row covers the 2–4-sentence elevator pitch plus the **key design difference** — the load-bearing fact a Buiy spec author needs.

## At a glance

| Kit | Paradigm | Built on | Scope | Status |
|---|---|---|---|---|
| `bevy_feathers` | Opinionated widget set + theme tokens | `bevy_ui` + `bevy_ui_widgets` + `bevy_a11y` + AccessKit | Editor / tooling | Experimental (default-off feature) |
| `bevy_ui_widgets` | Headless primitives (behavior + a11y, no visuals) | `bevy_ui` + `bevy_a11y` + AccessKit | General | Experimental (default-off feature) |
| `bevy_egui` | Immediate-mode wrapper around `egui` | Own render path (not `bevy_ui`) | General / debug | Mature, third-party |
| `bevy_lunex` | Parallel UI, `Transform`-based | Own layout + render path | Game UI | Active, third-party |
| `sickle_ui` | Themed widgets on top of `bevy_ui` | `bevy_ui` | General | Active, third-party |
| `woodpecker_ui` | Custom declarative API | Own runtime | General | Active, third-party |
| `kayak_ui` | Retained widget tree, custom DSL | Own runtime | General | **Archived 2024** |
| **Buiy** | Parallel UI stack, web-platform-parity | Taffy + cosmic-text + AccessKit + bevy_picking + Bevy render graph (direct) | Game + app, comprehensive | Pre-foundation (spec phase) |

## vs `bevy_ui_widgets` (sibling)

`bevy_ui_widgets` is the **headless primitive set** feathers styles. The 0.17 release notes describe the relationship: "It builds on top of Bevy's new general-purpose 'headless' widget set: `bevy_ui_widgets`." Both crates landed in the same Bevy 0.17 release; both are experimental; both depend on `bevy_a11y` + AccessKit. The split is design-intent: `bevy_ui_widgets` ships interaction logic + accessibility wiring + state machines without making visual decisions; feathers adds the OKLCH palette, font styles, layout containers, and the editor-axis tokens on top.

**Key design difference.** "Headless" means no visuals — and that lets a third party build their own opinionated widget kit on `bevy_ui_widgets` without conflicting with feathers. In principle a different in-house design language could replace feathers wholesale by re-styling the same primitives. In practice nobody has done that yet, and the two crates evolve in lockstep.

## vs `bevy_egui` (different paradigm)

`bevy_egui` wraps `egui` (the immediate-mode Rust GUI library) and renders it into a Bevy window. It is the most-used UI kit in the Bevy ecosystem by a wide margin — by far the dominant choice for in-game debug overlays, tool UIs, and developer-facing menus.

**Key design difference.** Immediate-mode vs retained-mode. `bevy_egui` rebuilds the widget tree every frame from imperative calls (`ui.button("Click me")`) and does not integrate with `bevy_ui`'s Taffy layout, `bevy_a11y`'s AccessKit tree, or `bevy_picking`. Feathers (and Buiy) are retained-mode, ECS-native, AccessKit-wired. The paradigms do not blend: feathers and `bevy_egui` do not share visuals, hit-testing, focus, or accessibility.

## vs `bevy_lunex` (parallel stack)

`bevy_lunex` is a third-party UI system that does **not** build on `bevy_ui`. It implements its own layout (anchored, `Transform`-based, with explicit pixel/percent/relative containers) and renders via Bevy's standard 2D sprite pipeline. It targets game UI explicitly — HUDs, menus, animated UI in worldspace — and is the closest paradigm cousin to Buiy's parallel-stack stance.

**Key design difference.** Lunex chooses `Transform`-based positioning (so UI is just a 2D entity tree) where feathers / `bevy_ui` use Taffy + a separate `UiTransform`. Lunex therefore gets 3D-anchored / diegetic UI for free; feathers cannot. Buiy chose Taffy + general `Transform` (no `UiTransform` divergence per [architecture.md § 3.17](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)) — closer to Lunex than to feathers on the worldspace-UI axis, but with full CSS Grid / Flexbox layout via Taffy rather than only anchored positioning.

## vs `sickle_ui` (third-party widget kit on `bevy_ui`)

`sickle_ui` is the closest direct ancestor of feathers in design space — an opinionated, themed widget kit built on top of `bevy_ui`, predating feathers by ~1 year. It ships a broader widget set (tabs, scrollboxes, color pickers, panels, tree views), a token-based theming system, and is third-party / community-maintained.

**Key design difference.** Authority and lifecycle. `sickle_ui` is a community crate with no Bevy Foundation guarantee; feathers is in-tree, with Bevy minor-release migration support. Practically: `sickle_ui` has more widgets today (and a more battle-tested set), but feathers is on a path to becoming the de-facto official option as the Bevy editor consumes it. Studios choosing today face the unstable-but-official vs more-mature-but-third-party tradeoff.

## vs `woodpecker_ui` (custom declarative API)

`woodpecker_ui` is a successor to ideas in `kayak_ui` — a declarative widget tree with its own runtime, not built on `bevy_ui`. It uses an event-driven update model and supports JSX-style composition through Rust macros.

**Key design difference.** A custom runtime vs `bevy_ui`'s ECS-native model. `woodpecker_ui` re-implements layout, rendering, and event handling inside its own subsystem; feathers reuses `bevy_ui`'s. The tradeoff: woodpecker has more freedom to design its own widget abstractions but loses ECS-integration ergonomics; feathers gets ECS integration for free but inherits `bevy_ui`'s constraints.

## vs `kayak_ui` (archived; cautionary tale)

`kayak_ui` was the most ambitious third-party Bevy UI kit through 2022–2023 — a retained widget tree with a custom DSL, hot-reload support, and an active community. It was **archived in 2024** when its primary maintainer stepped back and no successor maintainer emerged.

**Key design difference vs feathers.** Lifecycle assurance. The Bevy-monorepo placement means feathers cannot be "archived" the same way — if the original authors step back, the Bevy maintainers absorb maintenance. This is a structural argument for in-tree placement that the Bevy Foundation has cited (informally). Buiy is currently a single-author project (intendednull); the same archive risk applies to Buiy that applied to `kayak_ui`. See [`../bevy-ui/governance.md`](../bevy-ui/governance.md) for the foundation-level argument.

## vs Buiy (parallel-stack, web-platform-parity, AccessKit-first)

Feathers and Buiy occupy adjacent but non-overlapping slots:

| Axis | feathers | Buiy |
|---|---|---|
| **Built on** | `bevy_ui` + `bevy_ui_widgets` + `bevy_a11y` + AccessKit (transitively) | Taffy + cosmic-text + AccessKit + bevy_picking + Bevy render graph (directly) |
| **Scope** | Editor + utilities; "can be used in games but not motivating" | Game + app; comprehensive; web-platform-parity catalog |
| **Theming** | Single dark OKLCH palette; no light / high-contrast variants; no hot reload | Multi-variant tokens; OS-preference-driven; subtree overrides; hot-reloadable; contrast linter |
| **Widget set** | 14 controls + 5 containers + 2 display (and growing) | Full APG catalog (button, link, listbox, combobox, slider, tabs, dialog, popover, tree, grid, menu, etc.) per [media-and-widgets.md](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md) |
| **Accessibility** | `bevy_a11y` (megacomponent until PR #24308); no per-widget WCAG SC mapping; partial `AccessibleLabel` post-fix | Direct AccessKit; decomposed `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations` from day one; full WCAG 2.2 AA per-widget mapping in [accessibility.md](../../specs/2026-05-07-buiy-foundation/accessibility.md) |
| **Renderer feature parity** | `bevy_ui`'s capabilities (no `backdrop-filter`, no `mix-blend-mode`, no true top layer, no non-rect clipping) — see [`../bevy-ui/critiques.md`](../bevy-ui/critiques.md) | Owns its render pipeline: rounded clipping, `clip-path`, `backdrop-filter`, `mix-blend-mode`, isolation, true top layer, gradients in any color space |
| **Animation** | None ([open-problems.md](open-problems.md)) | Transitions, keyframes, layout transitions, springs, reduced-motion-gated |
| **Status** | Experimental, in-tree, Bevy-versioned | Pre-foundation, single-author, spec phase |
| **Coexistence with Bevy `App`** | Native | Per-window with `bevy_ui` (no same-tree mixing) — see [cross-cutting.md § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md) |

The diverging design bets Buiy makes vs feathers (and the rationale for each):

1. **Parallel-stack, not layered on `bevy_ui`.** Feathers inherits `bevy_ui`'s render caps; Buiy owns its pipeline to close the web-platform gap. See [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § "The renderer feature gaps."
2. **Web-platform-parity feature catalog, not editor-scope.** Feathers ships what the editor needs; Buiy ships the modern-web UI feature set (anchor positioning, container queries, view transitions, top layer, the popover state machine, the full APG widget set). See [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) Validates row "Parallel-stack rationale."
3. **AccessKit-first with decomposed components.** Feathers (via `bevy_a11y`) is mid-migration off the megacomponent; Buiy starts decomposed. PR #24308 is the late-fix; Buiy's stance is the alternative.
4. **Game and app, both.** Feathers is explicit it is not aimed at games; Buiy commits to both in [README.md goal 6](../../specs/2026-05-07-buiy-foundation/README.md).

## Sources

- `bevy_feathers` source — `https://github.com/bevyengine/bevy/tree/main/crates/bevy_feathers/src`.
- `bevy_ui_widgets` source — `https://github.com/bevyengine/bevy/tree/main/crates/bevy_ui_widgets/src`.
- `bevy_egui` — `https://github.com/vladbat00/bevy_egui`.
- `bevy_lunex` — `https://github.com/bytestring-net/bevy_lunex`.
- `sickle_ui` — `https://github.com/UmbraLuminosa/sickle_ui`.
- `woodpecker_ui` — `https://github.com/StarArawn/woodpecker_ui`.
- `kayak_ui` (archived) — `https://github.com/StarArawn/kayak_ui`.
- Bevy 0.17 release notes — `https://bevy.org/news/bevy-0-17/`.
- PR #24308 (AccessibleLabel) — `https://github.com/bevyengine/bevy/pull/24308`.
