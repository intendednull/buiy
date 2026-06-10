# Buiy Text T1: Engine Foundation Implementation Plan

**Date:** 2026-06-09
**Status:** landed
**Spec:** [specs/2026-06-09-buiy-text-rendering-design/architecture.md](../specs/2026-06-09-buiy-text-rendering-design/architecture.md) §§ 1–2, 7 + [font-assets.md](../specs/2026-06-09-buiy-text-rendering-design/font-assets.md) §§ 1, 4–5
**Campaign:** [plans/2026-06-09-buiy-text-campaign.md](2026-06-09-buiy-text-campaign.md) — phase T1

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the cosmic-text engine foundation: the `cosmic-text = "0.19"` dependency (default features only — `shape-run-cache` OFF), the `buiy_core::text` module, the embedded deterministic default font (Fira Sans Regular latin subset, OFL-1.1, behind a default-on `default_font` feature) with its provenance script, `SharedFontSystem(Arc<Mutex<FontSystem>>)` constructed registered-fonts-only with pinned generic families + the deterministic `BuiyFallback`, the render-world `BuiySwashCache` resource, and the opt-in background system-font scan with the rebuild swap + `FontsGeneration` reshape trigger.

**Architecture:** One `FontSystem` behind `Arc<Mutex<_>>` (verified `Send + Sync` in 0.19), inserted into the main `App` by a new `BuiyTextPlugin` and Arc-cloned into the `RenderApp` when one exists (architecture § 1.1). Construction is `FontSystem::new_with_locale_and_db_and_fallback` over a fontdb `Database` pre-loaded with ONLY the embedded default font — no system scan at startup (architecture § 2.1; `FontSystem::new()` and `new_with_fonts()` both mmap every system font — issue #505, source-verified). System fonts are opt-in, scanned off-thread, swapped in via the font-assets § 3.1 rebuild pattern under one lock hold, bumping `FontsGeneration` exactly once. `SwashCache` is a render-world-only resource used uncached-only — the atlas is the one bitmap cache (architecture § 1.3, pre-phase decision 3).

**Tech Stack:** cosmic-text 0.19 (+ its `pub use fontdb` re-export of fontdb 0.23), sys-locale 0.3 + unicode-script 0.5 (version-synced to cosmic-text's own pins — see Decision 2/3 below), Bevy 0.18 (`AsyncComputeTaskPool`, `SubApp`), pyftsubset (fonttools 4.56.0) for the one-time font subset.

**Test reality:** T1 is **headless-only** (campaign charter: "no adapter anywhere"). Every test in this plan runs on `MinimalPlugins` with no `RenderApp` and no wgpu adapter. The render-world half is tested against a bare `bevy::app::SubApp`; the live-`RenderApp` wiring is first exercised on the GPU lane in T4.

---

## The gate (keep green at every commit)

**Gate per task:** headless `cargo test --workspace -j 2` + fmt + clippy --workspace --all-targets -D warnings + doc; `cargo deny check` on the dep task. NO GPU lane needed (T1 is headless-only).

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  xvfb-run -a cargo test --workspace -j 2
```

(`xvfb-run -a` is for the pre-existing windowed tests on Linux, per CLAUDE.md; the new T1 tests themselves never need a display.)

---

## Orientation: verified facts this plan builds on

All cosmic-text/fontdb facts below were re-verified against docs.rs **0.19.0** / **0.23.0** on 2026-06-09 (the spec's version-pinning policy, architecture § 7). Do not re-derive from the prior-art folder — it is stale on `Sync` (architecture Open question 1).

| Fact | Verified shape |
|---|---|
| `FontSystem` auto-traits | `Send + Sync` (docs.rs auto-trait impls) — `Arc<Mutex<_>>` is sound, no `NonSend` |
| Registered-only constructors | `new_with_locale_and_db(locale: String, db: Database) -> Self`; `new_with_locale_and_db_and_fallback(locale: String, db: Database, impl_fallback: impl Fallback + 'static) -> Self` — **neither scans the filesystem** |
| The scan trap | `FontSystem::new()` **and** `new_with_fonts(..)` both call `db.load_system_fonts()` (source-verified `src/font/system.rs` `load_fonts`) — never call either |
| Locale plumbing | `FontSystem::locale(&self) -> &str`; `into_locale_and_db(self) -> (String, Database)`; cosmic-text's own `get_locale()` is **private**: `sys_locale::get_locale().unwrap_or("en-US")` |
| `Fallback` trait | `common_fallback(&self) -> &[&'static str]`; `forbidden_fallback(&self) -> &[&'static str]`; `script_fallback(&self, script: Script, locale: &str) -> &[&'static str]` — `Script` is `unicode_script::Script`, **not re-exported** by cosmic-text (root re-exports are `fontdb`, `skrifa`, `harfrust` only) |
| `SwashCache` | `Send + Sync`; `new() -> Self`; `get_image_uncached(&mut self, &mut FontSystem, CacheKey) -> Option<SwashImage>` |
| cosmic-text 0.19 features | `default = ["std", "swash", "fontconfig"]`; `std` enables `sys-locale` (pinned 0.3.2) + `fontdb/memmap`; `shape-run-cache` is opt-in (stays OFF) |
| fontdb 0.23 `Database` | `new()`, `len()`, `load_font_source(Source) -> TinyVec<[ID; 8]>`, `load_system_fonts()`, `set_{serif,sans_serif,cursive,fantasy,monospace}_family(impl Into<String>)`, `query(&Query) -> Option<ID>`, `face(ID) -> Option<&FaceInfo>` (`FaceInfo.families: Vec<(String, Language)>`, `.post_script_name: String`); `Source::Binary(Arc<dyn AsRef<[u8]> + Sync + Send>)`; `Query { families: &[Family], weight, stretch, style }` implements `Default` |
| Upstream font | `https://raw.githubusercontent.com/mozilla/Fira/4.202/ttf/FiraSans-Regular.ttf` — HTTP 200, **403 924 bytes**, sha256 `a389cef71891df1232370fcebd7cfde5f74e741967070399adc91fd069b2094b`; tag `4.202` = commit `48a8d0a0354e933c0d1cfcf9feb07ccb00eb6fa9`; `LICENSE` at the same tag is OFL-1.1 with Reserved Font Name "Fira" |

Codebase shapes consumed (read before editing, confirm current):

- `crates/buiy_core/src/lib.rs` — module list (`a11y` … `theme`), root re-exports, `BuiySet` enum, `CorePlugin`. **Gains `pub mod text;` + re-exports.**
- `crates/buiy_core/Cargo.toml` — no `[features]` section today; deps end at `bitflags = "2.11.1"`. **Gains three deps + the `default_font` feature.**
- `crates/buiy/src/lib.rs` — `BuiyPlugin::build` plugin tuple (core → theme → a11y → focus → layout → picking ×2 → widgets → render). **Gains `BuiyTextPlugin` before `WidgetsPlugin`** (foundation § 2.8 order: … input → text → widgets).
- `crates/buiy_core/src/render/atlas/mod.rs` `register(render_app: &mut SubApp)` — the precedent for a render-world registration function called from a plugin's `RenderApp` branch. T1's `register_render_world` mirrors it.
- `crates/buiy_core/tests/layout_pipeline_order.rs` — the headless `App::new()` + `MinimalPlugins` test idiom (MinimalPlugins includes `TaskPoolPlugin`, which initializes `AsyncComputeTaskPool`).

## Decisions this plan encodes (resolved against the spec — do not relitigate)

1. **Constructor:** `new_with_locale_and_db_and_fallback`, not bare `new_with_locale_and_db`. The campaign charter's "built via `new_with_locale_and_db`" is shorthand: the `Fallback` is constructor-injected and global (font-assets § 6), and the deterministic `BuiyFallback` is a T1 charter deliverable, so the `_and_fallback` variant is the only way to honor both. font-assets § 3.1's rebuild snippet already uses it. (font-assets § 4's `new_with_fonts(..)` mention is unusable — that constructor scans system fonts, source-verified; architecture § 2.1's registered-only pin governs.)
2. **Locale:** architecture § 2.1 pins "Locale via the `sys-locale` default feature", but cosmic-text keeps its `get_locale()` private and the registered-only constructors take an explicit `String`. Buiy therefore mirrors cosmic-text's exact behavior with a direct `sys-locale = "0.3"` dependency: `sys_locale::get_locale().unwrap_or_else(|| String::from("en-US"))`. Runner-up rejected: hard-coding `"en-US"` — contradicts the spec line; determinism is unaffected either way at T1 (`BuiyFallback` ignores its locale argument until T5 grows script lists).
3. **`unicode-script = "0.5"` direct dependency:** implementing `Fallback::script_fallback` requires *naming* `unicode_script::Script`, which cosmic-text does not re-export. Version-synced to cosmic-text 0.19's own pin (0.5.8); cargo unifies to one copy, and a future upstream bump to 0.6 surfaces as a loud type-mismatch compile error. (font-assets § 1's "one direct dependency" cannot hold literally once `BuiyFallback` is built — recorded here, flagged for the spec's next edit pass in Task 6.)
4. **Owning plugin:** `BuiyTextPlugin { system_fonts: bool }` (named in font-assets §§ 5, 8). It owns both worlds itself: main-world insert in `build`, plus the `RenderApp` branch guard (`get_sub_app_mut(RenderApp)` → no-op headless) — the same convention as `BuiyRenderPlugin`. `BuiyPlugin` adds it in `build` after `DefaultPlugins`, so the `RenderApp` already exists in real apps (render architecture § 1.1 finish-ordering seam, as built).
5. **RenderApp clone lands in T1.** Architecture § 1.1 ties the clone to "render-side text registration" without naming a phase; the charter puts the render-world `SwashCache` resource in T1, so the registration function exists in T1 and inserts both (the clone in Task 3, `BuiySwashCache` in Task 4). Its consumer (`extract_buiy_glyphs`, lock site #3) arrives in T4.
6. **All five generic families pin to the embedded face.** font-assets § 4 names sans-serif + serif/monospace and § 6 says "the deterministic five … map directly" through § 4's pins; cursive/fantasy are included so no generic ever dangles (the exact failure § 4 criticizes in cosmic-text's built-in defaults).
7. **Subset ranges:** font-assets § 4 pins "the latin unicode ranges plus the layout features shaping needs" without listing them. This plan pins the common latin web-subset set: `U+0000-00FF, U+0131, U+0152-0153, U+2013-2014, U+2018-201A, U+201C-201E, U+2026, U+2039-203A`, and layout features `ccmp,kern,liga,clig,calt,locl,mark,mkmk`. Recorded in the script — the single source of truth.

## File structure

```
crates/buiy_core/
├── Cargo.toml                      # +cosmic-text, +sys-locale, +unicode-script, +[features] default_font
├── assets/fonts/
│   ├── FiraSans-Regular-latin.ttf  # generated artifact — only ever written by the script
│   └── OFL-FiraSans.txt            # upstream OFL-1.1 license, shipped + packaged alongside
├── src/
│   ├── lib.rs                      # +pub mod text; +root re-exports
│   └── text/
│       ├── mod.rs                  # module doc, BuiyTextPlugin, register_render_world
│       ├── font_system.rs          # SharedFontSystem, BuiyFallback, FontsGeneration,
│       │                           #   DEFAULT_FONT_FAMILY, embed, registered_fonts_db,
│       │                           #   build_font_system, buiy_locale
│       ├── swash.rs                # BuiySwashCache
│       └── system_scan.rs          # PendingSystemFontScan, spawn/apply systems, swap_font_db
└── tests/
    ├── text_engine.rs              # dep smoke, plugin insertion, determinism, SubApp registration
    ├── text_default_font.rs        # artifact parse + family-resolution smoke
    └── text_system_scan.rs         # injected-swap generation bump, default-off, flag-spawns
crates/buiy/src/lib.rs              # +BuiyTextPlugin in the BuiyPlugin tuple + re-export
tools/fonts/subset_default_font.sh  # provenance script (pinned URL/sha/flags/fonttools)
```

---

## Task 1 — `cosmic-text = "0.19"` dependency + `buiy_core::text` skeleton

**Files:**
- Modify: `crates/buiy_core/Cargo.toml`
- Modify: `crates/buiy_core/src/lib.rs`
- Create: `crates/buiy_core/src/text/mod.rs`
- Create: `crates/buiy_core/tests/text_engine.rs`

- [ ] **Step 1: Flip the campaign row.** In `docs/plans/2026-06-09-buiy-text-campaign.md`, Phase status table: `| T1 | Engine foundation | proposed |` → `| T1 | Engine foundation | in progress |`. In this plan's header: `**Status:** proposed` → `**Status:** active`.

- [ ] **Step 2: Write the failing test** — create `crates/buiy_core/tests/text_engine.rs`:

```rust
//! Headless tests for the `buiy_core::text` engine foundation (T1).
//!
//! Spec: docs/specs/2026-06-09-buiy-text-rendering-design/architecture.md
//! §§ 1–2. No wgpu adapter, no RenderApp — T1 is headless-only.

use cosmic_text::FontSystem;
use cosmic_text::fontdb::Database;

/// The dependency smoke: cosmic-text links, and the registered-only
/// constructor (architecture § 2.1) builds without touching system fonts.
#[test]
fn cosmic_text_constructs_a_registered_only_font_system() {
    let font_system = FontSystem::new_with_locale_and_db(String::from("en-US"), Database::new());
    assert_eq!(font_system.locale(), "en-US");
    assert_eq!(
        font_system.db().len(),
        0,
        "new_with_locale_and_db must not scan system fonts"
    );
}
```

- [ ] **Step 3: Run it, expect compile FAIL** — `cargo test -p buiy_core --test text_engine` → error: unresolved module or unlinked crate `cosmic_text`.

- [ ] **Step 4: Add the dependency** in `crates/buiy_core/Cargo.toml` (declared here directly, like `bitflags` — buiy_core is its only consumer; the spec mandates the pin be Buiy's own, never transitive):

```toml
# Text engine (buiy-text-rendering-design, architecture § 7): Buiy's OWN pin,
# never ridden transitively (the bevy_cosmic_edit decay path). DEFAULT
# FEATURES ONLY — `shape-run-cache` stays OFF (decided in spec review round 1;
# architecture § 7 is the decision record): the retained TextBuffer's
# per-line caches already amortize re-shaping, while the run cache grows
# FontSystem-side without bound (gate #15). All fontdb types are consumed
# through cosmic-text's crate-root `pub use fontdb` re-export — no second
# fontdb pin (font-assets § 1). Every version bump re-verifies the
# load-bearing API facts against docs.rs (architecture § 7) and re-runs
# `cargo deny check` first (CLAUDE.md supply-chain gate).
cosmic-text = "0.19"
```

- [ ] **Step 5: Supply-chain gate.** Run `cargo deny check` (the campaign pre-verified this clean before T1 started; re-run in-task — the lockfile changes here). Expected: PASS. If it newly fails, STOP and resolve before proceeding (no `deny.toml` exceptions without orchestrator sign-off).

- [ ] **Step 6: Module skeleton.** Create `crates/buiy_core/src/text/mod.rs`:

```rust
//! Buiy text engine: cosmic-text ownership and lifecycle.
//!
//! Spec: `docs/specs/2026-06-09-buiy-text-rendering-design/` — this module is
//! the T1 engine foundation (architecture.md §§ 1–2; font-assets.md §§ 1,
//! 4–5): the shared `FontSystem` resource, the embedded deterministic default
//! font + `BuiyFallback`, the render-world swash cache, and the opt-in
//! background system-font scan with the `FontsGeneration` reshape trigger.
//!
//! Later phases fill the module out: `TextBuffer` + the `TextSync`/
//! `TextCommit` layout steps (T2–T3), the `extract_buiy_glyphs` producer
//! (T4), font assets + the `FontStack` resolver + fallback correctness (T5).
//! Campaign: `docs/plans/2026-06-09-buiy-text-campaign.md`.
```

In `crates/buiy_core/src/lib.rs`, add the module between `render` and `theme`:

```rust
pub mod text;
```

- [ ] **Step 7: Run the test, expect PASS** — `cargo test -p buiy_core --test text_engine` → 1 passed.

- [ ] **Step 8: Confirm the feature stays off** — `cargo tree -p buiy_core -f "{p} {f}" | grep cosmic-text` → feature list contains `std,swash,fontconfig` (transitively `sys-locale`), and **not** `shape-run-cache`.

- [ ] **Step 9: Run GATE. Commit:** `feat(text): cosmic-text 0.19 dep (default features, shape-run-cache OFF) + buiy_core::text skeleton`

---

## Task 2 — The embedded default font artifact + provenance script

The subset is produced by a pinned `pyftsubset` invocation committed as a script; the artifact is committed and only ever regenerated by re-running the script — never edited by hand (font-assets § 4, normative).

**Files:**
- Create: `tools/fonts/subset_default_font.sh`
- Create (generated): `crates/buiy_core/assets/fonts/FiraSans-Regular-latin.ttf`
- Create (downloaded): `crates/buiy_core/assets/fonts/OFL-FiraSans.txt`
- Create: `crates/buiy_core/tests/text_default_font.rs`

- [ ] **Step 1: Write the failing tests** — create `crates/buiy_core/tests/text_default_font.rs`:

```rust
//! The embedded deterministic default font artifact (font-assets § 4).
//!
//! These tests read the committed artifact directly (no cargo feature
//! involved): it must parse via fontdb, register exactly one face under the
//! family name the OFL name records carry, and resolve through a
//! generic-family pin inside a registered-only `FontSystem`.

use std::sync::Arc;

use cosmic_text::FontSystem;
use cosmic_text::fontdb::{Database, Family, Query, Source};

static EMBEDDED: &[u8] = include_bytes!("../assets/fonts/FiraSans-Regular-latin.ttf");

#[test]
fn artifact_parses_as_a_single_fira_sans_face() {
    let mut db = Database::new();
    let ids = db.load_font_source(Source::Binary(Arc::new(EMBEDDED)));
    assert_eq!(ids.len(), 1, "the subset is a single-face ttf");
    let face = db.face(ids[0]).expect("face is registered");
    assert!(
        face.families.iter().any(|(name, _)| name == "Fira Sans"),
        "subset retains the family name (OFL name records); got {:?}",
        face.families
    );
}

#[test]
fn font_system_with_artifact_resolves_a_pinned_generic_family() {
    let mut db = Database::new();
    db.load_font_source(Source::Binary(Arc::new(EMBEDDED)));
    db.set_sans_serif_family("Fira Sans");
    let font_system = FontSystem::new_with_locale_and_db(String::from("en-US"), db);
    let resolved = font_system.db().query(&Query {
        families: &[Family::SansSerif],
        ..Query::default()
    });
    assert!(
        resolved.is_some(),
        "Family::SansSerif resolves through the pin to the embedded face"
    );
}
```

- [ ] **Step 2: Run, expect compile FAIL** — `cargo test -p buiy_core --test text_default_font` → `include_bytes!` error: couldn't read `assets/fonts/FiraSans-Regular-latin.ttf`.

- [ ] **Step 3: Write the provenance script** — create `tools/fonts/subset_default_font.sh` (then `chmod +x` it):

```bash
#!/usr/bin/env bash
# Provenance script for Buiy's embedded deterministic default font
# (docs/specs/2026-06-09-buiy-text-rendering-design/font-assets.md § 4,
# normative). Regenerating the artifact is ONLY done by re-running this
# script — the committed ttf is never edited by hand, so the embedded bytes
# are reproducible and the goldens' shaping baseline is auditable.
#
# Requirements:
#   - curl, sha256sum
#   - pyftsubset from fonttools, pinned: python3 -m pip install fonttools==4.56.0
#     (fontTools' subset module does not rewrite head.modified by default, so
#     the output is deterministic for a given input + fonttools version.)
set -euo pipefail

# --- pins -------------------------------------------------------------------
# Upstream: the mozilla/Fira foundry repo, release tag 4.202
# (commit 48a8d0a0354e933c0d1cfcf9feb07ccb00eb6fa9). Verified 2026-06-09.
UPSTREAM_FONT_URL="https://raw.githubusercontent.com/mozilla/Fira/4.202/ttf/FiraSans-Regular.ttf"
UPSTREAM_FONT_SHA256="a389cef71891df1232370fcebd7cfde5f74e741967070399adc91fd069b2094b"
UPSTREAM_LICENSE_URL="https://raw.githubusercontent.com/mozilla/Fira/4.202/LICENSE"
FONTTOOLS_PIN="4.56.0"

# The latin web-subset ranges (plan decision 7; font-assets § 4 pins "the
# latin unicode ranges" without enumerating — this list is the enumeration):
LATIN_UNICODES="U+0000-00FF,U+0131,U+0152-0153,U+2013-2014,U+2018-201A,U+201C-201E,U+2026,U+2039-203A"
# The layout features shaping needs (kerning, ligatures, composition, marks):
LAYOUT_FEATURES="ccmp,kern,liga,clig,calt,locl,mark,mkmk"

# --- paths ------------------------------------------------------------------
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="$REPO_ROOT/crates/buiy_core/assets/fonts"
OUT_FONT="$OUT_DIR/FiraSans-Regular-latin.ttf"
OUT_LICENSE="$OUT_DIR/OFL-FiraSans.txt"

# --- preflight ---------------------------------------------------------------
command -v pyftsubset >/dev/null || {
    echo "error: pyftsubset not found — python3 -m pip install fonttools==$FONTTOOLS_PIN" >&2
    exit 1
}
python3 - "$FONTTOOLS_PIN" <<'EOF'
import sys
import fontTools
want = sys.argv[1]
if fontTools.version != want:
    sys.exit(f"error: fontTools {fontTools.version} found, {want} required "
             f"(python3 -m pip install fonttools=={want})")
EOF

# --- fetch + verify ----------------------------------------------------------
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT
curl -fsSL "$UPSTREAM_FONT_URL" -o "$TMP_DIR/FiraSans-Regular.ttf"
echo "$UPSTREAM_FONT_SHA256  $TMP_DIR/FiraSans-Regular.ttf" | sha256sum -c -
curl -fsSL "$UPSTREAM_LICENSE_URL" -o "$TMP_DIR/LICENSE"
grep -q "SIL Open Font License, Version 1.1" "$TMP_DIR/LICENSE"

# --- subset -------------------------------------------------------------------
mkdir -p "$OUT_DIR"
# --name-IDs='*' / --name-languages='*' / --name-legacy keep the full name
# table — the OFL requires the copyright + license records survive the subset
# (font-assets § 4). --notdef-outline keeps a visible tofu box.
pyftsubset "$TMP_DIR/FiraSans-Regular.ttf" \
    --output-file="$OUT_FONT" \
    --unicodes="$LATIN_UNICODES" \
    --layout-features="$LAYOUT_FEATURES" \
    --name-IDs='*' \
    --name-languages='*' \
    --name-legacy \
    --notdef-outline
cp "$TMP_DIR/LICENSE" "$OUT_LICENSE"

echo "wrote $OUT_FONT ($(stat -c%s "$OUT_FONT") bytes)"
echo "sha256: $(sha256sum "$OUT_FONT" | cut -d' ' -f1)"
echo "wrote $OUT_LICENSE"
```

- [ ] **Step 4: Run the script once** — `tools/fonts/subset_default_font.sh` (network + `pip install fonttools==4.56.0` available). Expected: sha256 check passes, artifact + license written, size well under the 403 924-byte input (a latin subset of Fira Sans Regular is roughly 20–50 KB).
  - **Fallback (verify, don't guess):** if the download 404s or the sha mismatches, STOP — re-verify the tag and path via `gh api repos/mozilla/Fira/git/refs/tags/4.202` and the repo tree; if mozilla/Fira has moved, switch to the google/fonts OFL copy (`ofl/firasans/FiraSans-Regular.ttf` in `github.com/google/fonts`), pin the exact commit you fetch from, and update **both** URL and sha pins in the script before committing. Never commit an artifact whose input sha is unrecorded.

- [ ] **Step 5: Run the tests, expect PASS** — `cargo test -p buiy_core --test text_default_font` → 2 passed. If the family-name assertion fails, read the actual `face.families` from the panic message — do not loosen the assertion without understanding why the name records changed.

- [ ] **Step 6: Packaging sanity** — `crates/buiy_core/Cargo.toml` has no `include`/`exclude` keys, so `cargo package` ships `assets/` (artifact + `OFL-FiraSans.txt`) by default, satisfying font-assets § 4's "included in the published crate package". Confirm no `exclude` was added since; no change expected.

- [ ] **Step 7: Run GATE. Commit** (artifact + license + script + test together): `feat(text): embedded deterministic default font (Fira Sans latin subset, OFL-1.1, provenance script)`

---

## Task 3 — `SharedFontSystem` + `BuiyFallback` + `BuiyTextPlugin`

**Files:**
- Create: `crates/buiy_core/src/text/font_system.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`
- Modify: `crates/buiy_core/Cargo.toml` (two deps + the feature)
- Modify: `crates/buiy_core/src/lib.rs` (re-exports)
- Modify: `crates/buiy/src/lib.rs` (`BuiyPlugin` tuple + re-export)
- Test: `crates/buiy_core/tests/text_engine.rs`

- [ ] **Step 1: Write the failing tests** — append to `crates/buiy_core/tests/text_engine.rs`:

```rust
use bevy::app::SubApp;
use bevy::prelude::*;
use buiy_core::text::{BuiyTextPlugin, SharedFontSystem, register_render_world};
use cosmic_text::fontdb::{Family, Query};

fn text_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(BuiyTextPlugin::default());
    app
}

/// architecture § 2.1: SharedFontSystem exists and is lockable, with ONLY
/// registered fonts resident — no system scan at startup.
#[test]
fn plugin_inserts_lockable_registered_only_font_system() {
    let app = text_app();
    let fonts = app.world().resource::<SharedFontSystem>();
    let guard = fonts.lock();
    assert!(!guard.locale().is_empty(), "locale is pinned at construction");
    #[cfg(feature = "default_font")]
    assert_eq!(
        guard.db().len(),
        1,
        "exactly the embedded default font — a system scan would add hundreds"
    );
}

/// architecture § 1.1: one FontSystem, two worlds — the render-world resource
/// is an Arc clone of the main-world one, never a second engine
/// (fontdb IDs are stable only within one engine).
#[test]
fn render_world_registration_shares_one_engine() {
    let fonts = SharedFontSystem::default();
    let mut render_app = SubApp::new();
    register_render_world(&mut render_app, &fonts);
    let clone = render_app.world().resource::<SharedFontSystem>();
    assert!(
        std::sync::Arc::ptr_eq(&fonts.0, &clone.0),
        "render world must hold a clone of the SAME Arc"
    );
}

/// The campaign charter's determinism test: two constructions on a
/// zero-system-font baseline resolve every default family identically.
/// (The construction path never scans, so this holds on ANY host.)
#[cfg(feature = "default_font")]
#[test]
fn two_constructions_resolve_every_default_family_identically() {
    fn resolved_faces(app: &App) -> Vec<Option<String>> {
        let fonts = app.world().resource::<SharedFontSystem>();
        let guard = fonts.lock();
        let db = guard.db();
        [
            Family::SansSerif,
            Family::Serif,
            Family::Monospace,
            Family::Cursive,
            Family::Fantasy,
            Family::Name("Fira Sans"),
        ]
        .iter()
        .map(|family| {
            db.query(&Query {
                families: std::slice::from_ref(family),
                ..Query::default()
            })
            .and_then(|id| db.face(id))
            .map(|face| face.post_script_name.clone())
        })
        .collect()
    }

    let (app_a, app_b) = (text_app(), text_app());
    let (resolved_a, resolved_b) = (resolved_faces(&app_a), resolved_faces(&app_b));
    assert_eq!(resolved_a, resolved_b, "construction must be deterministic");
    for resolution in &resolved_a {
        assert!(
            resolution.is_some(),
            "every pinned generic family + the named family must resolve; got {resolved_a:?}"
        );
    }
}
```

Note for the implementer: `register_render_world` only inserts the `SharedFontSystem` clone in this task; Task 4 extends it (and this test file) with `BuiySwashCache`.

- [ ] **Step 2: Run, expect compile FAIL** — `cargo test -p buiy_core --test text_engine` → unresolved imports `buiy_core::text::{BuiyTextPlugin, SharedFontSystem, register_render_world}`.

- [ ] **Step 3: Add the two support deps + the feature** to `crates/buiy_core/Cargo.toml`:

```toml
# The locale for FontSystem::new_with_locale_and_db_and_fallback
# (architecture § 2.1 "Locale via the sys-locale default feature"):
# cosmic-text keeps its own sys_locale call private, so Buiy mirrors it
# exactly (get_locale().unwrap_or "en-US"). Version-synced to cosmic-text
# 0.19's own pin (0.3.2) — cargo unifies to one copy.
sys-locale = "0.3"
# Names the `Script` parameter in cosmic_text::Fallback::script_fallback —
# cosmic-text re-exports fontdb/skrifa/harfrust but NOT unicode-script.
# Version-synced to cosmic-text 0.19's pin (0.5.8); an upstream bump to 0.6
# surfaces here as a loud type-mismatch compile error, by design.
unicode-script = "0.5"
```

and (new section):

```toml
[features]
default = ["default_font"]
# Embed the deterministic default font (Fira Sans Regular latin subset,
# OFL-1.1 — font-assets § 4) and pin all five generic families to it, so
# Family::SansSerif etc. resolve identically on every host. Disable to ship
# zero font bytes — the app must then register fonts before any text renders.
default_font = []
```

- [ ] **Step 4: Supply-chain gate again** — `cargo deny check` (two new direct deps; both already in the resolved tree via cosmic-text, so this should be a no-op PASS).

- [ ] **Step 5: Implement** — create `crates/buiy_core/src/text/font_system.rs`:

```rust
//! The shared cosmic-text engine: `SharedFontSystem`, its registered-only
//! construction over the embedded default font, and the deterministic
//! `BuiyFallback`.
//!
//! Spec: architecture.md §§ 1.1–1.2, 2.1; font-assets.md §§ 4–5.

use std::sync::{Arc, Mutex, MutexGuard};

use bevy::prelude::*;
use cosmic_text::{Fallback, FontSystem, fontdb};
use unicode_script::Script;

/// Family name the embedded default font registers under, and the target of
/// all five generic-family pins (font-assets § 4).
pub const DEFAULT_FONT_FAMILY: &str = "Fira Sans";

/// The embedded deterministic default font: Fira Sans Regular, latin subset,
/// OFL-1.1. Generated ONLY by `tools/fonts/subset_default_font.sh` (which
/// pins the upstream artifact, the subset ranges, and the fonttools version);
/// the license ships alongside at `assets/fonts/OFL-FiraSans.txt`.
#[cfg(feature = "default_font")]
static DEFAULT_FONT_BYTES: &[u8] =
    include_bytes!("../../assets/fonts/FiraSans-Regular-latin.ttf");

/// The one cosmic-text `FontSystem`, shared across the main and render worlds
/// (architecture § 1.1). `FontSystem` is verified `Send + Sync` in 0.19, so
/// `Arc<Mutex<_>>` is sound — no `NonSend` pinning. Exactly three sites may
/// lock it (architecture § 1.2): the Taffy measure closure (T3), `TextCommit`
/// shaping (T3), and the glyph producer's atlas-miss closure (T4). Reviewers
/// reject a fourth.
#[derive(Resource, Clone)]
pub struct SharedFontSystem(pub Arc<Mutex<FontSystem>>);

impl SharedFontSystem {
    /// Build the engine: registered fonts only, never a system scan
    /// (architecture § 2.1; `FontSystem::new()`/`new_with_fonts()` both mmap
    /// every system font — issue #505).
    pub fn new() -> Self {
        Self(Arc::new(Mutex::new(build_font_system())))
    }

    /// Lock the engine. Panics if a previous holder panicked mid-mutation —
    /// a poisoned `FontSystem` has no safe recovery (fail loud, root-cause).
    pub fn lock(&self) -> MutexGuard<'_, FontSystem> {
        self.0
            .lock()
            .expect("SharedFontSystem poisoned: a text system panicked while holding the lock")
    }
}

impl Default for SharedFontSystem {
    fn default() -> Self {
        Self::new()
    }
}

/// The deterministic app-global last-resort fallback (font-assets §§ 3.1, 5–6):
/// injected at construction in place of cosmic-text's platform-varying
/// `PlatformFallback`, so CI resolution never depends on the host platform.
/// T1 ships the minimal deterministic lists; T5 grows per-script entries
/// alongside the per-script OFL fixture fonts (seam named, not built).
pub struct BuiyFallback;

impl Fallback for BuiyFallback {
    fn common_fallback(&self) -> &[&'static str] {
        // Harmless when `default_font` is off: a fallback name absent from
        // the database is simply skipped.
        &[DEFAULT_FONT_FAMILY]
    }

    fn forbidden_fallback(&self) -> &[&'static str] {
        &[]
    }

    fn script_fallback(&self, _script: Script, _locale: &str) -> &[&'static str] {
        // Deliberately empty (never platform-dependent) until T5 pins
        // deterministic per-script lists.
        &[]
    }
}

/// The font-set generation counter (architecture § 2.2): bumped exactly once
/// per font-set change (system-scan swap completion here; runtime asset
/// registration joins in T5). `TextSync`'s trigger set consumes it from T2 —
/// every `TextBuffer` reshapes once against the enriched fallback set, so
/// late fonts never leave stale tofu.
#[derive(Resource, Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct FontsGeneration(pub u64);

/// The registered-fonts-only database: the embedded default font plus the
/// five generic-family pins (font-assets § 4). This is BOTH the construction
/// baseline and the base every system-scan rebuild re-adds before scanning
/// (font-assets § 5). T5's `FontRegistry` extends it with every registered
/// `Source::Binary`.
pub fn registered_fonts_db() -> fontdb::Database {
    #[allow(unused_mut)] // `mut` is unused only when `default_font` is off.
    let mut db = fontdb::Database::new();
    #[cfg(feature = "default_font")]
    {
        let ids = db.load_font_source(fontdb::Source::Binary(Arc::new(DEFAULT_FONT_BYTES)));
        debug_assert_eq!(ids.len(), 1, "the embedded subset is a single-face ttf");
        // Pin ALL FIVE generic families to the embedded face so no generic
        // ever dangles (cosmic-text's built-in defaults name fonts that are
        // simply not in a registered-only database — font-assets § 4).
        db.set_sans_serif_family(DEFAULT_FONT_FAMILY);
        db.set_serif_family(DEFAULT_FONT_FAMILY);
        db.set_monospace_family(DEFAULT_FONT_FAMILY);
        db.set_cursive_family(DEFAULT_FONT_FAMILY);
        db.set_fantasy_family(DEFAULT_FONT_FAMILY);
    }
    db
}

pub(crate) fn build_font_system() -> FontSystem {
    FontSystem::new_with_locale_and_db_and_fallback(
        buiy_locale(),
        registered_fonts_db(),
        BuiyFallback,
    )
}

/// The locale handed to the registered-only constructor: the system locale
/// via sys-locale, mirroring cosmic-text's own (private) behavior exactly
/// (architecture § 2.1 "Locale via the sys-locale default feature").
fn buiy_locale() -> String {
    sys_locale::get_locale().unwrap_or_else(|| String::from("en-US"))
}
```

- [ ] **Step 6: Implement the plugin** — replace the body of `crates/buiy_core/src/text/mod.rs` (keep the Task-1 module doc, then):

```rust
mod font_system;
mod system_scan; // created in Task 5; for THIS task leave this line out
mod swash; // created in Task 4; for THIS task leave this line out

pub use font_system::{
    BuiyFallback, DEFAULT_FONT_FAMILY, FontsGeneration, SharedFontSystem, registered_fonts_db,
};

use bevy::app::SubApp;
use bevy::prelude::*;
use bevy::render::RenderApp;

/// Registers the text engine in both worlds (architecture §§ 1–2).
///
/// Add AFTER Bevy's `RenderPlugin` (i.e. after `DefaultPlugins`) — like
/// `BuiyRenderPlugin`, the render-world half is guarded on a live `RenderApp`
/// and silently no-ops headless (the CI gate has no adapter).
#[derive(Default)]
pub struct BuiyTextPlugin {
    /// Opt-in background system-font scan (font-assets § 5). OFF by default:
    /// startup never pays the issue-#505 scan cost, and golden determinism
    /// never depends on host fonts. When enabled, the scan runs on
    /// `AsyncComputeTaskPool`, swaps in under one lock hold, and bumps
    /// `FontsGeneration` exactly once.
    pub system_fonts: bool,
}

impl Plugin for BuiyTextPlugin {
    fn build(&self, app: &mut App) {
        let fonts = SharedFontSystem::new();
        app.insert_resource(fonts.clone());
        app.init_resource::<FontsGeneration>();

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            register_render_world(render_app, &fonts);
        }
    }
}

/// The render-world half of text registration (mirrors `atlas::register`):
/// the `SharedFontSystem` Arc clone — one engine, two worlds (architecture
/// § 1.1; fontdb IDs are stable only within one engine, so a second instance
/// would mis-key every glyph). Public so the headless `SubApp` registration
/// test (and any external render setup) can drive it without a live
/// `RenderApp`; the live wiring is exercised on the GPU lane from T4.
pub fn register_render_world(render_app: &mut SubApp, fonts: &SharedFontSystem) {
    render_app.insert_resource(fonts.clone());
}
```

(The two `mod` lines marked "leave this line out" are shown so the final shape is unambiguous; this task compiles without them.)

- [ ] **Step 7: Wire the crate roots.** In `crates/buiy_core/src/lib.rs`, add to the re-export block:

```rust
pub use text::{BuiyTextPlugin, FontsGeneration, SharedFontSystem};
```

In `crates/buiy/src/lib.rs`: add `text::{BuiyTextPlugin, FontsGeneration, SharedFontSystem},` to the `pub use buiy_core::{...}` block, and insert into the `BuiyPlugin::build` tuple between `buiy_core::picking::BuiyPickingBackendPlugin` and `WidgetsPlugin` (foundation § 2.8 order: … → text → widgets), with a comment:

```rust
            // Text engine foundation (buiy-text-rendering-design T1): the
            // shared FontSystem + the FontsGeneration reshape trigger.
            // System-font scan stays opt-in/off in the composed default.
            buiy_core::text::BuiyTextPlugin::default(),
```

- [ ] **Step 8: Run the tests, expect PASS** — `cargo test -p buiy_core --test text_engine` → 4 passed.

- [ ] **Step 9: Feature-off check** — `cargo check -p buiy_core --no-default-features` → compiles (the embed and the pins are cfg-gated; nothing else may depend on the feature).

- [ ] **Step 10: Run GATE. Commit:** `feat(text): SharedFontSystem + BuiyFallback + BuiyTextPlugin (registered-only construction, default_font embed)`

---

## Task 4 — Render-world `BuiySwashCache` resource

**Files:**
- Create: `crates/buiy_core/src/text/swash.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`
- Test: `crates/buiy_core/tests/text_engine.rs`

- [ ] **Step 1: Write the failing test** — append to `crates/buiy_core/tests/text_engine.rs` (extend the existing imports with `BuiySwashCache`):

```rust
/// architecture § 1.3: the swash cache is a plain render-world resource,
/// registered alongside the SharedFontSystem clone. Its consumer (the glyph
/// producer's uncached-only miss path, lock site #3) arrives in T4.
#[test]
fn render_world_registration_inserts_swash_cache() {
    let fonts = SharedFontSystem::default();
    let mut render_app = SubApp::new();
    register_render_world(&mut render_app, &fonts);
    assert!(
        render_app
            .world()
            .get_resource::<buiy_core::text::BuiySwashCache>()
            .is_some(),
        "BuiySwashCache must be registered with the render-world text half"
    );
}
```

- [ ] **Step 2: Run, expect compile FAIL** — `cargo test -p buiy_core --test text_engine` → no `BuiySwashCache` in `buiy_core::text`.

- [ ] **Step 3: Implement** — create `crates/buiy_core/src/text/swash.rs`:

```rust
//! The render-world rasterization context (architecture § 1.3).

use bevy::prelude::*;

/// Render-world-only wrapper around `cosmic_text::SwashCache` (verified
/// `Send + Sync`), kept SOLELY for API access to its internal scale context.
///
/// **Uncached-only contract (architecture § 1.3, adjudicated):** Buiy
/// rasterizes exclusively via `SwashCache::get_image_uncached` — the caching
/// path (`get_image`) is never called, `image_cache` stays empty by
/// construction, and the content-addressed, LRU-bounded `BuiyAtlas` is the
/// ONE bitmap cache (gate #15: no second cache, no trim machinery).
///
/// `ResMut` only in the glyph producer's atlas-miss path (T4, lock site #3);
/// it lives outside the `SharedFontSystem` mutex so a main-world shape pass
/// never serializes against the raster cache.
#[derive(Resource)]
pub struct BuiySwashCache(pub cosmic_text::SwashCache);

impl Default for BuiySwashCache {
    fn default() -> Self {
        Self(cosmic_text::SwashCache::new())
    }
}
```

In `crates/buiy_core/src/text/mod.rs`: add `mod swash;` + `pub use swash::BuiySwashCache;`, and extend `register_render_world`:

```rust
pub fn register_render_world(render_app: &mut SubApp, fonts: &SharedFontSystem) {
    render_app.insert_resource(fonts.clone());
    render_app.init_resource::<BuiySwashCache>();
}
```

- [ ] **Step 4: Run the tests, expect PASS** — `cargo test -p buiy_core --test text_engine` → 5 passed.

- [ ] **Step 5: Run GATE. Commit:** `feat(text): render-world BuiySwashCache (uncached-only raster context)`

---

## Task 5 — Opt-in background system-font scan + `FontsGeneration` bump

**Files:**
- Create: `crates/buiy_core/src/text/system_scan.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`
- Create: `crates/buiy_core/tests/text_system_scan.rs`

- [ ] **Step 1: Write the failing tests** — create `crates/buiy_core/tests/text_system_scan.rs`:

```rust
//! The opt-in background system-font scan (font-assets § 5) and the
//! `FontsGeneration` reshape trigger (architecture § 2.2).
//!
//! The swap machinery is tested with an INJECTED completed task carrying a
//! deterministic database — never a real `load_system_fonts()` driven to
//! completion in-test: the real scan's duration is host-font-dependent
//! (issue #505: ~1.3 s release / 10 s+ debug on font-heavy hosts) and golden
//! determinism forbids CI frames depending on host fonts anyway.

use std::time::Duration;

use bevy::prelude::*;
use bevy::tasks::AsyncComputeTaskPool;
use buiy_core::text::{
    BuiyTextPlugin, FontsGeneration, PendingSystemFontScan, SharedFontSystem, registered_fonts_db,
};

fn text_app(system_fonts: bool) -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins); // TaskPoolPlugin inits AsyncComputeTaskPool
    app.add_plugins(BuiyTextPlugin { system_fonts });
    app
}

/// Drive updates until the generation bumps (condition-based waiting, bounded).
fn wait_for_generation(app: &mut App, want: u64) -> bool {
    for _ in 0..2000 {
        app.update();
        if app.world().resource::<FontsGeneration>().0 == want {
            return true;
        }
        std::thread::sleep(Duration::from_millis(1));
    }
    false
}

/// The charter test: a completed scan bumps `FontsGeneration` EXACTLY once.
#[test]
fn injected_scan_swap_bumps_generation_exactly_once() {
    let mut app = text_app(false);
    assert_eq!(app.world().resource::<FontsGeneration>().0, 0);

    // Deterministic stand-in for the scan result: the registered baseline
    // itself (what a scan on a zero-system-font host would produce).
    let task = AsyncComputeTaskPool::get().spawn(async move { registered_fonts_db() });
    app.world_mut()
        .insert_resource(PendingSystemFontScan(Some(task)));

    assert!(
        wait_for_generation(&mut app, 1),
        "completed scan must bump FontsGeneration"
    );
    assert!(
        app.world().get_resource::<PendingSystemFontScan>().is_none(),
        "the scan slot is consumed by the swap"
    );
    for _ in 0..10 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<FontsGeneration>().0,
        1,
        "exactly once — no repeat bumps after the swap"
    );
}

/// The swap preserves the locale and the registered baseline: post-swap, the
/// pinned families still resolve exactly as before (font-assets § 5: the
/// fresh db re-adds every registered binary before scanning).
#[cfg(feature = "default_font")]
#[test]
fn swap_preserves_locale_and_registered_baseline() {
    use cosmic_text::fontdb::{Family, Query};

    let mut app = text_app(false);
    let locale_before = {
        let fonts = app.world().resource::<SharedFontSystem>();
        let guard = fonts.lock();
        guard.locale().to_owned()
    };

    let task = AsyncComputeTaskPool::get().spawn(async move { registered_fonts_db() });
    app.world_mut()
        .insert_resource(PendingSystemFontScan(Some(task)));
    assert!(wait_for_generation(&mut app, 1));

    let fonts = app.world().resource::<SharedFontSystem>();
    let guard = fonts.lock();
    assert_eq!(guard.locale(), locale_before, "the swap carries the locale through");
    let resolved = guard.db().query(&Query {
        families: &[Family::SansSerif],
        ..Query::default()
    });
    assert!(resolved.is_some(), "sans-serif still resolves post-swap");
}

/// OFF by default: no scan slot, no bump, ever (font-assets § 5).
#[test]
fn default_plugin_never_scans_or_bumps() {
    let mut app = text_app(false);
    for _ in 0..5 {
        app.update();
    }
    assert!(app.world().get_resource::<PendingSystemFontScan>().is_none());
    assert_eq!(app.world().resource::<FontsGeneration>().0, 0);
}

/// The opt-in flag spawns the background scan. Poll-agnostic on purpose: by
/// the time we look, the task is either still pending or already applied —
/// we never WAIT on the real host scan (see module doc).
#[test]
fn system_fonts_flag_spawns_the_background_scan() {
    let mut app = text_app(true);
    app.update(); // Startup schedule runs the spawn
    let pending = app.world().get_resource::<PendingSystemFontScan>().is_some();
    let generation = app.world().resource::<FontsGeneration>().0;
    assert!(
        pending || generation == 1,
        "the flag must kick off the scan (pending={pending}, generation={generation})"
    );
}
```

- [ ] **Step 2: Run, expect compile FAIL** — `cargo test -p buiy_core --test text_system_scan` → no `PendingSystemFontScan` in `buiy_core::text`.

- [ ] **Step 3: Implement** — create `crates/buiy_core/src/text/system_scan.rs`:

```rust
//! Opt-in background system-font discovery (font-assets § 5) and the
//! rebuild swap (font-assets § 3.1) that merges it in.

use std::mem;

use bevy::prelude::*;
use bevy::tasks::{AsyncComputeTaskPool, Task, block_on, futures_lite::future};
use cosmic_text::{FontSystem, fontdb};

use super::font_system::{BuiyFallback, FontsGeneration, SharedFontSystem, registered_fonts_db};

/// The in-flight system-font scan, if any. Inserted by
/// [`spawn_system_font_scan`] (the `BuiyTextPlugin { system_fonts: true }`
/// startup path) and consumed by [`apply_system_font_scan`]. Public so tests
/// (and advanced apps) can inject a prebuilt database task; absent in the
/// steady state — the poll system is inert without it.
#[derive(Resource)]
pub struct PendingSystemFontScan(pub Option<Task<fontdb::Database>>);

/// Spawn the scan on `AsyncComputeTaskPool`: a FRESH database built from the
/// registered baseline (the embedded default font + family pins; T5's
/// `FontRegistry` adds every registered `Source::Binary`) plus
/// `load_system_fonts()`. First paint never waits on this — the issue-#505
/// cost (~1.3 s release) stays off the startup path.
pub fn spawn_system_font_scan(mut commands: Commands) {
    let task = AsyncComputeTaskPool::get().spawn(async move {
        let mut db = registered_fonts_db();
        db.load_system_fonts();
        db
    });
    commands.insert_resource(PendingSystemFontScan(Some(task)));
}

/// Poll the pending scan; on completion, swap the rebuilt database in and
/// bump [`FontsGeneration`] EXACTLY once (architecture § 2.2 — the reshape
/// trigger `TextSync` consumes from T2). Runs before `BuiySet::Layout` so a
/// completed swap is visible to the same frame's layout pass.
pub fn apply_system_font_scan(
    pending: Option<ResMut<PendingSystemFontScan>>,
    fonts: Res<SharedFontSystem>,
    mut generation: ResMut<FontsGeneration>,
    mut commands: Commands,
) {
    let Some(mut pending) = pending else { return };
    let Some(task) = pending.0.as_mut() else { return };
    let Some(db) = block_on(future::poll_once(&mut *task)) else {
        return;
    };
    // A finished Task must not be polled again: clear the slot immediately
    // (the remove_resource command applies later, at the next sync point).
    pending.0 = None;
    commands.remove_resource::<PendingSystemFontScan>();

    swap_font_db(&fonts, db);
    generation.0 += 1; // exactly one bump per completed scan
}

/// The font-assets § 3.1 rebuild pattern, under ONE lock hold so no other
/// world ever observes the placeholder: carry the locale through
/// `into_locale_and_db`, rebuild over the new database with the same
/// deterministic [`BuiyFallback`]. `new_with_locale_and_db*` does no
/// filesystem scan, so the swap itself is cheap. Every fontdb ID is
/// reissued by a rebuild — `AtlasKey`s are never persisted across one
/// (font-assets § 3.2; enforced by the T4 producer, not here).
pub fn swap_font_db(fonts: &SharedFontSystem, db: fontdb::Database) {
    let mut guard = fonts.lock();
    let old = mem::replace(&mut *guard, placeholder_font_system());
    let (locale, _discarded_db) = old.into_locale_and_db();
    *guard = FontSystem::new_with_locale_and_db_and_fallback(locale, db, BuiyFallback);
}

/// Briefly parked in the mutex during [`swap_font_db`]'s `mem::replace`;
/// never observable (the swap completes under the same lock hold).
fn placeholder_font_system() -> FontSystem {
    FontSystem::new_with_locale_and_db(String::from("en-US"), fontdb::Database::new())
}
```

- [ ] **Step 4: Wire the plugin** — in `crates/buiy_core/src/text/mod.rs`: add `mod system_scan;` + `pub use system_scan::{PendingSystemFontScan, apply_system_font_scan, spawn_system_font_scan, swap_font_db};`, and extend `BuiyTextPlugin::build` (after `init_resource::<FontsGeneration>()`):

```rust
        // The poll/swap system is registered UNCONDITIONALLY: it is inert
        // without a PendingSystemFontScan resource (zero steady-state cost),
        // and tests / apps may inject a scan task without the startup flag.
        // Before Layout, so a completed swap (and its FontsGeneration bump)
        // is visible to the same frame's TextSync (T2+).
        app.add_systems(
            Update,
            apply_system_font_scan.before(crate::BuiySet::Layout),
        );
        if self.system_fonts {
            app.add_systems(Startup, spawn_system_font_scan);
        }
```

- [ ] **Step 5: Run the tests, expect PASS** — `cargo test -p buiy_core --test text_system_scan` → 4 passed. Also re-run `--test text_engine` (5 passed — the plugin change must not disturb it).

- [ ] **Step 6: Run GATE. Commit:** `feat(text): opt-in background system-font scan + FontsGeneration reshape trigger`

---

## Task 6 — Docs-with-change

**Files:**
- Modify: `docs/plans/2026-06-09-buiy-text-campaign.md`
- Modify: `docs/README.md`
- Modify: `docs/plans/2026-06-09-buiy-text-t1-engine-foundation.md` (this file)

- [ ] **Step 1: Campaign flip.** In `docs/plans/2026-06-09-buiy-text-campaign.md`, Phase status table: `| T1 | Engine foundation | in progress |` → `| T1 | Engine foundation | landed |`.

- [ ] **Step 2: Docs index row.** In `docs/README.md` § Text → **Plans**, after the campaign row, add:

```markdown
- [Buiy text T1 — engine foundation](plans/2026-06-09-buiy-text-t1-engine-foundation.md) — cosmic-text 0.19 dep (default features only; `shape-run-cache` OFF), `buiy_core::text` module, embedded Fira Sans latin subset + `tools/fonts/subset_default_font.sh` provenance script (OFL-1.1, default-on `default_font` feature), `SharedFontSystem` + deterministic `BuiyFallback` + `BuiyTextPlugin`, render-world `BuiySwashCache` (uncached-only), opt-in background system-font scan + `FontsGeneration`. `[landed]`
```

- [ ] **Step 3: Spec errata note (do not edit the spec's decisions).** Plan decisions 1–3 found two mechanical inaccuracies for the spec's next edit pass: font-assets § 4's `new_with_fonts(..)` mention (that constructor scans system fonts) and § 1's "one direct dependency" (BuiyFallback needs `unicode-script`; the locale needs `sys-locale`). Append both to the campaign plan as a short "T1 errata for the spec edit pass" note under the T1 phase entry — superseding context, not silent contradiction.

- [ ] **Step 4: Flip this plan's Status** from `active` to `landed`.

- [ ] **Step 5: Run GATE** (docs-only change; gate confirms nothing drifted). **Commit:** `docs(text): T1 engine foundation landed — campaign + index flips, spec errata note`

---

## Done criteria

- [ ] Gate green at every task boundary; **zero** new `#[ignore]` tests (T1 is headless-only; the GPU lane is untouched and stays green by construction).
- [ ] `cosmic-text = "0.19"` direct in `buiy_core`, default features only; `cargo tree` shows no `shape-run-cache`; `cargo deny check` clean (run in Tasks 1 and 3).
- [ ] `tools/fonts/subset_default_font.sh` committed with pinned URL/sha/ranges/features/fonttools version; `crates/buiy_core/assets/fonts/FiraSans-Regular-latin.ttf` + `OFL-FiraSans.txt` committed; artifact parses as one "Fira Sans" face.
- [ ] `SharedFontSystem(Arc<Mutex<FontSystem>>)`: registered-only construction (`new_with_locale_and_db_and_fallback`, sys-locale locale, `BuiyFallback`), all five generic families pinned to the embedded face behind default-on `default_font`; `db().len() == 1` at startup (no scan).
- [ ] `register_render_world` inserts the Arc clone + `BuiySwashCache` (uncached-only contract documented); `BuiyTextPlugin` guards on `RenderApp` and is composed into `BuiyPlugin` before `WidgetsPlugin`.
- [ ] Scan path: `system_fonts: true` spawns on `AsyncComputeTaskPool`; the swap runs the § 3.1 rebuild under one lock hold, preserves locale, bumps `FontsGeneration` exactly once; default-off never scans or bumps.
- [ ] The charter's two headless test surfaces pass: cross-construction determinism (`two_constructions_resolve_every_default_family_identically`) and the exactly-once bump (`injected_scan_swap_bumps_generation_exactly_once`).
- [ ] Campaign T1 row + docs/README.md row + this plan's Status all flipped to landed.

## Seams named here, built later (do NOT build in T1)

| Seam | Where named | Built in |
|---|---|---|
| `BuiyFallback` per-script deterministic lists | `script_fallback` doc | T5 (with the per-script OFL fixture fonts) |
| `FontsGeneration` consumption in `TextSync`'s trigger set | `FontsGeneration` doc | T2 |
| `BuiySwashCache` consumer (`get_image_uncached` miss path, lock site #3) | `BuiySwashCache` doc | T4 |
| `registered_fonts_db` re-adding `FontRegistry` binaries into the scan db | `registered_fonts_db` doc | T5 (`BuiyFont` asset + registry) |
| Lock sites #1/#2 (measure closure, `TextCommit`) | `SharedFontSystem` doc | T3 |
| Live-`RenderApp` wiring exercised on the GPU lane | `register_render_world` doc | T4 |
| `AtlasKey`-never-persisted-across-rebuild enforcement | `swap_font_db` doc | T4 (the producer rebuilds keys from live IDs) |
| `@font-face` asset path (`BuiyFont`/`BuiyFontLoader`), `font-display`, `unicode-range` | font-assets §§ 2–3, 6–7 (not referenced in T1 code) | T5 |
| Atlas ASCII warmup ("what to warm", architecture § 2.3) | not referenced in T1 code | T4/T9 |
