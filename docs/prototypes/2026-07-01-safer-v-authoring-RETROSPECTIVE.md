**Date:** 2026-07-01
**Status:** Prototype retrospective — the learning gate. Seeds the FINAL's research.
**Prototype:** `worktree-interface-proto` @ `11b94fb` (off `origin/main` @ `6c4ff22`), commits `7d67df7` (W1) · `f185e6c` (W2) · `11b94fb` (W3). **Code is throwaway — DO NOT MERGE.**
**Journal:** [safer-v-authoring-journal.md](2026-07-01-safer-v-authoring-journal.md). **Design:** [prototype spec](2026-07-01-safer-v-authoring-prototype.md).

# Safer-V authoring — prototype retrospective (for the final)

## Verdict

**The target is achievable and the bet holds.** The Iced-style `view(&Model) -> Element<Msg>` surface + a reconciler + typed routing, built on MVU-as-core, was implemented and **RUN** across three apps of rising difficulty — Counter (W1), TodoMVC (W2), a scaling demo (W3) — with **9/9 headless tests green and every claim re-verified on a real GPU** (I recompiled and viewed the pixels each wave; the recurring rust-analyzer `E0583`/proc-macro errors were stale LSP lies every time). Every app-author file is **`Model` + `Msg` + `update` + `view` and nothing else** — zero hand-written `route_*`/`bind_*` systems — so it **kills the demos-migration report's DX-2 (no declarative view) and DX-3 (no `OnPress→Model` routing)** outright.

The strongest positive result was unplanned: because safer-V is **one model + a derived view**, **whole-UI record/replay becomes a property of the model alone** (W2). All rows/checkboxes/fields are pure derived view the reconciler rebuilds from the replayed model, so the §7.4 keyed-list-replay wall the MVU design flagged as *unproven* actually **holds** here. The single sharpest gap is **`Cmd::task` (async from `update`) is a confirmed MVU roadmap hole** (W3) — the one thing the final must add to `buiy_core`.

## Validated — KEEP (port as-is, re-derive the rationale)

- **`view(&Model) -> Element<Msg>` + reconciler as the app-author surface.** Kills DX-2/DX-3; corroborated by the 6–0 LLM panel + the demos report. Patch-in-place proven (entity ids stable across folds; respects `set_if_neq`).
- **Typed routing via marker components carrying a `Msg` value** (`PressAction<M>`; `route_presses<M>`/`route_text_input`/`route_text_submit` are library-generic). No stored closures where a value or a bare `fn` (an enum tuple-variant ctor, e.g. `Msg::SetDraft: fn(String)->Msg`) suffices → **replay-safe, funnel-lowered through the real `enqueue`**.
- **`keyed_column(iter, key_fn, view_fn)` with a REQUIRED key** → keyed child reconcile (spawn-new / despawn-gone / **reorder-in-place** without rebuild; row identity + `A11yToggled` preserved). This is the deliberate fix for the panel's silent-`.key()` landmine.
- **`when(cond, el)` + `Kind::Empty`** for conditionals — hide/show is a `content↔Empty` kind-swap at a **stable index**, so siblings keep identity (a bare `Option` that changes child count causes positional churn — avoid).
- **`Element::map(ParentMsg::Child)` message-lifting + parent-owned child sub-state** as the composition default (W3): the real Counter embedded twice, isolated, `view`/`update` reused verbatim, replay-preserving. Reserve the MVU **machine tier** for widgets with independent lifecycles (it fragments state across entities → breaks whole-UI single-model replay).
- **The "one model + derived view" shape** — the architectural win that makes replay a model property.
- **`ui(init, update, view)` as a one-call `App` ext** (spawns the model entity + a stable `LogicalId`, registers the reducer, installs reconciler+router).

## REFINE / REDESIGN (the final does these differently)

