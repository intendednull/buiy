**Date:** 2026-05-22
**Status:** active
**Subject:** egui — honest critiques of scope limitations and structural costs

# Critiques

egui is **excellent at the scope it has chosen**. The critiques below are about scope itself — what egui structurally doesn't try to do, and what costs follow from that. Where third-party community framing is invoked, it is cited; where the critique is Buiy's own reading, it is labeled.

## The "looks like egui" homogeneity

egui's default theme — slate-gray panels, blue accent, Ubuntu-Light/Hack fonts, rounded 4px corners — is recognizable across the Rust ecosystem. By ~2024 the community joke had crystallized: "you can spot an egui app at 100 meters." This is the most-discussed egui critique on r/rust + HN.

Why it sticks: egui has a `Style`/`Visuals` system (see [styling-and-theming.md](styling-and-theming.md), Agent A) but it's coarse, and the cost of building a distinctive visual identity from scratch is high enough that almost nobody does it. Rerun customizes meaningfully; most other adopters don't. The result is aesthetic monoculture — every Rust crypto wallet, every Rust homelab dashboard, every Rust ML scratchpad looks vaguely like the same app.

**Implication for Buiy:** Buiy's token-based theming (`buiy_theme`) deliberately targets distinctiveness — theme variants, OS-pref binding, semantic tokens. If Buiy ships with one default-flavored "Buiy aesthetic" and lets users diverge cheaply, we avoid the egui homogeneity trap. If we don't, we'll have our own "looks like Buiy" critique by year 3.

## Performance at scale: rebuild-every-frame is real

Immediate-mode rebuilds the widget tree every frame (see [immediate-mode-deep-dive.md](immediate-mode-deep-dive.md), Agent A). The pattern is fine for ~hundreds of widgets at 60fps; it gets uncomfortable past 10k. Rerun's own engineers have documented this in their internal benchmarks (not published as a paper, but acknowledged on Discord and in PRs).

What egui has done to mitigate:

- **Multipass (0.29, 2024-09)** so a frame can settle layout without paying for it every frame's first pass.
- **`epaint` tessellator with rayon parallelism** (optional feature).
- **Galley caching** in the text-rendering pipeline.
- **Partial texture-atlas updates** (font glyphs only re-uploaded when dirty).
- **`Galley::pos_from_layout_cursor`** as a `pub` API (0.34) so high-perf consumers don't redundantly re-shape text.

What egui still can't do:

- **Layout caching across frames.** Even with multipass, the layout solver runs every frame. Buiy/retained alternatives compute layout once and re-run only when the tree changes.
- **Selective rebuild.** No mechanism to say "only rebuild the panels that need redrawing."
- **Hierarchical hit-test caching.** The hit-test pass walks the freshly-built tree every frame.

For Rerun's workload (mostly-3D-with-egui-chrome) this is fine. For a hypothetical 10k-widget data-grid app, it's not.

## Accessibility maturity: integrated but limited

AccessKit landed in 0.20 (2022-12) and went always-on in 0.34 (2026-03). Compared to "no a11y story" (where Iced and Slint were for years), this is a major win. Compared to retained-mode systems that have ARIA-tree-shape-as-first-class-data, it's still limited:

- **Tree shape is heuristic.** egui builds AccessKit nodes during end-of-frame from widget calls; the tree structure is whatever the immediate-mode code happened to produce. There is no first-class "this is a tabpanel and these are its tabs" structural relationship.
- **Live regions are not first-class.** No ARIA `live="polite"` semantic; a developer who wants live-region announcements wires AccessKit nodes manually.
- **Roving tabindex / APG keyboard patterns are not stock.** egui provides a focus system; APG widget contracts (combobox keyboard interaction, etc.) are the developer's responsibility.
- **Real-screen-reader test coverage is small.** `egui_kittest` tests AccessKit-tree-shape, not real-AT-utterance. Real-AT testing happens manually in Rerun's QA, not in CI.

