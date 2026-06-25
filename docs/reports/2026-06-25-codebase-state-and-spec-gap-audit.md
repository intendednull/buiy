# Codebase state & spec-gap audit

**Date:** 2026-06-25
**Scope:** Whole-codebase audit of `origin/main @ fdb8dda` (all merged work through
PR #80) **plus** the in-progress Widget-Catalog parity prototype/FINAL campaign.
**Method:** a 13-agent parallel audit (one auditor per subsystem + the catalog
prototype, the FINAL build, and the deferred backlog, then a completeness critic
cross-checking the foundation feature inventory), ~1.13 M tokens, with the
load-bearing claims (stale `main`, the in-flight parity work) verified by hand.
**Audience:** Buiy maintainers deciding what to build, merge, and track next.

## Verdict

The **built subsystems are in strong shape and track their specs faithfully.**
Layout, render, text, text-editing, verification, the a11y/agent-interface
substrate, and BSN are all landed with real two-lane (headless + GPU) test
coverage, and the spec-vs-code gaps *inside* them are almost all small and
explicitly tracked in `docs/plans/follow-ups.md`.

**The big gaps are not inside those subsystems — they are the negative space.**
Large parts of the foundation's headline vision (images/media, most of the APG
widget set, the DOM-style event model, gamepad, drag-and-drop, the animation
surface, OS-preference binding, the agent MCP transport) have little-to-no code,
and several are not even on the tracked roadmap. Separately, the verification
*breadth* is thin under an excellent harness, and the catalog parity FINAL build
is not yet merge-ready — its Wave-4 shell/screens landed mid-audit (`b881008`)
but Waves 5–6 and the human-review-gated PR remain.

## Two ground-truth corrections

Both surfaced during the audit and both matter for anyone reading the tree:

1. **`origin/main` is at `fdb8dda` (PR #80); the local `main` checkout was 72
   commits behind** — it predates #69–#80 (layout/text/render follow-up drains,
   BSN + Bevy 0.19, the testing-infra audit, CI hardening, agent-interface
   Phase 0, and the widget-catalog co-drive). Work or review done against a local
   `main` was looking at a month-old tree. Reconcile before further work.

2. **Agent-interface is further along than the campaign memory records.** The
   "only Phase 0 landed" note is stale: **Phases 0 and 1a–1d** (the decomposed
   semantic-tree component model, ACCNAME, the inbound action router + per-role
   contract registry, and the in-process inspect/control driver) are **all on
   `main`** as of #80. Only Phase 2 (the `buiy_mcp` companion) is unbuilt.

## Subsystem status (shipped `main` @ fdb8dda)

| Subsystem | Status | One-line |
|---|---|---|
| Foundation / Phase-0 substrate | **mostly-landed** | BuiyPlugin, `BuiySet` chain, focus, picking, interaction, theme — all real & unit-tested |
| Layout (14 phases) | **mostly-landed** | Taffy bridge + CSS box/flex/grid/positioning/stacking/transforms/containment/CQ/writing-modes |
| Render pipeline (R1–R11) | **landed** | Full extract→prepare→node-draw spine + effect compositor + GPU atlas; GPU-verified on real hardware |
| Text rendering (T1–T9) | **landed** | cosmic-text engine, Taffy measure seam, glyph producer, fonts/fallback/bidi, decoration/caret |
| Text editing & IME (E1–E6) | **landed** | Editor substrate, per-platform keymap, selection, clipboard/undo, IME splice, `TextInput` widget |
| Verification harness | **mostly-landed** | All 5 tiers real & anti-vacuity-tested — but the *case corpus* is sparse (see § Big gaps) |
| Agent interface | **partial** | P0–P1d landed (semantic tree, router, in-process driver); **P2 / `buiy_mcp` = 0%** |
| BSN authoring | **landed** | Thin `bsn!` re-export + `#[require]` contracts + 14 scene-fns; exceeds its spec |
| Widget catalog / gallery | **mostly-landed** | 5 screens + 12 widgets real; the fixture-enrollment verification thesis is unbuilt |

