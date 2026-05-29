**Date:** 2026-05-22
**Status:** archived
**Subject:** kayak_ui — public API surface at 0.5.0: widget vocabulary, `rsx!` composition, state + event flow.

# API

Snapshot from kayak_ui `0.5.0` (last release, 2024-02-11). Surface is frozen; nothing here has shipped or shifted since.

## App setup

The canonical wire-up:

```rust
use bevy::prelude::*;
use kayak_ui::prelude::*;
use kayak_ui::widgets::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugin(KayakContextPlugin)
        .add_plugin(KayakWidgets)
        .add_systems(Startup, startup)
        .run();
}

fn startup(mut commands: Commands) {
    let camera = commands.spawn(Camera2dBundle::default())
        .insert(CameraUIKayak).id();
    let mut widget_context = KayakRootContext::new(camera);
    widget_context.add_plugin(KayakWidgetsContextPlugin);
    let parent_id = None;
    rsx! {
        <KayakAppBundle>
            <TextWidgetBundle text={"hello".into()} />
        </KayakAppBundle>
    };
    commands.spawn((widget_context, EventDispatcher::default()));
}
```

Three plugins to think about: `KayakContextPlugin` (Bevy plugin, core systems + render), `KayakWidgets` (Bevy plugin, default widgets), `KayakWidgetsContextPlugin` (kayak_ui-internal plugin via the `KayakUIPlugin` trait, registers the widget set into a specific `KayakRootContext`). The two-tier plugin system is unusual and a documented source of consumer confusion.

## Widget vocabulary at 0.5.0

The bundled `kayak_ui::widgets` module exposes a deliberately-small set; the README markets these as "*useful as default widgets for debugging purposes*" and "*a guide for building your own widgets.*" Consumers building real UI were expected to hand-roll most widgets.

**Core / container**
- `KayakApp` — the root.
- `Element` — a generic divlike container.
- `Background` — colored / textured background quad.
- `Clip` — clipping container.

**Display**
- `Text` (a.k.a. `TextWidget`) — single-line text node.
- `KImage` — image rendering.
- `KSvg` — SVG rendering (added in 0.4, 2023-04).
- `NinePatch` — nine-slice image.
- `TextureAtlas` — atlas-backed sprite.

