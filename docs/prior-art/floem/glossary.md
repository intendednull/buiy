**Date:** 2026-05-22
**Status:** active
**Subject:** Floem — system-specific terminology

Terms specific to Floem or used in Floem's documentation with idiosyncratic meaning.

## Reactivity

- **Signal** — atomic reactive cell holding a value. Reads via `.get()`; writes via `.set(v)` or `.update(|t| ...)`. Floem's `RwSignal<T>` is `Copy` when `T: Copy`.
- **Effect** — closure registered via `create_effect`; re-runs when any signal it reads changes. Side-effecting.
- **Derived / Memo** — cached computed value via `create_memo`; recomputes only when its tracked signals change.
- **Batch** — call wrapper (`batch(|| ...)`) that coalesces signal writes; deferred effects fire once at end.
- **Scope** — ownership boundary for signals and effects. Dropping a scope disposes its reactive state.
- **`with(|t| ...)`** — callback-style read of a non-`Copy` signal (avoids cloning the inner value).
- **Tracked** — a signal *read* inside a reactive closure is "tracked" and re-runs the closure when the signal changes. Reads outside reactive closures are untracked.

## Views

- **View** — anything implementing the `View` trait. Has an id, child views, style, layout, paint, event hooks.
- **View function** — a `fn() -> impl View` that constructs a subtree. Runs once at app start.
- **`prelude`** — Floem's curated import bundle.
- **`h_stack` / `v_stack` / `stack` / `dyn_stack`** — horizontal / vertical / generic / dynamic stack containers.
- **`label`** — static or reactive text view.
- **`text_input`** — single-line editable text.
- **`scroll`** — scrollable container with virtual scrollbar.
- **`virtual_list`** — windowed list for large datasets.
- **`dyn_container`** — view that swaps children based on a signal value.
- **`tab`** — tabbed container.

## Style

- **`Style`** — builder type aggregating per-node style overrides.
- **`.hover(|s| ...)`** — pseudo-state hook applied while pointer is over.
- **`.active(|s| ...)`** — pseudo-state hook applied during pointer press.
- **`.focus(|s| ...)`** — pseudo-state hook applied to focused node.
- **`.responsive_breakpoint(BP, |s| ...)`** — width-breakpoint hook from the `responsive` module.
- **Theme** — a shared style closure applied across a view subtree.

## Animation

- **Transition** — animate on style property change.
- **Keyframe** — `@keyframes`-like animation track.
- **Spring** — physically-modeled interruptible motion.

## Render

- **vger** — Floem-team's GPU renderer over wgpu. Default backend.
- **vello** — Linebender's GPU compute renderer (optional backend).
- **Skia** — via `floem_skia_renderer`, AnyRender-backed.
- **`tiny-skia`** — CPU software-rasterizer fallback.

## Text

- **Parley** — Linebender's shaping + layout engine. Used by Floem.
- **Swash** — Chad Brokaw / Linebender's font rasterizer.
- **Fontique** — Linebender's font enumeration + fallback.

## Layout

- **Taffy** — DioxusLabs's layout engine (flexbox + grid). Used by Floem and Buiy.

## Other

- **`floem-winit`** — Floem's published mirror of the `lapce/winit` fork.
- **`understory_*`** — Lapce-team sister crates (box-tree, focus). Dependencies of Floem.
- **`floem_reactive`** — the standalone reactive-runtime crate. Re-exported from `floem` as `reactive`.

## Cross-references

- Solid.js / Leptos terms (Signal, Effect, Derived, Scope) carry the same meaning in Floem. See [`fine-grained-reactivity.md`](fine-grained-reactivity.md) for the lineage.
- AccessKit terms are absent from Floem's surface (no integration). See [`../accesskit/glossary.md`](../accesskit/glossary.md) for AccessKit-specific terms.
- cosmic-text terms (Buffer, Editor, Action, Cursor) do **not** apply to Floem (Floem uses Parley). See [`../cosmic-text/glossary.md`](../cosmic-text/glossary.md).

## Sources

- Floem docs.rs — https://docs.rs/floem/latest/floem/
- Floem README — https://github.com/lapce/floem/blob/main/README.md
- floem_reactive docs.rs — https://docs.rs/floem_reactive/
