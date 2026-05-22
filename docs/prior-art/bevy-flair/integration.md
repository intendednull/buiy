**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_flair — integration: plugin setup, coexistence with programmatic styling, bevy_feathers interaction, per-frame cost

# Integration

How bevy_flair plugs into a Bevy app, what it coexists with cleanly, what it doesn't.

## 1. Plugin setup

The minimum:

```rust
App::new()
    .add_plugins((DefaultPlugins, FlairPlugin))
    .add_systems(Startup, setup)
    .run();
```

`FlairPlugin` is composed in this order (verified in [`architecture.md`](architecture.md) § 3):

```
PropertyRegistryPlugin → FlairStylePlugin → FlairCssParserPlugin
```

If the app needs custom component-property mappings (CSS name ↔ Bevy component field), they must be registered *before* `FlairPlugin`:

```rust
app
    .register_component_properties::<MyCustomComponent>()
    .add_plugins(FlairPlugin);
```

## 2. Asset directory layout

Stylesheets live under `assets/` like any other Bevy asset:

```
assets/
├── ui/
│   ├── menu.css
│   ├── menu-imports/
│   │   ├── _tokens.css
│   │   └── _typography.css
│   └── fonts/
│       └── Poppins-Regular.ttf
```

Inside `menu.css`:

```css
@import url("menu-imports/_tokens.css");
@import url("menu-imports/_typography.css");

:root {
  --color-bg: oklch(0.95 0.02 240);
  --color-fg: oklch(0.15 0.02 240);
}

button {
  background-color: var(--color-bg);
  color: var(--color-fg);
  padding: 8px 16px;
  border-radius: 4px;
  transition: background-color 200ms ease;
}

button:hover {
  background-color: oklch(0.92 0.04 240);
}
```

The actual `examples/game_menu.rs` ships ~120 lines of CSS exercising flex layout, custom properties, hover, transitions, fonts, and gradients. The example is the de facto reference for what a working `bevy_flair`-styled app looks like.

## 3. Coexistence with programmatic `bevy_ui` styling

This is the load-bearing integration question.

**Rule of thumb:** for any given component on any given entity, pick one source of truth — bevy_flair *or* programmatic — not both.

The mechanics (per [`api.md`](api.md) § 6):

- bevy_flair re-runs the cascade every `PostUpdate`. Every matched property writes a value to its mapped component field.
- A programmatic `commands.entity(e).insert(BackgroundColor(red))` runs in `Update`. The next `PostUpdate` cascades; if any rule writes `background-color` to that entity, the user write is clobbered.
- A programmatic write of a component that **no** rule touches on that entity is preserved — bevy_flair only clobbers what it manages.
- Components managed by bevy_flair (because rules write to them) are auto-removed when no rule matches anymore (0.6 behavior). If user code inserted `BorderColor(red)` and a rule transiently matched, then stopped matching, the component vanishes — possibly surprising.

The practical patterns:

- **Static visuals via CSS, dynamic state via code.** Hover, focus, layout, fonts, colors live in CSS. Per-entity dynamic data (current HP bar fill, currently-selected item highlight) lives in code via components bevy_flair doesn't mention.
- **Don't write CSS rules for ephemeral state.** A short-lived effect like "flash green on success" is better as an Bevy animation or direct component mutation than a `@keyframes` toggled via a class change — class changes in bevy_flair require swapping a component on the entity each frame the class is "active."

## 4. bevy_feathers and bevy_ui_widgets interaction

bevy_feathers is Bevy's official widget kit on top of bevy_ui. bevy_flair's README **does not mention bevy_feathers**, and the examples directory contains no `feathers_demo.rs` or similar. From inspection:

- bevy_feathers widgets ship with their own programmatic styling (per-widget `FeathersTheme` resource, color tokens, hardcoded defaults). They are **megacomponent-flavored** in the same way `bevy_a11y::AccessibilityNode` was — widget visuals are wired internally, not exposed as separate stylable components.
- Applying a `Styled` ancestor on top of a bevy_feathers widget *will* style the underlying `bevy_ui` nodes (button background, padding) because those are still standard `BackgroundColor` / `Node` components. But it **will not** retheme the parts of the widget that bevy_feathers handles internally (e.g. the disclosure arrow on a `bevy_feathers::Disclosure`).
- **There is no documented "use bevy_flair to retheme bevy_feathers" workflow.** The two crates exist in parallel.

