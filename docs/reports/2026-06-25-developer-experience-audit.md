**Date:** 2026-06-25
**Status:** active

# Buiy — Developer Experience Audit

One-shot audit of what it looks like to *use* Buiy as an application developer today —
the authoring surface, the styling/state/event model, and the friction a new user hits.
Produced as the grounding input to a prior-art research pass + a brainstorm on a better
public interface (see the companion research report
`2026-06-25-ui-dx-composition-prior-art.md` and any resulting `-interface-design` spec).

Every code block is a real excerpt from the tree or the README at main `8125ba2`; every
friction item carries a `file:line` reference. Method: an 8-way parallel read of the usage
surface (first-touch/docs, bootstrap, imperative authoring, `bsn!`, styling/theming, widget
state API, the gallery TodoMVC, footguns), each grounded in real files, then synthesized.
Load-bearing claims were spot-checked by hand (the magenta-shadow token, the a11y-shaped
state reads, the single `OnPress` sink).

---

## 1. What using Buiy looks like (a guided tour)

### (a) The minimal app — first touch

A new user copies the README "Quick start." The Rust is accurate and `hello_button` mirrors it:

```rust
// examples/hello_button/src/main.rs
use bevy::prelude::*;
use buiy::*;

fn main() {
    App::new()
        .add_plugins(DefaultPlugins)
        .add_plugins(BuiyPlugin)        // composes layout, render, text, a11y, focus, picking, widgets
        .add_systems(Startup, setup)
        .add_systems(Update, log_press)
        .run();
}

fn setup(mut commands: Commands) {
    commands.spawn(Camera2d);
    commands.spawn(Button::new("Save"));
}

fn log_press(mut events: MessageReader<OnPress>) {
    for ev in events.read() { info!("button pressed: {:?}", ev.0); }
}
```

But the `Cargo.toml` directly above it **does not build**:

```toml
# README.md:64 — the broken pin
bevy = "0.18"
buiy = { git = "https://github.com/intendednull/buiy" }
```

`buiy` needs `bevy = "0.19.0-rc.3"`, so `bevy = "0.18"` next to the git dep is an unresolvable
two-major conflict. The literal first copy-paste fails before any Buiy code runs. Behind the
happy path sit three rules enforced only by prose and surfaced only as a panic or a blank
window: **BuiyPlugin must come after DefaultPlugins** (else a `PipelineCache` panic),
**MinimalPlugins needs `InputPlugin` added by hand**, and **every app must spawn its own
`Camera2d`** — forget it and you get a blank window, no error.

### (b) Imperative authoring, including raw text

Spawning a packaged widget is a true one-liner. One line of themed text is a 4-tuple:

```rust
// examples/hello_text/src/main.rs — no text("…") constructor to match button("…")
let title = commands
    .spawn((Node, Style::default(), Text(String::from("Hello, Buiy text!")), FontSize(32.0)))
    .id();
```

And a parent-with-children is a manual entity-handle dance, even though a `children![]` macro
exists internally:

```rust
// examples/hello_text/src/main.rs — the .id() + .add_children() ceremony
let title = commands.spawn((Node, Style::default(), Text(...), FontSize(32.0))).id();
let body  = commands.spawn((Node, Style::default(), Text(...), FontSize(16.0))).id();
commands
    .spawn((Node, Style::default().flex_column().width_px(560.0).padding(24.0).gap_px(12.0)))
    .add_children(&[title, body]);
```

The fluent `Style` chain is the pleasant part; the spawn-to-`let`, capture `.id()`, then
`.add_children(&[…])` is the ceremony, and nesting depth grows linearly in `let` bindings.

### (c) `bsn!` declarative authoring

`bsn!` is a thin re-export of Bevy 0.19's macro — no HTML-like DSL. Widget composition is clean;
raw layout is heavy:

