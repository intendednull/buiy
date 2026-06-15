# Buiy Text-Editing E3 — Caret + Selection Model + Painting

**Date:** 2026-06-13
**Phase:** E3 of the text-editing campaign (E1–E6)
**Campaign:** [2026-06-13-buiy-text-editing-campaign.md](2026-06-13-buiy-text-editing-campaign.md) § "E3 — Caret + selection model + painting"
**Spec:** [specs/2026-06-09-buiy-text-rendering-design/editing-and-ime.md](../specs/2026-06-09-buiy-text-rendering-design/editing-and-ime.md) §§ 4.1, 4.2, 4.3, 5, 11
**Branch:** `text-editing-e3` (off `main`, which now includes E1 + E2)
**Repo root:** `/mnt/storage/projects/buiy/.claude/worktrees/render-pipeline`

---

## Goal

Give the editor a **caret** and a **selection** that are visible and mouse-drivable.
E2 already routes keyboard motion/extend into `cosmic_text::Editor` (the editor's
internal `Selection` and `Cursor` move correctly in visual/BiDi order). E3 turns
that internal editor state into:

1. A **Buiy-owned `TextSelection`** type (multi-range-*shaped*; v1 single-range
   behavior) mirrored to/from the editor — the public type every other layer
   (events, a11y, future multi-cursor) will name, so the editor's
   `cosmic_text::Selection` never leaks.
2. **Caret geometry** written into the existing T7 `CaretVisual` paint seat
   (`cursor_position` + line metrics), including the **BiDi split caret** on a
   direction boundary (a second rect, CPU geometry only).
3. **Selection geometry** written into the existing T7 `SelectionVisual` paint
   seat (the producer already sweeps `LayoutRun::highlight` and re-tints glyphs —
   E3 only *produces the endpoints*).
4. **Mouse selection** — focus-on-click, `Click`/`DoubleClick`/`TripleClick`/`Drag`
   with `Word`/`Line` granularity, mapping a pointer position to a `Cursor` via
   `Buffer::hit`.
5. A **per-entity caret blink** (`CaretBlink` phase origin), reworking T7's global
   stateless `write_caret_blink` to be phase-relative and edit/caret-move-resetting,
   reduced-motion steady.
6. The **`SelectionChanged` / `CaretMoved`** Messages, emitted on transition only.

The headless gate stays green; the painted-pixels assertion is one additive
`#[ignore]` GPU golden built on the T7 `text_selection_caret_gpu` harness.

## Architecture

**Where the new state lives.** `TextEditState` (in
`crates/buiy_core/src/text/edit/state.rs`) today carries **only** `editor` +
`intrinsics` — E1 deferred `selection`/`preedit`/`undo`/`blink` (state.rs doc
lines 9–13). E3 adds **two** fields:

- `selection: TextSelection` — the Buiy-owned mirror of the editor's primary
  selection. Lives on `TextEditState` (not a separate component) because it is
  read/written together with `editor` in the same system, under the same `&mut`
  borrow, and the spec § 2.2 component sketch places it there.
