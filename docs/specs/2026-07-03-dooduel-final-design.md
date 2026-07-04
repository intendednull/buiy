# Dooduel FINAL — Phase B design

**Date:** 2026-07-03
**Status:** `approved` — human gate passed 2026-07-03 (pushes + PR-opening authorized; **merges remain per-PR human-gated**). The staged-development **spec stage** of Phase B (the FINAL) of the Dooduel campaign. **Revision 3** — the human gate resolved every §5 open question into a decision, grounded against the real skribbl.io game (see §5 + the §8.2 change log). Revision 2 was the 3-reviewer-gate pass (architecture CHANGES-REQUIRED / verification CHANGES-REQUIRED / design-fidelity APPROVE-WITH-NITS); all findings were re-verified against `file:line` and applied (§8.1). NO implementation code.
**Base:** `origin/main` @ `a969cbf` — the **same base the prototype was cut from**, so the prototype's framework commits cherry-pick / re-apply cleanly.
**Worktree:** `dooduel-final` (branch `worktree-dooduel-final`).
**Precondition (gate finding H2):** the prototype **lineage docs** this spec's relative links point at — the retrospective, journal, playtest evidence, charter, rebaseline audit, and the `reference-designs/dooduel/` bundle — currently live **unmerged** on `worktree-scribbl-campaign` / `worktree-dooduel-proto1`. A **lineage-docs PR MUST land before or with this spec** (per `prototype-first-development`), or every seed link here dangles. That PR also stamps the **superseded-by back-pointer onto the rebaseline audit** (this spec supersedes its build list, §1.2). This spec is not merge-ready until that precondition is met. *(Rev-3: the lineage-docs PR is **in flight** as of the 2026-07-03 gate — §5.e.)*
**Supersedes:** nothing directly. It is the ground-up FINAL that **re-decides** every choice the prototype made — it does not port the prototype wholesale. It supersedes the rebaseline audit's "re-baselined build list" (`../reports/2026-07-02-dooduel-rebaseline-audit.md`) wherever the retrospective corrected it (see §1.2).
**Seeds (the required reading, in dependency order):**
- The prototype retrospective — the Phase-A learning gate (`../prototypes/2026-07-02-dooduel-PROTO1-RETROSPECTIVE.md`). Its §7 build strategy is this spec's PR skeleton; its §2 KEEP / §3 refine-redesign / §4 bug honor-roll / §5 DX report / §6 residual gaps are the requirements inventory.
- The prototype journal (`../prototypes/2026-07-02-dooduel-PROTO1-journal.md`, W0→W8-live) — the wave-by-wave API + file evidence behind the retrospective.
- The campaign charter + amendment (`../prototypes/2026-07-01-scribbl-campaign-charter.md`) — the contract: framework features ship as reviewed **general-purpose** PRs dogfooded by Dooduel; the FINAL re-decides; acceptance = exact design parity + verified + multi-agent playtest.
- The design bundle (`../reference-designs/dooduel/` + `REQUIREMENTS-DELTA.md`) — the parity target. **Match it exactly.**

> **Prototype lineage.** Phase A built the whole app to strong parity — native + web-desktop + web-mobile, drawing canvas, real-time game loop, theming, persistence — and ran a full 4-agent playtest to completion on the first live try. It **proved the path** (every framework gap is additive surface, not an architectural wall) and produced a bounded, character-layer residual-gap list. The prototype code (`worktree-dooduel-proto1`) is an **unmerged reference — DO NOT MERGE.** This spec is the re-decided production design: **port the validated, redesign the pressure points, ship the framework work as general-purpose PRs.**

---

## Table of contents

- **§1** Intent, provenance, and the acceptance bar
- **§2** The framework PR series (F1–F9) — scope, API-as-the-FINAL-builds-it, verification tier, audited-port disposition
  - §2.1 F1 — raster/image-node primitive + the bordered-rounded regression guard (Tier-4 SDF cross-check)
  - §2.2 **F2 — the one coherent view layout surface (specced in full: the v1 modifier set)**
  - §2.3 F3 — the view styling surface + borderless-rounded fill
  - §2.4 F4a — paint order: the general per-raster-anchor interleave (supersedes the top-layer split)
  - §2.5 F4b — shape/decoration render fixes (rescoped; the shadow-corner-radius instance RESOLVED IN — §5.f)
  - §2.6 F5 — press routing generalization + the interaction-state visual layer
  - §2.7 F6 — picking safety + the native-pointer live-interaction test tier
  - §2.8 F7 — the MVU clock ergonomic (`ClockPlugin<M>`, poll-clock-as-Msg) + `on_submit_with` + headless clock advance
  - §2.9 F8 — the playtest driver (stroke/drag helper + unified headless recipe)
  - §2.10 F9 — the HiDPI / web scale-factor investigation
- **§3** The app (`apps/dooduel`) — architecture, module split, gameplay-bug fixes + targeted tests, the graduated playtest host
- **§4** Verification strategy — per-PR gates, the repeatable multi-agent playtest, web + GPU lanes, the lowest-tier rule
- **§5** Gate questions — RESOLVED (human gate 2026-07-03, skribbl.io-grounded)
- **§6** Sequencing + risks
- **§7** Re-decisions vs the prototype + rejected alternatives
- **§8** Provenance

---

## §1. Intent, provenance, and the acceptance bar

### 1.1 What the FINAL is

Dooduel is a fully-featured skribbl.io-clone draw-and-guess game — the campaign's **product #1**. It is built **together with** the general-purpose Buiy framework features it forces (**product #2**) and the dev-experience feedback from building a real game on Buiy (**product #3**). The FINAL delivers all three:

1. **The app** — full feature parity with the design bundle, running on **native desktop + web-desktop + web-mobile**, verified by running it on all three.
2. **Nine general-purpose framework PRs (F1–F9)** — reviewed, standalone, dogfooded by Dooduel but **not Dooduel-local**. They land the one net-new subsystem (the drawing canvas), the coherent view layout surface, the render parity fixes, the press/picking safety work, the MVU clock ergonomic, and the playtest driver.
3. **A repeatable multi-agent playtest gate** — the graduated `playtest_host` makes "4 agents, each a different player, play a full match" a documented, re-runnable acceptance procedure, not a one-shot.

### 1.2 What the FINAL builds on and re-decides

The prototype's central bet — *a whole game on Buiy today, with only additive framework growth* — is **proven**. This spec inherits that verdict and the retrospective's disposition of every decision:

- **KEEP (port, re-derive the rationale):** the `RasterImage` distinct-pipeline primitive, the CPU-authoritative paint buffer, the keyed `PaintCanvases` resource, the `raster()`/`icon()` elements with identity patching, the pressable role-keyed press route, `.ignore_picking()`, the bordered-rounded + lens render fixes, the stroke/drag helper + unified headless driver, the Tick-fold game clock, the model-owned-tool→canvas projection, the honest per-seat playtest views, and the storage blob shape. (Retro §2.)
- **REDESIGN (the FINAL does it differently) — the three explicit re-decisions the retrospective mandates:**
  1. **Paint order = the general per-raster-anchor interleave**, NOT the prototype's top-layer-suffix split (F4a, §2.4).
  2. **The game clock = a poll-clock-as-Msg `ClockPlugin<M>` / `Cmd::tick_every`**, explicitly NOT an edge-triggered `Cmd::interval`/`Cmd::timeout` (F7, §2.8).
  3. **The view sizing/layout surface designed as ONE coherent pass**, not the wave-by-wave ad-hoc accretion the prototype shipped (F2, §2.2).
- **Bake in the recurring-bug guards:** the **structural auto-`Pickable::IGNORE` + a Tier-3 occluder invariant + a native-pointer live-interaction test tier** (the invisible-occluder class, ≥3rd appearance in project history), the **Tier-4 SDF cross-check** as the standing bordered-rounded ears/lens guard (not a stored golden — finding H1), and **side-surface version counters** (the persist one-frame race).

The rebaseline audit's build list was accurate about *shape*; the retrospective corrected it in six places found only by building + running (the canvas strategy reversal, G3-is-ergonomic-not-capability, the light-palette stub, the view-surface long tail, the click-swallowing occluder, and the bordered-rounded render-bug class). Where audit and retrospective disagree, **the retrospective wins**.

### 1.3 The acceptance bar

The FINAL is accepted when **all** of the following hold:

