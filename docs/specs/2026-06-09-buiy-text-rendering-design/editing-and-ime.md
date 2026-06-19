# Buiy text — editing and IME (the F-tier editor surface)

**Parent:** [README.md](README.md)

This file owns every **F** row of [foundation text.md § 3.5](../2026-05-07-buiy-foundation/text.md#35-text-editing):
the editor surface (single-line, multi-line, read-only, disabled, placeholder),
the caret & selection model (logical+visual BiDi caret, UAX #9 traversal,
single+multi-range selection, mixed-direction selection rects, caret styling /
blink / `caret-color`, auto-scroll-into-view), the full IME composition family
(events, preedit rendering, preedit cursor, commit-as-one-undo-unit, popup
positioning), and the editing operations (standard keys, grapheme-correct
delete, cut/copy/paste, undo/redo with composition grouping). The rich-text
edit surface (mixed runs, inline images/links) is **E-tier and out of scope**;
word-segmented navigation per locale is **C** (cosmic-text's
unicode-segmentation default suffices for F).

---

## § 1 Scope and seams

What this file consumes from siblings, and what it produces:

| Concern | Owner |
|---|---|
| `FontSystem` ownership (`SharedFontSystem(Arc<Mutex<FontSystem>>)`), `SwashCache`, scheduling discipline | [architecture.md](architecture.md) |
| The per-entity shaped `Buffer`, the Taffy measure protocol, when shaping runs | [measure-and-layout.md](measure-and-layout.md) |
| Turning `layout_runs` into `GlyphAlphaInstance`s + atlas inserts | [glyph-pipeline.md](glyph-pipeline.md) |
| Selection/decoration quad emission, the caret/line-through stamp seats, `::selection` paint | [decoration-and-paint.md](decoration-and-paint.md) |
| Editor **state machine**: `TextEditState`, Actions, caret/selection model, geometry | **this file** |
| IME lifecycle, preedit splice invariants, `ime_position` plumbing | **this file** |
| Clipboard facade, undo stack, `EditCommand` keymap, edit Message taxonomy | **this file** |
| `TextInput` widget bundle (policy: sizes, tokens, submit-on-Enter) | `buiy_widgets` (§ 2.3) |

This file produces *state and geometry* (caret rects, selection rect lists,
preedit span ranges, per-glyph tint overrides); the painting mechanics ride the
existing GPU-verified paths — quads via the batched node, glyph recolor via
`GlyphAlphaInstance.color` ([render § 4.1](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md),
`crates/buiy_core/src/render/atlas/primitive.rs:30-48`). No new GPU work is
required by anything in this file (§ 5).

> **Status: implemented (editing, E1–E6) — as landed 2026-06-13.** The editor
> surface this file designs is built and proven on the two-lane suite (the
> headless geometry gate every PR plus the `#[ignore]` GPU pixels lane): the
> `buiy-text-editing` campaign (E1–E6,
> [campaign plan](../../plans/2026-06-13-buiy-text-editing-campaign.md)) landed
> `TextEditState` over `Editor<'static>`, the `ReadOnly` / `Disabled` /
> `SingleLine` / `Placeholder` markers, the focus-gated `EditCommand` keymap,
> the caret + multi-range-shaped `TextSelection`, the IME display-splice preedit
> (the four § 6.2 invariants), the arboard clipboard facade, the two-stack
> `UndoUnit` model with composition grouping, auto-scroll via `ScrollOffset`,
> the placeholder, the § 11 Message taxonomy, and the
> `buiy_widgets::TextInput` bundle. Per-section **"As landed (E_n)"** notes
> below record the mechanical errata folded at closure. The **named deferrals**
> (multi-range selection *behavior*, HTML/image clipboard,
> compose-over-selection) are filed in
> [follow-ups.md](../../plans/follow-ups.md) (§ 13). The **BiDi split caret**
> secondary indicator landed as a post-E3 follow-up (§§ 4.1, 5).
>
> *(Superseded 2026-06-13 by the as-landed paragraph above — the
> proposal-time record, kept for history.)* **Status: design-only (deferred
> build targets).** As of 2026-06-13, none of the editor state machine described
> here is implemented. `TextEditState`, `EditCommand`, `UndoStack`,
> `TextSelection`, the `ReadOnly` / `Disabled` / `SingleLine` / `Placeholder`
> markers, `PreeditSpan`, and the IME machinery (§§ 2.2, 3, 4, 6, 8) are **this
> campaign's implementation targets**, not verified code — `TextEditState` is
> explicitly deferred at `crates/buiy_core/src/text/components.rs`. The
> **painting surfaces** they drive (`CaretVisual`, `SelectionVisual`) and every
> architectural seam (focus, Tab, `ScrollOffset`, picking, `BuiySet` order,
> paint rank, damage gates) ARE built and verified (T6–T8); see the readiness
> report `docs/reports/2026-06-13-text-editing-design-readiness.md`.

> **Supersession, stated up front.** The prior-art guidance to keep preedit as a
> "parallel … render-layer overlay" that never mutates the `Buffer`
> ([bevy-cosmic-edit lessons.md "IME without preedit rendering"](../../prior-art/bevy-cosmic-edit/lessons.md),
> [cosmic-text editing.md § IME](../../prior-art/cosmic-text/editing.md)) is
> **superseded by § 6's display-splice model**. The *bet* those notes encode —
> the logical value, the event stream, and the undo history never see preedit —
> is kept exactly (§ 6.2 invariants); only the *display* representation changes,
> because an overlay cannot reflow a line mid-composition (§ 6.1 rationale).

---

## § 2 Editor substrate and component model

### § 2.1 Decision: wrap `cosmic_text::Editor`, do not rebuild it

**Decision.** The editing core is `cosmic_text::Editor<'static>` (0.19), driven
through the `Edit` trait; Buiy layers everything cosmic-text deliberately leaves
to embedders: the undo stack (§ 8), IME preedit (§ 6), clipboard (§ 7), the
multi-range selection type (§ 4.2), and the event surface (§ 11). **F**

**Rationale.** `Editor` delivers exactly the hard 20%: BiDi-correct visual caret
stepping (`Motion::Left/Right` walk visual order across `LayoutGlyph` runs),
grapheme-cluster-correct `Backspace`/`Delete`, `Click`/`DoubleClick`/
`TripleClick`/`Drag` hit-testing with `Word`/`Line` selection granularity,
affinity-disambiguated soft-wrap carets, and `Change` emission for undo — all
verified present in 0.19 ([`trait.Edit`](https://docs.rs/cosmic-text/0.19.0/cosmic_text/trait.Edit.html),
[`enum.Action`](https://docs.rs/cosmic-text/0.19.0/cosmic_text/enum.Action.html),
[`enum.Motion`](https://docs.rs/cosmic-text/0.19.0/cosmic_text/enum.Motion.html) — 22 Motion
variants). Reimplementing BiDi caret math is the single largest correctness
risk in this area and buys nothing.

**Runner-up rejected:** Buiy-owned editor state directly over `Buffer`,
Iced-style (cosmic-text used only for shape/layout/hit-test). Iced's
route-around predates `Editor` maturity and its `TextInput` is single-line;
Buiy needs multi-line + BiDi at F on day one. The bevy-cosmic-edit post-mortem
indicts the bridge-crate *structure*, not `Editor` itself
([why-archived.md](../../prior-art/bevy-cosmic-edit/why-archived.md)) — Buiy
owning the seam first-party is the fix. **Also rejected:** `ViEditor` (the
`vi`-feature editor) for its built-in undo stack — it drags in `modit` +
`syntect` and its `cosmic_undo_2` stack still lacks composition grouping
([prior-art editing.md § Undo/redo](../../prior-art/cosmic-text/editing.md)).

**Lock-in containment.** All Buiy systems talk to a thin `TextEditState`
facade; no system outside the `text/edit` module names an `Editor`, `Edit`,
`Action`, or `Change` type. A future substrate swap stays local to one module.

### § 2.2 The components — `buiy_core`

`Editor` and `FontSystem` are both `Send + Sync` in 0.19 (docs.rs auto-trait
impls; this **corrects** the prior-art folder's "non-Sync `FontSystem`" note),
so these are plain `Component`s/`Resource`s — no `NonSend` contortions.

```rust
/// buiy_core::text::edit — the editing state machine. Optional: entities with
/// only a display Buffer never pay for it (editor-optional / buffer-required,
/// the bevy-cosmic-edit Borrow #1 shape).
#[derive(Component)]
pub struct TextEditState {
    editor: cosmic_text::Editor<'static>, // BufferRef::Owned — § 2.2a
    selection: TextSelection,             // multi-range-shaped, § 4.2
    preedit: Option<PreeditSpan>,         // § 6
    undo: UndoStack,                      // § 8
    blink: CaretBlink,                    // § 10
}

#[derive(Component)] pub struct ReadOnly;            // marker: caret+selection+copy yes; mutation no
#[derive(Component)] pub struct Disabled;            // marker: no focus, no caret, no IME
#[derive(Component)] pub struct Placeholder(pub String); // shown when empty (§ 10)
#[derive(Component)] pub struct SingleLine;          // marker: Enter ⇒ Submit, Wrap::None (§ 3.3)
```

**§ 2.2a Buffer ownership for editable entities.** `Editor<'static>` is
constructed with `BufferRef::Owned(Buffer)`
([`enum.BufferRef`](https://docs.rs/cosmic-text/0.19.0/cosmic_text/enum.BufferRef.html)):
when `TextEditState` is present, **its owned Buffer is the authoritative one**,
and the measure seam / glyph producer reach it through a unified accessor
(`with_buffer` / `with_buffer_mut` re-exposed on the facade) that prefers the
editor's buffer over the display-only component — the `EditorBuffer` QueryData
pattern ([bevy-cosmic-edit lessons.md Borrow #1](../../prior-art/bevy-cosmic-edit/lessons.md)).
A component cannot hold `BufferRef::Borrowed` (no self-borrowing components),
and `BufferRef::Arc` forbids mutation; `Owned` is the only shape that works.
The accessor is pinned: `TextBufferAccess`
([measure-and-layout.md § 2.3](measure-and-layout.md)) — `&mut TextBuffer` +
`Option<&mut TextEditState>` with `with_buffer`/`with_buffer_mut` preferring
the editor's owned buffer when present.

> **As landed (E1): the intrinsics cache lives on `TextEditState`.** The
> `IntrinsicWidths` cache that measure reads moved off `TextBuffer` onto
> `TextEditState.intrinsics` (`text/edit/state.rs`) so it keys to the
> AUTHORITATIVE (editor-owned) buffer it describes; `TextBufferAccess` gained
> editor-first `intrinsics()` / `cache_intrinsics()` / `invalidate_intrinsics()`
> methods (`text/edit/access.rs`) that dispatch to whichever side owns the
> authoritative buffer. A display-only entity's cache stays on its `TextBuffer`.
> Zero behavior change — the cache just keys to the right buffer.

Markers decompose rather than aggregate (ReadOnly as marker is
[Borrow #3](../../prior-art/bevy-cosmic-edit/lessons.md); decomposed style/behavior
components are Borrow #2). `ReadOnly` keeps caret/selection/copy and IME-disabled;
`Disabled` additionally refuses focus and suppresses the caret entirely.

### § 2.3 Crate split — mechanism in core, policy in widgets

**Decision.** `buiy_core` owns `TextEditState`, the markers, the IME / undo /
clipboard systems, the `EditCommand` keymap, and the Message taxonomy.
`buiy_widgets::TextInput::new(...)` composes them into an `impl Bundle`,
exactly as `Button::new` does (`crates/buiy_widgets/src/button.rs:29-60`). **F**

**Rationale.** Mirrors the established convention — `focus.rs` and picking live
in core while `Button` is a widgets bundle over core components. IME plumbing,
undo, and clipboard are reusable mechanism any text-editing surface needs (the
future E-tier rich editor included), and core must not depend on the widget
crate. **Runner-up rejected:** everything in `buiy_widgets` as a TextInput
plugin — strands reusable mechanism in policy territory. **Also rejected:**
the widget in core — sizes, tokens, and submit-on-Enter are catalog policy.
Focus-on-click is widget policy too, not core mechanism
([Borrow #7](../../prior-art/bevy-cosmic-edit/lessons.md)): the `TextInput`
bundle opts in; core never auto-focuses.

> **As landed (E6): the `TextInput` bundle + the `cosmic_text`-free seam.** The
> `buiy_widgets::TextInput` bundle (`crates/buiy_widgets/src/text_input.rs`)
> composes the core editor + markers + focus + node/style + catalog tokens with
> a widget-side `focus_on_click` system. Two facts from the as-built code: (1)
> the Phase-0 `A11yRole` taxonomy has no `TextInput` / `TextField` variant, so
> the bundle uses `A11yRole::Text` (a richer role is a later a11y-taxonomy
> slice); (2) the bundle never names a `cosmic_text` type — the core
> `TextEditState::for_font_size(f32)` constructor is the seam that keeps the
> facade boundary (`buiy_widgets` does not depend on `cosmic-text`).

---

## § 3 Input translation — `KeyboardInput` → `EditCommand` → `Action`

**Decision.** Editing input consumes Bevy 0.18 `KeyboardInput` **Messages**
(`{key_code, logical_key: Key, state, text: Option<SmolStr>, repeat, window}`,
bevy_input-0.18.1 `src/keyboard.rs:109-139`), routed through a **data-driven
keymap table** keyed on `(modifiers, logical Key)` that produces a Buiy
`EditCommand`, which then lowers to cosmic `Action`s. Character insertion uses
the event's `text` field — layout-resolved, dead-key-composed — iterated as
chars into `Action::Insert`; `repeat: bool` is honored for motions and
deletions. **F**

**Rationale.** `ButtonInput<KeyCode>` (the runner-up, and the Phase-0 pattern
`focus.rs:60-73` uses for Tab) is physical-layout-blind: it cannot produce text
on non-QWERTY layouts or dead-key sequences and misses key-repeat semantics —
it loses immediately for real text input. The Tab-traversal system keeps its
pattern; **text editing supersedes it locally**.

`EditCommand` borrows the cosmic `Action` shape
([cosmic-text lessons.md Borrow #10](../../prior-art/cosmic-text/lessons.md))
but is Buiy-owned because clipboard/undo/submit verbs do not exist in `Action`:

```rust
pub enum EditCommand {
    Motion(Motion, /* extend_selection: */ bool), // arrows, Home/End, PgUp/PgDn, word-nav, doc start/end
    Insert(SmolStr), Backspace, Delete, Enter,
    Cut, Copy, Paste,                 // § 7
    Undo, Redo,                       // § 8
    SelectAll, Escape, Submit,        // Submit: single-line Enter (§ 3.3)
}
```

### § 3.1 The standard-keys table (F row, normative)

Arrows ⇒ `Motion::{Left,Right,Up,Down}`; word-nav modifier+arrows ⇒
`Motion::{LeftWord,RightWord}`; Home/End ⇒ `Motion::{Home,End}`,
doc-modifier+Home/End ⇒ `Motion::{BufferStart,BufferEnd}`; PgUp/PgDn ⇒
`Motion::{PageUp,PageDown}`; Shift+any motion ⇒ `extend_selection = true`
(anchor held); Ctrl/Cmd-A ⇒ `SelectAll`; Backspace/Delete ⇒ grapheme-correct
deletion (inherited from `Action::Backspace/Delete`); Ctrl/Cmd-{X,C,V,Z} and
redo (Ctrl-Y / Ctrl-Shift-Z / Cmd-Shift-Z) per § 7/§ 8.

### § 3.2 Platform conventions

One keymap **table per platform** (Ctrl on Linux/Windows vs Cmd on macOS;
macOS word-nav = Option+arrows, line-ends = Cmd+arrows), selected at startup —
a data swap, not scattered `cfg`/runtime conditionals. The table is the later
hook for user rebinding; v1 ships the fixed per-platform tables only.

### § 3.3 Single-line policy

Lives in this layer, gated on the `SingleLine` marker: Enter ⇒ `Submit`
Message (never `Action::Enter`); paste strips newlines before insertion; the
buffer is configured `Wrap::None` (the measure seam reads the marker). **F**

---

## § 4 Caret and selection model

### § 4.1 The caret: logical position, visual motion

The logical caret is cosmic-text's
`Cursor { line: usize, index: usize, affinity: Affinity }` — `index` is a byte
offset into the line's UTF-8, `affinity` disambiguates the soft-wrap boundary
(end of visual line N vs start of N+1). Visual semantics are inherited:
`Motion::Left/Right` step in **visual** order across BiDi runs per UAX #9,
`Up/Down` walk **visual** (wrapped) lines preserving x, word motions use
unicode-segmentation — the keymap never computes BiDi state
([prior-art editing.md § Cursor movement](../../prior-art/cosmic-text/editing.md)). **F**

Caret geometry: `LayoutRun::cursor_position(&Cursor) -> Option<f32>` plus the
run's `line_top`/`line_height` give the caret rect in buffer-local coordinates.
When the caret sits on a direction boundary, **both** positions are emitted
(BiDi split caret: primary full-height + secondary indicator), resolved from
the two candidate runs the affinity pair names. **F**

> **As landed: the secondary indicator now lands (follow-up after E3).** E3
> shipped only the primary caret; a post-E3 slice added the secondary
> (`secondary_caret_rect_for`, caret.rs). The honest residual the draft's
> "two candidate runs" framing got wrong: cosmic 0.19's `cursor_glyph`
> (buffer.rs:151-174) is affinity-blind AND order-defined — it resolves
> `index == glyph.start` BEFORE `index == glyph.end`, so its single
> `cursor_position` only ever surfaces the AFTER (start-glyph) edge, which the
> primary already paints. Buiy computes the SECONDARY directly as the BEFORE
> glyph's (`end == index`) LOGICAL-END visual edge — LTR → `x + w`, RTL → `x`
> (cosmic's own convention, buffer.rs:120-142 / `cursor_from_glyph_right`). It
> rides `CaretVisual.secondary: Option<Rect>` and paints as a second solid
> stamp (CPU geometry only — no new GPU). A line is one `LayoutRun`, so there
> are no "two candidate runs"; the second position is glyph-level, not run-level.
> See [follow-ups.md § Text editing — BiDi split caret](../../plans/follow-ups.md).

### § 4.2 Decision: a multi-range-shaped Buiy selection type

**Decision.** Buiy owns the selection type; the editor's single selection is a
mirror of its primary range:

```rust
pub struct SelectionRange { pub anchor: cosmic_text::Cursor, pub active: cosmic_text::Cursor }
pub struct TextSelection { pub primary: SelectionRange, pub secondary: SmallVec<[SelectionRange; 2]> }
```

The primary mirrors into `Edit::set_selection` (so `Action`-driven behavior —
drag-extend granularity, `delete_selection` — keeps working); **v1 ships
single-range behavior** (`secondary` always empty) but the public type, the
`SelectionChanged` payload, and the geometry pipeline are multi-range-shaped. **F**

**Rationale.** text.md pins "Selection ranges (single + multi-range)" at F, and
cosmic-text's `Selection` (`None | Normal | Line | Word` — verified) is
structurally single-range with no extension point. **Runner-up rejected:**
shipping `cosmic_text::Selection` alone and deferring multi-range — retrofitting
the type later breaks the event payloads, the a11y mapping, and `::selection`
APIs; cheap to shape now, expensive to reshape. Multi-range *behavior*
(multi-cursor editing) is the named next slice, not silently dropped (§ 13).

### § 4.3 Selection geometry

Per range: `Edit::selection_bounds()` (for the mirrored primary) or the range's
ordered cursor pair, swept across `Buffer::layout_runs()` with
`LayoutRun::highlight(start, end) -> impl Iterator<Item = (f32, f32)>` —
multiple `(x, width)` spans per mixed-direction line are **automatic** (one per
BiDi run intersected), so `"hello עולם world"` selects correctly with zero
Buiy-side BiDi math. N ranges = N sweeps. The prior-art claim of an
`Editor::with_selection_bounds(|rects|)` callback is **stale — no such API in
0.19**; `selection_bounds` + `LayoutRun::highlight` is the real pair
([`struct.LayoutRun`](https://docs.rs/cosmic-text/0.19.0/cosmic_text/struct.LayoutRun.html)). **F**

---

## § 5 Painting — quad-path reuse, no new primitives

**Decision (revised in review round 1 to consume
[decoration-and-paint.md § 4.2](decoration-and-paint.md)'s as-built paint
seats).** All editor visuals are existing-primitive emissions:

- **Selection rects** (§ 4.3) are **quad-tier** instances under the text —
  the fixed paint rank `quad 1 < glyph 2` (buckets.rs:42–53) makes
  selection-behind-text free — carried by `ExtractedTextQuads`
  ([decoration-and-paint.md § 4.6](decoration-and-paint.md)).
- **Selected-text recolor** and `::selection` foreground tokens are pure
  per-instance `GlyphAlphaInstance.color` overrides — alpha-as-color means
  **zero atlas work** (`render/atlas/primitive.rs:30-48`). Token choice is owned
  here; emission mechanics in [decoration-and-paint.md](decoration-and-paint.md).
- **The caret** is a **glyph-tier solid stamp**: a `GlyphAlphaInstance`
  sampling the 1×1 solid-white `CoverageR8` atlas texel
  ([decoration-and-paint.md §§ 4.2–4.3](decoration-and-paint.md)), emitted
  after the run's glyphs so it paints **over** the text in the same tier. A
  quad caret cannot: v1 routes everything to layer 0 (per-layer interleave
  does not exist — buckets.rs:9–11, 146–153), so a quad always paints under
  glyphs, and a "next layer" would misuse the `painters_z` stacking index for
  a within-node ordering concern. Split caret (§ 4.1) = a **secondary
  `CaretVisual` rect + a second stamp** (CPU geometry only — still no GPU work);
  *as landed (follow-up after E3): the secondary indicator NOW lands as a
  `secondary: Option<Rect>` FIELD on `CaretVisual` (not a standalone component)
  + a second solid-stamp instance reusing the primary's entry/color/clip/page;
  the secondary sits at the boundary's BEFORE-glyph logical-end visual edge.
  Why a field, not a component: the extract producer's component
  query/`Changed` trigger/`RemovedComponents` params are all at Bevy's 15-tuple
  cap (extract.rs § 6.1), so a 16th seat is impossible without refactoring the
  hottest text system — the field rides `Changed<CaretVisual>` damage and
  `RemovedComponents<CaretVisual>` clear for free.*
- **Caret blink** is a `CaretVisual { visible, rect }` state edge written by
  render-prep ([decoration-and-paint.md § 6.3](decoration-and-paint.md)); the
  edge rebuilds `ExtractedGlyphs` through the **independent glyph damage
  gate** (`prepare.rs:230-283`), so a blink re-uploads only the glyph buffer
  — a quad caret would re-upload the whole quad buffer every blink. Timer
  resets on every edit and caret move; reduced-motion ⇒ steady caret, no
  blink — never a shader concern.
- **Preedit underline** (§ 6) is a quad-tier underline forced over the preedit
  range — the painting primitive pinned in
  [decoration-and-paint.md § 8](decoration-and-paint.md).

**Runner-up rejected:** a dedicated caret/selection primitive + shader — adds a
pipeline, a WGSL branch, and GPU-lane tests for zero visual capability over
what the GPU-verified quad path and the warm stamp texel already do. **Also
rejected (this file's pre-review draft):** the quad-tier caret "emitted on the
next layer" with blink as a quad visibility toggle — falsified by the two
as-built facts above (the fixed paint rank; the independent quad/glyph damage
gates). **F**

---

## § 6 IME composition

The committed bet stands: **winit owns the IME state machine; Buiy translates.**
Bevy 0.18's `Ime` Message enum is the surface — `Preedit { window, value,
cursor: Option<(usize, usize)> } | Commit { window, value } | Enabled { window }
| Disabled { window }` (bevy_window-0.18.1 `src/event.rs:253-284`).

### § 6.1 Decision: preedit as a display-splice, not an overlay

**Decision.** On `Ime::Preedit`, the preedit string is **spliced into the
editor's (display) Buffer** at the caret as a metadata-marked `Attrs` span;
each subsequent `Preedit` replaces the span; `Preedit` with empty value, or
`Ime::Disabled`, or focus loss removes it. **F**

**Rationale.** Web parity requires preedit to **reflow** the line: composing
CJK mid-paragraph shifts following text and can re-wrap, and the preedit-cursor
F row is only correct when the preedit participates in real shaping.
**Runner-up rejected:** the pure overlay decoration (the literal prior-art
phrasing — "never touch the Buffer", render a separately-shaped run at the
caret): it cannot reflow, so it is only correct for append-at-end fields. This
is the § 1 supersession — deviating from lessons.md's *letter* for display
while keeping its *bet* via § 6.2. **Also rejected:** insert-and-rollback
(commit preedit chars via `Action::Insert`, undo on each update) — pollutes the
`Change` stream and risks event leaks; bevy_cosmic_edit's "commit-only, no
preedit render" gap is the cautionary tale.

### § 6.2 The four invariants (normative)

(a) **Undo never sees preedit:** no `start_change` wraps preedit splices, so no
`Change` reaches the undo stack. (b) **Value reads exclude preedit:**
`TextChanged` payloads and any logical-value accessor skip the preedit byte
range. (c) **Commit is one undo unit:** `Ime::Commit` deletes the span and
inserts the committed text inside a single `start_change`/`finish_change` pair
(§ 8). (d) **No orphans:** `Ime::Disabled` / focus loss / `Escape` removes the
span. The preedit underline styles the marked span (§ 5); the in-preedit cursor
from `Preedit.cursor` (byte range into the preedit string) renders as a caret
inside the span. **F**

### § 6.3 Popup positioning through Bevy 0.18

**Decision.** Use the supported `Window` fields: set `Window.ime_enabled = true`
while a focused, non-`ReadOnly`, non-`Disabled` `TextEditState` exists (false
otherwise); on every caret move / preedit update write `Window.ime_position` =
the caret rect's bottom-left in **logical window coordinates**
(`Edit::cursor_position()` buffer-local → node `GlobalTransform` → window
space). bevy_winit forwards these to `set_ime_allowed` / `set_ime_cursor_area`
(bevy_winit-0.18.1 `src/system.rs:503-512`). **F**

**Known limitation (verified in vendored source):** bevy_winit hardcodes the
exclusion-area *size* to `PhysicalSize::new(10, 10)`, so positioning is
caret-accurate but the candidate popup may overlap a tall line. Accepted for
v1; the cure is an upstream Bevy issue/PR plumbing a size through. **Runner-up
rejected:** reaching the `winit::Window` handle directly to pass a real
caret-sized rect — it races bevy_winit's cache-diff writes of the same property
and couples Buiy to bevy_winit internals, exactly the bridge-crate coupling the
post-mortem warns against. Revisit only after the upstream size plumbing lands.

> **As landed (E5): `Window.ime_position` is `Vec2`, not `Option<Vec2>`.**
> bevy_window 0.18.1 types `ime_position: Vec2` (a plain field, not optional),
> so `write_ime_window` (`text/edit/ime.rs`) writes the caret bottom-left
> directly (value-compared to avoid re-ticking `Changed<Window>`); there is no
> "clear to `None`" — when no editor is focused, `ime_enabled` goes false and
> the stale position is inert.

Composition Messages (`CompositionStart/Update/End`, § 11) emit on the
`Preedit`-empty→non-empty, non-empty→non-empty, and `Commit`/cancel transitions
respectively. Platform variance in `Preedit.cursor` semantics is a named
verification risk (§ 12).

---

## § 7 Clipboard

**Decision.** `arboard` 3.6.x as a direct dependency, **behind a `buiy_core`
facade** — a `ClipboardProvider` Resource trait-object so tests inject a fake
and the dependency stays swappable. `cargo deny check` runs at adoption (the
CLAUDE.md supply-chain rule; `deny.toml` exists; arboard is not yet in
`Cargo.lock`). **F**

**Rationale.** winit exposes no clipboard API and Bevy 0.18 ships none, so a
dependency is mandatory for the F cut/copy/paste row. arboard is the ecosystem
standard (egui/bevy_egui lineage), 1Password-stewarded, MIT/Apache, covering
text + image on X11/Wayland/macOS/Windows. **Runner-up rejected:** hand-rolled
per-platform crates (wl-clipboard-rs / x11rb directly) — re-implements
arboard's platform matrix for no supply-chain win; arboard's transitive deps
are the same crates. **Also rejected:** deferral — cut/copy/paste is F and
cheap once the facade exists.

**Phasing.** v1 ships plain text (`Cut`/`Copy` from `copy_selection()`, `Paste`
through the § 3.3 newline policy). The F row names text + HTML + image MIME:
HTML/image flavors are the named follow-up slice — arboard's HTML *read-side*
support is **unverified** and must be confirmed before that slice is promised
(§ 13).

---

## § 8 Undo / redo with composition-aware grouping

**Decision.** A Buiy-owned two-stack model over the verified `Change` substrate
(`Change::reverse()` + `Edit::apply_change()` are the exact replay pair):

```rust
pub struct UndoUnit {
    change: cosmic_text::Change,
    caret_before: Cursor,  caret_after: Cursor,
    selection_before: TextSelection, selection_after: TextSelection,
    group: GroupKind, // Composition | TypingRun | DeleteRun | Discrete
}
pub struct UndoStack { undo: Vec<UndoUnit>, redo: Vec<UndoUnit>, /* bounded */ }
```

Grouping rules: an IME composition is **one unit** (§ 6.2c); consecutive
typing coalesces by time window + caret adjacency into a `TypingRun`;
consecutive same-direction deletes coalesce likewise; any motion, click, or
discrete command seals the open group. Undo restores `caret_before` /
`selection_before`; redo restores the `_after` pair. The redo stack clears on
any new edit. The stack is depth-bounded (config; v1 default 1000 units). **F**

> **As landed (E4): a no-op edit yields an empty `Change`, dropped at record.**
> cosmic 0.19's `finish_change` returns `Some(Change { items: [] })` (not
> `None`) for an edit that changed nothing — Backspace at offset 0, Delete at
> end. `UndoStack::record` / `record_grouped` (`text/edit/undo.rs`) drop a change
> whose `items` are empty, so the stack stays clean and the logical value stays
> unchanged. The replay pair (`Change::reverse` + `Edit::apply_change`) is
> otherwise exactly as designed.

**Rationale.** The differentiating F requirement — composition-as-one-unit plus
caret/selection restoration — is Buiy-layer aggregation **no** option provides;
once grouping exists, the residual stack is ~a hundred lines. **Runner-up
rejected:** `cosmic_undo_2::Commands<Change>` (the crate `ViEditor` uses) —
buys only its replay model at the cost of a new dep (single-purpose, last
published 2023-11-15) and an impedance mismatch with grouping. **Also
rejected:** `ViEditor`'s built-in stack (§ 2.1). bevy_cosmic_edit *removing*
undo in 0.17 is the canonical "cannot be an optional plugin" warning
([lessons.md "Removing undo/redo"](../../prior-art/bevy-cosmic-edit/lessons.md)):
undo lives in core, on by default.

---

## § 9 Auto-scroll-into-view

**Decision.** The editor's viewport pans via Buiy's layout `ScrollOffset` (x
for single-line, y for multi-line); the Buffer is laid out at full content size
and never scrolls internally. After each caret move / edit, compute the caret
rect (§ 4.1) and clamp `ScrollOffset` so the rect stays inside the clip
viewport with a small margin; `Action::Scroll { pixels }` / `PageUp` / `PageDown`
lower to `ScrollOffset` deltas too. **F**

**Rationale.** `ScrollOffset` already exists with the exact property needed —
mutating it deliberately does **not** invalidate Taffy layout
(`crates/buiy_core/src/layout/components.rs:516-526` and its invariant test) —
and it composes with Buiy's overflow/clip/scrollbar/snap machinery.
**Runner-up rejected:** cosmic-text's Buffer-internal scroll
(`Buffer::set_scroll` + `Action::Scroll`) — a second, invisible scroll model
the layout system cannot see: scrollbars, overscroll, and scroll-into-view of
the *input itself* would disagree, and `Editor`'s vertical motions would fight
Buiy's clip rect. **Accepted cost:** very large documents shape fully (no
virtualization) — fine for F-tier inputs; virtualization is E-tier rich-editor
territory.

> **As landed (E6): the clamp uses `ResolvedLayout.size`, not a content-box
> extent.** `auto_scroll_caret` (`text/edit/scroll.rs`) clamps the caret rect
> into the node's **border-box** viewport (`ResolvedLayout.size`) with a
> generous `SCROLL_MARGIN` that absorbs the small border/padding inset for v1,
> rather than resolving the content box — `Edges.border` / `Edges.padding` are
> `Length` (not `f32`), so a precise content-box extent would need a `Length`→px
> resolution this v1 deliberately skips (a trivial follow-up if a fixture ever
> shows the margin is too coarse). `SingleLine` ⇒ pan x; multi-line ⇒ pan y;
> `ScrollOffset` still does not invalidate Taffy.

---

## § 10 Focus and lifecycle

All editing systems gate on `FocusedEntity` (`crates/buiy_core/src/focus.rs:38`)
pointing at an entity with `TextEditState` and not `Disabled`. On focus gain:
caret becomes visible, blink timer resets, `Window.ime_enabled` goes true
(unless `ReadOnly`). On focus loss: open undo group seals, preedit span is
removed (§ 6.2d), `ime_enabled` goes false, caret hides; **selection is
retained** (web-parity: re-focus restores it). `FocusVisible` (focus.rs:45)
keeps driving the focus ring; the caret is independent of it. The blink timer
resets on every edit and caret move; reduced-motion ⇒ steady caret (§ 5).
**Placeholder:** when the logical value is empty (preedit excluded, § 6.2b) the
`Placeholder` string renders as a styled display-only Buffer (`::placeholder`
token); the placeholder never enters the editor Buffer and is replaced the
moment a real or preedit character exists. **F**

> **As landed (E6): the placeholder buffer shapes itself under the lock.** The
> `sync_placeholder` system (`text/edit/placeholder.rs`) maintains a distinct
> display-only `PlaceholderBuffer` (not the dormant display `TextBuffer`),
> gated on `value().is_empty() && !has_preedit()`. Because nothing downstream
> reaches a `PlaceholderBuffer` (`TextCommit` reshapes only the buffer
> `TextBufferAccess` reaches), the system locks `SharedFontSystem` and shapes
> its own buffer: cosmic 0.19's `Buffer::set_text` / `set_metrics` are
> **lock-free** (they only record + dirty), so the lone lock-bearing call is the
> trailing `buffer.shape_until_scroll(&mut fs, false)`. The placeholder paints
> as a SEPARATE additive producer branch that does not feed the editor buffer's
> §3.2 run-count assert.

---

## § 11 Events — the Message taxonomy

**Decision.** The editor publishes a full taxonomy of Bevy 0.18 **`Message`s**
(the buffered-event type — `Event` is reserved for observers in 0.18; the
`button.rs:6-8` precedent), each carrying the entity + a minimal payload:

| Message | Emitted on | Payload (beyond entity) |
|---|---|---|
| `TextChanged` | logical value change (never preedit) | — (value read via component) |
| `SelectionChanged` | selection transition | `TextSelection` (multi-range-shaped) |
| `CaretMoved` | caret transition without selection change | `Cursor` |
| `CompositionStart/Update/End` | § 6.3 transitions | preedit string (Update), committed string (End) |
| `EditSubmitted` | single-line Enter (§ 3.3) | — |
| `EditUndone` / `EditRedone` | § 8 | `GroupKind` |

Focus signals stay with the focus module. Events emit on transition, not per
frame, so volume is bounded. **Runner-up rejected:** a single changed event
(bevy_cosmic_edit's `CosmicTextChanged` shape) — a named failure: consumers
were forced to poll-and-diff for cursor/selection/composition state
([lessons.md "No event API"](../../prior-art/bevy-cosmic-edit/lessons.md)). The
taxonomy is what the AccessKit layer and the widget catalog subscribe to —
mechanism, not speculation. **F**

---

## § 12 Verification

Per [verification.md](verification.md), this area is provable almost entirely
headless — synthetic `KeyboardInput` / `Ime` Messages need no winit window:

- **Headless unit:** keymap table tests per platform (key + modifiers →
  `EditCommand`); single-line policy (Enter ⇒ Submit, newline-stripped paste);
  grapheme-correct delete fixtures (emoji ZWJ, combining marks); BiDi fixtures
  asserting mixed-direction selection **rect counts** and split-caret presence;
  IME state-machine table tests over `Preedit`/`Commit`/`Enabled`/`Disabled`
  sequences asserting the § 6.2 invariants (undo stack empty during
  composition, value excludes preedit, commit = exactly one `UndoUnit`);
  clipboard via the fake `ClipboardProvider`; auto-scroll clamp math.
- **Property test:** for arbitrary edit scripts, `apply_change(reverse(c))`
  after `c` is identity on the buffer text, and undo-all restores the initial
  value + caret.
- **GPU lane (`#[ignore]`, additive):** one golden for caret + selection +
  preedit-underline rendering on a mixed-direction fixture, on the existing
  readback harness (`crates/buiy_core/tests/support/mod.rs`).
- **Manual matrix (named, CI-impossible):** real IMEs per platform
  (Wayland/X11/macOS/Windows; CJK + dead keys) against a checklist — winit's
  per-platform `Preedit.cursor` semantics are under-documented (risk § 6.3).
- **Latency:** typing latency (keystroke → glyph visible) is the metric the
  bevy_cosmic_edit no-benchmark lesson says to gate; the fixture + budget live
  with [verification.md](verification.md).

---

## § 13 Phasing — v1 slice and named deferrals

**v1 (this spec's implementation campaign):** `TextEditState` over
`Editor<'static>`; focus-gated keymap translation with the § 3.1 table; mouse
`Click`/`DoubleClick`/`TripleClick`/`Drag` through picking coordinates;
single-range selection behavior on the multi-range type; caret + selection +
preedit painting via the decoration-and-paint seats (selection + preedit
underline = quads via `ExtractedTextQuads`; caret = glyph-tier stamp; BiDi
multi-rects); full IME
lifecycle (§ 6); grapheme-correct delete (inherited); plain-text
cut/copy/paste; `UndoStack` with composition grouping + typing coalescing;
`ReadOnly`/`Disabled`/`Placeholder`/`SingleLine`; caret blink with
reduced-motion; auto-scroll via `ScrollOffset`; the § 11 taxonomy; the
`TextInput` bundle.

**Landed as a follow-up after E3:** the **BiDi split caret** secondary indicator
(§§ 4.1, 5) — `secondary_caret_rect_for` (the before-glyph logical-end edge) on
`CaretVisual.secondary: Option<Rect>` + a second solid stamp. No residual:
the secondary now paints at every LTR↔RTL direction boundary, INCLUDING one on a
soft-wrapped continuation segment — `secondary_caret_rect_for` mirrors the
primary's all-runs scan over the multiple `LayoutRun`s a wrapped logical line
emits (continue past a non-owning wrap run; only conclude `None` after the last),
rather than inspecting just the first `line_i`-matching run.

**Deferred within F (named, next slice, not dropped):** multi-range selection
*behavior* (§ 4.2); HTML + image clipboard flavors (§ 7);
**compose-over-selection** (§§ 6.1, 6.2 — E5 splices the preedit at the caret
and does not replace an active selection first;
[follow-ups.md § Text editing — compose-over-selection](../../plans/follow-ups.md)).
**Out (E-tier):** rich-text edit surface, document virtualization.

---

## Open questions

1. **Frame-ordering for edit→layout — RESOLVED (accepted one-frame latency).**
   `BuiySet` chains Layout → Style → Input → … → Render
   (`BuiySet` in `crates/buiy_core/src/lib.rs`), and `TextSync` / `TextCommit`
   live *inside* Layout (`pipeline.rs`). Editor input lands in `BuiySet::Input`,
   two sets **after** Layout, so a keystroke mutates the Buffer after this frame's
   layout already ran; the change is picked up by **next-frame** `TextSync` and
   flows TextSync → measure → TextCommit → `extract_buiy_glyphs`, publishing one
   frame later (N → N+1). Caret geometry, `ime_position` (§ 6.3), and auto-scroll
   (§ 9) come current the **same** frame the edit's TextCommit publishes, because
   the caret reads the `ComputedTextLayout` written in that TextCommit — no extra
   caret lag. This inherits the accepted one-frame latency pinned by
   [architecture.md § 5.1](architecture.md) (a `BuiySet::Style` value reaches
   shaping next-frame TextSync; the editor Input path is the same structure one
   set down).

   **Same-frame re-entry is rejected (blocker-class wrong).** It would need a
   fourth Taffy compute site beyond the architecture's 2×-per-frame cap; the only
   same-frame Layout re-runs (`cq_flip_rerun`, `cq_descendant_rerun`) are
   container-query driven and never fire from Input
   (`measure-and-layout.md` § 4.3). The editing campaign **must not** attempt
   same-frame re-entry — one-frame latency needs **zero** new machinery.

   **Gate caveat.** The T8 typing-latency fixture
   (`crates/buiy_core/tests/text_typing_latency.rs:80-109`) mutates the `Text`
   component *before* Layout, so it proves the **sync-side** path one-frame, not
   the editor Input path. The editor-input latency gate needs its **own**
   Input-driven N→N+1 fixture (edit applied in `BuiySet::Input`, glyph publish
   asserted at N+1); do not cite the T8 fixture as editor-path proof.
2. **Prior-art drift needs a correction note.** Verified against 0.19:
   `Editor::with_selection_bounds` does not exist (real pair:
   `selection_bounds()` + `LayoutRun::highlight`); `Action::Scroll` takes
   `{ pixels: f32 }`, not `{ lines: i32 }`; `FontSystem` and `Editor` ARE
   `Send + Sync`; `Motion` has 22 variants. The `docs/prior-art/cosmic-text/`
   folder should receive a correction note (outside this spec folder's write
   scope), and sibling files citing those claims must re-verify.
3. **arboard HTML read-side** is unverified (§ 7) — confirm before scheduling
   the HTML-clipboard slice.
4. **Shared-accessor type for the editor-owned Buffer** (§ 2.2a): the concrete
   QueryData shape is pinned by [measure-and-layout.md](measure-and-layout.md);
   if that file lands a Buffer-as-separate-component model incompatible with
   `BufferRef::Owned`, the two files must reconcile before implementation.
   *Resolved in review round 1: measure-and-layout § 2.3 pins
   `TextBufferAccess` (display component + optional editor, editor-preferred),
   explicitly compatible with `BufferRef::Owned`.*

## Sources

- cosmic-text 0.19.0 API (verified): [trait.Edit](https://docs.rs/cosmic-text/0.19.0/cosmic_text/trait.Edit.html), [struct.Editor](https://docs.rs/cosmic-text/0.19.0/cosmic_text/struct.Editor.html), [enum.Action](https://docs.rs/cosmic-text/0.19.0/cosmic_text/enum.Action.html), [enum.Motion](https://docs.rs/cosmic-text/0.19.0/cosmic_text/enum.Motion.html), [struct.Cursor](https://docs.rs/cosmic-text/0.19.0/cosmic_text/struct.Cursor.html), [enum.Selection](https://docs.rs/cosmic-text/0.19.0/cosmic_text/enum.Selection.html), [enum.BufferRef](https://docs.rs/cosmic-text/0.19.0/cosmic_text/enum.BufferRef.html), [struct.Change](https://docs.rs/cosmic-text/0.19.0/cosmic_text/struct.Change.html), [struct.LayoutRun](https://docs.rs/cosmic-text/0.19.0/cosmic_text/struct.LayoutRun.html)
- cosmic-text IME issue #10 (open since 2022; embedder-owned): https://github.com/pop-os/cosmic-text/issues/10
- arboard 3.6.1 (1Password): https://crates.io/crates/arboard
- cosmic_undo_2 (rejected runner-up): https://crates.io/crates/cosmic_undo_2
- Vendored Bevy 0.18.1 sources: `bevy_window-0.18.1/src/event.rs:253-284` (Ime), `src/window.rs:279,285` (ime_enabled/ime_position); `bevy_winit-0.18.1/src/system.rs:503-512` (hardcoded 10×10 cursor area); `bevy_input-0.18.1/src/keyboard.rs:109-139` (KeyboardInput); `winit-0.30.13/src/window.rs:1248,1283` (set_ime_cursor_area/set_ime_allowed)
