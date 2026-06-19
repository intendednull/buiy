# TodoMVC → production: complete instruction set

`2026-06-18` · A standalone knowledge-transfer artifact for planning the production work. The implementation it describes lives on a separate exploratory prototype branch (**not part of this PR, not for merge**).

## 0. What this is and how to use it

This branch is a **throwaway exploratory prototype**: it built the classic
TodoMVC app on the current Buiy stack and, in doing so, surfaced and patched
several real library bugs/gaps. **We will not merge it.** Instead, a *fresh*
session will read this report and **plan the production implementation** — doing
each piece the *right* way, informed by everything we learned here.

This document is therefore an **instruction set**: it tells that session
*everything that needs to be done* to reach the working state we have, *why*
each piece was needed, *how the prototype did it*, and — most importantly —
**how to do it correctly in production** (the prototype's approach plus its
limitations). It holds nothing back, including the latent bugs we did **not**
fix and the dead-ends.

**This document is self-contained** — everything needed to understand and act on
it is inline. It is the consolidated output of an exploratory prototype effort
carried out on a **separate, throwaway branch that is not part of this PR and is
not for merge**; that branch is the reference implementation, but you do not need
it — the root causes, fixes, and instructions are all written out below.

**Read order for the planner:** §1 (what we proved) → §2 (the split: real vs
throwaway) → §3–§4 (the load-bearing library work) → §5–§8 (gaps/edges/verification)
→ §10 (the ordered production work breakdown) → §11 (pitfalls).

**Scope of the exploratory work** (so you know how much was touched): roughly
3,000 lines across ~30 files, spanning the picking, text (commit/extract/sync),
a11y, widgets, and verification subsystems, plus a throwaway `examples/todomvc`
app used to exercise the library. The library-relevant changes are what §3–§7
distill; the app itself is scaffolding (§9).

---

## 1. Executive summary — what we proved

1. **TodoMVC is buildable on Buiy today** by assembling landed layout / text /
   editor / button primitives, plus a small number of additions. The prototype
   implements **9 of the 10** canonical behaviors and is **interactive and styled
   in a real window** (clicking toggles checkboxes, typing + Enter adds rows, the
   count updates, filters/clear/destroy work).
