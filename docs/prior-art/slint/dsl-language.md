**Date:** 2026-05-22
**Status:** active
**Subject:** Slint — the `.slint` DSL: syntax, type system, property qualifiers, bindings, callbacks, animations, states; tooling (VSCode + Live Preview + Figma)

# The `.slint` DSL

The `.slint` language is Slint's authoring surface and its product. The DSL is declarative, statically typed, and compiled — every binding's type is inferred or declared at compile time; every property's owner and visibility (`in` / `out` / `in-out`) is explicit; binding expressions must be pure. This file walks the language surface so a Buiy designer evaluating "should we ship a DSL above ECS+BSN?" has a concrete reference for what such a DSL would have to be like.

## Component declaration

A `.slint` source is a sequence of `component` declarations. Components compose:

```slint
import { Button, VerticalBox } from "std-widgets.slint";

export component HelloWorld inherits Window {
    in property <string> name: "World";
    callback greet();

    VerticalBox {
        Text { text: "Hello, " + root.name; }
        Button { text: "Greet"; clicked => { root.greet(); } }
    }
}
```

- `inherits Window` makes the component a top-level window.
- `in property <string> name` declares an input property of type `string` with a default binding.
- `callback greet()` declares an outbound event the host language handles.
- Children are nested directly. `VerticalBox` and `Button` come from the standard widgets module.
- `clicked => { root.greet(); }` connects a child's callback to the parent's.
- `root` is the implicit reference to the enclosing component.

The shape is intentionally QML-flavored. Goffart and Hausmann maintained QtQml at the Qt Company; the family resemblance is by design (see [`history.md`](history.md)).

## Types

Slint's type system is closed (no user-defined generics) and aimed at UI:

- **Primitive**: `bool`, `int`, `float`, `string`.
- **Length**: `length` (logical pixels), `physical-length`, `duration` (milliseconds), `angle`, `percent`.
- **Color**: `color`, `brush` (a brush is a color or gradient).
- **Image**: `image`.
- **Aggregates**: `struct` (named-field record), `enum` (closed set), arrays.
- **Component references**: `[ComponentType]` arrays for ListView etc.

Types are inferred at binding sites; explicit annotation is required at property declarations. Two-way bindings (`<=>`) require type compatibility.

## Property qualifiers

The three qualifiers control direction:

