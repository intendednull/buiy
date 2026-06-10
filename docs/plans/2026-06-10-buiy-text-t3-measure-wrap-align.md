# Buiy Text T3: Measure + Wrap/Align Implementation Plan

**Date:** 2026-06-10
**Status:** landed
**Spec:** [specs/2026-06-09-buiy-text-rendering-design/measure-and-layout.md](../specs/2026-06-09-buiy-text-rendering-design/measure-and-layout.md) §§ 2–7 + [architecture.md](../specs/2026-06-09-buiy-text-rendering-design/architecture.md) §§ 1.2, 4.2–4.3, 5.1
**Campaign:** [plans/2026-06-09-buiy-text-campaign.md](2026-06-09-buiy-text-campaign.md) — phase T3 (depends on T2, landed @ `d7db654`; the implementer starts from a branch with T1+T2 merged)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make text content sizing real — the campaign's keystone phase. Land: the `LayoutTree` migration `TaffyTree<()>` → `TaffyTree<Entity>` with edge-triggered context registration; one shared `compute_roots_with_text_measure` helper replacing plain `compute_layout` at **all three** compute sites; the measure protocol (cached min/max-content intrinsics, definite-width relayout fold, ceil policy); intrinsic keywords (`Sizing::MinContent`/`MaxContent`) realized on text leaves; the F-tier carriers `LineHeight`/`WhiteSpace`/`TextWrap`/`TextAlign` with the § 5.1–5.3 value tables (balance/pretty/stable and justify-all warn-once degrades); `BuiyLayoutStep::TextCommit` (reshape at final content-box, align-at-commit, `ResolvedBaseline` + `ComputedTextLayout` idempotent writes); and the steady-state instruments `TextMeasureCallCount` / `TextCommitReshapeCount`.

**Architecture:** The measure closure rides Taffy's existing passes — it adds **zero** extra Taffy invocations, preserving the layout-architecture "never more than 2× per frame" ceiling verbatim (measure § 4.3); `LayoutTaffyComputeCount` semantics are unchanged. Text leaves register their `Entity` as the Taffy node context (`TaffyTree<Entity>`, measure § 2.1); the closure resolves the entity against an ECS query it captures, computes/serves cached intrinsics, relays the buffer out at the candidate width, and folds `layout_runs()` into a `Size<f32>`. `TextCommit` is the new **final** layout step (after `CqDescendantReRun`, architecture § 4.2): it reconciles every buffer to its final Taffy content-box, applies `Align` (a finalize concern, § 5.3), and writes the output components idempotently. All in-place `TextBuffer` mutation keeps T2's `bypass_change_detection` discipline (measure § 7).

