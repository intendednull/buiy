**Date:** 2026-07-03
**Status:** active — Phase-B execution plan for the Dooduel FINAL (the framework PR series F1–F9 + the `apps/dooduel` app + the acceptance gate).
**Kind:** campaign
**Spec:** [Dooduel FINAL — Phase B design](../specs/2026-07-03-dooduel-final-design.md) (revision 3 — decision-complete; approved 2026-07-03).
**Base:** `origin/main` @ `a969cbf` — the same base the prototype was cut from, so the prototype's framework commits re-apply cleanly. **`origin/main` advances during the campaign** (other tracks land); every PR **rebases on the latest `origin/main` before it opens** (see *Cross-cutting rules*).
**Worktree (this plan + spec):** `dooduel-final` (branch `worktree-dooduel-final`). Each PR below is built in **its own worktree off the latest `origin/main`** (fresh base), not on this branch.
**Prototype reference (unmerged — DO NOT MERGE):** `worktree-dooduel-proto1` @ the F-series commits cited per wave; `worktree-scribbl-campaign` holds the app + lineage docs.

# Plan — Dooduel FINAL (F1–F9 + app + acceptance)

**Strategy.** An **audited port that re-decides**, not a wholesale port. The prototype proved the whole game runs on Buiy with only additive framework growth; this plan lands that growth as **nine general-purpose, standalone, individually-reviewed PRs** (F1–F9, F4 split into F4a/F4b → ten PRs) that each touch `crates/` and are **dogfooded by — never local to — Dooduel**, then builds the app on the landed surface. Each PR is developed under `subagent-driven-development`, RUN + gated, rebased on the latest `origin/main`, and **stopped at a per-PR human merge gate** (pushes + PR-opening are authorized; **merges are individually human-gated** — authorization for one PR never carries to the next, spec §5.e). Docs (README index + any supersession) ship **with** each PR, not after.

---

## Merge order + build concurrency

Merges **serialize** (one human-gated PR at a time; each subsequent PR rebases on the newly-merged prior). Builds can run **concurrently** across independent crates. The two dimensions:

**Build lanes (may proceed in parallel):**
- **LANE A — `buiy_view` / `buiy_core::mvu` (parallel with LANE B):** F2 (layout; a small `buiy_core` `.fixed()`-anchor touch) ‖ F7 (mvu clock).
- **LANE B — `buiy_core` render (STRICTLY SERIAL — same files):** F1 (raster) → F3 (styling + borderless-rounded fill) → F4a (interleave) → F4b (shape). **F9 (HiDPI) edits the same `buiy_core` scale-factor seam and lands IN this line** (investigate early, land serially).
- **Independent, anytime:** F8 (`buiy_verify` driver).
- **Gated on LANE A:** F5 (press — needs F2), F6 (picking — needs F2).

**Canonical merge order** (respects every dependency edge + the serial render line):

```
Wave 0   lineage-docs PR (precondition) + CI env decision
  │
  ├─ F1   raster primitive            (LANE B head)
  ├─ F2   coherent layout surface     (LANE A — builds ‖ F1; merges in a gap)
  ├─ F7   MVU clock                   (LANE A — builds ‖ F1; merges in a gap)
  ├─ F8   playtest driver             (independent — merges any gap)
  │
  ├─ F3   styling + borderless fill   (needs F1 + F2; render line: F1 → F3-fill → …)
  ├─ F4a  paint-order interleave      (render line; after F3-fill)
  ├─ F4b  shape/decoration fixes      (needs F3's ExtractedNode.radius — the F3→F4b edge)
  ├─ F5   press route + interaction   (needs F2)
  ├─ F6   picking safety              (needs F2)
  └─ F9   HiDPI scale-factor          (render line tail; blocks the mobile acceptance criterion)
  │
App-1   apps/dooduel port (native + web crate + ci job)   ← on the fully-landed framework surface
App-2   6 gameplay-bug fixes + graduated playtest_host
  │
Acceptance   4-agent playtest + web (both backends + mobile + HiDPI) + full-workspace gate + closeout
```

F2/F7/F8 land in the gaps around the serial render line — whenever their gate is green and a human merge slot opens. The render line (F1 → F3 → F4a → F4b, plus F9) is **the schedule's critical path**; F9's unknown depth (risk R2) is the biggest scheduling risk on it.

**Total PR count: ~13** — 1 (Wave 0 lineage docs) + 10 framework (F1, F2, F3, F4a, F4b, F5, F6, F7, F8, F9) + 2 app (App-1 port, App-2 fixes + playtest_host). **→ 14** if F4b's dashed-stipple sub-item splits out (the pre-identified cleave, §5.f / risk R1), or if App web splits from App-1. Acceptance is a gate wave; its evidence + doc-status flips ride a small closeout on App-2 (or a 13th/15th PR if the maintainer prefers a standalone closeout).

---

## Cross-cutting rules (apply to EVERY wave)

1. **Rebase before PR.** `git fetch origin && git rebase origin/main` immediately before opening each PR — `origin/main` advances as other tracks land (CLAUDE.md *Branch from the latest upstream*). For the serial render line, cut each PR's branch from the `origin/main` that **already contains the merged prior render PR** (F3 from post-F1 main, F4a from post-F3 main, …) so the same-file edits stack cleanly.
2. **Render-path PRs serialize.** F1, F3-fill, F4a, F4b, F9 all edit `buiy_core` render (`extract.rs` / `instance.rs` / `node.rs` / `buckets.rs` / shaders / scale-factor) and **must not land concurrently** — churn otherwise. Keep every render change **additive + byte-stable** (the disjoint-node-set discipline, spec finding #2) so a later render PR rebases on an earlier one without golden churn.
3. **Byte-stability is the render contract.** Every existing display-list snapshot + GPU golden must stay **byte-identical** across a render PR unless the PR's own new fixture is the sole diff. A render PR that moves an unrelated golden is a bug — root-cause it (usually a fold that wasn't `set_if_neq`/`!=`-guarded), don't re-bless.
4. **Snapshot entity#-brittleness.** Layout snapshots pin raw `entity#<index>` for unnamed internal nodes (`buiy_verify/src/snapshot.rs:16`); **adding systems/observers/messages that spawn before a fixture shifts those indices** even when geometry is byte-identical (the Track-C C3/C4 churn). When a wave adds pre-fixture spawns: (a) prefer **Name-tagging** any new internal node so it is not index-pinned; (b) if an `.snap` must move, refresh it with a **verified entity-id-only diff** (no glyph/pos/size/color change) and say so in the commit. Do not let an id-only churn hide a real geometry change.
5. **Docs ship with the PR.** Each framework PR updates the `docs/README.md` catalog if it adds a doc, and flips any status it supersedes. The spec itself flips to `landed` only when the whole F-series + app has shipped (a closeout task).
6. **RUN the GUI every wave.** Headless-green is necessary, never sufficient (the standing Buiy lesson — the live crash, toolbar overflow, emoji tofu, band lens, click-swallowing occluder were all invisible headless). Capture a PNG / run the relevant example / drive the live shell every wave.
7. **Force a real recompile before trusting an LSP/`E0583`/proc-macro "error"** (the view/BSN prototypes hit stale rust-analyzer signals every wave — trust `cargo`, not the analyzer).