- **REDESIGN — `Cmd::task` (the #1 item).** Async must be a value returned from `update`: `Cmd::task(future, |r| Msg::Loaded(r))`, with the folded result stamped **`Origin::Command`** so replay re-plays the *recorded result*, not the non-deterministic effect (this is why `Origin::Command` is already reserved in the log format). The prototype hand-drove async via `AsyncComputeTaskPool` + `enqueue` outside `update`; the final makes it first-class in `buiy_core::mvu`. Also add **`Cmd::map`** for child async.
- **REFINE — styling must be patchable (component-shaped, not a `Bundle`).** `Style` is a `#[derive(Bundle)]`, so the reconciler could only apply container style **at spawn, never patch it** (F4 biting the reconciler — the biggest structural limitation of W1). The final needs a **decomposed-style patch path** (`FlexGap`/`BoxModel`/`FlexParams`), or the surface emits the decomposed components directly.
- **REFINE — a widget "slot" contract.** The reconciler had to walk into a `Button`'s child `Text` + root `A11yLabel` to patch the label (widget internals leak into the reconciler). The final needs a clean "what is this widget's label/content slot" contract so the reconciler isn't coupled to each widget's child layout.
- **REFINE — the `on_input`/closure-lift story.** `.map` cannot lift `on_input` (a bare `fn` can't compose; a runtime-valued handler needs a boxed `Fn`). An input-bearing child component needs a boxed `Fn` handler + a runtime purity check. Decide this explicitly.
- **REFINE — reconciler timing.** It ran in `MvuSet::Bind` (after `BuiySet::Layout`), so structurally-new nodes lay out one frame late. The final should reconcile **before** layout.
- **REFINE — a "controlled leaf" mode.** The core's `advance_toggle_on_press` leaf can't be suppressed per-widget, so a checkbox **double-folds** (leaf + model route), converging only via single-writer `Set`-on-drift. The final should let the surface cleanly **own** a widget as a controlled leaf.
- **REFINE — `ui()` type inference.** Pin `M` from `init`+`update`+`view` (not the reducer alone, which leaves `view`/`init` unconstrained).

## Framework/system gaps the prototype surfaced (by RUNNING)

- **`Cmd::task` roadmap gap** — `Cmd` is `None|Emit|Batch` only (`mvu/mod.rs:114`); a pure `update` cannot launch an effect. The sharpest DX gap. (REDESIGN above.)
- **`Style`-is-a-Bundle** ⇒ not reconciler-patchable (F4, confirmed as a reconciler blocker).
- **Enqueue-only-on-drift is load-bearing** — the drain records every folded Msg incl. idempotent `set_if_neq`-noops, so a controlled `Set` enqueued every frame would flood the replay log. The reconciler must enqueue a controlled write only on drift.
- **`advance_toggle_on_press` unsuppressible per-widget** ⇒ checkbox double-fold.
- **Editor seam:** `clear ≠ Insert("")` (empty insert doesn't delete the selection → needs `SelectAll`+`Delete`); controlled sync must use the low-level `apply()` seam (no `TextChanged`/`EditLog` feedback loop).
- **Cosmetic:** the `✓` glyph (U+2713) renders as tofu/narrow (default font lacks it) — same class as the widget-catalog checkbox; not a correctness issue.

## Residual gaps for the final to close

- **W4 was not built:** measure the **reconciler's steady-frame cost** (`node_rebuilds`/work-counters) vs the hand-bind baseline — does rebuild+diff defeat `set_if_neq` at steady state? — and wire **typed tokens** end-to-end (F6). This is a go/no-go the final must gate.
- **Byte-identical editor-internal replay** (mid-edit caret/selection) needs stable field `LogicalId`s; today only the *value* is model-reconstructed.
- **`Cmd::map`** / async child composition; the boxed-`Fn` `on_input`-lift path.
- A production **widget-slot contract** + the decomposed-style patch path.

## Build strategy (Phase B — the final)

- **Shared base (`6c4ff22`)** ⇒ the validated prototype commits are cherry-pickable — an **audited port**, not a rebuild. Port the KEEP work (Element / reconciler / router / `keyed_column` / `when` / `map`) into a real crate (candidate: **`buiy_view`**), and **re-implement the REFINE/REDESIGN items as deliberate commits** (`Cmd::task` in `buiy_core::mvu`, patchable styling, the widget-slot contract, controlled-leaf mode, reconcile-before-layout).
- **Scope decision the final's spec must make:** `Cmd::task` touches `buiy_core::mvu` (a roadmap item) — decide whether to land it in core as part of this surface, or split it into its own PR the surface depends on. Likewise decide the `buiy_view` crate boundary and whether the reconciler lives in core or a new crate.
- **Re-decide every choice with the full picture** (the prototype decided sequentially, blind to downstream). Then execute in gated waves, **RUN each**, and **merge-gate on human review** (do not self-merge).