- `blink: CaretBlink` — the per-entity blink phase origin (`Duration` since the
  app clock's last caret reset). Lives on `TextEditState` for the same reason:
  the edit/caret-move reset point is exactly where `selection` and the geometry
  are recomputed, all under one `&mut TextEditState`. (The campaign plan calls
  this "E1's `CaretBlink` field (built but inert)" — **that is an erratum: E1
  built no such field**; E3 introduces both the type and the field. See § Errata.)

**The render-prep seam (the new E3 system order).** E3 adds **one** main-world
system, `write_caret_and_selection`, in the **`after(BuiySet::Input)` →
`before(BuiySet::Picking)`** window (the same render-prep window
`write_caret_blink` already uses). It reads each editor's `TextEditState` + its
`ComputedTextLayout` (written by *this frame's* `TextCommit` inside
`BuiySet::Layout`, two sets earlier — so the geometry is current the frame the
edit's reshape publishes, OQ#1's "caret comes current the same frame"), computes
the **single** caret rect + selection geometry, and writes/removes `CaretVisual` /
`SelectionVisual`. It emits `CaretMoved` / `SelectionChanged` on transition.

`write_caret_blink` is **reworked** (not replaced) to read each entity's
`CaretBlink.origin` and drive `CaretVisual.visible` from `now − origin` instead
of raw `time.elapsed()`. It must run **after** `write_caret_and_selection` (which
sets/resets the origin), still before `BuiySet::Picking`.

**The BiDi split caret is DEFERRED out of E3 (deferred-within-F).** The spec § 5
calls for a secondary stamp on a direction boundary ("split caret = two stamps").
E3 paints the **single primary caret only**, and files the split caret as a named
deferral. The reason is a verified API gap, not scope-trimming for convenience:
cosmic-text 0.19's `LayoutRun::cursor_position` → `cursor_glyph` (buffer.rs:148–179)
compares **only** `cursor.line` and `cursor.index` — it never reads
`cursor.affinity`, and a single unwrapped mixed-direction line (e.g. `"abעב"`) is
**one** `LayoutRun`. So there is no "second run at a different x" to find, and
cosmic exposes no API returning the dual BiDi caret positions (its own
`Editor::draw` paints a single caret via a first-run `find_map`). A correct split
caret must walk `run.glyphs` for the two glyphs adjacent to the boundary byte and
take their opposite edges (the `cursor_from_glyph_left`/`cursor_from_glyph_right`
shape, buffer.rs:181–197, which *do* encode affinity) — a materially harder,
separate algorithm with its own test surface. Deferring carries **zero**
correctness risk: the single primary caret is fully functional everywhere; the
split is a mixed-direction visual nicety. Task 7 (closure) files the deferral in
`docs/plans/follow-ups.md`, the campaign plan's E3 deliverable, and spec §§ 5/13,
alongside the existing multi-range-behavior deferral — a documented decision, not
a silent drop.

**Mouse.** There is **no** existing local-hit seam and **no** focus-on-click
(focus.rs is keyboard-only; picking yields only `Hovered(Option<Entity>)`, no
position). E3 builds a `pointer_selection` system in `BuiySet::Input` (alongside
`apply_keyboard_edits`) that: reads `bevy::picking::pointer::PointerLocation` +
mouse-button state, hit-tests via the existing `Hovered` resource, maps the
window-space pointer into buffer-local coords
(`pointer − (GlobalTransform.translation().xy() + ComputedTextLayout.content_offset)`
— the exact term `extract.rs:398` uses), and applies cosmic
`Action::Click/DoubleClick/TripleClick/Drag { x, y }` to the focused editor. It
sets `FocusedEntity` on press (focus-on-click is *core* mechanism here because
the caret/selection are core; the widget's focus *policy* is E6). Double/triple
detection is wall-clock + position-adjacency.

**The mirror direction (load-bearing).** Both keyboard (`apply_keyboard_edits`,
E2) and mouse (`pointer_selection`, E3) drive the **editor's** `Selection`/`Cursor`
as the source of truth (the editor owns BiDi-correct visual motion and
drag-granularity). E3's `write_caret_and_selection` then **reads** the editor
(`selection_bounds()` / `cursor()`) and mirrors *out* into the Buiy
`TextSelection` + the paint seats. So `TextSelection` is a **projection** of the
editor, recomputed each frame the editor changed — never a second source of truth
to keep in sync bidirectionally. (The spec § 4.2 "mirrors its primary into
`set_selection`" is satisfied trivially: the editor already holds it; E3 doesn't
push back.)

## Tech stack

- Rust, Bevy 0.18.1 (Messages = buffered events; `Component`; `MessageReader`/
  `MessageWriter`), cosmic-text 0.19.0.
- cosmic-text APIs E3 calls (verified against vendored
  `cosmic-text-0.19.0/src/{cursor.rs,edit/mod.rs,buffer.rs}`):
  - `Cursor { line: usize, index: usize, affinity: Affinity }`,
    `Cursor::new(line, index)`, `Affinity::{Before(default), After}`.
  - `Edit`: `selection() -> Selection`, `selection_bounds() -> Option<(Cursor,Cursor)>`,
    `cursor() -> Cursor`, `action(&mut self, &mut FontSystem, Action)`,
    `with_buffer(&self, F)`.
  - `LayoutRun::cursor_position(&self, &Cursor) -> Option<f32>`; fields
    `line_i: usize`, `line_top: f32`, `line_height: f32`, `rtl: bool`.
    (Note: `cursor_position` → `cursor_glyph` compares only `line`/`index`, never
    `affinity` — so it yields ONE position even on a direction boundary; this is
    why the BiDi split caret is deferred, see Architecture.)
  - `LayoutRun::highlight(&self, Cursor, Cursor) -> impl Iterator<Item=(f32,f32)>`
    **(x, width)** — already consumed by `extract_buiy_glyphs`.
  - `Buffer::hit(&self, x: f32, y: f32) -> Option<Cursor>`,
    `Buffer::layout_runs(&self) -> LayoutRunIter`.
  - `Action::{Click,DoubleClick,TripleClick,Drag} { x: i32, y: i32 }`.
- The T7 paint seats E3 **drives** (does not rebuild):
  `crates/buiy_core/src/text/components.rs` — `CaretVisual { visible, rect }`
  (:384), `SelectionVisual { start, end }` + `SelectionVisual::new(a,b)` (:420);
  `crates/buiy_core/src/text/extract.rs` — `extract_buiy_glyphs` (selection-quad
  emission :414–480 via `highlight`, per-glyph re-tint :561–580, caret stamp
  :651–684); `crates/buiy_core/src/text/visual.rs` — `write_caret_blink` (:61),
  `blink_phase` (:48), `caret_stamp_rect` (:84), `CaretBlinkInterval` (:32).
- GPU golden harness: `crates/buiy_core/tests/support/mod.rs`
  (`gpu_render_app`, `register_fixture_font`, `render_to_image`,
  `spawn_capture_camera`, `wait_for_text_ready`, `readback_rgba`, `px`),
  `crates/buiy_core/src/render/golden.rs` (`GoldenConfig::deterministic`,
  `perceptual_diff`), precedent `crates/buiy_core/tests/text_selection_caret_gpu.rs`.

## Build & test commands

Per-task fast loop (headless, no adapter):

```sh
cargo test -p buiy_core --test <name>
```

Full headless gate before commit:

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  xvfb-run -a cargo test --workspace
```

GPU lane (orchestrator-run, real adapter — agents only build it):

```sh
cargo test -p buiy_core -j 2 -- --ignored --test-threads=1
```

The new test files are auto-discovered (no `[[test]]` block in
`crates/buiy_core/Cargo.toml`). E3 adds:

- `crates/buiy_core/tests/text_selection_model.rs` — `TextSelection` type + mirror.
- `crates/buiy_core/tests/text_caret_geometry.rs` — caret/selection geometry +
  blink phase + Messages (headless).
- `crates/buiy_core/tests/text_mouse_selection.rs` — mouse → Cursor → selection.
- `crates/buiy_core/tests/text_caret_selection_e3_gpu.rs` — the additive
  `#[ignore]` GPU golden (single caret + selection).

---

## Task 1 — The `TextSelection` type (multi-range-shaped, single-range behavior)

Create the public selection type and its construction from the editor's
`selection_bounds()`. Pure data, no systems — the type other layers name.

### Step 1.1 — Failing test: the type, its ordering, and editor-bounds construction

- [ ] Create `crates/buiy_core/tests/text_selection_model.rs` with:

```rust
//! E3 — the Buiy-owned `TextSelection` type (editing-and-ime § 4.2): the
//! multi-range-SHAPED public selection (v1 single-range behavior — `secondary`
//! always empty), built from the editor's `selection_bounds()` shape, and a
//! collapsed-selection caret-position. Pure-data headless tests.

use buiy_core::text::edit::{SelectionRange, TextSelection};
use cosmic_text::Cursor;

#[test]
fn selection_range_orders_anchor_and_active() {
    // active < anchor (a backward drag): ordered() yields (active, anchor).
    let r = SelectionRange {
        anchor: Cursor::new(0, 8),
        active: Cursor::new(0, 3),
    };
    let (lo, hi) = r.ordered();
    assert_eq!((lo.line, lo.index), (0, 3));
    assert_eq!((hi.line, hi.index), (0, 8));
    assert!(!r.is_collapsed());

    // A forward drag orders the other way.
    let f = SelectionRange {
        anchor: Cursor::new(0, 3),
        active: Cursor::new(1, 1),
    };
    let (lo, hi) = f.ordered();
    assert_eq!((lo.line, lo.index, hi.line, hi.index), (0, 3, 1, 1));
}

#[test]
fn collapsed_range_is_a_caret() {
    let c = SelectionRange {
        anchor: Cursor::new(2, 5),
        active: Cursor::new(2, 5),
    };
    assert!(c.is_collapsed());
    let (lo, hi) = c.ordered();
    assert_eq!((lo.line, lo.index), (hi.line, hi.index));
}

#[test]
fn text_selection_v1_is_single_range() {
    let sel = TextSelection::collapsed(Cursor::new(0, 4));
    assert!(sel.primary.is_collapsed());
    assert!(sel.secondary.is_empty(), "v1 behavior: secondary always empty");
    assert!(sel.is_collapsed());

    let ranged = TextSelection::from_bounds(Cursor::new(0, 1), Cursor::new(0, 6), Cursor::new(0, 6));
    assert!(!ranged.is_collapsed());
    assert!(ranged.secondary.is_empty());
    // active is the moving endpoint (here the end); anchor the held one.
    assert_eq!((ranged.primary.active.line, ranged.primary.active.index), (0, 6));
    assert_eq!((ranged.primary.anchor.line, ranged.primary.anchor.index), (0, 1));
}
```

- [ ] Run it — expect a compile failure (the type does not exist yet):

```sh
cargo test -p buiy_core --test text_selection_model
```

Expected: `error[E0432]: unresolved import` for `SelectionRange` / `TextSelection`.

### Step 1.2 — Minimal impl: `selection.rs` in the edit module

- [ ] Create `crates/buiy_core/src/text/edit/selection.rs`:

```rust
//! `TextSelection` — the Buiy-owned, multi-range-SHAPED selection type
//! (editing-and-ime § 4.2). v1 ships single-range *behavior* (`secondary`
//! always empty), but the public type, the `SelectionChanged` payload, and the
//! geometry pipeline are multi-range-shaped so the multi-cursor next slice
//! (§ 13) is additive, not a reshape. This type is a PROJECTION of the editor's
//! single `cosmic_text::Selection` (the editor owns BiDi-correct motion); the
//! input systems drive the editor, and `write_caret_and_selection` mirrors the
//! editor OUT into this type each frame the editor changed (architecture note,
//! E3 plan § Architecture "mirror direction"). It names ONE cosmic type —
//! `Cursor`, pure-data — so the facade-boundary tripwire (`Editor`/`Edit`/
//! `Action`/`Change`) does not flag it.

use cosmic_text::Cursor;
use smallvec::SmallVec;

/// One contiguous selection range: a held `anchor` and a moving `active`
/// endpoint (the caret end). A collapsed range (`anchor == active`) IS a caret.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct SelectionRange {
    /// The fixed end (where the drag/extend started).
    pub anchor: Cursor,
    /// The moving end (the live caret).
    pub active: Cursor,
}

impl SelectionRange {
    /// The endpoints in document order (`lo ≤ hi`, `(line, index)`
    /// lexicographic — the `selection_bounds()` ordering). Direction-agnostic;
    /// geometry sweeps use this, the caret uses `active`.
    pub fn ordered(&self) -> (Cursor, Cursor) {
        if (self.active.line, self.active.index) < (self.anchor.line, self.anchor.index) {
            (self.active, self.anchor)
        } else {
            (self.anchor, self.active)
        }
    }

    /// `anchor == active` (position-wise): paints nothing; the caret is `active`.
    pub fn is_collapsed(&self) -> bool {
        (self.anchor.line, self.anchor.index) == (self.active.line, self.active.index)
    }
}

/// The full selection: a `primary` range plus `secondary` ranges for the
/// multi-cursor next slice. **v1: `secondary` is always empty.**
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct TextSelection {
    pub primary: SelectionRange,
    pub secondary: SmallVec<[SelectionRange; 2]>,
}

impl TextSelection {
    /// A collapsed selection (a bare caret) at `caret`.
    pub fn collapsed(caret: Cursor) -> Self {
        Self {
            primary: SelectionRange {
                anchor: caret,
                active: caret,
            },
            secondary: SmallVec::new(),
        }
    }

    /// Build from the editor's `selection_bounds()` ordered pair plus the live
    /// `active` cursor (so the anchor — the OTHER bound — and the moving end are
    /// distinguished for direction-aware undo/extend later). `active` is one of
    /// `lo`/`hi`; the anchor is the other.
    pub fn from_bounds(lo: Cursor, hi: Cursor, active: Cursor) -> Self {
        let anchor = if (active.line, active.index) == (hi.line, hi.index) {
            lo
        } else {
            hi
        };
        Self {
            primary: SelectionRange { anchor, active },
            secondary: SmallVec::new(),
        }
    }

    /// The whole selection collapses to a caret (no painted range anywhere).
    pub fn is_collapsed(&self) -> bool {
        self.primary.is_collapsed() && self.secondary.is_empty()
    }
}
```

- [ ] `smallvec` is **already a direct dependency** of `buiy_core`
  (`crates/buiy_core/Cargo.toml`: `smallvec = { workspace = true }`, used by the
  extract path) — **no Cargo.toml change is needed**; `use smallvec::SmallVec;`
  resolves. (Confirm with `rg -n '^smallvec' crates/buiy_core/Cargo.toml` if you
  want — it is present. No `cargo deny` re-run is triggered.)

- [ ] Wire the module + re-exports. In `crates/buiy_core/src/text/edit/mod.rs`,
  add `mod selection;` (after `mod keymap;`) and extend the `pub use`:

```rust
pub use selection::{SelectionRange, TextSelection};
```

- [ ] Re-export from the text module. In `crates/buiy_core/src/text/mod.rs`, the
  `pub use edit::{ ... }` block (around line 53–56) gains `SelectionRange,
  TextSelection,` so `buiy_core::text::edit::TextSelection` resolves (the test
  imports it via `edit::`).

### Step 1.3 — Run it green + commit

- [ ] Run:

```sh
cargo test -p buiy_core --test text_selection_model
```

Expected: `test result: ok. 3 passed`.

- [ ] Commit:

```sh
git add -A && git commit -m "feat(text-editing): E3 Task 1 — TextSelection type (multi-range-shaped)"
```

---

## Task 2 — `TextEditState` gains `selection` + `blink`; the `CaretBlink` type

Add the two deferred fields and the per-entity blink phase origin. Pure state +
accessors; the system that consumes them is Task 3/5.

### Step 2.1 — Failing test: the fields exist, blink origin resets, selection round-trips

- [ ] Create `crates/buiy_core/tests/text_caret_geometry.rs` (this file grows
  across Tasks 2/3/4/5/6 — start it here):

```rust
//! E3 headless — caret/selection geometry, the per-entity blink phase, the
//! split caret, and the SelectionChanged/CaretMoved Messages (editing-and-ime
//! §§ 4.1, 4.3, 5, 11). No GPU: the geometry is pure CPU math over a shaped
//! buffer; pixels are the additive `_gpu` golden.

use std::time::Duration;

use bevy::prelude::*;
use buiy_core::text::edit::{CaretBlink, TextEditState};
use cosmic_text::{Edit, Metrics};

#[test]
fn caret_blink_origin_defaults_to_zero_and_resets() {
    let mut blink = CaretBlink::default();
    assert_eq!(blink.origin, Duration::ZERO, "fresh caret blinks from t=0");
    // Reset stamps the current app-clock instant as the new phase origin.
    blink.reset(Duration::from_millis(1234));
    assert_eq!(blink.origin, Duration::from_millis(1234));
    // The phase is measured RELATIVE to the origin.
    assert_eq!(blink.phase_elapsed(Duration::from_millis(1734)), Duration::from_millis(500));
    // A `now` before the origin (clock paused/rewound in tests) saturates to 0
    // rather than underflowing.
    assert_eq!(blink.phase_elapsed(Duration::from_millis(1000)), Duration::ZERO);
}

#[test]
fn text_edit_state_mirrors_editor_selection_into_text_selection() {
    let fonts = buiy_core::text::SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    // Type "hello", then select-all so the editor holds a Normal selection.
    state.apply(
        &mut fs,
        buiy_core::text::edit::EditCommand::Insert("hello".into()),
        false,
        false,
    );
    state.apply(&mut fs, buiy_core::text::edit::EditCommand::SelectAll, false, false);
    drop(fs);

    let sel = state.mirror_selection();
    assert!(!sel.is_collapsed(), "select-all is a non-empty selection");
    let (lo, hi) = sel.primary.ordered();
    assert_eq!((lo.line, lo.index), (0, 0));
    assert_eq!((hi.line, hi.index), (0, 5));

    // With no selection (collapse), the mirror is a caret at the cursor.
    let mut fs = fonts.lock();
    state.apply(
        &mut fs,
        buiy_core::text::edit::EditCommand::Motion(cosmic_text::Motion::Home, false),
        false,
        false,
    );
    drop(fs);
    let sel = state.mirror_selection();
    assert!(sel.is_collapsed(), "collapsed selection is a caret");
    assert_eq!((sel.primary.active.line, sel.primary.active.index), (0, 0));
}
```

- [ ] Run — expect compile failure (`CaretBlink`, `mirror_selection` missing):

```sh
cargo test -p buiy_core --test text_caret_geometry
```

Expected: `error[E0432]`/`error[E0599]` for `CaretBlink` and `mirror_selection`.

### Step 2.2 — Minimal impl: `CaretBlink`, the new fields, and the mirror accessor

- [ ] Add `CaretBlink` to `crates/buiy_core/src/text/edit/state.rs`. Put it after
  the imports, before `TextEditState`:

```rust
use std::time::Duration;

/// The per-entity caret-blink phase origin (editing-and-ime §§ 5, 10). The T7
/// `write_caret_blink` writer was deliberately GLOBAL + stateless — a square
/// wave of the raw app clock (visual.rs module doc: "per-entity phase reset on
/// edit/caret-move is the editing campaign's `CaretBlink` state"). E3 lands that
/// state: the blink is phase-relative to `origin`, which is RESET to the current
/// app-clock instant on every edit and caret move, so the caret is always
/// solid-on for one half-period immediately after the user acts (web parity).
/// Reduced-motion steadiness is the writer's concern, not this type's.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub struct CaretBlink {
    /// App-clock instant (`Time::elapsed()`) of the last reset. The blink phase
    /// is `now − origin`. Default `ZERO` = "blink from the start of time" (a
    /// caret that has never moved blinks on the global phase, harmless).
    pub origin: Duration,
}

impl CaretBlink {
    /// Stamp `now` as the new phase origin (call on edit / caret move).
    pub fn reset(&mut self, now: Duration) {
        self.origin = now;
    }

    /// Elapsed time since the origin, saturating (a paused/rewound test clock
    /// where `now < origin` yields `ZERO`, never an underflow panic).
    pub fn phase_elapsed(&self, now: Duration) -> Duration {
        now.saturating_sub(self.origin)
    }
}
```

- [ ] Add the two fields to `TextEditState`. Replace the struct body (state.rs
  ~:38–49) so it reads:

```rust
#[derive(Component)]
pub struct TextEditState {
    /// The wrapped editor over `BufferRef::Owned`. Private: the only way to
    /// reach its buffer from outside `text::edit` is `TextBufferAccess`.
    pub(crate) editor: Editor<'static>,
    /// Cached intrinsic widths for the AUTHORITATIVE (editor-owned) buffer
    /// (E1 plan decision 3). `None` until measure computes them.
    pub(crate) intrinsics: Option<IntrinsicWidths>,
    /// The Buiy-owned mirror of the editor's primary selection (§ 4.2). A
    /// PROJECTION: recomputed from the editor by `write_caret_and_selection`
    /// each frame the editor changed; never a second source of truth. Read by
    /// the geometry writer + the `SelectionChanged` emitter.
    pub(crate) selection: TextSelection,
    /// The per-entity caret-blink phase origin (§§ 5, 10). Reset on edit /
    /// caret move by `write_caret_and_selection`; read by `write_caret_blink`.
    pub(crate) blink: CaretBlink,
}
```

- [ ] Update `TextEditState::new` (state.rs ~:57) to seed both fields. The fresh
  editor's cursor is `(0,0)`:

```rust
    pub fn new(metrics: Metrics) -> Self {
        Self {
            editor: Editor::new(Buffer::new_empty(metrics)),
            intrinsics: None,
            selection: TextSelection::collapsed(cosmic_text::Cursor::new(0, 0)),
            blink: CaretBlink::default(),
        }
    }
```

- [ ] Add `use crate::text::edit::selection::TextSelection;` to state.rs imports.
  (state.rs and selection.rs are sibling modules; `super::selection::TextSelection`
  also works — use whichever the crate's existing intra-module style prefers; the
  rest of the edit module uses `super::`.) Concretely add near the top:

```rust
use super::selection::TextSelection;
```

- [ ] Add the `mirror_selection` accessor + `blink` access to the `impl
  TextEditState` block (after `value()`), reading the editor's bounds/cursor:

```rust
    /// Project the editor's current `Selection` + cursor into a Buiy
    /// `TextSelection` (§ 4.2). The editor owns BiDi-correct motion; this is the
    /// READ-OUT mirror. A `selection_bounds()` of `None` (no selection) is a
    /// collapsed caret at the live cursor.
    pub fn mirror_selection(&self) -> TextSelection {
        use cosmic_text::Edit;
        let active = self.editor.cursor();
        match self.editor.selection_bounds() {
            Some((lo, hi)) if (lo.line, lo.index) != (hi.line, hi.index) => {
                TextSelection::from_bounds(lo, hi, active)
            }
            _ => TextSelection::collapsed(active),
        }
    }

    /// The live caret (the editor's cursor) — the `active` endpoint.
    pub fn caret(&self) -> cosmic_text::Cursor {
        use cosmic_text::Edit;
        self.editor.cursor()
    }
```

- [ ] Export the new types. In `crates/buiy_core/src/text/edit/mod.rs`, extend
  the `state` re-export line:

```rust
pub use state::{CaretBlink, Disabled, Placeholder, ReadOnly, SingleLine, TextEditState};
```

  and in `crates/buiy_core/src/text/mod.rs` add `CaretBlink,` to the `pub use
  edit::{ ... }` block.

### Step 2.3 — Run it green + commit

- [ ] Run:

```sh
cargo test -p buiy_core --test text_caret_geometry
```

Expected: `test result: ok. 2 passed`.

- [ ] Sanity: the facade boundary still holds (no new cosmic type leaked outside
  `text/edit/`):

```sh
cargo test -p buiy_core --test text_facade_boundary
```

Expected: `ok`.

- [ ] Commit:

```sh
git add -A && git commit -m "feat(text-editing): E3 Task 2 — TextEditState.selection + CaretBlink fields + mirror"
```

---

## Task 3 — Caret + selection geometry writer + the Messages

The render-prep system that turns editor state into `CaretVisual` /
`SelectionVisual`, resets the blink, and emits `CaretMoved` / `SelectionChanged`.
Mouse is Task 4; blink rework is Task 5. (E3 paints the single primary caret; the
BiDi split caret is a documented deferral — see Architecture + Task 7 closure.)

### Step 3.1 — Failing test: geometry writer drives the paint seats + Messages

- [ ] Append to `crates/buiy_core/tests/text_caret_geometry.rs`:

```rust
use buiy_core::text::edit::{CaretMoved, SelectionChanged};
use buiy_core::text::{CaretVisual, ComputedTextLayout, SelectionVisual};
use buiy_core::FocusedEntity;
use buiy_core::layout::{LayoutPlugin, Style};
use buiy_core::{CorePlugin, Node};
use buiy_core::text::{BuiyTextPlugin, Text};
use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonInput, ButtonState};

/// A headless app that runs the full text pipeline (TextSync → measure →
/// TextCommit) plus E3's render-prep window, with focus + input wired so the
/// editor can be driven by synthetic KeyboardInput. Returns (app, window).
fn caret_app() -> (App, Entity) {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(CorePlugin)
        .add_plugins(buiy_core::focus::FocusPlugin)
        .add_plugins(LayoutPlugin)
        .add_plugins(BuiyTextPlugin::default());
    app.add_message::<KeyboardInput>();
    app.insert_resource(ButtonInput::<KeyCode>::default());
    let window = app.world_mut().spawn(()).id();
    (app, window)
}

fn spawn_editor(app: &mut App, text: &str) -> Entity {
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from(text)),
            TextEditState::new(Metrics::new(16.0, 19.2)),
        ))
        .id();
    // A sized parent so layout produces a real ComputedTextLayout.
    app.world_mut()
        .spawn((
            Node,
            Style::default().flex_row().width_px(400.0).height_px(40.0),
        ))
        .add_child(editor);
    editor
}

fn type_char(app: &mut App, window: Entity, ch: &str) {
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyA,
        logical_key: Key::Character(ch.into()),
        state: ButtonState::Pressed,
        text: Some(ch.into()),
        repeat: false,
        window,
    });
    app.update();
}

#[test]
fn focused_editor_gets_a_caret_visual_with_a_real_rect() {
    let (mut app, window) = caret_app();
    let editor = spawn_editor(&mut app, "");
    app.update(); // settle spawn + first layout
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

    type_char(&mut app, window, "h");
    type_char(&mut app, window, "i");
    app.update(); // OQ#1: the edit reshaped at N; geometry comes current here

    let caret = app
        .world()
        .get::<CaretVisual>(editor)
        .expect("a focused editor has a caret");
    // Non-degenerate rect: the caret bar has the line-box height and sits to
    // the RIGHT of x=0 (after "hi"), with a positive width.
    assert!(caret.rect.height() > 1.0, "caret spans the line box: {:?}", caret.rect);
    assert!(caret.rect.min.x > 0.0, "caret after 'hi' is right of origin: {:?}", caret.rect);
    assert!(caret.rect.width() >= 1.0, "caret has a >=1px bar width: {:?}", caret.rect);
}

#[test]
fn selection_writes_selection_visual_and_collapse_removes_it() {
    let (mut app, window) = caret_app();
    let editor = spawn_editor(&mut app, "");
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    type_char(&mut app, window, "h");
    type_char(&mut app, window, "i");

    // Select-all (Ctrl/Cmd-A) — platform-correct modifier.
    let cmd = if cfg!(target_os = "macos") { KeyCode::SuperLeft } else { KeyCode::ControlLeft };
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().press(cmd);
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::KeyA,
        logical_key: Key::Character("a".into()),
        state: ButtonState::Pressed,
        text: Some("a".into()),
        repeat: false,
        window,
    });
    app.update();
    app.world_mut().resource_mut::<ButtonInput<KeyCode>>().release(cmd);
    app.update();

    let sel = app
        .world()
        .get::<SelectionVisual>(editor)
        .expect("a non-empty selection paints");
    assert!(!sel.is_collapsed());
    assert_eq!((sel.start.line, sel.start.index), (0, 0));
    assert_eq!((sel.end.line, sel.end.index), (0, 2)); // "hi" = 2 bytes

    // Collapse via Home (a non-extend motion) — SelectionVisual is REMOVED.
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::Home,
        logical_key: Key::Home,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
    app.update();
    app.update();
    assert!(
        app.world().get::<SelectionVisual>(editor).is_none(),
        "collapsing the selection removes the paint seat"
    );
}

#[test]
fn caret_move_and_selection_change_emit_messages_on_transition_only() {
    let (mut app, window) = caret_app();
    let editor = spawn_editor(&mut app, "");
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    app.add_message::<CaretMoved>();
    app.add_message::<SelectionChanged>();

    // Collect emitted Messages across a frame via a draining reader.
    fn drain_caret(app: &mut App) -> Vec<Entity> {
        let mut out = Vec::new();
        app.world_mut().resource_scope(|_w, mut msgs: Mut<Messages<CaretMoved>>| {
            let mut cursor = msgs.get_cursor();
            for m in cursor.read(&msgs) {
                out.push(m.0);
            }
        });
        out
    }
    fn drain_sel(app: &mut App) -> usize {
        let mut n = 0;
        app.world_mut().resource_scope(|_w, mut msgs: Mut<Messages<SelectionChanged>>| {
            let mut cursor = msgs.get_cursor();
            n = cursor.read(&msgs).count();
        });
        n
    }

    type_char(&mut app, window, "a");
    type_char(&mut app, window, "b");
    // Typing moves the caret: CaretMoved fired for this editor.
    assert!(!drain_caret(&mut app).is_empty(), "typing moves the caret");

    // A pure left motion: CaretMoved, no SelectionChanged (selection stays empty).
    app.world_mut().write_message(KeyboardInput {
        key_code: KeyCode::ArrowLeft,
        logical_key: Key::ArrowLeft,
        state: ButtonState::Pressed,
        text: None,
        repeat: false,
        window,
    });
    app.update();
    assert!(!drain_caret(&mut app).is_empty(), "ArrowLeft moves the caret");
    assert_eq!(drain_sel(&mut app), 0, "no selection change on a bare motion");

    // An IDLE frame (no input): neither Message fires (transition-only).
    app.update();
    assert!(drain_caret(&mut app).is_empty(), "idle frame: no CaretMoved");
    assert_eq!(drain_sel(&mut app), 0, "idle frame: no SelectionChanged");
}
```

- [ ] Run — expect compile failure (`CaretMoved`/`SelectionChanged`,
  `write_caret_and_selection` missing) and, once those resolve, an assertion
  failure (no system writes `CaretVisual` yet):

```sh
cargo test -p buiy_core --test text_caret_geometry
```

Expected first: `error[E0432]` for `CaretMoved`/`SelectionChanged`.

### Step 3.2 — Minimal impl: the geometry writer + Messages

- [ ] Create `crates/buiy_core/src/text/edit/caret.rs`:

```rust
//! E3 — the caret + selection geometry writer (editing-and-ime §§ 4.1, 4.3, 5,
//! 11). One render-prep system, `write_caret_and_selection`, runs in the
//! `after(BuiySet::Input) .. before(BuiySet::Picking)` window — two sets after
//! Layout, so it reads THIS frame's `ComputedTextLayout` (the OQ#1 "caret comes
//! current the same frame the edit publishes"). It mirrors each focused, non-
//! Disabled editor's selection OUT into `TextSelection` + the T7 paint seats
//! (`CaretVisual`, `SelectionVisual`), resets the blink phase on a caret/
//! selection transition, and emits `CaretMoved` / `SelectionChanged` on
//! transition only. E3 paints the SINGLE primary caret; the BiDi split caret is
//! a named deferral (cosmic 0.19 exposes no dual-caret API — see the plan's
//! Architecture + the follow-up filed in Task 7).
//!
//! This file NAMES `Edit` (it reads the editor through `cursor()` /
//! `with_buffer`), so it MUST stay inside the facade (the boundary tripwire).

use bevy::math::Rect;
use bevy::prelude::*;
use cosmic_text::{Cursor, Edit};

use super::state::TextEditState;
use super::selection::TextSelection;
use crate::text::{CaretVisual, SelectionVisual};

/// Emitted when the caret position changes without a selection change
/// (editing-and-ime § 11 row `CaretMoved`). Payload: the entity + the new
/// cursor.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct CaretMoved(pub Entity, pub Cursor);

/// Emitted when the selection transitions (editing-and-ime § 11 row
/// `SelectionChanged`). Payload: the entity + the new (multi-range-shaped)
/// selection.
#[derive(Message, Debug, Clone, PartialEq, Eq)]
pub struct SelectionChanged(pub Entity, pub TextSelection);

/// The caret bar width in logical px (snapped to >=1 physical px at paint by
/// `caret_stamp_rect`). A thin vertical bar; 1.0 logical is the convention.
const CARET_W: f32 = 1.0;

/// The caret rect for `caret` in CONTENT-BOX-LOCAL coords (logical px), from the
/// run that owns the cursor: `cursor_position` gives x, `line_top`/`line_height`
/// give y/height (§ 4.1). Returns `None` if no run owns the cursor (degenerate /
/// not-yet-shaped buffer).
fn caret_rect_for(buffer: &cosmic_text::Buffer, caret: &Cursor) -> Option<Rect> {
    for run in buffer.layout_runs() {
        if let Some(x) = run.cursor_position(caret) {
            return Some(Rect::new(x, run.line_top, x + CARET_W, run.line_top + run.line_height));
        }
    }
    None
}

/// Render-prep: drive every focused, non-`Disabled` editor's caret + selection
/// paint seats from the editor state, emit transition Messages, reset blink.
#[allow(clippy::type_complexity)]
pub fn write_caret_and_selection(
    mut commands: Commands,
    time: Res<Time>,
    focused: Option<Res<crate::FocusedEntity>>,
    mut editors: Query<
        (
            Entity,
            &mut TextEditState,
            Option<&CaretVisual>,
            Option<&SelectionVisual>,
        ),
        Without<super::state::Disabled>,
    >,
    mut caret_moved: MessageWriter<CaretMoved>,
    mut selection_changed: MessageWriter<SelectionChanged>,
) {
    let Some(focused) = focused else { return };
    let Some(focused_entity) = focused.0 else { return };

    for (entity, mut state, prev_caret, prev_sel) in &mut editors {
        // Only the focused editor shows a caret (§ 10). A non-focused editor
        // keeps its SelectionVisual (web-parity retention is E6; here a
        // non-focused editor simply isn't recomputed — its seats persist).
        if entity != focused_entity {
            continue;
        }

        let new_sel = state.mirror_selection();
        let caret = new_sel.primary.active;

        // --- Caret geometry into CaretVisual ---------------------------------
        let new_rect = state
            .with_buffer(|buffer| caret_rect_for(buffer, &caret))
            .unwrap_or(Rect::from_corners(Vec2::ZERO, Vec2::new(CARET_W, 0.0)));

        let caret_changed = prev_caret.map(|c| c.rect) != Some(new_rect);
        if caret_changed {
            // Preserve the current visibility (the blink writer owns it); a NEW
            // caret defaults visible.
            let visible = prev_caret.map(|c| c.visible).unwrap_or(true);
            commands.entity(entity).insert(CaretVisual { visible, rect: new_rect });
        }

        // --- Selection geometry into SelectionVisual -------------------------
        let prev_sel_endpoints = prev_sel.map(|s| (s.start, s.end));
        let (sel_present, new_sel_endpoints) = if new_sel.is_collapsed() {
            (false, None)
        } else {
            let (lo, hi) = new_sel.primary.ordered();
            (true, Some((lo, hi)))
        };
        if sel_present {
            let (lo, hi) = new_sel_endpoints.unwrap();
            commands.entity(entity).insert(SelectionVisual::new(lo, hi));
        } else if prev_sel.is_some() {
            commands.entity(entity).remove::<SelectionVisual>();
        }

        // --- Transition detection + Messages + blink reset -------------------
        let selection_transitioned = {
            let prev_norm = prev_sel_endpoints.map(|(s, e)| (
                (s.line, s.index), (e.line, e.index)
            ));
            let new_norm = new_sel_endpoints.map(|(s, e)| (
                (s.line, s.index), (e.line, e.index)
            ));
            prev_norm != new_norm
        };

        if selection_transitioned {
            selection_changed.write(SelectionChanged(entity, new_sel.clone()));
            state.blink.reset(time.elapsed());
            state.selection = new_sel;
        } else if caret_changed {
            // Caret moved without a selection change (§ 11 CaretMoved).
            caret_moved.write(CaretMoved(entity, caret));
            state.blink.reset(time.elapsed());
            state.selection = new_sel;
        }
    }
}
```

- [ ] Wire the module + re-exports. In
  `crates/buiy_core/src/text/edit/mod.rs`: add `mod caret;` and

```rust
pub use caret::{CaretMoved, SelectionChanged, write_caret_and_selection};
```

  In `crates/buiy_core/src/text/mod.rs` add `CaretMoved, SelectionChanged,
  write_caret_and_selection,` to the `pub use edit::{ ... }` block.

- [ ] Register the system + Messages in `BuiyTextPlugin::build`
  (`crates/buiy_core/src/text/mod.rs`), in the render-prep window. Insert AFTER
  the existing `write_caret_blink` registration block (~:176–181) so the ordering
  is explicit:

```rust
        // E3 (editing-and-ime §§ 4.1, 4.3, 5, 11): the caret + selection
        // geometry writer — mirrors editor state into the T7 paint seats,
        // resets the per-entity blink, emits CaretMoved/SelectionChanged. Runs
        // in the render-prep window, BEFORE write_caret_blink (which reads the
        // CaretBlink origin this system resets).
        app.add_message::<crate::text::edit::CaretMoved>();
        app.add_message::<crate::text::edit::SelectionChanged>();
        app.add_systems(
            Update,
            crate::text::edit::write_caret_and_selection
                .after(crate::BuiySet::Input)
                .before(crate::text::visual::write_caret_blink),
        );
```

  And change the existing `write_caret_blink` registration so it is ordered
  after this writer and still before Picking (replace the existing
  `.add_systems(Update, visual::write_caret_blink.after(Animate).before(Picking))`
  call — the `.before(write_caret_blink)` above already chains them; keep
  `write_caret_blink`'s `.before(BuiySet::Picking)`). Net order:
  `Input < write_caret_and_selection < write_caret_blink < Picking`.

> The `visual` module is `pub(crate)` within `text`; referencing
> `crate::text::visual::write_caret_blink` from the plugin (same crate) is fine.
> If `visual` is private, make it `pub(crate)` or add a re-export — verify with
> `rg -n "mod visual" crates/buiy_core/src/text/mod.rs` (it is already
> `pub use visual::{...}` at text/mod.rs, so the function path resolves).

### Step 3.3 — Run it green + commit

- [ ] Run:

```sh
cargo test -p buiy_core --test text_caret_geometry
```

Expected: `test result: ok. 5 passed` (Task 2's 2 + Task 3's 3).

- [ ] Facade boundary intact:

```sh
cargo test -p buiy_core --test text_facade_boundary
```

Expected: `ok`.

- [ ] Commit:

```sh
git add -A && git commit -m "feat(text-editing): E3 Task 3 — caret/selection geometry writer + CaretMoved/SelectionChanged"
```

---


## Task 4 — Mouse selection: focus-on-click + Click/DoubleClick/TripleClick/Drag

A `BuiySet::Input` system mapping pointer position → buffer-local → cosmic
`Action`. Focus-on-click sets `FocusedEntity`. Double/triple = wall-clock +
adjacency.

### Step 4.1 — Failing test: a synthetic click sets focus + caret; a drag extends

- [ ] Create `crates/buiy_core/tests/text_mouse_selection.rs`. Mouse picking in a
  headless app is awkward (no real `PointerLocation`), so E3 exposes a **pure
  mapping helper** `pointer_to_cursor` and an **action-applying helper**
  `apply_pointer_gesture` that the system uses, and the test drives those + the
  click-classifier directly. The system-level wiring (reading
  `PointerLocation`/`Hovered`) is integration-tested where a window exists; the
  headless test pins the *logic*:

```rust
//! E3 headless — the mouse-selection LOGIC (editing-and-ime § 4, mouse
//! Click/DoubleClick/TripleClick/Drag): the window→buffer-local mapping, the
//! click-count classifier, and the gesture→Action application. The full
//! PointerLocation/Hovered wiring is GPU/windowed; here we pin the platform-
//! independent geometry + state machine.

use std::time::Duration;

use bevy::math::Vec2;
use buiy_core::text::edit::{ClickTracker, PointerGesture, TextEditState, pointer_to_cursor};
use cosmic_text::{Edit, Metrics};

#[test]
fn pointer_maps_to_a_cursor_via_buffer_local_coords() {
    let fonts = buiy_core::text::SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    state.apply(&mut fs, buiy_core::text::edit::EditCommand::Insert("hello".into()), false, false);
    // Shape: a hit at far-left (x≈0) lands at index 0; a hit far-right lands at
    // the end (index 5). content_offset/origin are zero here (no node).
    let origin = Vec2::ZERO;
    let pointer = Vec2::new(0.0, 8.0); // left edge, on the single line
    let cursor = state.with_buffer(|b| pointer_to_cursor(b, pointer, origin)).unwrap();
    assert_eq!((cursor.line, cursor.index), (0, 0));

    let pointer = Vec2::new(1000.0, 8.0); // far right ⇒ clamps to line end
    let cursor = state.with_buffer(|b| pointer_to_cursor(b, pointer, origin)).unwrap();
    assert_eq!((cursor.line, cursor.index), (0, 5));
    drop(fs);
}

#[test]
fn click_tracker_classifies_single_double_triple_by_time_and_adjacency() {
    let mut t = ClickTracker::default();
    let near = Vec2::new(10.0, 10.0);
    // First click ⇒ single.
    assert_eq!(t.classify(near, Duration::from_millis(0)), PointerGesture::Click);
    // Within the double window + near ⇒ double.
    assert_eq!(t.classify(near, Duration::from_millis(200)), PointerGesture::DoubleClick);
    // Again within the window + near ⇒ triple.
    assert_eq!(t.classify(near, Duration::from_millis(380)), PointerGesture::TripleClick);
    // A 4th rolls back to single (triple is the max).
    assert_eq!(t.classify(near, Duration::from_millis(520)), PointerGesture::Click);
    // A far click resets to single even within the time window.
    let far = Vec2::new(200.0, 10.0);
    assert_eq!(t.classify(near, Duration::from_millis(600)), PointerGesture::Click);
    assert_eq!(t.classify(far, Duration::from_millis(650)), PointerGesture::Click);
    // A click after the window lapses ⇒ single.
    assert_eq!(t.classify(near, Duration::from_millis(2000)), PointerGesture::Click);
}

#[test]
fn gesture_applies_the_matching_cosmic_action_and_moves_the_caret() {
    let fonts = buiy_core::text::SharedFontSystem::new();
    let mut state = TextEditState::new(Metrics::new(16.0, 19.2));
    let mut fs = fonts.lock();
    state.apply(&mut fs, buiy_core::text::edit::EditCommand::Insert("hello world".into()), false, false);

    // A single click near the start collapses the selection there.
    let origin = Vec2::ZERO;
    state.apply_pointer_gesture(&mut fs, PointerGesture::Click, Vec2::new(0.0, 8.0), origin);
    assert!(state.editor_selection_is_none());
    assert_eq!(state.caret().index, 0);

    // A double click selects the word under the pointer (cosmic DoubleClick).
    state.apply_pointer_gesture(&mut fs, PointerGesture::DoubleClick, Vec2::new(2.0, 8.0), origin);
    let (lo, hi) = state.editor_selection_bounds().expect("double-click selects a word");
    assert_eq!((lo.index, hi.index), (0, 5), "the word 'hello'");
    drop(fs);
}
```

- [ ] Run — expect compile failure for the new symbols:

```sh
cargo test -p buiy_core --test text_mouse_selection
```

### Step 4.2 — Minimal impl: the mapping helper, the tracker, the gesture application

- [ ] Create `crates/buiy_core/src/text/edit/pointer.rs`:

```rust
//! E3 — mouse selection (editing-and-ime § 4, mouse gestures). The pure mapping
//! (`pointer_to_cursor`), the click-count state machine (`ClickTracker` /
//! `PointerGesture`), and the gesture→`Action` application on `TextEditState`.
//! The windowed wiring (reading `PointerLocation`/`Hovered`, setting
//! `FocusedEntity`) is `pointer_selection`, registered in `BuiySet::Input`.
//! This file NAMES `Action`/`Edit` (the lowering) ⇒ inside the facade.

use std::time::Duration;

use bevy::math::Vec2;
use cosmic_text::{Action, Buffer, Cursor, Edit, FontSystem};

use super::state::TextEditState;

/// The classified pointer gesture (one mouse-down).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerGesture {
    Click,
    DoubleClick,
    TripleClick,
    /// A move while the button is held (extends the selection from the anchor).
    Drag,
}

/// The multi-click window (wall-clock) and the adjacency radius (logical px).
const MULTI_CLICK_WINDOW: Duration = Duration::from_millis(450);
const MULTI_CLICK_RADIUS: f32 = 4.0;

/// Tracks consecutive clicks to classify single/double/triple (no platform API
/// gives this — cosmic only consumes the already-classified `Action`).
#[derive(Debug, Clone, Copy, Default)]
pub struct ClickTracker {
    last_pos: Vec2,
    last_time: Option<Duration>,
    streak: u8, // 0 ⇒ none; 1 single; 2 double; 3 triple
}

impl ClickTracker {
    /// Classify a press at `pos` / `now`. Increments the streak when the press
    /// is within the time window AND adjacency radius of the previous press;
    /// caps at triple, then rolls back to single.
    pub fn classify(&mut self, pos: Vec2, now: Duration) -> PointerGesture {
        let within = self
            .last_time
            .map(|t| now.saturating_sub(t) <= MULTI_CLICK_WINDOW)
            .unwrap_or(false)
            && pos.distance(self.last_pos) <= MULTI_CLICK_RADIUS;
        self.streak = if within && self.streak < 3 { self.streak + 1 } else { 1 };
        self.last_pos = pos;
        self.last_time = Some(now);
        match self.streak {
            2 => PointerGesture::DoubleClick,
            3 => PointerGesture::TripleClick,
            _ => PointerGesture::Click,
        }
    }
}

/// Map a window-space `pointer` (logical px) to a buffer `Cursor` via the
/// buffer-local hit (`pointer − origin`, then `Buffer::hit`). `origin` is the
/// content-box top-left in window space (the caller folds
/// `GlobalTransform.translation().xy() + ComputedTextLayout.content_offset`,
/// the `extract.rs:398` term). `None` if the buffer has no run at that y.
pub fn pointer_to_cursor(buffer: &Buffer, pointer: Vec2, origin: Vec2) -> Option<Cursor> {
    let local = pointer - origin;
    buffer.hit(local.x, local.y)
}

impl TextEditState {
    /// Apply a classified pointer gesture at `pointer` (window space) to the
    /// editor via the matching cosmic `Action` (§ 4). `Click`/`Drag` use
    /// `Action::Click`/`Drag`; `Double`/`Triple` use word/line granularity.
    /// cosmic's `Action::{Click,...}` take `i32` window-LOCAL pixel coords, so
    /// we fold `origin` and round.
    pub fn apply_pointer_gesture(
        &mut self,
        font_system: &mut FontSystem,
        gesture: PointerGesture,
        pointer: Vec2,
        origin: Vec2,
    ) {
        let local = pointer - origin;
        let (x, y) = (local.x.round() as i32, local.y.round() as i32);
        let action = match gesture {
            PointerGesture::Click => Action::Click { x, y },
            PointerGesture::DoubleClick => Action::DoubleClick { x, y },
            PointerGesture::TripleClick => Action::TripleClick { x, y },
            PointerGesture::Drag => Action::Drag { x, y },
        };
        self.editor.action(font_system, action);
    }

    /// Test/inspection: the editor has no selection (a bare caret).
    pub fn editor_selection_is_none(&self) -> bool {
        self.editor.selection() == cosmic_text::Selection::None
    }

    /// Test/inspection: the editor's selection bounds (ordered), if any.
    pub fn editor_selection_bounds(&self) -> Option<(Cursor, Cursor)> {
        self.editor.selection_bounds()
    }
}
```

- [ ] Wire the module + re-exports. `crates/buiy_core/src/text/edit/mod.rs`: add
  `mod pointer;` and

```rust
pub use pointer::{ClickTracker, PointerGesture, pointer_selection, pointer_to_cursor};
```

  (`pointer_selection` is added in Step 4.3.) Re-export through
  `crates/buiy_core/src/text/mod.rs` `pub use edit::{ ... }` as well.

### Step 4.3 — The windowed `pointer_selection` system + focus-on-click

- [ ] Add the system to `crates/buiy_core/src/text/edit/pointer.rs`. It reads the
  Bevy picking pointer + mouse button, hit-tests via Buiy's `Hovered`, folds the
  origin, and drives the focused editor. Verify the `Hovered` resource path and
  `PointerLocation` import against the picking agent's findings
  (`crates/buiy_core/src/picking/mod.rs` — `Hovered(pub Option<Entity>)`):

```rust
use bevy::input::ButtonInput;
use bevy::picking::pointer::PointerLocation;
use bevy::prelude::*;

use crate::picking::Hovered;
use crate::text::{ComputedTextLayout, SharedFontSystem};
use crate::FocusedEntity;
use super::state::Disabled;

/// The focus-gated mouse-selection system (editing-and-ime § 4), `BuiySet::Input`
/// (alongside `apply_keyboard_edits`). On a left press over an editable, non-
/// `Disabled` entity it sets `FocusedEntity` (focus-on-click — CORE mechanism,
/// since the caret is core; widget focus POLICY is E6) and applies a classified
/// Click/Double/Triple. While the button stays held and the pointer moves it
/// applies `Drag`. All `Option<...>` params so a headless harness without
/// picking/input infra runs it inertly (the apply_keyboard_edits precedent).
///
/// **One-frame `Hovered` lag (accepted).** `Hovered` is written by
/// `update_hovered` in `BuiySet::Picking`, which runs AFTER `BuiySet::Input`
/// (lib.rs ordering) — so a press this frame hit-tests against LAST frame's
/// `Hovered`. Functionally correct and consistent with the OQ#1 one-frame
/// latency posture (a pointer that moved between frames is a sub-frame
/// nicety); not a bug. Note: `origin` folds `GlobalTransform + content_offset`
/// only — it does NOT fold `ScrollOffset`, correct for E3 (auto-scroll-into-view
/// is E6; until then the buffer is laid out at full size at the node origin).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn pointer_selection(
    time: Res<Time>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    hovered: Option<Res<Hovered>>,
    pointers: Query<&PointerLocation>,
    fonts: Res<SharedFontSystem>,
    mut focused: Option<ResMut<FocusedEntity>>,
    mut tracker: Local<ClickTracker>,
    mut editors: Query<(&mut TextEditState, &GlobalTransform, &ComputedTextLayout), Without<Disabled>>,
) {
    let (Some(mouse), Some(hovered), Some(focused)) = (mouse, hovered, focused.as_mut()) else {
        return;
    };
    // The active pointer's window-space position (logical px).
    let Some(pointer_pos) = pointers
        .iter()
        .find_map(|p| p.location.as_ref().map(|l| l.position))
    else {
        return;
    };

    let pressed = mouse.just_pressed(MouseButton::Left);
    let held = mouse.pressed(MouseButton::Left);

    if pressed {
        // Focus-on-click: only when the press lands on an editable entity.
        let Some(hit) = hovered.0 else { return };
        if editors.get(hit).is_err() {
            return; // clicked a non-editor — leave focus to other handlers
        }
        focused.0 = Some(hit);
        let gesture = tracker.classify(pointer_pos, time.elapsed());
        if let Ok((mut state, gt, layout)) = editors.get_mut(hit) {
            let origin = gt.translation().truncate() + layout.content_offset;
            let mut fs = fonts.lock();
            state.apply_pointer_gesture(&mut fs, gesture, pointer_pos, origin);
        }
    } else if held {
        // Drag-extend on the focused editor (the press already focused it).
        let Some(entity) = focused.0 else { return };
        if let Ok((mut state, gt, layout)) = editors.get_mut(entity) {
            let origin = gt.translation().truncate() + layout.content_offset;
            let mut fs = fonts.lock();
            state.apply_pointer_gesture(&mut fs, PointerGesture::Drag, pointer_pos, origin);
        }
    }
}
```

> Implementer verification: confirm `bevy::picking::pointer::PointerLocation` has
> a `.location: Option<Location>` with `Location.position: Vec2` in 0.18.1 (the
> picking agent reported this path). If the field is a method
> (`.location()`), adjust. If `ButtonInput<MouseButton>` is absent in a headless
> harness, the `Option` guard no-ops — matching `apply_keyboard_edits`. Note the
> drag path re-applies a Drag every held frame even without movement; cosmic's
> `Action::Drag` is idempotent for an unchanged position, so this is correct (a
> movement-gate is a YAGNI optimization for v1).

- [ ] Register it in `BuiyTextPlugin::build` next to `apply_keyboard_edits`
  (`crates/buiy_core/src/text/mod.rs`):

```rust
        app.add_systems(
            Update,
            crate::text::edit::pointer_selection.in_set(crate::BuiySet::Input),
        );
```

### Step 4.4 — Run it green + commit

- [ ] Run:

```sh
cargo test -p buiy_core --test text_mouse_selection
```

Expected: `test result: ok. 3 passed`.

- [ ] Full edit-test sweep + boundary:

```sh
cargo test -p buiy_core --test text_caret_geometry --test text_mouse_selection --test text_facade_boundary --test text_editing_ops
```

Expected: all `ok`.

- [ ] Commit:

```sh
git add -A && git commit -m "feat(text-editing): E3 Task 4 — mouse selection (focus-on-click, click/double/triple/drag)"
```

---

## Task 5 — Rework `write_caret_blink` to be per-entity phase-relative

The T7 writer is global+stateless (raw `time.elapsed()`). Rework it to read each
entity's `CaretBlink.origin` (reset by Task 3's writer + Task 4's mouse path), so the caret is solid for one
half-period after every edit/move. Reduced-motion steady is preserved.

### Step 5.1 — Failing test: the blink is phase-relative to the per-entity origin

- [ ] Append to `crates/buiy_core/tests/text_caret_geometry.rs`:

```rust
use buiy_core::text::CaretBlinkInterval;
use buiy_core::theme::UserPreferences;

#[test]
fn blink_is_phase_relative_to_the_per_entity_caret_origin() {
    let (mut app, window) = caret_app();
    // Pause the virtual clock so we control elapsed precisely.
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    let editor = spawn_editor(&mut app, "");
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

    // Advance well past one half-period, THEN type: the edit resets the origin,
    // so the caret must be VISIBLE immediately after (phase 0 from the reset),
    // not hidden by the absolute clock.
    app.world_mut().resource_mut::<Time<Virtual>>().advance_by(Duration::from_millis(700));
    app.update();
    type_char(&mut app, window, "x"); // resets blink origin to now (~700ms)
    app.update();
    assert!(
        app.world().get::<CaretVisual>(editor).unwrap().visible,
        "caret is solid-on immediately after an edit (phase reset)"
    );

    // Advance one half-period (500ms) past the reset ⇒ hidden phase.
    app.world_mut().resource_mut::<Time<Virtual>>().advance_by(Duration::from_millis(500));
    app.update();
    assert!(
        !app.world().get::<CaretVisual>(editor).unwrap().visible,
        "caret hides one half-period after the reset"
    );
}

#[test]
fn reduced_motion_keeps_the_caret_steady() {
    let (mut app, window) = caret_app();
    // `UserPreferences` is `#[non_exhaustive]` (theme.rs) — a struct literal is
    // forbidden from this external test crate. Build the default and mutate the
    // field (the T7 `text_caret_selection.rs:222-223` precedent). `caret_app`
    // adds no ThemePlugin, so insert the resource ourselves.
    let mut prefs = UserPreferences::default();
    prefs.prefers_reduced_motion = true;
    app.insert_resource(prefs);
    app.world_mut().resource_mut::<Time<Virtual>>().pause();
    let editor = spawn_editor(&mut app, "");
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);
    type_char(&mut app, window, "x");
    // Advance many half-periods: a steady caret never hides.
    for _ in 0..5 {
        app.world_mut().resource_mut::<Time<Virtual>>().advance_by(Duration::from_millis(500));
        app.update();
        assert!(
            app.world().get::<CaretVisual>(editor).unwrap().visible,
            "reduced motion ⇒ steady caret"
        );
    }
}
```

> Implementer note: `UserPreferences` IS `#[non_exhaustive]` and DOES derive
> `Default` (theme.rs) — so the default-then-mutate pattern above is mandatory; a
> `UserPreferences { .. }` struct literal will not compile from the test crate.
> Do NOT "fix" a literal by adding fields.

- [ ] Run — expect the FIRST test to fail (the current global writer hides the
  caret at absolute t=700ms regardless of the edit):

```sh
cargo test -p buiy_core --test text_caret_geometry blink_is_phase_relative
```

Expected: assertion failure "caret is solid-on immediately after an edit".

### Step 5.2 — Minimal impl: per-entity phase in `write_caret_blink`

- [ ] Rework `write_caret_blink` in `crates/buiy_core/src/text/visual.rs`. It now
  needs each entity's `CaretBlink` origin, so it queries `(&mut CaretVisual,
  &TextEditState)` — but `TextEditState` lives in `text::edit` and `visual.rs` is
  a sibling text module; reading the `blink` field requires a facade accessor.
  Add a `pub fn blink_origin(&self) -> Duration` to `TextEditState` (state.rs)
  and use it here. Replace the writer:

```rust
use super::edit::TextEditState;

