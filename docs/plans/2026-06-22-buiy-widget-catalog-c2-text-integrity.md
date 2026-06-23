# Editor Text-Integrity (Bugs 2 + 3) Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (- [ ]) syntax for tracking.

**Goal:** A `FontsGeneration` bump (system-font scan OR runtime `add_font` batch) must never clobber an editor-owned buffer's typed content or live IME preedit, must keep re-applying the editor's style (metrics/wrap/tab-width/attrs), and must never leave a committed buffer unshaped at extract — fixing the data-loss bug (Bug 3) and the silent-no-paint bug (Bug 2) as one coordinated change. The editor's content seed/programmatic-set channel is the **existing** `EditCommand` verbs (`Insert` for the empty-editor seed, `SelectAll` + `Insert` for a programmatic set); **no new `EditCommand::SetValue` is added** — the `EditCommand` surface is owned by the agent-interface campaign (umbrella §2.7), which lowers `Action::SetValue`-text via the same existing `SelectAll` + `Insert` (`action-router.md` §4, `phasing.md` P1c).

**Architecture:** Two coordinated edits in the text measure/commit path, plus a confirmed seed/set channel using existing verbs: (1) `TextSync` (`sync.rs`) branches on a new `TextBufferAccessItem::has_edit()` discriminant — editor entities take a **style-only** lowering (`apply_authored_style_to_editor_buffer`) that re-applies metrics/wrap/tab-width and refreshes per-line default attrs but **never calls `set_text`**; display entities keep the unchanged content+style path. (2) `text_commit` (`commit.rs`) gains a fourth short-circuit term `shape_stale` that re-detects the exact mismatch extract asserts (`layout_runs().count() != computed.lines.len()`) and reshapes at the commit lock site. (3) The editor's seed/programmatic-set channel is the **existing** `EditCommand::Insert` (empty-editor seed) and `EditCommand::SelectAll` + `Insert` (programmatic set) — **no new variant** (agent-interface-owned `EditCommand` surface, umbrella §2.7). No new `EditCommand` variants, components, resources, schedules, or byte/stride formats.

**Tech Stack:** Rust, Bevy 0.19.0-rc.3 ECS, cosmic-text 0.19.0 (`Buffer`/`BufferLine`/`AttrsList`/`Editor`/`Action`), the `text::edit` facade (`TextEditState`/`TextBufferAccess`), the adapterless `TextExtractHarness` test substrate.

**Wave / dependencies:** Wave 1 (umbrella §5). Independent of C1/C3/C6 (no coordinate/picking/styling contract). **C7 OWNS the shared Tier-B infrastructure** — the cross-plan contract makes C7 the SOLE creator of `crates/buiy_core/tests/support/extract_harness.rs::bump_fonts_generation`, the Tier-B file `crates/buiy_core/tests/text_font_reload_survival.rs`, and every harness-based Wave-1 RED test (landed as committed `#[ignore = "RED until C2 lands: …"]`, not hand-reverts). **C2 does NOT create that file, that method, or those tests.** C2's verification task is to UN-IGNORE the C7-owned RED tests (delete the `#[ignore]` attribute) and assert GREEN once the joint fix lands, plus keep the already-green Tier-B arms green. The C7-owned Tier-B tests C2 gates against:

| C7-owned test (in `text_font_reload_survival.rs`) | C7 state on pre-C2 main | C2's action |
|---|---|---|
| `editor_content_survives_a_fonts_generation_bump` | `#[ignore]`-RED | delete `#[ignore]` → GREEN |
| `preedit_survives_a_fonts_generation_bump` | `#[ignore]`-RED | delete `#[ignore]` → GREEN |
| `label_reshapes_and_keeps_glyphs_after_a_bump` | GREEN (not ignored) | must stay GREEN |
| `empty_editor_emits_zero_glyphs_and_does_not_crash_on_bump` | GREEN (not ignored) | must stay GREEN |
| `editor_style_stays_live_after_a_bump` | `#[ignore]`-RED (C7 adds; see Task 4) | delete `#[ignore]` → GREEN |