**Where T3 ends (honesty pin):** layout knows how big text is and the buffer is left shaped at its final box — **nothing paints**. `extract_buiy_glyphs`, the atlas producer, `GlyphAlphaInstance` emission, and the `hello_text` example are **T4**. The § 5.4 `TextDirection` strong-mark prepend and the `FontStack` resolver are **T5**. `ResolvedBaseline` consumers (`vertical-align`, inline baseline alignment, AccessKit text geometry) are **C-tier seams named, not built** (measure § 5.5/§ 6) — T3 only writes the component. `overflow-wrap`, `text-indent`, `fit-content()` realization, and per-span metrics stay named deferrals (§ 5.5, decision 14). The `TextBufferAccess` QueryData is **deferred to the editing campaign** (decision 12 — supersedes T2's seam table; recorded as T3 erratum).

**Tech Stack:** cosmic-text 0.19 (dep since T1), taffy 0.10.1 (vendored; the measure API verified below), Bevy 0.18.1 ECS. **No new dependencies** — if any task appears to need one, STOP: that contradicts the charter.

**Test reality:** T3 is **headless-only** (campaign test surface: "headless layout snapshots"). Shaping with the embedded Fira Sans latin subset needs no adapter and no display. Zero new `#[ignore]` tests; the GPU lane is untouched.

---

## The gate (keep green at every commit)

**Gate per task:** headless only.

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  cargo test --workspace -j 2
```

(On this Linux host the pre-existing windowed tests need a display server: prefix the test step with `xvfb-run -a` exactly as CLAUDE.md's gate does. The new T3 tests themselves never need one.)

---

## The three compute sites (verified 2026-06-10 — the charter's line numbers drifted)

The campaign charter names `systems.rs:2625/3602/2876`. Current verified positions in
`crates/buiy_core/src/layout/systems.rs` (re-verify before editing; they drift):

| Site | System (fn at) | `tree.tree.compute_layout(` at | Resets `LayoutTaffyComputeCount`? |
|---|---|---|---|
| 1 | `taffy_compute` (`:2598`) | **`:2627`** | yes (frame start, `:2609`) |
| 2 | `cq_flip_rerun` (`:3424`) | **`:3604`** | no (increments only) |
| 3 | `cq_descendant_rerun` (`:2764`) | **`:2878`** | no (increments only) |

A fourth `compute_layout` at `systems.rs:788` is `resolve_column_widths`' **local throwaway**
`TaffyTree<()>` for table column resolution — it never touches `LayoutTree`, never measures
text, and is **deliberately untouched** by this plan (it stays `TaffyTree<()>`).

Sites 2 and 3 are mutually exclusive within one frame (`CqFlipReRanThisFrame`, the D4
cost-ceiling guard at `systems.rs:2728`), so the helper runs **at most twice per frame**.

## The locking discipline (the re-entrancy risk, encoded)

Measure must be callable from `cq_flip_rerun` and `cq_descendant_rerun`, not just
`taffy_compute` — the spec calls migrating only site 1 "a correctness bug, not an option"
(measure § 4.3: a cq flip that changes a text ancestor's width would re-lay-out the leaf with
the **zero** measure and collapse it mid-frame). Re-entrancy is safe only under this
discipline, which every task below preserves:

1. **One `SharedFontSystem::lock()` per helper invocation** (architecture § 1.2 lock site #1;
   measure § 3.4): the guard is taken inside `compute_roots_with_text_measure`'s body, held
   for the duration of the per-root `compute_layout_with_measure` loop, and dropped before
   return. It is **never** stored in a resource, a `Local`, or any state that outlives the
   call — each of the ≤ 2 measure-bearing compute calls per frame takes and releases the lock
   independently.
2. **The closure reborrows the held guard** (`&mut FontSystem`); it must **never** call
   `SharedFontSystem::lock()` itself — `std::sync::Mutex` is not reentrant, so a nested lock
   self-deadlocks the app on the first measured text node. This is the one way the cq
   re-entrancy can hang, and the code shape below makes it unrepresentable (the closure
   receives `&mut FontSystem`, not the resource).
3. **The closure is rebuilt per call from current world state, holds no cross-call state, and
   never issues `Commands`** (measure § 4.3). Buffer mutation is idempotent (`set_size` to
   the same width is a no-op relayout), so a flip frame's second compute re-measures only
   nodes the flip dirtied — Taffy's per-node cache covers the rest.
4. **`TextCommit` is lock site #2** and locks **lazily** — once per frame, only when at least
   one buffer actually reshapes. Steady frames never lock. Measure (inside `Layout` steps
   3/5/9) and commit (step 10) are sequential systems in one schedule — their lock scopes can
   never overlap.
5. Architecture § 1.2's ledger is exhaustive: T3 fills sites **#1 and #2**; site #3 (the
   render-world atlas-miss closure) is T4's. Reviewers reject a fourth lock site.

---

## Orientation: verified facts this plan builds on

cosmic-text facts source-verified against the vendored **0.19.0**; taffy against vendored
**0.10.1**; Bevy against **0.18.1**. Re-verify file/line refs before editing — they drift.

| Fact | Verified shape |
|---|---|
| `TaffyTree<NodeContext = ()>` | taffy_tree.rs:147 — the context is a generic; the migration is a type-parameter change, not a data-structure change |
| `compute_layout_with_measure` | taffy_tree.rs:905–921 — `(&mut self, NodeId, Size<AvailableSpace>, MeasureFunction) -> Result<(), TaffyError>` where `MeasureFunction: FnMut(Size<Option<f32>>, Size<AvailableSpace>, NodeId, Option<&mut NodeContext>, &Style) -> Size<f32>` |
| plain `compute_layout` | taffy_tree.rs:925–927 — literally `compute_layout_with_measure(node, avail, \|_,_,_,_,_\| Size::ZERO)`; today every text leaf measures **zero** |
| `new_leaf_with_context(Style, NodeContext)` | taffy_tree.rs:581 — sets `has_context = true`; no `mark_dirty` (the node is new) |
| `set_node_context(NodeId, Option<NodeContext>)` | taffy_tree.rs:642–657 — **calls `mark_dirty` internally** (:656) ⇒ edge-triggered registration only (measure § 2.2) or the O(0) steady state silently dies |
| `get_node_context(NodeId) -> Option<&NodeContext>` | taffy_tree.rs:659 — the test observable for registration |
| `TaffyTree::remove` | taffy_tree.rs:595–630 — does **not** clear `node_context_data`; stale entries are unreachable (`NodeData::new` sets `has_context = false` on slot reuse, dispatch checks the flag, :318–326) — upstream behavior, not a Buiy leak to fix |
| measure dispatch | taffy_tree.rs:318–326 — closure invoked only for **childless** nodes whose `has_context` is true, wrapped in `compute_cached_layout` (identical probes hit Taffy's per-node cache) |
| `compute_leaf_layout` | leaf.rs:111–146 — content-box inset (padding+border+scrollbar gutter) subtracted from available space before measure and added back after: the closure **never does BoxModel math**. The closure receives `known_dimensions` only under `RunMode::ComputeSize`; under `PerformLayout` it receives `Size::NONE` and the resolved width arrives as `AvailableSpace::Definite` (:131–139) — so the protocol's `known.or(…)` fold answers the Definite arm at layout time. `known_dimensions` overrides the measured size at :143–146 — the § 3.3 intrinsic-keyword-under-stretch fidelity limit |
| `Layout::content_box_size()` | layout.rs:310 — `size − padding − border` per axis. **Note:** it does *not* subtract `scrollbar_size`, while measure-side `content_box_inset` does include the gutter — a text leaf that is also a scroll container would commit slightly wider than it measured; named edge (text leaves are not scroll containers in v1), do not "fix" |
| `mark_dirty(NodeId)` | taffy_tree.rs:873 — recursive to **ancestors**, never descendants (a parent style change does not clear leaf caches; only changed probe inputs re-invoke measure) |
| `shape_until_scroll(&mut self, &mut FontSystem, prune: bool)` | buffer.rs:571 — with `height_opt = None`, `scroll_end = scroll_start + f32::INFINITY` (:609–612) ⇒ **all** lines shape and lay out (charter risk 2, pinned by test in Task 4) |
| `Buffer::resolve_dirty` external-invalidation branch | buffer.rs:426–434 — when buffer-level dirty flags are empty it still returns true if any line `needs_reshaping()`; `BufferLine::set_align` → `reset_layout` → `Cached::Unused` → `needs_reshaping` true (buffer_line.rs:148–156, 203–211, 239–241; cached.rs `is_invalidated`) ⇒ **align-only changes do relayout through a plain `shape_until_scroll`** — the enabler for align-at-commit |
| `BufferLine::set_align(Option<Align>) -> bool` | buffer_line.rs:148–156 — internally guarded, returns true only on change (the commit's change signal); getter `align()` :139 |
| `set_text(&mut self, &str, &Attrs, Shaping, Option<Align>)` | buffer.rs:934 — alignment `Some` sets every line; **`None` leaves reused lines' align untouched** (set_text_impl only calls `set_align` when `alignment.is_some()`) — the `Some→None` transition can only be owned by a pass that sets align on every line: `TextCommit` (decision 8) |
| `set_size(Option<f32>, Option<f32>)` | buffer.rs:818 — lazy (`RELAYOUT` flag only on actual change); width/height clamped `≥ 0` internally ⇒ the commit's compare target must pre-clamp (decision 7) |
| `Buffer::size() -> (Option<f32>, Option<f32>)` | buffer.rs:813 — `const fn`, the commit's cheap equality guard |
| `set_metrics(Metrics)` | buffer.rs:729 — **panics** on zero `font_size` *or* `line_height` ⇒ the `METRICS_FLOOR` clamp (decision 11) |
| `set_metrics_and_size` | buffer.rs:838 — exactly `set_metrics` + `set_size`; not needed (sync owns metrics, commit owns size) |
| `layout_runs(&self) -> LayoutRunIter` | buffer.rs:1134 — read-only; the iterator **terminates** (does not skip) at the first line whose shape or layout cache is not `Used` (buffer.rs:246–249, the `?`s) — why every consumer must run after a resolving shape pass |
| `LayoutRun` fields | buffer.rs:36–55 — `line_i/text/rtl/glyphs/decorations/line_y/line_top/line_height/line_w`; `line_y` = "Y offset to baseline of line" (the `ResolvedBaseline` source) |
| `Buffer.lines: Vec<BufferLine>` | buffer.rs:336 — pub; the commit's per-line align loop |
| `Wrap` / `Align` variants | layout.rs:128–137 `None\|Glyph\|Word\|WordOrGlyph`; :152–158 `Left\|Right\|Center\|Justified\|End` |
| `Metrics::new(font_size, line_height)` / `relative(font_size, scale)` | buffer.rs:302–315 |
| Bevy system-param cap | 16 params per system (the `cq_flip_rerun` doc comment, systems.rs:3419–3422); tuples of `SystemParam`s are themselves a single `SystemParam` — the grouping lever Tasks 3/5 use |
| Bevy auto sync points | commands issued by an earlier ordered system flush before the next ordered system (T2-established: `TextSync`'s deferred `TextBuffer` insert is visible to the same frame's `SyncStyles`) |

Codebase shapes consumed (read before editing, confirm current):

- `crates/buiy_core/src/layout/tree.rs` — `LayoutTree { tree: TaffyTree<()>, by_entity }`,
  `mark_dirty_for_entity` (no-op without a node), `#[doc(hidden)]` `by_entity()` / `tree_ref()`.
  **Gains the type flip + `set_text_context`/`clear_text_context`.**
- `crates/buiy_core/src/layout/systems.rs` — the three compute sites (table above);
  `translate_one_entity` (:2295–2363, the `new_leaf` arm at :2350); `NodeQueryItem` (:2263–2278,
  14 elements — **unchanged** by this plan, decision 1); `sync_styles` (:1954, 13 params);
  `write_resolved_layout`'s idempotent-insert guard (:2659–2693 — the discipline `TextCommit`
  copies); `LayoutTaffyComputeCount` (:109) / `SyncStylesIterCount` (:119) — the instrument
  precedents.
- `crates/buiy_core/src/layout/pipeline.rs` — `BuiyLayoutStep` (12 variants) + `configure_pipeline`.
  **Gains `TextCommit`** (the module doc already names it: "text T3 appends `TextCommit` as the
  new final step").
- `crates/buiy_core/src/layout/translate.rs:625–637` — `sizing_to_dim` maps
  `MinContent|MaxContent|FitContent` to `Dimension::auto()` ("until Phase 10 + text rendering
  integrate"). taffy 0.10.1's `Dimension` cannot carry intrinsic keywords (measure § 3.3) —
  the mapping **stays**; the measure closure realizes the keyword. Comment updated in Task 4.
- `crates/buiy_core/src/text/` — T2 as-built: `components.rs` (`TextBuffer { pub buffer,
  intrinsics: Option<IntrinsicWidths> }` with `intrinsics()`/`invalidate_intrinsics()`;
  `ComputedTextLayout`/`ComputedTextLine`; `TEXT_SHAPING`), `sync.rs` (`text_sync_buffers`,
  6-member `TextSyncTriggers`, `AuthoredStyle`, `apply_authored`, `DEFAULT_LINE_HEIGHT_SCALE
  = 1.2`, `DEFAULT_WRAP = Wrap::Word`, `DEFAULT_TAB_WIDTH = 8`), `whitespace.rs`
  (`CollapseMode` + `collapse_whitespace`, all three modes BUILT), `font_system.rs`
  (`SharedFontSystem::lock()` — panics on poison), `mod.rs` (plugin wiring).
  **Gains `measure.rs`, `commit.rs`, the four carriers, and `cache_intrinsics`.**
- `crates/buiy_core/tests/layout_pipeline_order.rs` — tracker labels currently 10
  (`["gc","wmi","text_sync","sync","cq_activate","taffy","cq_flip","cq_rerun","post_taffy","write"]`),
  asserted `n == 10`. **Grows `text_commit`, n == 11**, plus a commit-after-`CqDescendantReRun`
  pair test (the existing step-8/9 pair tests are the template).
- `crates/buiy_core/tests/text_sync.rs` — `text_app()`/`settle()`/`applied()` helpers, the
  trigger-row tests, the dirty-probe (which already guards against per-frame
  `set_node_context`: a steady-frame re-registration would flip the probe to `Some(true)`).
- `crates/buiy_core/tests/layout_container_queries.rs` — `cq_same_frame_relayout_caps_at_2x_taffy`
  (:98–168): the flip fixture Task 5's re-entrancy tests extend.
- `crates/buiy_core/src/lib.rs:52` + `crates/buiy/src/lib.rs:34` — the `text::{…}` re-export
  groups (grow).

## Decisions this plan encodes (resolved against the spec — do not relitigate)

1. **Context registration is split across the two natural edges** (resolves architecture
   § 4.3's coordination pin): (a) **node creation** — `translate_one_entity`'s `new_leaf` arm
   becomes `new_leaf_with_context(style, entity)` when the entity is a text leaf, so a
   brand-new text entity is measurable on its **first** frame (TextSync's deferred
   `TextBuffer` insert flushes before `SyncStyles`); (b) **text-edge-on-existing-node** —
   TextSync's creation loop calls `LayoutTree::set_text_context` (covers `Text` added to an
   already-laid-out entity; no-op when the node doesn't exist yet) and the
   `RemovedComponents<Text>` arm calls `clear_text_context` (whose internal `mark_dirty`
   correctly forces the now-plain leaf to re-measure as zero). *Runner-ups rejected:*
   registering on the frame-2 `Added<TextBuffer>` echo — one full frame of zero-measured text
   (text appears a frame late, exactly what T2's creation-arm design prevents); threading
   `Has<TextBuffer>` through `NodeQueryItem` — pushes the shared 14-tuple to Bevy's 15 cap and
   churns every caller for what one side query answers.
2. **The text-leaf probe at translate time is `Query<(), With<Text>>`** — the authored
   component, present immediately at spawn (no dependence on the deferred `TextBuffer` flush
   order), passed to `translate_one_entity` as a `bool`. All three translate callers
   (`sync_styles`, `cq_flip_rerun`, `cq_descendant_rerun`) gain the same side query.
3. **One `#[derive(SystemParam)] struct TextMeasureParam`** (fonts + buffers + call-count)
   so each compute site grows by exactly **one** parameter; `fonts`/`call_count` are
   `Option` so `LayoutPlugin`-without-`BuiyTextPlugin` apps (the standing layout tests) run
   unchanged — absent engine ⇒ plain zero-measure compute. Param-cap bookkeeping (the 16
   cap): `taffy_compute` 4→6; `sync_styles` 13→14; `cq_flip_rerun` 15→16 with the
   `(roots, windows)` pair grouped into one tuple param in Task 5; `cq_descendant_rerun` is
   **already at 16** — Task 3 groups `(rules, containers)`, Task 5 groups `(roots, windows)`.
   *Runner-up rejected:* loose params — blows the cap at site 3 immediately.
4. **The locking discipline** is the section above — encoded in the helper's shape, not in
   reviewer vigilance: the closure receives `&mut FontSystem`, never the resource.
5. **Measure returns `ceil()`ed sizes.** Taffy's whole-px rounding (`use_rounding`,
   taffy_tree.rs:915–919) must never round the final box below the content it was measured
   for — a < 1 px width deficit re-wraps the last word at commit. The bevy_text precedent.
   *Runner-up rejected:* raw f32 — wrap jitter at rounded final widths; per-fixture epsilons
   forever.
6. **Intrinsics are computed under the buffer's resolved `Wrap`** (`set_size(Some(0.0),
   None)` → min-content; `set_size(None, None)` → max-content; measure § 3.2). For
   `Wrap::None` nothing breaks ⇒ min == max — the CSS `nowrap` behavior, for free. The cache
   key is "invalidate on every TextSync" (T2's `invalidate_intrinsics` already fires on every
   sync, and the new mode carriers are union members) — no separate version counter.
   *Runner-up rejected:* forcing `Wrap::Word` for the min-content probe — wrong for `nowrap`
   text and mutates state the protocol would have to restore.
7. **`TextCommit` iterates all text entities with a cheap reconcile guard** — per entity:
   read the **current** Taffy layout (`tree.tree.layout(node).content_box_size()`, clamped
   ≥ 0 to match `set_size`'s internal clamp), apply align per line (`set_align`'s guarded
   bool is the change signal), and skip unless `align_changed || buffer.size() != target`.
   This is the spec's own steady-state shape ("the `buffer.size()` equality short-circuits",
   § 4.2) **and it closes a hole in the § 5.1 trigger row**: a buffer measure-touched this
   frame whose resolved size did not change (ancestor grew; max-content text narrower than
   both widths) fires neither the TextSync set nor `Changed<ResolvedLayout>` — yet its
   buffer was left at a probe width. The catch-all works because **measure always leaves
   `height_opt = None`** while commit always sets `Some` — a measured buffer can never
   compare equal. Geometry is read from the tree, not `ResolvedLayout` (border-box, lacks
   padding; and step 9 may have just recomputed). Recorded as **T3 erratum** for the spec
   edit pass. *Runner-up rejected:* the literal § 5.1 row (TextSync-dirty set ∪
   `Changed<ResolvedLayout>`) — misses the probe-leftover case; an 11-member Or-filter for
   strictly less correctness.
8. **Align is applied at commit, per line, every processed entity** (measure § 5.3: "a
   finalize concern, not a measure concern"). `TextSync` keeps passing `alignment: None` to
   `set_text` — set_text with `None` leaves reused lines' align untouched (verified), so the
   `Some→None` transition (`center` → `start`) is only correct in a pass that calls
   `set_align` on **every** line: the commit loop. `set_align`'s line-layout reset is picked
   up by `shape_until_scroll` via `resolve_dirty`'s external-invalidation branch (verified —
   the facts table). `Changed<TextAlign>` still joins the **TextSync** union per the § 5.1
   carrier pin (cost: a spurious re-measure on align-change frames; alignment never changes
   measured w/h, so results are identical). *Runner-up rejected:* align as a commit-only
   trigger — contradicts the § 5.1/§ 7 carrier pin for a micro-optimization.
9. **Commit sets `set_size(Some(w), Some(h))` — spec § 4.2 verbatim — keeping cosmic's
   height windowing.** Consequence (recorded as T3 erratum): lines past the content-box
   height do not lay out (`shape_until_scroll` stops at `scroll_end`; `LayoutRunIter` also
   cuts at `height_opt`), so `overflow: visible` text taller than its box is absent from
   `ComputedTextLayout` and from T4's emission until the overflow seam is revisited.
   *Runner-up rejected:* `Some(w), None` — full fidelity for overflow, but lays out every
   line of arbitrarily long buffers each reshape, losing the windowing that makes scrollable
   text views viable; flagged, not taken.
10. **The new carriers fall back to their `Default` impls, not to `TextStyleDefaults`.**
    font-assets § 8 pins the defaults resource to the font trio; line-height /
    white-space / wrap / align have CSS initials as component defaults (`Normal` / `Normal` /
    `Wrap` / `Start`). *Runner-up rejected:* growing `TextStyleDefaults` now — no spec
    backing; the theme seam (font-assets § 9) may grow it later.
11. **`METRICS_FLOOR = 0.01`:** cosmic's `set_metrics` panics on zero `font_size` or
    `line_height`; authored data (`FontSize(0.0)`, `LineHeight::Px(0.0)`, `Scale(0.0)`) must
    degrade, never panic the app. (T2 shipped this hazard latently — `Metrics::relative(0.0,
    1.2)` would already panic; the floor closes it where the mapping now lives.)
12. **`TextBufferAccess` is deferred to the `buiy-text-editing` campaign** — its `edit` arm
    binds `TextEditState`, which cannot exist before that campaign; a one-arm QueryData
    wrapper today is dead abstraction. The measure closure and `TextCommit` bind
    `&mut TextBuffer` directly; the swap is mechanical when the editor lands. **Supersedes**
    T2's seam-table "built in T3" row — recorded as T3 erratum, not silently dropped.
13. **`match-parent` is not a `TextAlign` variant** — the § 5.3 table's own note: resolved at
    style time, "never reaches cosmic-text as a distinct value"; Buiy has no style-resolution
    cascade yet. Named in the enum doc.
14. **`Sizing::FitContent(_)` and height-axis intrinsic keywords stay `auto`-equivalent** —
    measure § 3.3 pins width-axis `MinContent|MaxContent` only; both named in the closure's
    doc as deferrals.
15. **`ResolvedBaseline` is removed (not zeroed) when the buffer has no laid-out runs** —
    an empty `Text("")` leaf has no baseline; consumers branch on presence, never on a
    sentinel.
16. **Instruments:** `TextMeasureCallCount` (spec § 7 — reset in `taffy_compute`, incremented
    by every text-leaf closure invocation at any site, mirroring `LayoutTaffyComputeCount`'s
    reset-once-increment-everywhere shape) and `TextCommitReshapeCount` (spec § 8 item 4's
    "zero buffer relayouts" made assertable; overwritten per commit invocation, the
    `TextSyncAppliedCount` precedent).
17. **The measure protocol lives in `crates/buiy_core/src/text/measure.rs`** — the measure
    § 1 ownership table assigns the protocol to text and the compute sites to layout; the
    in-crate call cycle (layout systems → text helper; text commit → layout outputs) is
    exactly why architecture § 7 keeps text a `buiy_core` module. *Runner-up rejected:*
    helper in `layout/systems.rs` — layout would own width resolution, the intrinsic cache,
    and the lock discipline it has no stake in.

## File structure

```
crates/buiy_core/
├── src/
│   ├── lib.rs                       # text re-export group grows
│   ├── layout/
│   │   ├── pipeline.rs              # +BuiyLayoutStep::TextCommit (enum + chain)
│   │   ├── tree.rs                  # TaffyTree<()> → TaffyTree<Entity>; +set_text_context/clear_text_context
│   │   ├── systems.rs               # translate_one_entity is_text arm; 3 sites → compute_roots_with_text_measure;
│   │   │                            #   text_leaves side queries; param groupings
│   │   └── translate.rs             # sizing_to_dim comment update (keywords realized in the closure)
│   └── text/
│       ├── components.rs            # +LineHeight, WhiteSpace, TextWrap, TextAlign (+value tables,
│       │                            #   warn-onces), +ResolvedBaseline, +TextBuffer::cache_intrinsics
│       ├── sync.rs                  # union 6 → 10; AuthoredStyle gains the carriers; metrics floor;
│       │                            #   context-edge calls
│       ├── measure.rs               # NEW: TextMeasureParam, TextMeasureCallCount, measure protocol,
│       │                            #   compute_roots_with_text_measure
│       ├── commit.rs                # NEW: text_commit, TextCommitReshapeCount, computed_outputs
│       └── mod.rs                   # +mod measure/commit; plugin wiring; re-exports
└── tests/
    ├── layout_pipeline_order.rs     # grows the text_commit tracker (10 → 11) + the pair test
    ├── text_components.rs           # grows carrier defaults/registration
    ├── text_sync.rs                 # grows carrier triggers, white-space content, context edges,
    │                                #   migration behavior pin
    ├── text_measure.rs              # NEW: content sizing, wrap-on-shrink, keywords, intrinsics,
    │                                #   shape_until_scroll pin, cq re-entrancy
    └── text_commit.rs               # NEW: reshape-at-final-width, align, idempotency, baseline,
    │                                #   steady state, regression edges
crates/buiy/src/lib.rs               # text re-export group grows
```

---

## Task 1 — Status flips + the four F-tier carriers (pure value tables)

The authored carriers measure §§ 5.1–5.3 pin, with their normative value tables and the two
warn-once degrades. Pure — nothing consumes them until Task 2.

**Files:**
- Modify: `docs/plans/2026-06-09-buiy-text-campaign.md`
- Modify: `docs/plans/2026-06-10-buiy-text-t3-measure-wrap-align.md` (this file)
- Modify: `crates/buiy_core/src/text/components.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`
- Modify: `crates/buiy_core/src/lib.rs`
- Modify: `crates/buiy/src/lib.rs`
- Modify: `crates/buiy_core/tests/text_components.rs`

- [ ] **Step 1: Flip the status rows.** Campaign phase table: `| T3 | Measure + wrap/align |
  proposed |` → `in progress`. This plan's header: `**Status:** proposed` → `active`.

- [ ] **Step 2: Write the failing tests** — grow `crates/buiy_core/tests/text_components.rs`:

```rust
use buiy_core::text::{LineHeight, TextAlign, TextWrap, WhiteSpace, resolve_wrap};
use buiy_core::text::CollapseMode;
use cosmic_text::{Align, Wrap};

/// The carriers' defaults are the CSS initials (measure §§ 5.1–5.3).
#[test]
fn t3_carrier_defaults_are_the_css_initials() {
    assert_eq!(LineHeight::default(), LineHeight::Normal);
    assert_eq!(WhiteSpace::default(), WhiteSpace::Normal);
    assert_eq!(TextWrap::default(), TextWrap::Wrap);
    assert_eq!(TextAlign::default(), TextAlign::Start);
}

/// measure § 5.2 — the normative white-space value table, both columns.
#[test]
fn white_space_value_table() {
    let rows = [
        (WhiteSpace::Normal, CollapseMode::Collapse, Wrap::Word),
        (WhiteSpace::Nowrap, CollapseMode::Collapse, Wrap::None),
        (WhiteSpace::Pre, CollapseMode::Preserve, Wrap::None),
        (WhiteSpace::PreWrap, CollapseMode::Preserve, Wrap::Word),
        (WhiteSpace::PreLine, CollapseMode::PreserveBreaks, Wrap::Word),
    ];
    for (ws, mode, wrap) in rows {
        assert_eq!(ws.collapse_mode(), mode, "{ws:?} collapse column");
        assert_eq!(ws.base_wrap(), wrap, "{ws:?} wrap column");
    }
}

/// measure § 5.2 — text-wrap composes over the table: `nowrap` forces
/// `Wrap::None`; `wrap` keeps the table value; balance/pretty/stable
/// degrade to the greedy table value (warn-once, not asserted here).
#[test]
fn text_wrap_composition() {
    assert_eq!(resolve_wrap(WhiteSpace::Normal, TextWrap::Nowrap), Wrap::None);
    assert_eq!(resolve_wrap(WhiteSpace::Pre, TextWrap::Wrap), Wrap::None);
    assert_eq!(resolve_wrap(WhiteSpace::Normal, TextWrap::Wrap), Wrap::Word);
    for degraded in [TextWrap::Balance, TextWrap::Pretty, TextWrap::Stable] {
        assert_eq!(resolve_wrap(WhiteSpace::Normal, degraded), Wrap::Word);
        assert_eq!(resolve_wrap(WhiteSpace::Nowrap, degraded), Wrap::None);
    }
}

/// measure § 5.3 — the normative text-align value table. `start` → None
/// (cosmic's unaligned default follows BiDi direction — exactly CSS
/// `start`); `justify-all` degrades to Justified (warn-once).
#[test]
fn text_align_value_table() {
    assert_eq!(TextAlign::Start.to_cosmic(), None);
    assert_eq!(TextAlign::End.to_cosmic(), Some(Align::End));
    assert_eq!(TextAlign::Left.to_cosmic(), Some(Align::Left));
    assert_eq!(TextAlign::Right.to_cosmic(), Some(Align::Right));
    assert_eq!(TextAlign::Center.to_cosmic(), Some(Align::Center));
    assert_eq!(TextAlign::Justify.to_cosmic(), Some(Align::Justified));
    assert_eq!(TextAlign::JustifyAll.to_cosmic(), Some(Align::Justified));
}
```

  Also grow `authoring_types_are_registered_for_reflection`'s name list with
  `"buiy_core::text::components::LineHeight"`, `…::WhiteSpace`, `…::TextWrap`, `…::TextAlign`.

- [ ] **Step 3: Run, expect compile FAIL** — `cargo test -p buiy_core --test text_components`
  → unresolved imports.

- [ ] **Step 4: Implement** — in `crates/buiy_core/src/text/components.rs` (add
  `use cosmic_text::{Align, Wrap};` to the imports and re-export `CollapseMode` usage from
  `super::whitespace`):

```rust
use std::sync::atomic::{AtomicBool, Ordering};

use super::whitespace::CollapseMode;

/// CSS `line-height` (measure § 5.1, F) — feeds `Metrics.line_height`, the
/// Σ term of measured height. Per-span line-height (`AttrsList`) is the
/// C-tier rich-text path, named in § 5.1's runner-up, not built.
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Debug)]
#[reflect(Component, Default)]
pub enum LineHeight {
    /// `line-height: normal` — the common UA factor 1.2
    /// (`DEFAULT_LINE_HEIGHT_SCALE`, the T2 stand-in, now the Normal arm).
    #[default]
    Normal,
    /// Unitless number — multiplier on font-size (`Metrics::relative`).
    Scale(f32),
    /// Fixed logical px (`Metrics::new`).
    Px(f32),
}

/// CSS `white-space` (measure § 5.2, F) — resolves to a
/// (collapse mode × `Wrap`) pair via the normative value table.
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub enum WhiteSpace {
    #[default]
    Normal,
    Nowrap,
    Pre,
    PreWrap,
    PreLine,
}

impl WhiteSpace {
    /// The table's collapse column (pre-pass mode, CSS Text L3 § 4.1 phase I).
    pub fn collapse_mode(self) -> CollapseMode {
        match self {
            WhiteSpace::Normal | WhiteSpace::Nowrap => CollapseMode::Collapse,
            WhiteSpace::Pre | WhiteSpace::PreWrap => CollapseMode::Preserve,
            WhiteSpace::PreLine => CollapseMode::PreserveBreaks,
        }
    }

    /// The table's `Wrap` column. `text-wrap` composes over it
    /// ([`resolve_wrap`]); the C-tier `overflow-wrap` later flips
    /// `Word` → `WordOrGlyph`/`Glyph` (measure § 5.5 — named, not built).
    pub fn base_wrap(self) -> Wrap {
        match self {
            WhiteSpace::Normal | WhiteSpace::PreWrap | WhiteSpace::PreLine => Wrap::Word,
            WhiteSpace::Nowrap | WhiteSpace::Pre => Wrap::None,
        }
    }
}

/// CSS `text-wrap` (measure § 5.2, F). `balance`/`pretty`/`stable` parse
/// and degrade to greedy `Word` wrap with a warn-once — no engine support
/// (cosmic-text and Parley both lack balancing); promotable later without
/// resharpening this seam.
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub enum TextWrap {
    #[default]
    Wrap,
    Nowrap,
    Balance,
    Pretty,
    Stable,
}

/// measure § 5.2: `text-wrap` composes where CSS says it does —
/// `nowrap` forces `Wrap::None` over the white-space table's wrap column;
/// `wrap` keeps it; the style keywords degrade to it (warn-once).
pub fn resolve_wrap(white_space: WhiteSpace, text_wrap: TextWrap) -> Wrap {
    match text_wrap {
        TextWrap::Nowrap => Wrap::None,
        TextWrap::Wrap => white_space.base_wrap(),
        TextWrap::Balance | TextWrap::Pretty | TextWrap::Stable => {
            warn_once_text_wrap_style_degrades();
            white_space.base_wrap()
        }
    }
}

/// CSS `text-align` (measure § 5.3, F) — applied at `TextCommit` (a
/// finalize concern: cosmic `Align` positions runs against the final line
/// width), never during measure. `match-parent` is deliberately NOT a
/// variant: per the § 5.3 table it is resolved at style time (the parent's
/// computed value lowered against the parent's direction) and never
/// reaches cosmic-text as a distinct value — a style-resolution-tier seam.
#[derive(Component, Reflect, Default, Clone, Copy, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub enum TextAlign {
    #[default]
    Start,
    End,
    Left,
    Right,
    Center,
    Justify,
    JustifyAll,
}

