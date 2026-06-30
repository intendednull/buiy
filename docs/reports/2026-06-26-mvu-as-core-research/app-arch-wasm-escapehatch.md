# MVU-as-core research — App architecture, WASM constraints, the escape hatch

**Date:** 2026-06-26
**Stage:** RESEARCH (`/staged-development`), prototype-3 ("MVU as the CORE")
**Area:** app/plugin architecture · schedule integration · WASM · the escape hatch · core-vs-module placement
**Base tree:** current `origin/main` worktree at `/mnt/storage/projects/buiy/.claude/worktrees/mvu-core`
**Charter:** `docs/prototypes/2026-06-26-mvu-as-core-PROTO3-charter.md` (in the `state-mgmt-elm-prototype` worktree)

> Charter rule honored: every choice re-decided from evidence; `buiy_core` redesign is on the
> table; nothing inherited blindly. Where the evidence cuts against the charter's framing I say so
> (see § 3 the completeness-vs-escape contradiction, and § 6 R3 the TextEditState crux — "Reflect
> everything" is the wrong frame for the editor).

---

## 0. TL;DR — recommendations

1. **Schedule.** Put the substrate in `buiy_core` as a new module composed by `CorePlugin`, and
   **fold its sets into the existing `BuiySet` chain** rather than running an uncoordinated parallel
   chain (proto-2 ran `(Enqueue, Drain, Bind).chain()` standalone — `runtime.rs:413-417`). Concrete
   ordering: `… Picking → A11yUpdate → [MvuEnqueue → (sync) → MvuDrain → MvuBind] → Render`, with an
   **explicit `ApplyDeferred` between Enqueue and Drain** (the routing observers and `enqueue` both
   defer via `Commands` — `routing.rs:59`, `runtime.rs:172-178`). This pins the proto-2 REFINE-#2
   "latency is one designed frame, not emergent" item. Render `extract` runs in `ExtractSchedule`
   (render world, after `Update` ends — `render/mod.rs:305-317`), so the only hard constraint is
   "drain + bind finish in `Update` before the frame closes."

2. **WASM — zero new *compile* obstacle, confirmed.** `ron` is already in the lock
   (`Cargo.lock:4494`, pulled by `bevy_scene`), `serde`/`serde_json` are workspace deps
   (`Cargo.toml:59-60`), `Reflect` is pure Rust, the bus (`Messages<T>`) is a `Vec` (no threading),
   and `AppTypeRegistry` is an `Arc<RwLock<…>>` that is uncontended under the single-threaded web
   scheduler. The substrate introduces **no new crate that lacks a wasm backend** (the thing that
   actually broke wasm was `arboard`, wasm spec D3). Three runtime caveats must be cfg-gated/bounded,
   none of which is a *new* obstacle: (i) `catch_unwind` reducer supervision is **inert on wasm**
   (`panic = abort`; wasm spec § 4 already states this), (ii) `LogicalId` must be **deterministic**
   (a counter/hash, never `uuid`/random) or it re-activates `getrandom` on web (feasibility § B,
   currently absent), and (iii) the `MsgLog` is an **unbounded `Vec`** (`runtime.rs:112-116`) — a
   memory leak acute on 32-bit wasm; must be off-by-default + bounded.

3. **Escape hatch — structural and additive, but it *contradicts* "complete log".** The drain only
   touches entities carrying a registered `Model` (`Query<&mut M>`, `runtime.rs:301,318`); entities
   without one are invisible to MVU. Power users drop to raw ECS by simply **not attaching a `Model`
   and writing ordinary `Update` systems** — exactly what `buiy_widgets` does today (22 `add_systems`
   / observers). Keep `OnPress` a public `Message` and components `&mut`-able so the hatch needs no
   fighting. **But** a raw-ECS write is invisible to the log, so the charter's headline advantage #1
   ("complete recordable stream → whole-UI replay") and the escape hatch are in **direct tension**:
   you cannot have an unrestricted hatch *and* a complete log. Resolve by **tiering the guarantee**
   (replay is complete over the MVU-governed subtree; the boundary is explicit and debug-detectable),
   not by pretending both hold.

4. **Placement.** **Core (always compiled, cheap when unused):** the funnel + drain + `MvuSet` +
   `Model`/`Cmd` + `PureEnv`. **Core but gated/off-hot-path:** the `Reflect`/`ron` record-and-replay
   harness (a *consumer* of the substrate, not the substrate). **Separable/incremental:** the
   every-widget-an-actor reshape of `buiy_widgets`, the `bind`/derive ergonomics, the variadic reducer
   macro, and the MCP transport (already `buiy_mcp`). The substrate *must* be core because the
   `OnPress` sink and the `Action→OnPress` router are already core and "the dependency points the
   wrong way" for an opt-in top layer (`interaction.rs:9-17`, charter trigger #2).

---

## 1. How Buiy plugins & schedules compose today (the slot map)

### 1.1 Plugin composition

`BuiyPlugin::build` (`crates/buiy/src/lib.rs:216-292`) composes, in a documented order
(architecture.md § 2.8): guarded `bevy::picking::PickingPlugin` + `PointerInputPlugin`
(`lib.rs:232-254`), then a single `add_plugins((…))` tuple: `CorePlugin`, `ThemePlugin`, `A11yPlugin`,
`AccessKitAdapterPlugin`, `FocusPlugin`, `LayoutPlugin`, `PickingPlugin`, `BuiyPickingBackendPlugin`,
`ScrollInputPlugin`, `BuiyTextPlugin`, `AnimationPlugin`, `WidgetsPlugin`, `BuiyRenderPlugin`
(`lib.rs:255-290`). A headless subset, `BuiyHeadlessPlugin` (`lib.rs:338-355`), drops
winit/picking/scroll/animation.

`CorePlugin::build` (`crates/buiy_core/src/lib.rs:94-151`) does three load-bearing things:
- adds `InteractionPlugin` (the `Messages<OnPress>` sink) **in core** so core producers exist without
  `buiy_widgets` (`lib.rs:98-100`, and `interaction.rs:32-43`);
- configures the **top-level `BuiySet` chain**: `Layout → Style → Input → Animate → Picking →
  A11yUpdate → Render`, `.chain()` (`lib.rs:78-87` enum, `lib.rs:104-116` config);
- schedules the transform-propagation bridge in `Update`, pinned `.after(Animate).before(Picking)`
  (`lib.rs:128-149`).

### 1.2 The activation data-flow (where a click becomes state today)

- **Producers of `OnPress`** (a buffered `Message`, `interaction.rs:29-30`): the C3 pointer layer
  (`Pointer<Click> → OnPress`) and the P1c action router (`Action::Click → OnPress`, plus Button
  Enter/Space) — both **in core** (`interaction.rs:6-17`).
- **Consumers of `OnPress`** mutate widget state directly in `BuiySet::Input` — e.g.
  `advance_toggle_on_press` writes `&mut A11yToggled` (`buiy_widgets/src/lib.rs:189`,
  `checkbox.rs:11-14`). This is the proto-1 REFINE-#5 "self-update vs controlled double-write" the
  charter wants MVU to kill (charter advantage #2).
- **The agent write-side already exists in core:** `a11y::inprocess::perform` →
  `dispatch_action_request` lowers each `Action` into the **real `OnPress` / `FocusedEntity` /
  contract sinks** synchronously (`inprocess.rs:360-388`). This is precisely the "lower actions
  *through* the funnel" the charter (trigger #2) wants — it is half-built and core-resident already.

### 1.3 Where a core-MVU substrate slots

The proto-2 substrate is three sets — `MvuSet::{Enqueue, Drain, Bind}` (`runtime.rs:154-162`) — plus
`MsgLog` (`runtime.rs:111-117`), installed by `MvuPlugin` which **configures them as a standalone
`.chain()` in `Update`** with no relation to anything else (`runtime.rs:411-418`). That is the gap:
in the real app the drain must be ordered against `BuiySet`, because its input (`OnPress`/routing) is
produced across `Input`/`Picking` and its output (model state) is read by `Render` extract.

**Recommended integration (see § 5 for the full rationale + runner-up):**

```
BuiySet::Layout → Style → Input → Animate → Picking → A11yUpdate
   → MvuSet::Enqueue           (.after(A11yUpdate))
   → ApplyDeferred             (flush Commands from enqueue + routing observers)
   → MvuSet::Drain             (the single ordered fold + record tap)
   → MvuSet::Bind              (Changed<Model> → derived views / Text)
   → BuiySet::Render           (.after(MvuSet::Bind))
```

`Render`'s GPU `extract` is in `ExtractSchedule` on the render world (`render/mod.rs:254-317`), which
runs *after* `Update`, so binds that mutate main-world `Text`/`Node` before `BuiySet::Render` (the
main-world render-prep: `write_clip_rects`, `write_effect_groups`, `write_paint_skip` —
`render/mod.rs:103-134`) are seen by extract the same frame. The hard real-time constraint is only
"drain+bind complete within `Update`."

---

## 2. (b) WASM — does the substrate add any new obstacle?

**Constraint (wasm campaign):** the web target is "purely additive… native builds must be
byte-for-byte unaffected" and must add *zero* new wasm obstacle (wasm spec § 1, § 2 D6).

### 2.1 Compile-time: NO new obstacle (evidence-backed)

| Substrate ingredient | wasm status | Evidence |
|---|---|---|
| `Reflect` log (`TypedReflectSerializer`) | pure Rust, no platform deps; runs on wasm | `runtime.rs:127-144`; bevy_reflect has no wasm gate |
| `ron` serialize | **already in the lock** (via `bevy_scene`) — no new dep | `Cargo.lock:4494`, `:702`; proto-2 retro "ron already transitive, no lockfile churn under `--locked`" |
| `serde`/`serde_json` | workspace deps already | `Cargo.toml:59-60` |
| Message bus `Messages<Envelope<M>>` | a double-buffered `Vec`; no threads, no atomics | proto-2 KEEP #1; `runtime.rs:48-63` |
| `AppTypeRegistry` read each pass | `Arc<RwLock<TypeRegistry>>`; **uncontended** under single-threaded web scheduler | `bevy_ecs-0.19.0/world/reflect.rs:91`; wasm spec D6 (single-threaded) |
| `LogicalId` (u64) keying | a plain integer; no RNG **if assigned deterministically** | `runtime.rs:94-99` |

The substrate adds **no crate** to the graph (everything it needs is already resolved), so it cannot
reintroduce the `arboard`-class "no wasm backend" failure the wasm spec D3 had to gate. `getrandom` is
**not** in the wasm production graph today (feasibility § B, line 115) — and the substrate keeps it out
*provided* `LogicalId` is a counter/structural-hash, not a `uuid`/random id (which replay determinism
demands anyway — a random id breaks cross-process re-fold).

### 2.2 Runtime: three items to gate/bound (none a *new* obstacle)

1. **`catch_unwind` reducer supervision is inert on wasm.** The charter lists "`catch_unwind`
   reducer supervision as a core concern." `wasm32-unknown-unknown` is `panic = abort`; `catch_unwind`
   compiles but **cannot recover** — a reducer panic aborts the module. This is **not new**: *every*
   Bevy system already aborts on wasm panic, and the wasm spec explicitly notes "`panic = abort` on
   wasm makes render-extract panics fatal" (§ 4). Recommendation: `cfg(not(target_arch = "wasm32"))`
   the supervision, document the degradation, and keep the **dead-letter** path (a `continue` on a
   despawned target — `runtime.rs:318-321`) which works everywhere.

2. **`LogicalId` assignment must be deterministic** (see § 2.1) — a counter or structural hash, never
   a random/uuid id. Doubles as the replay-determinism requirement and aligns with the charter's
   "`LogicalId` unified with the agent-interface test-id space."

3. **`MsgLog` is an unbounded `Vec` that grows every fold** (`runtime.rs:112-116, 138-143`). On 32-bit
   wasm with browser memory limits this is a leak. Recommendation: recording **off by default** on the
   hot path (§ 6 R1), and when on, a bounded ring buffer or explicit `start()`/`stop()` windows
   (`MsgLog::start` already clears — `runtime.rs:120-125`).

**Net:** the charter's "zero new wasm obstacles" is **achievable**, with the supervision feature
degrading to native-only and the log bounded/off-by-default. This is a *runtime-gating* exercise, not
an architectural blocker.

---

## 3. (c) The ESCAPE HATCH — and the contradiction it exposes

### 3.1 The hatch is already structural

The drain operates only on entities matching `Query<&mut M>` for a *registered* model `M`
(`runtime.rs:301,318`). An entity with no `Model` component is **untouched** by MVU. Therefore a power
user drops to raw ECS by the most boring move possible: **don't attach a `Model`; add your own
`Update` systems against plain components.** This is not a special mechanism — it is exactly what every
current widget system is (`buiy_widgets` ships 22 `add_systems`/observers that read `OnPress` and write
components directly). MVU-as-core does **not** remove that capability; it adds a *layer over* it.

For the hatch to need no fighting, the core MVU design must preserve two public seams it already has:
- `OnPress` stays a **public `Message`** in `Messages<OnPress>` (not funnel-private) —
  `interaction.rs:29-43`. A raw system can still `MessageReader<OnPress>` and act.
- Widget state components stay **public and `&mut`-able** (`A11yToggled`, `ScrollOffset`,
  `FocusedEntity`, …). MVU governs *whether you route through a reducer*, it does not seal the data.

**Boundary, stated crisply:** the `Model` component is the membrane. Inside it (entity has a `Model`)
= funnel-governed, recorded, replayable. Outside it = raw ECS, your responsibility, invisible to the
log. The two compose in one `World` and one schedule.

### 3.2 The contradiction the charter must own

Charter **advantage #1** is "complete recordable stream → whole-UI byte-identical replay." Charter
**hard-question ESCAPE HATCH** is "must still let power users drop to raw ECS." These are in **direct
tension**: a raw-ECS write is, by construction, *not* a logged `Msg`, so any entity using the hatch
makes the log **incomplete**, and replay of a subtree that touches it **diverges silently**. You cannot
hold both "the log is complete" and "the hatch is unrestricted."

This is not a reason to abandon either; it is a reason to **stop over-promising**. Recommended
resolution — **tier the guarantee:**
- "Replay is complete and byte-identical **over the MVU-governed subtree**" (entities with a `Model`,
  whose only state mutation is through the drain).
- Raw-ECS entities are **explicitly outside** the replay boundary, and the boundary is **detectable**:
  a debug-build auditor can watch `Changed<T>` on recorded components and warn when a non-drain system
  mutated one (a "write outside the funnel" lint). Don't ship "whole-UI replay" as an unconditional
  claim; ship "complete over the governed subtree, with a detectable boundary."

This tiering is also what makes the migration safe (§ 6 R8): un-migrated widgets keep their imperative
systems and are simply *outside the boundary* until ported — the system is usable at every step.

---

## 4. (d) Placement — separable module vs fully core

| Layer | Placement | Why | Evidence |
|---|---|---|---|
| Funnel + single drain + `MvuSet` + `Model`/`Cmd` + `PureEnv` | **Fully core** (`buiy_core::mvu`, composed by `CorePlugin`) | The `OnPress` sink + `Action→OnPress` router are already core; an opt-in top layer can't make core route through it (dep points the wrong way). Cheap when unused (no `Model` ⇒ drain no-ops, `runtime.rs:308-309`). | `interaction.rs:9-17`; `inprocess.rs:360-388`; charter trigger #2 |
| `Reflect`/`ron` record + cross-process replay harness | **Core module, gated + off-hot-path** | It is a *consumer* of the drain, not the substrate. Recording every fold is the perf risk (§ 6 R1) and the wasm memory risk (§ 2.2). Off by default; opt-in via `MsgLog::start`. | `runtime.rs:110-145`; proto-2 KEEP #5 |
| Routing ext (`EntityEvent`+observer), `Callback`, `bind`/derive, variadic reducer macro | **Core but additive authoring sugar** | Ergonomics, not substrate. Can land incrementally; the macro is a REFINE-#1 open question (turbofish vs bare-param). | `routing.rs`; proto-2 REFINE #1, #5 |
| Every-widget-an-actor reshape of `buiy_widgets` | **Separable, incremental migration** | `buiy_core` is mature/verified; a big-bang rewrite is "likely wrong" (charter). Leaf widgets may stay imperative routers (§ 6 R8 / open Q). | charter MIGRATION COST; 22 `buiy_widgets` systems |
| MCP transport | **Already separate** (`buiy_mcp`), unchanged | Transport over the in-process driver; the driver stays core. | memory: agent-interface campaign |

**One-line split:** *substrate* is core and always-on (cheap); *recording* is core but gated;
*surface migration* is incremental. This satisfies the charter's "substrate core; surface
core/default; core may be redesigned" while keeping the hot path and wasm clean.

---

## 5. Decision detail — schedule integration (recommendation + runner-up)

**DECISION: fold `MvuSet` into the `BuiySet` chain, late in `Update`, with a pinned `ApplyDeferred`
between Enqueue and Drain.**

**Recommendation.** Configure (in `CorePlugin`, beside the existing `BuiySet` chain — `lib.rs:104-116`):
`MvuSet::Enqueue.after(BuiySet::A11yUpdate)`, an `ApplyDeferred` flush, then
`MvuSet::Drain.after(MvuSet::Enqueue)`, then `MvuSet::Bind.after(Drain).before(BuiySet::Render)`.
- *Why Enqueue after A11yUpdate:* `OnPress` is produced by the pointer layer in/around `BuiySet::Picking`
  and by the action router; routing (`bridge_press` + observer) must see this frame's `OnPress` and
  `commands.trigger` a `Routed<M>` whose observer `enqueue`s (`routing.rs:32-62`). Placing Enqueue
  after the producers lets a click settle the **same frame**, fixing the current 1-frame inversion
  (toggle consumer runs in `Input`, *before* the `Picking` producer).
- *Why the explicit `ApplyDeferred`:* both `enqueue` (`commands.queue` → writes `Messages<Envelope>`,
  `runtime.rs:172-178`) and the routing observer (`commands`, `routing.rs:59`) **defer**; the drain
  reads `MessageReader<Envelope<M>>`. Without a pinned flush the chain "spans a couple frames depending
  on where Bevy inserts `apply_deferred`" — the literal proto-2 REFINE #2 finding. Pin it.
- *Why Bind before Render:* derived `Text`/`Node` writes must precede the main-world render-prep
  (`render/mod.rs:103-134`) and `ExtractSchedule` so the painted frame reflects this fold.

**Runner-up: keep proto-2's standalone `(Enqueue, Drain, Bind).chain()`** (`runtime.rs:413-417`),
unordered vs `BuiySet`. *Rejected:* it leaves drain-vs-extract and drain-vs-producer ordering to
emergent scheduler choices — exactly the non-determinism the thesis is supposed to remove, and the
source of the proto-2 multi-frame latency. The charter's whole value (deterministic, tick-exact)
demands the ordering be *designed*, not emergent.

**Second runner-up: a new dedicated `RenderSet`-style top-level `BuiySet::Update` member that replaces
`Input`** (drain *is* the input stage for MVU widgets). *Rejected for v1:* it forces every widget to be
an actor before anything works (no incremental path), and it entangles the substrate with the widget
migration. Prefer the additive late-Update sub-chain; let widgets migrate into it over time.

---

## 6. Risks

### R1 — CRITICAL: recording cost on the 60 Hz hot path
`MsgLog::record` runs `TypedReflectSerializer` + `ron::ser::to_string` **per fold**
(`runtime.rs:127-144`) — allocation-heavy, on every interaction, scaling with widget count. Against the
perf campaign's 60 Hz hard floor on weak machines this could kill "every widget is an actor."
**Mitigation:** recording **off by default**; when on, record the *typed value* and serialize **lazily
on dump** (move `ron` out of the fold), or sample; gate with hw-independent iai-callgrind. Evidence:
`runtime.rs:127-144`; charter PERFORMANCE.

### R2 — HIGH: "complete log" vs "escape hatch" is a real contradiction
A raw-ECS write is invisible to the log; an unrestricted hatch makes whole-UI replay unsound (§ 3.2).
**Mitigation:** tier the guarantee (complete over the MVU-governed subtree) + a debug `Changed<T>`
"write-outside-the-funnel" auditor that makes the boundary detectable. Evidence: `runtime.rs:318`
(entity-scoped drain) vs charter advantage #1; `buiy_widgets` 22 raw systems.

### R3 — HIGH: the TextEditState crux is *not* solved by "Reflect everything"
`TextEditState` is **deliberately not Reflect** — it wraps a `cosmic_text::Editor`, a foreign type at
the cosmic boundary (`state.rs:89-92`). Making the substrate core does **not** make it reflectable.
The Elm-correct answer is **command-sourcing, not state-sourcing**: log the `Msg` (the `EditCommand`
verbs — `Insert`/`SelectAll`/…, `text/edit/command.rs`, surfaced at `buiy/src/lib.rs:76`), and treat
the `Editor`/`Buffer` as **derived View state that is never logged** (rebuilt by replaying commands).
This is consistent with what proto-1/2 already do (they log the `Msg`, not the model snapshot — KEEP
#2/#5). The risk is that the obvious "snapshot every Model via Reflect" framing is wrong for the one
component the charter named as the crux; the spec must commit to command-sourcing for the editor (and,
for consistency, everywhere). **Mitigation:** spec the editor Model as `{buffer text + cursor +
selection}` (reflectable POD) with the `cosmic_text::Editor` reconstructed on demand; log
`EditCommand`. Evidence: `state.rs:89-92`; charter trigger #3.

### R4 — MEDIUM: command-flush latency across the routing→drain edge
Without pinned sync points the bridge→trigger→observer→drain chain spans frames (proto-2 REFINE #2).
**Mitigation:** the § 5 explicit `ApplyDeferred` between Enqueue and Drain + an integration test
asserting same-frame settle. Evidence: proto-2 retrospective REFINE #2; `routing.rs:59`,
`runtime.rs:172-178`.

### R5 — MEDIUM: unbounded `MsgLog` memory (acute on 32-bit wasm)
`entries: Vec<LoggedEntry>` grows every recorded fold with no cap (`runtime.rs:112-116`).
**Mitigation:** off-by-default + bounded ring buffer / explicit record windows. Evidence:
`runtime.rs:112-143`; wasm spec D6 (single-threaded, memory-constrained).

### R6 — MEDIUM: `catch_unwind` supervision degrades to abort on wasm
`panic = abort` on `wasm32-unknown-unknown` means reducer supervision cannot recover (§ 2.2).
**Mitigation:** cfg-gate supervision to native; keep dead-letter (works everywhere); document the
degradation in the spec's wasm-posture section. Evidence: wasm spec § 4; `runtime.rs:318-321`.

### R7 — LOW–MEDIUM: `AppTypeRegistry` read-lock per drain pass
The drain takes `registry.read()` once per pass (already amortized — `runtime.rs:313`). Uncontended on
single-threaded wasm; cheap but non-zero on native. **Mitigation:** only acquire when recording is on
(skip entirely when off — which is the default per R1). Evidence: `runtime.rs:304,313`.

### R8 — LOW–MEDIUM: migration scope of MVU-ifying mature widgets
`buiy_widgets` is 22 imperative systems/observers over verified widgets; a big-bang rewrite is "likely
wrong" (charter). **Mitigation:** substrate first; widgets stay imperative *routers* and migrate to
Models one at a time, staying outside the replay boundary until ported (ties to R2's tiering). Open
question: do leaf widgets (Button) ever become actors, or only stateful ones (Checkbox/Slider/
TextField/Disclosure/Menu)? Evidence: `buiy_widgets/src/lib.rs:189-379`; charter "widget granularity".

---

## 7. Open questions (for the spec stage)

1. **Command-sourcing vs state-sourcing, committed globally?** R3 forces command-sourcing for the
   editor; consistency argues for it everywhere (log the `Msg`, never the Model snapshot). Confirm and
   write it into the spec as *the* recording model.
2. **Widget granularity.** Every widget a `Model`+reducer, or only stateful widgets become actors with
   leaf widgets staying imperative routers? Perf (R1) + ergonomics decide; the prototype should measure
   a thousands-of-widgets actor count.
3. **`ApplyDeferred` placement vs bevy_picking's own sync points.** The § 5 flush must not collide with
   picking's internal `apply_deferred`; verify against `PickingSystems` ordering in the prototype.
4. **`LogicalId` assignment strategy** unified with the agent-interface test-id space (deterministic;
   no `getrandom`). Counter, structural path-hash, or AccessKit `NodeId` reuse?
5. **Bind/derive ergonomics** (`bind(|m| …)` + `derive!`) — still open from proto-1/2; affects whether
   `MvuSet::Bind` is hand-written systems or a generated layer.
</content>
</invoke>
