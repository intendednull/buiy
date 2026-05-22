**Date:** 2026-05-22
**Status:** active
**Subject:** Xilem + Masonry — Validates / Avoid / Borrow decisions for Buiy

# Lessons for Buiy

The consult-this-when-designing file. The other Xilem+Masonry files are evidence; this file is decisions.

Xilem + Masonry sit in a unique spot in Buiy's prior-art corpus: they are **the closest existing-art reference for "next-generation Rust UI substrate"** (foundation [`architecture.md § 2.2`](../../specs/2026-05-07-buiy-foundation/architecture.md)). Buiy is not building on Xilem/Masonry — Buiy is building *parallel to* Xilem/Masonry on a different substrate composition (Bevy ECS + Bevy render-graph + cosmic-text instead of Vello + Parley + Masonry's retained tree). The Linebender stack is what a successful unbundled-substrate Rust UI looks like in practice, and reading it carefully calibrates what Buiy can credibly promise.

## Top of file: one finding reframes the rest

### The substrate-vs-framework adoption split is the load-bearing observation.

Per [`ecosystem-comparisons.md`](ecosystem-comparisons.md): Linebender's **substrate crates** (Vello, Parley, Kurbo, Color) are widely adopted by non-Linebender consumers (Bevy, woodpecker_ui, Lapce, others). Linebender's **framework crates** (Xilem, Masonry) are essentially Linebender-internal.

This validates the unbundled-substrate posture *as a strategy* (the substrate is more durable than the framework), and it tells Buiy two things:

1. **Studying the substrate is high-value.** Vello, Parley, Color, Kurbo are battle-tested via multi-consumer adoption.
2. **Studying the framework is reference-only.** Xilem + Masonry are pre-adoption; their framework choices may or may not pan out.

Most of the Borrow rows below name *substrate-level* lessons; most of the Avoid rows name *framework-level* gotchas.

---

## Validates

Buiy decisions confirmed by Xilem + Masonry's experience:

- **Unbundled substrate is a viable Rust UI strategy.** Linebender's Vello/Parley/Kurbo/Peniko/Color decomposition is *adopted by their competitors* (Bevy via `bevy_text` 0.19, woodpecker_ui via `bevy_vello`, Lapce experiments). The unbundled-substrate strategy is empirically validated; Buiy's foundation goal #4 (parallel-to-bevy_ui with directly-integrated primitives) is the same strategy at a different scope. See [`linebender-stack.md`](linebender-stack.md).

- **AccessKit as the a11y substrate.** Masonry's `Widget::accessibility(&mut accesskit::Node)` shape is the producer-side correct pattern (per [`../accesskit/lessons.md`](../accesskit/lessons.md) Top of file). Buiy's decomposed `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations` driving `TreeUpdate`s is the ECS-flavored equivalent. Functionally identical; mechanism differs. See [`accessibility.md`](accessibility.md).

- **Apache-2.0 license posture is defensible** (Xilem/Masonry are single-Apache-2.0). Buiy's MIT-OR-Apache-2.0 dual is closer to the Bevy ecosystem norm but Linebender's choice demonstrates Apache-2.0 alone works for a serious UI library. See [`distribution-governance.md`](distribution-governance.md).

- **The toolkit-vs-reactive split is clean.** Masonry below + Xilem above is a clean architectural decomposition where each layer can be reasoned about independently. Buiy's `buiy_core` (component model + render + layout + focus + theme + a11y) + `buiy_widgets` (APG patterns) + future signal sub-spec split mirrors the same architectural intent: one layer that's framework-paradigm-agnostic, one layer that adds the reactive paradigm. The architectural decomposition is the same shape. See [`masonry-toolkit.md`](masonry-toolkit.md), [`xilem-architecture.md`](xilem-architecture.md).