- `in property <T>` — set by the parent / owner; read-only inside the component. The "data flows in" case.
- `out property <T>` — read-only for the parent; set by the component itself (e.g. a `TextInput`'s `text` is `out`). The "data flows out" case.
- `in-out property <T>` — read/write both directions. Useful for form state.

A property with no qualifier defaults to `private` (the component's internal scratch space). The split is the load-bearing API-surface design — components have a clean public-input / public-output / private contract that the compiler enforces.

## Bindings

A property may carry a **binding expression** — a pure expression over other properties:

```slint
property <length> button-width: parent.width / 2 - 10px;
```

The runtime tracks the dependency `button-width depends on parent.width`; when `parent.width` changes, `button-width`'s binding is marked dirty; next access (or layout invalidation) re-evaluates it.

**Purity is enforced.** The compiler refuses bindings that contain callback invocations, mutations, or other side effects. Pure expressions are the only contract; if you want imperative work, use a callback handler (`=>` block).

**Two-way bindings** use `<=>`:

```slint
TextInput { text <=> root.user-input; }
```

`<=>` propagates changes in both directions, with cycle detection.

## Callbacks

Callbacks are the imperative escape hatch. Declared with `callback name(args) -> ReturnType;`; emitted from inside the component; handled by external code or by a parent component's `=>` block.

```slint
component Counter inherits VerticalBox {
    in-out property <int> count: 0;
    callback incremented(int);

    Button {
        text: "Count: " + root.count;
        clicked => {
            root.count += 1;
            root.incremented(root.count);
        }
    }
}
```

- `clicked => { ... }` is the handler syntax.
- Inside a handler, imperative statements are allowed (mutations, callback emissions). Outside handlers (in property bindings), only pure expressions.

**Property-changed callbacks** — added under the experimental gate `SLINT_ENABLE_EXPERIMENTAL_FEATURES=1` — let you react to a specific property changing:

```slint
changed count => { debug("count is now", root.count); }
```

These fire on the next event-loop tick and coalesce — multiple changes within one tick fire one callback.

## Animations and states

Animations are declarative:

```slint
Rectangle {
    background: highlighted ? red : blue;
    animate background { duration: 200ms; easing: ease-in-out; }
}
```

Any property change can be animated by declaring `animate <property> { duration, easing }`.

**States** are named groupings of property values:

```slint
states [
    pressed when touch.pressed: {
        background: gray;
        offset: 2px;
    }
    hovered when touch.has-hover: {
        background: lightgray;
    }
]
transitions [
    in pressed: { animate background { duration: 100ms; } }
    out pressed: { animate background { duration: 300ms; } }
]
```

States + transitions + animations together give a declarative state-machine UX similar to QML.

## Modules and standard widgets

`import { Foo } from "module.slint"` imports components. Slint ships standard widgets in `std-widgets.slint`: `Button`, `CheckBox`, `ComboBox`, `LineEdit`, `TextEdit`, `Slider`, `SpinBox`, `Switch`, `ScrollView`, `ListView`, `StandardListView`, `TabWidget`, `GroupBox`, `GridBox`, `VerticalBox`, `HorizontalBox`, `Dialog`, `ProgressIndicator`, `TimePickerPopup` (1.7+), `DatePickerPopup` (1.7+), and a MenuBar (1.11+).

Each widget honors the active **style** — Fluent (the default since 1.16), Material, Cupertino, Cosmic — selectable per build via the `SLINT_STYLE` env var or `slint::set_style()` API. The style choice affects look, not API.

## Layout primitives

Layouts are containers that arrange children: `VerticalBox`, `HorizontalBox`, `GridLayout`. Children carry `min-width`, `max-width`, `preferred-width`, `horizontal-stretch`, etc. The solver is custom (not Taffy or any external engine) — this is one of the places Slint diverges from the Rust UI mainstream where Taffy is the de-facto layout engine ([`../bevy-ui/layout.md`](../bevy-ui/layout.md), [`../taffy/`](../taffy/)).

## Tooling

The DSL is supported by a tooling stack that's the most polished part of the Slint product:

- **VSCode extension** (`Slint.slint` on the marketplace) — syntax highlighting, autocomplete, go-to-definition, refactoring, and a built-in **Live Preview** that renders the `.slint` source in a split pane and reloads on save.
- **Slint LSP** (`slint-lsp`) — language server for editors other than VSCode (Vim, Emacs, Helix, Sublime, etc.).
- **Slint Viewer** (`slint-viewer`) — standalone binary that renders any `.slint` file. Used for sharing UI prototypes without a host application.
- **Figma plugin** (1.10+, February 2025) — exports Figma designs as `.slint` code; supports Figma variables → Slint structs/enums/globals; honors Figma modes for theming.
- **Online editor** (https://slint.dev/editor) — browser-based playground, WASM-driven, same Live Preview semantics.

The tooling story is a clear strength relative to the rest of the Rust UI ecosystem — egui, Iced, and Dioxus don't have a Live Preview of comparable polish, and none have a Figma export.

## Implications for Buiy

- **The `in` / `out` / `in-out` qualifiers are a clean property-visibility design Buiy could borrow at the component level.** BSN-friendly Buiy components already commit to "small, public-fielded, observable, decomposed"; Slint's qualifiers are the same idea expressed as a property-direction taxonomy. Borrowable for component-API conventions in the widget catalog ([`docs/specs/2026-05-07-buiy-foundation/media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)).
- **Purity-enforced binding expressions are a useful primitive if Buiy ever adds a reactivity layer.** Slint enforces purity at compile time; a Bevy-side equivalent would enforce purity in the type system of any signal/computed primitive. Open question in foundation README § 5.
- **Two-way binding (`<=>`) is the canonical form-state primitive.** Buiy's `buiy-forms-design` sub-spec will need an equivalent; Slint's two-way binding (with cycle detection) is the most directly borrowable shape.
- **Live Preview is a tooling target.** Buiy's `buiy-devtools-design` sub-spec lists inspector / overlay / contrast checker / focus visualizer / theme editor; adding a "hot-reload a `.bsn` file with WASM-rendered preview" is the moral equivalent of Slint's Live Preview, and it would be a clear competitive differentiator. The Figma plugin is also worth studying — exporting tokens from Figma into Buiy's semantic-tokens system would be a 1.x feature, not a v1 commitment, but Slint's plugin is the prior art.
- **Slint's DSL is not a Buiy authoring path.** Buiy commits to ECS + BSN. The DSL framing is "this is what we would have to ship if we ever added a DSL layer," not "Buiy should adopt Slint." The foundation spec's "no DSL in v1" stance is a deliberate scope choice, validated by Slint's experience that maintaining a DSL is a continuing cross-language-binding cost.

## Sources

- Slint language reference: https://docs.slint.dev/latest/docs/slint/
- Slint properties docs: https://docs.slint.dev/latest/docs/slint/guide/language/coding/properties/
- Slint blog "Property Changed Callback": https://slint.dev/blog/property-changed-callback
- Slint VSCode extension: https://marketplace.visualstudio.com/items?itemName=Slint.slint
- Slint Figma plugin docs: https://docs.slint.dev/latest/docs/slint/guide/tooling/figma-inspector/
- Slint blog "Slint 1.10 Released": https://slint.dev/blog/slint-1.10-released
- Slint blog "Slint 1.7 Released": https://slint.dev/blog/slint-1.7-released
- Buiy foundation widgets: [`../../specs/2026-05-07-buiy-foundation/media-and-widgets.md`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md)
- Sibling files: [`architecture.md`](architecture.md), [`accessibility.md`](accessibility.md)
