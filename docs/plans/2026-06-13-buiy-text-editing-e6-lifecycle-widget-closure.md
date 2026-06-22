# Buiy text-editing E6 — Focus/lifecycle + placeholder + auto-scroll + widget + closure

**Date:** 2026-06-15
**Status:** landed
**Phase:** E6 (the FINAL phase of the E1–E6 text-editing campaign)
**Branch:** `text-editing-e6` (off `main`, which now includes E1 + E2 + E3 + E4 + E5)
**Campaign plan:** [2026-06-13-buiy-text-editing-campaign.md](2026-06-13-buiy-text-editing-campaign.md) § "E6 — Focus/lifecycle + placeholder + auto-scroll + widget + closure"
**Spec:** [editing-and-ime.md](../specs/2026-06-09-buiy-text-rendering-design/editing-and-ime.md) §§ 9 (auto-scroll via `ScrollOffset`), 10 (focus & lifecycle + placeholder), 11 (the full Message taxonomy + `EditSubmitted`), 2.3 (crate split — the `TextInput` widget is `buiy_widgets`), 13 (the v1 slice checklist — E6 ticks the LAST items).
**Readiness:** [2026-06-13-text-editing-design-readiness.md](../reports/2026-06-13-text-editing-design-readiness.md)

---

## Goal

Close the editor surface. E6 delivers the **lifecycle wiring** that makes the
already-built editor parts (caret, blink, IME, undo, preedit) behave correctly
across focus transitions; the **placeholder** rendering; **auto-scroll-into-view**;
the host-facing **`EditSubmitted`** Message that finalizes the § 11 taxonomy; the
`buiy_widgets::TextInput::new(...)` bundle that composes core mechanism with widget
policy; and the **campaign closure** (errata fold + spec README flip + docs/README +
follow-ups), grep-verified like the T9 closure.

E6 delivers, across `crates/buiy_core/src/text/edit/`, `crates/buiy_widgets/`, and
the docs tree:

1. **Focus lifecycle** (spec § 10) — a new `focus_lifecycle` system that detects the
   `FocusedEntity` transition (gain / loss) via a `Local<Option<Entity>>`
   previous-value compare (no such detector exists today — confirmed by grounding),
   plus an M1 change making `write_caret_blink` the single focus-aware owner of caret
   visibility:
   - **`write_caret_blink` (M1)** forces a non-focused editor caret steady-hidden and
     blinks only the focused one — caret show/hide is the blink writer's, NOT
     `focus_lifecycle`'s (they run in different windows; the blink writer runs LATER
     and drives `CaretVisual.visible` unconditionally, so it must own visibility or it
     clobbers any earlier write). Bare carets / harnesses without a `FocusedEntity`
     keep the pre-E6 global phase (the E3/E5 goldens depend on it). This also fixes a
     latent pre-E6 bug: a blurred E3/E5 caret never stopped blinking.
   - **gain** of a non-`Disabled` editor: blink ORIGIN reset (the focused caret is
     solid-on for one half-period). `ime_enabled` is already handled by E5's
     `write_ime_window` from focus + markers alone — E6 does **not** duplicate it.
   - **loss** of an editor: `seal()` the open undo group + `remove_preedit` (wiring
     E5's deferred focus-loss removal — E5's `apply_ime` only removes on
     `Ime::Disabled`, not on a focus change with no Ime event) + the **M1 dirty-mark**
     (`invalidate_intrinsics()` + `mark_dirty_for_entity` — `remove_preedit` does
     neither, so without it the orphan preedit glyphs persist a frame).
     **Selection / buffer RETAINED** (web parity — we do NOT touch `SelectionVisual`).
2. **Placeholder rendering** (spec § 10, decoration-and-paint § 7) — a
   `sync_placeholder` system maintaining a display-only `PlaceholderBuffer` shaped
   from the `Placeholder` string in the field's own style, shown through the
   **existing** glyph producer in the `color.text.placeholder` token iff
   `state.value().is_empty() && !state.has_preedit()`. The string never enters the
   editor buffer or undo history.
3. **Auto-scroll-into-view** (spec § 9) — an `auto_scroll_caret` system clamping the
   caret rect into the node's content-box viewport via the layout `ScrollOffset`
   (x single-line / y multi-line), after each caret move / edit; `PageUp`/`PageDown`
   already lower to `Motion::PageUp/PageDown` in E2 — auto-scroll follows the caret
   they move, so no separate Page handling is needed.
4. **`EditSubmitted`** (spec § 11) — the host-facing Message, emitted from
   `apply_keyboard_edits` when `EditOutcome.submitted` (the internal flag E2 built).
