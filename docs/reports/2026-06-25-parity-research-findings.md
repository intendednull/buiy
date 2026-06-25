# Widget Catalog Parity — Research Findings (Phase 2 fleet)
> Output of the `parity-research-fleet` workflow (8/8 tracks covered). Raw build-designs feeding the spec. Preserved verbatim for the FINAL phase to re-evaluate.

## theming-tokens  — complexity: large
**Summary.** Design a complete dark theme + token taxonomy for the Widget Catalog to achieve exact visual parity. Enumerate all 33 colors used in the design (surfaces #0b0c0e/#0d0e11/#16181c/#121417/#1a1d22/#1e2127; borders #1c1f24/#262a31/#2c313a/#3a4150; ink #f1f3f6/#e7eaef/#c2c8d2/#868d99/#6f7783/#555c67/#3a4049; accents #5b86f5/#45c07d/#b98aff/#f0655b and their derived --ac2/--acsoft/--acglow ramps; status colors #45c07d/#d7a23f/#f0655b; component-specific shades like #391b1a). Map to hierarchical named-token taxonomy (semantic layers: surface/border/text/accent/status + scales). Decide whether to add ColorToken::Literal(Color) variant (ergonomics vs token discipline). Design the runtime accent-swap mechanism (how --ac2 derives via lighten() at +22%, how --acsoft/--acglow are alpha variants). Cover space and radius scales.

**Design requirements (exact target values).** Design file colors (exact hexes from Widget Catalog.dc.html, lines 17-668): surfaces #0b0c0e (body bg), #0d0e11 (header/nav bg), #16181c (card bg), #121417 (input bg), #1a1d22 (menu bg), #1e2127 (icon bg); borders #1c1f24 (primary divider), #262a31 (input border), #2c313a (modal border), #3a4150 (focus ring); text ink #f1f3f6 (primary), #e7eaef (danger text), #c2c8d2 (secondary), #868d99 (tertiary), #6f7783 (quaternary), #555c67 (placeholder), #3a4049 (disabled). Accents: #5b86f5 (blue primary), #45c07d (green), #b98aff (violet), #f0655b (coral/red) each with derived --ac2 (lighten +22%), --acsoft (alpha .16), --acglow (alpha .55). Status colors: green #45c07d (success), gold #d7a23f (warning), red #f0655b (error). Component-specific: #391b1a (danger bg), #3a2422 (danger border), #07101f (text on accent). Spaces hardcoded as 4/8/12/16px gaps. Radii: 2px (sm), 6px (md), 8px (stepper/input), 9px (modal), 10px (toast), 12px (cards), 14px (modal content).

**Recommended approach.** Implement a 4-layer token taxonomy: (1) Semantic/Foundation Layer: 16 CSS system colors (Canvas, CanvasText, etc.) for forced-colors, plus core dark-mode tokens (surface primary/secondary/tertiary/interactive/menu/icon-bg, text primary/secondary/tertiary/quaternary/disabled/placeholder/danger/on-accent, borders primary/secondary/tertiary/focus/danger, accents blue/green/violet/red, status success/warning/error). (2) Computed/Derived Layer: accent ramp (--ac primary, --ac2 lighten(+22%), --acsoft rgba(.16), --acglow rgba(.55)) auto-computed at runtime on accent-change without hardcoding. (3) Scale Tokens: spaces 0-4 hardwired to 0/4/8/12/16px; radii sm/md/lg as 2/6/12px with component-local overrides (8px inputs, 9-14px modals). (4) Theme Asset Integration: default_dark_theme() populates all 30+ named tokens into a HashMap; a stub forced-colors variant maps to 16 system-color keys; runtime accent-swap mutates Res<Theme> at UserPreferences edge, re-computing ac2/acsoft/acglow before extract re-resolves all token-bearing paints via color.rs resolve_token(). Defer ColorToken::Literal variant: enforces token discipline required for forced-colors gate #11, simplifies linting, maps one-off colors (#391b1a danger-bg) to named tokens in the theme instead of literals in the component.

**Alternatives considered.**
- _Add ColorToken::Literal(Color) variant for one-off component shades_ — Breaks forced-colors gate #11(a): non-token colors fall outside system-color remapping, fail static analyzer. Buiy's constraint is enforceable: every paint color is a token reference, period. One-off shades (danger-bg #391b1a, menu-bg #1a1d22) are named tokens in the theme, not literals in components. Stricter than CSS but correct for accessibility.
- _Store accent ramp in separate AccentRamp resource_ — Adds second parallel resource to sync with Theme. Every token-bearing entity reads both at extract time. More complex, no benefit. v1: store ac2/acsoft/acglow as derived entries in same Theme.colors HashMap. Single resource, single is_changed() gate.
- _Compute accent ramp derivation in extract time, not main-world_ — Extract runs every frame. Computing ac2=lighten(primary,22%) per-frame is cheap per-instance but repeats for every accent-referencing paint (focus rings, gradients, shadows). Main-world mutation computes once per accent-change frame, stores in Theme. Cleaner: main world sees new Theme with pre-computed ramp, extract reads directly. Existing is_changed() edge costs nothing.
- _Use CSS-like cascade with inheritance_ — Deferred C-tier. v1 is flat global Theme + optional per-subtree override. Cascade is future sub-spec. Sufficient for gallery.
- _Hardcode all 33 colors + ramps into default_dark_theme()_ — Works for v1 but inflexible. v1 commits: hardwire 33 named tokens, design runtime accent-swap mechanism so ac2/acsoft/acglow are computed. Keeps space open for future theme assets (buiy-theme-tokens-design).

**Codebase integration points.**
- crates/buiy_core/src/theme.rs:22-99 — Theme resource with HashMap<String,Color> colors, spaces, radii; default_light_theme() and forced_colors_theme() stubs
- crates/buiy_core/src/render/color.rs:86-155 — resolve_token(token, theme) extract-time resolver; ColorToken enum (Transparent|Token|CurrentColor|SystemColor)
- crates/buiy_core/src/render/components.rs:17-250 — Background, Border, BoxShadow, Outline, TextColor components all carry ColorToken fields
- crates/buiy_core/src/render/extract.rs — extract_buiy_draws calls resolve_token(token, theme) for every paint instance
- crates/buiy_core/src/render/mod.rs — extract re-runs on theme.is_changed() to invalidate all token bearings on theme swap
- examples/buiy_gallery/src/lib.rs:515-524 — applyAccent(hex) computes ac2, acsoft, acglow; sets CSS custom props --ac/--ac2/--acsoft/--acglow (design file only; will port to Theme resource)

**Implementation sketch.** 1. Enumerate Design Colors into Named Tokens (theme.rs, lines 64-98): Add 30+ color entries to default_dark_theme() HashMap following the design file exactly. Surface tokens: color.surface.primary=#0b0c0e, color.surface.secondary=#0d0e11, color.surface.tertiary=#16181c, color.surface.interactive=#121417, color.surface.menu=#1a1d22, color.surface.icon-bg=#1e2127. Border tokens: color.border.primary=#1c1f24, color.border.secondary=#262a31, color.border.tertiary=#2c313a, color.border.focus=#3a4150, color.border.danger=#3a2422. Text tokens: color.text.primary=#f1f3f6, color.text.secondary=#c2c8d2, color.text.tertiary=#868d99, color.text.quaternary=#6f7783, color.text.disabled=#555c67, color.text.placeholder=#555c67, color.text.danger=#e7eaef, color.text.on-accent=#07101f. Accent tokens: color.accent.primary=#5b86f5, color.accent.secondary=#45c07d, color.accent.tertiary=#b98aff, color.accent.danger=#f0655b. Status and other: color.surface.danger=#391b1a, color.status.success=#45c07d, color.status.warning=#d7a23f, color.status.error=#f0655b, color.focus.ring=#5b86f5, color.selection.bg=#5b86f5, color.selection.fg=#f1f3f6. 2. Accent Ramp Derivation (theme.rs, new derive_accent_ramp fn): On accent-change event, compute ac2=lighten(primary, 22%), acsoft=rgba_alpha(primary, 0.16), acglow=rgba_alpha(primary, 0.55). Store as derived entries in the same Theme.colors HashMap. Extract the exact lighten formula from design file line 517: lighten = (v) => Math.min(255, Math.round(v + (255-v)*0.22)). 3. Space and Radius Scales (theme.rs, lines 89-98 pattern): Hardwire space.0 to space.4 as 0/4/8/12/16 logical pixels. Hardwire radius.sm/md/lg as 2/6/12px. Gallery-specific radii (8px stepper, 9px modal, 10px toast, 12px cards, 14px modal content) are component-local Length overrides, not theme tokens. 4. Theme Swap System (theme.rs + render/mod.rs): Leverage existing Theme::is_changed() gate in extract (color-and-forced-colors.md § 2.3). New main-world system: observe UserPreferences.accent change, mutate Res<Theme> to insert new accent tokens + recompute ramp, triggering is_changed() marker, forcing extract re-resolution of all token-bearing entities. 5. Forced-Colors Stub (theme.rs:110-150): forced_colors_theme() already carries 16 system-color keys. On forced_colors edge, swap the active Theme to this variant; accent-swap still works (the forced theme will have accent.primary mapped to system AccentColor). 6. ColorToken::Literal Decision (DEFER): Reject v1 Literal(Color) variant. Reason: (a) enforces token discipline for gate #11(a) forced-colors analyzer, (b) no color literals to track for contrast linting, (c) one-off colors (danger-bg #391b1a, menu-bg #1a1d22, icon-bg #1e2127) are named tokens in the theme, not in the component. Gallery widgets reference these tokens, not raw colors.

**Prior art.**
- CSS Custom Properties (--ac, --ac2, --acsoft, --acglow in design file lines 36-40, 216, 327, 461 etc.). Buiy's Theme resource is the Rust-side equivalent, swappable at runtime. CSS model: live variable reassignment (instant); Bevy model: resource swap + extract invalidation (frame-delayed, accepted).
- Godot Control theme() method (docs/prior-art/godot-control/theme-and-styling.md) — per-node theme inheritance. Buiy: global Theme resource + optional per-subtree override (deferred C-tier).
- Design systems (Figma tokens, Parity framework) — flat-namespace semantic tokens (color.text.primary) with variant swaps (light/dark). Buiy mirrors exactly: HashMap<String,Color> + runtime variant selection.
- Bevy_flair CSS styling (docs/prior-art/bevy-flair/critiques.md) — cascade-vs-tokens tension. Buiy chose tokens for enforced discipline, forced-colors compatibility, shader-system agnosticity.
- Vello gradients (C-tier deferred) — rasterized at atlas-bake time per atlas-and-text-seam.md; accent-swap will invalidate cached textures. Owned by that spec.
- Zed/GPUI theming — similar token model in Rust structs. Buiy's HashMap is more flexible (dynamic names, hot-reload) at cost of late binding.

**Risks.**
- Correctness: if accent-swap mutates Theme but extract does NOT re-resolve all token-bearing entities, accents will not update live. Mitigation: directly couple accent-swap to Theme::is_changed() edge. Verify: swap accent, assert next frame's extracted color is new ramp.
- Performance: accent-swap triggers global re-extract of all token-bearing entities. Bounded but non-zero cost on rare frames. Acceptable per color-and-forced-colors.md § 2.3. Future optimization: fine-grained re-resolve per entity if token taxonomy is typed (buiy-theme-tokens-design).
- Integration: design file (HTML JS line 517) hardcodes lighten() formula +22% and rgba(r,g,b,.16/.55). If Rust implementation drifts, visual parity breaks. Mitigation: copy exact formula into theme.rs, document it.
- Scope: 33 colors in flat HashMap may become unwieldy if future screens add more per-component shades. v1 accepts the namespace. Future: typed token scales (buiy-theme-tokens-design).
- ColorToken::Literal temptation: every dev will want to hardcode a color for their widget. Guard via gate #11 analyzer: static check asserts every Background/Border/Outline/BoxShadow is a Token, never literal.

**Open questions.**
- Should accent-ramp derivation (ac2=lighten +22%) live in main-world Theme mutation, or render-extract time? Current design: main-world system mutates Theme on accent-change edge, pre-computing ramp before extract re-resolves. Alternative: compute in extract per-frame. Trade-off: main-world is cleaner (one computation per rare frame), extract stays pure token->Color.
- Which colors are truly 'gallery-specific' vs core dark-mode tokens? (e.g. #1e2127 icon-bg vs #16181c card). Current decision: all are named tokens in the theme. If future screens add many more per-component shades, a small ColorToken::Literal escape-hatch might be justified (gated by verification).
- Does the accent ramp (ac2, acsoft, acglow) belong in Theme as derived entries, or computed live in render? v1 decision: stored as derived entries. Rationale: one computation per accent-change frame, not per-instance.
- The design uses linear-gradient(150deg, --ac, --ac2) in multiple places (logo, slider box, progress bar). Gradient rendering is C-tier deferred. How does accent-swap invalidate gradient textures? Flagged for buiy-theme-tokens-design and atlas-and-text-seam.md; v1 renders no gradients, so punt.

## gradients  — complexity: medium
**Summary.** Design linear-gradient (150deg, 90deg) and radial-gradient (dotted 22px grid) fills for exact Claude design parity. The design uses 4 gradient instances: logo square + slider-preview square + accent buttons all linear-gradient(150deg, --ac, --ac2); progress meter fill linear-gradient(90deg, --ac, --ac2); viewport background radial-gradient(#16181c 1px, transparent 1px) size 22px. Currently Background.color holds a single ColorToken solid fill only; C-tier reserved BackgroundLayer struct will ship as Vec<BackgroundLayer>. Implementation must resolve color tokens inside gradient stops at extract-time, compute gradients either in-shader via SDF math or pre-bake to atlas LUT texture, and handle angle/shape/stop semantics per W3C CSS Images spec.

**Design requirements (exact target values).** Linear gradients: angle 150deg (background logo, slider preview, accent buttons), angle 90deg (progress meter fill). Radial gradients: dotted circle pattern (1px solid #16181c circles on transparent, 22px x 22px repeat, viewport background). Color stops use theme tokens (--ac = primary accent, --ac2 = lighter accent variant, resolved from theme at runtime). No gradient animation/transitions in v1 (C-tier). Exact parity on gradient stops, angle rendering, and token resolution in mixed-type fills.

**Recommended approach.** Tier split: F-tier holds solid Background.color: ColorToken. C-tier (this track) adds Background.layers: Vec<BackgroundLayer> enum with Solid(ColorToken) | Linear(LinearGradient) | Radial(RadialGradient) variants. LinearGradient { angle_deg: f32, stops: Vec<ColorStop> }. RadialGradient { shape: GradientShape (Circle/Ellipse), size: RadialSize (ClosestSide/FarthestSide/etc), position: (Percent, Percent), stops }. ColorStop { color: ColorToken, position: Percent }. Extract-time resolves all color tokens in stops (via color::resolve_token, identical to Background.color path). GPU approach: compute gradients in fragment shader via SDF math (linear: perpendicular-distance-to-line * angle cosine; radial: distance-to-center). Avoids atlas baking (stores texture memory, adds async dependency). Shader emits gradient color by interpolating between resolved stop colors at the fragment's normalized position along the gradient axis. Viewport dotted-radial is a special case: use a checkerboard distance-field pattern in shader (distance-to-nearest-circle-center mod-grid math). Affine transform applies to gradient axis (rotation/scale inherit from GlobalTransform). Token resolution at extract keeps gradient data CPU-lean (instances carry linear-resolved colors, not tokens). No per-frame dynamic stops (animated gradients = E-tier future work).

**Codebase integration points.**
- crates/buiy_core/src/render/components.rs:22-28 (Background struct, reserved layers field comment)
- crates/buiy_core/src/render/color.rs:127-145 (resolve_token entry point, reused for stop resolution)
- crates/buiy_core/src/render/extract.rs:74-135 (ExtractedNode struct, color field; add ExtractedGradient parallel)
- crates/buiy_core/src/render/instance.rs:99-144 (pack_instance / pack_extracted, LinearRgba pre-linearization; add gradient packing)
- crates/buiy_core/src/render/shader.wgsl:74-91 (fragment shader, SDF-based; integrate gradient computation before final color blend)
- crates/buiy_core/src/render/pipeline.rs (SpecializedRenderPipeline trait; gradient instances reuse quad pipeline if color data fits, else new gradient-specific pipeline entry)
- crates/buiy_core/src/render/atlas/types.rs:82-117 (AtlasEntryKind enum; Gradient entry kind already reserved at byte 2)
- crates/buiy_widgets/src/scene.rs (scene helper fns; add gradient-builder helpers for logo/slider/meter/button)
- examples/buiy_gallery/src/lib.rs (5 screens; inject gradients into logo, slider-preview, progress meter, accent buttons)

**Implementation sketch.** 1. Data model: Extend components.rs Background with optional layers field (Vec<BackgroundLayer>, C-tier). Define BackgroundLayer, LinearGradient, RadialGradient, ColorStop, Percent types. 2. Extract phase: In extract.rs, produce ExtractedGradient (parallel to ExtractedNode) holding resolved linear colors + stop positions + angle/shape. For each ColorStop, call resolve_token (color.rs:127) to get Color, linearize via LinearRgba::from(), and pack into ExtractedGradient.stops. 3. Instance packing: Extend instance.rs PackedInstance to carry gradient metadata (angle_deg, stop_count, stop positions/colors packed inline or as reference). Decide: (a) Fixed-size PackedInstance + inline stops (requires stop_max to be small, e.g. 4 stops), or (b) Variable-length instance buffer + separate stop-color buffer (more complex, unbounded stops). Start with (a) for v1 (4-stop max per gradient, enough for the design's 2-stop patterns). 4. Shader: Extend shader.wgsl fragment shader to detect gradient-type instances and compute per-fragment color via SDF math (linear: abs(dot(frag - start, perpendicular)) * interp_factor; radial: distance(frag, center) scaled to [0, 1] against radius). Interpolate between resolved stop colors. For viewport radial-dotted, use a modulo-distance pattern (checkerboard distance field). 5. Prepare/pipeline: Reuse the existing quad pipeline (same instance layout, same blend). Gradient vs solid is a shader branch at runtime (no separate pipeline needed if instances unify). 6. Widget integration: Add scene.rs helpers (gradient_builder fn) to construct Background with layers for logo/slider/meter/buttons. Update gallery lib.rs to apply gradients.

**Prior art.**
- CSS Images Module Level 3 (W3C): linear-gradient() and radial-gradient() syntax, angle semantics (counter-clockwise from horizontal, y-up frame), color-stop interpolation (linear in sRGB by default, Oklch in new specs). Reference: https://www.w3.org/TR/css-images-3/
- bevy_flair (CSS to bevy_ui compiler, 0.5+): Supports linear-gradient(...) / radial-gradient(...) / conic-gradient(...) parsing (cssparser crate). Generates BackgroundGradient component (bevy_ui's representation). Relevant: cssparser color parsing, gradient value types, interpolation mode.
- Vello (Linebender GPU renderer): Uses Kurbo SDF library for vector shape rasterization; no dedicated gradient pipeline (samples via atlas or procedural shader). Relevant: SDF distance-field patterns for dotted/grid backgrounds (related to viewport radial pattern).
- GPUI (Zed's UI framework): Scene primitives include Quad with background fill. Gradients are handled per-render (Metal/DirectX/wgpu), not shown in high detail in public docs. Relevant: Quad = bounds + corner radii + background model (similar to Buiy's), shader-computed appearance.
- bevy_ui (Bevy 0.18 baseline): BackgroundColor is a solid-color-only component. No gradient support in render pipeline; would need custom plugin (not present in released 0.18). This is why Buiy must build gradient support from scratch.
- W3C CSS Backgrounds and Borders Module Level 3: background-size, background-position, background-repeat for image backgrounds. Relevant: 22px x 22px sizing semantics (in Buiy, radial-gradient size is part of the gradient spec, not a separate property).
- WebKit / Firefox / Blink gradient implementations: Linear gradients computed in fragment shaders using perpendicular-distance math; radial gradients use distance-to-center + clamping to shape bounds. Relevant: algorithm validation, performance trade-offs (compute vs bake).

**Risks.**
- Correctness: Gradient angle math must match CSS Images spec exactly (150deg is counter-clockwise from horizontal, y-up convention). Off-by-one or inverted-axis bugs will be immediately visible. Test against design mockup pixel-for-pixel.
- Performance: Gradient math in fragment shader (trig, distance, interpolation) is expensive at scale. If 1000+ gradient quads per frame appears in user code, in-shader compute becomes a bottleneck. Mitigated by: (a) measuring early, (b) atlas baking as a C-tier follow-up if needed, (c) noting that the gallery design only has ~10 gradient quads.
- Token resolution at extract: If a gradient stop's color token misses, resolve_token returns magenta + warn!. Extract happens once per frame, so a persistent miss is loud but the gradient is wrong every frame (not a silent bug, but user sees magenta). Acceptable under color-and-forced-colors.md philosophy.
- Scope creep: Temptation to add conic-gradient, repeating-linear-gradient, gradient-animation, animated stop positions. All C-tier+. Scope to 2-stop linear + radial + fixed-angle only for v1.
- Instance stride growth: Adding gradient fields to PackedInstance grows the 68-byte stride. If gradient metadata (angle, stop positions/colors) adds 32+ bytes, stride becomes 100+, reducing batch size. Mitigated by: inline fixed-size stops (4 max), or separate buffer. Decision needed early to avoid stride thrashing.

**Open questions.**
- Fixed vs variable-length gradient stops: v1 assumes 4-stop max (covers all design cases), but future dynamic gradients may need unbounded stops. Decision: store in PackedInstance inline (4 fixed stops, ~32 bytes extra per quad) vs separate GPU buffer (more complex, buys future extensibility).
- Affine transform semantics: Does a rotated gradient-filled box rotate its gradient angle with it, or stay world-axis-aligned? CSS gradient angle is relative to the box's local frame (y-up), so a 150deg gradient on a rotated element should rotate with it. Requires angle to be pre-rotated at extract-time based on GlobalTransform affine before packing.
- Viewport dotted radial as background-image vs Background fill: The design uses CSS radial-gradient on main as background-image (not background-color). Buiy background model currently does not distinguish image vs color fills. Option: (a) Treat viewport dotted as a special BackgroundLayer::Radial case (one-off in layout), or (b) Upgrade Background to carry both color and image layers uniformly. Recommend (a) for v1 (viewport is privileged; generalize later if needed).
- Token resolution in gradients under forced-colors: When forced-colors is active and a gradient stop references a brand token not in the forced-colors map, should the stop mis-resolve to magenta (like any other token) or degrade to a system-color alternative? Recommend: mis-resolve to magenta for consistency with color-and-forced-colors.md section 2.2 (single path, self-announcing).
- Performance: Computing gradients per-fragment (vs pre-baking) is faster at runtime but uses more shader ALU. Measure perf on a dense-gradient workload (e.g. 1000 gradient quads per frame) to decide if baking to atlas LUT is justified later. For now, compute in-shader (no texture dependency, lower build complexity).

## transitions-animation  — complexity: medium
**Summary.** Design a CSS-transition and keyframe-animation system. Spec requires: switch thumb left-position (3px→20px, 0.15s, cubic-bezier(0.2,0.8,0.2,1)); progress bar width (0.3s cubic-bezier); disclosure chevron rotate(90deg, 0.15s); blink indicator (1.6s infinite opacity pulse); spinner (continuous 360deg rotate); entrance keyframes (menu-in, modal-in, toast-in with opacity+translateY+scale). Animatable: Translate, Rotate, Scale, Background color, Opacity, Width. Integration: tween updates in BuiySet::Animate (after Input, before Picking), damage-gated per-frame, respects prefers-reduced-motion.

**Design requirements (exact target values).** Spec (Widget Catalog.dc.html): Switch thumb 'transition:left 0.15s cubic-bezier(0.2,0.8,0.2,1)'; Progress 'width 0.3s cubic-bezier(0.2,0.8,0.2,1)'; Disclosure chevron 'transform 0.15s'; Keyframes: menu-in (0ms: opacity 0 translateY(-6px) scale(0.98); 100ms: opacity 1 translateY(0) scale(1)); modal-in (0ms: opacity 0 translateY(8px) scale(0.985)); toast-in (0ms: opacity 0 translateY(8px)); blink (0%,55%: opacity 1; 56%,100%: opacity 0.25, 1.6s infinite); spin (360deg rotate). All cubic-bezier(0.2,0.8,0.2,1) easing. Respects UserPreferences::prefers_reduced_motion.

**Recommended approach.** Implement lightweight tween registry (not bevy_animation: game-keyframe-binding is over-weight). Components: (1) Tween<T> generic (source, target, duration, elapsed, easing_id, on_complete). (2) Easing enum (Linear, CubicBezier(f32,f32,f32,f32)) with 64-point lookup table. (3) Animatable markers: AnimateTranslate, AnimateRotate, AnimateScale, AnimateBackgroundColor, AnimateOpacity, AnimateWidth for property binding. (4) KeyframeTrack (property_id, keyframes: Vec<(time_ms, value)>, current_time, loop_count) for declarative animations. (5) Systems in BuiySet::Animate: tween_update_translate/rotate/scale/color/opacity (sample easing(t/duration), interpolate value, write to target component); keyframe_update (advance timeline, sample keyframe track, write value). (6) TransitionDescriptor component auto-spawns tweens on component change (e.g., Translate mutated → spawn Tween<Translate>). (7) Motion gate: skip all tween systems if prefers_reduced_motion=true (jump to end state). Cubic-bezier via precomputed Newton-Raphson table (64 entries) + binary search interpolation. Example: Switch gets Tween<Translate> on A11yToggled change, easing=CubicBezier(0.2,0.8,0.2,1), duration=150ms.

**Alternatives considered.**
- _Use bevy_animation AnimationPlayer with binding framework_ — Game animation system couples keyframe-track binding, graph composition, and Extract<AnimationPlayer> patterns. UI tweens are simpler (source → target interpolation with easing). AnimationPlayer adds 300+ lines of binding complexity for 4 tween types. Recommended approach is lighter and testable on CI headless.
- _Manual imperative tweening in each widget (egui style)_ — Each widget (switch, disclosure, progress) would reimplement interpolation logic. No centralized easing library. Difficult to test easing curves. CSS-like declarative model is more maintainable and matches designer expectations.
- _Add implicit TransitionDescriptor auto-detection on component change_ — Magical behavior: component mutation → tween spawn implicitly. Difficult to debug, hard to document, violates principle of least surprise. Explicit Tween spawn in widget systems is clearer: changed event → spawn tween in response system.
- _Keyframe animation with per-keyframe easing (complex)_ — CSS Animations supports per-keyframe easing via animation-timing-function in each @keyframes block. Implementation cost: keyframe easing parser + per-segment interpolation. Design spec uses single easing per animation (blink, spin, menu-in all use same curve). Ship simple (single easing) first, defer per-keyframe if design demands it.

**Codebase integration points.**
- crates/buiy_core/src/lib.rs:71-82 BuiySet enum (Animate set already defined, runs after Input)
- crates/buiy_core/src/layout/types.rs Length enum definition
- crates/buiy_core/src/layout/components.rs Translate, Rotate, Scale component definitions (Translate used in switch.rs:192-193, 263)
- crates/buiy_core/src/render/components.rs Background, Opacity component definitions
- crates/buiy_core/src/theme.rs Theme, UserPreferences (prefers_reduced_motion at line 53)
- crates/buiy_widgets/src/switch.rs:245-269 update_switch_visual writes Translate directly; will integrate with Tween spawn
- crates/buiy_widgets/src/disclosure.rs update_disclosure_visual writes Rotate; will integrate with Tween spawn
- crates/buiy_core/src/render/mod.rs:125 clip::write_clip_rects runs .after(BuiySet::Animate)
- examples/buiy_gallery/src/lib.rs widget spawning and state changes (triggers where animations fire)

**Implementation sketch.** Files to create/modify: (1) crates/buiy_core/src/animation/ (new): easing.rs (Easing enum, ease_cubic_bezier function, lookup table); tween.rs (Tween<T> component, generic tween_update system); keyframe.rs (KeyframeTrack, KeyframeAnimation components, keyframe_update system). (2) crates/buiy_core/src/lib.rs: export animation module, register Tween/KeyframeAnimation types. (3) crates/buiy_core/src/render/mod.rs or crates/buiy_core/src/lib.rs: wire tween systems into BuiySet::Animate schedule. (4) crates/buiy_widgets/src/switch.rs: modify update_switch_visual to spawn Tween<Translate> instead of direct write. (5) crates/buiy_widgets/src/disclosure.rs: spawn Tween<Rotate> on disclosure expand/collapse. (6) Scene functions: add keyframe_animation(entity, tracks) builder. Render extract damage already gates color resolution per BuiySet::Style; tween systems follow in Animate, so changed component + tween spawn triggers re-extract on first frame, tween updates trigger per-frame via Changed<Tween>. Width animation animates a Layout component (or new ProgressValue, written to Layout width). Cubic-bezier uses 64-point precomputed table sampled via binary search.

**Prior art.**
- CSS Transitions spec (W3C): property, duration, delay, easing-function (cubic-bezier defined by 4 control points, precomputed tables in browsers)
- CSS Animations spec (W3C): @keyframes, animation-duration, -timing-function. Keyframes are % or ms-based, values interpolated via easing
- bevy_animation AnimationPlayer: game-focused, uses animation graph tracks (over-weight for UI tweens)
- Slint DSL animations: duration + easing + state transitions; simpler than CSS
- egui: no built-in tweens; animations via retained state + imperative interpolation
- GPUI (Zed): AnimationContext + springs (physics-based, not easing-curve-based)
- Servo/Stylo: cubic-bezier sampling via Newton-Raphson approximation (browser standard)
- Flutter: Tween<T> + AnimationController (game-side pattern, applicable to UI-layer tweens)

**Risks.**
- Cubic-bezier lookup table precomputation on first use (lazy static) vs build-time: lazy is simpler but may have first-frame hitch. Mitigate: precompute on CorePlugin init.
- Floating-point accumulation in elapsed_time over many frames (drift). Mitigate: sample at fixed Delta intervals, clamp elapsed >= duration.
- Tween spawning in response to component change requires either TransitionDescriptor pattern or auto-detection. Tradeoff: explicit wins for debuggability; auto-magic wins for ergonomics.

**Open questions.**
- Should Tween<T> auto-cleanup on completion or require manual removal?
- Should transitions be auto-spawned (TransitionDescriptor) or always manual (explicit Tween spawn in widget systems)?
- Should prefers-reduced-motion skip tweens entirely or jump to final value instantly?
- Should keyframe animations support easing per-keyframe or single easing for whole timeline?
- What is the max simultaneous Tweens per entity before perf degrades?

## backdrop-blur  — complexity: large
**Summary.** Backdrop-filter blur samples the painted parent region before descendants render, applies a Gaussian or dual-Kawase blur shader, then composites the element on top. The compositor already reserves this as a C-tier seam: BackdropFilter component exists, EffectReason::BACKDROP_FILTER bit is defined, pooling is complete, and parent-target access is reserved. Remaining: blur shader implementation, parent-sample pass architecture, and nested-parent scratch target management.

**Design requirements (exact target values).** Design specifies two uses: viewport header blur(6px) on #0d0e11cc, modal backdrop blur(2px) on rgba(4,5,7,.66). CSS Gaussian blur requires sigma = blur-radius in pixels. Blur applies to parent region BEFORE descendants paint. Parity requirements: (1) correct Gaussian kernel, (2) linear-space compositing, (3) backdrop then foreground, (4) clip handling.

**Recommended approach.** Two-phase: (1) Blur shader: dual-Kawase (O(log r) passes) or separable Gaussian (O(r) samples). Dual-Kawase preferred - pyramid downsample/upsample with 2-3 tap passes; blur(6px) = 4 passes. Input: FilterFn::Blur(Length), resolve sigma. Output: scratch target Rgba16Float. (2) Pass insertion: Before group subtree (step 1 in compositor.rs:157), copy parent region to scratch, run blur shader on scratch, group subtree renders with blurred backdrop. Parent access: window-parent via ViewTarget::post_process_write (compositor.rs:226); nested-parent needs extra scratch target copy (known scope, compositor.rs:226-228).

**Codebase integration points.**
- crates/buiy_core/src/render/components.rs:306 - BackdropFilter component (C-tier reserved)
- crates/buiy_core/src/render/effect.rs:56,71 - backdrop filter predicate
- crates/buiy_core/src/render/compositor.rs:49 - EffectReason::BACKDROP_FILTER bit
- crates/buiy_core/src/render/compositor.rs:224-228 - seam spec and parent-target access via post_process_write
- crates/buiy_core/src/render/node.rs - BuiyNode::run step 1 (insert backdrop-filter pass before subtree rasterize)
- crates/buiy_core/src/render/prepare.rs:593 - prepare_effect_groups allocates scratch targets
- new blur.wgsl shader - dual-Kawase or separable Gaussian

**Implementation sketch.** 1. Blur shader (blur.wgsl): dual-Kawase implementation with 2-3 tap kernels per pass (4 passes for blur(6px)). Input: source texture, uniforms for blur_sigma and pass. Output: scratch target Rgba16Float. 2. Scratch target pool: extend PreparedEffectTargets with backdrop_target Vec, sized to group bounds, acquired in prepare_effect_groups. 3. Pass insertion (node.rs): Step 0.5 before group subtree rasterize - copy parent region via post_process_write (window) or copy-pass (nested), run blur shader iteratively, set blurred sample as group background. 4. Parent-region capture: window-parent uses ViewTarget::post_process_write (verified in bevy_render 0.18); nested-parent inserts copy-pass after parent finishes. 5. No new components - FilterFn::Blur already carries Length; prepare pass resolves to sigma px and packs into per-group uniform.

**Prior art.**
- Skia Gaussian blur: separable convolution SkGaussianConvolver, LUT for small kernels. Reference: src/core/SkGaussianBlur.cpp. Buiy should match their kernel math.
- Blink/WebKit: backdrop-filter lifecycle in effect property tree; blur as one filter effect.
- WebRender: separable Gaussian in shader for effects. Reference: https://github.com/servo/webrender/blob/main/webrender/src/shader_source.rs
- Dual-Kawase: Kawase 2003, GDC 2018. O(log r) passes, 2-3 tap kernels [0.5,0.25,0.25], downsample then upsample. Reference: https://github.com/Jam3/glsl-fast-gaussian-blur
- Separable Gaussian: Nystrom & Öqvist 2010 kernels for 1-20px sigma. https://learnopengl.com/Advanced-Lighting/Bloom
- CSS Filters spec: W3C Filter Effects Module, Gaussian blur defined as sigma = radius.
- GPUI: no backdrop-filter, but shows dual-Kawase cost-effective in GPU UI (not applicable to shadows).
- Vello: blur/filter effects in flux, do not depend on pre-1.0 crate.

**Risks.**
- Blur shader correctness: Gaussian kernels well-documented, but Kawase algebra less common - needs careful math review vs Skia/WebKit implementations.
- Nested-parent scope: spec defers to v1 (compositor.rs:226), but gallery might have nested modals. Design must either implement both or assert no nested backdrops in v1.
- Performance: blur pass on every backdrop group could cause hitches if unoptimized. Benchmark dual-Kawase vs Gaussian vs precomputed kernels; may need fallback.
- Color space discontinuity: if parent is sRGB and group linear, blur math must account. This is the blend-space seam compositor.rs:124 flags.
- Bevy ViewTarget API stability: post_process_write is internal to 0.18; future versions may change. Design must pin version or abstract.
- Clipping + kernel bleed: small ClipRect + blur kernel samples outside clip undefined. ClipRect applies after paint (compositor.rs:84-85), so blur may sample outside. Requires sampler clamping.
- Golden test coverage: blur artifacts artifact-prone - small kernel changes show as aliasing. Golden must be high-quality reference (Skia/Cairo), not screenshot; perceptual metrics not pixel-perfect.

**Open questions.**
- Dual-Kawase vs Gaussian: Kawase O(log r) but complex algebra; Gaussian O(r) samples single pass. For 2-6px blur both fast - choose by code clarity and wgpu patterns.
- Nested-parent copy-pass: extra pass + scratch doubles memory. May be negligible (modals usually at root) but gate on fixture count.
- Color space during blur: blur on linear Rgba16Float or sRGB parent? Linear is correct; if parent is sRGB, copy must linearize. Spec pins groups to Rgba16Float (compositor.rs:120), so parent context critical.
- Clip boundary for backdrop: should blur respect ClipRect? CSS says yes - kernel must clamp to avoid sampling outside. Implement via sampler clamping or shader bounds-check.
- Animation of blur(6px) to blur(0px): FilterFn::Blur carries Length::px or Cq*, both must resolve. Prepare already resolves ink terms (compositor.rs:89), integrate here.
- Test fixture sensitivity: blur(2px) and blur(6px) small kernels - golden image comparison sensitive to kernel differences. Verify against macOS/Safari/Chrome reference.

## icons-vector  — complexity: medium
**Summary.** Design has 25+ inline SVG stroke-based line icons (logo bars, rail glyphs, menu/stepper icons, checkmarks, chevrons, search, gear, github-fill, +/- buttons, warning triangles, toast checks) at 13-24px, stroke-width 1.7-2.4. For exact parity with accent-color tinting on theme changes, render real stroked vector paths with pixel-perfect anti-aliasing. Codebase reserves Path primitive (C-tier) and Icon atlas entries; glyph-alpha trick (alpha-as-color) enables per-instance recolor. Decision: tessellate SVG paths via lyon, rasterize to R8 coverage atlas via CPU, emit GlyphAlphaInstance-like primitives with per-instance tint matching the accent token.

**Design requirements (exact target values).** Stroke icons: 1.7-2.4px stroke-width, 13-24px viewport. Logo: linear-gradient 150deg (#5b86f5 to #6f96ff). Screen-rail icons: 17px. Menu items: 15px + icon label + kbd. Stepper +/-: 15px. Chevrons (disclosure): 16px, rotate 90deg on open. Search: 15px. Close X: 14px. Checkmark: 13px. Warning triangle: 19px. Toast check: 16px. All icons must tint via --ac accent color on inspector swatch change (live theme color changes). Anti-aliasing must match text glyph quality (integer-scale scanline sufficient for fixed 13-24px sizes).

**Recommended approach.** Implement a vector icon producer that parses SVG path syntax into tessellated polygons, rasterizes them to R8 coverage bitmaps, and inserts into the existing BuiyAtlas using the glyph-alpha (alpha-as-color) trick. Components: (1) Icon { path_d: String, stroke_width: f32, size_px: u16 } component in render/components.rs; (2) vector producer system in render/icon_producer.rs extracting icons and building AtlasKey from hash((path_d, stroke_width, scale, size)); (3) CPU rasterizer crate (buiy_icon_rasterize) using lyon for tessellation + tiny_skia/scanline for rasterization; (4) GlyphAlphaInstance emission with color = resolved accent token (enables live theme recolor without atlas mutation); (5) batching as (Glyph, page, layer) in the existing CoverageR8 pool. This reuses the proven glyph-atlas infrastructure (eviction, warmup, per-instance tinting) and requires no new GPU code (existing glyph-alpha shader handles coverage).

**Alternatives considered.**
- _Option B: Icon Font (Lucide/Feather TTF)_ — Font icons bake stroke width at compile time (Lucide ~2.0 for 24px). Design needs 1.7-2.4px — 0.3px variance is visible at 24px (misaligned with design). To support runtime stroke-width variations, font variants multiply (expensive). Custom icons (e.g., branded logo) require custom font build. Lucide is 90% coverage fast-path (2 days, MIT licensed), but exact-parity requirement blocks adoption. Recommendation: use Lucide as fallback glyph for missing icons, but don't ship Lucide as primary.
- _Option C: Pre-Rasterized PNG Sprite Sheet_ — Sprite sheets are lowest-latency: author-time rasterization, static atlas page, bind once per frame. Drawback 1: no runtime scale independence — 24px icon at zoom 2.0 is blurry (no 48px variant) or multiply sheets per scale (3-4x memory + tooling). Drawback 2: accent recolor demands RGB bitmaps (ColorRgba8 = 32 MiB/page) vs. R8 (8 MiB); pre-rasterized color doesn't compress. Drawback 3: design iteration is slow (rebuild sheet, upload binary per icon change). Drawback 4: tinting either pre-bakes N accent variants (explosion, infeasible) or per-instance multiply (poor compression vs. single-channel coverage). Use case: static icon set + performance-critical. Buiy's accent-switching (live inspector swatch) makes tinting essential → uncompetitive vs. vector rasterization.
- _Option D: SDF Vector Rendering (Signed Distance Fields)_ — SDF rasterization is GPU-accelerated (frag shader evaluates implicit distance functions). Tradeoffs vs. tessellate+rasterize CPU: (1) SDF analytically represents curves (no tessellation cost), but requires custom shader math per shape type (path, circle, rect) — less general. (2) SDF is subpixel-perfect at any zoom (scale-independent), but design has fixed icon sizes (13-24px); subpixel quality is orthogonal to integer-scale case. (3) SDF shader is 100+ LOC per shape; tessellation + scanline is 3-4 library calls. (4) SDF is hard to debug (black-box distance math); tessellation + rasterization is direct (visual output matches design). When to revisit: if perf profiling shows rasterization is frame bottleneck AND icons need scale-independence (app zoom 3x). For now: fixed-size icons at integer scales, tessellation is simpler + faster.

**Codebase integration points.**
- crates/buiy_core/src/render/buckets.rs:34 — BuiyPrimitiveKind enum reserves Path variant (paint_order 3)
- crates/buiy_core/src/render/atlas/types.rs:82-117 — AtlasEntryKind reserves Icon entry type (future; currently uses Glyph CoverageR8)
- crates/buiy_core/src/render/atlas/atlas.rs — BuiyAtlas.get_or_insert(key, CoverageR8, || rasterize_icon()) closure API
- crates/buiy_core/src/render/components.rs — render-side component model (Background, Border, etc.); Icon component added
- crates/buiy_core/src/render/extract.rs:extract_buiy_nodes — ExtractedNode assembly; icon producer runs .after(maintain_atlas)
- crates/buiy_widgets/src/{button,checkbox,disclosure,menu}.rs — widget library icon authoring
- examples/buiy_gallery/src/lib.rs — gallery screens S1-S5 replace inline SVG with Icon component spawn
- docs/specs/2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md § 4.2 — Icon/sprite primitive seam definition

**Implementation sketch.** Step 1: Icon component. Add to render/components.rs: #[derive(Component)] pub struct Icon { pub path_d: String, pub stroke_width: f32, pub size_px: u16 } — holds the SVG path syntax + stroke parameters (matching design spec). Step 2: Icon producer system. New file render/icon_producer.rs: system extract_buiy_icons runs .after(maintain_atlas) in ExtractSchedule. For each Icon entity, hash (path_d, stroke_width, window scale factor, px_size) → AtlasKey. Call atlas.get_or_insert(key, CoverageR8, || rasterize_icon(...)) with lazy closure (only rasterize on miss). Step 3: CPU rasterizer. New crate buiy_icon_rasterize: depends on lyon (tessellation) + tiny_skia or custom scanline. (a) parse path_d via lyon::path::Builder + svg path syntax (M/L/C/Q/A/Z). (b) tessellate with lyon::tessellation::StrokeTessellator { stroke_width, linecap: Round, linejoin: Round, ... }. (c) rasterize tessellated mesh to R8 bitmap via tiny_skia::RasterizeImage or custom 2D rasterizer (scan + distance-to-stroke evaluation). Return AtlasBitmap { size: UVec2, format: CoverageR8, data: Vec<u8> }. Step 4: Icon instance emission. In extract_buiy_icons (or batching phase), emit GlyphAlphaInstance { rect: screen-space box, uv: AtlasEntry.uv, color: resolve_color_token(accent_token), clip: ClipRect, page: AtlasEntry.page }. Reuse existing glyph-alpha batch bucket key (Glyph, page, layer). Step 5: Shader. Existing render/shader.wgsl glyph-alpha fragment shader: coverage = textureSample(atlas_r8, uv).r; out = color * coverage. No new GPU code. Step 6: Batching. render/buckets.rs: icon instances pack into PrimitiveBatchKey { primitive: Glyph, layer } via the existing InstanceBuckets. Icons and text glyphs share CoverageR8 pages, same bind group, single instanced draw per (page, layer). Step 7: Gallery migration. Replace examples/buiy_gallery inline SVG spawn (line 36-37, 45, 107, etc.) with Icon { path_d: \"...\", stroke_width: 1.7, size_px: 17 } component spawn on a Child entity.

**Prior art.**
- **GPUI (Zed): PolychromeSprite primitive.** Full-color sprite atlas (prior-art/gpui/gpu-rendering.md 'Glyph atlas and the alpha-as-color trick'). Icons store color directly; no per-instance recolor. Buiy reuses GPUI's alpha-as-color trick for monochrome icons (single-channel coverage + per-instance tint).
- **resvg + tiny_skia.** Zed uses resvg for SVG rendering in font subsystem. Lightweight, C-compatible Rust SVG library (Apache 2.0). Buiy can adopt resvg for path parsing if lyon is deemed heavyweight; tradeoff: fewer customization hooks for stroke semantics.
- **Lucide Icons.** ~1500 stroke-based line icons (web/React Native/Flutter). Renders as SVG in HTML, React, or native Canvas. Per-instance stroke color + stroke-width control. Design palette matches Lucide stroke philosophy (1.5-2.5px on 24px canvas). MIT license; can copy Lucide SVG paths directly if exact match needed.
- **Feather Icons.** Minimalist predecessor to Lucide, 400 icons, simpler paths (fewer curves). Used in high-perf native UIs (Flutter, iOS). Smaller asset footprint if design scope is constrained to ~100 icons.
- **egui icon handling.** egui renders SVGs via egui_extras::image::svg (wraps resvg internally). No custom atlas cache; rasterized on-demand each frame. Simpler but less batched than Buiy's approach.
- **CSS mask-image + SVG <mask>.** Web baseline for arbitrary clip-path shapes. Buiy reserves Mask entry kind (atlas-and-text-seam.md § 6, C-tier) to future-proof a similar mask-image primitive (rendered via SDF or tessellation).
- **Font icons (Material Design, FontAwesome).** Icon fonts ship glyphs at fixed size/weight per TTF. No runtime stroke customization (baked at font compile time). Rejected: design demands exact stroke widths (1.7-2.4px) at runtime; font glyphs don't support this.

**Risks.**
- **Correctness: SVG stroke semantics.** SVG paths use stroke-linecap (butt/round/square) and stroke-linejoin (miter/round/bevel/miter-clip). Design assumes round for all. If lyon::tessellation::StrokeTessellator doesn't exactly match design round-cap geometry, icons will visually misalign (off-by-1px visible at 13-24px sizes). Mitigation: hardcode StrokeOptions { linecap: Round, linejoin: Round, miter_limit: 4.0, ... } and validate rasterized output against design screenshots (pixel-diff).
- **Performance: CPU rasterization latency.** Tessellation (O(path complexity)) + rasterization (O(icon_size²)) is CPU-bound. Estimate: 24px icon ~0.5-1ms on modern CPU (i7/Ryzen). Cold app startup with 30+ icons could add 30-50ms. Warmup queue (atlas-and-text-seam.md § 2.3) defers this to pre-paint. Mitigation: measure actual latency; if >10ms total, parallelize or pre-warm.
- **Atlas memory: Icon clustering at fixed page size.** Pages are 1024x1024 (atlas § 2.2). Small icons (13px) pack 5600+ per page; large icons (24px) pack 1600+ per page. Page budget = 8 (config.page_budget, atlas § 2.4). If designers add 256px icon later, memory pressure spikes. Mitigation: measure icon size distribution + atlas growth. Document that icons >64px should use SvgImage (not Icon component).
- **Render-world seam: producer placement.** Icon producer (extract_buiy_icons) must run after maintain_atlas + layout, before glyph extraction. If icon producer runs main-world (compile style → icon), extract phase must query already-resolved entities. Seam risk: if icon producer runs after glyph producer, icons render in wrong layer. Mitigation: schedule explicitly: maintain_atlas → extract_buiy_icons → extract_buiy_glyphs (.after(extract_buiy_icons)).
- **Theme color re-tinting: Instance staleness.** Icon instances carry color = theme token at extract time. If accent token changes mid-frame, instances are stale. No stale-paint bug (atlas-as-color = recolor safe), but visual pop on swatch change is possible. Mitigation: add theme-change easing animation? Or accept pop as design intent (instant swap)?
- **SVG path syntax: Design complexity.** Full SVG path syntax (M/L/H/V/C/S/Q/T/A/Z) + arc flags is complex. Design may use non-canonical shorthands or relative coordinates. Parser robustness: if a path_d is slightly malformed, does lyon panic or return Err? Mitigation: run design SVGs through svgo (normalizer) to canonical form. Add path validation + error logging.

**Open questions.**
- Rasterization strategy: Antialiased scanline via tiny_skia vs. custom distance-to-stroke? Scanline mandatory for exact parity. Are subpixel buckets needed (like text), or integer-scale-only?
- Icon scaling: Are icons defined at a nominal design size (16px) and scaled at render, or per-usage size_px? Design has 13-24px — are variants baked or computed?
- Color vs. monochrome: Design is monochrome + accent tint. If color icons (emoji) arise, separate SvgImage component or reuse Icon with ColorRgba8 format?
- Warmup: Pre-warm common icons (screen-rail, action buttons) at app startup, or lazy-load on first use? Measure: cold atlas load time + texture memory overhead.
- Malformed paths: If path_d has invalid syntax, what UX? Silent white placeholder? Error color (red)? Debug logging?
- Accent gradient: Design logo has linear-gradient 150deg. Icon instances carry flat color. Is logo a special SvgImage, or icon with gradient-fill support deferred?

## fonts-text  — complexity: medium
**Summary.** Design requires Geist (weights 400/450/500/600/700) and Geist Mono (400/500/600) as embedded OFL fonts registered into cosmic-text's FontSystem. Expose letter-spacing via new LetterSpacing component wired to cosmic_text::Attrs.letter_spacing (range -.025em to .14em). Confirm FontWeight, line-through text-decoration, and TextAlign suffice (all exist). Enumerate the 14-stop type scale (10px to 30px across both families with varying weight patterns per usage context).

**Design requirements (exact target values).** Fonts: Geist sans-serif (weights 400, 450, 500, 600, 700) and Geist Mono monospace (400, 500, 600), both OFL-1.1 licensed from Google Fonts. Type scale: 14 discrete stops at 10px, 11px, 11.5px, 12px, 12.5px, 13px, 13.5px, 14px, 14.5px, 15px, 16px, 18px, 20px, 30px. Letter-spacing: em-relative values from -.025em to .14em (range -0.4px to +2.24px at 16px base). Line-through: color #3a4049 (muted, not text color) on completed items. Line-height: normal (1.2) sufficient; no per-element explicit values.

**Recommended approach.** PHASE 1 - Font Licensing and Registration: (1) License Geist families from Google Fonts (OFL-1.1); download TTF files. (2) Place in crates/buiy_core/assets/fonts/, optionally subset via tools/fonts/subset_default_font.sh precedent. (3) Implement BuiyFont asset and BuiyFontLoader per font-assets.md section 2 (AssetLoader for ttf/otf/ttc/otc). (4) At BuiyTextPlugin init, register both families into FontRegistry via load_font_source(Source::Binary), using declared family name as key. PHASE 2 - Letter-Spacing Component: (1) Add pub struct LetterSpacing(pub f32) in text/components.rs after TextDirection (line 295). (2) Derive Component, Reflect. (3) Add Changed LetterSpacing to TextSyncTriggers union in sync.rs (line 88). (4) Add Option LetterSpacing to SyncedText tuple (line 109). (5) Update AuthoredStyle struct to include letter_spacing: f32 field, resolved in AuthoredStyle::resolve(). (6) Wire through span_attrs() at measured width: call attrs.letter_spacing(px_value). PHASE 3 - Type Scale Registry: Document all 14 font sizes and per-size weight patterns in TextStyleDefaults or TypeScale companion resource for theme-driven selection.

**Codebase integration points.**
- crates/buiy_core/src/text/components.rs (295): Add LetterSpacing(pub f32) component with Component, Reflect derives and Default=0.0
- crates/buiy_core/src/text/sync.rs (34, 68, 88, 109): Import LetterSpacing; add Changed LetterSpacing to TextSyncTriggers union; add to SyncedText tuple; wire AuthoredStyle.letter_spacing field through resolve() and span_attrs()
- crates/buiy_core/src/text/components.rs (124, 247, 373): Confirm FontWeight, TextAlign, TextDecorations all present and sufficient
- crates/buiy_core/assets/fonts/: Add Geist weights 400/450/500/600/700 and Geist Mono 400/500/600 TTF files
- crates/buiy_core/src/text/font_system.rs (15-22, 65): Confirm DEFAULT_FONT_FAMILY pin and BuiyFallback implementation

**Implementation sketch.** LetterSpacing(f32) component as per-entity carrier like FontSize (line 109). TextSync trigger union includes Changed LetterSpacing (line 88); AuthoredStyle resolved at sync time with Option LetterSpacing member extracted like FontFamily/FontWeight. At TextCommit measure boundary, span_attrs() receives authored px value (em-to-px pre-converted) and calls cosmic_text's Attrs.letter_spacing(px). Font registration: BuiyFont assets in crates/buiy_core/assets/fonts/; at BuiyTextPlugin init, register Geist and Geist Mono into main-world FontRegistry via load_font_source(Source::Binary(Arc)), storing family name as key (font-assets.md section 3 pattern). Fallback hierarchy: named stack entries (Geist/Geist Mono) → generics (SansSerif/Monospace pinned to Geist families) → BuiyFallback default. Type scale: 14 documented stops with weight patterns per size (10px=500, 13px=450/500/600, etc.) in TextStyleDefaults or companion registry.

**Prior art.**
- CSS letter-spacing (MDN, CSS Text L4): em-relative and px values both valid; inter-glyph spacing adjustment relative to logical advance width.
- cosmic-text Attrs.letter_spacing: public f32 field (px), passed to skrifa shaping. No em-to-px conversion; caller pre-computes.
- CSS font-weight (MDN): numeric 100-900 and keywords (normal=400, bold=700); variable-font weight axis; synthetic bold (C-tier).
- Buiy FontStack resolver (font-assets.md section 5): per-codepoint fontdb Query matching, coverage span-splitting, unicode-range filtering, FontFallbackIter as implicit last resort.
- Bevy 0.18 Text (prior-art/bevy-ui/text-and-input.md): cosmic-text 0.13, no letter-spacing, FontSize only, legacy system font registration. Buiy supersedes with explicit FontSystem and per-property ComponentModel.
- egui (prior-art): simple f32 FontSize, no weight/spacing; immediate-mode.
- Zed/GPUI (prior-art): per-run font props including letter-spacing, shaped at run creation.
- Google Fonts Geist release: verify TTF availability for all weights and OFL-1.1 license.

**Risks.**
- Font subsetting stability: if undertaken, verify Geist family name consistency across subsets. fontdb matches by declared name; mismatch leaves glyphs unmatched until theme-alias seam (C-tier) lands.
- Weight 450 non-standard: if Geist lacks 450-weight TTF, round to 500 or interpolate via variable font. Design uses 450 in 6 places (input placeholder weight). Risk: medium if not available.
- Letter-spacing precision: em values (-.025em = -0.4px at 16px) may accumulate rounding error at other font sizes. Risk: low if em-to-px conversion at sync time.
- TextSync tuple growth: LetterSpacing adds to 16-member tuple (Rust trait impl limit). Next component requires refactor. Risk: medium.
- Theme letter-spacing token: font-assets.md section 9 seam not yet designed. Component-per-entity works, but no theme-swap pattern yet (like ColorToken). Risk: low (blocking only if theme-driven letter-spacing required).

**Open questions.**
- Font subsetting: subset Geist like Fira Sans (build-time complexity but smaller binary), or ship full TTF faces (simpler, larger binary)?
- Letter-spacing units: store and author in em (design), convert to px at sync time for FontSize-relativity? Or px directly? Recommend: px with em input support.
- Weight 450 availability: non-standard CSS. Verify Geist offers 450-weight TTF; fallback to 500 or variable-font interpolation if not.
- LetterSpacing absence default: use TextStyleDefaults.letter_spacing or 0.0? Precedent: FontSize defaults 16px; recommend 0.0.

## shell-architecture  — complexity: large
**Summary.** Design a unified shell architecture combining persistent 3-pane IDE-style shell (top chrome, left Screens rail with Stats, center viewport with radial-grid background, right Inspector with live-state and accent-swatches, status bar, toast) with ScreenRouter resource (enum-based screen state) and exclusive SwitchScreen message applier (despawns old subtree, spawns new with per-screen state isolation). Extend the gallery's intent-collector plus exclusive-applier pattern into composable per-screen plugins. Design reusable composites (stepper, segmented, search-input, animated meter, toast, badge, stat-row, table-row with selection and sticky header) as scene-fns, mapping each of 5 screens to existing scene structures.

**Design requirements (exact target values).** Exact parity with Widget Catalog.dc.html: Shell 100vh flex-column dark #0b0c0e overflow hidden. Top chrome 52px flex-row #0d0e11 gap 18px border-bottom 1px #1c1f24. Left rail 248px fixed #0d0e11 with Screens header (10px Geist Mono uppercase letter-spacing .14em) plus 5 nav buttons with active accent bar, icon, name, desc; flex:1 spacer; Stats footer (key-value pairs). Center viewport flex 1 flex-column with #0b0c0e radial-gradient(#16181c 1px transparent 1px) 22px size, header 42px gap 12px border-bottom backdrop-blur 6px, canvas flex:1 overflow-auto. Right Inspector 280px fixed #0d0e11 with live-state and 4 accent-swatches. Status bar 28px ready indicator plus path and version. Toast fixed bottom-44px centered. 5 screens: TodoMVC (exact parity with S1), Virtual List (1000 rows searchable with selection sticky header), Overlay Menu (5-item anchored popover with scrim and kbd hints), Modal Dialog (create/delete focus-trapped), Controls showcase (Switch, Slider, Segmented, Stepper, animated Meter, Disclosure).

**Recommended approach.** 1. ScreenRouter Resource and Message: Define enum Screen {Todo, ScrollList, Menu, Modal, Showcase} and enum SwitchScreen(Screen) message. Store current screen in ScreenRouter resource. 2. Shell Architecture: Root scene-fn (shell_root) composes 5 fixed panes (top chrome, left rail, center viewport, right inspector, status bar) with flex-column at 100vh. Each pane references named entities (#TopChrome, #ScreenRail, #Viewport, #Inspector, #StatusBar). Center viewport contains #Canvas flex-child. 3. Screen Content Container: Spawn a named #ScreenContent node inside #Canvas that holds the active screen subtree. 4. Exclusive SwitchScreen Applier: Run .after(BuiySet::Input) before A11yUpdate, reads SwitchScreen messages, despawns all children of #ScreenContent, spawns new subtree from screen_todo/screen_scroll_list/screen_overlay_menu/spawn_modal/screen_showcase based on enum, and clears the message. 5. Per-Screen Plugins: Each screen has a plugin (TodoPlugin, ScrollListPlugin, MenuPlugin, ModalPlugin, ShowcasePlugin) with systems .after(BuiySet::Input) .before(A11yUpdate) to collect intents and apply mutations. 6. Composite Scene-fns: stepper(label, value, min, max), segmented(id, options, selected), search_input(placeholder), animated_meter(label, value_pct), toast(message, duration_ms), badge(label, color_token), stat_row(key, value, color_token), table_row(columns, selected). 7. Toast Lifecycle: Toast resource with Timer; Animate-set system decrements Timer and handles fade/despawn. 8. Live Accent Swatches: Inspector pane spawns 4 swatch buttons; SetAccent message mutates Theme resource. 9. Sticky Header: scroll_list spawns #ScrollHeader marked row; Animate-set pins its Translate to scroll-top. 10. Icon Styling: SVG children inherit TextColor from parent based on button state. 11. Viewport Header: part of shell_root, driven by ScreenRouter change-detection system.

**Codebase integration points.**
- examples/buiy_gallery/src/lib.rs:198-257 screen_todomvc becomes screen-content child
- examples/buiy_gallery/src/lib.rs:309-340 TodoMvcPlugin pattern becomes template for all per-screen plugins
- examples/buiy_gallery/src/lib.rs:755-850 screen_scroll_list, screen_overlay_menu, spawn_modal, screen_showcase dispatched via ScreenRouter
- crates/buiy_widgets/src/scene.rs:1-100 widget scene-fns extended with new composites
- crates/buiy_core/src/interaction.rs:29 OnPress message pattern reused for screen-scoped button presses
- crates/buiy_core/src/lib.rs:73-82 BuiySet::Input for message collection; BuiySet::A11yUpdate for scheduling
- crates/buiy_core/src/theme.rs:18-99 Theme resource mutated on accent-swatch press
- crates/buiy_core/src/render/components.rs:227 Opacity component for toast fade transitions
- crates/buiy_core/src/render/components.rs:434 EffectGroup for viewport header backdrop-blur
- crates/buiy_core/src/layout/components.rs Position::Fixed or Translate for status bar and toast
- examples/buiy_gallery/src/lib.rs:1114-1150 spawn_modal logic generalized for S4

**Implementation sketch.** Create examples/buiy_gallery/src/shell.rs: Screen enum, ScreenRouter resource, SwitchScreen message, shell_root scene-fn, pane marker components, SwitchScreenPlugin with exclusive applier. Create examples/buiy_gallery/src/composites.rs: stepper, segmented, search_input, animated_meter, toast, badge, stat_row, table_row scene-fns; Toast resource; animate_toast system in Animate set. Refactor examples/buiy_gallery/src/lib.rs: Extract TodoPlugin, ScrollListPlugin, MenuPlugin, ShowcasePlugin to modules; each registers intent collector, exclusive applier, change-detection systems; move screen_* functions to respective modules; consolidate setup to spawn shell_root. Binary boots with BuiyPlugin + SwitchScreenPlugin + (initial screen plugin). Nav buttons dispatch SwitchScreen messages. Exclusive applier despawns #ScreenContent children and spawns new screen. Per-screen plugin systems run .after(Input) .before(A11yUpdate). Toast spawned via message, animated Animate-set, auto-despawned on timer expiry.

**Prior art.**
- Bevy 0.19 Message and exclusive systems: TodoMvcPlugin (C8 sect 3.4) pattern reified; shell-router reuses at app level
- React Router or Next.js: Screen enum mirrors route; per-screen resource mirrors local state; shell root mirrors parent context
- CSS Sticky positioning: sticky header modeled after CSS position:sticky; Translate-based implementation procedural equivalent
- Toast/Snackbar (Material Design, Chakra UI): Fixed viewport position, auto-dismiss, fade-in/out; single toast with Timer and Opacity
- Accent swatch and live theming (Figma, Framer): Swatches update CSS variables; our implementation swaps Theme token values
- Virtualized tables (React Virtualized, TanStack Table): 1000-row with ContentVisibility::Auto; we add search filter + selection + sticky header
- Focus-trapped dialogs (WAI-ARIA, Radix UI): Tab trap, Esc dismiss, focus restore; Bevy FocusPlugin and ModalFocus resource implement
- Icon fonts vs SVG (Tabler, Heroicons): Design uses SVG paths; render via SVG sprite or icon font in follow-up; paths baked into scene-fn
- Bevy ECS parent-child: Screen subtree under #ScreenContent; despawn subtree in one operation; children auto-despawn
- Bevy 0.19 system set chaining: All app logic .after(Input) .before(A11yUpdate); serializes message reads, mutations, derived updates

**Risks.**
- Despawn/spawn may cause focus-loss: Clear FocusedEntity before despawn; next screen setup sets focus explicitly
- Per-screen resource isolation may leak state: Exclusive applier resets per-screen resources to Default before spawning new screen
- Toast timer precision may drift: Use Bevy's Timer component (tick/finished methods); store in Toast entity component
- Accent-swatch and theme re-resolve may stutter: Accent only changes on button press (discrete); re-extract once per press
- Sticky header vertical overflow: Top-padding of scroll container set to header.height; sticky position redundant but cosmetic
- Screen switching during modal open leaves scrim dangling: Exclusive applier closes modal before despawning; or accept dangling scrim for rapid switching
- High plugin count (6 total) may obscure system ordering: Document in CLAUDE.md; name systems descriptively; use BuiySet conventions

**Open questions.**
- Should ScreenRouter use Resource or Component? Answer: Resource (singleton access, precedent in Filter)
- Should per-screen state live in per-screen resources or global machine? Answer: Per-screen resources (isolation, fewer bugs, easier reset)
- Should sticky header use Position::Sticky or custom Translate? Answer: Custom Translate (Bevy's sticky not implemented; Translate gives exact scroll-relative control)
- What event fires Toast? Answer: Exclusive applier stages Toast message on screen action; Toast-reader spawns entity with Timer
- How does focus-trap work in modal on S4? Answer: spawn_modal returns dialog entity; modal-plugin stores in ModalFocus resource
- Should Viewport header be part of shell_root or per-screen? Answer: Part of shell_root; content driven by ScreenRouter + per-screen metadata
- How is radial-grid background rendered? Answer: Background component with Gradient variant (future work); for now solid #0b0c0e stub

## bg-pattern-misc  — complexity: medium
**Summary.** The bg-pattern-misc track covers remaining exact-parity fidelity details: the viewport's dotted radial-grid background pattern (22px tiles of #16181c 1px dots on #0b0c0e), custom scrollbar rendering (10px track with #262a31 thumb, 3px transparent border, rounded), text-selection background color (rgba(91,134,245,.32)), focus-ring styling (existing framework token to be mapped to design values), glow/dot animations via box-shadow spread, kbd chip styling, active-screen accent left bar in navigation rail, and fine-grained spacing/radius per element.

**Design requirements (exact target values).** Viewport Background: radial-gradient(#16181c 1px, transparent 1px) at 22px x 22px tile size on #0b0c0e fill (Widget Catalog.dc.html:85). Scrollbar: track transparent; thumb #262a31, 10px, 3px transparent border, rounded (lines 19-22). Selection: ::selection background rgba(91,134,245,.32); foreground white. Blink dots: box-shadow 0 0 0 4px rgba(91,134,245,.16) + animation blink 1.6s (lines 216,450,28). Kbd chips: 500 10px Geist Mono, #555c67, padding 3-6px, border 1px #262a31, radius 5-6px, bg #121417. Active nav bar: 2.5px wide, accent-color, radius 99px, positioned left on active screen button (lines 561-562).

**Recommended approach.** Decompose into four phased sprints: (1) **Pattern Background** — implement Background.Pattern variant with shader-based radial-gradient tile sampler in shader.wgsl or cached Texture2D tiling. Add PatternBackground component wrapping tile-size + colors. Extract generates instances with uv offset. (2) **Scrollbar + Glow** — ScrollArea spawns track (transparent quad) + thumb (BoxShadow spread=4px for glow ring, animated via interim keyframes). Use Corners/Border/Background for thumb styling. Map color tokens (scroll.thumb, scroll.track) in theme. (3) **Selection + Focus** — Extract resolves color.selection.bg/fg tokens; paints SelectionVisual quad (T7) per entity/codepoint. For focus-ring, ensure Outline resolves FOCUS_RING_TOKEN and AncestorClip preserves exterior outline. Golden reftests capture pixel-perfect visuals. (4) **Kbd + Nav Bar** — Kbd chip = scene-fn(Text, Border, Background, Corners(5-6px), Padding(3-6px)); register in buiy_widgets. Nav bar = existing btnStyle.border left-side, ensure accent token + 2.5px width + 99px radius. All components integrate into gallery example screens without extra config. Gate on full coverage: gallery renders all five screens with exact-parity dotted bg, scrollbars, selection on text, blink dots, kbd chips, active nav indicator, and focus rings.

**Alternatives considered.**
- _OS-native scrollbars via Bevy's built-in scroll mechanism_ — Design requires exact-parity custom styling (10px, specific colors, 3px transparent border, rounded); OS natives cannot match. Buiy already defers C5-a scrollbar as custom UI entities, so this path aligns with existing direction.
- _Pre-baked radial-gradient sprite atlas for dotted pattern_ — High-DPI and large viewport scenarios would require multiple atlas sizes or runtime texture generation; shader-based sampling is more efficient (single small pattern + tile math in vertex/fragment).
- _Use CSS animation framework (e.g., inline @keyframes in Bevy asset) for blink/glow_ — Bevy is a Rust ECS, not a web framework; CSS is not available. Interim solution: CPU-driven keyframe interpolation in BuiyAnimation system (deferred). Verification: golden reftests capture frame-by-frame animations as pixel-perfect reference images.
- _Inline selection quads directly in glyph producer (cosmic-text layer)_ — Selection is T7 (composition/paint layer), separate from T5-T6 text shaping. Cleaner separation: extract selects codepoint extents, paints SelectionVisual quad AFTER glyph quads. Allows per-selection opacity/color override.
- _Dedicated Glow component separate from BoxShadow_ — BoxShadow already has spread field; composing glow via spread=4px + blur=0 + color reuses existing pipeline. Adding a separate Glow component would duplicate abstraction. Blink animation wraps the Glow as metadata on the entity.

**Codebase integration points.**
- crates/buiy_widgets/src/scroll_area.rs:1-66 — ScrollArea widget marker; integrates with Overflow/ScrollOffset; scrollbar rendering hooks TBD
- crates/buiy_core/src/render/components.rs:195-217 — BoxShadow struct with Shadow fields (offset_x/y, blur, spread); spread enables glow rings
- crates/buiy_core/src/render/color.rs:156-159,180-186 — SELECTION_BG_TOKEN, SELECTION_FG_TOKEN, resolve_selection_bg/fg functions
- crates/buiy_core/src/render/components.rs:42-67 — TextColor, CaretColor with ColorToken; selection visuals carry tokens
- crates/buiy_core/src/theme.rs:77-82 — default_light_theme() inserts color.focus.ring and color.selection.bg/fg tokens
- crates/buiy_core/src/text/components.rs:37-41 — Text component with Node requirement; selection rendering hooks TBD
- examples/buiy_gallery/src/lib.rs:556-567 — screen rail rendering (screens map, active state btnStyle/barStyle); accent bar is position-absolute left border
- crates/buiy_core/src/render/shader.wgsl:1-92 — rounded-rect SDF shader; pattern background via separate quad/tile primitive TBD

**Implementation sketch.** 1. **Dotted-Grid Background:** Add Background pattern variant (BackgroundKind enum with Solid | Pattern). Pattern stores tile-size (22px), dot-size (1px), dot-color (#16181c), base-color (#0b0c0e). Extract generates cached tiling texture or shader-based radial-gradient in WGSL (cheaper for large viewports). 2. **Scrollbar Rendering:** ScrollArea spawns overlay track (transparent) + thumb (#262a31, 10px) entities. Thumb uses BoxShadow spread=4px for glow ring, Background + Border (3px transparent, border-clip-content-box inset). Alternative: pure shader quad with radial SDF. 3. **Text Selection:** Extract resolves color.selection.bg/fg tokens; paints SelectionVisual quad (T7) per entity/codepoint extent. 4. **Glow Dots:** BoxShadow.spread=4px + color=rgba(91,134,245,.16) + blur=0 renders sharp ring. Blink animation via BuiyAnimation (deferred; interim: CSS keyframes in reftests). 5. **Kbd Chips:** Scene-fn composing Stack(Border, Background, Corners(5-6px), Padding(3-6px), Text). Register in buiy_widgets/src/lib.rs. 6. **Active Nav Bar:** Ensure existing btnStyle.border left-side = 2.5px width, accent-token, 99px radius when on===true. 7. **Focus Ring:** Map Outline.color to FOCUS_RING_TOKEN; ensure AncestorClip preserves exterior outline outside element clip.

**Prior art.**
- CSS radial-gradient pattern (MDN): equivalent shader or texture in Buiy
- CSS scrollbar styling (::-webkit-scrollbar with border-clip:content-box inset for thumb)
- CSS ::selection pseudo-element colors (opacity + RGBA); Bevy UI has no native selection, so Buiy renders quad in text layer
- CSS @keyframes blink/spin + cubic-bezier timing; Buiy defers keyframe system but can verify via pixel-perfect reftests
- SVG line icons (design-heavy); Buiy has none (deferred to icon track)
- Vello (GPU vector library) or wgpu compute for radial-gradient patterns
- Zed/GPUI (Rust UI) renders custom scrollbars + glow via similar Quad + BoxShadow abstraction

**Risks.**
- Dotted-grid pattern at high DPI may consume excess VRAM if cached texture; shader-based approach likely cheaper.
- Scrollbar rendering may conflict with Bevy built-in UI scroll; may require custom scroll-sync logic.
- Text selection (T7 quad) may need deeper cosmic-text integration for pixel-perfect caret/extent.
- Glow animations require CPU keyframe interpolation or GPU animation—both have trade-offs.
- Focus-ring color token mapping may conflict with forced-colors; ensure SystemColor fallback present.
- Fine-grained spacing/radius (kbd chip 3-6px padding) requires parametric scene-fns; scaling to many variants may bloat library.

**Open questions.**
- Should dotted-grid background be a tiled Texture2D asset or shader-generated pattern? (Perf implications for large viewports.)
- Does ScrollArea render OS-native scrollbars or custom Buiy scrollbars as UI entities? Current code suggests latter (deferred as C5-a).
- Is text-selection painting already T7-scoped or does it need new extraction logic?
- Should glow rings use dedicated Glow component or compose via BoxShadow spread? (BoxShadow already supports spread.)
- Is BuiyAnimation keyframe system ready for blink/spin/entrance animations or are interim golden reftests sufficient?
- Does focus-ring outline need explicit design-value mapping or is existing theme token sufficient?