/// Render-prep: drive every `CaretVisual.visible` from the caret's PER-ENTITY
/// blink phase (editing-and-ime §§ 5, 10) — `now − CaretBlink.origin`, where the
/// origin is reset on every edit / caret move by `write_caret_and_selection`. So
/// the caret is solid for one half-period immediately after the user acts (web
/// parity), instead of the T7 global square wave. Edge-only: `Mut` ticks only on
/// a flip (the O(0) steady state). Carets WITHOUT a `TextEditState` (a bare T7
/// display caret, if any) fall back to the global phase. Reduced-motion ⇒ steady.
pub fn write_caret_blink(
    time: Res<Time>,
    prefs: Option<Res<UserPreferences>>,
    interval: Res<CaretBlinkInterval>,
    mut carets: Query<(&mut CaretVisual, Option<&TextEditState>)>,
) {
    let steady = prefs.is_some_and(|p| p.prefers_reduced_motion);
    let now = time.elapsed();
    for (mut caret, state) in &mut carets {
        let elapsed = match state {
            Some(s) => now.saturating_sub(s.blink_origin()),
            None => now,
        };
        let phase = steady || blink_phase(elapsed, interval.0);
        if caret.visible != phase {
            caret.visible = phase;
        }
    }
}
```

- [ ] Add the accessor to `crates/buiy_core/src/text/edit/state.rs` `impl
  TextEditState`:

```rust
    /// The per-entity caret-blink phase origin (the `write_caret_blink` reader).
    pub fn blink_origin(&self) -> std::time::Duration {
        self.blink.origin
    }
