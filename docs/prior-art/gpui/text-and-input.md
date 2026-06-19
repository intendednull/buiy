**Date:** 2026-05-22
**Status:** active
**Subject:** GPUI — text rendering pipeline (OS-native shaping, font-kit, custom TextSystem), text-editing primitives (Zed's editor lives outside gpui), the action+key-context+focus system

# Text and input

GPUI's text and input story is **the editor's text stack** — Zed is a text editor, the framework was shaped by the editor's needs. Two consequences:

1. **Text shaping and rendering are first-class** with a dedicated `TextSystem` and `WindowTextSystem`. Quality matches OS-native (Core Text on macOS, DirectWrite on Windows). But it's an OS-shaping integration, not `cosmic-text` or Parley.
2. **The text-editing widget is not in GPUI.** Zed's code editor lives in `crates/editor/`, not in `crates/gpui/`. GPUI provides line-layout, glyph-rendering, key dispatch, and IME plumbing; the editor builds rich-text editing on top.

The split matters for Buiy: GPUI's `TextSystem` is the primitive-rendering layer (analogous to what `cosmic-text`'s `SwashCache` + `TextRenderer` provide in Buiy's stack); a separate-crate editor builds editing semantics on top (analogous to what Buiy's `buiy_text` crate will be per foundation §2.8).

## Text shaping — OS-native, not cosmic-text

Per the `Cargo.toml` and Scandurra's [_Videogame_](https://zed.dev/blog/videogame) post:

- **macOS:** Core Text via `core-text` + `core-graphics`. Shaping, font fallback, glyph rasterization all go through Core Text. This matches macOS system rendering exactly.
- **Windows:** DirectWrite. Subpixel ClearType anti-aliasing native to the platform; matches Windows system rendering.
- **Linux:** Custom path. The `font-kit` Cargo dependency lives in a Zed fork; shaping crates from the FreeType/HarfBuzz family are pulled in transitively. The text quality on Linux trails macOS and Windows historically and is the most-discussed text-rendering complaint in the Zed community.
- **Font parsing:** `ttf-parser = 0.25` across all platforms.
- **SVG icon text rendering:** `resvg` + `usvg = 0.45.0` for SVG-embedded text (icons with labels).

**Notably absent:** `cosmic-text`, Parley, swash, harfrust, fontique. GPUI does not use any of the Rust-ecosystem text stacks that Iced, Slint, Druid, Xilem, and Bevy converge on. This is a deliberate choice — Zed's bet is that OS-native shaping is the only way to match OS-native quality, and the price is platform-specific code paths.

For Buiy this is informative but not actionable. Buiy commits to **cosmic-text** (via Bevy's text stack, transitioning to Parley + swash on Bevy `main` per [bevy#21765](https://github.com/bevyengine/bevy/issues/21765)) for foundation §3.4-§3.5. The cosmic-text bet is right for Buiy because:

- Buiy is cross-platform and needs one shaping path that gives equivalent BiDi, RTL, and complex-script support everywhere.
- Buiy is a Bevy plugin and inherits Bevy's text-engine choices.
- Buiy targets game UIs in addition to app UIs; OS-native shaping isn't a hard requirement (games don't use OS menu fonts).

The GPUI lesson is **the gap exists**. If Buiy ships a system-app UI that needs to match OS conventions exactly, cosmic-text/Parley will fall short of DirectWrite/Core Text on subpixel positioning, hinting, and OS font fallback. The mitigation (foundation §3.4 open question) is per-platform shaping plugins for users who need them; the foundation default stays cosmic-text/Parley.

## The `TextSystem` API

