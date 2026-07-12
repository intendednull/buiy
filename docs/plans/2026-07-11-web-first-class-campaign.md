# First-class web (WASM) — campaign roadmap

**Date:** 2026-07-11
**Status:** draft
**Spec:** [`2026-06-30-buiy-browser-reach-widening-design.md`](../specs/2026-06-30-buiy-browser-reach-widening-design.md) (Track A residual) + [`2026-07-11-mvu-subscription-ingest-design.md`](../specs/2026-07-11-mvu-subscription-ingest-design.md) (Track B) + [`2026-07-11-buiy-url-router-design.md`](../specs/2026-07-11-buiy-url-router-design.md) (Track C)
**Realizes:** the true residual from [`reports/2026-07-11-web-first-class-audit.md`](../reports/2026-07-11-web-first-class-audit.md) (the reconciliation of GitHub issue #143)

## Framing

This is **not** a "build web support" campaign. The audit established that #143 was written against the
stale #85 baseline and never reconciled against origin/main: the [browser-reach widening
spec](../specs/2026-06-30-buiy-browser-reach-widening-design.md) + the Dooduel web waves W1–W5
(#94–#102) already landed Items 1/3/4/5/9. The campaign is therefore **(A) close the reach-widening
spec's residual + pay down the verification/operability debt it named but never scheduled, and (B/C)
two net-new, additive subsystems** (MVU Subscription ingest; URL router). Build **on** the 2026-06-30
spec, not from #143's stale checklist.

**Execution is BLOCKED on the Dooduel M1 merge — per the user's directive** ("blocked for actual impl
until it is merged"). This roadmap (spec + plan) is prep; nothing here has touched implementation code.

**Why, per track (the block is not uniform):**
- **Track B genuinely requires it** — `apps/dooduel_core`'s transport/`net.rs` is the reference impl B
  reconciles with; building B blind would duplicate/diverge from it.
- **Tracks A0 (verification debt) and C (router) do not depend on Dooduel code** — A0 adds
  tests/CI to already-landed web code on `origin/main`; C is net-new. They are held only by the user's
  blanket directive, not a code dependency, and are the natural first waves once unblocked.
- **The framework render fixes** (multi-page atlas bind, `Changed<Text>` render-invalidation) affect
  any web rendering and are the real shared dependency — but they are ~12 commits that could land as a
  **separate, earlier PR** ahead of the full 118-commit multiplayer merge.

**Schedule risk (surfaced, not hidden):** the critical path runs through a **local, unpushed,
no-PR, user-gated** branch that has sat awaiting a merge decision across multiple M1/QA cycles. If that
merge slips, the whole campaign slips. **Mitigation** (offered to the user): split the branch — land the
framework fixes (and optionally Track-B's transport reference) as an early PR so A0/C can start on
`origin/main` without waiting for the full multiplayer feature. Track the merge as the gating milestone.

## Track A — Hardening + verification debt (earn "first-class")

Owner spec: the 2026-06-30 reach-widening design (its one open deferral **D3** + the follow-ups its W4/W5
decisions named but did not schedule). Detailed, file:line-anchored worklist: **audit § 3.A**. Grouped
into waves (each item's effort + prototype-first flag are in the audit table). Low-priority mechanical
items (DOM `code`→`KeyCode` table completion; HTML/image clipboard OS flavors; brotli precompression)
stay **unscheduled**, tracked in audit § 3.A; fold into A0/A4 opportunistically.

- **A0 — Verification debt (fast, mostly S; highest value-per-effort).** a11y-sink CI regression guard
  (headless CDP `getFullAXTree`); `navigator.gpu` loader-selection test; post-deploy Pages smoke;
  complete the WebGL2 shader-conformance coverage (smoke only compiles the button's shaders — spawn a
  gradient bg + effect group so `gradient.wgsl` + the compositor pipelines actually translate); web IME
  automated test harness. **These convert "landed" → "honestly verified" and are the cheapest wins.**
- **A1 — Real-user reach passes (prototype-first; the audit § 4 "landed ≠ reaches a real user" traps).**
  Manual real-AT pass (NVDA/VoiceOver/TalkBack) against `gallery_web`; real-device mobile-OSK + real-IME
  (CJK/dead-key) confirmation. Do these EARLY — they tell the campaign what actually needs fixing vs. is
  already fine.
- **A2 — WebGL2 robustness.** **D3 `Rgba16Float` float-less compositor fallback** (M, prototype-first —
  the one genuine code gap; infra-blocked: no dev/CI adapter reproduces the float-less path, needs a
  forced-`Rgba8` rig); DPR `max_texture_dimension_2d` clamp (M, prototype-first); tint-CLI conformance
  lane (M); web paint golden upgrade from the variance floor (M, prototype-first — pinned SwiftShader).
- **A3 — A11y operability.** Inbound AT action bridge (M) + live focus bridge (S) — turns the read-only
  sink operable. Sequence after A1's real-AT pass reports whether the outbound tree announces correctly.
- **A4 — Interaction finishing (independent, parallelizable).** Guaranteed cross-app paste (DOM
  `paste`-event bridge, M); runtime macOS-modifier detection (M — browser macOS users get Ctrl not Cmd);
  `ime_position` caret tracking (S); `Msaa::Off` web evaluation (S).

## Track B — MVU Subscription ingest (issue #143 Item 6)

Spec: [MVU Subscription ingest](../specs/2026-07-11-mvu-subscription-ingest-design.md). The framework
lands the keyed `Subscription` seam (realizing MVU-as-core § 8; **zero new framework deps**) + a
documented app-owned transport pattern (ewebsock). Waves:
- **B0** — pin the `Subscription` public API against § 8 + the `Cmd::task` precedent (plan/review).
- **B1** — implement the keyed Subscription seam in `buiy_core` (diff-per-frame start/drop,
  `enqueue_with_origin(Origin::Subscription)`, replay re-feeds logged Msgs — never restarts).
- **B2** — reconcile `apps/dooduel`: `drain_client_net` → a thin Subscription adapter (additive; no code
  leaves `apps/`). Validates the seam against running multiplayer.
- **B3** — the scope-record annotation (`cross-cutting.md:96` / `README.md:49`: transport out, ingest
  seam in) + the reference pattern doc/example.
- **Deferred (demand-gated):** an opt-in `buiy_net` transport crate — only if the #142 widget-catalog
  multiplayer demo materializes.

## Track C — URL router (issue #143 Item 7)

Spec: [URL router](../specs/2026-07-11-buiy-url-router-design.md). Waves:
- **C0 — Scope carve-out (spec-first, GATES all C code).** Land the foundation edit: split routing from
  persistence at `cross-cutting.md:32` + `README.md:49` (persistence stays out; URL-view-navigation
  carves in web-only, native-no-op). No router code until this is approved.
- **C1** — `History` provider trait + `BrowserHistory` (wasm) / `MemoryHistory` (native), mirroring
  `ClipboardProvider`; add the web-sys `History`/`Location`/`PopStateEvent` features (no new crates).
- **C2** — the generic bridge: `Changed<R>` observer → `to_path` → push; popstate/memory-pop →
  `from_path` → `enqueue`; echo-loop guard (`replace`-on-pop + `set_if_neq`). Home: a `buiy_view`
  `router::<R>()` app-ext behind a `router` feature.
- **C3** — wire the gallery's `NavModel` as the first consumer (`/scroll`-style flat routes) + the Pages
  path-rewrite (`404.html` shell). Proves shareability end-to-end.

## Sequencing

1. **Now (unblocked):** finish prep — these three specs + this plan + the #143 rewrite/split drafts →
   **review gate** (fresh-context) → land the docs (PR).
2. **On Dooduel M1 merge (unblocks execution):** Track A0 (verification debt) + Track A1 (real-user
   passes) first — cheapest + they de-risk everything else. Then A2/A3/A4 as the passes report.
3. Track B and Track C run in parallel with Track A (independent subsystems). C0 (scope carve-out) and
   B0/B3 (scope annotation) are docs-gated decisions that can land in the prep PR wave. **Note: B3 and
   C0 both edit the same `README.md:49` non-goal line + `cross-cutting.md` — compose them into ONE
   reconciled non-goal revision (transport stays out; URL-view-navigation carves in), not two conflicting
   edits.**

## #143 disposition (per user decision: rewrite + split)

Rewrite #143's body against origin/main (mark W1–W5 done; fix the two factual errors — B2 is 16 attrs
not 17, the a11y sink reuses the `A11yNodeView` snapshot not `build_tree_update`; demote the a11y "long
pole XL" to ~M finishing). Split into children:
- **#143a — Web hardening & verification debt** → Track A (this plan).
- **#143b — MVU Subscription ingest (WebSocket-into-ECS)** → Track B.
- **#143c — URL router** → Track C.
- **Item 8 (OG/SSR)** → close out-of-scope with a pointer to app-backend ownership.

Also record in the rewrite the **stale-worktree-baseline process finding** (audit § 7.4): #143 drifted
because it was authored against `4010753` and never re-baselined — future auditors must read via
`origin/main`, and the epic must be re-baselined whenever a web wave lands.

Drafts prepared for user review **before** any GitHub edit: `$CLAUDE_JOB_DIR/tmp/issue-143-drafts.md`
(do not post until approved).

## Cross-references (shared web + mobile + catalog work)

Per the audit § 6, treat these as shared, not web-only: the D3 `Rgba16Float` float-less fallback and the
touch / clipboard / IME / a11y seams also discharge **native-mobile** (#141) obligations; the Track-B
Subscription seam + a future `buiy_net` also serve the **#142 widget-catalog** multiplayer demo; #142's
"missing Image primitive" is **already on `origin/main`** — the `RasterImage`/raster textured-node path
(#131 `bd79760`, verified present on `origin/main`, not just the Dooduel branch) — so audit #142's other
items against `origin/main` before building, since it too was written against a stale base.

## Product decisions + track-don't-build (carried from the audit; do not lose)

**Product decision (not code):** quantify the **no-WebGPU audience slice** (Firefox-Linux / older Safari
/ older Android) against the target app's user profile (audit § 3.B / § 9) and log it. The code
precondition (WebGL2 build) is already met — this is prioritization, not gating work.

**Track-don't-build (upstream / infra-blocked — NOT scheduled as build work):**
- **CI-enforced WebGPU paint stays out of reach** — the WebGPU smoke SKIPS on GPU-less hosted runners
  (Dawn exposes no adapter over lavapipe/SwiftShader); only the WebGL2 leg is CI-enforced. **A0 does not
  fully close proof-of-paint** — the WebGPU leg remains a manual gate until a self-hosted GPU runner
  exists (audit § 2 / § 5).
- `accesskit_web` — no crate exists; Buiy routed around it (the DOM/ARIA sink). Track upstream only.
- `winit#4424` (IME) / `bevy#13168` (single dual-backend wasm) — worked around in userspace; track for
  eventual shim removal.

## Verification (per foundation § 2.9 — web is a manual release gate, augmented)
- Track A: audit § 3 names the tier per item (headless test → web-smoke SwiftShader gate → manual
  real-AT/real-device pass → web golden). A0 items are CI-enforceable; A1 are manual-by-nature.
- Track B: headless in-memory-source unit tests (incl. the replay-no-restart invariant); native
  echo-server integration; wasm socket = manual smoke.
- Track C: headless native `MemoryHistory` round-trip; wasm `pushState`/`popstate` browser smoke;
  cold-URL shareability check.
- Each wave fresh-context review-gated per `staged-development`; commit per verified unit, no push/merge
  without an explicit go.
