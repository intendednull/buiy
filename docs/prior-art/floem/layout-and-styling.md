**Date:** 2026-05-22
**Status:** active
**Subject:** Floem — Taffy-based layout, Style v2 pipeline, theme system, responsive

## Layout: Taffy 0.9.2

Floem delegates flexbox + grid layout to **Taffy 0.9.2** (with the `grid` feature enabled in workspace deps). This is the same engine Buiy commits to, and the same engine that powers `bevy_ui`, Dioxus's native target, Iced's layout module, and several others.

The integration shape:

1. Floem builds a Taffy `TaffyTree` parallel to its view tree.
2. View styles populate Taffy `Style { display, flex_*, grid_*, gap, padding, margin, ... }` per node.
3. On window resize / content change, Floem calls Taffy's `compute_layout(root, available_space)`.
4. Taffy returns resolved `Layout { x, y, width, height }` per node, which Floem reads back into view nodes for paint + hit-testing.

This is a textbook Taffy integration. See [`../taffy/`](../taffy/) for the Taffy substrate's own folder. Buiy's layout integration story is essentially the same shape — Taffy is the substrate, Floem and Buiy are sibling embedders.

## Styling: the builder API and "Faster style v2"

Floem's `Style` is a builder over CSS-like properties:

```rust
button("save")
    .style(|s| s
        .background(Color::rgb8(0x33, 0x99, 0xff))
        .color(Color::WHITE)
        .padding(8.0)
        .border_radius(6.0)
        .hover(|s| s.background(Color::rgb8(0x66, 0xbb, 0xff)))
        .active(|s| s.background(Color::rgb8(0x11, 0x77, 0xdd)))
    )
```

PR #1063 ("Faster style v2", merged 2026-04-11) rewrote the style application pipeline to reduce per-frame overhead. The pre-v2 pipeline allocated style maps eagerly; v2 batches and reuses. The release notes don't quote a numeric improvement; the PR title is the only public signal.

For Buiy: the v1 → v2 refactor is a data point that *style application is a real hot path* in fine-grained reactive UIs. Buiy's analog (token-based theming + per-component style resolution) should expect the same hot-path attention.

## Style scope: per-node, with hover/active/focus pseudo-states

Built-in pseudo-state hooks:

- `.hover(|s| ...)` — applied while pointer is over the node.
- `.active(|s| ...)` — applied during pointer press.
- `.focus(|s| ...)` — applied when the node has focus.
- `.disabled(|s| ...)` — applied to disabled views.
- `.responsive_breakpoint(...)` (via the `responsive` module) — width-based breakpoints.

These are flat `Style` overlays, not a CSS-cascade system. No selectors, no specificity rules, no `!important`. A Buiy designer reading this should note: Floem deliberately rejected CSS cascade complexity. Buiy's token + variant model (foundation `architecture.md` §2.5) makes the same call. Both projects opt for **explicit per-component style override** over CSS-cascade matching.

## Theme system

Floem ships a `themes` example and exposes a theme-builder API. Themes in Floem are essentially "shared style closures applied across the view tree." There is no token catalog like Buiy plans (foundation `architecture.md` §2.5 calls out semantic tokens with OS-pref binding). Floem themes are simpler — closer to "default styles for built-in views" than to a design-token system.

Buiy implication: Floem is **not** a strong reference for token-based theming. The reference points are Material 3's token system, Adobe Spectrum, and the bevy_flair approach (see [`../bevy-flair/`](../bevy-flair/)).

## Responsive module

The `responsive` module exposes width breakpoints. Usage is roughly `.responsive_breakpoint(Breakpoint::SM, |s| ...)`. This is breakpoint-based responsive layout — the simple CSS approach, not the newer CSS Container Queries model that Buiy's foundation `visuals.md` calls out as future-absorbable.

For Buiy: Floem's responsive module is a baseline reference (breakpoints work). Container Queries are out of Floem's scope.

## Animation (added in 0.2.0)

The 0.2.0 release notes call out "Full keyframe animations with spring animation support" as a major addition. The `animate` module provides:

- Transitions on style properties (animate on style change).
- Keyframe animations (`@keyframes`-like).
- Spring physics (interruptible, physically-modeled motion).

This is unusually complete for a Rust-native UI library — most peers ship transitions but defer keyframes + springs. Buiy's `buiy-animation-design` sub-spec roadmap commits to all three (transitions, keyframes, springs); Floem's API shape is worth direct study when that sub-spec is written.

## Composition with the render backend

Style v2 produces a flat "computed style" per node per frame. The render backend (vger / vello / skia / tiny-skia) consumes the computed style for paint. This is the same separation Buiy plans (foundation `architecture.md` §2.2: layout → style → paint as ordered system sets).

## What Floem's styling does NOT do

- **CSS string parsing** — no stylesheet input format. Style is Rust-side builder calls.
- **Selectors** — no `.class`, `#id`, attribute selectors.
- **Cascade / specificity** — flat override, no specificity rules.
- **Computed-style inspection in dev tools** — the "Element inspector" exists but the surface is small.
- **Logical properties** (`margin-inline-start` etc.) — uses physical-only (`margin-left`).
- **`writing-mode`** — no vertical-text style.
- **Container queries** — width-breakpoint only.

These omissions are not flaws — they are deliberate scope cuts. For Buiy, they highlight where Buiy's foundation chose **more ambition** (logical properties, writing-mode, container queries are all in scope) vs Floem's deliberate minimalism.

## Sources

- Floem Cargo.toml — https://github.com/lapce/floem/blob/main/Cargo.toml
- PR #1063 "Faster style v2" — https://github.com/lapce/floem/pull/1063
- Floem `examples/themes` — https://github.com/lapce/floem/tree/main/examples/themes
- Floem `examples/responsive` — https://github.com/lapce/floem/tree/main/examples/responsive
- 0.2.0 release notes (keyframes + springs) — https://github.com/lapce/floem/releases
- Taffy — https://github.com/DioxusLabs/taffy
- Cross-link: [`../taffy/`](../taffy/)
- Cross-link: Buiy foundation `architecture.md` §2.5 (theming) — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
