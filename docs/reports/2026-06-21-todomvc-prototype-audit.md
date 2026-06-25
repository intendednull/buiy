# Independent Audit — TodoMVC Prototype → Production Re-implementation

**Date:** 2026-06-21
**Auditors:** 10 dimension auditors + adversarial refutation passes on the 3 claimed bugs (17-agent workflow; ~1.65M tokens)
**Audited handoff:** `docs/reports/2026-06-18-todomvc-to-production-instruction-set.md` (the prototype's own production-instruction set)
**Supporting artifacts read:** the prototype's `…-prototype-findings.md` and `…-live-input-debugging-journal.md` (both on the `worktree-todomvc-prototype` branch, not merged)
**Trees compared:** `ci-hardening` (== current `main`, bevy 0.19.0-rc.3 / accesskit 0.24) vs `todomvc-prototype` (bevy 0.18 / accesskit 0.21)

> **Provenance.** This report was synthesized from 10 independent dimension auditors plus adversarial refutation of the 3 claimed bugs. All "live on main" and code-location claims were spot-verified against the `ci-hardening` (== main) and `todomvc-prototype` worktrees on 2026-06-21. It is a research-phase deliverable that grounds the brainstorm → spec → plan for the re-implementation; it does not itself prescribe the implementation.

## Purpose

This is an **independent, clean-room audit** of the Buiy TodoMVC prototype and its handoff document, produced to ground a clean-room **re-implementation** of the same exemplar (a widget catalog / interactive app) on current `main`. The prototype was a deliberate throwaway spike whose value is the *bugs it surfaced* and the *seams it stressed*, not its code. The handoff doc claims three core bugs, a state seam + Checkbox feature, a set of widget/styling/verification gaps, and an A→F production breakdown. We verified every load-bearing claim against the actual code in both trees, ran adversarial refutation on the three bugs, and asked a completeness critic what the exemplar never touched. The lead-auditor conclusion: **the diagnoses are largely sound and the bugs are real and still live on `main`, but the handoff is systematically optimistic — it under-scopes a coordinate-space bug *class* into one instance, mislabels root causes, calls "done" things that are half-wired, over-flags one already-correct path, and is built on a now-superseded base so none of its code applies as written.**

---

## 1. Verdict on the handoff doc (lead with this)

### What it got RIGHT
- **The three bugs are real and still live on `main`** (verified directly): picking AABB-tests parent-relative `ResolvedLayout.position` (`picking/backend.rs:42`, `mod.rs:51-56`); `text_commit` lacks any `shape_stale` term (`grep -c shape_stale` over `crates/buiy_core/src` = 0); TextSync `set_text`-clobbers the editor buffer (`sync.rs` is **byte-identical** between trees — the prototype never fixed it).
- **The retained-mode ECS app model is correct and is the committed v1 design** — plain systems + change-detection, no signals layer, is explicitly a foundation non-goal and is independently validated by the `belly` prior-art. The handoff's "no reactivity layer is fine for v1" holds.
- **The picking fix's direction is correct** — reading `GlobalTransform.translation().truncate()` aligns picking with render's *exact* coordinate source (render/mod.rs:435, "pillar 5") and inherits ancestor-scroll folding for free via the bridge.
- **The `text_commit` `shape_stale` fix is sound by construction** — it compares the *exact two quantities* extract counts and asserts equal (`layout_runs().count()` vs committed `lines.len()`), so it cannot drift from extract's truth and does not false-positive on height-cropped text.
- **The A11yToggled/Checkbox/keyboard additions work** for a 2-state checkbox (tests green; click and Tab+Space both flip state and emit `Toggled`).
- **The styling §6 flat-render table is accurate** — the live path paints only background fill + uniform corner radius + text + flexbox; it is, in fact, *more truthful than the render-pipeline spec's own status note*.
- **The verification gap is real and well-evidenced** — no tier runs a real DefaultPlugins app with real winit input; the e2e tests set `Hovered`/`FocusedEntity` by hand, bypassing `emit_picks` and focus entirely.

### What it got WRONG (factual errors that would misdirect production)
- **"Caret math likely also off / not yet verified" (§3.1, §8.3) — FALSE.** `text/edit/pointer.rs:167,175` already use `gt.translation().truncate() + content_offset` (absolute). This was never broken. The hedge signals the prototype never read the file and would send production to re-verify a correct path. *(Refutation: confirmed already-absolute on main.)*
- **"Edit-in-place blocked because `ClickTracker` is private to `text::edit`" — FALSE.** `ClickTracker`, `PointerGesture`, and `classify` are all `pub` and re-exported through `text/edit/mod.rs:46` → `text/mod.rs:55-57`. Only the *system* `pointer_selection` is editor-scoped; the classifier is public. The real gap is the absence of a widget-agnostic double-click *event*.
- **"click-to-place-caret (`pointer_to_cursor`) may still use relative coords" — FALSE** (same as caret, above; already absolute).
- **Root-cause of the text-commit escape is imprecise.** The handoff says "a definite-size node is never re-measured / stable measure cache." `mark_dirty` *is* always called (`sync.rs:350`). The auditor's *replacement* cause (Taffy `compute_leaf_layout` ComputeSize early-return) was itself **refuted** by the prototype's own test (a definite-size leaf *does* re-measure under `PerformLayout`, asserting `measures>=1`). The honest statement is the handoff's parenthetical "*or whose measure cache survives*" plus the **Display::None / `compute_hidden_layout`** path. Net: *both* the handoff and one auditor overstated; the bug is real, the precise gate is "the dirty flag did not force the leaf's measure closure to run."
- **"Add `ColorToken::Literal`" (§6) — contradicts the spec.** `component-model.md:103-104` already decided literal colors store a raw `Color` *outside* `ColorToken`; a `Literal` variant would bifurcate the themeable contract and silently skip forced-colors/dark-mode swaps. Verified: `ColorToken` has exactly `Transparent/Token/CurrentColor/SystemColor`, no `Literal`.
- **"hello_button golden baked the blank box as expected" (§5.1 framing) — FALSE.** The only `hello_button` golden is an *a11y JSON snapshot*, not an image; the visual e2e is `#[ignore]`d. The real defect is the *opposite*: there is **no** visual test of the button at all. (Both auditor and refutation agree.)
- **The font-reload regression test "guards the fix" — FALSE** (and the handoff half-admits it). `text_commit_font_reload.rs` auto-heals via `mark_dirty`→re-measure and passes with or without the fix; it isolates nothing.

### What it MISSED / under-scoped (the substantive failures)
1. **The picking bug is a coordinate-space *class* bug, not a one-off — `write_clip_rects` (`render/clip.rs:286`) has the identical relative-as-absolute bug, live on `main`, unfixed, unmentioned.** Verified: `Aabb::from_box(rl.position, rl.size)` is documented "window-relative" (clip.rs:28) and consumed as an absolute wgpu scissor rect, but `rl.position` is parent-relative with no accumulation and no `GlobalTransform`. *Asymmetry the refutation correctly flags:* clip is **latent** (a `ClipRect` is only emitted when an ancestor actually clips, and every existing clip test keeps ancestors at the window origin), whereas picking fired on the first centered card. Same class, lower immediate probability — "high" severity is for the structural trap, not v1 impact.
2. **The lying `ResolvedLayout.position` doc comment ("window-relative", `components.rs:65`) is the structural enabler** of every relative-as-absolute consumer bug. It is unfixed in both trees. Picking and clip both fell into it independently — that is the definition of a reusable trap, not a one-off.
3. **TextSync's editor lowering is the editor's ONLY content-seed and de-facto programmatic-set path** (there is no `EditCommand::SetValue`). The handoff's "TextSync must not lower display Text into an editor buffer" would break seeding *and* the only programmatic-set channel unless paired with an explicit seed/set verb. It also missed that TextSync is the **sole writer** of the editor buffer's wrap/tab-width/attrs (commit only sets size/align) — so the fix must be a content-vs-style *split*, not a blanket editor skip.
4. **Focus is structurally invisible.** `Outline` never renders (not in the extract query, `extract.rs:370` "FAN" TODO) and `FocusVisible` is a global bool resource read by *no* paint system (verified). The handoff lists both facts separately and never connects them: **keyboard users cannot see focus** — a WCAG 2.4.7 failure gating every interactive widget. Critical, and entirely absent from the §5 widget breakdown.
5. **No scroll/wheel input exists anywhere** (`grep` for `MouseWheel` in core+widgets = nothing). A long todo list is unscrollable today. The biggest fully-missed app-architecture capability; the handoff never mentions scrolling.
6. **The whole prototype is on a superseded base** — bevy 0.18 / accesskit 0.21 / no `buiy_bsn` / hand-assembled Button. Current `main` is bevy 0.19.0-rc.3 / accesskit 0.24 / `#[require]`+scene-fn Button + `buiy_bsn` (PR #70, landed *after* the prototype's base). **None of the 18 library diffs apply cleanly.** The handoff's "adopt the fix / mirror Button" disposition would, taken literally, revert main's widget architecture.

---

## 2. The three bugs

### Bug 1 — Picking uses parent-relative coordinates as absolute

| | |
|---|---|
| **Confirmed?** | Yes (unanimous; refutation: "diagnosis-holds") |
| **Live on `main`?** | **Yes** — `picking/backend.rs:42`, `mod.rs:51-56` read `layout.position`; only picking test spawns a non-offset node so it can't catch it |
| **Diagnosis** | Correct & complete. `write_resolved_layout` (`systems.rs:2976`) writes Taffy's parent-relative `layout.location` verbatim (only `PostTaffyPositionOverrides` substituted, no general accumulation); render reads absolute `GlobalTransform`. They coincide only when every ancestor sits at the origin — exactly the "top-left layout made relative≈absolute" condition the journal names. No simpler cause exists (single writer, no accumulation pass). |
| **Prototype fix** | Correct & minimal; aligns picking with render's exact source; inherits scroll folding via the bridge. |

**Adversarial counterpoints (must carry into production):**
- The fix keeps an `Option<&GlobalTransform>` + `unwrap_or(layout.position)` **fallback** that silently mixes two coordinate spaces. It exists only to keep bridge-less unit tests green. Render *hard-requires* `&GlobalTransform` (`mod.rs:421`); production should match that and fix the tests, not ship a fallback that masks a "shouldn't happen" case.
- `emit_picks` reads a **one-frame-stale** `GlobalTransform` (PreUpdate consumer; bridge propagation runs in `Update .before(Picking)`). Acceptable (matches the documented `Hovered` lag) but undocumented in the fix.
- `Entity::PLACEHOLDER` camera and smallest-area depth tiebreak are untouched Phase-0 stopgaps. The doc comment was not fixed.

**How production should approach it — alternatives to weigh:**
- **(a) Keep `ResolvedLayout` layout-local; route all absolute consumers (picking, clip, overlays, devtools) through `GlobalTransform`, NON-optional.** Small blast radius, snapshots unchanged, one rule ("absolute = GlobalTransform"). *Refutation strengthens (a):* making `ResolvedLayout.position` absolute would actually **break the bridge** (`bridge.rs:138` subtracts per-node `acc` assuming relative position — absolute would double-correct).
- **(b) Make `ResolvedLayout.position` truly absolute via accumulation** (the `world_position` helper at `systems.rs:413-441` already exists, used only for sticky). The doc becomes honest and one field serves all consumers — but the blast radius is large (every layout golden/snapshot re-blessed; sticky/scroll/anchor interactions; risk of two absolute sources re-diverging).
- **(c) Adopt a real pointer-events / hit-target model** (true depth from the existing `Stacking.painters_z`/`z_index`, real camera ref, `Pickable::should_block_lower` + bubbling). Removes the co-located-label wart and the `PLACEHOLDER` camera; largest scope; still needs (a) or (b) underneath.

**Do NOT just bless the prototype fix.** Production must (1) pick (a) vs (b) as a *coordinate-space decision* that gates the clip fix, (2) drop the fallback, (3) fix the clip.rs instance and the doc comment in the same pass, and (4) decide depth/camera separately.

### Bug 2 — text_commit skips reshaping an unshaped buffer

| | |
|---|---|
| **Confirmed?** | Yes (auditor + both refutations agree the bug is real and the fix mechanics are sound) |
| **Live on `main`?** | **Yes** — `commit.rs:102` gates on `align_changed \|\| offset_stale \|\| size_stale` only; no `shape_stale`; regression test absent on main |
| **Diagnosis** | *Mechanically* correct (FontsGeneration sweep unshapes a buffer without moving its box → both-axes-definite leaf reaches extract unshaped → `debug_assert` in debug, **silent no-paint in release**). *Precise gate* is contested (see §1 WRONG and Appendix B); the genuinely-correct addition is the **Display::None / `compute_hidden_layout`** escape. |
| **Prototype fix** | Correct, minimal, sound by construction; preserves the zero-reshape steady-state contract. |

**Adversarial counterpoints (must carry into production):**
- **No isolating test.** The shipped `text_commit_font_reload.rs` auto-heals and passes pre-fix. The fix lands **unverified** at the unit tier.
- **Release silent-no-paint is the more dangerous failure** and is unguarded on main; the handoff mentions it once and never weights it.
- **The crash entity is the EDITOR** (TextInput, editor-owned buffer); the test uses a plain display Text node — the arm that actually crashed is untested.
- **Steady-state cost:** `shape_stale` adds an unconditional per-frame `layout_runs().count()` walk (O(lines)) to every text entity, eroding the documented "zero work in steady state" guard. Unbenchmarked.
- **CRITICAL refutation finding — the fix may MASK a data-loss bug.** On the editor class, the FontsGeneration sweep is not merely an unshape, it is a **content clobber** (Bug 3 below): it writes display `Text` ("") onto the editor-owned buffer. For the *observed* empty-editor crash this was a no-op, which is exactly why "unshape only" looked complete. Adopting the commit guard as *the* production fix would ship a framework that silently eats editor content on any async font load while a TextInput is non-empty — the guard reshapes the now-empty buffer and *silences the assert*. This is why the fix is "correct but incomplete," and why Bug 2 and Bug 3 must be fixed together.

**How production should approach it — alternatives to weigh:**
- **Keep the commit-side `shape_stale` guard as a last line of defense** (cheap, general, catches *any* future unshape source) — **but** also fix the cause.
- **Fix the cause in the schedule:** route a **damage/dirty flag set at TextSync's `set_text` site** (the actual mutator), cleared by commit — O(1) per frame, preserves the steady-state contract, points at root cause. (Lock-discipline tradeoff: adds a third shape-decision site.)
- **Release-only self-healing in extract** (reshape-on-mismatch or skip-and-request-next-frame) as defense-in-depth so users never see invisible text; keep the `debug_assert` as the developer tripwire. (Phase concern: extract should be read-only.)
- **Write the isolating test** that bypasses the auto-heal: directly unshape a settled buffer (or hide under Display::None), cover the **editor-owned buffer** and the **empty-editor 0-vs-1** case, assert `reshape==1` and `layout_runs()==lines`.

### Bug 3 — TextSync clobbers the focused editor's content

| | |
|---|---|
| **Confirmed?** | Yes (auditor + both refutations: "diagnosis-holds" / "fix-holds") |
| **Live on `main`?** | **Yes** — `sync.rs` byte-identical between trees; the prototype's only mitigation was operational ("settle 5 frames before typing"), which is timing-luck and does NOT generalize |
| **Diagnosis** | Correct & mechanism-complete. `sync_one`→`apply_authored_to_buffer`→`buffer.set_text(&directed)` through the editor-first accessor (`access.rs`); a TextInput's display `Text` is `""`; the FontsGeneration sweep (`sync.rs:251`) runs over every editor. *Refutation adds:* the trigger is **broader** than the ~9s system scan — `apply_font_registry` (`registry.rs:543`) re-fires the all-buffers sweep on *every* runtime `add_font`, so the clobber can fire mid-typing, not just once post-startup. |
| **Prototype fix** | **None exists** — sync.rs unchanged. This is the most important standing bug. |

**Adversarial counterpoints:**
- The display-Text→editor seam is the editor's **only content-seed and programmatic-set path** (no `EditCommand::SetValue`); a naive editor-content-skip breaks seeding.
- TextSync is the **sole writer** of editor wrap/tab-width/attrs (commit only does size/align), so the fix must be a content-vs-style split.
- **IME preedit** is in the blast radius (one refutation): a mid-composition `set_text` destroys composition, not just committed text — the fix must be preedit-aware.
- A refutation proposes a **fourth alternative** the audit collapsed away: an **edge-triggered / value-diffing** lowering (cache last-lowered content; re-run `set_text` only when display Text actually changed). It kills the clobber by construction, keeps style/attrs flowing, preserves the seed/set channel with no new `EditCommand`, and is the standard controlled-input pattern (React `value`). Its cost: display Text stays nominally co-authoritative and the cache must update on user edits. This is a *durable* design, not a stopgap — the audit's "content/style split is THE clean answer" is overconfident.

**How production should approach it — alternatives to weigh:**
- **(a)** Skip content lowering for `TextEditState` entities; still apply style; re-seed once at creation (Added path); add `EditCommand::SetValue`.
- **(b)** Full content-sync vs style-sync split, editors opt out of content-sync. (cosmic has no "set attrs without text" on a whole buffer — style-only attr changes may need a content-preserving reshape reading the *editor* value.)
- **(c) — refutation's** edge-triggered value-diff (above); smallest surgical change, preserves the existing API and substrate tests.
- *Reject* snapshot/restore around the sweep (symptom-patching).
- Add a **content-survival regression test** (zero coverage today): editor with typed content != display Text, bump FontsGeneration, assert survival.

---

## 3. Features — a11y state seam / Checkbox / keyboard

**Design soundness:** The "`A11yToggled` IS the widget's logical checked state, single source of truth" decision works *for a 2-state checkbox* but is the weakest part of the feature and the handoff oversells it:
- `A11yToggled(pub bool)` **conflates two distinct ARIA states** (`aria-checked` for checkbox, `aria-pressed` for toggle-button — separate states with different keyboard semantics; a toggle button built on it would announce as *checked*, not *pressed*) and is **structurally incapable of tri-state (Mixed)**, which the foundation *already committed to* for both (`accessibility.md:24`, `media-and-widgets.md:56`). `accesskit::Toggled::From<bool>` provably drops `Mixed`.
- **No focus-on-click** for Checkbox/Button (verified: `focused.0` only read, never set in checkbox.rs/button.rs). Keyboard activation is reachable only via Tab — clicking a checkbox then pressing Space toggles whatever was last Tab-focused. The handoff presents keyboard activation as "done."
- **Enter toggles the checkbox** (`checkbox.rs:136`) — violates WAI-ARIA APG (checkbox = Space only; Enter should submit the form). The prototype review buried this as a "low note"; for an a11y primitive it is a contract error.
- `TextInput` uses `A11yRole::Text` (verified `text_input.rs:70`) — screen readers announce the editor as static text. A silent a11y bug live on main; the §5 audit never checked it.
- The `buiy_verify::a11y` `skip_serializing_if` serialization (byte-stable snapshots, absent/false/true distinction) is **genuinely good** and should carry over (extended to tri-state).

**Alternatives for the catalog spec (decide the toggle-state primitive BEFORE porting any code):**
- **(a)** Widen `A11yToggled` to an enum (`Off/On/Mixed`) — minimal, gains Mixed; still conflates checked/pressed (solvable by branching on role in translate); still couples app logic to an a11y-named type.
- **(b)** Separate widget-state component (`Checked`/`Pressed`/`Selected`) owned by `buiy_widgets`, one-way lowered into the a11y layer — clean layering (matches `bevy_ui_widgets` `Checked`/`Pressed`/`InteractionDisabled` and sickle-ui's `PseudoState`), `bsn!`-authorable (`(Checkbox, Checked)` spawns clean, no panic), distinct `aria-checked`/`aria-pressed`; reintroduces a sync system + one-frame lag.
- **(c)** Generic role-neutral `ToggleState` enum the a11y layer *reads* without owning, with `translate.rs` choosing `aria-checked`/`aria-pressed`/`aria-selected` by `A11yRole` — best of both (single source, domain-neutral name, tri-state-ready, generalizes to listbox option / switch), most up-front design.

**Constraints all options must meet:** tri-state-capable; distinguish aria-checked from aria-pressed; bsn!-authorable; add focus-on-click; Space-only for checkbox; add `A11yRole::TextField`. The whole feature is **absent from main** — production builds it fresh, re-derived against accesskit 0.24's `tree_id` (the prototype's `translate.rs` is on the pre-`tree_id` 0.21 API and would not compile).

---

## 4. Gaps

| Gap (§5/§6) | Verified state | Alternatives to weigh |
|---|---|---|
| **Button renders no label; smallest-area hit-test forces co-locating Text** | Confirmed Button has only `A11yLabel`, no `Text`, fixed 120×32. "Forces co-location" is *partial*: `hit_test` ignores `Pickable` entirely; `bevy_picking` already offers `Pickable::IGNORE`/`should_block_lower:false`. Co-location is a hit_test omission, not a law. | **(A)** co-located Text (prototype) — no core change, but one text run only, fights the `#[require]` BoxModel default; **(B)** child Text + fix `hit_test` to honor pick-through (canonical Bevy pattern, enables rich/icon labels, fixes the real root cause for all composite widgets); **(C)** `hit_test` resolves to nearest interactive ancestor (cheap, "click anywhere activates"). |
| **State-init duplicate-component panic** | Confirmed `Checkbox::new` bundles `A11yToggled(false)`; `(Checkbox, A11yToggled(true))` panics on bevy 0.19; example works around with insert-after-spawn; initial paint load-bearing on `Changed`-gated `sync_checkbox_visual`. | **(A)** constructor param `new(label, checked)` (doesn't scale, not bsn!-authorable); **(B)** separate state component, `(Checkbox, Checked)`, absence==default (no panic, bsn!-native, scales — the `bevy_ui_widgets` standard); **(C)** builder `.checked(true)` (fluent, still not bsn!-native). |
| **Prelude omits app surface** | Confirmed: `buiy::lib.rs:42` re-exports only `Button, OnPress, TextInput, WidgetsPlugin` + scene-fns; no `Checkbox`/`Toggled`/`EditCommand`/`EditSubmitted`/`TextEditState`/`TextDecorations`. | Expand the umbrella prelude to the full app-facing widget + editor surface so `use buiy::prelude::*` suffices. |
| **Styling / render reality (flat path)** | Confirmed: live path = background fill + uniform radius (from `top_left.x`, px-only) + text + flexbox. Border-sides/BoxShadow/Outline/gradients/group-opacity never read by extract. *Missed:* the render-pipeline spec README status note claims F-tier passes are "landed and verified," contradicting the code — the prototype's table is more truthful than the spec. `shadow.wgsl` + Shadow primitive kind exist but are **unfed**. | **Feed the existing F-tier surface** (extend extract, emit Shadow/border-band/outline into reserved buckets, shader work) — matches spec, unblocks the catalog look; vs **down-scope to an honest "flat F-subset"** and ship v1 within it (cards via fills, separators via thin filled Nodes) deferring shadows/borders/outlines. *Reject* `ColorToken::Literal` (spec-contradicting). Note: gradient is C-tier (later); Border/BoxShadow/Outline are F-tier (land first). |
| **Focus ring invisible** | Confirmed: `Outline` not in extract query; `FocusVisible` read by no paint system. Keyboard focus is undetectable. Critical, mis-filed under "styling." | Wire `Focusable`+`FocusVisible` → an `Outline`/ring paint in `extract_buiy_draws`, forced-colors-safe ring token, Tier-4 reftest. Hard dependency of the catalog. |
| **Verification blind spot** | Confirmed: every tier MinimalPlugins + synthetic input or GPU-capture with synchronous Ahem font; e2e `click()` sets `Hovered` by hand; no real-input tier; Tier-1 layout snapshot reads only `ResolvedLayout` (the field the bug lives in) so it is **structurally blind** to render/picking divergence. | **Headless synthetic-PointerHits tier** (inject `PointerLocation`, run `emit_picks`→`update_hovered` under the bridge — CI-runnable, no GPU/winit, catches the high-frequency picking class); vs **GPU-host DefaultPlugins+Winit+Asset smoke** (only path exercising the async font scan + winit coords + real focus — highest fidelity, `#[ignore]`-only so it can't gate PRs); vs **headless DefaultPlugins+AssetPlugin driving bevy input events** (no XTEST flakiness — the refutation's preferred cheaper middle path). *Reject* XTEST/xdotool as an automated tier (the journal's flakiness swamp). |

---

## 5. App architecture + deferred behaviors

- **Retained-mode ECS (plain systems + change-detection, one-entity-per-row, `RowOf` back-refs, chained `TodoLogic.after(BuiySet::Input)`)** is correct, idiomatic, and the committed v1 design. Keep it. (Minor: the manual child-entity caching in `TodoRow` denormalizes what `Children` already encodes.)
- **One-entity-per-row has an unnamed scaling ceiling** — no virtualization; N rows = N entities + N buffers + N layout nodes; `content-visibility:auto` is deferred/warn-once. Fine at TodoMVC scale; the handoff presents it as "the intended model" without naming the ceiling. A data-heavy app needs a list/virtualization story.
- **`apply_filter` Display-ownership is under-scoped.** It rewrites `Display` directly, but `Display` and `FlexParams.direction` are **decoupled components** (`style.rs:73-77`) — so it desyncs *direction*, not just non-flex rows. Fix with a `Hidden` marker (or productionized `content-visibility`) that leaves author `Display`+`FlexParams` intact.
- **Gesture seam is one architectural decision, not three gaps.** Buiy discards `bevy_picking`'s `Pointer<Click/DoubleClick/Drag/Press/Release>` bubbling taxonomy and reduces picking to a single `Hovered` resource everything polls via `just_pressed`. That one decision is the root of the edit-in-place gap, the §5.1 hit-steal, *and* the mouse-down-vs-up problem. Decide once: surface `bevy_picking`'s events (reuse `ClickTracker` only for the double-click timing bevy lacks) vs keep the thin Hovered layer.
- **Deferred/missing behaviors the exemplar never exercised:** scrolling/long-list (no wheel handler at all), overlays/popovers/menus/tooltips, modal/dialog/focus-trap (Tab cycles the entire global focusable set — no scope), drag-and-drop/reorder, resize/reflow, DPI/scale change, dark-mode/forced-colors live, multi-window, IME-while-other-widgets-exist. The "N items left" count is plain Text, not an a11y live region (no politeness primitive). Tab order tiebreaks on **entity index**, not document/layout order — incidentally correct here, wrong under despawn/respawn. The unexplained ~50% idle CPU in the journal is a dangling thread.

---

## 6. What the prototype/handoff MISSED — prioritized (completeness critic)

1. **(Critical) Focus ring structurally invisible** — Outline never paints, FocusVisible read by nothing. A UI framework whose keyboard focus is invisible fails the most basic a11y bar.
2. **(High) Coordinate-space bug class** — `clip.rs` is a second live instance; the lying `ResolvedLayout.position` doc comment is the reusable enabler.
3. **(High) Editor content-clobber on async font load** (Bug 3) is unfixed *and* masked by the Bug-2 commit guard on the editor class.
4. **(High) No scroll/wheel input** — long lists unusable.
5. **(High) Tab order by entity index** — APG traversal not guaranteed; churns entities (add/destroy/clear) yet never tests Tab order.
6. **(High) Picking depth = smallest-area** ignores stacking/top-layer — mis-picks under any overlay (all of which the catalog needs; TopLayer infra exists).
7. **(High) a11y-tree correctness under bulk ops** (toggle-all/clear/filter) never asserted — does a Display::None'd row leave the AT? Untested.
8. **(Medium) `TextInput` announces as static text** (`A11yRole::Text`) — silent live a11y bug.
9. **(Medium) Whole categories absent from the A→F breakdown:** design-system/token taxonomy, app-author guide doc (system ordering / editor-settle / box-sizing / despawn / message-timing rules are scattered notes), performance/reflow baseline, error-handling, overlays/drag surface, IME-under-focus-routing.
10. **(Medium) The label co-location "recipe" is a smell elevated to a recommendation** — bakes the picking limitation into every composite widget.

---

## 7. Prototype code/test quality — what NOT to carry over

- **The entire library diff is on a stale base** (bevy 0.18 / accesskit 0.21 / no `buiy_bsn` / hand-assembled Button). **Re-derive every fix against `main`.** The picking and commit *logic* is base-agnostic and portable; the a11y/translate change must be re-applied over the `tree_id` API; Checkbox/Button must be re-authored as `#[require]`+scene-fn widgets. Do **not** literally adopt the diffs — that reverts main's architecture.
- **Do NOT carry:** the `unwrap_or(layout.position)` picking fallback (mixes coordinate spaces; render hard-requires the transform); the auto-healing `text_commit_font_reload.rs` (guards nothing); `Checkbox::new` bundling `A11yToggled` (forces the insert-after-spawn panic dance); the co-located-Text label hack; Enter-toggles-checkbox; `Entity::PLACEHOLDER` camera; smallest-area as the depth model; `apply_filter`'s direct Display rewrite.
- **`text_shape::text_buffers_shaped` is a `pub mod` no CI gate runs by default** — it's only called from the throwaway example's tests. When the example is discarded it guards nothing. Wire it into a retained headless integration test in the workspace gate or it rots.
- **`shape_stale` adds unmeasured per-frame O(lines) steady-state cost** — benchmark against typing-churn/latency before adopting, or fold detection into a TextSync damage flag.
- **Keep (genuinely good):** the picking nested+offset regression test (asserts both directions); the widget tests (real negative cases — `keyboard_does_not_activate_unfocused_button`, `fill_tracks_checked_state`); the a11y three-way `absent/false/true` serialization test; the `buiy_verify::a11y` `skip_serializing_if` byte-stability approach — all extended to tri-state.

---

## 8. Consolidated production work-list (severity-ranked, dependency-ordered)

This seeds the spec/plan phase. Items marked **[ALT]** require weighing a real alternative *before* implementing.

**Tier 0 — decisions that gate everything (do first):**
1. **[ALT] Coordinate-space decision** (alt a: route absolute consumers through `GlobalTransform` non-optional / alt b: make `ResolvedLayout.position` absolute via accumulation). Gates the clip fix, the audit-guard, and Bug 1. *Refutation evidence favors (a): (b) breaks the bridge's per-node `acc` subtraction.* — **critical**
2. **[ALT] Toggle-state primitive decision** (a: widen `A11yToggled` enum / b: separate state component / c: role-neutral `ToggleState` read by a11y). Must be tri-state + distinguish aria-checked/aria-pressed. Gates Checkbox/Switch/toggle-Button/Listbox. — **high**
3. **[ALT] Picking event-model decision** (surface `bevy_picking`'s `Pointer<E>` taxonomy / keep thin `Hovered` layer). Governs gesture seam, composite-widget hit-bubbling, down-vs-up, drag-reorder. — **high**

**Tier 1 — bug fixes (after Tier 0 decisions):**
4. Re-derive the **picking fix** on main, `&GlobalTransform` non-optional (drop the fallback), port the nested-offset test. — **critical**
5. Fix **`write_clip_rects`** the same way as picking + add a nested+**offset** overflow-clip test; fix the `ResolvedLayout.position` **doc comment**. — **high** (depends on #1)
6. **[ALT] Fix TextSync editor-clobber** (alt a/b/c: content-skip+seed+`SetValue` / content-style split / edge-triggered value-diff), preedit-aware; add content-survival test. — **high**
7. **[ALT] Adopt the `shape_stale` guard** (commit-side last line / move to TextSync damage flag) + write an **isolating** test (editor buffer + empty-editor 0-vs-1) + benchmark steady-state cost + consider release-only self-heal. Must land *with* #6 or it masks data loss. — **high**

**Tier 2 — catalog blockers:**
8. **Make focus visible** — wire `Focusable`+`FocusVisible` → Outline/ring paint, forced-colors-safe token, Tier-4 reftest. — **critical** (depends on #11)
9. **[ALT] Replace smallest-area picking depth** with stacking/`painters_z` order + real camera ref (drop `PLACEHOLDER`); hit-test-through-overlay test. — **high** (depends on #4, input-events camera wiring)
10. **[ALT] Widget label rendering** (co-located vs child+pick-through) as a `#[require]`/scene-fn, content-sized, WCAG-min target, mirrored in constructor + scene-fn. — **high** (depends on #3)
11. **[ALT] Feed F-tier styling** (BoxShadow/Border-sides/Outline) into extract+shaders, OR officially down-scope to "flat F-subset" + reconcile the render-spec status note. — **high**
12. **Add `A11yRole::TextField`** + TextInput value/placeholder a11y seam; stop announcing editors as static text. — **high**
13. Re-author **Checkbox** as `#[require]`+scene-fn with separate state component + initial-checked param; **APG keyboard** (Space-only checkbox, Space+Enter button); **focus-on-click** for Button/Checkbox. — **high** (depends on #2, #4)

**Tier 3 — app architecture / scope:**
14. **[ALT] Scroll/wheel → ScrollOffset pipeline + scrollable container**; decide virtualization posture (recycle vs content-visibility). — **high**
15. **[ALT] `apply_filter` → `Hidden` marker** (vs productionized content-visibility) leaving Display+FlexParams intact. — **medium** (depends on #1 decision style)
16. **Specify tab-order** (document/layout order, survives despawn/respawn) + **focus-trap scope** for modals + roving-tabindex for lists; a11y **live-region** primitive for the count. — **medium**
17. Add **double-click/gesture event** for edit-in-place atop the chosen click event (reuse public `ClickTracker` for timing — NOT "make it public"). — **medium** (depends on #3)

**Tier 4 — verification + docs (cross-cutting, mostly no deps):**
18. **[ALT] Add the missing real-input tier** (headless synthetic-PointerHits / headless DefaultPlugins+Asset bevy-events / GPU-host Winit smoke); assert a11y-tree consistency after bulk ops; document the Tier-1 ResolvedLayout-only blind spot. — **high**
19. **Content-presence invariant + bless-guard** (a label fixture must emit >0 glyph instances; can't bless a semantically-empty golden); wire `text_buffers_shaped` into a retained gate test. — **high**
20. **[ALT] Choose the catalog exemplar** (widget gallery + long scrollable list + overlay/menu + modal/focus-trap + form, built as `buiy_verify` fixtures) vs another TodoMVC. — **medium**
21. **Design-system/token spec** (semantic tiers, literal-vs-token, dark/forced-colors variants) + **app-author guide doc** (system ordering, editor-settle, box-sizing, despawn, message timing). — **medium**
22. **Performance/reflow baseline** (scale-game to ~1000 rows) + investigate the ~50% idle CPU. — **low** (depends on #14)

---

## 9. Open questions for the brainstorm/spec gate

1. **Coordinate space:** keep `ResolvedLayout` layout-local + route absolute consumers through `GlobalTransform` (alt a), or make `.position` absolute via accumulation (alt b)? Quantify the snapshot/golden re-bless blast radius. (Refutation evidence: (b) breaks the bridge.)
2. **Toggle primitive:** one role-disambiguated tri-state component, or separate `Checked`/`Pressed`? How does it extend to `Switch` (no Mixed), `aria-selected`, `aria-expanded`?
3. **Picking events:** adopt `bevy_picking`'s `Pointer<E>`/observer model wholesale (and its bubbling + drag lifecycle), or keep the thin `Hovered` layer? Derive depth from the existing `Stacking.painters_z` so pick-order == paint-order?
4. **TextSync fix shape:** content/style split (a/b) vs edge-triggered value-diff (c)? Where do the editor's *seed* and *programmatic-set* come from once content lowering is suppressed (Added-path seed + `EditCommand::SetValue`)? How is IME preedit preserved? How do style-only attr changes reach a live editor given cosmic has no "set attrs without text"?
5. **text_commit fix locus:** commit-side `shape_stale` (re-detect) vs TextSync damage flag (root cause)? Steady-state cost budget for the per-entity walk? Release-only self-heal in extract — yes/no?
6. **Styling scope:** feed the F-tier surface now, or ship a documented "flat F-subset" v1 and defer shadows/borders/outlines? Does the per-instance vertex layout have room for shadow blur/spread + border widths without a stride bump (R1/R2 byte-stability)? Is uniform-radius-from-`top_left.x` an incomplete migration?
7. **Verification tier:** is headless synthetic-PointerHits sufficient for picking CI coverage with full-winit reserved as a GPU-lane smoke, or does the winit coordinate/scale-factor path need per-PR coverage? Can the FontsGeneration bump be injected headlessly to isolate Bugs 2/3 without winit?
8. **App scope:** is one-entity-per-row the production stance, or is a list/virtualization abstraction needed before any data-heavy app? Is there any focus-trap at all today (compute_next_focus iterates the global set with no scope)? Does Display::None actually remove a row from the accesskit tree under DefaultPlugins?
9. **Exemplar:** TodoMVC again, or a widget-gallery exemplar that exercises scroll/overlays/modal/forms — built as verification fixtures rather than a bespoke app?

---

## Appendix A — Top risks

1. **Adopting the prototype fixes/disposition literally reverts main's architecture:** the entire library diff is on a superseded base (bevy 0.18 / accesskit 0.21 / no buiy_bsn / hand-assembled Button) while main is bevy 0.19.0-rc.3 / accesskit 0.24 / `#[require]`+scene-fn Button + buiy_bsn. Every fix must be re-derived, not cherry-picked.
2. **The picking bug is a coordinate-space CLASS bug, not one instance:** `clip.rs` has the identical relative-as-absolute bug (live on main, latent, untested) and the lying `ResolvedLayout.position` doc comment is the reusable enabler. Fixing only picking leaves the trap armed for the next consumer (overlay/tooltip/drag/devtools).
3. **The text_commit `shape_stale` guard MASKS a data-loss bug:** on the editor class the FontsGeneration sweep clobbers typed content (`set_text("")` onto the editor-owned buffer), and the commit guard reshapes the now-empty buffer, silencing the assert while the user's text is gone. Bugs 2 and 3 must be fixed together or the framework silently eats editor content on any async font load.
4. **Keyboard focus is structurally invisible** (Outline never paints, FocusVisible read by no paint system) — a WCAG 2.4.7 failure gating every interactive widget, mis-filed by the handoff as a styling nicety. The exemplar "validated" keyboard activation while the user cannot see what is focused.
5. **The handoff certifies seams as "done" that are half-wired** (keyboard activation has no focus-on-click; the "kept" regression test auto-heals and guards nothing) and over-flags an already-correct path (caret math is already absolute) — its optimism means a planner trusting it under-scopes the work and chases phantom tasks.
6. **The verification strategy has no real-input tier** and the Tier-1 layout snapshot reads only `ResolvedLayout` (the field the bug lives in), so it is structurally blind to render/picking divergence; every existing interactive test sets `Hovered`/`FocusedEntity` by hand, so picking, focus, and a11y-tree-under-bulk-ops are unverified by CI.

## Appendix B — Preserved disagreements (productive tensions to resolve in spec)

1. **text-commit precise root cause:** the handoff says "definite box never re-measured / stable cache"; one auditor "corrected" this to Taffy `compute_leaf_layout`'s ComputeSize early-return; a refutation REFUTED that correction using the prototype's own test (a definite-size leaf DOES re-measure under PerformLayout, asserting measures>=1) — concluding the handoff's parenthetical "or whose measure cache survives" was actually more accurate. The genuinely-correct addition both agree on is the Display::None/`compute_hidden_layout` escape (though a second refutation argues even that is non-firing for the assert because a hidden node windows to 0 lines on both sides).
2. **Severity of the clip.rs missed bug:** the auditor rates it "high" / "IDENTICAL" to picking; both refutations push back that it is materially LOWER immediate impact — a ClipRect is only emitted when an ancestor actually clips, every existing clip test keeps ancestors at the window origin, so it is purely LATENT (never fired for a real top-level layout) whereas picking fired on the first centered card. Same bug CLASS, lower v1 probability; the structural-trap argument stands, the co-equal "high" urgency does not.
3. **TextSync fix completeness:** the text-sync-clobber auditor presents content/style split (a/b) as "THE clean answer"; a refutation calls this overconfident and surfaces a distinct FOURTH alternative the audit collapsed into the rejected snapshot/restore option — an edge-triggered value-diff (cache last-lowered content, re-set_text only on change) that kills the clobber without a new EditCommand or test reframe, and adds that the audit entirely omits IME preedit from the clobber blast radius.
4. **Whether the text-commit dimension's verdict is "fix-holds (incomplete note)" or "fix-flawed":** one production-soundness refutation lands on FLAWED because the audit recommends adopting the commit guard as THE production fix while never flagging that on the editor class the upstream is a data-loss clobber the guard masks — so a reader following the audit ships a content-eating framework believing the dimension is closed; the root-cause refutation lands on fix-holds with only a precision overreach.
5. **Whether the picking fix's GlobalTransform fallback is acceptable:** the picking auditor treats `unwrap_or(layout.position)` as a latent foot-gun to remove; it is a real disagreement with the prototype author's choice to keep it for bridge-less unit tests (render hard-requires the transform, so the two paths diverge).
6. **Substrate-test interpretation (minor):** the text-sync auditor twice states the substrate test "asserts the editor-owned buffer HOLDS that text"; the refutation corrects that the test actually asserts layout-size parity + shaped/sized, not the content string — so the "seam was DESIGNED to seed content" claim rests on inference, not a literal assertion (the substantive under-scoping charge still stands).
7. **Whether `ColorToken::Literal` should be added:** the handoff recommends it; the styling-render auditor REFUTES it against the spec (component-model.md:103-104 routes literal colors outside ColorToken; a Literal variant breaks forced-colors/dark-mode swaps). Direct contradiction of a handoff recommendation.