2. **It surfaced THREE real, latent `buiy_core` bugs** — none caught by the
   existing test suite, each of which any real interactive Buiy app would hit:
   - a **picking** bug that made *every* nested/centered widget unclickable;
   - a **text-commit** bug that **crashed** any `DefaultPlugins` app ~9 s in (on
     async font load);
   - (a third, **not yet fixed**: TextSync clobbering an editor's typed content).
3. **The through-line lesson:** Buiy's verification has **no tier that runs a real
   `DefaultPlugins` app and drives real winit input**. Every existing test is
   headless (MinimalPlugins) with *synthetic* input, or GPU-capture with a
   *synchronous* font. That blind spot hid all three bugs. **This is the single
   most important thing for production to fix in the verification strategy.**

---

## 2. The split — what is real library work vs throwaway prototype

| Bucket | Files | Production disposition |
|---|---|---|
| **Real `buiy_core` bug fixes** (load-bearing) | `picking/{backend,mod}.rs`, `text/{commit,extract}.rs` | **Must land in production**, properly reviewed + tested. The prototype's fixes are correct but minimal; production should redo with full review + the missing test tier. |
| **Library features** (catalog inputs) | `a11y/{mod,translate}.rs`, `buiy_widgets/{checkbox,button,lib}.rs` | **Design properly** in `buiy-widget-catalog-design`. The prototype *validated the approach* (esp. `A11yToggled` as the state primitive); production designs the real API. |
| **Verification hardening** | `buiy_verify/{a11y,text_shape,lib}.rs`, new regression tests | **Keep**; the `text_shape` predicate + regression tests are reusable. Add the missing real-input tier. |
| **The prototype app** | `examples/todomvc/**` | **Throwaway.** Informs the design (architecture, the restyle, the systems) but is not the production artifact. |
| **Docs** | this report | This report is the standalone deliverable. (The prototype branch also carried deeper gap-design findings and a debugging post-mortem; their substance is folded into this report.) |

Everything under `examples/todomvc/` is scaffolding to exercise the library. The
*library* changes are what production cares about.

---

## 3. Library bugs found + fixed (`buiy_core`) — production MUST land these

These are **not** prototype-specific. They are standing Buiy bugs that the
prototype merely *exposed*. Production needs each fixed regardless of TodoMVC.

### 3.1 Picking hit-tests parent-RELATIVE coords (interactivity-breaking)

**Symptom.** The live app rendered perfectly but was *completely non-interactive*
— clicks resolved to the wrong entity (always the outermost card), so no widget
ever fired.

**Root cause.** `crates/buiy_core/src/picking/backend.rs::emit_picks` and
`picking/mod.rs::{hit_test,point_in_aabb}` AABB-tested the cursor against
`ResolvedLayout.position`. **That field is Taffy's PARENT-RELATIVE location, not
the absolute screen position** (confirmed: `write_resolved_layout`,
`layout/systems.rs:2734`, writes `layout.location` with no parent accumulation —
a `world_position` helper at `systems.rs:373` exists but is unused). The render
does **not** use it: it uses the absolute, bridge-composed `GlobalTransform`
(`render/mod.rs:434`, "pillar 5"). So render and picking diverged — picking only
matched nodes whose ancestors sit at the origin (the full-window `page` → the
`card`). Every nested widget was unreachable. **It was always broken for nested
widgets**; the prototype's *centered card* offset the whole subtree and exposed
it (before, the top-left layout made relative≈absolute for shallow widgets, so
some clicks happened to land — "it worked before").

**The prototype's fix (correct, minimal — adopt it).** Point picking at the same absolute source the
render uses: `emit_picks` + `hit_test` now read
`GlobalTransform.translation().truncate()` (size still from `ResolvedLayout`),
with a fallback to `ResolvedLayout.position` for unit tests that spawn without
the transform bridge. New regression test `crates/buiy_core/tests/picking.rs::
hit_test_uses_absolute_global_transform_not_relative_resolved_layout`.

**How to do it RIGHT in production.**
- The prototype fix is correct and minimal — **adopt it**, but also:
- **Fix the misleading doc.** `ResolvedLayout.position`'s doc comment
  (`components.rs`) says "window-relative" (i.e. absolute). It is **parent-relative**.
  Either fix the comment OR decide `ResolvedLayout.position` *should* be absolute
  and make `write_resolved_layout` accumulate (via `world_position`) — a bigger
  change with blast radius (snapshots, sticky/scroll). The cleaner call (what the
  prototype took) is "picking/overlays use `GlobalTransform`; `ResolvedLayout` is
  layout-local," and fix the comment to say so.
- **Audit every other `ResolvedLayout.position` consumer** for the same trap.
  Confirmed/suspected consumers needing absolute coords: **click-to-place-caret**
  (`text::edit::pointer::pointer_to_cursor` — clicking *in* the text input to
  position the caret; not yet verified, likely also off for nested inputs),
  scroll/sticky math, and any future overlay/tooltip/drag system.
- **The `Entity::PLACEHOLDER` camera** in `emit_picks` (`backend.rs:65`) is a
  Phase-0 TODO; bevy_picking's interaction layer expects a real camera. Buiy's
  own `update_hovered` reads `PointerHits` directly so it works, but wiring a real
  camera ref is on the `buiy-input-events-design` list — do it for production.

**Verification (production must add):** a picking test with a **nested + offset**
tree (not the shallow scene the old test used) — the prototype added exactly this.
Plus the real-input smoke tier (§7).

### 3.2 Text commit skips reshaping an unshaped buffer (crash on async font load)

**Symptom.** Every `DefaultPlugins` app (incl. `hello_button`) **panicked
deterministically ~9 s after the window opened**, at
`crates/buiy_core/src/text/extract.rs:712`:
`assert_eq!(runs, computed.lines.len(), "TextBuffer dirty-unshaped at extract")`.

**Root cause.** `text/commit.rs::text_commit` has a steady-state guard that skips
reshaping when size/align/offset are unchanged. When the **async system-font
scan** completes, it bumps `FontsGeneration`; `TextSync`'s "late fonts never
leave stale tofu" sweep lazily `set_text`s the affected buffers (marking lines
unshaped) **without moving the content box**. The guard's only escape hatch for
that case is the *measure closure* leaving `height_opt = None` — but that runs
only when Taffy re-measures; a buffer whose box is unchanged (or whose measure
cache survives) is **not** re-measured, so commit skips it and the buffer reaches
extract unshaped (`layout_runs()==0` while `ComputedTextLayout.lines==1`) →
the debug-assert fires (and, in release, the glyphs silently never paint).

