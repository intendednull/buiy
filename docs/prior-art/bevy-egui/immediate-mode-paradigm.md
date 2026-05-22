**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_egui — the immediate-mode paradigm and why Buiy chose retained-mode instead

# Immediate-mode paradigm

This is the conceptual hinge of the bevy_egui corpus. Everything else — the plugin shape, the widget API, the use-case fit, the limits — falls out of the immediate-mode choice. Buiy is retained-mode. The two paradigms are *not* points on a continuum; they are different architectures with different costs that suit different workloads.

## What "immediate mode" means in egui

Each frame, the user re-runs their UI code from scratch:

```rust
fn ui_system(mut contexts: EguiContexts) {
    egui::Window::new("Inspector").show(contexts.ctx_mut().unwrap(), |ui| {
        if ui.button("Save").clicked() {
            // handle click *now*
        }
        ui.label(format!("FPS: {:.1}", 1.0 / dt));
    });
}
```

The `button` call **emits** a button into the current layout pass and **returns** a `Response` describing what happened to it this frame (clicked, hovered, dragged, etc.). There is no `Button` entity, no `OnPress` observer, no persistent widget tree. Each frame is independent; the data flow is one-way (state → widgets) with the return values closing the loop locally.

This is the architecture Casey Muratori sketched in his 2005 "IMGUI" forum thread and that Omar Cornut implemented as Dear ImGui in 2014. egui is the Rust-native lineal heir.

## egui's stated philosophy (Emil Ernerfeldt, verbatim from the upstream README)

> "egui (pronounced 'e-gooey') is a simple, fast, and highly portable immediate mode GUI library for Rust."

Goals named explicitly:

> - The easiest to use GUI library
> - Responsive: target 60 Hz in debug build
> - Friendly: difficult to make mistakes, and shouldn't panic
> - Portable: the same code works on the web and as a native app
> - Easy to integrate into any environment
> - Pure immediate mode: no callbacks

Non-goals named explicitly:

> - Not aimed at becoming "the most powerful GUI library"
> - Does not target "Native looking interface"

Audience:

> "egui aims to be the best choice when you want a simple way to create a GUI, or you want to add a GUI to a game engine."

"Not recommended for: non-Rust projects, applications requiring native OS appearance, or users needing API stability between versions."

The honesty of this list is worth pausing on. egui is not pitched as a comprehensive UI framework. It is pitched as the simplest possible way to put a GUI into a Rust program, with explicit understanding that other tools win on power, native look, and stability.

## What gets erased: state that doesn't exist between frames

Retained-mode systems (Bevy ECS, bevy_ui, Buiy) hold UI as a persistent tree of entities. Each entity has components, observers can fire on its events, animation systems can interpolate its fields, layout solvers can cache its measurements, accessibility adapters can walk it.

Immediate-mode systems hold UI as a sequence of function calls. **There is no widget tree between frames.** A button that was visible last frame is gone unless your code re-emits it this frame. There's no persistent button entity to attach an observer to, no place to hang an animation curve, no node for an accessibility tree to point at (without help — see below).

What egui actually keeps between frames is a single `egui::Context` struct holding:
- The font atlas / texture cache.
- A `Memory` map keyed by widget `Id` (see below), used to retain things like text-edit cursor position, scroll offset, collapsing-header open state, window position, drag-state.
- The previous frame's tessellated output, available for input hit-testing.

That is the entire retained surface. Everything else is rebuilt every frame.

## The id system: stable identity from the call stack

Because widgets don't persist as entities, egui has to derive a stable identity for "this button" across frames or it cannot remember things like "was this combo-box open last frame." egui's solution is its `Id` system — when you call `ui.button("Save")`, egui hashes:

1. The label `"Save"` (or an explicitly-passed `Id::new(...)`).
2. The current layout cursor's hash.
3. The parent UI's `Id` (recursive — `Window::new("Inspector")` contributes its title hash, the enclosing `ui.horizontal(...)` contributes its scope, etc.).

The resulting `Id` is the key into `Memory`. This works well for static layouts and fails for dynamic content — two list items with identical labels produce identical Ids, and the second one will inherit the first one's state (cursor position, scroll offset). The workaround is `ui.push_id(i, |ui| { ... })` per item. Every long-running egui project develops local muscle memory for which loops need a `push_id`. Emil discusses the tradeoff explicitly in the egui rustdoc.

This is the immediate-mode tax: persistent state needs an explicit collision-free key, derived from call-site context.

## When immediate-mode wins

Workloads where immediate-mode pays off:

- **Dev tools and inspectors.** The UI's structure depends on the current state of some external thing (the ECS world, a debugger snapshot). Rebuilding it from scratch each frame is *correct*; there's no synchronization problem. Reflecting an arbitrary live data structure into widgets is a few lines of code. `bevy-inspector-egui` is the canonical example.
- **Debug overlays.** FPS counter, log viewer, in-game console. The data changes continuously; rebuilding the UI each frame is exactly what you want.
- **Settings panels.** A few sliders, checkboxes, color pickers. Fits on one screen. No animation, no complex layout, no accessibility tree consumed by an OS-level AT.
- **Level editors, modding UI, asset browsers.** Game developers ship these to themselves or to a small power-user audience. egui's "not native-looking" is actually a feature here — these tools are *supposed* to look like dev tools.
- **Prototypes.** You can put a working UI together in 30 minutes.

The trade is honest: you give up the ability to layer behavior on a persistent tree (animation, complex focus management, accessibility integration without per-widget bookkeeping) and you gain the ability to throw a UI together in a few lines without thinking about state synchronization.

## When retained-mode wins (and Buiy lives here)

Workloads where retained-mode pays off — these are Buiy's target:

- **Production game HUD / menus** with custom visual style, animation, transition state, complex focus traversal across many screens. egui can render these, but every frame of every menu rebuilds the entire tree from scratch and you fight the framework on every animation. Most shipped Bevy games that look polished wrote a custom UI renderer (Tiny Glade) or used bevy_ui.
- **Productivity-app UIs** with hundreds of widgets, scroll virtualization, layout caching, screen-reader trees that need stable nodes across many frames. Each frame's tree rebuild is the cost model that breaks down.
- **BSN-authored UIs** (Bevy's draft scene format, PR [#20158](https://github.com/bevyengine/bevy/pull/20158), still unmerged as of 2026-05-22). BSN reflects components onto entities; immediate-mode has no entities to reflect onto. The two models are incompatible by construction.
- **AccessKit tree shape stability.** Real ATs (NVDA, VoiceOver, JAWS) work better with stable node IDs across frames. egui+AccessKit re-derives the tree each frame; the integration is real and shipping (re-enabled in bevy_egui 0.38.0) but is fundamentally bolt-on to a model that doesn't have persistent nodes.
- **Layout caching.** Taffy caches measurements across frames keyed on node identity. Immediate-mode has no node identity to key on, so each frame's layout is recomputed from scratch. egui's tessellator is fast enough that this is fine at moderate scale — Emil notes "most cases show 1-2 ms overhead" — but it's still a fixed per-frame floor that retained-mode can avoid.

## Comparison to Dear ImGui (the C++ progenitor)

Omar Cornut released Dear ImGui in 2014. egui shares the architecture — same per-frame rebuild, same call-site-derived id system, same `Response`-returning widget functions, same explicit `push_id` discipline. Differences:

- **Language fit.** Dear ImGui is C/C++; egui is Rust. egui's borrow-checker shape (`ctx.ctx_mut()` returns an `&mut Context` so widgets can mutate `Memory`) is a notable Rust-ism.
- **Rendering portability.** Dear ImGui ships a tessellated vertex buffer and lets the host renderer draw it. egui does the same with its `ClippedPrimitive` output. Both rely on a host integration layer (Dear ImGui has dozens; egui has eframe + bevy_egui + egui-wgpu + egui-winit + many community ones).
- **Style.** Dear ImGui defaults to a dense, dev-tools aesthetic; egui similar but with cleaner typography and antialiased shapes.
- **Accessibility.** Dear ImGui has no a11y story. egui has optional AccessKit integration — a notable lead for the egui camp, even if the underlying paradigm makes deep AT integration awkward.

The lineage is clear: every immediate-mode UI library since 2005 inherits the Casey Muratori sketch; Dear ImGui is the production proof; egui is the Rust-native equivalent.

## Why Buiy is retained-mode (the foundation spec's reasoning)

Buiy's foundation spec ([`/home/user/buiy/docs/specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.3) commits to a retained component model — `buiy::Node`, `buiy::Style`, focus components, a11y components, animation components — and ECS + BSN authoring (§ 2.4). The reasoning, restated through the immediate-mode lens:

1. **Web-platform parity** ([README.md § 1.1](../../specs/2026-05-07-buiy-foundation/README.md)). The web platform is retained-mode — the DOM is a persistent tree, CSS cascades on stable nodes, layout invalidation is on dirty subtrees, accessibility surfaces a persistent role tree. Parity requires a persistent surface to mirror.
2. **Accessibility tree shape.** AccessKit nodes survive frames; ATs are happier with stable IDs. Buiy's `A11yRole` / `A11yLabel` / `A11yStates` components are entity-backed and survive frames by construction ([architecture.md § 2.6](../../specs/2026-05-07-buiy-foundation/architecture.md)).
3. **BSN-native authoring** ([README.md § 1.3](../../specs/2026-05-07-buiy-foundation/README.md), foundation goal 3). BSN reflects components onto entities. Immediate-mode has no entities at the point of authoring.
4. **Layout caching at scale.** Productivity-app fixtures at 1000+ nodes (the verification harness in [verification.md](../../specs/2026-05-07-buiy-foundation/verification.md)) need cached layout, which needs stable node identity.
5. **Animation on persistent nodes** ([interaction.md § 3.8](../../specs/2026-05-07-buiy-foundation/interaction.md)). Property transitions, keyframe animations, layout transitions all assume nodes survive frames so animations have something to animate.

None of this is an indictment of egui. egui solves a different problem — see [use-cases.md](use-cases.md) for what it's good at. The point is that the paradigms are not interchangeable, and Buiy's retained-mode commitment is what the comprehensive-feature-parity + a11y + BSN goals require.

## The "dev mode + ship mode" pattern (preview)

A pattern shipped by many Bevy projects: use bevy_egui for dev-time inspectors and debug overlays, use a retained-mode stack (bevy_ui today, Buiy tomorrow) for the actual game HUD and menus. The two coexist cleanly because bevy_egui draws last (above bevy_ui) and consumes its own pointer hits via the picking integration. See [use-cases.md § dev/ship pattern](use-cases.md).

## Sources

- egui README — https://github.com/emilk/egui
- Emil Ernerfeldt — egui design philosophy (README sections "Goals," "Non-goals," "Why immediate mode")
- Casey Muratori — Immediate-Mode Graphical User Interfaces (2005) — https://caseymuratori.com/blog_0001
- Dear ImGui — https://github.com/ocornut/imgui
- bevy_egui repo — https://github.com/vladbat00/bevy_egui
- Buiy foundation spec — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Bevy PR #20158 (BSN, still draft) — https://github.com/bevyengine/bevy/pull/20158
- Sibling files: [architecture.md](architecture.md), [api-surface.md](api-surface.md), [integration.md](integration.md), [use-cases.md](use-cases.md)