5. **`buiy_widgets::TextInput::new(...)`** — the `impl Bundle` (the `Button::new`
   precedent) composing the core editor + markers + focus + node/style + catalog
   tokens, plus a widget-side `focus_on_click` system (focus-on-click is widget
   policy, never core auto-focus — spec § 2.3 / Borrow #7). A core-side
   `TextEditState::for_font_size(f32)` constructor keeps cosmic-text contained to
   `buiy_core` (the facade boundary): `buiy_widgets` never names `cosmic_text`.
6. **One additive `#[ignore]` GPU golden** — a `TextInput`-shaped composed bundle
   (placeholder when empty vs ink when typed), **build-only** in this phase. It lives
   in `crates/buiy_core/tests/` (not `buiy_widgets`) to avoid the dependency cycle
   (`buiy_widgets` → `buiy_core`; the GPU harness is a `buiy_core` test target).
7. **Campaign closure** (docs-only, grep-verified) — fold E1/E4/E5 errata into the
   spec section files as "As landed" notes, flip the spec README Status to
   "implemented (editing, E1–E6)", update `docs/README.md` catalog statuses, and file
   the named deferrals in `docs/plans/follow-ups.md`.

Everything that names a cosmic `Editor`/`Edit`/`Action`/`Change`/`Attrs` type stays
**inside `crates/buiy_core/src/text/edit/`** (`tests/text_facade_boundary.rs` fails
the build otherwise). The new E6 core systems name only the pure-data `Cursor`/
`Buffer`/`Metrics` types (allowed) or no cosmic type at all. `buiy_widgets` names
**zero** cosmic types — the `TextEditState::for_font_size` core constructor is the
seam that keeps it so.

## Architecture

```
   FocusedEntity (focus.rs, a Resource(Option<Entity>))
            │
            ▼  Local<Option<Entity>> previous-value compare  (the ONLY transition detector — none existed)
   focus_lifecycle  (Update, .after(BuiySet::Input).before(write_caret_blink) — the render-prep window)
            │   on LOSS of prev editor: state.undo.seal(); state.remove_preedit(fs) + M1 dirty-mark
            │                           selection/buffer RETAINED (SelectionVisual untouched)
            │   on GAIN of new editor:  state.blink.reset(now)   (caret visibility is NOT touched here)
            ▼
   ┌──────────────── the render-prep editor window (between Input and write_caret_blink) ─────────────┐
   │ write_caret_and_selection (E3) → CaretVisual / SelectionVisual / PreeditVisual + blink reset      │
   │ auto_scroll_caret (E6, .after(write_caret_and_selection)) → clamp caret into content box →        │
   │       ScrollOffset{x|y}  (does NOT invalidate Taffy — layout/components.rs:509-526 invariant)     │
   │ write_ime_window (E5) → Window.ime_enabled / ime_position                                          │
   │ focus_lifecycle (E6) → undo seal + preedit removal (+ dirty-mark) + blink-origin reset            │
   └────────────────────────────────────────────────────────────────────────────────────────────────────┘
            │
            ▼  (LATER — .after(BuiySet::Animate).before(Picking))
   write_caret_blink (E6 M1: now the SINGLE focus-aware owner of CaretVisual.visible)
            │   editor caret + focus resource present: hide unless it is the FocusedEntity
            │   focused editor: per-entity blink phase;  bare caret / no focus infra: global phase (unchanged)

   sync_placeholder (Update, BuiyLayoutStep::TextSync — alongside text_sync_buffers):
            Placeholder + TextEditState  → shape Placeholder.0 into PlaceholderBuffer (display-only)
            gate: value().is_empty() && !has_preedit()  → extract paints it in color.text.placeholder

   apply_keyboard_edits (E2, BuiySet::Input):  EditOutcome.submitted ⇒ EditSubmitted(entity)   (E6 adds the writer)

   buiy_widgets::TextInput::new(label) -> impl Bundle:
        TextEditState::for_font_size(16.0)  (core constructor — no cosmic_text in buiy_widgets)
        + SingleLine? + Placeholder + Node + Style(.overflow_hidden) + FontSize + TextColor
        + Focusable + A11yRole::Text + A11yLabel  + Text("")  (the display TextBuffer carrier)
   buiy_widgets::focus_on_click (Update, BuiySet::Input): Hovered + mouse just_pressed ⇒ FocusedEntity = e
```

**Scheduling rationale (grounded).** The render-prep editor window is
`.after(BuiySet::Input).before(write_caret_blink)` (where E3's
`write_caret_and_selection` and E5's `write_ime_window` already live —
`text/mod.rs:195-214`). E6's `focus_lifecycle` and `auto_scroll_caret` join it.
`auto_scroll_caret` must run `.after(write_caret_and_selection)` so it reads the
caret the E3 writer just published, and the `ScrollOffset` it writes is consumed by
the transform bridge `seed_scroll_dirty`/`write_buiy_transform` which run
`.after(BuiySet::Animate)` (`lib.rs:107-128`) — i.e. AFTER this window — so the
offset takes effect the **same frame** (confirmed by grounding). `sync_placeholder`
runs in `BuiyLayoutStep::TextSync` alongside `text_sync_buffers` so the placeholder
buffer is recorded and committed (by `TextCommit`) before extract.

## Tech stack

- **Rust** + **Bevy 0.18.1** (`Message` = buffered event; `add_message::<T>()`;
  `MessageWriter`/`MessageReader`).
- **cosmic-text 0.19** — `Cursor`, `Buffer`, `Metrics` (pure data; named only inside
  `text::edit`). `buiy_widgets` does **not** depend on cosmic-text.
- The layout **`ScrollOffset`** (`crates/buiy_core/src/layout/components.rs:509-526`)
  — `{ x: f32, y: f32 }`, mutated directly; does NOT invalidate Taffy (invariant test
  `tests/layout_scroll_offset_no_invalidate.rs`). Consumed by the transform bridge
  on a **scroll-container** node (the field's `Style` must be `.overflow_hidden()` —
  wait: see Task 3 note; auto-scroll writes the offset and the test asserts the value,
  not the visual pan, so the headless gate is overflow-agnostic).
- The **`color.text.placeholder`** token + `TextColor::placeholder()`
  (`render/components.rs:62`) + the themed value (`theme.rs:83`) — all built.
- The GPU harness `crates/buiy_core/tests/support/mod.rs`
  (`gpu_render_app` / `finish_and_run` / `register_fixture_font` /
  `render_to_image` / `spawn_capture_camera` / `wait_for_text_ready` /
  `readback_rgba`; `render/golden.rs` `GoldenConfig` / `perceptual_diff`).

---

## How to work this plan

Each task is **failing test → run-it-fail → minimal impl → run-it-pass → commit**.
Run from the repo root.
The headless gate command (no GPU, what CI runs):

```sh
cargo test -p buiy_core --test <file> -- <test_name>      # the focused loop
```

Before each commit, the per-crate quick gate:

```sh
cargo fmt -p buiy_core -p buiy_widgets && \
  cargo clippy -p buiy_core -p buiy_widgets --all-targets -- -D warnings
```

The GPU golden (Task 8) is `#[ignore]` and **build-only** in this phase — verify it
compiles (`cargo test -p buiy_core --no-run`); the orchestrator runs the GPU lane.

**Commit discipline:** one commit per task, message
`feat(text-editing): E6 Task N — <summary>` (Task 9 is `docs(text-editing): E6 closure — …`).
End each commit body with the `Co-Authored-By` trailer.

---

## Task 1 — `EditSubmitted` Message (finalize the § 11 taxonomy)

The internal `EditOutcome.submitted` flag exists (E2, `input.rs:30`, set on
single-line Enter / `EditCommand::Submit`); E6 surfaces it as the host-facing
Message. This is the smallest taxonomy task — do it first so the audit test (Task 7)
has all rows.

### Step 1.1 — failing test

Create `crates/buiy_core/tests/text_edit_submit.rs`:

```rust
//! E6 Task 1 — the host-facing `EditSubmitted` Message: a single-line editor
//! whose focused Enter resolves to `EditCommand::Submit` emits exactly one
//! `EditSubmitted(entity)` (editing-and-ime § 11). A multi-line editor's Enter
//! inserts a newline and emits NO `EditSubmitted`.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::prelude::*;
use buiy_core::text::BuiyTextPlugin;
use buiy_core::text::edit::{EditSubmitted, SingleLine, TextEditState};
use buiy_core::{FocusedEntity, Node};
use buiy_core::focus::FocusPlugin;
use cosmic_text::Metrics;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(FocusPlugin)
        .add_plugins(BuiyTextPlugin::default());
    app.init_resource::<ButtonInput<KeyCode>>();
    app
}

fn press_enter(app: &mut App, window: Entity) {
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Enter,
        logical_key: Key::Enter,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
}

fn submitted_count(app: &mut App) -> usize {
    let messages = app.world().resource::<Messages<EditSubmitted>>();
    let mut cursor = messages.get_cursor();
    cursor.read(messages).count()
}

#[test]
fn single_line_enter_emits_one_edit_submitted() {
    let mut app = app();
    let window = app.world_mut().spawn(()).id();
    let editor = app
        .world_mut()
        .spawn((Node, TextEditState::new(Metrics::new(16.0, 19.2)), SingleLine))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

    press_enter(&mut app, window);
    app.update();

    assert_eq!(submitted_count(&mut app), 1, "single-line Enter ⇒ one EditSubmitted");
}

#[test]
fn multi_line_enter_emits_no_edit_submitted() {
    let mut app = app();
    let window = app.world_mut().spawn(()).id();
    let editor = app
        .world_mut()
        .spawn((Node, TextEditState::new(Metrics::new(16.0, 19.2)))) // NOT SingleLine
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

    press_enter(&mut app, window);
    app.update();

    assert_eq!(submitted_count(&mut app), 0, "multi-line Enter inserts a newline, no submit");
}
```

### Step 1.2 — run it, watch it fail

```sh
cargo test -p buiy_core --test text_edit_submit
```

Expected: **compile error** — `EditSubmitted` is not exported. Good.

### Step 1.3 — minimal impl

In `crates/buiy_core/src/text/edit/input.rs`, add the Message type next to
`TextChanged` (after the `TextChanged` definition, ~line 22):

```rust
/// Emitted when a single-line editor is submitted (editing-and-ime § 11 row
/// `EditSubmitted`, § 3.3). Born from `EditCommand::Submit` — the focused
/// single-line Enter. Payload: the entity (the value is read via the
/// component, per the § 11 contract). This FINALIZES the § 11 taxonomy
/// (the host-facing surface of E2's internal `EditOutcome.submitted` flag).
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditSubmitted(pub Entity);
```

In `apply_keyboard_edits` (same file), add the writer param to the signature
(after `mut redone: MessageWriter<super::undo::EditRedone>,`):

```rust
    mut submitted: MessageWriter<TextChanged>, // placeholder — replaced below
```

No — add it as a distinct param. Insert after the `redone` param:

```rust
    mut submitted: MessageWriter<EditSubmitted>,
```

Then accumulate the flag in the apply loop. Add a `let mut any_submit = false;`
beside `let mut any_value_change = false;` (~line 545), set it in the loop body
(after the `let outcome = state.apply_tracked(...)` line, ~line 562):

```rust
        any_submit |= outcome.submitted;
```

and emit after the loop, beside the `if any_value_change { changed.write(...) }`
block (~line 597):

```rust
    if any_submit {
        submitted.write(EditSubmitted(entity));
    }
```

Re-export it. In `crates/buiy_core/src/text/edit/mod.rs`, extend the `input` line
(line 34):

```rust
pub use input::{EditContext, EditOutcome, EditSubmitted, TextChanged, apply_keyboard_edits};
```

In `crates/buiy_core/src/text/mod.rs`, find the E2 `add_message::<TextChanged>()`
(grounding: line 230) and the `pub use edit::{...}` re-flatten block, and add
`EditSubmitted` to both. Register the message beside `TextChanged`:

```rust
        app.add_message::<crate::text::edit::EditSubmitted>();
```

and add `EditSubmitted,` to the `pub use edit::{...}` list (the flattened re-export,
grounding: `text/mod.rs:54-60`).

### Step 1.4 — run it, watch it pass

```sh
cargo test -p buiy_core --test text_edit_submit
```

Expected: `test result: ok. 2 passed`.

### Step 1.5 — commit

```sh
cargo fmt -p buiy_core && cargo clippy -p buiy_core --all-targets -- -D warnings
git add -A && git commit
```

---

## Task 2 — focus lifecycle (undo seal + preedit removal; caret visibility owned by blink; selection retained)

Detect the `FocusedEntity` transition and run the spec § 10 lifecycle. No transition
detector exists in the codebase (grounding) — introduce the canonical
`Local<Option<Entity>>` previous-value compare.

> **Caret-visibility ownership (M1 fix — read first).** `CaretVisual.visible` is
> driven UNCONDITIONALLY for every `CaretVisual` by `write_caret_blink`
> (`visual.rs:71-90`) from the blink phase, **focus-blind**, and it runs LATER than
> `focus_lifecycle` (`focus_lifecycle` is `.after(Input)`; `write_caret_blink` is
> `.after(Animate).before(Picking)`). If `focus_lifecycle` set `visible = false` on
> blur, `write_caret_blink` would immediately recompute `visible = blink_phase(now −
> origin)` and flip a blurred caret back on (and under MinimalPlugins, near-zero clock
> advance ⇒ phase = true). So `focus_lifecycle` must NOT touch `CaretVisual.visible`
> at all. Instead, **`write_caret_blink` becomes the single, focus-aware owner of
> caret visibility**: it forces `visible = false` for any editor that is not the
> `FocusedEntity` (steady-hidden when unfocused), and only blinks the focused one.
> This also fixes the latent pre-E6 bug that a blurred E3/E5 caret never stops
> blinking. The bare-`CaretVisual` path (no `TextEditState`, or no `FocusedEntity`
> resource — the E3/E5 GPU goldens and the headless blink tests) must keep its
> current behavior (the `None`/`None` arms below preserve it).
>
> `focus_lifecycle` therefore does three things and three only: on **loss** — seal
> the open undo group + remove any preedit (+ the M1 dirty-mark below); on **gain** —
> reset the blink origin (so the focused caret is solid-on for one half-period). Caret
> show/hide is entirely the blink writer's now.

### Step 2.1 — modify `write_caret_blink` to be focus-aware (the M1 owner change)

This is a change to an EXISTING T7 function the E3/E5 goldens + blink tests exercise,
so it ships with a compatibility test FIRST.

Add a failing test asserting the new focus-aware behavior AND the preserved
bare-caret path. Create `crates/buiy_core/tests/text_caret_blink_focus.rs`:

```rust
//! E6 Task 2 (M1) — `write_caret_blink` is the single, focus-AWARE owner of
//! `CaretVisual.visible`: an editor that is NOT the FocusedEntity is forced
//! steady-hidden (blurred carets do not blink); the focused editor blinks on
//! its per-entity phase. The bare-caret path (no TextEditState, or no
//! FocusedEntity resource) keeps the pre-E6 global-phase behavior.

use bevy::prelude::*;
use buiy_core::focus::FocusPlugin;
use buiy_core::text::edit::TextEditState;
use buiy_core::text::{CaretBlinkInterval, CaretVisual};
use buiy_core::{FocusedEntity, Node};
use cosmic_text::Metrics;

fn blink_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(FocusPlugin)
        .add_plugins(buiy_core::text::BuiyTextPlugin::default());
    // Pin the clock so the blink phase is deterministic, not MinimalPlugins luck.
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app
}

#[test]
fn unfocused_editor_caret_is_forced_hidden() {
    let mut app = blink_app();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            TextEditState::new(Metrics::new(16.0, 19.2)),
            CaretVisual { visible: true, rect: Rect::new(0.0, 0.0, 1.0, 16.0) },
        ))
        .id();
    // Nothing focused.
    app.update();
    let caret = app.world().get::<CaretVisual>(editor).unwrap();
    assert!(!caret.visible, "an unfocused editor's caret is steady-hidden");
}

#[test]
fn focused_editor_caret_blinks_visible_at_phase_zero() {
    let mut app = blink_app();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            TextEditState::new(Metrics::new(16.0, 19.2)),
            CaretVisual { visible: false, rect: Rect::new(0.0, 0.0, 1.0, 16.0) },
        ))
        .id();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    // Clock paused at 0 ⇒ phase 0 ⇒ visible (even half-period).
    app.update();
    let caret = app.world().get::<CaretVisual>(editor).unwrap();
    assert!(caret.visible, "the focused editor's caret is visible at phase 0");
}

#[test]
fn bare_caret_without_editor_keeps_global_phase() {
    // No TextEditState ⇒ the None arm ⇒ global phase (clock at 0 ⇒ visible),
    // regardless of focus. The E3/E5 GPU goldens rely on this.
    let mut app = blink_app();
    let caret_entity = app
        .world_mut()
        .spawn((Node, CaretVisual { visible: false, rect: Rect::new(0.0, 0.0, 1.0, 16.0) }))
        .id();
    app.update();
    let caret = app.world().get::<CaretVisual>(caret_entity).unwrap();
    assert!(caret.visible, "a bare caret blinks on the global phase, focus-blind");
}
```

### Step 2.2 — run it, watch it fail

```sh
cargo test -p buiy_core --test text_caret_blink_focus
```

Expected: `unfocused_editor_caret_is_forced_hidden` FAILS (today `write_caret_blink`
shows it on the global phase). Good.

### Step 2.3 — make `write_caret_blink` focus-aware

In `crates/buiy_core/src/text/visual.rs`, replace the `write_caret_blink` signature
+ body (currently `visual.rs:71-90`) with the focus-aware version. The `None` arm of
each `Option` preserves the existing behavior (bare caret / no focus resource):

```rust
pub fn write_caret_blink(
    time: Res<Time>,
    prefs: Option<Res<UserPreferences>>,
    interval: Res<CaretBlinkInterval>,
    // E6 (M1): the SINGLE focus-aware owner of caret visibility. A non-focused
    // editor's caret is forced steady-hidden; only the focused editor blinks.
    // `Option` so a harness without `FocusPlugin` keeps the pre-E6 behavior.
    focused: Option<Res<crate::FocusedEntity>>,
    mut carets: Query<(Entity, &mut CaretVisual, Option<&TextEditState>)>,
) {
    let steady = prefs.is_some_and(|p| p.prefers_reduced_motion);
    let now = time.elapsed();
    let focused_entity = focused.as_ref().and_then(|f| f.0);
    for (entity, mut caret, state) in &mut carets {
        // An EDITOR caret (has TextEditState) that is not the focused entity is
        // forced hidden — blurred editors do not blink (§ 10). A bare caret (no
        // TextEditState) is focus-blind: the global phase, the pre-E6 behavior
        // the E3/E5 goldens depend on. When there is no FocusedEntity resource
        // at all, every editor is treated as "not unfocused" (no focus infra ⇒
        // keep blinking) so a standalone BuiyTextPlugin harness is unchanged.
        let phase = match state {
            // Editor caret + a focus resource present: hide unless focused.
            Some(_) if focused.is_some() && focused_entity != Some(entity) => false,
            // Editor caret, focused (or no focus resource): per-entity phase.
            Some(s) => steady || blink_phase(now.saturating_sub(s.blink_origin()), interval.0),
            // Bare caret: the global phase, focus-blind (unchanged).
            None => steady || blink_phase(now, interval.0),
        };
        // Edge-only: DerefMut (and the change tick) ONLY on a flip.
        if caret.visible != phase {
            caret.visible = phase;
        }
    }
}
```

Update the function's doc comment (the `visual.rs:62-70` block) to record that it now
owns focus-gating: append a sentence — *"E6: this is the single focus-aware owner of
`CaretVisual.visible` — a non-focused editor caret is forced steady-hidden; bare
carets and harnesses without a `FocusedEntity` resource keep the global phase."*

### Step 2.4 — run it, watch it pass

```sh
cargo test -p buiy_core --test text_caret_blink_focus
```

Expected: `test result: ok. 3 passed`. Also re-run the existing blink tests to prove
no regression:

```sh
cargo test -p buiy_core --test text_caret_blink 2>/dev/null || \
  cargo test -p buiy_core caret_blink
```

Expected: still green (the bare-caret / no-focus paths are unchanged).

### Step 2.5 — failing test for `focus_lifecycle` (seal + preedit removal)

Now the lifecycle system itself — it no longer touches caret visibility. Create
`crates/buiy_core/tests/text_focus_lifecycle.rs`:

```rust
//! E6 Task 2 — focus lifecycle (editing-and-ime § 10). On focus LOSS: the open
//! undo group seals and any live preedit is removed; the selection / buffer is
//! RETAINED (web parity). Caret VISIBILITY is owned by `write_caret_blink`
//! (M1) — proven separately in `text_caret_blink_focus.rs`; this file proves
//! the seal + preedit-removal edges.

use bevy::prelude::*;
use buiy_core::focus::FocusPlugin;
use buiy_core::text::edit::{EditCommand, TextEditState, focus_lifecycle};
use buiy_core::text::SharedFontSystem;
use buiy_core::{FocusedEntity, Node};
use cosmic_text::Metrics;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(FocusPlugin)
        .add_plugins(buiy_core::text::BuiyTextPlugin::default());
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    app
}

#[test]
fn focus_loss_seals_undo_and_retains_buffer() {
    let mut app = app();
    let editor = app
        .world_mut()
        .spawn((Node, TextEditState::new(Metrics::new(16.0, 19.2))))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    app.update();

    // Drive a typing run directly so an undo TypingRun group is OPEN.
    {
        let fonts = app.world().resource::<SharedFontSystem>().clone();
        let mut state = app.world_mut().get_mut::<TextEditState>(editor).unwrap();
        let mut fs = fonts.lock();
        for ch in "abc".chars() {
            state.apply(&mut fs, EditCommand::Insert(ch.to_string()), false, false);
        }
    }
    assert!(
        app.world().get::<TextEditState>(editor).unwrap().undo_open_for_test(),
        "a TypingRun group is open before blur"
    );

    // Blur.
    app.world_mut().resource_mut::<FocusedEntity>().0 = None;
    app.update();

    let state = app.world().get::<TextEditState>(editor).unwrap();
    assert!(!state.undo_open_for_test(), "focus loss seals the open undo group");
    // Retention (web parity): the buffer / selection survive blur.
    assert_eq!(state.value(), "abc", "blur retains the buffer");
}

#[test]
fn focus_loss_removes_an_active_preedit() {
    let mut app = app();
    let editor = app
        .world_mut()
        .spawn((Node, TextEditState::new(Metrics::new(16.0, 19.2))))
        .id();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    app.update();

    // Splice a preedit directly (simulating an in-flight composition).
    {
        let fonts = app.world().resource::<SharedFontSystem>().clone();
        let mut state = app.world_mut().get_mut::<TextEditState>(editor).unwrap();
        let mut fs = fonts.lock();
        state.splice_preedit(&mut fs, "ぁ", None);
    }
    assert!(app.world().get::<TextEditState>(editor).unwrap().has_preedit());

    // Blur ⇒ focus_lifecycle removes the orphan span (E5's deferred removal).
    app.world_mut().resource_mut::<FocusedEntity>().0 = None;
    app.update();

    let state = app.world().get::<TextEditState>(editor).unwrap();
    assert!(!state.has_preedit(), "focus loss removes the preedit (no orphan)");
    assert_eq!(state.value(), "", "the preedit was never part of the value");
}
```

(`splice_preedit` is E5's `pub` method — confirmed `ime.rs:104`. `undo_open_for_test`
is added below.)

### Step 2.6 — run it, watch it fail

```sh
cargo test -p buiy_core --test text_focus_lifecycle
```

Expected: **compile error** — `focus_lifecycle` and `undo_open_for_test` are not
exported. Good.

### Step 2.7 — minimal impl

First the test accessor. In `crates/buiy_core/src/text/edit/state.rs`, add to the
`impl TextEditState` block (next to `undo_depth`, ~line 209):

```rust
    /// Test/inspection: whether a coalescing undo run is currently open
    /// (the focus-loss seal closes it). Stays inside the facade.
    pub fn undo_open_for_test(&self) -> bool {
        self.undo.has_open_group()
    }

    /// Seal the open undo coalescing run (editing-and-ime § 10: focus loss
    /// seals). A motion-equivalent boundary — the next edit starts a fresh
    /// unit. Names the private `undo` field, so it lives on the facade.
    pub fn seal_undo_for_lifecycle(&mut self) {
        self.undo.seal();
    }
```

Now the lifecycle system. Create `crates/buiy_core/src/text/edit/lifecycle.rs`:

```rust
//! E6 — focus lifecycle (editing-and-ime § 10). One render-prep system,
//! `focus_lifecycle`, detects the `FocusedEntity` transition (gain / loss)
//! via a `Local<Option<Entity>>` previous-value compare (no transition
//! detector existed before E6) and runs the spec § 10 edges:
//!
//!   - on GAIN of a non-`Disabled` editor: the blink phase resets (the user
//!     just acted — the focused caret is solid-on for one half-period, web
//!     parity);
//!   - on LOSS of an editor: the open undo group seals (`seal()`), and any live
//!     preedit is removed (wiring E5's deferred focus-loss removal — E5's
//!     `apply_ime` only removes on `Ime::Disabled`, never on a bare focus
//!     change). The **selection / buffer is RETAINED** (we never touch
//!     `SelectionVisual` — re-focus restores it, web parity).
//!
//! **Caret visibility is NOT touched here (M1).** `write_caret_blink` is the
//! single focus-aware owner of `CaretVisual.visible` (it forces a non-focused
//! editor caret hidden and blinks the focused one). `focus_lifecycle` only
//! resets the blink ORIGIN on gain.
//!
//! `ime_enabled` is also NOT handled here: E5's `write_ime_window` already
//! decides it from focus + markers alone every frame (`ime.rs` enable_q).
//!
//! **The M1 dirty-mark.** `remove_preedit` does direct BufferLine surgery and
//! does NOT invalidate intrinsics or Taffy-dirty the node (verified `ime.rs` —
//! the removal there relies on `apply_ime`'s own dirty-mark at `ime.rs:390-395`).
//! On a bare focus change there is no `apply_ime` pass, so `focus_lifecycle`
//! MUST do the same dirty-mark itself after removing the preedit, or the
//! orphaned preedit glyphs persist a frame: `invalidate_intrinsics()` +
//! `tree.mark_dirty_for_entity(entity)` (the `apply_keyboard_edits` /
//! `apply_ime` seam).
//!
//! It names only the pure-data cosmic types via the facade accessors
//! (`remove_preedit` locks the `SharedFontSystem`; `seal` names none), so it
//! stays inside the `text::edit` facade.

use bevy::prelude::*;

use super::state::{Disabled, TextEditState};
use crate::FocusedEntity;
use crate::layout::LayoutTree;
use crate::text::SharedFontSystem;

/// Render-prep: react to focus gain / loss for editor entities (§ 10). Runs in
/// the `.after(BuiySet::Input).before(write_caret_blink)` window alongside the
/// E3 caret writer and E5 IME window writer.
///
/// `Local<Option<Entity>>` holds last frame's focused entity — the canonical
/// transition detector E6 introduces (the codebase had none). Option params
/// (`focused`, `tree`) follow the inert-harness discipline — a bare
/// `BuiyTextPlugin` without `FocusPlugin` / `LayoutPlugin` no-ops instead of
/// panicking at param validation (the `apply_ime` precedent).
#[allow(clippy::type_complexity)]
pub fn focus_lifecycle(
    time: Res<Time>,
    focused: Option<Res<FocusedEntity>>,
    fonts: Res<SharedFontSystem>,
    mut tree: Option<NonSendMut<LayoutTree>>,
    mut prev: Local<Option<Entity>>,
    mut editors: Query<&mut TextEditState, Without<Disabled>>,
) {
    let Some(focused) = focused.as_ref() else { return };
    let now = time.elapsed();
    let current = focused.0;
    if current == *prev {
        return; // no transition — the common case
    }
    let lost = *prev;
    let gained = current;
    *prev = current;

    // --- LOSS: seal undo, remove preedit (+ M1 dirty-mark), RETAIN selection --
    if let Some(old) = lost
        && let Ok(mut state) = editors.get_mut(old)
    {
        state.seal_undo_for_lifecycle();
        if state.has_preedit() {
            {
                let mut fs = fonts.lock();
                state.remove_preedit(&mut fs);
            }
            // The M1 dirty-mark: remove_preedit changed the buffer but tripped
            // no TextSyncTrigger and did not invalidate — do it here so next
            // frame's measure → TextCommit → extract republishes (the
            // apply_ime / apply_keyboard_edits seam, ime.rs:390-395).
            state.invalidate_intrinsics();
            if let Some(tree) = tree.as_deref_mut() {
                tree.mark_dirty_for_entity(old);
            }
        }
    }

    // --- GAIN: reset the blink origin (caret visibility is the blink writer's) -
    if let Some(new) = gained
        && let Ok(mut state) = editors.get_mut(new)
    {
        state.blink.reset(now);
    }
}
```

Re-export the system. In `crates/buiy_core/src/text/edit/mod.rs`, add the module
(with the others):

```rust
mod lifecycle;
```

and the re-export (next to the caret one):

```rust
pub use lifecycle::focus_lifecycle;
```

Register it. In `crates/buiy_core/src/text/mod.rs`, beside the
`write_caret_and_selection` registration (grounding: lines 195-200):

```rust
        app.add_systems(
            Update,
            crate::text::edit::focus_lifecycle
                .after(crate::BuiySet::Input)
                .before(crate::text::visual::write_caret_blink),
        );
```

### Step 2.8 — run it, watch it pass

```sh
cargo test -p buiy_core --test text_focus_lifecycle
```

Expected: `test result: ok. 2 passed`.

### Step 2.9 — commit

```sh
cargo fmt -p buiy_core && cargo clippy -p buiy_core --all-targets -- -D warnings
git add -A && git commit
```

---

## Task 3 — auto-scroll-into-view via `ScrollOffset`

After the caret moves / edits, clamp the caret rect into the node's content-box
viewport via `ScrollOffset` (x single-line / y multi-line). The clamp is a pure
function (headless-testable in isolation), then the system applies it.

**Design decision — viewport source.** The node's visible extent is
`ResolvedLayout.size` (the border box; grounding — there is no prebuilt viewport-size
component, and `ClipRect` is window-space and may be absent). The caret rect is
**content-box-local** (E3's `caret_rect_for`), offset from the border box by the
content inset. Rather than resolve `BoxModel.border`/`.padding` (which are `Length`,
not `f32` — verified: `Edges { top/right/bottom/left: Length }`), the clamp uses
`ResolvedLayout.size` directly with a generous `SCROLL_MARGIN` that absorbs the small
border/padding inset for v1; this keeps the caret comfortably in view without a
`Length`→px resolution dependency (a refinement to a precise content-box extent is a
trivial follow-up if a fixture ever shows the margin is too coarse). The clamp
compares content-box-local caret coords against `[ScrollOffset.{x|y},
ScrollOffset.{x|y} + extent]`. `SingleLine` ⇒ pan x only; multi-line ⇒ pan y only
(the spec § 9 axis split).

### Step 3.1 — failing test (the pure clamp first)

Create `crates/buiy_core/tests/text_auto_scroll.rs`:

```rust
//! E6 Task 3 — auto-scroll-into-view (editing-and-ime § 9). The pure clamp
//! (`clamp_into_view`) keeps the caret inside the viewport window with a
//! margin; the `auto_scroll_caret` system writes the result into the layout
//! `ScrollOffset` (x single-line / y multi-line), which does NOT invalidate
//! Taffy.

use bevy::prelude::*;
use buiy_core::focus::FocusPlugin;
use buiy_core::layout::components::ScrollOffset;
use buiy_core::layout::{ResolvedLayout, Style};
use buiy_core::text::edit::{EditCommand, SingleLine, TextEditState, clamp_into_view};
use buiy_core::text::{SharedFontSystem, Text};
use buiy_core::{FocusedEntity, Node};
use cosmic_text::{Metrics, Motion};

#[test]
fn clamp_pans_to_reveal_caret_past_the_right_edge() {
    // viewport [offset=0 .. 100]; caret at 130 (past the right edge); margin 4.
    // The window must shift so caret+margin <= offset+extent ⇒ offset = 130+4-100 = 34.
    let new = clamp_into_view(0.0, 100.0, 130.0, 1.0, 4.0);
    assert_eq!(new, 34.0);
}

#[test]
fn clamp_pans_to_reveal_caret_before_the_left_edge() {
    // viewport [offset=50 .. 150]; caret at 30 (before the left edge); margin 4.
    // offset must drop so caret-margin >= offset ⇒ offset = 30-4 = 26.
    let new = clamp_into_view(50.0, 100.0, 30.0, 1.0, 4.0);
    assert_eq!(new, 26.0);
}

#[test]
fn clamp_is_a_noop_when_caret_already_visible() {
    // caret comfortably inside [10 .. 110]; no change.
    let new = clamp_into_view(10.0, 100.0, 60.0, 1.0, 4.0);
    assert_eq!(new, 10.0);
}

#[test]
fn clamp_never_goes_negative() {
    // a caret near content start in a wide viewport keeps offset >= 0.
    let new = clamp_into_view(20.0, 100.0, 2.0, 1.0, 4.0);
    assert_eq!(new, 0.0);
}

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(FocusPlugin)
        .add_plugins(buiy_core::text::BuiyTextPlugin::default());
    app
}

#[test]
fn single_line_caret_past_right_edge_pans_x_only() {
    let mut app = app();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            // A narrow 40px-wide field with a long value — the caret at End is
            // far past the right edge after we move there.
            Style::default().width_px(40.0).height_px(20.0),
            Text(String::from("the quick brown fox jumps")),
            FontSize_default(),
            TextEditState::new(Metrics::new(16.0, 19.2)),
            SingleLine,
            ScrollOffset::default(),
            ResolvedLayout { position: Vec2::ZERO, size: Vec2::new(40.0, 20.0) },
        ))
        .id();
    // Settle so TextSync lowers Text → editor buffer and TextCommit shapes it.
    app.update();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

    // Move the caret to the End (far right).
    {
        let fonts = app.world().resource::<SharedFontSystem>().clone();
        let mut state = app.world_mut().get_mut::<TextEditState>(editor).unwrap();
        let mut fs = fonts.lock();
        state.apply(&mut fs, EditCommand::Motion(Motion::End, false), true, false);
    }
    app.update(); // E3 writes the caret rect; E6 auto-scroll pans
    app.update(); // (one-frame settle for the reshaped buffer)

    let offset = app.world().get::<ScrollOffset>(editor).unwrap();
    assert!(offset.x > 0.0, "single-line caret past the right edge pans x: {}", offset.x);
    assert_eq!(offset.y, 0.0, "single-line never pans y");
}

// Tiny helper: a default FontSize component (16px) so shaping matches metrics.
fn FontSize_default() -> buiy_core::text::FontSize {
    buiy_core::text::FontSize(16.0)
}
```

(Rename the helper to a snake_case `font_size_default()` to satisfy clippy — fix in
the final fmt/clippy pass; the intent is shown above.)

### Step 3.2 — run it, watch it fail

```sh
cargo test -p buiy_core --test text_auto_scroll
```

Expected: **compile error** — `clamp_into_view` and the `auto_scroll_caret` system
do not exist. Good.

### Step 3.3 — minimal impl

Create `crates/buiy_core/src/text/edit/scroll.rs`:

```rust
//! E6 — auto-scroll-into-view (editing-and-ime § 9). The editor's viewport
//! pans via the layout `ScrollOffset` (x single-line / y multi-line); the
//! Buffer is laid out at full content size and never scrolls internally —
//! `ScrollOffset` deliberately does NOT invalidate Taffy
//! (`layout/components.rs:509-526` + its invariant test). After each caret
//! move / edit, `auto_scroll_caret` clamps the caret rect into the node's
//! content-box viewport with a small margin.
//!
//! `clamp_into_view` is a PURE function (the headless unit) — it takes the
//! current offset, the viewport extent on one axis, the caret's leading and
//! trailing coordinates on that axis, and the margin, and returns the new
//! offset. The system reads geometry (`CaretVisual` + `ResolvedLayout`) and
//! applies it on the right axis per the `SingleLine` marker.
//!
//! It names NO cosmic type (the caret rect comes from the E3 `CaretVisual`
//! seat, pure Bevy `Rect`), so it is free of the facade boundary, but it lives
//! in `text::edit` for cohesion.

use bevy::prelude::*;

use super::state::{Disabled, SingleLine, TextEditState};
use crate::components::ResolvedLayout;
use crate::layout::components::ScrollOffset;
use crate::text::CaretVisual;

/// The keep-off-the-edge margin in logical px. Generous enough to also absorb
/// the small border/padding inset between the border box (`ResolvedLayout.size`)
/// and the content box for v1 (a precise content-box extent via a `Length`→px
/// resolution of `BoxModel.border`/`.padding` is a trivial follow-up).
const SCROLL_MARGIN: f32 = 6.0;

/// Clamp `caret_lead`..`caret_lead + caret_size` into the viewport window
/// `[offset, offset + extent]` with `margin`, returning the new offset.
///
/// - If the caret's trailing edge (+margin) exceeds the window's far edge, pan
///   forward so it is just visible.
/// - Else if the caret's leading edge (−margin) is before the window's near
///   edge, pan back.
/// - Else no change.
///
/// The offset is clamped to `>= 0` (content never pans past its start). When
/// the caret is larger than the viewport (degenerate tiny field), revealing the
/// leading edge wins (the trailing branch would push it off the near side).
pub fn clamp_into_view(
    offset: f32,
    extent: f32,
    caret_lead: f32,
    caret_size: f32,
    margin: f32,
) -> f32 {
    let near = offset;
    let far = offset + extent;
    let caret_trail = caret_lead + caret_size;

    let mut new = offset;
    if caret_trail + margin > far {
        new = caret_trail + margin - extent;
    }
    // The leading-edge branch runs AFTER and can override (small viewport): it
    // guarantees the caret start is visible.
    if caret_lead - margin < near.min(new) || caret_lead - margin < new {
        new = (caret_lead - margin).min(new);
    }
    new.max(0.0)
}

/// Render-prep: pan the focused editor's `ScrollOffset` so the caret stays in
/// view (§ 9). Runs `.after(write_caret_and_selection)` so it reads the caret
/// rect that writer just published; the `ScrollOffset` it writes is consumed
/// by the transform bridge later this frame (`seed_scroll_dirty`,
/// `.after(BuiySet::Animate)`).
///
/// Single-line ⇒ pan x; multi-line ⇒ pan y. The viewport extent is the node's
/// border-box size (`ResolvedLayout.size`); `SCROLL_MARGIN` absorbs the small
/// content inset for v1. Option params / `Without<Disabled>` follow the
/// editor-system discipline.
#[allow(clippy::type_complexity)]
pub fn auto_scroll_caret(
    focused: Option<Res<crate::FocusedEntity>>,
    mut editors: Query<
        (&CaretVisual, Has<SingleLine>, &ResolvedLayout, &mut ScrollOffset),
        (With<TextEditState>, Without<Disabled>),
    >,
) {
    let Some(focused) = focused else { return };
    let Some(entity) = focused.0 else { return };
    let Ok((caret, single_line, layout, mut offset)) = editors.get_mut(entity) else {
        return;
    };

    if single_line {
        let extent = layout.size.x;
        if extent <= 0.0 {
            return; // not laid out yet
        }
        let new_x = clamp_into_view(offset.x, extent, caret.rect.min.x, caret.rect.width(), SCROLL_MARGIN);
        if offset.x != new_x {
            offset.x = new_x;
        }
        // Single-line never pans y.
        if offset.y != 0.0 {
            offset.y = 0.0;
        }
    } else {
        let extent = layout.size.y;
        if extent <= 0.0 {
            return;
        }
        let new_y = clamp_into_view(offset.y, extent, caret.rect.min.y, caret.rect.height(), SCROLL_MARGIN);
        if offset.y != new_y {
            offset.y = new_y;
        }
    }
}
```

Confirm `ResolvedLayout` is at `crate::components::ResolvedLayout` (grounding:
`components.rs:23-30`, `{ position: Vec2, size: Vec2 }`) and `ScrollOffset` at
`crate::layout::components::ScrollOffset`. The clamp math is independent of these
paths.

Re-export. In `mod.rs`:

```rust
mod scroll;
```
```rust
pub use scroll::{auto_scroll_caret, clamp_into_view};
```

Register, `.after(write_caret_and_selection)`, in `text/mod.rs`:

```rust
        app.add_systems(
            Update,
            crate::text::edit::auto_scroll_caret
                .after(crate::text::edit::write_caret_and_selection)
                .before(crate::text::visual::write_caret_blink),
        );
```

### Step 3.4 — run it, watch it pass

```sh
cargo test -p buiy_core --test text_auto_scroll
```

Expected: `test result: ok. 5 passed`. If the single-line integration test is flaky
on viewport timing, confirm the field is laid out (the test sets `ResolvedLayout`
directly so it does not depend on the layout plugin running).

### Step 3.5 — commit

```sh
cargo fmt -p buiy_core && cargo clippy -p buiy_core --all-targets -- -D warnings
git add -A && git commit
```

---

## Task 4 — placeholder rendering (display-only buffer, `color.text.placeholder`)

When the editor value is empty (preedit excluded), shape the `Placeholder` string
into a **separate display-only buffer** and paint it through the existing producer in
the placeholder token. The spec already decided this mechanism (decoration-and-paint
§ 7: same pipeline, one different tint; the rejected runner-up is a dedicated
placeholder paint path). The editor buffer is empty exactly when a placeholder is
wanted, so the producer's editor-first accessor naturally yields nothing — the
placeholder buffer is the one with glyphs.

**Design decision — where the placeholder buffer lives.** A distinct
`PlaceholderBuffer(Buffer)` component, NOT the entity's dormant display `TextBuffer`.
Overloading `TextBuffer` on an editor entity (option i in grounding) entangles two
sync triggers (`Changed<Text>` vs `Changed<Placeholder>`) and two meanings on one
component; a distinct component keeps the placeholder lifecycle independent and the
producer's paint a clean additive branch. The cost — one new component + one sync arm
+ one producer branch — is the clean boundary.

> **M3 — the placeholder buffer must shape ITSELF (it has no downstream shaper).**
> The editor-owned buffer is shaped lazily because `TextCommit` reshapes whatever
> `TextBufferAccess` reaches (the editor buffer). The `PlaceholderBuffer` is reached by
> NOTHING downstream — `TextCommit` never touches it — so a lazy `set_text` (defer
> shaping) leaves its `layout_runs()` empty forever and the placeholder paints
> nothing. Therefore `sync_placeholder` **locks `SharedFontSystem` and shapes its own
> buffer**: `buffer.set_text(...)` then `buffer.shape_until_scroll(&mut fs, false)`
> (verified `buffer.rs:571`). This is NOT lock-free — `sync_placeholder` takes the
> lock, which is correct in the `BuiyLayoutStep::TextSync` step (the same step
> `text_sync_buffers` runs in; the lock is the measure-pipeline lock).
>
> **M4 — the placeholder paints as a SEPARATE additive branch, NOT through the §3.2
> assert.** The producer's architecture-§3.2 tripwire
> `debug_assert_eq!(runs, computed.lines.len())` (`extract.rs:704`) counts `runs` from
> the buffer it iterates (the editor buffer via `with_buffer`) against the editor
> buffer's `ComputedTextLayout` (`commit.rs:156` builds `computed.lines` from the
> editor buffer only). For a placeholder-showing entity the editor buffer is empty —
> it shapes to **one empty run**, so `computed.lines.len() == 1` and the assert holds
> as-is. The placeholder painting must therefore be a SECOND, independent emission
> that runs AFTER the editor-buffer assert and iterates
> `PlaceholderBuffer.buffer.layout_runs()` on its own — it does **not** contribute to
> `runs` and is **not** part of the §3.2 check. (Equivalently: the placeholder is a
> different display buffer, with its own runs, not part of the editor's
> `ComputedTextLayout`.)
>
> **M2 — the extract query is at Bevy's 15-tuple cap (already 14, `extract.rs:206`).**
> Adding BOTH `Has<PlaceholderActive>` AND `Option<&PlaceholderBuffer>` → 16 → over
> the cap (compile error). So the extract query adds at most ONE slot:
> `Option<&PlaceholderBuffer>` only. The painting branch keys on
> "`PlaceholderBuffer` present AND it shaped to ≥1 run" — the marker `PlaceholderActive`
> is for the **headless test + the damage gate** (a separate `Changed<PlaceholderActive>`
> probe member, not the inner tuple). `sync_placeholder` only INSERTS a
> `PlaceholderBuffer` when active, so presence is a sufficient paint signal.
>
> **Test split.** The headless gate asserts the **main-world state** (`PlaceholderActive`
> present iff value empty; the `PlaceholderBuffer` shapes to ≥1 run — guards against a
> 0-run regression the all-default fixtures would hide). The pixel paint is proven by
> the Task 8 GPU golden. This split (headless state + GPU pixels) is the campaign's
> standing discipline.

### Step 4.1 — failing test

Create `crates/buiy_core/tests/text_placeholder.rs`:

```rust
//! E6 Task 4 — placeholder rendering (editing-and-ime § 10). When the editor's
//! logical value is empty (preedit excluded), the `Placeholder` string is
//! shaped into a display-only `PlaceholderBuffer`; the moment a real char
//! exists the placeholder buffer is cleared. The string NEVER enters the
//! editor buffer or the undo history.

use bevy::prelude::*;
use buiy_core::focus::FocusPlugin;
use buiy_core::text::edit::{EditCommand, Placeholder, PlaceholderActive, TextEditState};
use buiy_core::text::{FontSize, SharedFontSystem, Text};
use buiy_core::{FocusedEntity, Node};
use cosmic_text::Metrics;

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(FocusPlugin)
        .add_plugins(buiy_core::text::BuiyTextPlugin::default());
    app
}

#[test]
fn placeholder_is_active_when_value_empty() {
    let mut app = app();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Text(String::new()),
            FontSize(16.0),
            TextEditState::new(Metrics::new(16.0, 19.2)),
            Placeholder(String::from("Search…")),
        ))
        .id();
    app.update();
    app.update();

    assert!(
        app.world().get::<PlaceholderActive>(editor).is_some(),
        "an empty editor with a Placeholder shows it"
    );
    // The editor buffer is still empty — the placeholder never entered it.
    assert_eq!(app.world().get::<TextEditState>(editor).unwrap().value(), "");
    assert_eq!(
        app.world().get::<TextEditState>(editor).unwrap().undo_depth(),
        0,
        "the placeholder is not an undoable edit"
    );
}

#[test]
fn placeholder_vanishes_on_first_real_char() {
    let mut app = app();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Text(String::new()),
            FontSize(16.0),
            TextEditState::new(Metrics::new(16.0, 19.2)),
            Placeholder(String::from("Search…")),
        ))
        .id();
    app.update();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    assert!(app.world().get::<PlaceholderActive>(editor).is_some());

    // Type one real char.
    {
        let fonts = app.world().resource::<SharedFontSystem>().clone();
        let mut state = app.world_mut().get_mut::<TextEditState>(editor).unwrap();
        let mut fs = fonts.lock();
        state.apply(&mut fs, EditCommand::Insert("a".to_string()), false, false);
    }
    app.update();

    assert!(
        app.world().get::<PlaceholderActive>(editor).is_none(),
        "placeholder vanishes once a real char exists"
    );
}

#[test]
fn active_placeholder_buffer_shapes_to_at_least_one_run() {
    // M3 regression guard: the PlaceholderBuffer must SHAPE (its own
    // shape_until_scroll), not defer — else layout_runs() is empty and the
    // placeholder paints nothing. All-default fixtures would hide a 0-run bug.
    use buiy_core::text::edit::PlaceholderBuffer;

    let mut app = app();
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Text(String::new()),
            FontSize(16.0),
            TextEditState::new(Metrics::new(16.0, 19.2)),
            Placeholder(String::from("Search")),
        ))
        .id();
    app.update();
    app.update();

    let ph = app
        .world()
        .get::<PlaceholderBuffer>(editor)
        .expect("an active placeholder has a PlaceholderBuffer");
    let run_count = ph.buffer.layout_runs().count();
    assert!(run_count >= 1, "the placeholder buffer is shaped (>=1 run), got {run_count}");
}
```

Use a `PlaceholderActive` marker as the observable activation state (the headless test
asserts it; the producer's damage gate keys on it). The `PlaceholderBuffer` field is
`pub buffer` so the run-count test reads it. This keeps the headless test independent
of the render world.

### Step 4.2 — run it, watch it fail

```sh
cargo test -p buiy_core --test text_placeholder
```

Expected: **compile error** — `PlaceholderActive` / `sync_placeholder` do not exist.

### Step 4.3 — minimal impl

Create `crates/buiy_core/src/text/edit/placeholder.rs`:

```rust
//! E6 — placeholder rendering state (editing-and-ime § 10,
//! decoration-and-paint § 7). The placeholder is "just text with a different
//! tint" (the spec decision; the rejected runner-up is a dedicated placeholder
//! paint path). E6 maintains a `PlaceholderActive` marker — present iff the
//! editor's logical value is empty (preedit excluded) AND a non-empty
//! `Placeholder` string exists — and shapes the string into a display-only
//! `PlaceholderBuffer` the glyph producer paints in `color.text.placeholder`.
//! The string never enters the editor buffer or the undo history.
//!
//! **M3 — the placeholder buffer shapes ITSELF.** Unlike the editor buffer
//! (which `TextCommit` reshapes downstream via `TextBufferAccess`), nothing
//! downstream touches a `PlaceholderBuffer` — so `sync_placeholder` must lock
//! `SharedFontSystem` and call `buffer.shape_until_scroll(&mut fs, false)`
//! after `set_text`, or `layout_runs()` stays empty and the placeholder paints
//! nothing. This system takes the lock (correct — it runs in the
//! `BuiyLayoutStep::TextSync` step, the measure-pipeline lock window).
//!
//! This file names only the pure-data `Buffer`/`Metrics`/`Attrs`/`Shaping`/
//! `FontSystem` cosmic types (no `Editor`/`Edit`/`Action`/`Change`), so it
//! stays inside the `text::edit` facade.

use bevy::prelude::*;
use cosmic_text::{Attrs, Buffer, Metrics, Shaping};

use super::state::{Disabled, Placeholder, TextEditState};
use crate::text::{FontSize, SharedFontSystem};

/// Marker: the placeholder is currently shown (the editor value is empty,
/// preedit excluded, and a non-empty `Placeholder` exists). Drives the
/// producer's DAMAGE gate (a `Changed<PlaceholderActive>` probe member) and the
/// headless test; the producer's PAINT branch keys on `PlaceholderBuffer`
/// presence (M2 — the inner extract tuple is at the 15-cap, so the marker is
/// not in it). Lean derives (not reflect-registered — no authored data).
#[derive(Component, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaceholderActive;

/// The display-only shaped buffer for the placeholder string. NEVER the
/// editor buffer (§ 10: "the placeholder never enters the editor Buffer").
/// `pub buffer` so the run-count test + the producer read it; not
/// reflect-registered (carries a cosmic `Buffer`, the cosmic boundary).
#[derive(Component)]
pub struct PlaceholderBuffer {
    pub buffer: Buffer,
    /// The string the buffer was last shaped from — so we only re-shape on a
    /// `Placeholder` text change, not every frame.
    shaped_from: String,
}

/// Main-world (the `BuiyLayoutStep::TextSync` step): maintain `PlaceholderActive`
/// + `PlaceholderBuffer` for every editor with a `Placeholder`. Present the
/// marker iff `value().is_empty() && !has_preedit()` and the placeholder string
/// is non-empty; remove BOTH the marker and the buffer otherwise (so the
/// producer's `PlaceholderBuffer`-presence paint signal is exact). When active,
/// SHAPE the string into the display-only buffer (M3 — its own
/// `shape_until_scroll`, since nothing downstream shapes it).
///
/// Runs in the same step as `text_sync_buffers` (the measure-pipeline lock
/// window); `Without<Disabled>` follows the editor-system discipline.
#[allow(clippy::type_complexity)]
pub fn sync_placeholder(
    mut commands: Commands,
    fonts: Res<SharedFontSystem>,
    mut editors: Query<
        (
            Entity,
            &TextEditState,
            &Placeholder,
            Option<&FontSize>,
            Option<&mut PlaceholderBuffer>,
            Has<PlaceholderActive>,
        ),
        Without<Disabled>,
    >,
) {
    for (entity, state, placeholder, font_size, ph_buffer, was_active) in &mut editors {
        let active = state.value().is_empty() && !state.has_preedit() && !placeholder.0.is_empty();

        if active && !was_active {
            commands.entity(entity).insert(PlaceholderActive);
        } else if !active && was_active {
            // Remove BOTH — the producer paints on PlaceholderBuffer presence.
            commands.entity(entity).remove::<PlaceholderActive>();
            commands.entity(entity).remove::<PlaceholderBuffer>();
        }

        if !active {
            continue;
        }

        // Shape the placeholder string into the display-only buffer. M3: lock
        // and shape OURSELVES — nothing downstream shapes a PlaceholderBuffer
        // (TextCommit only reshapes the editor-owned buffer). Skip when the
        // string is unchanged (already shaped this content).
        let size = font_size.map(|f| f.0).unwrap_or(16.0);
        let metrics = Metrics::new(size, size * 1.2);
        match ph_buffer {
            Some(buf) if buf.shaped_from == placeholder.0 => { /* unchanged */ }
            Some(mut buf) => {
                buf.buffer.set_metrics(metrics); // no FontSystem (buffer.rs:729)
                buf.buffer
                    .set_text(&placeholder.0, &Attrs::new(), Shaping::Advanced, None);
                // M3: shape OURSELVES — shape_until_scroll DOES take the lock.
                buf.buffer.shape_until_scroll(&mut fonts.lock(), false);
                buf.shaped_from = placeholder.0.clone();
            }
            None => {
                // `Buffer::new(&mut fs, metrics)` is the shaped constructor; then
                // set_text (no fs, defers) + shape_until_scroll (fs, shapes).
                let mut buffer = Buffer::new(&mut fonts.lock(), metrics);
                buffer.set_text(&placeholder.0, &Attrs::new(), Shaping::Advanced, None);
                buffer.shape_until_scroll(&mut fonts.lock(), false);
                commands.entity(entity).insert(PlaceholderBuffer {
                    buffer,
                    shaped_from: placeholder.0.clone(),
                });
            }
        }
    }
}
```

> **Shaping API (verified against vendored 0.19, M3).** Only two of these take a
> `FontSystem`: `Buffer::new(&mut FontSystem, Metrics)` (the shaped constructor) and
> `shape_until_scroll(&mut FontSystem, prune: bool)` (`buffer.rs:571`). `set_metrics(Metrics)`
> (`buffer.rs:729`) and `set_text(&str, &Attrs, Shaping, Option<Align>)` (`buffer.rs:934`)
> do **not** — they record + dirty, deferring the actual shape. The load-bearing M3
> fact: the placeholder has no downstream shaper, so `sync_placeholder` calls
> `shape_until_scroll` under the lock itself (E1's editor buffer skips this only because
> TextCommit shapes it later). `fonts.lock()` is taken twice (once per FontSystem-bearing
> call) — fine; each is a short hold. If clippy flags the double-lock, hoist
> `let mut fs = fonts.lock();` and pass `&mut fs` to both.

Re-export. In `mod.rs`:

```rust
mod placeholder;
```
```rust
pub use placeholder::{PlaceholderActive, PlaceholderBuffer, sync_placeholder};
```

Register it in the same layout step as `text_sync_buffers` — verified:
`text_sync_buffers` is `.in_set(crate::layout::BuiyLayoutStep::TextSync)`
(`text/mod.rs:168`), so the placeholder sync joins that step (it produces a
display-only buffer the same way, shaped lazily, before `TextCommit`):

```rust
        app.add_systems(
            Update,
            crate::text::edit::sync_placeholder
                .in_set(crate::layout::BuiyLayoutStep::TextSync),
        );
```

**Producer wire-up (the paint half — M2 + M4).** In
`crates/buiy_core/src/text/extract.rs`:

1. **The extract query — add exactly ONE slot (M2).** The inner `texts` tuple is at
   14 of Bevy's 15-member cap (`extract.rs:206`). Add **only** `Option<&PlaceholderBuffer>`
   (→ 15, at the cap). Do **not** add `Has<PlaceholderActive>` to the inner tuple — it
   would be the 16th member (compile error). The producer keys the paint branch on
   `PlaceholderBuffer` presence (sync only inserts it when active, and removes it when
   inactive — Step 4.3 — so presence is an exact "show placeholder" signal).

2. **Paint as a SEPARATE additive branch AFTER the §3.2 assert (M4).** The editor
   buffer's glyph loop and its tripwire `debug_assert_eq!(runs, computed.lines.len())`
   (`extract.rs:704`) stay EXACTLY as-is — for a placeholder-showing entity the editor
   buffer is empty, shapes to one empty run, and `computed.lines.len() == 1`, so the
   assert holds untouched. AFTER that assert, add:

   ```rust
   // E6 — placeholder paint (editing-and-ime § 10, decoration-and-paint § 7).
   // A SEPARATE additive emission: the placeholder is its own display buffer
   // with its own runs — NOT part of the editor's ComputedTextLayout, so it
   // does NOT feed the §3.2 run-count assert above (M4). Painted only when the
   // editor showed nothing (value empty ⇒ the editor loop emitted no ink) and a
   // shaped PlaceholderBuffer is present; tinted to the placeholder token.
   if let Some(ph) = placeholder_buffer {
       let ph_color = linear_color(resolve_token(
           &TextColor::placeholder().0,
           theme,
       ));
       for run in ph.buffer.layout_runs() {
           for glyph in run.glyphs.iter() {
               // …the SAME per-glyph emit body the editor loop uses (physical()
               // → AtlasKey → get_or_insert → GlyphAlphaInstance), but with
               // `ph_color` as the color and the run's line_top for y. Factor
               // the editor loop's glyph-emit into a small local closure/fn and
               // call it here so the two paths share one emitter (DRY).
           }
       }
   }
   ```

   where `placeholder_buffer` is the new `Option<&PlaceholderBuffer>` query member.
   Extract the editor loop's inner glyph-emit (atlas insert + instance push +
   `ResidentTextKeys` touch) into a small local helper so the placeholder branch reuses
   it verbatim with a different color + buffer — no duplicated emit logic.

3. **Damage gate (M4).** Join `Changed<PlaceholderActive>`, `Changed<Placeholder>`,
   and `Changed<FontSize>` into the producer's probe union (the `changed` query,
   `extract.rs:216+`) so toggling the placeholder OR reshaping an already-active one
   re-extracts the entity. `PlaceholderActive` is the toggle gate signal (cheap `Copy`
   marker); `PlaceholderBuffer` is the paint payload. The buffer also reshapes WHILE
   already active when the `Placeholder` string or `FontSize` changes — `sync_placeholder`
   rebuilds the buffer but inserts NO `PlaceholderActive` (it stays present) and the empty
   editor value yields NO `ComputedTextLayout` tick, so without the `Changed<Placeholder>` /
   `Changed<FontSize>` probes the screen keeps the stale glyphs. Both are small
   runtime-mutable components — cheap and exact to gate on.