impl TextAlign {
    /// The § 5.3 value table. `Start` → `None`: cosmic-text's unaligned
    /// default follows the line's BiDi direction — exactly CSS `start`.
    /// `justify-all` degrades to `Justified` with a warn-once (last-line
    /// justification is not exposed upstream; promotable without
    /// reshaping this seam).
    pub fn to_cosmic(self) -> Option<Align> {
        match self {
            TextAlign::Start => None,
            TextAlign::End => Some(Align::End),
            TextAlign::Left => Some(Align::Left),
            TextAlign::Right => Some(Align::Right),
            TextAlign::Center => Some(Align::Center),
            TextAlign::Justify => Some(Align::Justified),
            TextAlign::JustifyAll => {
                warn_once_justify_all_degrades();
                Some(Align::Justified)
            }
        }
    }
}

static WARNED_TEXT_WRAP_STYLE: AtomicBool = AtomicBool::new(false);
static WARNED_JUSTIFY_ALL: AtomicBool = AtomicBool::new(false);

/// The translate.rs `warn_once_fr_outside_grid` precedent.
fn warn_once_text_wrap_style_degrades() {
    if !WARNED_TEXT_WRAP_STYLE.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: text-wrap balance/pretty/stable have no engine support; \
             degrading to greedy word wrap (warned once)"
        );
    }
}

fn warn_once_justify_all_degrades() {
    if !WARNED_JUSTIFY_ALL.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: text-align: justify-all degrades to justify — last-line \
             justification is not exposed by cosmic-text (warned once)"
        );
    }
}
```

  Wire up: `text/mod.rs` re-export grows
  `LineHeight, TextAlign, TextWrap, WhiteSpace, resolve_wrap` (note `CollapseMode` is already
  exported); `BuiyTextPlugin::build`'s `register_type` block grows the four carriers;
  `crates/buiy_core/src/lib.rs` and `crates/buiy/src/lib.rs` text groups grow
  `LineHeight, TextAlign, TextWrap, WhiteSpace`.

- [ ] **Step 5: Run the tests, expect PASS** — `cargo test -p buiy_core --test text_components`.

- [ ] **Step 6: Run GATE.** **Commit:** `feat(text): T3 carriers — LineHeight/WhiteSpace/TextWrap/TextAlign value tables`

---

## Task 2 — `TextSync` consumes the carriers (union 6 → 10)

The § 5.1 trigger union over the now-complete F-tier carrier set, and the lowering:
line-height → `Metrics` (with the panic floor), white-space → collapse mode, white-space ×
text-wrap → `Wrap`. Align is **trigger-only** here (commit applies it — decision 8).

**Files:**
- Modify: `crates/buiy_core/src/text/sync.rs`
- Modify: `crates/buiy_core/tests/text_sync.rs`

- [ ] **Step 1: Write the failing tests** — grow `crates/buiy_core/tests/text_sync.rs`
  (imports grow `LineHeight, TextAlign, TextWrap, WhiteSpace`):

```rust
#[test]
fn t3_carrier_changes_fire_the_union() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "carrier triggers");
    settle(&mut app);

    app.world_mut().entity_mut(entity).insert(LineHeight::Px(30.0));
    app.update();
    assert_eq!(applied(&app), 1, "Changed<LineHeight> fires the union");
    let metrics = app.world().get::<TextBuffer>(entity).unwrap().buffer.metrics();
    assert_eq!(metrics, Metrics::new(16.0, 30.0), "line-height → Metrics (§ 5.1)");

    app.world_mut().entity_mut(entity).insert(WhiteSpace::Nowrap);
    app.update();
    assert_eq!(applied(&app), 1, "Changed<WhiteSpace> fires the union");
    assert_eq!(
        app.world().get::<TextBuffer>(entity).unwrap().buffer.wrap(),
        Wrap::None,
        "§ 5.2 nowrap row"
    );

    app.world_mut().entity_mut(entity).insert(TextWrap::Balance);
    app.update();
    assert_eq!(applied(&app), 1, "Changed<TextWrap> fires the union");
    assert_eq!(
        app.world().get::<TextBuffer>(entity).unwrap().buffer.wrap(),
        Wrap::None,
        "balance degrades to the table value; nowrap's None wins here"
    );

    app.world_mut().entity_mut(entity).insert(TextAlign::Center);
    app.update();
    assert_eq!(
        applied(&app),
        1,
        "Changed<TextAlign> fires the union (§ 5.1 carrier pin) — \
         the VALUE is applied at TextCommit, not here"
    );
}

/// § 5.2 preserve rows: `pre` keeps runs of spaces + hard breaks and
/// maps to Wrap::None.
#[test]
fn white_space_pre_preserves_content_verbatim() {
    let mut app = text_app();
    let entity = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("a  b\tc\nsecond  line")),
            WhiteSpace::Pre,
        ))
        .id();
    app.update();
    assert_eq!(
        buffer_lines(&app, entity),
        vec!["a  b\tc", "second  line"],
        "preserve mode: nothing collapses; segment breaks become buffer lines"
    );
    assert_eq!(
        app.world().get::<TextBuffer>(entity).unwrap().buffer.wrap(),
        Wrap::None
    );
}