Within these, the spec-vs-code gaps are **mostly tracked deferrals**, not
surprises (full list in the appendix). The ones worth naming up front:

- **`transform-origin` is silently ignored** — `compose_transform`
  (`layout/systems.rs`) rotates/scales about the box-local **top-left**, not the
  CSS-default 50%/50% center. Render transports the affine exactly as
  `GlobalTransform` encodes it, so paint and hit-test agree, but the result is
  visually wrong vs CSS for any non-identity rotate/scale. Tracked as
  `follow-ups.md` "Residual A". **Medium.**
- **The `Length` unit surface is a minority of the spec's target.** Only
  `Px/Percent/Fr/Cq*/AnchorSize` exist; `Em/Rem`, the viewport family
  (`Vw/Vh/Svw/Lvw/Dvw/…`), and `calc()` are absent from the enum entirely — so
  authoring code cannot express them at all. This also blocks sticky em/rem/V*
  insets. Honestly tracked (the layout spec self-demotes to `active` for exactly
  this reason). **Medium.**
- C-tier render shaders (filter, mix-blend-mode, rounded-rect clip, inset
  shadow, dashed/dotted/double borders) are **reserved-not-built**; multi-cursor
  *behavior* is deferred (the `TextSelection` type is already multi-range-shaped);
  table colspan/rowspan + caption, multicol fragmentation, subgrid/masonry, and
  per-window top-layer are stubbed-and-warned.

## The catalog (parity) prototype work — current state

A **3-phase prototype-first effort** to reach *exact* visual parity with a Claude
"Widget Catalog" design (dark Geist IDE-style 3-pane shell, 5 switchable screens,
live accent re-theming):

- **Prototype** (`parity-prototype`, **complete, deliberately not merged**): built
  and GPU-proved on real hardware (RX 6700 XT) — a dark token set + accent-ramp
  swap, a `Tween`/`Easing` animation system, embedded Geist + Geist Mono +
  `LetterSpacing`, **gradients, dotted-grid backgrounds, vector (lyon→atlas)
  icons, and backdrop-blur** (the Bevy 0.19 `ViewTarget` post-process seam turned
  out to be cleanly reachable, not vaporware), the shell + `ScreenRouter`, and all
  5 restyled screens. Along the way it surfaced **real production bugs**: an
  icon-only-frame paint skip, the `LetterSpacing` em-vs-px lowering bug, a missing
  `color.shadow.card` token, and the `extract_buiy_nodes` query hitting Bevy's
  15-tuple `QueryData` bound. The journal is the seed deliverable for the FINAL.

- **FINAL** (`parity-final`, **in progress, merge-gated on human review**): a
  re-decided clean rebuild off `origin/main`. **Committed (Waves 2–3):** the dark
  theme + accent swap, the animation module, Geist + `LetterSpacing` (with the
  em-vs-px bug **fixed**), prelude promotions, the ported render caps
  (gradients/dotted-grid/icons/backdrop-blur), the M1/M5/M6 paint fixes, and a
  `NodePaintQuery` extract refactor escaping the 15-tuple bound.

### The FINAL: Wave-4 landed mid-audit; Waves 5–6 remain