```

> Borrow note: `write_caret_and_selection` (E3 writer) holds `&mut TextEditState`
> and runs `.before(write_caret_blink)`; `write_caret_blink` holds `&TextEditState`
> (read-only) — disjoint by ordering, no conflict. Both are in the render-prep
> window. The `Option<&TextEditState>` keeps the writer working for any caret
> without an editor (defense-in-depth; v1 every caret has one).

### Step 5.3 — Run it green; verify the T7 GPU golden's contract still holds

- [ ] Run:

```sh
cargo test -p buiy_core --test text_caret_geometry
```

Expected: `test result: ok. 7 passed` (Task 2's 2 + Task 3's 3 + Task 5's 2
blink tests).

- [ ] **The existing T7 headless blink tests must stay green** — this is the
  load-bearing compatibility check of the rework. `crates/buiy_core/tests/text_caret_selection.rs`
  has three tests (`blink_writes_only_on_phase_edges`,
  `reduced_motion_pins_steady_visible`,
  `reduced_motion_flip_during_hidden_phase_is_one_edge_to_visible`) that spawn a
  **bare `CaretVisual::default()` with NO `TextEditState`** and assert flips on the
  ABSOLUTE virtual clock (e.g. "cross the 500 ms edge ⇒ visible flips false" at
  absolute t = 500 ms). The reworked writer's `None` arm (`elapsed = now` when no
  editor) reproduces that absolute-clock behavior verbatim, so they pass unchanged.
  Run them explicitly:

```sh
cargo test -p buiy_core --test text_caret_selection
```

Expected: `ok` (all T7 paint-seat tests, including the three blink tests, still
pass — the `None` arm is exactly the old global writer). If any of the three fail,
the `None`-arm fallback is wrong; do NOT "fix" the T7 test — fix the fallback to
match the old `blink_phase(time.elapsed(), interval)` exactly for editor-less
carets.

- [ ] The T7 `caret_blink_fixed_clock_pair` GPU golden likewise authored a *bare*
  `CaretVisual` with no `TextEditState` (text_selection_caret_gpu.rs:243–257) — the
  `None` arm preserves its global-phase behavior, so that golden is unaffected.
  **Build it (do not run — GPU lane):**

```sh
cargo test -p buiy_core --test text_selection_caret_gpu --no-run
```

Expected: compiles clean.

- [ ] Commit:

```sh
git add -A && git commit -m "feat(text-editing): E3 Task 5 — per-entity phase-relative caret blink"
```

---

## Task 6 — The additive GPU golden (`#[ignore]`, build-only for the agent)

