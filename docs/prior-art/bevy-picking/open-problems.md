**Date:** 2026-05-22
**Status:** active
**Subject:** bevy_picking — critiques and structural open problems

# Critiques & open problems

The honest list of what bevy_picking does not do well, what it doesn't do at all, and what Buiy will have to either work around or extend.

## Critiques

### API churn at every Bevy minor

Three significant renames in three releases:

- 0.15 — Initial in-tree shape. Existing `bevy_mod_picking` users had a migration to do.
- 0.16 — `PickingBehavior` → `Pickable`.
- 0.17 — `Pointer<Down>` / `Pointer<Up>` → `Pointer<Press>` / `Pointer<Release>`.

For a load-bearing dependency on rolling-latest-Bevy policy ([`architecture.md` § 2.9](../../specs/2026-05-07-buiy-foundation/architecture.md)), this means **at least one Buiy code change per Bevy minor for at least the next year**. The renames are quality-positive — `Pickable` reads better than `PickingBehavior`, `Press`/`Release` matches the broader Bevy event vocabulary — but they're still breakage for downstream tutorials, blog posts, AI-training data, and Buiy's own docs.

### `Pickable`-opt-in tradeoff

Default behaviour without `Pickable` is "block lower + hoverable" — the *more aggressive* setting. This means **every entity is in the picking system by default**, even decorative wrappers and group nodes that probably shouldn't be. For a Buiy form with hundreds of decorative `Node`s, that's hundreds of entities being hover-state-tested every frame.

The alternative — opt-in only, default-no-pick — has its own tradeoff (every interactive widget must remember to add `Pickable`). bevy_picking picked the opt-out side. Buiy inherits this; per-widget defaults in Buiy will explicitly set `Pickable::IGNORE` on internal wrapper nodes.

### Single global `order` field

The most consequential structural critique. `PointerHits.order` is a single global f32 per backend per frame. There's **no API for "backend X owns window W, suppress all other backends on W."** Coexistence between bevy_ui's backend and Buiy's backend on the same Bevy `App` relies on each backend manually filtering its picks to its own windows — see [`integration.md`](integration.md).

The current "convention" is `order = camera_order + 0.5`. Two backends that both want the same window arbitrate by which assigned itself the higher offset. There's no way to express "I am the authoritative UI backend for this window" in a way that's enforceable. If Buiy and bevy_ui ever end up reporting hits for the same window (developer error), the result is a race.

### No `Pointer<E>` source attribution beyond `pointer_id`

`Pointer<E>` carries `pointer_id`, but doesn't carry the **backend** that produced the hit. If an observer wants to behave differently when hit by Buiy's backend vs the sprite backend (e.g. "only enter drag mode if it was a 'real' UI hit, not a sprite-overlap"), there's no built-in mechanism — observers see only the final event. Buiy works around this by component-marking entities ("this is a Buiy node").

### Mesh picking is naive

`O(triangles)` per ray per pointer per frame. Bevy's release notes explicitly defer optimised picking to `bevy_rapier` / `avian`. A user who wants click-on-3D-meshes in a scene-editor or selection-tool ends up either using the slow built-in or pulling in a physics engine as a dependency they otherwise don't need. Not Buiy's direct concern (Buiy is 2D UI for v1) but a flag for `buiy_3d` futures.

### Window backend's "catch-all" semantics

The window backend reports the window entity as hit whenever no higher-priority backend covers the pointer. Useful, but the priority/ordering is implicit. A custom backend with negative `order` would *not* sit "below" the window backend's catch-all in a way that's documented; in practice the window backend is treated as the floor.

## Structural open problems

### Backend priority API for multi-stack coexistence

