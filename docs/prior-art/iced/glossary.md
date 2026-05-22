**Date:** 2026-05-22
**Status:** active
**Subject:** Iced — glossary of system-specific terms

Cross-link target for every other file in the folder. Terms are listed in rough dependency order: paradigm and runtime concepts first, then types, then crates, then people and projects.

## Paradigm and runtime concepts

- **Iced** — The crate (`iced` on crates.io). Most-adopted retained-mode Rust GUI library (1,885,134 lifetime downloads as of 2026-05-22). Repo: https://github.com/iced-rs/iced.
- **The Elm Architecture (TEA)** — The Model + Message + Update + View pattern Iced ports to Rust. State is a single value; mutations happen only via `update(state, message)`; views are pure functions of state. Canonical reference: https://guide.elm-lang.org/architecture/. See [`elm-architecture.md`](elm-architecture.md).
- **Model** — In Iced, the user-named application state struct (often `State`). The single source of truth for everything an app knows. One per `Application`; multi-window apps share it.
- **Message** — User-defined `enum` whose variants are every interaction or async-result an app handles. Dispatched centrally to `update`; pattern-match exhaustiveness checks every case is covered.
- **Update** — The function `fn(&mut State, Message) -> Task<Message>`. The only place state mutates. Returns a `Task` describing any side effect.
- **View** — The function `fn(&State) -> Element<'_, Message>`. Re-runs every event tick. Builds a fresh widget tree from current state.
- **Retained-mode (Iced flavour)** — Application *state* is retained between frames; the *widget tree* is rebuilt every `view()` call. Inverse of dominant retained models (Qt, GTK, web DOM) which retain the widget tree and mutate it. The parallel widget-state `Tree` reconciles state across rebuilds.
- **Stateless widgets** — Iced's widget values (a `Button` value, a `Text` value) are short-lived; they exist only during the layout / draw / event-dispatch pass. State that needs to outlive a frame (button pressed-flag, text-input cursor) lives in the parallel `Tree`. See [`elm-architecture.md`](elm-architecture.md) § "The stateless widgets trade-off."

## Iced types and traits