This branch is exercised by the Task 8 GPU golden (pixels) + the Step 4.1
`active_placeholder_buffer_shapes_to_at_least_one_run` headless test (≥1 run guards a
0-run regression). Keep it minimal and additive.

### Step 4.4 — run it, watch it pass

```sh
cargo test -p buiy_core --test text_placeholder
```

Expected: `test result: ok. 3 passed` (active, vanishes-on-char, shapes-to-≥1-run).

### Step 4.5 — commit

```sh
cargo fmt -p buiy_core && cargo clippy -p buiy_core --all-targets -- -D warnings
git add -A && git commit
```

---

## Task 5 — `TextEditState::for_font_size` core constructor (cosmic-text containment)

`buiy_widgets::TextInput::new` must compose a `TextEditState` without naming
`cosmic_text::Metrics` (which would crack the facade boundary — grounding). Add a
core constructor that takes a Buiy `f32` font size and computes `Metrics` internally.

### Step 5.1 — failing test

Append to `crates/buiy_core/tests/text_edit_substrate.rs` (the E1 test file — add a
new `#[test]`; if the file imports differ, add the needed `use` lines):

```rust
#[test]
fn for_font_size_constructs_an_editor_with_matching_metrics() {
    // for_font_size(16.0) ⇒ an editor whose buffer metrics are (16, 19.2).
    let state = buiy_core::text::edit::TextEditState::for_font_size(16.0);
    let (fs, lh) = state.metrics_for_test();
    assert_eq!(fs, 16.0);
    assert!((lh - 19.2).abs() < 1e-4, "line height = size * 1.2: {lh}");
    assert_eq!(state.value(), "", "a fresh editor is empty");
}
```

