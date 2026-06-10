# Buiy text — font assets, registration, and discovery

**Parent:** [README.md](README.md)

This file owns the **`@font-face` analogue**: how font bytes enter a Buiy app
(the `BuiyFont` asset + loader), how they register into the shared
`FontSystem`'s fontdb and unregister without leaking, the embedded
deterministic default font, system-font discovery, the `font-family` stack
resolver with weight/style matching, and the registration descriptors
(`unicode-range`, `font-display`). It maps the foundation rows
[text.md § 3.4](../2026-05-07-buiy-foundation/text.md#34-typography):
`font-family` stack + fallback (**F**, :10), `font-size` incl. keywords
(**F**, :12), `font-weight` (**F**, :13), and the full Font-registration row —
source, format, unicode-range, font-display (**F**, :28).

Boundaries with siblings: `FontSystem` ownership, scheduling, and damage flow
are [architecture.md](architecture.md)'s (this file *uses* the
pinned `SharedFontSystem` and emits the font-damage event that file routes);
shaping consumes this file's resolved `Attrs` spans
([measure-and-layout.md](measure-and-layout.md)); rasterization and
`AtlasKey` construction are [glyph-pipeline.md](glyph-pipeline.md)'s. Nothing
here touches the render-side seam — already built and GPU-verified
([render § atlas-and-text-seam](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md),
`crates/buiy_core/src/render/atlas/types.rs:12-23`,
`crates/buiy_core/src/render/prepare.rs:46-51`).

---

## 1. Dependency contract

**Decision: one direct dependency — `cosmic-text = "0.19"`, default features
only; all fontdb types are consumed through cosmic-text's crate-root
`pub use fontdb` re-export.** Not yet declared in `Cargo.toml` at spec time
(grep of `Cargo.toml` + `Cargo.lock` returns no cosmic-text/fontdb/swash;
`bevy_text` is not compiled — Bevy 0.18 is pinned `default-features = false`
with no text feature, `Cargo.toml:45` — so there is no transitive cosmic-text,
no second `FontSystem`, and no Bevy `default_font` in the binary). `bevy_asset`
**is** enabled, which is what § 2's loader builds on. `cargo deny check`
precedes the dependency bump (CLAUDE.md supply-chain gate).

The pin must be **Buiy's own, never transitive**: bevy_cosmic_edit's archive
post-mortem names riding bevy_text's transitive cosmic-text pin as a cause of
death ([prior-art/bevy-cosmic-edit/lessons.md](../../prior-art/bevy-cosmic-edit/lessons.md)).
The `shape-run-cache` feature stays **OFF** in v1 (decided, review round 1):
the retained `TextBuffer`'s per-line caches already amortize re-shaping, while
the run cache grows `FontSystem`-side without bound —
[architecture.md § 7](architecture.md) is the decision record, including the
rejected ON-with-trim runner-up (the critiques row "most embedders should turn
it on" is that runner-up's evidence, superseded by the damage-discipline
argument). Buiy stays on cosmic-text although Bevy 0.19 moved to
Parley — the documented bet in
[prior-art/cosmic-text/README.md](../../prior-art/cosmic-text/README.md).
Pre-1.0 churn is a named risk (the rustybuzz→harfrust swap landed inside
0.15.0): pin `"0.19"` and review the upstream PR queue before any bump.

