# Track A — The Agent Probe / "Eyes" — Implementation Plan

> Use `subagent-driven-development`; gate + verify each wave. Realizes the north-star spec §4 Track A. Base: `origin/main` @ `9c4b881` (Track B merged).

**Goal:** Make the semantic-tree feedback loop a **first-class, agent-facing tool**: run a Buiy scene **headless + GPU-free**, and read a **stable, diffable snapshot** (roles / accessible names / state / layout rects / text content) the agent's build/test loop can observe — closing the author→build→run→inspect loop the prototype proved (principles P1/P5). Built on the *shipped* `a11y::inprocess` driver (snapshot/perform/click/get_by_role/…) — do not reinvent it.

**Architecture:** Two additions + one exposure. (1) `BuiyProbePlugin` — a headless **no-render** preset (composes Core/Theme/A11y/Focus/Layout/Text/Widgets, omits render/picking/winit) so a probe run needs no adapter (the prototype found `BuiyHeadlessPlugin` pulls `BuiyRenderPlugin` → wants a RenderApp; the probe wants GPU-free). (2) An agent-facing **snapshot serializer** — `a11y::inprocess::snapshot_report(world) -> String`: a Playwright-style indented semantic tree augmented with layout rect + text content (the SemanticTree omits plain `Text`; the prototype found agents need it). (3) Prelude/`buiy` exposure + a `buiy_probe` example (the productionized `examples/llm_probe`).

**Non-goals (deferred):** the `buiy_mcp` transport (Phase-2, separate); a GPU pixel-readback lane; live-drive orchestration beyond the existing `perform`/`click`/`wait_for`.

**Snapshot format (decided):** a **Playwright-style indented text tree** — token-efficient, human+agent readable, matches the a11y-snapshot agents already consume (research feedback-loop facet). One line per node: `<role> "<name>" [<state>] @<x,y wxh>`, children indented; a trailing `--- text ---` section lists non-a11y `Text` content + zero-size flags (catches "invisible content"). JSON is a trivial follow-on (serde on `SemanticTree`), not v1.

---

## Wave 1 — `BuiyProbePlugin` (headless, no-render preset)

**Files:** `crates/buiy/src/lib.rs` (next to `BuiyHeadlessPlugin`) or a `crates/buiy_core` module; `crates/buiy/tests/` (a headless probe test).

- [x] **1.1** Failing test: build an app with `MinimalPlugins` + `InputPlugin` + `AssetPlugin` + `BuiyProbePlugin`, spawn `Button::new("Save")`, step frames, `snapshot(world)` → a Button node named "Save" (no GPU/window). — `crates/buiy/tests/probe.rs`.
- [x] **1.2** Define `BuiyProbePlugin` composing the sub-plugins the prototype hand-listed (Core/Theme/A11y/Focus/Layout/Text::default/Widgets), **omitting** `BuiyRenderPlugin`, picking, winit, scroll/animation (add-on). Doc the composition + the "no adapter needed; semantic tree + layout are pure ECS" rationale. — `crates/buiy/src/lib.rs`.
- [x] **1.3** Run 1.1 → PASS. Commit (`6a517de`).
- [x] **GATE 1:** composition confirmed GPU-free (omits render/picking/winit); full-workspace build green incl. the probe test; verified by running `cargo run -p buiy_probe`.

## Wave 2 — Agent-facing snapshot serializer

**Files:** `crates/buiy_core/src/a11y/inprocess.rs` (or a sibling `report.rs`).

