**Date:** 2026-05-22
**Status:** active
**Subject:** egui — the immediate-mode paradigm in full depth, history, costs, and the accessibility / animation problems it cannot fully solve

# Immediate-mode deep dive

This file is the conceptual hinge of the egui corpus. Everything in `architecture.md`, `api-surface.md`, `styling-and-theming.md`, and `text-rendering.md` follows from the immediate-mode choice. A shorter version of this conversation lives in [`../bevy-egui/immediate-mode-paradigm.md`](../bevy-egui/immediate-mode-paradigm.md) — that file framed immediate-mode through the Bevy bridge; this one goes deeper into the paradigm itself, its history, its structural limits, and the cases where immediate-mode is the right answer vs the wrong answer.

## What "immediate mode" actually means

In an immediate-mode UI, **the UI is a function call, not a data structure.** Each frame, the user runs a procedural routine that emits widgets:

```rust
fn ui(ctx: &egui::Context) {
    egui::Window::new("Inspector").show(ctx, |ui| {
        if ui.button("Save").clicked() {
            save_now();
        }
        ui.label(format!("FPS: {:.1}", current_fps()));
    });
}
```

Each call (`ui.button`, `ui.label`, `Window::new(...).show`) does three things at once:

1. **Allocates** a layout rectangle at the current cursor position.
2. **Paints** the widget into that rectangle (queues shapes into the current frame's `Vec<ClippedShape>`).
3. **Returns** a `Response` describing what happened to this widget this frame (clicked, hovered, dragged, focused).

There is no widget object, no widget tree, no event dispatcher. The "click handler" is the code that runs *after* `.clicked()` returns true, on the next line. State lives in the application's own variables, which the widget reads and mutates directly:

```rust
ui.checkbox(&mut self.show_grid, "Show grid");
ui.slider_value(&mut self.zoom, 0.1..=10.0).text("Zoom");
```

The `&mut` borrow is the data binding. There is no `model.set(...)` indirection, no observer, no derived state — the widget reads the variable to paint, writes the variable on interaction, and the next frame paints whatever value the application logic left in place.

## History: a 20-year arc

The immediate-mode UI tradition starts with one identifiable moment:

- **2005 — Casey Muratori**, then at Insomniac Games (best known for the *Ratchet & Clank* tools pipeline), publishes the forum thread [*Immediate-Mode Graphical User Interfaces*](https://caseymuratori.com/blog_0001) and a companion video. His argument: retained-mode UI toolkits (Win32, MFC, GTK) impose enormous bookkeeping cost on game-tool UIs that are throw-away by design. He sketches an alternative: every frame, walk a procedural routine that draws the UI and handles input in the same pass.

- **2014 — Omar Cornut** (Media Molecule, ex-Tequila Works) publishes [*Dear ImGui*](https://github.com/ocornut/imgui) as a C++ single-header library implementing Muratori's idea. Within five years it becomes the de facto debug-UI library for the entire commercial game industry — used inside Unity, Unreal, Frostbite, Blizzard, EA, every game studio with internal tools. The aesthetic ("dev tools look") becomes ubiquitous.

- **~2018 — Emil Ernerfeldt** starts an experimental Rust UI library called Emigui as a personal project. He renames it to **egui** in 2020 and publishes 0.1.0 to crates.io on 2020-05-30.

- **2021–2026 — egui matures.** Adoption inside the Rust gamedev community is immediate (no pun intended). Emil founds **Rerun.io** in 2022 (a startup building a visualizer for multimodal robotics/ML data); Rerun's viewer is built in egui, and Rerun becomes the commercial driver behind egui development. The README states "egui development is sponsored by Rerun."

- **Today (2026-05-22)** — egui at 0.34.2 with 16.96M total downloads, sole most-downloaded Rust GUI library. Dear ImGui still dominates C++ gamedev tooling; egui dominates Rust gamedev tooling. The two libraries are siblings, not competitors — they target different language ecosystems but share the architecture.

## The "no separate data model" promise

The defining claim of immediate-mode is **"the UI code IS the data model."** A traditional retained-mode framework asks the user to:

1. Define a widget tree (View, Model, ViewModel — pick your acronym).
2. Wire up data bindings (`bind`, `@Published`, `signal`, `useState`).
3. Define event handlers as callbacks (`onClick`, `onChange`).
4. Manage lifetimes (`Widget::dispose`, RAII handles, automatic vs manual cleanup).
5. Synchronize state changes back to the tree (rebuild diffs, virtual DOM, fine-grained reactivity).

Immediate-mode collapses all five into "write the UI function, your variables are the model." The function reads from variables, paints based on what it reads, returns interaction results, and your code handles them inline before the next variable is read. There is no synchronization problem because there is no separate model to synchronize *with*.

This is the central design insight. Everything good and everything bad about immediate-mode follows from it.

## What you give up

The trade is real. Things that are difficult or impossible without a persistent tree:

### Animation that survives a logic frame

An animation needs state that lives between frames. egui handles this by stashing animation state in `Memory` keyed by widget `Id` — `ui.animate_bool(id, target)` is the canonical helper. But this state isn't owned by a "widget object" — it's owned by a hash map, keyed by an identity the user code has to construct correctly. Subtle bugs: if a widget's `Id` collides with another widget's, their animations interfere. If a widget moves between code paths (e.g. an item shown sometimes in a list, sometimes elsewhere), its animation state may follow or may not, depending on whether the `Id` derivation is stable.

For simple animations (button hover, expand/collapse), this is fine. For complex animation graphs (a particle-spawning settings panel where multiple animations layer and interact), the framework fights you because the animation state has nowhere natural to live.

### Layout caching across frames

Retained-mode systems (the DOM, bevy_ui, Buiy) cache layout measurements keyed by node identity. Taffy in particular goes to substantial lengths to skip layout work for subtrees whose inputs haven't changed. Immediate-mode has no node identity to key on — every frame's layout is recomputed from scratch.

For dev-tools workloads (~hundreds of widgets, simple layouts), this is fine. For productivity-app workloads (thousands of widgets, complex layouts, scroll virtualization), it's a fixed per-frame floor that retained-mode can avoid. Emil's own data: "1-2 ms overhead per frame" for typical UIs; that's the cost of rebuilding the layout every frame.

### Stable accessibility tree

This is the deepest structural problem.

Assistive technologies (NVDA, JAWS, VoiceOver, Narrator, Orca, AT-SPI) work via an external API: the app exposes a tree of accessible nodes, each with a stable identity, role, name, value, and state. The AT walks this tree, caches information about it, and uses node identity to track focus, announce changes, and offer navigation commands ("next heading," "previous landmark").

A persistent tree is the natural substrate. The DOM exposes it directly; native UI frameworks (UIKit, AppKit, GTK, Qt) build it from their widget objects.

Immediate-mode has no persistent tree. **egui's accessibility solution** is to build one anyway, *at the end of each frame*, specifically for AccessKit:

1. During the frame, every widget call appends an `accesskit::Node` (or similar) into a per-frame builder.
2. At `end_pass`, egui finalizes this into an `accesskit::TreeUpdate`.
3. The host backend (`eframe`, `bevy_egui`) pushes the `TreeUpdate` into the `accesskit_winit::Adapter` for the window.
4. The OS-side AT bridge translates the `TreeUpdate` into UIA / AT-SPI / NSAccessibility notifications.

This works — egui ships AccessKit integration that real screen readers can consume. It is also fundamentally a bolt-on. Specific costs:

- **Identity is derived from `Id`** (see [`architecture.md`](architecture.md)). When the user code's call structure changes between frames, the derived `accesskit::NodeId`s shift, and the AT may lose track of focus or announce phantom changes.
- **No incremental updates.** The tree is rebuilt every frame; even small UI changes trigger a full tree diff. AccessKit's diffing engine absorbs this, but the steady-state CPU cost is higher than a retained-mode system would pay.
- **Complex ARIA patterns (`aria-activedescendant`, live regions, `aria-controls`/`aria-owns`) require the user code to thread state through frames manually** — there's no persistent node to attach the relationship to.
- **`aria-busy`, `aria-live`, polite vs assertive announcements** all need careful handling because the announcer state is per-frame, not per-region-entity.

The egui+AccessKit integration is real and shipping (the 0.33.0 release notes mention AccessKit 0.21.0, and the 0.34.0 release notes call out "Scroll bars and resize splitters now visible to AccessKit"). It is also the case that **deep AT integration with complex APG patterns is structurally awkward** in any immediate-mode framework, because the underlying paradigm doesn't have a persistent surface for the AT to reason about. Buiy's foundation spec ([`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.6) commits to AccessKit-first with retained entity-backed components for exactly this reason.

## Emil Ernerfeldt's design philosophy (verbatim)

From the upstream egui README, the goals section:

> - The easiest to use GUI library
> - Responsive: target 60 Hz in debug build
> - Friendly: difficult to make mistakes, and shouldn't panic
> - Portable: the same code works on the web and as a native app
> - Easy to integrate into any environment
> - A simple 2D graphics API for custom painting (epaint)
> - Pure immediate mode: no callbacks
> - Extensible: easy to write your own widgets for egui
> - Modular: ability to use small parts of egui and combine them in new ways
> - Safe: there is no unsafe code in egui
> - Minimal dependencies

And the non-goals:

> - Become the most powerful GUI library
> - Native looking interface

And from the README's positioning text:

> "egui aims to be the best choice when you want a simple way to create a GUI, or you want to add a GUI to a game engine."

> "Not recommended for: non-Rust projects, applications requiring native OS appearance, or users needing API stability between versions."

The honesty of this list deserves attention. egui is not pitched as a comprehensive UI framework. It is pitched as the simplest possible way to put a GUI into a Rust program, with explicit acknowledgment that other tools win on power, native look, and stability. The non-goals are doing real work — they're not throwaway disclaimers but actual scope decisions that explain every API limitation downstream.

## When immediate-mode is the RIGHT choice

The fit-for-purpose workloads:

### Dev tools and inspectors

The UI's structure depends on the live state of some external thing (the ECS world, a debugger snapshot, an asset's metadata). Rebuilding the UI from scratch each frame is *correct* — there's no synchronization problem because the UI is always derived from current state. Reflecting an arbitrary live data structure into widgets is a few lines of code. `bevy-inspector-egui` is the canonical example: it walks the ECS via Bevy's reflection and emits egui widgets for every field. The framework's "rebuild everything each frame" cost model exactly matches the requirement.

### Debug overlays

FPS counter, memory usage, log viewer, in-game console, profiler timeline. The data changes continuously; rebuilding the UI each frame is exactly what you want. No animation budget, no accessibility requirement, no UX polish required.

### Settings panels and configuration UIs

A few dozen sliders, checkboxes, color pickers, text inputs. Fits on one screen. No animation. No complex focus traversal. Often the user closes the panel after each edit. egui's per-frame cost is trivial at this scale; the API's ergonomics (`&mut` data binding) make this the fastest possible way to write the panel.

### Level editors, modding UI, asset browsers

Game developers ship these to themselves or to a small power-user audience. egui's "not native-looking" is actually a *feature* here — these tools are supposed to look like dev tools, not like polished consumer apps. The visual style signals "this is the editor, not the game." Studios using egui in this role include Embark Studios (their in-house tools).

### Prototypes

You can put a working UI together in 30 minutes. The Rust borrow checker doesn't fight you because there's no shared mutable state between widgets — each widget call has its own `&mut Ui` scope. For "I need to test this idea right now" workflows, the time-to-first-working-UI is unmatched.

### Rerun's data visualization viewer

The flagship production app shipping on egui is Rerun itself. The Rerun viewer is a multimodal data visualizer (point clouds, time-series, tensors, log messages) where the UI is fundamentally driven by external streaming data. The viewer's UI is inherently dev-tools-shaped — many panels, dense information density, no animation, technical user audience. egui's paradigm is a perfect fit, and Emil eats his own dog food. This is the cleanest "immediate-mode at scale, in production" case study.

## When immediate-mode is the WRONG choice

The misfit workloads. These are also the workloads Buiy targets.

### Production game UI (HUD, main menu, in-game menus)

A polished game UI is built around:

- **Animation everywhere** — buttons that pulse, panels that slide in, settings sliders that ease into position, transition animations between menu states. Each animation needs state that survives many frames; each is timed against a curve that needs interpolation across frames; combined animations layer in complex ways.
- **A consistent visual identity** that matches the game's art direction (not "egui's look").
- **Complex focus traversal** across many screens, often gamepad-driven, with screen-to-screen state restoration.

egui *can* render production game UI — many small Bevy games ship that way at first — but every animation is a fight with the framework, and the result looks like egui (which is fine for dev tools, wrong for a published title). Most shipped Bevy games with polished UI either wrote a custom UI renderer (Tiny Glade) or used bevy_ui.

### Accessibility-critical applications

Productivity apps, government services, anything that needs to clear WCAG 2.2 AA. The accessibility limitations discussed above are not just inconveniences — they're structural. An app whose users include screen-reader users wants every interaction to feel snappy to the AT, focus to be tracked precisely, ARIA relationships to survive frames. Immediate-mode + bolted-on AccessKit gives you something AT-readable, but the polish ceiling is real. For an app where accessibility is a launch criterion, retained-mode is the right substrate.

### Complex animations

Animations with curves, springs, layered timing, layout transitions (CSS-style `transition: layout 0.2s ease`), keyframe sequences. Each of these wants state that lives somewhere stable, on something stable. egui's `Memory`-keyed animation state can implement a single simple curve cleanly; once you need multiple simultaneous animations on the same widget, or layout transitions, or sprite-sheet-driven UI animations, the model fights you.

### Productivity-app scale

Tools with hundreds of panels, thousands of widgets, scroll-virtualized lists with millions of items, complex layout caches. Immediate-mode's per-frame rebuild cost (~1-2 ms for typical UIs per Emil's data) scales linearly with widget count and starts to dominate the frame budget around 1000-5000 widgets depending on machine. Retained-mode plus layout caching plus scroll-area virtualization (only emit widgets for visible rows) absorbs this — Buiy's foundation spec targets 1000+ node fixtures in its verification harness ([`../../specs/2026-05-07-buiy-foundation/verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)).

### BSN-authored UI

Bevy's draft scene format (PR [#20158](https://github.com/bevyengine/bevy/pull/20158)) reflects components onto entities. Immediate-mode has no entities at the point of authoring — the UI exists only during the function call, not as a serializable graph. BSN and immediate-mode are model-incompatible by construction. Buiy's foundation goal "BSN-native" ([`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 1.3) directly requires retained-mode.

## The dev/ship pattern

The pattern that emerges in practice from many shipping Bevy projects: **use egui (via bevy_egui) for dev-time inspectors and debug overlays; use a retained-mode stack (bevy_ui today, Buiy tomorrow) for the actual game HUD and menus.** The two paradigms coexist cleanly because:

- egui draws last (above bevy_ui / Buiy by default render-graph ordering).
- Pointer input is captured by whichever stack the cursor is over; bevy_egui's picking integration handles this transparently.
- Each stack owns its own AccessKit adapter (or one is suppressed in dev mode; production builds typically ship without dev-tools accessibility).
- Dev builds compile bevy_egui in; production release builds compile it out via cargo features.

This pattern lets a Bevy project enjoy egui's rebuild-from-scratch dev velocity for tooling while keeping retained-mode rigor for shipping UI. Buiy's coexistence design ([`../../specs/2026-05-07-buiy-foundation/cross-cutting.md`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md) § 3.18) explicitly supports this — Buiy and bevy_egui can run side-by-side on the same Bevy app, just not in the same window's UI tree.

## See also

- [`architecture.md`](architecture.md) — the mechanics of how egui implements immediate-mode (Context, Ui, FullOutput, Memory, Id).
- [`api-surface.md`](api-surface.md) — the widget vocabulary that results.
- [`../bevy-egui/immediate-mode-paradigm.md`](../bevy-egui/immediate-mode-paradigm.md) — shorter version, framed through the Bevy bridge.
- [`../bevy-egui/use-cases.md`](../bevy-egui/use-cases.md) — the dev/ship pattern in practice.
- [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) — Buiy's retained-mode commitment and the reasoning behind it.

## Sources

- egui README — https://github.com/emilk/egui
- Casey Muratori — *Immediate-Mode Graphical User Interfaces* (2005) — https://caseymuratori.com/blog_0001
- Omar Cornut / Dear ImGui — https://github.com/ocornut/imgui
- egui CHANGELOG (AccessKit notes) — https://raw.githubusercontent.com/emilk/egui/master/CHANGELOG.md
- AccessKit — https://accesskit.dev
- Rerun.io — https://rerun.io
- Bevy PR #20158 (BSN draft) — https://github.com/bevyengine/bevy/pull/20158
- bevy-inspector-egui — https://github.com/jakobhellermann/bevy-inspector-egui
- Buiy foundation spec — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
