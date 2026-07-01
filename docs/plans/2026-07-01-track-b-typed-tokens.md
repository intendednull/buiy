# Track B — Typed Theme Tokens + Theme Contract — Implementation Plan

> **For agentic workers:** Use `subagent-driven-development` to execute wave-by-wave; gate each wave with a fresh-context reviewer + the project gate. Steps use `- [ ]`.

**Goal:** Replace the stringly `ColorToken::Token(Cow<str>)` → `HashMap<String,Color>` → magenta-fallback theme system with a **closed-enum `ColorToken`** resolved through a **compiler-enforced `Theme` contract**, so a typo or a missing token is a compile error, never a silent magenta ship (friction F6 — the one deterministic failure in every N=4 prototype probe).

**Architecture:** `ColorToken` becomes a closed enum (flat, semantic — `ColorToken::SurfaceCard`, matching the exact API a prelude-only agent guessed). `Theme` implements a `ThemeContract` trait whose `resolve(&self, ColorToken) -> Color` is an **exhaustive `match`** (a dropped token ⇒ `E0004`, the vanilla-extract completeness for free — proven in prototype R2). The dynamic accent ramp is computed inside `resolve` from the theme's stored accent (not a pre-seeded map). A `ColorToken::Custom(Color)` escape hatch covers genuinely dynamic colors. The stringly `Token(Cow<str>)` variant and the `colors: HashMap` are removed.

**Tech Stack:** Rust, `bevy::color::Color`, `buiy_core::{render::color, theme}`. Base: `origin/main` @ `f37c6fa`.

**Scope (v1):** COLOR tokens only (the 192 call sites). Spacing/radius/typography/motion tokens are a documented follow-on (spec §3.2 "cover all" → tracked, not in this PR). Rationale: color is where F6 lives (magenta) and where all 192 sites are; a bounded first PR de-risks the pattern before widening.

---

## File structure

- `crates/buiy_core/src/render/color.rs` — the `ColorToken` enum (redefined), `ThemeContract` trait, `resolve_token` → delegates to the contract; remove `resolve_named` + `MISSING_TOKEN_FALLBACK` (no missing path).
- `crates/buiy_core/src/theme.rs` — `Theme` drops `colors: HashMap`, stores base palette + accent as typed fields, `impl ThemeContract for Theme` (exhaustive match, accent ramp computed here); `default_light_theme`, `SetAccent`, forced-colors updated.
- `crates/buiy_core/src/render/forced_colors.rs` — the forced-colors theme becomes a `ThemeContract` impl (or a `Theme` in forced mode).
- Call sites (192): `buiy_widgets` (10 files), `buiy_gallery`, `buiy_verify`, `buiy_core`, `buiy_bench_support`, `hello_bsn`, `capture` — `ColorToken::Token("color.x.y".into())` → `ColorToken::XY`.
- Tests: `render/color.rs` unit tests (missing-token test repurposed → the escape hatch + a resolve-roundtrip test); any theme tests.

## Token → variant map (the closed vocabulary, ~50 variants, flat + grouped)