1. **Exact design parity** — structure, layout, spacing, the protokit color ladder, the Caveat/Shantell type pairing, doodle avatars, rounded pills/cards, sketchy ink outlines + wobble, the 3D-press, dark theme, timer ring, painted canvas, and confetti all match the design bundle, RUN-verified by GPU capture eyeballed against the design HTML. The scope of "exact" vs the character-layer residuals is **resolved (§5.f, rev-3): dashed borders + the press-down 3D-press + the crisp zero-blur rounded shadow are IN; color emoji is deferred to its own campaign; radial gradients / ribbon / elliptical wobble are attempt-if-cheap.**
2. **All three targets run** — native desktop, web-desktop (both WebGPU + WebGL2 backends), and web-mobile, verified by running each. HiDPI phones must render correctly (the prototype's top residual — F9).
3. **Verified working, not "should work"** — every wave RUN; headless-green is necessary but not sufficient (the standing Buiy lesson: RUN the GUI).
4. **The multi-agent playtest passes as a repeatable gate** — 4 agents, each driving a different seat through the graduated `playtest_host`, complete a full match; the documented procedure (§4.3) re-runs on demand.

---

## §2. The framework PR series (F1–F9)

Each PR is **general-purpose**, touches `crates/` (never `apps/dooduel`), and lands with its own verification. Disposition column: whether to cherry-pick the prototype commit or re-implement clean (the prototype commits are on `worktree-dooduel-proto1`; all touch `crates/` distinct from the app commits, so they are cleanly separable). The retrospective's §7 table is the source; deviations are called out.

**One-line map (deviations from retro §7 flagged):**

| PR | Crate(s) | Scope | Disposition |
|---|---|---|---|
| **F1** | buiy_core | raster/image-node primitive (distinct pipeline) + bump render system-count constants + **the bordered-rounded fill regression guard** (Tier-4 SDF cross-check + one establishment golden, §2.1) | re-implement clean (retro `646aabc`+`22c3a91`) |
| **F2** | buiy_view + buiy + **buiy_core** | **the coherent layout surface in one pass** (§2.2) + `raster()` element + identity patch. **Now spans buiy_core** — the `.fixed()`→viewport anchor is a layout-semantics change (finding #4, §2.2) | re-implement clean (retro `6ef6df5`, redesigned) |
| **F3** | buiy_view + buiy_core | styling surface (`Color::Custom`/`rgb` + facade, `.color`/`.font`/`.weight`/`.border(w,c,style)`, `icon()`+viewbox, styleable buttons) **+ borderless-rounded fill (introduces `ExtractedNode.radius`)** | re-implement clean (retro `fc2a72b`) — **deviation: borderless-rounded fill folded here, not into F4; F4b then builds on its `ExtractedNode.radius`** (§2.3, §6) |
| **F4a** | buiy_core | paint order: **general per-raster-anchor interleave** — supersedes the top-layer split (retires the raster-under-**opaque** modal limit; effect-group-nested raster is a documented follow-up, finding #5) | re-implement clean, SUPERSEDES retro `2854643` |
| **F4b** | buiy_core | shape/decoration fixes: bordered-rounded fill (**reuses the existing `PackedInstance.radius` slot — no stride change**), per-corner band lens fix (**shader-only**), dashed/dotted stipple, raster rounded clip, per-quad particle alpha, shadow corner radius (**the one dedicated-instance item — §5.f resolved IN: the dedicated rounded-shadow instance, §2.5.1**) | re-implement clean (retro `fb4cc18`) — **scope decided by §5.f (rev-3): dashed + crisp shadow IN** (§2.5, §6) |
| **F5** | buiy_view | press routing: pressable icon (KEEP) + **container/raster press route** + **the widget-runtime interaction-state visual layer** (NOT `ControlledLeaf` — finding #1) | re-implement clean (retro `f4357af` fw parts) |
| **F6** | buiy_view + buiy_verify | `.ignore_picking()` + auto-`IGNORE` for transparent top-layer containers + **the native-pointer live-interaction test tier** | re-implement clean (retro `9237822` fw part) |
| **F7** | buiy_core mvu | `ClockPlugin<M>` / `Cmd::tick_every` (poll-clock-as-Msg) + `on_submit_with` + headless `Time::advance_by` harness | **net-new** (not in the prototype — hand-rolled there) |
| **F8** | buiy_verify | stroke/drag helper + `drive_stroke` + the unified headless driver recipe (bake `init_asset::<Image>()`) | cherry-pick-clean (retro `f92cb04`) |
| **F9** | buiy_core | the HiDPI / web scale-factor bug investigation | net-new investigation |

**Deviations from retro §7, with rationale:**
- **Borderless-rounded fill moves from the render bucket (retro "F4b") into F3.** Without rounded fills the F3 styling surface renders square pills — grouping them makes F3 self-demonstrable as one reviewable unit (the prototype's own W3 did styling + borderless-rounded fill together). F3 introduces `ExtractedNode.radius`; **F4b's bordered-rounded fill then builds on that field** (the F3→F4b dependency edge, finding #6, §6). The **bordered**-rounded fill (F4b) reuses the *same* `PackedInstance.radius` slot on a **disjoint node set** (bordered nodes vs F3's borderless nodes) — no conflict, no stride change (finding #2). Cost: F3 mixes a small `buiy_core` extract change into a mostly-`buiy_view` PR — flagged for the gate (§5.e).
- **Retro's F4 is presented as two PRs, F4a (interleave) + F4b (shape fixes).** The retro already anticipated the split ("the biggest render PR — may split"); F4a (paint order) and F4b (shape) are independent and independently reviewable. **Rev-2: F4b is materially smaller than rev-1 believed** — the lens fix is shader-only (the band instance already carries per-corner elliptical radii) and the bordered-rounded fill reuses an existing slot; only the shadow corner radius touches a new instance — §5.f (rev-3) keeps the crisp edge, so that instance is built (finding #2, §2.5.1).
- **F2 now spans buiy_core** — rev-1 scoped F2 to buiy_view + buiy, but the `.fixed()`→viewport anchor is a `buiy_core` layout-semantics change (`Position::Fixed` re-parents to the root content box today, `layout/systems.rs:2878-2885`). Scope widened + flagged (finding #4, §2.2, §5.e).
- **F7 is net-new** — the prototype hand-rolled the clock in 6 lines and *proved the edge-triggered timer is the wrong shape*; F7 lifts the proven poll-clock pattern into a reusable plugin.

### 2.1 F1 — raster/image-node primitive + the bordered-rounded regression guard

**Scope.** The app's one net-new subsystem: a texture-presenting node primitive that samples a CPU-authored `Image` onto a layout node's rect — the substrate for both the game canvas (720×450) and the avatar editor (220×220).

**API as the FINAL builds it (KEEP the prototype's shape, re-derive):**
- A `RasterImage(Handle<Image>)` component + a `RasterInstance` (48 B) record + a **distinct `BuiyRasterPipeline`** (own WGSL shader, `@group(0)` view uniform + `@group(1)` per-node texture + Nearest sampler) + its extract→prepare→draw glue, wired into `BuiyViewPipelines`/`BuiySpecializedPipelines` and a draw section in `buiy_pass`.
- **Mirrors the band/gradient precedent exactly** — a distinct pipeline keyed by record, **NOT a new `BuiyPrimitiveKind`**. The closed `BuiyPrimitiveKind { Shadow, Quad, Glyph, Path }` enum and the byte-stable quad path stay untouched; the reserved `Path` slot is left for real vector art (freehand+fill+erase is semantically raster, not vector — re-argue and keep, retro §2).
- CPU-authoritative buffer: the app owns an `Image` (`Rgba8UnormSrgb`, `RenderAssetUsages::all()` so the main-world `data` survives the render-world clone — the `RENDER_WORLD`-only `data.take()` trap is documented), paints into `data`, marks it `Modified` → re-extract → re-upload.
- **Bump the render system-count meta-test constants** (`render_smoke`/`render_prepare`/`render_compositor` asserted `== 5` extract systems; the raster pipeline adds a 6th). The prototype left these red (they are `#[ignore]` GPU-lane tests); the FINAL fixes them in the same PR that adds the system.

**The recurring-bug guard baked in (rev-2, finding H1 — corrected from a stored golden to the standing SDF cross-check):** the framework never had a bordered-rounded fixture, so the prototype's ears/lens fixes moved zero goldens. F1 adds a **bordered-rounded fixture (a `buiy_verify` widget card: rounded fill + visible border)**, but the **standing regression guard is the existing Tier-4 SDF cross-check** (`run_sdf_cross_check`, `buiy_verify/src/reftest.rs:168` — rasterizes the canonical `sdf_rounded_rect` CPU twin and compares to the GPU, **zero stored bytes**), which catches a *rounded-fill corner regression* structurally without a golden to re-bless. **Stored goldens are scoped to only what the SDF cross-check cannot see:** the first-establishment-vs-design capture, and (in F4b) the band lens — the SDF oracle fixture is a **zero-width band**, so it exercises the *fill* SDF, not the *band* SDF (finding H1/§2.5). This is the lowest-tier-rule applied: an invariant/cross-check over a golden wherever the property is computable.

**Verification (lowest tier that observes it):** a **layout snapshot** (the raster element lands in the tree with its fixed size) + a **display-list snapshot** (the `RasterInstance` record extracts) headless; the **Tier-4 SDF cross-check** for the bordered-rounded fill corner; then a **GPU golden** on the `#[ignore]` lane (both legs — `buiy_core` + `buiy_verify`) proving byte-exact readback of brush/fill/eraser (the prototype's `[220,40,40]` red, filled green, erased→paper) + the one bordered-rounded establishment capture. The GPU golden is the last resort for the *rasterization residue and first-establishment only* — the record/extract is proven headless, the corner geometry by the cross-check.

**Perf note (deferred, not a v1 gate):** the prototype re-uploads the full ~1.3 MB buffer per dirty frame — fine at 720×450. A dirty-rect partial upload is a documented follow-up if a 60 Hz continuous drag on a weak machine shows in the frame budget (measure first; do not pre-optimize).

### 2.2 F2 — the one coherent view layout surface

> **The centerpiece.** The prototype's layout surface **accreted ad-hoc across four waves** (`.width` → `.fill` → `.grow` → `.shrink` → `.justify_*` → `.align_*` → `.top_layer` → `.fixed` → `.radius_corners` → `.ignore_picking`, each landing when a screen demanded it — retro §5 friction #2). The FINAL designs the whole surface **in one pass**, including the four things worked-around-app-side every time (flex-wrap, per-side padding, text-align, inset/absolute) and the #1 most-wanted missing piece (a scroll container). The *altitude* the prototype found is correct and is preserved: **expose intents** (`fill`, `grow`, `center`, `wrap`, `scroll`) as thin `Element` builder methods; the reconciler owns the `Sizing`/`FlexParams`/`Position`/`Overflow` lowering. No raw `Sizing` enum, no `Percent`, no `Length` leaks into the view vocabulary.

**Design principle.** Every modifier is a drift-only (`set_if_neq` / `!=`-guarded) write in the reconciler's `apply_container_props`, folding into the existing `changed` count exactly like `gap`/`padding` — so an unrelated fold never re-uploads and every existing snapshot stays byte-identical (the prototype proved this holds for the whole set). Components not `#[require]`'d by `Node` (`FlexItem`) are **inserted on demand** and toggled OFF by writing the neutral value (e.g. `grow = 0.0`), kept present — no `RemovedComponents` dependence.

#### The v1 modifier set (signatures + lowering + IN/OUT decision)

**Sizing (per-axis, `f32` logical px):**

| Method | Lowers to | v1? | Notes |
|---|---|---|---|
| `.width(px: f32)` / `.height(px: f32)` | `BoxModel.width/height = Sizing::Length(Px)` | **IN** | fixed size; KEEP from prototype |
| `.min_width(px)` / `.min_height(px)` / `.max_width(px)` / `.max_height(px)` | `Sizing` min/max | **IN** | the prototype clamped mobile card + overlay width via `.width`; explicit min/max is the coherent form (overlay `max_w`, mobile card fit-inside-padding) |
| `.fill()` | both axes `Sizing::Length(Length::Percent(100))` | **IN** | KEEP. **Lowering decided (finding #7):** `Sizing` has **no `Percent` variant** (`layout/types.rs:96` = `Auto/None/Length/Stretch/…`) — the percentage lives on `Length::Percent`. `.fill()` → `Length(Percent(100))` (100% of the **containing block**), which is what made the prototype's root fill the *viewport* and a child fill its *parent content box*. **Semantics named:** `Percent(100)` resolves against the containing-block dimension, so under flex-shrink it can **overflow** when siblings + gaps compete — which is why `raster()` defaults `.shrink(false)` and why **`.grow()` (flex-grow), not `.fill()`, is the tool for "take the *remaining* space among siblings."** Rejected: `Sizing::Stretch` — it is the flex cross-axis stretch keyword and would **not** fill a root against the viewport (breaks the proven root-fill use case). |
| `.fill_width()` / `.fill_height()` | one axis `Length(Percent(100))` | **IN** | **new** — the prototype only had both-axis `.fill()`; per-axis fill is needed (a chat pane fills height but not width; the mobile scoreboard strip fills width) |

**Flex item + container main/cross axis:**

| Method | Lowers to | v1? | Notes |
|---|---|---|---|
| `.grow()` / `.grow_by(f32)` | `FlexItem.grow` (insert-on-demand) | **IN** | KEEP |
| `.shrink(bool)` / `.shrink_by(f32)` | `FlexItem.shrink` | **IN** | KEEP; `raster()` defaults `.shrink(false)` (the canvas-squish fix) |
| `.justify_start()` / `_center()` / `_end()` / `_between()` / `_around()` / `_evenly()` | `FlexParams.justify_content` via a `Justify` facade | **IN** | prototype had start/center/between/end; complete the facade to the 6 CSS values |
| `.align_start()` / `_center()` / `_end()` / `_stretch()` | `AlignItems` via an `Align` facade | **IN** | KEEP; `_end` retires the podium top-spacer hack, `_start` keeps 3-pane columns natural-height |
| `.wrap()` / `.wrap(bool)` | `FlexParams.flex_wrap` | **IN** | **new** — the 16-swatch toolbar overflowed into chat (found by RUNNING); restores single-row authoring |

**Spacing (per-side):**

| Method | Lowers to | v1? | Notes |
|---|---|---|---|
| `.padding(Space)` | uniform `BoxModel.padding` | **IN** | KEEP (exists) |
| `.padding_xy(h: Space, v: Space)` | horizontal/vertical padding | **IN** | **new** |
| `.padding_top/_right/_bottom/_left(Space)` | one side | **IN** | **new** — the fixed-60px top bar fought uniform padding (worked around with content-height + `Md`); per-side is the coherent form |
| `.gap(Space)` | `FlexParams.gap` | **IN** | KEEP (exists) |

**Text alignment:**

| Method | Lowers to | v1? | Notes |
|---|---|---|---|
| `.text_align(TextAlign)` with `TextAlign::{Start, Center, End, Justify}` | the text layout inline-align | **IN** | **new** — centered copy rendered left-aligned; the word-slot underline was faked as a child 4px bar; system chat lines were centered by an `align_center` wrapper. All three want real text-align |

**Positioning + overlays:**

| Method | Lowers to | v1? | Notes |
|---|---|---|---|
| `.top_layer()` | `Stacking.top_layer = Popover` | **IN** | KEEP |
| `.fixed()` | `Position::Fixed` **re-anchored to the viewport (ICB)** | **IN** | KEEP shape; **FIX the anchor — but this is a `buiy_core` LAYOUT-SEMANTICS change, not a buiy_view one (finding #4).** `Position::Fixed` today re-parents to the *root content box* (`layout/systems.rs:2878-2885`), so the prototype's `.fixed().fill()` scrim landed at the padded root origin, not `(0,0)`. The FINAL re-anchors `Fixed` against the initial containing block (the viewport). **F2's scope therefore widens to buiy_core** (flagged like §5.e), and F4/F9's byte-stability concerns apply — ships with a **layout-snapshot test proving fixed-vs-viewport** (a `.fixed()` element at `(0,0)` regardless of root padding). |
| `.absolute()` + `.inset(top, right, bottom, left)` (each `Option<f32>` or an `.inset_top(px)` etc. builder) | `Position::Absolute` + offsets | **IN** | **new** — no way to place the per-seat drawing/guessed **corner badges** or pixel-perfectly center a modal. The seat badges are the concrete forcing case (worked around with `SCALE` highlight instead). |
| `.center_self()` | `Position::Absolute` + `inset(0,0,0,0)` + `margin: auto` | **IN** | **new — disambiguated (finding #7).** Rev-1's `.center()` conflated two lowerings under one name. `.center_self()` centers **this element within its containing block** (the pixel-perfect modal-centering primitive — the classic absolute + inset-0 + margin-auto). Centering a **container's children** is the existing `.justify_center().align_center()` — a *different* operation, kept separate. |

**Scroll (the #1 most-wanted):**

| Method | Lowers to | v1? | Notes |
|---|---|---|---|
| `scroll_column(...)` / `.scroll_y()` | `Overflow::Scroll` (y) + a **controlled stick-to-bottom** | **IN** | **new, #1 priority** — no view-level scroll forced the chat cap-to-12 and constrains the mobile single column. `ScrollArea` (buiy_widgets) already exists but is unreachable from a `view` element. **Stick-to-bottom is CONTROLLED, not "auto-max on append" (finding #3):** `ScrollOffset` is **runtime-input-owned** (`layout/components.rs:509-526`), so blindly forcing it to max on every append would **yank a user reading scrollback back to the bottom**. Instead the **model owns a `stick` intent**; the reconciler drift-asserts `ScrollOffset = max` **only while `stick` is true**; the scroll-input handler **clears `stick` on scroll-away** (and re-sets it on scroll-to-bottom). This is the **same runtime-state↔pure-view class as the F5 interaction-state visual layer** and is resolved jointly with it (§2.6). |
| `.scroll_x()` / horizontal scroll | `Overflow::Scroll` (x) | **IN** | the mobile scoreboard strip (worked around with `.grow()` even-split); the design uses overflow-x |

**DEFERRED from v1 (with rationale — each is character-layer or belongs to another PR):**

| Item | Why deferred | Where it lands |
|---|---|---|
| `.rotate(deg)` | pure decoration (±3° pick-tile tilt, the "free & open source" ribbon); not gameplay parity. The transform/tween bridge already drives `Rotate` cheaply (confetti proves it) — low-risk when wanted. | **§5.f (rev-3): attempt-if-cheap alongside F4b's character pass** (the rotated ribbon is the forcing case), defer-with-note otherwise |
| `.ignore_picking()` | belongs with the picking-safety work | **F6** (§2.7) |
| `.color` / `.font` / `.weight` / `.border` / `.radius` / `.radius_corners` / `.shadow` | these are **styling**, not layout | **F3** (§2.3) |
| bottom-sheet modal (mobile) | a phone-parity refinement; the prototype's clamped-scrim overlay is acceptable at v1 | phone-parity follow-up (may fold into F2 if cheap) |

**The `raster()` element (part of F2).** `raster(handle, w, h)` reconciles to a `Node` + `RasterImage` + fixed size, defaulting `.shrink(false)`. Handle patched **by identity** (`RasterImage` is not `PartialEq`) so an unrelated fold never re-uploads the texture; the entity is preserved across unrelated model patches (a reconciler test asserts this). The element **beat the mount-under-ECS-shell hatch decisively** (retro §2): one `Kind` + one reconciler arm; the canvas gets `keyed`/`when`/lifecycle for free (despawn on screen-swap, respawn on re-entry); the handle rides the replayable model.

**Verification.** Layout geometry is **observable pre-raster**, so nearly all of F2 gates at the **layout-snapshot tier** (headless): one snapshot per modifier proving the resolved rect/flex/position/overflow. Specific tests the findings require: the **`.fixed()`-vs-viewport** anchor (a `.fixed()` element resolves to `(0,0)` regardless of root padding — finding #4); the **controlled stick-to-bottom** behavioral test (append N messages *while sticking* → `ScrollOffset == max`; then simulate a scroll-away → `stick` clears → a further append does **not** move the offset — finding #3). Text-align gets a **layout/text snapshot** (glyph run x-origin). **No goldens** — geometry needs no rasterization. The `raster()` identity-patch gets a reconciler unit test (entity + handle survive an unrelated fold).

**Audited-port disposition: re-implement clean.** The prototype's shapes are right but the surface must be *designed*, not ported piecemeal — re-implement from `6ef6df5` as one coherent module.

### 2.3 F3 — the view styling surface + borderless-rounded fill

**Scope.** Everything that makes a node *look* like the design (color, type, outline, rounding, shadow, icon), plus the one render change (borderless-rounded fill) without which styled pills render square.

**API as the FINAL builds it:**
- **`Color::Custom(u8,u8,u8,u8)` + `Color::rgb(u8,u8,u8)`** — an exact-sRGB escape lowering to `ColorToken::Custom`. The 5-variant semantic facade cannot name the protokit ladder (canvas/surface/surface-2/ink/ink-2/muted/on-accent/pos/warn/danger), so non-accent design colors are pinned exact. **Decision: keep the documented `Custom` escape AND grow the semantic facade** to cover the protokit token *roles* (so an app names `Color::Surface2` rather than a magic `Custom`), backed by the theme. The accent stays semantic (`Color::Accent` + a `SetAccent` startup write, so it re-themes for dark). The two-layer split the prototype validated — **app `Palette` for the design ladder + `Theme` resource for widget defaults** — is the seam (re-themes even widgets the app can't reach; the `text_input` went dark for free). See §3 for the app side; the **light-palette completion** (the audit's "9-token stub" that Track B only field-covered — the retrospective found the non-accent tokens are family-derived stubs, not the protokit ladder) ships either as the app's `Palette` or as a real `default_light_theme()` ladder — **open-question (§5) which layer owns it.**
- **`.color(Color)`** → `TextColor` (foreground for Text / Button-label / Icon).
- **`.font(family)`** → `FontFamily` (sans fallback appended). The loader is **sfnt-only (ttf/otf/ttc) — no woff2**; the design's Caveat + Shantell Sans ship as `.ttf` **`include_bytes!`-embedded** (verified to render on both wasm backends with no fetch). The registered family string must equal the font's **internal** family name (nameID-16 typographic family).
- **`.weight(FontWeight)`** → the variable-font weight axis. **new** — the prototype loaded the variable fonts but `.font()` had no weight arg, so all text rendered at the default instance. The design specifies Caveat 600/700 and Shantell 400–700; weight is a real parity element. Low-risk (the fonts are loaded variable; thread the axis).
- **`.border(width_px, Color, LineStyle)`** → `BoxModel.border` + a 4-side `Border`. **new: the `LineStyle` arg** — the prototype's `.border()` hard-coded `Solid` (dashed was not even requestable from the view). The design uses dashed motifs (room-code box, join input). The *rasterization* of dashed lives in F4b; F3 makes it requestable.
- **`.radius(Radius)`** with `Radius::{Xl(22), Full(999), …}` + **`.radius_corners(tl,tr,br,bl: f32)`** → `Border.radius` (`Corners`) — the wobble radii. KEEP.
- **`.shadow(dx,dy,blur,spread,color)`** → `BoxShadow`, chains front-to-back (CSS order). The ambient card `--sh-*` + the 3D-press underside. KEEP shape; the crisp zero-blur "sticker" edge needs the shadow corner radius (F4b).
- **`icon(path_d, size_px, stroke_width, viewbox)` element** — a `buiy_core::Icon` on a layout node; the SAME node also carries `.background` + `.radius`, so ONE node paints a tinted circular badge with the doodle stroked on top. **new: the `viewbox` arg** — the design's 40×40 viewBox vs `Icon`'s hard-pinned 24×24 forced a per-app 0.6 coord+stroke scale; the arg removes it.
- **Styleable buttons** — `.background/.radius/.border/.color/.font/.weight/.size/.width/.height/.grow` apply to `Kind::Button`, **gated on "explicitly styled"** so an unstyled `button("x")` keeps every `buiy_widgets::Button` default (the shared-crate safety that keeps the counter/gallery goldens byte-identical — the §4.1c suppression gotcha from CLAUDE.md).

**Borderless-rounded fill (the `buiy_core` render change in this PR).** `pack_extracted` hard-coded `radius: 0.0` (`render/instance.rs:116,131` "per-node corner radius is not yet on the extract record") — so background fills never rounded; widgets only *looked* rounded because their border BAND masked square fill corners. Complete the stubbed path: **add `ExtractedNode.radius`**, resolve it in `extract_buiy_nodes` from `Border.radius` **only when no border side paints** (a borderless-rounded node), min-clamped to `≤ min(half_w, half_h)` (wide→pill, square→circle, never a per-axis lens), packed into the existing `PackedInstance.radius` slot. A **bordered** node keeps `radius 0` (its band traces the rounding) → **every existing display-list snapshot + GPU golden is byte-identical**. **F3 introduces `ExtractedNode.radius`; F4b's bordered-rounded fill then reuses this exact field + the same `PackedInstance.radius` slot on the *disjoint* bordered-node set — the F3→F4b dependency edge (finding #6, sequenced in §6).**

**Verification.** Color/font/weight/border/radius/shadow are **display-list observable** → display-list snapshots (headless). The borderless-rounded fill's *corner geometry* → the **Tier-4 SDF cross-check** (the standing zero-stored-bytes guard, §2.1); its first-establishment rasterization → **one GPU golden**, byte-identical to the pre-change goldens for every existing fixture (the borderless-only gate).

**Audited-port disposition: re-implement clean** from `fc2a72b`.

### 2.4 F4a — paint order: the general per-raster-anchor interleave

**Scope.** Retire the prototype's top-layer-suffix split (a KEEP-pattern-REDESIGN-mechanism, retro §2) with the general fix both W3a and W5 pointed at.

**The problem the prototype's split leaves open.** The prototype draws in-flow quads → raster (one global tier) → top-layer quads → glyphs. This is byte-correct *only* under one `ui()` root whose top-layer members form a **contiguous quad suffix**. It leaves TWO limits:
- (a) a **non-top-layer** overlay (an in-flow sibling positioned over the canvas) still draws UNDER the canvas — overlays MUST be `.top_layer()`;
- (b) **a raster inside a top-layer modal is HIDDEN** under the modal's own background (W5's avatar editor had to become a full in-flow screen because a modal panel painted over the raster it contained). The rule the prototype shipped: *"a modal may contain text + quads, but a modal containing a raster must be a full screen."*

**The FINAL fix.** **Interleave each raster into the flat quad draw by its OWN `node_quad_anchor`** — the *exact* gradient-bleed precedent the render pipeline already uses for the gradient tier. A raster paints at its true stacking position with no top-layer special case and no contiguous-suffix assumption. This retires (a) and (b) at once and unblocks a **true modal-with-a-raster** for an **opaque** top-layer modal (so the avatar editor can be the design's modal, not a full screen — a parity improvement).

**The boundary this does NOT cross (rev-2, finding #5 — stated explicitly + a documented follow-up).** The flat-interleave fix reaches only the **main flat pass**. A raster nested in a top-layer container that is itself an **effect group** (an `Opacity < 1.0` / backdrop-filter member) is rendered in the **off-screen group pass, which composites quads + glyphs only — a raster in that pass is DROPPED**. So the fix unblocks a raster inside an **opaque** modal panel, **not** inside a translucent/blurred one. **App consequence (required):** the avatar-editor modal **PANEL must be opaque** (a translucent *scrim* as a sibling behind it is fine — the scrim carries no raster). A raster inside an effect group is a **documented follow-up** (it needs the compositor's group pass to gain a raster step), not v1.

**API/design.** No app-facing API change; it is a render-internal re-ordering. `extract_buiy_rasters` already carries each raster's node; `prepare` maps each to its quad-instance anchor; `node.rs` splices a per-raster `FlatDrawStep::Raster(anchor)` rather than one global `Rasters` marker.

**Verification (the load-bearing correctness claim — attack this).** The interleave order is **observable in the flat draw list** without a GPU → a **paint-order/display-list snapshot** tier (the prototype used 6 interleave unit tests) proving each raster splices at its anchor and that a **non-raster view is byte-identical** (the `None`/sentinel path reproduces the old draw exactly). Then **GPU goldens** on both legs for the two cases the fix unblocks: raster-over-and-under an overlay, and a raster-in-a-top-layer-modal (visible over the modal panel). The gate must prove the general interleave does not regress the byte-stable non-raster path.

**Audited-port disposition: re-implement clean, SUPERSEDES `2854643`.**

### 2.5 F4b — shape/decoration render fixes

**Scope.** The remaining render parity fixes. **Rev-2: materially smaller than rev-1 believed** (finding #2) — the lens fix is shader-only, the bordered-rounded fill reuses an existing slot, and only the shadow corner radius touches a new instance. **§5.f resolved (rev-3): dashed + the crisp shadow are BOTH v1** — F4b carries its full slate below; the dashed stipple remains the natural cleave point if review size demands a split (an implementation freedom, not an open question). Each fix is a rasterization-residue matter, guarded by the Tier-4 SDF cross-check where computable and scoped GPU goldens otherwise (§2.5.1).

- **Bordered-rounded fill (the "ears").** A bordered rounded fill shows square-corner ears (band rounds, fill doesn't). The prototype's W6 fix packed the fill's uniform radius to the border band's **inner radius** (min of the per-corner inner radii) — EXACT for a uniform border, byte-identical for a square box. **Rev-2 (finding #2): NO stride change.** It reuses the **existing `PackedInstance.radius` slot** (the same field F3's borderless-rounded fill writes) on a node set **disjoint** from F3's (bordered vs borderless), so there is no conflict and no packing change. It needs the F3-introduced `ExtractedNode.radius` (the §6 F3→F4b edge).
- **Per-corner band radii + the radius LENS (`band.wgsl`) — SHADER-ONLY, zero stride (finding #2).** `BorderBandInstance` **already carries per-corner elliptical radii** — `outer_radius: [f32; 8]` + `inner_radius: [f32; 8]` = `(rx, ry) × 4` for TL/TR/BR/BL (`render/instance.rs:248-252`). The lens is a **shader** bug: `band.wgsl` read `outer_radius_tl_tr.x` (TL only) circularly, so a wide `border-radius:9999px` box drew a pointed lens. Fix: the fragment selects each corner's `(rx, ry)` by quadrant from the arrays the instance **already** packs (the standard per-corner rounded-box SDF). **No instance/stride change.** Byte-identical for uniform radii. KEEP from prototype.
- **Dashed / dotted borders.** `LineStyle::Dashed` renders solid today (two-layer gap: `.border()` hard-coded Solid — fixed in F3 — and `band.wgsl` has no dash pattern). F4b adds a **screen-space arc-length stipple** in the band shader. **This is the heaviest sub-item and the first candidate to split out** if F4b is too big; the design's dashed room-code box is the forcing case. **§5.f resolved (rev-3): dashed IS v1 — the sub-item is IN** (finding M2's gate closed in favor of inclusion).
- **Rounded clip for `raster`.** The raster shader clips to a rectangular AABB, so custom-drawn avatars render as square stickers while stock doodles are circular. Add a corner-radius clip so a drawn avatar reads as round.
- **Per-quad particle alpha.** A Buiy `Opacity < 1.0` forms an `EffectGroup` (off-screen composite boundary), so ~110 confetti `OpacityTween`s would spin up ~110 off-screen targets. `Translate`/`Rotate` form only a cheap stacking context. Add a **cheap per-quad alpha** (a fade that does NOT promote to a group) — needed by any particle system, not just confetti's end-of-life fade.
- **Shadow corner radius — the ONE item needing a new channel; §5.f RESOLVED it IN (finding #2; rev-3).** `pack_shadow` sets `radius: shadow.sigma` — the `PackedInstance.radius` slot **is** the blur sigma for the shadow primitive (`render/instance.rs:174-185`, `shadow.wgsl`), so a shadow corner radius has nowhere to live in the current instance. This is the only F4b item that needs a dedicated instance (§2.5.1). It exists **only** to render the crisp zero-blur 3D-press "sticker" edge (the prototype softened it with a 2px blur). **§5.f keeps the crisp edge in v1** (the 2px-blur softening was ruled a real parity miss), so F4b builds the dedicated rounded-shadow instance (Option B, §2.5.1).

**ATTEMPT-IF-CHEAP during F4b, defer-with-note otherwise (§5.f, rev-3 — was DEFERRED in rev-2):** **per-axis elliptical corners** — note (finding #2): the band instance **already carries** per-corner elliptical `(rx, ry)`, so this is **not a data/stride limit** but a deliberate *shader* choice (the fix uses `min(rx, ry)` for a circular corner); per-axis ellipticity is shader-only-away but **invisible at Dooduel's ±3px wobble** — take it only if it falls out of the lens fix nearly free. **Radial-gradient backgrounds** (flattened) and the **rotated ribbon** (app-decoration, not render primitives): same rule — attempt each if genuinely cheap during F4b's character pass, otherwise record the residual with a note and move on.

#### 2.5.1 The shadow-corner-radius instance decision (RESOLVED — §5.f keeps the crisp 3D-press edge: build Option B)

**Rev-2: this decision shrank to shadow-only.** Rev-1 believed both the per-corner fill radius and the shadow radius needed a stride change; finding #2 corrected both away (the fill reuses the existing slot on a disjoint node set; the lens/per-corner band is shader-only because the band instance already carries per-corner elliptical radii). **The one remaining item is the shadow corner radius**, and **§5.f (rev-3) resolved it: the crisp zero-blur 3D-press edge IS a v1 parity requirement**, so the instance is built. The two options weighed:

- **Option A — widen the shared `PackedInstance`** to add a shadow corner radius. Touches the shadow + text + quad packing and the byte-stable quad path (the R8b `36→52` stride history). **Risk: ripples through the byte-stable path every golden depends on.**
- **Option B (recommended) — a dedicated rounded-shadow instance**, mirroring the band/gradient/raster precedent (a distinct record + pipeline keyed by need). Keeps the byte-stable quad path untouched; only a shadow that needs a corner radius pays for the extra instance. **Same "distinct pipeline keyed by record" pattern F1 uses.**

**DECIDED: Option B (§5.f, rev-3).** Preserves byte-stability by construction. Implementation review must still attack whether a dedicated rounded-shadow instance composes correctly with the F4a interleave and the effect-group compositor (risk §6.2.4).

**Verification (finding H1 — goldens scoped to what the SDF cross-check can't see).** The **rounded-fill corner** rides the **Tier-4 SDF cross-check** (`run_sdf_cross_check`, zero stored bytes, §2.1) as the standing guard. **GPU goldens (both legs) are scoped to only:** the **band lens** (the SDF oracle fixture is a *zero-width band*, so it exercises the fill SDF, not the *band* SDF), the **dashed stipple**, **raster rounded clip**, **shadow blur AA + the crisp-edge radius**, and **per-quad particle alpha** (§5.f resolved dashed + crisp radius IN). Byte-identical for every pre-change fixture (uniform-radius / no-border / opaque cases).

**Audited-port disposition: re-implement clean** from `fb4cc18`. **Scope decided (§5.f, rev-3): dashed + the rounded-shadow instance are IN**; splitting the dashed stipple into its own PR stays an implementation freedom if the diff proves too large to review as one.

### 2.6 F5 — press routing generalization + the interaction-state visual layer

**Scope.** Extend the proven role-keyed press route beyond leaf icon/button; add a **widget-runtime-owned interaction-state visual layer** (hover/press styling owned *outside* the model) for the 3D-press-down.

- **KEEP: pressable `Icon`** — an `icon(...)` with `.on_press(msg)` + `.label(name)` becomes an activatable a11y button (`A11yRole::Button` + `A11yLabel` + the view's `PressAction`). Routes through BOTH a real pointer click (`pointer_click_emits_on_press`, gated on the activatable role) AND a probe/AT `Action::Click` — the **Button contract is role-keyed** (`contract_for(Button).honor(Click)` emits `OnPress` with no `Button` component).
- **NEW: container / raster press route.** A clickable container (children intercept the hit + carry no role) or a pressable `raster` (own seat chip) silently ignores `.on_press` today. Generalize the role-keyed route so a container/raster with `.on_press` becomes activatable — the click lands on the container itself. Forcing cases: the pick-word tiles as clickable containers, the custom-avatar raster seat chip.
- **NEW: the interaction-state visual layer (rev-2 — re-specced, finding #1).** A `view` is a pure `fn(&Model) -> Element`; a transient pressed/hover style is ephemeral interaction state that does **not** belong in the model, so the prototype's 3D-press ships **resting-only** (no press-down `translateY` + shadow-collapse). **The PRIMARY mechanism is a widget-runtime-owned interaction-state visual layer** — hover/press styling owned *outside* the model by the widget runtime (the same place `buiy_widgets::Button` would own it), applied on top of the view-authored resting style. **`ControlledLeaf` is NOT the vehicle** (rev-1 mis-cited it): `ControlledLeaf` is an **opt-OUT suppression marker** (`Without<ControlledLeaf>` filter, `mvu/leaf.rs:33-49`) that *excludes* a checkbox leaf from the built-in press-to-toggle so a model can own its `A11yToggled` — a **reverse-data-flow toggle-ownership** marker, unrelated to press *visuals*; and `buiy_widgets::Button` carries **zero press-visual machinery** today (`button.rs:181-188`), so there is nothing to surface. The layer is **net-new**: a small `InteractionState` (hover/press/none) the widget runtime writes on pointer enter/press, consumed by a press-style resolver *outside* the pure view. This is **correct for MVU** (a pressed style is ephemeral non-model state) and is the **same runtime-state↔pure-view class as the F2 scroll stick-to-bottom** (§2.2, finding #3) — the two are designed together. **Open risk (§6):** the depth of the interaction-state layer (does it need a full hover/press/focus transition engine, or does a discrete-state resolver suffice?). Attack at the gate.

**Verification.** The container/raster press route → the **live-interaction tier** (real shell + picking + synthetic clicks — a headless snapshot can't see pick occlusion) + an a11y-probe test (the role-keyed route). The interaction-state visual layer → a **live-interaction press test** (a real synthetic press applies the pressed style and reverts on release) + a headless `InteractionState`-transition unit test.

**Audited-port disposition: re-implement clean** from the framework parts of `f4357af`.

### 2.7 F6 — picking safety + the native-pointer live-interaction test tier

**Scope.** Make the invisible-occluder bug class **unwritable by construction** (the primary defense), and add a live-interaction backstop for what structure can't cover.

- **PRIMARY — auto-`Pickable::IGNORE` for transparent (`Color::NONE`) top-layer containers (rev-2, finding M3: this is the STRUCTURAL prevention, foregrounded).** The prototype's floating theme toggle (a transparent `.fixed().fill().top_layer()` container) sat topmost in the pick order and **occluded every click on every screen**; the picking backend truncates at the first occluder. Making a transparent top-layer container auto-transparent-to-picking makes the bug **unwritable** — the author cannot forget, because there is no opt-in to forget. **Discipline-based gating (remembering to run a test) is the failure mode that shipped the bug three times; structure is the fix.**
- **`.ignore_picking()` (`Pickable::IGNORE`)** — the explicit escape for the non-transparent case (a node transparent to picking — neither hit-target nor occluder — while its interactive CHILDREN stay pickable, the `pointer-events:none`-on-container / `auto`-on-children pattern). The picking backend already has the pass-through machinery + a comment describing this exact hazard. Opt-in, default no-op, byte-identical snapshots.
- **A Tier-3 invariant/proptest over the fixture catalog (rev-2, finding M3).** Assert structurally, across the whole widget/screen fixture catalog: **"no transparent top-layer container is a pick occluder"** — a property test that would fail the moment a fixture (re)introduces the class, independent of anyone remembering to click it.
- **The native-pointer live-interaction test tier (the backstop).** For a bug class that has appeared **≥3 times in project history** (the parity campaign's detached modal; the wasm cold-synthetic-click artifact; now this occluder): drives **real synthetic pointer clicks** against the running app on the **DEFAULT (headless) lane** — NOT the GPU lane, and NOT a11y-clicks (which action by role+label, bypassing the exact pick occlusion this class lives in). The **default gate for any `.top_layer()`/transparent-container change.** It catches the *non-transparent, non-catalog* occluder the structural rule + proptest can't see.

**Verification.** F6 *is* a verification tier. The Tier-3 invariant runs headless over the fixture catalog (the structural guarantee); the live-interaction tier self-test: a transparent top-layer container over a button — a real synthetic pointer click penetrates to the button and folds its Msg (the occluder guard + the auto-`IGNORE` regression). `.ignore_picking()` byte-identical for every existing snapshot (opt-in no-op).

**Audited-port disposition: re-implement clean** from the framework part of `9237822`.

### 2.8 F7 — the MVU clock ergonomic

**Scope.** Lift the prototype's hand-rolled 6-line game clock into a reusable primitive — **as a poll-clock-as-Msg, NOT an edge-triggered timer** (the prototype's key re-decision).

- **`ClockPlugin<M>` / `Cmd::tick_every`** — a runtime-provided **poll clock source as a Msg**: `M::tick(now)` enqueued every frame, the reducer deriving everything (countdowns, hint reveals, phase timeouts, bot fires) from `now − anchor`. The prototype **proved the edge-triggered `Cmd::interval`/`Cmd::timeout` is the WRONG shape**: a fired-once timer is edge-triggered (hard to replay, hard to keep `set_if_neq`-clean), whereas the poll-from-`now` fold is **level-triggered** (replayable, idempotent, perf-free — store only DERIVED values, never raw `now`, so a steady frame folds byte-identically and `set_if_neq` absorbs it). The gap is purely ergonomic (every app re-hand-rolls the driver + anchor arithmetic); the semantics are already correct.
- **`on_submit_with(fn(String) -> Msg)`** — `on_submit` takes a *static* `Msg`, so a typed guess round-trips through a model field (`on_input → SetChatInput` → `on_submit → SubmitGuess`). The `_with` variant deletes the two-message dance (mirrors the existing `on_input_with`).
- **Headless virtual-clock advance (`Time::advance_by`) harness** — `BuiyHeadlessPlugin` omits `AnimationPlugin` and a tight `app.update()` loop has ~µs `delta`, so the confetti capture had to `sleep 16ms` between frames. A headless-animation harness wants a **virtual advance**, not real sleeps.

**Relationship to the MVU spec's roadmap.** The MVU-as-core FINAL (`2026-06-29-mvu-as-core-design.md` §8) reserved a keyed `Subscription`; F7 is the **narrower, proven** ergonomic — a poll clock, not a general subscription diff. The MVU spec's §8 invariant ("payload-carries-nondeterminism; replay re-feeds logged Msgs, never re-runs") holds: the clock's `now` enters the log as the `Tick(now)` payload. F7 does **not** need the full Subscription machinery (the prototype confirmed the poll clock is the whole answer for game timing).

**Verification.** A headless test injecting `Tick(now)` at chosen `n` (the prototype's virtual-clock pattern — zero wall-clock flakiness, instant full-match sim) + the `set_if_neq` no-cascade **steady-frame gate** (the blink-fixture shape from MVU §11: on a steady tick, `models_mutated == 0 && node_rebuilds == 0`; a model wrongly storing raw `now` would fail). Headless, default lane.

**Audited-port disposition: net-new** (the prototype hand-rolled it; F7 generalizes the proven pattern).

### 2.9 F8 — the playtest driver

**Scope.** The headless machinery for the multi-agent playtest — the framework's missing stroke/drag helper + the unified driver recipe.

- **`PointerHarness::stroke(path)` / `drag(from, to, steps)` + a reusable `drive_stroke(app, window, pointer, path)` free fn.** Presses primary at `path[0]`, writes a `PointerAction::Move{delta}` per subsequent point (updating `PointerLocation` coherently via `PointerInput::receive`), releases at the last — so `bevy_picking`'s `pointer_events` derives `DragStart → Drag → DragEnd` on the PRESS target. **This is the FIRST headless driver of bevy's drag machine** — every prior harness test used `move_to`, which by design emits NO `Move` and never drags (a naive "sequence of `move_to`s" silently draws nothing). Pure additive `buiy_verify` surface, no framework bug.
- **The unified headless driver recipe** — `BuiyProbePlugin` (probe eyes) + `bevy::picking` + `buiy_core::picking` + the backend (harness hands) compose in ONE App (the probe preset omits picking, so adding it back conflicts with nothing). **Bake in the one non-obvious piece: `app.init_asset::<Image>()`** — a headless canvas host has no `ImagePlugin`/`RenderPlugin` to register `Assets<Image>`, so it must register the asset itself (else an opaque "Resource does not exist" panic under `debug=0`). Scroll/animation are NOT needed. Either document the line or have `CanvasPlugin` register defensively (`is_plugin_added`-guarded).

**Verification.** The `pointer_drag` test (headless, no GPU): an N-step drag emits exactly 1 `DragStart` + N `Pointer<Drag>` (each with the right `delta`) + 1 `DragEnd`, all on the press target; plus a dedicated test that `move_to` provably does NOT drag. A canvas e2e (input → funnel → the app's own paint observers → ink in the buffer, no GPU).

**Audited-port disposition: cherry-pick-clean** from `f92cb04` (the only new primitive that wave; kept clean).

### 2.10 F9 — the HiDPI / web scale-factor investigation

**Scope.** The prototype's **top mobile residual** — a focused `buiy_core` investigation, not a pre-decided fix.

**The bug.** At `devicePixelRatio > 1` (every real phone / retina) the UI renders scaled ~dpr× and overflows, dpr-proportionally (correct at dsf=1, ~2× at dsf=2, ~3× at dsf=3). The responsive *logic* is correct (reads logical `window.width()`); the bug is in the **render/layout scale-factor handling on the wasm path**. A dynamic window resize also mis-sizes the surface (confines to a sub-region) — likely the same scale-factor/surface-reconfigure seam. The mobile layout is only verified clean at dsf=1.

**Why an investigation, not a spec.** The root cause is unknown depth — could be a shallow scale-factor plumbing miss or a deep surface-reconfigure seam. F9 is scoped as: reproduce at dsf=2/3 in a browser, root-cause, fix, verify no overflow at dpr>1 on a real mobile browser. It **blocks the mobile acceptance criterion** (charter v1 target), so it runs as an **early parallel investigation** (§6), not gated behind F1–F8.

**Verification.** **A headless HiDPI proxy FIRST (rev-2, finding M4):** construct the app with a `Window` at `scale_factor = 2` and assert the resulting `ResolvedLayout` fits the **logical** viewport (no overflow) — a headless, CI-able reproduction that de-risks the fix *before* touching a browser. **Then** the browser check: web-driven (both backends) at dsf=1/2/3, the fix verified by rendering at dpr>1 and asserting the layout fits the viewport (no top/right clip); a real mobile browser check at a milestone (the browser/real-device legs are **manual milestone gates**, not per-wave CI — §4.4/§4.5, finding N1).

---

## §3. The app — `apps/dooduel`

### 3.1 Where it lives (RESOLVED — §5.d: `apps/dooduel`)

**Recommendation: graduate out of `examples/` into a new top-level `apps/dooduel`.** The `examples/` crates (`buiy_gallery`, `counter_view`, `hello_*`) are framework smoke-tests and dogfood demos; Dooduel is a **fully-featured standalone product** (the charter's word), not a smoke test. A top-level `apps/` cleanly separates "the framework's own examples" from "an app shipped on the framework." The `dooduel_web` crate + `install_runtime` + `ViewportPlugin` + `playtest_host` bin move with it. Runner-up: keep `examples/dooduel` (less churn, but conflates product with demo). **RESOLVED (§5.d, human gate 2026-07-03): `apps/dooduel` is ratified** — a new top-level `apps/` workspace tree; `examples/` keeps only framework examples. Migration note in §5.d.

### 3.2 Module split (restructure, don't keep-shape)

The prototype's `lib.rs` grew to 3082 lines (the whole view + ~40 helpers in one file — a DX friction, retro §5). The FINAL splits it while **keeping the single-model / single-`ui()` shape** (correct for Dooduel: ONE model, one reducer, screens via `match` on a `Screen` enum, sub-views via `Element::map`):

```
apps/dooduel/
├── src/
│   ├── lib.rs            — the MVU model (Dooduel { screen, game, tool, avatar, theme, viewport, … }),
│   │                       the Msg enum, the thin update reducer (delegating to game.rs), ui() install,
│   │                       install_runtime (the shared native/web plugin set)
│   ├── game.rs           — the PURE game core (ports near-verbatim — the highest-quality, most-reusable
│   │                       app code: zero framework coupling, every rule a &mut self method, unit-testable
│   │                       with no ECS/GPU/clock). Phase machine, scoring, words, hints, guess normalize/
│   │                       levenshtein/is_close, seeded bots, the honest word_display() redaction accessor
│   ├── paint.rs          — keyed PaintCanvases resource (HashMap<CanvasKind, PaintSurface>) + the
│   │                       model-owned-tool → canvas projection (sync each frame) + the monotonic *_seq
│   │                       imperative ops (clear/undo/per-turn-auto-clear) + Press/Drag/Release observers
│   ├── storage.rs        — the typed load_persisted/save_persisted per-target seam (native JSON file /
│   │                       wasm localStorage), the avatar PNG-encode+base64, the saved_version counter
│   ├── theme.rs          — the Palette struct (LIGHT/DARK ladders) + ThemePref + sync_theme_resource
│   │                       (model → Theme resource swap) + the font names + accent
│   ├── avatar.rs         — the 22 doodles as structured data + hash_str + doodle_avatar(name, px)
│   ├── confetti.rs       — the decoupled ConfettiPlugin (rising-edge model-observing side effect)
│   ├── view/
│   │   ├── mod.rs        — the Screen router (match) + shared scaffolding
│   │   ├── widgets.rs    — the shared view helpers (card/pill/eyebrow/title/badge/scrim/soft_button/…)
│   │   ├── home.rs · join.rs · lobby.rs · in_game.rs · podium.rs · avatar_editor.rs
│   └── bin/
│       └── playtest_host.rs  — the graduated file-protocol host (§3.4)
├── (dooduel_web crate)   — the wasm entry (dual-backend webgpu|webgl2, canvas-bound window)
```

**Load-bearing app patterns to KEEP (retro §2, §5 — the game-on-a-game-engine positives):** the pure `Game` core; **bots-as-emits** (a bot guess is a real `Msg` run-to-completion in the same drain a human hits — idempotency free via `turn_guesses`); the **rising-edge side-effect observer** (confetti / per-turn canvas clear / theme sync / persist all READ the model via a `Local` latch, never enqueue); **model-owned tool state projected model→canvas each frame**; the **keyed `PaintCanvases` resource** (survives the reconciler's despawn-on-screen-swap where a component would lose pixels); the **honest per-seat view** (clone `Game`, set `viewing_as=i`, reuse `word_display()` — the redaction has ONE home so it cannot leak the word). The phase machine goes in **MVU** (the Menu-machine precedent — `bevy_state` is not compiled into the workspace at all); this seam is a proven strength, not a tax.

**Canvas lifecycle pins (rev-2, finding #21).** The game canvas **clears on entry to the Drawing phase** (the `chooseWord` transition — matching the design), driven by the rising-edge side-effect observer reading `Game.phase`; and it **re-initializes / refits on a viewport-mode change** (desktop↔mobile changes the canvas backing size — 460 vs 280px, §4.4), so the `PaintSurface` is re-sized and the in-progress pixels re-fit rather than clipped. Both ride the existing model-observing-side-effect + monotonic-`*_seq` machinery.

### 3.3 The playtest-found gameplay-bug fixes (each with its targeted test)

The multi-agent playtest surfaced **6 code-level gameplay bugs** (retro §4.11–16) no prior gate could see. Each fix ships with a targeted test — the tests the playtest showed were missing. (Rev-2, finding #22: the W8-live consolidation *listed 8 findings*, but 2 of them are **product/design items, not code bugs** — the canvas-appears-all-at-once transport gap → §3.4 per-stroke streaming, and the drawer-payout deficit → §5.a. Those are handled there, not as code fixes here.)

| # | Bug (playtest evidence) | Fix | Targeted test |
|---|---|---|---|
| 1 | **Round-counter overflow** — podium reads "Round 2/1" (`round=2, total_rounds=1`); `total_rounds` also reads 2 pre-start vs 1 in-match (stale default before host config applies) | Set `total_rounds` from config **at match start** (not a stale default read); clamp the display so `round` never exceeds `total`. **Round-indicator strings VARY by surface (rev-2, finding #20):** desktop header renders **"Round {r} / {t}"** (slash), while the **mobile** header and **system chat** render **"Round {r} of {t}"** (word) — both are in the design; render each per its surface. This is a rendering-correctness fix, not a missing feature (§5.c) | unit: at `Final`, `round_display() == total` and never exceeds; pre-start `total_rounds == config.rounds`; both string forms render on their surfaces |
| 2 | **Stale drawer at podium** — `drawer`/`drawer_name` never cleared; the final screen says "Drawing: Alex" and tags him "(drawing)" | clear `drawer` on the transition to `Final` (or gate the "(drawing)" tag on `phase == Drawing`) | unit: at `Final`, no seat is tagged drawing; `drawer_name()` is `None`/absent |
| 3 | **The drawer plays blind** — "X guessed the word!" lines became visible to the drawer only AFTER the turn flipped; an agent wasted a clear+redraw on an already-fully-guessed turn | expose a **live** `guessed_count()` / `all_guessed()` accessor visible in the Drawing phase (render it in the drawer's header/scoreboard); ensure the "guessed" chat lines are visible **in-phase** (they already fold — the host's per-seat view lagged, §3.4) | unit: after K mid-draw guesses, the drawer's projected `guessed_count() == K` and the "guessed" line is present in-phase |
| 4 | **No wrong/close-guess feedback + no echo** — no echo of your own guess, nothing on a miss; the close-guess ("So close! 👀") path was never exercised (all guesses exact) | **Design-exact three-way (rev-2, finding #18):** a **wrong** guess posts to the **SHARED chat** (the guesser's name + the literal guessed text — **everyone** sees it, as in the real game); a **near-miss** (Levenshtein ≤2, len diff ≤2) fires a **PRIVATE toast "So close! 👀"** (only the guesser); an **exact** match posts **"X guessed the word!"** with the word hidden from those who haven't guessed | unit: a wrong guess appears in the shared chat with name + literal text (all seats see it); a near-miss fires the private toast only; an exact match posts "X guessed the word!" + word stays hidden |
| 5 | **Hint reveal — schedule already PORTED; the gap is a missing test + unverified render (rev-2, findings #15/#16)** — the schedule is correctly implemented (`game.rs:396-414`); it simply never fired in live play (fast guesses preempted it) and was never rendered-verified | Add the missing **threshold-crossing test** + a render verification. **Precise schedule (finding #15):** for `i` in **`1..=hintCount`** (1-based, inclusive; default `hintCount=2`), reveal at `floor(totalDraw · (0.6 − i·0.18)).max(1)` **seconds-LEFT** (⇒ 2 hints @ 80s draw = **33s** and **19s** left); each reveal picks a **RANDOM unrevealed `[a-z]` position**; under the poll clock use **CROSSING semantics** (fire when `seconds_left` first `≤` a threshold, latch the revealed count) — **not** equality (a poll can skip the exact second) | unit: tick a slow-guess turn so `seconds_left` **crosses** 33 then 19; assert the latched revealed count goes 1 then 2, that revealed positions are `[a-z]` and previously-unrevealed, and never exceeds `min(hintCount, letterCount−1)` |
| 6 | **Countdown units (host)** — the host's rendered "Ns left" decremented at a non-wall-clock rate (~1.3/s and ~4–5/s measured) — a ticks-vs-seconds mismatch (the game clock itself is correct; only the host report is wrong) | the host's `state.json` renders `seconds_left` from the game accessor (`now − anchor` wall-clock), not a tick count | host check / unit on the accessor: `seconds_left` decrements ~1/s of wall time |

### 3.4 The graduated `playtest_host`

The prototype's file-protocol host **met the acceptance bar on the first live run** and is a **FINAL-shippable playtest harness**. Graduate it (retro §6, §7, W8-live open items):

- **Richer `state.json`** — add word length, hint count, per-seat "can I act now" flags (drawer-vs-guesser gating), the live `guessed_count`/`all_guessed` signal (fix #3), and `turn X / round N / total` (fix #1). Agents currently infer too much.
- **Per-stroke streaming** — the transport batches strokes + flushes `canvas.png` at ~2 Hz, so the canvas appears **all-at-once** to guessers — the partial-art tension that makes skribbl fun is absent from agent play. A **per-stroke PNG flush** restores it (the real UI streams live — W7's browser drives prove it; this is a host+agent-transport gap, NOT a UI gap).
- **In-protocol `continue` / skip-reveal** — an agent can advance the reveal (the design's "Continue →").
- **Chat streamed as a separate file** — so agents needn't diff the whole view to see new lines (couples with fix #3's in-phase delivery).
- **Keep the honest per-seat view design verbatim** — clone + `viewing_as` + the single `word_display()` home (the only real correctness risk — leaking the word — is closed by construction).

**Build-env pins — operationalized as a named `ci.yml` env, not a doc note (rev-2, finding N2).** The `apps/dooduel` build/test job in `ci.yml` sets **`RUST_MIN_STACK: "33554432"`** (32 MiB — rustc SIGSEGVs during monomorphization of the large bevy example bins otherwise, retro §4.10) as a job-level `env:` (also mirrored in the build docs so a local build matches CI), plus **`CARGO_BUILD_JOBS`** capped if the host link OOMs (the standing `-j 2`/`CARGO_BUILD_JOBS=4` discipline). Any headless canvas host bakes the **`init_asset::<Image>()`** line (F8).

---

## §4. Verification strategy

### 4.1 The lowest-tier rule (per `using-buiy-verification`)

Every test goes at the **lowest tier that can observe the bug**: layout snapshot → display-list snapshot → invariant → reftest → golden. Goldens are the **last resort for the rasterization residue only.** Applied per-PR:

| PR | Primary tier | GPU golden? |
|---|---|---|
| F1 raster | layout + display-list snapshot (element in tree; record extracts) + **Tier-4 SDF cross-check** for the bordered-rounded fill corner | **scoped** — byte-exact brush/fill/eraser readback (residue) + one bordered-rounded establishment capture; both legs |
| F2 layout | **layout snapshot** (geometry is observable pre-raster) + the **`.fixed()`-vs-viewport** test + the **controlled stick-to-bottom** behavioral test | no |
| F3 styling | display-list snapshot (color/font/weight/border/radius/shadow) + **Tier-4 SDF cross-check** for the borderless-rounded fill corner | **scoped** — one first-establishment fill capture only |
| F4a interleave | paint-order/display-list snapshot (interleave order; non-raster byte-identical) | **yes** — raster-over/under-overlay + raster-in-**opaque**-modal; both legs |
| F4b shape | **Tier-4 SDF cross-check** for rounded-fill corners (standing guard, zero stored bytes) | **scoped (finding H1)** — band lens, dashed stipple, raster rounded clip, shadow blur AA + crisp radius, particle alpha (§5.f resolved all IN); both legs |
| F5 press | **live-interaction tier** (container/raster press) + a11y probe + a headless `InteractionState`-transition test | no (unless a rendered press state is wanted) |
| F6 picking | **Tier-3 "no transparent top-layer container is a pick occluder" invariant/proptest over the fixture catalog (structural)** + the native-pointer live-interaction backstop (default headless lane) | no |
| F7 clock | headless virtual-clock inject + the `set_if_neq` steady-frame no-cascade gate | no |
| F8 driver | headless `pointer_drag` + canvas e2e (no GPU) | no |
| F9 HiDPI | **headless proxy first** (`Window scale_factor=2` → `ResolvedLayout` fits logical viewport) then web-driven at dsf=1/2/3 | via browser render check (manual milestone) |

**The GPU `#[ignore]` lane runs both legs** (`buiy_core` + `buiy_verify`) on a real adapter (or lavapipe) — additive, must pass on a GPU host; the headless gate stays green without an adapter (CLAUDE.md). The pinned-lavapipe reconstruction recipe (memory `buiy-verify-followups-campaign`) is the local golden-blessing path.

### 4.2 The recurring-bug guards (mandatory tiers, not optional)

Guards that exist specifically because the prototype found the bug class only by running:
1. **Structural occluder prevention (F6)** — auto-`Pickable::IGNORE` for transparent top-layer containers (the class is *unwritable*) + the Tier-3 "no transparent top-layer container is a pick occluder" invariant/proptest over the fixture catalog; the native-pointer live-interaction tier is the backstop for the non-structural case (finding M3). Discipline-based gating is the failure mode that shipped this bug three times.
2. **The bordered-rounded regression guard (F1)** — the standing guard for the ears/lens class is the **Tier-4 SDF cross-check** (zero stored bytes), *not* a stored golden (finding H1); goldens are scoped to first-establishment + what the cross-check can't see (the band SDF, dashed, shadow AA, particle alpha).
3. **Side-surface version counters (app)** — the persist/observe key includes a monotonic version for any side-surface that lags the model by a frame (the persist one-frame race).

### 4.3 The end-to-end acceptance: the repeatable multi-agent playtest

The headline acceptance is a **documented, re-runnable procedure** (the graduated host makes it repeatable, not one-shot):

1. `cargo build -p dooduel --bin playtest_host` (with `RUST_MIN_STACK=33554432`).
2. `playtest_host --dir <shared-dir>` (env knobs: `draw 120s / pick 45s / reveal 12s` defaults for slow agents; `bots_enabled=false` so all four seats are agent-driven).
3. Point **four independent agents** at `<shared-dir>/seat_{0..3}_view.md` + `state.json` + `canvas.png` (read) and `commands.jsonl` (append). The drawer agent picks + strokes; the others guess; the host advances in real time.
4. **Pass criteria:** the match completes end-to-end (all rounds × turns); each agent draws once and its drawing is recognized by the others from `canvas.png` alone; every word is guessed; the honest per-seat views hold (drawer sees the word, guessers see blanks); the final podium is correct (round counter, drawer cleared — fixes #1/#2); no gameplay-bug regression.

Preserve the run's evidence (`commands.jsonl`, `host.log`, `state.json`, `canvas.png`, a sample seat view) as the prototype's playtest folder did.

### 4.4 Web verification (both backends + mobile + HiDPI)

- **Both wasm backends** — `tools/build-web.sh apps/dooduel` produces webgpu + webgl2 artifacts + the `navigator.gpu` loader; both boot + paint + are interactive (WebGPU on a real adapter, WebGL2 on SwiftShader). The four harness traps are documented (never `getContext` on the app canvas; drive the relative root loader; rebuild both artifacts; cold-synthetic-click needs move→settle→down→settle→up).
- **Mobile — a viewport-width-driven PROP swap between two hand-authored layouts (rev-2, finding #19 — corrected).** Dooduel does **not** use Protokit's sidebar↔bottom-nav shell reflow. It swaps a `layout: desktop | mobile` prop at a **≤430px** viewport width between two hand-authored layouts: the **mobile layout is a single stacked column** (top bar → header card with a **small timer ring, radius 12 vs the desktop 26** → horizontally-scrolling scoreboard strip → **~280px canvas vs the desktop ~460** → stacked toolbar → chat), and the **avatar editor renders as a bottom SHEET** rather than a centered modal. The canvas **re-inits/refits on the mode swap** (§3.2, #21). Touch drawing (proven zero-framework in the prototype), touch-tap button activation, and the soft-keyboard/IME via `WebImePlugin` all carry over.
- **Manual milestone gates, not per-wave CI (rev-2, finding N1).** Web-interactive drives, the **HiDPI browser verification at dsf=2/3 on a real device** (the F9 headless proxy is the per-wave CI proxy, §2.10/§4.1), and **real-device IME** (`ime_position`, touch-only policy) are **manual gates run at milestones** — no headless CI covers `#[cfg(wasm32)]` DOM behavior. The HiDPI fix is a hard *acceptance* gate; its *CI* form is the headless proxy.

### 4.5 The standing gates (every wave)

Headless full-workspace `cargo nextest`; the GPU `#[ignore]` lane (both legs); `cargo deny check` (with the documented base-triage + iai ignores from the MVU campaign); a `gallery_web`/`dooduel_web` wasm build; `RUSTDOCFLAGS="-D warnings" cargo doc`, `fmt`, `clippy -D warnings`. **RUN the GUI every wave** — headless green ≠ works (the standing Buiy lesson, proven repeatedly: the live crash, the toolbar overflow, the emoji tofu, the band lens, the click-swallowing occluder were all invisible headless).

---

## §5. Gate questions — RESOLVED (human gate 2026-07-03, skribbl.io-grounded)

**All questions below are DECIDED (rev-3).** The human direction: *use the orchestrator's recommendations, grounded against the real skribbl.io game.* Each original question is kept verbatim for the record; a **DECISION** block follows it, each carrying a **skribbl.io grounding** note. Grounding source: a 2026-07-03 research pass against the **official client** (`https://skribbl.io/js/game.js` + `style.css`, fetched live), the reverse-engineered protocol gist, and the Fandom/NamuWiki wikis. Confidence tiers: UI/protocol/turn-structure behavior is verified against the official client; **scoring is computed server-side and no exact public formula exists** (the client contains zero scoring math — the server broadcasts final per-turn deltas), so any clone's numeric curve is a design decision, not parity data. ((c) was already resolved in rev-2 and stands unchanged.)

**(a) Drawer payout formula — genuinely open.** The team-lead framing ("flat +100 vs skribbl-style scaling") is imprecise: the prototype **under-implemented** the design. The design-literal formula (`REQUIREMENTS-DELTA.md` §2) is **`drawerPoints = round(100 · correctCount / guesserCount)`** (0 if nobody guessed) — count-scaled, not flat +100. But: (i) the Game Spec prose says the drawer "Earns based on how many players guessed correctly **and how quickly**" — a *speed* term the pinned formula omits; (ii) even the count-scaled formula leaves the drawer at a **deficit** when everyone guesses (drawer = 100; guessers = `max(20, round((50+450·frac)·0.82^order))` ≈ 300–500), which the playtest confirmed (Sam guessed everything first-try and still placed last). **Decision needed:** implement the design-literal count-scaled formula as-pinned; add the "how quickly" speed term; or deviate to skribbl-style scaling to remove the drawer deficit? A game-design call.

> **DECISION (a) — v1 implements the design-JS-literal formula:** `drawerPoints = round(100 · correctCount / guesserCount)`, **0 if nobody guessed**. Neither the Game-Spec prose's "how quickly" speed term nor a rebalance is v1 scope. The playtest-confirmed **drawer deficit** (a perfect drawer maxes at 100 while guessers earn ≈300–500) is recorded as a **NAMED design-feedback follow-up — `dooduel-followup: drawer-payout-balance` (post-v1)**: revisit the drawer curve (add the speed term and/or rescale toward guesser parity) as a deliberate balance change ratified by the designer, never silently in v1.
> **skribbl.io grounding (2026-07-03 research):** the real game matches the formula's *shape* — the drawer scores **only if at least one player guesses** ("If the drawer left the game or nobody guessed, nobody scores points" — Fandom wiki), scaled by **how many guessed and how quickly** (Fandom + NamuWiki). The **exact numeric formula is not publicly documented** (server-side; the official client `game.js` contains zero scoring math), so there is no parity number to copy: the design-JS-literal count-scaled pin is correct for v1 — it matches skribbl's count axis + zero-if-none rule and omits only the speed term, which the named follow-up owns.

**(b) Hint-reveal schedule — pinned + already ported; confirm the values.** The schedule is pinned AND correctly implemented (`game.rs:396-414`): `i` in `1..=hintCount` (default 2), reveal at `floor(totalDraw · (0.6 − i·0.18)).max(1)` **seconds-left** (⇒ 33s & 19s at an 80s draw), random unrevealed `[a-z]` positions, capped `≤ letterCount−1`. It never fired live (fast guesses); fix #5 adds the missing crossing-semantics test + render verification. **Confirm the thresholds/counts are the intended values** — the FINAL tests them regardless (finding #15/#16).

> **DECISION (b) — CONFIRMED at the design values, as already implemented:** `hintCount = 2` default; for `i` in `1..=hintCount`, reveal at `floor(totalDraw · (0.6 − i·0.18)).max(1)` **seconds-left** (⇒ **33s & 19s** at an 80s draw); each reveal a **random unrevealed `[a-z]` position**, capped `≤ letterCount − 1`; **crossing semantics** under the poll clock (fire on first `seconds_left ≤ threshold`, latch the revealed count — never equality). Fix #5's crossing-semantics test + render verification ship as specced.
> **skribbl.io grounding (2026-07-03 research):** the real game's *mechanism* matches — letters are **server-pushed as `(position, letter)` pairs** (protocol packet 13 "Reveal Hint"; the client has no schedule), **2 reveals by default** on public servers, and private rooms configure a 0–N Hints setting — but its default *thresholds* differ: **50s and 25s remaining** at the 80s default (Fandom Words page; wiki-tier confidence). The parity target is the **design bundle, not skribbl**: the design pins a fraction-of-draw-time curve (`0.6 − i·0.18`), which scales with custom draw times where the wiki's fixed seconds do not. The design values stand; the 33/19-vs-50/25 delta is noted here so nobody later mistakes it for a porting error.

**(c) Turn X/N indicator — RESOLVED, no input needed.** The design HTML renders `Round {round} / {totalRounds}` (a round/total indicator), **not** a turn-of-N. The prototype's "Round 2/1" was the round-counter overflow bug (fix #1), not a missing design feature. No gap to decide.

**(d) Where the app lives long-term.** `apps/dooduel` (recommended, §3.1) vs `examples/dooduel` vs its own repo. The human ratifies the long-term home.

> **DECISION (d) — `apps/dooduel` (the §3.1 recommendation), ratified.** A **new top-level `apps/` tree**; Dooduel (with its `dooduel_web` crate and the `playtest_host` bin) is a **workspace member** under it; **`examples/` keeps only framework examples** (`buiy_gallery`, `counter_view`, `hello_*`). **Migration note:** add `apps/*` to the root `Cargo.toml` workspace members; the prototype's `examples/dooduel` + `examples/dooduel_web` shapes are re-implemented under `apps/dooduel/` per the §3.2 layout (nothing framework-owned moves; the `examples/` crates are untouched).
> **skribbl.io grounding:** n/a — a repo-layout decision with no gameplay-parity dimension.

**(e) PR merge cadence / authorization + the two crate-boundary crossings.** How the F1–F9 general-purpose PRs land: one at a time, each gated + human-authorized (the project's default), or bundled? Do the framework PRs merge before the app lands, or does the app ride on a stack? **Two PRs cross their nominal crate boundary and want ratification (findings #4, #6):** (i) F3 folds the borderless-rounded fill (`buiy_core`) into the mostly-`buiy_view` styling PR — acceptable, or keep all render in F4? (ii) **F2 now spans `buiy_core`** because the `.fixed()`→viewport anchor is a layout-semantics change — acceptable inside the layout-surface PR, or split the `Fixed`-anchor change into its own `buiy_core` PR?

> **DECISION (e) — the F1–F9 + app series lands as specced: one PR at a time in the §6.1 order, each with its own gate.** Both crate-boundary crossings are **ratified as specced**: (i) the **F3 borderless-rounded-fill `buiy_core` fold is ACCEPTED** — small and self-demonstrating (the styled pills F3 introduces actually render rounded in the same PR); (ii) F2 keeps the `.fixed()`→viewport anchor change inside the layout-surface PR (small, well-isolated, proven by the fixed-vs-viewport layout snapshot). **Pushes + PR-opening are AUTHORIZED by the human as of 2026-07-03; merges remain individually human-gated** (the project default — authorization to merge one PR does not carry to the next). The framework PRs merge before the app lands. **The lineage-docs PR is the sequenced precondition and is in flight** (front matter; §8 finding H2).
> **skribbl.io grounding:** n/a — a process/cadence decision with no gameplay-parity dimension.

**(f) "Exact parity" scope vs the character-layer residuals.** The acceptance bar is "exact design parity," but the prototype reached *strong* parity with a bounded residual list. Does v1 require closing **all** character-layer residuals — color emoji (needs a COLR/CBDT/sbix pipeline or an outline-emoji font — potentially a net-new subsystem), dashed borders (the F4b stipple — **gates F4b's dashed sub-item**), per-axis elliptical wobble, radial-gradient backgrounds, the rotated ribbon, and the **crisp zero-blur 3D-press edge (gates the F4b shadow-corner-radius instance, §2.5.1)** — or is strong-parity-with-enumerated-residuals acceptable for v1? This directly scopes F4b. **The smooth timer ring is REMOVED from the residual list (rev-2, finding #23):** the design binds `strokeDashoffset` to a per-second `frac` driven by `setInterval(…, 1000)` with **no CSS transition on the ring stroke**, so the design ring **steps once per second** — the prototype already matches it; there is nothing owed.

> **DECISION (f) — v1 parity INCLUDES:** **dashed borders** (the F4b stipple sub-item is IN — finding M2's gate closed in favor of inclusion), **the press-down 3D-press animation** (F5's interaction-state visual layer carries the press state, §2.6), and **the crisp zero-blur rounded shadow** — so the **§2.5.1 contingency RESOLVES to: build the dedicated rounded-shadow instance (Option B) in F4b**. **Color emoji (COLR/CBDT/sbix) is DEFERRED as its own future campaign** — a genuine net-new subsystem (the coverage rasterizer is monochrome R8), not a Dooduel sub-item. **Radial-gradient backgrounds, the rotated ribbon, and per-axis elliptical wobble corners: attempt-if-cheap during F4b, defer-with-note otherwise** (each is character-decoration; if the attempt exceeds a small budget, record the residual and move on). Everything else on the residual list is closed by the F-series as specced. §2.5, §2.5.1, §4.1, and §6 are updated to reflect this resolution.
> **skribbl.io grounding (2026-07-03 research):** the parity target is the **Dooduel design bundle**, not skribbl's visual chrome — skribbl grounds *behavior*, the bundle grounds *look*. The research confirms the behavioral surface v1 already covers matches the real game (strokes stream live and incrementally — protocol packet 19 + a client-side command queue; drawer tools = brush sizes/fill/undo/clear + a color palette; guess feedback = green player-row + broadcast chat line), and **none of the deferred character items** (color emoji, radial gradients, the ribbon, elliptical wobble) has a skribbl behavioral counterpart — deferring them costs no gameplay parity.

**(g) Podium pedestal heights — deviate or match the literal design? (rev-2, finding #17 — promoted from §7).** The design's `PODIUM_H = ['92px','124px','72px']` is indexed by **rank**, and `PODIUM_ORDER = [1, 0, 2]` places 2nd-left / 1st-center / 3rd-right — so the design **literally renders 2nd place with the TALLEST pedestal (on the left)**, not the winner. The prototype "corrected toward intent" (winner's center pedestal tallest, `PEDESTAL_H = [124,92,72]` by place) — a **visible deviation from the literal design**. **Decision needed:** match the literal design (2nd-place-tallest quirk) for exact parity, or keep the winner-tallest correction? A parity-vs-intent call the human should make, not the spec.

> **DECISION (g) — winner CENTER + TALLEST, as an INTENTIONAL, DOCUMENTED DEVIATION from the literal design code.** The design's rank-indexed `PODIUM_H` under `PODIUM_ORDER = [1, 0, 2]` literally renders **2nd place tallest (left)** — read as an apparent indexing quirk in the design JS, not intent. The FINAL renders the prototype's correction: **1st place center on the tallest pedestal (124px), 2nd left (92px), 3rd right (72px)**. This deviation is recorded here (and must be carried into the parity-verification notes) so an exact-parity eyeball against the design HTML does not mis-flag it as a regression — and it is **flagged for the designer to RATIFY**; if the designer insists on the literal quirk, reverting is a two-constant change.
> **skribbl.io grounding (2026-07-03 research — verified from the live official client `game.js` + `style.css`):** skribbl's end-of-game podium places **`.podest-1` gold at CSS `order:1` = CENTER and TALLEST (40% border height vs 30% for the others)** with a **trophy GIF** behind the winner's avatar and a looping winner-bounce animation; silver 2nd sits at `order:0` = LEFT (30%), bronze 3rd at `order:2` = RIGHT (30%). The real game — like general podium convention — presents the **winner most prominently**, confirming the design code's 2nd-tallest-left as a quirk and grounding the deviation toward winner-center-tallest.

---

## §6. Sequencing + risks

### 6.1 Sequencing

Dependency-ordered. **Rev-2 corrects the rev-1 parallelization claim (finding M1): most render work SERIALIZES.** F1, F3's borderless-rounded fill, F4a, F4b, and F9 **all touch `buiy_core` render** (extract/instance/`node.rs`/shaders/scale-factor) and cannot land concurrently without churn; **F2 also now reaches `buiy_core`** (the `.fixed()` anchor, #4). Only **F2 (mostly `buiy_view`) and F7 (`buiy_core::mvu` module — a different subtree)** genuinely parallelize with the render line.

```
Two independent lanes up front:
  LANE A (buiy_view/mvu, parallel):   F2 (layout; small buiy_core Fixed-anchor touch) ‖ F7 (mvu clock)
  LANE B (buiy_core render, SERIAL):  F1 (raster) → F3-fill → F4a (interleave) → F4b (shape)
                                      F9 (HiDPI, buiy_core scale-factor — serializes with LANE B; start its
                                          investigation early but LAND it in the render line)
                                                          ↓
  After F1+F2:  F3 (styling; needs F1 fill + F2 modifiers) · F5 (press; needs F2) · F6 (picking; needs F2)
  Anytime:      F8 (buiy_verify; independent)
                                                          ↓
The app (apps/dooduel)    ports game.rs + rebuilds the views on the landed surface;
                          fixes gameplay bugs #1–#6; graduates playtest_host
                                                          ↓
Acceptance                the repeatable 4-agent playtest + web (both backends + HiDPI) + GPU both legs
```

**Rationale.** F2 (layout) and F7 (clock) are the only true early parallelism — different crates/subtrees. The **render line is serial**: F1 lands the raster primitive + `ExtractedNode.radius` is introduced by F3, then F4a's interleave and F4b's shape fixes build on the same files, so they queue behind each other (**the F3→F4b edge, #6**). F5/F6 depend on F2. F8 is independent. **F9 is investigated early but lands in the serial render line** (it edits the same `buiy_core` scale-factor seam), not truly parallel. **F4b's scope is decided (§5.f, rev-3): dashed + crisp-shadow are IN** — F4b is the biggest render PR; the dashed stipple is the pre-identified cleave if it must split (finding M2, closed).

### 6.2 Risks (biggest first — what to de-risk early)

1. **The serial `buiy_core` render line (rev-2 headline risk, finding M1).** F1 → F3-fill → F4a → F4b (+ F9) all edit the same render files and cannot parallelize. **Mitigation:** sequence them explicitly (§6.1); keep each change additive + byte-stable (the disjoint-node-set discipline of #2) so a later render PR rebases cleanly on an earlier one; F4b now carries the full §5.f-decided slate (dashed stipple + the rounded-shadow instance), so split the dashed stipple out if the tail PR drags.
2. **F9 HiDPI unknown depth.** A web/wgpu scale-factor bug of unknown root cause; could be shallow or a deep surface-reconfigure seam. It **blocks the mobile v1 target** and **serializes with the render line** (same `buiy_core` seam). **Mitigation:** investigate early (the headless proxy, §2.10/§4.1, de-risks it before a browser); verify at dsf=2/3 on a real device at a milestone.
3. **The interaction-state visual layer (F5) — depth unknown (rev-2, re-scoped, finding #1).** `ControlledLeaf` is NOT the vehicle (it's an opt-out toggle-ownership marker) and `Button` has zero press-visual machinery, so the layer is **net-new**: a widget-runtime-owned `InteractionState` (hover/press) applied outside the pure view. **Risk:** does it need a full hover/press/focus transition engine or does a discrete-state resolver suffice? Designed jointly with the F2 scroll stick-to-bottom (same runtime-state↔pure-view class). **Gate must attack** the layer's depth.
4. **F4b's one dedicated-instance item (shadow corner radius) — DECIDED, not a broad stride change (rev-2 corrected; rev-3 resolved).** Rev-1 over-scoped this to per-corner fill + shadow needing a stride change; #2 corrected it — the fill reuses an existing slot, the lens is shader-only. **§5.f keeps the crisp edge in v1, so the crisp-3D-press shadow radius gets its dedicated instance (Option B, §2.5.1).** **Mitigation:** implementation review **must attack** its composition with the F4a interleave + the effect-group compositor.
5. **Multi-App topology unretired.** The single-model file-protocol host MET the bar, but a true networked / multi-instance topology is unproven (no process-global state blocks N Apps in one process, but nothing has run concurrent Apps). **Mitigation:** the single-model host is the v1 playtest topology; multi-App concurrency is a **deferred, separately-scoped investigation** only needed if a networked future is pursued — do NOT block v1 on it. Flag it as a residual so a networked future doesn't discover it late.
6. **Emoji strategy — RESOLVED for v1 (§5.f, rev-3): color emoji is DEFERRED as its own future campaign.** The coverage rasterizer (monochrome R8) can't do COLR/CBDT/sbix — a real pipeline addition, correctly scoped as a standalone campaign, not a Dooduel sub-item. The risk is retired for v1 (an outline-emoji font through the existing coverage path remains the cheaper option when that campaign runs). *(The smooth timer ring is no longer a residual — finding #23, §5.f: the design ring steps per-second too.)*
7. **Two crate-boundary crossings (§5.e).** F3 folds borderless-rounded fill (`buiy_core`) into a `buiy_view` PR; F2 reaches `buiy_core` for the `.fixed()` anchor. **Mitigation:** both are small, well-isolated changes; splitting either into its own `buiy_core` PR is one merge-cadence decision away (§5.e).

---

## §7. Re-decisions vs the prototype + rejected alternatives

**The three mandated re-decisions (retro §1):**
- **Per-raster-anchor interleave** (F4a) over the top-layer-suffix split — retires the contiguous-suffix assumption + the non-top-layer-overlay-under-canvas limit + raster-under-**opaque**-modal (an effect-group-nested raster stays a documented follow-up, §2.4).
- **Poll-clock-as-Msg `ClockPlugin<M>`** (F7) over the wished-for edge-triggered `Cmd::interval`/`Cmd::timeout` — the prototype proved the edge-triggered timer is the wrong shape (hard to replay, hard to keep `set_if_neq`-clean).
- **The one coherent layout surface** (F2) designed in a single pass over the wave-by-wave accretion — including the four always-worked-around gaps (flex-wrap, per-side padding, text-align, inset/absolute) + the #1 scroll container from the start.

**Rejected alternatives (named, with the reason):**
- **GPU accumulation render-target for the canvas** — flood fill needs to *read* pixels (a GPU-RT makes it a readback); undo/serialize/persist all want the buffer on the CPU. The RT only saves the re-upload (a perf refinement, not a capability). CPU-authoritative buffer wins.
- **The reserved `Path` vector channel for the canvas** — freehand+fill+erase is semantically raster; flood-fill/eraser are wrong on a vector scene; no Path shader exists. `Path` stays reserved for real vector art.
- **A new `BuiyPrimitiveKind` for the raster** — the closed enum + byte-stable quad path stay untouched; a distinct pipeline keyed by record (band/gradient precedent) is the extension pattern.
- **Mount-under-ECS-shell for the canvas** — the `raster()` element beat it decisively (one Kind + one reconciler arm; keyed/when/lifecycle free; the handle rides the replayable model). The hatch is only for a genuinely foreign surface (a 3D viewport).
- **A `PaintSurface` component on the canvas node** — the reconciler despawns the node on screen-swap (a component loses the pixels); the keyed `Resource` map survives + advertises N-canvas scaling.
- **Widen the shared `PackedInstance`** for the shadow corner radius (F4b; §5.f kept the crisp 3D-press edge, so the instance IS built) — ripples through the byte-stable quad + shadow + text path; a dedicated rounded-shadow instance (Option B) is safer and is the decided form. (Rev-2: the per-corner *fill* radius does **not** need this — it reuses the existing slot on a disjoint node set; the lens is shader-only — finding #2.)
- **`ControlledLeaf` as the press-visual vehicle (F5)** — rev-1's mis-cite; `ControlledLeaf` is an opt-out toggle-ownership suppression marker (`mvu/leaf.rs:33-49`), unrelated to press *visuals*. The FINAL uses a net-new widget-runtime-owned interaction-state layer (finding #1, §2.6).
- **Edge-triggered `Cmd::interval`/`Cmd::timeout`** — retro-proven wrong shape (see above).
- **`bevy_state` for the phase machine** — not compiled into the workspace at all; the phase machine goes in MVU (the Menu-machine precedent), which the prototype proved clean.
- **A per-particle `OpacityTween` fade** — forms ~110 off-screen EffectGroups; a cheap per-quad alpha (F4b) instead.
- **The podium-height literal design code** — rev-1 buried a "correct toward winner-tallest intent" call in this list; the design *literally* renders 2nd-place-tallest, so rev-2 promoted it to a human question (§5.g, finding #17). **Rev-3 resolved it: winner-center-tallest as an intentional documented deviation** — grounded by the real skribbl.io podium (gold `.podest-1` center + tallest + trophy, verified from the live client), pending designer ratification (§5.g).

---

## §8. Provenance

- **Seed — the Phase-A learning gate:** the prototype retrospective (`../prototypes/2026-07-02-dooduel-PROTO1-RETROSPECTIVE.md`) + journal (`../prototypes/2026-07-02-dooduel-PROTO1-journal.md`, W0→W8-live, each RUN + verified) + playtest evidence (`../prototypes/2026-07-02-dooduel-PROTO1-playtest/`).
- **Campaign contract:** the charter + 2026-07-02 amendment (`../prototypes/2026-07-01-scribbl-campaign-charter.md`).
- **Capability grounding (superseded where the retro corrected it):** the rebaseline audit (`../reports/2026-07-02-dooduel-rebaseline-audit.md`).
- **Parity target:** the design bundle `../reference-designs/dooduel/` (`Dooduel Prototype.dc.html` — the parity target; `Dooduel - Game Spec.dc.html`; `DoodleAvatar.dc.html`; the protokit token bundle) + `REQUIREMENTS-DELTA.md`.
- **House-style + framework grounding:** the MVU-as-core FINAL (`2026-06-29-mvu-as-core-design.md` — §8 Subscription roadmap, §11 the `set_if_neq` no-cascade gate); the view-authoring design (`2026-07-01-buiy-view-authoring-design.md` — `ControlledLeaf`, the `buiy::view` sub-prelude); the verification conventions (`using-buiy-verification`, `docs/specs/2026-06-15-buiy-verification-design/`).
- **Prototype framework code (unmerged reference, `worktree-dooduel-proto1` off `a969cbf`):** `crates/buiy_core/src/render/{raster.rs,raster.wgsl,extract.rs,buckets.rs,node.rs,band.wgsl}`, `crates/buiy_view/src/{element.rs,reconcile.rs,tokens.rs}`, `crates/buiy_verify/src/pointer.rs`, and the app in `examples/dooduel` + `examples/dooduel_web`.
- **Docs debt = a merge PRECONDITION, not a follow-up (rev-2, finding H2).** The retrospective + journal + playtest evidence + charter + rebaseline audit + `reference-designs/dooduel/` currently live **unmerged** on `worktree-scribbl-campaign` / `worktree-dooduel-proto1`, so **every relative seed link in this spec dangles until they land**. Per `prototype-first-development`, a **lineage-docs PR MUST land before or with this spec** — and it stamps the **superseded-by back-pointer onto the rebaseline audit** (this spec supersedes its build list). This spec is not merge-ready until then (restated from the front-matter precondition).
- **Memory:** `buiy-scribbl-campaign`.

### §8.1 Revision-2 change log (gate findings applied)

A 3-reviewer gate (architecture CHANGES-REQUIRED / verification CHANGES-REQUIRED / design-fidelity APPROVE-WITH-NITS) reviewed rev-1. All 23 findings were **re-verified against `file:line` / the design HTML** (the four load-bearing ones — `Sizing` has no `Percent`; `BorderBandInstance` already carries per-corner elliptical radii; `pack_shadow` reuses `radius` as blur sigma; `ControlledLeaf` is an opt-out marker — were confirmed directly) and applied:

- **Architecture:** #1 F5 dropped `ControlledLeaf`, made the widget-runtime interaction-state layer primary (§2.6). #2 §2.5.1 rescoped to shadow-only + contingent; lens = shader-only, fill = existing slot (§2.5). #3 scroll = controlled stick-to-bottom (§2.2). #4 F2 widened to `buiy_core` for the `.fixed()`→viewport anchor + a fixed-vs-viewport test (§2.2, §5.e). #5 F4a retires raster-in-**opaque**-modal only; effect-group-nested raster = documented follow-up; avatar panel must be opaque (§2.4, §3). #6 F3→F4b `ExtractedNode.radius` edge added (§2.3, §6). #7 `.fill()`→`Length(Percent(100))` decided + `.center_self()` disambiguated (§2.2).
- **Verification:** #8/H1 rounded-fill corner → the Tier-4 SDF cross-check (zero stored bytes); goldens scoped (§2.1, §2.5.1, §4.1). #9/H2 lineage-docs PR = precondition (front matter, §8). #10/M1 parallelization corrected — the render line serializes (§6.1). #11/M2 F4b split resolves from §5.f (§2.5, §6). #12/M3 auto-`IGNORE` foregrounded + Tier-3 occluder invariant (§2.7, §4). #13/M4 headless HiDPI proxy added (§2.10, §4.1). #14/N1 web/HiDPI/IME = manual milestone gates (§4.4); N2 `RUST_MIN_STACK`/`CARGO_BUILD_JOBS` as named `ci.yml` env (§3.4).
- **Design-fidelity:** #15/#16 hint schedule already ported (`game.rs:396-414`); fix #5 = missing crossing-semantics test + render verify; precise formula (1-based `i`, `.max(1)`, 33s/19s, random `[a-z]`, crossing not equality) (§3.3, §5.b). #17 podium 2nd-place-tallest promoted to §5.g. #18 fix #4 three-way feedback (shared wrong-guess / private near-miss toast / hidden-word exact) (§3.3). #19 mobile = viewport-width prop-swap, not Protokit shell reflow (§4.4). #20 round strings "Round r / t" (desktop) vs "Round r of t" (mobile + chat) (§3.3, §5.c). #21 canvas clear on Drawing entry + refit on mode swap (§3.2). #22 "8 bugs" → 6 code fixes + 2 product items (§3.3). #23 smooth timer ring removed from residuals — design ring steps per-second too (`setInterval(…,1000)`, no CSS transition) (§5.f, §6).

### §8.2 Revision-3 change log (human gate 2026-07-03 — the §5 questions resolved, skribbl.io-grounded)

The human gate resolved every §5 open question, directing: *use the orchestrator's recommendations, grounded against the real skribbl.io game*. A dedicated 2026-07-03 research pass (the **official client** `game.js`/`style.css` fetched live + the reverse-engineered protocol gist + the Fandom/NamuWiki wikis) grounds each decision; every §5 entry keeps its original question verbatim and now carries a DECISION block with a skribbl.io-grounding note. Key research fact: **skribbl's scoring is server-side with no public formula** — behavior is multi-source verified, numbers are not, so the design bundle's pinned values are the correct parity source for anything numeric.

- **(a)** drawer payout = the design-JS-literal `round(100 · correctCount / guesserCount)`, 0 if none; the playtest's drawer deficit → **named post-v1 design-feedback follow-up `drawer-payout-balance`** (skribbl grounding: count-scaled + zero-if-none matches the real game's shape; its exact formula is unpublished, so no parity number exists to copy).
- **(b)** hint schedule **CONFIRMED at the design values** (2 hints; 33s/19s-left at 80s via `0.6 − i·0.18`; random `[a-z]`; crossing semantics). Skribbl grounding: same server-pushed `(position, letter)` mechanism + 2-by-default, but public skribbl reveals at 50s/25s — the design's draw-time-scaled curve wins as the parity target; delta noted.
- **(c)** unchanged (already resolved in rev-2 — the round indicator is a rendering-correctness fix, not a feature gap).
- **(d)** app home = **`apps/dooduel`** — new top-level `apps/` workspace tree; `examples/` keeps only framework examples; migration note added (§3.1, §5.d).
- **(e)** the F1–F9 + app series lands **as specced, one PR at a time**; the F3 borderless-rounded-fill `buiy_core` fold **ACCEPTED** (small, self-demonstrating) + F2's Fixed-anchor crossing ratified; **pushes + PR-opening AUTHORIZED 2026-07-03, merges per-PR human-gated**; the lineage-docs PR = the in-flight sequenced precondition.
- **(f)** v1 parity **INCLUDES dashed borders (F4b stipple IN) + the press-down 3D-press (F5) + the crisp zero-blur rounded shadow → §2.5.1 RESOLVES to Option B: build the dedicated rounded-shadow instance in F4b**. Color emoji **DEFERRED as its own future campaign** (genuine subsystem). Radial-gradient backgrounds + rotated ribbon + per-axis elliptical wobble = **attempt-if-cheap during F4b, defer-with-note otherwise**. §1.3, §2.2 (`.rotate` row), §2.5, §2.5.1, §4.1, §6.1, §6.2 updated accordingly.
- **(g)** podium = **winner center + tallest as an INTENTIONAL DOCUMENTED DEVIATION** from the design code's literal rank-indexed heights (an apparent indexing quirk), pending designer ratification. Skribbl grounding: the live client's `.podest-1` gold podium is **center (`order:1`) and tallest (40% vs 30%)** with the trophy + winner-bounce — the real game presents the winner most prominently (§5.g, §7).
- Status flipped `draft` → **`approved`** (front matter); the `docs/README.md` index entry updated to match.