/// Authored zero metrics degrade instead of hitting cosmic's
/// `set_metrics` assert (the METRICS_FLOOR clamp).
#[test]
fn zero_font_size_and_line_height_do_not_panic() {
    let mut app = text_app();
    let entity = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("degenerate")),
            FontSize(0.0),
            LineHeight::Px(0.0),
        ))
        .id();
    app.update(); // would panic inside set_metrics without the floor
    let metrics = app.world().get::<TextBuffer>(entity).unwrap().buffer.metrics();
    assert!(metrics.font_size > 0.0 && metrics.line_height > 0.0);
}
```

- [ ] **Step 2: Run, expect FAIL** — `cargo test -p buiy_core --test text_sync` → the
  carrier inserts don't resync (`applied == 0`), `pre` content collapses, the zero-metrics
  test panics.

- [ ] **Step 3: Implement** — in `crates/buiy_core/src/text/sync.rs`:

  1. Imports grow: `LineHeight, TextAlign, TextWrap, WhiteSpace, resolve_wrap` from
     `super::components`. Delete the now-stale `DEFAULT_WRAP` const (the table owns wrap).
  2. The union (update the module-doc ledger: line-height/white-space/wrap/align joined in
     T3; remaining members: `TextDirection` → T5, theme font-token swap →
     `buiy-theme-tokens-design`):

```rust
type TextSyncTriggers = Or<(
    Changed<Text>,
    Changed<FontFamily>,
    Changed<FontSize>,
    Changed<FontWeight>,
    // T3 carriers (measure §§ 5.1–5.3). TextAlign is TRIGGER-ONLY here:
    // its value is applied at TextCommit (§ 5.3 — a finalize concern);
    // union membership is the § 5.1 carrier pin (an align edit must
    // dirty-mark the node like any other text-style change).
    Changed<LineHeight>,
    Changed<WhiteSpace>,
    Changed<TextWrap>,
    Changed<TextAlign>,
    Added<TextBuffer>,
    Changed<WritingModeResolved>,
)>;
```

  3. `SyncedText`/`SyncedTextItem` grow `Option<&LineHeight>, Option<&WhiteSpace>,
     Option<&TextWrap>` (and the `unsynced` creation query + both loop destructures grow the
     same three; align is not read by sync).
  4. `AuthoredStyle` grows the resolved values and the floor:

```rust
struct AuthoredStyle<'a> {
    family: &'a FontStack,
    size: f32,
    weight: u16,
    line_height: LineHeight,
    white_space: WhiteSpace,
    text_wrap: TextWrap,
}

/// cosmic-text's `set_metrics` asserts BOTH fields non-zero
/// (buffer.rs:729); authored data must degrade, never panic the app.
const METRICS_FLOOR: f32 = 0.01;
```

     `resolve` maps the three new `Option`s through `copied().unwrap_or_default()`
     (decision 10); `metrics()` becomes:

```rust
    /// font-size + line-height → `Metrics` (measure § 5.1).
    fn metrics(&self) -> Metrics {
        let font_size = self.size.max(METRICS_FLOOR);
        let line_height = match self.line_height {
            LineHeight::Normal => font_size * DEFAULT_LINE_HEIGHT_SCALE,
            LineHeight::Scale(scale) => font_size * scale,
            LineHeight::Px(px) => px,
        }
        .max(METRICS_FLOOR);
        Metrics::new(font_size, line_height)
    }
```

  5. `apply_authored` uses the tables (the doc comment keeps the § 5.4 prepend slot note):

```rust
fn apply_authored(buffer: &mut TextBuffer, text: &Text, style: &AuthoredStyle<'_>) {
    let collapsed = collapse_whitespace(&text.0, style.white_space.collapse_mode());
    buffer.buffer.set_metrics(style.metrics());
    buffer
        .buffer
        .set_wrap(resolve_wrap(style.white_space, style.text_wrap));
    buffer.buffer.set_tab_width(DEFAULT_TAB_WIDTH);
    buffer
        .buffer
        .set_text(&collapsed, &style.attrs(), TEXT_SHAPING, None);
    buffer.invalidate_intrinsics();
}
```

     (`alignment: None` stays — decision 8; the intrinsics invalidation on every sync IS the
     content-version cache key, decision 6.)

- [ ] **Step 4: Run the tests, expect PASS** — `cargo test -p buiy_core --test text_sync`
  (the whole file: the pre-existing T2 rows must stay green — `DEFAULT_WRAP`'s deletion must
  not change the `Normal` default's `Wrap::Word`).

- [ ] **Step 5: Run GATE.** **Commit:** `feat(text): TextSync lowers the T3 carriers — union 6→10, Metrics floor, wrap table`

---

## Task 3 — `TaffyTree<()>` → `TaffyTree<Entity>` + context lifecycle (behavior-unchanged)

The riskiest mechanical step, isolated: the context type flips and text leaves register
their `Entity`, but **nothing reads contexts yet** — plain `compute_layout`'s zero-measure
closure ignores them (taffy_tree.rs:925–927), so behavior is provably unchanged. The full
workspace suite is the does-not-change-behavior snapshot; a dedicated pin test makes the
invariant explicit.

**Files:**
- Modify: `crates/buiy_core/src/layout/tree.rs`
- Modify: `crates/buiy_core/src/layout/systems.rs`
- Modify: `crates/buiy_core/src/text/sync.rs`
- Modify: `crates/buiy_core/tests/text_sync.rs`

- [ ] **Step 1: Write the failing tests** — grow `crates/buiy_core/tests/text_sync.rs`:

```rust
/// The TaffyTree<Entity> migration is behavior-neutral until the measure
/// closure lands (Task 4): contexts are REGISTERED but plain
/// compute_layout's zero-measure closure ignores them, so a text leaf
/// still measures zero. This test is the migration's explicit
/// does-not-change-behavior snapshot; Task 4 flips the height assertion.
#[test]
fn migration_pin_text_leaf_still_measures_zero_and_context_is_registered() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "not yet measured");
    let plain = app.world_mut().spawn((Node, Style::default())).id();
    settle(&mut app);

    let layout = app
        .world()
        .get::<buiy_core::ResolvedLayout>(entity)
        .expect("text leaf has a layout");
    assert_eq!(
        layout.size.y, 0.0,
        "zero-measure behavior unchanged by the type migration"
    );

    let tree = app.world().non_send_resource::<LayoutTree>();
    let node = *tree.by_entity().get(&entity).unwrap();
    assert_eq!(
        tree.tree_ref().get_node_context(node),
        Some(&entity),
        "text leaf registered its Entity as node context (measure § 2.1)"
    );
    let plain_node = *tree.by_entity().get(&plain).unwrap();
    assert_eq!(
        tree.tree_ref().get_node_context(plain_node),
        None,
        "non-text nodes carry no context"
    );
}

/// measure § 2.2 — unregistration on the Text-removal edge.
#[test]
fn removing_text_clears_the_node_context() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "ephemeral context");
    settle(&mut app);

    app.world_mut().entity_mut(entity).remove::<Text>();
    app.update();

    let tree = app.world().non_send_resource::<LayoutTree>();
    let node = *tree.by_entity().get(&entity).unwrap();
    assert_eq!(
        tree.tree_ref().get_node_context(node),
        None,
        "clear_text_context on the RemovedComponents<Text> edge"
    );
}

/// measure § 2.2 — Text ADDED to an entity that already has a Taffy node
/// (the existing-node half of the registration split, decision 1).
#[test]
fn adding_text_to_an_existing_node_registers_the_context() {
    let mut app = text_app();
    let entity = app.world_mut().spawn((Node, Style::default())).id();
    settle(&mut app);

    app.world_mut()
        .entity_mut(entity)
        .insert(Text(String::from("late text")));
    app.update();

    let tree = app.world().non_send_resource::<LayoutTree>();
    let node = *tree.by_entity().get(&entity).unwrap();
    assert_eq!(tree.tree_ref().get_node_context(node), Some(&entity));
}
```

- [ ] **Step 2: Run, expect FAIL** — `cargo test -p buiy_core --test text_sync` → contexts
  are `None` (and `get_node_context` may not type-check against `TaffyTree<()>`'s `()`).

- [ ] **Step 3: Implement the type flip** — `crates/buiy_core/src/layout/tree.rs`:

```rust
use bevy::prelude::Entity;
use std::collections::HashMap;
use taffy::{NodeId as TaffyNodeId, TaffyTree};

#[derive(Default)]
pub struct LayoutTree {
    /// `TaffyTree<Entity>` (text measure § 2.1): text leaves register
    /// their entity as the node context; the measure closure receives
    /// `Option<&mut Entity>` straight from Taffy's leaf dispatch and
    /// resolves it against ECS queries. Non-text nodes carry no context.
    pub(crate) tree: TaffyTree<Entity>,
    pub(crate) by_entity: HashMap<Entity, TaffyNodeId>,
}
```

  `tree_ref()`'s return type becomes `&taffy::TaffyTree<bevy::prelude::Entity>`. Add the two
  edge helpers next to `mark_dirty_for_entity`:

```rust
    /// Register `entity` as its own node's measure context (text measure
    /// § 2.2). EDGE-TRIGGERED ONLY — `set_node_context` calls `mark_dirty`
    /// internally (taffy_tree.rs:656), so a per-frame call would silently
    /// kill the O(0) steady state (the text_sync dirty-probe test is the
    /// tripwire). No-op when the entity has no node yet: a brand-new text
    /// leaf's node is created WITH its context by `translate_one_entity`
    /// later the same frame.
    pub(crate) fn set_text_context(&mut self, entity: Entity) {
        if let Some(&node) = self.by_entity.get(&entity) {
            self.tree
                .set_node_context(node, Some(entity))
                .expect("LayoutTree: by_entity points at a live Taffy node");
        }
    }

    /// Unregister on the Text-removal edge. The internal `mark_dirty` is
    /// load-bearing here: the now-plain leaf must re-measure as zero.
    pub(crate) fn clear_text_context(&mut self, entity: Entity) {
        if let Some(&node) = self.by_entity.get(&entity) {
            self.tree
                .set_node_context(node, None)
                .expect("LayoutTree: by_entity points at a live Taffy node");
        }
    }
```

- [ ] **Step 4: Implement the creation-edge half** — `crates/buiy_core/src/layout/systems.rs`:

  1. `translate_one_entity` gains a trailing `is_text_leaf: bool` param; the `None` arm
     becomes:

```rust
        None => {
            // Text leaves are created WITH their measure context (text
            // measure § 2.1/§ 2.2): new_leaf_with_context registers the
            // entity at node birth, so a brand-new text entity is
            // measurable on its FIRST frame. Plain nodes use new_leaf —
            // no context, zero-measure dispatch never fires.
            let created = if is_text_leaf {
                tree.tree.new_leaf_with_context(taffy_style, entity)
            } else {
                tree.tree.new_leaf(taffy_style)
            };
            match created {
                Ok(id) => {
                    tree.by_entity.insert(entity, id);
                }
                Err(err) => {
                    warn!(
                        ?entity,
                        ?err,
                        "buiy: layout new_leaf failed; entity will be skipped this frame"
                    );
                }
            }
        }
```

  2. The three callers each gain one read-only side query
     `text_leaves: Query<(), With<crate::text::Text>>` (decision 2 — `With<Text>` is
     immediate at spawn; the deferred `TextBuffer` insert also flushes before `SyncStyles`,
     but `Text` removes the ordering dependence entirely) and pass
     `text_leaves.contains(entity)` at every `translate_one_entity` call site:
     - `sync_styles` (13 → 14 params): both the main loop and any creation path.
     - `cq_flip_rerun` (15 → 16 params — **at the cap**; Task 5 regroups before adding measure).
     - `cq_descendant_rerun` (**already at 16**): group `rules` + `containers` into one tuple
       param `(rules, containers): (Query<…>, Query<…>)` (tuples of `SystemParam`s are one
       param), then add `text_leaves` → 16. Destructure at the top of the body; the body's
       uses are otherwise unchanged.

- [ ] **Step 5: Implement the text-edge half** — `crates/buiy_core/src/text/sync.rs`:
  in the creation loop (after `commands.entity(entity).insert(buffer);`):

```rust
        if let Some(tree) = ctx.tree.as_deref_mut() {
            // Text added to an entity that ALREADY has a Taffy node
            // (decision 1's edge (b)); no-op for brand-new entities,
            // whose node is created with its context by sync_styles
            // later this frame.
            tree.set_text_context(entity);
        }
```

  and in the `removed_texts` loop (alongside the component removal — move the loop above the
  `commands` usage if borrow order requires, keeping behavior):

```rust
    for entity in removed_texts.read() {
        if let Some(tree) = tree_for_removal.as_deref_mut() {
            tree.clear_text_context(entity);
        }
        if let Ok(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.remove::<(TextBuffer, ComputedTextLayout)>();
        }
    }
```

  (Adapt to the existing `SyncContext` borrow structure — the tree is already a system param;
  despawned entities are covered by `gc_removed_nodes` + the `by_entity` guard. Update the
  T2 comment "the Taffy `set_node_context` unregistration … is T3's" — it lands here.)

- [ ] **Step 6: Run the new tests, expect PASS**, then run the **full workspace suite** —
  `cargo test --workspace -j 2` — this IS the migration's behavior snapshot: every layout,
  render, and text test must pass untouched. Investigate ANY layout-test delta as a
  migration bug, never as drift to accept.

- [ ] **Step 7: Run GATE.** **Commit:** `feat(layout): TaffyTree<()> → TaffyTree<Entity> — edge-triggered text context, behavior-neutral`

---

## Task 4 — The measure protocol + site 1 (`taffy_compute`)

The keystone: `text/measure.rs` with the § 3.2 protocol, `TextMeasureParam`, the intrinsics
cache fill, and `compute_roots_with_text_measure` wired into site 1. Sites 2/3 follow in
Task 5 (the helper is built for all three from the start; the cq tests land with their
wiring).

**Files:**
- Create: `crates/buiy_core/src/text/measure.rs`
- Modify: `crates/buiy_core/src/text/components.rs` (cache_intrinsics)
- Modify: `crates/buiy_core/src/text/mod.rs`
- Modify: `crates/buiy_core/src/layout/systems.rs` (`taffy_compute`)
- Modify: `crates/buiy_core/src/layout/translate.rs` (comment only)
- Modify: `crates/buiy_core/src/lib.rs`, `crates/buiy/src/lib.rs`
- Create: `crates/buiy_core/tests/text_measure.rs`

- [ ] **Step 1: Write the failing tests** — create `crates/buiy_core/tests/text_measure.rs`:

```rust
//! The Taffy measure seam (measure-and-layout §§ 2–3): content sizing,
//! wrap-on-shrink, intrinsic keywords, the intrinsics cache, and the
//! shape_until_scroll total-height pin. Headless — shaping uses the
//! embedded Fira Sans latin subset; no adapter anywhere.
//!
//! Assertion style: layout tests assert RELATIONS and invariants
//! (min < max, height multiples of line_height, equality against the
//! entity's own cached intrinsics) — never font-metric constants, which
//! belong to the GPU-lane goldens (T4+).

use bevy::prelude::*;
use buiy_core::layout::{AlignItems, LayoutPlugin, Length, Sizing, Style};
use buiy_core::text::{
    BuiyTextPlugin, SharedFontSystem, Text, TextBuffer, TextMeasureCallCount, WhiteSpace,
};
use buiy_core::{CorePlugin, Node, ResolvedLayout};
use cosmic_text::{Attrs, Buffer, Metrics, Shaping};

