**Date:** 2026-05-22
**Status:** active
**Subject:** egui — structural problems egui does not currently solve

# Open problems

These are problems egui **structurally does not solve** as of 0.34.2 (2026-05-04). Some are scope-out-by-design (egui won't ever solve them), some are scope-in-but-deferred (egui will tackle eventually), some are open in the literal sense (no one in the Rust GUI space has solved them well).

## Full ARIA APG / WCAG 2.2 AA conformance

egui ships AccessKit nodes for widgets. It does **not** ship per-widget APG keyboard contracts, accessible-name resolution per ACCNAME 1.2, full focus-management semantics (focus traps, restoration, roving tabindex), or live-region semantics.

What's missing:

- **APG keyboard patterns.** APG specifies 30+ widget patterns (combobox, listbox, menu, tabs, treeview) each with a defined keyboard contract. egui's combobox/menu/tabs implementations are approximate, not APG-compliant.
- **ARIA properties.** AccessKit supports the role+state+property model; egui populates role + a subset of states/properties, not the full ARIA 1.2 surface.
- **Live regions.** No first-class `aria-live` semantic. Developers wire AccessKit live-region nodes manually.
- **Accessible name + description resolution.** Per ACCNAME 1.2 there's a precedence chain (aria-labelledby > aria-label > text content > title); egui resolves a single string per widget.
- **WCAG 2.2 SC enforcement.** No CI gates or runtime constraints for the 50+ WCAG SCs. Color-contrast checking is not enforced.

**Implication for Buiy.** Buiy's `buiy-accessibility-design` plan to ship every widget with its APG keyboard contract + AccessKit tree shape is a substantially higher bar. egui's example shows this is not free: AccessKit-using ≠ accessible.

## Internationalization: BiDi, vertical writing, complex scripts

egui's text path uses skrifa + vello_cpu (since 0.34) for shaping. This is **better than ab_glyph** (no shaping at all → hint + variable fonts) but still well short of HarfBuzz.

Not currently supported:

- **Bidirectional text (UAX #9).** Arabic / Hebrew display works approximately, not by-spec. No paragraph-level bidi resolution.
- **Vertical writing modes.** No `writing-mode: vertical-rl`-equivalent.
- **Complex-script shaping.** Devanagari conjuncts, Indic reordering, Arabic ligatures — limited.
- **Locale-aware text.** No locale-dependent line-breaking, no locale-aware number/date formatters built in.
- **CJK text-input edge cases.** IME works (0.32-0.34 improvements) but doesn't match native OS text widget parity for things like pre-edit underline styling, candidate-window positioning, multi-character commit semantics.

**Implication for Buiy.** Buiy uses cosmic-text (HarfBuzz via rustybuzz) and inherits its complex-script support. The cost: bigger dependency, slower text path, no CPU-rasterization-style speed advantage. The benefit: by-spec text shaping for everything web-platform supports. Different tradeoff.

## Theme expressiveness

egui has `Style` + `Visuals` — flat structs with knobs for spacing, rounding, colors, fonts. It does **not** have:

- **Semantic tokens.** No "primary-color" / "danger-color" / "surface-1" token abstraction; only raw `Color32` knobs.
- **Token cascading.** No CSS-cascade-like resolution from theme → context → widget.
- **OS-preference binding.** `prefers-color-scheme` light/dark is wired (egui has `Visuals::dark` / `Visuals::light`); `prefers-contrast`, `prefers-reduced-motion`, `prefers-reduced-transparency`, `forced-colors` are not.
- **Theme variants.** Light + dark is the binary. No high-contrast variant, no per-screen-size theme variant.
- **Animation primitives.** `Context::animate_value` provides f32 interpolation per call; no declarative transition system, no keyframes.

**Implication for Buiy.** Buiy's `buiy-theme-tokens-design` sub-spec explicitly targets the gap. The egui example is instructive: shipping a theme system without semantic tokens is the bottom-up cost path; adding them later is hard.

## Touch + gamepad UX maturity

Touch input works (`PointerButton` has touch-equivalents, multi-touch is wired) but ergonomics are weak:

- No gesture-detection state machine for swipe/pinch/long-press at framework level.
- Mobile virtual-keyboard hint is rudimentary.
- Touch-target sizing (44px-minimum for WCAG 2.5.5) is not enforced.

Gamepad input is **not built in.** Game-engine consumers (bevy_egui) bridge gamepad input → keyboard input via custom translation; egui itself sees keyboard events.

**Implication for Buiy.** Foundation spec § 1 goal 6 calls out gamepad spatial nav as in-scope. egui's example: not having gamepad nav as a first-class primitive bites in game contexts.

## Performance at 10k+ widget scale

The immediate-mode rebuild cost is unavoidable past a certain widget count. Mitigations exist (multipass, rayon tessellation, galley caching, partial atlas updates) but they shave constants — they don't change the asymptotic cost. There is no "selective rebuild" mechanism, no layout-cache-across-frames, no widget-tree-incremental-update.

For Rerun (mostly 3D + thin egui chrome) this is fine. For a hypothetical 10k-row data-grid + nested-tree-view + ten panels worth of widgets, it's structurally uncomfortable.

**Implication for Buiy.** Retained-mode pays the upfront cost (component tree) and reaps the asymptotic benefit (no rebuild). For Buiy's data-grid + complex-form workloads this is the right tradeoff; for egui's 30-minute-tool workloads, the upfront cost is the wrong one.

## Multi-window context management

Multi-viewport landed in 0.24 (2023-11) and works well for Rerun's dockable-floating-panel use case. The remaining roughness:

- **Per-viewport state isolation.** Memory keyed by `Id` is global-per-`Context`; multi-viewport apps share state across viewports unless the developer is careful.
- **Cross-viewport drag-and-drop.** Limited; the OS-level drag-from-window-A-to-window-B path is not first-class.
- **Per-viewport input focus arbitration** when multiple OS windows are involved.

**Implication for Buiy.** Foundation spec § 4 lists `buiy-window-and-surface-design` as a sub-spec. egui shows that multi-window is solvable but non-trivial; budgeting it as a sub-spec rather than a paragraph is right.

## Custom render-pass integration with non-egui pipelines

egui owns its render passes. Inserting custom render content **inside** an egui frame is supported via `Painter::add` with a callback closure — but the closure runs in egui's render context, with limited access to the outer renderer's resources.

Inserting egui content **inside** a custom render pipeline is the supported direction (egui produces `ClippedPrimitive`s the host renderer consumes), but it's awkward to interleave: alternating egui-passes with custom-passes within a single frame requires careful render-graph wiring.

**Implication for Buiy.** Bevy's render graph is composable per-node; Buiy's `buiy-render-pipeline-design` sub-spec covers this. egui's example: a black-box egui-pass-as-render-graph-node works for chrome around custom content; it does not work for fine-grained interleaving.

## WASM bundle size

`eframe + egui + wgpu` WASM builds are large — multi-megabyte. Mitigations exist (the `glow` feature instead of `wgpu` shrinks bundles substantially; `opt-level = 2` in the workspace; `panic = "abort"`) but the baseline is still big compared to Yew or Leptos hello-world.

**Implication for Buiy.** Foundation spec § 5 lists WASM-target policy as an open question. egui's WASM-bundle pain is a data point: shipping comprehensive UI features will not shrink the binary.

## Animation / transition primitives

egui has:

- `Context::animate_value` — f32 interpolation, eased.
- `Context::animate_bool` — bool-to-float interpolation, eased.

It does **not** have:

- Declarative CSS-transition-like syntax.
- Keyframe animation.
- Spring physics.
- Layout transitions (FLIP-style animations when a widget moves).
- Choreographed-multi-element animations.

**Implication for Buiy.** Foundation spec § 4 lists `buiy-animation-design` as a sub-spec. The CSS-animation feature surface (transitions + keyframes + view transitions + scroll-driven animations) is real work; egui's gap is the size of the work.

## Mobile target maturity

Android works via `eframe` (game-activity or native-activity backends, per `eframe` Cargo features). iOS works via bring-your-own-embedding (no official `eframe-ios`). Both are documented as "rough."

Specific gaps:

- No first-class mobile layout primitives (responsive breakpoints, safe-area-aware layouts).
- No notch / safe-area / status-bar accommodation.
- Virtual-keyboard ergonomics weak.
- App-lifecycle (background/foreground/suspend) handling is minimal.

**Implication for Buiy.** Foundation spec § 5 lists platform-support-staging as open. The honest take: shipping mobile-first quality is several years of work in any Rust UI; egui's pragmatic "rough but workable" matches the ecosystem state of the art.

## The "Production Game UI" gap

Nobody ships a shipping-to-players game UI on egui. The reasons are concrete:

- Visual homogeneity makes branded distinctive UI expensive.
- Immediate-mode cost uncomfortable for HUDs.
- Custom-shader interleaving with widgets is awkward.
- No animation primitives → no polish-feel.
- No gamepad nav primitives.

This is not a "bug" — it's a scope statement. egui's authors don't claim production-game-UI as a target. But the gap matters to Buiy because Buiy *does* target game UI (foundation § 1 goal 6).

## egui's relationship to the Linebender stack (Parley, Vello)

egui has started absorbing Linebender stack components:

- **skrifa** (Linebender's font reading) replaced ab_glyph in 0.34 (2026-03).
- **vello_cpu** (Linebender's CPU-only Vello variant) is now egui's rasterizer.
- **parley** is on the roadmap — referenced in 0.33's "improved kerning" notes as the next step.

The interesting question: does egui eventually swap epaint's tessellator for Vello (GPU)? The hints suggest "perhaps," but no roadmap commitment exists. The Linebender stack absorption is a quiet major-architecture trend that's worth tracking — it affects Buiy's text-rendering choice (cosmic-text vs parley) indirectly.

**Implication for Buiy.** Buiy uses cosmic-text. If parley becomes the dominant Rust text-shaping crate (because egui + Xilem + others adopt it), Buiy's `buiy-text-rendering-design` sub-spec may revisit the choice. Not urgent; worth watching.

## Sources

- egui CHANGELOG (release notes + roadmap hints) — https://raw.githubusercontent.com/emilk/egui/main/CHANGELOG.md
- egui README "Goals" + "Non-goals" — https://raw.githubusercontent.com/emilk/egui/main/README.md
- AccessKit — https://accesskit.dev
- ARIA APG — https://www.w3.org/WAI/ARIA/apg/
- WCAG 2.2 — https://www.w3.org/TR/WCAG22/
- ACCNAME 1.2 — https://www.w3.org/TR/accname-1.2/
- UAX #9 (BiDi) — https://www.unicode.org/reports/tr9/
- Linebender stack — https://linebender.org
- Parley — https://github.com/linebender/parley
- Vello — https://github.com/linebender/vello
- Skrifa — https://crates.io/crates/skrifa
- bevy_egui open-problems (cross-link) — `prior-art/bevy-egui/open-problems.md`
- Buiy foundation spec — `docs/specs/2026-05-07-buiy-foundation/README.md`
