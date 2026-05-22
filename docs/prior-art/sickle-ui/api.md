**Date:** 2026-05-22
**Status:** archived
**Subject:** sickle_ui — API: the extension-trait DSL, theming, custom widgets, BSN-compat assessment

# API

sickle_ui's user-facing API is **fluent, trait-dispatched, and runtime-built.** This is the most important API characterization in this corpus, because it determines whether sickle is BSN-compatible (it is not, see § "BSN-compat assessment" below) and whether Buiy can borrow API shape from it (mostly no, with two exceptions).

## The UiBuilder entry point

All UI authoring in sickle starts with one of these calls:

```rust
// At the app root, called from a setup system:
commands.ui_builder(UiRoot)
    .column(|column| {
        column.label(LabelConfig::from("Hello"));
        column.button(...);
    });

// Or attached to a specific existing entity:
commands.ui_builder(some_entity)
    .container(|c| { ... });
```

`Commands::ui_builder(E)` is sickle's extension on Bevy's `Commands`, returning a `UiBuilder<E>`. `E` is either `UiRoot` (the top-of-tree marker) or an `Entity`. The builder carries the commands reference + the parent entity + ambient context; every method on it operates "inside this entity's children scope."

## The extension-trait dispatch pattern

`UiBuilder<Entity>` itself has zero widget-spawn methods. Every method comes from an extension trait per widget:

```rust
// From sickle_ui::widgets::layout::column:
pub trait UiColumnExt {
    fn column(&mut self, spawn_children: impl FnOnce(&mut UiBuilder<Entity>)) -> UiBuilder<Entity>;
}
impl UiColumnExt for UiBuilder<Entity> { ... }

// From sickle_ui::widgets::inputs::checkbox:
pub trait UiCheckboxExt {
    fn checkbox(&mut self, label: Option<&str>, checked: bool) -> UiBuilder<Entity>;
}
impl UiCheckboxExt for UiBuilder<Entity> { ... }
```

The user imports `sickle_ui::prelude::*` (which re-exports 200+ traits per the docs.rs prelude index) and writes `ui.column(|c| c.checkbox(...))`. Rust resolves `.column(...)` because `UiColumnExt` is in scope; resolves `.checkbox(...)` because `UiCheckboxExt` is in scope.

This is mechanically clean: each widget module owns its trait, the prelude re-exports them, the user's code never names the trait explicitly. The cost — and it is severe — is that **the widget vocabulary is not data.** It is a set of method dispatches that Rust resolves at compile time. There is no introspectable list of "widgets sickle knows how to spawn" at runtime, because the answer is "whatever traits are in scope at this call site."

## Composition via `UiBuilderGetId`

`UiBuilderGetId` is the trait that says "I am a typed wrapper around an entity, and you can get my entity id." Anything that implements it gains the full `UiBuilder` method surface by way of a blanket impl. The pattern lets widgets expose typed sub-builders (e.g. `slider.bar()` returns a `UiBuilder<SliderBarEntity>`) so styling can target the inner entities without losing type-safety. From the docs:

> as long as the type has a way of returning its own entity Id, all methods implemented on the UiBuilder becomes available.

This is the one piece of the API surface that is genuinely composable in the Rust sense — it cleanly delegates the trait surface to any wrapper that knows its entity id.

## The style-setter trait family — `UiStyleExt` and `Set<Property>Ext`

Beyond spawn methods, sickle exposes a parallel family of trait-methods for *styling* the just-spawned entity:

```rust
ui.button(...).style()
    .background_color(BUTTON_COLOR)
    .padding(UiRect::all(Val::Px(8.0)))
    .border_radius(BorderRadius::all(Val::Px(4.0)));
```

`.style()` returns a `UiStyle`-builder. Each `.background_color(...)` / `.padding(...)` / `.border_radius(...)` method comes from a separate `Set<Property>Ext` trait — there is one per CSS-like property. Per the docs.rs `prelude` index, this is the bulk of the 200+ traits the prelude re-exports.

