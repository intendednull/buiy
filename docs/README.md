# Buiy docs

Master index of Buiy's design specs, implementation plans, and reports. Grouped by feature area for discovery and onboarding.

For build/test/dev commands, see `../CLAUDE.md`. This file does not duplicate that content.

## Where to start (new agents and humans)

Reading order for newcomers:

1. [Buiy foundation design](specs/2026-05-07-buiy-foundation/README.md) — the target shape of the library: feature inventory, architectural foundation, sub-spec roadmap. Multi-file folder; start at the README, then read children in the order it lists.
2. [Docs organization design](specs/2026-05-07-docs-organization-design.md) — how this docs tree is structured.

## Document types

Four document types, each with one job. If a doc does not fit one of these, the type list is wrong, not the doc.

- **Spec** (`specs/`) — *what we are building toward.* Target shape of the code: types, traits, invariants, public API. Long-lived, canonical.
- **Plan** (`plans/`) — *how we get from current code to the target.* Migration steps, file-by-file changes, PR breakdown. Cites the spec it realizes. Goes stale once shipped.
- **Report** (`reports/`) — *findings from a one-shot investigation of our codebase.* Audits, post-mortems. Dated, immutable.
- **Prior-art** (`prior-art/<system>/`) — *deep dive on an external system we want to learn from.* Living documents; updated as the external system evolves. One folder per system; categories live in the catalog only.

## Status tags

Specs / plans / reports carry one of:

- `[draft]` — being written, target not yet stable.
- `[active]` — current target / in-flight migration.
- `[landed]` — realized in code; canonical reference.
- `[superseded]` — replaced; entry links to successor.

Prior-art docs carry `[active]` or `[archived]`.

## Catalog

Areas appear here as soon as there is a real doc to slot under them. Each area is a `### ` header with **Specs** and **Plans** subsections; entries are one line each:

```markdown
- [Title](specs/YYYY-MM-DD-name-design.md) — 5–15 word summary. `[draft]`
```

If a doc spans areas, file it under its primary area only. Reference any adjacent topics in the summary.

### Foundation

**Specs**

- [Buiy foundation design](specs/2026-05-07-buiy-foundation/README.md) — feature inventory, architectural foundation, sub-spec roadmap (multi-file). `[draft]`

**Plans**

- [Phase 0 foundations](plans/2026-05-07-buiy-phase-0-foundations.md) — workspace, BuiyPlugin, system sets, minimal render/layout/a11y/focus/picking/theme, verification harness skeleton, hello-world Button. `[landed]`
- [Phase 0 closeout](plans/2026-05-08-buiy-phase-0-closeout.md) — render-pipeline draws, AccessKit per-window adapter, `bevy_picking` backend; closes the three substantive deferrals from the Phase 0 self-review. `[landed]`

### Layout

**Specs**

- [Buiy layout design](specs/2026-05-08-buiy-layout-design/README.md) — Taffy bridge, hybrid `Style` builder + decomposed components, anchor positioning, container queries, writing modes, stacking + top layer, transforms + containment (multi-file). `[active]`

**Plans**

- [Buiy layout foundation](plans/2026-05-08-buiy-layout-foundation.md) — Phase 1: 8-step pipeline skeleton, decomposed components for the Phase-0 surface, hybrid `Style` builder, `Button` migration. `[landed]`
- [Buiy layout overflow and scrolling](plans/2026-05-08-buiy-layout-overflow-and-scrolling.md) — Phase 2: `Overflow` / `Scroll` / `ScrollOffset` / `ScrollSnapItem` components, Taffy overflow mapping, scroll-position-doesn't-invalidate invariant. `[landed]`
- [Buiy layout grid](plans/2026-05-09-buiy-layout-grid.md) — Phase 3: `GridParams` + `GridItem`, `TrackSize` / `GridLine` / `GridAreas` value types, `Display::Grid` → Taffy, Subgrid + Masonry warn-once stubs. `[landed]`
- [Buiy layout writing modes](plans/2026-05-10-buiy-layout-writing-modes.md) — Phase 4: `WritingMode` + `WritingModeResolved`, inheritance pass, `LogicalBoxModel` / `LogicalInset` builders, sideways-* warn-once stubs. `[landed]`
- [Buiy layout container queries](plans/2026-05-21-buiy-layout-container-queries.md) — Phase 5: `Container` + `ContainerQuery`, `Length::Cq{w,h,i,b,min,max}`, `cq_activate` / `cq_flip_check` / `cq_flip_rerun` pipeline systems, same-frame re-layout capped at 2× Taffy. `[landed]`