**Implication for Buiy:** AccessKit-first is the right primitive, but "AccessKit-first" is a floor not a ceiling. Buiy's `buiy-widget-catalog-design` plan to ship every widget with its APG keyboard contract + AccessKit tree shape + accessible name/role/value is a substantially higher bar than egui's current floor. We should not assume that AccessKit-using = accessible; egui's example shows the gap.

## Custom widget complexity

Extending egui is **harder than the docs suggest**. The README's "Extensible: easy to write your own widgets" goal is half-true — writing a `impl Widget for Foo` for a self-contained widget is easy. Writing a widget that participates in the focus system, has its own AccessKit semantics, holds state across frames, and composes layout-correctly with other widgets is real work.

Practical pain points seen in community PRs:

- **`Id` collisions in loops.** Same widget in a loop needs `Id::new(i)` or `push_id(i)`; forgetting this is the single most common bug in community egui code.
- **State persistence is awkward.** `Memory::data` is a typed-map keyed by `Id`; cross-frame state lives there. Boilerplate per-widget.
- **Layout primitives are ad-hoc.** No first-class "min-content / max-content / fit-content" sizing — you get `desired_size` heuristics and the sizing-pass workaround.
- **Sub-widget composition** (a widget that renders + handles input for a child widget) requires understanding `InnerResponse`, `Response::union`, and the interaction-stack mechanics.

Retained alternatives (Iced, Dioxus, Buiy) front-load the conceptual cost via a component model but reward it: writing a custom widget once gives you reusable state + a stable tree shape + automatic a11y propagation.

## Touch / mobile: works but weak