**The prototype's fix (reviewed clean — adopt it).** `commit.rs` adds a `shape_stale` term: reshape
whenever `buffer.layout_runs().count() != committed lines.len()` — **extract's own
truth**, so it cannot false-positive on height-cropped text (cropped text compares
equal; an unshaped buffer compares unequal). `extract.rs` enriched the tripwire to
name the offending entity. Verified: binary runs 35 s+ with no panic.

**How to do it RIGHT in production.**
- The fix is correct and was reviewed clean (`text_commit` 10/10 incl. the
  "zero reshapes in steady state" contract). **Adopt it.**
- **Honest caveat on the regression test.** `tests/text_commit_font_reload.rs`
  reproduces the font-reload path headlessly, but the simple scenario **auto-heals**
  (`TextSync`'s `mark_dirty` makes Taffy re-measure → reshape regardless of the
  fix), so it does **not** isolate the commit-guard fix. The *real* trigger is a
  node that escapes re-measure (likely the empty editor), which only reproduces
  under the live render schedule. **The fix's primary verification is the running
  binary + the `text_buffers_shaped` invariant (§7).** Production should add the
  DefaultPlugins-font-load smoke test that would isolate it.

---

## 4. Library features added — catalog inputs (`buiy_core` a11y + `buiy_widgets`)

These are **features**, not bugs. The prototype implemented them minimally-but-
really to *validate the design*; production should design the real catalog API in
`buiy-widget-catalog-design`.

### 4.1 a11y `A11yToggled` state seam + `A11yRole::Checkbox` (`a11y/{mod,translate}.rs`)

**What.** The a11y tree previously carried only *identity* (role + name + desc +
focusable). The prototype added the **first piece of widget STATE**:
- `A11yRole::Checkbox` (`mod.rs`), mapped in `translate.rs` to `Role::CheckBox`.
- `A11yToggled(pub bool)` component — a decomposed `aria-checked`/`aria-pressed`
  state. `Default`, `Reflect`, `Copy`, registered.
- `A11yNodeView.toggled: Option<bool>` — `None` = no toggle semantics (distinct
  from present-and-false). `build_tree` reads `Option<&A11yToggled>`, adds it to
  the skip-guard, and populates `toggled: toggled.map(|t| t.0)`.
- `translate.rs::to_accesskit_node`: `if let Some(checked) = view.toggled {
  node.set_toggled(checked.into()); }` (`accesskit::Toggled: From<bool>`).

**Design decision the prototype validated (and production should adopt):**
**`A11yToggled` IS the widget's logical checked state — there is no duplicate
`Checked` component and no sync system.** A single component is the source of
truth; the visual and the AT both read it. This held up cleanly across click,
keyboard, and direct app mutation (toggle-all writes `A11yToggled` directly and
the visual follows for free). **Recommendation:** make `A11yToggled` the canonical
checked/pressed/selected primitive in the catalog spec; generalize to tri-state
(`Mixed`), `Switch`, `MenuItemCheckbox`, toggle buttons (`aria-pressed`). The
`Mixed`/tri-state case is the open generalization (the prototype is 2-state only).

### 4.2 `Checkbox` widget (`buiy_widgets/src/checkbox.rs`, NEW)

