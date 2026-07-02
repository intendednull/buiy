# Track B — Typed Theme Tokens + Theme Contract — Implementation Plan (v2, post-gate)

> **For agentic workers:** Use `subagent-driven-development`; gate each wave with a fresh-context reviewer + the project gate. `- [ ]` steps.
>
> **v2** incorporates the plan-gate review (7 must-fixes). v1 was mined from `color.*` string literals only and would have broken the build + silently disabled the gate-#11 a11y analyzer. See `docs/reports/2026-07-01-llm-dev-support-prototype-retrospective.md` and the spec `specs/2026-07-01-first-class-llm-dev-support-design.md` §3.2.

**Goal:** Replace stringly `ColorToken::Token(Cow<str>)` → `HashMap<String,Color>` → magenta with a **closed-enum `ColorToken`** resolved through a **compiler-enforced `ThemeContract`**, so a typo/missing token is a compile error, never a silent magenta ship (F6). Preserve every current behavior: forced-colors, the gate-#11 analyzer, auto-caret, and byte-identical rendered colors.

**Architecture:** `ColorToken` keeps its non-semantic variants (`Transparent` `#[default]`, `CurrentColor`, `SystemColor(SystemColorKeyword)`) + gains `Custom(Color)` (the escape hatch replacing `Token`) + the **closed semantic vocabulary** (union of theme-seed keys ∪ real usage, ~55 variants). `Theme` drops `colors: HashMap`, stores a typed palette + `accent: Color` + an explicit `mode: PaletteMode { Normal, ForcedColors }`, and `impl ThemeContract` with an exhaustive `match` that branches on `mode` (forced maps every semantic token → a `SystemColorKeyword`). `caret`/`preedit-underline` are NOT resolvable variants — their consumers default to `CurrentColor` (auto). The gate-#11 analyzer switches from magenta-equality to a `ColorToken` variant-kind check.

**Tech Stack:** Rust, `bevy::color::Color`, `buiy_core::{render::color, render::forced_colors, render::forced_colors_analyzer, theme}`, `buiy_verify::reftest`, `buiy_bsn`. Base: `origin/main` @ `e431eef` (rebased).

**Scope (v1 of the token system):** COLOR tokens. `spaces`/`radii` stay stringly HashMap (keep `Theme::space()`/`radius()`); typography/motion deferred + tracked. `Theme` becomes a hybrid (typed colors, stringly spaces/radii) — intentional.

---

## MUST-FIX ledger (from the gate — each mapped to a wave)

| # | Must-fix | Wave |
|---|---|---|
| 1 | Keep `Transparent`(`#[default]`)/`CurrentColor`/`SystemColor(kw)` + `Default` derive | W1 |
| 2 | Vocab = union(theme seed keys, usage); +~14 tokens; drop phantom `text.bad` | W1 |
| 3 | Forced-colors: `Theme` value + explicit `mode`; re-express selection/CurrentColor prefs; system value for every token | W1 |
| 4 | Gate-#11 analyzer → variant-KIND check, not magenta-equality; replace `MISSING_TOKEN_FALLBACK` uses | W1 |
| 5 | `caret`/`preedit-underline` stay auto (`CurrentColor` default), NOT resolvable variants | W1 |
| 6 | Re-scope ~40 test-injection files + reftest engine + `buiy_bsn` round-trip → `Custom(Color)` | W3 |
| 7 | Accent swatches = literals distinct from live accent; regression test for `SetAccent` | W1+W5 |

## Gate re-review closing edits (applied — CONDITIONAL GO → these make it a GO)