- **AccessKit version cadence decouples from the UI framework's release cadence.** Linebender absorbed three AccessKit minor bumps (0.21 → 0.24) between Xilem 0.4.0 (released) and workspace HEAD. Buiy's foundation [`architecture.md § 2.9`](../../specs/2026-05-07-buiy-foundation/architecture.md) open question — "AccessKit major release between Bevy minors triggers a Buiy patch release" — is **already settled in Linebender's direction** by their lived experience. AccessKit bumps absorbed as patch releases, not bundled with framework majors. See [`accessibility.md`](accessibility.md).

- **Linebender `async-io` choice on `accesskit_winit` (Linux).** Lighter dep closure than `tokio`. Buiy's [`../accesskit/lessons.md`](../accesskit/lessons.md) Avoid row "Forgetting the Unix async-runtime feature flag" identified this; Linebender's choice is independent confirmation of `async-io`. See [`accessibility.md`](accessibility.md).

- **harfrust is the right shaper.** Both Linebender's Parley and Buiy's cosmic-text shape text via harfrust. Shaper-substrate convergence across the Rust UI ecosystem (per [`../woodpecker-ui/lessons.md`](../woodpecker-ui/lessons.md) Top finding #2) is real; Buiy benefits from this convergence regardless of the cosmic-text vs Parley API divergence above. See [`text-and-rendering.md`](text-and-rendering.md).

- **`tree_arena`-style stable WidgetId → NodeId as the AccessKit identity scheme.** Masonry uses `WidgetId` (NonZeroU64); Buiy uses `Entity::to_bits()`. Same principle, same diff-model match with AccessKit. See [`accessibility.md`](accessibility.md), [`../accesskit/lessons.md`](../accesskit/lessons.md) Borrow #3.

- **Three-named-lead bus factor is meaningfully more durable than solo-author.** Linebender's Raph + Daniel McNab + Olivier Faure is ~3× the half-life expectation of solo-author Bevy UI crates (per [`../woodpecker-ui/lessons.md`](../woodpecker-ui/lessons.md) Top finding #1). For Buiy's own bus-factor planning: aim for ≥3 named active leads before claiming v1. See [`distribution-governance.md`](distribution-governance.md).

## Avoid

Pitfalls drawn from Xilem + Masonry, with Buiy mitigation:

| Pitfall | Source | Buiy mitigation |
|---|---|---|
| **Constraint-passing layout instead of Taffy** — Masonry's BoxConstraints-passing is Flutter/Druid lineage and increasingly the outlier in Rust. Subgrid, container queries, anchor positioning, view-transitions all land in Taffy first; Masonry would need to reimplement each. | [`masonry-toolkit.md`](masonry-toolkit.md) § "Constraint-passing layout"; [`critiques-and-open-problems.md`](critiques-and-open-problems.md) #5. | Buiy commits to Taffy integration ([foundation `architecture.md § 2.2`](../../specs/2026-05-07-buiy-foundation/architecture.md)). New CSS layout features land via Taffy. Foundation reinforced. |
| **Pre-1.0 substrate for production game UI** — both Xilem and Masonry are openly experimental, 10-month minor cadence, no flagship adoption. Counting on 1.0 within 12 months is unrealistic; 24-36 months is more realistic. | [`critiques-and-open-problems.md`](critiques-and-open-problems.md) #1, #3; [`history.md`](history.md). | Buiy treats Xilem/Masonry as **reference, not dependency**. Foundation explicitly lists Vello as a feasibility witness, not a dep. |
| **"Yet another paradigm" cognitive cost** — Xilem's view-trees-as-pure-functions is its own learning curve, separate from Bevy ECS, separate from React, separate from Elm. Shipping a foundation UI with a paradigm orthogonal to its host engine is a steep ramp. | [`xilem-architecture.md`](xilem-architecture.md); [`critiques-and-open-problems.md`](critiques-and-open-problems.md) #4. | Buiy commits to **ECS-native authoring** (foundation goal #3 "BSN-native") rather than introducing a new paradigm. Buiy's foundation [`architecture.md § 2.7`](../../specs/2026-05-07-buiy-foundation/architecture.md) explicitly **no signals in v1**; observers + change detection only. |
| **Counting on Linebender to deliver on full ambition** — Raph's projects historically take time (Druid 2018, Xilem paper 2022, Xilem 0.4.0 2025-10-29: a 7-year arc). Linebender's bandwidth is split across Vello + Parley + Skrifa + Fontique + Kurbo + Peniko + Color + Masonry + Xilem + governance work. | [`history.md`](history.md); [`distribution-governance.md`](distribution-governance.md). | Buiy's foundation doesn't plan against Xilem/Masonry's roadmap. AccessKit + cosmic-text + Taffy + Bevy are Buiy's substrate; if Vello / Parley pull ahead of those, Buiy *may* reconsider but isn't planning on it. |
| **Single Apache-2.0 license incompatible with broader Rust ecosystem norms** — limits cross-pollination with MIT-only downstreams. | [`distribution-governance.md`](distribution-governance.md). | Buiy is MIT-OR-Apache-2.0 dual per Bevy convention. No lift-from-Xilem code; only architectural patterns. |
| **No formal CHANGELOG before 0.4.0** — Linebender's release notes say "we plan to start keeping a changelog after this release," meaning 0.1 → 0.3 changes are reconstructable only from release-page text + blog posts. | [`distribution-governance.md`](distribution-governance.md); [`critiques-and-open-problems.md`](critiques-and-open-problems.md) #8. | Buiy keeps formal per-release CHANGELOGs from day one. The verification-harness commitment makes per-release change-tracking mechanically necessary anyway. |
| **Theme/token system deferred** — Xilem 0.4.0 ships styling-properties-on-widgets but no semantic-token theme layer, no OS-preference binding, no light/dark variant switching. | [`critiques-and-open-problems.md`](critiques-and-open-problems.md) O3. | Buiy's `buiy-theme-tokens-design` is first-class foundation work (foundation [`architecture.md § 2.5`](../../specs/2026-05-07-buiy-foundation/architecture.md)). Hot-reloadable, OS-pref-bound, contrast-linted. |
| **APG widget coverage thin** — Xilem ships ~15 widgets; APG catalog is ~60 patterns. Closing that gap is a multi-engineer-year commitment Linebender can't deliver fast. | [`critiques-and-open-problems.md`](critiques-and-open-problems.md) O4; [`accessibility.md`](accessibility.md). | Buiy's `buiy-widget-catalog-design` enumerates every APG pattern with tier F/C/E/O. Verification harness gate #7 verifies APG keyboard contracts per widget. Foundation [`media-and-widgets.md § 3.10`](../../specs/2026-05-07-buiy-foundation/media-and-widgets.md). |
| **Animation system absent** — Xilem 0.4.0's animation story is a blinking text cursor. No spring physics, no keyframes, no layout transitions, no reduced-motion gating. | [`critiques-and-open-problems.md`](critiques-and-open-problems.md) O10. | Buiy's `buiy-animation-design` is its own sub-spec. First-class transitions + keyframes + springs + layout transitions, all reduced-motion-gated (foundation [`interaction.md`](../../specs/2026-05-07-buiy-foundation/interaction.md)). |
| **i18n / BiDi-caret / vertical writing not addressed** — Xilem inherits Parley's BiDi but doesn't ship locale-aware formatters, vertical writing modes, BiDi-caret-aware text editing. | [`critiques-and-open-problems.md`](critiques-and-open-problems.md); [`text-and-rendering.md`](text-and-rendering.md). | Buiy's `buiy-i18n-design` covers locale-aware formatters; `buiy-text-editing-design` covers BiDi caret + multi-line + IME. |
| **Vello compute-shader portability risk** — Vello has worked through several "doesn't run on this GPU" rounds. Compute-shader-based path is leading-edge but not universal. | [`critiques-and-open-problems.md`](critiques-and-open-problems.md) O6. | Buiy's render pipeline is wgpu-based but doesn't commit to compute-shader-based path-fill. Trade some Vello capabilities for portability. `buiy-render-pipeline-design` makes the tradeoff explicit. |
| **xilem_web as a separate framework from xilem** — `xilem_web` uses the DOM, not Masonry/Vello. Architecturally a different framework with shared paradigm. Cross-platform-from-one-codebase isn't a Linebender deliverable today. | [`critiques-and-open-problems.md`](critiques-and-open-problems.md) O2; [`distribution-governance.md`](distribution-governance.md). | Buiy commits to Bevy's WASM target (visual+input+layout; a11y pending AccessKit web adapter). One codebase, one widget set, web a11y deferred. |

## Borrow

Concrete primitives worth studying and adapting:

1. **The Xilem reactive paradigm shape (if Buiy ever adds signals as a sub-spec layer).** Should foundation [`architecture.md § 2.7`](../../specs/2026-05-07-buiy-foundation/architecture.md)'s "no signals in v1" eventually flip via a follow-up sub-spec, Xilem's view-as-function-of-state + id-path-message-routing + `Adapt` lensing is the cleanest published reference. Map to Bevy ECS: World-as-State, Query-as-Lens, Entity-as-id-path-leaf. See [`xilem-architecture.md`](xilem-architecture.md).

2. **The Vello + Parley capability set as the Buiy render-pipeline target.** Vello demonstrates that compute-shader-based anti-aliased path-fill, gradients in arbitrary color spaces, blur, blend, clip-path arbitrary shape all work on wgpu. Buiy's own render passes don't depend on Vello but model their capability set. Linebender Color's color-space-aware interpolation is the closest reference for CSS-spec-compliant gradient sampling. See [`text-and-rendering.md`](text-and-rendering.md), [`linebender-stack.md`](linebender-stack.md).

3. **The Masonry decomposition shape (`_core` + `_winit` + `_testing` + `tree_arena`).** Toolkit-vs-platform-vs-test-harness split is exactly the shape of Buiy's `buiy_core` (no platform) + future windowing adapter + `buiy_verify` (test harness). Reading Masonry's workspace `Cargo.toml` is the closest published reference for a sane multi-crate Rust UI workspace. See [`masonry-toolkit.md`](masonry-toolkit.md) § "Crate split inside the workspace."

4. **The `Widget::accessibility(&mut accesskit::Node)` producer-side shape.** Masonry passes the widget a mutable `accesskit::Node` to populate. Buiy's ECS-flavored equivalent: `BuiySet::A11yUpdate` system walks entities with decomposed a11y components and populates the `TreeUpdate.nodes` per entity. The producer-shape is the same: mutate the AccessKit node directly, no megacomponent wrapper. See [`accessibility.md`](accessibility.md), [`../accesskit/lessons.md`](../accesskit/lessons.md) Top of file.

5. **The `masonry_testing` snapshot-rendering test harness.** Render a tree, snapshot-compare with `insta`, push synthesized input, assert on tree state. *Exactly* the shape of Buiy's `buiy_verify` crate's commitments (foundation [`verification.md`](../../specs/2026-05-07-buiy-foundation/verification.md)). Reading `masonry_testing`'s API + `insta` integration is the closest published reference for visual-regression + synthesized-input testing of a Rust UI. See [`masonry-toolkit.md`](masonry-toolkit.md) § "Testing infrastructure."

6. **Parley's `accesskit` feature** as the model for text-run-to-AccessKit-node mapping. `buiy_text` needs the equivalent for cosmic-text (which lacks the feature). When `buiy_text` builds AccessKit `Node`s for text widgets, the per-run boundaries should map onto Parley's shape. See [`accessibility.md`](accessibility.md), [`text-and-rendering.md`](text-and-rendering.md).

7. **Stable `WidgetId` (Masonry) ⇔ `Entity::to_bits()` (Buiy) as AccessKit `NodeId`.** Same identity-stability principle; same diff-model match with AccessKit. See [`accessibility.md`](accessibility.md).

8. **`async-io` feature flag on `accesskit_winit` (Linux).** Lighter dep closure than `tokio` for the async runtime AccessKit needs. Linebender's choice; Buiy adopts the same. See [`accessibility.md`](accessibility.md).

9. **Linebender Color for CSS-spec-compliant gradient sampling.** Buiy's render pipeline implements "gradients in any color space" (foundation [`visuals.md`](../../specs/2026-05-07-buiy-foundation/visuals.md)); Linebender Color is the published reference for OKLab / Display-P3 / ICC interpolation correctness. Could be a direct dep or a reference implementation; either way worth studying. See [`linebender-stack.md`](linebender-stack.md).

10. **Kurbo for 2D-curve utilities.** *The* Rust 2D-curve library. Path-flattening, intersection, bounding-box. Could be a direct dep for Buiy's clip-path implementation. See [`linebender-stack.md`](linebender-stack.md).

11. **The unbundled-substrate strategy itself.** Buiy's foundation [`architecture.md § 2.2`](../../specs/2026-05-07-buiy-foundation/architecture.md) integrates substrate primitives directly (Taffy, cosmic-text, AccessKit, bevy_picking, Bevy render-graph). Linebender's strategy validates this *as a strategy*; the proof is that Linebender's substrate crates are multi-consumer-adopted while their framework is Linebender-internal. See [`linebender-stack.md`](linebender-stack.md), [`ecosystem-comparisons.md`](ecosystem-comparisons.md).

12. **Three-named-active-leads as a bus-factor target.** Linebender's leadership shape is structurally more durable than solo-author projects. Buiy aims for ≥3 named active leads before claiming v1 (call out in maintenance plan). See [`distribution-governance.md`](distribution-governance.md), [`../woodpecker-ui/lessons.md`](../woodpecker-ui/lessons.md) Avoid row.

## How to use this file

When designing a Buiy feature that touches substrate (text, rendering, layout, a11y):

1. **First check [`../accesskit/lessons.md`](../accesskit/lessons.md), [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md), [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)** — those are direct-dependency prior-art. Their lessons are higher-priority.
2. **Then check this file** for the architectural decomposition shape (substrate-vs-framework split, Widget::accessibility shape, masonry_testing harness pattern).
3. **For specific substrate questions** (Vello capability set, Parley vs cosmic-text, Linebender Color), read [`linebender-stack.md`](linebender-stack.md) and [`text-and-rendering.md`](text-and-rendering.md).
4. **Don't lift Xilem code directly** — single-Apache-2.0 license + experimental status + paradigm mismatch with Buiy's ECS+BSN model. Borrow patterns, not code.
5. **Promote decisions into Buiy specs.** This file captures lessons; commitments live in `docs/specs/`.

## Sources

- This corpus's evidence files — [`README.md`](README.md), [`xilem-architecture.md`](xilem-architecture.md), [`masonry-toolkit.md`](masonry-toolkit.md), [`linebender-stack.md`](linebender-stack.md), [`text-and-rendering.md`](text-and-rendering.md), [`history.md`](history.md), [`accessibility.md`](accessibility.md), [`distribution-governance.md`](distribution-governance.md), [`ecosystem-comparisons.md`](ecosystem-comparisons.md), [`critiques-and-open-problems.md`](critiques-and-open-problems.md), [`glossary.md`](glossary.md)
- Buiy foundation spec — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Sibling Buiy prior-art folders (consult before this one for direct deps): [`../accesskit/lessons.md`](../accesskit/lessons.md), [`../cosmic-text/lessons.md`](../cosmic-text/lessons.md), [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md), [`../woodpecker-ui/lessons.md`](../woodpecker-ui/lessons.md)
- Xilem repo: https://github.com/linebender/xilem
- Xilem paper (Raph Levien, 2022-05-07): https://raphlinus.github.io/rust/gui/2022/05/07/ui-architecture.html