### Standard gate (SG) — every PR, headless, green before opening

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --locked -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked
xvfb-run -a cargo nextest run --workspace --locked      # headless; NO --ignored, NO adapter
cargo deny check                                          # before ANY dep bump
```

If the test step link-OOMs under full `mold` parallelism, add `-j 2` (CLAUDE.md). `--locked` is mandatory (a needed lockfile change is a real failure to surface).

### Render gate (RG) = SG **plus** the GPU two-leg `#[ignore]` lane (both legs, on a real adapter / pinned lavapipe)

```sh
cargo test -p buiy_core   -j 2 -- --ignored --test-threads=1     # render GPU path
cargo test -p buiy_verify -j 2 -- --ignored --test-threads=1     # visual-bug verification suite
```

Both legs must pass on a GPU host; the headless SG must stay green **without** an adapter (CI has none). Local golden blessing uses the pinned-lavapipe reconstruction recipe (memory `buiy-verify-followups-campaign`: Mesa 24.3.4 + Arch-archive libLLVM18/libxml2.so.2/icu75). **RG applies to F1, F3, F4a, F4b** (they carry GPU goldens per spec §4.1) and defensively to **F9** (it edits the render scale-factor seam). F2/F5/F6/F7/F8 are **SG-only** (no goldens — spec §4.1).

---

## Wave 0 — precondition: lineage docs + CI env decision

**Not a framework change.** Two deliverables that must be settled before F1 opens.

**0a — the lineage-docs PR (hard merge precondition, spec front-matter + §8 finding H2).** Branch `docs/dooduel-campaign-lineage` (exists, assembly in flight). Land on `main`, before or with the spec, the docs the spec's relative links point at, or **every seed link in the spec dangles**:
- the retrospective, journal, and playtest evidence folder (`docs/prototypes/2026-07-02-dooduel-PROTO1-*`);
- the charter + amendment (`docs/prototypes/2026-07-01-scribbl-campaign-charter.md`);
- the rebaseline audit (`docs/reports/2026-07-02-dooduel-rebaseline-audit.md`) — **stamp the superseded-by back-pointer onto it** (this spec supersedes its build list, spec §1.2);
- the `reference-designs/dooduel/` bundle + `REQUIREMENTS-DELTA.md`;
- the `docs/README.md` Dooduel-area *prototype lineage* line resolves (drop the "lives unmerged" note once landed).
- **Gate:** docs-only; SG's `fmt`/`clippy`/`nextest` still run (no code change → trivially green); confirm no dangling relative links (grep the spec's `../` links resolve). **Merge gate: human.**

**0b — the CI env decision (spec §3.4 finding N2).** The build-env pin is **ratified here, applied in App-1** (the `apps/dooduel` CI job does not exist until the app lands, so the `env:` cannot be added earlier without its job). Decision recorded: App-1's new `ci.yml` job for `apps/dooduel` carries, as a **job-level `env:`**:
- `RUST_MIN_STACK: "33554432"` (32 MiB — rustc SIGSEGVs during monomorphization of the large bevy app bins otherwise, retro §4.10);
- `CARGO_BUILD_JOBS` capped (e.g. `4`) if the host link-OOMs (the standing `-j 2` discipline).
Mirror both in the App build docs so a local build matches CI. (No `ci.yml` edit in Wave 0 itself — this is the decision, landed as the App-1 job.)

---

## Framework waves (F1–F9)

Each wave: **worktree/branch** off the latest `origin/main`; **build steps**; **gate**; **PR title**; **what Dooduel dogfoods**; **merge gate: human**.

### F1 — raster / image-node primitive + the bordered-rounded regression guard
- **Worktree/branch:** `.claude/worktrees/dooduel-f1` · `feat/dooduel-f1-raster` (off `origin/main`).
- **Scope (spec §2.1):** the app's one net-new subsystem — a texture-presenting node primitive. Re-implement clean from prototype `646aabc`+`22c3a91`.
- **Build:**
  1. `RasterImage(Handle<Image>)` component + a `RasterInstance` (48 B) record + a **distinct `BuiyRasterPipeline`** (own WGSL: `@group(0)` view uniform + `@group(1)` per-node texture + Nearest sampler) + extract→prepare→draw glue, wired into `BuiyViewPipelines`/`BuiySpecializedPipelines` + a draw section in `buiy_pass`. **Mirror the band/gradient precedent — a distinct pipeline keyed by record, NOT a new `BuiyPrimitiveKind`** (the closed `{ Shadow, Quad, Glyph, Path }` enum + byte-stable quad path stay untouched; `Path` stays reserved).
  2. CPU-authoritative buffer: `Image` (`Rgba8UnormSrgb`, `RenderAssetUsages::all()` so main-world `data` survives the render-world clone — document the `RENDER_WORLD`-only `data.take()` trap); paint into `data`, mark `Modified` → re-extract → re-upload.
  3. **Bump the render system-count meta-test constants** (`render_smoke`/`render_prepare`/`render_compositor` `== 5` → `== 6` extract systems) — in this same PR (the prototype left them red).
  4. Add a **bordered-rounded fixture** to `buiy_verify` (a widget card: rounded fill + visible border). The **standing regression guard is the existing Tier-4 SDF cross-check** (`run_sdf_cross_check`, `buiy_verify/src/reftest.rs:168` — zero stored bytes), **not** a stored golden (finding H1).
- **Gate:** **RG.** Layout snapshot (element in tree, fixed size) + display-list snapshot (`RasterInstance` extracts) headless; **Tier-4 SDF cross-check** for the bordered-rounded fill corner; GPU goldens (both legs) = byte-exact brush/fill/eraser readback (`[220,40,40]` red, filled green, erased→paper) + **one** bordered-rounded establishment capture. RUN: a raster smoke (paint into a canvas, capture, eyeball).
- **PR title:** `feat(render): raster/image-node primitive (distinct pipeline) + bordered-rounded SDF guard`
- **Dooduel dogfoods:** the game drawing canvas (720×450) and the avatar-editor raster (220×220).