The `UiStyleUnchecked` variant (and `UiStyleUncheckedExt`) is the bypass for `LockedStyleAttributes` — when a widget marks a property as locked (so it can't be accidentally overridden), unchecked styling lets the caller force the override. The lock-and-override pattern is sickle's answer to "the widget owns its visual identity, but the app wants to escape."

## Theming — `Theme<C>` and `PseudoTheme<C>`

A theme in sickle is a runtime data structure keyed by component type and pseudo-state list. Defining a theme for a custom widget `MyWidget`:

```rust
// 1. Declare the widget component.
#[derive(Component, DefaultTheme, UiContext)]
pub struct MyWidget;

// 2. Implement DefaultTheme to provide the baseline:
impl DefaultTheme for MyWidget {
    fn default_theme() -> Option<Theme<MyWidget>> {
        let base = PseudoTheme::deferred(None, |style: &mut StyleBuilder, theme: &ThemeData| {
            style.background_color(theme.colors.surface);
        });
        let hovered = PseudoTheme::deferred(Some(vec![PseudoState::Hover]), |s, t| {
            s.background_color(t.colors.surface_hover);
        });
        Some(Theme::new(vec![base, hovered]))
    }
}

// 3. Register the theme plugin.
app.add_plugins(ComponentThemePlugin::<MyWidget>::default());
```

The `PseudoTheme` closures receive the live `ThemeData` (theme tokens — colors, spacing, typography) and a `StyleBuilder` they populate. The engine resolves them lazily when `PseudoStates` change on an entity holding `MyWidget`.

The `UiContext` derive lets the widget expose named sub-entity slots. Implementing `fn get(&self, target: &str) -> Result<Entity, _>` lets a theme target `"my_widget.bar"` or `"my_widget.handle"` and the theme engine resolves it. This is sickle's analog of CSS selectors targeting `.bar` inside `.widget`.

## Creating a custom widget — the four-tier ladder

The README (verified against the surviving fork) describes four ways an app extends sickle's vocabulary, in increasing complexity:

1. **Structural widgets** — define a marker component, write a `UiMyWidgetExt::my_widget(...)` extension trait that spawns a styled bundle of `bevy_ui::Node` + the marker, register it with the user's app. No theming integration; the widget is a styled `Node`.
2. **Functional extensions** — same as 1, plus the widget participates in `FluxInteraction` and `DynamicStyle`. Hover / press / disabled visuals work automatically.
3. **Themed components** — same as 2, plus `DefaultTheme` is implemented and the widget reads color/spacing/typography from the theme registry. Theme switching at runtime works.
4. **Contextually-themed widgets** — same as 3, plus the widget owns sub-entities (a slider's bar, handle, label) and implements `UiContext` so themes can target sub-entities. The pattern requires the most boilerplate but yields per-slot theming.

This ladder is the cleanest part of the sickle design — it explicitly stages how much theming machinery a widget opts into, rather than forcing all widgets through the heaviest path. Buiy's widget-catalog sub-spec could mirror this staged opt-in.

## Derive macros — what `sickle_macros` provides

Four derive macros surface in `sickle_ui::prelude`:

- **`DefaultTheme`** — auto-implements `DefaultTheme` for the widget by reading a `#[default_theme(...)]` attribute (or returns `None`, meaning the user provides the theme manually).
- **`EventHandler`** — registers a Bevy one-shot system as the handler for a widget event. Sugar for the boilerplate of wiring up `Trigger`s.
- **`StyleCommands`** — generates the `Set<Property>Ext` traits for a list of style properties. This is what `sickle_ui` itself uses internally to produce the ~150 generated style traits in `ui_style::generated`.
- **`UiContext`** — implements `UiContext::get(&str) -> Entity` based on `#[ui_context(...)]` field annotations on a struct.

The `StyleCommands` macro is the leverage point that lets sickle express 200+ style-setter traits without hand-writing each.

## BSN-compat assessment

**Verdict:** sickle's API is **not BSN-compatible** and structurally cannot be made so without rewriting the authoring surface.

BSN (the proposed Bevy Scene Notation, PR [#20158](https://github.com/bevyengine/bevy/pull/20158), still draft as of 2026-05-22 per [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)) is a *data* format that names components and their fields. Authoring a UI in BSN means writing:

```text
Column {
    Label { text: "Hello" },
    Button { label: "Click me", on_click: ... },
}
```

…where `Column`, `Label`, `Button` are component types and the nested syntax is the entity hierarchy. The hot-reload story requires BSN to be parseable, diffable, and reload-applicable without invoking arbitrary Rust code.

sickle's authoring pattern is the opposite:

- **Widget vocabulary is methods, not components.** Calling `ui.button(...)` is dispatched through `UiButtonExt`. There is no `Button` component you can mention in BSN that would produce the same entity shape — the `Button` marker exists, but its spawn-time configuration (children, style, event wiring) lives in the trait method.
- **Style is method-chained.** `.style().background_color(...).padding(...)` is a sequence of trait method calls; BSN has no equivalent.
- **Themes are runtime closures.** `PseudoTheme::deferred(|s, t| { ... })` carries a Rust closure that captures the theme data and writes into a `StyleBuilder`. BSN cannot serialize a Rust closure.
- **`DynamicStyle` is constructed at runtime** from builder calls; the on-entity component is opaque from outside.

The only sickle pattern that **would** be BSN-friendly, if rewritten, is the **widget-marker component**. `Button`, `Slider`, `Checkbox` (the markers) could be authored in BSN if the spawn-time configuration was migrated from trait-arguments to companion components. For example: `Slider` (marker) + `SliderValue { value: 0.5, min: 0.0, max: 1.0 }` + `SliderAxis::Horizontal` (component, not enum-variant-in-config). That is the BSN-friendly shape Buiy's widget catalog commits to.

The deeper issue is that sickle's DSL was designed when BSN did not exist as a target. The extension-trait approach is excellent ergonomics for Rust-authored UI; it is structurally hostile to data-authored UI. A library starting today, knowing BSN is in flight, would design differently. Buiy's foundation explicitly designs differently — see [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.4 (component-first authoring).

## Implications for Buiy

1. **Borrow:** the **four-tier widget extension ladder** (structural / functional / themed / contextually-themed). It cleanly stages how much theming machinery a widget opts into. Buiy's widget-catalog sub-spec should adopt this staging.
2. **Borrow:** the **`UiContext::get(name) -> Entity` pattern** for naming sub-entities. Even in a BSN-authored world, themes need a way to target sub-parts of a composite widget; named-slot lookup is a clean expression of that.
3. **Avoid:** **trait-dispatched spawn methods.** Buiy widgets must be components, period. The spawn helpers (if any) are sugar over component insertion, not the primary surface.
4. **Avoid:** **closure-carrying theme definitions.** Buiy's theming sub-spec (foundation [architecture.md § 2.5](../../specs/2026-05-07-buiy-foundation/architecture.md)) commits to **theme assets** (declarative, hot-reloadable, BSN-style) rather than runtime closures. sickle's `PseudoTheme::deferred(|s, t| { ... })` is exactly the shape Buiy rules out.
5. **Avoid:** the **200+ extension-trait prelude.** The discoverability cost is real — IDE autocomplete on `ui.` returns the entire union of widget-spawn-traits in scope, with no semantic grouping. Buiy's surface should be small and component-typed.

## Sources

- docs.rs prelude index — https://docs.rs/sickle_ui/0.4.0/sickle_ui/prelude/index.html
- `ui_builder` — https://docs.rs/sickle_ui/0.4.0/sickle_ui/ui_builder/index.html
- `ui_style` — https://docs.rs/sickle_ui/0.4.0/sickle_ui/ui_style/index.html
- `theme` — https://docs.rs/sickle_ui/0.4.0/sickle_ui/theme/index.html
- `theme::pseudo_state` — https://docs.rs/sickle_ui/0.4.0/sickle_ui/theme/pseudo_state/index.html
- `theme::dynamic_style` — https://docs.rs/sickle_ui/0.4.0/sickle_ui/theme/dynamic_style/index.html
- Surviving fork README (widget-ladder description) — https://github.com/UkoeHB/sickle_ui
- BSN tracking PR — https://github.com/bevyengine/bevy/pull/20158
- Buiy foundation architecture (BSN-friendly rule) — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- bevy_ui lessons (BSN landing status) — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
