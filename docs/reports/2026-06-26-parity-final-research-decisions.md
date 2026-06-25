# Widget Catalog Parity — FINAL (Phase 3) Research: re-decided architecture

> Output of the `parity-final-research` fleet (7/7 tracks). Re-evaluation of every prototype decision with the full picture: KEEP / REFINE / REDESIGN + the build strategy. Synthesized into the phase-3 spec + plan.

COVERED 7 / 7

####################################################################################
TRACK: render-arch
####################################################################################
BUILD STRATEGY: ADAPT the prototype code with focused refinements. The render architecture is validated: all 5 screens + shell + live accent theming work end-to-end, 1774 headless + 69 GPU tests green, every design capability (gradients, icons, backdrop-blur, stacking) is GPU-proven. The extract-query arity is the ONE decision that warrants redesign (nested from the start, not a mid-flight fix). The sibling components, instance models, and paint-skip logic are proven sound and should transfer as-is. Build strategy: (1) adopt the prototype's validated render files (extract.rs, components.rs, instance.rs, visibility.rs, gradient+icon producers, cross_root_rank in StackingContext) as the foundation; (2) redesign the extract-query dispatcher to use nested sub-queries per the refined decision (core loop + optional producer arms), keeping the single-source-of-truth `ExtractedNodes` map; (3) port the M1+M6 fix (cross_root_rank stacking) and Display::None skip (visibility.rs) into the final's layout/render boundary; (4) re-bless CI goldens on the final's lavapipe adapter (the prototype was GPU-proven on RX 6700 XT; CI uses lavapipe).

DECISIONS:
  [REFINE] Extract-query arity: monolithic vs. nested
      → Design the extract phase as a NESTED query from the start: Core loop over `(Entity, &Node, &ResolvedLayout)`, then branch to optional secondary queries for paint-bearing components (`(&Background, &Border, …)` in one arm, `&Icon` in another, `&BackdropFilter` in a third). Each ar
  [KEEP] BackgroundLayers design: sibling component vs. field
      → Retain the sibling `BackgroundLayers(Vec<BackgroundLayer>)` component design exactly as implemented. It is composable, source-compatible, and mirrors the existing visual-components pattern. Add no changes.
  [KEEP] Gradients: instance model (inline stops vs. separate buffer)
      → Retain the distinct `GradientInstance` (104B) design with inlined resolved stops. The prototype's probe proved the stride cost is acceptable, and the dedicated instance + shader is cleaner than macro-izing the quad pipeline to carry optional gradient fields. For future work: if 1
  [KEEP] Vector icons: implementation (lyon tessellation + R8 atlas)
      → Retain the lyon + R8 atlas design exactly. It proved correct (GPU proof `b3-icons.png`; 25-icon catalog, all 4 instances with 2 dedup, live re-tint on accent swap verified). The cold-start optimization (bbox SDF) is the right algorithm, not a band-aid. No changes needed.
  [KEEP] Stacking and paint order: cross_root_rank model
      → Keep the `cross_root_rank` model exactly as implemented. It is a small, focused fix to a real framework bug (M1+M6), validated by full regression testing (`render_paint_skip.rs`, `fix_m1m6_top_layer_descendants_fill_and_text_paint`). The final should inherit this as-is.
  [KEEP] Display::None paint-skip visibility honoring
      → Keep the `SkipReason::DisplayNone` implementation exactly. It is a small, correct fix that restores the spec invariant ('Display::None is never given a Taffy node'; runtime-flip edge now handled). The final should inherit this as-is.