Per [docs.rs](https://docs.rs/gpui/latest/gpui/) and the source:

- **`TextSystem`** — application-scoped. Owns font collections, glyph atlases, line-layout cache.
- **`WindowTextSystem`** — window-scoped wrapper. Adds window-specific DPI scaling and the glyph atlas associated with the window's GPU device.
- **`LineLayout`** — the result of shaping a single visual line: glyph IDs, positions, run boundaries, ascender/descender metrics.
- **`ShapedLine`** — `LineLayout` plus style information (color, decoration), ready to paint.
- **`WrappedLine`** — multi-line wrapping of a `ShapedLine`. Soft wrap is its own pass over the shaped output.
- **`LineWrapper`** — wraps text at a target width, using cached char widths to avoid reshaping for common ASCII paths.

The shape "shape once, layout many" is critical for performance. Zed's editor reshapes only when text changes; line wrapping rebuilds on viewport resize without reshaping; tinting is per-paint without touching the cache.

## IME, BiDi, RTL

- **IME (Input Method Editor)** integration is platform-specific. Each backend (mac/linux/windows) handles IME composition events natively. The editor crate routes composed text back into its buffer through GPUI's text-input event API.
- **BiDi** is delegated to the OS shaper. Core Text handles UAX #9 on macOS; DirectWrite handles it on Windows; on Linux, BiDi support depends on the HarfBuzz pipeline configuration.
- **RTL** text editing in Zed is functional but not first-class — the editor is a code editor; RTL editing primarily matters for prose, and Zed has comments/strings as the main RTL content.

Cross-platform IME parity is not a solved problem in GPUI; bug threads exist for each platform. This is the cost of three text stacks: each is independently OS-quality, but feature parity across them lags.

## The action and key-context system

GPUI separates "what happens" from "how it's triggered." A user-defined `Action` is a typed message:

```rust
actions!(workspace, [SaveAll]);
```

This generates `SaveAll` as a unit struct implementing the `Action` trait. Handlers register on focusable elements:

```rust
div()
    .key_context("Editor")
    .on_action(cx.listener(|this, _action: &SaveAll, cx| { ... }))
```

Keybindings are defined in a separate `keymap.json` (or via Rust API):

```json
{ "context": "Editor", "bindings": { "cmd-s": "workspace::SaveAll" } }
```

When a key is pressed, GPUI walks the focus path from the focused element up, matching each element's `key_context` against keymap entries. The first match dispatches the action.

**This is structurally similar to:**
- Bevy's `Trigger` observer system (typed messages, dispatched along an entity hierarchy)
- Emacs keymap mode-stacking (contexts compose; the most specific wins)
- The web's keydown handler bubbling

For Buiy, the takeaway is **keymap-as-asset is a strong pattern.** GPUI's `keymap.json` is hot-reloadable; users customize without recompiling. Buiy's foundation §3.7 input-events sub-spec should consider the same — declarative keymap assets bound to typed actions, dispatched via Bevy's observer system over the focus path. The implementation is largely "wrap `Trigger<MyAction>` with a context-matching dispatch layer."

## Focus model

GPUI's focus is **per-window** with a single focused element at a time. The focus path (focused element + all its ancestors) is what determines keymap matching. Focusable elements register via `.focusable()` on the `Interactivity` trait.

- `FocusHandle` is a typed handle to a focusable element; it can be passed around and used to programmatically focus.
- `cx.focused()` returns the current focus handle in a window.
- Focus is **not** automatically restored on window switch — that's the editor crate's responsibility.

Notably absent compared to Buiy's foundation §3.7 ambitions:

- No `:focus-visible` distinction (focused-by-keyboard vs focused-by-pointer)
- No focus traps as a first-class primitive (Zed implements them manually per dialog)
- No `inert` subtree support
- No roving tabindex helper
- No spatial / gamepad navigation
- No `aria-activedescendant` model (no ARIA at all — see [`accessibility.md`](accessibility.md))

GPUI's focus model is **sufficient for a code editor** and **insufficient for a WAI-ARIA APG widget library**. Buiy's `buiy-focus-model-design` sub-spec needs to be substantially richer; the GPUI implementation is at most a syntactic reference for how `FocusHandle` and focus-path traversal can look in Rust.

## Drag and drop

GPUI has basic drag-and-drop integration in `Interactivity`:

```rust
div().on_drag(payload, |drag, cx| { ... })
    .drag_over::<MyPayload>(|drop_target, _ev, cx| { ... })
    .on_drop(|payload: &MyPayload, _ev, cx| { ... })
```

This handles intra-window drag-and-drop. Cross-window and OS-native drag-and-drop (files dragged from Finder/Explorer into Zed) hook in through the platform layer.

The model is type-driven: payload types specify which drop targets accept which drags. This is cleaner than the web's `DataTransfer` string-typed drag types but more rigid (no MIME-type-driven generic handling).

Buiy's foundation §3.7 commits to drag-and-drop with a **drag a11y replacement contract** (foundation requires every drag UI to have a keyboard-accessible alternative path per WCAG SC 2.5.7). GPUI does not have this contract. The typed-payload pattern is borrowable; the keyboard-accessible alternative model is Buiy-specific.

## Sources

- GPUI `Cargo.toml`: https://github.com/zed-industries/zed/blob/main/crates/gpui/Cargo.toml
- _Leveraging Rust and the GPU to render user interfaces at 120 FPS_: https://zed.dev/blog/videogame
- GPUI docs.rs (TextSystem, FocusHandle, Action): https://docs.rs/gpui/latest/gpui/
- DeepWiki GPUI section: https://deepwiki.com/zed-industries/zed/2.2-ui-framework-(gpui)
- Zed on Windows (DirectWrite + DX11): https://zed.dev/windows
- bevy#21765 (Bevy main migration to Parley): https://github.com/bevyengine/bevy/issues/21765
- Cross-link: cosmic-text prior-art: [`docs/prior-art/cosmic-text/`](../cosmic-text/)
