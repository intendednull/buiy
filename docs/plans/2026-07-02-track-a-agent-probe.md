# Track A — The Agent Probe / "Eyes" — Implementation Plan

> Use `subagent-driven-development`; gate + verify each wave. Realizes the north-star spec §4 Track A. Base: `origin/main` @ `9c4b881` (Track B merged).

**Goal:** Make the semantic-tree feedback loop a **first-class, agent-facing tool**: run a Buiy scene **headless + GPU-free**, and read a **stable, diffable snapshot** (roles / accessible names / state / layout rects / text content) the agent's build/test loop can observe — closing the author→build→run→inspect loop the prototype proved (principles P1/P5). Built on the *shipped* `a11y::inprocess` driver (snapshot/perform/click/get_by_role/…) — do not reinvent it.

**Architecture:** Two additions + one exposure. (1) `BuiyProbePlugin` — a headless **no-render** preset (composes Core/Theme/A11y/Focus/Layout/Text/Widgets, omits render/picking/winit) so a probe run needs no adapter (the prototype found `BuiyHeadlessPlugin` pulls `BuiyRenderPlugin` → wants a RenderApp; the probe wants GPU-free). (2) An agent-facing **snapshot serializer** — `a11y::inprocess::snapshot_report(world) -> String`: a Playwright-style indented semantic tree augmented with layout rect + text content (the SemanticTree omits plain `Text`; the prototype found agents need it). (3) Prelude/`buiy` exposure + a `buiy_probe` example (the productionized `examples/llm_probe`).

**Non-goals (deferred):** the `buiy_mcp` transport (Phase-2, separate); a GPU pixel-readback lane; live-drive orchestration beyond the existing `perform`/`click`/`wait_for`.

**Snapshot format (decided):** a **Playwright-style indented text tree** — token-efficient, human+agent readable, matches the a11y-snapshot agents already consume (research feedback-loop facet). One line per node: `<role> "<name>" [<state>] @<x,y wxh>`, children indented; a trailing `--- text ---` section lists non-a11y `Text` content + zero-size flags (catches "invisible content"). JSON is a trivial follow-on (serde on `SemanticTree`), not v1.

---

## Wave 1 — `BuiyProbePlugin` (headless, no-render preset)

**Files:** `crates/buiy/src/lib.rs` (next to `BuiyHeadlessPlugin`) or a `crates/buiy_core` module; `crates/buiy/tests/` (a headless probe test).

- [ ] **1.1** Failing test: build an app with `MinimalPlugins` + `InputPlugin` + `AssetPlugin` + `BuiyProbePlugin`, spawn `Button::new("Save")`, step frames, `snapshot(world)` → a Button node named "Save" (no GPU/window).
- [ ] **1.2** Define `BuiyProbePlugin` composing the sub-plugins the prototype hand-listed (Core/Theme/A11y/Focus/Layout/Text::default/Widgets), **omitting** `BuiyRenderPlugin`, picking, winit, scroll/animation (add-on). Doc the composition + the "no adapter needed; semantic tree + layout are pure ECS" rationale.
- [ ] **1.3** Run 1.1 → PASS. Commit.
- [ ] **GATE 1:** the preset composes exactly the GPU-free subset; no render/adapter dependency; a widget scene lays out + projects a11y under it.

## Wave 2 — Agent-facing snapshot serializer

**Files:** `crates/buiy_core/src/a11y/inprocess.rs` (or a sibling `report.rs`).

- [ ] **2.1** Failing test: `snapshot_report(world)` for a card(title "Settings" + Checkbox "Dark mode" checked + Button "Save") contains the checkbox line with `checked`, the button, AND the title text "Settings" (which the bare SemanticTree omits).
- [ ] **2.2** Implement `snapshot_report(world: &mut World) -> String`: walk `snapshot()`'s `SemanticTree` in document order, indent by depth, emit `role "name" [state]`; augment each node with its `ResolvedLayout` rect (via `entity_for_node_id`); append a `--- layout/text ---` section querying `(ResolvedLayout, Option<Text>)` for zero-size + plain-text visibility. Deterministic ordering (no HashMap iteration).
- [ ] **2.3** Run 2.1 → PASS. Commit. **GATE 2:** format is stable/diffable, deterministic, and surfaces the prototype's two gaps (plain text; zero-size invisible content).

## Wave 3 — Exposure + example

**Files:** `crates/buiy/src/lib.rs` (re-export `BuiyProbePlugin` + the `a11y::inprocess` driver surface through the prelude); `examples/buiy_probe/` (productionized `llm_probe`).

- [ ] **3.1** Re-export `BuiyProbePlugin` + `snapshot`/`snapshot_report`/`perform`/`click`/`get_by_role`/`wait_for` through `buiy` (+ prelude) so an agent reaches the loop from the front door.
- [ ] **3.2** `examples/buiy_probe`: a `scene()` slot + a `main` that runs it under `BuiyProbePlugin` and prints `snapshot_report` — the reference agent loop (author → `cargo run -p buiy_probe` → read the tree). Doc it in the example header.
- [ ] **3.3** Commit. **GATE 3:** the loop is reachable + documented from the prelude; the example runs GPU-free and prints a correct report.

## Wave 4 — Verify + docs + PR

- [ ] **4.1** Full gate: `cargo fmt --check`, `cargo clippy --workspace --all-targets -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps`, `cargo nextest run --workspace`. (No new GPU tests — the probe is GPU-free; the existing GPU lane must stay green.)
- [ ] **4.2** Flip spec §4 Track A note (loop/probe landed). Add an `AGENTS.md`-style pointer (or note it for Track D). Update `docs/README.md` if a doc is added.
- [ ] **4.3** PR `feat(probe): BuiyProbePlugin + agent-facing semantic-tree snapshot (Track A)` → `main`; rebase onto current `origin/main` first; green CI; merge on green (owner-authorized loop).

## Self-review
- Realizes spec §4 Track A (loop/eyes) for the headless-authoring case; `buiy_mcp` + GPU pixel lane explicitly deferred.
- Low risk: packages the *shipped, tested* `a11y::inprocess` driver + a preset + a serializer; the prototype (`examples/llm_probe`) is the working reference.
- Verification: the probe is GPU-free, so the full proof is the headless nextest + the example running; no new golden/GPU surface.
