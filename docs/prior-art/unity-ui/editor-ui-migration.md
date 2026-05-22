**Date:** 2026-05-22
**Status:** active
**Subject:** Unity UI — the 7+-year IMGUI → UI Toolkit migration of the Unity Editor; lessons for Buiy's BSN authoring story

# The Editor migration story

Between 2017 (UIElements debut) and 2026 (Unity 6.3 LTS), Unity Technologies has been migrating its **own Editor** from IMGUI to UI Toolkit. The migration is ongoing — most built-in Editor windows still embed IMGUI containers, the Inspector is a hybrid, and Unity Discussions hosts running threads of the form `"When will IMGUI be fully replaced by UI Toolkit?"`. The honest summary: **a complete migration of a mature, internally-used UI system to a new substrate takes 7+ years even with full vendor commitment**. This is the cautionary tale most relevant to Buiy's foundation §2.4 (BSN authoring) and to any future migration off bevy_ui for projects considering Buiy.

## Timeline (verified from public sources)

| Year | Event |
|---|---|
| 2017 | UIElements first announced; roadmap published 2018 |
| 2019.1 | UIElements (a.k.a. `UIElements` package) ships in Editor; first Editor windows authorable in it |
| 2020.1 beta | UIElements **renamed to UI Toolkit** (Unity Discussions thread, March 2020) |
| 2021 LTS | UI Toolkit built-in (no longer a separable package); runtime UI Toolkit shipped; **UI Builder** WYSIWYG editor matures |
| 2022 LTS | UI Toolkit declared "production-ready"; many new Editor windows author-in UI Toolkit |
| 2023.2 | UI Toolkit gains data-binding APIs (SerializedObject + custom runtime bindings); Accessibility module ships |
| 2024 (Unity 6) | UI Toolkit improvements; Vector Graphics package fully integrated |
| 2025 (Unity 6.2) | World-space UI Toolkit added at runtime |
| 2026 (Unity 6.3 LTS) | UGUI still officially recommended for runtime; many built-in Editor windows still IMGUI-shelled |

**Total elapsed time:** 7 years (2019→2026) from first Editor adoption to current state, and still incomplete.

## What got migrated

- New Editor windows added since ~Unity 2020.1 are authored predominantly in UI Toolkit.
- The **package manager**, **Asset Database** browser views, **Project Settings** panels, **Preferences** UI, **Profiler tabs**, and many newer feature windows (XR Setup, Adaptive Performance, etc.) use UI Toolkit.
- **Custom Inspector** code can be authored via UI Toolkit (the `CreateInspectorGUI()` override returns a `VisualElement`); IMGUI Inspectors (`OnInspectorGUI()`) still work alongside.

## What did not get migrated (as of Unity 6.3)

Community surveys identify these as still-IMGUI-or-hybrid:

- **Hierarchy window** — still IMGUI per multiple community surveys.
- **Project window** — still IMGUI.
- **Animator graph window** — still IMGUI.
- **Default Inspector** — IMGUI is still the default; UI Toolkit Inspector is opt-in per `Editor` subclass.
- **Many older built-in EditorWindows** — `IMGUIContainer` shells with `rootVisualElement` left empty.

Per community reporting (Unity Discussions, "When will IMGUI be fully replaced by UI Toolkit?"): *"even in Unity 6, many editor windows, including most built-in windows, mostly consist of nothing else than an IMGUIContainer ... either because they're still unachievable with UI Toolkit, or very likely because they're too huge to be reworked."*

## What worked

- **Interop primitives shipped early.** `IMGUIContainer` lets IMGUI code live inside a `VisualElement` tree and vice versa. New code can be UI Toolkit; old code can stay IMGUI; the same window can mix. This is the migration's load-bearing primitive.
- **The Inspector dual-path.** `Editor.CreateInspectorGUI()` (UI Toolkit) and `Editor.OnInspectorGUI()` (IMGUI) coexist; a third-party `Editor` script can opt in to UI Toolkit without breaking IMGUI users.
- **UI Builder dropped the learning cliff.** Without UI Builder (Unity 2021 LTS+), every Editor extension author would have had to write UXML/USS by hand. UI Builder closed the WYSIWYG gap and made UI Toolkit a tractable choice for first-time Editor extension authors.
- **Stability commitments.** Unity 2022 LTS marked the surface as stable; subsequent breaking changes were rare. This let third-party Editor extensions commit.

## What did not work

- **The migration timeline was vastly under-estimated.** Public Unity statements ~2019 implied a multi-year migration; in 2026 we're 7+ years in and the Hierarchy / Project / Animator are unmigrated. The cost of migrating mature internal UI to a new substrate exceeds the cost of building the substrate.
- **Two systems coexisting forever.** IMGUI is now de facto permanent. Unity Discussions threads ask "will IMGUI be removed" — the official answer is essentially "no, not in any committed timeline." The "legacy stack persistence" tax is a permanent line-item cost.
- **Hybrid code is uglier than either pure system.** `IMGUIContainer`s mixing into UXML, and `EditorGUILayout` blocks calling out to UI Toolkit panels via callbacks, are operationally functional but produce code that is harder to read than either pure IMGUI or pure UI Toolkit.
- **Third-party Editor-extension ecosystem fragmented.** Asset Store inspectors split into "pure IMGUI" (older) vs "UI Toolkit" (newer) vs "hybrid" (transitioning) camps; consumers download a UI Toolkit inspector and find a half-migrated experience.
- **Accessibility lag for the Editor.** The Editor itself doesn't pass accessibility audits; the migration moved style but didn't fix structural a11y. Editor accessibility is its own open problem (Discussions thread cited in [`accessibility.md`](accessibility.md)).
- **Runtime/Editor divergence in practice.** Even though UI Toolkit unifies Editor + runtime in theory, in practice the Editor uses many Editor-only elements (`PropertyField`, `ObjectField`, `EnumField`) that don't run at runtime, and runtime UI Toolkit projects rarely import Editor USS themes. The "write once, run in either" promise is partly aspirational.

