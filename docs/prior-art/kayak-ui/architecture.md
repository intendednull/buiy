**Date:** 2026-05-22
**Status:** archived
**Subject:** kayak_ui — architecture: React-style declarative UI, custom `rsx!` macro, parallel render + layout + focus stack.

# Architecture

kayak_ui was a **parallel UI stack** in the same sense Buiy aims to be — its own component model, its own render pipeline, its own layout engine, its own focus tree — but assembled from a different set of substrate decisions and with a fundamentally different authoring paradigm. Buiy is ECS-native + BSN-friendly-by-construction; kayak_ui was React-native (function widgets + state hooks + JSX-analog DSL), with ECS used as plumbing under the hood.

## The React-style paradigm in Rust

kayak_ui's load-bearing decision was to bring **React's mental model** — function components, props, state hooks, declarative trees, top-down composition — into Bevy. A widget was a function (or unit struct with a function-shaped `render` impl) that returned a tree of children. State lived in hook-like primitives; re-renders happened when state changed; children re-evaluated when props changed.

This was a deliberate departure from the prevailing Bevy idioms (entities + components + systems with no implicit re-render concept). Reactivity was kayak_ui's own concern, not Bevy's. The cost: an entire reactive runtime layered on top of ECS, plus a custom DSL to make it ergonomic, plus a custom renderer because the React-style render-vs-real-tree split doesn't map cleanly onto bevy_ui's "the entity tree is the UI."

For Buiy, this is the **shape we explicitly reject** (per [`../../specs/2026-05-07-buiy-foundation/architecture.md` § Reactivity](../../specs/2026-05-07-buiy-foundation/architecture.md) — observers + change detection as the reactivity primitive in v1, signals only as a follow-up). See [`lessons.md`](lessons.md) § Avoid.

## The `rsx!` macro

The DSL was a proc-macro named **`rsx!`** (exported from the `kayak_ui_macros` companion crate alongside a `constructor` macro). It parsed a JSX-like syntax and expanded to ECS spawn calls:

```rust
rsx! {
    <KayakAppBundle>
        <WindowBundle styles={KStyle::default()}>
            <TextWidgetBundle text={"Hello".into()} />
            <KButtonBundle on_event={OnEvent::new(...)}>
                <TextWidgetBundle text={"Click me".into()} />
            </KButtonBundle>
        </WindowBundle>
    </KayakAppBundle>
}
```

The macro lived **outside** Bevy's `Component` derive ecosystem — it knew nothing of Bevy's reflection registration, nothing of BSN syntax (which had not yet been drafted when kayak_ui was designed), and required `Bundle`-shaped types as its tag names. Every breaking change to Bevy's `Bundle` semantics (and there were several across 0.9 → 0.12) was a `rsx!`-expansion-site migration event. See [`history.md`](history.md) § Bevy compat.

This is the **custom-DSL-friction** lesson per [`lessons.md`](lessons.md): a third-party crate maintaining its own widget-declaration macro outside the host engine's macro system pays a recurring tax with every Bevy minor release. Bevy's later move toward BSN ([discussion #14437](https://github.com/bevyengine/bevy/discussions/14437), [PR #20158](https://github.com/bevyengine/bevy/pull/20158)) signals that the *engine itself* will own the declarative-authoring syntax — making any third-party DSL a parallel-and-shrinking surface.

## Plugin shape

Consumers wired kayak_ui into a Bevy app via two pieces:

```rust
App::new()
    .add_plugins(DefaultPlugins)
    .add_plugin(KayakContextPlugin)   // <- Bevy Plugin
    .add_plugin(KayakWidgets)         // <- bundled default widgets
    .add_systems(Startup, startup);
```

- **`KayakContextPlugin`** — the Bevy `Plugin` that registered systems + resources, set up the render pipeline, initialized the layout engine, and wired input routing.
- **`KayakWidgets`** — a separate Bevy plugin that registered the bundled default widgets (`KButton`, `KWindow`, `TextBox`, etc.) — opt-in if a consumer wanted to ship their own widget set.

A separate concept — **`KayakUIPlugin`** — is a kayak_ui-internal trait (NOT a Bevy `Plugin`) used to extend a `KayakRootContext`:

```rust
pub trait KayakUIPlugin {
    fn build(&self, context: &mut KayakRootContext);
}
```

The naming collision (`KayakUIPlugin` the trait vs `KayakContextPlugin` the Bevy `Plugin` vs `KayakWidgets` the second Bevy `Plugin`) was a frequent source of confusion in the issue tracker and tutorials, and is itself a small lesson: **don't shadow the host engine's plugin nomenclature with a trait of nearly the same name.**

## Render path

kayak_ui shipped its own **custom render graph node** (`KayakUiPass`, in the `DrawUiGraph`) running parallel to bevy_ui's render. Highlights:

- **MSDF font rendering** — kayak_ui rasterized fonts using Multi-channel Signed Distance Fields, distinct from bevy_ui's bitmap-atlas approach. MSDF gave it sharp-at-arbitrary-scale text but required pre-baked font assets in MSDF format.
- **Quad rendering with rounded corners** — kayak_ui drew rounded-corner quads via its own shader, not via bevy_ui's renderer (bevy_ui's rounded-corner support shipped later and has had open clipping bugs ever since — see [`../bevy-ui/critiques.md`](../bevy-ui/critiques.md)).
- **Custom UI nodes** — consumers could provide their own draw primitives via a `MaterialUI` extension surface, analogous to bevy_ui's later `UiMaterial` (kayak_ui's shipped earlier).
- **Batched rendering** — quads with the same material/clip context batched into single draw calls.
- **Opacity layers** — for animating subtree opacity without per-node alpha multiplication.
- **DPI scaling** — handled in kayak_ui's render, not delegated to bevy_render's window-scale.

