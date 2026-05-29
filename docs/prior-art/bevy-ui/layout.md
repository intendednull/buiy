**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_ui — layout primitives (flex, grid, block), what Taffy ships and what it doesn't, scroll containers, subgrid/masonry status

## Layout engine: Taffy 0.10

bevy_ui's `UiPlugin` drives [Taffy](https://github.com/DioxusLabs/taffy) for all layout. As of bevy_ui 0.19-dev the pinned version is `taffy 0.10` with features `["std", "block_layout", "flexbox", "grid", "content_size", "taffy_tree"]` ([Cargo.toml](https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/Cargo.toml)). bevy_ui 0.18.1 pinned `taffy 0.9` ([0.18.1 Cargo.toml](https://github.com/bevyengine/bevy/blob/v0.18.1/crates/bevy_ui/Cargo.toml)). Bevy minor releases routinely bump Taffy, sometimes through breaking changes (e.g. [PR #15844 — "Upgrade to Taffy 0.6"](https://github.com/bevyengine/bevy/pull/15844) added `box_sizing` and exposed `margin`).

The translation layer is `bevy_ui::layout::convert`, which maps `Node`'s style fields onto `taffy::Style`. See [architecture.md](architecture.md) for system ordering.

## Flexbox

Fully supported via `Display::Flex`. The standard CSS flex surface is wired through:

- `flex_direction`, `flex_wrap`, `flex_grow`, `flex_shrink`, `flex_basis`
- `justify_content`, `align_items`, `align_self`, `align_content`
- `gap`, `row_gap`, `column_gap`
- `min_*`, `max_*` sizing including `Val::Auto`, `Val::Px`, `Val::Percent`, `Val::Vw`, `Val::Vh`, `Val::VMin`, `Val::VMax`