### Step 5.2 — run it, watch it fail

```sh
cargo test -p buiy_core --test text_edit_substrate -- for_font_size
```

Expected: **compile error** — `for_font_size` / `metrics_for_test` do not exist.

### Step 5.3 — minimal impl

In `crates/buiy_core/src/text/edit/state.rs`, add to `impl TextEditState` (next to
`new`):

```rust
    /// Construct an editor from a Buiy logical font size (logical px), computing
    /// the cosmic `Metrics` internally with the default 1.2 line-height scale.
    /// This is the seam that keeps `cosmic_text::Metrics` OUT of downstream
    /// crates (`buiy_widgets::TextInput::new` calls this — it never names a
    /// cosmic type, preserving the § 2.1 facade boundary).
    pub fn for_font_size(font_size: f32) -> Self {
        Self::new(Metrics::new(font_size, font_size * 1.2))
    }

    /// Test/inspection: the editor buffer's `(font_size, line_height)` metrics.
    /// Stays inside the facade.
    pub fn metrics_for_test(&self) -> (f32, f32) {
        use cosmic_text::Edit;
        self.editor
            .with_buffer(|b| (b.metrics().font_size, b.metrics().line_height))
    }
```

### Step 5.4 — run it, watch it pass

```sh
cargo test -p buiy_core --test text_edit_substrate -- for_font_size
```