fn text_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app
}

/// Spawn frame + the Added<TextBuffer> echo frame (T2's documented
/// creation echo — the echo re-syncs, re-dirty-marks, and re-measures
/// once; steady state begins on frame 3).
fn settle(app: &mut App) {
    app.update();
    app.update();
}

fn measure_calls(app: &App) -> usize {
    app.world().resource::<TextMeasureCallCount>().0
}

/// A text leaf on the main axis of a flex row sizes to its content:
/// width == ceil(max-content) when it fits, height == one line.
/// (align_items: FlexStart so cross-axis stretch doesn't mask the
/// measured height.)
#[test]
fn text_leaf_sizes_to_content_in_a_flex_row() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::from("hello world"))))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_row()
                .align_items(AlignItems::FlexStart)
                .width_px(600.0)
                .height_px(100.0),
        ))
        .add_child(text);
    settle(&mut app);

    let intrinsics = app
        .world()
        .get::<TextBuffer>(text)
        .unwrap()
        .intrinsics()
        .expect("the first measure call computes and caches intrinsics (§ 3.2)");
    assert!(
        0.0 < intrinsics.min_content && intrinsics.min_content < intrinsics.max_content,
        "two words: longest-word min < unwrapped max; got {intrinsics:?}"
    );

    let layout = app.world().get::<ResolvedLayout>(text).unwrap();
    assert_eq!(
        layout.size.x,
        intrinsics.max_content.ceil(),
        "flex-basis auto = the measured max-content width (it fits in 600)"
    );
    // 16 px × 1.2 line-height = 19.2 → measure ceils to 20.
    assert_eq!(layout.size.y, 20.0, "one line at the default metrics");
}

/// The campaign's wrap-on-shrink row: in a flex column the cross axis
/// stretches the text to the parent width; shrinking the parent re-wraps
/// and ResolvedLayout height GROWS accordingly.
#[test]
fn text_wraps_when_parent_width_shrinks_and_height_grows() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from(
                "a reasonably long sentence that will need several lines",
            )),
        ))
        .id();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default().flex_column().width_px(400.0).height_px(300.0),
        ))
        .add_child(text)
        .id();
    settle(&mut app);
    let wide_height = app.world().get::<ResolvedLayout>(text).unwrap().size.y;

    app.world_mut().entity_mut(parent).insert(
        Style::default().flex_column().width_px(120.0).height_px(300.0),
    );
    settle(&mut app);

    let narrow = app.world().get::<ResolvedLayout>(text).unwrap();
    assert_eq!(narrow.size.x, 120.0, "cross-axis stretch to the new width");
    assert!(
        narrow.size.y > wide_height,
        "narrower box ⇒ more lines ⇒ taller: {} !> {wide_height}",
        narrow.size.y
    );
    assert_eq!(
        narrow.size.y % 20.0,
        0.0,
        "height is a whole number of ceil'd 19.2-px lines"
    );
}

/// § 3.3 — Sizing::MinContent/MaxContent on a text leaf resolve from the
/// cached intrinsics (realized in the measure closure; sizing_to_dim
/// still translates the keyword to Dimension::auto).
#[test]
fn intrinsic_keywords_resolve_on_text_leaves() {
    let mut app = text_app();
    let min_leaf = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width(Sizing::MinContent),
            Text(String::from("alpha beta gammaword")),
        ))
        .id();
    let max_leaf = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width(Sizing::MaxContent),
            Text(String::from("alpha beta gammaword")),
        ))
        .id();
    let parent = app.world_mut().spawn((
        Node,
        Style::default()
            .flex_row()
            .align_items(AlignItems::FlexStart)
            .width_px(600.0)
            .height_px(200.0),
    ));
    let parent = parent.id();
    app.world_mut()
        .entity_mut(parent)
        .add_children(&[min_leaf, max_leaf]);
    settle(&mut app);

    let min_intr = app.world().get::<TextBuffer>(min_leaf).unwrap().intrinsics().unwrap();
    let min_layout = app.world().get::<ResolvedLayout>(min_leaf).unwrap();
    assert_eq!(
        min_layout.size.x,
        min_intr.min_content.ceil(),
        "MinContent = longest-word width under Wrap::Word"
    );
    assert!(
        min_layout.size.y > 20.0,
        "min-content width wraps the three words onto multiple lines"
    );

    let max_intr = app.world().get::<TextBuffer>(max_leaf).unwrap().intrinsics().unwrap();
    let max_layout = app.world().get::<ResolvedLayout>(max_leaf).unwrap();
    assert_eq!(max_layout.size.x, max_intr.max_content.ceil());
    assert_eq!(max_layout.size.y, 20.0, "max-content never wraps");
}

/// CJK: break opportunities exist between characters (unicode-linebreak
/// is character-class-driven — no CJK font coverage needed; the embedded
/// subset renders .notdef but widths are real), so min-content is far
/// below max-content.
#[test]
fn cjk_min_content_breaks_between_characters() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default().width(Sizing::MinContent),
            Text(String::from("漢字文章測試")),
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_row()
                .align_items(AlignItems::FlexStart)
                .width_px(600.0)
                .height_px(300.0),
        ))
        .add_child(text);
    settle(&mut app);

    let intr = app.world().get::<TextBuffer>(text).unwrap().intrinsics().unwrap();
    assert!(
        intr.min_content < intr.max_content / 2.0,
        "six CJK chars: min (one char) ≪ max (six chars); got {intr:?}"
    );
}

/// No-break fixture: a single unbreakable word ⇒ min == max.
#[test]
fn single_word_min_content_equals_max_content() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::from("Antidisestablishmentarianism"))))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_row()
                .align_items(AlignItems::FlexStart)
                .width_px(600.0)
                .height_px(100.0),
        ))
        .add_child(text);
    settle(&mut app);

    let intr = app.world().get::<TextBuffer>(text).unwrap().intrinsics().unwrap();
    assert_eq!(intr.min_content, intr.max_content);
}

/// Tab fixture (charter risk 4): in preserve mode tabs advance to the
/// 8-column tab stops, so max-content with a tab exceeds the same text
/// without one.
#[test]
fn preserved_tabs_advance_intrinsic_width() {
    let mut app = text_app();
    let tabbed = app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::from("a\tb")), WhiteSpace::Pre))
        .id();
    let plain = app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::from("ab")), WhiteSpace::Pre))
        .id();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_row()
                .align_items(AlignItems::FlexStart)
                .width_px(600.0)
                .height_px(100.0),
        ))
        .id();
    app.world_mut().entity_mut(parent).add_children(&[tabbed, plain]);
    settle(&mut app);

    let tabbed_intr = app.world().get::<TextBuffer>(tabbed).unwrap().intrinsics().unwrap();
    let plain_intr = app.world().get::<TextBuffer>(plain).unwrap().intrinsics().unwrap();
    assert!(tabbed_intr.max_content > plain_intr.max_content);
}

/// Charter risk 2 — `shape_until_scroll(fs, false)` with `height_opt =
/// None` shapes and lays out ALL lines (scroll_end = ∞, buffer.rs:609).
/// Direct unit-style pin against the real engine.
#[test]
fn shape_until_scroll_with_no_height_lays_out_every_line() {
    let fonts = SharedFontSystem::new();
    let mut font_system = fonts.lock();
    let mut buffer = Buffer::new_empty(Metrics::new(16.0, 20.0));
    let text = (0..100).map(|i| format!("line {i}")).collect::<Vec<_>>().join("\n");
    buffer.set_text(&text, &Attrs::new(), Shaping::Advanced, None);
    buffer.set_size(Some(500.0), None);
    buffer.shape_until_scroll(&mut font_system, false);

    let runs: Vec<_> = buffer.layout_runs().collect();
    assert_eq!(runs.len(), 100, "every buffer line produced a layout run");
    let total: f32 = runs.iter().map(|r| r.line_height).sum();
    assert_eq!(total, 100.0 * 20.0, "total height = Σ line_height");
}

/// The instrument: text changes re-measure; the count is per-frame.
#[test]
fn text_change_invokes_measure_and_count_resets_per_frame() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::from("count me"))))
        .id();
    settle(&mut app);

    app.world_mut().get_mut::<Text>(text).unwrap().0 = String::from("count me again");
    app.update();
    assert!(
        measure_calls(&app) > 0,
        "a content change dirty-marks the node and re-measures"
    );
}
```

- [ ] **Step 2: Run, expect compile FAIL** — `TextMeasureCallCount` unresolved.

- [ ] **Step 3: Implement the protocol** — create `crates/buiy_core/src/text/measure.rs`:

```rust
//! The Taffy measure seam (measure-and-layout §§ 3–4.3): the measure
//! protocol, the intrinsics cache fill, and the one shared compute helper
//! all three layout compute sites call.
//!
//! **Lock discipline (architecture § 1.2 site #1; measure § 3.4):** ONE
//! `SharedFontSystem::lock()` per [`compute_roots_with_text_measure`]
//! invocation, scoped to its body; the closure reborrows the held guard as
//! `&mut FontSystem` and must NEVER lock the resource itself —
//! `std::sync::Mutex` is not reentrant, a nested lock self-deadlocks. The
//! helper runs at most twice per frame (`taffy_compute` + at most one cq
//! re-run — `CqFlipReRanThisFrame` makes sites 2/3 mutually exclusive);
//! each invocation takes and releases the lock independently. The closure
//! is rebuilt per call from current world state, holds no cross-call
//! state, and never issues `Commands` — cq re-entrancy is then free
//! (measure § 4.3).

use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use cosmic_text::FontSystem;
use taffy::{AvailableSpace, NodeId, Size as TaffySize};

use crate::layout::{BoxModel, LayoutTaffyComputeCount, LayoutTree, Sizing};

use super::components::{IntrinsicWidths, TextBuffer};
use super::font_system::SharedFontSystem;

/// Per-frame count of measure-closure invocations on text leaves (measure
/// § 7 — the `SyncStylesIterCount` precedent). Reset by `taffy_compute`
/// at frame start; the cq re-run sites increment without resetting
/// (mirroring `LayoutTaffyComputeCount`). `tests/text_commit.rs` asserts
/// ZERO on a no-change frame — Taffy's cache holds and the edge-triggered
/// context registration holds (measure § 2.2).
#[derive(Resource, Default, Debug)]
pub struct TextMeasureCallCount(pub usize);

/// The text inputs each compute site threads into the helper — one
/// `SystemParam` so every site grows by exactly one parameter.
/// `fonts`/`call_count` are `Option`: `LayoutPlugin` without
/// `BuiyTextPlugin` (the standing layout tests) has neither resource and
/// degrades to the plain zero-measure compute.
#[derive(SystemParam)]
pub struct TextMeasureParam<'w, 's> {
    fonts: Option<Res<'w, SharedFontSystem>>,
    buffers: Query<'w, 's, (&'static mut TextBuffer, Option<&'static BoxModel>)>,
    call_count: Option<ResMut<'w, TextMeasureCallCount>>,
}

impl TextMeasureParam<'_, '_> {
    /// Frame-start reset (called by `taffy_compute` only — the
    /// `LayoutTaffyComputeCount` reset pattern).
    pub(crate) fn reset_call_count(&mut self) {
        if let Some(count) = self.call_count.as_deref_mut() {
            count.0 = 0;
        }
    }
}

/// The one compute helper, three sites (measure § 4.3): replaces plain
/// `compute_layout` at `taffy_compute`, `cq_flip_rerun`, and
/// `cq_descendant_rerun`. Measure adds ZERO extra Taffy passes — it rides
/// whatever passes layout already runs; `LayoutTaffyComputeCount`
/// semantics are unchanged (reset stays in `taffy_compute` alone).
pub(crate) fn compute_roots_with_text_measure(
    tree: &mut LayoutTree,
    measure: &mut TextMeasureParam,
    window_size: Vec2,
    roots: &[(Entity, NodeId)],
    compute_count: &mut LayoutTaffyComputeCount,
    site: &'static str,
) {
    let available = TaffySize {
        width: AvailableSpace::Definite(window_size.x),
        height: AvailableSpace::Definite(window_size.y),
    };
    // Lock site #1: one lock per invocation, guard scoped to this body,
    // dropped before return. Never stored. (See the module doc.)
    let mut guard = measure.fonts.as_ref().map(|fonts| fonts.lock());
    let buffers = &mut measure.buffers;
    let mut calls = 0usize;
    for &(entity, node) in roots {
        let result = match guard.as_deref_mut() {
            Some(font_system) => tree.tree.compute_layout_with_measure(
                node,
                available,
                |known_dimensions, available_space, _node_id, node_context, _style| {
                    let Some(&mut text_entity) = node_context else {
                        // Childless non-text leaf: no context registered.
                        return TaffySize::ZERO;
                    };
                    calls += 1;
                    measure_text_node(
                        font_system,
                        buffers,
                        text_entity,
                        known_dimensions,
                        available_space,
                    )
                },
            ),
            // No engine ⇒ no text entities can exist either.
            None => tree.tree.compute_layout(node, available),
        };
        match result {
            Ok(()) => compute_count.0 += 1,
            Err(err) => {
                warn!(?entity, ?err, "buiy: layout compute_layout ({}) failed", site);
            }
        }
    }
    drop(guard);
    if let Some(count) = measure.call_count.as_deref_mut() {
        count.0 += calls;
    }
}

