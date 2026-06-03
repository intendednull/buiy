# Buiy render — color correctness and the forced-colors contract

**Parent:** [README.md](README.md)

This file pins three things: how Buiy renders **color-correctly** (linear-light compositing, sRGB output, pre-tonemapping placement); how render **resolves theme tokens** against `Res<Theme>` at extract time and re-extracts on a theme switch; and the **forced-colors contract** (gate #11) — the two static checks that keep Buiy usable under Windows High Contrast and equivalent OS modes. It closes with the **boundary** against deferred C-tier color management, naming exactly where color-space conversion slots in without committing that work.

It owns no shaders the other children own: the typed-primitive node lives in [architecture.md](architecture.md), the off-screen compositor (where group `opacity` blends) in [effect-compositor.md](effect-compositor.md), the atlas (where gradient/mask samples will land) in [atlas-and-text-seam.md](atlas-and-text-seam.md), and the component fields in [component-model.md](component-model.md). This file is about *what color values flow through those pieces and in what space*.

Tier markers follow the foundation legend (**F** = foundation, **C** = core, **E** = extended); see [foundation/visuals.md § 3.3](../2026-05-07-buiy-foundation/visuals.md#33-visual-styling-and-rendering).

---

## 1. The color pipeline: linear-light render, sRGB output

### 1.1 The invariant

> **Buiy composites in linear light and outputs sRGB-encoded 8-bit color, pre-tonemapping, sharing the frame's color pipeline.** Colors are converted sRGB → linear on the CPU before they reach the GPU; the GPU re-encodes to sRGB on store because the render target is `Rgba8UnormSrgb`. **F**

This is already true at Phase 0 and the target state preserves it — it is a load-bearing property, not a thing to redesign. The chain, traced through the shipped code:

1. **Resolve** — a theme token resolves to a `bevy_color::Color` (the default theme stores `Color::srgb(..)` and `Color::WHITE`; see [theme.rs](../../../crates/buiy_core/src/theme.rs)). `Color` is the gamma-encoded authoring value.
2. **Convert to linear on the CPU** — `render::instance::to_instance` ([instance.rs](../../../crates/buiy_core/src/render/instance.rs)) calls `LinearRgba::from(draw.color)` and packs `[red, green, blue, alpha]` as `f32` into `InstanceData.color`. This is `bevy_color`'s `impl From<Color> for LinearRgba`, i.e. `Color::to_linear()` — sRGB gamma is removed here (per-channel `Srgba::gamma_function_inverse`), so every value crossing the GPU boundary is **linear**.
3. **Composite linear** — the SDF fragment shader ([shader.wgsl](../../../crates/buiy_core/src/render/shader.wgsl)) blends `color.rgb` against the antialiased coverage `alpha` with `BlendState::ALPHA_BLENDING` ([pipeline.rs](../../../crates/buiy_core/src/render/pipeline.rs)). Alpha blending is only physically correct in linear space; doing it on gamma-encoded values darkens edges and tints semi-transparent overlaps. Because the inputs are linear, the blend is correct.
4. **Re-encode on store** — the pipeline's single color target is `TextureFormat::Rgba8UnormSrgb`. A `*Srgb` swapchain/texture format makes the GPU apply the sRGB OETF automatically on every write. The shader emits linear; the hardware stores sRGB; no manual `pow(2.2)` ever appears in WGSL.

The CPU-side `LinearRgba` conversion is retained in the target state. When the hybrid handoff lands ([architecture.md](architecture.md), pillar 3) the *coordinate* packing moves to a view uniform, but the **color** stays pre-linearized on the CPU: it is a single multiply-free table-ish transcendental per channel done once per changed instance, and keeping it CPU-side means the instance buffer is unambiguously linear regardless of which primitive shader consumes it.

### 1.2 Pre-tonemapping placement is the color-management hook

The Buiy node sits inside `Core2d` after `Node2d::EndMainPass` and **before** `Node2d::Tonemapping` ([node.rs](../../../crates/buiy_core/src/render/node.rs); the Core2d order `EndMainPass → StartMainPassPostProcessing → Tonemapping → …` is fixed by `bevy_core_pipeline`'s `Core2dPlugin`). Buiy widgets therefore write into the **same color attachment as 2D scene content, in the same (linear, working) space, and pass through the same `TonemappingNode` and color-management the rest of the frame does.**

This is pillar 2's "(pre-tonemapping, so output participates in HDR/color management)" clause, and it is the *enabling seam* for the deferred E-tier HDR pass and C-tier wide-gamut output ([foundation/visuals.md § 3.3](../2026-05-07-buiy-foundation/visuals.md#33-visual-styling-and-rendering): "HDR rendering pass" **E**, "Color management … sRGB / display-p3 / rec2020 output" **C**). When a window advertises an HDR or wide-gamut surface, the working format upstream of tonemapping becomes an extended-range float target and Buiy's linear output flows through unchanged — Buiy does not have to learn a second color path. **Inverting this placement (painting after tonemapping) is rejected**: it would force Buiy to color-match the post-tonemap frame itself, which is exactly the complexity the pre-tonemap choice avoids (recorded in [node.rs](../../../crates/buiy_core/src/render/node.rs) module docs).

> **Constraint for every future primitive.** Quad, shadow, glyph-alpha, and path primitives ([architecture.md](architecture.md)) all target the same working-space attachment and all receive **linear** color. The atlas-sampled primitives (glyph alpha, gradient, mask) sample *coverage/alpha*, not color — their color tint arrives linear in the instance record, so atlas content needs no color-space tag in v1. The off-screen effect-group targets ([effect-compositor.md](effect-compositor.md)) are likewise allocated in the working linear space so a group's contents composite correctly before the group's `opacity` multiply.

---

## 2. Theme-token resolution at extract time

### 2.1 Resolution happens in `Extract`, against `Res<Theme>`

Render resolves color tokens to concrete `Color` values **during the extract phase**, reading the main world's `Theme` resource through `Extract<Res<Theme>>`. The Phase-0 shape (`extract_buiy_draws` in [mod.rs](../../../crates/buiy_core/src/render/mod.rs)) is the target shape; the target state only generalizes *which* components carry tokens:

```rust
// Target: Background/Border/Outline/BoxShadow carry tokens; extract resolves
// each against Res<Theme> to a packed linear instance. Phase 0's single
// Visual.background_token generalizes to this set (component-model.md).
fn extract_buiy_paint(
    mut commands: Commands,
    paints: Extract<Query<(&Background, &ResolvedLayout, &GlobalTransform, &ClipRect /* … */), With<Node>>>,
    theme: Extract<Res<Theme>>,
    windows: Extract<Query<&Window, With<PrimaryWindow>>>,
) { /* token -> Color via theme.color(token) -> LinearRgba::from(..) -> InstanceData */ }
```

`Theme::color(&str) -> Option<Color>` ([theme.rs](../../../crates/buiy_core/src/theme.rs)) is the resolver. **Why extract-time, not a main-world pre-pass:** resolution is a pure read of a layout-independent resource, it produces a value only render needs, and doing it in extract keeps it off the main-world hot path and naturally re-runs every frame the extract query fires. It does **not** violate pillar 1 (thin render consumer): pillar 1 forbids render *re-deriving geometry / paint order / stacking*; token→color is none of those — it is a leaf-value lookup with no tree traversal. (Clip geometry, by contrast, *does* traverse and so is computed in the `WriteClipRects` render-prep pass, not extract — see [clip-and-transform.md](clip-and-transform.md).)

### 2.2 Missing-token policy

On a miss, `Theme::color` returns `None`. Phase 0 substitutes a **magenta sentinel** `Color::srgb(1.0, 0.0, 1.0)` and emits a `warn!` naming the token ([mod.rs](../../../crates/buiy_core/src/render/mod.rs) `MISSING_TOKEN_FALLBACK`). The target state keeps the visible-magenta-plus-warn behavior — a missing token is an author bug that should be *loud in screenshots and logs*, never silently transparent. The Phase-0 comment's "promote to `error!` once tokens are typed" is a typed-token-taxonomy decision owned by `buiy-theme-tokens-design`, not this spec; render's contract is only "miss → sentinel + diagnostic, never panic, never silent."

The sentinel participates in the same linear pipeline as any other color (it is an ordinary `Color::srgb`), so a missing token is visible *and* correctly composited — you see solid magenta, not a magenta-tinted blend artifact.

### 2.3 A theme switch invalidates and re-extracts

Swapping the active theme (light↔dark, a brand variant, or a hot-reloaded `ThemeAsset` — gate #13) must re-resolve every token-bearing entity's color. The mechanism:

- **`Theme` is a `Resource`** ([theme.rs](../../../crates/buiy_core/src/theme.rs)). Replacing it (`commands.insert_resource(new_theme)`) or mutating it via `ResMut<Theme>` marks the resource **changed**.
- **Extract re-runs and re-reads it.** Because resolution is extract-time and reads `Theme` live, the next frame's extract sees the new resource and produces the new linear colors. There is no cached, theme-stamped instance buffer to invalidate on the render side — the `Changed<T>`-gated instance rebuild ([architecture.md](architecture.md), pillar 3) is keyed on the *paint components and layout*, and those have not changed, so a naïve gate would **miss a theme-only switch**.
- **The fix is an explicit theme-change trigger.** Extract treats `theme.is_changed()` as a global "re-resolve all token-bearing instances this frame" signal: when the `Theme` resource is changed, the per-entity `Changed<T>` short-circuit is bypassed and every token-bearing paint is re-resolved and re-packed. This mirrors the layout side, where a theme switch that alters spacing/radii tokens invalidates `BoxModel` and re-runs `SyncStyles` ([layout architecture.md § 9](../2026-05-08-buiy-layout-design/architecture.md#9-performance-contract), "Mass-mutation frame (e.g. theme switch …)"). Color tokens that feed *only* paint (not layout) are render's responsibility to invalidate, and `Theme::is_changed()` is the single edge that does it.

> **Open — token typing and the change granularity.** Whether a fine-grained "only re-resolve entities whose *referenced* tokens changed" path is worth building (vs. the all-token-bearing re-resolve above) depends on the typed-token taxonomy and is owned by `buiy-theme-tokens-design`. The coarse all-re-resolve is correct and bounded (`O(token-bearing entities)` on the rare theme-switch frame, `O(0)` steady-state); the fine-grained path is an optimization, not a correctness fix. Flagged, not silently resolved.

---

## 3. The forced-colors contract (F-tier, gate #11)

**Forced-colors** is the OS-driven high-contrast mode (Windows High Contrast, and the `forced-colors: active` media feature). When active, the OS mandates a small palette of **system colors** and the app must paint within it. Buiy surfaces the OS state as `UserPreferences.forced_colors: bool` ([theme.rs](../../../crates/buiy_core/src/theme.rs)); the foundation lists this **F** ([accessibility.md](../2026-05-07-buiy-foundation/accessibility.md), "User preferences: `forced-colors: active | none` + system color keywords. **F**"). The CSS system-color keyword set is foundation-**F** too ([visuals.md § 3.3](../2026-05-07-buiy-foundation/visuals.md#33-visual-styling-and-rendering): `Canvas`, `CanvasText`, `LinkText`, `ButtonText`, `ButtonBorder`, `GrayText`, `Highlight`, `HighlightText`, `Field`, `FieldText`, `Mark`, `MarkText`, `SelectedItem`, `SelectedItemText`, `AccentColor`, `AccentColorText`).

Gate #11 ([foundation/verification.md § CI gates](../2026-05-07-buiy-foundation/verification.md#ci-gates)) is two static + visual checks. This section explains how the render component model satisfies each.

### 3.1 Check (a): no color outside the system-color token set when forced-colors is active

**The claim:** when `forced-colors: active`, no widget in the default catalog paints a color that did not resolve from a **system-color token**. (`forced-color-adjust: none` per-element opt-out and `color-scheme` are **C** — [visuals.md § 3.3](../2026-05-07-buiy-foundation/visuals.md#33-visual-styling-and-rendering) — and out of v1's default-catalog scope; the v1 gate covers the default theme path where every paint goes through forced-colors remapping.)

**How the component model supports it.** Every render-side paint color is a **token reference resolved against `Res<Theme>`** (§ 2), never a literal `Color` baked into a widget. So forced-colors is modeled as a **theme variant**: when `forced_colors` is set, the active `Theme` is the forced-colors theme whose `colors` map keys are exactly the 16 system-color tokens, and every `Background`/`Border`/`Outline`/`BoxShadow` token a default widget references resolves into that constrained map. A widget that hard-codes a non-token color, or references a brand token absent from the forced map, fails resolution → magenta sentinel + `warn!` (§ 2.2) — i.e. the violation is *self-announcing at runtime* in addition to being caught statically.

**The static check is a token-flow analyzer** (gate #11, "token-flow analyzer"). It is a build-time / test-time pass over the default widget catalog that walks each widget's emitted `Background`/`Border`/`Outline`/`BoxShadow` and asserts the color field is a **token reference** (not a literal `Color`) and, under the forced-colors theme, that the token resolves inside the system-color key set. The analyzer is possible *only because* paint color is uniformly a token edge in the component model — there is no second, literal-color path for it to miss. This is why § 2's "tokens, not literals" rule is load-bearing for accessibility, not just theming.

### 3.2 Check (b): no shadow-only affordance

**The claim:** every focusable / state-bearing widget conveys its affordance and state with a **non-shadow** visual cue — a border, fill, or outline — because `box-shadow` is **dropped entirely** in forced-colors mode (the OS/UA removes shadows), so a widget that signals "focused" or "pressed" using only a shadow becomes indistinguishable.

**How the component model supports it.** The render component model deliberately separates the **shadow channel from the structural channels**:

| Channel | Component | Survives forced-colors | Role in the check |
|---|---|---|---|
| Shadow | `BoxShadow` | **No** (dropped in forced-colors) | Must never be a widget's *only* state cue. |
| Fill | `Background` | Yes (remapped to a system color) | A non-shadow cue. |
| Border | `Border` | Yes (remapped; `ButtonBorder`/`CanvasText`) | A non-shadow cue. |
| Focus ring | `Outline` | Yes (remapped to `Highlight`) | A non-shadow cue; painted outside the border box, never clipped (component-model.md). |

Because these are **four distinct components**, "does this widget have a non-shadow cue for state X?" is a structural query over which components a widget emits in each state — answerable statically without rendering. The gate's static half walks the catalog's per-state styling and asserts: for every focusable/state-bearing widget and every state it distinguishes (`:focus-visible`, pressed, checked, selected, …), at least one of `Background`/`Border`/`Outline` *differs from the resting state*, independent of any `BoxShadow` delta. A widget whose only inter-state difference is `BoxShadow` fails. The gate's **visual half** is the golden-image diff under the forced-colors theme (gate #11, "golden visual diff under forced-colors") — it confirms the *rendered* result, with `BoxShadow` suppressed, still shows the distinction.

> **Why `Outline` is the structural anchor for focus.** `Outline` is a **dedicated F-tier component** ([README § 3.2](README.md#32-render-owned-this-spec-introduces)) precisely so the focus indicator is never expressed as a shadow. It is painted *outside* the border box and *is not clipped by the element's own clip* (component-model.md), which keeps the focus ring at full ≥2px / ≥3:1 strength (WCAG 2.4.7 / 2.4.11 — [accessibility.md](../2026-05-07-buiy-foundation/accessibility.md)) even on a clipped/overflow-hidden element, and visible in forced-colors where it remaps to `Highlight`. This is the render-side reason the foundation can mark "Focus Visible **F**" with confidence.

### 3.3 Forced-colors suppression is a render-side rule, not per-widget code

In forced-colors mode, render **does not draw `BoxShadow` instances** for default-catalog widgets — the shadow primitive batch ([architecture.md](architecture.md)) is skipped when `UserPreferences.forced_colors` is set. This is one branch in extract, not a per-widget opt-out, so widgets cannot accidentally retain a shadow. (`forced-color-adjust: none` to *preserve* an author's shadow/color is **C** and rides the same branch later, as a per-entity override — the seam is the `forced_colors` read in extract; the override component is reserved to `buiy-theme-tokens-design`.) The structural-cue guarantee from § 3.2 is what makes unconditional shadow-suppression safe: nothing load-bearing is lost.

### 3.4 What this file owns vs. defers for gate #11

- **Owned here:** the rule that paint color is a token edge (enables check (a)); the four-channel component split and unconditional shadow-suppression (enables check (b)); the placement of forced-colors as a `Theme` variant selected by `UserPreferences.forced_colors`.
- **Deferred / cross-file:** the *contents* of the forced-colors theme (which token maps to which system color) belong to `buiy-theme-tokens-design`; the analyzer's harness wiring and the golden-image baseline belong to [verification.md](verification.md) and `buiy-verification-design`; the per-widget "which states must differ" roster belongs to `buiy-widget-catalog-design`. This file pins the *render-side mechanism that makes those checks expressible*.

---

## 4. Boundary: deferred C-tier color management

Full color management is **C-tier and explicitly deferred** ([foundation/visuals.md § 3.3](../2026-05-07-buiy-foundation/visuals.md#33-visual-styling-and-rendering); [README § 1 non-goals](README.md#non-goals): "gradients … are deferred"). v1 ships exactly the linear-light sRGB pipeline of § 1. The deferred set and **where each slots in** without re-architecting:

| Deferred feature | Tier | Where it slots in (no v1 commitment) |
|---|---|---|
| `lab()` / `lch()` / `oklab()` / `oklch()` color | C | Resolution-time only: the token resolver returns a `bevy_color::Color`, whose enum already carries `Laba`/`Lcha`/`Oklaba`/`Oklcha`/`Xyza` variants in Bevy 0.18. `LinearRgba::from(color)` already converts *any* `Color` variant to linear via Oklch as the intermediary. So an `oklch()` authored token flows through the **existing** § 1 pipeline unchanged — no render change, only a token-parser change owned by `buiy-theme-tokens-design`. |
| `color()` profiles (display-p3, rec2020, a98, prophoto, xyz) | C | Output-gamut, not blend-space. The pipeline's working space stays linear; the *target format* and a final gamut-map slot in at the §1.2 pre-tonemapping seam when the window advertises a wide-gamut surface (`buiy-window-and-surface-design` owns surface format). Buiy's linear output is gamut-agnostic until that final encode. |
| `color-mix(in <space>, …)` | C | A resolution-time function over two resolved colors in a named space; produces one `Color`. Slots in at the token resolver, upstream of § 1. No render change. |
| **Gradient / color-stop interpolation in a named space** | C | **This is the one place a *render-side* color-space conversion lands.** A gradient is rasterized into the texture atlas ([atlas-and-text-seam.md](atlas-and-text-seam.md)); interpolating stops "in oklch" vs "in srgb" is a per-texel conversion done **when the gradient texture is generated**, in the atlas-bake step — not in the per-fragment hot path. The atlas seam reserves the gradient slot; the *interpolation-space* parameter rides the gradient's descriptor. v1 bakes no gradients, so the converter is absent, but the bake step is the named insertion point. |
| Per-element `filter` / `backdrop-filter` / `mix-blend-mode` color math | C | Run inside the off-screen effect-group compositor in **linear working space** ([effect-compositor.md](effect-compositor.md)); their components ship v1 (reserved), shaders deferred. Because compositing is already linear (§ 1.1), these effects get a correct color space for free when their shaders land. |

**The non-preclusion guarantee:** every deferred item above attaches at one of three named seams — the **token resolver** (color spaces, `color-mix`), the **atlas bake step** (gradient interpolation space), or the **pre-tonemapping working-space target** (wide-gamut output, HDR). None of them requires changing the §1 linear-blend invariant, the `LinearRgba`-on-the-CPU packing, or the component model. The v1 pipeline is the *floor* of the C-tier pipeline, not a parallel path that C-tier work would have to tear out. Where any of these is genuinely undecided (e.g. whether gradient bake or a per-fragment LUT is the right gradient strategy at scale), it is an open question for `buiy-theme-tokens-design` / a future render follow-up — see [README § 5](README.md#5-open-questions); this file does not invent a resolution.

---

## 5. Verification

Tied to the gates in [foundation/verification.md](../2026-05-07-buiy-foundation/verification.md) and detailed in [verification.md](verification.md):

- **Linear-light correctness (§ 1)** — gate #2 (visual regression): a golden of two overlapping semi-transparent fills and an antialiased rounded-rect edge; a gamma-space blend regresses the overlap tint and the edge darkening, so the golden *is* the linear-blend assertion. The sRGB-target re-encode is exercised by every golden (output is read back from the `Rgba8UnormSrgb` attachment).
- **Token resolution + theme switch (§ 2)** — unit/headless (no GPU): assert `extract` produces the magenta sentinel + one `warn!` on a missing token; assert that replacing `Res<Theme>` re-resolves a fixture's instance color on the next frame (the `is_changed()` invalidation edge). Hot-reload of `ThemeAsset` is gate #13.
- **Forced-colors (§ 3, gate #11)** — check (a): the token-flow analyzer over the default catalog (build/test-time, no GPU) asserts every paint color is a system-color token reference under the forced theme. Check (b): the structural query asserts every focusable/state-bearing widget has a non-`BoxShadow` inter-state cue, plus the forced-colors golden-image diff (gate #2 harness, shadows suppressed) confirms the distinction renders.
- **C-tier boundary (§ 4)** — no gate in v1 (the features are deferred); the test that *protects the seam* is the linear-blend golden above, which fails if a future change accidentally moves blending out of linear space.