Mobile support exists (Android via `eframe`'s game-activity / native-activity backends; iOS via custom embedding). The README acknowledges "still rough around the edges." Specific weaknesses:

- **Virtual keyboard ergonomics.** Mobile virtual keyboards landed in bevy_egui 0.35 (similar story upstream); still documented as rough.
- **Touch gestures.** Multi-finger gestures (pinch-zoom, two-finger scroll) are supported but the gesture-detection state machine is single-pointer-shaped.
- **Mobile-shaped layouts.** No responsive-layout primitives; the developer hand-rolls "if screen is narrow, stack vertically."
- **No iOS official `eframe` story.** Bring-your-own-embedding.

For a game-engine context (Bevy on mobile), the underlying touch-input story is fine; the egui-side ergonomics are the gap.

## Styling limits

egui's `Style` system is **flat and ad-hoc** by web-platform standards:

- No CSS-style cascade.
- No design tokens (semantic-color tokens, semantic-spacing tokens).
- No OS-preference auto-binding (`prefers-color-scheme`, `prefers-contrast`, `prefers-reduced-motion` — egui has a `dark_mode` toggle and that's about it).
- No animation primitives. No transitions. Implementations exist (e.g. `Context::animate_value` for f32 interpolation) but they're per-call, not declarative.

Cross-link: see [styling-and-theming.md](styling-and-theming.md) (Agent A) for what egui *does* offer.

## Layout limits

egui's layout is **simpler than Flexbox or Grid** by design:

- Horizontal-vertical-columns-grid primitives, not arbitrary 2D layout.
- No `flex-grow`/`flex-shrink`/`flex-basis` cascade.
- No CSS Grid (named lines, areas, auto-placement).
- No container queries.
- No anchor positioning.
- Layout is single-pass historically; multipass (0.29+) is a workaround for the cases where single-pass fails, not a redesigned layout solver.

Buiy uses Taffy for layout and gets Flexbox + (future) Grid for free; this is a deliberate scope divergence.

## Text shaping limits

egui's text path went through ab_glyph (raster only) → skrifa + vello_cpu (hinting + variable fonts, 0.34). It still does **not** have:

- **HarfBuzz-level complex-script shaping.** Arabic ligatures, Devanagari conjuncts, Indic scripts requiring re-ordering — limited support at best. (See [text-rendering.md](text-rendering.md) Agent A for the specifics.)
- **Bidirectional text (UAX #9).** Limited. Right-to-left scripts (Arabic, Hebrew) work approximately, not by-spec.
- **Vertical writing modes.** Not supported.
- **OS-IME integration parity.** IME works (and improved through 0.32-0.34) but edge cases — pre-edit composition styling, multi-character commit semantics across CJK input methods — are not at par with native OS text widgets.

Buiy uses cosmic-text (which uses HarfBuzz via `rustybuzz`) for text — different scope choice, different cost.

## The "Production Game UI" gap

**No AAA or notable indie game ships its in-game UI on egui.** The closest claim is "internal dev tools at game studios"; the actual shipped-to-players UI is on engine-native systems or other Rust UIs.

This is a real gap, not just an unmet aspiration:

- The immediate-mode rebuild cost is uncomfortable for HUDs with many widgets + animations.
- The visual homogeneity makes "branded distinctive game UI" expensive.
- Custom rendering integration (egui widgets layered with custom shaders / particle effects / 3D content) is awkward — the egui pass is a black box to the surrounding render graph.

Cross-link: [`prior-art/bevy-egui/critiques.md`](../bevy-egui/critiques.md) makes the same point about bevy_egui specifically.

## State persistence boilerplate

Cross-frame state in egui lives in `Memory::data`, a typed-map keyed by `Id`. Storing a struct across frames:

```rust
let id = ui.make_persistent_id("my_state");
let mut state: MyState = ui.data_mut(|d| d.get_persisted::<MyState>(id).unwrap_or_default());
// ... use + modify state ...
ui.data_mut(|d| d.insert_persisted(id, state));
```

Per widget that needs state. The retained alternative — fields on a component struct that the framework manages — is structurally cleaner. egui's tradeoff: state is opt-in and explicitly-scoped (no hidden retained tree), at the cost of more code per stateful widget.

## `Id` system pitfalls

The hash-based `Id` system that makes egui work also makes loops dangerous:

```rust
for item in items {
    ui.button(&item.name);  // BUG: all buttons share Id if names collide
}
// Fix:
for (i, item) in items.iter().enumerate() {
    ui.push_id(i, |ui| {
        ui.button(&item.name);
    });
}
```

The bug is silent until the user clicks one button and the wrong one "lights up." Community Discord answers this question weekly. egui has added `DebugOptions::warn_if_rect_changes_id` (0.34) as a partial mitigation, but the underlying sharp-edge is structural — it's the cost of stateless-API-with-implicit-state.

## Honest verdict

egui is the **right tool** for the workload it serves — Rust dev tools, internal dashboards, Rerun-shaped streaming-data apps. It is **the wrong tool** for: AAA in-game UI, polished consumer apps where distinctiveness matters, accessibility-critical apps where APG widget contracts are non-negotiable, complex-script i18n-critical apps, 10k+-widget data-grid apps. This is not a "criticism" so much as a scope-fit map. Buiy occupies a different cell on the same map (retained, web-platform-parity, AccessKit-first, BSN-native) — see [comparisons.md](comparisons.md).

## Sources

- egui CHANGELOG (mitigation history) — https://raw.githubusercontent.com/emilk/egui/main/CHANGELOG.md
- README "State / features" section — https://raw.githubusercontent.com/emilk/egui/main/README.md
- PR #7701 — AccessKit always-on — https://github.com/emilk/egui/pull/7701
- PR #7694 — skrifa migration — https://github.com/emilk/egui/pull/7694
- bevy_egui critiques (cross-link) — `prior-art/bevy-egui/critiques.md`
- bevy_egui open-problems (cross-link) — `prior-art/bevy-egui/open-problems.md`
- r/rust + HN "looks like egui" threads (not pinned to specific URLs; recurring community framing 2023–2026)