1. **(B, MF#4) `FocusRing` is forced-colors-safe.** Add `ColorToken::FocusRing` to `is_forced_colors_safe()` — the forced theme deliberately seeds `focus.ring→yellow` ("ring stays visible under forced colors", `theme.rs:464-474`); flagging it would contradict that contract. **Restate the resolve-vs-analyzer split explicitly:** forced-*resolve* maps EVERY semantic token → a system color (safety net); the *analyzer* still flags semantic tokens (best-practice nudge) — deliberately NON-equivalent to the old magenta check. So **W1.11 GATE-1 criterion is operational, not "equivalent":** `live_catalog_has_no_forced_colors_violations` stays GREEN **and** `broken_fixture_produces_violation` (broken-brand) stays RED.
2. **(C, W1.5) Light-value SOURCE (decided, not free-invented):** the ~46 net-new light tokens are **derived by family** from the existing 9 light neutrals + the dark palette's structure — surface.* → white→light-gray ladder; text.* → dark-ink ladder; border.* → light-gray ladder; status.* → shared semantic hues; accent* → shared live accent / `derive_accent_ramp`; shadow.* → low-alpha black; scrollbar.* → gray; misc (white/dot-bg/scrim) → literals. These are **completeness defaults, NOT pixel-critical**: no light-default consumer paints them today (they were magenta and nothing broke), so the pixel-critical parity path is the DARK/gallery theme. Expect a few headless **light snapshots to shift magenta→real → re-bless in W4** (legitimate, not a regression).
3. **(A, W1.0) Forced-theme values are expected-to-change, NOT a parity target.** Under forced mode most semantic tokens are magenta today → W1.4 maps them to role-based system colors (an improvement). W1.1 asserts only the Normal/dark value. GATE-1 sanity-checks the forced role map (judgment calls, lossy by nature). Safety confirmed: `live_catalog_has_no_forced_colors_violations` already proves no real widget paints a semantic-token pixel under forced, so no forced golden moves.
4. **(W2 add)** Substitution table must explicitly map `color.surface.transparent → Transparent` (22 uses, collapsed into the default) and `color.scrollbar.track → ScrollbarTrack` (resolves `NONE`) so the fan-out agents don't flag them as "not in table."
5. **(W3 add)** Delete/repurpose the magenta-assertion tests `render_extract.rs:30` + `render_extract_background.rs:26` (they assert `resolve(fake) == magenta`, a behavior that no longer exists) — delete, not substitute.
6. **(note)** `debug_name()` returns `"SurfaceCard"`, not the old `"color.surface.card"` — confirm `modal_showcase_c8c.rs:408`'s assertion still means the same thing after W3.4 (it asserts *a* name is extractable, not the exact string — verify).

## Enum shape (v1, locked)

```rust
#[derive(Clone, Copy, Debug, PartialEq, Reflect, Default)]
pub enum ColorToken {
    #[default]
    Transparent,                       // Color::NONE (KEEP)
    CurrentColor,                      // inherit; the auto default for text/caret/icon (KEEP)
    SystemColor(SystemColorKeyword),   // forced-colors system palette (KEEP)
    Custom(Color),                     // genuinely-dynamic / test-only escape hatch (replaces Token)
    // --- closed semantic vocabulary (union of theme seeds ∪ usage) ---
    // Surface: App Primary Secondary Card Raised RaisedAlt Inset Chrome ChromeTranslucent Danger DangerSoft DangerStrong
    // Text:    Primary Secondary Muted Dim Dimmer Faint Bright Placeholder Danger DangerDim OnAccent
    // Border:  Default Subtle Subtle2 Muted Strong Strong2 Danger
    // Accent (LIVE, derive from self.accent): Accent AccentLighter AccentSoft AccentGlow
    // Accent (FIXED swatches, literals):       AccentBlue AccentGreen AccentViolet AccentCoral
    // Status:  Ok Warn Error
    // Shadow:  Card Menu Modal SliderThumb SwitchThumb DangerButton
    // Selection: Bg Fg
    // Scrollbar: Thumb ThumbHover Track
    // Misc:    Icon FocusRing Scrim White DotBg
    // (NOT variants: caret, preedit-underline → consumer default = CurrentColor; text.bad → test phantom)
}

impl ColorToken {
    /// Gate-#11: a token is forced-colors-safe iff it is a system/neutral kind
    /// (SystemColor / Transparent / CurrentColor), NOT a concrete semantic color.
    pub fn is_forced_colors_safe(&self) -> bool {
        // FocusRing included: the forced theme seeds focus.ring->yellow so the ring
        // stays visible under forced-colors (theme.rs:464-474) — not a violation.
        matches!(self, ColorToken::SystemColor(_) | ColorToken::Transparent
            | ColorToken::CurrentColor | ColorToken::FocusRing)
    }
    /// For introspection sites that used to read the token string.
    pub fn debug_name(&self) -> String { format!("{self:?}") }
}
```

## Wave 1 — Typed core: enum + contract + theme + forced-colors + analyzer (the hard wave)

**Files:** `render/color.rs`, `theme.rs`, `render/forced_colors.rs`, `render/forced_colors_analyzer.rs`, `render/components.rs` (defaults).

- [ ] **1.0 Capture ground truth:** dump the current resolved `Color` for EVERY (token, theme∈{light, dark, forced}) triple (a throwaway test printing `resolve_token` over all seeded keys) → the parity oracle for W1/W5. Record dark-theme values (the gallery palette) especially — these must stay byte-identical.
- [ ] **1.1** Failing test: `default_dark_theme().resolve(SurfaceCard)` == the captured dark value; `Transparent` == `Color::NONE`; `SetAccent(green)` moves `Accent` but NOT `AccentBlue`.
- [ ] **1.2** Define the enum (above), keeping the 3 non-semantic variants + `Default`. Delete `Token(Cow<str>)`. Add `is_forced_colors_safe` + `debug_name`.
- [ ] **1.3** `ThemeContract { fn resolve(&self, ColorToken) -> Color; }`. `Theme`: drop `colors`, add typed palette fields + `accent: Color` + `mode: PaletteMode`. Keep `spaces`/`radii` + `space()`/`radius()`.
- [ ] **1.4** `impl ThemeContract for Theme` — exhaustive match. `Normal`: typed palette; `Accent*` live (reuse `derive_accent_ramp(self.accent)`); `Accent{Blue,Green,Violet,Coral}` = fixed literals; `Custom(c)=>c`; `Transparent=>NONE`; `CurrentColor=>` inherit sentinel (as today); `SystemColor(kw)=>` system palette. `ForcedColors`: every semantic token → its `SystemColorKeyword` mapping (Canvas/CanvasText/Highlight/HighlightText/ButtonText/GrayText/LinkText per role); selection.bg→Highlight, selection.fg→HighlightText, text.*→CanvasText, surface.*→Canvas, border.*→CanvasText, accent/focus→Highlight/LinkText, etc.
- [ ] **1.5** Port `default_light_theme` + `default_dark_theme` to the typed struct; **author light-theme values for the ~40 tokens it omits today** (they were magenta; pick sensible light values — this is the half-wiring fix). `SetAccent`/`seed_accent_tokens` → set `self.accent` only (ramp computed in resolve).
- [ ] **1.6** `resolve_token(token, theme)` → `theme.resolve(token)` shim. Re-express `resolve_selection_bg/_fg` + the `CurrentColor`→CanvasText preference against `theme.mode == ForcedColors` (not HashMap presence). `resolve_caret_color`/preedit keep `CurrentColor` default (auto).
- [ ] **1.7** `forced_colors.rs`: `forced_colors_theme()` returns a `Theme` with `mode: ForcedColors` (stays in `Res<Theme>`); `PrePreferenceTheme` save/restore unchanged (it's a `Theme`).
- [ ] **1.8** `forced_colors_analyzer.rs`: replace `resolve_token(..) == MISSING_TOKEN_FALLBACK` with `!token.is_forced_colors_safe()`. Remove `MISSING_TOKEN_FALLBACK` (or keep private if still referenced). Keep the `non_system_token_under_forced_theme_is_a_violation` test semantics (now: a semantic-family token IS a violation).
- [ ] **1.9** Migrate `buiy_core`'s own call sites (incl. `render/components.rs` defaults: `TextColor`/`CaretColor`/`IconColor` default `CurrentColor`; `BackgroundLayer::Solid`). `cargo build -p buiy_core`.
- [ ] **1.10** Run 1.0/1.1 parity tests — PASS.
- [ ] **1.11** Commit. **GATE 1 (fresh reviewer):** vocab = superset of reality; dark-theme value-parity; forced mapping covers every token + preserves the selection/CurrentColor prefs; analyzer kind-check equivalent to the old magenta check; auto-caret preserved; swatch-vs-live-accent correct.

## Wave 2 — Migrate production `color.*` literal call sites

**Files:** `buiy_widgets` (10), `buiy_gallery`, `hello_bsn`, `capture` (the literal-`Token("color.x")` sites).

- [ ] **2.1** Substitution table string→variant (from 1.2). Fan out under `reliable-agent-fleet` per crate (distinct files); each agent returns changed sites + flags any string not in the table (→ escalate: new variant or `Custom`).
- [ ] **2.2** `cargo build --workspace` (widgets/gallery/examples). Resolve residuals.
- [ ] **2.3** Commit. **GATE 2:** every site maps to the semantically-correct variant; `grep -r 'ColorToken::Token'` empty in these crates.

## Wave 3 — The idiom redesign (test injection + reftest engine + bsn) — DESIGN, not a sweep

**Files:** `buiy_verify/src/reftest.rs` (+ `reftest_engine_gpu.rs`), `buiy_bsn/tests/round_trip.rs`, `verify_headless/{contrast.rs, modal_showcase_c8c.rs}`, `buiy_bench_support/{lib.rs,mvu_scenes.rs}`, and the ~40 `crates/buiy_core/tests/**/*_gpu.rs` + `buiy_verify/tests/verify_gpu/*` theme-injection tests.

- [ ] **3.1** Reftest engine: replace the `theme.colors.insert(key,color) + Token(key)` draw-color path with a direct `Color` / `ColorToken::Custom(color)` field — redesign the harness API (it's shared).
- [ ] **3.2** Test-injection files: `insert("test.red",c)+Token("test.red")` → `Custom(c)` (drop the injection). Fan out under `reliable-agent-fleet` per file; parity oracle = the same pixel.
- [ ] **3.3** `buiy_bsn/tests/round_trip.rs`: `Token(Cow::Borrowed("color.brand"))` → a variant or `Custom` authored via the real `bsn!`/`spawn_scene` path.
- [ ] **3.4** Introspection `modal_showcase_c8c.rs:408`: `Token(t)=>Some(t.to_string())` → `token.debug_name()`.
- [ ] **3.5** Dynamic sites (`reftest.rs:227/211`, `render_prepare.rs:373`, `render_smoke.rs:355`, `render_border_shadow*`, `render_patch_upload_gpu.rs:46`, `bench_support`) → `Custom(color)` / helper.
- [ ] **3.6** `cargo build --workspace --all-targets`. Commit. **GATE 3:** no `Token(` remnants anywhere; the reftest engine redesign is sound; injections preserve their pixels.

## Wave 4 — Full verification (verify, don't just read)

- [ ] **4.1** `cargo fmt --all -- --check`; `cargo clippy --workspace --all-targets --locked -- -D warnings`; `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked`.
- [ ] **4.2** `cargo test --workspace --locked` (headless).
- [ ] **4.3** GPU lanes (real adapter): `cargo test -p buiy_core -j2 -- --ignored --test-threads=1` + `cargo test -p buiy_verify -j2 -- --ignored --test-threads=1` — **token colors byte-identical → goldens unchanged** (the core parity proof).
- [ ] **4.4** `cargo deny check`. **Run `cargo run -p buiy_gallery`** — all screens render correct colors (not magenta), `SetAccent` still re-themes, swatches stay fixed.
- [ ] **4.5** Commit. **GATE 4:** all lanes green; parity oracle satisfied; gallery visually correct.

## Wave 5 — Regression tests + docs + PR

- [ ] **5.1** Add: (a) swatch-stability under `SetAccent` (must-fix #7); (b) all-variants-resolve-non-magenta (exhaustiveness already guarantees, assert anyway); (c) forced-colors: every semantic token → a system color, `is_forced_colors_safe` matches the analyzer; (d) `Custom` roundtrip; (e) auto-caret (no `color.caret` → CurrentColor).
- [ ] **5.2** Flip spec §3.2 note (typed color tokens landed; spacing/radius/typography follow-on tracked). Update `docs/README.md` if a token doc is added. Note the light-theme half-wiring fix (magenta→real) as intentional.
- [ ] **5.3** Open PR `feat(theme): typed closed-enum ColorToken + theme contract` → `main`; wait for green CI (3-OS + GPU lavapipe + MSRV + web-smoke + deny); merge on green (owner-authorized).

---

## Self-review notes
- **All 7 must-fixes mapped to waves** (ledger above); the gate's CRITICAL build-break (dropped variants / `MISSING_TOKEN_FALLBACK`) is closed in W1.1–1.2 + 1.8.
- **Value-parity oracle is W1.0** — dark-theme (gallery) colors byte-identical is the load-bearing correctness check, re-proven by GPU goldens in W4.3. Light-theme missing tokens change magenta→real (intentional half-wiring fix).
- **Forced-colors is the hardest wave (W1.4/1.7/1.8)** — explicit mode + full system mapping + analyzer-by-kind; gated hardest.
- **Idiom redesign (W3) is design work** across ~40 files + the shared reftest engine — not a literal sweep; fanned out but oracle-checked.
