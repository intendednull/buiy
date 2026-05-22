**Date:** 2026-05-22
**Status:** active
**Subject:** Unity UI — Validates / Avoid / Borrow decisions for the Buiy foundation

# Lessons for Buiy

This is the consult-this-when-designing decision file for Unity UI. The other files in this corpus are evidence; this file is the synthesis. Three sections — **Validates** (Buiy choices Unity's experience confirms), **Avoid** (pitfalls to mitigate), **Borrow** (primitives worth studying).

Unity UI is the **single most important** prior-art in this corpus for one reason: **UI Toolkit is the only worked example of "comprehensive web-platform-inspired UI shipping inside a production game engine at massive scale."** Every Buiy design choice that touches authoring (UXML-style declarative + ECS-spawn dual-path), styling (USS-style stylesheets, foundation §5 open question), layout (Yoga/Taffy substrate parallel), or accessibility (the 9-year UGUI gap that Buiy's day-one commitment is designed against) has a Unity precedent to learn from.

## Top of file: three findings that frame the synthesis

### 1. UI Toolkit proves "web-platform parity in a game engine" is shippable.

UXML + USS + VisualElement + Yoga + UI Builder + Editor + runtime + 5+ years of production use at Unity scale = existence proof that the Buiy foundation thesis (foundation §2.1-§2.3) is achievable. Buiy's *parallel-stack* bet is structurally identical to Unity shipping UI Toolkit alongside UGUI — and UI Toolkit's market penetration (Editor since 2019.1, runtime since 2021 LTS, recommended for menus / data-heavy UI by 2025) confirms the model works.

### 2. The cost of "comprehensive" is multi-year + permanent legacy tax.

UI Toolkit's Editor migration is 7+ years in and incomplete. UGUI is still officially recommended for runtime as of Unity 6.3 LTS. The legacy stack does not go away. This reframes Buiy's foundation §2.4 hard rule (BSN-friendly day one) — paying the small-decomposed-component tax now is **cheap relative to migrating later**, and Unity's experience is the cited proof.

### 3. Unity ships no ARIA model and no WCAG conformance claim — Buiy's day-one accessibility commitment is the differentiator.

Unity Technologies has full corporate commitment, dedicated team, and twelve years of UGUI runway, and yet: no ARIA `role` attribute in UXML, no `aria-label`, no APG keyboard contracts, no first-party WCAG claim, no automatic role inference for built-in widgets. The 2023.2+ Accessibility module is an after-the-fact bridge, not an authoring-level primitive. Buiy's foundation §2.6 + accessibility.md is *structurally further along* than Unity's offering as of Unity 6.3. This is the single largest opportunity Buiy has to differentiate against the most production-validated game UI on Earth.

---

## Validates

These Buiy design choices are confirmed by Unity UI's experience:

- **Parallel-stack rationale (foundation README § 1.4).** Unity ships UGUI and UI Toolkit side-by-side and has done since 2019.1 — seven years of two-stack-cohabitation in the same engine. Per-project hybrid choices ("UGUI for game HUD, UI Toolkit for menus") work at scale. This is the **strongest validation** of Buiy standing parallel to `bevy_ui` rather than replacing it. See [`ui-toolkit-architecture.md`](ui-toolkit-architecture.md), [`ugui-architecture.md`](ugui-architecture.md), [`history.md`](history.md).
- **Comprehensive web-platform parity is achievable (foundation README goal 1).** UXML + USS + Yoga + UI Builder + Editor + runtime is the worked example. Even with USS-vs-CSS divergences, UI Toolkit demonstrates that a CSS-flavoured stylesheet on a game-engine UI **ships at scale, is taught at scale, and is adopted at scale**. See [`uxml-uss-web-parallels.md`](uxml-uss-web-parallels.md).
- **Token-style theming over per-component fields (foundation § 2.5).** UGUI's `Selectable.colors` per-Selectable megacomponent is the canonical anti-pattern. USS class selectors are a better answer but still less disciplined than Buiy's semantic tokens. The progression Buiy follows is correct. See [`ugui-architecture.md`](ugui-architecture.md), [`uxml-uss-web-parallels.md`](uxml-uss-web-parallels.md).
- **Decomposed components over megacomponents (foundation § 2.4).** UGUI's `ColorBlock` struct + Selectable fields, and Unity's late accessibility integration (Accessibility module 2023.2+) are both shaped by absent decomposition. Buiy's decomposed `A11yRole` / `A11yLabel` / etc. (foundation §2.6) is the corrective. See [`accessibility.md`](accessibility.md).
- **Day-one accessibility commitment (foundation § 2.6).** Unity 2014 → 2023.2 is *nine years* of UGUI shipping a11y-less. Most existing Unity titles will never be accessible. Buiy's AccessKit-first commitment is validated by this gap; the cost of catching up is in this corpus. See [`accessibility.md`](accessibility.md).
- **OS-preference integration via `UserPreferences` resource (foundation § 2.5).** USS has *zero* support for `prefers-reduced-motion`, `prefers-contrast`, `forced-colors`, `prefers-color-scheme`. Buiy's OS-preference-driven theme variants are a clean lead. See [`uxml-uss-web-parallels.md`](uxml-uss-web-parallels.md).
- **Single batched mesh per panel render pipeline (foundation § 2.3).** UI Toolkit's batched-mesh-per-panel approach beats UGUI's per-element `CanvasRenderer` model at 1000+ elements. Buiy's render pipeline follows the same shape. See [`ui-toolkit-architecture.md`](ui-toolkit-architecture.md).
- **First-class world-space UI (foundation § 2.3 + `buiy-3d-anchored-ui-design`).** UGUI has had world-space Canvas since 2014 (eleven years before UI Toolkit caught up in Unity 6.2). World-space UI is a foundational primitive, not a late-add. Buiy commits day one. See [`ugui-architecture.md`](ugui-architecture.md).
- **Devtools-grade UI inspector (foundation § 2.3, `buiy_devtools`).** UI Toolkit Debugger + UI Builder + Accessibility Hierarchy Viewer set the bar. Buiy's `buiy_devtools` should match — tree inspection, matched selectors, layout boxes, focus order, accessibility tree. See [`ui-toolkit-architecture.md`](ui-toolkit-architecture.md), [`accessibility.md`](accessibility.md).
- **Multi-segment widget catalog: game-UI + productivity-UI (foundation README goal 6).** Unity ships UGUI for game UI and App UI (`com.unity.dt.app-ui`) for productivity-app patterns — *two separate kits* to cover the same span Buiy commits to in a single catalog. Buiy's single-catalog approach is more disciplined; the existence of App UI as a separate Unity package validates that "game-only" widget kits are insufficient for productivity-app use cases. See [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md).
- **Open-source substrate over proprietary commercial steward (foundation § 2.9).** The Unity Runtime Fee 2023 saga (announced 2023-09-13, cancelled 2024-09-12 after CEO + CPO departures) demonstrated that proprietary substrate carries unilateral-change risk. Buiy's MIT/Apache dual-licensed open source plus the Bevy Foundation governance are the structural answer. See [`distribution-and-governance.md`](distribution-and-governance.md).
- **Taffy substrate (foundation § 2.2) over Yoga.** Yoga implements Flexbox subset only — no Grid, no subgrid, no container queries, no anchor positioning. Taffy ships all of these or has them on roadmap. Buiy inherits a structurally more complete layout substrate than Unity's worked example. See [`ui-toolkit-architecture.md`](ui-toolkit-architecture.md), [`uxml-uss-web-parallels.md`](uxml-uss-web-parallels.md).

## Avoid

Pitfalls drawn from Unity UI's experience, with Buiy's mitigation.

| Pitfall | Source | Buiy mitigation |
|---|---|---|
| **Editor migration cost (7+ years and counting)** — IMGUI → UI Toolkit migration started 2019.1; still incomplete 2026. Hierarchy / Project / Animator / many built-in windows are IMGUI shells. Unity has full corporate commitment + dedicated team; community substitute will not match those resources. | [`editor-ui-migration.md`](editor-ui-migration.md). | Commit BSN-friendly day one (foundation §2.4 hard rule: small, public-fielded, observable, decomposed, reflection-registered). No legacy stack to migrate. Coexistence with `bevy_ui` per-window only (foundation §2.9 + cross-cutting.md §3.18). |
| **USS-vs-CSS divergence creates onboarding friction** — `display: flex|none` only, no `calc()`, no `@keyframes`, no `:focus-visible`, no Grid, no media queries, no `:nth-child()`. Web devs hit these within hours and lose confidence. | [`uxml-uss-web-parallels.md`](uxml-uss-web-parallels.md). | If Buiy ships a stylesheet (foundation README §5 open question), commit to true CSS semantics where the feature is supported and **omit** unsupported features rather than provide divergent versions. Let Buiy-specific additions be additions (semantic-token-flavoured), not subtractions. |
| **No keyframed animation in USS (transitions only)** — single most-cited UI Toolkit runtime gap in community discussion 2024-2026. Complex sequence animation requires C# `schedule` loops or hybrid Animator-on-UIDocument tricks. | [`ui-toolkit-architecture.md`](ui-toolkit-architecture.md), [`critiques-and-open-problems.md`](critiques-and-open-problems.md). | `buiy-animation-design` sub-spec (foundation §4) commits to transitions + keyframes + springs day one. |
| **No ARIA model in authoring layer** — UXML has no `role` / `aria-label` / `aria-describedby` / `aria-expanded` attribute surface. Accessibility module (2023.2+) is a separate parallel API. Application code builds the AccessibilityHierarchy manually for every project. | [`accessibility.md`](accessibility.md). | Buiy's decomposed `A11yRole` / `A11yLabel` / `A11yDescription` / `A11yStates` / `A11yRelations` components (foundation §2.6) ship as required-components on every Buiy widget. Automatic role inference for built-in widgets via `#[require(...)]`. |
| **No first-party WCAG conformance claim** — Unity does not warrant engine output meets WCAG. Game-side conformance is developer's burden. No first-party contrast linter, no focus-order validator, no SC-coverage table. | [`accessibility.md`](accessibility.md), [`critiques-and-open-problems.md`](critiques-and-open-problems.md). | Buiy foundation README goal 2 + accessibility.md §3.11 enumerate WCAG SCs with verification strategy per. CI gate for machine-testable SCs; manual release gate for human-judgment SCs (foundation verification.md). |
| **Legacy stack persistence** — UGUI is still officially recommended for runtime twelve years after launch and seven years after UI Toolkit's debut. The legacy stack does not go away; the project carries two stacks forever. | [`history.md`](history.md), [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md). | Buiy does not have a legacy stack — but Buiy will coexist with `bevy_ui` indefinitely. Foundation cross-cutting.md §3.18 commits to per-window coexistence only. Don't promise full migration off bevy_ui; the cost would not be recouped. |
| **Performance cliff at 1000+ active CanvasRenderers (UGUI)** — single-Canvas re-batch on any descendant mutation. Mitigation patterns (split Canvas, disable RaycastTarget, pool list items) are well-known but require manual application. | [`ugui-architecture.md`](ugui-architecture.md). | Buiy's render pipeline (foundation §2.3) uses dirty-region tracking and per-VisualElement-equivalent mesh updates rather than single-batch-per-tree. Verification harness fixtures include 1000+ node scenes (foundation verification.md). |
| **Proprietary substrate governance risk** — Unity Runtime Fee 2023-2024 demonstrated unilateral pricing change attempt; the reversal took a year, a CEO departure, and a CPO resignation. Closed-source forecloses community fixes. | [`distribution-and-governance.md`](distribution-and-governance.md). | Open-source dual MIT/Apache (foundation §2.9). Bevy Foundation governance. Community fixes routable upstream. |
| **Per-element field-tweak theming (UGUI's `ColorBlock`, USS class theming as the only alternative)** — every project re-invents a theme system. No first-class semantic-tokens primitive in either Unity stack. | [`ugui-architecture.md`](ugui-architecture.md), [`uxml-uss-web-parallels.md`](uxml-uss-web-parallels.md). | Buiy's token-based theming (foundation §2.5) — semantic tokens, hot-reloadable theme assets, OS-pref-driven variant binding, contrast linter. The "right answer" for the open question in foundation README §5 (stylesheet) is "if at all, atop tokens, not instead of." |
| **Web (WebGL) accessibility has no bridge** — Unity-WebGL emits no `aria-*` to the surrounding DOM. WebGL accessibility is essentially impossible. | [`accessibility.md`](accessibility.md). | Buiy's WASM target waits on AccessKit's web adapter (foundation README §5 open question on Bevy WASM target policy). When AccessKit's web adapter ships, Buiy WASM has a11y — Unity WebGL never will (no equivalent). |
| **Two competing stacks confuse adoption** — Unity ships UGUI + UI Toolkit; recommendation has shifted over years; community hybrid patterns fragment further. | [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md). | Buiy is **one** stack. Coexistence with `bevy_ui` is per-window (foundation cross-cutting.md §3.18); Buiy itself does not split into two competing surfaces. |
| **Complex Indic shaping + Arabic shaping quality gaps** — TMP/TextCore in-house shaper underperforms HarfBuzz on Indic conjuncts, Arabic ligatures. | [`text-rendering.md`](text-rendering.md). | cosmic-text + rustybuzz (HarfBuzz port) substrate (foundation §2.2). HarfBuzz-quality shaping out of the box. |
| **No vertical writing mode (`writing-mode: vertical-rl`)** — no Unity text stack supports traditional Japanese/Chinese typesetting. | [`text-rendering.md`](text-rendering.md). | `buiy-i18n-design` sub-spec scopes vertical writing. cosmic-text supports vertical layout. |
| **Capability gaps block migration** — Unity Animator window cannot migrate to UI Toolkit because graph-visualization primitive wasn't shipped early. | [`editor-ui-migration.md`](editor-ui-migration.md). | Buiy foundation enumerates the full primitive surface (foundation §3.1-§3.10) upfront; tier markers F/C/E ensure foundation primitives ship before widgets depend on them. |

## Borrow

Concrete primitives worth studying and adapting:

1. **VisualElement tree pattern.** Single tree per panel; declarative authoring; single batched mesh. Buiy's render pipeline + ECS-entity-as-VisualElement-analog is structurally the same shape, adapted to Bevy ECS. Study UI Toolkit's panel + UIDocument + VisualElement hierarchy as the worked example. See [`ui-toolkit-architecture.md`](ui-toolkit-architecture.md).

2. **UXML-style declarative markup mapped to native types.** UXML is XML with element names mapped to `VisualElement` subclasses by reflection. BSN is the Bevy-native analog (reflection-driven asset format, components addressed by type). Buiy's BSN authoring (foundation §2.4) inherits the lesson that *declarative-asset → typed-object reflection is the right shape*. See [`ui-toolkit-architecture.md`](ui-toolkit-architecture.md).

3. **USS-style cascading + class selectors + custom-property variables.** If Buiy ever ships a stylesheet (foundation README §5), the USS feature surface is the floor: type/class/id/descendant selectors, pseudo-states (`:hover`/`:active`/`:focus`/`:disabled`/`:checked`), custom properties via `var()`, transitions. Add what USS omits: `calc()`, `@keyframes`, `:focus-visible`, media queries (`prefers-*`), Grid, container queries. See [`uxml-uss-web-parallels.md`](uxml-uss-web-parallels.md).

4. **UQuery-style query selector API.** `element.Q<T>(name, className)` and `element.Query<T>()` return LINQ-style results. Buiy's devtools / introspection API benefits from an analog — a `query::<T>(world, name, class)` over the Buiy entity tree. See [`ui-toolkit-architecture.md`](ui-toolkit-architecture.md).

5. **SerializedObject data binding pattern (Editor-side).** UXML's `<PropertyField binding-path="..."/>` binds element to a serialized property by path. The pattern (asset-driven binding paths into a reflection-described object) is the right shape for Buiy's BSN authoring tool integration — *bind a Buiy widget property to a Bevy reflected field via a path expression*. See [`ui-toolkit-architecture.md`](ui-toolkit-architecture.md).

6. **`<Instance template="..."/>` template inclusion.** UXML's template-inclusion mechanism (`<Instance>` references another UXML asset, parameters via attributes) is the worked example for BSN-template composition. Buiy's BSN authoring should support an analog (`<bsn:template src="my-card.bsn"/>` or similar). See [`ui-toolkit-architecture.md`](ui-toolkit-architecture.md).

7. **UI Builder WYSIWYG editor.** UI Toolkit's adoption multiplier; without it, third-party Editor-extension authors would not have migrated. Buiy's BSN visual authoring tool (a future spec area inside `buiy-bsn-integration-design`) is the direct equivalent; UI Builder's scope (UXML hierarchy + USS rule editing + live preview) is the spec to match. See [`editor-ui-migration.md`](editor-ui-migration.md).

8. **UI Toolkit Debugger.** Live VisualElement tree inspection, matched USS selectors per element, layout box visualization, event log. Buiy's `buiy_devtools` should match the feature set. See [`ui-toolkit-architecture.md`](ui-toolkit-architecture.md).

9. **Accessibility Hierarchy Viewer.** Editor window showing the live `AccessibilityHierarchy` in Play mode — node roles, labels, states. Buiy's devtools accessibility viewer (foundation §2.3) should match this shape with the AccessKit tree as the data source. See [`accessibility.md`](accessibility.md).

10. **9-slice background as a USS primitive (`-unity-slice-*`).** Stretchy panels / borders for game-UI buttons/cards/dialogs. CSS has no `background-slice`; Unity's `-unity-slice-*` family is the right shape for an engine-specific addition Buiy should keep in mind for its visual styling sub-spec. See [`uxml-uss-web-parallels.md`](uxml-uss-web-parallels.md).

11. **TMP-style inline rich-text markup (`<color=red>`, `<size=20>`, `<sprite=0>`, `<link>`).** Inline formatting within text strings — more ergonomic than nested `<span>` for game-UI scenarios where text changes shape mid-line. `buiy-text-rendering-design` should consider an analog (probably TMP-tag-compatible or a small Buiy-defined inline set). See [`text-rendering.md`](text-rendering.md).

12. **Sprite-Asset fallback inside text runs.** Inline glyph fallback to image / SVG resources within a text run — gamepad button glyphs, item icons in chat, custom emoji. Buiy's text rendering should consider a per-glyph fallback to image resources as a first-class primitive. See [`text-rendering.md`](text-rendering.md).

13. **Editor + runtime unified UI substrate.** UI Toolkit ships the same code path for Editor and runtime. Buiy's BSN authoring tool, when built, should run on Buiy itself — *not* a separate UI substrate. The "eat your own dogfood" benefit is operational (every dogfood bug is also a user bug) and confidence-building. See [`ui-toolkit-architecture.md`](ui-toolkit-architecture.md), [`editor-ui-migration.md`](editor-ui-migration.md).

14. **The IMGUI → UI Toolkit migration playbook.** For any Buiy user who comes from a bevy_ui codebase and wants to migrate, the Unity migration story is the precedent. Key primitives that worked: interop shim (`IMGUIContainer`), gradual per-window opt-in, dual-path overrides on the same MonoBehaviour, staged feature parity. Buiy's future `buiy-coexistence-design` (conditional sub-spec, foundation README §4) should study this playbook before designing. See [`editor-ui-migration.md`](editor-ui-migration.md).

## How to use this file

When designing a Buiy feature:

1. **Find the row in `Avoid`** that names a pitfall close to your design. Read the linked file for the original incident.
2. **Find the entry in `Borrow`** that names a primitive close to what you're designing. Read the linked file to understand the Unity shape, then adapt for Buiy's component model (decomposed, public-fielded, observable, reflection-registered, ECS-spawned).
3. **Promote any decision into a Buiy spec** under `docs/specs/` — this file captures what we learn from Unity, not Buiy's own decisions.

A note on framing: Unity UI is proprietary, closed-source, commercial substrate. Buiy borrows **lessons** and **architectural patterns** — never code or designs verbatim. The Unity-UI experience is informational input, not implementation reference.

## Sources

- Sibling evidence files: [`README.md`](README.md), [`ugui-architecture.md`](ugui-architecture.md), [`ui-toolkit-architecture.md`](ui-toolkit-architecture.md), [`uxml-uss-web-parallels.md`](uxml-uss-web-parallels.md), [`text-rendering.md`](text-rendering.md), [`accessibility.md`](accessibility.md), [`editor-ui-migration.md`](editor-ui-migration.md), [`history.md`](history.md), [`distribution-and-governance.md`](distribution-and-governance.md), [`ecosystem-and-comparisons.md`](ecosystem-and-comparisons.md), [`critiques-and-open-problems.md`](critiques-and-open-problems.md), [`glossary.md`](glossary.md)
- Buiy foundation README — [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md)
- Buiy foundation architecture — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Buiy foundation accessibility — [`../../specs/2026-05-07-buiy-foundation/accessibility.md`](../../specs/2026-05-07-buiy-foundation/accessibility.md)
- Buiy foundation text — [`../../specs/2026-05-07-buiy-foundation/text.md`](../../specs/2026-05-07-buiy-foundation/text.md)
- bevy_ui lessons (cross-link) — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
