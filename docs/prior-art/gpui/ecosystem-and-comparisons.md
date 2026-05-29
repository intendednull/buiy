**Date:** 2026-05-22
**Status:** active
**Subject:** GPUI — ecosystem (production users: Zed primary, Longbridge secondary, gpui-ce fork) and comparisons (vs egui, Iced, Slint, Dioxus, Xilem/Masonry, Bevy/Buiy)

# Ecosystem and comparisons

## Production users

GPUI's user count is **small but real**. Three classes of adopters:

### 1. Zed Industries (primary, the only first-tier user)

[Zed](https://zed.dev/) is the entire reason GPUI exists. Every GPUI feature corresponds to a Zed feature; every Zed feature can use any GPUI API. The dogfooding is **complete and unidirectional** — GPUI serves Zed, not the other way around.

Zed's UI complexity is substantial:

- Multi-pane editor with infinite splits
- File tree, project tree, outline tree, git diff tree (each thousands of nodes)
- Terminal emulator (PTY integration, ANSI rendering)
- Command palette, completion popup, hover docs, signature help
- Multi-buffer view (project-wide search results editable in place)
- Collaborative editing UI (per-user cursors, presence indicators)
- AI agent chat panel, inline AI edit suggestions
- Settings UI, keymap editor, theme picker
- Diff view, merge conflict resolution, git blame inline

All of this ships on GPUI. The existence proof for "custom retained-mode UI scales to a serious productivity app" is Zed.

### 2. Longbridge (secondary production user)

[Longbridge](https://longportapp.com/) is a Singapore-based brokerage. Their Pro desktop trading client was rewritten from Electron to GPUI for performance reasons ([_Community Champion Spotlight: Jason Lee_](https://zed.dev/blog/community-champion-jason-lee), Zed blog).

Crucially, **Longbridge could not use GPUI off the shelf** — GPUI ships with no widgets. Jason Lee (Longbridge engineer) built [`longbridge/gpui-component`](https://github.com/longbridge/gpui-component) — a 60+-component UI kit on top of GPUI:

- Basic widgets (buttons, checkboxes, switches, sliders, inputs, dropdowns)
- Layout primitives (tabs, panels, dockable panels, accordion, splitters)
- Data display (tables with virtualization, trees, lists, code editors with syntax highlighting)
- Feedback (toast, dialog, modal, drawer, popover, tooltip)
- Form components with validation

`gpui-component` is the **only viable path** for building a real application UI on GPUI today. The Longbridge story confirms two things:

1. GPUI can support a non-editor production app.
2. The cost of doing so is "build your own widget library first."

### 3. Experimental / hobbyist (long tail)

~101k crates.io downloads across all `gpui = 0.2.x` versions suggests modest experimental adoption. The HN reference ("almost no usage" — [comment 45721667](https://news.ycombinator.com/item?id=45721667)) is consistent. Most downloads are likely CI builds against Zed-derived projects, vendoring, and brief experimentation.

No public third application using GPUI is widely cited beyond Zed and Longbridge Pro. The `gpui-ce` fork and `Glass-HQ/gpui` (an unrelated "native Rust UI framework" using the same crate name — see [GitHub](https://github.com/Glass-HQ/gpui)) are infrastructure projects, not end-user apps.

### Community fork: `gpui-ce`

[gpui-ce](https://github.com/gpui-ce/gpui-ce) ("GPUI Community Edition") was started by a former Zed employee as the response to Zed's stated unwillingness to accept community-priority work into mainline GPUI. As of May 2026:

- ~348 stars, ~23 forks
- Single-digit merged PRs
- Behind mainline by ~381 commits (per HN reference)
- Single-maintainer activity profile

Per the HN founder comment ([47005761](https://news.ycombinator.com/item?id=47005761)), the founder is becoming "more interested in a fresh approach" than maintaining a fork. The trajectory is unclear; the fork is alive but not at scale.

`gpui-component` has a discussion thread about feature-flagging support for `gpui-ce` ([discussion 1856](https://github.com/longbridge/gpui-component/discussions/1856)) — Longbridge wants the optionality but mainline GPUI remains their primary target.

## Comparisons

GPUI sits in a specific quadrant of the Rust UI design space: **GPU-accelerated, retained-mode (with immediate-mode escape hatch), custom paradigm, native-API-first**. The neighbors are different bets.

### vs egui ([`/home/user/buiy/docs/prior-art/egui/`](../egui/))

| | egui | GPUI |
|---|---|---|
| Paradigm | Pure immediate-mode | Hybrid immediate + retained |
| State model | `Memory` map keyed by widget ID | Typed `Entity<T>` ownership |
| Frame work | Rebuild widget tree every frame | Rebuild dirty views only |
| Idle CPU cost | Continuous (60 FPS rebuild) | Zero (notify-driven) |
| Layout | Manual cursor-based | Taffy Flexbox |
| Text | epaint glyph cache | OS-native shaping |
| Accessibility | AccessKit-enabled in eframe | None |
| Production at scale | Rerun (streaming data viz) | Zed (editor) |
| API stability | Stable-ish, semantic versioned | Pre-1.0, breaking changes |
| License | MIT/Apache | Apache-2.0 only |

GPUI wins on **idle cost** and **production-app polish**. egui wins on **simplicity** and **dev-tool fit**. They don't compete directly — egui is the dev-tool/debug/inspector default, GPUI is the production-app retained-mode bet. Buiy plans to **use bevy_egui for its own dev tooling** (foundation `cross-cutting.md` § 3.18 and bevy-egui lessons) while building production UI on its own retained-mode pipeline more analogous to GPUI's shape.

### vs Iced ([`/home/user/buiy/docs/prior-art/iced/`](../iced/))

| | Iced | GPUI |
|---|---|---|
| Paradigm | The Elm Architecture (pure) | Hybrid mutable |
| State model | Single global `Model` | Many `Entity<T>` handles |
| Update | `Model + Msg -> Model` | Mutable closures with effect queue |
| View | Pure `Model -> Element` | `Render::render(&mut self)` mutable |
| Renderer | `iced_wgpu` + `iced_tiny_skia` (wgpu end-to-end) | Metal / wgpu / DX11 (three backends) |
| Layout | Custom flex algorithm in `iced_core::layout` | Taffy |
| Text | cosmic-text via `cryoglyph` fork of `glyphon` | OS-native (Core Text / DirectWrite / FreeType-adjacent) |
| Accessibility | None (draft PRs only) | None |
| Production at scale | System76's COSMIC desktop (every cosmic-* app) | Zed editor |
| License | MIT | Apache-2.0 only |
| Cross-platform | wgpu uniform | Three native backends |

GPUI vs Iced is the **mutability** debate played out at framework scale. Iced's Elm purity is intellectually satisfying and produces clean code; GPUI's mutability is pragmatic and produces fast code. Both ship production apps. Neither has shipped accessibility. The renderer-uniformity bet (Iced = wgpu everywhere; GPUI = native where possible) is the cleanest empirical comparison — both produce shippable cross-platform apps, but Iced has uniform behavior while GPUI has platform-native polish at the cost of three code paths.

### vs Slint ([`/home/user/buiy/docs/prior-art/slint/`](../slint/))

| | Slint | GPUI |
|---|---|---|
| Authoring | DSL (`.slint` files compiled to Rust) | Rust code only |
| Hot reload | DSL-level | None |
| Paradigm | Property bindings + components | Hybrid render-method-based |
| Renderer | Skia or software (FemtoVG, OpenGL ES) | Metal / wgpu / DX11 |
| Layout | Slint-specific (compiled from DSL) | Taffy |
| Accessibility | AccessKit integrated | None |
| Target | Embedded + desktop | Desktop |
| Commercial model | Dual GPL/commercial | Apache-only |

Slint and GPUI represent **two different bets on where UI authoring lives**. Slint compiles a separate DSL into UI code; GPUI is "the UI is Rust." Slint's hot-reload at the DSL level is a Buiy-relevant feature (Buiy commits to hot-reloadable BSN); GPUI lacks any equivalent and likely never will because the UI _is_ Rust source.

### vs Dioxus ([`/home/user/buiy/docs/prior-art/dioxus/`](../dioxus/))

| | Dioxus | GPUI |
|---|---|---|
| Paradigm | React-style (VDOM + hooks) | Hybrid mutable |
| Authoring | `rsx!` macro JSX-alike | Rust fluent builders |
| Renderer | WebView, Liveview, Desktop (Blitz/native) | Metal / wgpu / DX11 |
| Hot reload | Yes (RSX-level) | None |
| Accessibility | Inherits from target (WebView, etc.) | None |

Dioxus and GPUI are explicitly different paradigms; Dioxus wants Rust-React, GPUI explicitly rejects diffing. Their target users barely overlap — Dioxus appeals to React developers; GPUI appeals to systems developers comfortable with Rust ownership.

### vs Xilem / Masonry (Linebender)

The Linebender ecosystem ([Xilem](https://github.com/linebender/xilem), [Masonry](https://github.com/linebender/xilem/tree/main/masonry)) is the most architecturally interesting comparison:

| | Xilem / Masonry | GPUI |
|---|---|---|
| Paradigm | Re-render through view tree, structural diff | Dirty-view rebuild without diff |
| State | App-owned struct, view captures via closures | App-owned `Entity<T>` |
| Renderer | Vello (wgpu-backed, GPU compute path tracing) | Metal / wgpu / DX11 |
| Text | Parley | OS-native |
| Accessibility | AccessKit integrated from day one | None |
| Backing | Linebender (Druid lineage) | Zed Industries |

Xilem represents "what GPUI would look like if it integrated AccessKit from the start." Both are GPU-accelerated retained-mode Rust UI; Xilem's accessibility-first design is the path Buiy mirrors. Vello as the renderer is a different bet from GPUI's per-primitive SDF shader pipeline — Vello is a general 2D renderer; GPUI's shaders are UI-specific. Xilem is younger and less production-tested; GPUI ships in Zed.

### vs Bevy ECS UI / Buiy

Already covered in [`architecture.md` § "Comparison to Bevy's ECS"](architecture.md) and [`element-tree.md` § "vs Bevy ECS UI (and Buiy)"](element-tree.md). The compressed version:

- **Semantic match is uncanny.** Both are typed-handle-into-global-storage with dirty-flag propagation and externalized Taffy layout. Buiy gets equivalent primitives free from Bevy's ECS.
- **Authoring diverges.** Buiy commits to declarative BSN with hot-reload; GPUI is Rust-only.
- **Accessibility diverges.** Buiy commits AccessKit-first; GPUI has none.
- **Cross-platform diverges.** Buiy commits to Bevy's wgpu (one path); GPUI has three native backends.

The right summary: **Buiy is "GPUI with Bevy's ECS doing the ownership work, AccessKit-first, wgpu-uniform, BSN-authored."** Most of GPUI's value is in the rendering primitives (SDF shaders, batched instance draws, glyph atlases) — primitives Buiy can borrow at the design level and reimplement clean-room on Bevy's render graph.

## Sources

- Zed product page: https://zed.dev/
- _Community Champion Spotlight: Jason Lee_ (Longbridge): https://zed.dev/blog/community-champion-jason-lee
- `longbridge/gpui-component`: https://github.com/longbridge/gpui-component
- `gpui-ce` fork: https://github.com/gpui-ce/gpui-ce
- HN reference "almost no usage": https://news.ycombinator.com/item?id=45721667
- HN gpui-ce founder reflection: https://news.ycombinator.com/item?id=47005761
- Glass-HQ/gpui (unrelated namespace collision): https://github.com/Glass-HQ/gpui
- Cross-link prior-art folders: egui, Iced, Slint, Dioxus in [`/home/user/buiy/docs/prior-art/`](../)
- Xilem: https://github.com/linebender/xilem
- Vello: https://github.com/linebender/vello
