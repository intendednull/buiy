**Date:** 2026-05-22
**Status:** active
**Subject:** Makepad — the Live DSL: syntax, hot-reload, property bindings, animations, tooling

# The Live language

Makepad's authoring DSL. Lives in two homes: **embedded in Rust** via `live_design! { ... }` macro blocks (compile-time-parsed, runtime-loaded), and **external `.live` files** (runtime-loaded, hot-reloadable). The compiler is the `makepad-live-compiler` crate (`makepad-live-tokenizer` → parser → expander → `LiveRegistry`).

## Syntax sketch

Live syntax is its own thing — not JSON, not Rust, not a Lisp. Closer to QML / CSS-with-types than to a templated XML. From the `hello_world` and `uizoo` examples (paraphrased):

```live
use link::widgets::*;
use link::theme::*;

App = {{App}} {
    ui: <Root> {
        main_window = <Window> {
            window: { title: "Hello Makepad" }
            body = <ScrollXYView> {
                show_bg: true,
                draw_bg: { color: #2 }
                flow: Down, spacing: 16.0, padding: 16.0,

                greeting = <Label> {
                    text: "Hello, world!"
                    draw_text: { color: #f, text_style: { font_size: 24.0 } }
                }

                button = <Button> {
                    text: "Click me"
                    draw_bg: {
                        color: #357,
                        instance hover: 0.0,
                        fn pixel(self) -> vec4 {
                            return mix(self.color, #4af, self.hover);
                        }
                    }
                    animator: {
                        hover = {
                            default: off,
                            off = { from: {all: Forward {duration: 0.1}}, apply: {draw_bg: {hover: 0.0}} }
                            on = { from: {all: Forward {duration: 0.1}}, apply: {draw_bg: {hover: 1.0}} }
                        }
                    }
                }
            }
        }
    }
}
```

Key constructs:

- **`Foo = {{FooStruct}} { ... }`** — bind a Live identifier to a Rust struct (`FooStruct` implements `Widget` / `Live` / `LiveHook`). The `{{ }}` double-brace is Makepad's syntax for "this Live block instantiates this Rust struct."
- **`<View> { ... }`** — instantiate a built-in or library widget by type name (`<Root>`, `<Window>`, `<Label>`, `<Button>`, `<Splitter>`, `<TextInput>`, etc.).
- **`property: value`** — bind a `#[live]` field on the underlying Rust struct.
- **`draw_bg: { ... }`** — nested Live block configuring a `DrawQuad` / `DrawShader` sub-widget. Includes inline GLSL-flavoured shader code (`fn pixel(self) -> vec4`).
- **`animator: { ... }`** — declarative animation state machine. Named states (`off`, `on`), transitions (`from: {all: Forward {duration: 0.1}}`), applied property changes (`apply: { ... }`). Equivalent in spirit to Slint's `states + transitions` block. See [`../slint/dsl-language.md`](../slint/dsl-language.md).

## Types and properties

Live supports a small fixed type set: `f32`, `f64`, `u32`, `i64`, `bool`, `String`, `vec2`/`vec3`/`vec4`, `Color` (with `#rrggbb` / `#rgb` / `#rrggbbaa` literals), `LiveDependency` (asset paths), `LiveType` (component references). Composite types come from Rust via `#[derive(Live)]`.

Property direction qualifiers are **not present** in Live's surface syntax the way Slint has `in` / `out` / `in-out`. Property direction is implicit in the Rust struct definition: a `#[live]` field is a Live-bound input, a `#[rust]` field is internal state, and "outputs" are encoded as Rust action types (see below).

## Actions: events and outputs

Makepad widgets emit **Actions** — Rust enums sent up the widget tree via `Cx`'s action queue. Parent widgets handle child actions in `handle_event`:

```rust
let actions = cx.actions();
for action in actions {
    if let Some(action) = action.as_widget_action() {
        if action.widget_uid == self.button(id!(submit)).widget_uid() {
            if matches!(action.cast(), ButtonAction::Pressed { .. }) {
                self.handle_submit(cx);
            }
        }
    }
}
```

Actions are the moral equivalent of Slint's `callback name(args)` and of Buiy's observers / events on widget components. They are not declared in Live syntax; they are Rust types the runtime routes through `Cx`.

## Hot-reload mechanism

The capability that distinguishes Makepad from most Rust UI alternatives. The runtime watches `.live` source files (and, in the Makepad Studio IDE, even Rust `live_design! { ... }` blocks inside open buffers). On change:

1. Re-tokenize and re-parse the changed `.live` source.
2. Re-expand into a new `LiveRegistry` (or a diff against the existing one).
3. Re-resolve `#[derive(Live)]` Rust structs against the updated registry — this re-binds `#[live]` field values without re-running constructors.
4. Trigger `LiveHook::after_apply` / `after_update_from_doc` on affected widgets to let them re-bake derived state.
5. Redraw.

The mechanism does **not** require the Rust binary to recompile for Live-syntax-only changes (property values, colours, layout dimensions, shader code, animation parameters). Behaviour changes in Rust `impl Widget` blocks still require a recompile.

