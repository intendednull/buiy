# Widget-gallery exemplar — child C8 of the widget-catalog campaign

`2026-06-22` · `[draft]` · Wave 5 · realizes foundation `media-and-widgets §3.10` (per-widget verification-fixture-coverage), `verification` (gates 2/3/4/7/10/11) · depends on C0, C3, C4, C5, C6, C7

> **Parent:** [README.md](README.md) (the umbrella — read §2 decisions incl. **§2.7 coordinate-with-agent-interface**, §4 decomposition, §6 cross-cutting arbitration first). This child is **pure composition + the app-author narrative**: it defines **no** primitive, component, or system that a sibling owns. Everything it touches is authored on top of C3–C7's surfaces **and the agent-interface campaign's P1d widget bundles** (see [Coordination with the agent-interface campaign](#coordination-with-the-agent-interface-campaign)). Where a contract is shared, it references the umbrella §6 by number rather than redefining it. Siblings C1–C7 are drafted in parallel — this spec relies on their umbrella-pinned contracts, not on their files existing yet — and the agent-interface P1d widgets (`docs/specs/2026-06-18-buiy-agent-interface-design/`) are the **canonical widget bundles** the gallery composes.

---

## 1. Problem & current state

The campaign's chosen exemplar (umbrella §2.1) is a **widget gallery** that exercises scrolling/long-lists, overlays/menus, modal + focus-trap, and the full F-tier look, with TodoMVC subsumed as **one** screen. Today none of that exists as a verifiable artifact, and the prototype's flat 2-row card exercises almost none of the catalog the foundation promises. Concretely:

- **The exemplar is a throwaway on a superseded base.** The prototype app (`/mnt/storage/projects/buiy/.claude/worktrees/todomvc-prototype/examples/todomvc/src/lib.rs`) is bevy 0.18 / accesskit 0.21 / pre-`buiy_bsn`, hand-assembles `Button` via a co-located-`Text` hack (`labeled_button`, lib.rs:90–112), bundles state via `A11yToggled` (lib.rs:23, :248), and is **not** wired to `buiy_verify` — it is an example binary with ad-hoc tests on the prototype branch, not a fixture corpus. The audit (§7) is explicit: **re-derive, do not cherry-pick** — adopting it literally reverts main's widget architecture (audit Appendix A.1).

- **The verification harness has exactly ONE fixture.** `crates/buiy_verify/fixtures/` holds only `button/resting.rs` (verified: `find …/fixtures -type f` → one file). The coverage machinery (`coverage/fixture.rs`, `coverage/enroll.rs`, `coverage/matrix.rs`) is built and proven, but there is **no gallery**, no scroll fixture, no overlay/modal fixture, no multi-widget composite. The foundation requires every widget ship "coverage by the verification fixture matrix … (gates 2 — visual regression, 3 — AccessKit tree snapshot, 4 — announcement output, 7 — APG keyboard contract)" (`media-and-widgets.md:43`). The gallery is the **coverage-by-construction** vehicle that satisfies that requirement for the composite/screen level.

- **No screen demonstrates the capabilities the campaign builds.** Scroll/wheel input does not exist on main (audit §6.4: `grep MouseWheel` → nothing); overlays/menus/modal/focus-trap are unbuilt (audit §5 "deferred behaviors"); the F-tier look (shadows on cards, per-side borders, outlines, the focus ring) is unfed through extract (audit §6.1, §4 styling row). C3–C6 build these; **C8 is the screen that proves they compose** under real input, real focus, and real paint.

- **The load-bearing app-author rules are scattered prose, not a guide.** The prototype encodes them as comments and a `chain()`-ed system set: logic must run `.after(BuiySet::Input)` (lib.rs:602), the editor must settle before typing (audit §3 "settle 5 frames" timing-luck), `border_box` sizing math (lib.rs:389–391), despawn semantics for rows (lib.rs:498–508), message-timing so a single `update()` settles an interaction (lib.rs:591–593). The audit flags the absence of an **app-author guide doc** as a Medium-priority miss (audit §6.9, §8 item 21). C8 owns writing it down.

This child does **not** re-fix any of the three bugs, build any widget, or wire any extract path — those are C1–C7. It **composes** their outputs into screens, authors those screens as fixtures so coverage falls out by construction, settles C5's virtualization ceiling with a 1000-row scale-game, and writes the app-author guide.

---

## 2. Target design

### 2.1 The gallery as a screen set

The gallery is a set of **screens**, each a self-contained BSN-authored scene (`fn() -> impl Scene`, the `hello_bsn` pattern at `examples/hello_bsn/src/lib.rs:31`) living in a new `buiy_gallery` crate under `examples/`. Each screen is `Name`-tagged at every entity so dumps are content-keyed (snapshot.rs keys on `Name`, never `Entity` bits). A top-level screen-switcher (a `MenuButton` or tab strip, itself a gallery widget) navigates between them; the binary boots to the screen-switcher, and each screen is **also** exported as a `pub fn screen_*() -> impl Scene` so a `buiy_verify` fixture can spawn the exact same tree the binary renders (the "example IS the fixture" discipline, `hello_bsn` doc-comment lines 5–12).

**The five screens (resolved — see §3.1):**

| # | Screen | Composes (widget owner / container owner) | Capabilities proven |
|---|---|---|---|
| **S1** | **TodoMVC** | agent-interface **TextInput** + tri-state **Checkbox** (P1d bundles) + **Button**/destroy (P1d / `buiy_widgets`) + filter `Radio`/toggle group (C4/C5) + activation via `OnPress`/the agent-interface action router (umbrella §2.7) + `ValueChange` (C4) + card shadow + per-side border (C6) | The literal exemplar: edit-in-place (double-click on a row via C3's **widget-agnostic `MultiClick` `EntityEvent`**, input-event-model §2.11 — derived from the `ClickTracker` timing since bevy_picking 0.19 has `Click.count`/`Press.count` but no `Pointer<DoubleClick>`; a todo row is not an editor, so it consumes the widget-level `MultiClick`, not the editor-internal classifier), Enter-to-add, toggle, filter, clear, "N left" as an aria-live `Status` region (C5 Slice C). Retained-mode ECS pattern KEPT (§3.4). The Checkbox/TextInput/Button **bundles + `A11yContract` + APG keyboard** are agent-interface P1d; C8 + C4/C6 supply their **visible rendering + picking + visual state** read from `A11yToggled`. |
| **S2** | **Long list (scale-game)** | `ScrollArea` (C5 Slice A) + 1000 rows + `ContentVisibility::Auto` (landed) + keyboard scroll (C5) + focus-ring on the focused row (C6) | Settles C5's virtualization ceiling (§3.2). `Pointer<Scroll>` → clamped `ScrollOffset` (C3 entry / C5 routing, umbrella §6.3). Roving-tabindex over rows (C5 Slice C). |
| **S3** | **Overlay / menu** | agent-interface **Tooltip-trigger** (P1d) + this campaign's `MenuButton`/`Menu`/`MenuItem` + `Popover` positioning + light-dismiss (C5 Slice B) + pick-depth == paint-order (C3, umbrella §6.1) | Overlay-aware picking: a menu painted above a button is hit FIRST (pick-order == paint-order). Light-dismiss on outside-click. Tooltip on hover/focus, WCAG 1.4.13 dismissable — the agent-interface Tooltip-trigger advertises `{ShowTooltip, HideTooltip}`; C5 supplies the positioning + show/hide timing geometry. |
| **S4** | **Modal + focus-trap** | agent-interface **Dialog** (P1d: modal `A11yModal` + `labelled_by`/`described_by` + the focus-trap/Esc/restore overlay state machine) + `::backdrop` + scoped `compute_next_focus` focus-trap (C5 Slice C) + `Inert` on the background (C5) + focus restoration (C5) + true top-layer (C5/render) | WCAG 2.1.2 no-keyboard-trap PASS *and* 2.4.3 focus-order trapped-to-dialog. Tab cycles only inside the dialog; Escape closes and restores focus to the invoker; background is `Inert` (pruned from focus walk + a11y tree + hit-test, umbrella §6.4). The Dialog **bundle + modal/labelling a11y** is agent-interface P1d; C5 supplies the focus-trap **traversal geometry** + `Inert` container; C8 composes the scene + backdrop render. |
| **S5** | **F-tier look showcase** | **Switch** + **Slider** + **Disclosure** (agent-interface P1d bundles, styled here) + `BoxShadow` (multi, spread) + per-side `Border` + `border-radius` + `Outline` + the `:focus-visible` ring (C6) + `opacity` group + forced-colors-safe tokens (umbrella §6.8) | Cards with elevation, real bordered buttons/inputs/switches/sliders, visible focus rings (WCAG 2.4.7 — the audit's "structurally invisible focus" fixed and *proven* here). Forced-colors variant proves no shadow-only affordance (gate #11b). The Switch/Slider/Disclosure **bundles** are agent-interface P1d; S5 is where this campaign's C6 styling makes them **look like** F-tier widgets. |

S1 (TodoMVC) is **one screen**, not the whole gallery — honoring umbrella §2.1's "TodoMVC subsumed as one screen." S2 is the scale-game. S3+S4 are where the next real bugs hide (overlay pick-depth, focus-trap), which is the campaign's stated gap-finding purpose (umbrella §2.1).

### 2.2 Authored as fixtures — coverage by construction, with a screen-scale matrix

Each screen is registered as a `buiy_verify` fixture via the `fixture!` macro (`coverage/fixture.rs:80`), so it auto-enrolls across the structured tiers (layout snapshot, display-list snapshot, invariant scenes) and — selectively — the GPU golden tier. The decisive coverage property (`coverage/mod.rs:5`): one fixture file enrolls across every tier with zero edits to a central list.

**But screens are large composites, and the matrix multiplies.** The CI matrix is `cells_per_fixture = 24` (`matrix.rs:83`, 2 themes × 3 viewports × 2 forced-colors × 2 dpr) with a hard `CELL_CEILING_PER_FIXTURE = 32` (`matrix.rs:23`). Every fixture enrolls into all 24 cells in the CPU snapshot tiers *and* the GPU golden tier (`coverage_golden.rs`). A 1000-row scale-game screen captured at 24 GPU goldens × 5 screens is both a golden-corpus explosion and a per-cell capture-time problem. This is a real architectural tension C8 must resolve, not paper over (§3.3):

- **Per-widget leaf fixtures** (the small `button/resting.rs`-shaped rows) stay on `Matrix::ci_default()` (24 cells) — they are cheap and that is what the matrix was sized for. C4/C6 author these for *their* widgets; C8 does **not** duplicate them.
- **Screen fixtures** (S1–S5, large composites) enroll through a **dedicated reduced matrix** `Matrix::gallery_screen()` — `1 theme (light) × 1 viewport (desktop) × 1 dpr (X1) × 2 forced_colors = 2 cells/screen` for the CPU structured tiers, and the **single canonical golden cell** (light/desktop/X1/forced=false, the `verification.md` gate #2 "single canonical CI GPU class") for the GPU tier. This keeps the structured (cheap, byte-stable, headless) coverage broad while keeping the expensive GPU golden corpus to one capture per screen. The reduced matrix is a *new constructor on the existing `Matrix` type* (no new machinery), gated by its own `cells_per_fixture <= CELL_CEILING_PER_FIXTURE` self-test.

This split is the "lowest tier that can observe the bug" rule (project `CLAUDE.md`, `using-buiy-verification`) applied at the screen scale: a screen's *layout* and *display-list membership* (does the menu paint above the button? does the modal backdrop cover the page? is the focus ring in the extract output?) are observable in the **headless structured tiers** at 2 cells; only the F-tier rasterization residue (shadow blur kernel, AA) needs the GPU golden, and one canonical cell suffices for that.

### 2.3 Screen fixture shape (Rust sketch)

```rust
// examples/buiy_gallery/src/screens/todomvc.rs  — the screen, authored once.
pub fn screen_todomvc(seeds: &[(&str, bool)]) -> impl Scene {
    bsn! {
        #TodoCard
        Node template_value(Display::flex_column())
        // F-tier look: card elevation + radius (C6 feeds these through extract).
        BoxShadow [ Shadow { blur: 8.0, color: token("color.shadow.card"), .. } ]
        Border { radius: { Corners::all(Radius::circular(6.0)) }, .. }
        Background { color: { token("color.surface.primary") } }
        Children [
            (#Header text_input_single_line("What needs to be done?")),   // agent-interface TextInput bundle, styled by C6
            (#List todo_list(seeds)),                                     // rows: agent-interface Checkbox bundle + label + Button(destroy)
            (#Footer todo_footer()),                                      // Status live-region + filter Radio group (C4/C5)
        ]
    }
}

// crates/buiy_verify/fixtures/gallery/todomvc.rs — the fixture wrapper (composition only).
crate::fixture! {
    name  = "gallery-todomvc",
    state = "resting",
    spawn = |app: &mut App| {
        app.world_mut().spawn(Camera2d);
        app.world_mut().spawn_scene(buiy_gallery::screen_todomvc(buiy_gallery::DEMO_SEEDS));
    },
}
```

The fixture body is **only** `Camera2d` + `spawn_scene` — every widget bundle comes from the **agent-interface P1d widgets** (Checkbox/Switch/Slider/Disclosure/Dialog/Tooltip/TextInput), every state component from the agent-interface `a11y/states.rs` substrate, and every style + container + picking layer from the sibling-owned (C4–C6) scene-fns. The fixture is the thinnest possible composition shell; this is what makes C8 "pure composition" — it neither defines a widget bundle nor an a11y-state component, it *arranges and renders* them.

### 2.4 Interaction-state fixtures (the state axis)

The fixture model encodes interaction state *per fixture* (one file per state — `coverage/fixture.rs:31`): the widget is spawned already in that state. For the gallery, the load-bearing states are the ones C7's Tier-A real-input harness *drives* and the structured tiers *observe statically*:

- **S3 menu-open** (`gallery-menu/open`) vs **menu-closed** (`/resting`) — proves the overlay enters the display-list / top-layer when open and is absent when closed.
- **S4 modal-open** (`gallery-modal/open`) — proves the backdrop + `Inert` background + trapped focus scope; a `gallery-modal/resting` (closed) proves the inverse.
- **S5 focus-visible** (`gallery-fields/focus-visible`) — proves the focus ring is in the extract output (the audit's WCAG 2.4.7 fix, made observable as a display-list-membership assertion, not a golden).

Spawning "already in state" is the static observation; C7's Tier-A harness additionally *drives the transition* with synthetic `PointerInput` and asserts the state-flip + observer capture (umbrella §6, C7 Tier-A). The two are complementary: the fixture is the snapshot, C7 is the live-input gate.

### 2.5 The app-author guide

A new doc `examples/buiy_gallery/AUTHORING.md` (and a condensed mirror in the crate root doc-comment) collects the load-bearing rules currently scattered as prototype comments (audit §6.9). The guide is **descriptive of the composed contracts**, not a new design — each rule cites the sibling/foundation that owns it:

> **Scope boundary — design-system / token spec is OUT of this campaign (audit W21).** Audit item W21 has two halves. The **first half** — a full design-system / token spec (semantic token tiers, the literal-value-vs-token rule, dark-mode and forced-colors token *variants*, the system-color map *contents*) — is **deliberately out of campaign scope**, deferred to a future `buiy-theme-tokens-design`. This campaign ships **only** the minimal 16-key forced-colors *stub* (umbrella §6.8, wired by C6; the full map remains `buiy-theme-tokens-design`'s), and the gallery's `token("…")` calls (§2.3) resolve against whatever token surface exists on `main` — C8 authors no new token tiers. The **second half** — the app-author guide (this `AUTHORING.md`) — **stays in C8** and is delivered here. Recording this so the design-system spec is not silently assumed delivered by the gallery.

1. **System ordering after `BuiySet::Input`.** App logic reading `OnPress`/`ValueChange`/`Set` (activation via `OnPress` + the agent-interface router — **no competing `Activate`**, umbrella §2.7; value/selection via the C4 vocabulary, umbrella §6.9) must run `.after(BuiySet::Input)` so events emitted in the input phase are readable the same frame (prototype lib.rs:602; C3 owns `BuiySet::Input`, and the agent-interface `route_action_requests` runs first-in-`Input`, phasing.md P1c).
2. **Editor-settle.** A `TextInput` must settle (async font shaping complete) before programmatic value reads/sets are reliable; C2's preedit-aware fix (umbrella §2.6) removes the *clobber*, but the guide documents the settle frame and the programmatic-seed path. **Note (agent-interface):** there is **no** `EditCommand::SetValue` variant — the agent-interface TextInput contract lowers a `SetValue` *action* via the existing `EditCommand::SelectAll` then `EditCommand::Insert(s)` (widget-contracts.md §5 TextInput); the guide documents the seed as that SelectAll+Insert path, not a new editor command. (The only new `EditCommand` the agent-interface campaign adds is `SetSelection { anchor, focus }`, for absolute-range selection — phasing.md P1c.)
3. **Box-sizing.** `border_box()` includes padding in the declared width (prototype lib.rs:389); the guide states the content-width math the gallery relies on (card 408 − 2×24 padding = 360 content).
4. **Despawn semantics.** Despawning a row (`commands.entity(row).despawn()`, prototype lib.rs:506) recursively removes children, the a11y subtree node, and the focus-tree entry; the guide documents that focus must be restored if the despawned entity held focus (C5 focus restoration).
5. **Message timing.** `chain()`-ing the logic systems (prototype lib.rs:603–617) makes a single `update()` settle one interaction deterministically; the guide states this is the recommended pattern for retained-mode apps and why (no signals layer — a committed v1 non-goal, audit §5).
6. **State authoring without panics.** The agent-interface Checkbox bundle carries a tri-state `A11yToggled` (default `False`, absence-friendly) — author it via the bundle, never bundle a duplicate state component (the prototype's `A11yToggled` post-spawn-insert dance, lib.rs:247–249, is the anti-pattern the guide warns against). The gallery reads `A11yToggled`/`A11ySelected`/`A11yExpanded` (agent-interface `a11y/states.rs`) for **visual** state; it never defines a competing `Checked`/`Pressed` component (umbrella §2.7 supersedes the original §2.3 separate-component plan).

---

## 3. Decisions & rejected alternatives

### 3.1 Screen list = TodoMVC + long-list + overlay/menu + modal + F-tier-look (five screens)

**Decided.** The five screens in §2.1 each demonstrate a distinct campaign capability with minimal overlap, and together cover every sibling's surface: C3 (pick-depth via S3 overlay), C4 (widgets via S1), C5 (scroll S2, overlay S3, modal/focus-trap S4), C6 (F-tier S5 + card/ring across all). TodoMVC is one screen per umbrella §2.1.

**Rejected — "one mega gallery screen with everything on it":** a single screen showing all widgets at once (the sickle-ui / iced `tour` example pattern). It reads well as a demo but is a poor *fixture*: every widget's state change re-blesses the whole screen's snapshot, the layout dump is enormous and unreviewable, and you cannot enroll a "menu-open" state without dragging the entire scene through it. Rejected for verification ergonomics — the per-screen split keeps each fixture's blast radius small.

**Rejected — "faithful flat TodoMVC only" (the literal prototype):** umbrella §2.1 already rejected this at the campaign level as under-ambitious for a kickoff; C8 inherits that. TodoMVC stays as S1.

**Deferred (not rejected) — a `Tabs`/`Combobox` screen:** the foundation catalogs these at F (`media-and-widgets.md:59–62, :74`), but the v1 widget set the gallery can compose is the agent-interface **P1d** bundles (Checkbox/Switch/Slider/Disclosure/Dialog/Tooltip/TextInput) plus this campaign's C4 additions (Button(+toggle)/Radio/RadioGroup + one selection widget). Tabs/Combobox are in **neither** P1d nor C4's v1 set, so a screen demonstrating them would have nothing to compose. Deferred to the post-campaign per-widget catalog children (umbrella §3 roadmap) / a future agent-interface phase; the gallery crate is structured so adding a screen is one new `screen_*` fn + one fixture file. (Note: `Slider` **is** an agent-interface P1d bundle, so it is **not** deferred — it appears on S5.)

### 3.2 Long-list virtualization = lean on the LANDED `ContentVisibility::Auto`; no recycling list now

**Decided.** S2's 1000-row screen uses one entity per row (the retained-mode model, audit §5) and relies on `ContentVisibility::Auto`, which is **already landed on main**: the off-screen skip is built (`render/components.rs:351` `OffscreenAuto` marker, layout-written/render-read; `follow-ups.md:543` "content-visibility: auto off-screen skip — LANDED"; `layout/translate.rs:484` off-screen sentinel sizing). An off-screen row's subtree is pruned from layout intrinsic-sizing and from `painters_z`/extract (`render/components.rs:442`, `SkipReason::OffscreenAuto`), so 1000 rows cost 1000 entities but only the on-screen window costs paint + shaping.

This is the umbrella §7 / §10 "virtualization ceiling" question, and C8 is where it is settled (umbrella §5 Wave 5: "its long-list screen settles C5's virtualization-ceiling question — a 1000-row scale-game fixture"). The scale-game (`scale-game` skill) at 1000 rows is the test: it proves the architecture holds at 1000× the TodoMVC scale *without* a new recycling abstraction, because the off-screen skip already exists.

**Rejected — a recycling virtual-list (DOM-recycling / windowed list) now:** the standard high-scale list pattern (recycle a fixed pool of N visible entities, re-bind data on scroll). It is genuinely better at the 100k-row scale, but: (a) it is **unbuilt** and would be a large new abstraction (entity pool, data-binding, scroll-position→data-index mapping) that no sibling owns — building it in C8 violates "pure composition, defines no primitive"; (b) the off-screen skip *already* removes the paint/shape cost, so the remaining cost at 1000 rows is N entities + N layout nodes, which is fine at TodoMVC-to-moderate scale (audit §5 "fine at TodoMVC scale"); (c) the foundation catalogs Scrollbar/overflow-scroll at C-tier and a windowed list is not a committed F-tier primitive. **The honest posture, stated in the guide: one-entity-per-row + `ContentVisibility::Auto` is the v1 model; a recycling list is a named future capability for data-heavy apps (≥~10k rows), owned by a future list/virtualization spec, not this campaign.** S2 documents the measured cost (entities/layout-nodes/frame-time at 1000 rows) so the future spec has a baseline (audit §8 item 22, the performance/reflow baseline). I coordinate with C5: C5's "virtualization posture" (umbrella §4 C5 Slice A) records the same call; C8 is the empirical settling fixture, C5 is the container primitive that exposes `ContentVisibility::Auto` on `ScrollArea` children.

**Runner-up note (coordination):** if C5's implementation reveals `ContentVisibility::Auto` does *not* compose cleanly with a clipping `ScrollArea` (e.g. the off-screen predicate uses the wrong viewport rect under scroll), S2 becomes the RED fixture that surfaces it, and the fix lands in C5 — not a recycling list in C8. This keeps the decision falsifiable.

### 3.3 Screen-fixture matrix = a dedicated reduced `Matrix::gallery_screen()`; one canonical golden per screen

**Decided.** §2.2's split: leaf-widget fixtures on `Matrix::ci_default()` (24 cells); screen fixtures on a reduced `Matrix::gallery_screen()` (2 structured cells: light/desktop/X1 × {forced=false, forced=true}) plus exactly one canonical GPU golden cell per screen. The reduced matrix is a new constructor on the existing `Matrix` type — no new tier, no new machinery — and carries its own `gallery_screen_cells_under_ceiling` self-test.

**Why:** the matrix's combinatorics were sized for *leaf* widgets (24 cells × ~dozens of leaf fixtures is the budget the `CELL_CEILING_PER_FIXTURE = 32` discipline protects, `matrix.rs:23`). Screen composites are 10–50× larger scenes; multiplying five of them by 24 GPU goldens is a corpus and capture-time explosion the ceiling discipline exists to prevent. The structured tiers (headless, byte-stable, CPU) are where screen-scale *structure* bugs live (paint membership, top-layer ordering, focus-ring presence, modal backdrop), and 2 cells (the forced-colors axis being the load-bearing one for a screen) covers them. The GPU golden is needed only for rasterization residue, and gate #2 already commits "a single canonical CI GPU class" (`verification.md` gate #2) — one cell per screen honors that.

**Rejected — enroll screens on the full `ci_default` 24-cell matrix:** uniform, no special case. Rejected because it blows the golden corpus (120 screen goldens), risks tripping the cell ceiling if any axis widens, and provides little marginal coverage (a screen's *layout* at phone vs tablet vs desktop is already a leaf-widget concern; the screen-level bugs are theme/forced-colors/top-layer, not viewport-pixel-exact). The reduced matrix is the `scale-game`/fuzz-budget discipline applied to combinatorics.

**Rejected — screens as plain integration tests, NOT fixtures:** author S1–S5 as hand-written `#[test]` integration tests (the prototype's approach). Rejected because it forfeits coverage-by-construction: each tier (layout, display-list, invariant, golden, forced-colors) would need hand-written per-screen test code, exactly the duplication the fixture model eliminates (`coverage/mod.rs:5`). The fixture path means adding a screen enrolls it everywhere with one file.

### 3.4 Retained-mode ECS app model = KEEP; fix the `apply_filter` Display-direction desync via a `Hidden` marker

**Decided.** S1 keeps the prototype's retained-mode ECS pattern (plain systems + change-detection, one-entity-per-row, `RowOf` back-refs, `chain()`-ed `TodoLogic.after(BuiySet::Input)`) — it is "correct, idiomatic, and the committed v1 design" (audit §5), independently validated by the `belly` prior-art. But two prototype warts are fixed in the gallery's authoring (composition-level, not a primitive):

- **`apply_filter` Display desync** (audit §5): the prototype rewrites `Display` directly (lib.rs:556–572), but `Display` and `FlexParams.direction` are decoupled components, so toggling `Display::None`↔`Display::flex_row()` *also* rewrites direction. The gallery uses **C5's `Hidden` marker** (scroll-overlay-modal §A.7 / §3.3): a marker that overrides the *resolved* Taffy display to `None` **without mutating** the author's `Display` + `FlexParams`, and prunes the filtered row from the a11y tree (audit §8 item 15). C5 explicitly **rejects** `CssVisibility::Hidden`/`ContentVisibility::Hidden` for the filter case (scroll-overlay-modal §3.3 "Rejected — `CssVisibility::Hidden`"): those keep the layout box **and** a11y presence, which is wrong for a filtered-out row — a hidden todo must collapse its box and leave the a11y tree, not occupy space invisibly. The marker (component + style-sync override + a11y prune) is **owned by C5**, not introduced here — C8 composes it.
- **The denormalized child cache** (`TodoRow.label`/`.checkbox`, lib.rs:122–127) is dropped in favor of a `Children` walk + marker queries, since the audit notes it "denormalizes what `Children` already encodes" (audit §5). Minor; keeps the example clean.

**Rejected — introduce a reactivity/signals layer for the gallery:** would make the view-sync ergonomic, but signals are an explicit foundation v1 non-goal (audit §5) and adding one in an *example* would misrepresent the framework's model. Rejected.

### 3.5 Gallery crate placement = a new `examples/buiy_gallery/` crate; fixtures in `buiy_verify/fixtures/gallery/`

**Decided.** The screens live in a new `examples/buiy_gallery/` workspace crate (a `lib.rs` exporting `screen_*` fns + a `main.rs` booting the screen-switcher, mirroring `hello_bsn`'s lib+main split). The fixture wrappers live in `crates/buiy_verify/fixtures/gallery/<screen>.rs` and are declared in `coverage/mod.rs` via `#[path]` (the existing pattern, `coverage/mod.rs:35`). `buiy_verify` takes a `dev-dependency` on `buiy_gallery` so fixtures can call `buiy_gallery::screen_*`.

**Rejected — screens inline in `buiy_verify`:** keeps everything in one crate, but then the gallery is not a runnable example (no binary), and the campaign's exemplar should be *runnable* (`cargo run -p buiy_gallery`) as a visual smoke test, mirroring `hello_button`/`hello_text`/`hello_bsn` (project `CLAUDE.md` § Build & Test). The lib+main+fixture split is the established `hello_bsn` pattern. Rejected.

**Dependency-cycle check:** `buiy_verify` already depends on `buiy_widgets`/`buiy_core`; `buiy_gallery` depends on `buiy` (the umbrella crate) + siblings; `buiy_verify` adds a *dev-dependency* on `buiy_gallery`. No cycle (`buiy_gallery` does not depend on `buiy_verify`). Confirmed against the current crate graph (`buiy_verify` is a leaf consumer).

---

## 4. Contracts & interfaces

### 4.1 Shared contracts consumed (referenced, not redefined — per umbrella §6)

C8 is a pure consumer of every shared contract; it defines none. It relies on:

- **Pick-depth from `painters_z`** (umbrella §6.1, owner C3): S3's overlay-aware picking and S4's modal interception assume pick-order == paint-order. C8 *consumes* this — its menu-open / modal-open fixtures are the composition that *exercises* it; the depth derivation is C3's.
- **Coordinate space** (umbrella §6.2, owner C1): every screen's picking, clip (S2 scroll, S4 backdrop), and overlay positioning (S3) ride C1's non-optional `GlobalTransform` routing. C8 authors no coordinate math.
- **`Pointer<Scroll>`** (umbrella §6.3): C3 owns the event entry, C5 owns nearest-container routing + clamp; S2 *composes* a `ScrollArea` and asserts the clamped offset, consuming both.
- **Focus** (umbrella §6.4): S4's focus-trap consumes C5's scoped `compute_next_focus` + `Inert`; S5's ring consumes C3/C5's `FocusVisible` signal + C6's ring paint. C8 authors the *scene* that has a focus scope; the scope machinery is C5's.
- **A11y wire format + the decomposed a11y-state components** (umbrella §6.5 + **§2.7**, owner = the **agent-interface campaign**): the AccessKit-tree snapshots the gallery fixtures produce (gate #3) read the agent-interface tri-state `A11yToggled`/`A11ySelected`/`A11yExpanded`/`A11yValue`/`A11yTextValue` fields (`a11y/states.rs`) lowered through the agent-interface derive fold + serialization discipline. C8 adds no wire fields and defines no a11y-state component; it reads them for **visual** state. Gate #3 itself is an agent-interface gate (its `accesskit_consumer` in-process tier) — C8 supplies the *scene* it snapshots, not the snapshot machinery.
- **Action routing + activation** (umbrella §2.7 + §6.9, owner = the **agent-interface campaign** for the inbound router; C3 for the pointer-event entry): S1's row activation flows through the existing `OnPress` and the agent-interface `route_action_requests` (Action::Click → OnPress/Focus/EditCommand) — **there is no competing `Activate` event**. App logic reads `ValueChange<T>`/`Set<X>` (C4 vocabulary) for value/selection deltas; the guide §2.5(1) documents the `.after(BuiySet::Input)` ordering this requires (and the agent-interface router's explicit intra-`BuiySet::Input` ordering, phasing.md P1c).
- **Forced-colors 16-key stub** (umbrella §6.8, owner C6): S5's forced-colors variant + every screen's `forced=true` structured cell resolve against C6's minimal system-color stub. C8 authors no system-color map.

### 4.2 Contracts C8 owns (defined here, precisely)

These are composition-level artifacts, owned by C8:

- **`buiy_gallery` crate surface:** `pub fn screen_todomvc(seeds) -> impl Scene`, `screen_long_list(n: usize) -> impl Scene`, `screen_overlay_menu() -> impl Scene`, `screen_modal() -> impl Scene`, `screen_styling_showcase() -> impl Scene`, plus `pub const DEMO_SEEDS`. Each is a pure `Scene` factory (no `Camera2d`, no plugins) so both the binary (`spawn_scene`) and fixtures (`world.spawn_scene`) reuse the exact tree.
- **`Matrix::gallery_screen()`** — the reduced screen matrix (§3.3): `{Light} × {desktop} × {false, true} × {X1}` = 2 cells, with a `gallery_screen_cells_under_ceiling` self-test. (A new constructor on the *existing* `matrix.rs` type — the only `buiy_verify` source edit C8 makes; it is additive and owned by C8 because it is a gallery-scale concern, but it touches a C7-owned file, so it lands coordinated with C7 per §5.)
- **The screen fixture files** `crates/buiy_verify/fixtures/gallery/{todomvc,long-list,overlay-menu,modal,styling}.rs` + their `#[path]` declarations in `coverage/mod.rs`, each a thin `Camera2d + spawn_scene` shell (§2.3).
- **`examples/buiy_gallery/AUTHORING.md`** — the app-author guide (§2.5), the canonical home for the six load-bearing rules.

### 4.3 The TodoMVC-as-a-screen contract (the literal exemplar mapping)

For traceability, S1 maps the prototype's nine systems (prototype lib.rs:603–615) onto the re-derived event vocabulary so the audit's "what to KEEP vs fix" is explicit:

| Prototype system | Gallery disposition |
|---|---|
| `add_todo_on_submit` (reads `EditSubmitted`) | KEEP; reads the submit event; clears the field via `EditCommand::SelectAll` + `EditCommand::Insert("")` (the agent-interface SetValue-text lowering — **no `EditCommand::SetValue` variant**, widget-contracts.md §5) over the C2 preedit-aware path. |
| `toggle_todo` (reads `Toggled` + `A11yToggled`) | FIX (agent-interface): reads the agent-interface **tri-state `A11yToggled`** (`a11y/states.rs`) — the same component the Checkbox bundle carries and that `Action::Click`/Space mutate via the contract `honor`. C8 defines **no** parallel `Checked`/`ToggleState` component (umbrella §2.7 supersedes §2.3's separate-component plan). The app system maps `A11yToggled` → `bool` (`True` ⟹ done; the gallery's Checkbox is binary here, so `Mixed` is unused on a row) when writing `TodoRow.completed`. |
| `toggle_all` | KEEP (advances each row's Checkbox `A11yToggled` to `True` — via the bundle's state, not a hand-inserted component). |
| `destroy_todo`, `clear_completed` | KEEP; documents focus-restoration-on-despawn (guide §2.5(4)). |
| `set_filter` | KEEP; filter buttons become a `Radio`/toggle group (C4) emitting `Set<FilterMode>`. |
| `restyle_completed` | KEEP (strike-through + dim on `Changed<TodoRow>`). |
| `apply_filter` | FIX: C5's `Hidden` marker (resolved-display override + a11y prune, author `Display`/`FlexParams` untouched), not direct `Display` rewrite — **not** `CssVisibility/ContentVisibility::Hidden`, which C5 rejects for this case (§3.4, scroll-overlay-modal §3.3). |
| `update_count` | FIX: write into an aria-live `Status` region (C5 Slice C announcer), not a plain `Text` — the audit's "N items left is not a live region" miss (audit §5). |

---

## 5. Migration / build steps (ordered; blast radius named)

C8 is **Wave 5** (umbrella §5) — it lands last, after C3–C7 **and the agent-interface P1d widget bundles** are live (umbrella §2.7/§8). Every step is composition; no sibling primitive and no widget bundle / a11y-state component is created here. Per umbrella §8 the plan's first step is the rebase + re-confirm of file:line anchors against the then-current `origin/main` **and a re-confirm that the agent-interface P1a (states) / P1c (router+driver) / P1d (widgets) have landed to the level S1–S5 compose**.

1. **Scaffold `examples/buiy_gallery/`** (lib.rs + main.rs + Cargo.toml). Blast radius: new crate, workspace `Cargo.toml` member add. Mirrors `examples/hello_bsn/`.
2. **Author S5 (F-tier showcase) first** — it has the fewest behavioral dependencies (paint only, no scroll/overlay/modal), so it is the first screen that can be authored once C6 lands and the agent-interface **Switch/Slider/Disclosure** P1d bundles exist. Add its fixture + `#[path]`. RED until C6 feeds shadow/border/outline through extract (the display-list-membership assert is the RED-first gate, §6).
3. **Author S1 (TodoMVC)** once the agent-interface **Checkbox/TextInput** P1d bundles + C4's value/selection vocabulary + C2's editor fix are live. Add fixture + the AUTHORING.md guide (the guide is written *with* S1, since S1 is where every rule first applies). Blast radius: the largest screen; its layout/display-list snapshots are new `.snap`s (no re-bless of existing fixtures — additive).
4. **Author S2 (long-list scale-game)** once C5 Slice A (`ScrollArea` + `Pointer<Scroll>` routing) is live. Add fixture. Capture the 1000-row performance baseline (entities, layout-nodes, frame-time) into AUTHORING.md (audit §8 item 22), **and run down the audit's "~50% idle CPU" miss as part of that baseline** — reshape thrash vs continuous redraw vs other (§7 perf-baseline deliverables). Blast radius: the scale-game; verifies `ContentVisibility::Auto` composes with `ScrollArea` (the §3.2 falsifiable check).
5. **Author S3 (overlay/menu)** once C5 Slice B (`Menu`/`Popover` positioning + light-dismiss) + the agent-interface **Tooltip-trigger** P1d bundle + C3 pick-depth are live. Add `menu-open` + `menu-closed` state fixtures. Blast radius: two fixtures; the display-list-membership assert proves overlay-above-button paint order.
6. **Author S4 (modal/focus-trap)** once C5 Slice C (scoped focus + `Inert`) + the agent-interface **Dialog** P1d bundle (modal a11y + Esc/restore state machine) are live. Add `modal-open` + `modal-closed` fixtures. Blast radius: two fixtures; the C7 Tier-A harness + the agent-interface input-replay seam drive the focus-trap (no-keyboard-trap, WCAG 2.1.2) live — C8 supplies the scene + backdrop render, the agent-interface Dialog the modal a11y, C7/agent-interface the gate.
7. **Add `Matrix::gallery_screen()`** + its self-test to `matrix.rs` (coordinated with C7, umbrella §6.5/§7 — it is a `buiy_verify` source edit). Re-point the screen fixtures' enrollment at it. Blast radius: one new constructor + one self-test in a C7-owned file; the leaf-widget enrollment is untouched.
8. **Bless the five canonical screen goldens** on the GPU host (`BUIY_BLESS=1 … coverage_golden -- --ignored`), one cell per screen. Blast radius: 5 new golden PNGs (the residue tier); reviewed per the human-curated `--accept` workflow (`verification.md` gate #2). Headless gate stays green without them.
9. **Wire `cargo run -p buiy_gallery`** as the documented visual smoke test; add it to project `CLAUDE.md` § Build & Test alongside `hello_button`/`hello_text`/`hello_bsn`. Flip this child's status `[draft]`→`[active]` and the umbrella's C8 row.

**Snapshots/goldens touched:** all *additive* (new screen `.snap`s + 5 new goldens). No existing leaf-widget snapshot or golden re-blesses, because the gallery enrolls through a *separate* fixture set on a *separate* matrix constructor — this is the central reason for the §3.3 split (avoid re-blessing the whole corpus when a screen changes).

---

## 6. Verification (how C7 gates this; RED-first)

C8 is verified *by* the very fixtures it authors — that is the coverage-by-construction property. The gates, and what must be proven RED-first:

- **Tier 1 (layout snapshot, `coverage_layout.rs`):** each screen's resolved-layout dump on `Matrix::gallery_screen()`. Proves the card/list/footer/modal geometry. RED-first: before C6/C5 land, the screen fixture either fails to compile (missing scene-fn) or its layout dump lacks the expected node — the snapshot is written only once the screen composes.
- **Tier 2 (display-list snapshot, `coverage_display_list.rs`):** the **load-bearing gate for this child**. It observes *paint membership and order* headlessly:
  - **S5 focus-visible:** the `Outline`/ring instance MUST be in the extract output — the audit's "focus structurally invisible" (WCAG 2.4.7) fixed and *proven without a GPU*. **Proven RED-first:** author the `gallery-fields/focus-visible` fixture and assert the ring is in the display-list *before* C6 feeds `Outline` through extract — it MUST be absent (RED), then present (GREEN) once C6 lands. This is the anti-vacuous-green discipline (umbrella §9.5).
  - **S3 menu-open:** the menu's instances paint *after* (above) the button's — pick-order == paint-order observable as display-list order.
  - **S4 modal-open:** the backdrop instance covers the page; the `Inert` background is present-but-non-interactive.
- **Tier 3 (invariant, `coverage_invariants.rs`):** the screens flow through the same invariant predicates (e.g. every focusable node has an accessible name; no orphans) as generated scenes.
- **Gate #3 (AccessKit tree — an *agent-interface* gate over its `accesskit_consumer` in-process tier):** each screen's tree snapshot — S1 proves the agent-interface **Checkbox** lowers tri-state `A11yToggled`→`aria-checked` (not `aria-pressed`, role-disambiguated by the agent-interface derive fold), the agent-interface **TextInput** announces as `Role::TextInput` not static `Text` (audit §3 the live a11y bug, fixed by the agent-interface P1d TextInput off the `A11yRole::Text` stopgap); S4 proves the `Inert` background is pruned from the tree (umbrella §10 deferred Q — C8's modal fixture is the **observation point**; the prune mechanism + the gate are owned by C5/agent-interface, see §7). The gate machinery is the agent-interface campaign's; **C8 supplies the composed scene each snapshot is taken over** — it does not author the snapshot/serialization or the role-stringifier lockstep.
- **Gate #4 (announcement output — agent-interface gate):** S1's "N items left" `Status` live-region utterance order. The Announcer→accesskit mapping is C5 Slice C's / the agent-interface live-derivation; C8 composes the live region.
- **Gate #7 (APG keyboard — an *agent-interface* gate, driven via C7 Tier-A pointer input + the agent-interface input-replay seam):** S1 the agent-interface Checkbox = **Space-only** (not Enter — the load-bearing Button-vs-Checkbox asymmetry the agent-interface contract enforces, widget-contracts.md §5), S3 menu arrow-key nav, S4 Escape-closes + Tab-trapped, S5 Slider arrows/Home/End + Disclosure Enter/Space. The **per-widget APG keyboard contract + the #7 fixtures** are agent-interface P1d; C7's `PointerHarness` + synthetic keyboard plus the agent-interface input-replay seam drive them; **C8 supplies the composed multi-widget scene** these contracts are exercised in.
- **Gate #10 (hit-target ≥24×24):** every interactive widget in every screen (`verification.md` gate #10) — the geometric check runs over the gallery fixtures automatically.
- **Gate #11 (forced-colors):** the `forced=true` structured cell of each screen feeds `live_catalog_paint` (`coverage/forced_colors.rs`) — proves (a) no non-system color and (b) no shadow-only affordance (S5's focus ring/border is the non-shadow cue under forced-colors). The forced-colors *visual* residual stays GPU-blocked (`verification.md` §11 note) — C8 does not unblock it.
- **Tier 5 (golden, `coverage_golden.rs`, `#[ignore]` GPU):** one canonical cell per screen — the rasterization residue (shadow blur, AA, ring antialiasing). Blessed on the GPU host, reviewed per the `--accept` workflow.

**The RED-first discipline for C8 specifically:** because C8 lands last, its fixtures are written against sibling surfaces that *already exist* by Wave 5. But the focus-ring display-list assert (S5) is the one that must be demonstrated RED on a tree where the ring is NOT yet in extract — so the plan stages S5's *fixture + assert* in step 2 *before* asserting GREEN, proving the gate has teeth (it would catch a regression that drops the ring from extract). The audit's warning (§9.5, the existing `picking_backend.rs` hand-writes `ResolvedLayout` and is structurally blind) is the cautionary tale: the gallery's assertions read the *live extract output*, never a hand-built descriptor.

---

## 7. Open questions deferred + dependencies

**Resolved here** (see §3): screen list (§3.1); virtualization posture — lean on landed `ContentVisibility::Auto`, no recycling list (§3.2, settles umbrella §10's virtualization-ceiling Q for the gallery); screen-fixture matrix cost (§3.3); retained-mode KEEP + `apply_filter` fix (§3.4); crate placement (§3.5).

**Deferred (genuinely depends on un-built sibling work):**

- **Whether `ContentVisibility::Auto` composes with a clipping `ScrollArea`** — settled empirically by S2, but only once C5 Slice A's `ScrollArea` exists. If it does not compose, the fix is C5's, surfaced by S2 as the RED fixture (§3.2). Cannot be closed before C5.
- **Whether the inert/hidden background is pruned from the AccessKit tree** (umbrella §10): S4's gate-#3 snapshot is the observation point, but the prune mechanism is the **agent-interface campaign's** call — its `build_tree` PRUNE rule (`A11yHidden`/inert + descendants emit no node, semantic-tree.md / phasing.md P1b) plus C5's `Inert` traversal marker. C8 observes; the agent-interface campaign + C5 decide.
- **The Announcer→accesskit mapping** for S1's "N items left" `Status` region (umbrella §10): S1 consumes it; the live-region derivation (role-implied politeness/atomic) is the **agent-interface** derive fold's (semantic-tree.md / live-regions.md) and the announce surface is C5 Slice C's. C8's gate-#4 snapshot is blocked on both.
- **Per-fixture performance budget numbers** for gate #14 on the screens (`verification.md` gate #14, budgets are an open `buiy-verification-design` Q): S2 captures the 1000-row *baseline*, but the *budget* (±slack threshold) is owned by the verification spec, not C8.

**Perf-baseline deliverables (owned here — captured during the S2 1000-row scale-game, §3.2 / build-step 4, recorded in AUTHORING.md):**

- **Steady-state cost at 1000 rows:** entities, layout-nodes, and frame-time, so the future recycling-list spec inherits a baseline (audit §8 item 22).
- **The audit's "~50% idle CPU" investigation (audit §6, perf miss):** the prototype burned roughly half a core *at idle* (no input, no animation). C8 owns running this down during the scale-game and recording the cause — the leading hypotheses to discriminate are **(a) layout/reshape thrash** (a change-detection or dirty-flag false-positive re-running Taffy every frame), **(b) continuous redraw** (the app requesting a redraw every frame instead of on-demand / damage-driven), and **(c) some other per-frame scan** (e.g. an unconditional all-entities query). The deliverable is the measured cause + whether it is a C8-authoring fix (e.g. on-demand redraw in the gallery `main`), a sibling bug surfaced by S2 (filed against the owning child), or a framework-level redraw-scheduling question deferred with a named owner. This is explicitly **not** silently dropped: if S2 cannot isolate it within the scale-game, the open question is recorded with the narrowed-down candidate and its owner.

**Dependencies (hard ordering):** C8 is Wave 5, gated behind **all** of C3 (input + pick-depth), C4 (visual/picking widget layer + value/selection vocabulary), C5 (scroll/overlay/modal/focus), C6 (F-tier paint + ring), C7 (the Tier-A harness that drives the keyboard/pointer geometry gates and the display-list infrastructure), **and the agent-interface campaign's P1d** (the canonical Checkbox/Switch/Slider/Disclosure/Dialog/Tooltip/TextInput **bundles + `A11yContract` + APG keyboard** the gallery composes — without P1d there are no widgets to compose; with P1a/P1c there is no state to read or router to activate through). It defines nothing any of them need — the dependency is strictly one-way (umbrella §4 dependency column: C8 depends on C0, C3–C7; **§2.7** adds the agent-interface P1a/P1c/P1d substrate dependency).

---

## Coordination with the agent-interface campaign

This child is reconciled to umbrella **§2.7 + §8** (the user's "coordinate, don't cede" decision, 2026-06-22). The **agent-interface campaign** (`docs/specs/2026-06-18-buiy-agent-interface-design/`; P0 landed via PR #79, building P1a→P1d) OWNS the accessibility substrate **and the canonical APG widget bundles**; C8 is **pure composition** that arranges, styles, and renders those widgets into the exemplar screens. The gallery is the **meeting point** where the agent-interface widgets become a visible, runnable app.

**C8 consumes (does NOT define) — from the agent-interface campaign:**
- The **canonical P1d widget bundles** — Checkbox (tri-state), Switch, Slider, Disclosure/Accordion, Dialog (modal), Tooltip-trigger, TextInput — each with its **bundle + `A11yContract` (advertised verbs + `honor`) + APG keyboard contract** (widget-contracts.md §5, phasing.md P1d). S1 composes Checkbox/TextInput/Button; S3 the Tooltip-trigger; S4 the Dialog; S5 Switch/Slider/Disclosure. C8 **does not author a widget bundle**.
- The **decomposed a11y-state components** (`a11y/states.rs`): `A11yToggled` (tri-state incl. `Mixed`, role-disambiguated), `A11ySelected`, `A11yExpanded`, `A11yValue`, `A11yTextValue`, `A11yPlaceholder`, the marker set (`A11yDisabled`/`ReadOnly`/`Modal`/`Hidden`/…). C8 reads these for **visual** state (e.g. strike-through a done row off `A11yToggled`); it defines **no** competing `Checked`/`Pressed`/`ToggleState` component (this supersedes the original umbrella §2.3 separate-component plan — see umbrella §2.7).
- The **inbound action router** (`route_action_requests` over `bevy_winit ActionRequestWrapper`; `Action::Click`→`OnPress`/Focus/EditCommand) and **`EditCommand::SetSelection`**. S1 row activation flows through `OnPress` / this router — there is **no competing `Activate` event** authored here. The `SetValue`-text seed lowers via the **existing** `SelectAll`+`Insert` (no new `EditCommand::SetValue`).
- The **a11y gates #3 / #4 / #7** and their machinery — the `accesskit_consumer` in-process tier, the role/state/announcement snapshots, the APG input-replay. C8 **supplies the composed scene** each gate is exercised over; it does not author the snapshot/serialization, the role-stringifier lockstep, or the per-widget #3/#7 fixtures (those ship **with** the P1d widgets).

**C8 owns here (the layer the agent-interface widgets need but its campaign does not build):**
- The **exemplar screen set** (S1–S5) + the **`buiy_gallery` crate surface** (`screen_*` scene-fns + `DEMO_SEEDS`) — the runnable app and the fixture corpus that compose the P1d widgets under real scroll/overlay/modal/F-tier.
- The screens that **exercise this campaign's containers + styling**: C5 scroll/overlay/modal/focus-trap geometry, C6 F-tier styling + the focus-ring paint, and the **visible rendering + picking** of the P1d widgets (label, focus ring, `Pickable::IGNORE` pick-through). The agent-interface Dialog/Tooltip/Menu sit **inside** C5's containers; the agent-interface Checkbox/Switch/Slider are made to **look like** F-tier widgets by C6.
- The **`MultiClick` edit-in-place** reference (S1 double-click on a row → editor): C3's widget-agnostic `MultiClick` `EntityEvent` (input-event-model §2.11; no `Pointer<DoubleClick>` in bevy_picking 0.19). This is a **pointer gesture geometry** owned by C3, consumed at the gallery level — not the editor-internal click classifier.
- The **screen-scale verification artifacts**: `Matrix::gallery_screen()` (the reduced screen matrix), the screen fixture files, the **app-author guide** (`AUTHORING.md`), and the 1000-row **scale-game** perf baseline. These extend the project's verification rig and C7's tiers; they never stand up a parallel a11y gate.

**Removed / deferred because the agent-interface campaign owns it:**
- The original plan to read app-state from a `buiy_widgets`-owned `Checked(ToggleState)` component — replaced by reading the agent-interface `A11yToggled`.
- A gallery-authored `Activate` consumer — replaced by `OnPress` + the agent-interface router.
- An `EditCommand::SetValue` seed path — replaced by the agent-interface `SelectAll`+`Insert` lowering.
- Authoring any widget bundle, `A11yContract`, APG keyboard contract, or a11y gate (#3/#4/#7) — those land **with** the agent-interface P1d widgets; C8 only composes them.

**Meeting-point sequencing:** C8 (Wave 5) lands only after agent-interface **P1d** (the widget bundles) — confirmed at the rebase (build-step intro). If a P1d widget's visible rendering is incomplete when its screen is authored, C8 coordinates the render/picking/style addition per-widget (umbrella §2.7 "meeting point"), it does not fork the bundle.
