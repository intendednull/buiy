# Changelog

All notable changes to Buiy are recorded here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
once it reaches `0.1.0`. Pre-`0.1.0` releases are pre-alpha; APIs may break in any commit.

## [Unreleased]

Pre-`0.1.0` development. Detailed change tracking begins with the first
tagged release.

### Added
- Render pipeline now produces real pixels for Buiy nodes (instance-buffer
  construction, clip-space conversion, draw call). Closes the Phase 0
  render deferral.
- Per-window AccessKit tree-update bridge. Buiy translates its widget tree
  to `accesskit::TreeUpdate` each frame and pushes it through bevy_winit's
  `ACCESS_KIT_ADAPTERS` so real screen readers attached to a Buiy window
  see the live tree. (Bevy 0.18 owns adapter creation, so Buiy bridges
  rather than owning `Adapter` objects directly.) Closes the Phase 0 a11y
  deferral.
- `bevy_picking` backend. `Hovered` becomes a thin layer over the standard
  `PointerHits` event flow. Closes the Phase 0 picking deferral.
- `buiy_core::components::Visual` component (`background_token`,
  `foreground_token`, `border_radius`) carrying the render-side surface
  formerly mixed into the Phase 0 mega-`Style`. Authors who want themed
  widgets insert `Visual` alongside the new layout `Style` builder.
  Eventual home is `buiy-render-pipeline-design`.
- `buiy_core::layout` module: 8-step layout pipeline (`BuiyLayoutStep`
  system sets), decomposed `BoxModel` / `Display` / `Position` /
  `FlexParams` / `FlexItem` components, hybrid `Style` builder that
  expands to a `Bundle` on spawn.
- Doc-hidden read-only accessors on `LayoutTree`: `by_entity()` and
  `tree_ref()` for integration-test introspection.
- Layout `Overflow` component (per-axis `OverflowMode` + `scrollbar_*`,
  `scroll_behavior`, `overscroll_*`). Wired into `taffy::Style.overflow`
  and `taffy::Style.scrollbar_width`. Spec:
  `docs/specs/2026-05-08-buiy-layout-design/overflow-and-scrolling.md`.
- Layout `Scroll` component (snap-type, snap padding, snap margin) for
  scroll-snap container declaration.
- Layout `ScrollOffset` runtime-state component (per-axis scroll
  position). Mutation does not invalidate `ResolvedLayout` (asserted by
  `tests/layout_scroll_offset_no_invalidate.rs`).
- Layout `ScrollSnapItem` decomposed-only child-side component.
- `Overflow::is_scroll_container()` predicate (spec § 1.2).
- 9 supporting layout enum types: `OverflowMode`, `ScrollbarGutter`,
  `ScrollbarWidth`, `ScrollbarColor`, `ScrollBehavior`,
  `OverscrollBehavior`, `SnapType`, `SnapAlign`, `SnapStop`.
- `Style` builder: `Overflow` and `Scroll` fields; 12 fluent setters
  (`.overflow_x()`, `.overflow_y()`, `.overflow()`, `.overflow_hidden()`,
  `.overflow_y_scroll()`, `.overflow_x_scroll()`, `.scrollbar_gutter()`,
  `.scrollbar_width()`, `.scroll_behavior()`, `.snap_type()`,
  `.snap_padding()`, `.snap_margin()`).

### Changed
- Layout subsystem foundation rewritten. Phase 0's flat `layout.rs` is
  replaced by a `layout/` directory module. The pipeline is an 8-step
  ordered chain (`BuiyLayoutStep` system sets) inside `BuiySet::Layout`;
  Phase 1 implements steps 0/1/3/7 and stubs the remaining four for
  later phase plans.
- `Style` is now a `Bundle` that decomposes on insert, not a reflectable
  `Component`. Reflection / inspectors / BSN see the decomposed
  components (`BoxModel`, `Display`, `Position`, `FlexParams`).
- The render extract now queries `(&Visual, &ResolvedLayout)` instead
  of `(&Style, &ResolvedLayout)`; entities without `Visual` are skipped
  by render. `Button::new` inserts a `Visual` carrying the same theme
  tokens Phase 0's `Style` did, so visual appearance is preserved.
- `sync_styles`' change-detection trigger set widens to include
  `Changed<Overflow>` and `Changed<Scroll>`; remains exclusive of
  `Changed<ScrollOffset>` and `Changed<ScrollSnapItem>`.

### Removed
- `buiy_core::components::Style` (the Phase 0 mega-component) and
  `buiy_core::components::FlexDirection`. Their roles are taken by
  `buiy_core::layout::Style` (the hybrid builder) and
  `buiy_core::layout::FlexAxis` (the four-variant axis enum).
