**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_egui — where it shines, where it shouldn't be used, and the dev/ship pattern

# Use cases

bevy_egui is the default Bevy choice for dev tooling. With ~2 million lifetime downloads and ~287k in the last 90 days, it is by a wide margin the most-installed Bevy UI plugin. This file documents what that adoption is *for* — and what it's *not* for. The goal is honest scoping, not advocacy.

## Where bevy_egui wins

### Dev tools: world inspectors, debuggers, profilers

The flagship case. `bevy-inspector-egui` (by jakobhellermann, ~600k+ lifetime downloads) is the de-facto Bevy world inspector: every entity, every component, every resource, every asset shown live in a panel, every field directly editable. The fit is exact — the data tree changes every frame, immediate-mode rebuilds the UI from the live data with no synchronization layer, edits flow back directly because each field's `&mut T` reference is the editor.

This pattern works because:
- The inspector's structure mirrors the ECS world's structure; there's no separate UI state to keep in sync.
- The user audience is the developer themselves; egui's dev-tools aesthetic is the right look.
- Performance ceiling is one machine running the dev build; the immediate-mode cost model is not the bottleneck.
- Accessibility is not required (dev tools, not shipped).

Adjacent: `bevy_editor_pls` (in-app developer panel), `bevy-debug-text-overlay`, `bevy_console`, `bevy_dev_tools`, plus dozens of project-local inspectors.

### Debug overlays

FPS counters, frame-time graphs, log viewers, ECS schedule visualizers, render-graph viewers. Same fit: data is volatile, audience is the developer, no a11y requirement, low widget count. egui's `Plot` API (now in the separate `egui_plot` crate, formerly bundled) is a workhorse for live metric visualization.

### Level editors / scene editors

The pattern bevy_editor_pls follows: a panel-based dock UI with property inspectors, scene hierarchy, asset browser, scrubbable timelines, and a 3D viewport with picking. Every editor written in Bevy that hasn't built a custom UI uses egui. The closest production-grade example is the in-progress Bevy Editor project itself (still pre-1.0), which has experimented with both bevy_ui and bevy_egui for various panels.

### Settings panels, debug menus

A few widgets, fits on one screen, no animation polish, no AT consumption — egui is overkill *enough* that you can stand it up in 20 minutes. Production games sometimes ship a `~` console (developer cheat menu, gated behind a key) on top of egui even when the rest of the HUD is custom.

### Modding UI: plugin browsers, asset selectors

Same shape as a level editor's asset browser. Sandboxed inside a host app; users are technical; consistency with host's visual style is not required (often actively undesired — modding UIs are *supposed* to look like dev tools to signal "advanced").

### Prototypes

When the question is "does this game mechanic feel right" and not "does this UI look right," egui is the fastest path from idea to playable build. Sliders for tuning constants, checkboxes for toggling features, drop-down for swapping behavior modes — all in under an hour.

## Where bevy_egui does *not* belong

### Production game HUD / menus

Honest reality, not editorial:

