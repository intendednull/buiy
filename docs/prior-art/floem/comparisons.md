**Date:** 2026-05-22
**Status:** active
**Subject:** Floem — comparison with Dioxus / Xilem / Iced / Solid.js / egui / Buiy on the axes that matter

## Side-by-side

| Dimension | Floem | Dioxus 0.5+ | Xilem | Iced | egui | Solid.js (JS reference) | Buiy (planned) |
|---|---|---|---|---|---|---|---|
| Reactivity model | Fine-grained signals (leptos_reactive lineage) | Signals over VDOM | View-tree diff with reactive lenses | Elm: `view(model) -> Element`, coarse | Immediate-mode | Fine-grained signals | Observers + change detection (ECS); no signals in v1 |
| View representation | Persistent view graph | Virtual DOM | Persistent view tree (Masonry) | Re-created `Element` tree per frame | Re-created per frame | Persistent DOM | ECS entity hierarchy |
| Diff phase | None | Yes (VDOM diff) | Implicit via lenses | Yes | None | None | None |
| Text engine | Parley + Swash + Fontique | Blitz (servo-based) for desktop | Parley + Swash + Fontique | cosmic-text | epaint glyph cache | Browser native | cosmic-text |
| Layout | Taffy 0.9.2 | Taffy (native), Blitz (desktop) | Custom (Masonry) | Custom (Iced layout) | Custom (egui layout) | Browser CSS | Taffy |
| Renderer | vger / vello / skia / tiny-skia (multi) | Blitz / wgpu | Vello | wgpu / tiny-skia | epaint over wgpu/glow | Browser | Bevy render graph |
| Accessibility | **None (issue #8 unstaffed 3y)** | Partial via Blitz | **AccessKit-first** | AccessKit since 0.13 | AccessKit since 2022 | Browser native | **AccessKit-first** |
| Animation | Transitions + keyframes + springs | Transitions | Limited | Transitions | Limited | CSS | Transitions + keyframes + springs (planned) |
| Theming | Closure-based styles | CSS-in-Rust | Style trait | Style trait | `Style` struct | CSS / utility libs | Token-based (foundation §2.5) |
| Hot reload | None | Yes (RSX hot reload) | None | None | None | Yes (Vite HMR) | BSN hot-reload (planned) |
| Web (WASM) | Experimental | Yes (primary target) | Yes (planned) | Yes | Yes | Native | Bevy WASM (open question) |
| Mobile | No | Yes (Dioxus mobile) | Planned | No | Yes (with caveats) | Mobile web only | Planned (foundation goal #6) |
| Foundation / steward | Lapce team (no full-time) | Dioxus Labs (corp-backed) | Linebender (broad coalition) | iced-rs core team | emilk (single primary) | Ryan Carniato + SolidJS Inc | Buiy team (planned) |
| Release cadence (4y) | 3 versions | 0.1 → 0.6, many points | Pre-1.0, regular | 0.1 → 0.13 | 30+ | Major versions yearly | TBD |

## Where Floem sits in the design space

Floem is a **persistent-view-tree fine-grained-signal native-Rust** UI library. Of the peers above:

- It is **most similar to Solid.js / Leptos** in reactivity model. The persistent view tree + signals architecture is the same.
- It is **most similar to Xilem** in design space (native Rust UI, view functions, Linebender text stack) — but Xilem has a different update model (lenses + reactive props rather than signals).
- It is **least similar to Iced / egui / immediate-mode** in update model — Floem has no per-frame view rebuild.
- It is **least similar to Dioxus** in framing — Dioxus has a virtual DOM (or Blitz HTML engine), Floem doesn't.

For a Buiy designer asking "what's the right reactivity reference if §2.7 is reopened?":

- **Conceptual reference**: Solid.js (most mature, best-documented, JS-side).
- **Rust port reference**: Leptos's `leptos_reactive` (the actual code Floem is derived from).
- **Native-app application reference**: Floem (signals applied to a non-DOM, non-web view tree).
- **Alternative model reference**: Xilem (signal-free; lens-based update).

Read in that order. Floem is the third stop, not the first.

## Comparison with Buiy specifically

| Axis | Floem | Buiy |
|---|---|---|
| Substrate | Standalone Rust UI | Bevy game-engine UI |
| State model | Signals + view-local | Bevy ECS components + observers |
| Render | Multi-backend (vger/vello/skia/tiny-skia) | Bevy render graph (wgpu) |
| Layout | Taffy | Taffy (same) |
| Text | Parley + Swash + Fontique | cosmic-text |
| A11y | None | AccessKit-first |
| Authoring | View functions in Rust | BSN + ECS spawning |
| Hot reload | None | BSN hot-reload (planned) |
| 3D-anchored UI | n/a | First-class (foundation §3.17) |
| WCAG 2.2 AA | Not a target | The floor (foundation goal #2) |

The deepest delta: **Floem owns the platform layer; Buiy doesn't.** Floem can choose its event loop (custom winit fork), its render pipeline (vger), its window model. Buiy plugs into Bevy's. This makes Buiy a parallel UI stack within an existing engine rather than a standalone library. The constraints are different and the design-space exploration must respect that.

## Where Floem could influence Buiy

If §2.7 (no signal layer in v1) is reopened:

- **API shape for signals.** `RwSignal<T>` + `create_effect` + `create_memo` + `batch` is a workable Rust shape. Buiy can copy it.
- **Closure-into-property pattern.** `label(move || format!("count: {}", c.get()))` — a Buiy `Text` component could accept either `String` or `impl Fn() -> String + 'static` to trigger fine-grained text updates.
- **Scope tied to entity lifetime.** Floem's `Scope` is per-view-subtree; Buiy's analog would be per-`Entity` or per-`SystemSet`.

What Floem **shouldn't** influence:

- Accessibility model (Floem has none).
- Distribution / release process (Floem is the cautionary tale).
- Mobile / platform-coverage roadmap (Floem hasn't done it).

## Sources

- All peer projects' READMEs and docs sites.
- Floem repo — https://github.com/lapce/floem
- Cross-links to sibling files: [`fine-grained-reactivity.md`](fine-grained-reactivity.md), [`text-and-parley.md`](text-and-parley.md), [`accessibility.md`](accessibility.md), [`ecosystem.md`](ecosystem.md).
- [`../dioxus/`](../dioxus/), [`../accesskit/`](../accesskit/), [`../cosmic-text/`](../cosmic-text/) prior-art folders.
