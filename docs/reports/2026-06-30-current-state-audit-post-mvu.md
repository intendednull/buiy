**Date:** 2026-06-30
**Status:** active

> Audits `origin/main` @ `abb76fb` (the PR #87 *MVU-as-core* merge, 2026-06-30), the post-merge resumption point of the developer-experience / interface redesign (paused while state-management was decided). Companion to the [DX audit](2026-06-25-developer-experience-audit.md) (frictions F1–F8) + the [UI-DX prior-art research](2026-06-25-ui-dx-composition-prior-art.md). Method: a 7-agent grounded read of the merged surface (MVU API+wiring, spec↔code alignment, tiered widgets + a11y/AT seam, authoring DX, the F1–F8 re-scorecard, broader state/health, backlog), every claim cited to `file:line`, with headline claims hand-verified and current `main` confirmed compile-clean (`cargo test --workspace --no-run`, exit 0).

# Buiy — Current-State Audit (post-MVU-as-core)

## 1. Headline — where things stand

Origin/main @ `abb76fb` is a wide, mature pre-0.1 stack. Five large campaigns landed back-to-back: #80 (widget-catalog/agent-interface), #83 (exact-parity gallery), #84 (perf: `buiy_bench_support` + iai gates), #85 (WebGPU/WASM examples), and now #87 (MVU-as-core). The crate graph is `buiy_core` (render pipeline, 9-phase layout, text render+edit, AccessKit a11y, picking, and the new opt-in MVU substrate) plus `buiy_widgets`, `buiy_bsn`, `buiy_verify`, `buiy_bench_support`, the `buiy` facade, and 7 examples — including a 10,277-LoC, 5-screen parity gallery and two wasm/WebGPU demos. CI is genuinely disciplined: 9 lanes across 3 OSes, SHA-pinned actions, MSRV 1.95, a pinned-lavapipe GPU `#[ignore]` lane, a headless WebGPU web-smoke lane, and `cargo-deny` (`.github/workflows/ci.yml:42-396`).

PR #87 shipped a clean, well-documented `buiy_core::mvu` substrate — a `Model` trait (assoc `Msg` only) + free-function reducer returning `Cmd<Msg>` (None/Emit/Batch), an `Envelope<M>` per-type inbox, a single `enqueue` ingress, a sealed `PureEnv` allowlist, a `set_if_neq` drain, `MvuWorkCounters`, `LogicalId`/`MsgLog`/`RecordSession`/`RecordMode`, and the opt-in `MvuCorePlugin` (`crates/buiy_core/src/mvu/mod.rs:64-122,453,481-510`). The spec↔code fidelity is unusually high: of D1–D14 plus the two MUST-FIX items, every functional decision LANDED, and the deferred set matches the §15 ROADMAP exactly. The three error-prone surfaces the 3-reviewer gate fixed — D4 early drain+bind (prevents a one-frame `aria-expanded` regression), D5 the AT inline-fold seam, D11 a can-fail structural perf gate — are all present with dedicated acceptance tests.

But the substrate is consumed only *inside* `buiy_widgets`. Exactly two production `impl Model` exist: `A11yToggled` (the Checkbox/Switch leaf, `mvu/leaf.rs:60`) and `MenuModel` (the lone migrated machine, `menu.rs:659`). The flagship 10k-LoC gallery registers **zero** reducers and authors **zero** domain Models — all five screens still use the pre-MVU imperative pattern (`MessageReader<OnPress>` collectors → intent `Resource`s → exclusive `apply_*` systems). The `buiy` facade prelude re-exports **nothing** from `mvu`, so `use buiy::prelude::*;` cannot reach `Model`, `Cmd`, `enqueue`, or `MvuCorePlugin`.

**One-line verdict:** MVU-as-core is a correct, well-engineered widget-internal state substrate — but as an app-author DX surface it is invisible and unergonomic; it *settled* F1's architecture at the machine tier while leaving every app-author-facing friction (F2, F4–F8) untouched and introducing a fresh layer of MVU-authoring debt (no prelude, no `enqueue_world`, manual `LogicalId`, per-widget routers). The interface brainstorm's mandate is essentially unchanged, plus a new column.

## 2. The MVU substrate as it actually shipped

**The full author loop (test fixture — there is no app equivalent).** Defining state is tidy; wiring and reach are not:

```rust
// crates/buiy_core/tests/crosscut/mvu.rs
#[derive(Component, Default, Debug, Clone, PartialEq, Reflect)]
#[reflect(Component)]
struct Counter { value: i64 }
impl Model for Counter { type Msg = CounterMsg; }

#[derive(Clone, Debug, Reflect, PartialEq)]
enum CounterMsg { Increment, Add(i64), TickTo(i64) }

fn counter_update(c: &mut Counter, msg: CounterMsg) -> Cmd<CounterMsg> {
    match msg { CounterMsg::Increment => c.value += 1, /* ... */ }
    Cmd::none()
}

// wiring — three calls (the `mvu_model` one-call sugar is NEVER used):
app.add_plugins(MvuCorePlugin);
app.register_type::<Counter>();
app.add_model::<Counter>();
app.add_reducer::<Counter, _>(counter_update);

// enqueue — must `use buiy_core::mvu::enqueue` (absent from buiy_core root re-export AND the buiy facade):
enqueue::<Counter>(&mut commands, target, CounterMsg::Increment);
```

**Wiring / discoverability.** `MvuCorePlugin` is opt-in and correctly **not** composed into `CorePlugin` (`mvu/mod.rs:1080`); `WidgetsPlugin` pulls it in (`buiy_widgets/src/lib.rs:160-161`), so widget-using apps get it transitively. But the *authoring types* are unreachable from the front door: `crates/buiy/src/lib.rs:7-153` re-exports layout, render, text, theme, bsn — and nothing from `mvu`; `pub mod prelude { pub use crate::*; }` therefore also has zero MVU surface. Worse, the `buiy_core` root re-export is curated backwards — it ships the never-called `MvuModelExt` sugar but **omits `enqueue`** (`crates/buiy_core/src/lib.rs:62`), the function the module doc calls "the ONLY way a message enters the funnel" (`mvu/mod.rs:453`). `LogicalId` is likewise omitted.

**The tiers, concretely:**

- **Router-leaf (Button).** NOT implemented — `OnPress → Model` routing is explicit ROADMAP (`mvu/mod.rs:30-31`; `with_routing()` is a TODO stub at `:1019-1020`). Authors still hand-wire press→msg.
- **Stateful-leaf (Checkbox/Switch).** The a11y component *is* the model — no domain struct:
  ```rust
  // crates/buiy_core/src/mvu/leaf.rs:60
  impl Model for A11yToggled { type Msg = ToggleMsg; }
  pub fn toggle_reducer(state: &mut A11yToggled, msg: ToggleMsg) -> Cmd<ToggleMsg> { /* Toggle | Set(on) */ }
  ```
- **Machine (Menu).** The one genuine domain model, with a11y as a projection:
  ```rust
  // crates/buiy_widgets/src/menu.rs:592
  pub struct MenuModel { pub open: bool, pub active: Option<usize>, pub dismissed: Option<DismissReason> }
  // bind_menu_model PROJECTS MenuModel -> CssVisibility + active_descendant + button.A11yExpanded + focus
  ```
  `active` is an **index, not an Entity**, specifically to keep the recorded log replay-portable (`menu.rs:581-584`). The migration deleted both old reconciliation systems (`sync_menu_open`/`sync_menu_dismissed`).
- **Composite raw-ECS hatch.** Dialog/Popover/Disclosure/Tooltip carry no `Model` — unchanged raw-ECS `CssVisibility`/`A11yExpanded` machines.

**Per-machine scheduling is hand-rolled every time.** There is no helper that bundles "register an early-window machine + its bind"; each re-derives the D4 ordering by hand:

```rust
// crates/buiy_widgets/src/lib.rs:444-485 — and leaf.rs:125-146 repeats the same ceremony
app.configure_sets(Update, (MenuSet::Enqueue, MenuSet::Drain, MenuSet::Bind)
    .chain().after(BuiySet::Picking).before(BuiySet::A11yUpdate));
app.add_systems(Update, ApplyDeferred.after(MenuSet::Enqueue).before(MenuSet::Drain));
app.add_reducer_in_set::<menu::MenuModel, _>(menu::menu_reducer, menu::MenuSet::Drain);
app.add_systems(Update, menu::route_menu_press.in_set(MenuSet::Enqueue));
app.add_systems(Update, menu::bind_menu_model.in_set(MenuSet::Bind));
```

**Record/replay.** `RecordMode` defaults OFF (`tick_seq` returns `None` ⇒ zero work; `mvu/mod.rs:189,201`); the unified log is `LogicalId`-keyed (`:141`). The §7.5 single-writer auditor is folded into the per-model `count_binds` under `cfg(debug_assertions)` (no new system ⇒ no entity-id drift; release byte-identical; `:575-606`). Whole-UI byte-identical replay is real but lives only in `examples/buiy_gallery/tests/mvu_whole_ui_replay.rs` (a 528-line fixture) and covers only funnel-governed state — the gallery's own add/destroy/filter mutations are explicitly **off-log**.

**The AT seam.** Well-built and shares ONE fold body with the batch drain (`fold_one_inline` calls the same `fold_one_with` as the drain — `mvu/mod.rs:750,636`), so the seam and drain cannot diverge in what they record. `menu_inline_action_hook` does the cross-entity hop button→`controls[0]`→menu and folds the **absolute** verb (Open/Close, never Toggle; `menu.rs:728-764`). The reducer is a bare `fn` pointer (not an `FnMut` closure), so a captured `Res` snapshot can't make replay diverge.

## 3. Friction scorecard (F1–F8) at HEAD

| Friction | Status | Evidence | One-line |
|---|---|---|---|
| **F1** app state was the a11y tree | **Improved (relocated)** | `menu.rs:592,659`; `mvu/leaf.rs:60` | Machine tier inverts it (`MenuModel`, a11y projected); leaf tier *blesses* the coupling (a11y component IS the model); app domain still raw ECS. |
| **F2** one untyped `OnPress(Entity)` | **Open** | `interaction.rs:30`; gallery `lib.rs:1217-1231`; `mvu/mod.rs:1019` | No typed events; routing is ROADMAP; gallery still disambiguates by `Has<>` marker queries. |
| **F3** silent-wrong footguns | **Improved (narrow)** | `mvu/mod.rs:576-614`; `render/color.rs:146-153` | Debug-only single-writer auditor added for Model components; magenta-token + `#[require]`-suppression footguns untouched. |
| **F4** Style is a Bundle, bsn! can't author it | **Open** | `layout/style.rs:49`; `hello_bsn/src/lib.rs:37-65` | Layout vocabulary still forks between the fluent builder and bsn!'s decomposed `BoxModel`/`FlexParams`. |
| **F5** one widget, 4 spellings | **Open** | `button.rs:59,116`; `scene.rs:90` | Button still has marker / `::new` / scene-fn / `bsn!` spellings with subtly different output. |
| **F6** stringly-typed theme tokens | **Open** | `theme.rs:22-24`; `render/color.rs:101` | Tokens still `Cow<str>` → `HashMap` resolved at extract; bad key → magenta sentinel + warn. |
| **F7** retained-mode boilerplate / list reactivity | **Open** | gallery `lib.rs:1266,271,994-999` | Dynamic lists still hand-driven (intent buffer + exclusive applier + manual recount). |
| **F8** verbosity | **Open** | gallery `src/` 10,277 LoC; one row ≈ 163 lines (`lib.rs:1018-1181`) | MVU added surface (reducers/sets/binds), not less. |

**What MVU SETTLED.** Only the *architecture* of F1, and only at the machine tier: `MenuModel` is a true domain model with a11y as a `bind`-projection — the inversion F1 wanted. The substrate also makes app-domain MVU *possible* and adds a real (if debug-only) F3 safety net.

**What it RELOCATED rather than resolved.** The stateful-leaf tier hard-couples state to `A11yToggled` by design (`leaf.rs:18`: "There is NO per-checkbox/per-switch Model type"). There is **no domain accessor** — no `Checkbox::checked() -> bool`, no `Switch::is_on()`; the only `impl` blocks are `new()` (`checkbox.rs:141`, `switch.rs:162`). Every reader pattern-matches a foreign tri-state enum even for a binary switch: `world.get::<A11yToggled>(e).map(|t| t.0) == Some(Toggled::True)` (gallery `lib.rs:1327,1550,1685`). So F1's "accessibility tree doubles as application state" persists for Checkbox/Switch, just behind a thin veneer. Worse, *writing* a checkbox now requires the funnel, which in an exclusive `&mut World` system means hand-building the transport: `inbox.write(Envelope { target: cb, msg: ToggleMsg::Set(on) })` (`lib.rs:1335-1341`) — strictly *more* app-author ceremony than the prior direct write.

**What it LEFT OPEN for the brainstorm.** All of F4 (styling-as-components), F5 (one spelling), F6 (typed tokens), F7/F8 (boilerplate + verbosity) are untouched, **plus** F2 (typed events + the deferred `OnPress → Model` routing), **plus** a new requirement the brainstorm now inherits: MVU-authoring ergonomics (prelude, sugar, world-level enqueue, `LogicalId` derivation).

## 4. New frictions & risks the MVU merge introduced

- **No MVU prelude / no authoring sugar (N1).** `buiy::prelude` exposes zero `mvu` items; an app touching MVU must add a direct `buiy_core` dep and `use buiy_core::mvu::{...}` (gallery does, `lib.rs:111`). The "primary state interface" is unpresented.
- **No world-level `enqueue` helper (N2).** `enqueue` takes `&mut Commands` only (`mvu/mod.rs:453`), so every exclusive `&mut World` caller reimplements it by hand against `Messages<Envelope<M>>` — both the gallery (`lib.rs:1335`) and the menu (`enqueue_menu`, `menu.rs:1038`) do this independently. The "single ingress" invariant is convention + copy-paste, not an API.
- **Dead/wrong sugar promoted over the essential primitive.** `mvu_model` (the one-call wiring sugar) has **zero call sites** and only wires the late-default `MvuSet::Drain` tier — which **no shipped model uses** (both leaf and Menu need the early window via `add_reducer_in_set`). So the discoverable sugar covers the tier nobody ships, the two tiers everybody ships have no one-call helper, and `MvuModelExt` is re-exported at the `buiy_core` root while `enqueue` is not.
- **Coexisting message substrates (N: two vocabularies).** Apps must learn both bevy `Message`s (`OnPress`, `EditSubmitted`, read via `MessageReader`) *and* the MVU per-type `Messages<Envelope<M>>` inbox (written via `enqueue`). There is no unified "widget changed" typed event; for a checkbox you poll `Changed<A11yToggled>` to read and enqueue a `ToggleMsg` to write.
- **Per-widget OnPress→enqueue routers (N: bespoke glue).** `advance_toggle_on_press`, `advance_expanded_on_press`, and `route_menu_press` are three structurally-identical "read OnPress, look up role, enqueue/flip" systems (`buiy_widgets/src/lib.rs:90,136`; `menu.rs`). This is exactly the generic routing the spec defers — until it lands, every new stateful widget adds another bespoke router.
- **Manual `LogicalId` with a silent UNRESOLVED fallback (new F3-class footgun).** No auto-assignment/derive/helper; every replay-relevant spawn hand-numbers `LogicalId(...)` (`mvu_whole_ui_replay.rs:121-126`, `mvu_scenes.rs:107/157/275`). Forgetting it does **not** error — the fold keys on `LogicalId::UNRESOLVED` (`u64::MAX`, `mvu/mod.rs:643-646`), so all untagged actors collide on one log key and cross-process replay misroutes silently — on the very capstone the substrate exists for.
- **Half-migrated single-writer.** `A11yToggled` is funneled, but sibling `A11yExpanded` is still flipped by a direct raw write in `advance_expanded_on_press` (`expanded.0 = !expanded.0`, `lib.rs:141`), and Slider's `A11yValue` is mutated directly by its contract honor. The auditor only covers Model-bearing components, so Disclosure's write is not even audited. Two near-identical toggle-state components, one funneled and one not.
- **Un-migrated machines (Dialog/Popover/Disclosure/Tooltip/ScrollArea).** The machine tier is demonstrated exactly once. No generalized `DialogModel`/`PopoverModel` pattern exists to copy; `apply_dialog_modal_state` still keys off `Changed<CssVisibility>` (`lib.rs:288-310`). Acknowledged §15 deferral, but it leaves two coexisting overlay-state paradigms.
- **Opt-in plugin discoverability (low severity).** A headless/custom app wiring its own Model *without* `WidgetsPlugin` must remember `MvuCorePlugin` or its drains/counter-reset silently never run.
- **One-frame-lag caveat — actually fixed, not introduced.** The D4 early window pins `Enqueue→ApplyDeferred→Drain→Bind` `.before(A11yUpdate)`, and the acceptance test `same_frame_aria_expanded_projection_on_press` (`menu.rs tests:443`) reads `aria-expanded` from the production a11y snapshot same-frame — it would fail under the prototype's late bind. No residual lag in the migrated path.
- **Spec↔code doc gaps (cosmetic).** §2 says the `PureEnv` allowlist blesses `Local` (it does not — `:479` TODO defers it) and that `Envelope` carries an origin tag (it does not — origin lives on `LoggedEntry`, `:159-184`). One stale in-file anchor: `MvuWorkCounters` doc says reset is `.before(Picking)` but it installs `.before(Input)` (`:344` vs `:1123`).

## 5. Spec ↔ code alignment & what's deferred

**Fully landed (D1–D14 + MUST-FIX):**
- **D1** substrate exactly as §2 (`Cmd` = None/Emit/Batch; single ordered drain system, never an observer; folds onto a clone; `mvu/mod.rs:64-122,636-730,865-895`).
- **D2** `set_if_neq` + `MvuWorkCounters`, gate-tested (idempotent fold ⇒ `models_mutated==0, binds_fired==0`).
- **D3** all four tiers present (stateful-leaf, machine, router-leaf as the *interface point* though routing itself is roadmap, raw-ECS hatch).
- **D4** early drain+bind, with the same-frame `aria-expanded` acceptance test.
- **D5** AT inline-fold seam (`fold_one_inline` + `InlineActionRegistry` + cross-entity hop + absolute Open/Close), with the §5.7 four-point contract test (`mvu_at_seam.rs`).
- **D6** editor command-sourcing (additive; `set_value` treated as seed-scene state, `replay.rs:208-237`).
- **D7** record/replay + the §7.5 debug single-writer auditor (4-case test).
- **D9** dismiss as a resource registry (`DismissRegistry`, model-agnostic; `dismiss.rs:102,155-176`).
- **D10** MUST-FIX closed (`toggle_all_rows` rerouted to `ToggleMsg::Set`; three at-spawn seeds documented and auditor-exempt).
- **D11** perf gate: can-fail `BlinkLeaf` folds a genuine per-frame `Tick`, projects a **structural** `ComputedPaintSkip` flip, runs in the **headless default gate** (`blink_funneled_node_rebuilds_zero`, not `#[ignore]`), with a soft iai ceiling.
- **D12** MUST-FIX closed (all four `cargo-deny` advisories triaged, incl. the pre-existing `ttf-parser` base failure).
- **D13** WASM-clean (MVU files free of `thread`/`Instant`/`SystemTime`/`rayon`).
- **§14** migration ledger executed (`sync_menu_*` deleted; inspector reads live `MenuModel.open`, `inspector.rs:857-865`, backed by live-interaction tests).

**Partial / "targeted, not yet proven":**
- **D14** derived-structure replay — the two corrections that ship regardless are present (live per-entry `resolve_lid`; typed `warn!`-logged `DeadLetter`), but the **keyed-list proving fixture is absent**, so clause (a) correctly carries the downgraded wording (`replay.rs:177-194`).

**Deferred (matches §15 ROADMAP exactly, all confirmed absent from code):**
- D8 `Cmd::task` + keyed `Subscription` (only the `Origin` log-format hook is baked now; `Origin::Command/Subscription` are reserved, never emitted).
- The entire **env-reducer path** ships with **zero call sites** — sealed `PureEnv` allowlist, `Reducer<M,E>` trait, `add_reducer_env_in_set` — "No shipped model uses an env reducer" (`mvu/mod.rs:391`). It doubles the drain implementation (env-free `fold_one_with` vs env body) that must be kept in sync: real future surface, dead weight in v1.
- `PureEnv` `Local` + `#[derive(PureEnv)]`, the variadic-reducer macro, cross-process serialized `UnifiedLog`, Dialog/Popover machine migrations, `catch_unwind` supervision, and the **FunnelHooks** registry unification.
- §15 was overridden at the human gate to ship as **ONE PR (#87)** instead of the two-PR machine-boundary split (retained only as the internal W0–W8 wave seam).

**Acceptance-test posture.** Strong: the three gate-fixed surfaces (D4/D5/D11) each have dedicated tests reading from the production snapshot, plus the W4 byte-identical whole-UI replay fixture and live menu open/dismiss interaction tests honoring the "RUN don't trust green" posture.

## 6. Broader state & health

**Perf (#84).** The work-counter gate (#2: `node_rebuilds`/`instances_built`/`node_patches`) and the dhat alloc-budget gate (#3) live in `tests/` so nextest runs them in CI — guarded. But the **#5 headline (8.6× atlas-touch O(1)) has no active regression guard anywhere**: the results report concedes `atlas_touch_ops` is only a residency invariant and cannot see the per-touch cost #5 fixed; the real guard is the iai-callgrind instruction count, "deferred to CI (no local Valgrind)" — and there is **no iai job in `ci.yml`**, nor any `iai`/`callgrind` entry in `follow-ups.md`. The `mvu_iai.rs`/`pipeline.rs` benches only compile-check via `clippy --all-targets`; they never run. The gap isn't even tracked.

**WASM/parity/CI.** WebGPU v1 is real and prototype-validated (the WGSL `fwidth`/`textureSample`-after-early-return uniformity bug — native-naga-lenient/Tint-strict — was found by running it, overturning the spec's "zero render changes" claim; the mask-instead-of-early-return D2 fix is the landed prerequisite). WebGL2 reach, IME, mobile keyboard, and browser a11y are honestly staged out. CI is the project's strongest asset (9 lanes, 3-OS nextest with insta unreferenced-snapshot gate, MSRV 1.95 exercised, pinned-lavapipe GPU lane, `cargo-deny --locked`, committed `Cargo.lock`). Two CI gaps: the **full `gallery_web` wasm path is not CI-built** (only the minimal `buiy_web` is smoked, `ci.yml:348,357`), and the **web-smoke shader-conformance check is best-effort** (skips on hosted runners with no software WebGPU adapter; enforced only on a dev GPU host — honestly documented and tracked). **Documentation drift** is the other health gap: `CHANGELOG.md` stops at layout Phase 9 — none of #80/#83/#84/#85/#87 are recorded — and the docs index still labels the wasm spec "draft", perf "[active]", and parity "merge-gated", despite all three being merged. Cosmetic relative to code, but it erodes the index's stated role as the entry point.

## 7. The open backlog, prioritized for the interface work

Ordered by leverage for resuming the DX/interface redesign:

1. **Finish F2 — typed per-widget events + `OnPress → Model` routing.** Highest leverage and the spec's explicit ROADMAP debt (`with_routing()` TODO, `mvu/mod.rs:1019`). It eliminates the marker-query disambiguation footgun *and* the three bespoke per-widget routers in one stroke, and it is the missing bridge that makes MVU reachable for app domain logic.
2. **Make MVU reachable and ergonomic (N1/N2/sugar).** Re-export `Model`/`Cmd`/`enqueue`/`LogicalId`/`MvuCorePlugin` through `buiy::prelude`; add `enqueue_world(world, target, msg)`; replace the dead `mvu_model` (wrong tier, zero call sites) with a real one-call helper for the early-window tiers people actually ship, bundling `configure_sets`+`ApplyDeferred`+drain+bind. Until this lands, "primary state interface" is aspirational.
3. **Provide a domain accessor for the stateful-leaf (F1 ergonomics).** `Checkbox::checked() -> bool` / `Switch::is_on()` so apps stop pattern-matching foreign `accesskit::Toggled`. Decide whether the leaf model should remain `A11yToggled` or gain a thin domain newtype.
4. **Auto-derive/assign `LogicalId`** to kill the silent `UNRESOLVED` collision footgun before record/replay is promoted to an app feature.
5. **F4 styling-as-components** — make `Style` (or its decomposition) bsn!-authorable so the layout vocabulary stops forking by spawn idiom. Largest verbosity/consistency win for F8.
6. **F5 one canonical Button spelling; F6 typed/checked theme tokens.** Both are self-contained and remove silent-wrong classes (label-less marker, magenta sentinel).
7. **Finish the single-writer migration** (Disclosure `A11yExpanded`, Slider `A11yValue`) and generalize a `DialogModel`/`PopoverModel` machine pattern, so the codebase stops carrying two overlay-state paradigms.
8. **Health/tracking debt (lower DX leverage, but cheap):** add the iai CI lane (or at least track it), CI-build `gallery_web` wasm, refresh `CHANGELOG.md` + docs-index status labels, fix the `MvuWorkCounters` `.before(Picking)` doc anchor, and reconcile §2's `PureEnv`/`Envelope` over-claims.

**What the interface brainstorm should now target.** The still-open frictions are F2, F4, F5, F6, F7, F8 — MVU resolved none of them for app authors; it only settled F1's *architecture* at the machine tier and relocated it at the leaf tier. The decisive new constraint is that the authoring sugar must now **express MVU wiring as a first-class part of the authoring path, not plumbing the app sees**: a declarative way to attach a domain `Model` + reducer to a subtree, typed widget events that route into it automatically (closing F2), an `enqueue` reachable from the prelude and from exclusive systems, and `LogicalId` assigned by the framework. The target is a single coherent authoring vocabulary — today five coexist (raw `world.spawn` tuples dominate; the fluent `Style` builder; widget scene-fns; bundle constructors; the MVU `Envelope` inbox) — in which writing a small interactive app means declaring a Model and a view, not hand-wiring intent resources, marker queries, recount systems, and inbox writes.