**What.** `Checkbox::new(label) -> impl Bundle` (marker + Node + 24×24 Style +
`A11yToggled(false)` source-of-truth + Border + Focusable + `A11yRole::Checkbox`
+ `A11yLabel`), a `Toggled(Entity)` `Message`, click toggle
(`toggle_checkbox_on_click`, mouse-down, mirrors `Button`), keyboard toggle
(`toggle_checkbox_on_key`, Space/Enter when focused), and a single visual writer
(`sync_checkbox_visual` on `Changed<A11yToggled>` — accent fill when checked).

**How to do it RIGHT in production.**
- Mirror `Button`'s structure (validated). But fix the two gaps below (§5).
- **`Checkbox::new(label)` takes no initial-state parameter** → see §5.2.
- The fill is a colored box (no check glyph) — a real checkbox needs a check mark
  (a glyph or a small SDF/path). The prototype used an accent fill (sufficient to
  exercise the harness; production needs the glyph).
- Mouse-DOWN activation (no press-drag-cancel) is deliberately deferred (matches
  `Button`); the catalog's APG pass owns the full press semantics.

### 4.3 Keyboard activation for `Button` (and `Checkbox`) (`button.rs`, `checkbox.rs`)

**What.** `activate_focused_button_on_key` (Space/Enter → `OnPress` when a `Button`
is `FocusedEntity`) and `toggle_checkbox_on_key`. Gated on `FocusedEntity` identity
so a focused text input's Enter (submit) / checkbox's Space never collide. Closes
the `button.rs` Phase-0 TODO for the keyboard path. **Still deferred:** the
mouse-DOWN→UP press-drag-cancel semantics (flagged minor; APG catalog work).

### 4.4 Pre-existing infrastructure relied on (NOT our work, but load-bearing)

`text_input.rs::focus_on_click` (click-to-focus for `TextInput`) and
`text::edit::pointer::pointer_selection` (focus-on-click for editors) already
existed; they read `Hovered`, so **they only worked once §3.1 (picking) was
fixed**. Production note: the whole focus/typing chain was gated behind the
picking bug — fixing picking is what made text entry work.

---

## 5. Widget-catalog gaps (must address for ANY real UI)

### 5.1 Button (and the catalog) renders NO visible label — **Gap 3**

`Button::new(label)` carries an `A11yLabel` (screen-reader name) but **no visible
`Text`**. Every button paints its box and no label glyph — `hello_button`'s "Save"
is a blank box too. It is also fixed-size (120×32), which would clip most labels.
**No test caught it** (the ECS tiers assert the `A11yLabel` is present; the
`hello_button` golden baked the blank box in as "expected").

**Production requirement:** the catalog's Button **must render its own label** —
content-sized to the label, centered, tinted with a foreground token that
contrasts the surface.

**CRITICAL interaction with picking (learned the hard way):** Buiy's `hit_test`
picks the **smallest-area** node under the cursor. So a label rendered as a
**child** `Text` of the button would *steal the hover* from its button and the
button would never activate on a label click. The prototype's working recipe:
**co-locate `Text` on the button entity itself** (so there is no smaller child) and
auto-size it. Production must either do that, or give picking a
pointer-events/hit-target-bubbling story so composite widgets work. **This is a
real architectural input for the catalog + picking design.**

### 5.2 Widget constructors bundle their state → no inline initial state

`Checkbox::new` carries `A11yToggled(false)`; spawning `(Checkbox::new(t),
A11yToggled(true))` is a **duplicate-component panic** (bevy bundle dedup). The
prototype works around it with an insert-after-spawn override. Worse, the correct
*initial visual* of an initially-checked checkbox is then **load-bearing on
change-detection** (`sync_checkbox_visual` must run before the first paint).
**Recommendation:** widget constructors that own a state component should take it
as a parameter (`Checkbox::new(label, checked)`) or expose a builder.

### 5.3 Prelude gap — `buiy` does not re-export the app-facing surface

