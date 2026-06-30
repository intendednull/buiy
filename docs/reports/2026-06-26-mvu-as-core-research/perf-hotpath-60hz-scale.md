# MVU-as-core research — PERFORMANCE: hot path, 60 Hz floor, scale

**Date:** 2026-06-26
**Stage:** RESEARCH (a `/staged-development` gate input for proto-3 "MVU as the core").
**Area:** the load-bearing performance risk — *can "every widget is an actor" (a funnel
/ drain / record on every interaction, Reflect-serialize on the hot path, a per-entity
Model + mailbox at thousands of widgets) survive the 60 Hz hard floor on weak machines?*
**Base:** current `origin/main` worktree `/mnt/storage/projects/buiy/.claude/worktrees/mvu-core`,
main @ `5c0da9f` (perf campaign #84 merged on top of the parity #83 `7752c01`).
**Method:** read the perf campaign infra (audit + design + results + bench + gates), the
proto-2 native runtime mechanics, and the current interaction/extract hot path. Analysis
only; no production code modified.

---

## TL;DR (the headline, against the charter's own framing)

The charter names **"Reflect-serialize cost on the hot path"** as *"the thing that kills
'every widget is an actor.'"* The evidence cuts against that framing. After quantifying it:

1. **The funnel/drain/record cost is input-bounded, not widget-bounded, and lands ~3
   orders of magnitude below the costs that actually consume the 60 Hz budget.** Message
   rate is `O(interactions/frame)` (a click, a keystroke, a scroll tick) — typically
   single-digit per frame — **never** `O(widgets)` and **never** `O(frames × widgets)`.
   A small-Msg Reflect+RON serialize is ~low-µs; even a generous 100 msgs/frame is
   <3 % of a weak-machine frame budget, versus the **3.18 ms** a single full node
   re-extract at 5 000 nodes costs (`extract`, pre-#2). The serialize is in the noise.

2. **The real perf killer is not the funnel — it is the drain DEFEATING the damage gates
   the 60 Hz floor is built on.** The whole perf campaign (#2 keyed partial re-extract, #3
   gated post-Taffy passes, #5 O(1) atlas touch) buys its 60 Hz headroom from **precise
   change-detection**: a change touches `Changed<Background>` / `Changed<ResolvedLayout>`
   and *one* record re-extracts. If the MVU drain naively `&mut`-derefs every model on
   every message (tripping `Changed<M>` even on a no-op fold), every fold cascades into
   binds → layout → a full re-extract, and the campaign's wins evaporate. **That** is the
   thing that could break the floor — and it is a *solved* discipline in this codebase
   (`set_if_neq` / conditional `deref_mut`, used verbatim by the #2 patch path).

3. **"Every widget is a Model" is perf-FEASIBLE — on the per-instance and per-message
   axes.** The cost that *does* scale is **per-MODEL-TYPE, not per-instance**: one drain
   system + one observer + one `Messages<Envelope<M>>` per *model type*. Keep model types
   ≈ widget *kinds* (~20–50) and the idle floor is a small constant well under the existing
   `O(N_widgets)` layout/atlas idle cost. The boundary to draw is explicit: **a Model type
   per widget *kind*, never per widget *instance* or per list *row*.**

So: do not adopt the substrate behind the *Reflect-serialize* fear. Adopt it behind two
hard rules — **(a) recording is opt-out/lazy (off and zero-cost in production)** and
**(b) the drain obeys `set_if_neq` change-detection so it never defeats the damage gates** —
and add a hw-independent substrate gate (a work-counter twin + an iai bench) so both rules
are *enforced*, not asserted.

---

## 1. The perf budget / floor, and how it is measured + gated TODAY

### 1.1 The floor (a stated, re-tunable contract)

- **60 Hz / 16.7 ms per frame is a HARD FLOOR, never "good enough"; weaker machines are
  the explicit target — do not trust the dev box's RX 6700 XT headroom.**
  (`docs/specs/2026-06-26-buiy-performance-final-design.md` §0.)
- The floor is made **hardware-independent** by deriving a weak-machine *instruction*
  budget from a STATED reference: **≈2-wide in-order, ≈1.4 GHz, ≈0.7 IPC ⇒ ≈16 M
  instructions/frame @ 60 Hz**, each scale point expressed as a % of it. The reference
  clock/IPC is the named, re-tunable scaling assumption (design §0). This is the number
  the substrate must be budgeted against, **not** wall-clock on the dev GPU.

### 1.2 The one measured datapoint (the reason perf is load-bearing)

`docs/reports/2026-06-25-performance-audit.md` §2: a **static, warm-cache, 128-text-node
screen cost 10.87 ms/frame** (1.31× over 120 Hz; 0.65× of 60 Hz) and **45.38 ms at 512
nodes** (2.72× over 60 Hz) — *while nothing was changing*. Root cause was the #5 atlas-LRU
touch (O(visible glyphs × cells)/frame), now fixed to **O(1)** (**8.6×**, 10.6→1.24 ms in
the prototype; `docs/reports/2026-06-26-buiy-performance-final-results.md` §3). So a steady
text frame after #5 is ~1.2–1.5 ms on the dev box — real but *not large* headroom, and a
weak target machine is 5–50× slower. **The budget the substrate spends from is small.**

### 1.3 The gates (which are load-bearing, which are informational)

| Gate | Kind | HW-independent? | Cadence | Sees the substrate? |
|---|---|---|---|---|
| **Work-unit counters** (`render/counters.rs` `RenderWorkCounters`: `node_rebuilds`, `instances_built`, `node_patches`, `atlas_touch_ops`, `resident_keys`; main-world per-resource counts) | integers asserted EXACTLY on a settled scene (`tests/crosscut/work_counters.rs`) | **Yes** (plain ECS resources, wasm-safe) | every PR, headless | **only if we add MVU counters** |
| **dhat allocation-count** (`tests/alloc_budget.rs`, `#[global_allocator] dhat::Alloc`) | block/byte budgets; idle=33 / rebuild=64 blocks measured @ 1 000 nodes, budgets 50/120 | **Yes** (pure Rust, cross-platform) | every PR | only if MVU allocs in the harness |
| **iai-callgrind** (`EventKind::Ir` + `EstimatedCycles` = Ir + L1 + 5×LL + 35×RAM) | **the weak-machine backbone** (design §0/§1.3) | **Yes** (identical on dev GPU and a Celeron) | **DESIGNED, deferred to CI — NOT yet landed** (no local Valgrind; results.md §2 "deferred"); no `pipeline_iai.rs` exists on main | not yet |
| **criterion wall-clock** (`benches/pipeline.rs`) | mean/p-trend | No | INFORMATIONAL only, **never a CI gate** (DG-3, `benches/pipeline.rs` doc) | partially |

**Key facts for the substrate design:**
- The **work-unit counter pattern is the cheapest, highest-leverage gate** and the proven
  template (`work_counters.rs`: `assert_eq!(node_rebuilds, 0)` on idle). A substrate gate
  should be a twin: `drain_folds == 0` (and `messages_recorded == 0`) on an idle frame.
- The **iai-callgrind instruction gate is the *only* gate that prices the weak machine**,
  and **it is not built yet** — it is "the next CI lane" (results §6). Whoever lands the
  substrate must also land its iai twin, or the substrate's per-message cost is gated by
  *nothing* host-independent. This is a real gap to close, not inherit.
- The whole gating philosophy is **deterministic integers + instruction counts, never
  wall-clock** — because wall-clock flakes on shared runners and the CI GPU lane is
  lavapipe. The substrate must be measurable the same way: **count folds/records/binds**,
  not time them.

---

## 2. Cost model: funnel / drain / record on EVERY interaction

The mechanics being proposed for core are proto-2's runtime
(`examples/mvu_native/src/runtime.rs`). Walking the actual code, here is where each cost
lands and how big it is.

### 2.1 The enqueue edge — one boxed closure per message

`enqueue::<M>` (`runtime.rs:160-168`) does `commands.queue(move |world| world.resource_mut::<Messages<Envelope<M>>>().write(...))`.
**Cost: one heap allocation (the boxed closure) + a deferred resource write, per message.**
- This is an ergonomic choice for firing from *observers* (which only hold `Commands`).
  A system-side producer can write `Messages<Envelope<M>>` directly with a `MessageWriter`
  and pay **zero** box. **Re-decide:** prefer a direct writer where the producer is a
  system (the press bridge, scroll, typing); keep the boxed `commands.queue` only for the
  observer/callback edge. The dhat idle budget (50 blocks) catches a per-frame box blow-up.

### 2.2 The routing edge — an EntityEvent bubble per press

`bridge_press` (`routing.rs:34-47`) reads `Messages<OnPress>`, and per matching press
`commands.trigger(Routed::<M>{...})`. The `Routed<M>` `#[entity_event(propagate)]` bubbles
up `ChildOf`; a global `route_observer::<M>` (`routing.rs:54-66`) runs **at each ancestor
step** until the first model owner enqueues + halts.
**Cost: O(tree depth) observer dispatches per routed press** (~5–15 for typical UIs), only
on an actual press. Idle cost = 0 (observers don't fire without a trigger). Negligible —
presses are single-digit per frame.

### 2.3 The drain — one ordered fold pass per model type per frame

`add_reducer_env`'s `drain` (`runtime.rs:265-310`):
1. `inbox.read().cloned().collect()` into a `VecDeque<Envelope<M>>` — **clones every
   pending Envelope** (Msg: Clone) so `Emit` re-folds can run-to-completion without holding
   the reader borrow. `if work.is_empty() { return; }` — **idle = O(1) early-out** (a
   MessageReader fetch + emptiness check).
2. Per message: `ids.get(target)` → `log.record(...)` → `models.get_mut(target)` → fold →
   apply `Cmd` (`Emit` pushes back onto `work`, run-to-completion).

**Cost lands as:** idle → ~constant per model type (empty-check). Active → `O(messages
this frame + Emit re-folds)` × (one `get_mut` + the reducer body + one record). The reducer
body is user code (a `match`); the structural overhead is a query `get_mut`, a clone, and
the record.

### 2.4 The record — the "Reflect-serialize on the hot path" the charter fears

`MsgLog::record` (`runtime.rs:110-128`): `TypedReflectSerializer::new(msg, registry)` +
**`ron::ser::to_string(&ser)`** + a `Msg::type_path().to_string()`, **per message**, pushed
onto an unbounded `Vec<LoggedEntry>` (each holds two heap `String`s).

**Quantify it.** For a small Msg (e.g. `enum CounterMsg { Increment }` or
`ScrollBy(f32)`): a reflect type-info walk + a small RON string alloc (~20–80 B) + the
type-path String. Order **~1–5 K instructions and 2–3 allocations per message**. Set that
against the weak-machine budget (§1.1, ≈16 M instr/frame):

| Scenario | msgs/frame | serialize cost | % of 16 M weak-machine budget |
|---|---|---|---|
| Idle (no input) | 0 | 0 | **0 %** |
| A click | 1 (+~2 Emit re-folds) | ~15 K instr | **~0.1 %** |
| Typing (1 keystroke/frame) | 1 | ~5 K instr | **~0.03 %** |
| Scroll drag (1 ScrollMsg/frame) | 1 | ~5 K instr | **~0.03 %** |
| Pathological storm | 100 | ~500 K instr | **~3 %** |

For comparison, a **single full node re-extract at 5 000 nodes = 3.18 ms** (audit #2;
≈ the *entire* weak-machine frame budget). The record is ~1000× cheaper than the render
work a single interaction triggers downstream. **The Reflect-serialize is not the
hot-path killer.**

**The two real caveats** (both fully mitigable, §5):
- **Always-on recording = an unbounded `Vec<String>` memory leak.** Every keystroke,
  scroll tick, hover, and (if widget-internal state is funneled) caret-blink appends a
  `String` *forever*. Over a long session this is the actual cost of always-on recording —
  not CPU, *memory growth*. → ring buffer / cap / default-off.
- **Eager RON in the drain is wasted work even when recording.** The drain serializes to a
  String it will (almost always) never read. → store the typed message (or `Box<dyn
  Reflect>`); serialize **lazily** only at export/checkpoint. This removes the format+alloc
  from the hot path entirely (§5).

---

## 3. Scale: per-entity Model + mailbox + drain at thousands of widgets

This is where the design must be precise, because the charter's "thousands of widgets"
framing conflates two very different axes.

### 3.1 Per-INSTANCE cost (thousands of widgets) — essentially free

- **A Model is a `Component` (`runtime.rs:23` `Model: Component`).** A per-widget Model is
  just the widget's state component — which buiy **already stores per widget today**
  (`Checkbox`, `TextEditState`, etc.). At thousands of widgets this is the memory buiy
  already pays. The redesign must make the existing widget components *be* the Models, not
  add a Model wrapper on top (that would *double* per-instance memory — a real risk to call
  out, §6).
- **There is NO per-instance mailbox.** The inbox is **one `Messages<Envelope<M>>` per
  model TYPE** (`runtime.rs:42`), with `Envelope{ target: Entity, msg }` carrying the
  address. So 5 000 buttons share **one** Button inbox, not 5 000 mailboxes. This is the
  single most important scale fact and it cuts strongly *for* feasibility.
- **The drain is `O(messages/frame)`, not `O(instances)`.** A frame with 5 000 idle
  buttons and one click folds **one** message. The drain never iterates the 5 000.

### 3.2 Per-TYPE cost (the axis that actually scales) — a bounded constant floor

Per model type `M`, the wiring adds (proto-2): **1 drain system** (`Update`), **1
`bridge_press::<M>` system** (if routed), **1 global `Routed<M>` observer**, **1
`Messages<Envelope<M>>` resource** (double-buffered, swapped each `First` by Bevy's
`message_update_system`).

So the idle floor is `O(N_model_types)`:
- ~`N_types` drain systems each doing an empty-inbox check per frame. Estimate ~50–200 ns
  each incl. scheduler bookkeeping. At **~30–50 model types** (≈ the mature widget set:
  Button/Checkbox/Switch/Slider/TextField/Menu/Dialog/ScrollArea/… ~20 + app models
  ~10–30) → **~5–10 µs/frame on the dev box**, ~25–75 µs on a 5–10× weak machine =
  **<0.5 % of the 16.7 ms budget**.
- ~`N_types` `Messages` double-buffer swaps in `First` — trivial.
- Observers fire only on trigger → idle = 0.

**This floor does not change the idle *complexity class*.** The existing idle cost is
`O(N_widgets)` (the #3 ungated post-Taffy passes, the #5 atlas touch — thousands of
entity-visits at thousands of widgets). The MVU floor is `O(N_types)` — a *smaller* term.
MVU adds a small constant to an already-`O(N_widgets)` idle frame.

### 3.3 The boundary (state it as a hard design rule)

The floor is fine **iff `N_model_types` stays bounded by widget *kinds*.** It breaks if the
design ever mints a Model *type* per instance, per list *row*, or per widget *variant*:

- ✅ One `Button` model type, 5 000 instances → 1 drain, 1 inbox. Fine.
- ✅ A virtualized 10 000-row list where each row is a `Row` model → 1 `Row` type, 1 drain.
  Fine (and pairs with the audit's noted need for a virtualized-list primitive).
- ❌ A distinct model *type* per row, or per dynamically-composed widget → hundreds–
  thousands of drain systems + observers + `Messages` resources. **This is the
  perf-infeasible region.** On **wasm (single-threaded)** it is worse: Bevy can't
  parallelize the per-type drains, so `N_types` empty-checks run fully serially every
  frame.

**Boundary verdict:** "every widget is a Model" is perf-feasible **when "widget" means
widget *kind*.** "Every widget *instance* is its own actor *type*" is not. The spec must
make model-type identity = widget-kind identity, and provide a *generic/parameterized*
model for data-driven lists rather than codegen-per-row.

### 3.4 Memory at thousands of widgets

- Per-instance: the Model component (≈ existing widget state). No new growth **if** Models
  reuse existing components. `TextEditState` (`text/edit/state.rs:93`) is already heavy
  (a cosmic-text `Editor<'static>` + undo stack + intrinsics + IME span) — funneling it
  through MVU does not add to it; it's already resident.
- The log: the only **unbounded** structure (§2.4). Cap it.

---

## 4. The actual load-bearing risk: the drain must not defeat the damage gates

This is the finding the charter's framing misses, and it is the one that can genuinely
break the 60 Hz floor.

The 60 Hz headroom on weak machines exists **because of precise change-detection**:
- #2 keyed partial re-extract: a hover re-tint re-resolves + re-uploads **1** record, not
  N — and it is correct **only because** the classifier reads exact `Changed` filters and
  the patch path *"must NOT `deref_mut` `ExtractedEffectGroups` on a Patch (keeps
  `groups.is_changed()==false`)"* (design §2). The codebase already treats *spurious*
  change-detection as a perf bug.
- #3 gated post-Taffy passes: gated on a `Changed`-union dirty flag; an over-firing
  `Changed` re-runs ~12 full-tree O(N) passes.

**If the MVU drain folds a message and unconditionally `&mut model` (proto-2's drain does
`models.get_mut(target)` and the reducer takes `&mut M`, `runtime.rs:290/300`), it trips
`Changed<M>` every fold — even a no-op fold.** That `Changed<M>` flows into the view-binds
(`MvuSet::Bind` reads `Changed<Model>`), which rewrite `Text`/`Style`, which re-trip layout
+ the #3 passes + a node re-extract. **A caret-blink message (2/sec) or a hover message
would then force a full-scene rebuild — exactly the #6/#2 cliffs the perf campaign just
eliminated.** The funnel would *re-introduce* the costs the campaign removed.

**Severity: HIGH** — this silently reverses the perf campaign. **But it is a solved
discipline here:** the drain must apply the mutation through `set_if_neq` /
`DetectChangesMut` (the bench already imports `DetectChangesMut`,
`benches/pipeline.rs:29`) — `deref_mut` the model **only when the fold actually changes
it**, never on a no-op or idempotent fold. The reducer returns its new state; the drain
diffs and conditionally writes. The same `bg.set_changed()`-discipline the #2 path lives
by. This must be a **spec invariant + a gate** (`drain_folds` can be N while
`models_mutated`/`binds_fired` stays 0 on an idempotent fold), not a convention.

---

## 5. Mitigations (the charter's four, assessed + a fifth)

| Mitigation (charter) | Verdict | How |
|---|---|---|
| **(a) recording opt-out / sampling** | **Adopt — default OFF in production.** | A `RecordMode { Off, Ring(n), Full }` resource, default `Off`. `Off` = the drain skips `record()` entirely (proto-2's `MsgLog.recording` flag already does this, `runtime.rs:120`) → **zero serialize, zero log growth in production**. `Ring(n)` for always-on crash-repro / agent sessions (bounded memory). `Full` only for explicit record/replay. Recording is a *session mode*, not an always-on tax. |
| **(b) lazy / deferred Reflect** | **Adopt — strictly better than proto-2.** | Do **not** `ron::ser::to_string` in the drain. Record the **typed message** (or `Box<dyn Reflect>` + `LogicalId` + seq) into a typed per-type ring; serialize to RON **only** at export / replay-checkpoint / agent-read time. Removes the format + String alloc from the hot path **even when recording is on**. The drain's record cost drops to an enum move + a ring push. |
| **(c) record-only-on-change** | **Already true for messages; extend to model + binds.** | Messages only exist when input produces them (input-bounded, §3.1), so the *message* log is already change-only. The discipline that matters is on the **model mutation** (§4, `set_if_neq`) and **bind firing** (`Changed<Model>` → only fire the bind when the model actually changed). |
| **(d) lazy/deferred + sampling for hot continuous gestures** | **Adopt for scroll/drag/IME.** | A continuous gesture (scroll drag) should coalesce to **one message/frame** (already natural — input is sampled per frame), and the model mutation goes through `set_if_neq` so a no-movement frame is a no-op. Do not emit a message per pointer sub-event. |
| **(e) [added] `set_if_neq` drain discipline as a gate** | **Adopt — this is the real protection (§4).** | The drain conditionally `deref_mut`s; a substrate work-counter (`drain_folds`, `models_mutated`, `binds_fired`) asserts that an idempotent fold mutates 0 models and fires 0 binds. This is what stops the funnel from defeating #2/#3. |

---

## 6. Recommended perf strategy for the substrate

1. **Make recording a default-OFF, lazily-serialized session mode.** Production pays
   **zero** for the log (no serialize, no growth). Record/replay/agent/crash-repro flip a
   `RecordMode`; even then, store typed messages in a bounded ring and serialize lazily at
   export. This neutralizes the charter's #1 fear by construction. (Decision D1.)

2. **Mandate `set_if_neq` change-detection in the drain + binds, and gate it.** The drain
   `deref_mut`s a model only on a real change; binds fire only on `Changed<Model>`. This is
   the load-bearing rule that keeps #2/#3/#5 intact. Enforce with a substrate work-counter
   gate, not a comment. (Decision D2 — the most important one.)

3. **Bound model-type count to widget *kinds*; provide a parameterized model for
   data-driven lists.** No type-per-instance / type-per-row. Document the `O(N_types)`
   idle floor and gate it. (Decision D3.)

4. **Reuse existing widget components AS Models — do not wrap.** The MVU-ification reshapes
   `Checkbox`/`TextEditState`/focus/scroll *into* Models; it must not add a parallel Model
   component beside them (double memory at thousands of widgets). This is a core-redesign
   constraint, not a layering. (Decision D4.)

5. **Land the substrate's hw-independent gates *with* the substrate** (the campaign's
   "red→green against a gate" discipline, design §0):
   - **Work-unit counters** (the cheapest gate, the proven template): `MvuWorkCounters {
     drain_folds, messages_recorded, models_mutated, binds_fired, emits_refolded }`, init'd
     in both the real app and the bench harness (mirroring `RenderWorkCounters`). Idle
     assertions: `drain_folds == 0 && messages_recorded == 0 && models_mutated == 0`. An
     idempotent-fold assertion: `drain_folds == k && models_mutated == 0`. This catches the
     §4 gate-defeat regression directly.
   - **dhat budget**: recording-OFF idle adds **0** allocs over the current 33-block
     baseline; recording-ON one-message ≤ a small committed band (the ring push). Catches
     the boxed-closure / per-frame-Vec blow-ups (§2.1/§2.3).
   - **iai-callgrind twin** (the weak-machine backbone — **and it does not exist yet**, so
     this is net-new work the substrate owns): `mvu_idle/{50 model types}` proves the idle
     floor is flat in widget count; `mvu_one_message` and `mvu_fold_storm/{1,10,100}` price
     the per-message instruction cost against the 16 M-instr weak-machine budget; an
     `mvu_record_off_vs_on` pair prices the record tax. Land informational-first, flip to
     `--regression=Ir` once stable (the design's own iai cadence, §1.3).
   - **Extend the criterion `pipeline` bench** (informational) with an MVU scene: settle a
     1k/5k-widget tree, drive one message/frame, confirm the steady frame stays flat —
     i.e. the funnel did not re-introduce the all-or-nothing rebuild.

6. **WASM: zero new obstacles, confirmed.** The serialize uses `bevy_reflect`'s
   `TypedReflectSerializer` + `ron` (already transitively in `Cargo.lock:702`/`4494` via
   bevy_scene/bsn) or the workspace `serde_json` — all pure Rust, wasm-safe. The
   counters are plain ECS resources; dhat/iai/criterion are dev-only / Linux-CI-only and
   never enter the production graph (design §0). The one wasm-specific note: the
   `O(N_types)` drain floor is **fully serial on wasm** (single-threaded) — another reason
   to bound model-type count (§3.3). No new per-instance field, no compute/threads dep —
   consistent with the #82 WASM constraints.

### Is "every widget is a Model" perf-infeasible? — explicit answer

**No, it is feasible — with a stated boundary.** Feasible because: (i) message rate is
input-bounded, so the funnel/record cost is ~3 orders below the render work a single
interaction already triggers; (ii) there is no per-instance mailbox (one inbox per type);
(iii) a Model is a component buiy already stores. The boundary that makes it infeasible if
crossed: **a Model *type* per instance/row/variant** (the `O(N_types)` floor explodes,
worst on single-threaded wasm), and **a drain that defeats change-detection** (re-introduces
the #2/#3 cliffs the floor depends on). Stay inside the boundary (type = widget *kind*,
`set_if_neq` drain, opt-out lazy recording) and the substrate is well within the 60 Hz
floor; the dominant per-frame cost remains layout + extract, exactly as today.

---

## 7. Decisions (recommendation + runner-up)

**D1 — Recording placement & eagerness.**
- *Recommend:* default-OFF `RecordMode`; **lazy** serialize (store typed messages in a
  bounded ring, serialize to RON only at export). Production pays zero.
- *Runner-up:* proto-2's always-on eager `ron::ser::to_string` in the drain with a
  `recording` bool. Rejected: even gated, eager RON is wasted format+alloc, and an
  always-on Full log is an unbounded memory leak (§2.4).

**D2 — Drain change-detection (the load-bearing one).**
- *Recommend:* the drain `deref_mut`s a model only on a real change (`set_if_neq` /
  `DetectChangesMut`); binds fire only on `Changed<Model>`. Gate it with an MVU work-counter.
- *Runner-up:* proto-2's unconditional `models.get_mut` + `&mut M`. Rejected: trips
  `Changed<M>` on no-op folds → cascades into binds → re-extract, **re-introducing the
  #2/#6 cliffs** the perf campaign eliminated (§4). HIGH-severity if shipped as-is.

**D3 — Model-type granularity.**
- *Recommend:* a Model *type* per widget *kind*; a single parameterized model for
  data-driven/virtualized lists. Document + gate the `O(N_types)` idle floor.
- *Runner-up:* a Model type per composed widget / per row. Rejected: hundreds–thousands of
  drain systems + observers + `Messages` resources, serial on wasm — the perf-infeasible
  region (§3.3).

**D4 — Model storage.**
- *Recommend:* reshape existing widget components *into* Models (the charter's core-redesign
  opportunity #4) — no parallel Model component.
- *Runner-up:* a Model wrapper layered beside existing widget state. Rejected: doubles
  per-instance memory at thousands of widgets (§3.4).

**D5 — Enqueue edge.**
- *Recommend:* direct `MessageWriter<Envelope<M>>` from system-side producers (press
  bridge, scroll, typing); boxed `commands.queue` only for the observer/callback edge.
- *Runner-up:* proto-2's universal `commands.queue` box. Rejected: one heap alloc per
  message where a writer is free (§2.1); dhat-visible.

**D6 — Substrate gates (must ship WITH the substrate).**
- *Recommend:* MVU work-counters (idle + idempotent-fold asserts) + a dhat record-off/on
  band + a net-new iai-callgrind twin (`mvu_idle`/`mvu_one_message`/`mvu_fold_storm`).
- *Runner-up:* rely on the existing render counters + wall-clock criterion. Rejected: the
  render counters can't see the drain; wall-clock isn't a gate (DG-3); **the iai weak-machine
  backbone does not exist yet** and is the only host-independent pricer of per-message cost.

---

## 8. Risks (severity · evidence · mitigation)

1. **[HIGH] Drain defeats the damage gates → re-introduces the #2/#6 full-rebuild cliffs.**
   *Evidence:* proto-2 drain unconditionally `get_mut`s the model (`runtime.rs:290`); the
   #2 path is correct *only* by avoiding spurious `deref_mut` (design §2; `groups.is_changed()==false`
   discipline). A no-op fold tripping `Changed<M>` cascades bind→layout→extract.
   *Mitigation:* D2 — `set_if_neq` drain + binds-on-`Changed` + an MVU work-counter gate
   asserting `models_mutated == 0` on an idempotent fold.

2. **[HIGH] Model-type explosion (type-per-instance/row) blows the `O(N_types)` idle
   floor; worst on single-threaded wasm.** *Evidence:* per-type wiring = 1 drain + 1
   observer + 1 `Messages` (`runtime.rs`/`routing.rs`); Bevy can't parallelize per-type
   drains on wasm. *Mitigation:* D3 — type = widget *kind*; a parameterized model for lists;
   gate the floor (`mvu_idle/{N types}` iai bench flat in widget count).

3. **[MED] Always-on recording = unbounded `Vec<String>` memory growth.** *Evidence:*
   `MsgLog.entries: Vec<LoggedEntry>` with two Strings each, appended per message
   (`runtime.rs:110-128`), never trimmed. *Mitigation:* D1 — default-OFF; `Ring(n)` for
   always-on sessions; lazy serialize so even `Full` stores typed messages, not Strings.

4. **[MED] The weak-machine pricer (iai-callgrind) does not exist yet.** *Evidence:*
   results.md §2/§6 "deferred to CI"; no `pipeline_iai.rs` on main. Without it the
   substrate's per-message cost is gated by nothing host-independent. *Mitigation:* D6 —
   land the iai twin *with* the substrate (informational-first, then `--regression=Ir`).

5. **[LOW] Per-message allocations (boxed `commands.queue` closure; `inbox...cloned().collect()`
   VecDeque).** *Evidence:* `runtime.rs:160-168`, `runtime.rs:267`. *Mitigation:* D5
   (direct writer) + the dhat budget; both are amortized small and only on the active path.

6. **[LOW] Double per-instance memory if Models wrap existing widget state.** *Evidence:*
   `TextEditState` already heavy (`text/edit/state.rs:93`); a parallel Model would double
   it ×N_widgets. *Mitigation:* D4 — reshape components into Models, don't wrap.

---

## 9. Open questions for the spec stage

1. **What is the real per-frame Full/Patch/retain mix once widget-internal state (focus,
   scroll, caret, IME) flows through the funnel?** The perf design itself flags this as
   audit-open ("measure the real per-frame Full/Patch/retain mix on the gallery + todomvc
   before sizing the glyph-buffer mirror", design §2). The substrate changes *what*
   produces the `Changed` signals; the spec needs the measured mix on a real gallery
   MVU-ified scene before committing the bind topology.
2. **Should the drain be one system per type, or one type-erased drain over a model-type
   registry?** Per-type is simplest and parallelizes off-wasm; a type-erased drain caps the
   system count (helps the §3.3 wasm-serial floor) at the cost of dynamic dispatch. Decide
   against the `mvu_idle/{N types}` iai number.
3. **Caret blink specifically** — does it become a Msg, and if so does D2 keep it off the
   re-extract path? The audit's #6 (caret blink → full re-extract) is exactly the kind of
   high-frequency widget-internal signal the charter wants funneled; the spec must show the
   blink Msg + `set_if_neq` keeps `node_rebuilds == 0`.
4. **Replay determinism vs. lazy serialize** — if recording stores typed `Box<dyn Reflect>`
   and serializes lazily, the cross-process byte-identical replay proof (proto-2's capstone)
   must still hold at the serialize boundary; confirm the lazy path round-trips identically.