### F2 — the one coherent view layout surface
- **Worktree/branch:** `.claude/worktrees/dooduel-f2` · `feat/dooduel-f2-layout` (off `origin/main`; **LANE A — build ‖ F1**).
- **Scope (spec §2.2, the centerpiece):** design the whole view layout surface in ONE pass. Re-implement clean from `6ef6df5` (redesigned, not ported piecemeal). Every modifier is a **drift-only (`set_if_neq`/`!=`-guarded) write** in `apply_container_props`, folding into the existing `changed` count so an unrelated fold never re-uploads and existing snapshots stay byte-identical. `FlexItem` etc. inserted-on-demand + toggled by neutral value (no `RemovedComponents`).
- **Build — the v1 modifier set (each with its lowering, spec §2.2 tables):**
  - Sizing: `.width/.height`, `.min_/.max_width/height`, `.fill()` → both axes `Length(Percent(100))` (**not** `Sizing::Stretch` — that is the cross-axis keyword and would not fill a root against the viewport), `.fill_width()/.fill_height()` (per-axis, new).
  - Flex: `.grow()/.grow_by()`, `.shrink(bool)/.shrink_by()`, `.justify_*` (complete the 6-value `Justify` facade), `.align_*` (`Align` facade incl. `_stretch`), `.wrap()` (new).
  - Spacing: `.padding()`, `.padding_xy()` (new), `.padding_top/_right/_bottom/_left()` (new), `.gap()`.
  - Text: `.text_align(TextAlign::{Start,Center,End,Justify})` (new) → the text-layout inline align.
  - Positioning: `.top_layer()`; **`.fixed()` re-anchored to the viewport/ICB** — a **`buiy_core` layout-semantics change** (`Position::Fixed` today re-parents to the *root content box*, `layout/systems.rs:2878-2885`); ships with a **layout-snapshot test proving `.fixed()` resolves to `(0,0)` regardless of root padding**. `.absolute()` + `.inset()`/`.inset_top()` etc. (new). `.center_self()` (new — absolute + inset-0 + margin-auto; **distinct** from centering a container's children, which stays `.justify_center().align_center()`).
  - Scroll (the #1 gap): `scroll_column(...)`/`.scroll_y()` → `Overflow::Scroll(y)` + a **controlled stick-to-bottom** (`ScrollOffset` is runtime-input-owned, `layout/components.rs:509-526`) — **the model owns a `stick` intent; the reconciler drift-asserts `ScrollOffset = max` only while `stick`; the scroll-input handler clears `stick` on scroll-away**. Designed jointly with F5's interaction-state layer (same runtime-state↔pure-view class). `.scroll_x()` for the mobile scoreboard strip.
  - The **`raster()` element** (`raster(handle, w, h)` → `Node` + `RasterImage` + fixed size, default `.shrink(false)`; **handle patched by identity**, entity preserved across unrelated folds — a reconciler test asserts this).
  - **Deferred from v1 (spec §2.2):** `.rotate()` (→ F4b attempt-if-cheap), `.ignore_picking()` (→ F6), all styling modifiers (→ F3), the mobile bottom-sheet (phone-parity follow-up).
- **Gate:** **SG.** One **layout snapshot per modifier** (resolved rect/flex/position/overflow — geometry is observable pre-raster, no goldens). Required behavioral tests: `.fixed()`-vs-viewport anchor; the **controlled stick-to-bottom** (append N *while sticking* → `ScrollOffset==max`; scroll-away → `stick` clears → a further append does NOT move the offset). Text-align: a layout/text snapshot (glyph-run x-origin). `raster()` identity-patch reconciler unit test. RUN: an example exercising wrap + per-side padding + scroll.
- **PR title:** `feat(view): the coherent layout surface (sizing/flex/spacing/text-align/position/scroll) + raster() element`
- **Dooduel dogfoods:** the 16-swatch toolbar (`.wrap()`), the 60px top bar (`.padding_top`), centered copy (`.text_align`), per-seat corner badges (`.absolute().inset()`), pixel-centered modal (`.center_self()`), the chat + scoreboard scroll, the `.fixed().fill()` scrim, the canvas `raster().shrink(false)`.

### F7 — the MVU clock ergonomic
- **Worktree/branch:** `.claude/worktrees/dooduel-f7` · `feat/dooduel-f7-clock` (off `origin/main`; **LANE A — build ‖ F1; a different subtree (`buiy_core::mvu`) from the render line**). **net-new** (the prototype hand-rolled it; F7 generalizes the proven pattern).
- **Scope (spec §2.8):** lift the hand-rolled game clock into a reusable primitive — as a **poll-clock-as-Msg, NOT an edge-triggered timer** (the prototype proved edge-triggered `Cmd::interval`/`Cmd::timeout` is the wrong shape).
- **Build:**
  1. `ClockPlugin<M>` / `Cmd::tick_every` — a runtime poll clock enqueuing `M::tick(now)` every frame; the reducer derives countdowns/hints/phase-timeouts/bot-fires from `now − anchor`. **Store only DERIVED values, never raw `now`** (so a steady frame folds byte-identically and `set_if_neq` absorbs it — level-triggered, replayable). `now` enters the log only as the `Tick(now)` payload (MVU §8 invariant holds).
  2. `on_submit_with(fn(String) -> Msg)` — the capturing variant of `on_submit` (mirrors `on_input_with`), deleting the two-message `SetChatInput`→`SubmitGuess` dance.
  3. Headless virtual-clock advance harness (`Time::advance_by`) — `BuiyHeadlessPlugin` omits `AnimationPlugin`, so a headless loop has ~µs delta; provide a **virtual advance** instead of real sleeps.
- **Gate:** **SG.** Headless test injecting `Tick(now)` at chosen `n` (instant full-match sim, zero wall-clock flakiness) + the **`set_if_neq` steady-frame no-cascade gate** (on a steady tick, `models_mutated == 0 && node_rebuilds == 0`; a model wrongly storing raw `now` FAILS). RUN: a headless countdown sim.
- **PR title:** `feat(mvu): ClockPlugin<M> / Cmd::tick_every poll-clock + on_submit_with + headless virtual-clock advance`
- **Dooduel dogfoods:** the whole game clock (round countdown, hint reveals, phase timeouts, bot fires); the typed-guess submit.

### F8 — the playtest driver
- **Worktree/branch:** `.claude/worktrees/dooduel-f8` · `feat/dooduel-f8-driver` (off `origin/main`; **independent — merge any gap**). cherry-pick-clean from `f92cb04`.
- **Scope (spec §2.9):** the framework's missing stroke/drag helper + the unified headless driver recipe (pure additive `buiy_verify`).
- **Build:**
  1. `PointerHarness::stroke(path)` / `drag(from, to, steps)` + a reusable `drive_stroke(app, window, pointer, path)` free fn — press at `path[0]`, a `PointerAction::Move{delta}` per subsequent point (updating `PointerLocation` coherently via `PointerInput::receive`), release at the last — so `bevy_picking` derives `DragStart → Drag → DragEnd` on the press target. **The first headless driver of bevy's drag machine** (prior harness tests used `move_to`, which emits no `Move` and never drags).
  2. The unified headless driver recipe: `BuiyProbePlugin` + `bevy::picking` + `buiy_core::picking` + the backend compose in one App. **Bake in `app.init_asset::<Image>()`** (a headless canvas host has no `ImagePlugin`/`RenderPlugin` to register `Assets<Image>`; else an opaque "Resource does not exist" panic under `debug=0`) — either document the line or have `CanvasPlugin` register defensively (`is_plugin_added`-guarded).
- **Gate:** **SG.** `pointer_drag` test (N-step drag → exactly 1 `DragStart` + N `Pointer<Drag>` with correct `delta` + 1 `DragEnd`, all on the press target) + a test that `move_to` provably does NOT drag + a canvas e2e (input → funnel → paint observers → ink in the buffer, no GPU).
- **PR title:** `feat(verify): PointerHarness stroke/drag driver + unified headless canvas recipe`
- **Dooduel dogfoods:** the 4-agent playtest strokes; the headless canvas tests.

### F3 — the view styling surface + borderless-rounded fill
- **Worktree/branch:** `.claude/worktrees/dooduel-f3` · `feat/dooduel-f3-styling` (off the `origin/main` that **already contains F1 + F2**). Render line: F1 → **F3-fill** → F4a. Re-implement clean from `fc2a72b`.
- **Scope (spec §2.3):** everything that makes a node *look* like the design, plus the one render change (borderless-rounded fill) without which styled pills render square. **Crate-boundary crossing ratified (spec §5.e (i)):** the `buiy_core` fill fold rides this mostly-`buiy_view` PR (small, self-demonstrating — the styled pills actually render rounded in the same PR).
- **Build:**
  - `Color::Custom(u8,u8,u8,u8)` + `Color::rgb(u8,u8,u8)` → `ColorToken::Custom` (exact-sRGB escape) **and grow the semantic facade** to the protokit token roles (`Color::Surface2` etc., theme-backed); accent stays semantic (`Color::Accent` + a `SetAccent` startup write). The two-layer seam (app `Palette` + `Theme` resource) is validated.
  - `.color(Color)` → `TextColor`; `.font(family)` → `FontFamily` (sfnt-only ttf/otf/ttc, `include_bytes!`-embedded; the registered family string must equal the font's internal nameID-16 family); `.weight(FontWeight)` → the variable-font weight axis (**new** — the prototype rendered all text at the default instance).
  - `.border(width_px, Color, LineStyle)` → `BoxModel.border` + 4-side `Border` (**new `LineStyle` arg** — makes dashed *requestable*; the rasterization is F4b). `.radius(Radius)` + `.radius_corners(tl,tr,br,bl)` → `Border.radius`. `.shadow(dx,dy,blur,spread,color)` → `BoxShadow` (front-to-back CSS order).
  - `icon(path_d, size_px, stroke_width, viewbox)` element (**new `viewbox` arg** — removes the per-app 0.6 coord/stroke scale for the design's 40×40 viewBox); the same node also carries `.background` + `.radius` (one node = tinted badge + stroked doodle).
  - **Styleable buttons** — style modifiers apply to `Kind::Button` **gated on "explicitly styled"** so an unstyled `button("x")` keeps every `buiy_widgets::Button` default (the §4.1c suppression safety — keeps counter/gallery goldens byte-identical).
  - **Borderless-rounded fill (`buiy_core`):** complete the stubbed `pack_extracted` `radius: 0.0` (`render/instance.rs:116,131`) — **add `ExtractedNode.radius`**, resolve from `Border.radius` **only when no border side paints**, min-clamped `≤ min(half_w, half_h)`, packed into the existing `PackedInstance.radius` slot. A **bordered** node keeps `radius 0` → every existing snapshot + golden byte-identical. **This field is what F4b's bordered-rounded fill reuses (the F3→F4b edge).**
- **Gate:** **RG.** Display-list snapshots (color/font/weight/border/radius/shadow) + **Tier-4 SDF cross-check** for the borderless-rounded fill corner; **one** GPU golden = the first-establishment fill capture, byte-identical to pre-change goldens for every existing fixture (the borderless-only gate). RUN: capture styled pills/cards/icons, eyeball vs the design.
- **PR title:** `feat(view,render): styling surface (color/font/weight/border/radius/shadow/icon) + borderless-rounded fill`
- **Dooduel dogfoods:** the protokit color ladder, Caveat/Shantell + weights, the room-code dashed border (requestable), rounded pills + wobble radii, card + 3D-press shadows, the doodle icon badges, styled buttons.

### F4a — paint order: the general per-raster-anchor interleave
- **Worktree/branch:** `.claude/worktrees/dooduel-f4a` · `feat/dooduel-f4a-interleave` (off post-F3 `origin/main`; render line). Re-implement clean, **SUPERSEDES** `2854643`.
- **Scope (spec §2.4):** retire the prototype's top-layer-suffix split. **Interleave each raster into the flat quad draw by its OWN `node_quad_anchor`** (the exact gradient-bleed precedent) — a raster paints at its true stacking position, no top-layer special case, no contiguous-suffix assumption. Retires (a) non-top-layer overlay-under-canvas and (b) raster-under-**opaque**-modal at once.
- **The boundary this does NOT cross (finding #5 — document it):** the fix reaches only the **main flat pass**. A raster nested in a top-layer container that is itself an **effect group** (`Opacity<1.0`/backdrop-filter) is in the **off-screen group pass, which composites quads+glyphs only — a raster there is DROPPED**. So the fix unblocks a raster inside an **opaque** modal panel, not a translucent/blurred one. **App consequence (carried into App-1):** the avatar-editor modal **PANEL must be opaque** (a translucent scrim sibling is fine — it carries no raster). Effect-group-nested raster = a **documented follow-up**, not v1.
- **Build:** `extract_buiy_rasters` already carries each raster's node; `prepare` maps each to its quad-instance anchor; `node.rs` splices a per-raster `FlatDrawStep::Raster(anchor)` instead of one global `Rasters` marker. No app-facing API change.
- **Gate:** **RG.** **Paint-order/display-list snapshot** (each raster splices at its anchor; a **non-raster view is byte-identical** — the `None`/sentinel path reproduces the old draw exactly) + the prototype's 6 interleave unit tests. GPU goldens (both legs) for the two unblocked cases: raster-over-and-under an overlay; raster-in-a-top-layer-**opaque**-modal (visible over the panel). **Attack:** the general interleave must not regress the byte-stable non-raster path.
- **PR title:** `feat(render): general per-raster-anchor paint-order interleave (supersedes the top-layer split)`
- **Dooduel dogfoods:** the canvas painting at its true stacking position; the avatar editor as an opaque modal-with-a-raster (a parity improvement over the prototype's full-screen workaround).

### F4b — shape / decoration render fixes
- **Worktree/branch:** `.claude/worktrees/dooduel-f4b` · `feat/dooduel-f4b-shape` (off post-F4a `origin/main`; render line tail). Re-implement clean from `fb4cc18`. **Needs F3's `ExtractedNode.radius`.**
- **Scope (spec §2.5; §5.f resolved — dashed + crisp shadow are BOTH v1):** the remaining render parity fixes. Materially smaller than first believed (finding #2): the lens is shader-only, the fill reuses an existing slot, only the shadow corner radius needs a new instance.
- **Build (each a rasterization-residue fix):**
  1. **Bordered-rounded fill ("ears"):** pack the fill's uniform radius to the border band's **inner radius** (min of per-corner inner radii) — EXACT for a uniform border, byte-identical for a square box. **Reuses the existing `PackedInstance.radius` slot** on a node set **disjoint** from F3's (bordered vs borderless) — no stride change.
  2. **Per-corner band radii + the radius LENS — SHADER-ONLY (`band.wgsl`), zero stride.** `BorderBandInstance` **already** carries `outer_radius:[f32;8]` + `inner_radius:[f32;8]` (`(rx,ry)×4`, `render/instance.rs:248-252`); the lens is a shader bug (`band.wgsl` read `outer_radius_tl_tr.x` for all corners). Fix: select each corner's `(rx,ry)` by quadrant (standard per-corner rounded-box SDF). Byte-identical for uniform radii.
  3. **Dashed / dotted borders:** a **screen-space arc-length stipple** in the band shader (`LineStyle::Dashed` renders solid today). **The heaviest sub-item — the pre-identified split cleave** (risk R1): if the F4b diff is too large to review as one, dashed splits into its own PR (`feat(render): dashed/dotted border stipple`).
  4. **Rounded clip for `raster`:** add a corner-radius clip to the raster shader (custom avatars currently render as square stickers).
  5. **Per-quad particle alpha:** a **cheap per-quad alpha** fade that does NOT promote to an `EffectGroup` (a per-particle `OpacityTween` would spin up ~110 off-screen targets for confetti).
  6. **Shadow corner radius — the ONE new-channel item; §5.f resolved it IN.** `pack_shadow` reuses `PackedInstance.radius` as the blur sigma (`render/instance.rs:174-185`), so a shadow corner radius has nowhere to live. Build **Option B — a dedicated rounded-shadow instance** (distinct record + pipeline, the band/gradient/raster precedent; keeps the byte-stable quad path untouched). It renders the **crisp zero-blur 3D-press "sticker" edge**. **Do NOT widen the shared `PackedInstance`** (Option A — ripples through the byte-stable quad+shadow+text path; rejected §2.5.1/§7).
  7. **Attempt-if-cheap during F4b's character pass, defer-with-note otherwise (§5.f):** per-axis elliptical corners (shader-only-away but invisible at ±3px wobble), radial-gradient backgrounds (flattened), the rotated ribbon (`.rotate` — the transform/tween bridge drives `Rotate` cheaply). Record each residual with a note if it exceeds a small budget.
- **Gate:** **RG.** **Tier-4 SDF cross-check** for rounded-fill corners (standing guard, zero stored bytes). **GPU goldens (both legs) scoped to what the cross-check can't see:** the **band lens** (the SDF oracle fixture is a *zero-width band* — it exercises the fill SDF, not the band SDF), **dashed stipple**, **raster rounded clip**, **shadow blur AA + the crisp-edge radius**, **per-quad particle alpha**. Byte-identical for every pre-change fixture (uniform-radius / no-border / opaque). **Implementation review must attack** whether the dedicated rounded-shadow instance composes correctly with the F4a interleave + the effect-group compositor (risk R4). RUN: capture pills/room-code box/rounded avatar/3D-press/confetti, eyeball.
- **PR title:** `feat(render): shape/decoration fixes — bordered-rounded fill, band lens, dashed stipple, raster clip, particle alpha, rounded-shadow instance`
- **Dooduel dogfoods:** the bordered-rounded pills (no ears), full-radius pills (no lens), the dashed room-code/join box, round custom avatars, the confetti fade, the crisp 3D-press.

### F5 — press routing generalization + the interaction-state visual layer
- **Worktree/branch:** `.claude/worktrees/dooduel-f5` · `feat/dooduel-f5-press` (off `origin/main`; **needs F2**). Re-implement clean from the framework parts of `f4357af`.
- **Scope (spec §2.6):**
  1. **KEEP: pressable `Icon`** — `icon(...).on_press(msg).label(name)` → an activatable a11y button (`A11yRole::Button` + `A11yLabel` + `PressAction`), routing through BOTH a real pointer click (`pointer_click_emits_on_press`, role-gated) AND a probe/AT `Action::Click` (the Button contract is role-keyed — `contract_for(Button).honor(Click)` emits `OnPress`, no `Button` component).
  2. **NEW: container / raster press route** — a clickable container (children intercept the hit, carry no role) or a pressable `raster` becomes activatable; the click lands on the container itself. Forcing cases: the pick-word tiles (clickable containers), the custom-avatar raster seat chip.
  3. **NEW: the interaction-state visual layer (finding #1 — `ControlledLeaf` is NOT the vehicle).** A `view` is a pure `fn(&Model)->Element`; a transient pressed/hover style is ephemeral non-model state. Build a **net-new widget-runtime-owned `InteractionState` (hover/press/none)** written on pointer enter/press, consumed by a **press-style resolver OUTSIDE the pure view** (the same place `buiy_widgets::Button` would own it — it carries zero press-visual machinery today, `button.rs:181-188`). This is the **same runtime-state↔pure-view class as F2's scroll stick-to-bottom** — design them together. **Risk R3 (attack at the gate):** does it need a full hover/press/focus transition engine, or does a discrete-state resolver suffice?
- **Gate:** **SG.** The container/raster press route → the **live-interaction tier** (real shell + picking + synthetic clicks — a headless snapshot can't see pick occlusion) + an a11y-probe test (the role-keyed route). The interaction-state layer → a **live-interaction press test** (a real synthetic press applies the pressed style, reverts on release) + a headless `InteractionState`-transition unit test. RUN: click a pick-tile + a seat chip; watch a button press down + release.
- **PR title:** `feat(view): container/raster press route + widget-runtime interaction-state visual layer`
- **Dooduel dogfoods:** the pick-word tiles, the custom-avatar seat chip, the press-down 3D-press animation (spec §5.f — a v1 parity requirement).

### F6 — picking safety + the native-pointer live-interaction test tier
- **Worktree/branch:** `.claude/worktrees/dooduel-f6` · `feat/dooduel-f6-picking` (off `origin/main`; **needs F2**). Re-implement clean from the framework part of `9237822`. **F6 *is* a verification tier.**
- **Scope (spec §2.7 — make the invisible-occluder class unwritable by construction):**
  1. **PRIMARY (structural): auto-`Pickable::IGNORE` for transparent (`Color::NONE`) top-layer containers** — a transparent top-layer container (the prototype's floating theme toggle) that sat topmost + occluded every click becomes auto-transparent-to-picking. The bug is **unwritable** — no opt-in to forget. (Discipline-based gating is the failure mode that shipped this bug **three times**; structure is the fix.)
  2. **`.ignore_picking()` (`Pickable::IGNORE`)** — the explicit escape for the non-transparent case (a node transparent to picking while its interactive CHILDREN stay pickable). Opt-in, default no-op, byte-identical snapshots.
  3. **A Tier-3 invariant/proptest over the fixture catalog:** *"no transparent top-layer container is a pick occluder"* — fails the moment a fixture re-introduces the class, independent of anyone remembering to click.
  4. **The native-pointer live-interaction test tier (the backstop)** — real synthetic pointer clicks against the running app on the **DEFAULT (headless) lane** (NOT the GPU lane; NOT a11y-clicks, which action by role+label and bypass the exact pick occlusion this class lives in). The **default gate for any `.top_layer()`/transparent-container change.**
- **Gate:** **SG.** The Tier-3 invariant runs headless over the fixture catalog; the live-interaction self-test: a transparent top-layer container over a button → a real synthetic click penetrates to the button and folds its Msg (the occluder guard + the auto-`IGNORE` regression). `.ignore_picking()` byte-identical for every existing snapshot. RUN: drive a click through a transparent overlay onto the control beneath it.
- **PR title:** `feat(view,verify): auto-IGNORE transparent top-layer occluders + .ignore_picking() + native-pointer live-interaction tier`
- **Dooduel dogfoods:** the floating theme toggle no longer swallows clicks; `.ignore_picking()` for any decorative overlay.

### F9 — the HiDPI / web scale-factor investigation
- **Worktree/branch:** `.claude/worktrees/dooduel-f9` · `feat/dooduel-f9-hidpi` (off the latest render-line `origin/main`; **lands IN the serial render line** — edits the `buiy_core` scale-factor seam). **net-new investigation.** **Start the investigation EARLY (parallel), land it in the render line.**
- **Scope (spec §2.10):** the prototype's top mobile residual — a focused root-cause, not a pre-decided fix. **The bug:** at `devicePixelRatio > 1` the UI renders scaled ~dpr× and overflows dpr-proportionally; a dynamic window resize also mis-sizes the surface (likely the same scale-factor/surface-reconfigure seam). The responsive *logic* is correct (reads logical `window.width()`); the bug is in render/layout scale-factor handling on the wasm path. **Blocks the mobile acceptance criterion (risk R2, unknown depth).**
- **Build:** reproduce at dsf=2/3, root-cause, fix, verify. Could be a shallow scale-factor plumbing miss or a deep surface-reconfigure seam — scope is *investigate → fix → verify*, not a pre-committed change set.
- **Gate:** **RG (defensive — edits render scale-factor) + the headless HiDPI proxy FIRST (finding M4):** construct the app with a `Window` at `scale_factor = 2` and assert the resulting `ResolvedLayout` fits the **logical** viewport (no overflow) — a headless, CI-able reproduction that de-risks the fix before a browser. **Then** the browser check (both backends) at dsf=1/2/3 — the browser + real-device legs are **manual milestone gates** (§4.4/§4.5), verified in the Acceptance wave, not per-wave CI. The headless proxy is the per-wave CI form.
- **PR title:** `fix(render): HiDPI / web scale-factor overflow at devicePixelRatio > 1`
- **Dooduel dogfoods:** mobile phones at dpr>1 render correctly (the mobile v1 target).

---

## App waves — `apps/dooduel`

The app is built **on the fully-landed framework surface** (all of F1–F9 merged). Two PRs.

### App-1 — the `apps/dooduel` port (native app + web crate + CI job)
- **Worktree/branch:** `.claude/worktrees/dooduel-app1` · `feat/dooduel-app-port` (off the `origin/main` that contains F1–F9).
- **Scope (spec §3.1, §3.2 — `apps/dooduel` ratified §5.d):** graduate the app out of `examples/` into a new top-level `apps/` tree; **restructure, don't keep-shape** (the prototype's 3082-line `lib.rs`), keeping the single-model / single-`ui()` shape.
- **Build:**
  1. **Workspace:** add `apps/*` (or `apps/dooduel`, `apps/dooduel/dooduel_web` — whatever the crate layout resolves to) to the root `Cargo.toml` `members`. `examples/` keeps only framework examples (untouched).
  2. **The module split (spec §3.2):** `lib.rs` (the MVU `Dooduel` model + `Msg` + thin reducer + `ui()` install + `install_runtime`); `game.rs` (the **PURE game core** — port near-verbatim: phase machine, scoring, words, hints, guess normalize/levenshtein/is_close, seeded bots, the honest `word_display()` redaction accessor — zero framework coupling, unit-testable with no ECS/GPU/clock); `paint.rs` (keyed `PaintCanvases` resource + model-owned-tool→canvas projection + monotonic `*_seq` imperative ops + Press/Drag/Release observers); `storage.rs` (typed per-target load/save seam + avatar PNG-encode+base64 + `saved_version` counter); `theme.rs` (the `Palette` LIGHT/DARK ladders + `ThemePref` + `sync_theme_resource` + font names + accent); `avatar.rs` (22 doodles + `hash_str` + `doodle_avatar`); `confetti.rs` (the decoupled `ConfettiPlugin` rising-edge side effect); `view/` (`mod.rs` Screen router + `widgets.rs` shared helpers + `home/join/lobby/in_game/podium/avatar_editor.rs`); `bin/playtest_host.rs`.
  3. **Load-bearing patterns to KEEP (spec §3.2):** the pure `Game` core; **bots-as-emits** (a bot guess = a real `Msg` in the same drain); the **rising-edge side-effect observer** (confetti / per-turn canvas clear / theme sync / persist READ the model via a `Local` latch, never enqueue); model-owned tool state projected model→canvas each frame; the keyed `PaintCanvases` resource (survives despawn-on-screen-swap); the honest per-seat view (clone `Game`, set `viewing_as=i`, reuse `word_display()`). The **phase machine goes in MVU** (the Menu-machine precedent — `bevy_state` is not compiled into the workspace).
  4. **Canvas lifecycle pins (finding #21):** the game canvas **clears on entry to the Drawing phase** (the `chooseWord` transition) via the rising-edge observer reading `Game.phase`; and **re-inits/refits on a viewport-mode change** (desktop↔mobile, 460 vs 280px canvas) so the `PaintSurface` is re-sized and in-progress pixels re-fit rather than clipped.
  5. **The avatar-editor modal PANEL is opaque** (F4a boundary consequence — a translucent scrim sibling is fine).
  6. **Web:** the `dooduel_web` crate (dual-backend `webgpu|webgl2`, canvas-bound window) + `install_runtime` + `ViewportPlugin`. **Mobile = a viewport-width-driven PROP swap** at **≤430px** between two hand-authored layouts (finding #19 — NOT a Protokit shell reflow): the mobile layout is a single stacked column (top bar → header card with a small timer ring radius 12 vs desktop 26 → horizontally-scrolling scoreboard strip → ~280px canvas → stacked toolbar → chat), and the avatar editor is a **bottom SHEET**. Touch drawing / touch-tap activation / soft-keyboard via `WebImePlugin` carry over.
  7. **The CI job (Wave 0b applied):** add an `apps-dooduel` job to `ci.yml` with `env: RUST_MIN_STACK: "33554432"` (+ `CARGO_BUILD_JOBS` cap if it link-OOMs). Any headless canvas host bakes the `init_asset::<Image>()` line (F8).
- **Gate:** **SG** (the app is now a workspace member, so the standing gate builds/tests it) + `game.rs` unit tests (pure-core, no ECS) + a `dooduel_web` wasm build (both backends). **RUN all three targets** — native desktop, web-desktop (both backends), and view each of the 6 screens; this is where headless-invisible bugs surface.
- **PR title:** `feat(apps): apps/dooduel — pure game core + module split + native/web on the landed framework surface`
- **Note:** gameplay bugs #1–#6 are still present here — App-1 is the faithful port; fixes are App-2.

### App-2 — the gameplay-bug fixes + the graduated `playtest_host`
- **Worktree/branch:** `.claude/worktrees/dooduel-app2` · `feat/dooduel-gameplay-fixes` (off post-App-1 `origin/main`).
- **Scope (spec §3.3, §3.4):** the 6 playtest-found gameplay-bug fixes (each with its targeted test — the tests the playtest showed were missing) + graduate the file-protocol host.
- **Build — the 6 fixes (spec §3.3 table), each with the named targeted test:**
  1. **Round-counter overflow:** set `total_rounds` from config **at match start** (not a stale default); clamp so `round` never exceeds `total`. **Round strings vary by surface (finding #20):** desktop header **"Round {r} / {t}"** (slash); mobile header + system chat **"Round {r} of {t}"** (word). *Test:* at `Final`, `round_display()==total` and never exceeds; pre-start `total_rounds==config.rounds`; both string forms render on their surfaces.
  2. **Stale drawer at podium:** clear `drawer` on the transition to `Final` (or gate the "(drawing)" tag on `phase==Drawing`). *Test:* at `Final`, no seat tagged drawing; `drawer_name()` absent.
  3. **Drawer plays blind:** expose a **live** `guessed_count()`/`all_guessed()` accessor visible in the Drawing phase (render it in the drawer's header); ensure "guessed" chat lines are visible **in-phase** (they already fold — the host's per-seat view lagged, §3.4). *Test:* after K mid-draw guesses, the drawer's projected `guessed_count()==K` and the line is present in-phase.
  4. **No wrong/close feedback + no echo → the design-exact three-way (finding #18):** a **wrong** guess posts to the **SHARED chat** (guesser name + literal text — everyone sees it); a **near-miss** (Levenshtein ≤2, len diff ≤2) fires a **PRIVATE toast "So close! 👀"** (only the guesser); an **exact** match posts **"X guessed the word!"** with the word hidden from those who haven't guessed. *Test:* the three paths, each with the correct visibility.
  5. **Hint reveal — schedule already ported (`game.rs:396-414`); the gap is a test + render (findings #15/#16).** Add the **threshold-crossing test** + render verification. **Schedule (confirmed at design values, §5.b):** for `i` in `1..=hintCount` (default 2), reveal at `floor(totalDraw·(0.6 − i·0.18)).max(1)` **seconds-LEFT** (⇒ 33s & 19s at 80s draw); each a **random unrevealed `[a-z]` position**; **CROSSING semantics** (fire on first `seconds_left ≤ threshold`, latch the count — never equality; a poll can skip the exact second); capped `≤ min(hintCount, letterCount−1)`. *Test:* tick a slow-guess turn so `seconds_left` crosses 33 then 19; assert the latched count goes 1 then 2, positions are `[a-z]` and previously-unrevealed, never exceeds the cap.
  6. **Countdown units (host):** the host's `state.json` renders `seconds_left` from the game accessor (`now − anchor` wall-clock), not a tick count. *Test:* the accessor decrements ~1/s of wall time.
  - *(Not code bugs — finding #22: the canvas-all-at-once → §3.4 per-stroke streaming below; the drawer-payout deficit → the named `drawer-payout-balance` post-v1 follow-up, §5.a. Drawer payout v1 = the design-JS-literal `round(100·correctCount/guesserCount)`, 0 if none.)*
- **Build — the graduated `playtest_host` (spec §3.4):**
  - Richer `state.json`: word length, hint count, per-seat "can I act now" flags (drawer-vs-guesser), the live `guessed_count`/`all_guessed` (fix #3), `turn X / round N / total` (fix #1).
  - **Per-stroke streaming:** a per-stroke PNG flush (the transport batches at ~2 Hz → the canvas appears all-at-once; per-stroke restores the partial-art tension — a host+transport gap, NOT a UI gap; the real UI streams live).
  - In-protocol `continue` / skip-reveal (the design's "Continue →").
  - Chat streamed as a separate file (agents needn't diff the whole view; couples with fix #3's in-phase delivery).
  - **Keep the honest per-seat view verbatim** (clone + `viewing_as` + the single `word_display()` home — the only real correctness risk, word-leak, is closed by construction).
- **Gate:** **SG** + the 6 targeted unit tests (pure `game.rs`, no ECS) + a host smoke (drive a short match through `playtest_host` headless). **RUN** the app and confirm each fix live (round counter at podium, no stale drawer, drawer sees guessed count, wrong/near/exact feedback, hints reveal on the crossing, countdown at ~1/s).
- **PR title:** `fix(apps): 6 playtest-found gameplay bugs (each targeted-tested) + graduated playtest_host`

---

## Acceptance wave — the repeatable multi-agent playtest + web + full gate + closeout

Runs after App-2 lands. Not primarily a code PR — a **gate procedure**; its evidence + doc-status flips ride a small closeout on App-2 (or a standalone closeout PR).

**1. The repeatable 4-agent playtest (spec §4.3 — the headline acceptance):**
1. `cargo build -p dooduel --bin playtest_host` (with `RUST_MIN_STACK=33554432`).
2. `playtest_host --dir <shared-dir>` (env knobs: `draw 120s / pick 45s / reveal 12s` for slow agents; `bots_enabled=false` so all four seats are agent-driven).
3. Point **four independent agents** at `<shared-dir>/seat_{0..3}_view.md` + `state.json` + `canvas.png` (read) and `commands.jsonl` (append) — **each agent a different player** (via the Track-A probe). The drawer picks + strokes; the others guess; the host advances in real time.
4. **Pass criteria:** the match completes end-to-end (all rounds × turns); each agent draws once and its drawing is recognized by the others **from `canvas.png` alone**; every word is guessed; the honest per-seat views hold (drawer sees the word, guessers see blanks); the final podium is correct (round counter, drawer cleared — fixes #1/#2); no gameplay-bug regression.
5. **Preserve the run's evidence** (`commands.jsonl`, `host.log`, `state.json`, `canvas.png`, a sample seat view) as the prototype's playtest folder did — as `docs/prototypes/2026-07-03-dooduel-FINAL-playtest/` (the closeout commit).

**2. Web milestone gates (spec §4.4 — manual, run at this milestone):**
- **Both wasm backends** — `tools/build-web.sh apps/dooduel` produces webgpu + webgl2 artifacts + the `navigator.gpu` loader; both boot + paint + are interactive (WebGPU on a real adapter, WebGL2 on SwiftShader). The four harness traps documented (never `getContext` on the app canvas; drive the relative-root loader; rebuild both artifacts; cold-synthetic-click = move→settle→down→settle→up).
- **Mobile emulation** — the ≤430px prop-swap: single stacked column, small timer ring (radius 12), horizontally-scrolling scoreboard, ~280px canvas, bottom-sheet avatar editor; canvas re-inits/refits on the mode swap; touch drawing + touch-tap activation + soft-keyboard via `WebImePlugin`.
- **HiDPI** — the **F9 headless proxy is the per-wave CI form** (already gated in F9); the **hard acceptance gate is the real-device browser check at dsf=2/3** (no top/right clip, layout fits the viewport). Real-device IME (`ime_position`, touch-only policy) is a manual milestone gate too.

**3. The full-workspace gate (spec §4.5 — every standing gate):** headless `cargo nextest --workspace`; the GPU `#[ignore]` lane (**both legs**); `cargo deny check`; a `dooduel_web` wasm build (both backends); `RUSTDOCFLAGS="-D warnings" cargo doc`; `fmt`; `clippy -D warnings`. **RUN the GUI.**

**4. Closeout (the doc reconciliation, CLAUDE.md *When work lands, reconcile*):**
- Flip the spec `2026-07-03-dooduel-final-design.md` `approved` → `landed` (header + README `[approved]` → `[landed]`).
- Flip this plan `active` → `landed`.
- Add the playtest-evidence folder to the README index (Dooduel area).
- Log the deferred items where the project tracks follow-ups: `drawer-payout-balance` (post-v1 design feedback, §5.a); effect-group-nested raster (§2.4); dirty-rect partial canvas upload (§2.1 perf, measure-first); multi-App concurrency topology (risk R5); color emoji as its own future campaign (§5.f); any F4b attempt-if-cheap residuals (radial gradient / ribbon / elliptical wobble).
- Update the `buiy-scribbl-campaign` memory.

---

## Risk register (spec §6.2, biggest first) + rollback / split points

| # | Risk | Mitigation | Rollback / split point |
|---|---|---|---|
| R1 | **The serial `buiy_core` render line** (headline, finding M1) — F1 → F3-fill → F4a → F4b (+ F9) edit the same files, cannot parallelize | sequence explicitly (Merge order); keep every change additive + byte-stable (disjoint-node-set discipline) so each rebases cleanly on the prior | **Split F4b's dashed-stipple into its own PR** if the tail drags (the pre-identified cleave, §5.f) → the 14th PR |
| R2 | **F9 HiDPI unknown depth** — a web/wgpu scale-factor bug, shallow-or-deep; **blocks the mobile v1 target** and serializes with the render line | **investigate early** (parallel); the **headless proxy de-risks before a browser**; verify at dsf=2/3 on a real device at the milestone | if deep (surface-reconfigure seam), F9 can land the **headless-proxy-green fix** and defer the dynamic-resize sub-case with a note — mobile-at-load is the hard criterion, live-resize is secondary |
| R3 | **The interaction-state visual layer (F5) — depth unknown (finding #1)** — net-new (`ControlledLeaf` is NOT the vehicle; `Button` has zero press-visual machinery) | designed jointly with the F2 scroll stick-to-bottom (same runtime-state↔pure-view class); **the gate must attack** whether a discrete-state resolver suffices vs a full transition engine | if a full transition engine is needed, ship the **discrete-state resolver** (resting + pressed) for v1 and defer hover/focus transitions with a note — the 3D-press-down is the v1 parity requirement |
| R4 | **F4b's dedicated rounded-shadow instance** (§2.5.1 Option B) — must compose with the F4a interleave + the effect-group compositor | **implementation review must attack** its composition; it mirrors the band/gradient/raster precedent (proven pattern) | if it does not compose cleanly with the group pass, the crisp edge is deferrable with a note (the prototype's 2px-blur softening is the fallback) — but §5.f wants it in v1 |
| R5 | **Multi-App topology unretired** — the single-model file-protocol host MET the bar, but concurrent Apps in one process are unproven | the **single-model host IS the v1 playtest topology**; multi-App concurrency is a **deferred, separately-scoped investigation** — do NOT block v1 | flag as a residual (closeout) so a networked future doesn't discover it late |
| R6 | **Emoji strategy** — the coverage rasterizer (monochrome R8) can't do COLR/CBDT/sbix | **RESOLVED for v1 (§5.f): color emoji DEFERRED as its own future campaign** — not a Dooduel sub-item | n/a (retired for v1; an outline-emoji font through the existing coverage path is the cheaper option when that campaign runs) |
| R7 | **Two crate-boundary crossings (§5.e)** — F3 folds borderless-rounded fill (`buiy_core`) into a `buiy_view` PR; F2 reaches `buiy_core` for the `.fixed()` anchor | both **ratified as specced** (small, well-isolated); each proven by its own test (fixed-vs-viewport; the borderless-only golden gate) | splitting either into its own `buiy_core` PR is one merge-cadence decision away — do it if a reviewer objects |

**Additional standing hazards (cross-cutting rules above):** snapshot entity#-brittleness (rule 4 — Name-tag or verified-id-only-diff); origin/main drift (rule 1 — rebase before every PR); MT-executor snapshot fragility (adding an `Update` system can flip a hidden-node snapshot — fold widget-visual paint into an existing system, memory `buiy-mt-snapshot-schedule-fragility`); headless-green ≠ works (rule 6 — RUN the GUI every wave).

---

## Self-review

- **Realizes** the spec's rev-3 sequencing (§6.1): the two parallel build lanes (F2‖F7), the serial render line (F1→F3-fill→F4a→F4b, F9 in-line), F5/F6 gated on F2, F8 independent — as the canonical merge order.
- **Every §5 decision is carried:** `apps/dooduel` (§5.d), per-PR cadence with pushes/PR-opening authorized + per-PR merge gate (§5.e), the F3 + F2 crate-boundary crossings ratified (§5.e), v1 parity = dashed + 3D-press + crisp rounded shadow → the dedicated rounded-shadow instance in F4b (§5.f), color emoji deferred (§5.f), podium winner-center-tallest as a documented deviation (§5.g — carried into App-1's podium view + the parity-verification notes), drawer payout design-literal + the named follow-up (§5.a), hint schedule at the design values with crossing semantics (§5.b).
- **The recurring-bug guards are mandatory tiers, not optional** (§4.2): structural auto-`IGNORE` + the Tier-3 occluder invariant + the native-pointer tier (F6); the Tier-4 SDF cross-check as the standing bordered-rounded guard with goldens scoped to what it can't see (F1/F3/F4b); side-surface version counters (App-1).
- **The lowest-tier rule is applied per PR** (§4.1): layout snapshots for F2 (no goldens — geometry is pre-raster), display-list for F3/F4a, SDF cross-check + scoped goldens for F1/F4b, live-interaction for F5/F6, headless virtual-clock for F7, headless proxy for F9.
- **Docs ship with each PR** (rule 5) and the closeout reconciles the spec/plan status + the follow-up ledger.
- **Honest scope:** the app is the faithful port first (App-1), fixes second (App-2) — so a fix's regression test has a real pre-fix RED to prove against; the framework PRs never touch `apps/dooduel`.