> **Runner-up rejected: a direct `fontdb` dependency alongside, version-synced**
> (the `guillotiere` treatment, [render § 2.1](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#21-type-and-placement)).
> The situations differ in exactly one verified fact: `guillotiere` is *not*
> re-exported by `bevy_image`, while `fontdb` *is* re-exported at cosmic-text
> 0.19's crate root (`pub use fontdb;` — and `cosmic_text::Family`/`Weight`/
> `Style`/`Stretch` are themselves fontdb re-exports, source-verified against
> fontdb 0.23). A second pin would *add* version-drift risk instead of removing
> it. Version identity is guaranteed by construction. **F**

## 2. The `BuiyFont` asset and loader

**Decision: a dedicated `BuiyFont` asset + `BuiyFontLoader` `AssetLoader`,
plus a thin embedded-bytes escape hatch.** The foundation row pins registration
to the "Bevy asset-pipeline equivalent of `@font-face`"
([text.md:28](../2026-05-07-buiy-foundation/text.md)); asset-pipeline load
states are exactly the substrate `font-display` (§ 7) needs, and hot-reload
comes free.

```rust
#[derive(Asset, TypePath)]
pub struct BuiyFont {
    /// Raw sfnt bytes (ttf/otf/ttc/otc). Arc so registration hands fontdb a
    /// zero-copy `Source::Binary(Arc<dyn AsRef<[u8]> + Send + Sync>)`.
    pub data: Arc<Vec<u8>>,
}

/// AssetLoader: extensions() = ["ttf", "otf", "ttc", "otc"].
pub struct BuiyFontLoader;

/// Escape hatch for include_bytes! fonts (the § 4 embedded default uses it):
/// registers without an asset path, same FontRegistry row as loaded assets.
pub fn register_font_bytes(registry: &mut FontRegistry, family_hint: Option<&str>,
                           bytes: Arc<Vec<u8>>, descriptors: FontFaceDescriptors) -> FontKey;
```

**v1 formats: ttf/otf/ttc/otc — exactly fontdb's native set** (verified:
fontdb 0.23 "Will load ttf, otf, ttc and otc fonts"; no WOFF/WOFF2). The
loader's invariant is **loader-output-is-always-sfnt**: whatever the loader
accepts, the bytes handed to fontdb are sfnt. That invariant *is* the named
woff2 seam — adding woff2 later means a magic-byte sniff + decompression
pre-pass inside the loader, touching neither the registry nor the
`FontSystem`. **C (seam named, § 9)**

> **Runner-up rejected: woff2 in v1.** fontdb cannot consume woff2 directly,
> so v1 support means adopting a brotli/woff2 decompression crate through
> `cargo deny` for a format desktop apps rarely ship. Zero F-tier need; the
> sfnt invariant makes the deferral free.
>
> **Runner-ups rejected for the asset type:** registration-API-only (no Bevy
> asset) loses load states — making `font-display` unimplementable — plus
> hot-reload, and diverges from the foundation row. Reusing `bevy_text::Font`
> is not even possible: bevy_text is not compiled (`Cargo.toml:45`), and
> depending on it violates the parallel-stack mandate (§ 1). **F**

Hot-reload: `AssetEvent::Modified` is handled as **remove + re-add** (§ 3.1's
two paths composed), so edited font files on disk re-register cleanly during
development.

## 3. `FontRegistry` — the `FontFaceSet` analogue

**Decision: a main-world `FontRegistry` resource holding STRONG handles, with
explicit register/unregister — the CSS Font Loading API (`FontFaceSet`) model,
not the Bevy drop-unload idiom.**

```rust
#[derive(Resource, Default)]
pub struct FontRegistry {
    /// family name → registered faces + descriptors + load state.
    families: HashMap<String, FamilyRecord>,
}

struct FamilyRecord {
    faces: Vec<cosmic_text::fontdb::ID>,   // filled on registration into the db
    descriptors: FontFaceDescriptors,      // unicode_range, font_display, …
    load_state: FontLoadState,             // Loading | Loaded | Failed
    handle: Handle<BuiyFont>,              // STRONG — pins the asset alive
}

pub struct FontFaceDescriptors {
    pub unicode_range: Option<UnicodeRanges>,  // § 6.1
    pub font_display: FontDisplay,             // § 7; default Swap
}
```

Rationale: Buiy text references fonts by **family name** (the CSS model), not
by `Handle` — no component naturally owns the handle. With a weak registry,
dropping the last handle silently unregisters the face and visibly re-fallbacks
text mid-frame. Strong-held registration until explicit `unregister_family`
matches `@font-face` document-lifetime semantics. **F**