```rust
// examples/hello_bsn/src/lib.rs — the headline showcase scene
bsn! {
    #Toolbar
    Node
    template_value(Display::flex_column())
    FlexParams { direction: FlexAxis::Column, gap: { FlexGap { row: Length::px(8.0), column: Length::px(8.0) } } }
    BoxModel { width: { Sizing::Length(Length::Px(340.0)) }, padding: { Edges::all(12.0) } }
    Background { color: { ColorToken::Token("color.surface.primary".into()) } }
    Children [
        (#Search text_input_single_line("Search…")),
        (#Actions Node template_value(Display::flex_row())
         FlexParams { direction: FlexAxis::Row, gap: { FlexGap { row: Length::px(8.0), column: Length::px(8.0) } } }
         BoxModel { width: { Sizing::Length(Length::Px(320.0)) } }
         Children [
            (#Save button("Save") BoxModel { width: { Sizing::Length(Length::Px(140.0)) } }),
            (#Cancel button("Cancel")),
         ]),
    ]
}
```

The whole spectrum in one block: clean widget composition (`button("Save")`,
`text_input_single_line`, `#Name` tags, `Children [ … ]`) next to heavy raw layout — the flex
axis spelled **twice** (`Display::flex_column()` *and* `FlexParams.direction`),
`{ Sizing::Length(Length::Px(340.0)) }` three-name towers in expression-patch braces, and
stringly-typed `"color.surface.primary"`.

Why widgets must be authored through scene-fns, not bare markers — the §4.1c landmine:

```rust
// crates/buiy_widgets/src/scene.rs — re-spell each field so a user's outer patch MERGES
pub fn button(label: impl Into<String>) -> impl Scene {
    bsn! {
        Button
        BoxModel { width: { bm.width }, height: { bm.height }, padding: { bm.padding } }
        Background { color: { bg.color } }
        ...
    }
}
```

Writing the intuitive `bsn! { Button BoxModel { width } }` instead **suppresses** the
`#[require(BoxModel = …)]` initializer entirely — the patch lands on a plain `Default` and the
button's padding is silently dropped. Correct and wrong paths differ by one token; both compile
and run.

### (d) Styling & theming

Layout flows through the fluent builder. Everything *visual* lives in separate render components
`Style` never touches, referenced by free-form strings:

```rust
// crates/buiy_core/src/theme.rs — the theme is flat string-keyed maps
pub struct Theme { pub colors: HashMap<String, Color>, pub spaces: HashMap<String, f32>, pub radii: HashMap<String, f32> }
t.colors.insert("color.surface.primary".into(), Color::WHITE);
```

```rust
// crates/buiy_core/src/render/color.rs — the only safety net is magenta + a warn
None => { tracing::warn!(token = %name, "missing theme color token; falling back to magenta sentinel"); MISSING_TOKEN_FALLBACK }
```

The token namespace is opaque map keys — nothing the compiler checks `ColorToken::Token("…")`
against. **A typo compiles and ships as magenta on screen — already live in the flagship gallery:
`"color.shadow.card"` is registered in no theme** (`examples/buiy_gallery/src/lib.rs:1160`). Even
the framework's own button hardcodes `8.0`/`6.0` with `TODO`s naming the `space.2`/`radius.md`
tokens it isn't resolving.

### (e) The widget catalog — reading state & handling events

13 widget markers ship. The entire widget event surface is one Message carrying only an entity:

```rust
// crates/buiy_core/src/interaction.rs
pub struct OnPress(pub Entity);
```

One event type, shared by buttons, checkboxes, switches, disclosures, menu items. No kind, no
value, no payload — and no per-widget change events (`CheckedChanged`/`ValueChanged` don't exist;
only text emits `TextChanged`). So "which widget fired" means pre-tagging each one and re-querying:

```rust
// examples/buiy_gallery/src/lib.rs — demux one global stream by re-query
for OnPress(e) in reader.read() {
    let Ok((is_destroy, is_clear, fb)) = kinds.get(*e) else { continue };
    if is_destroy { ... } else if is_clear { ... } else if let Some(fb) = fb { filter.0 = fb.0; }
}
```

And widget state is exposed only through accessibility components + foreign `accesskit` enums —
there is no `Checkbox::checked()`:

```rust
// gallery — "is this checkbox checked?" / "set it"
let completed = toggled.0 == Toggled::True;                       // read: &A11yToggled
if let Some(mut t) = world.get_mut::<A11yToggled>(checkbox) { t.0 = Toggled::True; }  // write
```