The RED-first relationship (what each C2 production change un-blocks): the `sync.rs` content-skip turns `editor_content_survives_…` from RED→GREEN (without it, `value()` is clobbered to `""`); the style-only re-lower turns `editor_style_stays_live_…` GREEN (metrics re-applied); the `commit.rs` `shape_stale` term is proven NON-VACUOUSLY by a C2-owned directed unit test (`shape_stale_reshapes_a_committed_but_unshaped_buffer`, Task 5) that constructs a committed-but-unshaped buffer directly (a `reset_shaping()` with no `FontsGeneration` bump and no `Text` edit, so the TextSync auto-heal never runs) and asserts `text_commit` reshapes it — RED (`layout_runs().count()==0`) without the term, GREEN (`==1`) with it; the C7 end-to-end `label_reshapes_…`/`editor_style_stays_live_…` arms stay green as belt-and-suspenders regression guards but auto-heal, so they are NOT the isolating proof; the preedit safety turns `preedit_survives_…` RED→GREEN. Depends on C0 (umbrella, anchors §2.6) and C7 (the Tier-B harness + file must land first; C2's PR un-ignores). C4 consumes the editor seed/set via the existing `Insert` / `SelectAll` + `Insert` + the preserved `TextChanged` but does not block this. The `EditCommand` surface is owned by the agent-interface campaign (umbrella §2.7); C2 adds no `EditCommand` variant.

---

## PHASE 0 — Rebase + re-confirm anchors

**Why:** Implementation is GATED on the inspection-tools merge + a fresh rebase (umbrella §8). This worktree was authored against `507855f` (== `origin/main` at authoring time); the code blocks below MUST be re-confirmed against the rebased tree. All `file:line` anchors here are the `507855f` versions; re-grep and fix drift before writing any code.

- [ ] **Fetch + branch fresh from current origin/main.** Run:
  ```sh
  git -C /mnt/storage/projects/buiy fetch --all --prune
  git -C /mnt/storage/projects/buiy log --oneline -1 origin/main
  ```
  Confirm whether `origin/main` has advanced past `507855f` (the testing-audit #77 / CI-hardening #78 may have merged). Create the work branch from the remote ref, NOT a stale local:
  ```sh
  git -C /mnt/storage/projects/buiy branch c2-editor-text-integrity origin/main
  ```
  (If you are working inside this worktree, rebase it onto `origin/main` instead: `git rebase origin/main`.)

- [ ] **Integrate the merged inspection tools.** If the inspection-tools branch merged into `origin/main`, it is now in the tree from the step above — no extra action. Verify the text test substrate still builds: `crates/buiy_core/tests/support/extract_harness.rs` (`TextExtractHarness`, `glyph_count()`, `settle()`, `changed_frames()`) and `crates/buiy_core/tests/support/mod.rs` exist.

- [ ] **Confirm C7's Tier-B infrastructure is present (C2 DEPENDS on it; C2 does NOT create it).** The cross-plan contract makes C7 the sole creator of `bump_fonts_generation` and the Tier-B file. C7's PR must have landed before C2's. Verify:
  ```sh
  cd /mnt/storage/projects/buiy
  grep -n "pub fn bump_fonts_generation" crates/buiy_core/tests/support/extract_harness.rs
  ls crates/buiy_core/tests/text_font_reload_survival.rs
  grep -n "fn editor_content_survives_a_fonts_generation_bump\|fn preedit_survives_a_fonts_generation_bump\|fn label_reshapes_and_keeps_glyphs_after_a_bump\|fn empty_editor_emits_zero_glyphs_and_does_not_crash_on_bump\|fn editor_style_stays_live_after_a_bump" crates/buiy_core/tests/text_font_reload_survival.rs
  grep -cn 'ignore = "RED until C2' crates/buiy_core/tests/text_font_reload_survival.rs   # the RED tests C2 un-ignores
  ```
  All must be present. If C7 has NOT landed yet, STOP — C2's Task 4–6 un-ignore steps have nothing to un-ignore. (If the build sequencing put C2 first by necessity, escalate to the umbrella coordinator; do NOT recreate C7's file/method/tests — that violates the ownership contract.)

- [ ] **Re-grep every anchor cited in this plan and fix drift.** Run each and confirm the cited line still matches the quoted code:
  ```sh
  cd /mnt/storage/projects/buiy
  grep -n "generation.0 += 1" crates/buiy_core/src/text/registry.rs        # spec: registry.rs:543
  grep -n "fn apply_authored_to_buffer" crates/buiy_core/src/text/sync.rs   # spec: sync.rs:503
  grep -n "buffer.set_text(&directed" crates/buiy_core/src/text/sync.rs     # spec: sync.rs:530
  grep -n "let blocked = access.with_buffer_mut" crates/buiy_core/src/text/sync.rs  # spec: sync.rs:332
  grep -n "if !align_changed && !offset_stale && !size_stale" crates/buiy_core/src/text/commit.rs  # spec: commit.rs:102
  grep -n "let size_stale = access.with_buffer" crates/buiy_core/src/text/commit.rs # spec: commit.rs:101
  grep -n "pub fn with_buffer<T>" crates/buiy_core/src/text/edit/access.rs  # spec: access.rs:59
  grep -n "pub fn apply_tracked" crates/buiy_core/src/text/edit/input.rs    # spec: input.rs:94
  grep -n "EditCommand::SelectAll\|EditCommand::Insert" crates/buiy_core/src/text/edit/input.rs  # the existing seed/set verbs C2 uses (NO new variant)
  grep -n "pub enum EditCommand" crates/buiy_core/src/text/edit/command.rs  # spec: command.rs:21
  # Confirm the EditCommand surface state on the rebased tree. The agent-interface
  # campaign OWNS EditCommand: it adds SetSelection (P1c) and lowers Action::SetValue-text
  # via the existing SelectAll + Insert — it adds NO SetValue variant. C2 must NOT add one.
  grep -cn "EditCommand::SetValue\|SetValue(String)" crates/buiy_core/src   # MUST be 0 — C2 adds no SetValue variant (agent-interface-owned surface)
  grep -cn "EditCommand::SetSelection" crates/buiy_core/src                 # informational: present iff agent-interface P1c landed; C2 does not depend on it
  grep -cn "shape_stale" crates/buiy_core/src                               # MUST be 0 (the term C2 adds)
  ```
  If any anchor drifted, update the file:line references in the relevant task before editing. The exact bodies quoted below are the `507855f` versions; if a re-confirm step is flagged on a code block, diff it against the rebased file and adapt.

- [ ] **Confirm the cosmic-text 0.19.0 invariants this plan relies on** (these are load-bearing for the `shape_stale` design and the style-only path; re-confirm against the pinned source):
  ```sh
  CT=$(find ~/.cargo/registry/src -maxdepth 1 -type d -name 'cosmic-text-0.19.0' | head -1)
  sed -n '729,737p' "$CT/src/buffer.rs"        # set_metrics sets DirtyFlags::RELAYOUT (layout reset → layout_opt unset)
  sed -n '247,252p' "$CT/src/buffer.rs"        # LayoutRunIter::next: line.shape_opt()? / line.layout_opt()? — terminates at first unshaped line
  sed -n '128,136p' "$CT/src/buffer_line.rs"   # set_attrs_list calls reset_shaping() and returns false when unchanged
  sed -n '203,207p' "$CT/src/buffer_line.rs"   # reset_shaping resets shape + layout cache
  sed -n '511,515p' "$CT/src/attrs.rs"         # AttrsList::defaults()
  ```
  The design relies on: `set_metrics`/`set_wrap` set `RELAYOUT` (so `layout_opt` unsets → `layout_runs().count()` drops below `lines.len()` → that IS the `shape_stale` trigger); `set_attrs_list` resets shaping when the attrs differ. If cosmic offers no clean "reset shape, keep text," the `refresh_line_default_attrs` fallback (Task 4) is a content-preserving `set_text(line.text(), …)` reading the buffer's OWN current text (still no clobber).

- [ ] **Run the existing gate green as a baseline.** Establish that the text suite is green before touching it:
  ```sh
  cd /mnt/storage/projects/buiy
  cargo test -p buiy_core --test text_sync --test text_commit
  cargo test -p buiy_core --test text_mouse_selection --test text_edit_submit
  ```
  Expect all PASS. Record the count. (Full workspace gate runs at the end of the plan.)

---

## Tasks 1 & 2 — Confirm the editor seed/set channel uses the EXISTING `Insert` / `SelectAll` + `Insert` (NO new `EditCommand` variant)

**Reconciled to the agent-interface campaign (umbrella §2.7).** The original C2 plan added an `EditCommand::SetValue(String)` variant (old Task 1) and implemented its recorded, composition-cancelling apply (old Task 2). That is **superseded**: the agent-interface campaign **owns** the `EditCommand` surface and lowers `Action::SetValue`-text via the **existing** `SelectAll` + `Insert` (`action-router.md` §4; `phasing.md` P1c: "`SetValue`-text lowers fine now via `SelectAll` + `Insert`, both existing") — it adds **no** value-set variant. C2 therefore adds **no production change** to `command.rs`/`input.rs` here; it seeds the empty editor with the existing `Insert(initial)` and does a programmatic set with `SelectAll` + `Insert(new)`. This task is a **verification-only** task: a headless test that pins those existing verbs achieve seed/set, so C4's lifecycle (and the bare-`TextInput` migration, Task 7b) can rely on them.

**Files:**
- Test: `crates/buiy_core/tests/text_set_value.rs` (Create) — exercises ONLY the existing verbs; no production edit.
- **No edit to `crates/buiy_core/src/text/edit/command.rs` or `input.rs`** (no new variant, no new arm).

Steps:

- [ ] **Write the seed/set channel test using ONLY existing verbs.** Create `crates/buiy_core/tests/text_set_value.rs`:
  ```rust
  //! C2 — the editor's seed / programmatic-set channel is the EXISTING
  //! `EditCommand` verbs: `Insert` seeds an empty editor; `SelectAll` + `Insert`
  //! does a whole-value programmatic set. There is NO `EditCommand::SetValue`
  //! (the agent-interface campaign owns the `EditCommand` surface and lowers
  //! `Action::SetValue`-text via this same `SelectAll` + `Insert` — umbrella §2.7,
  //! action-router.md §4, phasing.md P1c).
  //!
  //! Headless: builds a `TextEditState` directly and applies via the public
  //! facade, locking the shared FontSystem the way `text_mouse_selection.rs`
  //! does (no LayoutPlugin needed — apply is a pure edit-path call).

  use buiy_core::text::SharedFontSystem;
  use buiy_core::text::edit::{EditCommand, TextEditState};

  fn editor(fonts: &SharedFontSystem, seed: &str) -> TextEditState {
      // for_font_size(16.0) == Metrics::new(16.0, 19.2) (the 1.2 line-height
      // scale, state.rs:143) — the ONE constructor form used across all plans.
      let mut state = TextEditState::for_font_size(16.0);
      let mut fs = fonts.lock();
      // SEED: a fresh editor is empty (one empty line, value()==""); a single
      // Insert into the empty buffer makes the whole value. This is the seed path.
      state.apply(&mut fs, EditCommand::Insert(seed.into()), false, false);
      drop(fs);
      state
  }

  /// Programmatic whole-value set via the EXISTING verbs: select-all, then insert
  /// (type-over). This is the exact lowering the agent-interface router applies
  /// for `Action::SetValue` on a text field (action-router.md §4).
  fn set_value(state: &mut TextEditState, fonts: &SharedFontSystem, new: &str) -> buiy_core::text::edit::EditOutcome {
      let mut fs = fonts.lock();
      state.apply(&mut fs, EditCommand::SelectAll, false, false);
      let outcome = state.apply(&mut fs, EditCommand::Insert(new.into()), false, false);
      drop(fs);
      outcome
  }

  #[test]
  fn select_all_plus_insert_replaces_the_whole_value() {
      let fonts = SharedFontSystem::new();
      let mut state = editor(&fonts, "old content");
      assert_eq!(state.value(), "old content");

      let outcome = set_value(&mut state, &fonts, "new value");

      assert_eq!(state.value(), "new value", "SelectAll + Insert replaces the entire logical value");
      assert!(outcome.value_changed, "a real value change flags value_changed → TextChanged");
  }

  #[test]
  fn insert_seeds_an_empty_editor() {
      let fonts = SharedFontSystem::new();
      // A bare editor is "" before any verb; seeding is a single Insert.
      let mut state = TextEditState::for_font_size(16.0);
      assert_eq!(state.value(), "", "a fresh editor is empty");
      let mut fs = fonts.lock();
      state.apply(&mut fs, EditCommand::Insert("seed text".into()), false, false);
      drop(fs);
      assert_eq!(state.value(), "seed text", "Insert into the empty editor seeds the whole value");
  }

  #[test]
  fn select_all_plus_insert_to_empty_clears_the_value() {
      let fonts = SharedFontSystem::new();
      let mut state = editor(&fonts, "content");
      set_value(&mut state, &fonts, "");
      assert_eq!(state.value(), "", "SelectAll + Insert(\"\") empties the editor");
  }

  #[test]
  fn set_value_via_existing_verbs_is_undoable() {
      // The programmatic set inherits the existing verbs' undo behavior (no new
      // grouping is added by C2). One Undo after SelectAll+Insert returns toward
      // the prior value via the existing recorded edits — assert the value is
      // restorable, not a specific undo-unit count (the count is the existing
      // verbs' behavior, agent-interface-owned, not a C2 contract).
      let fonts = SharedFontSystem::new();
      let mut state = editor(&fonts, "first");
      set_value(&mut state, &fonts, "second");
      assert_eq!(state.value(), "second");

      let mut fs = fonts.lock();
      // Undo enough to walk back past the type-over (the existing verbs each
      // record; loop until the value differs from "second" or the stack drains).
      for _ in 0..4 {
          if state.value() != "second" { break; }
          state.apply(&mut fs, EditCommand::Undo, false, false);
      }
      drop(fs);
      assert_ne!(state.value(), "second", "the programmatic set is undoable via the existing verbs");
  }
  ```
  Re-confirm at Phase 0: `EditCommand::SelectAll` and `EditCommand::Insert` exist (`command.rs`); `EditOutcome` is re-exported from `text::edit`; `apply` returns `EditOutcome { value_changed, .. }`. If `EditOutcome` is not publicly re-exported, drop the `set_value` helper's return type annotation and inline the two `apply` calls in the test, asserting `value()` and (where reachable) the second call's `value_changed`.

  **NOTE — no production change in this task.** Do NOT add `EditCommand::SetValue` to `command.rs` and do NOT add an `apply_tracked` arm. The agent-interface campaign owns the `EditCommand` surface; C2 only consumes the existing verbs. If a future need for a dedicated whole-value verb arises, it is the agent-interface campaign's call on the surface it owns.

  **Preedit during a programmatic set:** the `SelectAll` and `Insert` arms go through the existing `apply_tracked` path, which already handles a live composition the same way the agent-interface `Action::SetValue` lowering relies on. C2 adds no special composition handling here. (C2's preedit GUARANTEE is the §2.1 style-only path across a `FontsGeneration` bump — Task 6 — which is independent of any seed/set verb.)

- [ ] **Run & confirm PASS (no RED phase — these exercise existing, already-implemented verbs).**
  ```sh
  cargo test -p buiy_core --test text_set_value
  ```
  Expected: all four tests PASS against the existing `SelectAll`/`Insert` verbs (no production change needed). This pins the seed/set channel C4 and Task 7b rely on. (If `EditOutcome` re-export differs, apply the Phase-0 fallback above.)

- [ ] **Commit.**
  ```sh
  git add crates/buiy_core/tests/text_set_value.rs
  git commit -m "test(text): pin editor seed/set via existing Insert / SelectAll + Insert (no new verb)

The editor's content seed (Insert into the empty editor) and programmatic set
(SelectAll + Insert) use the EXISTING EditCommand verbs — the same lowering the
agent-interface router applies for Action::SetValue-text (umbrella §2.7,
action-router.md §4, phasing.md P1c). C2 adds NO EditCommand::SetValue variant
(the EditCommand surface is agent-interface-owned). Verification-only: no
production change to command.rs/input.rs.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Task 3 — `has_edit()` discriminant on `TextBufferAccessItem`

The content-vs-style split point: the accessor already binds `edit: Option<&mut TextEditState>`; expose whether an editor owns the buffer so `sync_one` can branch.

**Files:**
- Modify `crates/buiy_core/src/text/edit/access.rs:56` (add method to the `impl TextBufferAccessItem` block)
- Test: `crates/buiy_core/tests/text_edit_substrate.rs` (add a `has_edit` case) — re-confirm this file exists at Phase 0; if absent, add the test to `text_set_value.rs`.

Steps:

- [ ] **Write the failing test.** Append to `crates/buiy_core/tests/text_edit_substrate.rs` (the file that pins the editor-arm accessor; confirm its imports include `TextBufferAccess`, `TextEditState`, `TextBuffer`):
  ```rust
  #[test]
  fn has_edit_distinguishes_editor_from_display_entities() {
      use buiy_core::text::edit::{TextBufferAccess, TextEditState};
      use buiy_core::text::TextBuffer;
      use cosmic_text::Metrics;

      let metrics = Metrics::new(16.0, 19.2); // for the display TextBuffer
      let mut world = World::new();
      let display = world.spawn(TextBuffer::new(metrics)).id();
      // Editor uses the facade constructor (for_font_size == Metrics::new(16.0,
      // 19.2), state.rs:143) — the ONE form across all plans.
      let editor = world
          .spawn((TextBuffer::new(metrics), TextEditState::for_font_size(16.0)))
          .id();

      let mut q = world.query::<TextBufferAccess>();
      assert!(
          !q.get(&world, display).unwrap().has_edit(),
          "a display-only entity has no editor"
      );
      assert!(
          q.get(&world, editor).unwrap().has_edit(),
          "an entity with TextEditState owns its buffer"
      );
  }
  ```
  (If `text_edit_substrate.rs` does not exist post-rebase, place this test in `text_set_value.rs` with `use bevy::prelude::*;` added.)

- [ ] **Run & confirm FAIL.**
  ```sh
  cargo test -p buiy_core --test text_edit_substrate has_edit_distinguishes 2>&1 | head -30
  ```
  Expected: COMPILE ERROR — `no method named `has_edit` found for ... TextBufferAccessReadOnlyItem`. (The read-only companion is what `query::get` yields.) This is the RED signal.

- [ ] **Add `has_edit()` to BOTH accessor item forms.** The mutable item is used by `sync_one`; the read-only companion is what `query.get` yields in the test. Add to the `impl TextBufferAccessItem<'_, '_>` block (`access.rs:56`, after `with_buffer`):
  ```rust
      /// `true` when an editor owns this entity's authoritative buffer (the
      /// content-vs-style split point, C2 §2.1: editors own their content, so
      /// TextSync re-applies STYLE but never `set_text`s; display entities own
      /// neither and take the unchanged content+style path).
      pub fn has_edit(&self) -> bool {
          self.edit.is_some()
      }
  ```
  And the same method on the read-only companion `impl TextBufferAccessReadOnlyItem<'_, '_>` (`access.rs:114`, after its `with_buffer`):
  ```rust
      /// `true` when an editor owns this entity's authoritative buffer (read-only
      /// companion of [`TextBufferAccessItem::has_edit`]).
      pub fn has_edit(&self) -> bool {
          self.edit.is_some()
      }
  ```
  Re-confirm at Phase 0: the field is named `edit` (`access.rs:47`); the read-only companion struct name is `TextBufferAccessReadOnlyItem` (generated by `#[query_data(mutable)]`, named at `access.rs:114`).

- [ ] **Run & confirm PASS.**
  ```sh
  cargo test -p buiy_core --test text_edit_substrate has_edit_distinguishes
  ```
  Expected: PASS.

- [ ] **Commit.**
  ```sh
  git add crates/buiy_core/src/text/edit/access.rs crates/buiy_core/tests/text_edit_substrate.rs
  git commit -m "feat(text): add TextBufferAccessItem::has_edit() discriminant

The content-vs-style split point for the Bug-3 fix (C2 §2.1): exposes whether
a TextEditState owns the entity's authoritative buffer, so TextSync can route
editor entities to a style-only lowering that never set_text-clobbers their
typed content. Added on both the mutable item (sync_one) and the read-only
companion.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Task 4 — TextSync: style-only lowering for editor entities (Bug 3 fix) + un-ignore C7's content/style survival tests

The core data-loss fix. `sync_one` branches on `has_edit()`: editor entities take `apply_authored_style_to_editor_buffer` (re-applies metrics/wrap/tab-width + refreshes per-line default attrs, NEVER `set_text`); display entities keep `apply_authored_to_buffer`. **C2 owns this production change.** The Tier-B file, the `bump_fonts_generation` harness method, and the survival tests are **C7-owned** (the cross-plan ownership contract); C2 does NOT create them — it deletes the `#[ignore]` from C7's RED tests and asserts GREEN.

**Files:**
- Modify `crates/buiy_core/src/text/sync.rs:332` (branch `sync_one`)
- Modify `crates/buiy_core/src/text/sync.rs` (add `apply_authored_style_to_editor_buffer`, `refresh_line_default_attrs`, `style_block_flag` private fns)
- Modify `crates/buiy_core/tests/text_font_reload_survival.rs` (C7-OWNED — C2 only DELETES the `#[ignore]` attribute from `editor_content_survives_a_fonts_generation_bump` and `editor_style_stays_live_after_a_bump`; C2 does NOT create the file, the harness method, the seed helper, or any test body).

Steps:

- [ ] **Confirm the C7-owned RED tests exist and are RED on pre-fix main (NOT created by C2).** Before touching production code, capture the RED:
  ```sh
  cd /mnt/storage/projects/buiy
  # Run the two C7 RED tests via their #[ignore] override — they FAIL pre-fix.
  cargo test -p buiy_core --test text_font_reload_survival -- --ignored \
    editor_content_survives_a_fonts_generation_bump editor_style_stays_live_after_a_bump 2>&1 | tail -20
  ```
  Expected: `editor_content_survives_a_fonts_generation_bump` FAILS with `value == ""` vs `"Hello"` (the unfixed `sync_one` `set_text("")`s the editor-owned buffer via the editor-first accessor — the Bug-3 reproduction); `editor_style_stays_live_after_a_bump` FAILS on the metrics assert (no style re-applied yet). These are C7's tests using C7's `spawn_seeded_editor`/`bump_fonts_generation`; C2 reads them, does not author them.

  **If `editor_style_stays_live_after_a_bump` is absent in C7's landed file:** the ownership contract lists it as a C7-owned Tier-B test. Do NOT add it to the file from C2 — instead coordinate with C7 to land the style-survival arm (C7 owns the file) before C2's PR un-ignores it. The umbrella coordinator sequences this. (The contract's authoritative C7 Tier-B set is: `editor_content_survives_…`, `label_reshapes_…`, `empty_editor_emits_…`, `preedit_survives_…`, `editor_style_stays_live_…`.)

- [ ] **Branch `sync_one` on `has_edit()`.** In `sync.rs`, replace the `let blocked = access.with_buffer_mut(|buffer| { apply_authored_to_buffer(...) });` block (`sync.rs:332-342`) with:
  ```rust
      let is_editor = access.has_edit();
      let blocked = access.with_buffer_mut(|buffer| {
          if is_editor {
              // Bug-3 fix (§2.1): editor entities own their content. Re-apply the
              // SAME metrics/wrap/tab-width as the display path and refresh the
              // per-line default attrs, but NEVER set_text — that is the clobber.
              apply_authored_style_to_editor_buffer(
                  buffer,
                  &style,
                  ctx.registry,
                  ctx.index,
                  ctx.now,
                  single_line,
              )
          } else {
              apply_authored_to_buffer(
                  buffer,
                  text,
                  &style,
                  ctx.registry,
                  ctx.index,
                  ctx.now,
                  single_line,
              )
          }
      });
  ```

- [ ] **Add the style-only lowering fns.** In `sync.rs`, after `apply_authored_to_buffer` (`sync.rs:552`), add:
  ```rust
  /// Style-only re-lower onto an editor-owned buffer (Bug 3 fix, §2.1). Applies
  /// the SAME metrics/wrap/tab-width as [`apply_authored_to_buffer`] but
  /// PRESERVES the buffer's existing line text — `set_text` is the clobber, so
  /// it is never called. The editor owns its content (seeded via the existing
  /// `EditCommand::Insert`, programmatic-set via `SelectAll` + `Insert`);
  /// TextSync remains the sole writer of its STYLE.
  /// Returns the `font-display: block` flag (derived over the buffer's CURRENT
  /// first-line text), so a Block family still gates the editor.
  fn apply_authored_style_to_editor_buffer(
      buffer: &mut Buffer,
      style: &AuthoredStyle<'_>,
      registry: &FontRegistry,
      index: &mut FontMatchIndex,
      now: f64,
      single_line: bool,
  ) -> bool {
      buffer.set_metrics(style.metrics());
      let wrap = if single_line {
          cosmic_text::Wrap::None
      } else {
          resolve_wrap(style.white_space, style.text_wrap)
      };
      buffer.set_wrap(wrap);
      buffer.set_tab_width(DEFAULT_TAB_WIDTH);
      // Refresh each line's resolved default attrs (weight/family/decoration
      // bits) WITHOUT dropping its text. `set_attrs_list` resets the line's shape
      // cache so TextCommit reshapes it at the next lock site (it is also the
      // shape-unset that the §2.2 shape_stale guard catches and reshapes).
      refresh_line_default_attrs(buffer, style);
      // The block flag derives from resolving the CURRENT first-line text against
      // the authored family (a Block family must still gate the editor's paint).
      style_block_flag(buffer, style, registry, index, now)
  }

  /// Rewrite each `BufferLine`'s default attrs to the authored base attrs,
  /// preserving the line text. Reuses the `AttrsList::defaults()` precedent
  /// from `ime.rs` (the splice/remove path carries `defaults()` to keep resolved
  /// attrs across surgery). A v1 editor is a single default-attrs run; per-span
  /// rich-text editor refresh is a documented follow-up (C2 §7).
  fn refresh_line_default_attrs(buffer: &mut Buffer, style: &AuthoredStyle<'_>) {
      let base = style.attrs();
      for line in buffer.lines.iter_mut() {
          // Replace the line's attrs list with one defaulting to the new base.
          // set_attrs_list resets shaping when it differs (cosmic buffer_line.rs),
          // which is exactly the re-measure trigger we want; an unchanged base is
          // a no-op (no spurious reshape).
          let attrs_list = cosmic_text::AttrsList::new(&base);
          line.set_attrs_list(attrs_list);
      }
  }

  /// The `font-display: block` flag for an editor buffer: resolve the buffer's
  /// CURRENT first-line text against the authored family/weight (the content
  /// path derives `blocked` from `resolve_spans` over the lowered text — the
  /// editor has no `Text` to lower, so resolve over its own first line). An
  /// empty buffer (one empty line) resolves to `false` (nothing blocks).
  fn style_block_flag(
      buffer: &Buffer,
      style: &AuthoredStyle<'_>,
      registry: &FontRegistry,
      index: &mut FontMatchIndex,
      now: f64,
  ) -> bool {
      let first_line = buffer.lines.first().map(|l| l.text()).unwrap_or("");
      if first_line.is_empty() {
          return false;
      }
      resolve_spans(first_line, style.family, style.weight, registry, index, now).blocked
  }
  ```
  Re-confirm at Phase 0: `AttrsList`, `Buffer`, `cosmic_text::Wrap` are imported in `sync.rs` (`Attrs, Buffer, Family, Metrics, Weight` at `sync.rs:29` — add `AttrsList` to that `use` line, or spell `cosmic_text::AttrsList`); `resolve_wrap`, `DEFAULT_TAB_WIDTH`, `resolve_spans`, `AuthoredStyle::{attrs,metrics,family,weight,white_space,text_wrap}` all exist (`sync.rs:36,56,517,446,430`). `BufferLine::set_attrs_list` returns `bool` (we discard it). Note: `AuthoredStyle.white_space`/`text_wrap` are private fields of the struct in the same module — accessible here.

- [ ] **Un-ignore C7's content + style survival tests and assert GREEN.** In the C7-OWNED file `crates/buiy_core/tests/text_font_reload_survival.rs`, DELETE the `#[ignore = "RED until C2 …"]` attribute line above `editor_content_survives_a_fonts_generation_bump` and above `editor_style_stays_live_after_a_bump` (the un-ignore is the C2 verification, per the contract — NOT a manual hand-revert). Change nothing else in the file. Then run them in the DEFAULT (non-`--ignored`) lane — they are now collected:
  ```sh
  cargo test -p buiy_core --test text_font_reload_survival \
    editor_content_survives_a_fonts_generation_bump editor_style_stays_live_after_a_bump
  ```
  Expected: both PASS. Content survives the bump (the editor branch never `set_text`s); the style-only path re-applies the bumped metrics (C7's style-survival assert sees the updated `metrics_for_test()`). The two already-green C7 arms (`label_reshapes_…`, `empty_editor_emits_…`) stay green — they assert properties the editor branch preserves.

- [ ] **Run the existing TextSync suite to confirm no display-path regression.**
  ```sh
  cargo test -p buiy_core --test text_sync
  ```
  Expected: all PASS (the display path `apply_authored_to_buffer` is byte-unchanged; only the editor branch is new — and `text_sync.rs::fonts_generation_bump_sweeps_every_buffer` spawns display-only nodes, so `applied()` count is unchanged).

- [ ] **Commit.**
  ```sh
  git add crates/buiy_core/src/text/sync.rs crates/buiy_core/tests/text_font_reload_survival.rs
  git commit -m "fix(text): TextSync applies style-only to editor buffers (Bug 3)

The FontsGeneration sweep no longer clobbers an editor's typed content. sync_one
branches on TextBufferAccessItem::has_edit(): editor entities take a new
style-only path (apply_authored_style_to_editor_buffer) that re-applies
metrics/wrap/tab-width and refreshes per-line default attrs but NEVER set_text
(the clobber). Display entities keep the unchanged content+style path. The
editor's content channel is the existing Insert / SelectAll + Insert (no new
EditCommand variant — the EditCommand surface is agent-interface-owned).

Un-ignores C7's Tier-B editor_content_survives_a_fonts_generation_bump and
editor_style_stays_live_after_a_bump (delete the #[ignore]) — that RED→GREEN
transition is the C2 content/style-survival verification. The Tier-B file and
the bump_fonts_generation harness method are C7-owned; C2 only un-ignores.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Task 5 — text_commit: the `shape_stale` guard (Bug 2 fix)

The defense-in-depth net: a fourth short-circuit term that re-detects the exact mismatch extract asserts and reshapes at the commit lock site. After Task 4 the sweep no longer clobbers content, but `set_metrics`/`set_attrs_list`/a `reset_shaping()` set the line's shape/layout cache to `unused` (cosmic `buffer_line.rs:203-207`) — so `buffer.layout_runs().count()` drops below `computed.lines.len()` (`LayoutRunIter::next` does `line.shape_opt()?`/`line.layout_opt()?` and terminates at the first unshaped line). The short-circuit (`commit.rs:102`) gates on `align_changed || offset_stale || size_stale` only: an unshape does NOT move the content-box `size`/`offset` or `align`, so all three are false → `continue` → that buffer reaches extract unshaped, painting **zero glyphs** (silent-no-paint in release; `debug_assert` panic in debug — and a `debug_assert` may be compiled out, so `layout_runs().count()`/`glyph_count()` are the load-bearing observables). C2 owns the `commit.rs` production change.

**The isolating proof is a C2-OWNED directed unit test** (`crates/buiy_core/tests/text_commit.rs`), NOT a hand-revert and NOT the C7-owned font-reload file. The audit's CRITICAL gap (§2 Bug 2, Appendix-A.5) is that the end-to-end `FontsGeneration`-bump path **auto-heals** and "isolates nothing": the bump runs the `text_sync_buffers` sweep, which calls `tree.mark_dirty_for_entity(entity)` (`sync.rs:350`) → Taffy re-measures the leaf → the leaf reshapes **regardless of the `shape_stale` guard**. So C7's `label_reshapes_…` (display) is GREEN with OR without the term, and `editor_style_stays_live_…` asserts only `metrics_for_test()` (never `glyph_count()`). The `shape_stale` term could be deleted and every end-to-end test would still pass — a **vacuous green**.

The directed test sidesteps the auto-heal by constructing the committed-but-unshaped state **directly**, with NO `FontsGeneration` bump and NO `Text`/style-carrier mutation: settle a real text node (so it has a `LayoutTree` node, a committed `ComputedTextLayout` with `lines.len() == 1`, and a shaped buffer), then reach into `TextBuffer.buffer` and call `buffer.lines[0].reset_shaping()` (cosmic `buffer_line.rs:203` — `pub`; `buffer.lines` is `pub`, `buffer.rs:336`). That unsets the line's shape+layout cache so `layout_runs().count() == 0`, while `buffer.size()`, the per-line align, and the content-box offset are **all unchanged**. Because the mutation is a direct world `get_mut` (NOT a `FontsGeneration` bump and NOT a `Changed<Text>`/style edit), the `text_sync_buffers` sweep does **not** run (its triggers are `fonts_generation.is_changed()` OR the `Or<(Changed<Text>, …)>` set, `sync.rs:69,251-253`; `Changed<TextBuffer>` is explicitly NOT a trigger, pinned by `text_sync.rs`), so `mark_dirty_for_entity` is never called and **Taffy never re-measures** — the one thing that auto-heals the buffer is removed. On the next `app.update()` the ONLY system that can reshape this buffer is `text_commit`, and the ONLY guard term that can fire is `shape_stale` (size/align/offset are equal). **Without `shape_stale` this is RED** (`text_commit` short-circuits, the buffer stays unshaped, `layout_runs().count() == 0 != 1`); **with it, GREEN** (`layout_runs().count() == 1 == lines.len()`). This is the directed proof the guard's decision is non-vacuous.

The C7-owned end-to-end font-reload tests stay as **additional regression guards** (belt-and-suspenders): they prove the production trigger (a `FontsGeneration` bump) keeps content painting after the fix, but — because of the auto-heal — they are NOT the isolating proof of the `shape_stale` term. C2 does NOT add reshape/empty-editor tests to the C7-owned file; it un-ignores C7's RED arms (Tasks 4, 6) and adds its own directed unit test here.

**Files:**
- Modify `crates/buiy_core/src/text/commit.rs:98-104` (add the `shape_stale` term)
- Modify `crates/buiy_core/tests/text_commit.rs` (add the C2-OWNED directed `shape_stale` isolation test — this file is in C2's domain, tests `commit.rs`, needs no `PointerHarness`, and is NOT the C7-owned font-reload file)

Steps:

- [ ] **Write the failing directed test FIRST (RED before the guard).** Append to `crates/buiy_core/tests/text_commit.rs`. Every symbol the test uses is ALREADY in the file's head (verified at `text_commit.rs:6-12`): `Text`, `TextBuffer`, `ComputedTextLayout`, `TextCommitReshapeCount` (from `buiy_core::text`), `Style` (from `buiy_core::layout`), `Node` (from `buiy_core`), and the `text_app()`/`settle()` helpers. It uses `buffer.lines[0].reset_shaping()` and `buffer.layout_runs().count()` directly on the `pub` `TextBuffer.buffer: cosmic_text::Buffer` (`buffer.lines` is `pub`, `BufferLine::reset_shaping()` is `pub`) — **no new `use`** is needed (re-confirm the head imports at Phase 0):
  ```rust
  /// Bug-2 ISOLATION (C2 §2.2; audit §2 Bug 2, Appendix-A.5). The `shape_stale`
  /// guard's NON-VACUOUS proof: a buffer that was committed (has a
  /// `ComputedTextLayout`) but is then UNSHAPED — with its content-box size,
  /// per-line align, and content offset all UNCHANGED — must be reshaped by
  /// `text_commit`, because extract asserts `layout_runs().count() ==
  /// computed.lines.len()` (extract.rs:709).
  ///
  /// Why this isolates `shape_stale` where the end-to-end font-reload path
  /// CANNOT: the real `FontsGeneration` bump auto-heals via the
  /// `text_sync_buffers` sweep, which calls `tree.mark_dirty_for_entity`
  /// (sync.rs:350) → Taffy re-measures → the buffer reshapes regardless of the
  /// guard. This test removes that auto-heal entirely: it unshapes the buffer by
  /// a DIRECT `reset_shaping()` on the buffer line (NO FontsGeneration bump, NO
  /// Text/style-carrier edit), so the TextSync sweep never runs (its triggers are
  /// `fonts_generation.is_changed()` or the `Or<(Changed<Text>, …)>` set, sync.rs:69;
  /// `Changed<TextBuffer>` is NOT a trigger) and Taffy never re-measures. The ONLY
  /// system that can reshape the buffer on the next frame is `text_commit`, and the
  /// ONLY guard term that can fire is `shape_stale` (size/align/offset are equal) —
  /// so a PASS proves the `shape_stale` term did the reshape. WITHOUT the term this
  /// is RED (the buffer stays unshaped, `layout_runs().count() == 0`).
  #[test]
  fn shape_stale_reshapes_a_committed_but_unshaped_buffer() {
      let mut app = text_app();
      let text = app
          .world_mut()
          .spawn((Node, Style::default(), Text(String::from("hello"))))
          .id();
      app.world_mut()
          .spawn((
              Node,
              Style::default()
                  .flex_column()
                  .width_px(300.0)
                  .height_px(100.0),
          ))
          .add_child(text);
      settle(&mut app);
      app.update(); // flush any cascade remnant — reach a true steady state
                    // (the steady_state test's discipline, text_commit.rs:251):
                    // a plain frame here reshapes 0 buffers, so the post-unshape
                    // reshape below is attributable solely to shape_stale.

      // Precondition: committed and shaped. One line ("hello"), one layout run.
      let committed_lines = {
          let tb = app.world().get::<TextBuffer>(text).unwrap();
          let computed = app.world().get::<ComputedTextLayout>(text).unwrap();
          assert_eq!(
              tb.buffer.layout_runs().count(),
              computed.lines.len(),
              "precondition: settled buffer is shaped (runs == committed lines)"
          );
          assert_eq!(computed.lines.len(), 1, "single-line 'hello'");
          let size = tb.buffer.size();
          assert!(size.0.is_some() && size.1.is_some(), "both axes committed");
          computed.lines.len()
      };

      // Construct the committed-but-UNSHAPED state directly: reset the line's
      // shape+layout cache (cosmic buffer_line.rs:203). This is the EXACT
      // mismatch extract asserts — layout_runs() now terminates at the first
      // unshaped line — while buffer.size()/align/content_offset are all
      // unchanged (so commit's size/align/offset terms stay false). Mutating
      // via a direct get_mut does NOT bump FontsGeneration and does NOT touch
      // Text, so the TextSync sweep (the auto-heal) never runs.
      {
          let mut tb = app.world_mut().get_mut::<TextBuffer>(text).unwrap();
          tb.buffer.lines[0].reset_shaping();
          assert_eq!(
              tb.buffer.layout_runs().count(),
              0,
              "constructed RED state: buffer unshaped (runs=0) while committed lines=1"
          );
      }

      // One frame: TextSync does NOT sweep (no bump, no Text change), Taffy does
      // NOT re-measure (resolved size unchanged), so text_commit is the only
      // system that can reshape — and only via shape_stale.
      app.update();

      let tb = app.world().get::<TextBuffer>(text).unwrap();
      assert_eq!(
          tb.buffer.layout_runs().count(),
          committed_lines,
          "shape_stale must reshape the unshaped-but-committed buffer back to \
           layout_runs().count() == computed.lines.len() (WITHOUT the term this \
           is 0 — the silent-no-paint / debug_assert state at extract)"
      );
      assert_eq!(
          app.world().resource::<TextCommitReshapeCount>().0,
          1,
          "exactly one buffer reshaped this frame — the shape_stale-triggered reshape"
      );
  }
  ```

- [ ] **Run the directed test & capture the expected FAIL (RED, pre-guard) under nextest — the real profile.** With the `shape_stale` term ABSENT (the current `commit.rs:102` guards on `align_changed || offset_stale || size_stale` only):
  ```sh
  cd /mnt/storage/projects/buiy
  cargo nextest run -p buiy_core --test text_commit shape_stale_reshapes_a_committed_but_unshaped_buffer 2>&1 | tail -25
  ```
  Expected RED: the final `assert_eq!(tb.buffer.layout_runs().count(), committed_lines, …)` FAILS with `left: 0, right: 1` — `text_commit` short-circuited (all three existing terms false), so the buffer is still unshaped; `TextCommitReshapeCount == 0`. **This is the captured, profile-independent RED** (it asserts on `layout_runs().count()`, not on the `debug_assert`, which a release-ish nextest profile could compile out). The preconditions PASS (the settle shaped the buffer; the `reset_shaping()` unshaped it to runs=0), so the failure is precisely the missing reshape — proving the test is RED *for the right reason* and is NOT vacuous.

- [ ] **Add the `shape_stale` term.** In `commit.rs`, between the `size_stale` line (`commit.rs:101`) and the short-circuit `if` (`commit.rs:102`), insert the new term and extend the guard:
  ```rust
          let size_stale = access.with_buffer(|buffer| buffer.size() != target);
          // The reshape guard (Bug 2, §2.2): extract asserts
          // `layout_runs().count() == computed.lines.len()` (extract.rs:709). A
          // buffer unshaped AFTER its last commit (a FontsGeneration sweep's
          // set_metrics/attr-reset → DirtyFlags::RELAYOUT; a future Display::None
          // escape) leaves layout_runs() short of the committed line count, and
          // reaches extract unshaped (debug_assert panic / silent-no-paint in
          // release). Re-detect with the SAME comparison extract makes, so the two
          // cannot diverge. Gated on `existing_layout.is_some` — inert on a
          // never-committed buffer (zero added work to the first-commit path),
          // only an O(lines) walk on already-committed entities (the same walk
          // computed_outputs already runs on a reshape, commit.rs:156).
          let shape_stale = existing_layout.is_some_and(|computed| {
              access.with_buffer(|buffer| buffer.layout_runs().count() != computed.lines.len())
          });
          // § 4.2's steady-state short-circuit (+ the T4 offset term + the shape guard).
          if !align_changed && !offset_stale && !size_stale && !shape_stale {
              continue;
          }
  ```
  Remove the now-superseded original comment line `// § 4.2's steady-state short-circuit (+ the T4 offset term).` (`commit.rs:100`) — the new comment above the `if` covers it. Re-confirm at Phase 0: `existing_layout` is `Option<&ComputedTextLayout>` (`commit.rs:50` query binding); `ComputedTextLayout.lines` is a `Vec<ComputedTextLine>` (`components.rs:626`); `access.with_buffer` is `&self` (re-entrant read — `size_stale` already calls it, so a second read is fine).

- [ ] **Run the directed test & confirm it flips to GREEN (the isolating proof) — under nextest.**
  ```sh
  cargo nextest run -p buiy_core --test text_commit shape_stale_reshapes_a_committed_but_unshaped_buffer 2>&1 | tail -15
  ```
  Expected GREEN: the unshaped-but-committed buffer is reshaped — `layout_runs().count() == 1 == computed.lines.len()` and `TextCommitReshapeCount == 1`. The ONLY change between the captured RED above and this GREEN is the added `shape_stale` term (size/align/offset were equal in both runs), so the flip is *attributable to `shape_stale` alone* — the non-vacuous proof the audit requires. (Sanity check the isolation: temporarily scratch-delete just the `&& !shape_stale` conjunct and re-run — it goes RED again — then restore it. This is a one-off local check, NOT a committed hand-revert.)

- [ ] **Run the existing commit suite to confirm no steady-state regression.**
  ```sh
  cargo nextest run -p buiy_core --test text_commit
  ```
  Expected: all PASS, including the new `shape_stale_reshapes_a_committed_but_unshaped_buffer` and especially `steady_state_zero_measure_calls_and_zero_reshapes` (`text_commit.rs:230`) — the `shape_stale` walk runs but `layout_runs().count() == lines.len()` holds in steady state, so no reshape triggers; `TextCommitReshapeCount == 0` on the no-change frame.

- [ ] **Run the C7-owned end-to-end suite as the belt-and-suspenders regression guard.**
  ```sh
  cargo nextest run -p buiy_core --test text_font_reload_survival
  ```
  Expected: every C7 Tier-B test PASS — `label_reshapes_and_keeps_glyphs_after_a_bump` (display label still N glyphs), `empty_editor_emits_zero_glyphs_and_does_not_crash_on_bump` (stays 0-glyph, no panic — `shape_stale` does not false-positive on the empty synthetic-line case, where the synthetic empty `LayoutLine` keeps `layout_runs().count() == computed.lines.len() == 1`), plus the content/style/preedit survival tests un-ignored across Tasks 4 and 6. These prove the production `FontsGeneration`-bump path keeps painting content after the fix; they are NOT the isolating proof of `shape_stale` (they auto-heal via the TextSync sweep's `mark_dirty_for_entity` → Taffy re-measure, so they are GREEN with or without the term) — the directed `text_commit.rs` test above is that proof.

- [ ] **Commit** (production change + the C2-owned directed isolation test — the C7-owned test file is NOT touched by this task):
  ```sh
  git add crates/buiy_core/src/text/commit.rs crates/buiy_core/tests/text_commit.rs
  git commit -m "fix(text): text_commit shape_stale guard reshapes unshaped buffers (Bug 2)

A fourth steady-state short-circuit term re-detects the exact mismatch extract
asserts (layout_runs().count() != computed.lines.len()) and reshapes at the
commit lock site, so a buffer unshaped after commit (the FontsGeneration sweep's
set_metrics/attr-reset, a future Display::None escape) can never reach extract
unshaped — no silent-no-paint (zero glyphs) in release, no debug_assert panic in
debug. Gated on existing_layout.is_some so it adds zero work to the
never-committed path.

Isolating proof is the C2-owned directed unit test
shape_stale_reshapes_a_committed_but_unshaped_buffer (text_commit.rs): it
constructs a committed-but-unshaped buffer via a direct reset_shaping() — NO
FontsGeneration bump, NO Text edit — so the TextSync sweep never runs and Taffy
never re-measures (the auto-heal the end-to-end font-reload path cannot avoid,
audit Appendix-A.5). text_commit is then the only system that can reshape it,
via shape_stale alone: RED (runs=0) without the term, GREEN (runs==lines) with
it. The C7-owned end-to-end survival tests stay as belt-and-suspenders
regression guards (they auto-heal, so they are not the isolating proof).

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Task 6 — Un-ignore C7's preedit-survival test (the joint fix's third leg)

The style-only path is preedit-safe by construction (it never `set_text`s, so the spliced preedit bytes + the `PreeditSpan` record survive a bump). C7 owns the test `preedit_survives_a_fonts_generation_bump` (landed `#[ignore]`-RED). This task DELETES that `#[ignore]` and asserts GREEN — the C2-CONTRACT-4 gate. **No production change** (the style-only path from Task 4 already provides preedit safety); **no test authored by C2** (C7 owns the file + the test body).

**Files:**
- Modify `crates/buiy_core/tests/text_font_reload_survival.rs` (C7-OWNED — C2 only DELETES the `#[ignore = "RED until C2 …"]` attribute above `preedit_survives_a_fonts_generation_bump`).

Steps:

- [ ] **Confirm the C7-owned preedit test is RED pre-fix (via the `#[ignore]` override).** Before un-ignoring, capture the RED on a tree where Task 4 is NOT yet applied (or via a scratch revert of Task 4):
  ```sh
  cd /mnt/storage/projects/buiy
  cargo test -p buiy_core --test text_font_reload_survival -- --ignored preedit_survives_a_fonts_generation_bump 2>&1 | tail -20
  ```
  Expected (pre-fix): FAIL — the pre-fix clobber's `set_text("")` on the editor-owned buffer destroys both the committed text AND the spliced preedit run, so C7's assertion (the preedit `X`/`み` no longer in the buffer) fails. This is the double-corruption-during-composition reproduction. With Task 4 applied, the style-only path never `set_text`s, so it survives.

- [ ] **Un-ignore the preedit-survival test and assert GREEN.** In the C7-OWNED file `crates/buiy_core/tests/text_font_reload_survival.rs`, DELETE the `#[ignore = "RED until C2's preedit-aware TextSync fix lands (C2's PR un-ignores)"]` attribute line above `preedit_survives_a_fonts_generation_bump`. Change nothing else. Then run it in the default lane:
  ```sh
  cargo test -p buiy_core --test text_font_reload_survival preedit_survives_a_fonts_generation_bump
  ```
  Expected: PASS — the style-only path (Task 4) never `set_text`s, so C7's preedit survives the bump (its `with_buffer(|b| … contains('X'))` assertion holds; the `PreeditSpan` record is untouched because the splice bytes stay in the line). This is the C2-CONTRACT-4 verification — the RED→GREEN transition C2 owns.

- [ ] **Commit** (un-ignore only — C7 owns the test body):
  ```sh
  git add crates/buiy_core/tests/text_font_reload_survival.rs
  git commit -m "test(text): un-ignore C7 preedit-survival across a FontsGeneration bump (C2-CONTRACT-4)

Deletes the #[ignore] from C7's preedit_survives_a_fonts_generation_bump — the
RED→GREEN transition is C2's preedit-safety verification. The style-only TextSync
path (the Bug-3 fix) never set_text's, so a bump preserves both the spliced
preedit run and the PreeditSpan record (the pre-fix clobber double-corrupted both
during composition). The Tier-B file + test body are C7-owned; C2 only un-ignores.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Task 7 — Steady-state cost regression + programmatic-set undo assertion (existing verbs)

Lock in §3.4's cost claim (the `shape_stale` walk never triggers a reshape in steady state) and add a programmatic-set undo assertion (via the **existing** `SelectAll` + `Insert`, not a new `SetValue` verb) to the clipboard/undo suite, closing the verification matrix in §5 "Tests/snapshots touched."

**Files:**
- Modify `crates/buiy_core/tests/text_commit.rs:230` (extend `steady_state_zero_measure_calls_and_zero_reshapes`)
- Modify `crates/buiy_core/tests/text_clipboard_undo.rs` (add a `SelectAll` + `Insert` programmatic-set undo assertion) — re-confirm the file name at Phase 0 (`grep -l "undo_depth\|EditCommand::Undo" crates/buiy_core/tests/`).

Steps:

- [ ] **Extend the steady-state reshape test.** `text_commit.rs::steady_state_zero_measure_calls_and_zero_reshapes` (`text_commit.rs:230`) already asserts `TextCommitReshapeCount == 0` on the steady frame. It uses a display node, which exercises the same `shape_stale` walk path. Confirm the assertion message names the guard so the intent is documented; add a comment above the steady-frame `TextCommitReshapeCount` assert:
  ```rust
      // The shape_stale guard (C2 §2.2) WALKS layout_runs().count() here but must
      // not TRIGGER a reshape in steady state: layout_runs().count() ==
      // computed.lines.len() holds, so the short-circuit still fires (§3.4 cost).
  ```
  (If the existing assert already reads `app.world().resource::<TextCommitReshapeCount>().0, 0` — confirm by reading lines 230-265 — no logic change is needed; this is a documentation pin. If after rebase the test does NOT cover an already-committed buffer across a steady frame, add an editor variant: spawn an editor, settle, then assert zero reshape on the next steady `app.update()`.)

- [ ] **Add the programmatic-set undo assertion (existing verbs).** In the clipboard/undo test file, add a test that the `SelectAll` + `Insert` programmatic set is undoable via the existing verbs (C2 adds NO new undo grouping; the count is the existing verbs' behavior, agent-interface-owned, so assert *restorability*, not a specific unit count):
  ```rust
  #[test]
  fn programmatic_set_via_select_all_plus_insert_is_undoable() {
      use buiy_core::text::SharedFontSystem;
      use buiy_core::text::edit::{EditCommand, TextEditState};

      let fonts = SharedFontSystem::new();
      // for_font_size(16.0) — the ONE constructor form across all plans (state.rs:143).
      let mut state = TextEditState::for_font_size(16.0);
      let mut fs = fonts.lock();
      state.apply(&mut fs, EditCommand::Insert("seed".into()), false, false);
      // Programmatic whole-value set via the EXISTING verbs (the agent-interface
      // Action::SetValue lowering — action-router.md §4). No EditCommand::SetValue.
      state.apply(&mut fs, EditCommand::SelectAll, false, false);
      state.apply(&mut fs, EditCommand::Insert("whole new value".into()), false, false);
      assert_eq!(state.value(), "whole new value");
      // Undo walks back via the existing recorded edits; the set is reversible.
      for _ in 0..4 {
          if state.value() != "whole new value" { break; }
          state.apply(&mut fs, EditCommand::Undo, false, false);
      }
      assert_ne!(state.value(), "whole new value", "the programmatic set is undoable via the existing verbs");
      drop(fs);
  }
  ```

- [ ] **Run & confirm PASS.**
  ```sh
  cargo test -p buiy_core --test text_commit steady_state_zero_measure_calls_and_zero_reshapes
  cargo test -p buiy_core --test text_clipboard_undo programmatic_set_via_select_all_plus_insert_is_undoable
  ```
  Expected: both PASS. (If the second file is named differently, adjust `--test`.)

- [ ] **Commit.**
  ```sh
  git add crates/buiy_core/tests/text_commit.rs crates/buiy_core/tests/text_clipboard_undo.rs
  git commit -m "test(text): pin shape_stale steady-state cost + programmatic-set undo (existing verbs)

Documents that the shape_stale guard walks layout_runs() but never triggers a
reshape in steady state (C2 §3.4 cost claim), and asserts the programmatic
whole-value set (SelectAll + Insert — the agent-interface Action::SetValue
lowering, no new EditCommand verb) is undoable via the existing recorded edits.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Task 7b — Bare `TextInput` still seeds `""` (no regression from removing the `Text`→editor seam)

The Bug-3 fix removes content-lowering from `Text` onto the editor buffer (§2.1). The migration note (§5 step 5) asserts "the bare `TextInput` seeds `""`, so no behavior change" — but that is exactly the kind of claim that needs a test, because the fix changes the path that used to (incidentally) make a bare input's editor buffer empty. A bare `TextInput` is `(Text(""), TextEditState::for_font_size(16.0), …)` (`text_input.rs:77,80`): the editor's `for_font_size` → `new()` seeds one empty line, so `value()` is `""` at construction WITHOUT any explicit seed (no `Insert` needed for an empty field). This task pins that the empty seed survives a `FontsGeneration` bump (the same trigger Bugs 2/3 ride) — i.e. the style-only path leaves an empty editor empty, and no explicit seed verb is required for the empty case.

This closes the review's spec-gap: the bare-input "`""` unchanged" claim becomes a verified Wave-1 assertion, not a comment.

**Files:**
- Modify `crates/buiy_widgets/tests/text_input.rs` (extend the widget suite — `buiy_widgets` is where `TextInput` lives; this is the lowest tier that observes the bare-widget contract). The existing `single_line_text_input_composes_editor_markers_and_focusable` (`text_input.rs:13`) already asserts a FRESH input's `value() == ""`; this task adds the POST-BUMP survival arm.

Steps:

- [ ] **Add the post-bump bare-input survival test.** Append to `crates/buiy_widgets/tests/text_input.rs`:
  ```rust
  #[test]
  fn bare_text_input_value_stays_empty_across_a_fonts_generation_bump() {
      use buiy_core::text::FontsGeneration;

      let mut app = App::new();
      app.add_plugins(MinimalPlugins);
      app.add_plugins(buiy_core::CorePlugin);
      app.add_plugins(WidgetsPlugin);

      let entity = app.world_mut().spawn(TextInput::single_line("Search…")).id();
      app.update();
      assert_eq!(
          app.world().get::<TextEditState>(entity).unwrap().value(),
          "",
          "precondition: a bare TextInput seeds \"\" (no explicit seed verb needed for the empty case)"
      );

      // Bump FontsGeneration (the runtime add_font / system-font-scan trigger,
      // registry.rs:543) and run a frame. With the §2.1 style-only path, the
      // empty editor buffer is never set_text'd to anything else — it stays "".
      app.world_mut().resource_mut::<FontsGeneration>().0 += 1;
      app.update();

      assert_eq!(
          app.world().get::<TextEditState>(entity).unwrap().value(),
          "",
          "a bare TextInput's value stays \"\" after a bump — the Text->editor seam \
           removal (Bug-3 fix) introduces NO seed regression for the empty case (§5 step 5)"
      );
  }
  ```
  Re-confirm at Phase 0: `FontsGeneration` is re-exported from `buiy_core::text` (used the same way in the Tier-B harness); `WidgetsPlugin` pulls in `BuiyTextPlugin`/TextSync (so the bump's sweep actually runs over the widget's buffer) — if `WidgetsPlugin` alone does NOT register TextSync, add `buiy_core::text::BuiyTextPlugin` (confirm the plugin set the same way the existing `clicking_a_text_input_focuses_it` test composes plugins). If TextSync is not registered under this plugin set, the bump is inert and the test still passes vacuously — so additionally assert the buffer is reachable and empty via the editor (the value assert above already does).

- [ ] **Run & confirm PASS.**
  ```sh
  cargo test -p buiy_widgets --test text_input bare_text_input_value_stays_empty_across_a_fonts_generation_bump
  ```
  Expected: PASS — the bare input is `""` before AND after the bump (the empty editor needs no explicit seed verb and the style-only path never re-`set_text`s it).

- [ ] **Commit.**
  ```sh
  git add crates/buiy_widgets/tests/text_input.rs
  git commit -m "test(widgets): bare TextInput value stays \"\" across a FontsGeneration bump (§5 step 5)

Pins the migration claim that removing the Text->editor content seam (the Bug-3
fix) introduces no seed regression for the empty case: a bare TextInput
(Text(\"\") + TextEditState::for_font_size, no explicit seed verb) is \"\" at
construction and stays \"\" after a FontsGeneration bump (the style-only TextSync
path never re-set_text's the empty editor buffer). Closes the review's
bare-input spec-gap.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Task 8 — Export `EditCommand` through the prelude (closes the audit §4 gap)

`EditCommand` is `pub use`d from `buiy_core::text::edit` (`mod.rs:37`) but is NOT in the top-level `buiy` prelude — the audit §4 prelude gap. The editor's seed/programmatic-set channel is the existing `EditCommand` verbs (`Insert` for the seed, `SelectAll` + `Insert` for a programmatic set — C2 adds no `SetValue` variant; the `EditCommand` surface is agent-interface-owned, umbrella §2.7), and apps and C4's `TextField` drive them through `EditCommand`, so the type belongs in the prelude.

> **Coordination note:** if the agent-interface campaign's P1c (which adds `EditCommand::SetSelection` and owns the `EditCommand` surface) has already added an `EditCommand` prelude re-export by the time C2 lands, this task is a **no-op** — confirm the re-export exists and the smoke test passes, then skip the edit. Do NOT duplicate the re-export.

**Files:**
- Modify `crates/buiy/src/lib.rs:42` (the `buiy_widgets`/edit re-export block) OR the `buiy_core::text` re-export block (`crates/buiy/src/lib.rs:36-41`)
- Test: `crates/buiy/tests/` smoke (Create or extend) — a compile-only `use buiy::prelude::*;` reference.

Steps:

- [ ] **Write the failing prelude smoke test.** Create `crates/buiy/tests/prelude_edit_command.rs` — name an **existing** verb through the prelude (NOT `SetValue`, which does not exist):
  ```rust
  //! C2 §5 step 1 / audit §4: EditCommand is reachable from the `buiy` prelude —
  //! the editor's seed/set verbs (Insert, SelectAll) apps/C4 call. C2 adds no
  //! SetValue variant (agent-interface-owned EditCommand surface, umbrella §2.7).
  #[test]
  fn edit_command_is_in_the_prelude() {
      use buiy::prelude::*;
      // Compile-only: name the existing seed verb through the prelude path.
      let _cmd: EditCommand = EditCommand::Insert(String::from("x"));
      let _sel: EditCommand = EditCommand::SelectAll;
  }
  ```

- [ ] **Run & confirm FAIL** (skip if the agent-interface re-export already landed — see the coordination note; then the test PASSES with no edit).
  ```sh
  cargo test -p buiy --test prelude_edit_command 2>&1 | head -20
  ```
  Expected (if not yet exported): COMPILE ERROR — `cannot find type/value `EditCommand` in this scope` (or `unresolved import`). RED.

- [ ] **Add the re-export** (skip if already present from the agent-interface P1c). In `crates/buiy/src/lib.rs`, extend the `buiy_core::text` re-export block (`crates/buiy/src/lib.rs:36`, the `text::{ ... }` group) to add the edit surface, OR add a dedicated line after it. Add:
  ```rust
  pub use buiy_core::text::edit::{EditCommand, TextChanged};
  ```
  immediately after the closing of the `buiy_core::{ ... }` block (`crates/buiy/src/lib.rs:43`). Re-confirm at Phase 0: `buiy_core::text::edit` is public (`text/mod.rs:24 pub mod edit`) and `EditCommand`/`TextChanged` are re-exported from it (`edit/mod.rs:37,42`). `TextChanged` is added too because C4's controlled-`TextField` consumes the message this fix keeps honest (§4.1), and it pairs naturally with the edit verbs.

- [ ] **Run & confirm PASS.**
  ```sh
  cargo test -p buiy --test prelude_edit_command
  ```
  Expected: PASS.

- [ ] **Commit.**
  ```sh
  git add crates/buiy/src/lib.rs crates/buiy/tests/prelude_edit_command.rs
  git commit -m "feat(buiy): re-export EditCommand + TextChanged through the prelude

Closes the audit §4 prelude gap: the editor's seed/set channel is the existing
EditCommand verbs (Insert, SelectAll + Insert — no new SetValue variant; the
EditCommand surface is agent-interface-owned), which apps and C4's TextField
drive through EditCommand, so the type belongs next to the widget surface in
`use buiy::prelude::*;`. TextChanged pairs with it (C4 consumes the message C2
keeps honest). No-op if the agent-interface P1c already re-exported EditCommand.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

---

## Task 9 — Full gate + spec/doc flip

Run the project's full check command, then flip the C2 spec status and the docs catalog to reflect the landed work (CLAUDE.md "doc updates ship WITH the change").

**Files:**
- Modify `docs/specs/2026-06-22-buiy-widget-catalog-design/editor-text-integrity.md` (status `[draft]` → `[active]`/landed note)
- Modify `docs/README.md` (catalog entry, if the C2 child is listed with a status)

Steps:

- [ ] **Run the full workspace gate** (mirrors CI, CLAUDE.md `## Build & Test`):
  ```sh
  cd /mnt/storage/projects/buiy
  cargo fmt --all -- --check && \
    cargo clippy --workspace --all-targets -- -D warnings && \
    RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
    xvfb-run -a cargo test --workspace
  ```
  Expected: all green. If the test step link-OOMs, add `-j 2` to the `cargo test` step. Fix every clippy/fmt/doc warning before proceeding (no `#[allow]` band-aids without a comment justifying them).

- [ ] **Run the GPU lane is NOT required for C2** (C2 touches no render GPU path — the Tier-B suite is adapterless). Note in the PR body that the `#[ignore]` lane is unaffected by C2. (No command needed; this is a scope note.)

- [ ] **Flip the C2 spec status.** The spec's §2.3 was reconciled (2026-06-22) to the agent-interface campaign: it no longer adds an `EditCommand::SetValue` variant — the editor seed/set uses the **existing** `Insert` / `SelectAll` + `Insert`, matching the agent-interface `Action::SetValue` lowering (umbrella §2.7). In `editor-text-integrity.md`, change the header `[draft]` to a landed marker and add a one-line note under §2 that the fix landed (cite the branch/PR). Do NOT delete the design rationale (CLAUDE.md "supersede, don't silently contradict"), and do NOT reintroduce an `EditCommand::SetValue` variant. Confirm the spec carries no stale `SetValue`-variant claim through rebase:
  ```sh
  # The spec must NOT claim it ADDS a SetValue variant. Remaining matches must be
  # `Action::SetValue` (the agent-interface action) or explicit "no new
  # EditCommand::SetValue" / rejected-runner-up prose — NOT a "C2 adds SetValue".
  grep -n "EditCommand::SetValue" docs/specs/2026-06-22-buiy-widget-catalog-design/editor-text-integrity.md
  ```
  If any line claims C2 ADDS the variant (a rebase reintroduced the drift), re-apply the reconciliation: drop the variant, use the existing `Insert` / `SelectAll` + `Insert` seed/set path, and keep the §8 "Coordination with the agent-interface campaign" section.

- [ ] **Update the docs catalog** if `docs/README.md` tracks per-child status for the widget-catalog campaign. Mark C2 as implemented.

- [ ] **Commit the doc flip.**
  ```sh
  git add docs/specs/2026-06-22-buiy-widget-catalog-design/editor-text-integrity.md docs/README.md
  git commit -m "docs(text): mark C2 editor-text-integrity landed

Bugs 2+3 fixed together (TextSync editor content-skip + text_commit shape_stale
guard + preedit safety). The editor seed/set uses the existing Insert / SelectAll
+ Insert (no new EditCommand::SetValue — the EditCommand surface is
agent-interface-owned, umbrella §2.7). C7's Tier-B survival suite (C7-owned file
+ harness) un-ignored by C2 as it goes GREEN. Spec status flipped; design
rationale and the §8 agent-interface coordination section preserved.

Co-Authored-By: Claude Opus 4.8 (1M context) <noreply@anthropic.com>"
  ```

- [ ] **Request review** per the umbrella §5 Wave-1 gate (fresh-context agents: logic/correctness + spec-alignment). Confirm the RED→GREEN transitions were demonstrated before merging: C7's `editor_content_survives_…`, `editor_style_stays_live_…`, and `preedit_survives_…` un-ignored and GREEN; C7's `label_reshapes_…` green with the guard (its RED is `glyph_count()==0`); C2's `text_set_value.rs` (existing-verb seed/set), `bare_text_input_value_stays_empty_…`, and `prelude_edit_command` GREEN. Confirm C2 added **no** `EditCommand::SetValue` variant (the `EditCommand` surface is agent-interface-owned, umbrella §2.7).

---

## Verification matrix (C2 §6 → tasks)

All Tier-B tests are **C7-owned** (file `crates/buiy_core/tests/text_font_reload_survival.rs`, harness method `bump_fonts_generation`). C2's action is the un-ignore (delete `#[ignore]`) + assert GREEN, NOT recreation. C2-owned tests are `text_set_value.rs` (the editor seed/set via the **existing** `Insert` / `SelectAll` + `Insert` — no new `EditCommand` variant; the `EditCommand` surface is agent-interface-owned, umbrella §2.7), `text_edit_substrate.rs` (the `has_edit` arm), `text_commit.rs` extensions (including the directed `shape_stale_reshapes_a_committed_but_unshaped_buffer` isolation test — the non-vacuous proof of the guard), `text_clipboard_undo.rs`, `crates/buiy_widgets/tests/text_input.rs` (the bare-input arm), and `crates/buiy/tests/prelude_edit_command.rs`.

| Contract | Test (owner) | Task | C2 action / RED-proof |
|---|---|---|---|
| C2-CONTRACT-1 content survival | `editor_content_survives_a_fonts_generation_bump` (C7) | 4 | delete `#[ignore]` → GREEN; pre-fix RED: `value()` = `""` (sweep clobber) |
| C2-CONTRACT-2 style survival | `editor_style_stays_live_after_a_bump` (C7) | 4 | delete `#[ignore]` → GREEN; pre-fix RED: metrics stale (blanket skip) |
| **C2-CONTRACT-3 reshape guard (ISOLATING proof)** | **`shape_stale_reshapes_a_committed_but_unshaped_buffer` (C2, `text_commit.rs`)** | **5** | **directed: settle → `reset_shaping()` (no bump, no Text edit) → `app.update()`. RED without the term (`layout_runs().count()==0 != 1`, `TextCommitReshapeCount==0`); GREEN with it (`==1`). Bypasses the auto-heal (no TextSync sweep ⇒ no `mark_dirty` ⇒ no Taffy re-measure), so the flip is attributable to `shape_stale` alone.** |
| C2-CONTRACT-3 reshape guard (regression, belt-and-suspenders) | `label_reshapes_and_keeps_glyphs_after_a_bump` (C7, green-on-main) | 5 | stays GREEN with guard; NOT isolating (auto-heals via the TextSync sweep regardless of the term) — the directed test above is the isolating proof |
| C2-CONTRACT-3 edge (empty 0-vs-1) | `empty_editor_emits_zero_glyphs_and_does_not_crash_on_bump` (C7, green-on-main) | 5 | stays GREEN; guards no false-positive on the empty synthetic-line case |
| C2-CONTRACT-4 preedit survival | `preedit_survives_a_fonts_generation_bump` (C7) | 6 | delete `#[ignore]` → GREEN; pre-fix RED: preedit destroyed (clobber) |
| C2-CONTRACT-5 seed/set channel (existing verbs, NO new variant) | `text_set_value.rs` (4 cases over `Insert` / `SelectAll` + `Insert`, C2) + `programmatic_set_via_select_all_plus_insert_is_undoable` (C2) | 1&2, 7 | exercises existing verbs (PASS, no production change); the editor seed/set matches the agent-interface `Action::SetValue` lowering |
| §5 step 5 bare-input no-regression | `bare_text_input_value_stays_empty_across_a_fonts_generation_bump` (C2, widgets) | 7b | bare `TextInput` is `""` before AND after a bump (no explicit seed verb needed) |
| §3.4 cost | `steady_state_zero_measure_calls_and_zero_reshapes` (C2 extends) | 7 | n/a (no-regression) |
| audit §4 prelude gap | `prelude_edit_command` (C2) | 8 | unresolved import |