Expected: `test result: ok. 1 passed`.

### Step 5.5 — commit

```sh
cargo fmt -p buiy_core && cargo clippy -p buiy_core --all-targets -- -D warnings
git add -A && git commit
```

---

## Task 6 — `buiy_widgets::TextInput::new` bundle + `focus_on_click`

The widget bundle (the `Button::new` precedent) composing core editor mechanism with
widget policy. Focus-on-click is widget policy (spec § 2.3 / Borrow #7), never core
auto-focus.

### Step 6.1 — failing test

Create `crates/buiy_widgets/tests/text_input.rs`:

```rust
//! E6 Task 6 — the `TextInput` widget bundle (editing-and-ime § 2.3). It
//! composes the core editor mechanism (`TextEditState` + `SingleLine` +
//! `Placeholder`) with widget policy (sizes, focusable, a11y, focus-on-click).
//! `buiy_widgets` names ZERO cosmic types — `TextEditState::for_font_size`
//! is the seam.

use bevy::prelude::*;
use buiy_core::focus::Focusable;
use buiy_core::text::edit::{Placeholder, SingleLine, TextEditState};
use buiy_widgets::{TextInput, WidgetsPlugin};

#[test]
fn single_line_text_input_composes_editor_markers_and_focusable() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(WidgetsPlugin);

    let entity = app
        .world_mut()
        .spawn(TextInput::single_line("Search…"))
        .id();
    app.update();

    let world = app.world();
    assert!(world.get::<TextEditState>(entity).is_some(), "has the editor");
    assert!(world.get::<SingleLine>(entity).is_some(), "single-line marker");
    assert!(world.get::<Focusable>(entity).is_some(), "focusable");
    let placeholder = world.get::<Placeholder>(entity).expect("placeholder");
    assert_eq!(placeholder.0, "Search…");
    assert_eq!(
        world.get::<TextEditState>(entity).unwrap().value(),
        "",
        "a fresh input is empty"
    );
}

#[test]
fn multi_line_text_input_has_no_single_line_marker() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(WidgetsPlugin);

    let entity = app.world_mut().spawn(TextInput::multi_line("")).id();
    app.update();

    assert!(app.world().get::<SingleLine>(entity).is_none(), "multi-line ⇒ no SingleLine");
    assert!(app.world().get::<TextEditState>(entity).is_some());
}

#[test]
fn clicking_a_text_input_focuses_it() {
    use buiy_core::FocusedEntity;
    use buiy_core::focus::FocusPlugin;
    use buiy_core::picking::Hovered;

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(buiy_core::CorePlugin);
    app.add_plugins(FocusPlugin);
    app.add_plugins(WidgetsPlugin);
    app.init_resource::<ButtonInput<MouseButton>>();

    let entity = app.world_mut().spawn(TextInput::single_line("")).id();
    app.update();

    // Hover + mouse-down on the input.
    app.world_mut().insert_resource(Hovered(Some(entity)));
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();

    assert_eq!(
        app.world().resource::<FocusedEntity>().0,
        Some(entity),
        "click focuses the input (widget policy)"
    );
}
```

### Step 6.2 — run it, watch it fail

```sh
cargo test -p buiy_widgets --test text_input
```

Expected: **compile error** — `TextInput` does not exist. Good.

### Step 6.3 — minimal impl

Create `crates/buiy_widgets/src/text_input.rs`:

```rust
//! `TextInput` widget (editing-and-ime § 2.3). Composes the `buiy_core` editor
//! mechanism (`TextEditState` + markers + the display `Text` carrier) with
//! widget policy: catalog sizes/tokens, focusable + a11y, submit-on-Enter (the
//! `SingleLine` marker drives `EditCommand::Submit`), and focus-on-click.
//!
//! `buiy_widgets` names NO cosmic type — `TextEditState::for_font_size` is the
//! seam (the facade boundary the campaign guards). Mirrors `Button::new`
//! (`button.rs:29-60`).

use bevy::prelude::*;
use buiy_core::a11y::{A11yLabel, A11yRole};
use buiy_core::components::Node;
use buiy_core::focus::Focusable;
use buiy_core::layout::Style;
use buiy_core::picking::Hovered;
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::{Background, Border, Corners, Radius, TextColor};
use buiy_core::text::edit::{Placeholder, SingleLine, TextEditState};
use buiy_core::text::{FontSize, Text};
use buiy_core::FocusedEntity;
use std::borrow::Cow;

/// Marker for a text-input widget (the `Button` precedent). Carried so
/// `focus_on_click` and a11y can identify the widget.
#[derive(Component, Reflect, Default, Clone, Debug)]
#[reflect(Component)]
pub struct TextInput;

/// The catalog font size for a text input (logical px). Matches the `Button`
/// hardcoded-size convention (TODO: size tokens — buiy-widget-catalog-design).
const TEXT_INPUT_FONT_SIZE: f32 = 16.0;

impl TextInput {
    /// A single-line text input with a placeholder (Enter ⇒ Submit, `Wrap::None`
    /// — the `SingleLine` policy). Returns `impl Bundle` (the `Button::new`
    /// precedent), so callers get the full editor contract without assembling
    /// it.
    #[allow(clippy::new_ret_no_self)]
    pub fn single_line(placeholder: impl Into<String>) -> impl Bundle {
        (base_bundle(placeholder), SingleLine)
    }

    /// A multi-line text input with a placeholder (Enter inserts a newline).
    #[allow(clippy::new_ret_no_self)]
    pub fn multi_line(placeholder: impl Into<String>) -> impl Bundle {
        base_bundle(placeholder)
    }
}

/// The shared composition: editor + display carrier + node/style + a11y +
/// focusable + tokens. `single_line` adds `SingleLine` on top.
fn base_bundle(placeholder: impl Into<String>) -> impl Bundle {
    let placeholder = placeholder.into();
    (
        TextInput,
        Node,
        // TODO(buiy-widget-catalog-design): size tokens. 200x32 is a typical
        // single-line input; >=24x24 meets WCAG 2.5.8. Overflow-hidden so the
        // content clips (and the auto-scroll ScrollOffset has a scroll
        // container to pan — § 9).
        Style::default()
            .width_px(200.0)
            .height_px(32.0)
            .padding(8.0)
            .overflow_hidden(),
        Background {
            color: ColorToken::Token(Cow::Borrowed("color.surface.secondary")),
        },
        Border {
            radius: Corners::all(Radius::circular(6.0)),
            ..Default::default()
        },
        // The editor mechanism + its display carrier. `Text("")` is the
        // required display `TextBuffer` carrier (editor-optional /
        // buffer-required — the editor-owned buffer is authoritative, but the
        // entity still needs a `Text` so TextSync runs and the node measures).
        Text(String::new()),
        FontSize(TEXT_INPUT_FONT_SIZE),
        TextColor::default(),
        TextEditState::for_font_size(TEXT_INPUT_FONT_SIZE),
        Placeholder(placeholder),
        Focusable::default(),
        // The Phase-0 A11yRole taxonomy stops at `Text` (no `TextInput`/
        // `TextField` variant yet — verified: a11y/mod.rs:28-40). Use `Text`;
        // the full role taxonomy is buiy-accessibility-design's, and a
        // `TextInput` role is a clean additive follow-up there.
        A11yRole::Text,
        A11yLabel(String::new()),
    )
}

/// Widget-side focus-on-click (editing-and-ime § 2.3 / Borrow #7 — focus is
/// WIDGET policy, never core auto-focus). On a left mouse-down over a hovered
/// `TextInput`, set `FocusedEntity`. Mirrors `emit_on_press_on_click`
/// (`button.rs:71-94`): `Option` params so a partial harness no-ops.
pub fn focus_on_click(
    hovered: Option<Res<Hovered>>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    inputs: Query<(), With<TextInput>>,
    focused: Option<ResMut<FocusedEntity>>,
) {
    let (Some(hovered), Some(mouse), Some(mut focused)) = (hovered, mouse, focused) else {
        return;
    };
    if !mouse.just_pressed(MouseButton::Left) {
        return;
    }
    let Some(entity) = hovered.0 else { return };
    if inputs.get(entity).is_ok() {
        focused.0 = Some(entity);
    }
}
```

> **A11y role.** `A11yRole` (verified: `a11y/mod.rs:28-40`) is the Phase-0 taxonomy
> — `Generic`/`Button`/`Link`/`Image`/`Text`/`Heading`/`Dialog`/`AlertDialog`/
> `Tooltip`. There is **no** `TextInput`/`TextField` variant; the bundle uses
> `A11yRole::Text` (the closest), and a proper text-input role is a clean additive
> follow-up owned by `buiy-accessibility-design`. Do not invent a variant the a11y
> adapter cannot map.

Wire the module + plugin. In `crates/buiy_widgets/src/lib.rs`:

```rust
pub mod text_input;
pub use text_input::{focus_on_click, TextInput};
```

and in `WidgetsPlugin::build` (grounding shows the exact body):

```rust
        app.register_type::<Button>()
            .register_type::<text_input::TextInput>()
            .add_message::<OnPress>()
            .add_systems(
                Update,
                (
                    button::emit_on_press_on_click,
                    text_input::focus_on_click,
                )
                    .in_set(buiy_core::BuiySet::Input),
            );
```

Re-export from the meta-crate. In `crates/buiy/src/lib.rs`, extend the
`pub use buiy_widgets::{...}` line (grounding: line 42) with `TextInput`.

### Step 6.4 — run it, watch it pass

```sh
cargo test -p buiy_widgets --test text_input
```

Expected: `test result: ok. 3 passed`.

### Step 6.5 — commit

```sh
cargo fmt -p buiy_widgets && cargo clippy -p buiy_widgets --all-targets -- -D warnings
git add -A && git commit
```

---

## Task 7 — the § 11 taxonomy audit (every Message exists + is registered)

A single test that asserts the full spec § 11 taxonomy is present and registered —
the completeness audit E6 owns. This catches a future drift (a Message renamed, or
left unregistered).

### Step 7.1 — failing test

Create `crates/buiy_core/tests/text_message_taxonomy.rs`:

```rust
//! E6 Task 7 — the editing Message taxonomy audit (editing-and-ime § 11). Every
//! row of the § 11 table must EXIST as a registered Bevy `Message` after
//! `BuiyTextPlugin`. This is the campaign's completeness gate: `TextChanged`
//! (E2), `SelectionChanged`/`CaretMoved` (E3), `EditUndone`/`EditRedone` (E4),
//! `CompositionStart/Update/End` (E5), `EditSubmitted` (E6).

use bevy::prelude::*;
use buiy_core::text::BuiyTextPlugin;
use buiy_core::text::edit::{
    CaretMoved, CompositionEnd, CompositionStart, CompositionUpdate, EditRedone, EditSubmitted,
    EditUndone, SelectionChanged, TextChanged,
};

fn app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(buiy_core::CorePlugin)
        .add_plugins(BuiyTextPlugin::default());
    app
}

/// A registered `Message` has a `Messages<T>` resource. Assert each row.
macro_rules! assert_registered {
    ($app:expr, $($t:ty),+ $(,)?) => {
        $(
            assert!(
                $app.world().get_resource::<Messages<$t>>().is_some(),
                "§ 11 taxonomy: {} must be a registered Message",
                std::any::type_name::<$t>()
            );
        )+
    };
}

#[test]
fn full_section_11_taxonomy_is_registered() {
    let app = app();
    assert_registered!(
        app,
        TextChanged,
        SelectionChanged,
        CaretMoved,
        EditUndone,
        EditRedone,
        CompositionStart,
        CompositionUpdate,
        CompositionEnd,
        EditSubmitted,
    );
}
```

### Step 7.2 — run it, watch it fail (or pass)

```sh
cargo test -p buiy_core --test text_message_taxonomy
```

Expected: this should **pass immediately** if Tasks 1–6 registered everything (all
prior Messages were registered by E2–E5; E6 added `EditSubmitted` in Task 1). If it
fails, a Message is unregistered — fix the `add_message` in `text/mod.rs`. The test
is the audit; a green run IS the deliverable (no new impl unless it catches a gap).

### Step 7.3 — commit

```sh
git add -A && git commit
```

---

## Task 8 — `#[ignore]` GPU golden (placeholder vs typed value) — build-only

One additive GPU golden on the existing readback harness, modeled on the E3/E5
goldens. It lives in `crates/buiy_core/tests/` (NOT `buiy_widgets` — the harness is a
`buiy_core` test target and `buiy_widgets → buiy_core` is the only direction). It
tests the **composed component bundle** (the same components `TextInput::new`
produces) — not `TextInput::new` itself — to avoid the dependency cycle. **Build-only
this phase** (`#[ignore]`); the orchestrator runs the GPU lane.

### Step 8.1 — write the golden

Create `crates/buiy_core/tests/text_placeholder_gpu.rs`:

```rust
//! E6 Task 8 — GPU golden (editing-and-ime § 10, decoration-and-paint § 7):
//! an empty editor with a `Placeholder` paints the placeholder string in the
//! `color.text.placeholder` token; after typing, it paints the typed ink in
//! the normal text color and the placeholder is gone. `#[ignore]` — needs a
//! wgpu adapter; the orchestrator runs the GPU lane. Build-only here.
//!
//! Hard-won E5 lesson: a `gpu_render_app` is NOT finished — call
//! `support::finish_and_run(.., 0)` BEFORE `register_fixture_font` (a pre-finish
//! update runs the render schedule with no `RenderDevice`/`PipelineCache` and
//! panics), then spawn the capture camera AFTER the content settles.

mod support;

use bevy::prelude::*;
use buiy_core::layout::Style;
use buiy_core::render::color::ColorToken;
use buiy_core::render::components::TextColor;
use buiy_core::render::golden::{GoldenConfig, perceptual_diff};
use buiy_core::text::edit::{EditCommand, Placeholder, TextEditState};
use buiy_core::text::{FamilyEntry, FontFamily, FontSize, FontStack, SharedFontSystem, Text};
use buiy_core::{FocusedEntity, Node};
use std::borrow::Cow;

const W: u32 = 256;
const H: u32 = 48;
const PLACEHOLDER_TOKEN: &str = "color.text.placeholder";
const TEXT_TOKEN: &str = "color.text.primary";
// A registerable fixture (NOT the embedded default Fira Sans). Use Hebrew so the
// placeholder + typed glyphs resolve in THIS font, the E3/E5 golden precedent.
const FIXTURE_FAMILY: &str = "Noto Sans Hebrew";
const FIXTURE_FILE: &str = "NotoSansHebrew-hebrew.ttf";
const PLACEHOLDER_TEXT: &str = "שלום"; // "hello" — resolves in the Hebrew fixture
const TYPED_TEXT: &str = "עולם"; // "world"

/// Capture the field in one of two states: `typed = false` ⇒ empty (placeholder
/// visible); `typed = true` ⇒ after inserting TYPED_TEXT.
fn capture(typed: bool) -> Vec<u8> {
    let _cfg = GoldenConfig::deterministic();
    let mut app = support::gpu_render_app(W, H);
    app.add_plugins(buiy_core::focus::FocusPlugin);
    app.world_mut().insert_resource(ButtonInput::<KeyCode>::default());
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    support::finish_and_run(&mut app, 0);
    support::register_fixture_font(&mut app, FIXTURE_FAMILY, FIXTURE_FILE);
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert(TEXT_TOKEN.into(), Color::WHITE);
        theme.colors.insert(PLACEHOLDER_TOKEN.into(), Color::srgb(0.55, 0.55, 0.55));
    }
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width_px(W as f32).height_px(H as f32).padding(4.0),
            Text(String::new()),
            FontFamily(FontStack(vec![FamilyEntry::Named(FIXTURE_FAMILY.into())])),
            FontSize(20.0),
            TextColor(ColorToken::Token(Cow::Borrowed(TEXT_TOKEN))),
            TextEditState::for_font_size(20.0),
            Placeholder(String::from(PLACEHOLDER_TEXT)),
        ))
        .id();
    app.update();
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

    if typed {
        let fonts = app.world().resource::<SharedFontSystem>().clone();
        let mut state = app.world_mut().get_mut::<TextEditState>(editor).unwrap();
        let mut fs = fonts.lock();
        state.apply(&mut fs, EditCommand::Insert(TYPED_TEXT.to_string()), true, false);
    }
    app.update();
    app.update();

    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());
    support::wait_for_text_ready(&mut app, 60);
    support::readback_rgba(&mut app, target)
}