A caret-over-text + selection golden on a mixed-direction fixture, building on the
T7 readback harness. The orchestrator runs the GPU lane; the agent only compiles.

### Step 6.1 — Author the golden

- [ ] Create `crates/buiy_core/tests/text_caret_selection_e3_gpu.rs`. It captures
  a focused editor with a caret + a mixed-BiDi selection, driven through the E3
  geometry writer (NOT a hand-authored `SelectionVisual`/`CaretVisual` — E3's
  point is that the *editor* drives them), and asserts disjoint selection rects +
  a caret column, reusing the T7 classifiers:

```rust
//! E3 GPU golden (#[ignore], additive — CLAUDE.md GPU lane): a FOCUSED editor's
//! caret + mixed-BiDi selection, driven END-TO-END through E3's geometry writer
//! (editor state → CaretVisual/SelectionVisual → the T7 paint seats → pixels).
//! Distinct from the T7 golden, which hand-authored the paint seats: this proves
//! the E3 writer produces them. Builds on the text_selection_caret_gpu harness.
//!
//! Run: cargo test -p buiy_core --test text_caret_selection_e3_gpu -- --ignored --test-threads=1

mod support;

use std::ops::Range;

use bevy::prelude::*;
use buiy_core::layout::Style;
use buiy_core::render::color::{ColorToken, SELECTION_BG_TOKEN, SELECTION_FG_TOKEN};
use buiy_core::render::components::TextColor;
use buiy_core::render::golden::{GoldenConfig, perceptual_diff};
use buiy_core::text::edit::TextEditState;
use buiy_core::text::{FamilyEntry, FontFamily, FontSize, FontStack, Text};
use buiy_core::{FocusedEntity, Node};
use cosmic_text::Metrics;
use std::borrow::Cow;

const W: u32 = 256;
const H: u32 = 64;
const TEXT_TOKEN: &str = "test.text";

fn sel_red() -> Color { Color::srgb(1.0, 0.0, 0.0) }
fn sel_blue() -> Color { Color::srgb(0.0, 0.0, 1.0) }

fn is_strong_red(p: [u8; 4]) -> bool { p[0] >= 200 && p[1] <= 20 && p[2] <= 20 }
fn is_blue_ink(p: [u8; 4]) -> bool { p[2] >= 180 && p[0] <= 150 }

fn cols_where(pixels: &[u8], w: u32, h: u32, pred: impl Fn([u8; 4]) -> bool) -> Vec<u32> {
    (0..w).filter(|&x| (0..h).any(|y| pred(support::px(pixels, w, x, y)))).collect()
}
fn bands(sorted: &[u32]) -> Vec<Range<u32>> {
    let mut out: Vec<Range<u32>> = Vec::new();
    for &i in sorted {
        match out.last_mut() {
            Some(b) if b.end == i => b.end = i + 1,
            _ => out.push(i..i + 1),
        }
    }
    out
}

/// Spawn a FOCUSED editor over the T7 mixed-BiDi corpus line, select [10,18)
/// through the EDITOR (set the editor's selection via SelectAll-then-narrow is
/// fiddly; instead we drive the editor cursor with motions to anchor [10] and
/// extend to [18]). The E3 writer then mirrors it into SelectionVisual.
fn capture() -> Vec<u8> {
    let _cfg = GoldenConfig::deterministic();
    let mut app = support::gpu_render_app(W, H);
    support::finish_and_run(&mut app, 0);
    support::register_fixture_font(&mut app, "Noto Sans Hebrew", "NotoSansHebrew-hebrew.ttf");
    {
        let mut theme = app.world_mut().resource_mut::<buiy_core::theme::Theme>();
        theme.colors.insert(TEXT_TOKEN.into(), Color::WHITE);
        theme.colors.insert(SELECTION_BG_TOKEN.into(), sel_red());
        theme.colors.insert(SELECTION_FG_TOKEN.into(), sel_blue());
    }
    let editor = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("hello עולם world")),
            FontFamily(FontStack(vec![
                FamilyEntry::Named(String::from("Fira Sans")),
                FamilyEntry::Named(String::from("Noto Sans Hebrew")),
            ])),
            FontSize(20.0),
            TextColor(ColorToken::Token(Cow::Borrowed(TEXT_TOKEN))),
            TextEditState::new(Metrics::new(20.0, 24.0)),
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default().flex_column().width_px(W as f32).height_px(H as f32)))
        .add_child(editor);
    app.update();
    app.world_mut().resource_mut::<FocusedEntity>().0 = Some(editor);

    // Drive the editor: place the anchor at byte 10 and extend to byte 18, so the
    // E3 writer mirrors a mixed-BiDi selection into SelectionVisual. Driving the
    // editor directly (test-inspection apply) keeps the golden end-to-end.
    {
        let fonts = app.world().resource::<buiy_core::text::SharedFontSystem>().clone();
        let mut state = app.world_mut().get_mut::<TextEditState>(editor).unwrap();
        let mut fs = fonts.lock();
        // Home, then Right ×10 to reach byte 10 (the corpus bytes map per T7).
        use buiy_core::text::edit::EditCommand;
        use cosmic_text::Motion;
        state.apply(&mut fs, EditCommand::Motion(Motion::Home, false), false, false);
        for _ in 0..10 {
            state.apply(&mut fs, EditCommand::Motion(Motion::Right, false), false, false);
        }
        for _ in 0..8 {
            state.apply(&mut fs, EditCommand::Motion(Motion::Right, true), false, false);
        }
    }
    app.update(); // E3 writer mirrors the selection into SelectionVisual + caret

    let target = support::render_to_image(&mut app, W, H);
    support::spawn_capture_camera(&mut app, target.clone());
    support::wait_for_text_ready(&mut app, 60);
    support::readback_rgba(&mut app, target)
}

#[test]
#[ignore = "needs a wgpu adapter; E3 caret+selection end-to-end golden (editing-and-ime §§ 4, 5; verification § 12)"]
fn e3_editor_driven_selection_paints_disjoint_rects_and_caret() {
    let frame = capture();
    // The mixed-BiDi selection paints ≥2 disjoint red rects (the T7 contract,
    // here PRODUCED by the E3 writer from editor state).
    let sel_bands = bands(&cols_where(&frame, W, H, is_strong_red));
    let wide: Vec<Range<u32>> = sel_bands.iter().filter(|b| b.end - b.start >= 3).cloned().collect();
    assert!(wide.len() >= 2, "E3-driven mixed-BiDi selection paints disjoint rects: {wide:?}");
    // Selected ink re-tints to blue inside the rects.
    let blue = cols_where(&frame, W, H, is_blue_ink);
    assert!(
        blue.iter().any(|x| wide.iter().any(|b| b.contains(x))),
        "selected glyphs re-tint inside the rects (blue {blue:?} vs {wide:?})"
    );
    // Re-capture determinism: a fresh capture matches.
    let frame_b = capture();
    let diff = perceptual_diff(&frame, &frame_b);
    assert!(diff < 1e-4, "two fresh captures diverged: {diff}");
}
```