### Docs infrastructure

**Specs**

- [Docs organization design](specs/2026-05-07-docs-organization-design.md) — target structure of `docs/`, naming, headers, nesting. `[draft]`

## Prior art

External systems we learn from. Living documents — update on revision, archive when no longer worth tracking. Each system has its own subfolder under [`prior-art/`](prior-art/); categories below are organizational groupings in the index only — they do NOT exist as on-disk subfolders. Driven by the `researching-prior-art` skill (creation) and the `using-prior-art` skill (consumption).

### Bevy UI ecosystem

- [bevy_ui](prior-art/bevy-ui/) — official Bevy UI crate, the system Buiy is parallel to; Taffy + cosmic-text (until 0.19) + AccessKit substrate. Consult before any spec on render pipeline, component decomposition, layout integration, or BSN-friendly authoring. `[active]`
- [bevy_ui_widgets](prior-art/bevy-ui-widgets/) — official Bevy **headless widget primitives** crate (Bevy 0.17+); behavior-only components + observers for Button, Checkbox, Radio, Slider, Scrollbar, Menu (0.18+), Popover (0.18+), and EditableText input (0.19+). Sibling to bevy_feathers (which styles them). Latest stable 0.18.1; 201K downloads. Consult before any spec on Buiy's widget catalog, widget event vocabulary, headless-widget pattern, or APG keyboard contracts. `[active]`
- [bevy_picking](prior-art/bevy-picking/) — official Bevy hit-testing primitive; Buiy registers its own backend. Consult before any spec on input, pointer events, drag-and-drop, or focus/picking interaction. `[active]`
- [bevy_feathers](prior-art/bevy-feathers/) — official Bevy widget kit "for editors and utilities" (introduced in Bevy 0.17 via PR #19730 by viridia, 191K downloads). Color-only `UiTheme`, dark-theme-only, sparse AccessKit wiring, `CHECKBOX_SIZE=18` below WCAG 2.5.8 minimum. Consult before any spec on Buiy's widget styling, theme tokens, or per-widget a11y wiring. `[active]`
- [bevy_a11y](prior-art/bevy-a11y/) — official Bevy accessibility crate (since 0.10, PR #6874 by Nolan Darilek; 4.2M downloads). The `AccessibilityNode` megacomponent (issue #17644) is the canonical case study for Buiy's "no megacomponents" rule. PR #24308 (2026-05-21, Bevy 0.19) added a single `AccessibleLabel` sibling but the megacomponent is UNCHANGED; Bevy's a11y story is structurally fragmented across bevy_a11y + bevy_winit + bevy_ui. Buiy replaces bevy_a11y per-window. Consult before any spec on AccessKit producer pattern, decomposed a11y components, focus model, or per-window adapter ownership. `[active]`
- [bevy_flair](prior-art/bevy-flair/) — third-party CSS stylesheets on top of bevy_ui (5,885 downloads, single maintainer, bus factor 1); the only published "CSS in Bevy UI" precedent. Consult **before** drafting any future `buiy-css-stylesheet-design` sub-spec — the foundation README §5 leaves that an open question. `[active]`
- [belly](prior-art/belly/) — third-party Bevy UI plugin packaging HTML-like `eml!` markup + CSS-like `.ess` stylesheets + `from!`/`to!` data bindings (436 stars, never published to crates.io, single maintainer `jkb0o`, last release v0.5.0 Bevy 0.13 April 2024, no commits since — effectively dormant). The broader-scope precedent paired with bevy_flair on the foundation README §5 stylesheet question. Read together with [bevy_flair](prior-art/bevy-flair/) as the "what a stalled, never-published version of CSS-on-Bevy looks like" cautionary tale. `[active]` (with staleness flag)

### Non-Bevy Rust GUI

- [Freya](prior-art/freya/) — cross-platform, native (non-web) Rust GUI library powered by **Skia** (via `freya-skia-safe`) for rendering and **Dioxus** for reactivity, with own layout engine **Torin** (NOT Taffy) and AccessKit-integrated a11y. Single maintainer (Marc Espín Sanz, Barcelona, since 2022-07-27); pre-1.0 with active rc churn (`0.4.0-rc.19` 2026-04-23; last stable `0.3.4` June 2025). MIT, 33,720 downloads, ~2.8k stars, desktop-only. Closest existing-art for "Skia + reactive Rust UI"; structurally incompatible with Buiy (wgpu, not Skia; Bevy ECS, not Dioxus). Borrowable lessons: Dioxus signals work outside Dioxus core, CSS-aligned attribute naming, the Skia primitive set as a wgpu-shader checklist. Avoid: Skia C++ dep, Dioxus coupling, own-layout-engine path, stringly-typed styling props, single-maintainer governance. Consult before any spec touching reactivity (open question § 5), render primitives, or solo-vs-team governance. `[active]`
- [Xilem + Masonry](prior-art/xilem-masonry/) — Linebender's next-generation Rust UI experiments. Sibling crates in `linebender/xilem` (both 0.4.0, 2025-10-29): **Masonry** is the retained-mode widget toolkit (`Widget::accessibility(&mut accesskit::Node)` shape, AccessKit-integrated, own BoxConstraints layout NOT Taffy, paint via Vello), **Xilem** is the reactive layer on top (view-tree-diffing à la React/SwiftUI/Elm, id-path messages, `Adapt` lensing). Apache-2.0 single license. Linebender = volunteer collective; Raph Levien informally leads; 3 named active leads. Substrate-vs-framework split: Vello + Parley adopted by Bevy 0.19 + woodpecker_ui despite Xilem/Masonry having essentially no third-party framework adoption. Closest existing-art for "unbundled Rust UI substrate"; validates Buiy's parallel-substrate strategy (foundation § 2.2) and the AccessKit-cadence-decoupled-from-framework-cadence open question (§ 2.9). Borrow: Vello capability set as render-pipeline target, Masonry decomposition shape, `masonry_testing` snapshot harness, Parley's `accesskit` text-run integration. Avoid: BoxConstraints (Buiy commits to Taffy), Xilem reactive paradigm for v1 (foundation § 2.7 keeps signals deferred), pre-1.0 substrate as dependency. `[active]`
- [GPUI](prior-art/gpui/) — Zed's GPU-accelerated UI framework; the strongest existing-art for "production app UI on a custom retained-mode GPU pipeline in Rust." Apache-2.0 single license; `gpui = 0.2.2` (2025-10-22, ~101k downloads) published from `zed-industries/zed` monorepo. Hybrid immediate+retained paradigm (`Render` trait views + `Element` trait imperative escape hatch); Taffy layout (same as Buiy); three native rendering backends (Metal on macOS, wgpu on Linux post-Blade-migration PR #46758, DirectX 11 + DirectWrite on Windows); OS-native text shaping (Core Text / DirectWrite / FreeType-adjacent — NOT cosmic-text). Production users: Zed editor (primary, ships 1.0 October 2025), Longbridge Pro (via third-party `longbridge/gpui-component` widget kit). **No AccessKit, no screen-reader support** (discussion #6576 open since 2023). **Community-deprioritized February 2026** after Sequoia-led $32M Series B (Aug 2025); `gpui-ce` community fork sparse. Validates Buiy's foundation § 2.2-§ 2.3 custom-render-pipeline bet (Zed proves it ships). Borrow: four-stage paint pipeline (layout/prepaint/paint/GPU submit), SDF-based rounded-rectangle + shadow shader math, ~8 fixed primitive types, alpha-glyph atlas with per-instance color, `UniformList`/`List` virtualized-layout escape hatches, keymap-asset + typed action + key-context dispatch. Avoid: single-product dogfooding without ecosystem commitment, Apache-only license (Buiy is dual MIT/Apache), three-backend native-API strategy (Buiy commits wgpu-uniform), defer-AccessKit, ship-no-widgets, inline-only styling, single-corporate-VC-steward governance. Consult before any spec on render pipeline (§ 2.3), virtualized lists, keymap+action dispatch (§ 3.7), or the cost-of-deferring-a11y question. `[active]`
- [Makepad](prior-art/makepad/) — standalone Rust UI framework with a custom DSL (**Live language**, `.live` syntax) above its own GPU renderer (**direct Metal / DX11 / OpenGL / WebGL — NO wgpu**). MIT OR Apache-2.0 dual permissive; `makepad-widgets = 1.0.0` (2025-05-13, only 16,974 lifetime downloads); 6,418 GitHub stars. Core team: **Rik Arends** (ex-Cloud9 IDE), **Eddy Bruël**, **Sebastian Michailidis** (a.k.a. `okapii`, top contributor). Skips winit (own per-platform event loops) which **forecloses `accesskit_winit`**. **No AccessKit, no accessibility story** — issue [#196](https://github.com/makepad/makepad/issues/196) open since 2023-08 with zero team responses; maintainer presentation quoted as "AI will soon do the heavy lifting for us" — the cleanest existing-art against which Buiy's AccessKit-first commitment is a *named* corrective. Shipping mobile via `cargo-makepad` toolchain (Robrix Matrix client by Project Robius / Kevin Boos / Futurewei runs on macOS / Linux / Windows / Android / iOS / iPadOS). Best-in-class hot-reload (including inline shader code). Validates: DSL above runtime is shippable (second proof after Slint); mobile-first Rust UI is real; GPU rendering for production app UI works. Borrow: hot-reload pattern for `.bsn`, `cargo-makepad`-style integrated cross-target tooling, per-finger touch event primitives, declarative animator state-machine, `<StackNavigation>` mobile-pattern widget. Avoid: AI-replaces-a11y framing (Buiy red line), DSL-as-primary-authoring, yet-another-DSL syntax, skipping wgpu + winit, custom text below cosmic-text, LSP locked to one IDE (Makepad Studio), 5.92%/0%/0% docs.rs coverage. Consult before any spec on DSL authoring (alongside `slint/`), mobile targeting, hot-reload, the cost-of-deferring-a11y question, or framework positioning against "AI handles accessibility." `[active]`
- *(dioxus, cosmic-text crosslink, iced, slint, egui folders exist — index entries pending)*

### Game engine UI systems

- [NoesisGUI](prior-art/noesisgui/) — commercial XAML-based UI middleware by Noesis Technologies S.L. (Madrid, ~2-9 staff, founded ~2013); native C++ runtime with bindings to Unity (2020.2+), Unreal (UE 5.7), and custom C++ engines; latest 3.2.13 released 2026-04-27. Proprietary tiered licensing (Indie €195 < €100K rev / Pro €9K / Premium €18K / Enterprise; March 2024 license restructure). GPU-tessellated vector renderer, MVVM data binding, console support (Xbox One/Series, PS4/PS5, Switch/Switch 2), Rive + Lottie + variable fonts + complex-script shaping (3.2+). **No accessibility / AccessKit story.** Verified AAA customers: Baldur's Gate 3 (Larian, uses 3.1.6), Hellblade 2 (Ninja Theory), Age of Wonders 4 (Triumph), TopSpin 2K25 (Hangar 13), Hytale (Hypixel), Cricket 24, iRacing; ~100 studios across simulation + industrial sectors. Canonical existing-art for "proprietary commercial cross-engine UI middleware shipped in AAA games." Validates: GPU-vector custom-render pipeline ships at AAA scale (foundation § 2.3); comprehensive UI library + engine bindings is a viable product surface; MVVM separation scales to BG3 complexity. Avoid: proprietary lock-in (Buiy is MIT/Apache); no a11y (Buiy is AccessKit-first); per-engine binding tax (Buiy is Bevy-only); dependency-property runtime cost; Windows-only Blend tooling chain. Borrow: Framework/Integration API conceptual split, MVVM separation pattern, per-frame tessellation w/ atlas-only caching, single-pass stereo for VR, `BackgroundEffect` backdrop-blur, template hot-reload w/ instance state preservation (3.2.11+), Rive + Lottie native embedding. Consult before any spec on render pipeline, widget catalog scope, data binding / reactivity layer (foundation § 5 open question), 3D-anchored UI, console support disposition, or framing Buiy against "what AAA studios use today." `[active]`
- [Unreal Slate + UMG](prior-art/unreal-slate-umg/) — Epic Games' dual-layer UI stack: **Slate** (~2010, declarative C++ retained-mode framework written in `SNew(SButton)...` macro chains, powers the entire Unreal Editor) plus **UMG** (Unreal Motion Graphics, UE 4.5 / November 2014, `UObject`-wrapped Blueprint-friendly designer layer with Widget Blueprint assets and the Designer/Graph/Animation tabs). Every UMG widget wraps a Slate widget (`UButton` → `SButton`). Unreal-EULA source-available, **5% royalty above $1M lifetime** (3.5% with Epic Games Store), no OSI license, copyleft-incompatible. Owns its renderer (RHI-coupled, not portable), own non-Flexbox layout (boxes/overlay/canvas/grid), HarfBuzz+ICU+FreeType text with full BiDi/CJK/IME. Accessibility is limited and late: screen-reader bridge since UE 4.22 (2019), only five widgets (`UTextBlock`/`UEditableTextBox`/`USlider`/`UButton`/`UCheckBox`); **no Linux a11y**, no AccessKit, no `:focus-visible`, no live regions, no ARIA roles. Ships at AAA scale (Fortnite, Gears 5, Senua II, Black Myth: Wukong, Stalker 2, FF VII Rebirth, virtually every UE5 title). CommonUI plugin (extracted from Fortnite, UE 4.27+) adds cross-platform input routing, controller-icon swap, cardinal nav, activatable-widget stacks — the de-facto modern UMG layer for shipped games. UMG MVVM plugin (UE 5.1+) is the change-driven data-binding replacement for the perf-trap per-frame "Bind Function" attributes. Validates: **dual-layer authoring** (programmer code + designer-friendly asset), custom-render-pipeline at AAA, reflection-driven property surface unlocks visual editors, named-slot construction DSL is sound, single-steward → architectural stability. Avoid: **C++ macro DSL** (Buiy is Rust + BSN, no `SNew`/`SLATE_*` macros); **two parallel stacks** (Buiy commits to ONE ECS + one BSN authoring layer, no `U`-wrapper over `S`-base); **proprietary/royalty licensing** (Buiy is MIT-or-Apache); **a11y-as-afterthought** (Buiy is AccessKit-first, WCAG 2.2 AA at the floor); **per-frame Blueprint tick** (Buiy exposes no per-frame poll path); **renderer locked to engine RHI** (Buiy is wgpu via Bevy render-graph). Borrow: Widget-Blueprint asset shape → `.bsn` asset DX; `FArguments` + `SLATE_NAMED_SLOT` → BSN slot syntax; `FSlateBrush` family as render-primitive checklist; `FSlateStyleSet`/`FSlateStyleRegistry` → theme-asset model; CommonUI input-routing + activatable-widget stack + cardinal navigation → Buiy focus model; `SInvalidationPanel` → cached-paint subtree primitive; `FText`-everywhere localization discipline. Consult before any spec on BSN asset authoring (foundation § 2.4), declarative widget construction syntax, dual-layer programmer/designer split, render-primitive scope, cross-platform input routing, the cost-of-deferring-a11y question, or framing Buiy against "what AAA UE titles ship." `[active]`

*(Unity UGUI, Unity UI Toolkit, Godot Control, RmlUi, Coherent Gameface, Scaleform, Flutter-in-Flame, Defold GUI — pending)*

### Substrate primitives

- [Taffy](prior-art/taffy/) — load-bearing Rust layout engine (Flexbox + Grid + Block + Float since 0.10); DioxusLabs-org, Nico Burns-maintained. MIT, bus-factor risk. Consult before any spec on layout primitives, sticky/anchor/container-queries (which Buiy implements ABOVE Taffy), or `Style` decomposition. `[active]`
- [cosmic-text](prior-art/cosmic-text/) — load-bearing Rust text engine; harfrust (since 0.15.0, NOT rustybuzz) + swash + skrifa + unicode-bidi. System76-stewarded. Bevy 0.19-dev migrated to parley+swash — post-0.19 Buiy diverges from bevy_ui on text shaper. Consult before any spec on text shaping, BiDi, editing, IME, color emoji, or font fallback. `[active]`
- [AccessKit](prior-art/accesskit/) — load-bearing cross-platform a11y bridge; Pneuma Solutions-stewarded. Windows / macOS / Linux production; Android pre-1.0; **iOS adapter shipped 2026-05-11** (Buiy spec needs update); web adapter NOT yet shipped. Buiy is the *producer*, `accesskit_consumer` is for adapter-side code. Consult before any spec on a11y tree construction, AccessKit integration, ACCNAME 1.2, focus model, or per-window adapter ownership. `[active]`

### Archived

- [bevy_cosmic_edit](prior-art/bevy-cosmic-edit/) — third-party Bevy plugin bridging cosmic-text into bevy_ui and 2D sprites. **Repo archived 2025-03-21**; final release 0.26.0 (2024-12-07, pinned to Bevy 0.15). Documented as a structural anti-pattern case study (bridge crate between two fast-moving Rust UI ecosystems). Validates Buiy's commitment to own its text-edit surface end-to-end. `[archived]`

## Reference designs

Archived design bundles (immutable inputs to specs, not specs themselves) live in [`reference-designs/`](reference-designs/) when they exist.

## Conventions

Cemented in [`specs/2026-05-07-docs-organization-design.md`](specs/2026-05-07-docs-organization-design.md). Mirrored on demand by the `organizing-buiy-docs` skill. Summary:

### Naming

| Type | Pattern | Example |
|---|---|---|
| Spec | `specs/YYYY-MM-DD-<kebab>-design.md` | `2026-05-07-docs-organization-design.md` |
| Multi-file spec | `specs/YYYY-MM-DD-<kebab>/README.md` + children | `2026-05-07-example-design/README.md` |
| Plan | `plans/YYYY-MM-DD-<kebab>.md` | `2026-05-07-example-plan.md` |
| Report | `reports/YYYY-MM-DD-<kebab>.md` | `2026-05-07-example-audit.md` |
| Prior-art | `prior-art/<system>/README.md` (no date prefix) | `prior-art/bevy-feathers/README.md` |
| Prior-art child | `prior-art/<system>/<facet>.md` | `prior-art/bevy-feathers/architecture.md` |

The date is when the doc was written, not the implementation target. The `-design.md` suffix on specs is what visually distinguishes specs from plans in `ls` output. Prior-art uses the system name (no date prefix) because the folders are living docs — track revision via git, not filename.

### Document headers

Every new spec, plan, and report opens with:

```
**Date:** YYYY-MM-DD
**Status:** draft | active | landed | superseded
**Spec:** specs/...      (plans only — REQUIRED, points at the spec being realized)
**Supersedes:** specs/... (if applicable)
```

Prior-art docs use a different header:

```
**Date:** YYYY-MM-DD       (last meaningful update — bump on revision)
**Status:** active | archived
**Subject:** <System name + one-line scope>
```

### Nested folders

Use a folder (`specs/YYYY-MM-DD-<topic>/README.md` + children) only when one logical document is too large for a single file *and* the children are tightly coupled. Children use kebab-case topic names (no date prefix — they inherit the parent's date). Maximum one level deep. Multiple independent docs sharing a topic stay flat.

### Adding a new spec, plan, or report

1. Pick the right type (spec = target, plan = migration, report = audit).
2. Name with `YYYY-MM-DD-<kebab>-design.md` (spec) or `YYYY-MM-DD-<kebab>.md` (plan/report).
3. Add a one-line entry to this README under the right area, with a 5–15 word summary and `[draft]` tag.
4. Plans must include `**Spec:** specs/...` in their header.
5. Multi-file specs nest under `YYYY-MM-DD-<topic>/` with a required `README.md`.