The accessibility tree *is* the state model — it leaks directly into app logic. The constructor
surface is also uneven: `Slider::new("L", 0.5, 0.0, 1.0, 0.1)` is five positional `f64`s, the
dialog invoker is a free function, `Popover::anchored_to` returns `Self` not `impl Bundle`, and
`ScrollArea` has no constructor at all.

### (f) A real app — the gallery TodoMVC

A literal TodoMVC needs 1 plugin, 7 chained systems, 1 observer, a 5-field staging resource, ~17
marker components, and hand-rolled tree-walk helpers:

```rust
// examples/buiy_gallery/src/lib.rs — the schedule the app must thread by hand
.add_systems(Update,
    (collect_add_submit, collect_button_press, collect_edit_submit,
     apply_intents, apply_filter, update_count, restyle_completed)
        .chain().after(BuiySet::Input).before(BuiySet::A11yUpdate));  // omit → a11y tree one frame stale
```

State *is* the widget tree, so derived values get recomputed by re-walking every row every frame,
and every change is written twice — to visible pixels and to the a11y mirror:

```rust
// gallery update_count — recount by full re-walk; dual-write A11yLabel + Text
let remaining = rows.iter().filter(|c| !row_completed(c, &checkboxes)).count();
if label.0 != phrase { label.0 = phrase.clone(); }                // announced name
if text.0 != phrase { text.0 = phrase.clone(); }                  // visible pixels
```

There's no app-side model and no id cache, so `row_completed` is re-invoked by three systems each
re-traversing all rows every Update. Dynamic rows can't live in a `bsn!` scene at all, so
`screen_todomvc(seeds)` takes its seed param and immediately discards it (`let _ = seeds;`); rows
are spawned imperatively with app-defined `find_single`/`child_with`/`ancestor_row` helpers the
library doesn't ship.

---

## 2. The three authoring idioms, side by side

| | `spawn` + constructors | Raw component tuples | `bsn!` declarative |
|---|---|---|---|
| **Looks like** | `spawn(Button::new("Save"))` | `spawn((Node, Style::default(), Text(...), FontSize(32.0)))` | `bsn! { button("Save") }` → `spawn_scene(...)` |
| **Returns** | `impl Bundle` | tuple bundle | `impl Scene` |
| **Best for** | dropping in a widget | one-off layout / plain text | static trees |
| **Styling** | fluent `Style` builder | fluent `Style` builder | `Style` **unavailable** — hand-spell `Display`+`FlexParams`+`BoxModel`+`Background` |
| **Children** | manual `.id()` + `.add_children` | manual `.id()` + `.add_children` | nested `Children [ … ]` |
| **Per-widget name** | `Button::new(label)` | `Button` marker (label-less) | `button(label)` scene-fn |

They coexist inconsistently:

- **A single widget has up to four spellings:** bare marker `Button`, `Button::new(label)`
  (`impl Bundle`), `button(label)` (`impl Scene`), and the suppression-trap
  `bsn! { Button BoxModel {…} }`.
- **Constructor naming is non-uniform:** `::new` (Button, Checkbox) vs `::single_line`/`::multi_line`
  (TextInput); all `::new`s are `#[allow(clippy::new_ret_no_self)]`.
- **Scene-fn names are import-path-dependent:** `checkbox()` collides with the `Checkbox` marker,
  so `buiy_widgets` exports it as `checkbox_scene` and the prelude renames it back. Same function,
  two names depending on which crate you import.
- **The nice builder is steered away from:** the library points users toward `bsn!`, the one path
  where the fluent `Style` builder is unavailable.
- **Import convention is unsettled:** the three examples use `use buiy::*;`, `use buiy::BuiyPlugin;`,
  and `use buiy::prelude::*;` — and the prelude is literally `pub use crate::*`, so two are identical.

---

## 3. Friction inventory (worst first)

### Theme A — Silent-wrong footguns (the sharpest edges)