> Implementer caveats: (1) the byte/motion counts ("Right ×10", "×8") must land
> on the same `[10, 18)` logical range the T7 golden hand-authored — verify the
> corpus byte map (T7 comment: `0..6 "hello "`, `6..14` Hebrew, `14..20` " world").
> Motions step in VISUAL order across the BiDi run, so the exact count to reach a
> given LOGICAL byte may differ; if the count is off, set the editor selection
> directly instead via a test-only helper that calls `set_selection`/`set_cursor`
> through a `#[cfg(test)]`-gated facade method, OR author the `SelectionVisual`
> through the editor's `Action::Drag` at the two pixel x's. The robust path:
> reuse the T7 golden's proven `SelectionVisual::new(Cursor::new(0,10),
> Cursor::new(0,18))` BUT spawn it alongside the editor and assert the E3 writer
> does not CLOBBER a correct selection — if that proves circular, keep this golden
> focused on the caret-over-text assertion (which IS purely E3-driven) and let the
> T7 golden remain the selection-rect proof. **Decide during implementation; the
> non-negotiable is: one `#[ignore]` golden that exercises the E3 writer producing
> at least the caret stamp end-to-end.** Keep the selection assertion if the
> motion counts verify; otherwise narrow to the caret.

### Step 6.2 — Build it (agent does NOT run the GPU lane)