The render-graph-integration pattern (own a render node, do not draw via bevy_ui) **does** validate one Buiy choice (foundation [`architecture.md` § 2.3](../../specs/2026-05-07-buiy-foundation/architecture.md)). But the *substrate* choices (MSDF vs SDF vs bitmap atlas, custom rounded-corner shader, custom batching) are kayak_ui-era artifacts of a moment when bevy_render lacked the primitives Buiy can now build on. See [`lessons.md`](lessons.md) § Borrow item 4.

## Layout

kayak_ui chose **[morphorm](https://github.com/vizia/morphorm) 0.3** as its layout engine — explicitly *not* Taffy. Morphorm is a one-pass, row/column algorithm with fewer concepts than flexbox/grid; it's smaller, simpler, and produces "similar-to-flexbox" layouts without supporting the full flexbox spec.

For Buiy, this is a load-bearing **don't-do-this**: Taffy is now Bevy's layout substrate, has shipped CSS Grid (Taffy 0.3), block (0.4), float (0.10), and named-line grid (0.9), and is the upstream Buiy commits to ([`../taffy/`](../taffy/), [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § Validates "Taffy as the layout substrate"). Picking a parallel layout engine in 2022 was defensible; doing the same today would orphan Buiy from the entire bevy_ui layout-bug fix stream. kayak_ui's morphorm pin is a frozen 2022-era decision; the project ate the divergence quietly.

## Focus tree

kayak_ui shipped its own focus tree (the `FocusTree` resource exposed in 0.5.0 release notes — "added focus tree as a resource"), a `Focusable` trait, and tab navigation. This is conceptually right (matches [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md) § Avoid "Per-app reinvention of a focus model" — Buiy needs *one* coherent focus tree). The execution was limited: no `:focus-visible` semantics, no focus traps in 0.5.0 (despite the `Modal` widget shipping), no documented restoration policy on dialog close, no spatial gamepad nav. See [`critiques.md`](critiques.md) § Focus model.

## Component model + reactivity

The internal model:

- **Widget = function (or zero-sized struct with a function-shaped render impl)** taking `KStyle` + props + children, returning a child tree.
- **State via hook-like primitives** (e.g., `context.use_state(...)` style calls binding state to an entity/widget identity).
- **Re-evaluation on prop or state change**, triggering re-spawn / re-update of child entities.
- **ECS as plumbing** — every widget materialized as an entity with components attached, but the entity tree was *derived from* the React-style tree, not the other way around.

The 0.5.0 release notes explicitly mention "improved dashmap usage, fixed key entities and widget state management, resolved tree removal issues" — pointing to the load-bearing-and-fragile nature of the widget-identity / entity-mapping reconciliation. This is the kind of internal complexity StarArawn cited when introducing woodpecker_ui: "*Kayak UI suffered from overly complicated internals*"; "*reducing the primary system from over 1,000 lines to fewer than 200*" (woodpecker_ui README, see [`history.md`](history.md)).

For Buiy, this is the **load-bearing reason** to stay ECS-native: in Bevy, the entity tree *is* the UI, and reactivity is observers + change detection (per foundation [`architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)) — not a virtual-DOM-style reconciliation layered on top. See [`lessons.md`](lessons.md) § Avoid.

## Sources

- kayak_ui README — https://github.com/StarArawn/kayak_ui#readme
- kayak_ui docs.rs (0.5.0) — https://docs.rs/kayak_ui/0.5.0/kayak_ui/
- kayak_ui lib.rs (KayakUIPlugin trait, prelude) — https://github.com/StarArawn/kayak_ui/blob/main/src/lib.rs
- woodpecker_ui README (successor; cites kayak_ui internal-complexity rationale) — https://github.com/StarArawn/woodpecker_ui#readme
- morphorm layout engine — https://github.com/vizia/morphorm
- Buiy foundation architecture spec — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Bevy BSN discussion #14437 — https://github.com/bevyengine/bevy/discussions/14437
- Bevy BSN PR #20158 — https://github.com/bevyengine/bevy/pull/20158
