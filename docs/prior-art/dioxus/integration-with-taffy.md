**Date:** 2026-05-22
**Status:** active
**Subject:** Dioxus — Taffy usage (through Blitz on Native target; absent from web target)

# Integration with Taffy

Dioxus's relationship to Taffy is **indirect and target-specific**:

- **Web target:** Taffy is **not used**. The browser DOM has its own layout (Blink/Gecko/WebKit). Dioxus emits semantic HTML; the browser lays it out.
- **Desktop (Webview) / Mobile (Webview):** Taffy is **not used**. Webview-supplied layout engine.
- **Desktop (Blitz/WGPU) / Mobile (Blitz/WGPU) / Dioxus Native:** Taffy **is used**, indirectly through Blitz. Blitz's `blitz-dom` crate registers a Taffy `LayoutPartialTree` impl against its own DOM node arena.

This makes Dioxus the **second-largest production consumer of Taffy after Bevy itself**, but only on the pre-alpha Native target. The Webview targets — the production-tier paths — do not depend on Taffy.

## How Blitz uses Taffy

Blitz's layout flow (per `blitz-dom` and the `stylo_taffy` glue crate):

1. Stylo parses CSS + resolves cascade → produces a `ComputedValues` per element.
2. `stylo_taffy` translates Stylo's `ComputedValues` into Taffy's `Style` per element.
3. Blitz's `blitz-dom` exposes a `BlitzDocument` that implements Taffy's `LayoutPartialTree` + `TraversePartialTree` + `CacheTree` traits against the DOM tree.
4. `taffy::compute_root_layout(&mut document, root_id, available_space)` runs the algorithm.
5. Resolved layout (`Layout { location, size, content_size, ... }`) is written back into the DOM nodes.
6. Blitz's paint pipeline reads the resolved layout and produces draw commands for Vello.

This is the **trait-based integration path** documented in [`../taffy/architecture.md`](../taffy/architecture.md) (`LayoutPartialTree` / `TraversePartialTree` / `CacheTree`). Blitz does not wrap `TaffyTree` (the high-level API); it implements the low-level traits directly against `blitz-dom`. Compare to Bevy UI, which wraps `TaffyTree`.

## Taffy version pin

Per the Blitz `Cargo.toml` (verified via repo crawl 2026-05-22), Blitz pins **the experimental Taffy release line** — the `=0.11.0-experimental-cache-fix.*` series — because Blitz is the primary consumer of Nico Burns's cache-correctness work. This is the "do not depend on `-experimental-` versions" warning [documented in `../taffy/lessons.md` § "Avoid" row 1](../taffy/lessons.md): Blitz is exempt from that warning because cache correctness is load-bearing for it; production embedders should not be exempt.

## Why this matters for Buiy

The Buiy layout subsystem ([buiy-layout-design](../../specs/2026-05-08-buiy-layout-design/)) wraps `TaffyTree` (the high-level API) and bridges via a hybrid `Style` builder. Blitz wraps the trait surface (the low-level API). Both are valid integration patterns; Taffy ships both as first-class.

The differences worth noting:

| Integration concern | Bevy UI / Buiy (wraps `TaffyTree`) | Blitz (impls `LayoutPartialTree`) |
|---|---|---|
| Node storage | `TaffyTree`-internal arena + `HashMap<Entity, NodeId>` | DOM tree is the storage; node IDs are DOM-node-IDs directly |
| Child topology | `TaffyTree::set_children` | Direct via DOM parent/child pointers |
| Style sync | Per-frame `SyncStyles` system rebuilds `taffy::Style` from decomposed components | `stylo_taffy::to_taffy_style` from Stylo's `ComputedValues` |
| Measure function | Registered via `tree.new_leaf_with_context` | Implemented directly in the trait |
| Cache | Taffy's per-node `Cache` (TaffyTree-managed) | DOM-node-managed `Cache` (via `CacheTree`) |
| Cost | One extra hop (`Entity → NodeId`) | Direct; no map |
| Win | Familiar high-level surface; easy to evolve | Zero indirection; full control over cache strategy |

The trade space is exactly what [`prior-art/taffy/lessons.md` § "Validates" — "LayoutPartialTree / TraversePartialTree / CacheTree as a clean integration boundary"](../taffy/lessons.md) describes. Buiy's current choice (wrap `TaffyTree`) is the simpler entry point; the trait path is reserved as a future migration option if the `Entity ↔ NodeId` map ever becomes a measured hotspot. Blitz's path is the existence proof that the trait integration is production-viable (under the obvious "Blitz is pre-alpha" caveat — the trait integration is not what's holding it back).

## Cross-references to the Taffy folder

- [`../taffy/README.md`](../taffy/README.md) — Taffy overview, key facts, version status.
- [`../taffy/integration.md`](../taffy/integration.md) — embedder patterns, including Blitz and Bevy UI.
- [`../taffy/architecture.md`](../taffy/architecture.md) — `LayoutPartialTree` / `TraversePartialTree` / `CacheTree` trait split.
- [`../taffy/lessons.md`](../taffy/lessons.md) — the consult-this-when-designing decision file for Taffy.

## Implications for Buiy

- **Validates Taffy's multi-embedder discipline.** Blitz uses the trait surface directly against a non-ECS node arena; Bevy UI and Buiy wrap `TaffyTree` over ECS. One algorithm, two entry points, both production-tier (modulo Blitz's pre-alpha status). Buiy's bet on Taffy as a substrate is doubly validated — multiple consumers, multiple integration shapes, same engine.
- **DioxusLabs steward both Taffy and Blitz.** Funding flows from VC-backed DioxusLabs to Dioxus-the-framework, but **not** to Taffy-the-substrate (Nico Burns is independent — see [`governance.md`](governance.md) and [`../taffy/governance.md`](../taffy/governance.md)). Buiy's Taffy-fork-contingency story remains the right insurance. Blitz being the largest experimental-version consumer means Blitz's needs sometimes shape Taffy's API more than Bevy UI's do; watch the experimental-line work for signals about Taffy's direction.
- **Blitz is a useful reference for the trait-integration path.** If Buiy ever moves off `TaffyTree`-wrapping and onto the trait surface, Blitz's `blitz-dom` is the closest pre-existing implementation worth reading. The mechanical shape (impl three traits against a non-Taffy-owned arena) is what you'd write.

## Sources

- Blitz repo: https://github.com/DioxusLabs/blitz
- `blitz-dom` crate (implements Taffy traits): https://github.com/DioxusLabs/blitz/tree/main/packages/blitz-dom
- `stylo_taffy` glue crate (MPL-2.0): https://github.com/DioxusLabs/blitz/tree/main/packages/stylo_taffy
- Taffy crate: https://crates.io/crates/taffy
- Sibling: [`targets.md`](targets.md), [`../taffy/`](../taffy/)