- [x] **2.1** Failing test: `snapshot_report(world)` surfaces the Button a11y node AND the plain role-less title text "Settings" (which the bare SemanticTree omits). — `crates/buiy/tests/probe.rs::snapshot_report_surfaces_a11y_node_and_plain_text`.
- [x] **2.2** Implement `snapshot_report(world: &mut World) -> String`: walk `snapshot()`'s `SemanticTree` in document order, indent by depth, emit `role "name" [state] @x,y wxh`; rect via `entity_for_node_id` + `ResolvedLayout` (`@?` when absent); append a `--- text & layout ---` section over `Text`/`A11yLabel`-bearing laid-out entities (`[ZERO-SIZE]` flag). Deterministic — tree walk in document order, text rows sorted `(y, x, Entity)`, HashMaps for lookup only. — `crates/buiy_core/src/a11y/report.rs`.
- [x] **2.3** Run 2.1 → PASS. Commit (`9590d67`). **GATE 2:** format stable/diffable/deterministic; surfaces both gaps (plain text; zero-size flag). Verified live via `cargo run -p buiy_probe`.

## Wave 3 — Exposure + example

**Files:** `crates/buiy/src/lib.rs` (re-export `BuiyProbePlugin` + the `a11y::inprocess` driver surface through the prelude); `examples/buiy_probe/` (productionized `llm_probe`).

- [x] **3.1** Group the driver + `BuiyProbePlugin` under a `buiy::probe` module (the front door). Kept a distinct module rather than flattening into the prelude — the generic verbs (`click`/`focus`/`snapshot`/`perform`) would collide with the widget/focus surface, mirroring the `buiy::view` precedent. — `crates/buiy/src/lib.rs`.
- [x] **3.2** `examples/buiy_probe`: a `scene()` slot + a `main` that runs it under `BuiyProbePlugin`, prints `snapshot_report`, then *drives* it (clicks the checkbox → `[unchecked]`→`[checked]` observable in the next snapshot). Self-verifying lib test. Doc'd in the example header. — `examples/buiy_probe/`.
- [x] **3.3** Commit (`7be0e0f`). **GATE 3:** the loop is reachable + documented via `buiy::probe`; the example RUNS GPU-free and prints a correct report (verified). **Fresh-context reviewer: APPROVE-WITH-NITS** (composition provably GPU-free, serializer deterministic byte-identical across two processes, loop end-to-end). Findings addressed: **F1** (rect + reading-order sort now source ABSOLUTE coords from `GlobalTransform`, not parent-relative `ResolvedLayout.position` — regressed by `nested_node_reports_absolute_position` @70,70 + an `absolute_pos` unit test), **F2** (probe-module doc rationale corrected: altitude/prelude-pollution, NOT name collision — verified the verbs don't collide), **F3** (co-located `buiy_core` unit tests for `state_tokens` + `absolute_pos`), **F4** (text section no longer re-echoes a11y-node labels unless zero-size — denser report), **F5** (`selected`/`unselected`/`modal` state tokens added). F6 = rebase + docs-index (below).

## Wave 4 — Verify + docs + PR

- [x] **4.1** Full gate GREEN: `cargo fmt --all --check` clean; `cargo clippy --workspace --all-targets --locked -- -D warnings` clean; `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked` clean (fixed intra-doc links); `cargo nextest run --workspace --locked` = **1890/1890 passed, 97 skipped** (the GPU `#[ignore]` lane). Track A touches zero render-path code, so the GPU lane is unaffected — CI's pinned-lavapipe leg confirms on the PR.
- [x] **4.2** Flipped spec §4 Track A note (LANDED). `AGENTS.md`-style pointer deferred to Track D (its deliverable). No new standalone doc → `docs/README.md` unchanged (the plan + spec + example cover it).
- [ ] **4.3** PR `feat(probe): BuiyProbePlugin + agent-facing semantic-tree snapshot (Track A)` → `main`; rebase onto current `origin/main` first; green CI; merge on green (owner-authorized loop).

## Self-review
- Realizes spec §4 Track A (loop/eyes) for the headless-authoring case; `buiy_mcp` + GPU pixel lane explicitly deferred.
- Low risk: packages the *shipped, tested* `a11y::inprocess` driver + a preset + a serializer; the prototype (`examples/llm_probe`) is the working reference.
- Verification: the probe is GPU-free, so the full proof is the headless nextest + the example running; no new golden/GPU surface.