| Issue | Evidence | Sev |
|---|---|---|
| `bsn! { Button BoxModel { width } }` (the most intuitive thing to write) suppresses the `#[require]` initializer; padding silently dropped. Wrong/right path differ by one token, no diagnostic. | `scene.rs:5-10`; no `warn!`/`panic!` in widgets/bsn | **high** |
| Quick-start `Cargo.toml` pins `bevy = "0.18"` against a `buiy` needing `0.19.0-rc.3`. First copy-paste fails. | `README.md:64` | **high** |
| Typo'd theme token compiles and ships as magenta. Live in the gallery: `"color.shadow.card"` in no theme. | `gallery:1160`; `color.rs:116-153` | **high** |
| Flex axis set twice (`Display::flex_column()` + `FlexParams.direction`); can silently diverge. | `hello_bsn:42-46` | **high** |
| Forgetting `Camera2d` fails silently — blank window, no error, not in BuiyPlugin docs. | all examples; `buiy/src/lib.rs:123-181` | **medium** |
| `EventReader<OnPress>` compiles and reads nothing — `OnPress` is a `Message`. Only a comment guards it. | `hello_button:5-7` | low |
| `BUIY_GALLERY_SCREEN=scrol` silently runs todomvc (catch-all `_ =>`). | `gallery main:39-60` | low |
| Wrong gap setter is a silent no-op (`gap_px` vs `grid_gap_px`). | `style.rs:198-204` | low |

### Theme B — State model is accessibility-shaped (the deepest structural friction)

| Issue | Evidence | Sev |
|---|---|---|
| All observable widget state lives on a11y-named components + foreign `accesskit` enums. No `Checkbox::checked()`/`Slider::value()`. Read `&A11yToggled` vs `Toggled::True`; write `world.get_mut::<A11yToggled>`. | `checkbox.rs:67-78`; `gallery:589,354-357` | **high** |
| No per-widget value-changed event. Only text has `TextChanged`. Apps run own `Changed<A11y…>` or re-walk. | grep `ValueChanged` empty | **high** |
| `OnPress(Entity)` is a single shared sink across 5 widget kinds; identity is the dev's problem. No callback/closure/typed event. | `interaction.rs:30`; `gallery:387-394` | **high** |
| No two-way binding, no app-facing setters — all manual ECS mutate; clearing a text input routes through the a11y action router by `node_id`. | `gallery:355-356`; `inprocess.rs:434-449` | medium |
| App keeps visible + a11y reps in lock-step by hand (same value into `Text` and `A11yLabel`, `CssVisibility` and `A11yHidden`). | `gallery:564-574,506-512` | medium |

### Theme C — Verbosity & ceremony

| Issue | Evidence | Sev |
|---|---|---|
| `Style` is `#[derive(Bundle)]`, not a Component → not `bsn!`-authorable; every container hand-spells `Node`+`Display`+`FlexParams`+`BoxModel`. | `buiy_bsn:36`; `style.rs:49` | **high** |
| One line of themed text = a 4-tuple; no `text("…")` constructor. | `hello_text:24-31` | medium |
| Container-with-children = manual `.id()`+`.add_children`; `children![]` exists but only inside widget constructors. | `hello_text:24-55` | medium |
| `bsn!` values are nested towers: `width: { Sizing::Length(Length::Px(340.0)) }`. | `hello_bsn:48-51` | medium |
| Visual styling fragmented across `Background`/`Border`/`BoxShadow`/`TextColor`, none on `Style`. No `.background()`/`.radius()`. | `style.rs:50-65` | medium |
| Space/radius tokens not wired into builder or paint; raw f32 everywhere; button hardcodes `8.0`/`6.0`. | `button.rs:82-103` | medium |
| `Node`+`Style::default()` both named though `Node` already requires the Style decomposition (redundant). | `hello_text:25-27` | low |
| `FlexGap` has no `::all(8.0)` shorthand; `Length::Px` vs `Length::px` mixed in one block. | `hello_bsn:45,48` | low |

### Theme D — Boilerplate in real apps

| Issue | Evidence | Sev |
|---|---|---|
| Widgets addressable only by component → an app invents a marker struct per role (gallery declares ~17) + a `#Name` per node. | `gallery:96-148` | medium |
| No tree-lookup helpers shipped; each app **and each test** re-implements `find_single`/`child_with`/`ancestor_row` (identical `child_with<C>` re-declared in 3 test files). | `gallery:612-717`; widget tests | medium |
| Dynamic content can't live in a `Scene`; `screen_*` fns take seed/count params they discard (`let _ = seeds;`). | `gallery:199,756,295-299` | medium |
| Correct behavior needs slotting app systems into Buiy's internal schedule sets by hand; wrong → one-frame-stale tree, not a compile error. | `gallery:336-337` | medium |
| Locating a checkbox label couples app logic to the widget's private child structure (`CheckboxMark`). | `gallery:632-638` | medium |
| App reaches past the prelude into `buiy_core`/`buiy_widgets` internals and depends on all three crates. | `gallery:77-85` | low |