## Why the migration took (and is taking) so long

Three factors, drawn from community discussion and Unity blog posts:

1. **Internal API churn.** UI Toolkit's own surface changed substantially between 2019.1 and 2021 LTS (the rename was just the visible tip — element APIs, USS attribute syntax, factory patterns all shifted). Migrating windows mid-churn meant re-migrating.
2. **Capability gaps.** Some IMGUI capabilities (custom-drawn editor widgets like graphs, timelines, blend-tree visualisations) had no UI Toolkit analog for years. The Animator window can't migrate until a graph-visualisation primitive ships.
3. **Internal headcount allocation.** Migrating a mature, well-functioning Editor window has no immediate user-visible win; the migration competed against new-feature work for engineer time and usually lost.

## Implications for Buiy

Buiy's foundation §2.4 commits to BSN-friendly components from day one. Unity's migration story is the most important argument *for* paying that tax now.

1. **You can't migrate later cheaply.** Unity Technologies has full corporate commitment, dedicated team, source access — and is 7+ years in with the migration incomplete. Buiy as a community project has none of those advantages; if Buiy ships BSN-hostile components first and tries to migrate later, the migration will not complete. **Buiy commits to BSN-friendly from day one** (foundation §2.4 hard rule). This file is the cited justification.
2. **Plan for legacy-stack persistence anyway.** Even with BSN-friendly day-one components, Buiy will have *some* coexistence tax — with `bevy_ui` itself (foundation §2.9 + cross-cutting.md §3.18) and with non-Buiy widget kits. The IMGUI tax in Unity is permanent; Buiy's coexistence story with bevy_ui will be permanent too. Foundation cross-cutting.md §3.18 already commits to per-window coexistence; don't promise more.
3. **Ship an interop primitive early.** Unity shipped `IMGUIContainer` early; that was load-bearing. Buiy's equivalent — a way for Buiy and bevy_ui (or other UI stacks) to share a window where the user accepts the trade-off — is the `buiy-coexistence-design` conditional sub-spec (foundation README §4). Treat it as "ship the primitive when there's demand, but the design must exist before demand."
4. **Visual authoring tool is load-bearing.** UI Builder was the migration's adoption multiplier; without it, third-party Editor extension authors would not have migrated. Buiy's BSN visual authoring (a future spec area, currently in `buiy-bsn-integration-design`) is the equivalent and is **not optional** if Buiy wants third-party adoption.
5. **Don't promise the migration done by date X.** Unity told the community "soon" repeatedly between 2019 and 2023. Eventually the community stopped believing it. Buiy's planning docs (foundation README §4 — "phasing lives in plans/, not here") already commit to not over-promising; this is the precedent justifying that.
6. **Capability gaps block migration.** The Animator window can't migrate because graph visualization wasn't in UI Toolkit until late. Buiy must commit to its **full primitive surface** (foundation §3.1-§3.10) before the *expectation* of migration matters — half-finished primitive surface means stuck migrations.
7. **Authoring-tool accessibility is its own problem.** Unity Editor a11y is open. Buiy's BSN tool will face the same problem. Foundation accessibility.md §3.11 already commits to APG conformance for built-in widgets; the authoring-tool-itself accessibility should be a named requirement of `buiy-bsn-integration-design`.

## Sources

- Unity Discussions: When will IMGUI be fully replaced by UI Toolkit? — https://discussions.unity.com/t/when-will-imgui-be-fully-replaced-by-ui-toolkit/1616844
- Migrate from IMGUI to UI Toolkit (manual) — https://docs.unity3d.com/6000.3/Documentation/Manual/UIE-IMGUI-migration.html
- UIElements renaming announcement — https://discussions.unity.com/t/renaming-uielements-to-ui-toolkit/782459
- State of UI in Unity (One Wheel Studio, Oct 2022) — https://onewheelstudio.com/blog/2022/10/28/state-of-ui-tool-kit
- I Researched UI Toolkit So You Don't Have To — https://darkounity.com/blog/i-researched-ui-toolkit-so-you-dont-have-to
- UnityToolkitMixingExamples — https://github.com/JC3/UnityToolkitMixingExamples
- Unity Editor Scripting Series Chapter 4 — https://medium.com/@dilaura_exp/unity-editor-scripting-series-chapter-4-imgui-ui-toolkit-772db20a21fa
- Buiy foundation architecture §2.4 — [`../../specs/2026-05-07-buiy-foundation/architecture.md`](../../specs/2026-05-07-buiy-foundation/architecture.md)
- Buiy foundation cross-cutting §3.18 — [`../../specs/2026-05-07-buiy-foundation/cross-cutting.md`](../../specs/2026-05-07-buiy-foundation/cross-cutting.md)
- bevy_ui lessons (cross-link, BSN-friendly day-one) — [`../bevy-ui/lessons.md`](../bevy-ui/lessons.md)