The big one. A real fix requires bevy_picking to grow a **per-window backend-priority API**: backends should be able to register as "the UI backend for window W" and have other backends' hits on W be filtered out (or scored below) without each backend re-implementing window filtering. Buiy's per-window stance ([`cross-cutting.md` § 3.18`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)) is the workaround; the upstream fix would let Buiy + bevy_ui coexist on the same window in principle. Buiy's spec deliberately doesn't commit to that future.

### Gamepad / keyboard spatial-nav as first-class

bevy_picking supports custom pointers but ships no canonical "spatial-nav virtual pointer." Apps implementing gamepad-driven UI must either:

- emulate a cursor from analog stick input (works, but requires invariant maintenance — cursor position, hover handling); or
- skip the picking pipeline for gamepad entirely (loses observer convenience, must wire focus → activation manually).

Buiy goes route 2 — spatial focus navigation, not gamepad pointer. The cost: keyboard activation, spatial-nav activation, and AT activation all go through Buiy's focus → action plumbing rather than reusing bevy_picking's observer pattern. Code duplication between "pointer-driven click handler" and "focus-driven activate handler" is a real maintenance tax. Documented in [`interaction.md` § 3.7](../../specs/2026-05-07-buiy-foundation/interaction.md).

### Subpixel hit-testing for text caret placement

bevy_ui's text picking reports the section (a coloured span), not the cluster or grapheme. To place an editing caret on click in a textarea, the picking result is too coarse. Buiy's text editing (per [`text.md`](../../specs/2026-05-07-buiy-foundation/text.md)) does its own subpixel hit-test inside `buiy_text` against cosmic-text's layout, bypassing bevy_picking for the within-text-node step. Open problem upstream: should bevy_picking grow optional subpixel hits, and how would the API surface that without making the common case slower?

### Multi-window pointer sharing

`PointerLocation.target` updates when a cursor crosses windows, but **drag handoff** between windows isn't built in. Dragging a tab from one Buiy-owned window to another requires per-app code. Common UX pattern, not solved here.

### Drag accessibility (WCAG 2.5.7)

Every drag-driven interaction must have a non-drag alternative. bevy_picking provides the events but no built-in scaffolding. Each Buiy widget that uses drag must ship its own keyboard/menu alternative. Audit responsibility falls on widget authors; bevy_picking can't enforce.

### Hit-testing of non-rect shapes

`border-radius`, `clip-path`, overflow clipping with rounded corners — none of these affect bevy_ui's hit-test rect. Buiy is required to do shape-aware hit testing inside its own backend (per [`visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md), [`capabilities.md`](capabilities.md)). The structural-fix question upstream: should `bevy_ui` grow shape-aware hit testing or stay rect-only? No public discussion.

### Picking of GPU-rendered children (render-to-texture)

Bevy 0.17's `ViewportNode` picking is the first step: a UI node hosts a render-target, and clicks on the node can hit-test against the render-target's entities. But the integration is one-way — a `ViewportNode` exposes the underlying scene's picks; it doesn't compose. For Buiy's spec-permitted "render-to-texture surface containing a child Buiy subtree" pattern (per [`visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md) backdrop-filter and isolation), the hit-test routing needs to be re-derived. Not yet specified.

### Hover-delay / long-press / multi-click timing primitives

Tooltips, context menus, long-press affordances all need timing primitives. bevy_picking provides events but no timers. Each app rebuilds them. Buiy will ship hover-delay + long-press in `buiy_widgets` per [`interaction.md` § 3.7`](../../specs/2026-05-07-buiy-foundation/interaction.md).

### Maintainer/SME ownership

No single named picking lead post-upstream (per [`distribution.md`](distribution.md)). For Buiy as a load-bearing-dep consumer, this means upstream advocacy for the structural fixes above (multi-window priority, subpixel hits, render-to-texture composition) doesn't have a canonical reviewer to take it to. Buiy maintainers will likely need to file issues, write proof-of-concept PRs, and shepherd them through Bevy's general PR queue.

## Sources

- https://docs.rs/bevy_picking/0.18.1/bevy_picking/
- https://github.com/bevyengine/bevy/issues?q=label%3AA-Picking — open issues against picking
- https://bevy.org/news/bevy-0-15/, 0-16, 0-17 (rename history sourced from release notes)
- Bevy PR #15800 (mesh picking deferral to physics ecosystem, stated by the PR author)
- Buiy: `docs/specs/2026-05-07-buiy-foundation/{visuals,interaction,text,accessibility,cross-cutting}.md`