> **Runner-up rejected: weak registry driven by `AssetEvent::Removed`** (the
> Bevy idiom). The silent-fallback footgun outweighs idiom-consistency.
> `Removed` is still *handled* — as a forced unregister (§ 3.1) — so a
> deliberate asset unload stays correct; it just is not the intended API.

The registration system reacts to `AssetEvent<BuiyFont>::Added`: lock the
pinned `SharedFontSystem` ([architecture.md](architecture.md);
main-world systems lock around mutations), call
`font_system.db_mut().load_font_source(Source::Binary(arc))` — returns
`TinyVec<[ID; 8]>` (collections register every face) — record the IDs and
descriptors, flip `load_state`, and emit the **font-damage event** that
[architecture.md](architecture.md) routes into a global reshape.
Source-verified: `db_mut()` clears **only** `font_matches_cache`
(`pub fn db_mut(&mut self) -> &mut fontdb::Database { self.font_matches_cache.clear(); &mut self.db }`),
so in-place addition is safe — a *new* face can never make a cache entry stale.
Registration runs in the main world before the Layout step of the BuiySet
chain, so a frame never measures against a half-registered family.

### 3.1 Unregistration rebuilds the `FontSystem`

**Decision: face addition is in-place; face removal REBUILDS the `FontSystem`
via the `into_locale_and_db` round-trip.**

Source-verified rationale: cosmic-text 0.19's
`font_cache: HashMap<(fontdb::ID, fontdb::Weight), Option<Arc<Font>>>` has **no
public purge API**. After an in-place `db_mut().remove_face(id)`, the removed
face's `Arc<Font>` (full font data) is retained forever — a leak that grows
across hot-reload cycles — *and* `get_font(id, weight)` can still serve the
removed face to any un-reshaped buffer — a correctness hazard. The rebuild path
sidesteps both:

```rust
// Under ONE SharedFontSystem lock hold, so no other world ever observes
// the placeholder (architecture.md owns the lock discipline):
let old = mem::replace(&mut *guard, placeholder_font_system());
let (locale, mut db) = old.into_locale_and_db();
db.remove_face(id);                                   // for each unregistered face
*guard = FontSystem::new_with_locale_and_db_and_fallback(locale, db, BuiyFallback);
```

`new_with_locale_and_db*` does **no** filesystem scan (the issue-#505 cost
lives in `new`/`load_system_fonts`), so the rebuild is cheap; and losing a font
forces a global reshape anyway, so the mandatory font-damage broadcast costs
nothing extra. **F**

> **Runner-up rejected: in-place `db_mut().remove_face(id)` only.** Loses on
> the unpurgeable `font_cache` (leak + stale `get_font` hits). It wins only if
> upstream adds a cache-purge API — worth filing early (single-maintainer
> project; the governance bus-factor row applies,
> [prior-art/cosmic-text/governance.md](../../prior-art/cosmic-text/governance.md)).

### 3.2 fontdb ID instability — the `AtlasKey` consequence