- **`Application` trait** — The pre-0.13 entry-point trait. Defines `type Message`, `type Theme`, `update`, `view`, optionally `subscription` and `theme`. Still exists in 0.14 as a thin wrapper; the function-based `iced::run(title, update, view)` is the current idiom.
- **`Sandbox` trait** — Even older simplified entry point (no async, no subscriptions). Largely superseded by `Application`; rarely seen in 0.14 code.
- **`Program` trait** — The underlying abstraction in 0.13+. `Application` is a `Program` with extras (subscriptions, themes, async lifecycle). Apps use the builder form `iced::application(state_fn, update, view).theme(...).subscription(...).run()`.
- **`Command<Message>`** — The pre-0.13 effect type. **Renamed to `Task<Message>`** in [PR #2463](https://github.com/iced-rs/iced/pull/2463) (2024-09-18, 0.13.0 release). Treat both names as the same concept; `Command` is the legacy name.
- **`Task<Message>`** — The 0.13+ effect type. A *value* describing async work the runtime should execute. Constructors: `Task::none()`, `Task::done(msg)`, `Task::perform(future, mapper)`, `Task::run(stream, mapper)`, `Task::batch([...])`, `Task::chain(t2)`. The cleanest async-effect descriptor in Rust GUI.
- **`Subscription<Message>`** — Long-lived stream of `Message` values from background sources (timers, websockets, file watchers, IME events, OS events). The runtime subscribes/unsubscribes by hash across rebuilds. `Subscription::frame()` is the canonical animation-tick subscription pre-0.14.
- **`Element<'a, Message, Theme = Theme, Renderer = iced::Renderer>`** — The type-erased widget-tree container. Built fresh every `view()`. Generic over `Message` so widget event handlers stay type-safe. `Element::map(f)` rewrites the inner message type — used for substruct decomposition.
- **`Tree`** — The parallel widget-state tree (`iced_core::widget::tree`). Mirrors the `Element` tree, keyed by tree-position + widget type-id (`tree::Tag`). Holds per-widget state (button pressed-flag, text-input cursor, scroll offset) that must outlive `view()` calls.
- **`tree::Tag`** — Identity for state reconciliation. Each `Widget` impl declares its `Tag` (typically its type-id); the runtime uses it to match new widget values to existing `tree::State` after `view()` rebuilds.
- **`Widget<Message, Theme, Renderer>` trait** — The contract custom widgets implement. Required methods: `size()`, `layout()`, `draw()`. Optional: `state()`, `tag()`, `on_event()`, `mouse_interaction()`, `overlay()`. No accessibility hooks — see [`open-problems.md`](open-problems.md) § "AccessKit integration."
- **`Renderer` trait** — Backend-agnostic renderer abstraction. `iced_wgpu::Renderer` and `iced_tiny_skia::Renderer` are the two production implementations.
- **`Theme`** — Enum of built-in theme variants (`Light`, `Dark`, `Dracula`, `Nord`, `SolarizedLight/Dark`, `GruvboxLight/Dark`, `TokyoNight/Storm/Light`, `KanagawaWave/Dragon/Lotus`, `Moonfly`, `Nightfly`, `Oxocarbon`, `Ferra`, `Custom(...)`). Resolves to a `Palette { background, text, primary, success, warning, danger }`. 0.14 added Oklch-based palette derivation.
- **`Style`** — Per-widget styling output. Each styleable widget defines its `Style` struct (e.g. `button::Style { background, text_color, border, ... }`). Computed by a closure `Fn(&Theme, Status) -> Style`.
- **`Appearance`** — Pre-0.13 styling output type, equivalent to `Style`. The term still appears in older docs and third-party widgets. See [PR #2312](https://github.com/iced-rs/iced/pull/2312) ("Functional widget styling," 0.13).
- **`Status`** — Per-widget interactive-state enum (e.g. `button::Status::{Active, Hovered, Pressed, Disabled}`). The style closure receives this to compute hover/active/disabled variants.
- **`Catalog` trait** — Standardizes the style API: each widget defines `Catalog::default_style(&Theme) -> Style` plus optional named variants (`button::primary`, `button::secondary`, `button::danger`, `button::success`, `button::text`).
- **`Length`** — Sizing primitive: `Length::Fixed(f32)`, `Length::Fill`, `Length::FillPortion(u16)`, `Length::Shrink`. Iced's analog to CSS `width: Npx` / `100%` / `flex-grow: N` / `max-content`.
- **`Limits { min, max, fill }`** — Layout constraint structure propagated downward through the layout tree. Each widget's `layout()` returns a `Node` sized within its `Limits`.
- **`Node`** — Resolved layout output: `bounds: Rectangle` + `children: Vec<Node>`. The output of `flex::resolve` and per-widget `layout()` calls.
- **`Action`** — In the text-editor context, a verb in cosmic-text's editor command set: `Insert(char)`, `Backspace`, `Delete`, `Motion(...)`, etc. Iced translates winit input events to `Action` calls on the editor. See [`text-and-cosmic.md`](text-and-cosmic.md) and [`../cosmic-text/glossary.md`](../cosmic-text/glossary.md).

## Renderer and runtime crates

- **`iced_core`** — Core types (`Color`, `Point`, `Rectangle`, `Element`, layout primitives, font/text types). No renderer-specific code.
- **`iced_widget`** — The built-in widget catalog (`button`, `text`, `text_input`, `scrollable`, `column`, `row`, `container`, `pick_list`, `slider`, `canvas`, `image`, `svg`, `markdown`, `qr_code`, plus 0.14 additions: `table`, `grid`, `pin`, `float`, `wrap`, `sensor`, `stack`).
- **`iced_runtime`** — Runtime layer above the renderer/winit: `Task`, `Subscription`, clipboard, window control.
- **`iced_renderer`** — Façade that picks the actual backend (`iced_wgpu` or `iced_tiny_skia`) at compile/runtime.
- **`iced_wgpu`** — The default GPU renderer. Uses `wgpu 27.0` and `cryoglyph` (since March 2025) for text.
- **`iced_tiny_skia`** — CPU-software fallback renderer (via `tiny-skia 0.11`). Used when `wgpu` init fails or for headless tests.
- **`iced_winit`** — Window + event-loop integration via `winit 0.30`. Same `winit` version as Bevy 0.15+.
- **`iced_graphics`** — Backend-agnostic graphics types (mesh, gradient, paragraph, geometry); depends on `cosmic-text` directly.
- **`iced_futures`** — Subscription / executor plumbing. Optional `tokio` / `smol` / `thread-pool` integration.
- **`iced_highlighter`** — Syntax highlighting via `syntect`. Gated behind the `highlighter` feature.
- **`iced_debug`, `iced_devtools`, `iced_tester`, `iced_test`, `iced_program`, `iced_beacon`, `iced_selector`** — The 0.14-era cluster supporting time-travel debugging, hot reload, headless testing, end-to-end testing.

## Text-stack crates

- **`cryoglyph`** — Iced's March 2025 fork of `grovesNL/glyphon`. The cosmic-text → wgpu adapter that `iced_wgpu` consumes. Same cosmic-text underneath as upstream `glyphon`; different upstream control so Iced can move the adapter on its own roadmap. Repo: https://github.com/iced-rs/cryoglyph. See [`text-and-cosmic.md`](text-and-cosmic.md), [`distribution.md`](distribution.md).
- **`glyphon`** — The original cosmic-text → wgpu adapter, by `grovesNL`. Iced consumed `glyphon` directly from 0.10 (2023-07) through ~0.13; forked to `cryoglyph` in March 2025. Repo: https://github.com/grovesNL/glyphon.
- **`cosmic-text`** — The shaping + layout + cursor + selection + glyph-cache engine used by Iced. Maintained by System76 for COSMIC desktop. Iced 0.14 pins `cosmic-text = "0.15"`. The shaper underneath cosmic-text 0.15 is `harfrust` (HarfBuzz Rust port). See [`text-and-cosmic.md`](text-and-cosmic.md), [`../cosmic-text/README.md`](../cosmic-text/README.md).
- **`text::Shaping`** — Iced's shaping-mode enum: `Basic` (GSUB-skipping fast path, Latin-only), `Advanced` (full GSUB + GPOS + per-script shaping), `Auto` (added 0.14, auto-picks per run).

## Production projects and people

- **COSMIC desktop** — System76's Rust-based desktop environment, announced 2022, shipped in Pop!_OS COSMIC starting 2024. The flagship at-scale Iced deployment. Repo: https://github.com/pop-os/cosmic-epoch.
- **COSMIC Files** — The file manager. Repo: https://github.com/pop-os/cosmic-files.
- **COSMIC Settings** — The system-settings app. Inside `cosmic-epoch`.
- **COSMIC text-editor** — The default text editor. Inside `cosmic-epoch`.
- **`libcosmic`** — System76's higher-level styling + widget layer on top of Iced. Provides the `cosmic_theme::Theme` semantic-token system that raw Iced lacks. Repo: https://github.com/pop-os/libcosmic.
- **Halloy** — Open-source IRC client. Featured in the Iced README's screenshot showcase. Repo: https://github.com/squidowl/halloy.
- **Cryptowatch desktop** — Kraken's flagship crypto-charting application. Iced's most commercially-significant non-COSMIC user. URL: https://docs.cryptowat.ch/desktop.
- **Veloren** — Multiplayer voxel RPG. Uses Iced for in-game menus and HUD overlays. URL: https://veloren.net.
- **Sniffnet** — Network-traffic monitoring tool. ~10k+ GitHub stars. Repo: https://github.com/GyulyVGC/sniffnet.
- **modrinth-app** — **NOT an Iced user.** Listed in the original brief as "modrinth's desktop launcher built on Iced"; verification confirms `modrinth/code` is built with **Tauri + Vue**. Common confusion. See [`ecosystem.md`](ecosystem.md) § "Brief corrections."
- **Kraken** — The cryptocurrency exchange. Sponsors Iced via its Cryptowatch team (per Iced README's "Sponsors" section). Largest single financial backer since around the 0.6 era. URL: https://kraken.com.
- **Héctor Ramón** — GitHub handle `hecrj`. Founder, lead architect, sole committer on `iced-rs/iced`. Iced is his project from 2019 onward, spun out of his earlier `coffee` game-engine experiments. GitHub Sponsors page: https://github.com/sponsors/hecrj.
- **iced-rs** — The GitHub organization. Holds `iced`, `cryoglyph`, `awesome-iced`, `iced_aw`. URL: https://github.com/iced-rs.

## Community libraries and ecosystem

- **`iced_aw`** — Official-adjacent (iced-rs org) widget-extras crate. Badges, color/date pickers, drop-downs not in core, menu bars, modals, segmented buttons, tab bar. The de-facto "stdlib extension" for Iced apps. ~415k recent downloads. Repo: https://github.com/iced-rs/iced_aw.
- **`iced_audio`** — VST/LV2-oriented widget set (rotary knobs, sliders, X/Y pads). Repo: https://github.com/iced-rs/iced_audio.
- **`iced_video_player`** — GStreamer-backed video widget. Repo: https://github.com/jazzfool/iced_video_player.
- **`iced_term`** — Terminal-emulator widget. Repo: https://github.com/Harzu/iced_term.
- **`iced_fonts`** — Icon-font helpers (Bootstrap Icons, Nerd Fonts). ~336k recent downloads.
- **`plotters-iced`** — `plotters` rendering backend for Iced. ~98k recent downloads.
- **`iced_layershell`** — wlr-layer-shell binding (Wayland panel / lockscreen apps). Repo: https://github.com/waycrate/iced_layershell.
- **Cosmic Time** — Animation toolkit by System76 for COSMIC; pre-dates Iced 0.14's built-in Animation API. Repo: https://github.com/pop-os/cosmic-time.
- **`bevy_iced`** — Community crate that embeds Iced apps inside Bevy. The closest existing cross-link between Iced and Buiy's host engine. Runs two layout engines + two text caches; Buiy's foundation explicitly chose parallel-to-bevy_ui over this approach. Repo: https://github.com/tasgon/bevy_iced.

## Sources

- iced docs.rs — https://docs.rs/iced/0.14.0/iced/
- iced book — https://book.iced.rs/
- awesome-iced — https://github.com/iced-rs/awesome-iced
- All sibling files in this folder.