**Input**
- `KButton` — clickable button. (Note `K`-prefix to avoid colliding with Bevy's `Button`.)
- `TextBox` — single-line text input. No multi-line, no IME preedit rendering, no BiDi caret (see [`critiques.md`](critiques.md) § Text input).

**Container / overlay**
- `KWindow` — a movable, focusable subwindow container (in-app, not OS-level).
- `Modal` — modal overlay container (no formal focus trap, see [`architecture.md`](architecture.md#focus-tree)).
- `Window` — re-export ambiguity with `KWindow`; both names appear in 0.5.0 docs.

**Scroll**
- `ScrollBox`, `ScrollBar`, `ScrollContent` — paired scroll-container widgets.

**Animation**
- `Transition` — animated transitions for show/hide.

**Specialized**
- `Accordion`, with `AccordionContext`, `AccordionSummary`, `AccordionDetails` — the only APG-pattern widget in the set, and even it omits parts of the [APG Accordion pattern](https://www.w3.org/WAI/ARIA/apg/patterns/accordion/) (no `aria-controls` wiring, no `aria-expanded` accessible-state surfacing, no keyboard contract enforcement). See [`critiques.md`](critiques.md) § Accessibility.

**Visibly missing vs the Buiy foundation widget catalog** (per [`../../specs/2026-05-07-buiy-foundation/media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md) § 3.10): Checkbox, Switch, Radio Group, Listbox, Combobox, Slider, Spinbutton, Searchbox, all picker variants (date / time / color / file), Menu, Menubar, Tabs, Toolbar, Breadcrumb, Tree, Treegrid, Dialog (as APG widget), Popover, Tooltip, Disclosure, Window splitter, Progressbar, Meter, Alert, Status, Toast, Carousel. The bundled set covered <10% of the APG-required widget surface a comprehensive UI library needs.

## `rsx!` composition

The `rsx!` macro is the load-bearing authoring surface. Composition is by nesting:

```rust
rsx! {
    <ElementBundle styles={KStyle { background_color: Color::RED.into(), ..default() }}>
        <KButtonBundle on_event={OnEvent::new(handle_click)}>
            <TextWidgetBundle text={"Click".into()} />
        </KButtonBundle>
    </ElementBundle>
}
```

Notable constraints:
- Every tag must be a kayak_ui-known `Bundle` type. Plain Bevy bundles cannot be composed in.
- Attributes set component fields verbatim. There is no analogue of BSN's "set this field on a required-companion-component" pattern.
- Children are positional, not keyed — diffing relies on stable widget identity from spawn ordering. This is the source of the "tree removal issues" the 0.5.0 release notes call out as fixed.

## State + effects

The kayak_ui state model used context-bound hooks:

```rust
fn my_widget(context: &mut KayakRootContext, /* props */) -> bool {
    let state_entity = context.use_state(MyState::default(), widget_entity);
    let on_event = OnEvent::new(move |event_dispatcher, evt| {
        // event handling against state_entity
    });
    rsx! { /* ... */ };
    true
}
```

`OnEvent` was the unified event-callback wrapper, taking a `EventDispatcher` + the event payload. Events distinguished `MouseDown`, `MouseUp`, `Click`, `Hover`, `MouseLeave`, character input, and keyboard input.

The hook model is **not** Rust-native — Rust's borrow checker fundamentally cannot represent React's hooks-rules ("hooks must be called in the same order every render"). kayak_ui worked around this by passing the context explicitly and binding state to widget-entity identity; the ergonomics ranged from "tolerable" (issue #272: "I dont understand why widgets wrapped inside of a ContextProvider need to have a 'computed styles'") to "actively load-bearing for the maintenance complexity" (the woodpecker_ui rewrite cited "*1,000 lines to fewer than 200*" for the primary system as the kayak_ui-replacement justification — see [`history.md`](history.md)).

## Events

The event surface at 0.5.0:

- `EventDispatcher` — the per-`KayakRootContext` event-routing resource.
- `OnEvent` — wrapper for a single widget's event handler.
- `KEvent` — the event payload enum (`MouseDown`, `MouseUp`, `Click`, `Hover`, `MouseLeave`, `CharEvent`, `KeyEvent`, ...).
- Bubbling: events flowed up the kayak_ui widget tree, *not* up the Bevy entity tree.

For Buiy, this two-tree event model is a **don't-do-this**: Buiy events flow as standard Bevy observers along the entity tree (per foundation [`interaction.md`](../../specs/2026-05-07-buiy-foundation/interaction.md)), with no parallel kayak-style "widget tree." A consumer should never have to reason about which tree an event is flowing through.

## What 0.5.0 had at the perimeter

- `KStyle` — the styling struct (CSS-like fields), passed via the `styles={...}` attribute on every widget.
- `KayakUICameraPlugin` + `CameraUIKayak` — camera-side wiring (kayak_ui's UI is camera-attached, similar to bevy_ui's UI rendering).
- `WindowSize` — resource tracking the Bevy window size for kayak_ui's render.
- `MaterialUI` — extension point for custom UI shaders (analogue to bevy_ui's later `UiMaterial`).
- `DEFAULT_FONT` constant = `"Kayak-Default"` — the bundled MSDF font asset.

## Sources

- kayak_ui docs.rs (0.5.0, widget module) — https://docs.rs/kayak_ui/0.5.0/kayak_ui/widgets/index.html
- kayak_ui docs.rs (0.5.0, prelude) — https://docs.rs/kayak_ui/latest/kayak_ui/prelude/index.html
- kayak_ui book chapter 1 (App + rsx! example) — https://github.com/StarArawn/kayak_ui/blob/main/book/src/chapter_1.md
- kayak_ui book chapter 2 — https://github.com/StarArawn/kayak_ui/blob/main/book/src/chapter_2.md
- kayak_ui issue tracker (open issues, accumulated state) — https://github.com/StarArawn/kayak_ui/issues
- WAI-ARIA APG Accordion pattern — https://www.w3.org/WAI/ARIA/apg/patterns/accordion/
- Buiy foundation widget catalog — [`../../specs/2026-05-07-buiy-foundation/media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)
- Buiy foundation interaction spec — [`../../specs/2026-05-07-buiy-foundation/interaction.md`](../../specs/2026-05-07-buiy-foundation/interaction.md)