OPEN-Q RESOLVED:
   - Should the extract query be monolithic or nested? => NESTED from the start: core (Node, ResolvedLayout) loop, then optional secondary query arms for paint components (Background/Border/BoxShadow, Icon, BackdropFil
   - Is BackgroundLayers (sibling component) the right data model? => YES—keep it. Sibling components are idiomatic to Buiy's decomposed-visual-components pattern (Border, BoxShadow, Outline all siblings). Source-compatible with 1
   - Should gradient stops be inlined in PackedInstance or separate-buffered? => INLINE in GradientInstance (104B, fixed 2-stop design). Prototype measured: separate buffer adds plumbing complexity; inline hits ~10% per-instance cost, accept
   - Should vector icons use lyon tessellation, font, or pre-rasterized sprites? => lyon + R8 atlas (prototype's choice). GPU-proven, per-instance tintable, deterministic (no font variants), exact design fidelity (1.7–2.4px stroke variance), co
   - Is cross_root_rank the right stacking model for top-layer/anchor-override? => YES. Prototype fixed M1+M6 (modal/menu descendant paint) with cross_root_rank. Small, focused fix to StackingContext; threads through painters_z. Validated by r
   - How should Display::None suppress paint for runtime-toggled subtrees? => SkipReason::DisplayNone in write_paint_skip (visibility.rs), the single paint-skip source extract reads. Mirrors write_clip_rects's existing correct pruning; re

####################################################################################
TRACK: theme-tokens (subsystem architecture re-evaluation)
####################################################################################
BUILD STRATEGY: **PHASE 3 FINAL — THEME-TOKENS BUILD STRATEGY:**

**FOUNDATION:** The prototype's theme-tokens work (A2 wave, 755ef50) is production-ready. It's 700+ lines of tested, GPU-proven code spanning theme.rs (token defs + ramp derivation) + render/color.rs (resolution) + integrate into extract gate. The FINAL phase does NOT re-derive; it *lifts* the prototype code with precision.

**BUILD APPROACH: ADAPT THE PROTOTYPE (minimal re-work, maximum re-use)**

The prototype demonstrated exact parity end-to-end (all 5 screens, live accent-swap, GPU-verified). The full picture the FINAL phase has that the prototype lacked: (a) the journal's honest "RECONSIDER" list flagging design-clarity issues (e.g., dark default ambiguity), and (b) the shell-integration findings (C1-C4) that revealed token completeness gaps (shadow tokens). The build strategy is to **validate** the prototype's decisions with the full picture, **retain** what's sound, **refine** what was deferred for clarity.

**CONCRETE BUILD STEPS:**

1. **Token Taxonomy (copy-paste, validate):**  
   Source: parity-prototype/crates/buiy_core/src/theme.rs (default_dark_theme, lines 152-279).  
   Action: Copy the complete 33-token + shadow-tokens set into parity-final/theme.rs.  
   Validation: (a) Audit every token name matches values.md §1.1 exactly (e.g., "color.text.dimmer", not "color.text.dim2"). (b) Run a test: every ColorToken::Token used in the gallery resolves to a non-None color in the theme. No magenta misses.

2. **Accent Ramp Derivation (copy-paste, pin math):**  
   Source: parity-prototype theme.rs (derive_accent_ramp, lines 109-134).  
   Action: Copy derive_accent_ramp() + seed_accent_tokens() into parity-final.  
   Validation: Unit test the four design accents (blue/green/violet/coral) → verify ac2 / acsoft / acglow match values.md §1.2 byte-exactly. The prototype's tests passed; FINAL just runs them again as a regression guard.

3. **SetAccent Message + System (copy-paste, wire into schedule):**  
   Source: parity-prototype gallery/src/shell.rs + theme.rs (SetAccent message + apply_set_accent system).  
   Action: (a) Define SetAccent(Color) message in theme.rs. (b) Implement apply_set_accent system in theme.rs or a new theme_plugin.rs. (c) Register the system in BuiyPlugin, in BuiySet::Input, after UI input. (d) The system: read SetAccent, mutate Res<Theme> to seed new accent tokens, mark Theme::is_changed().  
   Validation: Inspector wiring (the gallery's swatch-click → SetAccent flow) already exists in parity-prototype. FINAL just ensures the system runs. Live test: click swatch in running gallery, verify all accent elements (logo, buttons, rail active bar, scroll selection) re-color in real-time.

4. **Extract Integration (no-op, verify gate):**  
   Source: parity-prototype render/mod.rs (extract re-gate on theme.is_changed).  
   Action: Verify extract_buiy_nodes runs on theme.is_changed(). No new code; this gate exists. FINAL just documents the behavior: "When SetAccent mutates Theme, extract re-runs on the next frame, re-resolving all token-bearing entities."

5. **Framework Default Clarification (doc + gallery setup):**  
   Source: parity-prototype main.rs (insert_resource(default_dark_theme())).  
   Action: (a) Add inline doc to default_light_theme: "This is the framework default for general Bevy apps. Widget Catalog gallery overrides with insert_resource(default_dark_theme()) — see buiy_gallery/src/main.rs." (b) Ensure gallery's main.rs EXPLICITLY calls insert_resource(default_dark_theme()) as the first thing after app.init_default_plugins(). (c) NO env-var magic, NO hidden defaults. Explicit is better than implicit.

6. **Testing Strategy:**  
   - Headless: `cargo test --lib theme` (existing tests from prototype, run as-is).  
   - Gallery: `cargo test -p buiy_gallery` (snapshot tests of tokens used, no new test code needed — the gallery's existing layout snapshot already encodes token resolution).  
   - Live: Run `cargo run -p buiy_gallery`, open inspector, click swatch, watch colors change. Document: "If accent swap is instant + all UI recolors (logo, buttons, selection, scrollbar), the live system works."

7. **Artifact Handling (zero re-work):**  
   - No ColorToken::Literal variant added (keep token discipline).  
   - No per-subtree theme overrides (flat + global for v1).  
   - No cascade / inheritance model (deferred to C-tier buiy-theme-tokens-design).  
   - All shadow tokens included (the A2 prototype gaps are closed).  

**RISK MITIGATIONS:**

- **Token name drifts:** Audit source → final naming 1:1. Use a copy-paste approach, not manual re-typing.  
- **Accent math regression:** Unit-test the four accents vs. values.md. The prototype's tests pass; FINAL's gate must also pass.  
- **Extract gate breakage:** Verify theme.is_changed() still fires in render/mod.rs. No breaking changes in Bevy 0.19 expected (gate is internal, framework-wide).  
- **Gallery theme visibility:** Add a "Theme:" label + hex display to the inspector so users can see active accent hex in real-time (helps debug mismatches).

**ESTIMATED EFFORT:**

- Copy default_dark_theme + derive_accent_ramp + seed_accent_tokens: **30 min** (careful paste, no changes).  
- Add SetAccent message + apply_set_accent system: **1 hour** (system boilerplate, zero complexity).  
- Gallery wiring (inspector swatches → SetAccent): **already done in prototype**, just verify the gallery/src/shell.rs code ports correctly.  
- Testing: **30 min** (run existing tests, verify live).  
- Docs: **30 min** (add inline comment re: framework default, SetAccent behavior).  

**TOTAL THEME-TOKENS BUILD: ~3 hours** (a short, high-confidence wave — all code validated + GPU-proven in prototype, no re-architecture needed).

DECISIONS:
  [KEEP] (1) ColorToken::Literal escape-hatch decision
      → KEEP the token-only discipline. No ColorToken::Literal variant. Rationale: (a) the prototype demonstrated the gallery works with ~33 named tokens without one-offs; (b) gate-#11 forced-colors analyzer requires this for static verification; (c) adding a literal escape-hatch invites
  [REFINE] (2) Dark-theme default (framework vs gallery)
      → FINAL approach: (1) Keep framework default = LIGHT (default_light_theme). (2) Gallery explicitly inserts default_dark_theme() FIRST THING in app setup (no magical env-var flip). (3) Add an inline doc comment in default_light_theme: 'This is the framework default for compatibility
  [KEEP] (3) Token taxonomy + naming consistency
      → FINAL: Adopt the prototype's exact 33-token set + naming verbatim. Copy default_dark_theme() from parity-prototype/theme.rs:152+ into parity-final/theme.rs. Include: (a) all surface.* entries (app, chrome, chrome-translucent, card, inset, raised, raised-alt, danger, danger-soft, 
  [KEEP] (4) Accent-swap mechanism + SetAccent message
      → FINAL: Adopt the prototype's SetAccent mechanism verbatim. (1) Copy derive_accent_ramp() + seed_accent_tokens() from theme.rs into the final. (2) Wire SetAccent into a system in BuiySet::Input (after user interactions, before render). The system reads SetAccent messages, mutates 
  [KEEP] (5) Token resolution at extract + re-extract on theme change
      → FINAL: The framework already has the extract-gate + resolve_token infrastructure (no prototype-specific changes needed). Gallery-side: every widget's Background/Border/TextColor/BoxShadow component that carries a ColorToken must resolve it via this mechanism. No component should 
  [KEEP] (6) Dark tokens completeness + shadow tokens
      → FINAL: Include the shadow tokens in default_dark_theme. Copy the complete token set from parity-prototype's default_dark_theme (lines 152-279 + shadow entries). Verify that every token referenced in the gallery (every ColorToken::Token call) has a corresponding entry in the theme
OPEN-Q RESOLVED:
   - Should the framework default to light or dark theme? Or should the gallery opt i => Framework defaults to LIGHT (backward-compat, neutral). Gallery explicitly calls insert_resource(default_dark_theme()) at boot. This is pragmatic (matches proto
   - Is a ColorToken::Literal variant necessary for one-off colors, or can all colors => All colors are named tokens. The prototype proved >33 tokens cover the entire gallery + design. No literals needed. This enforces gate-#11 forced-colors discipl
   - Where should accent-ramp derivation (ac2=lighten +22%) happen: main-world or ext => Main-world mutation. apply_set_accent system (on SetAccent message) calls derive_accent_ramp + seed_accent_tokens to pre-compute ramp before extract re-resolves
   - Should shadow tokens be part of the core 33, or deferred as gallery-specific? => Core. The prototype found they're referenced by the gallery (showcase_card_shadow) and must be in default_dark_theme. They're not an optional scale; they're fou
   - Should the token taxonomy use a typed scales system, or continue with flat HashM => Flat HashMap for v1. Typed scales are explicitly deferred to buiy-theme-tokens-design (Phase C). The flat model is proven to work, forced-colors-compatible, and

####################################################################################
TRACK: animation
####################################################################################
BUILD STRATEGY: For animation, ADAPT the validated prototype (it's green, GPU-proven, gate-tested): (1) Reproduce crates/buiy_core/src/animation/ module (easing.rs + tween.rs + mod.rs) verbatim from prototype — no cleanup, no refactor. All 15 headless tests pass, cubic-bezier math proven to the byte, Repeat/reduced-motion semantics correct. (2) REFINE the AnimatedBackgroundColor seam: promote from opt-in widget special-case into render extract's centralized resolve_background_color path (one-line check, zero duplication). (3) Wire into BuiySet::Animate in crates/buiy_core/src/lib.rs (AnimationPlugin adds the 5 systems to the set that CorePlugin already defines). (4) Update any widget that needs to spawn tweens (e.g., switch on toggle, disclosure on open/close) to call the Tween constructors with the right easing + duration from values.md. (5) Composites (switch, disclosure, nav-buttons, progress, entrance effects) authored in the shell to spawn AnimationPlugin-driven tweens. NO rebuilding from scratch — the prototype design is tight and proven. The only creative work is in the gallery authoring (shell.rs + composites.rs) which instantiate the framework's tween primitives for each interactive element. Why pragmatic: the animation framework is not a research question anymore; it's validated infrastructure. Spend time on the shell architecture + exact parity application, not re-deriving sound easing math.

DECISIONS:
  [KEEP] Tween<T> + Easing::CubicBezier design
      → Reproduce the animation module (easing.rs + tween.rs + mod.rs) verbatim into crates/buiy_core/src/animation/. The prototype is validated: all 15 animation tests pass (mod.rs:68-338), including the critical blink ping-pong test (lines 205-258: infinite oscillation never completes)
  [REFINE] AnimatedBackgroundColor seam (opt-in vs auto-composite)
      → PROMOTE into render extract (option a): In crates/buiy_core/src/render/extract.rs or color.rs, the resolve_background_color(entity, background, theme) function should: (1) Check if entity has AnimatedBackgroundColor component; (2) If yes, use that Color directly; (3) If no, resol
  [KEEP] Looping/infinite mode for blink + pulse animations
      → Keep Repeat as-is in the final. Ensure all callsites that spawn repeating tweens use .with_repeat(Repeat::PingPong { count: None }) syntax (composites::pulse_blink prototype example at animation/mod.rs:404 shows the pattern). Document: 'PingPong snaps to `from` under reduced moti
  [KEEP] Per-property tween components (TranslateTween, OpacityTween, etc.) vs monolithic AnimationPlayer
      → Reproduce the five per-target systems and their update functions verbatim. Do not collapse into a monolithic player or trait-object registry. The per-property design is the right call for UI.
  [KEEP] Reduced-motion semantics (snap-to-end vs fade-to-end)
      → Keep reduced-motion logic exactly as-is. Ensure UserPreferences::prefers_reduced_motion is threaded through at each update system call (prototype does this at tween.rs:339-341 via `reduced_motion(prefs: Option<&UserPreferences>) -> bool`). Add a comment: 'Under reduced motion, tw
  [KEEP] Easing::DESIGN = CubicBezier(0.2, 0.8, 0.2, 1.0) + EASE fallback
      → Reproduce easing module verbatim. Ensure DESIGN is the default easing for switch-thumb transitions and progress-fill width animations (values.md § 5.1 specifies it explicitly for those two). EASE is the fallback for nav-button background and disclosure chevron.
OPEN-Q RESOLVED:
   - Is bevy_animation the right foundation for the animation system, or should Buiy  => FINAL ANSWER: Roll own, 100% confirmed. Prototype proved the lightweight Tween<T> + per-property component model outweighs bevy_animation on every dimension for
   - AnimatedBackgroundColor — should it be auto-composited into render extract, or l => FINAL ANSWER: PROMOTE into render extract's resolve_background_color path. The prototype left this vague ('Wave D decides'); full picture shows the benefit of c
   - What is the correct reduced-motion behavior for animations? => FINAL ANSWER: Jump-to-rest on first tick (SNAP, no fade). Design intent is accessibility: users with motion sensitivity do NOT want motion at all. For blinks (P
   - Should tweens auto-cleanup on completion, or require manual removal? => FINAL ANSWER: AUTO-CLEANUP. Prototype (tween.rs:361-367, Done path) removes the TranslateTween component on completion, leaving the end-state Translate componen
   - Is the per-property tween component design (TranslateTween, OpacityTween, …) the => FINAL ANSWER: Per-property design is 100% correct. Prototype proved it: (1) fully typed, zero trait objects; (2) debuggable (inspector shows exactly which prope
   - How does the design handle blink dots that oscillate forever? => FINAL ANSWER: Repeat::PingPong { count: None } with OpacityTween. Prototype (tween.rs:100-140) defines PingPong that swaps from/to each pass and never decrement
   - Which animations in the design need to be implemented, and which are deferred? => FINAL ANSWER: All widget-needed animations are FINAL (switch thumb, disclosure chevron, nav-button bg, progress fill, opacity fades, entrance translateY/scale).

####################################################################################
TRACK: text-fonts
####################################################################################
BUILD STRATEGY: 
**Adapt + refine the prototype (hybrid). The prototype's core approaches (variable fonts, monospace re-pin, entity-level decoration) are correct and proven green (1725+ headless tests + GPU proof). One critical refinement: fix the LetterSpacing px↔em bug via changing the authoring contract to em-direct, removing the division in spaced().**

**Sequence:**

1. **Fonts (parallel with any other Wave A work):** Download + embed Geist/Geist-Mono variable TTFs from Google Fonts if not already present. Copy OFL-Geist.txt to assets/fonts/. No code changes beyond the prototype's registered_fonts_db() — it already registers them. Verify monospace generic is pinned to GEIST_MONO_FAMILY.

2. **LetterSpacing fix (critical, must precede shell restyle):** 
   - Remove the px→em division in spaced() (delete the line `/ self.size.max(METRICS_FLOOR)`)
   - Update LetterSpacing component doc to say "em units; cosmic applies letter_spacing × font-size"
   - Re-audit gallery shell/screens letter-spacing values: for each `LetterSpacing(px_value)`, convert to em using `px_value / font_size` (inverse of the old formula) and author as that em value.
   - Example: H1 "todos" was `LetterSpacing(-0.75)` @ 30px, change to `LetterSpacing(-0.025)` (= -0.75 / 30).

3. **Validate:** Run the shell screenshot + layout snapshots to confirm letter-spacing renders correctly (no zero-width labels, "SCREENS" not over-spaced, H1 tracking tight per design). Headless gate must pass.

**Risk profile:** Very low. The prototype's font stack (variable + re-pinning) is battle-tested. The LetterSpacing fix is a one-line deletion + audit pass. No render system changes, no new components. The fix unblocks exact parity on headings, which the prototype shipped as a known limitation (the comment in C3 journal entry: "drop the H1 letter-spacing rather than author the em-value workaround").

**Golden artifacts to re-bless:** c1-shell.png and c3-todo.png (letter-spacing tracking visible on labels + headings). The snapshot tests will re-settle with correct tracking.


DECISIONS:
  [KEEP] Variable fonts vs static faces
      → Keep the prototype's variable-face approach: Geist-Variable and GeistMono-Variable are the source of truth. They are lighter (two files vs ten static variants), proven to work with cosmic-text's variable_weight_match, and support the design's exact weights. Download from Google F
  [REFINE] LetterSpacing px vs em contract
      → Fix the lowering contract: change the authoring model to accept em values directly. Gallery authors write LetterSpacing(-0.025) for -0.025em (NOT px). Update the component doc to say '`em` units (cosmic applies letter_spacing × font-size)' and remove the px→em division in spaced(
  [KEEP] Monospace generic pinning
      → Keep the prototype's monospace re-pin. The FINAL baseline (parity-final) still pins monospace to DEFAULT_FONT_FAMILY (Fira Sans) in font_system.rs:129. After embedding Geist Mono and registering it, update that line to `db.set_monospace_family(GEIST_MONO_FAMILY)`. This is a one-l
  [KEEP] Geist font embedding + licensing
      → Keep the prototype's download + embedding approach. The FINAL phase re-downloads Geist and Geist Mono TTFs from Google Fonts if not present locally, embeds them in assets/fonts/, and registers via include_bytes!. Verify OFL-1.1 text is present in the repo. No code changes beyond 
  [KEEP] Entity-level vs per-span styling (text decorations, emphasis)
      → Keep entity-level decoration approach. The design never uses inline emphasis, bold runs, or color changes within a single text block — every instance is uniform styling on the whole entity. The prototype's entity-level DecorationLines + DecorationLineColor is correct and sufficie
  [KEEP] Type scale and font size canonical set
      → Document the 14-point type scale in a TypeScale comment or inline registry (optional; not required for parity). Gallery authors continue to use explicit FontSize values matched to the design table (values.md § 4). No code changes; the manual authoring is already proven to work in
OPEN-Q RESOLVED:
   - Should LetterSpacing be px or em? The prototype confused the two. => Change to em. Cosmic applies letter_spacing × font-size, so the authored value is em. Gallery authors write LetterSpacing(-0.025) for -0.025em @ any size, and c
   - Is Geist-Variable a reliable choice, or should we use static per-weight faces? => Variable fonts are correct and proven. One Geist-Variable.ttf + one GeistMono-Variable.ttf cover all weights. Cosmic-text's variable_weight_match applies Attrs.
   - Should monospace generic stay pinned to Fira Sans or move to Geist Mono? => Move to Geist Mono. The design specifies Geist Mono for monospace elements. Re-pinning the generic makes FontFamily([Generic(Monospace)]) resolve to the correct
   - Do we need a formal TypeScale registry, or is manual FontSize authoring enough? => Manual is enough for parity. The gallery authors select from the 14-point design table (values.md § 4) and write explicit FontSize values. A TypeScale enum (Siz
   - Should text-decoration color be per-entity or per-span? => Per-entity. The design uses one decoration color (#3a4049) globally on all strikethrough (completed todo items). No inline emphasis variation. Entity-level Deco

####################################################################################
TRACK: shell-screens (ScreenRouter, per-screen state, composites, capture harness, viewport widths)
####################################################################################
BUILD STRATEGY: For THIS subsystem: **adapt + refine the prototype code**. The prototype's shell architecture (Display::None toggle, global resources + markers, gallery-local composites, viewport responsive widths) is proven and production-ready. NO redesign needed for core mechanisms. REFINE in 3 areas: (1) Extract the 3 reusable composites (meter, table_row, search_input) to buiy_widgets as a clean layer. (2) Create a public BuiyHeadlessPlugin or headless subset to replace the hand-rolled capture_shell plugin list. (3) Ensure the shell.rs and composites.rs from the prototype are _adapted_ (not rewritten) — copy the prototype's imperative-builder pattern, the exact parity-proven styling (all colors/fonts/spacing from values.md), the animation integration (Tween, Repeat, pulse_blink), and the 5-screen spawning. The prototype code is clean, GPU-tested on the RX 6700 XT, and headless-tested on CI. Do NOT re-derive the shell layout, the token colors, the icon rendering, or the animation code. Instead, review the prototype's decisions (documented in the journal), weigh the specific alternatives I flagged (Candidate A vs B vs C, per-screen scoping, composite promotion, headless harness, viewport widths), and commit to the final architecture with full awareness. **Summary**: take the prototype code, integrate it into the final worktree, extract the 3 composites, add the headless plugin group, run the full suite (headless + GPU), screenshot-verify all 5 screens + accent-swap + toast, and gate on human review. Expected effort: 4–6 hours (copy + integration + promote composites + create headless group + gate). No surprises — the prototype proved the approach works.

DECISIONS:
  [KEEP] ScreenRouter mechanism: Display::None toggle vs despawn/respawn vs Bevy States
      → Keep Candidate A (Display::None toggle). It is the correct call: Taffy layout prune, zero hidden-screen cost verified at 1000 rows, state isolation free, operationally simpler for imperative S3/S4 spawns (popover/dialog anchors wired once at boot, not re-run per switch). Per-scre
  [KEEP] Per-screen state isolation: global resources + markers vs per-screen scoping
      → Keep global-resources + markers pattern. The state isolation is ALREADY SOLVED by not despawning. Adding per-screen resource scopes would be wasteful re-architecture — it would require a per-screen (or per-Screen enum) resource hierarchy, query scoping on each screen plugin, and 
  [REFINE] Composites: are the 10 gallery-local composites (stepper/segmented/search/meter/toast/badge/chip/kbd/status_dot/table_row) reusable enough to promote to buiy_widgets?
      → Promote ONLY the 3 most general composites to buiy_widgets: (1) meter (generic ScaleTween-animated progress fill; already token-agnostic), (2) table_row (generic row with optional icon/selection state), (3) search_input (TextInput wrapper with leading icon, consistent styling). K
  [REDESIGN] capture_shell hand-rolled headless plugin stack (drift from BuiyPlugin) → shared headless harness
      → Create a documented headless-plugin subset in buiy_core/src/lib.rs: pub fn BuiyPlugin::headless() -> PluginGroup (or a new BuiyHeadlessPlugin). Include: theme, layout, core, text, focus, a11y, widgets, render (all the data-side plugins). Exclude: picking, winit, input. BuiyPlugin
  [KEEP] Viewport widths: 880px grid (showcase target) vs ~752px actual (rail 248 + inspector 280 = 528px chrome, 1280 total - 528 = 752px)
      → Keep the prototype's chrome widths (rail 248px, inspector 280px, 52px total for other gaps/borders). Accept that at 1280px window width, the showcase grid is 752px, not 880px. The design is responsive; the 880px is a nominal target for an 1280px viewport, achieved by the relative
OPEN-Q RESOLVED:
   - Should the ScreenRouter despawn/respawn screens, or toggle Display::None? => Toggle Display::None. Taffy layout-prunes hidden subtrees, so a 1000-row scroll screen costs zero layout when hidden. State isolation is free by construction (n
   - Do screens need per-screen resource scoping for isolation? => No. The non-despawn architecture eliminates the need. Per-screen markers (TodoScreen, ScrollScreen, etc) ensure plugins query their own instances. Global resour
   - Should all 10 composites be promoted to buiy_widgets? => Only 3: meter (animation pattern), table_row (layout + selection), search_input (wrapper). Keep the other 7 (stepper, segmented, badge, chip, kbd, status_dot, t
   - How should headless rendering (capture_shell) stay in sync with BuiyPlugin? => Create a public BuiyHeadlessPlugin or BuiyPlugin::headless() subset (theme, layout, core, text, focus, a11y, widgets, render; no picking/winit). Make it first-c
   - What viewport width is the final target: 880px (design goal) or 752px (at 1280px => Keep 752px at 1280px window. The 880px is a responsive target for larger viewports. The showcase grid (CSS 1fr 1fr) fills whatever space is available. Changing 

####################################################################################
TRACK: virtualization-verify
####################################################################################
BUILD STRATEGY: For THIS subsystem (virtualization-verify), the final should ADAPT the prototype's paint-skip virtualization code (validated+green on RX 6700 XT, proven in C3-scroll, correctly reports 'mounted 1000' + visible window) without re-deriving it. The prototype's ContentVisibility::Auto + Display::None toggle is sound architecture; the final reuses it verbatim. For verification, the final REBUILDS clean on lavapipe: the prototype's RX PNGs were proof-of-concept, but the final's merge-gated CI goldens require pinned lavapipe and a formal blessing workflow. This is not re-deriving render logic; it is INTEGRATING the prototype's proven paint-skip widget tree into a lavapipe-based CI golden suite. The dual-path (headless snapshots for CI always + GPU goldens on pinned lavapipe for rasterization residue) follows the buiy_verify 5-tier pyramid already documented in CLAUDE.md. Implementation: (1) Port C3-scroll's scroll_list + footer/inspector reporting into the final's gallery parity-pass (Wave D in the final). (2) Add 3-5 GPU golden fixtures (logo gradient, dotted bg, icons) to buiy_verify with lavapipe blessing harness (local developer runs with lavapipe, confirms SSIM >0.99, commits .png). (3) Document the 'run the GUI locally before commit' discipline in CLAUDE.md. No architectural changes; the prototype's decisions were correct, just need CI blessing + documentation.

DECISIONS:
  [KEEP] Virtualization strategy: paint-skip vs true windowing
      → Keep paint-skip architecture (ContentVisibility::Auto on off-screen rows) for the final. The prototype proved this works end-to-end: all 1000 rows spawn once at boot, layout prunes hidden screen subtree via Display::None, paint skips offscreen rows. Inspector and footer both repo
  [KEEP] Inspector nuance: 'mounted' consistency across screens
      → Standardize the inspector/footer reporting across all screens. The prototype's resolved model: (1) 'mounted' always means resident ECS entities (all 1000 for scroll screen, ~5-10 for other screens depending on their content); (2) the visible window is projected from ScrollOffset 
  [REFINE] Verification strategy: headless layout snapshots vs GPU goldens for CI
      → Build a dual-path verification architecture: (1) Headless layout + snapshot tiers (Tiers 0-2 in buiy_verify): render the 5 screens to display-list snapshots, assert layout invariants (scroll row height 34px, inspector width 280px, modal bounds), capture pixel-exact layout dumps. 
  [REDESIGN] Golden blessing workflow and CI integration
      → Implement a 3-phase blessed-golden workflow: (1) LOCAL BLESSING PHASE (developer runs on GPU machine with lavapipe): author a manual test harness (buiy_verify integration, not CI) that: renders the 5 screens to RGBA textures, applies 2-3 permutations (scaled 2x/0.5x for robustnes
  [KEEP] Run-the-GUI discipline for visual verification
      → Institutionalize 'run the GUI' as a pre-commit, pre-gate discipline in the final: (1) Before committing any screen restyle or render change, run `cargo run -p buiy_gallery --release` locally (on a GPU machine if render features changed), visually inspect all 5 screens for layout/
  [REFINE] Which screens/capabilities get headless snapshots vs GPU goldens
      → Allocate testing tiers as: (1) HEADLESS SNAPSHOTS (Tier 1, CI always): shell_skeleton.snap (overall chrome layout), todomvc.snap (cards, buttons, inputs), scroll_list.snap (table, search, selection), modal.snap (overlays, focus), showcase.snap (all widgets). These snapshot the di
OPEN-Q RESOLVED:
   - Should the final do true DOM-style windowing (remount rows on scroll) or keep th => Keep paint-skip. The prototype proved all 1000 rows spawn once, layout prunes hidden subtrees to zero cost, paint skips off-screen via ContentVisibility::Auto. 
   - Is the inspector's 'mounted 1000' vs footer's 'rows 1–23' a parity bug, or corre => Correct reporting. 'Mounted' = resident ECS entities (all 1000 spawned at boot), 'rows X–Y' = visible window projected from ScrollOffset. These are not contradi
   - Can the final bless CI goldens on RX 6700 XT (the prototype's host), or must it  => Must use lavapipe. CI uses lavapipe; RX PNGs differ from lavapipe due to color-space/blend differences. The prototype's RX readbacks were proof-of-concept. The 
   - Should all 5 screens be blessed as GPU goldens, or only key render capabilities? => Only key capabilities: logo gradient (tests LinearGradient + token resolution + color math), dotted viewport pattern (tests RadialGradient + shader math), menu 
   - What is the final discipline for visual verification before committing? => Institutionalize 'run the GUI locally' as pre-commit, pre-gate discipline: (1) run `cargo run -p buiy_gallery --release` and eyeball all 5 screens locally (no s

####################################################################################
TRACK: api-prelude-buildplan
####################################################################################
BUILD STRATEGY: HYBRID CHERRY-PICK + REFINE APPROACH (phase 3 is not a full rewrite):

### THE DECISION: 60% port prototype validated code, 40% re-architect the identified pressure points

**Why hybrid (not full rebuild)?** The prototype's gate-green, GPU-proven landing proves the technical decisions are sound. Building from scratch would wastefully re-derive the same gradients/icons/animation/theming code that already works. BUT the prototype's exploratory process means some decisions (extract-query structure, prelude, dark-theme default) should be cleaned up in the FINAL to match production intent.

### WAVE SEQUENCE (parity-final, off main @ fdb8dda, 5-screen gallery baseline):

#### PHASE 3 WAVE 1: API Surface + Prelude Design (prep for all downstream waves)
- **Task 1.1 (design, ~2h):** Document final API surface — which types go in prelude, which stay in submodules. Update crates/buiy_core/src/lib.rs pub use blocks with BackgroundLayers, Icon, LetterSpacing, LinearGradient, RadialGradient, ColorStop, SetAccent. Create buiy::prelude module with consolidated re-exports (Rust convention).
- **Task 1.2 (design, ~2h):** Partition extract queries into logical sub-systems (the 15-tuple fix). Sketch the refactored render/extract.rs structure: extract_buiy_base (core), extract_buiy_colors (Background/Border/TextColor), extract_buiy_effects (BackdropFilter/EffectGroup), extract_buiy_gradients (BackgroundLayers), extract_buiy_icons (Icon). Document the seam boundaries (per render-pipeline spec).
- **Task 1.3 (review, ~1h):** Lock API surface decisions + sketch sign-off before coding.

#### PHASE 3 WAVE 2: Cherry-pick + land core animation/theming Wave A (parallel with 1, gate when 1 done)
- **Task 2.1 (port, ~3h):** Port prototype's A2 (dark-theme, derive_accent_ramp) + A3 (animation/Tween/Easing/Repeat) commits (755ef50, 4ee58d6, 9908fbd, 3d248ce) to parity-final. Validate format clean + docstring quality. Make dark theme the framework default (1-line change in BuiyPlugin build).
- **Task 2.2 (test, ~2h):** Gate: fmt/clippy/doc green, ~1340 headless tests pass (reuse Wave A test suite from prototype; no new gate).
- **Task 2.3 (review, ~1h):** Self-review + sign-off.

#### PHASE 3 WAVE 3: Port & refine render capabilities (gradients/icons/blur/cross_root_rank) — SEQUENTIAL on Wave 2
- **Task 3.1 (port, ~6h):** Port prototype's B-wave (gradients B1: e3866a5, icons B3: d494e63, blur B4: 272a4ef) + the M1/M6 fix (61ea95c cross_root_rank stacking) + the visibility/paint-skip fix (display-none handling). These are non-negotiable (GPU-proven, gate-green). While porting, apply the extract-query refactor (1.2 sketch) — this is the moment to restructure render/extract.rs per the 15-tuple fix before render capabilities solidify.
- **Task 3.2 (test, ~3h):** Gate: 1800+ headless + buiy_core GPU lane + render smoke tests green. The prototype's own GPU tests (gradient/icon readbacks, blur samples) reuse; no new ones needed at this stage.
- **Task 3.3 (review, ~1h):** Self-review + sign-off.

#### PHASE 3 WAVE 4: Shell + ScreenRouter + imperative screens (C-wave structure + C1/C3 landing)
- **Task 4.1 (port, ~8h):** Port prototype's C1 shell (d2b6a3c) + ScreenRouter + all 5 screen authoring (TodoMVC 2ec7f0d, Scroll 886e21a, Menu 04cc07c, Modal 32626af, Showcase 2f0fbbc). These are scene-building + layout; they depend on A-wave (theme/animation) + B-wave (render caps) being land. Screens are self-contained (each can be spot-checked).
- **Task 4.2 (test, ~4h):** Gate: shell_layout.snap + shell_router behavior + per-screen layout snapshots + 5 screens all render. Reuse prototype's snapshot suite + C1–C3 behavior tests (201 buiy_verify + gallery tests).
- **Task 4.3 (review, ~1h):** Self-review; verify no regressions on the 5 screens vs prototype screenshots.

#### PHASE 3 WAVE 5: Inspector + composites + live accent-swap wiring (C4 + polish)
- **Task 5.1 (port, ~4h):** Port C4 (inspector pane, live-state rows, accent swatches + SetAccent message wiring). Port the high-reuse composites (kbd_content, status_dot, pulse_blink) to buiy_widgets/composites.rs (new module).
- **Task 5.2 (test, ~2h):** Gate: inspector state-sync + accent-swap re-theming live + composites unit tests.
- **Task 5.3 (review, ~1h):** Self-review.

#### PHASE 3 WAVE 6: Final polish + parity audit (D-wave + human review prep)
- **Task 6.1 (audit, ~4h):** Re-verify all 5 screens via cargo run (GUI screenshot capture) + compare vs design values.md. Re-capture offscreen all 5 screens (re-run the capture_shell suite). Audit for the resolved bugs (M1/M6, M5, M4, M3, M2) + any new findings. Document any residual gaps.
- **Task 6.2 (cleanup, ~2h):** Docstring polish, update specs if needed (dark-theme default, extract-query refactor). Add a 'Phase 3 Decisions' section to the journal or a standalone FINAL-DECISIONS.md.
- **Task 6.3 (gate, ~3h):** Full workspace gate (fmt/clippy/doc/check/test/GPU) + 1800+ headless + GPU lanes all green.
- **Task 6.4 (review, ~1h):** Final self-review + sign-off for human-review gate.

### SEQUENTIAL vs PARALLEL:

- **Waves 1 & 2 can run in parallel** (1 is design, 2 is code porting; 1 doesn't block 2 beyond 'API names').
- **Waves 3, 4, 5 are SEQUENTIAL** (3 renders → 4 uses renders → 5 uses 4's structure). A stall on 3 (e.g., unexpected GPU bug) delays 4; this is the critical path.
- **Wave 6 is final** (cannot run until 5 complete).

**ESTIMATED TOTAL: ~40 hours** (serial path: 1+2 parallel [4h design + 5h code], 3 [9h], 4 [13h], 5 [6h], 6 [6h] = ~40h actual work for one agent; with team parallelization, tighter).

### THE HUMAN-REVIEW GATE:

After Wave 6 completes, **submit to human review** with:
1. Diffs (prototype → final for each wave).
2. Resolved-decisions doc (the api-prelude-buildplan decisions, captured here).
3. All 5 screenshots (vs design values.md for pixel-spot-check).
4. Full gate results (test counts, GPU green).
5. A brief narrative ('what changed from the prototype & why').

The human reviewer will spot-check GPU screenshots + the extract-query refactor + prelude structure. If approved, merge to main + tag as v0.4.0 (parity-complete).

DECISIONS:
  [REFINE] API Surface Promotion: BackgroundLayers + Icon + LetterSpacing + Gradient types
      → Promote to public prelude: add BackgroundLayers, BackgroundLayer, LinearGradient, RadialGradient, ColorStop, Icon, LetterSpacing to the root-level pub use in crates/buiy_core/src/lib.rs § render/components and text exports. Also promote SetAccent message (animation/mod.rs or them
  [KEEP] Animation & Tween promotion: SetAccent, Tween<T>, Easing + Repeat modes
      → KEEP the animation promotions AS-IS — they're already in the prelude and the prototype proved them essential (the shell's accent-swap swatch, the blink pulse, the switch/disclosure tweens all rode this path). Ensure Repeat enum + the PingPong + Loop constructors are documented in
  [KEEP] ColorToken::Literal escape-hatch — rejected or reconsider?
      → KEEP the token-only policy for FINAL. Do NOT add ColorToken::Literal. The prototype proved that registering ~33 named tokens is feasible + maintainable; the design's values.md is the single source of truth. Phase 3 inherits a fully-populated dark theme; no escape-hatch needed.
  [REFINE] Dark-theme default vs light default (framework baseline)
      → For FINAL: make the dark theme the EXPLICIT framework default. Option (a): BuiyPlugin (the meta-crate) inserts default_dark_theme() by default, with an optional opt-in to light_theme() via a plugin config. Option (b): Require every app to choose explicitly (gallery does; new apps
  [REDESIGN] Extract-query arity / nested QueryData design (the 15-tuple Bevy limit)
      → FINAL phase design: partition extract queries into LOGICAL sub-systems, NOT one flat monolithic query. Example structure: (1) extract_buiy_base (node, style, transform, clip — the core 5–6 fields); (2) extract_buiy_colors (background, border, text-color — 3 fields); (3) extract_b
  [REFINE] Composites: promote to buiy_widgets or keep gallery-local?
      → For FINAL: PROMOTE high-reuse composites to buiy_widgets/composites.rs (or a new buiy_widgets/composites submodule). Candidates: (1) kbd_content (split kbds with ⌘ icon), (2) status_dot (colored dot + glow shadow), (3) pulse_blink (the infinite opacity-tween helper). Keep screen-
  [REFINE] Widget prelude / re-export alignment across crates
      → FINAL: Add a pub mod prelude to buiy and buiy_widgets, consolidating the most-used types. Example: buiy::prelude = { Background, BackgroundLayers, LinearGradient, RadialGradient, Icon, LetterSpacing, Button, Switch, TextInput, Dialog, … }. This follows Rust convention + simplifie
OPEN-Q RESOLVED:
   - Should the gradient + icon + animation types be in prelude or submodules? => PRELUDE. The prototype's shell + 5 screens proved these are everyday authoring primitives (not niche features). Promoted to buiy::prelude for discoverability + 
   - Is the dark theme (or light) the framework default? => FINAL should make DARK the default (matching the design target). The prototype kept light as framework default + required gallery to opt-in. This was flagged as
   - Extract-query hitting the 15-tuple limit — design around it or work within it? => DESIGN AROUND IT. The prototype nested the query as a workaround. FINAL should partition extract into logical sub-systems (extract_buiy_colors, extract_buiy_eff
   - Which composites should be promoted to buiy_widgets? => kbd_content, status_dot, pulse_blink are reusable + go to buiy_widgets. table_row, search_input, table_header are screen-specific + stay in gallery. This is a c
   - Should ColorToken::Literal be added for ergonomics? => NO. The prototype proved token-only is correct + maintainable. Gate #11 (forced-colors compliance) requires it. All one-off colors are registered named tokens i