A real app cannot be written against `buiy` (the umbrella) alone. The example
needs `buiy_core`/`buiy_widgets` directly for: the new `Checkbox`/`Toggled`/
`A11yToggled`, and the **editor API** (`EditSubmitted`, `EditCommand`,
`TextEditState`, `TextDecorations`, `DecorationLines`) — all squarely app-facing.
**Recommendation:** expand `buiy`'s re-exports (the prelude) to cover the editor
message/command surface and the widgets so `use buiy::*` suffices.

---

## 6. Styling reality (Gap 4) — what the live render path actually paints

We mapped Buiy's styling surface against what the **live** draw path
(`render/mod.rs::extract_buiy_draws` + `draw_for_node`) actually consumes. **The
gap between component types that exist and pixels that render is wide:**

| Want | Component exists | Renders live? |
|---|---|---|
| Background fill | `Background` | **yes** |
| Rounded corners | `Border.radius` | **yes** (uniform — `top_left.x`, px) |
| Visible border (color/width/sides) | `Border` sides | **no** |
| Drop / inset shadow | `BoxShadow` (+ `shadow.wgsl`) | **no** — `extract.rs:348` "FAN … reserved" |
| Outline | `Outline` | **no** |
| Gradient | — | **no** |
| Group opacity | `Opacity` | not via the simple draw path |
| Token-color alpha | `ColorToken` → `Color` w/ alpha | **yes** (the only transparency) |
| Text color/size/weight/align/decoration | text components | **yes** |
| Flexbox + spacing + sizing | `Style` | **yes** |

So today's achievable look is **flat**: background fills, uniform rounded corners,
rich text, flexbox. The prototype's restyle (centered white card, muted-red title,
teal checkboxes, labelled buttons) works within that. **Two more concrete findings:**
- **Custom colors must be theme tokens.** `ColorToken` has **no literal-color
  variant** (only `Transparent` + `Token(name)`). Every custom color is added to
  the `Theme` resource (`theme.rs`, public `colors: HashMap<String,Color>`) and
  referenced by name; insert a custom `Theme` *after* `BuiyPlugin` to win. A
  `ColorToken::Literal(Color)` would save app authors the theme dance.
- **`ResolvedLayout.position` is parent-relative** (the picking bug, §3.1) — the
  render survives only because it uses `GlobalTransform`.