- [ ] Build-only:

```sh
cargo test -p buiy_core --test text_caret_selection_e3_gpu --no-run
```

Expected: compiles clean (the orchestrator runs `-- --ignored` on a GPU host).

- [ ] Commit:

```sh
git add -A && git commit -m "test(text-editing): E3 Task 6 — additive GPU golden for editor-driven caret+selection"
```

---

## Task 7 — Phase gate + docs

### Step 7.1 — Full headless gate

- [ ] Run the full gate:

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  xvfb-run -a cargo test --workspace
```

Expected: green (zero warnings; all headless tests pass; the four `#[ignore]`
GPU tests across `text_selection_caret_gpu` + `text_caret_selection_e3_gpu` are
skipped without `--ignored`).

- [ ] Build the full GPU lane (agent build-only):

```sh
cargo test -p buiy_core --tests --no-run
```

Expected: clean.

### Step 7.2 — Update the campaign plan errata + docs index

- [ ] Add an **E3 errata block** to
  `docs/plans/2026-06-13-buiy-text-editing-campaign.md` (a new `### E3 errata`
  subsection under Phases, mirroring the T-series discipline), recording:
  1. **The `CaretBlink` "built but inert" claim is wrong.** The E3 deliverable
     text says "E1's `CaretBlink` field (built but inert) gains its … reset" —
     E1 built **no** `blink` field (state.rs explicitly deferred
     `selection`/`blink`/`undo`/`preedit`, doc lines 9–13). E3 introduces BOTH
     the `CaretBlink` type and the `TextEditState.blink` field. The reword: "E3
     introduces the `CaretBlink` per-entity phase state and reworks the global
     T7 `write_caret_blink` to be phase-relative."
  2. **`LayoutRun::highlight` returns `impl Iterator<Item=(f32,f32)>` (x, width),
     not `Option<(f32,f32)>`.** Spec § 4.3 says both; the verified 0.19 signature
     is the iterator (the codebase's `extract.rs` already consumes it correctly).
     Geometry is unaffected (E3 produces *endpoints*, not rects; the producer
     sweeps).
  3. **No focus-on-click existed before E3.** The campaign positioned mouse
     selection as "through picking coordinates" — but Buiy's picking yields only
     `Hovered(Option<Entity>)` (no position) and no focus-on-click. E3 builds the
     window→buffer-local mapping and sets `FocusedEntity` on press itself. (Core
     focus-on-click here is the caret's, distinct from E6's widget focus policy.)
  4. **The BiDi split caret is deferred out of E3.** The campaign + spec § 5
     describe "split caret = two stamps", but cosmic 0.19's `cursor_position`
     reads only `line`/`index` (never `affinity`) and a mixed-direction line is a
     single `LayoutRun` — there is no second-run position to read, and cosmic
     exposes no dual-caret API. A correct split caret needs the harder
     `run.glyphs` boundary-edge walk (the `cursor_from_glyph_left/right` shape).
     E3 ships the single primary caret; the split is filed below as a
     deferred-within-F slice (zero correctness risk).

- [ ] **File the BiDi split-caret deferral** (the descope decision, made
  explicit). Append to `docs/plans/follow-ups.md` an entry: *"BiDi split caret —
  deferred from E3 (glyph-edge geometry: cosmic 0.19 exposes no dual-caret API;
  needs a `run.glyphs` boundary-edge walk à la `cursor_from_glyph_left/right`,
  buffer.rs:181-197). Single primary caret is fully functional; split is a
  mixed-direction visual nicety."* Add a one-line as-landed note to the campaign
  plan's E3 deliverable (the "split caret = … second stamp" clause → mark
  "(deferred — see E3 errata 4)") and to spec §§ 5 and 13 (the "split caret = two
  stamps" line in § 5 and the § 13 deferred-within-F list, alongside the existing
  multi-range-behavior deferral). Follow `organizing-buiy-docs` for the
  follow-ups + spec edits; supersede, don't silently contradict.

