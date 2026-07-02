**Date:** 2026-07-01
**Status:** Prototype — exploratory, DO NOT MERGE the code. The deliverable is the journal + retrospective.
**Base:** `origin/main` @ `6c4ff22` (post MVU-as-core #87, MVU-prelude #93, demos→MVU #91).
**Worktree:** `interface-proto` (branch `worktree-interface-proto`).
**Seeds:** the [authoring-paradigm comparison](2026-07-01-authoring-paradigm-comparison.md), the [view-declaration LLM panel](2026-07-01-view-declaration-llm-panel.md) (unanimous for the view-function), the [demos→MVU migration journal](../reports/2026-06-30-demos-mvu-migration-journal.md) (DX-2/DX-3), and the [DX audit](../reports/2026-06-25-developer-experience-audit.md) frictions F1–F8.

# "Safer V" authoring surface — prototype design

## Goal (what we build to LEARN)

Build the **Iced-style view-function** authoring surface on top of MVU-as-core, dogfood it by re-authoring the real demos in it, **RUN** them, and journal where it holds and where it breaks. The one-sentence bet: *an app author writes `Model` + `enum Msg` + `fn update(&mut Model, Msg) -> Cmd<Msg>` + `fn view(&Model) -> Element<Msg>`, and everything else (routing, binding, reconcile, a11y projection) is the library's job* — killing the demos-migration report's two dominant frictions:

- **DX-2 — no declarative View.** Today authors hand-write a `Query<&Model, Changed<Model>>` bind that imperatively projects each field onto view entities. This prototype provides `view(&Model) -> Element<Msg>` + a reconciler.
- **DX-3 — no `OnPress → Model` routing.** Today every press→model edge is a hand-rolled `route_*` enqueue system with hand target-resolution. This prototype provides `on_press(Msg)` / `on_input(fn)` / `on_toggle(fn)` that route to the owning model automatically.

**This is the road the maintainer leans to AND a unanimous 6–0 LLM-authorability panel picked** (near-Iced training priors, plain-Rust fallback, uniform dot-API, clean compiler errors). The panel's one warning — V's `.key()` is a *silent-omission* landmine — is designed out here via a **required-key** list API.

## The surface (what the author writes)

```rust
use buiy::prelude::*;              // Model, Cmd, Element, widgets, tokens — one import
use buiy::view::*;                 // column/row/container, button, text, text_input, checkbox, keyed_column

#[derive(Model, Default)] #[model(msg = Msg)]
struct Counter { count: i32 }

#[derive(Clone)] enum Msg { Inc, Dec, Reset }

fn update(s: &mut Counter, m: Msg) -> Cmd<Msg> {
    match m { Msg::Inc => s.count += 1, Msg::Dec => s.count -= 1, Msg::Reset => s.count = 0 }
    Cmd::none()
}

fn view(s: &Counter) -> Element<Msg> {
    column![
        text!("Count: {}", s.count),
        row![
            button("−").on_press(Msg::Dec),
            button("+").on_press(Msg::Inc),
            button("Reset").on_press_maybe((s.count != 0).then_some(Msg::Reset)),   // declarative disable
        ].gap(Space::Sm),
    ].gap(Space::Md).into()
}

fn main() { App::new().add_plugins(BuiyPlugins).ui(Counter::default(), update, view).run(); }
```

**Design commitments for the prototype (each a thing to validate by running):**

- `Element<Msg>` — an inert description value: a widget kind + typed props + modifier state + typed `Msg` handlers + optional key + children. **Not** an ECS entity.
- **Uniform dot-method API** — every knob is a method (`.gap`, `.padding`, `.on_press`, `.on_input`, `.on_toggle`, `.key`, `.background`, `.into`). No method-vs-bare-attribute split (the panel's #2 complaint about the macro tree).
- **Required-key lists** — `keyed_column(iter, |x| x.id, |x| Element)` makes the key a mandatory argument (mirrors the tree's mandatory `key:`), closing V's silent-`.key()` landmine. A plain `column(iter_of_elements)` exists only for static/never-reordered lists.
- **Typed tokens** — `Space::Md`, `Color::Surface`, `Radius::Lg` are enums/newtypes resolved at build time (F6), not stringly keys.
- **`ui(init, update, view)`** — an `App` extension that spawns the model entity, registers the reducer, and installs the reconciler + router. One call.

## The library's job (what we build — the uncertain core)

1. **Reconciler.** On `Changed<Model>`, call `view(&model)`, diff the returned `Element` tree against the retained Buiy widget entities under the model root, and **patch / spawn / despawn** to match — reusing the existing `buiy_widgets` scene-fns for the actual entities. Keyed children reconcile by key (reorder without losing per-widget state). This is the heart of the prototype and the biggest unknown (Buiy is retained; the demos hand-bind precisely because this didn't exist).
2. **Router.** Translate an `Element`'s typed `Msg` handlers into the MVU funnel: a `button(..).on_press(Msg::X)` lowers to enqueuing `X` on the owning model entity via the real `enqueue`, so **record/replay + agent-drive survive** (the whole reason MVU is core). No stored closures on components where a value suffices; where a closure is needed (`on_input(fn(String)->Msg)`), it lives on the `Element`, consumed by the router, not stored on an entity.
3. **Editor bridge.** `text_input(value).on_input(Msg)` binds the draft: value flows from the model, edits enqueue a `Msg`. Must reconcile with the command-sourced editor (§6) — a known pressure point.

## Waves (build → RUN → journal each; sequential in this warm worktree)

- **W1 — Element core + reconciler + router + Counter.** Build `Element`, the widget builders + modifiers, the `ui()` ext, the reconciler (patch/spawn/despawn, no keys yet), the press router. Re-author `hello_button`/Counter in it. **RUN the window**; confirm −/+/Reset work and the label updates. Journal: does the reconciler actually patch in place (respect `set_if_neq` / no full-tree churn)? does `on_press` route without a hand-written system?
- **W2 — `keyed_column` + TodoMVC.** Add keyed reconcile + the text-input/on_input editor bridge; re-author `examples/todomvc` in the surface. **RUN**: add, toggle, clear, and the derived "N left". Journal: the **keyed-list-replay wall** (§7.4 unproven) — does add/remove/toggle reconcile correctly and does replay hold or break? the editor-value→Msg bridge cost. Borrow/closure ceremony in the `on_toggle` per-row handler.
- **W3 — scaling: conditional + child sub-view + async.** A view with a runtime conditional branch, a **child sub-component with its own child `Model`** (message-lifting: `.map(ParentMsg::Child)` or the MVU machine-tier), and an **async `Cmd::task`** (a fake load that folds a `Msg` back). **RUN**. Journal: how message-lifting feels; whether child models compose or fight the single-`view` shape; whether async folds cleanly.
- **W4 (stretch) — tokens end-to-end + cost.** Typed tokens through the whole surface; measure the reconciler's steady-frame `node_rebuilds`/work-counters vs the hand-bind baseline (does rebuild+diff defeat `set_if_neq`?).

## Verification (RUN — don't trust green)

- Each wave: `cargo run` the example, view the window (real GPU), confirm the interaction by hand. Headless logic tests for update/reconcile where cheap. This is the prototype's whole point — the demos-migration report and the widget-catalog history both show headless-green ≠ works.

## Learning questions the retrospective must answer

1. Does `view(&Model) -> Element` + reconciler actually remove DX-2/DX-3 (measured against the current hand-bind todomvc)?
2. Does it lower cleanly onto MVU — funnel-routed events, replay preserved, no bypass?
3. Where does it break? (keyed-list replay §7.4, editor bridge, borrow/closure ceremony, reconciler steady-frame cost.)
4. Is `keyed_column`'s required key the right shape? Is `on_press_maybe` the right disable spelling?
5. Does child-model / message-lifting compose, or does it need the MVU machine tier?
6. Verdict: is this worth productionizing as a `buiy_view` surface in Phase B — and what changes (KEEP / REFINE / REDESIGN)?

## Non-merge

The prototype crate (`examples/safer_v_proto` or a throwaway `buiy_view_proto`) is **throwaway** — it optimizes for learning + speed-to-running, not polish. The FINAL (Phase B, a re-decided `staged-development` pass) re-decides every choice with the full picture and productionizes, merge-gated on human review. Only the CODE is thrown away; this doc, the journal, and the retrospective are the deliverables and are committed here.