```
color.surface.app|primary|secondary|card|raised|inset|chrome|danger|transparent
  → Surface{App,Primary,Secondary,Card,Raised,Inset,Chrome,Danger,Transparent}
color.text.primary|secondary|muted|dim|dimmer|faint|bright|placeholder|danger|bad
  → Text{Primary,Secondary,Muted,Dim,Dimmer,Faint,Bright,Placeholder,Danger,Bad}
color.border.default|subtle|muted|strong|danger → Border{Default,Subtle,Muted,Strong,Danger}
color.accent[.blue|coral|green|violet|soft|lighter|glow] → Accent, Accent{Blue,Coral,Green,Violet,Soft,Lighter,Glow}
color.status.ok|warn|error → Status{Ok,Warn,Error}
color.shadow.card|menu|modal → Shadow{Card,Menu,Modal}
color.selection.bg|fg → Selection{Bg,Fg}
color.scrollbar.thumb|track → Scrollbar{Thumb,Track}
color.caret|icon|focus.ring|scrim|misc.white → Caret, Icon, FocusRing, Scrim, White
```
Flat CamelCase (matches the prelude-only agent's `ColorToken::Surface*` guess). TEST-only strings (`color.a/b/g/r`, `color.brand`, `color.does.not.exist`) are NOT variants — their tests are repurposed (Wave 4).

---

## Wave 1 — The typed core (enum + contract + theme), gate before consumers

**Files:** `render/color.rs`, `theme.rs`, `render/forced_colors.rs`.

- [ ] **1.1** Write a failing test in `color.rs`: `default_light_theme().resolve(ColorToken::SurfaceCard)` returns the same `Color` the current `"color.surface.card"` HashMap entry holds (capture the current values first via a throwaway print or the existing theme source).
- [ ] **1.2** Define `pub enum ColorToken { …variants…, Custom(Color) }` (`#[derive(Clone, Copy, Debug, PartialEq, Reflect)]`) with the full closed vocabulary above; delete `Token(Cow<str>)`.
- [ ] **1.3** Define `pub trait ThemeContract { fn resolve(&self, token: ColorToken) -> Color; }`.
- [ ] **1.4** Redefine `Theme` in `theme.rs`: drop `colors: HashMap`; store the base palette (the current default values) + `accent: Color`. `impl ThemeContract for Theme` with an **exhaustive match** (accent ramp computed from `self.accent`; `Custom(c) => c`). Port `default_light_theme` to build the struct; port `SetAccent`/`seed_accent_tokens` to set `accent` (the ramp is now computed in `resolve`, not seeded). Keep `spaces`/`radii` fields untouched (out of scope).
- [ ] **1.5** `resolve_token(token, theme)` becomes `theme.resolve(token)` (keep the free fn as a thin shim so extract sites don't all change signature); remove `resolve_named` + `MISSING_TOKEN_FALLBACK`.
- [ ] **1.6** Port `forced_colors.rs`: the forced theme is a `Theme` (or `ThemeContract` impl) resolving every token to the forced-colors palette (System colors); exhaustive.
- [ ] **1.7** Run `cargo build -p buiy_core` — fix until it compiles (call sites in buiy_core itself, 4, migrate now).
- [ ] **1.8** Run the color unit test (1.1) — expect PASS (values preserved).
- [ ] **1.9** Commit: `feat(theme): typed closed-enum ColorToken + compiler-enforced ThemeContract`.
- [ ] **GATE 1:** fresh-context reviewer — enum completeness vs the mined vocabulary, value-parity with the old HashMap (no color drift), accent-ramp math preserved, forced-colors parity, `Reflect` derive intact (BSN/inspector).

## Wave 2 — Migrate the 192 consumer call sites (fan out under reliable-agent-fleet)

**Files:** `buiy_widgets/*` (10), `buiy_gallery`, `buiy_verify`, `buiy_bench_support`, `hello_bsn`, `capture`.

- [ ] **2.1** Build the exact string→variant substitution table (from the map above).
- [ ] **2.2** Fan out per-crate migration agents (reliable-agent-fleet: one agent per crate/example, DISTINCT files = no conflict; each returns the edits it made + a self-check that its crate compiles in isolation is NOT possible mid-migration, so each returns the sites it changed). Each: replace `ColorToken::Token("color.x.y".into())` / `ColorToken::Token(Cow::Borrowed("…"))` → the mapped variant. Flag any string not in the table.
- [ ] **2.3** Orchestrator applies/verifies; `cargo build --workspace` — resolve residuals (dynamic/computed token strings, if any, → `Custom` or a helper).
- [ ] **2.4** Commit per crate (or one squashed migration commit): `refactor: migrate ColorToken call sites to typed variants`.
- [ ] **GATE 2:** reviewer — every migrated site maps to the semantically-correct variant (not a typo'd neighbor), no `Token(` remnants (`grep` must be empty), no behavior change.

## Wave 3 — Diagnostics + the escape hatch + prelude

- [ ] **3.1** Ensure `ColorToken` (+ the families) are re-exported where `ColorToken` already is (prelude reach unchanged/added).
- [ ] **3.2** `#[doc]` the enum with the token semantics; optional `#[diagnostic::on_unimplemented]` is N/A (enum, not trait) — skip.
- [ ] **3.3** Confirm `Background { color: ColorToken::SurfaceCard }` authoring works from `buiy::prelude` (a small compile test).
- [ ] **3.4** Commit.

## Wave 4 — Tests + verification closure

- [ ] **4.1** Repurpose the old missing-token test: replace the `color.does.not.exist` → magenta assertion with (a) a compile-fail doc-test / note that invalid tokens can't be constructed, and (b) a `Custom(Color)` roundtrip test.
- [ ] **4.2** Add a resolve-parity test across ALL variants (each resolves to a non-magenta, deterministic color; the exhaustive match guarantees coverage).
- [ ] **4.3** Update any snapshot/golden that referenced token strings (unlikely — tokens resolve to the same colors, so goldens should be byte-identical; run the GPU lane to confirm).
- [ ] **4.4** Run the FULL project gate (see below). Fix all warnings.
- [ ] **4.5** Commit: `test(theme): typed-token resolve parity + escape hatch`.
- [ ] **GATE 3 (verify, don't just read):**
  - `cargo fmt --all -- --check`
  - `cargo clippy --workspace --all-targets --locked -- -D warnings`
  - `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --locked`
  - `cargo test --workspace --locked` (headless)
  - GPU lanes (real adapter): `cargo test -p buiy_core -j2 -- --ignored --test-threads=1` + `cargo test -p buiy_verify -j2 -- --ignored --test-threads=1` (token colors must render byte-identical — goldens unchanged).
  - `cargo deny check`
  - **Run a real app:** `cargo run -p buiy_gallery` — screens still render with correct colors (not magenta).

## Wave 5 — Docs + PR

- [ ] **5.1** Flip the spec §3.2 note (typed tokens landed for color; spacing/radius follow-on tracked). Update `docs/README.md` if a token doc is added.
- [ ] **5.2** Open PR `feat(theme): typed closed-enum ColorToken + theme contract` → base `main`; wait for green CI (3-OS + GPU lavapipe + MSRV + web-smoke + deny); merge on green (owner-authorized for this loop).

---

## Self-review notes
- **Spec coverage:** realizes spec §3.2 (closed enum + contract, exhaustive-match completeness, escape hatch, migration) for the color scope; spacing/radius explicitly deferred + tracked.
- **Value-parity risk:** the load-bearing correctness check is Wave-1 GATE + Wave-4 GPU goldens — token colors must be byte-identical to today (this is a refactor, not a re-theme). Any color drift is a bug.
- **Dynamic-token risk:** the accent ramp + any runtime-computed token must be computed in `resolve` from theme state or routed through `Custom(Color)`; Wave-2.3 catches strings that don't map.