The `hotload_ui` example is the canonical demonstration: it loads a `.live` file, runs the UI, and updates the running window when the file changes on disk.

**Comparison.** Slint's Live Preview is a comparable hot-reload story for `.slint` files; Bevy's BSN asset hot-reload is the in-Bevy equivalent. Both are *consumer* features; Makepad's stands out because hot-reload covers **shader code** too — change the inline `fn pixel(self) -> vec4 { ... }` GLSL and see the new shader compile and apply in the running window. This is Rik Arends's longtime live-coding-IDE thesis showing through (see [`history.md`](history.md)).

## Live's two-language refactoring cost

Property renames cross the language boundary: change `text: String` to `caption: String` in the Rust struct, update every Live block that says `text: "..."` to `caption: "..."`. No LSP server bridges the two (no equivalent of `slint-lsp` for `.live`); editor support is **Makepad Studio**, the IDE built on Makepad itself, dogfooded by the team. VS Code / Helix / Neovim users have no first-class Live language support as of folder-write.

## Comparisons

| Feature | Live (Makepad) | `.slint` (Slint) | BSN (Bevy 0.18+) |
|---|---|---|---|
| Source-of-truth role | Layout / styling / shaders / animation | Layout / styling / animation | Entity-tree templates |
| Lives in | `live_design!` macro + `.live` files | `.slint` files | `bsn!` macro + `.bsn` files |
| Hot-reload | Yes, including shaders | Yes (Live Preview), values & layout | Yes (planned, in `buiy-bsn-integration-design`) |
| Compile-time codegen | Proc-macro expansion (no `build.rs` required) | `slint!` macro or `slint-build` `build.rs` | Proc-macro (`bsn!`) or asset (`.bsn`) |
| LSP / editor support | Makepad Studio only | `slint-lsp` (editor-agnostic) | `rust-analyzer` covers macro form; `.bsn` files need new tooling |
| Property direction qualifiers | Implicit via `#[live]` / `#[rust]` | Explicit `in` / `out` / `in-out` | Implicit via component access (read = `Query<&T>`, write = `Query<&mut T>`) |
| Events / callbacks | Rust action enums via `Cx` action queue | `callback name(args)` in DSL | Observers + events on entities |

## Tooling

- **Makepad Studio** — the canonical Makepad-IDE-built-with-Makepad. Edits `.live` and `.rs`, embeds Live Preview, dogfooded by the team. `cargo run -p makepad-studio --release`.
- **`cargo-makepad`** — cross-platform toolchain installer + builder for iOS / Android / tvOS / OpenHarmony. See [`mobile-targets.md`](mobile-targets.md).
- **`makepad-lsp`** — does NOT exist as a standalone editor-agnostic LSP. Editor support outside Makepad Studio is unmaintained / community-maintained.
- **VS Code extension** — community-maintained syntax-highlighting for `.live` files exists as third-party extensions; no first-party Makepad VSIX equivalent to Slint's `Slint.slint` marketplace extension.

## Implications for Buiy

- **Borrow the hot-reload pattern for `.bsn`.** Makepad's `.live` + shader hot-reload is the polish target for Buiy's BSN asset hot-reload story ([`buiy-bsn-integration-design`](../../specs/2026-05-07-buiy-foundation/README.md)). Live shader hot-reload is the stretch goal Buiy should consider for shader assets in [`buiy-render-pipeline-design`](../../specs/2026-05-07-buiy-foundation/README.md).
- **Borrow the animator declarative state-machine.** Makepad's `animator: { state = { from, apply } }` block is the same shape as Slint's `states + transitions` (see [`../slint/lessons.md`](../slint/lessons.md) Borrow #3); Buiy's `buiy-animation-design` should consider a named-state-with-property-targets surface alongside keyframes / springs.
- **Avoid DSL-as-source-of-truth.** Makepad and Slint both make their DSL the canonical authoring layer. Buiy's ECS-and-BSN-equally-first-class choice rejects this; the Rust ECS world is the source of truth, BSN is one authoring surface among others. See [`lessons.md`](lessons.md).
- **Avoid the LSP-locked-to-one-editor pattern.** Slint's editor-agnostic `slint-lsp` is the right model; Makepad Studio's editor lock-in is not. Buiy's BSN tooling should be LSP-via-an-editor-agnostic-server from day one.

## Sources

- Makepad repo (examples): https://github.com/makepad/makepad/tree/dev/examples
- `hotload_ui` example: https://github.com/makepad/makepad/tree/dev/examples/hotload_ui
- `makepad-live-compiler` docs.rs: https://docs.rs/makepad-live-compiler/latest/makepad_live_compiler/
- Sibling files: [`README.md`](README.md), [`architecture.md`](architecture.md), [`gpu-rendering.md`](gpu-rendering.md), [`lessons.md`](lessons.md)
- Slint DSL precedent: [`../slint/dsl-language.md`](../slint/dsl-language.md), [`../slint/lessons.md`](../slint/lessons.md)
- Buiy foundation BSN: [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md) § 2.4