/// Count non-background pixels (any pixel with ink) — a crude ink presence
/// classifier. Background is the canvas clear (transparent / black).
fn ink_pixels(px: &[u8]) -> usize {
    px.chunks_exact(4).filter(|p| p[0] as u16 + p[1] as u16 + p[2] as u16 > 24).count()
}

#[test]
#[ignore = "needs a wgpu adapter; run on the GPU lane (CLAUDE.md § GPU lane)"]
fn placeholder_paints_when_empty_and_ink_when_typed() {
    let empty = capture(false);
    let typed = capture(true);

    assert!(ink_pixels(&empty) > 0, "the placeholder paints when empty");
    assert!(ink_pixels(&typed) > 0, "the typed value paints");

    // Determinism: re-capture each state in a fresh app and diff.
    let empty_b = capture(false);
    assert!(perceptual_diff(&empty, &empty_b) < 1e-4, "empty capture is deterministic");
}
```

> Confirm the fixture font name + file (`grep -rn "register_fixture_font" crates/buiy_core/tests | head`)
> and the `Theme` color-insert idiom (the E3 golden uses `theme.colors.insert`). Match
> the real harness — the structure (finish-before-font, camera-after-settle) is the
> load-bearing part.

### Step 8.2 — build-only verification

```sh
cargo test -p buiy_core --test text_placeholder_gpu --no-run
```

Expected: **compiles** (the `#[ignore]` test is built, not run). If you have a local
adapter, optionally:

```sh
cargo test -p buiy_core --test text_placeholder_gpu -- --ignored --test-threads=1
```

### Step 8.3 — commit

```sh
cargo fmt -p buiy_core && cargo clippy -p buiy_core --all-targets -- -D warnings
git add -A && git commit
```

---

## Task 9 — campaign closure (errata fold + spec README flip + docs + follow-ups)

The LAST task — docs-only, grep-verified (the T9 closure discipline). No code.

### Step 9.1 — fold the accumulated errata into the spec section files

Each is a small "As landed (E_n)" blockquote in the relevant `editing-and-ime.md` (or
the named sibling) section, mirroring the existing E3 split-caret note (§ 4.1).

**E1 — `IntrinsicWidths` cache relocation (§ 2.3 / measure-and-layout § 2.3).** The
intrinsics cache moved off `TextBuffer` onto `TextEditState.intrinsics` so it keys to
the authoritative buffer (E1 plan decision 3; `state.rs:79-82`). Add to
`editing-and-ime.md` § 2.2a (after the `TextBufferAccess` paragraph):

```markdown
> **As landed (E1): the intrinsics cache lives on `TextEditState`.** The
> `IntrinsicWidths` cache that measure reads moved off `TextBuffer` onto
> `TextEditState.intrinsics` (`state.rs`) so it keys to the AUTHORITATIVE
> (editor-owned) buffer it describes; `TextBufferAccess` gained editor-first
> `intrinsics()` / `cache_intrinsics()` / `invalidate_intrinsics()` methods.
> A display-only entity's cache stays on its `TextBuffer`. Zero behavior change
> — the cache just keys to the right buffer.
```

**E4 — empty `Change` on a no-op (§ 8).** `finish_change` returns
`Some(Change { items: [] })` for a no-op edit (Backspace at 0); the undo stack drops
it (`undo.rs:108-121`). Add to `editing-and-ime.md` § 8:

```markdown
> **As landed (E4): a no-op edit yields an empty `Change`, dropped at record.**
> cosmic 0.19's `finish_change` returns `Some(Change { items: [] })` (not
> `None`) for an edit that changed nothing — Backspace at offset 0, Delete at
> end (`editor.rs:512`). `UndoStack::record`/`record_grouped` drop an empty
> change, so the stack stays clean and `value_changed` stays false. The replay
> pair (`Change::reverse` + `Edit::apply_change`) is otherwise exactly as
> designed.
```

**E5 — `Window.ime_position` is `Vec2`, not `Option<Vec2>` (§ 6.3).** The popup
position is written as a bare `Vec2` (`ime.rs:452-453`). Add to § 6.3:

```markdown
> **As landed (E5): `Window.ime_position` is `Vec2`, not `Option<Vec2>`.**
> bevy_window 0.18.1 types `ime_position: Vec2` (a plain field, not optional),
> so `write_ime_window` writes the caret bottom-left directly (value-compared to
> avoid re-ticking `Changed<Window>`); there is no "clear to None" — when no
> editor is focused, `ime_enabled` goes false and the stale position is inert.
```

**E3 split-caret note — confirm consistency.** The § 4.1 "As landed (E3)" note and the
follow-ups entry already exist (grounding). Re-read both and confirm they agree (they
do — no edit needed; this is a verification step, not a write).

### Step 9.2 — flip the spec README Status

In `docs/specs/2026-06-09-buiy-text-rendering-design/README.md`:

- Line 4 frontmatter: change `**Status:** implemented (rendering, T1–T9)` to
  `**Status:** implemented (rendering T1–T9 + editing E1–E6)`.
- The § "Status" section (~line 199): change the body so `editing-and-ime.md` is no
  longer "target-state". Replace the sentence
  "`editing-and-ime.md` remains **target-state** for the named successor campaign
  `buiy-text-editing` — the editor/IME surface is designed here, not built." with:

```markdown
[editing-and-ime.md](editing-and-ime.md) is **implemented** — the
`buiy-text-editing` campaign (E1–E6) landed the F-tier editor surface
(`TextEditState` over `Editor<'static>`, the focus-gated keymap, the BiDi caret +
multi-range-shaped selection, the IME display-splice, the arboard clipboard, the
two-stack undo with composition grouping, auto-scroll via `ScrollOffset`,
placeholder, the § 11 taxonomy, and the `buiy_widgets::TextInput` bundle), proven on
the same two-lane suite. The named deferrals (multi-range selection *behavior*,
HTML/image clipboard, the BiDi split-caret secondary indicator) are filed in
[follow-ups.md](../../plans/follow-ups.md).
```

### Step 9.3 — flip the spec README OQ resolutions

In the README "Open questions → From editing-and-ime.md" block (~line 351), annotate
the now-resolved ones with E6 closure italics:

- OQ#1 (frame-ordering): append
  `*E6: realized — the OQ#1 one-frame path landed in E2 (the Input-driven N→N+1 fixture); auto-scroll, caret geometry, and `ime_position` come current the same frame the edit's TextCommit publishes (editing-and-ime § 9, OQ#1).*`