/// One measure invocation (measure § 3.2). Taffy already subtracted the
/// content-box inset from `available_space` and adds it back to the
/// return (leaf.rs:111–146) — no BoxModel math here. Under
/// `RunMode::PerformLayout` taffy zeroes `known_dimensions` and passes
/// the resolved width as `AvailableSpace::Definite`, so the fold's
/// Definite arm answers at layout time.
fn measure_text_node(
    font_system: &mut FontSystem,
    buffers: &mut Query<(&'static mut TextBuffer, Option<&'static BoxModel>)>,
    entity: Entity,
    known_dimensions: TaffySize<Option<f32>>,
    available_space: TaffySize<AvailableSpace>,
) -> TaffySize<f32> {
    let Ok((mut text, box_model)) = buffers.get_mut(entity) else {
        // The context outlived its TextBuffer within this frame (the
        // removal edge races the compute): measure as empty; the cleared
        // context lands at the next sync point.
        return TaffySize::ZERO;
    };
    // measure § 7: a width probe is not a semantic change — never tick.
    let text = text.bypass_change_detection();
    let intrinsics = cached_intrinsics(text, font_system);
    // § 3.3 — width-axis intrinsic keywords answer from the cache
    // regardless of the probe. A parent-resolved known width still
    // overrides the measured size downstream (leaf.rs:143–146) — the
    // documented under-stretch fidelity limit. `FitContent` and
    // height-axis keywords stay auto-equivalent (named deferrals).
    let keyword_width = box_model.and_then(|bm| match bm.width {
        Sizing::MinContent => Some(intrinsics.min_content),
        Sizing::MaxContent => Some(intrinsics.max_content),
        _ => None,
    });
    let width = known_dimensions.width.or(keyword_width).unwrap_or(
        match available_space.width {
            AvailableSpace::MinContent => intrinsics.min_content,
            AvailableSpace::MaxContent => intrinsics.max_content,
            AvailableSpace::Definite(w) => w,
        },
    );
    // The definite-width relayout: `set_size` invalidates only
    // `layout_opt` (per-line `shape_opt` survives — the amortization the
    // protocol rides, § 3.2). Height stays None: measure never crops, and
    // the None is the catch-all signal TextCommit uses to recognize a
    // probe-left buffer (commit always sets Some — decision 7).
    text.buffer.set_size(Some(width), None);
    text.buffer.shape_until_scroll(font_system, false);
    let (max_w, total_h) = fold_runs(&text.buffer);
    // Ceil: taffy's whole-px rounding must never round the final box
    // below the measured content (a <1px deficit re-wraps the last word
    // at commit — the bevy_text precedent; decision 5).
    TaffySize {
        width: max_w.ceil(),
        height: total_h.ceil(),
    }
}

/// (max line_w, Σ line_height) over the laid-out runs (§ 3.2's fold).
fn fold_runs(buffer: &cosmic_text::Buffer) -> (f32, f32) {
    buffer.layout_runs().fold((0.0_f32, 0.0_f32), |(w, h), run| {
        (w.max(run.line_w), h + run.line_height)
    })
}

/// Serve or fill the per-content-version intrinsics cache (§ 3.2):
/// min-content via width-0 layout (every wrap opportunity breaks; under
/// `Wrap::None` nothing breaks ⇒ min == max — the CSS nowrap behavior),
/// max-content via unconstrained layout. `TextSync` invalidates on every
/// content change — that invalidation IS the cache key.
fn cached_intrinsics(text: &mut TextBuffer, font_system: &mut FontSystem) -> IntrinsicWidths {
    if let Some(cached) = text.intrinsics() {
        return cached;
    }
    let buffer = &mut text.buffer;
    buffer.set_size(Some(0.0), None);
    buffer.shape_until_scroll(font_system, false);
    let min_content = fold_runs(buffer).0;
    buffer.set_size(None, None);
    buffer.shape_until_scroll(font_system, false);
    let max_content = fold_runs(buffer).0;
    let widths = IntrinsicWidths {
        min_content,
        max_content,
    };
    text.cache_intrinsics(widths);
    widths
}
```

  In `components.rs`, next to `invalidate_intrinsics`:

```rust
    /// Fill the cache (the T3 measure closure is the only writer).
    pub(crate) fn cache_intrinsics(&mut self, widths: IntrinsicWidths) {
        self.intrinsics = Some(widths);
    }
```

  In `text/mod.rs`: `mod measure;` +
  `pub use measure::{TextMeasureCallCount, TextMeasureParam};`
  (`compute_roots_with_text_measure` stays `pub(crate)`), and
  `app.init_resource::<TextMeasureCallCount>();` in `build`. Re-export `TextMeasureCallCount`
  from `crates/buiy_core/src/lib.rs` and `crates/buiy/src/lib.rs`.

- [ ] **Step 4: Wire site 1** — `crates/buiy_core/src/layout/systems.rs`, `taffy_compute`
  (keep the doc comment, append the measure note):

```rust
pub(super) fn taffy_compute(
    mut tree: NonSendMut<LayoutTree>,
    nodes: Query<(Entity, Option<&ChildOf>), With<Node>>,
    windows: Query<&bevy::window::Window>,
    mut compute_count: ResMut<LayoutTaffyComputeCount>,
    mut measure: crate::text::TextMeasureParam,
) {
    let tree = &mut *tree;

    // Frame-start resets. `cq_flip_rerun` / `cq_descendant_rerun`
    // increment both counters without resetting.
    compute_count.0 = 0;
    measure.reset_call_count();

    let window_size = windows
        .iter()
        .next()
        .map(|w| Vec2::new(w.width(), w.height()))
        .unwrap_or(Vec2::new(800.0, 600.0));

    let roots: Vec<(Entity, taffy::NodeId)> = nodes
        .iter()
        .filter(|(_, parent)| {
            parent
                .map(|p| !tree.by_entity.contains_key(&p.parent()))
                .unwrap_or(true)
        })
        .filter_map(|(entity, _)| tree.by_entity.get(&entity).map(|&id| (entity, id)))
        .collect();

    crate::text::measure::compute_roots_with_text_measure(
        tree,
        &mut measure,
        window_size,
        &roots,
        &mut compute_count,
        "main pass",
    );
}
```

  (Make `mod measure` visible to layout: `pub(crate) mod measure;` in `text/mod.rs` with the
  `pub use` kept. Adjust the existing import list, not the warn semantics — the helper's
  parameterized message replaces the site-local string.)

  In `translate.rs`, update `sizing_to_dim`'s comment: intrinsic keywords still translate to
  `Dimension::auto()` (taffy 0.10.1's `Dimension` cannot carry them — measure § 3.3); for
  **text leaves** the measure closure now realizes the keyword from the cached intrinsics;
  container intrinsic sizing remains the Phase 10 half.

- [ ] **Step 5: Run the new tests, expect PASS** —
  `cargo test -p buiy_core --test text_measure`. The Task 3 migration pin
  (`…still_measures_zero…`) now FAILS by design: flip its height assertion to
  `assert!(layout.size.y > 0.0, "the measure closure is live (Task 4)")`, rename it
  `text_leaf_measures_its_content_through_the_node_context`, and update its doc comment.

- [ ] **Step 6: Run GATE** (the full layout suite guards the `taffy_compute` rewrite).
  **Commit:** `feat(text): the Taffy measure seam — protocol, intrinsics cache, site 1`

---

## Task 5 — Sites 2 + 3 (`cq_flip_rerun` / `cq_descendant_rerun`) + re-entrancy

The correctness half of the helper (measure § 4.3's "runner-up rejected as a correctness
bug"): both cq re-run paths measure. Param-cap regrouping per decision 3.

**Files:**
- Modify: `crates/buiy_core/src/layout/systems.rs`
- Modify: `crates/buiy_core/tests/text_measure.rs`

- [ ] **Step 1: Write the failing tests** — grow `crates/buiy_core/tests/text_measure.rs`
  (imports grow `ContainerQuery, QueryCondition, ContainerQueryActive` and
  `buiy_core::layout::LayoutTaffyComputeCount`):

```rust
/// Site 3 (cq_descendant_rerun) re-entrancy: a container resize seeds the
/// descendant cascade the SAME frame (layout step 8→9); the re-run's
/// compute must measure the text leaf at its NEW width — with plain
/// compute_layout it would zero-collapse (measure § 4.3's named bug).
/// The compute ceiling holds: count == 2 × roots.
#[test]
fn cq_descendant_rerun_remeasures_text_same_frame_within_2x() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from(
                "long enough content to wrap at the narrow width for sure",
            )),
        ))
        .id();
    let mid = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width(Sizing::Length(Length::Cqw(50.0))),
        ))
        .add_child(text)
        .id();
    let container = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(800.0)
                .height_px(600.0)
                .container_size(),
        ))
        .add_child(mid)
        .id();
    settle(&mut app);
    settle(&mut app); // Cqw resolves against the settled snapshot
    let wide_height = app.world().get::<ResolvedLayout>(text).unwrap().size.y;

    // Shrink the container: step 7 surfaces the new size, step 8 seeds
    // mid+text dirty, step 9 re-translates them against the NEW snapshot
    // and recomputes — measuring text at ~200 px THIS frame.
    app.world_mut().entity_mut(container).insert(
        Style::default()
            .flex_column()
            .width_px(400.0)
            .height_px(600.0)
            .container_size(),
    );
    app.update();

    assert_eq!(
        app.world().resource::<LayoutTaffyComputeCount>().0,
        2,
        "descendant-cascade frame: exactly 2 Taffy passes (1 root × 2)"
    );
    assert!(
        measure_calls(&app) > 0,
        "the re-run measured text (site 3 carries the closure)"
    );
    let narrow = app.world().get::<ResolvedLayout>(text).unwrap();
    assert!(
        narrow.size.y >= wide_height && narrow.size.y > 0.0,
        "same-frame re-wrap at the narrower width — never zero-collapsed \
         (got {} after {wide_height})",
        narrow.size.y
    );

    // Let the multi-frame cascade settle, then: steady state.
    settle(&mut app);
    app.update();
    assert_eq!(measure_calls(&app), 0, "steady frame after the cascade");
}

/// Site 2 (cq_flip_rerun) re-entrancy: the activation-flip re-run
/// completes with text in the tree — no deadlock (the lock is scoped per
/// helper call), the 2× cap holds, and the text leaf keeps its measured
/// size through the flip frame.
#[test]
fn cq_flip_rerun_with_text_holds_the_2x_cap() {
    let mut app = text_app();
    // The cq_same_frame_relayout_caps_at_2x_taffy fixture + a text leaf.
    let text = app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::from("flip me"))))
        .id();
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            ContainerQuery {
                container: None,
                conditions: vec![QueryCondition::MinWidth(Length::Px(600.0))],
            },
        ))
        .add_child(text)
        .id();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_column()
                .width_px(500.0)
                .height_px(400.0)
                .container_size(),
        ))
        .add_child(child)
        .id();
    settle(&mut app);

    app.world_mut().entity_mut(parent).insert(
        Style::default()
            .flex_column()
            .width_px(700.0)
            .height_px(400.0)
            .container_size(),
    );
    app.update(); // the flip frame: taffy_compute + cq_flip_rerun

    assert!(app.world().get::<ContainerQueryActive>(child).is_some());
    assert_eq!(
        app.world().resource::<LayoutTaffyComputeCount>().0,
        2,
        "flip frame runs Taffy exactly twice — measure rode both passes"
    );
    assert!(
        app.world().get::<ResolvedLayout>(text).unwrap().size.y > 0.0,
        "text stayed measured through the flip re-run"
    );
}
```

- [ ] **Step 2: Run, expect FAIL** — the descendant-rerun test's height collapses to zero on
  the cascade frame (site 3 still zero-measures) — exactly the spec's named bug.

- [ ] **Step 3: Implement** — `crates/buiy_core/src/layout/systems.rs`:

  1. **`cq_flip_rerun`:** group `roots` + `windows` into one tuple param
     `(roots, windows): (Query<(Entity, Option<&Children>, Option<&ChildOf>, &Position), With<Node>>, Query<&bevy::window::Window>)`,
     then add `mut measure: crate::text::TextMeasureParam` (16 params — at the cap).
     Replace the per-root compute loop (`:3596–3619`) with root collection (same `is_root`
     filter) + `compute_roots_with_text_measure(tree, &mut measure, window_size, &roots_vec,
     &mut compute_count, "cq flip re-run")`. **No `compute_count` reset, no
     `reset_call_count`** — both live only in `taffy_compute`. Update the fn doc comment's
     param-count note.
  2. **`cq_descendant_rerun`:** group `roots` + `windows` the same way (this is the second
     grouping — Task 3 grouped `(rules, containers)`), add the measure param (16 params).
     Replace its compute loop (`:2870–2895`) identically with site label
     `"cq descendant re-run"`.
  3. Keep both bodies otherwise byte-identical in behavior — the children-sync passes, the
     snapshot rebuilds, and the `ResolvedLayout` rewrites are untouched.

- [ ] **Step 4: Run the tests, expect PASS** — `cargo test -p buiy_core --test text_measure
  --test layout_container_queries` (the pre-existing 2×-cap tests must stay green — the
  helper must not change `LayoutTaffyComputeCount` semantics).

- [ ] **Step 5: Run GATE.** **Commit:** `feat(layout): measure rides all three compute sites — cq re-entrancy under the 2x cap`

---

## Task 6 — `BuiyLayoutStep::TextCommit` — reshape at final width, align, outputs

The new final layout step (architecture § 4.2): reconcile every buffer to its Taffy
content-box, apply `Align`, write `ResolvedBaseline` + `ComputedTextLayout`
idempotent-insert. The campaign's moved idempotency test lands here.

**Files:**
- Modify: `crates/buiy_core/src/layout/pipeline.rs`
- Modify: `crates/buiy_core/src/text/components.rs` (ResolvedBaseline)
- Create: `crates/buiy_core/src/text/commit.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`
- Modify: `crates/buiy_core/src/lib.rs`, `crates/buiy/src/lib.rs`
- Modify: `crates/buiy_core/tests/layout_pipeline_order.rs`
- Create: `crates/buiy_core/tests/text_commit.rs`

- [ ] **Step 1: Write the failing order tests** — in
  `crates/buiy_core/tests/layout_pipeline_order.rs`: add a `text_commit` tracker
  `.in_set(BuiyLayoutStep::TextCommit)`, grow the expected array to
  `[…, "write", "text_commit"]` — note the tracked-label list skips the two untracked
  cq-descendant steps, so `text_commit` follows `"write"` — and `n == 11`; add a pair test
  `text_commit_runs_after_cq_descendant_rerun` cloned from
  `cq_descendant_rerun_runs_after_invalidate` (labels `"cq_descendant_rerun"` /
  `"text_commit"`).

- [ ] **Step 2: Write the failing commit tests** — create
  `crates/buiy_core/tests/text_commit.rs`:

```rust
//! `BuiyLayoutStep::TextCommit` (measure-and-layout §§ 4.2, 5.3, 6;
//! architecture §§ 3.3, 4.2): reshape at the final content-box, align at
//! commit, idempotent output writes, and the steady-state instruments.