When the audit data was first captured (earlier on 2026-06-25), the entire
Wave-4 parity restyle — `shell.rs` (1,663), `composites.rs` (1,581),
`inspector.rs` (1,183), `lib.rs` grown 1,257 → 5,829, plus the dark-theme
`main.rs`, capture bins, blessed snapshots, and 4 modified `buiy_verify`
acceptance tests — was **staged but in no commit** (`HEAD` ended at Wave 3),
leaving it both unprotected and invisible to the audit trail. **That risk has
since been resolved:** the work was committed as `b881008` ("port the unified
shell + ScreenRouter + 5 screens + composites + inspector") during the audit
window, and Wave-5 work (promoting the general composites into `buiy_widgets`) is
now in flight. This audit did not independently re-run the full workspace gate on
`b881008`, and the campaign remains **merge-gated on human review** — Waves 5–6
(composite promotion, a headless plugin, lavapipe-blessed CI goldens, the parity
audit, and the PR) are not yet complete, so the FINAL is not yet merge-ready.

One spec divergence to reconcile on the way to merge: the FINAL's `extract`
refactor took a single-system `NodePaintQuery` projection, while the spec marks a
five-producer-system split as "binding" — the code reaches the same goal (escaping
Bevy's 15-tuple `QueryData` bound) more simply, but the spec should be updated or
the systems split.

## The big gaps vs the specs

Ranked. The first two are the genuinely large ones; the rest are real but
narrower. Severity is the auditor's, "tracked?" says whether the gap is already
recorded as a deferral somewhere.

### 1. Whole foundation feature-areas have zero/minimal code — several untracked  · **HIGH**

The foundation spec is explicitly a long-horizon, tiered (F/C/E/O) feature
inventory, so forms/devtools/3D-UI/i18n being absent is *expected* — they are
future sub-spec rows on the §4 roadmap. But these **headline-goal** areas have no
implementation **and no clear tracking**:

| Area | Spec | State | Tracked? |
|---|---|---|---|
| **Media & images** (`Image`, object-fit, Canvas2D, SVG) | §3.9 / §3.10 (Image is **F-tier**) | Zero code. parity-final added vector *icons* only — no general image/SVG path | **No** — omitted from the foundation deferred list & roadmap |
| **APG widget set** | §3.10 (~30 F-tier widgets) | 12 widgets exist (gallery-sized). Absent: Link, Heading, Label, RadioGroup, Listbox, Combobox, Select, Spinbutton, Tabs, Progressbar, Alert/Status/Toast | Partially — gallery scope is tracked, the framework-set gap is not framed |
| **DOM-style event model** | §3.7 | Only `bevy_picking` + the text-edit path; no capture/bubble/`stopPropagation`, gestures, touch, or keyboard-chord registration | **No** — `buiy-input-events-design` not graduated |
| **Gamepad input + drag-and-drop** | §3.7 | Zero hits for either; both carry F-tier obligations (gamepad is core to "game *and* app") | **No** (only "spatial gamepad nav" is folded under the deferred focus model) |
| **Animation surface** | §3.8 | A minimal `Tween`/`Easing` primitive only; no transitions/keyframes/view-transitions/spring/scroll-driven; no WCAG-2.3.1 flash gate | **No** — animation absent from the foundation deferred list |
| **Global live-region announcer** | architecture §2.3 | Per-node politeness resolves, but the announcer resource the architecture promises does not exist; no Alert/Status/Toast widgets | **No** |
| **OS-preference binding** | architecture §2.5 / §3.14 | `UserPreferences` is a default resource nothing populates from winit/OS — every "forced-colors / reduced-motion / color-scheme honored automatically" claim is structurally inert until an app sets it by hand | Partially (the *reader* is noted deferred; the *consequence* is under-rated) |

These are the heart of the answer to "are there big gaps vs the specs?" — yes,
and the most important ones are the areas the project's own tracking does not yet
surface.

### 2. Agent-interface Phase 2 (the MCP companion) is 0% built · **HIGH (in scope; intentionally gated)**

No `buiy_mcp` crate, no socket transport, no MCP tool envelope, no
handshake/auth/capability gating, no push tree-deltas. The "one semantic tree →
screen-reader + tests + **LLM agent**" thesis ships only two of its three
consumers. It is explicitly gated behind your go-ahead (and a named prior-art
research debt — no MCP/Playwright-MCP/BRP prior-art folders yet), so it is a
deferred scope boundary, not a defect — but it is the bulk of the gap to that
spec's vision.

### 3. Verification breadth is thin under a strong harness · **MEDIUM–HIGH**

The 5-tier pyramid is real and rigorously anti-vacuity-tested *as machinery*
(mutation fixtures with teeth, RED/GREEN reftest truth tables, fail-closed golden
panics). But the **content riding it is sparse: ~2 reftest pairings and 2 blessed
golden cells** (rect-rounded, text-ahem); the named residue classes (shadow
kernel, color-emoji, atlas, compositor, blend/gamma) are unblessed or
renderer-blocked. Critically, the widget-catalog spec's core thesis — each
gallery screen auto-enrolled as a `buiy_verify` fixture across all tiers with one
canonical GPU golden — **was not built** (no `fixtures/gallery/`, no
`Matrix::gallery_screen()`; verification instead lives in hand-written test files
+ gallery-local layout snapshots, the alternative the spec explicitly rejected).
Consequence: composed render-correctness has weak automated gating — which is
exactly how **two live bugs (an editor unshaped-at-extract crash and totally
invisible content text) shipped past 1,653 green headless tests** until the
gallery was first run by hand on 2026-06-24.

### 4. Systemic wholesale-rebuild scaling pressure · **MEDIUM**

Three subsystems independently rebuild-everything each frame — `ExtractedGlyphs`
on any text damage, render node instances (no per-entity patching), and the full
AccessKit tree (~6 ms/idle-frame; the 1000-row screen burns ~22 ms/idle-frame in
debug). There is no incremental / change-detection-gated extract or tree-diffing
anywhere. It is one architectural pattern and a real scaling ceiling, currently
flagged piecemeal rather than as a single cross-cutting risk.

## Documentation drift

The code is consistently **ahead of the spec status flags** — a cheap cleanup
pass would realign them:

- **Stale `[draft]` statuses on landed specs.** All 8 agent-interface child specs
  and 7 of 8 widget-catalog child specs are still `[draft]` despite landing on
  `main` via #80; the widget-catalog README banner still reads "awaiting final
  PR… not yet merged."
- **Aspirational present-tense in the foundation spec.** `architecture.md`
  §2.3/§2.5/§2.8 describe themes-as-assets, automatic OS-pref→variant binding, and
  a `buiy_text/animation/forms/devtools` crate split as current capability — none
  match reality (5 crates; theme is a plain resource; no OS reader).
- **Render-pipeline spec is `[active]`, not `[draft]`,** and is accurate against
  `main` — but its "C-tier caps absent, that's why this is active" clause will go
  stale the moment parity-final merges (it implements several of them).
- **Stale `follow-ups.md` entries.** A few items marked "still deferred /
  renderer-blocked" have actually landed (e.g. the R11 forced-colors `BoxShadow`
  draw-skip suppression is live in `resolve_shadows`; only the *visual reftest*
  remains blocked).

## Recommendations (in order)

1. **Finish and merge the parity FINAL.** Its Wave-4 shell/screens landed
   mid-audit (`b881008`); run the full workspace gate on that commit and complete
   Waves 5–6 (composite promotion, headless plugin, lavapipe goldens, parity
   audit) through to the human-review-gated PR.
2. **Reconcile local `main` to `origin/main`** so subsequent work isn't cut from
   a stale base.
3. **Flip the stale `[draft]` spec statuses** to `[landed]` for the
   agent-interface and widget-catalog child specs, and correct the foundation
   `architecture.md` present-tense claims (one doc pass).
4. **Decide whether the untracked foundation gaps graduate to roadmap sub-specs**
   — at minimum images/media, the remaining widget set, the event model, gamepad,
   and drag-and-drop — so they stop being invisible to planning.
5. **Frame the wholesale-rebuild pattern as one tracked architectural item** and
   the verification-breadth build-out (gallery fixture enrollment + residue
   goldens) as a single follow-up, rather than scattered notes.

---

## Appendix — tracked deferrals by subsystem

These are real but already recorded in `docs/plans/follow-ups.md` or the owning
spec; listed for completeness so this report is a full record.

**Layout:** `calc()`/em/rem/viewport units; `transform-origin`; `Display::Contents`
re-parenting; table colspan/rowspan + caption/col/colgroup; multicol content
fragmentation (tier-E); subgrid + masonry; per-window top layer; sticky em/rem/V*
insets; will-change layer promotion; transformed-ancestor containing block for
`Position::Fixed`; perspective/backface/skew/general-matrix paint; scroll-snap
math + scrollbar widget + smooth scroll-to; `ScrollbarGutter::Stable`; CQ
transitive cascade one-frame-stale at depth > 1; Cq* translate units → 0.0.

**Render:** filter / mix-blend-mode shaders; rounded-rect clip; inset box-shadow;
dashed/dotted/double borders; perspective/backface/transform-origin/skew
consumption; will-change layer promotion; per-window UI routing; multi-page atlas
bind (page-0 only today); nested degraded effect-group forward-composite into
parent; forced-colors BoxShadow visual reftest (`#[ignore]` placeholder).

**Text rendering:** color-emoji rendering (skip+warn today); multi-page atlas;
wholesale `ExtractedGlyphs` rebuild; woff2 / variable axes beyond weight / font
synthesis / metric overrides; `text-decoration-style/-thickness/-offset`;
per-span rich-text editor refresh; ASCII pre-warm **rejected** (unmeasured).

**Text editing:** multi-range/multi-cursor *behavior* (type already shaped); IME
candidate-popup exclusion area (upstream Bevy limit); manual real-IME matrix
(CI-impossible).

**Verification:** Tier-4 reftest corpus breadth (2 pairings); Tier-5 residue
golden classes (shadow/emoji/atlas/compositor) unblessed; `Matrix::gallery_screen()`
+ per-screen goldens; invariant `realize()` re-implements 6f painters_z (a 6f-only
regression slips past Tier-3 — fault-injection-confirmed); invariant generator has
no `PositionKind` axis; quiescence gate conditions 2–4 untested headlessly; CPU
SDF oracle ↔ shader numeric pin; multi-reference reftest aggregation; golden-prune
bin; object-store migration.

**Agent interface:** **Phase 2 / `buiy_mcp` entirely**; `owns` re-parent +
`flow_to`/`details`/`error_message` relations (carried, unwired);
`TreeView::Merged` (accepted but no-op); exhaustive gate-#12 proptest generators;
`SetSelection`/`SetTextSelection`/`ReplaceSelectedText`; the actionability-gate
driver loop; `CustomAction` app-verb channel + `Scroll*` actions; AlertDialog /
multi-thumb slider / Accordion variants; lazy `TreeUpdate` diffing gated on
`AccessibilityRequested`; multi-window per-`WindowId` tree keying.

**BSN:** `.bsn` asset-file loader + hot-reload (blocked on upstream loader);
rc.3 → 0.19.0-stable bump (closes the rolling-latest-stable policy exception);
context-bearing `FromTemplate` path (correctly-unbuilt — no component needs it);
incremental widget scene-fn coverage as the catalog grows.

**Widget catalog:** `AUTHORING.md` app-author guide (named C8 deliverable,
absent); the 5 per-screen GPU goldens; symbol-glyph tofu (✓ U+2713 / ▸ U+25B8
render `.notdef` under the latin-subset font); 1000-row idle-CPU perf; S3 overlay
single-frame positioning fragility; catalog-wide `content_is_present` enroll
auto-check (blocked on a text-capable enroll stack).

**Foundation roadmap (future sub-specs, intentionally unbuilt):** forms &
constraint validation; devtools; 3D-anchored / diegetic UI + render-to-texture
surface; i18n beyond BiDi; full focus model (roving tabindex, spatial nav);
window-and-surface design (gates several layout/render multi-window items);
OS-integration / clipboard-and-OS reader; asset-pipeline.