`Val::SpaceEvenly` for `JustifyContent`/`AlignContent` shipped via the 0.2 Taffy upgrade ([PR #6743](https://github.com/bevyengine/bevy/pull/6743)).

## CSS Grid

Supported via `Display::Grid` since Taffy 0.3. The grid surface tracks Taffy upstream closely:

- `grid_template_columns`, `grid_template_rows` — explicit tracks (`px`, `fr`, `auto`, `min-content`, `max-content`, `minmax`, `repeat(auto-fill|auto-fit, ...)`)
- `grid_auto_columns`, `grid_auto_rows`, `grid_auto_flow`
- `grid_column`, `grid_row` placement on children
- `gap`/`row_gap`/`column_gap`
- Named grid lines and grid areas (Taffy 0.9, [release notes](https://github.com/DioxusLabs/taffy/releases))

What Grid does *not* yet include — see [Taffy roadmap #345](https://github.com/DioxusLabs/taffy/issues/345):

- **Subgrid** (CSS Grid Level 2) — tracked as Taffy [#468](https://github.com/DioxusLabs/taffy/issues/468), listed under "Future."
- **Masonry / `display: grid-lanes`** (CSS Grid Level 3) — Taffy [#910](https://github.com/DioxusLabs/taffy/issues/910), "Future."

bevy_ui exposes whatever Taffy ships. Until Taffy lands subgrid/masonry, bevy_ui apps can't author them.

## Block layout

Supported via `Display::Block`. Block layout shipped in Taffy 0.4 (the brief's "Taffy 0.7+" was wrong — corrected here). bevy_ui enables it via the `block_layout` Taffy feature flag.

Caveats:

- Block layout in Taffy is the "supports `display: block` inside an otherwise-flex/grid tree" form, not full CSS Block Formatting Context semantics (no margin collapsing through float interaction, etc., though basic margin collapsing for in-flow blocks does work).
- Inline-level block formatting (floats, `display: inline-block` text-flow interaction) is partial — Taffy has a float layout (released in 0.10, [release notes](https://github.com/DioxusLabs/taffy/releases)) but bevy_ui does not currently surface a `float` field on `Node`.

## Anchor positioning

CSS [anchor positioning](https://developer.mozilla.org/en-US/docs/Web/CSS/CSS_anchor_positioning) is **not supported** in Taffy or bevy_ui. Tracked as Taffy [#703](https://github.com/DioxusLabs/taffy/issues/703), "Future." Apps needing tooltip-anchored-to-trigger or popover-near-button positioning roll their own using `ComputedNode` reads and absolute positioning, or use the `Popover` widget added in `bevy_feathers` 0.18 ([Bevy 0.18 notes](https://bevy.org/news/bevy-0-18/)) which implements its own anchoring rather than relying on a layout primitive.

## Container queries

Not supported. Taffy does not implement CSS container queries (no item on the roadmap as of 2026-05). bevy_ui apps that want size-responsive children of a known parent compute their own breakpoints by reading `ComputedNode` in a downstream system.

## Writing modes

Not supported. CSS `writing-mode: vertical-rl` / `vertical-lr` is tracked as Taffy [#752](https://github.com/DioxusLabs/taffy/issues/752), "Future." bevy_ui layouts assume horizontal-tb. RTL *direction* (Taffy 0.10's "Direction support for RTL layout") works at the inline-axis level for flex/grid container ordering, but the block axis is fixed top-to-bottom.

## Scroll containers / overflow

Supported via `Node::overflow`. The surface:

- `OverflowAxis::Visible` — default; children render outside parent bounds.
- `OverflowAxis::Clip` — clip children to parent rect. The clip is rectangular only (see [architecture.md § "Renderer caps"](architecture.md)).
- `OverflowAxis::Hidden` — same as Clip but also blocks scroll.
- `OverflowAxis::Scroll` — clip + allow scroll position.

Scroll position lives on `ComputedNode` (as `scroll_position`) and is written by an input handler. There is no built-in scrollbar widget in `bevy_ui` core, but `bevy_ui_widgets` ships a headless `Scrollbar` and `bevy_feathers` provides styled scrollbars on top ([Bevy 0.17 notes](https://bevy.org/news/bevy-0-17/)).

Sticky elements (CSS `position: sticky`) are not supported as a primitive. The 0.18 `IgnoreScroll` component approximates "stuck row/column headers in scrollable layouts" ([Bevy 0.18 notes](https://bevy.org/news/bevy-0-18/)) by exempting marked descendants from scroll-position translation — a narrower contract than CSS sticky, but it covers the common case.

Clipping geometry is rectangular and inherits down the hierarchy ([focus.rs `clip_check_recursive`](https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/focus.rs)). `OverrideClip` opts a descendant out of inherited clipping (used by Feathers popovers/menus to escape their scrolling ancestor without leaving the UI tree).

## Summary: what Taffy gates, what bevy_ui gates

bevy_ui's layout surface is *almost entirely Taffy's surface*. When a CSS layout feature is missing from bevy_ui, the question is which layer is gating:

| Feature | Gated by | Status |
|---|---|---|
| Flex | Taffy (shipped, 0.1+) | Available |
| Grid (Level 1) | Taffy (shipped, 0.3+) | Available |
| Block (basic) | Taffy (shipped, 0.4+) | Available |
| Float | Taffy (shipped, 0.10) | Available in Taffy; not exposed by bevy_ui |
| Named lines / areas | Taffy (shipped, 0.9) | Available |
| Subgrid | Taffy roadmap | Pending |
| Masonry | Taffy roadmap | Pending |
| Anchor positioning | Taffy roadmap | Pending |
| Container queries | Not on Taffy roadmap | Unavailable |
| Writing modes | Taffy roadmap | Pending |
| Sticky positioning | Neither | Approximated via `IgnoreScroll` (0.18) |

Buiy treats Taffy the same way — it integrates Taffy directly rather than going through bevy_ui — so Taffy's roadmap items unblock automatically for both stacks. Features Taffy doesn't have on roadmap (container queries) are the ones Buiy's foundation spec notes as "absorbable" rather than blocking ([`docs/specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md) § 1).

## Sources

- https://github.com/DioxusLabs/taffy
- https://github.com/DioxusLabs/taffy/releases
- https://github.com/DioxusLabs/taffy/issues/345
- https://github.com/DioxusLabs/taffy/issues/468
- https://github.com/DioxusLabs/taffy/issues/703
- https://github.com/DioxusLabs/taffy/issues/752
- https://github.com/DioxusLabs/taffy/issues/910
- https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/Cargo.toml
- https://github.com/bevyengine/bevy/blob/v0.18.1/crates/bevy_ui/Cargo.toml
- https://github.com/bevyengine/bevy/blob/main/crates/bevy_ui/src/focus.rs
- https://github.com/bevyengine/bevy/pull/6743
- https://github.com/bevyengine/bevy/pull/15844
- https://bevy.org/news/bevy-0-17/
- https://bevy.org/news/bevy-0-18/