- OQ#3 (arboard HTML): append
  `*E6: still deferred — v1 ships plain-text clipboard (E4); the HTML/image slice is filed in follow-ups.md.*`

### Step 9.4 — update `docs/README.md` catalog statuses

In `docs/README.md`:

- The text-rendering spec entry (line 92): change the trailing clause
  "`editing-and-ime.md` remains target-state for the named successor campaign
  `buiy-text-editing`." to "the editing/IME surface is implemented (the
  `buiy-text-editing` campaign E1–E6)." and keep the `[landed]` tag.
- The text-editing campaign entry (line 109): change `[active]` to `[landed]`.

### Step 9.5 — file the named deferrals in `docs/plans/follow-ups.md`

The BiDi split-caret entry already exists (grounding: "Text editing — BiDi split
caret (E3 deferral)"). Add two new entries after it (the campaign's named deferrals):

```markdown
## Text editing — multi-range selection *behavior* (E-campaign deferral)

**Status:** deferred from the `buiy-text-editing` campaign (E1–E6;
[campaign plan](2026-06-13-buiy-text-editing-campaign.md)).

**What it is:** the `TextSelection` type is multi-range-**shaped** (`primary` +
`secondary: SmallVec<[…; 2]>`, editing-and-ime § 4.2) and the geometry pipeline,
`SelectionChanged` payload, and `::selection` APIs all carry the shape — but v1 ships
single-range **behavior** (`secondary` always empty). Multi-cursor editing (multiple
simultaneous carets/ranges, e.g. Ctrl-click-to-add-caret) is the named next slice.

**Why deferred:** cosmic-text's `Selection` is structurally single-range; multi-range
behavior is Buiy-layer aggregation over N mirrored ranges + N-caret input routing —
a focused slice, cheap because the type is already shaped (no reshape needed).

**Owner:** a focused follow-up slice after E1–E6.

**Spec touchpoint:** editing-and-ime.md §§ 4.2, 13 (named deferral).

## Text editing — HTML + image clipboard flavors (E-campaign deferral)

**Status:** deferred from the `buiy-text-editing` campaign (E4 shipped plain text).

**What it is:** the F row names text + HTML + image MIME for cut/copy/paste; E4 ships
**plain text only** (`Cut`/`Copy` via `copy_selection`, `Paste` through the § 3.3
newline policy) behind the `ClipboardProvider` facade. HTML + image flavors are the
named next slice.

**Why deferred:** arboard's HTML *read-side* support is unverified (editing-and-ime
OQ#3) and must be confirmed before the slice is promised; the facade makes adding
flavors local (no API churn).

**Owner:** a focused follow-up slice; gated on confirming arboard HTML read.

**Spec touchpoint:** editing-and-ime.md §§ 7, 13 (named deferral); OQ#3.
```

> **Compose-over-selection check (E5).** Confirm whether E5 left compose-over-selection
> as a deferral. `grep -rn "compose.*selection\|selection.*preedit" crates/buiy_core/src/text/edit/ime.rs docs/plans/2026-06-13-buiy-text-editing-e5-ime.md`.
> If E5 documented it as deferred, add a third short follow-up entry mirroring the
> above; if E5 handles it (the splice happens at the caret after `delete_selection`,
> or it is simply not a v1 concern), no entry is needed. Resolve this by reading, not
> assuming.

### Step 9.6 — grep-verify the closure

```sh
# The spec README no longer calls editing target-state:
grep -n "target-state" docs/specs/2026-06-09-buiy-text-rendering-design/README.md
# Expect: NO hit referring to editing-and-ime (only the rendering history paragraph, if any).

# The campaign is landed in the catalog:
grep -n "text-editing campaign" docs/README.md
# Expect: the entry tagged [landed].

# Every errata note landed:
grep -n "As landed (E1)\|As landed (E4)\|As landed (E5)" docs/specs/2026-06-09-buiy-text-rendering-design/editing-and-ime.md
# Expect: three hits (§ 2.2a, § 8, § 6.3).

# The named deferrals are filed:
grep -n "multi-range selection \*behavior\*\|HTML + image clipboard flavors" docs/plans/follow-ups.md
# Expect: both entries.

# The facade boundary held across the whole campaign (the lock-in containment check):
cargo test -p buiy_core --test text_facade_boundary
# Expect: green — no symbol outside text::edit names Editor/Edit/Action/Change.
```

### Step 9.7 — commit

```sh
git add -A && git commit
# message: docs(text-editing): E6 closure — errata fold + spec README flip + follow-ups
```

---

## Final gate (orchestrator)

After Task 9, the orchestrator runs both lanes (not the per-task agents):

```sh
# Headless gate (what CI runs — NO --ignored):
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  xvfb-run -a cargo test --workspace

# Additive GPU lane (on a host with an adapter — includes the new Task 8 golden):
cargo test -p buiy_core -j 2 -- --ignored --test-threads=1
```

Both green ⇒ E6 lands ⇒ the editing campaign closes.

---

## Self-review against the spec

**§ 9 (auto-scroll via `ScrollOffset`).** Task 3 — `clamp_into_view` (pure, 4 unit
tests) + `auto_scroll_caret` (x single-line / y multi-line, content-box viewport from
`ResolvedLayout` − border/padding, `ScrollOffset` write, `.after(write_caret_and_selection)`
so it reads the fresh caret and the offset is consumed by the transform bridge the
same frame). `ScrollOffset` does not invalidate Taffy (the spec's load-bearing
property — grounded against the invariant test). `PageUp`/`PageDown` need no separate
handling: E2 lowers them to `Motion::PageUp/PageDown`, and auto-scroll follows the
moved caret. ✔ Covered. Honest caveat surfaced in Task 3: the visual pan needs a
scroll-container `Overflow` (the `TextInput` Style sets `.overflow_hidden()` — Task 6);
the headless test asserts the `ScrollOffset` value, which is overflow-agnostic.

**§ 10 (focus & lifecycle + placeholder).** Task 2 — `focus_lifecycle` with the
`Local<Option<Entity>>` transition detector (the canonical pattern E6 introduces; none
existed). Gain ⇒ blink-origin reset; loss ⇒ undo seal + preedit removal (wiring E5's
deferral) + the **M1 dirty-mark** (`remove_preedit` doesn't invalidate/Taffy-dirty —
without it the orphan glyphs persist a frame) + **selection/buffer RETAINED** (we never
touch `SelectionVisual`). **Caret visibility is owned by `write_caret_blink` (M1)** —
the single focus-aware owner (forces a non-focused editor caret steady-hidden; bare
carets / no-focus harnesses keep the global phase the E3/E5 goldens depend on), because
it runs LATER and drives `CaretVisual.visible` unconditionally so any earlier write
would be clobbered; this also fixes a latent pre-E6 bug (a blurred caret never stopped
blinking). `ime_enabled` is NOT duplicated — E5's `write_ime_window` already decides it.
Task 4 — placeholder as a display-only `PlaceholderBuffer` (shaped by `sync_placeholder`
ITSELF under the lock, M3 — nothing downstream shapes it), painted in
`color.text.placeholder` iff `value().is_empty() && !has_preedit()`, as a SEPARATE
additive producer branch that does NOT feed the §3.2 run-count assert (M4) and adds
only ONE extract-tuple slot (`Option<&PlaceholderBuffer>`, M2 — the tuple was at the
15-cap). Never enters the editor buffer or undo (asserted); shapes to ≥1 run (asserted).
✔ Covered.

**§ 11 (the full Message taxonomy + `EditSubmitted`).** Task 1 — `EditSubmitted` from
the E2 `EditOutcome.submitted` flag. Task 7 — the audit test asserting all nine § 11
Messages are registered. ✔ Covered.

**§ 2.3 (crate split — the `TextInput` widget is `buiy_widgets`).** Task 6 —
`buiy_widgets::TextInput::{single_line, multi_line}` composing core mechanism with
widget policy (sizes, tokens, focus-on-click via `focus_on_click`). Task 5 — the
`TextEditState::for_font_size` core constructor keeps `cosmic_text` OUT of
`buiy_widgets` (the facade boundary the campaign guards). Focus-on-click is widget
policy, never core auto-focus. ✔ Covered.

**§ 13 (the v1 slice checklist).** E6 ticks the LAST items: `ReadOnly`/`Disabled`/
`Placeholder`/`SingleLine` (placeholder now rendered; the markers gate across the
campaign); caret blink with reduced-motion (E3 built; E6 M1 makes `write_caret_blink`
focus-aware + `focus_lifecycle` resets the origin on gain); auto-scroll via
`ScrollOffset` (Task 3); the § 11 taxonomy (Task 1/7); the `TextInput` bundle (Task 6).
The named deferrals (multi-range behavior, HTML/image clipboard, BiDi split caret) are
filed in follow-ups (Task 9). ✔ Covered.

**M1 compatibility check (the `write_caret_blink` change).** `write_caret_blink` is an
EXISTING T7 function the E3/E5 GPU goldens + the headless blink tests exercise. The
focus-aware change is strictly additive: a caret with **no `TextEditState`** (a bare
T7/display caret) or a harness with **no `FocusedEntity` resource** hits the `None`
arm / the `focused.is_some()`-guarded branch and keeps the exact pre-E6 global-phase
behavior — Task 2 Step 2.1's `bare_caret_without_editor_keeps_global_phase` test pins
this, and Step 2.4 re-runs the existing blink tests to prove no regression. The E3/E5
goldens (which spawn a focused editor and pause the clock) get `visible = true` at
phase 0 exactly as before. Only the genuinely-new case — an editor caret that is NOT
the focused entity — changes (now steady-hidden, the correct §10 behavior).

**Type consistency.** Every type named against the real code: `TextEditState`
(private fields; facade methods `value`/`has_preedit`/`remove_preedit`/`blink.reset`/
`undo` via `seal_undo_for_lifecycle`); `EditOutcome.submitted` (E2 flag);
`CaretVisual { visible, rect: Rect }` (E3/T7 seat); `ScrollOffset { x, y }` (layout);
`ResolvedLayout { position, size }`; `Placeholder(String)` (E1); `Metrics::new(f32,
f32)` (cosmic, contained); the `Button::new` bundle precedent + `WidgetsPlugin` body
(verbatim from grounding). Three API shapes were verified against the vendored 0.19
source and the layout crate while writing the plan and the code adjusted to match:
`Buffer::set_text(&str, &Attrs, Shaping, Option<Align>)` + `Buffer::set_metrics(metrics)`
are lock-free (Task 4 takes no FontSystem); `Edges.border`/`.padding` are `Length` not
`f32`, so Task 3 clamps against `ResolvedLayout.size` directly (no `Length`→px
resolution); `A11yRole` has no `TextInput` variant, so Task 6 uses `A11yRole::Text`.
Remaining verify-before-relying spots flagged inline: `ResolvedLayout`/`ScrollOffset`
import paths (Task 3) and the `Theme.colors.insert` idiom + fixture font (Task 8) —
each grounded but worth a final grep at implementation.

**No placeholders.** Every code step is complete and compiles against the grounded
APIs. The two "split state vs pixels" decisions (placeholder headless-state +
GPU-golden pixels; the GPU golden testing the composed bundle in `buiy_core` not
`TextInput` in `buiy_widgets`) are explicit, justified by the dependency-cycle and
render-world-damage-gate realities, and are the campaign's standing discipline.

## Erratum found while grounding (an E6 erratum to fold at this closure)

**`Window.ime_position` typing.** The campaign brief expected the spec to say
`Option<Vec2>`; the spec § 6.3 already says `Vec2` and E5 implemented it as `Vec2`
(`ime.rs:452`). This is the E5 erratum the brief names — confirmed accurate, folded in
Task 9 Step 9.1. **No NEW spec inaccuracy** was found in the E6 surface itself: §§ 9,
10, 11, 2.3, 13 are all consistent with the as-built E1–E5 substrate. The one
substantive design refinement E6 makes beyond the spec letter — `ime_enabled` is owned
solely by E5's `write_ime_window` (focus + markers), so `focus_lifecycle` does NOT
toggle it — is a *non*-duplication that keeps the single source of truth; it is noted
in the `lifecycle.rs` module doc, not a spec contradiction (the spec § 10 says
"`ime_enabled` goes true/false" on the transition, which the marker-only query
realizes every frame, transition included).
