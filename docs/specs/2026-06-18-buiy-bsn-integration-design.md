# Buiy BSN integration + Bevy 0.19 — design

**Date:** 2026-06-18
**Status:** landed
**Supersedes:** partially supersedes [`2026-06-03-buiy-render-pipeline-design/architecture.md`](2026-06-03-buiy-render-pipeline-design/architecture.md) **§ 1 (Render-graph integration), esp. § 1.3 (Node placement)** — the `ViewNode`/`ViewNodeRunner<BuiyNode>` model → systems-in-`Core2d`. That living foundation child is updated in the same change, not contradicted.

Realizes the `buiy-bsn-integration-design` sub-spec row of the [foundation roadmap](2026-05-07-buiy-foundation/README.md#4-sub-spec-roadmap). Research substrate: [`docs/reports/2026-06-18-bevy-0.19-bsn-migration-research.md`](../reports/2026-06-18-bevy-0.19-bsn-migration-research.md).

## Purpose

Make Buiy components authorable in **BSN** (Bevy Scene Notation) — the `bsn!` / `bsn_list!` macros and the `Template` / `Scene` system that shipped upstream in **Bevy 0.19** (PR #23413, in the `bevy_scene` crate). This is foundation **goal 3** ("BSN-native") moving from *by-construction readiness* to *actual authoring*.

BSN does not exist on Bevy 0.18 (Buiy's prior pin); the macro is a 0.19 feature. So this work has two coupled halves:

1. **A Bevy 0.18 → 0.19 migration** (the enabler). The blast radius is the render pipeline (wgpu 27→29 + the render-graph-API removal), a handful of ECS renames, and one AccessKit field. Buiy pulls no `bevy_ui`/`bevy_text`, so the upstream cosmic-text→parley and taffy-in-bevy_ui churn does not touch us.
2. **The `buiy_bsn` authoring layer** (the payoff). A thin crate: ergonomic re-exports + ensuring every author-facing Buiy component satisfies the BSN template contract. BSN authoring is **compile-time** and needs **no reflection registry**, so this half is small and low-risk.

This spec is the target state. The phased migration path is [`docs/plans/2026-06-18-bevy-0.19-bsn-migration.md`](../plans/2026-06-18-bevy-0.19-bsn-migration.md).

## 1. Background: BSN's real upstream status

The repo's earlier prior-art ([`prior-art/bevy-ui/`](../prior-art/bevy-ui/), dated 2026-05-22) recorded BSN as "still draft / unmerged" via PR #20158. That is now **stale and half-wrong**, and is corrected as part of this change:

- PR #20158 (the original draft) was never merged in its own form.
- The BSN **baseline merged via the successor PR #23413** ("Next Generation Scenes: core scene system, `bsn!` macro, Templates"), 2026-03-27, milestoned **Bevy 0.19**, living in the **`bevy_scene`** crate.
- The `.bsn` **asset-file loader** was explicitly deferred out of #23413 to a future upstream PR. Inline `bsn! { … }` (and function/`SceneList` scenes) is the landed surface; `asset_server.load("x.bsn")` has no runtime backing yet.
- As of 2026-06-18, Bevy 0.19 is at **0.19.0-rc.3** — a release candidate, not a stable tag.

## 2. Decision: pin Bevy `0.19.0-rc.3` (policy exception)

Foundation [architecture.md § 2.9](2026-05-07-buiy-foundation/architecture.md) commits Buiy to **rolling latest-*stable* Bevy**. Pinning a release candidate is a deliberate, scoped **exception** to that policy, taken because BSN authoring — foundation goal 3 — is unreachable on any stable Bevy, and the user has explicitly chosen to build real `bsn!` now rather than wait for 0.19 stable.

**Tradeoff accepted:** an rc can change before its stable tag, so a second small migration (rc.3 → 0.19.0 stable) is likely. The exception is bounded: when 0.19.0 stable releases, Buiy bumps to it and the exception closes. This is recorded in the dependency notes and `follow-ups.md`.

**Rejected alternatives:**
- *Wait for 0.19 stable.* Rejected: indefinite blocker on the user's explicit goal; the rc is API-frozen enough that the delta to stable is expected to be small.
- *Vendor / fork BSN onto 0.18.* Rejected: BSN depends on 0.19 `bevy_ecs` template machinery; back-porting is far larger than the forward migration and creates permanent divergence.
- *A Buiy-native authoring macro instead of BSN.* **Rejected by existing commitments.** Foundation [architecture.md § 2.4](2026-05-07-buiy-foundation/architecture.md) makes BSN a first-class authoring path against BSN-friendly (decomposed, public-fielded) components — a parallel proc-macro DSL would undercut that. The explicit "Buiy components do not ship their own proc-macro DSL" framing, and the sickle_ui / kayak_ui cautionary tales it rests on, are in [`prior-art/bevy-ui/comparisons.md`](../prior-art/bevy-ui/comparisons.md) and [`prior-art/sickle-ui/`](../prior-art/sickle-ui/) / [`prior-art/kayak-ui/`](../prior-art/kayak-ui/). A parallel authoring layer is exactly the anti-pattern Buiy committed to avoid.

**Dependency deltas** (full matrix in the research report). Buiy-owned pins that change: `bevy` 0.18→0.19.0-rc.3 (+`bevy_scene` feature), `accesskit` 0.21→0.24, `accesskit_winit` 0.29→0.32, dev-dep `naga` 27→29. Transitive, no Buiy pin change: wgpu 27→29.3, winit→0.30.13. Unchanged (Buiy owns them): `cosmic-text` 0.19, `taffy` 0.10, `guillotiere` 0.6.2.

## 3. Render pipeline: render graph → systems-in-schedule

**This is the largest code change and the one durable architecture shift.** Verified directly against vendored `bevy_render`/`bevy_core_pipeline` 0.19.0-rc.3 source.

**What changed upstream.** Bevy 0.19 **removed** the `RenderGraph` `Node`/`ViewNode` API. There is no `bevy::render::render_graph` module, and `add_render_graph_node` / `add_render_graph_edges` are gone. `RenderGraph` is now a render-world **schedule** driven by `RenderGraphSystems::{Begin, Render, Submit, Finish}`. Render passes are ordinary **systems** added to the `Core2d` schedule.

**New pass shape** (canonical, from `main_transparent_pass_2d`):

```rust
pub fn buiy_pass(
    world: &World,
    view: ViewQuery<(&ExtractedCamera, &ExtractedView, &ViewTarget, /* … */)>,
    /* Res<…> render-world resources Buiy needs */
    mut ctx: RenderContext,                 // now a SystemParam, not a &mut arg
) {
    let view_entity = view.entity();
    let (camera, extracted_view, target, /* … */) = view.into_inner();
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("buiy_pass"),
        color_attachments: &[Some(target.get_color_attachment())],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,               // new wgpu-28 field
    });
    // … existing Buiy draw logic, unchanged in spirit …
}
```

**Registration** replaces graph edges with system ordering. The verified 0.19 `Core2dSystems` enum is `{ Prepass, MainPass, EarlyPostProcess, PostProcess }` (chained in that order), with `tonemapping.in_set(PostProcess)`. Buiy's old "after main pass, before tonemapping" slot maps to **`Core2dSystems::EarlyPostProcess`**:

```rust
render_app.add_systems(Core2d, buiy_pass.in_set(Core2dSystems::EarlyPostProcess));
```

**Design invariants the migration must preserve** (the *what*, not the *how* — the plan owns the how):

- **Exact paint ordering.** Buiy drew after the main pass and before tonemapping (the removed 0.18 `StartMainPassPostProcessing → Buiy → Tonemapping` slot). The verified 0.19 equivalent is `Core2dSystems::EarlyPostProcess` (chained after `MainPass`, before `PostProcess`/`tonemapping`) — cite `bevy_core_pipeline` `core_2d/mod.rs:84-90`. System `.before()/.after()` ordering is strictly more expressive than the old linear graph edges, so no guaranteed ordering is lost.
- **Effect-compositor nesting (as realized: one system, straight-line).** The old single `ViewNode::run` did all of Buiy's passes (the effect-group passes, the nested composite, the main pass, the window composite) in one `run`. That is preserved by making `buiy_pass` a **single system** that runs those passes straight-line in body order, sharing **one** `RenderContext` command encoder — which is what keeps the cross-pass `LoadOp::Load` composite loads correct (splitting into multiple systems would give each its own encoder/command-buffer and break those loads). So the inner ordering is preserved by straight-line code, not by `.before()/.after()` between separate pass systems.
- **Single-writer / data-flow seams unchanged.** Extract/prepare stay as they are; only the *pass node* becomes a *pass system*. The render-side component model, clip threading, top-layer partition, and atlas binding are untouched by this shift.

This change is **GPU-lane-verified** (the `#[ignore]` render tests on a real adapter), since the pass only runs with a device.

The render-pipeline spec's [`architecture.md`](2026-06-03-buiy-render-pipeline-design/architecture.md) **§ 1 / § 1.3** (render-graph integration / node placement) is updated in this change to describe the systems model as the realized state.

## 4. The `buiy_bsn` authoring model

Realizes the planned [`buiy_bsn` crate](2026-05-07-buiy-foundation/architecture.md) (foundation architecture § 2.8). Its premise, confirmed against rc.3 source: **BSN authoring is compile-time and reflection-free.**

### 4.1 Component conformance — the template contract

A **component** is `bsn!`-authorable through one of two paths:

1. **Plain-data (blanket) path.** Any `Component + Clone + Default` ( `+ Unpin`, automatic) is its own `Template` via the upstream blanket impl. A `bsn!` patch `Foo { field: val }` layers onto the `Default` base. **This is the common case** — every Buiy component carrying `#[derive(Reflect, Default, Clone, Component)]` (the § 2.4 invariant) qualifies as-is. This covers the entire decomposed style surface (`BoxModel`, `Display`, `Position`, `FlexParams`, `Background`, `Border`, `BoxShadow`, `Opacity`, `A11yRole`, `A11yLabel`, `Focusable`, …).
2. **Context-bearing path.** A component with fields that need spawn-time world access (asset handles from string paths, entity references) derives `#[derive(Component, FromTemplate)]`, generating a `…Template` whose fields are themselves `FromTemplate` (e.g. `Handle<T>` ← `HandleTemplate<T>` resolves `"path.png"` via `AssetServer`). Required only where such fields exist; no current Buiy component needs it (flagged if the audit finds one).

**`bsn!` patches Components, not Bundles.** Upstream `Scene for TemplatePatch<F, T>` requires `T: Template<Output: Component>`. This shapes two Buiy realities:

- **`Style` is a `#[derive(Bundle)]` builder, not a Component** (`layout/style.rs`), so `bsn! { Style { … } }` does **not** compile. This is *correct and intended*: `Style` is Rust-spawn ergonomic sugar that decomposes into the style Components on insert. In BSN you author **the decomposed Components directly** — `bsn! { BoxModel { … } Background(…) }` — which is exactly the decompose-by-concern model (§ 2.4, foundation goal 3). The `Style` builder remains for `commands.spawn`; BSN does not need it. The round-trip test (§ 5) asserts the decomposed-component authoring path compiles and patches.

### 4.1a `#[require]` — from the `Node` marker up

**`Node` requires the style decomposition.** `sync_styles` queries the style components **non-optionally** under `With<Node>` (`Display`/`BoxModel`/`Position`/`FlexParams`/`Overflow`/`Scroll`/`GridParams`/`Containment`/`WritingModeResolved`, illustrative — `systems.rs`) — so a `Node` missing any of them is silently skipped by layout (no `ResolvedLayout`). Those components are therefore **mandatory companions** of a functioning `Node`, and Buiy now declares that explicitly: `Node` `#[require]`s the full style decomposition — the 14 components `Style` bundles (`Display`, `BoxModel`, `Position`, `FlexParams`, `Overflow`, `Scroll`, `GridParams`, `WritingMode`, `Container`, `MultiColumn`, `UiTransform`, `Containment`, `Stacking`, `ContainIntrinsicSize`). (`WritingModeResolved` is *not* required — it is computed per-`Node` by `inherit_writing_mode`, which runs before `sync_styles`, exactly as before this change.) This fixes a latent fragility (bare `spawn(Node)` / `bsn! { Node }` was *not* layout-valid before — only the `Style` bundle or a widget constructor produced a complete node) and makes `bsn! { Node BoxModel { … } Children [ … ] }` a valid container. It mirrors `bevy_ui`'s own `Node` (which requires its computed/transform companions) and is the realization of the "required-components are the architectural prerequisite for BSN" lesson ([`prior-art/bevy-ui/lessons.md`](../prior-art/bevy-ui/lessons.md)). Verified safe: the full headless suite passes (the added components are inert defaults; `sync_styles` already demanded them).

**Widgets require `Node` + their widget-specific companions.** `Button`/`TextInput` were bare markers whose contract was assembled only by the `Button::new()` / `TextInput::single_line()` **`impl Bundle`** constructors (not `Scene`s — so `bsn! { Button }` would spawn a bare marker). They now `#[require]` `Node` (which pulls the style decomposition) plus their own companions (`Background = button_background()`, `Border`, `Focusable`, `A11yRole::Button`, `A11yLabel`, and for `TextInput` the editor mechanism):

```rust
#[derive(Component, Reflect, Default, Clone)]
#[reflect(Component, Default)]
#[require(Node, Focusable, Background = button_background(), Border = button_border(),
          BoxModel = button_box_model(), A11yRole = A11yRole::Button, A11yLabel)]
pub struct Button;
```

A direct `#[require(C = init)]` overrides the default `C` that `Node` would pull (Bevy's "direct require has priority"), so the widget's canonical box/background win. Then `bsn! { Button }` materializes the full default button. The existing constructors stay (back-compat; required-components are a no-op when a companion is already explicitly inserted), with the canonical defaults living in shared `pub(crate)` initializer fns (`button_box_model()`, …) that **both** the `#[require]` and the § 4.1c scene-fns reuse — one source of truth. (For the patch-suppression nuance when field-patching a required component, and the scene-fn remedy, see § 4.1c.)

**Correction:** an earlier draft claimed Buiy "already does `#[require]` in each plugin `build`." That was wrong — Buiy ships **no** `#[require]` today (the 124 calls are `register_type`, unrelated). Adopting it for the widgets is **in scope for this work**; it is what makes `bsn!`-authored widgets actually usable. (Runtime caveat: required-components must be registered before first insertion — satisfied by registering the widget plugins early in `build`, as Buiy already does.)

### 4.1b Children

BSN `Children [ … ]` entries are themselves `Scene`s and inherit the § 4.1 contract — a child `(Button BoxModel { … })` group authors components/markers the same way the root does (and benefits from the same `#[require]` adoption). `children![]` (ECS spawn) and `Children [ ]` (BSN) are **independent** authoring surfaces that do not interleave inside a single literal — author a given subtree in one or the other, not both. Buiy uses neither today; `hello_bsn` introduces the BSN form.

### 4.1c Styling a required component — the merge gotcha + scene-fns

Required-components fill a component **only when it is missing**. So a `bsn!` *field-patch* of a required component starts from that component's **plain `Default`**, not the widget's require-initializer: `bsn! { Button BoxModel { width: 240px } }` yields `BoxModel { width: 240px, ..Default }` — i.e. padding `0`, **not** the button's canonical 8px. The require-init is suppressed because the patch makes `BoxModel` present. (Verified against `bevy_scene` 0.19-rc.3; the § 5 round-trip test pins this behavior explicitly.)

`bsn!{ Button }` (no patch) is unaffected — it materializes the full correct contract. The gotcha only bites when field-patching a required component.

**Remedy — widget scene-fns.** BSN patches of the *same component* **merge field-wise** (verified upstream: `bevy_scene/src/lib.rs` composition tests — `bsn! { enemy() Health { max: 200 } }` over `fn enemy() -> impl Scene { bsn! { Health { current: 100, max: 100 } } }` yields `Health { current: 100, max: 200 }`). So Buiy ships **parameterized widget scene-fns** in `buiy_widgets` — `button(label) -> impl Scene`, `text_input_single_line(...)`, `text_input_multi_line(...)` — whose bodies spell the widget's styling as explicit `bsn!` field-patches (reusing the **same** private initializer fns the `#[require]` uses, so the two paths share one source of truth). Then `bsn! { button("Save") BoxModel { width: 240px } }` merges field-wise and **keeps** the 8px padding. This is the idiomatic, upstream-blessed styled-authoring surface; it adds no proxy DSL (builds on `bsn!` + the widget components), satisfying § 4.2. The scene-fns live in **`buiy_widgets`** (so they reuse the widgets' private require-initializer fns as one source of truth — `bsn!{ button("X") }` is byte-equal to `spawn(Button)`) and are re-exported through **`buiy::prelude`**. They are *not* re-exported from `buiy_bsn`: that crate stays widget-agnostic (§ 4.2 — depends only on `bevy`+`bevy_scene`, not the widget catalog); `buiy` (the meta-crate) is where the macro layer and the widget catalog converge for the user.

**Conformance is therefore: an audit (most components already pass path 1) + a bounded widget refactor (`#[require]` for the bare/ECS contract, scene-fns for ergonomic styled authoring).** The § 5 round-trip test proves all of it: bare components, the `#[require]` contract (`bsn!{ Button }`), the suppression behavior under a raw patch, the scene-fn merge, and a `Children`-nested subtree.

### 4.2 Crate shape

`crates/buiy_bsn` is intentionally thin:

- **Re-exports** `bsn!`, `bsn_list!`, and the spawn extension traits (`WorldSceneExt`, `CommandsSceneExt`, `EntityWorldMutSceneExt`, …) into a `buiy_bsn::prelude`, so Buiy users get BSN authoring without taking a direct `bevy_scene` dependency or learning bevy's prelude layout.
- **No new authoring syntax.** It does not wrap or re-skin `bsn!`. The macro vocabulary is Bevy component types (Buiy's own components), per the dioxus prior-art lesson ("resist HTML cosmetics; component types are Rust identifiers").
- Depends only on `bevy` (with `bevy_scene`) + `buiy_core` types as needed for tests. Pulls no `bevy_ui`/`bevy_text`. `bevy_scene` reaches `buiy_bsn` (and every workspace member) via the additive workspace `bevy` feature list — no per-crate feature edit needed.

**Meta-crate surface (decision).** The `buiy` meta-crate re-exports `buiy_bsn` so downstream users reach BSN through the one crate they already depend on (as `hello_button`/`hello_text` do today). BSN lives at **`buiy::bsn`** (`pub use buiy_bsn as bsn;`) **and** its prelude is folded into `buiy::prelude`, so `use buiy::prelude::*;` brings `bsn!` into scope. `crates/buiy/Cargo.toml` gains a `buiy_bsn` path dependency. This wiring is a required plan task (Phase 4) — without it `hello_bsn` (which depends only on `buiy`) cannot reach the macro.

### 4.3 Registration ergonomics — decision

The foundation [open question](2026-05-07-buiy-foundation/README.md#5-open-questions) ("`register_type` via derive macro, per-crate sub-plugin, or single global plugin") is **resolved: per-crate plugin**, ratifying the de-facto pattern already in the code (each plugin's `build` runs its `register_type::<T>()` batch; 124 calls across 9 plugins today).

Rationale: it already works, keeps registration co-located with the components and their `#[require]`/observer wiring, and adds zero macro magic. **Crucially, BSN authoring does not consume the type registry at all** — registration matters only for the *future* editor/inspector and the *deferred* `.bsn` asset loader. So this question has low stakes for `bsn!` and the simplest answer is correct. Rejected: a derive-macro emitting `register_type` (hidden control flow, harder to audit which types are registered) and a single global plugin (couples all crates, breaks the opt-in-surface principle).

### 4.4 Scope boundaries — what this does NOT include

- **`.bsn` asset files.** Deferred upstream (no loader in rc.3). Buiy targets inline `bsn!` + function/`SceneList` scenes only. The `.bsn` pipeline stays with the (still unwritten) `buiy-asset-pipeline-design` sub-spec; tracked in `follow-ups.md`.
- **Component hot-reload.** Depends on the `.bsn` loader; deferred with it.
- **Editor / inspector reflection consumers.** Out of scope; the reflection registry that serves them is already maintained for free by § 4.3.
- **A reactivity/signal layer.** Unchanged non-goal (foundation § 1.3).

## 5. Verification

- **Round-trip authorability test** (headless — the "it works without a GPU" proof). Spawn `bsn! { … }` trees against a `World`, then query the resulting entity and assert components + patched field values. Exercises the real `Template`/`Scene` path, not a mock. **Required cases:** (a) bare plain-data components (`Background`, `BoxModel`, …) with field patches; (b) the `#[require]` contract — `bsn! { Button }` materializes the full companion set, and a raw field-patch `bsn! { Button BoxModel { width: … } }` exhibits the § 4.1c suppression (unpatched fields = `Default`), pinned explicitly; (c) the **scene-fn merge** — `bsn! { button("Save") BoxModel { width: … } }` keeps the widget's other canonical defaults (padding), the ergonomic styled path; (d) a `Children [ … ]`-nested subtree; (e) a `#Name` entity ref.
- **`hello_bsn` example** (`examples/hello_bsn`, mirrors `hello_button`/`hello_text`). Authors a real Buiy widget tree with `bsn!` and renders it. **Style widgets via the scene-fns** (`button("…")`) or full-component patches — **never** a single-field patch of a `#[require]`'d component (it would visibly drop the widget's other defaults, e.g. padding → 0, a misleading demo; § 4.1c). Gated by a headless display-list/layout snapshot via the `buiy_verify` harness (use the `using-buiy-verification` skill); visual smoke via `cargo run -p hello_bsn`.
- **The migration gates** (per the plan): headless gate green (CI, no adapter) and the GPU `--ignored` lane green (real adapter), independently — the standing two-lane discipline. The wgpu/render-graph changes are verified on the GPU lane; everything else on the headless gate.
- **Reflect-gap closure** (§ 6) is covered by the existing per-plugin registration plus the round-trip test.

## 6. Reflect-registration gaps closed alongside

The BSN-readiness audit found 8 stragglers in the by-construction invariant; closed as part of this change (they are pure conformance hygiene, independent of the macro):

- `Display` (`layout/components.rs:58`), `FlexItem` (`:131`), `LayoutAnchorBroken` (`:595`) — add `Default` to the `#[reflect(Component, …)]` list (component already derives `Default`).
- `CaretColor` (`render/components.rs:77`) — add a **manual** `Default` matching `caret-color: auto` (mirroring `TextColor`'s `CurrentColor` default), then `#[reflect(Component, Default)]`.
- Register the `FontFamily` nested value types `FontStack` / `FamilyEntry` / `GenericFamily` in `BuiyTextPlugin::build` (a real reflect round-trip gap, not cosmetic).
- `Angle`, `FilterFn` (`render/components.rs:243,250`) — `Default` for parity (note-level).

These matter for the *future* reflection consumers / `.bsn` loader, not for `bsn!` itself, but they close the audit and cost little.

## 7. Deferrals / follow-ups

Recorded in [`docs/plans/follow-ups.md`](../plans/follow-ups.md):

- rc.3 → Bevy 0.19.0 **stable** bump when stable releases (closes the § 2 policy exception).
- `.bsn` asset-file loader + component hot-reload (await upstream loader; `buiy-asset-pipeline-design`).
- Editor/inspector reflection consumers (`buiy-devtools-design`).
- Re-bless of any WGSL shader goldens perturbed by naga 29's stricter const-eval (verify pixel-correctness first; never to silence a failure).

## 8. Open questions

**Resolved by this spec:** registration ergonomics (per-crate plugin, § 4.3); rc-pin policy exception (§ 2).

**Remaining / deferred:** the `.bsn` loader contract and WASM `.bsn`/registration policy stay open under the asset-pipeline sub-spec; whether a Buiy `bsn!`-fixture should join the verification matrix as a permanent gate (vs. the one `hello_bsn` example) is a `buiy-verification-design` question, deferred until the example exists.

## References

- Research report — [`docs/reports/2026-06-18-bevy-0.19-bsn-migration-research.md`](../reports/2026-06-18-bevy-0.19-bsn-migration-research.md).
- Migration plan — [`docs/plans/2026-06-18-bevy-0.19-bsn-migration.md`](../plans/2026-06-18-bevy-0.19-bsn-migration.md).
- Foundation — [architecture.md § 2.4 (authoring), § 2.8 (crates), § 2.9 (Bevy policy)](2026-05-07-buiy-foundation/architecture.md); [README goal 3, roadmap, open questions](2026-05-07-buiy-foundation/README.md).
- Render pipeline — [architecture.md](2026-06-03-buiy-render-pipeline-design/architecture.md) (render-graph section updated by this change).
- Bevy PR #23413 (BSN baseline); `bevy_scene` 0.19.0-rc.3 docs; bevy 0.19 migration guide.
- BSN-readiness audit — workflow `bsn-support-analysis` (the 8 reflect gaps, § 6).
