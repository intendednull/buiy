# RD2 — L1: the funnel-routed high-frequency hot-path perf gate (open-Q11)

**Decision: caret-blink IS the canonical signal and IS routable, but only as a
per-frame idempotent-dominant LEAF fixture driven by a net-new tick "moral
subscription". The gate is a hard binary (steady frame ⇒
`models_mutated == binds_fired == node_rebuilds == 0`) whose PASS is near-certain
by composition of two already-proven edge-gates. The load-bearing new product is
a roadmap iai number bounding per-frame-routed-signal fixed cost. Do NOT migrate
production caret-blink to the funnel.**

Confidence: **high**.

---

## 1. Canonical signal = caret-blink (idempotent-dominant per-frame tick)

Evaluated every frame (~60/s) but flips only ~2/s (every 500 ms half-period), so
~58/60 frames are idempotent folds — the only candidate that exercises the
no-cascade property. Verified on the FINAL base:

- `write_caret_blink` computes `blink_phase` and writes `CaretVisual.visible`
  **edge-only** via a manual `if caret.visible != phase` (a hand-rolled
  `set_if_neq`) — `crates/buiy_core/src/text/visual.rs:75-108`. NOT funnel-routed.
- Scheduled `.after(BuiySet::Animate).before(BuiySet::Picking)` —
  `crates/buiy_core/src/text/mod.rs:230-235` (the window a funnel blink-drain must
  occupy).

Rejected alternatives, with reasons:

- **Scroll** (`auto_scroll_caret`, `scroll.rs:87-123`) is caret-move-EDGE-driven,
  not per-frame, and `ScrollOffset` deliberately does NOT invalidate Taffy — wrong
  shape.
- **Slider-drag** (`A11yValue.now`, `states.rs:155-237`) genuinely changes every
  drag frame (NOT idempotent) — the right **secondary** genuine-change bench, not
  the no-cascade canonical.
- **IME-preedit** is per-keystroke edge — not per-frame.

---

## 2. Routability

Caret-blink is render-prep-only today and routable as a leaf model
(the `A11yToggled`/D2 "reuse the component as Model" pattern). Minimal code to
**actually** route it (the measurement is fake otherwise):

- (a) `enum BlinkMsg { Tick(Duration) }` (`Reflect` — the funnel demands it);
- (b) a leaf model — a **dedicated bench `BlinkLeaf { visible, origin }`** whose
  `Changed`-gated bind writes `CaretVisual`. **Do NOT reuse `CaretVisual` as the
  Model**: `CaretVisual` is explicitly NOT reflect-registered
  (`crates/buiy_core/src/text/components.rs:414-433`, "machinery state"); making it
  a Model would force `Reflect` and perturb production. The dedicated bench model
  avoids that.
- (c) reducer `Tick(now) -> visible = blink_phase(now - origin)`, `Cmd::none()`,
  committed via the existing `set_if_neq` drain;
- (d) **THE net-new piece**: a per-frame enqueue system that writes
  `BlinkMsg::Tick(time.elapsed())` for each caret EVERY frame. The Cmd algebra is
  `None`/`Emit`/`Batch` only (`mvu/mod.rs:116-124`) — NO timer/Subscription — so
  this "moral subscription" must exist;
- (e) drain installed via the existing `add_reducer_in_set` primitive in the
  `.after(Animate).before(Picking)` window where `write_caret_blink` runs today, so
  extract still reads a settled `CaretVisual`.

---

## 3. The exact benches/tests

- **iai `mvu_blink_cadence`** (hw-independent, CI-gateable; added to the existing
  group in `benches/mvu_iai.rs` beside `mvu_idle`/`mvu_one_message`/
  `mvu_fold_storm`/`mvu_record_off_vs_on`): drive N frames of per-frame `Tick` at
  60 Hz through the REAL drain; assert across one half-period exactly ONE flip
  frame (`models_mutated == 1`) and all others `models_mutated == 0 &&
  binds_fired == 0`. The Ir/instruction count prices the per-frame, per-signal
  funnel fixed cost (enqueue + `ApplyDeferred` flush + inbox read + clone + reducer
  + `set_if_neq` `PartialEq` + the `count_binds` `Changed<M>` scan).
- **headless crosscut `blink_funneled_node_rebuilds_zero`** (the literal open-Q11
  resolution): couple the blink leaf's `CaretVisual` output into the real text
  glyph-extract harness (`buiy_bench_support` already inits `RenderWorkCounters`,
  `lib.rs:97-99`) and assert `RenderWorkCounters.node_rebuilds == 0` on every steady
  frame and `== 1` on the flip frame. `binds_fired == 0` is only the **main-world
  proxy**; `node_rebuilds` is the render-world truth open-Q11 names
  (`SYNTHESIS.md:210`, `render/counters.rs:24-28`). The synthesis **commits** to
  this render-coupled test — the proxy is not "good enough" to close the named
  must-fix.

---