### Theme E — Plugin-ordering & bootstrap footguns (documentation-only rules)

| Issue | Evidence | Sev |
|---|---|---|
| `BuiyPlugin` must be added after `DefaultPlugins` or panic on missing `PipelineCache`; type doesn't enforce; failure is a generic Bevy panic, no Buiy hint. | `buiy/src/lib.rs:174-181` | **high** |
| `MinimalPlugins` needs hand-added `InputPlugin` + `WindowPlugin`-gated `PointerInputPlugin`; raw Bevy panics deep in internals. | `buiy/src/lib.rs:146-149,213-217` | medium |
| `TransformPlugin` is a conditional requirement with a frame-delayed, hard-to-attribute failure. | `buiy/src/lib.rs:159-166` | low |

### Theme F — Namespace & discoverability

| Issue | Evidence | Sev |
|---|---|---|
| `use buiy::prelude::*;` is a firehose: `pub use crate::*` over 200+ names + `buiy_bsn::prelude::*` + a module. No curated subset. | `buiy/src/lib.rs:113-115` | medium |
| Constructor return types diverge from names (`new` → `impl Bundle`); no single discoverable "how do I make a checkbox." | `button.rs:115`, `scene.rs:90` | medium |
| Two `Scroll` types collide (`buiy::events::Scroll` wheel event vs layout `Scroll` overflow). | `buiy/src/lib.rs:93-97` | low |

---

## 4. Doc-vs-reality gaps (checklist)

- [ ] **Broken dependency pin.** Quick-start says `bevy = "0.18"`, Requirements says `0.19`, manifest is `0.19.0-rc.3` — three versions; neither `0.18` nor bare `0.19` resolves.
- [ ] **Widget count under-sold.** README calls the catalog "just two primitives — Button and TextInput"; **13 widget types + 14 BSN scene-fns ship.**
- [ ] **Demos list incomplete.** README advertises two demos; `examples/` has four + capture — the flagship 5-screen `buiy_gallery` and `hello_bsn` are invisible.
- [ ] **Stale "Bevy 0.18" references** survive in the crate-root doc and `hello_button` header.
- [ ] **README "run all checks" omits `--locked`** (and the `-j2` caveat) that CI runs.

---

## 5. Headline takeaways (what a brainstorm should target)

- **The state model is the accessibility tree.** Apps read/write business state through
  `A11yToggled`/`A11yValue`/`A11yExpanded` + foreign `accesskit` enums; no domain layer on top.
- **One untyped `OnPress(Entity)` sink for five widget kinds, no change events except text.**
  Identity, value, and "what changed" are all reconstructed by the developer via pre-tagging +
  per-frame re-query.
- **Silent-wrong is the default failure mode.** `#[require]` suppression, typo'd token, forgotten
  camera, wrong gap setter, `EventReader` vs `MessageReader`, misspelled env var — all compile and
  run, then misbehave with no diagnostic. The loud warn-once discipline in the layout/text core
  doesn't reach the widget/BSN layer.
- **The nicest API is unavailable on the path users are steered toward.** The fluent `Style`
  builder is the high point, but it's a `Bundle` not a Component, so `bsn!` can't use it.
- **One widget, up to four spellings, with import-path-dependent names.** No single discoverable
  answer to "how do I make a checkbox."
- **Stringly-typed everything-visual, no compile-time check, half-wired** (colors resolve; spacing
  and radius are hardcoded literals).
- **A real app pays steep retained-mode boilerplate the library doesn't help with** — ~17
  addressing markers, duplicated tree-walk helpers, dual-writes, full-tree re-walks, manual
  schedule ordering, imperative spawning for anything dynamic.
- **First-touch trust breaks before any Buiy code runs** — manifest doesn't resolve, catalog
  under-sold, three contradicting Bevy versions.