**Production recommendation:** wire `BoxShadow`/`Outline`/border-sides into
`extract_buiy_draws` (or document they're effect-group-only) so the catalog can
ship cards, focus rings, and separators that actually paint. Add
`ColorToken::Literal`.

---

## 7. Verification — hardening done + the structural gap

### What we added (keep these)
- **`buiy_verify::text_shape::text_buffers_shaped`** (`text_shape.rs`, NEW): a
  **CI-runnable (no-GPU)** invariant — the main-world twin of the extract
  dirty-unshaped tripwire. Asserts `layout_runs().count() == ComputedTextLayout
  .lines.len()` for every painted text entity, mirroring extract's exact skip set
  + truth. Dogfooded from `examples/todomvc/tests/verify_text_shape.rs`.
- **Regression tests:** `text_commit_font_reload.rs` (font-reload invariant guard;
  honest caveat in §3.2) and `picking.rs`'s nested-tree `hit_test` test.
- **`buiy_verify::a11y` serializer** extended to carry `toggled` (with
  `skip_serializing_if` so pre-existing a11y snapshots stay byte-identical) and
  stringify the `Checkbox` role — the a11y tree of a stateful widget is now
  verifiable. Plus `a11y_translate.rs` tests (the absent/false/true distinction).
- **Tier-1 layout + Tier-2 display-list snapshots** of the app (external-consumer
  dogfood; `#[track_caller]` lands `.snap` beside the caller — confirmed working).

### THE GAP (the most important verification lesson)
**No test tier runs a real `DefaultPlugins` app and drives real winit input.**
Every existing tier is one of: headless `MinimalPlugins` + *synthetic* `Hovered`/
`KeyboardInput` (the e2e behavior tests), or GPU-capture with a *synchronous* Ahem
font (no async load). **This blind spot hid all three bugs:**
- The async-font crash needs `AssetPlugin` (no headless tier has it).
- The picking bug needs `emit_picks` + real cursor coords (the e2e tests bypass
  `emit_picks` by setting `Hovered` by hand).
- The editor clobber (§8) needs the async font sweep mid-typing.

**Production must add a top tier:** a real `DefaultPlugins` smoke harness (run the
actual app a few frames on a GPU host, drive synthetic-but-real winit input via
XTEST or `bevy`'s input events, assert no panic + assert interactivity). This is
the highest-leverage verification investment. (For cheap local diagnosis, the
in-app `diag`-system method in §11 cracked the picking bug in one build: app-binary
(`main.rs`)-only rebuilds are ~3–19 s; only `buiy_core` changes cost the ~35-min
full rebuild.)

---

## 8. Known edges / latent bugs we did NOT fix (hold nothing back)

1. **TextSync clobbers a focused editor's typed content.**
   `text/sync.rs::apply_authored_to_buffer` (line ~530) does
   `buffer.set_text(display Text)` on the **editor-owned** buffer (the accessor is
   editor-first), and a `TextInput`'s display `Text` is `""`. So **any** `TextSync`
   run on the input — notably the async **font-generation sweep** — does
   `set_text("")` and **wipes the in-progress typed text**. We confirmed the
   mechanism in code; our live test got lucky (the sweep didn't coincide with
   typing, `TextSync applied = 0`), so text entry *worked*, but this is a real
   latent bug: type at the wrong moment and your text vanishes → empty submit → no
   row. **Production must fix:** `TextSync` must NOT lower the display `Text` into
   an editor's buffer — `TextEditState` owns its content; only *style*
   (metrics/wrap) should sync for editor entities. (The prototype first
   saw this as a "settle before typing" timing quirk; it is now root-caused to the
   `set_text` clobber above.)
2. **`ResolvedLayout.position` doc says "window-relative"** but is parent-relative
   (§3.1). Fix the comment (or the field).
3. **Click-to-place-caret** (`pointer_to_cursor`) is another `Hovered`+position
   consumer; the focus path is fixed, but the *caret-index math* may still use
   relative coords for nested inputs — **verify**.
4. **Edit-in-place (the 10th TodoMVC behavior) is DEFERRED.** Double-click → edit →
   Enter/blur commit, Esc cancel. The editor substrate supports it (double-click
   selection, `EditSubmitted`, focus-blur lifecycle, `value()`, Escape), but the
   *trigger* — double-click detection on a non-text widget — is a flagged gap
   (`ClickTracker` is private to `text::edit`). Production: expose a general
   double-click/gesture signal, then edit-in-place falls out.
5. **`apply_filter` owns the display kind** (writes `Display::None`/`flex_row`).
   Works only because every row is `flex_row`; a row with any other display would
   be silently rewritten. A real filter must not own the display kind (cache the
   original, or use a `Hidden`/`content-visibility` mechanism the layout honors).
6. **Box-sizing quirk:** the prototype used `.border_box()` to make
   `width + padding` not overflow; the default is content-box. Production layouts
   must choose box-sizing deliberately.

---

## 9. The prototype app architecture (informs the design; NOT merged)

`examples/todomvc/src/lib.rs` (620 lines) is the reference for "what a real
interactive Buiy app looks like." Retained-mode ECS, the intended Buiy model:
- **One entity per row:** `TodoRow { title, completed, label: Entity, checkbox:
  Entity }`; `RowOf(Entity)` back-references from checkbox/destroy to the row.
- **State = the row's `completed` bool**, mirrored to the checkbox's `A11yToggled`.
  Plain systems + change detection keep the view in sync; **no signals/reactivity
  layer** (a deliberate v1 non-goal — and it was ergonomic enough).
- **9 systems**, chained in a `TodoLogic` set `.after(BuiySet::Input)` (so widget
  messages emitted in `BuiySet::Input` are readable the same frame): `add_todo_on_submit`,
  `toggle_todo`, `toggle_all`, `destroy_todo`, `clear_completed`, `set_filter`,
  `restyle_completed`, `apply_filter`, `update_count`.
- **Restyle:** a `todo_theme()` palette over `default_light_theme()`; a full-window
  `page` node (`width/height Sizing::Length(Length::Percent(100))`) centering a
  fixed-width white `card`; labelled buttons (co-located `Text`, §5.1); rounded
  teal checkboxes; dimmed + struck completed labels.

**App-architecture usage notes (cost time to discover; document for app authors):**
- Widget messages (`OnPress`/`Toggled`/`EditSubmitted`) are emitted in
  `BuiySet::Input`; app logic that reads them must be `.after(BuiySet::Input)`.
- `Hovered` is set by picking in `BuiySet::Picking` (after `Input`), so headless
  tests must set it by hand and re-assert each click.
- `World::write_message(EditSubmitted)` *before* `update()` races the message
  double-buffer; drive the real input path instead.
- Despawn auto-updates the parent `Children` (no manual removal needed).
- One-frame visual latency is normal (change-detection-driven visuals).

---

## 10. Production work breakdown — the ordered instruction set

What a fresh session needs to *do* (the "right" way) to reach the working state we
have. Rough order = dependency order.

**A. Library bug fixes (land first — everything else depends on a working app).**
1. **Picking → absolute coords** (§3.1). Adopt the `GlobalTransform` fix; fix the
   `ResolvedLayout.position` doc; **audit all `ResolvedLayout.position` consumers**
   for the relative-as-absolute trap (caret math, sticky/scroll, future overlays);
   wire a real camera ref into `emit_picks`. Add the nested-tree picking test.
2. **Text commit → reshape unshaped buffers** (§3.2). Adopt the `shape_stale` fix.
   Add the DefaultPlugins-font-load smoke test that actually isolates it.
3. **TextSync editor-clobber** (§8.1). Make `TextSync` not overwrite an editor's
   owned content. This is required for *reliable* text entry.

**B. Verification — the missing tier (highest leverage).**
4. Add a **real-`DefaultPlugins`-app + real-input smoke tier** to `buiy_verify`
   (§7). Keep the `text_shape` predicate + the regression tests. This tier is what
   would have caught all three bugs and prevents regressions.

**C. Widget catalog (`buiy-widget-catalog-design`).**
5. **Button renders its label** (§5.1) — content-sized, centered, foreground token;
   resolve the child-vs-co-located hit-test question (and/or give picking a
   pointer-events story). Re-bless `hello_button`'s golden.
6. **Checkbox** as a real widget: a check glyph (not a fill), `Checkbox::new(label,
   checked)` initial-state param (§5.2), the APG keyboard/press contract.
7. Adopt **`A11yToggled` as the canonical state primitive** (§4.1); generalize to
   tri-state/`Switch`/toggle-button. Keep the `A11yNodeView.toggled` +
   `translate.rs` seam and the `buiy_verify::a11y` serialization.
8. Keyboard activation (§4.3) + the deferred mouse-down→up press semantics.

**D. Styling / theming.**
9. Wire `BoxShadow`/`Outline`/border-sides into `extract_buiy_draws` (§6); add
   `ColorToken::Literal`. These unlock the canonical look (shadows, focus rings,
   separators).

**E. Prelude / ergonomics.**
10. Expand `buiy`'s re-exports (§5.3) so apps don't reach into inner crates.
11. App-author guidance: the `TodoLogic.after(Input)` ordering, the editor settle
    behavior, box-sizing (§9).

**F. The deferred behavior.**
12. Expose a double-click/gesture signal; implement edit-in-place (§8.4).
13. Make filter visibility not own the display kind (§8.5).

---

## 11. Pitfalls + dev-environment notes (so the new session is efficient)

- **Builds are ~35 min** on this box: a *separate parallel job* builds the
  `bsn-support` worktree, contending the global `~/.cargo` package-cache lock, and
  rust-analyzer re-runs `cargo check --all-targets` on every `.rs` edit.
  **Mitigations:** batch all `.rs` edits, then build once; markdown edits are
  safe; **iterate diagnostics in `main.rs` only** (buiy_core stays cached → ~3–19 s
  rebuilds) and save the full buiy_core rebuild for the actual fix.
- **GUI testing flakiness:** multiple windows are titled "todomvc" (the app *and*
  terminals showing this work) — **find the app window strictly by `--pid`**, not
  `--name`. `pkill -f "target/debug/todomvc"` **self-kills** the launching shell
  (its command line contains the path) — kill by explicit PID. Fish shell:
  `setsid ./app > log 2>&1 &` works; `nohup`/`disown` idioms don't. XTEST input
  (`xdotool mousemove/click` *without* `--window`) reaches winit only if the app
  window is raised (`xdotool windowactivate --sync`); `--window` (XSendEvent) is
  ignored by winit.
- **`:0` reports "Scale factor: 0 / no monitor"** — a **red herring**: that's
  `scale_factor.unwrap_or(0.)` on a virtual X server with no RANDR monitor; the
  window's actual factor is 1.0. Do **not** chase it as a cause of input failure.
- **The cheap diagnostic that works** (reproduce it directly): add a temporary
  per-frame system to the app binary that logs, on hover-change / mouse-press /
  key-press, the cursor (`PointerLocation`), `Hovered`, `FocusedEntity`, and the
  `TextCommitReshapeCount`; plus a one-shot system that dumps every widget's
  `ResolvedLayout` (pos+size) and `GlobalTransform`. That dump is what revealed
  the cursor hitting the `card` while the widgets' `ResolvedLayout.position` held
  small parent-relative values — i.e. the picking bug — in a single build.

---

## 12. Codebase file map (where the work lives, for planning)

These are paths in the Buiy source tree — the places to read and change when
implementing the production version. (They are codebase locations, not external
documents; everything you need to *understand* the work is already inline above.)

- **Picking (the §3.1 fix + the relative-coords trap):**
  `crates/buiy_core/src/picking/backend.rs` (`emit_picks`),
  `crates/buiy_core/src/picking/mod.rs` (`hit_test` / `point_in_aabb`).
- **The absolute-position source the render uses (pillar 5):**
  `crates/buiy_core/src/render/mod.rs` (`draw_for_node`, `GlobalTransform.translation`).
- **The relative-position writer (the trap's origin):**
  `crates/buiy_core/src/layout/systems.rs` — `write_resolved_layout` (writes
  Taffy's parent-relative `layout.location`) and the unused `world_position` helper.
- **Text commit (the §3.2 crash fix):** `crates/buiy_core/src/text/commit.rs`
  (`text_commit` reshape guard); the tripwire it feeds:
  `crates/buiy_core/src/text/extract.rs`.
- **The TextSync editor-clobber seam (§8.1, NOT yet fixed):**
  `crates/buiy_core/src/text/sync.rs` (`apply_authored_to_buffer`, the editor-first
  `with_buffer_mut`).
- **a11y state seam (§4.1):** `crates/buiy_core/src/a11y/mod.rs` (`A11yToggled`,
  `A11yRole::Checkbox`, `A11yNodeView.toggled`, `build_tree`) and
  `crates/buiy_core/src/a11y/translate.rs` (`set_toggled` / `Role::CheckBox`).
- **Widgets (§4.2–§4.3, §5.1–§5.2):** `crates/buiy_widgets/src/checkbox.rs`,
  `crates/buiy_widgets/src/button.rs`, `crates/buiy_widgets/src/lib.rs`.
- **Verification (§7):** `crates/buiy_verify/src/text_shape.rs` (the predicate),
  `crates/buiy_verify/src/a11y.rs` (the `toggled` serialization).
- **Theme / styling (§6):** `crates/buiy_core/src/theme.rs` (`Theme.colors`,
  `default_light_theme`), `crates/buiy_core/src/render/components.rs` (`ColorToken`,
  `Background`, `Border`, `BoxShadow`, `Outline`).
