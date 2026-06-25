# Verification real-input tier + content-presence guard (C7) Implementation Plan

> Part of the [co-drive](2026-06-22-widget-catalog-agent-interface-codrive.md); Wave 1 (RED-first, first-in-wave) — the authoritative sequencing / scope / shared-contract source. C7 **extends** the landed test infrastructure (the `.config/nextest.toml` runner + the two consolidated `buiy_verify` binaries `verify_headless` / `verify_gpu` + `support/mod.rs` GPU helpers), never a parallel rig (§7).

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** Build C7's Wave-1 verification deliverables — a headless synthetic-`PointerHits` `PointerHarness` in `buiy_verify` that drives a real non-origin widget tree through `layout → bridge → GlobalTransform → bevy_picking` and asserts state-flip + observer capture; the Tier-B `FontsGeneration`-bump content-survival test on the adapterless `TextExtractHarness`; and the `content_is_present` invariant + golden bless-guard — each proven RED-before-GREEN so the gate is not vacuous-green.

**Architecture:** `buiy_verify` gains a `pointer` module (`PointerHarness`) building a headless ECS app (`MinimalPlugins + TransformPlugin + CorePlugin + LayoutPlugin + bevy::picking::PickingPlugin + BuiyPickingBackendPlugin`, plus C3's `InteractionPlugin`/`FocusPlugin` when they exist) that spawns an offset tree, lets the production transform-propagation chain produce `GlobalTransform`, and injects a synthetic `PointerId` + `PointerLocation`/`PointerInput`. **C7 is the SOLE creator** of the shared Wave-1 test infrastructure — the `PointerHarness` (with the final API: `spawn_offset_tree(offset: Vec2, scene: impl Bundle) -> Entity`, `move_to`, `press/release/click(button: PointerButton)`, `top_hit`, `global_center`, `world_mut`, `captured`), the `bump_fonts_generation` helper on `extract_harness.rs`, and the Tier-B file `text_font_reload_survival.rs`. Every harness-based RED test ships as a committed `#[ignore = "RED until <child> lands: …"]`; C1 un-ignores the offset test, C2 un-ignores the three text tests — neither recreates anything C7 owns. The content-presence invariant `content_is_present(&mut App)` runs the production `extract_buiy_glyphs` producer (adapterless, via the same `MainWorld` swap `TextExtractHarness` uses) and asserts `glyph_count > 0` for text-bearing scenes, with the parallel refusal `golden::bless_guard_check(text_bearing, glyph_count)` wired at the `coverage_golden.rs` bless/assert call site; both share one `glyph_census` code path. The catalog-wide enroll auto-check is deferred (the `enroll_all` stack has no text plugins; the Wave-1 catalog is non-text).

**Tech Stack:** Rust, Bevy 0.19.0-rc.3 (`bevy::picking`, `bevy::transform`, ECS observers), `buiy_core` (picking/backend, text/extract, text/edit, a11y), `buiy_verify` (coverage/enroll, invariant, golden, a11y), `serde`/`serde_json`, `insta` goldens, `cosmic-text`.

**Wave / dependencies:** **Wave 1.** Depends on **C0** (umbrella, landed) and is co-sequenced with **C1** (coordinate fix) and **C2** (text fix). C7 **lands RED-first**: its harness + predicates must exist *before* C1/C3 so they are the regression gate *for* those changes. The single load-bearing RED proof is **Tier A on an offset widget fails on current main (pre-C1)** — picking reads `ResolvedLayout.position` (parent-local) as absolute, so the synthetic pointer over the visually-correct position picks the wrong entity / no entity.

> **Co-drive §4 Wave-1 split (P1a coupling).** Per [co-drive §4 Wave 1](2026-06-22-widget-catalog-agent-interface-codrive.md): C7's **picking-geometry tier (Tier A) is fully P1a-INDEPENDENT** — it is the C1 gate and shares no edited code with the agent-interface a11y substrate, so it proceeds immediately regardless of P1a. C7's **a11y-state assertions LAG P1a** — they read the agent-interface `semantic_tree(app, view)` tier / decomposed state components and are deferred to Wave 2 (Task 4 behavior asserts) until P1a lands. The Tier-B content-survival + content-presence invariant (Tasks 1, 4) are likewise P1a-independent (pure text/render extract). This plan therefore builds only the P1a-independent Wave-1 pieces now; nothing here blocks on the agent-interface merge.

**The a11y `WireNode` tri-state + role extension is NOT in this plan — it is the agent-interface campaign's (umbrella §2.7).** Its P0 fixes the `snapshot_tree` ref off-by-one + extends `A11yRole` and both stringifiers, and its P1a widens `A11yNodeView`/`build_tree`/`to_accesskit_node` and exposes the decomposed state through `buiy_verify::a11y::semantic_tree(app, view)`. This plan **does not touch** `crates/buiy_verify/src/a11y.rs`'s `WireNode`/`role_to_str`/`KNOWN_ROLES`; any a11y assertion C7 needs is read through the agent-interface semantic-tree tier (consumed, not built). This plan builds only the Wave-1 **geometry/render-content** pieces (Tier-A picking-geometry harness skeleton + offset RED proof, Tier-B content-survival, content-presence invariant + bless-guard) and **builds on the incoming test infrastructure** (cargo-nextest + the 162→7 consolidated harnesses, PR #77, re-confirmed at Phase 0).

**Coordination with the agent-interface campaign + the incoming test infrastructure (umbrella §2.7/§8, spec §2.0 + Coordination section):**
- **Consumes, does not build:** the agent-interface a11y wire format + `semantic_tree(app, view)` snapshot tier (any a11y assertion), the decomposed state components `A11yToggled`/`A11ySelected`/`A11yExpanded` (state-flip reads), the `A11yRole` enum, the action router + in-process driver (`perform`/`click`, activation-parity), and the a11y gates #3/#4/#6/#7/#12. Also consumes the incoming cargo-nextest runner + the 162→7 consolidated harnesses (PR #77) — re-confirmed, never forked (Phase 0).
- **Builds here (owns):** the Tier-A `PointerHarness` (picking/coordinate-geometry — proves the Bug-1 picking divergence the semantic driver does not exercise), Tier-B FontsGeneration content-survival, the content-presence invariant + bless-guard (render-content), the Tier-C winit smoke, the catalog fixture-enrollment + `Matrix::gallery_screen()`. C1+C3 also deliver the stacking-aware `hit_test` the agent-interface actionability gate (`HitTargetable`, its follow-up #3) depends on.
- **Dropped vs the earlier draft:** the a11y `WireNode` tri-state + role-serialization task (now agent-interface P0/P1a); any C7-owned `A11yTreeBuilder`/`a11y_tree()` read; the Mixed-serializes-`"mixed"` mutation/control RED proof (now agent-interface gate #3/#4); and the Buiy-native `Activate` parity (now router/`OnPress` parity).

---

## Phase 0 — Rebase + re-confirm anchors (FIRST TASK, mandatory)

This worktree's code blocks were written against `507855f` and MUST be re-confirmed against the current base. The integration branch is already rebased onto `origin/main` @ `e54cf0c` (PR #77 testing-audit + #78 CI-hardening + #79 a11y P0 merged); per the co-drive §8 reread the non-widget app-correctness work (C1/C2/C7) is no longer gated on the agent-interface merge — C7 is RED-first, first-in-wave.

> **MAJOR DRIFT — C7 EXTENDS the landed consolidated test infrastructure (PR #77), it does NOT build a parallel rig (co-drive §7).** The testing audit already landed (162→7): the `.config/nextest.toml` config is **present**; the `buiy_verify` integration tests are now **two consolidated group binaries** — `crates/buiy_verify/tests/verify_headless.rs` (root) + `tests/verify_headless/<module>.rs`, and `crates/buiy_verify/tests/verify_gpu.rs` (root) + `tests/verify_gpu/<module>.rs` (the `#[ignore]`/GPU split); the `buiy_core` GPU helpers live in `crates/buiy_core/tests/support/mod.rs` + `extract_harness.rs`. Consequences for the files this plan **creates**:
> - The NEW test files are **modules**, not standalone binaries. `pointer_offset_regression.rs`, `pointer_press_smoke.rs`, and `content_presence.rs` go under `crates/buiy_verify/tests/verify_headless/` and **must each be registered** with a `#[path = "verify_headless/<name>.rs"] mod <name>;` line in `tests/verify_headless.rs` (a bare `mod` in a binary root resolves to `tests/<name>.rs`, not the subdir — the `#[path]` is required). Modules reference the lib via `buiy_verify::pointer` / `buiy_verify::invariant` external-crate paths (no `mod support;`).
> - The bless-guard wiring site (Task 1) is `crates/buiy_verify/tests/verify_gpu/coverage_golden.rs` (a `verify_gpu` module — it carries `#[ignore]` GPU goldens), reached via `--test verify_gpu`.
> - The C7-OWNED Tier-B file `text_font_reload_survival.rs` is a **`buiy_core` text-edit module**: place it at `crates/buiy_core/tests/text_edit/text_font_reload_survival.rs` (editor-surface tests live in the `text_edit` group binary; `TextEditState`/IME live there) and register `#[path = "text_edit/text_font_reload_survival.rs"] mod text_font_reload_survival;` in `crates/buiy_core/tests/text_edit.rs`. Reach the shared harness via `mod support;` at the binary root (already present) → `crate::support` inside the module — NOT a fresh `mod support;` in the module.
> - `crates/buiy_verify/src/pointer.rs` (the harness, src) and `crates/buiy_verify/src/invariant/content_presence.rs` are plain library modules — registered in `src/lib.rs` / `src/invariant.rs` as the plan says (no binary-root change). `src/pointer.rs` is **absent today** — C7 creates it.
> - **Run/filter:** `cargo nextest run -p buiy_verify --test verify_headless` (optionally filtered by module, e.g. `pointer_offset_regression::`); `--test verify_gpu` for the GPU leg; `cargo nextest run -p buiy_core --test text_edit text_font_reload_survival::` for the Tier-B file — never `--test pointer_offset_regression` / `--test text_font_reload_survival`.

### Files
- (no source edits) — branch + verification only.

### Steps
- [ ] **Confirm the current base (NOT 507855f).** Run:
  ```sh
  git -C /mnt/storage/projects/buiy fetch --all --prune
  git -C /mnt/storage/projects/buiy log --oneline -1 origin/main   # expect e54cf0c, NOT 507855f
  ```
  The integration branch is already rebased on `e54cf0c` (one commit above: `e1ff8c7`). Work on it directly. Confirm #77 testing-audit / #78 CI-hardening / #79 a11y P0 are present (`git -C /mnt/storage/projects/buiy log --oneline -15`). The testing audit consolidated test binaries and added `.config/nextest.toml`; `cargo nextest run` is the inner runner. **Confirm the consolidated layout** before creating any test file: `ls crates/buiy_verify/tests/` (expect `verify_headless.rs` + `verify_headless/`, `verify_gpu.rs` + `verify_gpu/`), `ls crates/buiy_core/tests/` (expect `text_edit.rs` + `text_edit/`, `support/`), and `ls .config/nextest.toml`.
- [ ] **Re-confirm the incoming test-infra + agent-interface substrate surface (build on it, do not fork it — umbrella §2.7/§8, spec §2.0).** C7 *extends* the then-current runner/harness/gate surface and *consumes* the agent-interface a11y tier; confirm both are present at their current API before depending on them:
  ```sh
  cd /mnt/storage/projects/buiy
  cargo nextest --version                                          # PR #77 consolidated runner present
  rg -n "fn semantic_tree|fn snapshot\b|fn perform\b|accesskit_consumer" crates/buiy_core/src/a11y/inprocess.rs crates/buiy_verify/src/a11y.rs  # agent-interface tier (consumed for a11y asserts)
  rg -n "A11yToggled|A11ySelected|A11yExpanded" crates/buiy_core/src/a11y/states.rs   # decomposed state components (read for state-flip)
  rg -n "fn route_action_requests|dispatch_action_request" crates/buiy_core/src/a11y  # the action router (activation-parity)
  ```
  If the agent-interface P1a/P1c surface is not yet merged, the Wave-2 behavior asserts (Task 4) that read the semantic-tree tier / consume the state components are deferred until it lands (per the implementation gate, umbrella §8); the Wave-1 geometry/render-content tasks below do NOT depend on it and proceed.
- [ ] **Re-grep every file:line anchor this plan cites and fix drift.** Run each and confirm the cited symbol is on (or near) the cited line; if it moved, update the task's anchor before writing code. **Note: C7 does NOT touch `a11y.rs`'s `WireNode`/`role_to_str`/`KNOWN_ROLES` (agent-interface owns them, §2.5) — no anchor for them here:**
  ```sh
  cd /mnt/storage/projects/buiy
  rg -n "fn emit_picks|point_in_aabb|Entity::PLACEHOLDER|hits.sort_by" crates/buiy_core/src/picking/backend.rs
  rg -n "pub fn point_in_aabb|fn hit_test|fn update_hovered|pub struct Hovered" crates/buiy_core/src/picking/mod.rs
  rg -n "pub fn glyph_count|pub fn settle|pub fn frame|init_resource::<MainWorld>|extract_buiy_glyphs" crates/buiy_core/tests/support/extract_harness.rs
  rg -n "pub fn apply\b|pub fn value\b|pub fn splice_preedit|pub fn for_font_size" crates/buiy_core/src/text/edit/input.rs crates/buiy_core/src/text/edit/state.rs crates/buiy_core/src/text/edit/ime.rs
  rg -n "struct FontsGeneration|fn apply_font_registry" crates/buiy_core/src/text/font_system.rs crates/buiy_core/src/text/registry.rs
  rg -n "fn enroll_all|fn enroll_fixtures|fn build_app" crates/buiy_verify/src/coverage/enroll.rs
  rg -n "fn assert_golden\b|fn check_golden_in|enum BlessMode|fn bless\b" crates/buiy_verify/src/golden/check.rs
  rg -n "base = Mat4::from_translation" crates/buiy_core/src/render/bridge.rs   # the §6.2 invariant to PRESERVE (expect ~line 138)
  rg -n "window-relative" crates/buiy_core/src/components.rs                     # C1's doc-lie target (expect ~line 65)
  ```
- [ ] **Confirm C3 is NOT yet present** (so Tier-A behavior asserts are written against the not-yet-built model and land RED): `rg -rn "InteractionPlugin|Pointer<" crates/buiy_core/src crates/buiy_widgets/src` must return nothing for `InteractionPlugin`/`Pointer<E>`. If C3 has already landed, the §3.2 build-step confirm (Task 3) becomes a direct check rather than a stub.
- [ ] **Baseline the existing gate green.** Run the headless gate and confirm it passes before any change:
  ```sh
  cd /mnt/storage/projects/buiy
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  xvfb-run -a cargo nextest run --workspace
  ```
  Expected: all green. Record the `buiy_verify` + `buiy_core` test counts so a later regression is visible.
- [ ] **Commit the branch point (no code yet):** nothing to commit; proceed to Task 1.

---

## Task 1 — `content_is_present` invariant + bless-guard (RED-first against a zero-glyph fixture)

The content-presence predicate (spec §2.4) runs the production `extract_buiy_glyphs` producer adapterless and asserts a text-bearing fixture emits `> 0` glyph instances; the bless-guard refuses to record a zero-glyph baseline. Text-bearing is **inferred** from `Text`/`TextEditState` presence (§3.4). This task is independent of C1/C2/C3 and lands fully GREEN within Wave 1.

**Stack constraint (verified, load-bearing):** the predicate runs the production glyph producer, which requires `SharedFontSystem` + the text/render resources. The coverage `enroll_all`/`build_app` stack (`enroll.rs:45-86`) is `MinimalPlugins + CorePlugin + LayoutPlugin` only — it has **no** `BuiyTextPlugin`/`BuiyRenderPlugin`, so `SharedFontSystem` is absent there. Therefore the predicate's caller MUST supply a text-capable app (the `content_test_app` below mirrors `TextExtractHarness::with_atlas_config`, which DOES add `BuiyTextPlugin` + `BuiyRenderPlugin` — `extract_harness.rs:66-100`). The `enroll_all`-driven catalog auto-check is **deferred** to Wave 3+ (it needs a text-capable enroll stack + a text fixture; the Wave-1 catalog is the non-text button only — `fixture.rs` registers `button.resting`). This task therefore ships the predicate + its two dedicated full-stack unit tests; it does NOT add a broken `enroll_all` auto-check that would panic on the missing `SharedFontSystem`.

### Files
- Create `crates/buiy_verify/src/invariant/content_presence.rs`
- Modify `crates/buiy_verify/src/invariant.rs` (re-export the predicate)
- Modify `crates/buiy_verify/src/golden/check.rs` (bless-guard helper + wiring note)
- Create `crates/buiy_verify/tests/verify_headless/content_presence.rs` (the predicate's RED-proof + a real text fixture) — and **register it** with `#[path = "verify_headless/content_presence.rs"] mod content_presence;` in `crates/buiy_verify/tests/verify_headless.rs` (Phase-0 consolidation rule; a bare `mod` in a binary root would resolve to `tests/content_presence.rs`).

### Steps

- [ ] **Write the failing test first.** The predicate does not exist yet; assert it both passes for a real text fixture and fails for a zero-glyph one. The full-stack `content_test_app` mirrors `TextExtractHarness::with_atlas_config` (`extract_harness.rs:66-100`) so `SharedFontSystem` + the producer's resources exist. The zero-glyph input is **whitespace-only** `Text("   ")`, which is VERIFIED to emit 0 glyphs through the production producer by `crates/buiy_core/tests/text/text_extract.rs::whitespace_only_entity_emits_no_run` (in the `text` group binary) (`assert_eq!(h.glyph_count(), 0)` at text_extract.rs:967) — not the unverified U+200B. `Text("Hi!")` → 3 glyphs is verified by the same file's `emits_one_instance_per_visible_glyph_with_resident_keys` (text_extract.rs:86). Create `crates/buiy_verify/tests/verify_headless/content_presence.rs`:
  ```rust
  //! The content-presence predicate's RED proof (C7 §2.4, §6). A text-bearing
  //! fixture MUST emit > 0 glyph instances on the production extract path; a
  //! fixture whose text silently fails to shape is the silent-no-paint failure
  //! (Bug 2 release mode) and MUST be caught here. The predicate runs the
  //! production `extract_buiy_glyphs` adapterless, so the test app carries the
  //! full text+render MAIN-world stack (mirror of TextExtractHarness).

  use bevy::prelude::*;
  use bevy::window::{PrimaryWindow, Window, WindowResolution};
  use buiy_core::Node;
  use buiy_core::layout::Style;
  use buiy_core::text::{FontSize, Text};
  use buiy_verify::invariant::content_is_present;

  /// A real text-bearing scene: a sized column root + a "Hi!" label. The
  /// production producer shapes "Hi!" to 3 glyph instances (text_extract.rs:86).
  fn spawn_label(app: &mut App) {
      let root = app
          .world_mut()
          .spawn((
              Node,
              Style::default().flex_column().width_px(300.0).height_px(100.0),
          ))
          .id();
      let label = app
          .world_mut()
          .spawn((Node, Style::default(), Text(String::from("Hi!")), FontSize(16.0)))
          .id();
      app.world_mut().entity_mut(root).add_child(label);
  }

  #[test]
  fn content_present_passes_for_a_shaping_label() {
      let mut app = content_test_app();
      spawn_label(&mut app);
      app.update(); // TextSync -> measure -> commit, so the buffer is shaped
      assert!(
          content_is_present(&mut app).is_ok(),
          "a label that shapes to 3 glyphs must satisfy content_is_present"
      );
  }

  #[test]
  fn content_present_fails_for_a_zero_glyph_text_fixture() {
      // A text-bearing entity whose content is whitespace-only: the producer
      // emits ZERO glyph instances (text_extract.rs:967, verified). This is the
      // structural stand-in for the silent-no-paint bug — the predicate catches it.
      let mut app = content_test_app();
      let root = app
          .world_mut()
          .spawn((
              Node,
              Style::default().flex_column().width_px(300.0).height_px(100.0),
          ))
          .id();
      let label = app
          .world_mut()
          .spawn((
              Node,
              Style::default(),
              Text(String::from("   ")), // whitespace: text-bearing, zero visible glyphs
              FontSize(16.0),
          ))
          .id();
      app.world_mut().entity_mut(root).add_child(label);
      app.update();

      let result = content_is_present(&mut app);
      assert!(
          result.is_err(),
          "a text-bearing fixture that emits 0 glyphs must violate content_is_present"
      );
      assert_eq!(result.unwrap_err().rule, "content_is_present");
  }

  /// The full text+render MAIN-world stack the predicate runs the producer
  /// against — the exact plugin set TextExtractHarness builds
  /// (extract_harness.rs:71-85): ThemePlugin + CorePlugin + LayoutPlugin +
  /// BuiyTextPlugin + BuiyRenderPlugin (its render half is a no-op without a
  /// RenderApp), plus a component-only synthetic PrimaryWindow. No wgpu adapter.
  fn content_test_app() -> App {
      let mut app = App::new();
      app.add_plugins(MinimalPlugins)
          .add_plugins(buiy_core::theme::ThemePlugin)
          .add_plugins(buiy_core::CorePlugin)
          .add_plugins(buiy_core::layout::LayoutPlugin)
          .add_plugins(buiy_core::text::BuiyTextPlugin::default())
          .add_plugins(buiy_core::render::BuiyRenderPlugin);
      app.world_mut().spawn((
          Window {
              resolution: WindowResolution::new(640, 480),
              ..Default::default()
          },
          PrimaryWindow,
      ));
      app
  }
  ```
  `ThemePlugin` is REAL (`crates/buiy_core/src/theme.rs:142`, re-exported via `pub mod theme`) and is part of the canonical adapterless stack (`extract_harness.rs:72`) — keep it. `BuiyTextPlugin` seeds `SharedFontSystem`, which the predicate reads.
- [ ] **Run it & show the expected FAIL.** The predicate is not yet defined:
  ```sh
  cd /mnt/storage/projects/buiy
  cargo test -p buiy_verify --test verify_headless content_presence:: 2>&1 | head -20
  ```
  Expected: a **compile error** — `cannot find function `content_is_present` in module `invariant``. (A compile failure IS the RED state for a not-yet-existing symbol.)
- [ ] **Write the minimal implementation.** Create `crates/buiy_verify/src/invariant/content_presence.rs`. The predicate takes `&mut App` (the caller owns the app), builds the bare extract `World` the producer touches — the exact resources `TextExtractHarness::with_atlas_config` seeds (`extract_harness.rs:88-100`) — swaps the caller's live (already-`update()`d) world into `MainWorld`, runs the `(maintain_atlas, extract_buiy_glyphs)` `ExtractSchedule`, swaps it back, and reads `ExtractedGlyphs::glyphs.len()`. No `unsafe`, no wgpu device. All import paths are the ones `extract_harness.rs:14-23` uses verbatim:
  ```rust
  //! `content_is_present` (C7 §2.4): the production extract path MUST emit
  //! > 0 glyph instances for a text-bearing scene. Text-bearing is inferred
  //! from `Text` / `TextEditState` presence (§3.4); the placeholder edge is
  //! handled by reading `PlaceholderActive` (an active placeholder is positive,
  //! an inactive empty editor is legal at 0 glyphs).
  //!
  //! Runs the production `extract_buiy_glyphs` adapterless via the same
  //! main-world↔MainWorld swap `TextExtractHarness` uses (extract_harness.rs).
  //! The CALLER owns the `App` and must build it on a text-capable stack
  //! (BuiyTextPlugin seeds `SharedFontSystem`) and `update()` it once before
  //! calling, so TextSync → measure → commit have shaped the buffers.

  use bevy::prelude::*;
  use bevy::render::{ExtractSchedule, MainWorld};

  use buiy_core::render::atlas::{AtlasConfig, BuiyAtlas, maintain_atlas};
  use buiy_core::render::extract::ExtractedTextQuads;
  use buiy_core::render::prepare::ExtractedGlyphs;
  use buiy_core::text::edit::{PlaceholderActive, TextEditState};
  use buiy_core::text::{
      BuiySwashCache, FontKeyInterner, GlyphMetaCache, ResidentTextKeys, SharedFontSystem, Text,
      extract_buiy_glyphs,
  };

  use super::predicates::Violation;

  /// Is `app`'s spawned scene text-bearing for the content-presence guard?
  /// A `Text` component, or an editor with an ACTIVE placeholder, or a
  /// non-empty editor value, counts; an empty editor with no active placeholder
  /// does not (the legal 0-glyph case, §3.4).
  fn scene_is_text_bearing(world: &mut World) -> bool {
      if let Some(mut q) = world.try_query::<&Text>()
          && q.iter(world).next().is_some()
      {
          return true;
      }
      if let Some(mut q) = world.try_query::<(&TextEditState, Option<&PlaceholderActive>)>() {
          for (state, active) in q.iter(world) {
              if active.is_some() || !state.value().is_empty() {
                  return true;
              }
          }
      }
      false
  }

  /// Run the production glyph producer over `app`'s live (already-updated)
  /// scene, adapterless, and return `(text_bearing, glyph_count)`. The honest,
  /// non-`unsafe` form: the caller owns the `App`, so we borrow its world via
  /// `world_mut()` and swap it through `MainWorld`. The bless-guard
  /// (`golden::bless_guard_check`) reuses this exact pair, so the invariant and
  /// the bless refusal never diverge.
  pub fn glyph_census(app: &mut App) -> (bool, usize) {
      // The shared font engine the producer shapes against (seeded by
      // BuiyTextPlugin in the caller's stack). Clone the Arc handle.
      let fonts = app.world().resource::<SharedFontSystem>().clone();

      // The bare extract world the producer touches — exactly the resources
      // `TextExtractHarness::with_atlas_config` seeds (extract_harness.rs:88-100).
      let mut render = World::new();
      render.insert_resource(BuiyAtlas::new(AtlasConfig::default()));
      render.init_resource::<ExtractedGlyphs>();
      render.init_resource::<ExtractedTextQuads>();
      render.init_resource::<FontKeyInterner>();
      render.init_resource::<ResidentTextKeys>();
      render.init_resource::<GlyphMetaCache>();
      render.init_resource::<BuiySwashCache>();
      render.insert_resource(fonts);
      render.init_resource::<MainWorld>();

      let mut schedule = Schedule::new(ExtractSchedule);
      schedule.add_systems((maintain_atlas, extract_buiy_glyphs).chain());

      let text_bearing = scene_is_text_bearing(app.world_mut());

      // The bevy_render extract dance (extract_harness.rs:130-140): swap the
      // live main world into MainWorld, run the schedule, swap it back.
      {
          let mut main = render.resource_mut::<MainWorld>();
          core::mem::swap(&mut **main, app.world_mut());
      }
      schedule.run(&mut render);
      {
          let mut main = render.resource_mut::<MainWorld>();
          core::mem::swap(&mut **main, app.world_mut());
      }

      (text_bearing, render.resource::<ExtractedGlyphs>().glyphs.len())
  }

  /// Assert > 0 glyph instances for a text-bearing scene (the silent-no-paint
  /// guard). A non-text scene, or a text scene that shaped some glyphs, is ok.
  pub fn content_is_present(app: &mut App) -> Result<(), Violation> {
      let (text_bearing, glyphs) = glyph_census(app);
      if text_bearing && glyphs == 0 {
          return Err(Violation::new(
              "content_is_present",
              "a text-bearing scene emitted 0 glyph instances (silent-no-paint)",
          ));
      }
      Ok(())
  }
  ```
  `Violation::new` is `pub(crate)` (`predicates.rs:39`), so a sibling module under `invariant/` constructs it directly — no API change needed. The `ExtractedGlyphs.glyphs` field is `pub` (`prepare.rs:52`); `.glyphs.len()` is the same read `TextExtractHarness::glyph_count` does (`extract_harness.rs:154-156`). **Re-confirm at Phase 0** only the line anchors (`extract_harness.rs:88-100,130-140`, `prepare.rs:52`), in case the inspection-tools merge shifted them; the symbols and signatures above are verified against the current tree.
- [ ] **Re-export from `invariant.rs`.** Edit `crates/buiy_verify/src/invariant.rs` (it currently re-exports `predicates`, `scene`, `bidi` — add the new module after them):
  ```rust
  pub mod content_presence;
  pub use content_presence::{content_is_present, glyph_census};
  ```
- [ ] **Run & show expected PASS** for the predicate tests:
  ```sh
  cd /mnt/storage/projects/buiy
  cargo test -p buiy_verify --test verify_headless content_presence:: 2>&1 | tail -15
  ```
  Expected: `content_present_passes_for_a_shaping_label` PASS (the "Hi!" label shapes to 3 glyphs) and `content_present_fails_for_a_zero_glyph_text_fixture` PASS (the whitespace `"   "` label emits 0 glyphs, so the predicate returns `Err` with `rule == "content_is_present"`).
- [ ] **Add the bless-guard (RED-first via a unit test).** In `crates/buiy_verify/src/golden/check.rs`, the spec requires the bless path to refuse a zero-glyph text-bearing cell with **no key-schema change** (§2.4, §6.7). The golden bless path operates on an `RgbaImage`, not a glyph count (`bless(dir, ledger_path, replace, key, actual, budget)` at check.rs:209 takes an `&RgbaImage`; the bless decision is resolved inside `check_golden_in` at check.rs:145, `if let BlessMode::Bless { replace } = mode { return bless(...) }`). So the guard takes an explicit "text-bearing & glyph_count" pair the **driver** computes and calls *before* it hands the capture to `assert_golden`. First write the failing unit test in `check.rs`'s `#[cfg(test)] mod tests` (which already exists at check.rs:455 — append to it):
  ```rust
  #[test]
  fn bless_guard_refuses_zero_glyph_text_bearing() {
      // A text-bearing cell with glyph_count == 0 must be refused, loudly.
      let r = bless_guard_check(/* text_bearing */ true, /* glyph_count */ 0);
      assert!(r.is_err(), "a zero-glyph text-bearing cell must not be blessable");
      // A non-text fixture, or a text fixture with glyphs, blesses fine.
      assert!(bless_guard_check(false, 0).is_ok());
      assert!(bless_guard_check(true, 3).is_ok());
  }
  ```
  Run `cargo test -p buiy_verify --lib golden::check::tests::bless_guard_refuses_zero_glyph_text_bearing` → FAIL (`cannot find function bless_guard_check`).
- [ ] **Implement `bless_guard_check`** in `check.rs`:
  ```rust
  /// The bless-guard (C7 §2.4): a fixture declared text-bearing CANNOT be
  /// blessed when it emitted zero glyph instances — that is the silent-no-paint
  /// hole at the corpus boundary. Returns `Err` with a loud message so the
  /// bless refuses rather than recording a blank baseline. No key-schema change.
  pub fn bless_guard_check(text_bearing: bool, glyph_count: usize) -> Result<(), String> {
      if text_bearing && glyph_count == 0 {
          return Err(
              "refusing to bless a text-bearing cell with 0 glyph instances \
               (silent-no-paint — fix the fixture's text shaping before blessing)"
                  .into(),
          );
      }
      Ok(())
  }
  ```
  Wire it at the real bless/assert call site: `crates/buiy_verify/tests/verify_gpu/coverage_golden.rs`'s `matrix_goldens` driver calls `assert_golden(&key, &img, &budget_for(&cov))` at **coverage_golden.rs:146**, once per `(fixture, cell)`. That GPU app is built via `DeterministicApp` (coverage_golden.rs:136), which carries the full text+render stack — so the SAME `(text_bearing, glyph_count)` the content-presence extract computes is available there. Insert the guard immediately before the `assert_golden` call:
  ```rust
  // crates/buiy_verify/tests/verify_gpu/coverage_golden.rs, inside the cell loop, just
  // before `assert_golden(&key, &img, &budget_for(&cov));` (coverage_golden.rs:146):
  let (text_bearing, glyph_count) = buiy_verify::invariant::glyph_census(&mut app);
  buiy_verify::golden::bless_guard_check(text_bearing, glyph_count)
      .unwrap_or_else(|e| panic!("bless-guard refused cell {}: {e}", key.slug()));
  let img = buiy_core::render::golden::capture_to_image(&mut app, &cfg);
  assert_golden(&key, &img, &budget_for(&cov));
  ```
  This reuses the `glyph_census(app: &mut App) -> (bool, usize)` already defined alongside `content_is_present` (the implementation step above factors the extract into `glyph_census` so the invariant and the bless refusal share one code path). Re-export `bless_guard_check` from `golden.rs` (add `bless_guard_check` to the `pub use check::{…}` list at golden.rs:39-42).

  In Wave 1 the only catalog fixture is the non-text `button` (`fixture.rs`), so at runtime the guard is invoked as `bless_guard_check(false, 0)` — the no-op (`Ok`) path — and `matrix_goldens` stays `#[ignore]`d GPU-only regardless. The guard's TEETH are proven NOW by the `bless_guard_check` unit test below; the wiring makes it load-bearing the moment C8 adds a text fixture, with no further edit.
- [ ] **Run & show expected PASS:**
  ```sh
  cd /mnt/storage/projects/buiy
  cargo test -p buiy_verify --lib golden::check 2>&1 | tail -8
  ```
  Expected: `bless_guard_refuses_zero_glyph_text_bearing` PASS plus the existing `golden::check::tests` still green.
- [ ] **Do NOT add an `enroll_all` catalog auto-check in Wave 1 — it would panic.** The `enroll_all`/`build_app` stack (`enroll.rs:45-86`) is `MinimalPlugins + CorePlugin + LayoutPlugin` only; it has no `BuiyTextPlugin`, so `app.world().resource::<SharedFontSystem>()` inside `glyph_census` would panic on every enrolled cell. The Wave-1 catalog is the non-text `button` anyway (`fixture.rs`), so there is nothing text-bearing to assert. The predicate's teeth are proven by the two dedicated full-stack unit tests above (the whitespace zero-glyph RED + the "Hi!" GREEN). **Record the deferral** as a one-line comment at the bottom of `content_presence.rs`'s test file:
  ```rust
  // DEFERRED (Wave 3+, with C8): a catalog-wide `enroll_all` auto-check that
  // every text-bearing cell satisfies `content_is_present`. It is blocked on
  // (a) a text fixture in the catalog and (b) a text-capable enroll stack —
  // `build_app` (enroll.rs) has no BuiyTextPlugin, so `glyph_census` would
  // panic on the missing SharedFontSystem. Until then the predicate is gated by
  // the two unit tests in this file; see follow-ups.md.
  ```
  Add the matching follow-up entry to `docs/plans/follow-ups.md` (the text-fixture / text-capable-enroll-stack item) so the deferral is tracked, not lost.
- [ ] **Commit.**
  ```sh
  cd /mnt/storage/projects/buiy
  git add crates/buiy_verify/src/invariant.rs crates/buiy_verify/src/invariant/content_presence.rs \
          crates/buiy_verify/src/golden/check.rs crates/buiy_verify/src/golden.rs \
          crates/buiy_verify/tests/verify_headless/content_presence.rs docs/plans/follow-ups.md
  git commit -m "test(verify): add content_is_present invariant + golden bless-guard (C7 Tier-3)

The production extract path must emit >0 glyph instances for a text-bearing
fixture; a zero-glyph text-bearing cell is the silent-no-paint failure (Bug 2
release mode) and cannot be blessed. Proven RED-first by a whitespace-only
zero-glyph fixture (verified 0-glyph through the production producer per
text_extract.rs). Text-bearing inferred from Text/TextEditState presence
(C7 spec §2.4). The catalog-wide enroll auto-check is deferred to Wave 3+
(needs a text fixture + a text-capable enroll stack; see follow-ups.md).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Task 2 — Tier-A `PointerHarness` skeleton + the load-bearing offset RED proof

The single most important proof in the campaign (umbrella §9.5, spec §6): the existing `picking_backend.rs:27-45` `emit_picks` hit-tests `point_in_aabb(cursor, &ResolvedLayout)` (mod.rs:51, reading `layout.position`) and the integration test hand-writes a single-node `ResolvedLayout` at an absolute position — so it is **structurally blind** to Bug 1 and must not be trusted as the gate. `PointerHarness` spawns a **non-origin** tree, lets the production `layout → write_buiy_transform → propagate_parent_transforms → GlobalTransform` chain run (`bridge.rs`; `CorePlugin` chains the three propagation systems in `Update` `.before(BuiySet::Picking)`, lib.rs:108-129), then injects a synthetic pointer through the sanctioned bevy_picking path. On **current main (pre-C1)** `emit_picks` reads `ResolvedLayout.position` (parent-local — Taffy `location`, `write_resolved_layout` @ systems.rs:2734) as absolute — so a pointer over the visually-correct absolute position picks the wrong entity / no entity. That divergence is the RED proof.

**Ownership boundary (this task):** C7 OWNS only the `PointerHarness` + the offset-picking RED test (`pointer_offset_regression.rs`, committed `#[ignore]`). The **clip-ordering** and **no-fallback (camera-ref)** RED tests are **C1-owned `buiy_core` tests** — C7 does NOT create them. C1 lands those in its own plan and un-ignores the offset test here.

### Files
- Create `crates/buiy_verify/src/pointer.rs`
- Modify `crates/buiy_verify/src/lib.rs` (`pub mod pointer;`)
- Create `crates/buiy_verify/tests/verify_headless/pointer_offset_regression.rs` — and **register it** with `#[path = "verify_headless/pointer_offset_regression.rs"] mod pointer_offset_regression;` in `crates/buiy_verify/tests/verify_headless.rs`.
- Modify `crates/buiy_core/tests/crosscut/picking_backend.rs` (doc note: superseded-for-integration by Tier A — it is a module of the `crosscut` group binary)

### Steps

- [ ] **Write the failing test first — the offset regression.** This test is designed to be RED on current main (the harness drives the *real* layout→bridge→GlobalTransform chain; the pre-C1 backend mis-reads coordinates). Create `crates/buiy_verify/tests/verify_headless/pointer_offset_regression.rs`:
  ```rust
  //! The load-bearing RED proof (C7 §6): a synthetic pointer over the
  //! VISUALLY-CORRECT absolute position of an offset widget must hit that
  //! widget. On current main (pre-C1) picking reads parent-local
  //! `ResolvedLayout.position` as absolute, so the hit lands on the wrong
  //! entity or no entity — this test is RED until C1 routes picking through
  //! `GlobalTransform`. It is what makes Tier A a real gate, not a
  //! green-by-construction rubber stamp (the existing picking_backend.rs
  //! hand-writes ResolvedLayout and is structurally blind to this bug).

  use bevy::prelude::*;
  use buiy_core::Node;
  use buiy_core::layout::Style;
  use buiy_verify::pointer::PointerHarness;

  #[test]
  fn synthetic_pointer_hits_offset_widget_at_its_global_position() {
      let mut h = PointerHarness::new();
      // The harness wraps `scene` under a root positioned at the EXPLICIT
      // `offset`, so the target sits at a NON-ORIGIN absolute position: its
      // ResolvedLayout.position is parent-local (small), its
      // GlobalTransform.translation is the accumulated absolute. Bug 1 only
      // diverges when these differ — `offset` forces that divergence.
      let target = h.spawn_offset_tree(
          Vec2::new(80.0, 60.0),
          (
              Node,
              Style::default().width_px(100.0).height_px(50.0),
              Name::new("target"),
          ),
      );

      // Read the target's absolute (global) center the layout chain produced,
      // and aim the synthetic pointer at it in window space.
      let center = h.global_center(target);
      h.move_to(center);

      let hit = h.top_hit();
      assert_eq!(
          hit,
          Some(target),
          "the synthetic pointer over the target's GLOBAL center must hit the \
           target; on pre-C1 main the backend mis-reads parent-local position \
           as absolute and this FAILS (the C1 regression gate)"
      );
  }
  ```
- [ ] **Run it & show the expected FAIL.** `PointerHarness` does not exist yet:
  ```sh
  cd /mnt/storage/projects/buiy
  cargo test -p buiy_verify --test verify_headless pointer_offset_regression:: 2>&1 | head -15
  ```
  Expected: compile error — `unresolved import `buiy_verify::pointer``. (After the harness is built, this test stays RED until C1 lands — that is the gate; see the final step.)
- [ ] **Write the minimal `PointerHarness`.** Create `crates/buiy_verify/src/pointer.rs`. It builds the headless interaction stack, spawns the tree, drives the production transform chain to a steady `GlobalTransform`, and injects a synthetic pointer. **It does NOT add C3's `InteractionPlugin` yet** (C3 is Wave 2) — the skeleton drives `BuiyPickingBackendPlugin` + `PickingPlugin` and reads `PointerHits` directly via the same `Messages<PointerHits>` seam `picking_backend.rs` uses, so the offset RED proof is exercisable now. C3's behavior asserts (Task 4) layer on later.
  ```rust
  //! Tier A — the headless synthetic-Pointer harness (C7 §2.1). Spawns a
  //! REAL non-origin widget tree, lets the production layout -> bridge ->
  //! transform-propagation chain produce `GlobalTransform`, then injects a
  //! synthetic `PointerId` + `PointerLocation` and reads the resulting
  //! `PointerHits` (and, once C3 lands, the durable widget-state flip + a
  //! thin observer-capture log). This fixes `picking_backend.rs`'s blind
  //! spot: that test hand-writes a single-node `ResolvedLayout` at an
  //! absolute position and is structurally incapable of observing Bug 1
  //! (C7 §1.1; umbrella §9.5).

  use bevy::camera::NormalizedRenderTarget;
  use bevy::ecs::message::Messages;
  use bevy::picking::backend::PointerHits;
  use bevy::picking::pointer::{Location, PointerId, PointerLocation};
  use bevy::prelude::*;
  use bevy::window::{PrimaryWindow, Window, WindowRef, WindowResolution};

  use buiy_core::Node;
  use buiy_core::components::ResolvedLayout;
  use buiy_core::layout::Style;
  use buiy_core::picking::{BuiyPickingBackendPlugin, PickingPlugin};

  /// The thin observer-capture log (§2.1): once C3's `Pointer<E>` observers
  /// exist (Task 4, Wave 2) they push `(entity, phase)` here so propagation /
  /// bubbling / propagate(false) tests can read which entities saw an event and
  /// in what order. This is the ONLY test-only wiring; it observes the
  /// production events C3 defines (it does not replace them). Empty in Wave 1.
  #[derive(Resource, Default)]
  pub struct CapturedEvents(pub Vec<(Entity, &'static str)>);

  pub struct PointerHarness {
      app: App,
      pointer: Entity,
      window: Entity,
  }

  impl PointerHarness {
      /// MinimalPlugins + TransformPlugin + CorePlugin + LayoutPlugin +
      /// bevy::picking::PickingPlugin + the Buiy backend. NO RenderPlugin, NO
      /// winit, NO AssetPlugin — picking runs as pure ECS so the full hit-test
      /// path is headless-CI-runnable. (C3's InteractionPlugin/FocusPlugin are
      /// added in Task 4 once C3 exists; §3.2 build-step confirms whether they
      /// read direct injection without PointerInputPlugin.)
      pub fn new() -> Self {
          let mut app = App::new();
          app.add_plugins(MinimalPlugins)
              .add_plugins(bevy::transform::TransformPlugin)
              .add_plugins(bevy::picking::PickingPlugin)
              .add_plugins(buiy_core::CorePlugin)
              .add_plugins(buiy_core::layout::LayoutPlugin)
              .add_plugins(PickingPlugin)
              .add_plugins(BuiyPickingBackendPlugin);
          app.init_resource::<CapturedEvents>();

          // A synthetic primary window — the layout solver reads its viewport
          // from a plain Query<&Window, With<PrimaryWindow>> (no WindowPlugin).
          let window = app
              .world_mut()
              .spawn((
                  Window {
                      resolution: WindowResolution::new(800, 600),
                      ..Default::default()
                  },
                  PrimaryWindow,
              ))
              .id();

          // The synthetic pointer entity (spawned once). PointerLocation is
          // (re)written by `move_to`. `PointerId::Mouse` passes through the
          // normal backend pipeline (picking_backend.rs:55-63).
          let target = WindowRef::Entity(window)
              .normalize(Some(window))
              .expect("normalize window target");
          let pointer = app
              .world_mut()
              .spawn((
                  PointerId::Mouse,
                  PointerLocation::new(Location {
                      target: NormalizedRenderTarget::Window(target),
                      position: Vec2::ZERO,
                  }),
              ))
              .id();

          Self { app, pointer, window }
      }

      /// Spawn `scene` as the single child of a root translated by the EXPLICIT
      /// `offset`, returning the `scene` entity (the entity under test). The
      /// root's `Translate` is folded by the production bridge into the child's
      /// `GlobalTransform` while the child's `ResolvedLayout.position` stays
      /// PARENT-LOCAL (Taffy `location`, write_resolved_layout @ systems.rs:2734)
      /// — so `GlobalTransform.translation` (absolute) diverges from
      /// `ResolvedLayout.position` (local) by exactly `offset`. That divergence
      /// is what the offset RED proof exercises. Drives a bounded settle so
      /// layout + bridge + the three propagation systems produce a steady
      /// `GlobalTransform` on the returned entity.
      pub fn spawn_offset_tree(&mut self, offset: Vec2, scene: impl Bundle) -> Entity {
          let target = self.app.world_mut().spawn(scene).id();
          let _root = self
              .app
              .world_mut()
              .spawn((
                  Node,
                  // Translate (not padding) so the child's parent-local position
                  // stays small while its accumulated global = offset + local.
                  Style::default()
                      .flex_column()
                      .width_px(800.0)
                      .height_px(600.0)
                      .translate_px(offset.x, offset.y),
              ))
              .add_child(target)
              .id();
          // Bounded settle: the bridge + the three propagation systems run in
          // Update before Picking; a few frames produce GlobalTransform.
          for _ in 0..4 {
              self.app.update();
          }
          target
      }

      /// The absolute (window-logical) center of `entity`, from the
      /// GlobalTransform the production chain produced + its ResolvedLayout size.
      pub fn global_center(&self, entity: Entity) -> Vec2 {
          let world = self.app.world();
          let gt = world
              .get::<GlobalTransform>(entity)
              .expect("entity has GlobalTransform (went through the bridge)");
          let size = world
              .get::<ResolvedLayout>(entity)
              .expect("entity has ResolvedLayout")
              .size;
          gt.translation().truncate() + size * 0.5
      }

      /// Move the synthetic pointer to a WINDOW-space position and run one
      /// update so the backend re-emits `PointerHits` for the new location.
      pub fn move_to(&mut self, pos: Vec2) {
          let target = WindowRef::Entity(self.window)
              .normalize(Some(self.window))
              .expect("normalize window target");
          {
              let mut loc = self
                  .app
                  .world_mut()
                  .get_mut::<PointerLocation>(self.pointer)
                  .expect("synthetic pointer has PointerLocation");
              *loc = PointerLocation::new(Location {
                  target: NormalizedRenderTarget::Window(target),
                  position: pos,
              });
          }
          self.app.update();
      }

      /// The top-most entity the backend reports under the pointer this frame
      /// (index 0 of `picks`, the closest). `None` if no Buiy node is hit.
      pub fn top_hit(&mut self) -> Option<Entity> {
          let messages = self.app.world().resource::<Messages<PointerHits>>();
          let mut cursor = messages.get_cursor();
          let mut latest: Option<Entity> = None;
          for hits in cursor.read(messages) {
              latest = hits.picks.first().map(|(e, _)| *e);
          }
          latest
      }

      /// Mutable world access for assertions (Checked/Pressed/Selected/
      /// FocusedEntity once C3/C4 land) and direct scene mutation.
      pub fn world_mut(&mut self) -> &mut World {
          self.app.world_mut()
      }

      /// Read the capture log (propagation tests; populated once C3 observers
      /// exist in Task 4).
      pub fn captured(&self) -> &CapturedEvents {
          self.app.world().resource::<CapturedEvents>()
      }
  }

  impl Default for PointerHarness {
      fn default() -> Self {
          Self::new()
      }
  }
  ```
  **Re-confirm at Phase 0** the line anchors only (the symbols/signatures are verified against the current tree): the `Messages<PointerHits>` + `get_cursor` seam and `PointerLocation::new(Location { target, position })` are pinned by `picking_backend.rs:12-71` (the integration test mirrors this exact API); `Style::translate_px` is `style.rs:491`; `write_resolved_layout` writes `position = layout.location` (parent-local) at `systems.rs:2734`; `gt.translation().truncate()` is the absolute-position contract C1 documents (coordinate-space-correctness.md §2). `PointerHits` is a bevy 0.19 `Message` (read via `Messages` + `get_cursor`), confirmed by `picking_backend.rs:13,70-71`.
- [ ] **Register the module.** Edit `crates/buiy_verify/src/lib.rs`: add `pub mod pointer;` to the module list (after `pub mod metric;`).
- [ ] **Run & show the harness compiles + the RED proof is RED for the RIGHT reason.**
  ```sh
  cd /mnt/storage/projects/buiy
  cargo test -p buiy_verify --test verify_headless pointer_offset_regression:: 2>&1 | tail -20
  ```
  Expected: the test **FAILS** with the assert message (`Some(target)` vs the wrong/`None` hit) — NOT a compile error. This is the load-bearing RED: on pre-C1 main the backend hit-tests against `ResolvedLayout.position` (parent-local) while the pointer aims at `GlobalTransform`-derived absolute center, so the pointer over the visual target misses it. **Capture this output** — it is the proof the harness is not vacuous-green. If it unexpectedly PASSES on pre-C1 main, the offset did not force divergence; confirm with a one-off `dbg!` that `ResolvedLayout.position != GlobalTransform.translation.truncate()` for the target, and increase `offset` (the `Translate` on the root must actually propagate into the child's `GlobalTransform`).
- [ ] **Annotate `picking_backend.rs` as superseded-for-integration.** Add a doc note at the top of `crates/buiy_core/tests/crosscut/picking_backend.rs` (keep the test — it is the unit-level backend test):
  ```rust
  //! NOTE (C7): this hand-writes `ResolvedLayout` at an absolute position and is
  //! therefore STRUCTURALLY BLIND to Bug 1 (parent-local vs absolute coordinate
  //! divergence). It is kept as the unit-level backend smoke; the integration
  //! regression coverage is `buiy_verify::pointer::PointerHarness`'s offset-tree
  //! test (crates/buiy_verify/tests/verify_headless/pointer_offset_regression.rs), which drives
  //! the real layout -> bridge -> GlobalTransform chain. Do NOT trust this file
  //! as the coordinate-correctness gate.
  ```
- [ ] **Mark the RED proof so CI does not block on it pre-C1.** Because this plan lands in Wave 1 *before* C1, the offset test is expected RED until C1 lands. Gate it so the Wave-1 merge stays green while preserving the proof:
  ```rust
  // Committed #[ignore] (NOT a hand-revert): until C1 lands this is the
  // documented RED gate. C1's PR DELETES this attribute (un-ignores) and the
  // test goes GREEN — that transition IS C1's coordinate-fix verification.
  #[ignore = "RED until C1 lands: picking reads ResolvedLayout.position (parent-local) as absolute; C1 routes it through GlobalTransform and un-ignores this test"]
  ```
  Apply `#[ignore = "..."]` to `synthetic_pointer_hits_offset_widget_at_its_global_position`. **C1's plan references this exact file/test and, in its own task, DELETES the `#[ignore]` attribute and asserts GREEN — C1 must NOT recreate the harness, the file, or the test, and must NOT use a manual hand-revert demonstration for it.** Document in the commit body that C1's PR removes the `#[ignore]` as it goes GREEN. Run the suite to confirm the `#[ignore]`d test is collected-but-skipped and the workspace gate is green:
  ```sh
  cd /mnt/storage/projects/buiy
  cargo test -p buiy_verify --test verify_headless pointer_offset_regression:: 2>&1 | tail -8
  cargo test -p buiy_verify --test verify_headless pointer_offset_regression:: -- --ignored 2>&1 | tail -8   # shows the RED proof on demand
  ```
  Expected: default run = 0 run / 1 ignored (green); `--ignored` run = 1 FAILED (the RED proof, captured above).
- [ ] **Commit.**
  ```sh
  cd /mnt/storage/projects/buiy
  git add crates/buiy_verify/src/lib.rs crates/buiy_verify/src/pointer.rs \
          crates/buiy_verify/tests/verify_headless/pointer_offset_regression.rs crates/buiy_core/tests/crosscut/picking_backend.rs
  git commit -m "test(verify): Tier-A PointerHarness skeleton + offset regression (C7, RED-first for C1)

PointerHarness spawns a real non-origin tree, drives the production
layout->bridge->GlobalTransform chain, and injects a synthetic pointer
through the sanctioned bevy_picking path. The offset-tree test is the
load-bearing RED proof for C1: it is #[ignore]d-RED on pre-C1 main (the
backend mis-reads parent-local ResolvedLayout.position as absolute) and
C1's PR un-ignores it as it goes GREEN. picking_backend.rs annotated as
structurally blind to Bug 1 and superseded for integration coverage.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Task 3 — Tier-A direct-injection vs `PointerInputPlugin` confirm + `CapturedEvents` log scaffold

The spec (§3.2) DECIDES direct `PointerLocation`/`PointerInput` injection (no `PointerInputPlugin`) but carries a one-line build-step confirm against C3's actual plugin graph. Since C3 is Wave 2 and not yet present (Phase 0 confirmed), this task completes the `PointerHarness` button surface (`press`/`release`/`click`, each taking an explicit `PointerButton`) and records the direct-injection decision in-code so Task 4 (Wave 2) is a pure fill-in. The `CapturedEvents` resource + `captured()` reader already landed in Task 2 (`new()` inits it); the C3-era observer that POPULATES it lands in Task 4. This keeps the Wave-1 deliverable self-contained without depending on C3.

### Files
- Modify `crates/buiy_verify/src/pointer.rs` (add `press`/`release`/`click(button: PointerButton)` writing `PointerInput`)
- Create `crates/buiy_verify/tests/verify_headless/pointer_press_smoke.rs` — and **register it** with `#[path = "verify_headless/pointer_press_smoke.rs"] mod pointer_press_smoke;` in `crates/buiy_verify/tests/verify_headless.rs`.

### Steps

- [ ] **Write the failing test first — a synthetic press reaches the backend.** This exercises the press/release path against the not-yet-C3 backend; on current main it asserts the lower-level `PointerHits`-still-fires-under-press contract (the durable state-flip assert lands in Task 4 once `Checked` exists). Create `crates/buiy_verify/tests/verify_headless/pointer_press_smoke.rs`:
  ```rust
  //! Smoke: a synthetic press at the pointer's current location does not
  //! disturb the hit stream and is injected through the sanctioned path
  //! (PointerInput written directly — §3.2). The DURABLE state-flip assert
  //! (Checked after click) lands in Task 4 with C3/C4; this proves the
  //! press/release injection seam works on the Wave-1 backend.

  use bevy::picking::pointer::PointerButton;
  use bevy::prelude::*;
  use buiy_core::Node;
  use buiy_core::layout::Style;
  use buiy_verify::pointer::PointerHarness;

  #[test]
  fn press_release_at_a_hit_keeps_the_entity_hit() {
      let mut h = PointerHarness::new();
      let target = h.spawn_offset_tree(
          Vec2::new(80.0, 60.0),
          (Node, Style::default().width_px(120.0).height_px(40.0), Name::new("btn")),
      );
      let center = h.global_center(target);
      h.move_to(center);
      // Press then release at the same spot: the hit must remain the target
      // across the press (no spurious clearing). The CapturedEvents log is
      // empty until C3's observers exist — this asserts only the hit stream.
      h.press(PointerButton::Primary);
      assert_eq!(h.top_hit(), Some(target), "the target stays hit under press");
      h.release(PointerButton::Primary);
      assert_eq!(h.top_hit(), Some(target), "the target stays hit after release");
  }
  ```
- [ ] **Run it & show the expected FAIL.** `press`/`release` are not defined:
  ```sh
  cd /mnt/storage/projects/buiy
  cargo test -p buiy_verify --test verify_headless pointer_press_smoke:: 2>&1 | head -12
  ```
  Expected: compile error — `no method named `press` found for struct `PointerHarness``.
- [ ] **Implement `press`/`release`/`click(button: PointerButton)`.** Add to `crates/buiy_verify/src/pointer.rs`. Each takes an explicit `PointerButton` (the FINAL API) and writes `PointerInput` directly per §3.2. Add the import `use bevy::picking::pointer::{PointerAction, PointerButton, PointerInput};` to the existing import block. (`CapturedEvents` + `captured()` already landed in Task 2; do NOT redefine them.)
  ```rust
  impl PointerHarness {
      /// Write a `PointerInput` Press of `button` at the current pointer
      /// location and run an update. Direct injection (§3.2 — the
      /// lessons-sanctioned synthetic path; NOT PointerInputPlugin, whose job is
      /// to translate winit events we are replacing). RE-CONFIRM AT WAVE 2: if
      /// C3's InteractionPlugin depends on PointerInputPlugin running first, add
      /// PointerInputPlugin in `new()` and feed it PointerInput rather than
      /// hand-maintaining state (§3.2 caveat).
      pub fn press(&mut self, button: PointerButton) {
          self.write_button(PointerAction::Press(button));
      }
      pub fn release(&mut self, button: PointerButton) {
          self.write_button(PointerAction::Release(button));
      }
      pub fn click(&mut self, button: PointerButton) {
          self.press(button);
          self.release(button);
      }

      fn write_button(&mut self, action: PointerAction) {
          let location = self
              .app
              .world()
              .get::<PointerLocation>(self.pointer)
              .expect("synthetic pointer has PointerLocation")
              .location()
              .expect("pointer has a location")
              .clone();
          self.app.world_mut().write_message(PointerInput {
              pointer_id: PointerId::Mouse,
              location,
              action,
          });
          self.app.update();
      }
  }
  ```
  `PointerLocation::location()` returns `Option<&Location>` (pointer.rs:197 in bevy_picking-0.19.0-rc.3), so `.location().expect(...).clone()` yields an owned `Location`. `PointerInput` is `#[derive(Message)]` with fields `pointer_id: PointerId`, `location: Location`, `action: PointerAction` (pointer.rs:280) — so `World::write_message(PointerInput { .. })` (bevy_ecs world/mod.rs:3015) is correct. `PointerButton::{Primary, Secondary, Middle}` (pointer.rs:161). **Re-confirm at Phase 0** the line anchors only; the types/fields are verified against the vendored bevy_picking-0.19.0-rc.3 source.
- [ ] **Run & show expected PASS:**
  ```sh
  cd /mnt/storage/projects/buiy
  cargo test -p buiy_verify --test verify_headless pointer_press_smoke:: 2>&1 | tail -8
  ```
  Expected: `press_release_at_a_hit_keeps_the_entity_hit` PASS.
- [ ] **Commit.**
  ```sh
  cd /mnt/storage/projects/buiy
  git add crates/buiy_verify/src/pointer.rs crates/buiy_verify/tests/verify_headless/pointer_press_smoke.rs
  git commit -m "test(verify): PointerHarness press/release/click + CapturedEvents scaffold (C7 §2.1, §3.2)

Direct PointerInput injection (the lessons-sanctioned synthetic path, NOT
PointerInputPlugin). CapturedEvents observer log scaffolded for the Wave-2
propagation asserts; empty until C3 registers Pointer<E> observers. §3.2's
direct-vs-PointerInputPlugin build-step confirm is recorded in-code for the
Wave-2 fill-in.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Task 4 — Tier-B `bump_fonts_generation` + content-survival test (RED-first for C2)

The audit found zero coverage for the editor-content clobber (Bug 3) and the shape-guard (Bug 2). Tier B folds a deterministic `FontsGeneration`-bump helper into the existing adapterless `TextExtractHarness` (`extract_harness.rs`) and writes `text_font_reload_survival.rs` (spec §2.2). The content-survival assert is the **RED-first proof for C2**: with C2's `TextSync` editor-clobber fix reverted (i.e. on current main), the bump clobbers the editor buffer to `""` and the test FAILS.

**Ownership (C7 is the SOLE creator):** C7 creates `bump_fonts_generation` (in `extract_harness.rs`) and the file `crates/buiy_core/tests/text_edit/text_font_reload_survival.rs` (this exact name — NOT `text_editor_integrity.rs`) with **all five** tests at these EXACT names: `editor_content_survives_a_fonts_generation_bump`, `label_reshapes_and_keeps_glyphs_after_a_bump`, `empty_editor_emits_zero_glyphs_and_does_not_crash_on_bump`, `preedit_survives_a_fonts_generation_bump`, `editor_style_stays_live_after_a_bump`. The three RED-on-pre-C2 tests (content-survival, preedit, editor-style) ship as committed `#[ignore = "RED until C2 lands: …"]`. **C2's plan references these C7-owned files/tests and, in its own task, DELETES the `#[ignore]` attribute (un-ignores) + asserts GREEN — C2 must NOT recreate the file, the `bump_fonts_generation` method, or the tests, and must NOT use a manual hand-revert demonstration for them.**

**Nextest, not debug_assert (contract):** the reshape/shape-guard RED must manifest as an observable `glyph_count() == 0` under the actual test profile (`cargo nextest run`), NOT only as a `debug_assert!` panic — `debug_assert` may be compiled out under a release-profile nextest run, so a count assert is the load-bearing signal and the `debug_assert` is at most a secondary one. Every Tier-B test here asserts a concrete `glyph_count()` / `value()` / `metrics_for_test()`, never relying on a `debug_assert` firing.

### Files
- Modify `crates/buiy_core/tests/support/extract_harness.rs` (add `bump_fonts_generation`)
- Create `crates/buiy_core/tests/text_edit/text_font_reload_survival.rs` — and **register it** with `#[path = "text_edit/text_font_reload_survival.rs"] mod text_font_reload_survival;` in `crates/buiy_core/tests/text_edit.rs`. The Tier-B test seeds editors via `TextEditState`/IME, so it belongs in the `text_edit` group binary (where `text_edit_substrate.rs`, `text_clipboard_undo.rs`, `text_ime_*` live), not the `text` render binary. The module reaches the shared harness via `support::extract_harness::TextExtractHarness` (the `text_edit` binary root already owns `mod support;`) — do NOT add a fresh `mod support;` inside the module; use `mod support;` → `crate::support` per the consolidation convention.

### Steps

- [ ] **Write the failing test first — editor content survives a font-generation bump.** Create `crates/buiy_core/tests/text_edit/text_font_reload_survival.rs`:
  ```rust
  //! Tier B (C7 §2.2) — the FontsGeneration-bump content-survival / reshape /
  //! empty-editor / preedit / editor-style tests, headless on the adapterless
  //! TextExtractHarness. These are the content/preedit/style-survival tests the
  //! audit says are MISSING (Bug 3: zero coverage). For Bug 2, the reshape arms
  //! here (`label_reshapes_…`, `editor_style_stays_live_…`) are REGRESSION
  //! guards, NOT the isolating `shape_stale` proof: the FontsGeneration sweep
  //! auto-heals via `mark_dirty_for_entity` → Taffy re-measure (audit
  //! Appendix-A.5), so they pass with or without the guard term. The directed,
  //! isolating `shape_stale` proof is C2-owned —
  //! `text_commit.rs::shape_stale_reshapes_a_committed_but_unshaped_buffer`
  //! (constructs the unshaped state with no sweep / no re-measure). See the C2
  //! plan Task 5.
  //!
  //! RED-first for C2: on current main (pre-C2) the FontsGeneration sweep
  //! clobbers the editor-owned buffer to "" (the all-buffers sweep at sync.rs:251
  //! set_text on the editor buffer), so the content-survival / preedit /
  //! editor-style asserts FAIL until C2's TextSync fix lands. C2's PR un-ignores
  //! them (deletes the #[ignore]) — it must NOT recreate this file or its tests.
  //!
  //! This file is a MODULE of the `text_edit` group binary (PR #77 consolidation),
  //! registered via `#[path] mod text_font_reload_survival;` in `tests/text_edit.rs`.
  //! It does NOT declare `mod support;` (the binary root owns it); it reaches the
  //! harness via `crate::support::extract_harness::TextExtractHarness`, the same
  //! way `tests/text/text_extract.rs` does.

  use bevy::prelude::*;
  use buiy_core::Node;
  use buiy_core::layout::Style;
  use buiy_core::text::SharedFontSystem;
  use buiy_core::text::edit::{EditCommand, TextEditState};
  use crate::support::extract_harness::TextExtractHarness;

  /// Spawn an editor entity pre-seeded with typed content via the real edit
  /// path, so the editor-owned buffer holds "Hello" while the display `Text`
  /// would be "" — the exact divergence Bug 3 clobbers (C2 spec §1.1).
  fn spawn_seeded_editor(h: &mut TextExtractHarness, content: &str) -> Entity {
      let mut state = TextEditState::for_font_size(16.0);
      {
          let fonts = h.app.world().resource::<SharedFontSystem>().clone();
          let mut fs = fonts.lock();
          state.apply(&mut fs, EditCommand::Insert(content.into()), false, false);
      }
      assert_eq!(state.value(), content, "seed sanity: editor holds the typed content");
      h.app
          .world_mut()
          .spawn((
              Node,
              Style::default().width_px(200.0).height_px(40.0),
              state,
          ))
          .id()
  }

  #[test]
  fn editor_content_survives_a_fonts_generation_bump() {
      let mut h = TextExtractHarness::new();
      let editor = spawn_seeded_editor(&mut h, "Hello");
      h.settle();
      // The bump fires the all-buffers TextSync sweep (the runtime add_font
      // trigger; registry.rs apply_font_registry bumps FontsGeneration).
      h.bump_fonts_generation();
      h.settle();

      let value = h
          .app
          .world()
          .get::<TextEditState>(editor)
          .expect("editor still present")
          .value();
      assert_eq!(
          value, "Hello",
          "the editor-owned buffer must STILL hold the typed content after a \
           FontsGeneration bump; on pre-C2 main the sweep clobbers it to \"\" \
           (Bug 3) — this is the C2 content-survival gate"
      );
  }
  ```
- [ ] **Run it & show the expected FAIL.** `bump_fonts_generation` does not exist:
  ```sh
  cd /mnt/storage/projects/buiy
  cargo test -p buiy_core --test text_edit text_font_reload_survival:: 2>&1 | head -12
  ```
  Expected: compile error — `no method named `bump_fonts_generation``.
- [ ] **Implement `bump_fonts_generation`.** Add to `crates/buiy_core/tests/support/extract_harness.rs`. `FontsGeneration(pub u64)` is defined at `font_system.rs:96` and re-exported as `buiy_core::text::FontsGeneration` (`text/mod.rs:68`). Bump it exactly as `apply_font_registry` does on a real font-set change: registry.rs:324 takes `mut generation: ResMut<FontsGeneration>` (registry.rs:328) and does `generation.0 += 1` (registry.rs:543, "exactly once per batch"):
  ```rust
  use buiy_core::text::FontsGeneration;

  impl TextExtractHarness {
      /// Inject the FontsGeneration bump deterministically: increment the
      /// `FontsGeneration` resource the way `apply_font_registry`
      /// (registry.rs:543) does on a runtime add_font, then run one frame. This
      /// is the trigger for Bugs 2/3 (the sweep fires on EVERY runtime add_font,
      /// not just startup — audit Bug 3). C2 must confirm this reproduces the
      /// clobber identically to the async loader path (C7 §3.3 hand-off).
      pub fn bump_fonts_generation(&mut self) -> &mut Self {
          {
              let mut generation = self.app.world_mut().resource_mut::<FontsGeneration>();
              generation.0 += 1;
          }
          self.frame();
          self
      }
  }
  ```
  **Re-confirm at Phase 0** the line anchors only (`font_system.rs:96`, `registry.rs:543`, `sync.rs:251`): `TextSync` keys its all-buffers sweep on `fonts_generation.is_changed() && !fonts_generation.is_added()` (`sync.rs:251`, with `fonts_generation: Res<FontsGeneration>` at `sync.rs:147`). Writing `generation.0 += 1` through `resource_mut` sets the `Changed` tick, so the sweep fires next frame — the same path the real `apply_font_registry` bump triggers.
- [ ] **Run & show expected FAIL for the RIGHT reason (the RED proof for C2).**
  ```sh
  cd /mnt/storage/projects/buiy
  cargo test -p buiy_core --test text_edit text_font_reload_survival:: 2>&1 | tail -20
  ```
  Expected: `editor_content_survives_a_fonts_generation_bump` **FAILS** with `value == ""` vs `"Hello"` — the Bug-3 clobber on pre-C2 main. **Capture this output** (the C2 gate). If it unexpectedly passes, confirm the seeded editor's owned buffer actually diverges from display `Text` (the clobber only bites when `TextBufferAccess` is editor-first; verify the entity has no `Text` component so the editor buffer is the authoritative one, per C2 §1.1).
- [ ] **`#[ignore]` the survival test pre-C2 (committed, NOT a hand-revert).** Like the offset RED proof, this ships as a committed `#[ignore]`; C2's PR DELETES the attribute (un-ignores) as it goes GREEN. Apply:
  ```rust
  #[ignore = "RED until C2 lands: the FontsGeneration sweep clobbers the editor-owned buffer to \"\"; C2's TextSync fix preserves it and un-ignores this test"]
  ```
  to `editor_content_survives_a_fonts_generation_bump`.
- [ ] **Add the reshape + empty-editor (GREEN today) and preedit + editor-style (RED-until-C2) tests.** The reshape and empty-editor tests assert properties C2 must preserve and pass on current main; the preedit and editor-style tests are committed `#[ignore]` RED proofs that C2 un-ignores. Append to `text_font_reload_survival.rs`:
  ```rust
  /// Reshape / shape-guard (Bug 2): after a bump, a NON-EDITOR text label
  /// reshapes (glyph_count stays correct, the producer rebuilt) — it does not
  /// silently go to zero glyphs (silent-no-paint). This is GREEN today and
  /// guards C2's shape-guard from regressing the reshape.
  #[test]
  fn label_reshapes_and_keeps_glyphs_after_a_bump() {
      let mut h = TextExtractHarness::new();
      h.app.world_mut().spawn((
          Node,
          Style::default().flex_column().width_px(300.0).height_px(100.0),
      ));
      h.app.world_mut().spawn((
          Node,
          Style::default(),
          buiy_core::text::Text(String::from("Hi!")),
          buiy_core::text::FontSize(16.0),
      ));
      h.settle();
      assert_eq!(h.glyph_count(), 3, "label shapes to 3 glyphs before the bump");
      let before = h.changed_frames();
      h.bump_fonts_generation();
      h.settle();
      assert_eq!(h.glyph_count(), 3, "label still shapes to 3 glyphs after the bump");
      assert!(
          h.changed_frames() > before,
          "the bump reshaped the buffer (the producer rebuilt) — not a silent no-op"
      );
  }

  /// Empty-editor 0-vs-1 (audit Bug 2 critical refutation): an empty editor
  /// with NO active placeholder emits 0 glyphs after the bump and does NOT
  /// crash / assert-fire. GREEN today; pins the empty case the prototype's
  /// "complete" fix accidentally relied on.
  #[test]
  fn empty_editor_emits_zero_glyphs_and_does_not_crash_on_bump() {
      let mut h = TextExtractHarness::new();
      h.app.world_mut().spawn((
          Node,
          Style::default().width_px(200.0).height_px(40.0),
          TextEditState::for_font_size(16.0),
      ));
      h.settle();
      assert_eq!(h.glyph_count(), 0, "an empty editor emits no glyphs");
      h.bump_fonts_generation(); // must not panic
      h.settle();
      assert_eq!(h.glyph_count(), 0, "still zero after the bump, no crash");
  }

  /// Preedit-survival (C2 §2.6 / spec §2.2): a live IME preedit survives the
  /// bump (a mid-composition set_text destroys composition). Committed #[ignore]
  /// RED until C2 lands; C2's PR deletes the attribute as it goes GREEN.
  #[ignore = "RED until C2 lands: the FontsGeneration sweep destroys a mid-composition preedit; C2's preedit-aware TextSync fix preserves it and un-ignores this test"]
  #[test]
  fn preedit_survives_a_fonts_generation_bump() {
      let mut h = TextExtractHarness::new();
      let mut state = TextEditState::for_font_size(16.0);
      {
          let fonts = h.app.world().resource::<SharedFontSystem>().clone();
          let mut fs = fonts.lock();
          state.apply(&mut fs, EditCommand::Insert("ab".into()), false, false);
          state.splice_preedit(&mut fs, "X", None);
      }
      let editor = h.app.world_mut().spawn((
          Node,
          Style::default().width_px(200.0).height_px(40.0),
          state,
      )).id();
      h.settle();
      h.bump_fonts_generation();
      h.settle();
      let has_preedit = h
          .app
          .world()
          .get::<TextEditState>(editor)
          .expect("editor present")
          .with_buffer(|b| b.lines.iter().any(|l| l.text().contains('X')));
      assert!(has_preedit, "the live preedit must survive the bump");
  }

  /// Editor-style-stays-live (C2 spec §2.2 / §1.1): the editor's owned style
  /// (font size) survives the bump — the sweep must reshape against the SAME
  /// metrics, not reset the editor to defaults. RED until C2's style-preserving
  /// TextSync fix lands; #[ignore]d like the survival test. The editor is seeded
  /// at a NON-default font size so a clobber-to-default is observable.
  #[ignore = "RED until C2 lands: the FontsGeneration sweep resets the editor's owned metrics; C2 preserves them and un-ignores"]
  #[test]
  fn editor_style_stays_live_after_a_bump() {
      let mut h = TextExtractHarness::new();
      let mut state = TextEditState::for_font_size(28.0); // non-default size
      {
          let fonts = h.app.world().resource::<SharedFontSystem>().clone();
          let mut fs = fonts.lock();
          state.apply(&mut fs, EditCommand::Insert("Ag".into()), false, false);
      }
      let (size_before, _) = state.metrics_for_test();
      assert_eq!(size_before, 28.0, "seed sanity: the editor holds the 28px metrics");
      let editor = h
          .app
          .world_mut()
          .spawn((Node, Style::default().width_px(200.0).height_px(60.0), state))
          .id();
      h.settle();
      h.bump_fonts_generation();
      h.settle();
      let (size_after, _) = h
          .app
          .world()
          .get::<TextEditState>(editor)
          .expect("editor present")
          .metrics_for_test();
      assert_eq!(
          size_after, 28.0,
          "the editor's owned font size must survive the bump; on pre-C2 main the \
           sweep resets it (the C2 editor-style-stays-live gate)"
      );
  }
  ```
  **Re-confirm at Phase 0** the line anchors only: `for_font_size(f32)` is at `state.rs:143` (the contract's "~line 170" was stale — verified at 143; **use `for_font_size` everywhere**, one form across all plans); `apply(font_system, command, single_line, read_only)` at `input.rs:67`; `splice_preedit(font_system, value, cursor)` at `ime.rs:104`; `metrics_for_test() -> (f32, f32)` (font_size, line_height) at `state.rs:149`; `with_buffer(|b| …)` at `state.rs:158` exposes `b.lines[i].text()`. If C2 lands a cleaner preedit/style-introspection seam, prefer it.
- [ ] **Confirm the dead auto-healing pattern is absent (verified).** Per spec §2.2 the prototype's `text_commit_font_reload.rs` auto-heals and guards nothing. On the current tree `rg -l text_commit_font_reload crates/` returns **nothing** — it never landed on main, so there is nothing to delete. Re-run the check at Phase 0; if the rebase somehow surfaced it, delete it (its coverage is replaced by `text_font_reload_survival.rs`). Otherwise note the absence in the commit body and skip.
- [ ] **Run & show expected results** under the actual nextest profile (the RED proofs skipped by default):
  ```sh
  cd /mnt/storage/projects/buiy
  cargo nextest run -p buiy_core --test text_edit text_font_reload_survival 2>&1 | tail -12
  cargo nextest run -p buiy_core --test text_edit --run-ignored ignored-only text_font_reload_survival 2>&1 | tail -12
  ```
  Expected default: `label_reshapes_and_keeps_glyphs_after_a_bump` PASS, `empty_editor_emits_zero_glyphs_and_does_not_crash_on_bump` PASS, the three `#[ignore]`d tests (content-survival, preedit, editor-style) skipped (green). `--run-ignored ignored-only` run: those three FAIL (the C2 RED proofs — captured above), each via a concrete `value()` / buffer-text / `metrics_for_test()` assert, not a `debug_assert`. (Plain `cargo test -- --ignored` works too if nextest's `--run-ignored` flag is unavailable on the pinned version — confirm at Phase 0.)
- [ ] **Commit.**
  ```sh
  cd /mnt/storage/projects/buiy
  git add crates/buiy_core/tests/support/extract_harness.rs crates/buiy_core/tests/text_edit/text_font_reload_survival.rs
  git commit -m "test(core): Tier-B FontsGeneration-bump content-survival on the adapterless harness (C7 §2.2, RED-first for C2)

bump_fonts_generation injects the FontsGeneration bump deterministically (the
runtime add_font trigger, registry.rs:543). text_font_reload_survival covers
content-survival (Bug 3), reshape/shape-guard (Bug 2), empty-editor 0-vs-1,
preedit, and editor-style. The content-survival + preedit + editor-style tests
are committed #[ignore]-RED on pre-C2 main (the sweep clobbers the editor
buffer / metrics); C2's PR DELETES the #[ignore] as each goes GREEN — that
transition is C2's content-integrity verification. The prototype's dead
auto-healing text_commit_font_reload pattern is absent on main (nothing to
delete).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Task 5 — Wave-1 gate green + cross-wave handoff notes

Confirm the full workspace gate is green with all RED proofs `#[ignore]`d, and record the un-ignore handoffs C1/C2 own.

### Files
- (no source edits) — verification + a handoff note appended to this plan / the child spec's §7.

### Steps
- [ ] **Run the full headless gate.** Confirm green with the RED proofs collected-but-skipped:
  ```sh
  cd /mnt/storage/projects/buiy
  cargo fmt --all -- --check
  cargo clippy --workspace --all-targets -- -D warnings
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps
  xvfb-run -a cargo nextest run --workspace
  ```
  Expected: all green; the four `#[ignore]`d RED proofs — `synthetic_pointer_hits_offset_widget_at_its_global_position` (pointer), `editor_content_survives_a_fonts_generation_bump`, `preedit_survives_a_fonts_generation_bump`, `editor_style_stays_live_after_a_bump` (text) — show as skipped, not run.
- [ ] **Run the RED proofs on demand to re-confirm they are non-vacuous** (they must FAIL on the current pre-C1/pre-C2 tree):
  ```sh
  cd /mnt/storage/projects/buiy
  cargo nextest run -p buiy_verify --test verify_headless --run-ignored ignored-only pointer_offset_regression 2>&1 | tail -6
  cargo nextest run -p buiy_core --test text_edit --run-ignored ignored-only text_font_reload_survival 2>&1 | tail -8
  ```
  Expected: offset-regression FAILS (wrong/no hit); content-survival FAILS (`""` vs `"Hello"`); preedit-survival FAILS; editor-style FAILS (metrics reset). **These captured failures are the deliverable's teeth.**
- [ ] **Record the un-ignore handoff** in the child spec's §7 dependencies (edit `docs/specs/2026-06-22-buiy-widget-catalog-design/verification-real-input.md` if it does not already say so) and in this plan:
  - **C1's PR** DELETES the `#[ignore]` from `synthetic_pointer_hits_offset_widget_at_its_global_position` (the C7-owned `crates/buiy_verify/tests/verify_headless/pointer_offset_regression.rs`) and proves it GREEN — that transition is C1's coordinate-fix verification. C1 does NOT recreate the harness/file/test and uses no hand-revert.
  - **C2's PR** DELETES the `#[ignore]` from `editor_content_survives_a_fonts_generation_bump`, `preedit_survives_a_fonts_generation_bump`, and `editor_style_stays_live_after_a_bump` (the C7-owned `crates/buiy_core/tests/text_edit/text_font_reload_survival.rs`) and proves them GREEN — that transition is C2's content-integrity verification. C2 does NOT recreate the file / `bump_fonts_generation` / tests and uses no hand-revert.
  - **C3 (Wave 2)** fills in Task 4's behavior asserts on `PointerHarness` (focus-on-click, pick-depth/stacking, overlay/modal hit-blocking, `Pickable::IGNORE` hit-through, activation-parity: pointer `Click` vs the agent-interface router's `Action::Click`→`OnPress` — no competing `Activate`, umbrella §2.7) and resolves the §3.2 direct-injection-vs-`PointerInputPlugin` build-step confirm against its real plugin graph; if `InteractionPlugin` needs `PointerInputPlugin`, add it in `PointerHarness::new()`. a11y-state reads in those asserts go through the agent-interface `semantic_tree(app, view)` tier / `A11yToggled` components.
  - **Agent-interface campaign (P0 + P1a)** lands the a11y `WireNode` ref-fix + role + tri-state extension and the `A11yNodeView`/`build_tree`/`to_accesskit_node` widen + the `semantic_tree(app, view)` tier — **NOT this plan, NOT C4+C7** (umbrella §2.7, spec §2.5). This Wave-1 plan deliberately does not touch `a11y.rs`'s `WireNode`/`role_to_str`/`KNOWN_ROLES`; the a11y goldens re-bless once, in that campaign's change, governed by its "every new role/state ships its #3 fixture in the same change" rule. C7's Wave-2 behavior asserts (Task 4) *read* that tier for a11y state; they do not extend it.
- [ ] **Commit any doc handoff edit** (if the spec §7 needed the explicit un-ignore note):
  ```sh
  cd /mnt/storage/projects/buiy
  git add docs/specs/2026-06-22-buiy-widget-catalog-design/verification-real-input.md
  git commit -m "docs(spec): record C1/C2 un-ignore handoff for C7's RED proofs

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Out of scope (explicit — owned by other campaigns / later waves, do NOT build here)

- **a11y `WireNode` tri-state + role serialization + the `A11yNodeView`/`build_tree`/`to_accesskit_node` widen + the `semantic_tree(app, view)` tier** (spec §2.5): owned by the **agent-interface campaign** (P0 + P1a, umbrella §2.7) — NOT C7, NOT a C4+C7 coordinated change. Touching `a11y.rs` here would fork the a11y view and double-re-bless every a11y golden. This plan leaves `WireNode`/`role_to_str`/`KNOWN_ROLES` untouched and consumes the agent-interface `semantic_tree(app, view)` tier + gates #3/#4/#6/#7/#12 for any a11y assertion. The in-process driver (`a11y/inprocess.rs` + `accesskit_consumer`) and gates #3/#4/#6/#7/#12 are likewise agent-interface — C7 does not duplicate them.
- **Tier-A behavior asserts** (focus-on-click, `painters_z` pick-depth/stacking, overlay/modal hit-blocking, `Pickable::IGNORE`, activation-parity via pointer `Click` vs the agent-interface router's `Action::Click`→`OnPress`, a11y-tree-under-bulk-ops *read through the agent-interface tier*): spec §2.1/§5 Task 4 — **Wave 2**, gated on C3's `Pointer<E>`/`painters_z` + the agent-interface state components/router (activation routes through `OnPress`/the router, no competing `Activate`).
- **Tier-C `#[ignore]` winit smoke** (spec §2.3/§3.5/§5 Task 6): the real-camera-ref + winit coordinate/scale-factor path — lands once C3 wires the real camera ref (Wave 2+), on the GPU lane.
- **Catalog fixture enrollment** (spec §2.6/§5 Task 7) and **`Matrix::gallery_screen()`** (spec §2.6): authored as C4/C5/C6/C8 land the widgets — **Waves 3-5**.
- **R1/R2 byte-stability tests** (spec §4 own-contracts, umbrella §6.7): C7 owns them but they gate C6's styling feed — land with C6 in **Wave 3**.