use bevy::prelude::*;
use buiy_core::layout::{LayoutPlugin, Style};
use buiy_core::text::{
    BuiyTextPlugin, ComputedTextLayout, ResolvedBaseline, Text, TextAlign, TextBuffer,
    TextCommitReshapeCount, TextMeasureCallCount,
};
use buiy_core::{BuiySet, CorePlugin, Node};
use cosmic_text::Align;

fn text_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app
}

fn settle(app: &mut App) {
    app.update();
    app.update();
}

/// § 4.2 — the buffer's last measured width can differ from the laid-out
/// width (stretch): commit reconciles to the final CONTENT-BOX. In a flex
/// column the leaf stretches to the parent width — wider than its
/// max-content — and measure left height_opt = None; commit must end with
/// buffer.size() == (Some(parent_w), Some(content_h)).
#[test]
fn commit_reshapes_the_buffer_to_the_final_content_box() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::from("short"))))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default().flex_column().width_px(300.0).height_px(100.0),
        ))
        .add_child(text);
    settle(&mut app);

    let buffer = &app.world().get::<TextBuffer>(text).unwrap().buffer;
    let (w, h) = buffer.size();
    assert_eq!(w, Some(300.0), "stretched to the parent content width");
    assert_eq!(h, Some(20.0), "committed height = the ceil'd measured line");
}

/// § 6 — ComputedTextLayout carries the per-line LayoutRun geometry and
/// ResolvedBaseline carries first/last line_y.
#[test]
fn commit_writes_computed_layout_and_baseline() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("first line second line third line")),
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default().flex_column().width_px(80.0).height_px(200.0),
        ))
        .add_child(text);
    settle(&mut app);

    let computed = app.world().get::<ComputedTextLayout>(text).unwrap();
    assert!(computed.lines.len() > 1, "80 px wraps the three 'words'");
    assert!(computed.size.x > 0.0 && computed.size.y > 0.0);
    let baseline = app.world().get::<ResolvedBaseline>(text).unwrap();
    assert_eq!(baseline.first, computed.lines.first().unwrap().line_y);
    assert_eq!(baseline.last, computed.lines.last().unwrap().line_y);
    assert!(baseline.last > baseline.first, "multi-line: baselines descend");
}

/// § 5.3 — align is applied AT COMMIT, per line, against the final width;
/// and the Some→None transition works (set_text(None) leaves reused
/// lines' align untouched — only the commit loop can clear it).
#[test]
fn align_applies_at_commit_and_clears_back_to_start() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("centered")),
            TextAlign::Center,
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default().flex_column().width_px(300.0).height_px(100.0),
        ))
        .add_child(text);
    settle(&mut app);

    {
        let buffer = &app.world().get::<TextBuffer>(text).unwrap().buffer;
        assert!(
            buffer.lines.iter().all(|l| l.align() == Some(Align::Center)),
            "every line carries the committed align"
        );
        let run = buffer.layout_runs().next().expect("one run");
        assert!(
            run.glyphs.first().expect("glyphs").x > 0.0,
            "centered in a 300-px box: the first glyph is offset from x=0"
        );
    }

    app.world_mut().entity_mut(text).insert(TextAlign::Start);
    settle(&mut app);
    let buffer = &app.world().get::<TextBuffer>(text).unwrap().buffer;
    assert!(
        buffer.lines.iter().all(|l| l.align().is_none()),
        "Start → None: the commit loop owns the Some→None transition"
    );
}

#[derive(Resource, Default)]
struct LayoutTickCount(usize);

fn count_layout_ticks(
    mut count: ResMut<LayoutTickCount>,
    changed: Query<(), Changed<ComputedTextLayout>>,
) {
    count.0 += changed.iter().count();
}

/// The campaign's moved-from-T2 test: ComputedTextLayout is
/// idempotent-insert (architecture § 3.3) — a steady frame leaves the
/// change tick untouched; a real change ticks exactly once.
#[test]
fn computed_text_layout_write_is_idempotent() {
    let mut app = text_app();
    app.init_resource::<LayoutTickCount>();
    app.add_systems(Update, count_layout_ticks.after(BuiySet::Layout));
    let text = app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::from("tick discipline"))))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default().flex_column().width_px(300.0).height_px(100.0),
        ))
        .add_child(text);
    settle(&mut app);
    let after_settle = app.world().resource::<LayoutTickCount>().0;
    assert_eq!(after_settle, 1, "exactly the first write (the echo frame reshapes to identical geometry — no tick)");

    app.update();
    app.update();
    assert_eq!(
        app.world().resource::<LayoutTickCount>().0,
        after_settle,
        "steady frames never tick ComputedTextLayout"
    );

    app.world_mut().get_mut::<Text>(text).unwrap().0 =
        String::from("genuinely new content that changes geometry");
    app.update();
    assert_eq!(
        app.world().resource::<LayoutTickCount>().0,
        after_settle + 1,
        "a real change ticks exactly once"
    );
}

/// Decision 15 — empty text has no baseline.
#[test]
fn empty_text_gets_no_resolved_baseline() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::new())))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default().flex_column().width_px(300.0).height_px(100.0),
        ))
        .add_child(text);
    settle(&mut app);
    assert!(app.world().get::<ResolvedBaseline>(text).is_none());
}
```

- [ ] **Step 3: Run, expect FAIL** — `TextCommit` variant and `ResolvedBaseline` unresolved.

- [ ] **Step 4: Implement the step** — `crates/buiy_core/src/layout/pipeline.rs`: append the
  variant after `CqDescendantReRun`:

```rust
    /// Step 10 (text) — reshape each `TextBuffer` at its FINAL Taffy
    /// content-box (the measured width can differ under stretch/grow, and
    /// measure leaves `height_opt = None`), apply text-align (a finalize
    /// concern — cosmic `Align` needs the final line width), and write
    /// `ResolvedBaseline` + `ComputedTextLayout` idempotently. Must trail
    /// `CqDescendantReRun`: steps 8–9 can still rewrite `ResolvedLayout`,
    /// and committing earlier would shape against sizes step 9
    /// immediately invalidates (text measure § 4.2).
    /// **Text T3** (architecture § 4.2).
    TextCommit,
```

  and add it to `configure_pipeline`'s chain tuple (now 13 sets).

- [ ] **Step 5: Implement the outputs + system** — `ResolvedBaseline` in
  `text/components.rs` (next to `ComputedTextLayout`):

```rust
/// Baseline offsets from the node's content-box top, from the first/last
/// `LayoutRun.line_y` ("Y offset to baseline of line", verified 0.19) —
/// measure § 6. Written by `TextCommit` idempotently; REMOVED when the
/// buffer has no laid-out runs (empty text has no baseline — consumers
/// branch on presence). Future consumers (vertical-align, inline baseline
/// alignment, AccessKit text geometry) are C-tier seams named in measure
/// § 5.5 — none are built in T3. Computed output — not reflect-registered.
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct ResolvedBaseline {
    /// First line's baseline offset.
    pub first: f32,
    /// Last line's baseline offset (== `first` for single-line text).
    pub last: f32,
}
```

  Create `crates/buiy_core/src/text/commit.rs`:

```rust
//! `BuiyLayoutStep::TextCommit` — reshape at final width (measure §§ 4.2,
//! 5.3, 6; architecture §§ 3.3, 4.2). Lock site #2: taken LAZILY, once,
//! only when at least one buffer reshapes — steady frames never lock.
//!
//! The trigger shape (decision 7, supersedes the § 5.1 row — recorded as
//! T3 erratum): iterate ALL text entities with a cheap reconcile guard
//! (per-line `set_align` compare + `buffer.size()` equality) instead of a
//! Changed<> filter. The guard is what catches the probe-left buffer: a
//! measure call always leaves `height_opt = None`, commit always sets
//! `Some`, so a measured-this-frame buffer can never compare equal even
//! when its resolved size didn't change.

use bevy::prelude::*;
use cosmic_text::FontSystem;
use std::sync::MutexGuard;

use crate::layout::LayoutTree;

use super::components::{
    ComputedTextLayout, ComputedTextLine, ResolvedBaseline, TextAlign, TextBuffer,
};
use super::font_system::SharedFontSystem;

/// Per-frame count of buffers `text_commit` actually reshaped (spec § 8
/// item 4's "zero buffer relayouts", made assertable; the
/// `TextSyncAppliedCount` precedent). Overwritten per invocation.
#[derive(Resource, Default, Debug)]
pub struct TextCommitReshapeCount(pub usize);

/// The `BuiyLayoutStep::TextCommit` body — the new FINAL layout step.
/// Geometry is read from the Taffy tree (current this frame, even after
/// the step-9 re-run), never from `ResolvedLayout` (border-box; lacks
/// padding). Steady-state cost: one hash lookup + one `Layout` read + an
/// O(lines) align compare + one tuple compare per text entity — no lock,
/// no shaping, no writes.
#[allow(clippy::type_complexity)]
pub fn text_commit(
    mut commands: Commands,
    tree: Option<NonSend<LayoutTree>>,
    fonts: Option<Res<SharedFontSystem>>,
    mut reshaped: ResMut<TextCommitReshapeCount>,
    mut texts: Query<(
        Entity,
        &mut TextBuffer,
        Option<&TextAlign>,
        Option<&ComputedTextLayout>,
        Option<&ResolvedBaseline>,
    )>,
) {
    reshaped.0 = 0;
    let (Some(tree), Some(fonts)) = (tree, fonts) else {
        // Standalone BuiyTextPlugin (no LayoutPlugin): nothing was
        // measured, nothing to commit.
        return;
    };
    let mut font_system: Option<MutexGuard<'_, FontSystem>> = None;
    for (entity, mut text, align, existing_layout, existing_baseline) in texts.iter_mut() {
        let Some(&node) = tree.by_entity.get(&entity) else {
            // Text on a non-Node entity (or GC'd this frame): no layout.
            continue;
        };
        let Ok(layout) = tree.tree.layout(node) else {
            continue;
        };
        // Pre-clamp to match set_size's internal `.max(0.0)`, or a
        // degenerate box (border+padding > size) would never compare
        // equal and reshape forever.
        let content = layout.content_box_size();
        let target = (
            Some(content.width.max(0.0)),
            Some(content.height.max(0.0)),
        );
        // measure § 7: commit writes are not damage on TextBuffer —
        // damage keys on the OUTPUT components below.
        let text = text.bypass_change_detection();

        // § 5.3 — text-align at commit, per line. set_align is internally
        // guarded (returns true only on change) and resets only that
        // line's layout; resolve_dirty's external-invalidation branch
        // makes the shape pass below pick the reset up.
        let align = align.copied().unwrap_or_default().to_cosmic();
        let mut align_changed = false;
        for line in text.buffer.lines.iter_mut() {
            align_changed |= line.set_align(align);
        }

        // § 4.2's steady-state short-circuit.
        if !align_changed && text.buffer.size() == target {
            continue;
        }

        // Lock site #2 — first reshape of the frame takes the lock.
        let font_system = font_system.get_or_insert_with(|| fonts.lock());
        text.buffer.set_size(target.0, target.1);
        text.buffer.shape_until_scroll(font_system, false);
        reshaped.0 += 1;

        // § 6 outputs — idempotent-insert (write_resolved_layout's
        // discipline): tick only when the value actually changed.
        let (computed, baseline) = computed_outputs(&text.buffer);
        if existing_layout.is_none_or(|current| *current != computed) {
            commands.entity(entity).insert(computed);
        }
        match (baseline, existing_baseline) {
            (Some(new), current) if current != Some(&new) => {
                commands.entity(entity).insert(new);
            }
            (None, Some(_)) => {
                commands.entity(entity).remove::<ResolvedBaseline>();
            }
            _ => {}
        }
    }
}

/// Fold the settled runs into the § 6 output pair.
fn computed_outputs(buffer: &cosmic_text::Buffer) -> (ComputedTextLayout, Option<ResolvedBaseline>) {
    let mut lines = Vec::new();
    let mut size = Vec2::ZERO;
    for run in buffer.layout_runs() {
        size.x = size.x.max(run.line_w);
        size.y += run.line_height;
        lines.push(ComputedTextLine {
            line_y: run.line_y,
            line_top: run.line_top,
            line_height: run.line_height,
            line_w: run.line_w,
            rtl: run.rtl,
        });
    }
    let baseline = match (lines.first(), lines.last()) {
        (Some(first), Some(last)) => Some(ResolvedBaseline {
            first: first.line_y,
            last: last.line_y,
        }),
        _ => None,
    };
    (ComputedTextLayout { lines, size }, baseline)
}
```

  Wire up in `text/mod.rs`: `mod commit;`, re-export
  `ResolvedBaseline, TextCommitReshapeCount, text_commit`,
  `app.init_resource::<TextCommitReshapeCount>();`, and register the system:

```rust
        app.add_systems(
            Update,
            (
                text_sync_buffers.in_set(crate::layout::BuiyLayoutStep::TextSync),
                // The new FINAL layout step (architecture § 4.2). Inert
                // without LayoutPlugin (Option params return early).
                commit::text_commit.in_set(crate::layout::BuiyLayoutStep::TextCommit),
            ),
        );
```

  Re-export `ResolvedBaseline, TextCommitReshapeCount` from `crates/buiy_core/src/lib.rs`
  and `crates/buiy/src/lib.rs`.

- [ ] **Step 6: Run the tests, expect PASS** —
  `cargo test -p buiy_core --test text_commit --test layout_pipeline_order`.

- [ ] **Step 7: Run GATE.** **Commit:** `feat(text): TextCommit — reshape at final content-box, align at commit, idempotent outputs`

---

## Task 7 — Steady-state contract + regression edges

The flagship invariant (measure § 7 / § 8 item 4) and the edges that protect it: a no-change
frame performs **zero** measure calls and **zero** buffer relayouts; the probe-leftover and
sweep paths reconcile correctly.

**Files:**
- Modify: `crates/buiy_core/tests/text_commit.rs`

- [ ] **Step 1: Write the failing tests** — grow `crates/buiy_core/tests/text_commit.rs`
  (imports grow `FontsGeneration`, `TextMeasureCallCount`):

```rust
/// measure § 7 + § 8 item 4 — THE steady-state contract: a no-change
/// frame performs zero measure invocations (Taffy's cache holds; the
/// edge-triggered context registration holds) and zero buffer relayouts
/// (the commit guard holds).
#[test]
fn steady_state_zero_measure_calls_and_zero_reshapes() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("steady as she goes")),
            TextAlign::Center,
        ))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default().flex_column().width_px(300.0).height_px(100.0),
        ))
        .add_child(text);
    settle(&mut app);
    app.update(); // flush any cascade remnants

    app.update(); // THE steady frame
    assert_eq!(
        app.world().resource::<TextMeasureCallCount>().0,
        0,
        "no-change frame: zero measure invocations"
    );
    assert_eq!(
        app.world().resource::<TextCommitReshapeCount>().0,
        0,
        "no-change frame: zero buffer relayouts"
    );
}