- **Visual polish.** egui's default visuals are dev-tools dense. Theming exists (`Visuals::dark()` / `light()` plus custom palette) but the design language is "tool" — flat panels, rectangular widgets, standard interaction patterns. Games that ship polished UI write either custom UI (Tiny Glade, Foresight Spar, many shipped indie titles) or use bevy_ui plus heavy `UiMaterial` work.
- **Animation.** egui has internal `animate_bool` / `animate_value` helpers that interpolate over a few frames, but there's no per-property transition system, no keyframe timelines, no spring physics. Production HUDs (combat damage popups, score-up flourishes, screen-transition wipes) need first-class animation; egui doesn't provide it.
- **Layout flexibility.** Containers are dock-style (left/right/top/bottom/central panels) plus windows. Free-form CSS-grid / flexbox layouts with named tracks, subgrid, anchor positioning — the modern web layout vocabulary — is not there. A game HUD with concentric ring meters around a health bar with damage indicators floating off the edges is more naturally a render-pass than an egui scene.
- **Accessibility.** AccessKit support is real ([api-surface.md § accessibility](api-surface.md)) but off by default in bevy_egui, focused on dev-tools widgets, and structurally limited by immediate-mode (see [immediate-mode-paradigm.md § when retained-mode wins](immediate-mode-paradigm.md)). WCAG 2.2 AA conformance for a complex menu hierarchy is not a goal egui targets.
- **Localization.** egui has no built-in i18n framework. Text wrapping respects BiDi and RTL (via egui's own text layout), but ICU MessageFormat, plural rules, locale-aware date/time formatting, all are app-side concerns.

Counterexample that supports the framing: `Foresight: Spar`, `RoboQuest`, `Tiny Glade`, `0AD`-style games that have shipped on Bevy do *not* use egui for the player-facing HUD. Most cited Bevy commercial release that has shipped — Tiny Glade — built a custom UI renderer entirely outside both bevy_ui and bevy_egui.

### Productivity apps with serious widget counts

Once you cross ~1000 visible widgets, immediate-mode's per-frame rebuild cost stops being negligible. egui's tessellator is fast — Emil notes "1-2 ms overhead" for typical cases — but that's a fixed floor per frame regardless of whether anything changed. A retained-mode stack with proper change-detection draws zero pixels for an unchanged frame. For long-running productivity workloads (IDEs, audio workstations, video editors), this matters.

The shipping counterexamples — Rerun (https://rerun.io, time-series viewer for ML/robotics) and Zed (https://zed.dev, code editor by Nathan Sobo) — are both worth flagging carefully:

- **Rerun** uses egui extensively and ships at scale. It works because Rerun's data is *always changing* (streaming sensor data) — there's no "unchanged frame" to optimize for, so immediate-mode's per-frame rebuild cost is amortized against work you'd be doing anyway. This is the pattern that fits egui best at scale.
- **Zed** is *not* on egui. Zed's UI is on GPUI, Nathan Sobo's bespoke retained-mode Rust framework. Naming it here as an egui win would be wrong.

### Apps requiring strict a11y / WCAG conformance

egui's AccessKit integration is genuine but unevenly mature: covers Windows / macOS solidly, Linux via experimental `accesskit_unix`, web via the AccessKit web adapter (incomplete). WCAG 2.2 SC enforcement, ARIA roles for all WAI-ARIA-APG patterns, accessible name/description computation per ACCNAME 1.2 — these are not what egui is designed for.

### BSN-authored UIs

BSN reflects components onto entities. egui widgets are not entities. The two are incompatible by construction.

## The "dev mode + ship mode" pattern

A common Bevy-shipped pattern:

```rust
fn main() {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);

    // Production UI — bevy_ui today, Buiy tomorrow.
    app.add_plugins(GameHudPlugin);

    // Dev tools — opt in via feature flag or runtime key.
    #[cfg(feature = "dev")]
    {
        app.add_plugins(EguiPlugin::default())
            .add_plugins(WorldInspectorPlugin::default());
    }

    app.run();
}
```

Or runtime-gated:

```rust
fn toggle_inspector(
    keys: Res<ButtonInput<KeyCode>>,
    mut visible: ResMut<InspectorVisible>,
) {
    if keys.just_pressed(KeyCode::F11) {
        visible.0 = !visible.0;
    }
}
```

This pattern works because bevy_egui draws on top of bevy_ui by default ([integration.md § coexistence](integration.md)) and consumes its own pointer hits via the picking integration. The dev UI doesn't interfere with the shipped UI; toggling it is one keystroke.

**For Buiy users this pattern is the right default.** Buiy occupies the production-HUD slot; bevy_egui occupies the dev-tools slot. They are designed for different workloads and there is no need to pick between them. The Buiy foundation spec's "no mixing in one window" rule ([cross-cutting.md § 3.18](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)) is about *bevy_ui* coexistence, not bevy_egui — bevy_egui's no-default-AccessKit + flat-tessellated-output composes cleanly with whatever else is on screen.

## What this means for the prior-art corpus

bevy_egui is in `docs/prior-art/` not because Buiy is competing with it but because Buiy users will use both. Every Buiy spec that touches dev-tooling (the inspector subsystem in [cross-cutting.md § 3.16](../../specs/2026-05-07-buiy-foundation/cross-cutting.md), the layout overlay, the focus visualizer, the theme editor) has a "should we ship this on bevy_egui or build it on Buiy itself" question. The honest answer for most: ship the dev tool on bevy_egui, build the production UI on Buiy. Dogfooding the Buiy devtools on Buiy itself is also fine, but it's a choice with tradeoffs — Buiy is heavier per widget than egui, and dev tools are exactly the workload egui is best at.

See [immediate-mode-paradigm.md](immediate-mode-paradigm.md) for the conceptual basis of the dev/ship split, and sibling files [architecture.md](architecture.md), [api-surface.md](api-surface.md), [integration.md](integration.md) for the mechanics.

## Sources

- bevy-inspector-egui — https://github.com/jakobhellermann/bevy-inspector-egui
- bevy_editor_pls — https://github.com/jakobhellermann/bevy_editor_pls
- bevy_console — https://github.com/RichoDemus/bevy-console
- Rerun (egui at scale, streaming-data workload) — https://github.com/rerun-io/rerun
- Tiny Glade (custom UI renderer, not bevy_ui or bevy_egui) — Pounce Light blog
- Zed (GPUI, not egui — counter-cite) — https://zed.dev
- bevy_egui README & CHANGELOG — https://github.com/vladbat00/bevy_egui
- Sibling files: [architecture.md](architecture.md), [immediate-mode-paradigm.md](immediate-mode-paradigm.md), [api-surface.md](api-surface.md), [integration.md](integration.md)
- Buiy foundation cross-cutting — [`../../specs/2026-05-07-buiy-foundation/cross-cutting.md`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)