fontdb `ID`s are **per-Database**: every rebuild (§ 3.1 unload, § 5 system-scan
swap) issues fresh IDs for every face, so every `AtlasKey` derived from
`(FontId, subpixel_bucket, glyph_id, px_size)`
(`crates/buiy_core/src/render/atlas/types.rs:12-23`) goes stale *at once*.
Bounded, not broken: keys are content-addressed and the atlas re-inserts on
miss ([render § 2.4](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#24-lru-eviction--the-gate-15-contract)),
so the cost is a one-frame rasterization storm on rare events. Two hard rules
fall out: **an `AtlasKey` is never persisted across a rebuild** (the glyph
producer rebuilds keys from live IDs every emission —
[glyph-pipeline.md](glyph-pipeline.md)), and the rebuild storm is a load-test
target ([verification.md](verification.md)).

## 4. The embedded deterministic default font

**Decision: embed one OFL-licensed sans subset — Fira Sans regular, latin
subset — behind a default-on `default_font` cargo feature, and pin the generic
families at construction.** `include_bytes!` → `Source::Binary` in
`FontSystem::new_with_fonts(..)`, then
`db.set_sans_serif_family("Fira Sans")` (+ serif/monospace pins to the same
embedded face until dedicated defaults justify their bytes), so
`Family::SansSerif` resolves **identically on every host**.

This deliberately overrides cosmic-text's verified built-in defaults — `new()`
sets "Open Sans" / "DejaVu Serif" / "Noto Sans Mono" — which *dangle* when
system fonts are off (§ 5): they name fonts that are simply not in the
database. Goldens and the GPU-readback harness
([render § verification](../2026-06-03-buiy-render-pipeline-design/verification.md))
need byte-identical text on hosts with zero installed fonts; an empty fontdb
renders nothing at all. The embedded face is also what the atlas warmup
contract pre-rasterizes ("the ASCII range of the default theme font",
[render § 2.3](../2026-06-03-buiy-render-pipeline-design/atlas-and-text-seam.md#23-warmup)). **F**

> **Runner-up rejected: FiraMono** (Bevy's exact embed choice — the precedent
> that validates the pattern). Loses only on typographic fit: a UI toolkit's
> default should be a text sans, not a code mono.
>
> **Runner-up rejected: no embedded font** (apps must register one). Every
> test fixture and every hello-world would have to ship a font, and the
> generic-family defaults would point at fonts that don't exist.

A latin subset cannot drive bidi/CJK/emoji fixtures — per-script OFL test
fonts ship as repo *test fixtures* (not embedded in the library), coordinated
in [verification.md](verification.md).

**Provenance and licensing (normative).** The subset is produced by a pinned
`pyftsubset` (fonttools) invocation committed as a script at
`tools/fonts/subset_default_font.sh` (input: the upstream Fira Sans release
artifact; flags: the latin unicode ranges plus the layout features shaping
needs), and the resulting artifact is committed at
`crates/buiy_core/assets/fonts/FiraSans-Regular-latin.ttf` — regenerated only
by re-running the script, never edited by hand, so the embedded bytes are
reproducible and the goldens' shaping baseline is auditable. Fira Sans is
OFL-1.1: the license ships alongside the artifact
(`crates/buiy_core/assets/fonts/OFL-FiraSans.txt`), is included in the
published crate package, and the subset retains the font's name and copyright
records as the OFL requires. **F**

## 5. System-font discovery — opt-in, background, swap

**Decision: system-font discovery is OFF by default.** Enabling it
(`BuiyTextPlugin { system_fonts: true }`) spawns an `AsyncComputeTaskPool`
task that builds a *fresh* `fontdb::Database` — `load_system_fonts()` plus
re-adding every registered `Source::Binary` (Arc clones, cheap) — then the
main thread swaps it in via the § 3.1 rebuild pattern
(`new_with_locale_and_db_and_fallback`) and emits global font damage. First
paint **never** waits on the scan; text shaped before the swap re-shapes once
after it (one reshape, by design — the swap is a § 3.2 rebuild event).

Rationale: cosmic-text issue #505 measured `FontSystem::new` at 1.344 s
release (84 % mmapping every face under `/usr/share/fonts`; 10 s+ debug). The
prior-art lesson commits Buiy to opt-in + background warmup
([prior-art/cosmic-text/lessons.md](../../prior-art/cosmic-text/lessons.md)
§ "Trusting FontSystem::new to be fast"), and golden determinism requires CI
frames to never depend on host fonts. **F**

> **Runner-up rejected: on-by-default at startup** (Bevy 0.15's headline
> behavior). Startup cost on the common path, non-deterministic goldens, and
> it relitigates a documented prior-art mitigation.
>
> **Runner-up rejected: lazy scan on first fallback miss.** Turns the first
> tofu glyph into a 1.3 s frame hitch — the worst possible placement.

Loud doc note for Linux users: with explicit stacks (§ 6) and opt-in system
fonts, **fontconfig aliases are deliberately not honored** (issue #499:
`Family::Name("monospace")` hits cosmic-text's hardcoded list, not the user's
fontconfig). Buiy's answer is explicit, deterministic chains — the lessons.md
"Fontconfig aliases" row.

## 6. Family-stack resolution and face matching

Two verified API facts shape everything here: `Attrs<'a>.family` is a
**single** `Family<'a>` (no stack), and the `Fallback` trait's methods return
`&[&'static str]` (constructor-injected, global to the `FontSystem`).
Per-node dynamic stacks therefore cannot live inside cosmic-text.

**Decision: Buiy owns stack resolution, above cosmic-text.**

```rust
/// The font-family stack value (text.md:10, F). Ordered; first match wins.
pub struct FontStack(pub Vec<FamilyEntry>);
pub enum FamilyEntry { Named(String), Generic(GenericFamily) }
```

The resolver runs per text run, before `Buffer`/`Attrs` construction
([glyph-pipeline.md](glyph-pipeline.md) consumes its output): walk the stack;
for each entry, consult `FontRegistry` (load state § 7, unicode-range § 6.1)
and `fontdb::Database::query(&Query { families, weight, stretch, style })` —
the verified CSS-property-mapped matcher with a *prioritized* families slice —
for installed faces. The first family covering the run's codepoints wins;
coverage splits produce multiple `AttrsList` spans, each carrying a **concrete
`Family::Name`** plus `Weight`/`Style`/`Stretch`. Below the stack, cosmic-text's
per-glyph `FontFallbackIter` (script lists + whole-db sweep) remains the
last-resort safety net — it runs only when the author's entire stack missed.
**F**

> **Runner-up rejected: a custom `Fallback` impl carrying the stack.** Loses
> on both verified constraints: `&'static str` returns would force leaking
> per-node strings, and the `Fallback` is injected once at construction —
> global, not per-run. It *is* still used once, app-globally: `BuiyFallback`
> (§ 3.1, § 5) pins deterministic last-resort script lists for CI instead of
> the platform-varying `PlatformFallback`.
>
> **Runner-up rejected: first-family-only + cosmic-text fallback.** Stack
> *order* is the F-tier semantic itself; cosmic-text's fallback order is the
> platform's list, not the author's stack.

Weight matching rides the committed surface end-to-end: `Attrs.weight` →
`fontdb::Query.weight` → `get_font(id: fontdb::ID, weight: Weight)`, and the
`font_cache` is keyed `(ID, Weight)` (both verified) — so **variable-font
weight already works** with no extra machinery. Style/stretch pass through
`Query` as-is; synthetic italic/oblique (`font-synthesis`) is **C** and out of
v1 (text.md:14, :21).

`GenericFamily` resolves through fontdb's `set_*_family` pins (§ 4) — the
deterministic five (`serif`/`sans-serif`/`monospace`/`cursive`/`fantasy`) map
directly; the extended set (text.md:11, **C**) and token-driven rebinding are
the theme seam (§ 9).

### 6.1 `unicode-range` — honored in the resolver

**Decision: `unicode-range` is enforced by the Buiy resolver as a
per-codepoint face filter.** fontdb has no unicode-range concept (verified:
neither queried `FaceInfo` nor `Query` carries ranges), so the declared ranges
live in `FontFaceDescriptors` and the § 6 stack walk skips a registered family
for codepoints outside its range — naturally producing the per-character
matching CSS specifies, via the same span-splitting mechanism coverage misses
already use. Cost is gated: families with **no** declared range skip the check
entirely, so typing latency on range-free apps pays nothing. **F**

> **Runner-up rejected: parse-but-advisory.** Silently violates an F-tier
> commitment (text.md:28). Acceptable only as an interim phase-1 sub-step if
> the resolver lands before span-splitting does (until then, range mismatches
> resolve whole-run).
>
> **Runner-up rejected: patching fontdb.** Upstream scope mismatch (fontdb
> matches *faces*, not codepoints) + fork burden.

## 7. `font-display` — the loading policy

**Decision: a per-registration `FontDisplay { Block, Swap, Fallback, Optional }`
descriptor; v1 implements `Swap` (the default) and `Block`; `Fallback`/`Optional`
are C-tier reserved.** cosmic-text has no progressive-loading notion
("embedders that load fonts asynchronously do so above cosmic-text" —
[prior-art/cosmic-text/critiques.md](../../prior-art/cosmic-text/critiques.md)
§ web font loading), so this is pure registry + paint machinery on hooks Buiy
already has:

- **`Swap` (default):** text whose stack references a `Loading` family renders
  immediately with the next resolved family; the font-damage event on load
  completion triggers the reshape — the FOUT analogue the prior-art names as
  the canonical embedder pattern. **F**
- **`Block`:** identical fallback *layout* (so geometry never jumps twice),
  but the affected runs' `GlyphAlphaInstance`s are emitted with **zero alpha**
  — the per-instance straight-alpha color slot,
  `crates/buiy_core/src/render/atlas/primitive.rs:30-47` — until load or a
  configurable timeout (web default: 3 s), then swap. No new render machinery.
  **F**

> **Runner-up rejected: web-classic block-then-swap as the default.**
> Invisible text on slow asset IO is the worst desktop failure mode, and it
> breaks golden determinism (a golden taken inside the block window differs
> from one taken after). Swap-by-default is also the prior-art-named pattern.
>
> **Runner-up rejected: deferring the whole field.** It is F-tier (text.md:28).

## 8. Authoring surface — the phase-1 component slice

The F-tier rows this file owns surface as three components (consumed by the
shaping side, [glyph-pipeline.md](glyph-pipeline.md) /
[measure-and-layout.md](measure-and-layout.md)):

```rust
#[derive(Component)] pub struct FontFamily(pub FontStack);     // text.md:10  F
#[derive(Component)] pub struct FontSize(pub f32);             // text.md:12  F  (px; cosmic-text Metrics are unit-agnostic px)
#[derive(Component)] pub struct FontWeight(pub u16);           // text.md:13  F  (→ cosmic_text::Weight)
```

`font-size` keywords (`small`/`medium`/`large`/…) are a constant px table in
v1. Plugin-level defaults (`BuiyTextPlugin`'s default stack/size/weight) cover
unset components; v1 components carry **explicit** stacks — token indirection
is the § 9 theme seam.

## 9. Deferred seams — named, not designed

- **woff2** — loader decompression pre-pass behind the § 2 sfnt invariant. **C**
- **`font-variation-settings`** (axes beyond weight) — blocked on upstream
  #406 (`ital`/`wdth`/`slnt`/`opsz` patchy, custom axes unexposed); weight-only
  is committed (§ 6). The lessons.md row: "reduce ambition until #406
  resolves." **C** (text.md:18, :26)
- **`font-style` synthesis** — `Style` passes through `Query` in v1; synthetic
  oblique / `font-synthesis` deferred. **C** (text.md:14, :21)
- **Metric overrides** (`size-adjust`, `ascent/descent/line-gap-override`) —
  `FontFaceDescriptors` grows fields; consumed by the measure seam. **C**
  (text.md:29)
- **Family-alias overrides** — re-registering a face under an author-chosen
  family name via `fontdb::Database::push_face_info` (verified API). **C**
- **Theme font tokens** — `buiy-theme-tokens-design` (foundation
  [README.md:88](../2026-05-07-buiy-foundation/README.md), unwritten) supplies
  token→`FontStack` indirection and generic-family→`set_*_family` rebinding.
  This file defines the `FontStack` value type and the resolver entry point it
  will target; building token plumbing now would invent the token model in the
  wrong document. **C**
- **Asset-pipeline split** — foundation
  [README.md:98](../2026-05-07-buiy-foundation/README.md) assigns font-asset
  GC and atlas-warmup *strategy* to `buiy-asset-pipeline-design`; this file
  owns registration *semantics* and hands that spec the `FontRegistry`/
  `BuiyFont` substrate.
- **Color-font tables (COLR/CBDT)** — rasterization-side; see
  [glyph-pipeline.md](glyph-pipeline.md) (upstream #446).

## 10. Verification

Headless (the default CI gate, no adapter): register→resolve→query round-trip
(registered family wins over generic for covered codepoints);
unregister-rebuild leaves no stale `get_font` hit and no `font_cache` growth
across N hot-reload cycles (the § 3.1 leak assertion); `font-display`
transitions (Loading→Loaded fires exactly one font-damage event; Block emits
zero-alpha then swaps); unicode-range span-splitting; determinism — two
`FontSystem` constructions on a zero-system-font host resolve every § 8
default identically. GPU lane (`#[ignore]`, real adapter): the § 4 embedded
font is the golden baseline; the § 3.2 rebuild storm is bounded (one frame of
misses, atlas page count returns to baseline). Fixture fonts and tolerances:
[verification.md](verification.md).

## Open questions

1. **Engine-ownership pin vs. this area's exploration recommendation.** This
   area's blueprint recommended a plain main-world `BuiyFontSystem(FontSystem)`
   `Resource` (FontSystem is verified Send+Sync in 0.19 — the "non-Sync"
   wording in [prior-art/cosmic-text/critiques.md](../../prior-art/cosmic-text/critiques.md)
   § FontSystem lifetime is STALE, describing the `&mut`-only API, not the
   marker traits) and explicitly rejected `Arc<Mutex<FontSystem>>` as "hiding
   the serialization instead of scheduling it"; it also placed `SwashCache` in
   the main world. The decided engine ownership (area 0,
   [architecture.md](architecture.md)) is
   `SharedFontSystem(Arc<Mutex<FontSystem>>)` cloned into the RenderApp, with
   `SwashCache` as a render-world resource. **This file is written against the
   pinned decision** (§ 3 paths lock `SharedFontSystem`; the § 3.1 rebuild
   swaps in place under one lock hold), recording the contradiction rather
   than silently dropping it: revisiting the lock later costs § 3 only
   mechanical `lock()`→`ResMut` edits, no semantic changes.

## Sources

- docs.rs/cosmic-text/0.19.0 — `FontSystem` (constructors, `db`/`db_mut`,
  `get_font(ID, Weight)`, `into_locale_and_db`; auto-traits `impl Send`/`impl
  Sync`); crate root (`pub use fontdb`); `struct.Attrs` (single `family`);
  `trait.Fallback` (`&[&'static str]` returns); `src/font/system.rs` (`db_mut`
  clears only `font_matches_cache`; `font_cache` has no purge API; `new()`
  defaults Open Sans / DejaVu Serif / Noto Sans Mono)
- docs.rs/fontdb/0.23.0 — `struct.Database` (`load_font_source` →
  `TinyVec<[ID; 8]>`, `load_system_fonts`, `remove_face`, `push_face_info`,
  `set_*_family`; formats "ttf, otf, ttc and otc"), `struct.Query`
  (prioritized `families: &[Family]`, weight/stretch/style)
- github.com/pop-os/cosmic-text — issues #505 (FontSystem::new 1.344 s scan),
  #499 (fontconfig aliases), #406 (variable axes), #446 (COLRv1)

---

*Scope: the `BuiyFont` asset + loader, `FontRegistry` (strong handles,
in-place add, rebuild-on-remove), the embedded deterministic default font +
generic-family pins, opt-in background system-font discovery, the Buiy-owned
`FontStack` resolver with weight/style/stretch matching and `unicode-range`
filtering, and `font-display` Swap/Block. F-tier except where marked; woff2,
variable axes beyond weight, metric overrides, synthesis, and theme tokens are
named C-tier seams.*