/// Decision 7's regression edge — the probe-left buffer: an ancestor
/// resize re-probes the leaf (new available width ⇒ Taffy cache miss ⇒
/// measure runs ⇒ buffer left at a probe width with height_opt = None)
/// while the leaf's RESOLVED size is unchanged (max-content text narrower
/// than both widths). Changed<ResolvedLayout> never fires for the leaf —
/// the § 5.1 trigger row would skip it — but the commit guard catches the
/// None height and reconciles. With align set, a probe-width buffer would
/// paint glyphs offset against the wrong width in T4 — this is the test
/// that keeps that bug impossible.
#[test]
fn ancestor_resize_with_unchanged_leaf_size_still_reconciles_the_buffer() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("tiny")),
            TextAlign::Center,
        ))
        .id();
    let parent = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .flex_row()
                .align_items(buiy_core::layout::AlignItems::FlexStart)
                .width_px(300.0)
                .height_px(100.0),
        ))
        .add_child(text)
        .id();
    settle(&mut app);
    let committed = app.world().get::<TextBuffer>(text).unwrap().buffer.size();
    let leaf_size = app.world().get::<ResolvedLayout>(text).unwrap().size;

    // Grow the parent: leaf max-content fits both widths ⇒ leaf size
    // unchanged ⇒ no Changed<ResolvedLayout> on the leaf.
    app.world_mut().entity_mut(parent).insert(
        Style::default()
            .flex_row()
            .align_items(buiy_core::layout::AlignItems::FlexStart)
            .width_px(500.0)
            .height_px(100.0),
    );
    app.update();

    assert_eq!(
        app.world().get::<ResolvedLayout>(text).unwrap().size,
        leaf_size,
        "fixture precondition: the leaf's resolved size did not change"
    );
    assert_eq!(
        app.world().get::<TextBuffer>(text).unwrap().buffer.size(),
        committed,
        "the commit catch-all reconciled the probe-left buffer back to \
         its content box (height Some, width = content)"
    );
}

/// architecture § 2.2 end-to-end: a FontsGeneration bump sweeps every
/// buffer through sync → measure → commit in one frame.
#[test]
fn fonts_generation_bump_remeasures_and_recommits() {
    let mut app = text_app();
    let text = app
        .world_mut()
        .spawn((Node, Style::default(), Text(String::from("sweep me"))))
        .id();
    app.world_mut()
        .spawn((
            Node,
            Style::default().flex_column().width_px(300.0).height_px(100.0),
        ))
        .add_child(text);
    settle(&mut app);
    app.update();

    app.world_mut().resource_mut::<FontsGeneration>().0 += 1;
    app.update();
    assert!(
        app.world().resource::<TextMeasureCallCount>().0 > 0,
        "the sweep dirty-marked the node — re-measured against the (new) font set"
    );
    let _ = text; // geometry assertions stay relational; same font ⇒ same layout

    app.update();
    assert_eq!(app.world().resource::<TextMeasureCallCount>().0, 0);
    assert_eq!(app.world().resource::<TextCommitReshapeCount>().0, 0);
}
```

- [ ] **Step 2: Run, expect PASS-or-fix** — these tests assert the **already-built**
  contract; any failure is a bug in Tasks 4–6 (a missed bypass, a per-frame
  `set_node_context`, a non-idempotent write). Fix at the source, never by loosening the
  test. (If the steady-state zero fails: the dirty-probe test in `text_sync.rs` and the
  `Changed<TextBuffer>`-never-fires test are the first places to look.)

- [ ] **Step 3: Run GATE.** **Commit:** `test(text): steady-state zero measure/reshape + probe-leftover regression pins`

---

## Task 8 — Docs-with-change + plan self-review

**Files:**
- Modify: `docs/plans/2026-06-09-buiy-text-campaign.md`
- Modify: `docs/README.md`
- Modify: `docs/plans/2026-06-10-buiy-text-t3-measure-wrap-align.md` (this file)

- [ ] **Step 1: Campaign flip.** Phase status table: `| T3 | Measure + wrap/align | in
  progress |` → `landed`.

- [ ] **Step 2: Campaign errata note** (the T1/T2 precedent — superseding context, not
  silent contradiction). Append a "T3 errata for the spec edit pass" list under the
  campaign's T3 entry with what was actually found; known members from this plan:
  1. *measure § 4.1/architecture § 5.1's `TextCommit` trigger row* ("the TextSync-dirty set
     ∪ `Changed<ResolvedLayout>`") misses measure-touched buffers whose resolved size did
     not change; as built, commit iterates all text entities behind a cheap reconcile guard,
     with measure's `height_opt = None` as the catch-all signal (plan decision 7).
  2. *measure § 2.3's `TextBufferAccess`* is deferred to the `buiy-text-editing` campaign —
     the `edit` arm binds `TextEditState`, which does not exist yet; the measure closure and
     `TextCommit` bind `&mut TextBuffer` directly (supersedes T2's seam-table "built in T3"
     row; plan decision 12).
  3. *measure § 4.2's `set_size(Some(w), Some(h))`* keeps cosmic's height windowing, so
     `overflow: visible` text taller than its box does not lay out past the content height
     (`LayoutRunIter` also cuts at `height_opt`) — revisit with T4's overflow painting (plan
     decision 9).
  4. The charter's compute-site line numbers drifted (2625/3602/2876 → 2627/3604/2878 at
     plan time) — re-pin in the spec's as-built references.
  Add any further mechanical inaccuracies discovered during implementation.

- [ ] **Step 3: Docs index row.** In `docs/README.md` § Text → **Plans**, after the T2 row:

```markdown
- [Buiy text T3 — Measure + wrap/align](plans/2026-06-10-buiy-text-t3-measure-wrap-align.md) — the Taffy measure seam: `TaffyTree<()> → TaffyTree<Entity>` with edge-triggered context registration, `compute_roots_with_text_measure` at all three compute sites (lock-scoped, cq-re-entrant, ≤2× ceiling preserved), the measure protocol (cached min/max-content intrinsics, definite-width relayout fold, ceil policy), intrinsic keywords on text leaves, the `LineHeight`/`WhiteSpace`/`TextWrap`/`TextAlign` carriers + value tables (balance/pretty/stable + justify-all warn-once degrades), `BuiyLayoutStep::TextCommit` (reshape at final content-box, align at commit, `ResolvedBaseline` + idempotent `ComputedTextLayout`), `TextMeasureCallCount`/`TextCommitReshapeCount` steady-state instruments. First pixels are T4. `[landed]`
```

- [ ] **Step 4: Flip this plan's Status** from `active` to `landed`.

- [ ] **Step 5: Implementation-vs-plan review.** Re-read this plan's **Decisions** list
  against the landed code: every numbered decision must be visible in code or doc comments
  (the locking discipline in `measure.rs`'s module doc, the decision-7 rationale at the
  commit guard, the edge-registration comments in `tree.rs`/`sync.rs`/`translate_one_entity`,
  the erratum-flagged deferrals). Fix drift in code comments, not by rewriting history here.

- [ ] **Step 6: Run GATE** (docs-only change; the gate confirms nothing drifted).
  **Commit:** `docs(text): T3 measure + wrap/align landed — campaign/index flips, errata note`

---

## Done criteria

- [ ] Gate green at every task boundary; **zero** new `#[ignore]` tests; **zero** new
  dependencies (`Cargo.toml`/`Cargo.lock` dependency sets unchanged); the GPU lane untouched.
- [ ] `LayoutTree.tree: TaffyTree<Entity>`; text contexts registered at node creation
  (`new_leaf_with_context`) and on the `Text` add/remove edges only (`set_text_context`/
  `clear_text_context`); the synthetic table tree at `systems.rs:~788` untouched.
- [ ] All **three** compute sites call `compute_roots_with_text_measure`; the lock is scoped
  per helper call, the closure never locks, never issues `Commands`;
  `LayoutTaffyComputeCount` semantics unchanged (cq tests' 2× caps still green).
- [ ] The measure protocol: intrinsics cached per content version, width fold
  (known → keyword → available), `ceil()` policy, `Wrap`-respecting min/max-content;
  `Sizing::MinContent`/`MaxContent` real on text leaves (stretch limit documented).
- [ ] Carriers + value tables: line-height → `Metrics` (with `METRICS_FLOOR`), the § 5.2
  white-space table + `text-wrap` composition (+ warn-once degrade), the § 5.3 align table
  (+ justify-all warn-once); TextSync union = 10 members; `tests/text_sync.rs` rows green.
- [ ] `BuiyLayoutStep::TextCommit` is the 13th chained step;
  `tests/layout_pipeline_order.rs` asserts 11 tracked labels + the after-`CqDescendantReRun`
  pair; commit reconciles to `content_box_size()` (clamped), applies align per line, writes
  `ResolvedBaseline` (removed when runless) + `ComputedTextLayout` idempotently.
- [ ] Campaign test list fully mapped: content sizing ✓ (Task 4), wrap-on-shrink ✓ (4),
  intrinsic keywords CJK/no-break/tab ✓ (4), `shape_until_scroll` total-height pin ✓ (4),
  cq re-entrancy ≤ 2× roots ✓ (5), `ComputedTextLayout` idempotency ✓ (6), pipeline-order
  growth ✓ (6), steady-state zero measure calls ✓ (7).
- [ ] Campaign T3 row + docs/README row + this plan's Status flipped to landed; the four
  errata recorded.

## Seams named here, built later (do NOT build in T3)

| Seam | Where named | Built in |
|---|---|---|
| `extract_buiy_glyphs` + atlas producer + `GlyphAlphaInstance` emission (the buffer's consumer) | `measure.rs`/`commit.rs` module docs | T4 |
| `debug_assert!` no visible dirty-unshaped buffer at extract | architecture § 3.2 | T4 (the producer) |
| Overflow-visible text past the content box (the decision-9 truncation) | commit doc + erratum | T4+ (overflow painting revisit) |
| `TextDirection` + the § 5.4 strong-mark prepend | `sync.rs` ledger (T2) | T5 |
| `FontStack` resolver (fontdb `Query` walk, span-splitting, `unicode-range`) | `AuthoredStyle::attrs` doc (T2) | T5 |
| `TextBufferAccess` QueryData (display/edit dispatch) | erratum + `TextBuffer` doc update | `buiy-text-editing` campaign (decision 12) |
| `ResolvedBaseline` consumers: `vertical-align`, inline baseline alignment, AccessKit text geometry | `ResolvedBaseline` doc | C-tier, own areas (measure §§ 5.5, 6) |
| `overflow-wrap` (`Word` → `WordOrGlyph`/`Glyph` flip), `text-indent`, per-span metrics (`AttrsList`) | `WhiteSpace::base_wrap` / `LineHeight` docs | C-tier, own areas (measure § 5.5) |
| `fit-content()` + height-axis intrinsic keywords | measure closure doc (decision 14) | with container intrinsic sizing (the Phase 10 half) |
| `text-wrap: balance` real balancing | `TextWrap` doc | promotable post-v1 (measure § 5.2) |
| Theme font-token swap member of the TextSync union | `sync.rs` ledger (T2) | `buiy-theme-tokens-design` |
| Multi-window measuring (first-window root sizing) | measure Open Q 3 | inherited layout limitation, not text's |
| Per-buffer memory trim policy (thousands of retained `BufferLine` caches) | measure Open Q 4 | verification area (gate #15 watch) |

## Plan self-review (performed at authoring, 2026-06-10)

1. **Charter coverage.** Every T3 deliverable maps to a task: `TaffyTree<Entity>` +
   edge-triggered `set_node_context` (Task 3), the shared helper at all three sites (Tasks
   4–5 — site line numbers re-verified: 2627/3604/2878), the measure protocol + intrinsics
   cache + definite-width fold (Task 4), `TextCommit` + `Align` + `ResolvedBaseline` +
   `ComputedTextLayout` (Task 6), line-height → `Metrics` / white-space + text-wrap →
   `Wrap` + degrade / text-align at commit (Tasks 1–2, 6), intrinsic keywords (Task 4),
   `TextMeasureCallCount` (Task 4, asserted Task 7). The charter's full test list is mapped
   in Done criteria with task numbers.
2. **Riskiest-step isolation.** The type migration (Task 3) lands behind a
   does-not-change-behavior pin (zero-measure assertion + full-workspace snapshot) BEFORE
   any closure exists; Task 4 then flips exactly one assertion in that pin, by name.
3. **Re-entrancy/locking.** The discipline has its own section, is restated in
   `measure.rs`'s module doc, and is structurally enforced (the closure receives
   `&mut FontSystem`, not the resource). Sites 2/3 are tested same-frame (Task 5) including
   the zero-collapse failure mode the spec names.
4. **Param-cap arithmetic** verified against current signatures: `sync_styles` 13→14,
   `cq_flip_rerun` 15→16 (one grouping), `cq_descendant_rerun` 16→16 (two groupings),
   `taffy_compute` 4→6 — all ≤ 16; the fallback ("group one more pair") is stated.
5. **Type consistency.** `TextMeasureParam` is the single cross-seam carrier;
   `cache_intrinsics` is the one cache writer; `to_cosmic`/`collapse_mode`/`base_wrap`/
   `resolve_wrap` names used uniformly across Tasks 1/2/6; `ComputedTextLine` field names
   match T2's as-built component exactly.
6. **Honesty.** T4+ seams are tabled; the three behavioral errata (commit trigger shape,
   `TextBufferAccess` deferral, overflow truncation) are flagged at decision time AND routed
   to the campaign errata in Task 8 — no silent spec contradiction ships.