## 4. Threshold (tied to the 60 Hz hard floor / ~16M-instr weak-machine budget)

- **HARD BINARY GO/NO-GO:** steady (non-flip) frame ⇒
  `models_mutated == 0 && binds_fired == 0 && node_rebuilds == 0`, EXACT integers.
  Any nonzero ⇒ the funnel re-introduces audit #6 (caret-blink → full re-extract)
  and the maximalist framing is dead.
- **SOFT iai REGRESSION BAND (the real new info):** a single idempotent blink-tick's
  funnel cost ≈ the substrate-proven ~525 instr/fold + enqueue/ApplyDeferred/
  Changed-scan overhead = low thousands of instructions ≈ <0.05% of 16M. Set a
  generous ceiling (fail `> 5K instr/frame` for one steady blink tick ≈ 0.03% of
  budget — catches a ~10× regression). The load-bearing product is this
  **per-frame-routed-signal fixed cost**: it bounds how many simultaneous
  per-frame funnel signals fit in one weak/wasm-single-threaded frame.

---

## 5. Gate vs roadmap — honest verdict

It is GO/NO-GO in **framing** (the named research must-fix) but PASS is
near-certain because it is the **composition** of two already-proven edge-gates:

- the substrate's `set_if_neq` ⇒ `models_mutated == 0 && binds_fired == 0` on an
  idempotent fold (proven: `tests/crosscut/mvu.rs:139-158`
  `mvu_idempotent_fold_no_mutation_no_bind`);
- the existing render extract's `Changed<CaretVisual>` ⇒ `node_rebuilds == 0` on a
  steady caret (proven: `text/extract.rs:224-277`, `Changed<CaretVisual>` at :250).

The only un-proven composite is wiring them at cadence end-to-end. So it is
**CONFIRMATORY**, not a coin-flip; run it in W1/W3 as scheduled, predicted PASS.
The genuinely new, decision-shaping output is the per-frame fixed-cost number.

**Fallback (pre-written so a fail is clean, not a scramble):** if a steady frame
trips `node_rebuilds`, the maximalist "funnel-route any hot signal" framing dies
and the narrower framing takes over — *"the funnel governs input-sourced state;
timer/animation render-prep signals stay out."* The spec must carry this paragraph.

---

## 6. Sharp corollary — do NOT migrate production caret-blink

It is already optimally edge-gated (the manual `!=`-guard), AND its visibility is a
pure function `visible = blink_phase(now − origin)` where `origin` is already
command-sourced (reset on edits/caret-moves, which ARE in the editor's
command-source log). So byte-identical replay re-derives blink from the
deterministic clock + logged edits WITHOUT a logged Tick stream. Funneling it adds
per-frame enqueue/ApplyDeferred/clone/inbox cost (strictly WORSE than the current
single Query+`!=` writer) for ZERO perf and ZERO replay benefit. **Caret-blink is
the canonical measurement fixture, not a production migration target.**

---

## Cross-finding scope trap (RD2 × RD3)

The blink bench's per-frame `Tick` "moral subscription" is **net-new** (Cmd algebra
has no timer). Keep **"measure"** and **"migrate"** strictly separate: the bench's
per-frame enqueue must NOT leak into a production caret-blink `Subscription`
expectation before RD3's effects/sources phase, and production caret-blink stays
render-prep. State this boundary explicitly so the fixture is not mistaken for a
production primitive.

---

## Residual open-for-spec

- Confirm the **render-coupled** node_rebuilds test (committed here) vs the lighter
  proxy — the synthesis picks render-coupled; the plan must budget the heavier
  harness.
- Realistic MAX simultaneous per-frame-routed signals (carets + animating widgets)
  × measured per-signal cost vs 16M sets how tight the iai ceiling is, and whether
  any per-frame funnel routing is advisable on single-threaded wasm.
- iai supply chain: the iai bench pulls unmaintained crates (RD4) — the dev-only
  cargo-deny advisory exception is a prerequisite for a green gate run.

## Key evidence

- `text/visual.rs:75-108` `write_caret_blink` (edge-gated, not funnel-routed);
  `text/mod.rs:230-235` (the schedule window); `text/components.rs:414-433`
  (`CaretVisual` not reflect-registered).
- `text/extract.rs:224-277` (`Changed<CaretVisual>` re-extract trigger);
  `render/counters.rs:24-28` (`node_rebuilds`); `buiy_bench_support/lib.rs:97-99`.
- `tests/crosscut/mvu.rs:139-158` (idempotent-fold no-cascade proof);
  prototype `mvu/mod.rs:624-652` (~525 instr/fold drain), `:454-465` (`count_binds`),
  `:116-124` (Cmd algebra None/Emit/Batch only); `leaf.rs:45-147` (leaf template).
- prototype `benches/mvu_iai.rs:42-95` (group with NO blink bench — the gap);
  `mvu_scenes.rs` (only Counter models).
- `SYNTHESIS.md:210` (open-Q11 names `node_rebuilds`), `:19` (H3 cliff).
