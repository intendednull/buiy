**Date:** 2026-05-22
**Status:** active
**Subject:** cosmic-text — critiques, open issues, and structural open problems

# Critiques and open problems

cosmic-text is the most usable pure-Rust text engine that exists. That doesn't make it complete. This file collects the published frustrations from issue threads, the structural API friction, and the missing-features list — verbatim where possible.

Cross-links: [editing.md](editing.md), [integration.md](integration.md), [capabilities.md](capabilities.md), [ecosystem.md § Parley contrast](ecosystem.md#parley-linebender) (a Parley comparison for many of these gaps).

## Performance critiques

### `FontSystem::new` is unacceptably slow

Issue **[#505](https://github.com/pop-os/cosmic-text/issues/505)** — open at folder-authoring time. Verbatim reporter:

> cosmic-text 0.14.2 spends approximately 1.344 seconds initializing `FontSystem`, consuming 84% of samples during a 1.6-second profiling window. cosmic-text appears to `mmap` every font it can find in `/usr/share/fonts`.

Despite an earlier lazy-mapping commit (2023), the April 2025 release still does the full filesystem walk on construction. NVMe-class storage is the reporter's setup — slower disks make it worse.

The Buiy implication: **Buiy must not call `FontSystem::new()` on the UI thread during startup**. Either off-thread the construction or use fontdb's `load_font_data` / `load_font_source` to register a known set of fonts at construction and defer the system-scan. The Buiy `buiy-asset-pipeline-design` sub-spec is the natural place to commit a strategy.

### Full-Buffer reshape on edit

Editing a single character in a long `Buffer` triggers re-shape of the affected `BufferLine` and re-layout of all subsequent visual lines (because line-break opportunities depend on width). For paragraph-length text this is fine; for a 1000-line text-area editing the first line forces a full reflow downstream. There's no exposed "shape only the affected range and patch" API.

The pre-amble's "full-rebuild cost on edit" — corrected: cosmic-text actually reshapes per-line, not per-buffer. The dominant cost is the layout-cascade through subsequent lines when wrap settings + width are in play.

### Atlas churn

cosmic-text owns `SwashCache` (the CPU glyph-bitmap cache); the embedder owns the GPU atlas. There's no API to evict glyphs from `SwashCache` selectively — it's a `HashMap` that grows. For long-running apps (a code editor open for days) the cache footprint trends up. Embedders that care implement their own eviction by wrapping `SwashCache` access; cosmic-text gives no help.

### Cold-cache shaping

The shape cache (`shape-run-cache` feature, off by default in `default`) is opt-in. Without it, every layout pass re-shapes runs that may already be in cache. Most embedders should turn it on; the spec does not flag this loudly enough.

## API surface critiques

### `Buffer` vs `Editor` split is awkward

`Editor<'buffer>` borrows a `Buffer`. To compose them you either own both (and lend a mutable borrow on demand) or wire complex lifetimes through. The result: every embedder ends up writing an "owning editor" wrapper of some kind (the COSMIC apps do; Iced does; `bevy_cosmic_edit` did). The split makes sense for "many viewports onto one Buffer" but that case is rare; the common case (one editor, one buffer) pays the lifetime tax.

### `FontSystem` lifetime ownership

`FontSystem` is non-`Sync` and non-`Clone`. Every layout/shape/render call requires `&mut FontSystem`. In multi-window or worker-thread apps, this means a `Arc<Mutex<FontSystem>>` and lock contention, or pinning text work to a single thread. Bevy 0.15 wraps it in a `Res<CosmicFontSystem>` newtype that pins to a specific thread; Iced serializes through its renderer. No embedder has found a clean way to parallelize text work across cores; the `FontSystem` is a process-wide bottleneck.

> **Correction (text campaign T9, 2026-06-11):** verified against cosmic-text 0.19 — non-`Clone` holds; non-`Sync` is **false** (docs.rs 0.19.0 lists `impl Send for FontSystem` and `impl Sync for FontSystem`). The serialization pressure this section describes is real, but its cause is the `&mut FontSystem` API, not a missing marker trait. See [text verification.md § 5](../../specs/2026-06-09-buiy-text-rendering-design/verification.md#5-prior-art-errata-ledger).

### `Attrs` lifetimes

`Attrs<'a>` borrows its font-name string. For dynamic attrs (e.g. theming-driven span attrs that change at runtime) you either keep the source strings alive in some auxiliary store or use `Attrs::new().family(Family::Name(&owned_name))` and re-build attrs each frame. The `smol_str` dep helps for short strings but doesn't relieve the structural pattern.

## IME boundary

The single largest user-visible cosmic-text limitation is that IME is not addressed. Issue **[#10](https://github.com/pop-os/cosmic-text/issues/10)** ("IME support") was opened by jackpot51 on **2022-10-24** and the entire body is:

> Will need a `winit` example to experiment with this.

Three and a half years later, no winit example has landed and no IME-specific API has been added. The maintainer's position is *not* that IME is unimportant — the COSMIC desktop must support CJK + Korean IMEs — but that the IME boundary is properly the embedder's responsibility, because IME state-machine details vary across desktops (IBus, fcitx5, macOS NSTextInput, Windows TSF, ChromeOS, mobile virtual keyboards) and across winit versions.

The structural answer: **winit's IME API is the gating dependency, not cosmic-text**. winit 0.29+ exposes `Ime::{Enabled, Preedit, Commit, Disabled}` events; embedders translate these to cosmic-text `Action::Insert` for commits and maintain preedit state in a parallel span. The Buiy spec ([text.md § IME composition](../../specs/2026-05-07-buiy-foundation/text.md)) commits to this pattern with composition-commit-as-undo-unit grouping, which Buiy implements above cosmic-text.

Embedder frustration in the wider community: there's no canonical "this is how you do IME with cosmic-text + winit" example anywhere. `bevy_cosmic_edit` had a partial implementation; it was archived before completing the Korean Hangul Jamo cases (see issue [#485](https://github.com/pop-os/cosmic-text/issues/485), open: Korean Hangul Jamo characters render as separate components instead of combined).

## Missing or incomplete features

Status of each feature in 0.19.0:

### Vertical writing modes

**Not supported.** No `text-orientation: mixed | upright | sideways`, no `writing-mode: vertical-rl | vertical-lr`. No tracking issue (a search of the repo's issue tracker for "vertical" returns scroll-related issues, not writing-mode requests). For CJK vertical typesetting, ruby annotations, or Mongolian script (LTR vertical), cosmic-text is the wrong tool today. Parley also lacks vertical writing, so this is a substrate-wide gap.

The Buiy implication: feature tier in [text.md § Bidirectional text](../../specs/2026-05-07-buiy-foundation/text.md) flags `text-orientation` as **E (extended)** and ruby as **E**. Both are likely punted past the first release window.

### Hyphenation

**Not supported.** No `hyphens: auto` analogue. No tracking issue. Hyphenation requires a language-tagged dictionary (`hyphen` crate exists for this, with LibreOffice's `hyph_*.dic` files), and embedders that need it wire their own pre-pass before feeding text to cosmic-text. The `Buffer` does not expose a "honor soft hyphens in this text" API; you'd have to insert `U+00AD` SOFT HYPHEN characters in the source and configure `unicode-linebreak` to respect them (which it does, but the integration isn't documented).

### Variable font axes beyond weight

Issue **[#406](https://github.com/pop-os/cosmic-text/issues/406)** — open. Verbatim: variable-font axes (e.g. Weight) aren't reflected in COSMIC UI when using "Adwaita Sans." Variable font support landed in 0.15.0 but with caveats: weight works; other registered axes (italic `ital`, width `wdth`, slant `slnt`, optical-size `opsz`) get patchy results depending on the font and the font's `STAT` table. Custom axes are not exposed in the public API.

### COLRv1 color fonts

Issue **[#446](https://github.com/pop-os/cosmic-text/issues/446)** — open. Verbatim:

> Fedora 43's adoption of the COLRv1 format for Noto Color Emoji has broken applications using cosmic-text. Swash doesn't support COLRv1 (the issue has been open for 4+ years). Swash development appears inactive.

cosmic-text supports CPALv0 / COLRv0 emoji (the older, simpler color-glyph table format Apple Color Emoji and earlier Noto Color Emoji used). COLRv1 (with gradients, transformations, sub-glyph composition) is *unsupported*, and the upstream blocker is `swash`. Without swash adding COLRv1, cosmic-text can't render Fedora 43+ default emoji.

This is the most acute Buiy-relevant gap. Buiy will hit it on Linux distros that have moved to COLRv1.

### `text-decoration` properties

Text decoration (`underline`, `strikethrough`) **shipped in 0.19.0** (April 2026). Before that, embedders drew underlines from glyph metrics. The 0.19.0 surface is minimal: line, color. Properties from the CSS spec that are NOT supported: `text-decoration-style` (`solid | dotted | dashed | wavy`), `text-decoration-thickness`, `text-underline-offset`, `text-underline-position`, `text-decoration-skip-ink`. Embedders that want anything beyond solid lines still draw their own decorations.

### `text-overflow: ellipsis`

**Shipped in 0.18.0** (February 2026) with start / middle / end variants. Patched twice the next day (0.18.1, 0.18.2) for aggressiveness regressions. Per the foundation spec, this is **C** tier and ellipsizing is now usable; multi-line `line-clamp` is the harder case and is **not** built in — embedders compute it from `Buffer::layout_runs().take(N)`.

### `text-transform`

`text-transform: uppercase | lowercase | capitalize` is **not in cosmic-text**. The transformation is conceptually a text-input layer above the shaper (you transform the source string before feeding it to `Buffer::set_text`). Embedders handle it; cosmic-text neither helps nor hurts.

### `letter-spacing` / `word-spacing`

`letter-spacing` is supported via per-`Attrs` letter spacing (added in an 0.1x release). `word-spacing` is **not** a separate API — the embedder adjusts source text or replaces `U+0020` with a wider whitespace character.

### Additional BiDi override controls

cosmic-text honors UAX #9 implicit BiDi via `unicode-bidi`. Explicit override characters (`U+202A` LRE, `U+202B` RLE, `U+202C` PDF, `U+2066` LRI, `U+2067` RLI, `U+2068` FSI, `U+2069` PDI) are recognized at the unicode-bidi layer; cosmic-text does not expose them as `Attrs` directly. CSS `unicode-bidi: embed | isolate | bidi-override` are not first-class; embedders insert the Unicode control characters themselves.

### Spell check / grammar

Explicitly out of scope, with the embedder boundary being clean. See [editing.md § Spell check](editing.md#spell-check--grammar). This is the rare case where "not in cosmic-text" is the correct architecture — text engines should not own dictionaries.

### Web font loading semantics (FOUT / FOIT)

Not applicable. cosmic-text is not a web rendering engine and has no notion of progressive font loading. Embedders that load fonts asynchronously do so above cosmic-text — typically by deferring text rendering until the font is registered with `fontdb`, or rendering with a fallback synchronously and re-shaping when the real font arrives (the FOUT analogue). Buiy's font registration is part of the asset pipeline; this is a non-issue for the engine.

### Subpixel rendering / hinting modes

Hinting was made configurable in **0.16.0** via the `Hinting` enum (`Hinting::None | Hinting::Slight | Hinting::Full`). Subpixel positioning is supported in the shape output (`LayoutGlyph::x` is float, not int); subpixel *anti-aliasing* (LCD-RGB) is a render-side concern that swash supports but cosmic-text leaves to the embedder.

## Other open issues to surface (Buiy will hit these)

- **[#499](https://github.com/pop-os/cosmic-text/issues/499)** — Fontconfig aliases not respected. `Family::Name("monospace")` falls back to cosmic-text's hardcoded list (DejaVu Sans Mono, Noto Color Emoji) rather than honoring the user's fontconfig configuration. Buiy on Linux will need to choose between fontconfig integration (via `fontconfig` feature on fontdb) and the hardcoded fallback list. The hardcoded fallback list is more deterministic for testing; fontconfig matches user expectations.
- **[#485](https://github.com/pop-os/cosmic-text/issues/485)** — Korean Hangul Jamo rendering. Composing Hangul (jamo characters → syllable blocks) sometimes leaves jamo separate. Likely shaper-config issue (Jamo cluster boundaries vs syllable boundaries). Affects Korean IME composition rendering specifically.
- **[#493](https://github.com/pop-os/cosmic-text/issues/493)** — Pixel fonts render 1 px to the right. Niche but reproducible.
- **[#465](https://github.com/pop-os/cosmic-text/issues/465)** — Repeated `+` characters render incorrectly. Likely ligature-table interaction.
- **[#446](https://github.com/pop-os/cosmic-text/issues/446)** — COLRv1 (see above).
- **[#406](https://github.com/pop-os/cosmic-text/issues/406)** — Variable font axes incomplete (see above).
- **[#10](https://github.com/pop-os/cosmic-text/issues/10)** — IME (see above).

## Open problems (structural, not bugs)

These aren't issues with tracking numbers; they're structural features cosmic-text is not architected to solve in its current form.

### Hyphenation

Requires a language-tagged dictionary, language-tag detection per text run, and integration with the line-break-opportunity pass. cosmic-text uses `unicode-linebreak` for break opportunities; hyphenation would either fork that crate or layer above it. No design discussion in cosmic-text issues. **Buiy stance:** out of foundation scope; treat as **E (extended)** per the spec, embedder-level pre-pass if needed.

### Vertical writing modes (CSS `writing-mode: vertical-rl | vertical-lr`)

A vertical-mode implementation touches every part of the pipeline: the BiDi run order is conceptually orthogonal to the horizontal/vertical axis but the layout step has to swap x and y semantics; line breaking becomes column breaking; the cursor model's "up/down vs left/right" reverses; selection rectangles change shape. Not a small patch. Both cosmic-text and Parley lack this. **Buiy stance:** **E (extended)** per [text.md](../../specs/2026-05-07-buiy-foundation/text.md), deferred past foundation.

### Variable fonts axis control

Partially shipped; the public surface exposes weight only. Custom axes (`opsz`, `wdth`, `slnt`, plus designer-defined axes) need an API addition (probably extending `Attrs` with a `variations: Vec<(Tag, f32)>` field). Tracked in #406 but with no committed roadmap.

### Spell check (out of scope — correctly)

The right architecture is for the engine to provide text + position queries and the embedder + OS to provide the dictionary + decorating spans. cosmic-text honors this boundary. Buiy will too — the foundation spec [text.md § OS integration](../../specs/2026-05-07-buiy-foundation/text.md) defers spellcheck to an OS bridge (`buiy-clipboard-and-os-integration-design`).

### Web-font loading semantics (FOUT / FOIT)

Not applicable — cosmic-text is not a web engine. Buiy's asset pipeline handles font availability; cosmic-text just sees what's registered.

### Subpixel rendering and hinting modes

`Hinting` enum exists since 0.16.0. LCD subpixel anti-aliasing is delegated to swash but not exposed as a first-class cosmic-text knob; the embedder selects the swash render mode when rasterizing. Adequate but not exposed as cleanly as `font-smoothing: antialiased | subpixel-antialiased` would be.

## Sources

- Issue #10 (IME) — https://github.com/pop-os/cosmic-text/issues/10
- Issue #406 (variable font axes) — https://github.com/pop-os/cosmic-text/issues/406
- Issue #446 (COLRv1) — https://github.com/pop-os/cosmic-text/issues/446
- Issue #485 (Hangul Jamo) — https://github.com/pop-os/cosmic-text/issues/485
- Issue #493 (pixel font 1px offset) — https://github.com/pop-os/cosmic-text/issues/493
- Issue #499 (fontconfig aliases) — https://github.com/pop-os/cosmic-text/issues/499
- Issue #505 (FontSystem::new slow) — https://github.com/pop-os/cosmic-text/issues/505
- Buiy text spec — `docs/specs/2026-05-07-buiy-foundation/text.md`
- swash COLRv1 status — https://github.com/dfrg/swash (4+-year-open issue noted in #446 reporter's analysis)
- winit IME events — https://docs.rs/winit/latest/winit/event/enum.Ime.html