- [ ] If `docs/README.md` lists per-phase plans, add a row for this file under the
  editing campaign (mirror the existing E1/E2 plan rows; follow
  `organizing-buiy-docs`).

- [ ] Commit:

```sh
git add -A && git commit -m "docs(text-editing): E3 errata + split-caret deferral + plan index"
```

---

## Self-review against spec §§ 4 / 5 / 11

**§ 4.1 caret (logical position, visual motion).** ✅ Caret geometry is
`cursor_position` + `line_top`/`line_height` (`caret_rect_for`, Task 3). Visual
motion is **inherited** — E3 never computes BiDi; the keymap (E2) drives the
editor, and `mirror_selection`/`caret()` read the editor's already-visual cursor.
⚠️ **The BiDi split caret is DEFERRED** (deferred-within-F): cosmic 0.19's
`cursor_position` reads only `line`/`index` (never `affinity`) and a mixed line is
one `LayoutRun`, so there is no second-run position to find and no dual-caret API;
a correct split caret needs the harder `run.glyphs` boundary-edge walk. The single
primary caret is fully functional; the deferral is filed in Task 7 (closure). ✅
(single caret) / deferred (split).

**§ 4.2 multi-range-shaped `TextSelection`.** ✅ `SelectionRange { anchor, active }`
+ `TextSelection { primary, secondary: SmallVec<[…;2]> }`, v1 `secondary` always
empty (Task 1). It lives on `TextEditState.selection` (E1-deferred field, added
Task 2). The "mirror into `set_selection`" is satisfied by the editor already
owning the selection (E2 set it; E3 reads it out) — the architecture note pins
the mirror DIRECTION (editor = source of truth; `TextSelection` = projection).
Type consistency: `SelectionVisual::new(lo, hi)` (T7 seat) and
`TextSelection::from_bounds` both consume the `selection_bounds()` ordered pair —
one ordering convention throughout. ✅

**§ 4.3 selection geometry (`selection_bounds` + `highlight`).** ✅ E3 produces
the ordered endpoints into `SelectionVisual`; the existing `extract_buiy_glyphs`
sweeps `LayoutRun::highlight` per run (mixed-direction multi-rects automatic) and
re-tints per cluster. E3 adds **zero** BiDi math (confirmed: it only writes the
endpoints; the producer does the sweep). ✅

**§ 5 painting (quad reuse, caret stamp, per-entity blink).** ✅
Selection rects ride the existing quad path (`ExtractedTextQuads`); selected-text
recolor rides `GlyphAlphaInstance.color`; the caret is the existing glyph-tier
solid stamp — no new pipeline, no new primitive, no new GPU work (spec § 5's hard
constraint). The split caret stamp is the deferred slice (above). Per-entity
blink: `CaretBlink.origin` + the reworked `write_caret_blink` (Task 5),
reduced-motion steady preserved, edge-only writes preserved (the O(0) steady state
and the T8 damage golden's contract are intact — the `None` arm keeps the T7
bare-caret golden's global phase). ✅ (with the split-stamp line deferred)

**§ 11 Messages (`SelectionChanged` / `CaretMoved`, transition-only).** ✅
`SelectionChanged(Entity, TextSelection)` on a selection transition;
`CaretMoved(Entity, Cursor)` on a caret transition WITHOUT a selection change
(mutually exclusive in the writer); both emit on transition only — the idle-frame
test (Task 3) pins zero emissions on a no-input frame. Payloads match the § 11
table (selection carries the multi-range-shaped type; caret carries the `Cursor`).
✅

**Lock-in containment.** ✅ Every new cosmic-naming file
(`selection.rs` names only `Cursor`; `caret.rs`, `pointer.rs` name `Edit`/`Action`)
lives in `text/edit/`. The facade-boundary test (`text_facade_boundary.rs`) is
re-run at Tasks 2/3/4. `SelectionRange`/`TextSelection`/`CaretMoved`/
`SelectionChanged`/`PointerGesture` expose `Cursor` only (pure data).

**No placeholders / completeness.** Every code step is complete and
copy-pasteable; no dead/stub code ships at any commit boundary (the earlier
split-caret stub was removed when the split caret was descoped — `caret.rs` is
exactly `CaretMoved`, `SelectionChanged`, `CARET_W`, `caret_rect_for`,
`write_caret_and_selection`, which all compile clean under `clippy -D warnings`).
Each task is failing-test → run-fail → minimal-impl → run-pass → commit. The GPU
golden is `#[ignore]` + build-only for the agent; the orchestrator runs the lane.
The descoped split caret is a documented deferral (Task 7), not a silent drop.

**Platform portability.** ✅ E3 is geometry (platform-independent). The one
modifier-driven test (select-all, Task 3 Step 3.1) uses the
`cfg!(target_os = "macos") ? SuperLeft : ControlLeft` pattern (the E2 erratum-2
fix), so it passes on the Linux local gate AND macOS/Windows CI.