For Buiy: this is a real warning. bevy_feathers's coupling between widget logic and widget visuals is the same anti-pattern bevy_a11y had with `AccessibilityNode` (cf [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § Avoid). If Buiy ever ships both widgets *and* a stylesheet layer, the widgets must be **decomposed-component-friendly** end-to-end — every styleable surface as its own component — or the stylesheet layer will only paint over the *exterior* of widgets, leaving the interior frozen.

## 5. AccessKit / a11y interaction

bevy_flair does not touch the AccessKit tree. It cascades visual + layout properties only. Forced-colors mode (the `@media (forced-colors)` query) is **not in the CHANGELOG** — meaning bevy_flair likely does not honor OS forced-colors automatically, and an app that wants WCAG forced-colors compliance must implement it manually (e.g. swap stylesheets at startup based on the OS pref).

For Buiy this is a gap. Foundation [accessibility.md](../../specs/2026-05-07-buiy-foundation/accessibility.md) requires forced-colors / prefers-contrast / prefers-color-scheme / prefers-reduced-motion to flow from OS prefs into theme variants automatically. A Buiy stylesheet layer over bevy_flair-shaped infrastructure would need to extend `@media` to bind those OS prefs to `UserPreferences`-resource-driven media queries.

## 6. Performance

bevy_flair commits to per-frame style resolution, but with marker-driven recalculation:

- Entities not marked by `MarkEntitiesForRecalculation` skip the `CalculateStyles` work for that frame.
- Marker bits flip when: a `Styled` root's asset changes (hot-reload), an ancestor changes class/state, a sibling is added/removed (for `:nth-child` re-evaluation, 0.2 fix), pseudo-state flips on the entity.
- The README markets this as: `"efficient and reactive when applying styles, with no unnecessary style re-application if the UI tree hasn't changed."`

**Benchmarks:** the workspace has a `benches/` directory; specific results are not published in the README or CHANGELOG. There are **no published numbers for "1000-node UI selector match cost per frame"** — the same gap [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) flags for bevy_ui itself.

For Buiy: do not adopt bevy_flair's per-frame model without bench numbers at productivity-app node counts (1000+) and at game-HUD pseudo-state churn rates (button hover/unhover at 60Hz across 20+ buttons). The Servo `selectors` crate is fast (production browser engines use it), but the *bridge* between Bevy entities and the selector engine is bevy_flair's code, and its cost at scale is unverified.

## 7. Cross-window

A `Styled` component is per-entity; a multi-window app spawns one `Styled` root per window. Stylesheets are `Asset<StyleSheet>` and shareable across windows by handle. Nothing in the CHANGELOG suggests cross-window-specific issues; nothing suggests cross-window-specific features either. Pseudo-state propagation across windows depends on `bevy_input_focus` semantics, which themselves are per-window.

## 8. WASM

`selectors` 0.32 supports WASM (0.4 fix when the upstream crate stopped requiring `getrandom` for tests). bevy_flair is reportedly WASM-compatible from 0.4 onward, though there are no published WASM demos in `examples/`. Buiy's WASM-target stance is itself an open question ([README.md § 5](../../specs/2026-05-07-buiy-foundation/README.md#5-open-questions)) — bevy_flair shipping on WASM is a useful precedent.

## 9. Summary integration matrix

| Concern | bevy_flair stance |
|---|---|
| Plugin install | `app.add_plugins(FlairPlugin)` — one line. |
| Asset directory | Standard `assets/` Bevy convention. |
| Programmatic + CSS on same component | **Do not.** CSS clobbers programmatic on `PostUpdate`. |
| bevy_feathers integration | **Undocumented, partial at best.** CSS reaches the bevy_ui surface, not the widget interior. |
| AccessKit / forced-colors | **Not handled.** App-side responsibility. |
| Hot-reload | First-class. |
| Multi-window | Works (per-root `Styled`). |
| WASM | Works since 0.4; not regularly demoed. |
| Per-frame cost | Marker-driven recalculation; no published benches at scale. |
| Coexistence with `Buiy` | Hypothetical only — bevy_flair binds to bevy_ui's component types, Buiy has its own component types. A Buiy adaptation would be a fork, not a drop-in. |

## Sources

- bevy_flair `examples/game_menu.rs` — https://github.com/eckz/bevy_flair/blob/main/examples/game_menu.rs
- bevy_flair README "Limitations" + non-goals — https://github.com/eckz/bevy_flair/blob/main/README.md
- CHANGELOG — https://github.com/eckz/bevy_flair/blob/main/CHANGELOG.md
- bevy_feathers prior-art — [`../bevy-feathers/`](../bevy-feathers/)
- bevy_ui prior-art (megacomponent anti-pattern) — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
- Sibling: [`api.md`](api.md), [`critiques.md`](critiques.md)
