# Buiy Text T2: Text Component + Buffer Lifecycle Implementation Plan

**Date:** 2026-06-10
**Status:** landed
**Spec:** [specs/2026-06-09-buiy-text-rendering-design/architecture.md](../specs/2026-06-09-buiy-text-rendering-design/architecture.md) §§ 3–5.1 + [measure-and-layout.md](../specs/2026-06-09-buiy-text-rendering-design/measure-and-layout.md) §§ 2.3, 4.1, 5.2 + [font-assets.md](../specs/2026-06-09-buiy-text-rendering-design/font-assets.md) § 8 + [glyph-pipeline.md](../specs/2026-06-09-buiy-text-rendering-design/glyph-pipeline.md) § 7
**Campaign:** [plans/2026-06-09-buiy-text-campaign.md](2026-06-09-buiy-text-campaign.md) — phase T2 (depends on T1, landed @ `15c79cc`)

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the per-text-entity state and its sync path: the authored `Text(pub String)` content component; the `FontFamily`/`FontSize`/`FontWeight` phase-1 components + `TextStyleDefaults` plugin defaults; `TextColor` (the graduated `Visual.foreground_token`); the retained `TextBuffer` component (0.19 lazy-setter contract, `Shaping::Advanced` hard-pinned, despawn cleanup free, bypass-change-detection discipline); the idempotent `ComputedTextLayout` output **type**; the white-space collapse pre-pass function (the § 5.2 value table); and the `BuiyLayoutStep::TextSync` layout step (trigger union, collapse pre-pass call site, intrinsics invalidation, Taffy `mark_dirty`).

**Architecture:** `TextSync` is a new named layout sub-step chained between `WritingModeInherit` and `SyncStyles` (architecture § 4.1). It creates/updates `TextBuffer` from the authored components via the 0.19 **lazy setters** — `set_text`/`set_metrics`/`set_wrap`/`set_tab_width` take no `FontSystem` by signature, so TextSync never locks `SharedFontSystem` (architecture § 1.2: exactly three lock sites, none in T2). All in-place `TextBuffer` mutation routes through `Mut::bypass_change_detection` — `Changed<TextBuffer>` is reserved for nothing (measure-and-layout § 7). Where the spec's § 5.1 trigger union names carriers that land in later phases (line-height/wrap/align → T3, `TextDirection` → T5, theme font-token swap → `buiy-theme-tokens-design`), T2 builds the union over the carriers that exist and **names** the joining seams.

**Where T2 ends (honesty pin):** `TextSync` ends at collapsed content into `Buffer::set_text` with pinned `Attrs` + `Shaping::Advanced`, intrinsics invalidated, Taffy `mark_dirty`. **Measurement itself is T3** — no Taffy measure closure, no `TextCommit`, no `shape_until_scroll`, no wrap-mapping completion, no align. The `ComputedTextLayout` idempotency test is **T3's** (the campaign moved it: "the component is only *written* by `TextCommit`, which lands there"). The § 5.4 direction strong-mark prepend is **T5's** (campaign T5 deliverable: "per-node direction via the strong-mark prepend"); its slot in the pre-pass pipeline is named, not built.

**Tech Stack:** cosmic-text 0.19 (already a dep — T1), taffy 0.10.1 (`mark_dirty`/`dirty`, already a dep), Bevy 0.18.1 ECS. **No new dependencies** — if any task appears to need one, STOP: that contradicts the charter (`unicode-script`/`sys-locale` already landed with T1; verify with `grep -n "cosmic-text\|sys-locale\|unicode-script" crates/buiy_core/Cargo.toml`).

**Test reality:** T2 is **headless-only** (campaign test surface: "headless — `tests/layout_pipeline_order.rs` grows `TextSync`; trigger-set tests per the § 5.1 rows; Changed-gated reshape"). Every test runs on `MinimalPlugins`; no adapter, no `#[ignore]`, the GPU lane is untouched.

---

## The gate (keep green at every commit)

**Gate per task:** headless only.

```sh
cargo fmt --all -- --check && \
  cargo clippy --workspace --all-targets -- -D warnings && \
  RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps && \
  cargo test --workspace -j 2
```

(On this Linux host the pre-existing windowed tests need a display server: prefix the test step with `xvfb-run -a` exactly as CLAUDE.md's gate does. The new T2 tests themselves never need one.)

---

## Orientation: verified facts this plan builds on

cosmic-text facts source-verified against the vendored **0.19.0** in the cargo registry on 2026-06-10; taffy against vendored **0.10.1**; Bevy against **0.18.1**. Re-verify file/line refs before editing — they drift.

| Fact | Verified shape |
|---|---|
| `Buffer::new_empty(metrics: Metrics) -> Self` | buffer.rs:383 — takes **no** `FontSystem` (`Buffer::new` does, buffer.rs:406 — never call it: a fourth lock site) |
| `set_text(&mut self, text: &str, attrs: &Attrs, shaping: Shaping, alignment: Option<Align>)` | buffer.rs:934 — the 0.19 signature: `&Attrs` **and** an alignment param, no FontSystem |
| Lazy setters | `set_metrics(Metrics)` :729, `set_wrap(Wrap)` :759, `set_tab_width(u16)` :801, `set_size(Option<f32>, Option<f32>)` :818 — none take a FontSystem |
| Getters | `metrics() -> Metrics` :720 (const), `wrap() -> Wrap` :754 (const); `Buffer.lines: Vec<BufferLine>` is **pub** :336; `BufferLine::text() -> &str` buffer_line.rs:68 |
| `Buffer::new_empty` default wrap | `Wrap::WordOrGlyph` (buffer.rs:392) — **not** the CSS initial; T2 pins `Wrap::Word` explicitly (§ 5.2 `normal` row) |
| `Metrics` | `#[derive(Clone, Copy, Debug, Default, PartialEq)]`; `Metrics::relative(font_size: f32, line_height_scale: f32)` buffer.rs:310 |
| `Attrs` | builder: `Attrs::new().family(Family).weight(Weight)` (attrs.rs:286+); `family` is a **single** `Family<'a>` (no stack — font-assets § 6) |
| `Family`/`Weight` | fontdb re-exports at the cosmic-text root (attrs.rs:12 `pub use fontdb::{Family, Stretch, Style, Weight}`) |
| `Shaping` | `#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]` (shape.rs:27) — `assert_eq!` works for the pin test |
| taffy dirty API | `TaffyTree::mark_dirty(&mut self, NodeId) -> TaffyResult<()>` taffy_tree.rs:873 (recursive to ancestors); `dirty(&self, NodeId) -> TaffyResult<bool>` :900 |
| `Commands::get_entity` | `-> Result<EntityCommands, InvalidEntityError>` (bevy_ecs 0.18.1 commands/mod.rs:492) — the panic-free path for the removed-`Text` cleanup |
| `inherit_writing_mode` | **guarded insert** (`layout/systems.rs:2993` — `if current.copied() != Some(new_resolved)`), so `Changed<WritingModeResolved>` fires only on real changes; safe in TextSync's trigger union |

Codebase shapes consumed (read before editing, confirm current):

- `crates/buiy_core/src/text/` — T1 as-built: `mod.rs` (`BuiyTextPlugin`, `register_render_world`), `font_system.rs` (`SharedFontSystem`, `FontsGeneration` — its doc already names T2 as the consumer), `swash.rs`, `system_scan.rs`. **T2 adds `components.rs`, `whitespace.rs`, `sync.rs` and grows `mod.rs`.**
- `crates/buiy_core/src/layout/pipeline.rs:17–76` — `BuiyLayoutStep` (11 variants) + `configure_pipeline`. The Phase-4 `WritingModeInherit` insertion is the precedent for growing the enum. **Gains `TextSync`.**
- `crates/buiy_core/src/layout/tree.rs` — `LayoutTree` is a **NonSendResource** (`TaffyTree` is `!Send`); `pub(crate) tree` + `pub(crate) by_entity`, plus `#[doc(hidden)]` read-only `by_entity()` / `tree_ref()` test accessors. **Gains `mark_dirty_for_entity`.**
- `crates/buiy_core/tests/layout_pipeline_order.rs` — the tracker-system order assertion: currently **9** tracked labels `["gc","wmi","sync","cq_activate","taffy","cq_flip","cq_rerun","post_taffy","write"]`, asserted `n == 9`. **Grows `text_sync`, n == 10.**
- `crates/buiy_core/src/layout/systems.rs:109–117` — `SyncStylesIterCount`: the per-frame counter-resource precedent `TextSyncAppliedCount` copies (overwritten at the top of each invocation, asserted zero on steady frames).
- `crates/buiy_core/src/render/components.rs` — `Background` (the `TextColor` derive/placement precedent: `#[derive(Component, Reflect, Default, Clone, PartialEq, Debug)] #[reflect(Component, Default)]`, author-set render components registered in `BuiyRenderPlugin::build`'s main-world block, `render/mod.rs:157+`); `crates/buiy_core/tests/render_components_registry.rs` is the registration test that grows.
- `crates/buiy_core/src/lib.rs:52` — `pub use text::{BuiyTextPlugin, FontsGeneration, SharedFontSystem};` (grows); `crates/buiy/src/lib.rs:25–32` — the `render::components::{…}` and `text::{…}` re-export groups (grow).

## Decisions this plan encodes (resolved against the spec — do not relitigate)

1. **The T2 trigger union = the § 5.1 row over the carriers that exist.** Architecture § 5.1 row 1 pins `Or<(Changed<Text>, Changed<text-style carriers (font family/size/weight/line-height/wrap/align/direction)>, Added<TextBuffer>, Changed<WritingModeResolved>)>` ∪ `FontsGeneration` bump ∪ theme font-token swap. In T2 the existing carriers are `Text`, `FontFamily`, `FontSize`, `FontWeight` (this phase) plus `WritingModeResolved` (layout Phase 4) and `FontsGeneration` (T1). Line-height / white-space / wrap / align carriers are **T3 deliverables**, `TextDirection` is **T5's** (campaign T5: "per-node direction via the strong-mark prepend"), the theme font-token swap is **`buiy-theme-tokens-design`'s** (font-assets § 9 — unwritten). Each missing member joins the union when its carrier lands; the union's module doc is the ledger. *Runner-up rejected:* defining stub carrier components now so the union is "complete" — dead authored surface with no consumer, contradicting the campaign's phase boundaries.
2. **The collapse pre-pass FUNCTION is fully built in T2 (all three modes); the carrier that selects modes is T3's.** Measure § 4.1 puts the pre-pass call inside TextSync, so the function lands here, complete per the § 5.2 value table (collapse / preserve / preserve-breaks, CSS Text L3 § 4.1 phase I). TextSync pins `CollapseMode::Collapse` — the `white-space: normal` initial — until T3's white-space carrier selects across the table. The table's **Wrap column** is likewise T3's; T2 pins the `normal` row's `Wrap::Word` (the `Buffer::new_empty` default is `WordOrGlyph` — wrong initial) and `set_tab_width(8)` (the § 5.2 "set to **8** … at `TextSync`" pin). *Runner-up rejected:* collapse-mode-only function now, other modes in T3 — splits one cohesive pure function across phases for zero savings; the modes are ~30 lines and the value table is normative now.
3. **Line-height stand-in: `Metrics::relative(size, 1.2)`.** `set_text`/`Buffer::new_empty` need a `Metrics`, but the line-height → `Metrics` mapping is T3's (campaign T3 deliverable; measure § 5.1). T2 uses the common UA `line-height: normal` factor 1.2, named `DEFAULT_LINE_HEIGHT_SCALE`, replaced by the carrier in T3. *Runner-up rejected:* `line_height = font_size` (×1.0) — typographically wrong, and goldens built on it in T4 would all churn when T3 lands.
4. **`TextStyleDefaults` resource, constructed from the component `Default` impls.** font-assets § 8: "Plugin-level defaults (`BuiyTextPlugin`'s default stack/size/weight) cover unset components." One resource (`family`/`size`/`weight`), built from `FontStack::default()` / `FontSize::default()` / `FontWeight::default()` so the two surfaces cannot diverge; values: sans-serif generic stack, 16 px (CSS `medium`), weight 400. The `small`/`medium`/`large` keyword table is named in § 8, not built (no keyword carrier in T2). *Runner-up rejected:* Bevy required-components auto-inserting defaults at spawn — freezes the default per-entity at spawn time (a later `TextStyleDefaults` swap wouldn't propagate) and contradicts "cover **unset** components".
5. **Interim family lowering: the stack's FIRST entry → `Attrs.family`.** The Buiy-owned resolver (fontdb `Query` walk, coverage span-splitting, `unicode-range`) is the **T5** deliverable (font-assets § 6). Until it lands, TextSync lowers `FontStack[0]` to a cosmic `Family` and lets `FontFallbackIter` + the deterministic `BuiyFallback` cover misses. font-assets § 6 rejects first-family-only **as the target** ("stack order is the F-tier semantic itself") — this is a named staging step toward T5, not a target. An empty authored stack degrades to `Family::SansSerif` (pinned to the embedded face).
6. **`TextColor` defaults to `ColorToken::CurrentColor`, not the derived `Transparent`.** `ColorToken::default()` is `Transparent` — correct for `Background` ("absent == no fill") but **invisible text** for glyphs. `CurrentColor` resolves through the existing `resolve_token` rule to the theme default foreground (`CanvasText` under forced-colors, else `color.text.primary`) — `render/color.rs:132–144`, whose doc already names this spec as the future carrier. *Runner-up rejected:* `Token("color.text.primary")` directly — duplicates the CurrentColor fallback rule and diverges under forced-colors.
7. **`TextColor` lives in `render/components.rs`, registered by `BuiyRenderPlugin`.** Glyph-pipeline § 7 calls it "render-owned" — it replaces `Visual.foreground_token` exactly as `Background` replaces `Visual.background_token`, and that file's module doc claims the `Visual` decomposition. Registration joins the existing main-world `register_type` block (headless-safe). *Runner-up rejected:* `text/components.rs` + `BuiyTextPlugin` registration — splits the `Visual` decomposition across modules and breaks the render-components-registry test's "every author-set render component" claim.
8. **`ComputedTextLayout` field shape: per-line geometry mirroring the verified 0.19 `LayoutRun` fields.** The spec pins the component's existence, write contract, and consumers (architecture § 3.3) but not its fields; the charter assigns the **type** to T2. Fields: `lines: Vec<ComputedTextLine>` (`line_y`/`line_top`/`line_height`/`line_w`/`rtl` — all verified `LayoutRun` fields; `rtl` is the flag measure § 5.4 says the editing campaign's caret model consumes) + `size: Vec2`. `PartialEq` is load-bearing: it IS the T3 idempotent-write compare. T3 may extend additively. *Runner-up rejected:* an empty marker type now — forces a T3 redesign for no benefit and gives `PartialEq` nothing to compare.
9. **ALL in-place `TextBuffer` mutation bypasses change detection — including TextSync's.** Measure § 7 names the measure closure and `TextCommit`; the charter's T2 test surface generalizes it: "`Changed<TextBuffer>` never fires — the bypass discipline". A sync write is not a damage signal (damage keys on commit outputs); the only `TextBuffer` tick ever observed is the insertion tick (the `Added<TextBuffer>` edge § 5.1 itself consumes). Task 8 pins this with a tracker test.
10. **`text_sync_buffers` is registered by `BuiyTextPlugin`, with `Option<NonSendMut<LayoutTree>>`.** The spec assigns the TextSync step to the text seam ("inserted into layout's chain", measure § 1). T1 established standalone `BuiyTextPlugin` use (the `text_engine`/`text_system_scan` tests run it without `LayoutPlugin` and call `update()`), so the `LayoutTree` param must be `Option` — absent tree ⇒ nothing to dirty-mark (and nothing measures). The `in_set(BuiyLayoutStep::TextSync)` membership is inert-but-harmless without `configure_pipeline` (same posture as T1's `.before(BuiySet::Layout)`). *Runner-up rejected:* registration by `LayoutPlugin` — layout would gain a dependency on text components, inverting the seam.
11. **`RemovedComponents<Text>` cleanup is T2's; `set_node_context` unregistration is T3's.** This phase is titled "Buffer lifecycle": when `Text` is removed from a live entity, TextSync removes `TextBuffer` + `ComputedTextLayout` (despawn cleanup is already free — plain components). The Taffy context unregistration on the same edge (measure § 2.2) lands with T3's `TaffyTree<Entity>` migration — there is no context to unregister yet.
12. **`TextSyncAppliedCount` is the trigger-test instrument.** The `SyncStylesIterCount` precedent (`systems.rs:109–117`): overwritten per invocation, asserted **zero on a no-change frame** and exact counts per § 5.1 trigger row. T3's `TextMeasureCallCount` (measure § 7) is a separate, later instrument.
13. **The creation-frame echo is accepted and documented.** TextSync inserts `TextBuffer` via `Commands` on the spawn frame (content fully applied — text never appears a frame late); the insertion tick fires the `Added<TextBuffer>` arm once more on the next frame — an idempotent lazy re-apply before any shaping consumer exists (T2 has none). Trigger tests `settle()` across both frames. *Runner-up rejected:* insert-empty-then-fill-on-Added — delays content by a frame for every new text entity.
14. **Observed spec gap (flagged, not built):** carrier-component **removal** (e.g. `FontSize` removed → entity should revert to defaults) is in no § 5.1 union; the entity resyncs only on the next other trigger. Record in the campaign's T2-errata note for the spec edit pass — superseding context, not silent contradiction. Do not invent a `RemovedComponents` arm for it here.

## File structure

```
crates/buiy_core/
├── src/
│   ├── lib.rs                       # root re-exports grow (text types, TextColor)
│   ├── layout/
│   │   ├── pipeline.rs              # +BuiyLayoutStep::TextSync (enum + chain)
│   │   └── tree.rs                  # +LayoutTree::mark_dirty_for_entity (pub(crate))
│   ├── render/
│   │   ├── components.rs            # +TextColor
│   │   └── mod.rs                   # +register_type::<TextColor>
│   └── text/
│       ├── mod.rs                   # +mod components/whitespace/sync; plugin wiring; re-exports
│       ├── components.rs            # Text, FontStack/FamilyEntry/GenericFamily, FontFamily,
│       │                            #   FontSize, FontWeight, TextStyleDefaults, TEXT_SHAPING,
│       │                            #   TextBuffer, IntrinsicWidths, ComputedTextLayout(+Line)
│       ├── whitespace.rs            # CollapseMode + collapse_whitespace (pure; § 5.2 table)
│       └── sync.rs                  # text_sync_buffers, TextSyncAppliedCount, AuthoredStyle
└── tests/
    ├── layout_pipeline_order.rs     # grows the text_sync tracker (9 → 10)
    ├── render_components_registry.rs# grows the TextColor assertion
    ├── text_components.rs           # NEW: component surface + defaults + registration
    └── text_sync.rs                 # NEW: § 5.1 trigger rows, dirty-mark, removal, bypass
crates/buiy/src/lib.rs               # re-export groups grow
```

---

## Task 1 — Authoring components + plugin style defaults

The `Text(pub String)` content pin (measure-and-layout § 4.1) and the phase-1 font trio + plugin defaults (font-assets § 8), with the `FontStack` value type from font-assets § 6.

**Files:**
- Create: `crates/buiy_core/src/text/components.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`
- Modify: `crates/buiy_core/src/lib.rs`
- Modify: `crates/buiy/src/lib.rs`
- Create: `crates/buiy_core/tests/text_components.rs`

- [x] **Step 1: Flip the status rows.** In `docs/plans/2026-06-09-buiy-text-campaign.md`, Phase status table: `| T2 | Text component + Buffer lifecycle | proposed |` → `| T2 | Text component + Buffer lifecycle | in progress |`. In this plan's header: `**Status:** proposed` → `**Status:** active`.

- [x] **Step 2: Write the failing tests** — create `crates/buiy_core/tests/text_components.rs`:

```rust
//! The phase-1 text authoring surface (font-assets § 8): the `Text` content
//! component, the font trio, and the plugin-level defaults that cover unset
//! components. Headless — no FontSystem interaction anywhere in this file.

use bevy::prelude::*;
use buiy_core::text::{
    BuiyTextPlugin, FamilyEntry, FontFamily, FontSize, FontStack, FontWeight, GenericFamily,
    TextStyleDefaults,
};

fn text_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(BuiyTextPlugin::default());
    app
}

/// The CSS initial values the unset-component fallbacks reproduce:
/// sans-serif / 16 px (`medium`) / 400 (`normal`).
#[test]
fn component_defaults_are_the_css_initials() {
    assert_eq!(FontSize::default().0, 16.0);
    assert_eq!(FontWeight::default().0, 400);
    assert_eq!(
        FontFamily::default().0,
        FontStack(vec![FamilyEntry::Generic(GenericFamily::SansSerif)])
    );
}

/// font-assets § 8: plugin-level defaults cover unset components. One source
/// of truth — the resource is constructed FROM the component defaults.
#[test]
fn plugin_defaults_mirror_component_defaults() {
    let app = text_app();
    let defaults = app.world().resource::<TextStyleDefaults>();
    assert_eq!(defaults.family, FontFamily::default().0);
    assert_eq!(defaults.size, FontSize::default().0);
    assert_eq!(defaults.weight, FontWeight::default().0);
}

/// Author-set components are reflect-registered (BSN / inspectors), the
/// layout-components convention.
#[test]
fn authoring_types_are_registered_for_reflection() {
    let mut app = text_app();
    app.update();
    let registry = app.world().resource::<AppTypeRegistry>().read();
    for name in [
        "buiy_core::text::components::Text",
        "buiy_core::text::components::FontFamily",
        "buiy_core::text::components::FontSize",
        "buiy_core::text::components::FontWeight",
    ] {
        assert!(
            registry.get_with_type_path(name).is_some(),
            "type not registered: {name}",
        );
    }
}
```

- [x] **Step 3: Run, expect compile FAIL** — `cargo test -p buiy_core --test text_components` → unresolved imports (`FamilyEntry`, `FontFamily`, … not in `buiy_core::text`).

- [x] **Step 4: Implement** — create `crates/buiy_core/src/text/components.rs`:

```rust
//! Per-text-entity components (T2): the authored content + style surface
//! (font-assets §§ 6, 8; measure-and-layout § 4.1) and — added later in this
//! phase — the retained `TextBuffer` state and the `ComputedTextLayout`
//! output type (architecture § 3).

use bevy::prelude::*;

/// The authored UTF-8 text content (measure-and-layout § 4.1) — the string
/// `TextSync` feeds to `Buffer::set_text`, after the § 5.2 white-space
/// collapse pre-pass (the § 5.4 direction strong-mark prepend joins the
/// pre-pass pipeline in T5).
///
/// Changing it is the canonical reshape trigger (architecture § 5.1):
/// `TextSync` rewrites the entity's `TextBuffer` in place via the 0.19 lazy
/// setters and dirty-marks the Taffy node. Shaping happens at the next
/// lock-bearing site (T3's measure closure / `TextCommit`), never here.
#[derive(Component, Reflect, Default, Clone, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub struct Text(pub String);

/// The `font-family` stack value (font-assets § 6; foundation text.md:10, F).
/// Ordered; first match wins. v1 components carry **explicit** stacks —
/// theme token→stack indirection is the font-assets § 9 theme seam.
///
/// T2 interim lowering: `TextSync` hands cosmic-text only the FIRST entry
/// (misses fall through to `FontFallbackIter` + the deterministic
/// `BuiyFallback`); the full Buiy-owned resolver — fontdb `Query` walk,
/// coverage span-splitting, `unicode-range` — is T5's (font-assets § 6).
#[derive(Reflect, Clone, PartialEq, Eq, Debug)]
pub struct FontStack(pub Vec<FamilyEntry>);

impl Default for FontStack {
    /// The CSS-initial analogue: the `sans-serif` generic
    /// (`registered_fonts_db` pins all five generics to the embedded face).
    fn default() -> Self {
        Self(vec![FamilyEntry::Generic(GenericFamily::SansSerif)])
    }
}

/// One `font-family` stack entry (font-assets § 6).
#[derive(Reflect, Clone, PartialEq, Eq, Debug)]
pub enum FamilyEntry {
    /// A concrete family name, e.g. `"Fira Sans"`.
    Named(String),
    /// A CSS generic family, resolved through fontdb's `set_*_family` pins.
    Generic(GenericFamily),
}

/// The deterministic five CSS generic families (font-assets § 6). The
/// extended set (`system-ui`, `ui-monospace`, …) is C-tier, deferred with
/// the theme seam (font-assets § 9).
#[derive(Reflect, Clone, Copy, PartialEq, Eq, Debug)]
pub enum GenericFamily {
    Serif,
    SansSerif,
    Cursive,
    Fantasy,
    Monospace,
}

impl GenericFamily {
    /// Lower to the cosmic-text (fontdb) generic. All five resolve through
    /// the `registered_fonts_db` family pins, so no generic ever dangles
    /// (font-assets § 4).
    pub fn to_cosmic(self) -> cosmic_text::Family<'static> {
        match self {
            GenericFamily::Serif => cosmic_text::Family::Serif,
            GenericFamily::SansSerif => cosmic_text::Family::SansSerif,
            GenericFamily::Cursive => cosmic_text::Family::Cursive,
            GenericFamily::Fantasy => cosmic_text::Family::Fantasy,
            GenericFamily::Monospace => cosmic_text::Family::Monospace,
        }
    }
}

/// `font-family` (font-assets § 8; text.md:10, F). Unset = the
/// `TextStyleDefaults` stack.
#[derive(Component, Reflect, Default, Clone, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub struct FontFamily(pub FontStack);

/// `font-size` in logical px (font-assets § 8; text.md:12, F — cosmic-text
/// `Metrics` are unit-agnostic px; Buiy pins logical px end-to-end,
/// architecture § 6). The `small`/`medium`/`large` keyword table is named
/// in font-assets § 8, not built in T2. Unset = `TextStyleDefaults.size`.
#[derive(Component, Reflect, Clone, Copy, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct FontSize(pub f32);

impl Default for FontSize {
    /// 16 px — the CSS `medium` initial.
    fn default() -> Self {
        Self(16.0)
    }
}

/// `font-weight` (font-assets § 8; text.md:13, F) — lowered to
/// `cosmic_text::Weight(u16)`. Variable-font weight rides the committed
/// `Attrs.weight → Query.weight → get_font(id, weight)` surface end-to-end
/// (font-assets § 6); style/stretch synthesis stays C-tier.
#[derive(Component, Reflect, Clone, Copy, PartialEq, Eq, Debug)]
#[reflect(Component, Default)]
pub struct FontWeight(pub u16);

impl Default for FontWeight {
    /// 400 — CSS `normal`.
    fn default() -> Self {
        Self(400)
    }
}

/// Plugin-level defaults covering UNSET font components (font-assets § 8:
/// "Plugin-level defaults (`BuiyTextPlugin`'s default stack/size/weight)
/// cover unset components"). Single source of truth: constructed from the
/// component `Default` impls so the two surfaces can never diverge. Swap
/// the resource to retheme app-wide defaults; per-entity components win.
#[derive(Resource, Clone, PartialEq, Debug)]
pub struct TextStyleDefaults {
    /// Default `font-family` stack for entities without `FontFamily`.
    pub family: FontStack,
    /// Default `font-size` (logical px) for entities without `FontSize`.
    pub size: f32,
    /// Default `font-weight` for entities without `FontWeight`.
    pub weight: u16,
}

impl Default for TextStyleDefaults {
    fn default() -> Self {
        Self {
            family: FontStack::default(),
            size: FontSize::default().0,
            weight: FontWeight::default().0,
        }
    }
}
```

- [x] **Step 5: Wire the module + plugin.** In `crates/buiy_core/src/text/mod.rs`, add below the existing `mod` lines:

```rust
mod components;
```

and to the re-export block:

```rust
pub use components::{
    FamilyEntry, FontFamily, FontSize, FontStack, FontWeight, GenericFamily, Text,
    TextStyleDefaults,
};
```

In `BuiyTextPlugin::build`, after `app.init_resource::<FontsGeneration>();`:

```rust
        // T2: the authoring-surface defaults (font-assets § 8) and the
        // author-set component registrations (reflection / BSN / inspectors —
        // the layout convention). The computed text state (TextBuffer,
        // ComputedTextLayout) is deliberately NOT registered, matching the
        // render components.rs convention for computed components.
        app.init_resource::<TextStyleDefaults>();
        app.register_type::<Text>()
            .register_type::<FontFamily>()
            .register_type::<FontSize>()
            .register_type::<FontWeight>();
```

Also update the `mod.rs` module doc's "Later phases" paragraph: change `` `TextBuffer` + the `TextSync`/`TextCommit` layout steps (T2–T3) `` to reflect that T2 is landing in this module now (e.g. "T2 (this phase) adds the authored components, `TextBuffer`, and `TextSync`; T3 adds measure + `TextCommit`").

- [x] **Step 6: Root re-exports.** In `crates/buiy_core/src/lib.rs`, grow the existing line 52:

```rust
pub use text::{
    BuiyTextPlugin, FontFamily, FontSize, FontWeight, FontsGeneration, SharedFontSystem, Text,
    TextStyleDefaults,
};
```

In `crates/buiy/src/lib.rs`, grow the `text::{…}` group in the `pub use buiy_core::{…}` block:

```rust
    text::{
        BuiyTextPlugin, FamilyEntry, FontFamily, FontSize, FontStack, FontWeight,
        FontsGeneration, GenericFamily, SharedFontSystem, Text, TextStyleDefaults,
    },
```

- [x] **Step 7: Run the tests, expect PASS** — `cargo test -p buiy_core --test text_components` → 3 passed.

- [ ] **Step 8: Run GATE. Commit:** `feat(text): authored Text + font trio components, TextStyleDefaults plugin defaults`

---

## Task 2 — `TextColor` (the graduated `Visual.foreground_token`)

Glyph-pipeline § 7: a render-owned `TextColor(ColorToken)` component, resolved at extract like `Background` **from T4** — T2 lands only the component + registration.

**Files:**
- Modify: `crates/buiy_core/src/render/components.rs`
- Modify: `crates/buiy_core/src/render/mod.rs`
- Modify: `crates/buiy_core/src/lib.rs`
- Modify: `crates/buiy/src/lib.rs`
- Test: `crates/buiy_core/tests/render_components_registry.rs`, `crates/buiy_core/tests/text_components.rs`

- [x] **Step 1: Write the failing tests.** Append to `crates/buiy_core/tests/text_components.rs`:

```rust
mod text_color {
    use buiy_core::render::color::ColorToken;
    use buiy_core::render::components::TextColor;

    /// `ColorToken::default()` is `Transparent` — right for `Background`
    /// ("absent == no fill"), INVISIBLE for glyphs. The glyph foreground
    /// defaults to `CurrentColor`, which `resolve_token` already lowers to
    /// the theme default foreground (`CanvasText` under forced-colors, else
    /// `color.text.primary`) — glyph-pipeline § 7.
    #[test]
    fn text_color_defaults_to_current_color_not_transparent() {
        assert_eq!(TextColor::default().0, ColorToken::CurrentColor);
    }
}
```

And in `crates/buiy_core/tests/render_components_registry.rs`: add `TextColor` to the existing `use buiy_core::render::components::{…}` import list, and append inside `author_set_render_components_are_registered` (alongside the existing assertions):

```rust
    assert!(
        reg.get(std::any::TypeId::of::<TextColor>()).is_some(),
        "TextColor"
    );
```

- [x] **Step 2: Run, expect compile FAIL** — `cargo test -p buiy_core --test text_components --test render_components_registry` → no `TextColor` in `render::components`.

- [x] **Step 3: Implement.** In `crates/buiy_core/src/render/components.rs`, after the `Background` block:

```rust
/// Glyph foreground color (v1 text) — the graduated `Visual.foreground_token`
/// reservation the atlas seam hands to the text spec
/// (atlas-and-text-seam.md § 1; glyph-pipeline.md § 7 owns the contract).
///
/// Consumed by `extract_buiy_glyphs` from T4: resolved at extract exactly
/// like `Background` (`render::color::resolve_token`), CPU-linearized, and
/// written **straight-alpha** into `GlyphAlphaInstance.color` — alpha-as-
/// color means a color change re-emits instances; the atlas is never
/// touched. Per-span `LayoutGlyph.color_opt` overrides it per-glyph when
/// rich-text spans land (C-tier).
///
/// Default is `CurrentColor` — the theme default foreground (`CanvasText`
/// under forced-colors, else `color.text.primary`) — NOT the derived
/// `Transparent` default, which would render text invisible.
#[derive(Component, Reflect, Clone, PartialEq, Debug)]
#[reflect(Component, Default)]
pub struct TextColor(pub ColorToken);

impl Default for TextColor {
    fn default() -> Self {
        Self(ColorToken::CurrentColor)
    }
}
```

In `crates/buiy_core/src/render/mod.rs`, append to the main-world `register_type` chain (the block at ~:157 ending `.register_type::<color::SystemColorKeyword>();`):

```rust
            .register_type::<components::TextColor>()
```

(insert before the final `;`). In `crates/buiy_core/src/lib.rs`, add `TextColor` to the `pub use render::components::{…}` list. In `crates/buiy/src/lib.rs`, add `TextColor` to the `render::components::{…}` group.

- [x] **Step 4: Run the tests, expect PASS** — `cargo test -p buiy_core --test text_components --test render_components_registry` → all pass.

- [ ] **Step 5: Run GATE. Commit:** `feat(render): TextColor component — CurrentColor default (glyph-pipeline § 7)`

---

## Task 3 — `TextBuffer` + `IntrinsicWidths` + `ComputedTextLayout` types

The retained per-entity state (architecture § 3.1; field shape per measure-and-layout § 2.3) and the idempotent output **type** (architecture § 3.3 — the writer and its idempotency test are T3's).

**Files:**
- Modify: `crates/buiy_core/src/text/components.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`
- Modify: `crates/buiy_core/src/lib.rs`
- Modify: `crates/buiy/src/lib.rs`

- [x] **Step 1: Write the failing unit tests.** Append to `crates/buiy_core/src/text/components.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use cosmic_text::{Metrics, Shaping};

    /// architecture § 3.2: `Shaping::Basic` breaks complex scripts for a
    /// micro-optimization and is never exposed. Drift tripwire.
    #[test]
    fn shaping_is_pinned_to_advanced() {
        assert_eq!(TEXT_SHAPING, Shaping::Advanced);
    }

    #[test]
    fn new_buffers_start_with_no_cached_intrinsics() {
        let buffer = TextBuffer::new(Metrics::new(16.0, 19.2));
        assert_eq!(buffer.intrinsics(), None);
    }

    #[test]
    fn invalidate_drops_cached_intrinsics() {
        let mut buffer = TextBuffer::new(Metrics::new(16.0, 19.2));
        buffer.intrinsics = Some(IntrinsicWidths {
            min_content: 10.0,
            max_content: 80.0,
        });
        buffer.invalidate_intrinsics();
        assert_eq!(buffer.intrinsics(), None);
    }
}
```

- [x] **Step 2: Run, expect compile FAIL** — `cargo test -p buiy_core --lib text::components` → `TEXT_SHAPING` / `TextBuffer` / `IntrinsicWidths` not found.

- [x] **Step 3: Implement.** In `crates/buiy_core/src/text/components.rs`, change the imports to:

```rust
use bevy::prelude::*;
use cosmic_text::{Buffer, Metrics, Shaping};
```

and append (above the tests module):

```rust
/// The one shaping mode Buiy ever passes to `set_text` (architecture § 3.2):
/// `Shaping::Basic` breaks complex scripts for a micro-optimization and is
/// never exposed. The unit test below is the drift tripwire.
pub const TEXT_SHAPING: Shaping = Shaping::Advanced;

/// Main-world retained per-text-entity state (architecture § 3.1; the field
/// shape — buffer plus cached intrinsics — is owned by measure-and-layout
/// § 2.3): the cosmic-text `Buffer`, mutated IN PLACE. Rebuilding the buffer
/// would discard the per-`BufferLine` shape/layout caches — the
/// typing-latency win the retained component exists for.
///
/// **Change-detection contract (measure-and-layout § 7):** every in-place
/// mutation — `TextSync` here, the measure closure and `TextCommit` in T3 —
/// goes through `Mut::bypass_change_detection`. `Changed<TextBuffer>` is
/// reserved for NOTHING: author intent rides `Changed<Text>` + the
/// text-style carriers; downstream damage keys on the commit outputs
/// (`ComputedTextLayout`), never on this component's ticks. The only tick
/// ever observed is the insertion tick (the `Added<TextBuffer>` trigger
/// edge architecture § 5.1 consumes).
///
/// Despawn cleanup is free (plain component); `Text`-removal cleanup is
/// `text_sync_buffers`' removed-stream arm. Editable entities will own
/// their authoritative buffer inside `TextEditState` (the successor
/// `buiy-text-editing` campaign); the one shared accessor over both — the
/// `TextBufferAccess` QueryData pinned by measure-and-layout § 2.3 — is
/// built with its first consumers (T3's measure closure / `TextCommit`;
/// T4's producer uses its read-only form), not in T2.
#[derive(Component)]
pub struct TextBuffer {
    /// The retained buffer. Logical px end-to-end (architecture § 6) —
    /// physical-px rasterization happens at emission (T4), never here.
    pub buffer: Buffer,
    /// Cached intrinsic widths, keyed by content version (measure § 3.2):
    /// `TextSync` invalidates on every content change; the T3 measure
    /// closure computes and re-caches.
    intrinsics: Option<IntrinsicWidths>,
}

impl TextBuffer {
    /// A new, empty, unshaped buffer. `Buffer::new_empty` takes no
    /// `FontSystem` — the lock-free TextSync contract (architecture § 1.2:
    /// exactly three lock sites, TextSync is none of them; `Buffer::new`
    /// takes `&mut FontSystem` and would be a forbidden fourth).
    pub fn new(metrics: Metrics) -> Self {
        Self {
            buffer: Buffer::new_empty(metrics),
            intrinsics: None,
        }
    }

    /// The cached intrinsic min-/max-content widths, if valid for the
    /// current content version. `None` until the T3 measure closure
    /// computes them, and after every `TextSync` invalidation.
    pub fn intrinsics(&self) -> Option<IntrinsicWidths> {
        self.intrinsics
    }

    /// Drop the cached intrinsics: every content change (text / attrs /
    /// font / metrics / wrap) invalidates them (measure § 3.2).
    pub(crate) fn invalidate_intrinsics(&mut self) {
        self.intrinsics = None;
    }
}

/// Cached intrinsic widths (measure-and-layout §§ 2.3, 3.2), logical px.
/// Computed by the T3 measure closure: min-content = longest-word width
/// under `Wrap::Word` (`set_size(Some(0.0), None)`); max-content =
/// unwrapped width (`set_size(None, None)`).
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct IntrinsicWidths {
    /// Longest-word width under `Wrap::Word`.
    pub min_content: f32,
    /// Unwrapped single-line width.
    pub max_content: f32,
}

/// The settled line geometry `TextCommit` (T3) writes after final-width
/// shaping (architecture § 3.3) — read by caret math, picking, a11y bounds,
/// and the extract damage probes (damage keys on THIS component, never on
/// `TextBuffer` ticks — measure § 6).
///
/// **Write contract (enforced by the T3 writer; its idempotency test lands
/// with it):** idempotent-insert — bump the change tick only when the value
/// actually changed, copying `write_resolved_layout`'s guard
/// (layout/systems.rs ~:2657–2691). An unconditional re-insert keeps
/// `Changed<ComputedTextLayout>` perpetually true and cascades a full
/// extract rebuild every frame. The `PartialEq` derive IS that guard's
/// comparison. Logical px (architecture § 6). Computed output — not
/// reflect-registered (the render components.rs convention).
#[derive(Component, Clone, PartialEq, Debug, Default)]
pub struct ComputedTextLayout {
    /// One entry per laid-out line, visual top-to-bottom order.
    pub lines: Vec<ComputedTextLine>,
    /// Laid-out extent: (max line width, Σ line heights).
    pub size: Vec2,
}

/// One laid-out line — the verified 0.19 `LayoutRun` per-line fields.
#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ComputedTextLine {
    /// Y offset of the line's baseline from the content-box top
    /// (`LayoutRun::line_y`, "Y offset to baseline of line") — the
    /// `ResolvedBaseline` (T3) source.
    pub line_y: f32,
    /// Y offset of the line's top (`LayoutRun::line_top`).
    pub line_top: f32,
    /// The line's height (`LayoutRun::line_height`).
    pub line_height: f32,
    /// The line's laid-out width (`LayoutRun::line_w`).
    pub line_w: f32,
    /// Whether the line's base direction resolved right-to-left
    /// (`LayoutRun::rtl`) — the flag the editing campaign's caret model
    /// consumes (measure § 5.4).
    pub rtl: bool,
}
```

- [x] **Step 4: Wire the exports.** `crates/buiy_core/src/text/mod.rs` re-export block grows:

```rust
pub use components::{
    ComputedTextLayout, ComputedTextLine, FamilyEntry, FontFamily, FontSize, FontStack,
    FontWeight, GenericFamily, IntrinsicWidths, TEXT_SHAPING, Text, TextBuffer,
    TextStyleDefaults,
};
```

`crates/buiy_core/src/lib.rs` root re-export grows `ComputedTextLayout, TextBuffer`; `crates/buiy/src/lib.rs` `text::{…}` group grows `ComputedTextLayout, ComputedTextLine, IntrinsicWidths, TextBuffer`.

- [x] **Step 5: Run the tests, expect PASS** — `cargo test -p buiy_core --lib text::components` → 3 passed. (Deviation: `invalidate_intrinsics` carries `#[cfg_attr(not(test), expect(dead_code))]` until Task 6's `text_sync_buffers` becomes the non-test caller — the plan's verbatim Task 3 fails `clippy -D warnings` at this commit without it; the then-unfulfilled expectation forces the attribute off in Task 6.)

- [ ] **Step 6: Run GATE. Commit:** `feat(text): retained TextBuffer + ComputedTextLayout output type (Shaping::Advanced pin)`

---

## Task 4 — The white-space collapse pre-pass

The pure `&str → Cow<str>` transform (measure-and-layout § 5.2), all three modes per the normative value table. **The function is complete in T2; the carrier that selects modes (and the table's `Wrap` column wiring) is T3's** — TextSync pins `Collapse` until then.

**Files:**
- Create: `crates/buiy_core/src/text/whitespace.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`

- [x] **Step 1: Write the failing tests.** Create `crates/buiy_core/src/text/whitespace.rs` with the module doc, the types as stubs **omitted** — write the tests module first at the bottom of the new file (the file won't compile until Step 3 fills the implementation, which is the failing state):

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;

    /// The § 5.2 collapse rules: segment breaks (LF, CR, CRLF) and tabs
    /// each become a collapsible space; runs collapse to one.
    #[test]
    fn collapse_folds_breaks_tabs_and_runs_to_single_spaces() {
        assert_eq!(
            collapse_whitespace("hello\nworld", CollapseMode::Collapse),
            "hello world"
        );
        assert_eq!(collapse_whitespace("a\r\nb", CollapseMode::Collapse), "a b");
        assert_eq!(
            collapse_whitespace("a \t \n b", CollapseMode::Collapse),
            "a b"
        );
    }

    #[test]
    fn collapse_trims_leading_and_trailing() {
        assert_eq!(
            collapse_whitespace("  padded  ", CollapseMode::Collapse),
            "padded"
        );
        assert_eq!(collapse_whitespace("\n\tlead", CollapseMode::Collapse), "lead");
        assert_eq!(collapse_whitespace(" \t\n ", CollapseMode::Collapse), "");
    }

    /// Steady-state typing over plain words must allocate nothing.
    #[test]
    fn collapse_borrows_through_when_already_collapsed() {
        assert!(matches!(
            collapse_whitespace("one two three", CollapseMode::Collapse),
            Cow::Borrowed(_)
        ));
    }

    /// Non-collapsible whitespace (U+00A0 no-break space) passes through —
    /// CSS phase I collapses only spaces, tabs, and segment breaks.
    #[test]
    fn nbsp_is_not_collapsible() {
        assert_eq!(
            collapse_whitespace("a\u{00A0} \u{00A0}b", CollapseMode::Collapse),
            "a\u{00A0} \u{00A0}b"
        );
    }

    /// `pre` / `pre-wrap`: nothing collapses; tabs pass through to the tab
    /// stops; segment breaks normalize to LF (hard buffer lines).
    #[test]
    fn preserve_keeps_spaces_and_tabs_normalizes_crlf() {
        assert_eq!(
            collapse_whitespace("a  b\tc", CollapseMode::Preserve),
            "a  b\tc"
        );
        assert_eq!(
            collapse_whitespace("a\r\nb\rc", CollapseMode::Preserve),
            "a\nb\nc"
        );
        assert!(matches!(
            collapse_whitespace("plain\nbreak", CollapseMode::Preserve),
            Cow::Borrowed(_)
        ));
    }

    /// `pre-line`: hard breaks survive; spaces/tabs collapse per segment.
    #[test]
    fn preserve_breaks_collapses_within_segments_keeps_breaks() {
        assert_eq!(
            collapse_whitespace("a  b\n  c\td  ", CollapseMode::PreserveBreaks),
            "a b\nc d"
        );
        assert_eq!(
            collapse_whitespace("x\r\ny", CollapseMode::PreserveBreaks),
            "x\ny"
        );
    }
}
```

- [x] **Step 2: Run, expect compile FAIL** — add `mod whitespace;` to `crates/buiy_core/src/text/mod.rs`, then `cargo test -p buiy_core --lib text::whitespace` → `CollapseMode` / `collapse_whitespace` not found.

- [x] **Step 3: Implement** — fill `crates/buiy_core/src/text/whitespace.rs` above the tests:

```rust
//! The white-space collapse pre-pass (measure-and-layout § 5.2).
//!
//! cosmic-text lays out the source string VERBATIM, so CSS-default
//! collapsing must happen before `set_text` or measured widths include
//! literal runs of spaces. A pure `&str → Cow<str>` transform, run inside
//! `TextSync` immediately before `set_text`, parameterized by the collapse
//! mode. Rules per CSS Text Level 3 § 4.1 phase I.
//!
//! T2 always uses [`CollapseMode::Collapse`] (the `white-space: normal`
//! initial); T3 lands the white-space carrier and the full
//! (collapse mode × `Wrap`) value-table mapping, and the mode joins the
//! intrinsic-cache content version (measure § 3.2). The § 5.4 direction
//! strong-mark prepend (T5) runs AFTER this transform, so the trim sees the
//! authored leading/trailing spaces, never the mark.

use std::borrow::Cow;

/// CSS Text Level 3 § 4.1 phase-I collapse modes — the left column of the
/// § 5.2 white-space value table (`normal`/`nowrap` → `Collapse`;
/// `pre`/`pre-wrap` → `Preserve`; `pre-line` → `PreserveBreaks`). The
/// carrier component selecting a mode is T3's.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CollapseMode {
    /// Segment breaks (LF, CR, CRLF — normalized first) and tabs each
    /// become a collapsible space; runs of collapsible spaces collapse to
    /// one; leading and trailing collapsible spaces are trimmed. The result
    /// reaches cosmic-text as ONE logical line — soft wrapping, if any, is
    /// `Wrap`'s job.
    Collapse,
    /// Nothing collapses; segment breaks become hard line breaks
    /// (cosmic-text buffer lines, normalized to LF); tabs pass through
    /// untouched to cosmic-text's tab stops (`set_tab_width(8)` at
    /// `TextSync` — the CSS `tab-size` initial).
    Preserve,
    /// Segment breaks become hard line breaks; spaces and tabs collapse as
    /// in [`CollapseMode::Collapse`] within each segment.
    PreserveBreaks,
}

/// Apply the phase-I transform. Borrows through (`Cow::Borrowed`) when the
/// input needs no rewrite, so steady-state sync of plain words allocates
/// nothing.
pub fn collapse_whitespace(text: &str, mode: CollapseMode) -> Cow<'_, str> {
    match mode {
        CollapseMode::Collapse => collapse_all(text),
        CollapseMode::Preserve => normalize_segment_breaks(text),
        CollapseMode::PreserveBreaks => preserve_breaks(text),
    }
}

/// The collapsible set: spaces, tabs, and segment-break characters. NOT
/// other whitespace (U+00A0 no-break space etc. pass through).
fn is_collapsible(c: char) -> bool {
    matches!(c, ' ' | '\t' | '\n' | '\r')
}

/// Does `text` need any collapse-mode rewrite? (Leading/trailing space,
/// any tab or break, or a multi-space run.)
fn needs_collapse(text: &str) -> bool {
    let bytes = text.as_bytes();
    if bytes.first() == Some(&b' ') || bytes.last() == Some(&b' ') {
        return true;
    }
    let mut prev_space = false;
    for &byte in bytes {
        match byte {
            b'\t' | b'\n' | b'\r' => return true,
            b' ' => {
                if prev_space {
                    return true;
                }
                prev_space = true;
            }
            _ => prev_space = false,
        }
    }
    false
}

fn collapse_all(text: &str) -> Cow<'_, str> {
    if !needs_collapse(text) {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut pending_space = false;
    for c in text.chars() {
        if is_collapsible(c) {
            // A leading run never sets pending: it trims away. CRLF folds
            // naturally — both chars land in the same pending run.
            pending_space = !out.is_empty();
        } else {
            if pending_space {
                out.push(' ');
                pending_space = false;
            }
            out.push(c);
        }
    }
    // A trailing pending run is dropped: the trim.
    Cow::Owned(out)
}

/// CR and CRLF → LF, nothing else touched (the `Preserve` whole-mode and
/// the first step of `PreserveBreaks`).
fn normalize_segment_breaks(text: &str) -> Cow<'_, str> {
    if !text.contains('\r') {
        return Cow::Borrowed(text);
    }
    let mut out = String::with_capacity(text.len());
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            if chars.peek() == Some(&'\n') {
                chars.next();
            }
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    Cow::Owned(out)
}

fn preserve_breaks(text: &str) -> Cow<'_, str> {
    let normalized = normalize_segment_breaks(text);
    if !normalized.split('\n').any(needs_collapse) {
        return normalized;
    }
    let rebuilt = normalized
        .split('\n')
        .map(collapse_all)
        .collect::<Vec<_>>()
        .join("\n");
    Cow::Owned(rebuilt)
}
```

Add to the `mod.rs` re-export block:

```rust
pub use whitespace::{CollapseMode, collapse_whitespace};
```

- [x] **Step 4: Run the tests, expect PASS** — `cargo test -p buiy_core --lib text::whitespace` → 6 passed.

- [ ] **Step 5: Run GATE. Commit:** `feat(text): white-space collapse pre-pass (CSS Text L3 § 4.1 phase I, § 5.2 value table)`

---

## Task 5 — `BuiyLayoutStep::TextSync` + the order pin

The named layout sub-step between `WritingModeInherit` and `SyncStyles` (architecture § 4.1 — named, not a bare `.before()`, "for the same reason layout named all eleven steps"). The set lands now; its system body is Task 6.

**Files:**
- Modify: `crates/buiy_core/src/layout/pipeline.rs`
- Test: `crates/buiy_core/tests/layout_pipeline_order.rs`

- [x] **Step 1: Write the failing test changes.** In `crates/buiy_core/tests/layout_pipeline_order.rs`, inside `layout_steps_are_chained_in_declared_order`, add a tracker between the `"wmi"` and `"sync"` registrations:

```rust
    app.add_systems(
        Update,
        make_tracker(o.clone(), "text_sync").in_set(BuiyLayoutStep::TextSync),
    );
```

Change the count assertion from `n, 9,` to `n, 10,` (and the `"expected exactly one full pipeline cycle ({} entries)"` first format arg from `9` to `10`), and the expected array to:

```rust
    assert_eq!(
        observed,
        &[
            "gc",
            "wmi",
            "text_sync",
            "sync",
            "cq_activate",
            "taffy",
            "cq_flip",
            "cq_rerun",
            "post_taffy",
            "write",
        ],
        "BuiyLayoutStep sets did not run in declared order; full trace: {:?}",
        observed_full,
    );
```

Also update the file's first doc line from `//! 9-step pipeline order asserted at the integration level.` to `//! Layout pipeline order asserted at the integration level (text T2 grows the TextSync step).`

- [x] **Step 2: Run, expect compile FAIL** — `cargo test -p buiy_core --test layout_pipeline_order` → no variant `TextSync` on `BuiyLayoutStep`.

- [x] **Step 3: Implement.** In `crates/buiy_core/src/layout/pipeline.rs`:

Insert the variant between `WritingModeInherit` and `SyncStyles`:

```rust
    /// Pre-step-1 (text) — create/update `TextBuffer` from the authored
    /// text components via the 0.19 lazy setters (lock-free) and mark the
    /// entity's Taffy node dirty when content changed (Taffy caches measure
    /// results — an un-dirtied node serves a stale measurement). After
    /// `WritingModeInherit` (the trigger union includes
    /// `Changed<WritingModeResolved>`), hard before `SyncStyles` (which
    /// must know whether an entity is a measured text leaf when creating
    /// its Taffy node — the T3 context migration).
    /// **Text T2** (text architecture § 4.1; measure-and-layout § 4.1).
    TextSync,
```

Add `BuiyLayoutStep::TextSync,` to the `configure_pipeline` chain tuple, between `WritingModeInherit` and `SyncStyles`. Update the module doc's first paragraph: "Eleven ordered sub-sets" → "Twelve ordered sub-sets", and append a sentence: "Text T2 inserts `TextSync` between `WritingModeInherit` and `SyncStyles` (text architecture § 4.1); text T3 appends `TextCommit` as the new final step." Update the stale `/// Configure the 9-step chain inside BuiySet::Layout.` doc on `configure_pipeline` to `/// Configure the ordered step chain inside BuiySet::Layout.`

- [x] **Step 4: Run the tests, expect PASS** — `cargo test -p buiy_core --test layout_pipeline_order` → all pass (the other tests in the file are untouched — they track steps 7–9 only).

- [ ] **Step 5: Run GATE. Commit:** `feat(layout): BuiyLayoutStep::TextSync step between WritingModeInherit and SyncStyles`

---

## Task 6 — `text_sync_buffers`: creation + the trigger union + the `FontsGeneration` sweep

The step body (measure-and-layout § 4.1; architecture § 5.1 row 1). No Taffy interaction yet (Task 7); no lock anywhere (lazy setters only).

**Files:**
- Create: `crates/buiy_core/src/text/sync.rs`
- Modify: `crates/buiy_core/src/text/mod.rs`
- Modify: `crates/buiy_core/src/lib.rs`, `crates/buiy/src/lib.rs` (re-export `TextSyncAppliedCount`)
- Create: `crates/buiy_core/tests/text_sync.rs`

- [x] **Step 1: Write the failing tests** — create `crates/buiy_core/tests/text_sync.rs`:

```rust
//! `BuiyLayoutStep::TextSync` trigger-set tests (architecture § 5.1 row 1;
//! measure-and-layout § 4.1).
//!
//! Headless — TextSync never locks `SharedFontSystem` (the 0.19 lazy
//! setters); nothing in this file shapes, measures, or rasterizes (T3/T4).

use bevy::prelude::*;
use buiy_core::layout::{Direction, LayoutPlugin, ScrollOffset, Style, WritingMode};
use buiy_core::text::{
    BuiyTextPlugin, FamilyEntry, FontFamily, FontSize, FontStack, FontWeight, FontsGeneration,
    Text, TextBuffer, TextSyncAppliedCount,
};
use buiy_core::{CorePlugin, Node};
use cosmic_text::{Metrics, Wrap};

fn text_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins(CorePlugin);
    app.add_plugins(LayoutPlugin);
    app.add_plugins(BuiyTextPlugin::default());
    app
}

fn spawn_text(app: &mut App, content: &str) -> Entity {
    app.world_mut()
        .spawn((Node, Style::default(), Text(String::from(content))))
        .id()
}

/// Run the spawn frame plus the one-shot `Added<TextBuffer>` re-apply frame
/// (the documented deferred-insert echo), landing in steady state.
fn settle(app: &mut App) {
    app.update();
    app.update();
}

fn applied(app: &App) -> usize {
    app.world().resource::<TextSyncAppliedCount>().0
}

fn buffer_lines(app: &App, entity: Entity) -> Vec<String> {
    app.world()
        .get::<TextBuffer>(entity)
        .expect("text entity has a TextBuffer")
        .buffer
        .lines
        .iter()
        .map(|line| line.text().to_owned())
        .collect()
}

#[test]
fn spawning_text_creates_a_buffer_with_collapsed_content() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "hello\n  world");
    app.update();

    assert_eq!(
        buffer_lines(&app, entity),
        vec!["hello world"],
        "the § 5.2 collapse pre-pass runs before set_text (white-space: normal initial)"
    );
    let buffer = app.world().get::<TextBuffer>(entity).unwrap();
    assert!(
        buffer.intrinsics().is_none(),
        "intrinsics start invalidated — the T3 measure closure computes them"
    );
    assert_eq!(
        buffer.buffer.wrap(),
        Wrap::Word,
        "§ 5.2 `normal` row pins Wrap::Word, not Buffer::new_empty's WordOrGlyph default"
    );
}

#[test]
fn unset_style_components_fall_back_to_plugin_defaults() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "default style");
    app.update();
    let metrics = app.world().get::<TextBuffer>(entity).unwrap().buffer.metrics();
    assert_eq!(
        metrics,
        Metrics::relative(16.0, 1.2),
        "TextStyleDefaults.size with the line-height:normal 1.2 stand-in (the carrier is T3's)"
    );
}

#[test]
fn steady_state_applies_zero() {
    let mut app = text_app();
    spawn_text(&mut app, "static");
    settle(&mut app);
    app.update();
    assert_eq!(applied(&app), 0, "no-change frame: TextSync must touch nothing");
}

#[test]
fn text_change_resyncs_only_the_changed_entity() {
    let mut app = text_app();
    let changed = spawn_text(&mut app, "before");
    let _static_peer = spawn_text(&mut app, "peer");
    settle(&mut app);

    app.world_mut().get_mut::<Text>(changed).unwrap().0 = String::from("after  edit");
    app.update();

    assert_eq!(applied(&app), 1, "exactly the Changed<Text> entity");
    assert_eq!(buffer_lines(&app, changed), vec!["after edit"]);
}

#[test]
fn font_size_change_resyncs_and_updates_metrics() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "resize me");
    settle(&mut app);

    app.world_mut().entity_mut(entity).insert(FontSize(24.0));
    app.update();

    assert_eq!(applied(&app), 1, "Changed<FontSize> fires the union");
    let metrics = app.world().get::<TextBuffer>(entity).unwrap().buffer.metrics();
    assert_eq!(metrics, Metrics::relative(24.0, 1.2));
}

#[test]
fn font_weight_and_family_changes_resync() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "restyle me");
    settle(&mut app);

    app.world_mut().entity_mut(entity).insert(FontWeight(700));
    app.update();
    assert_eq!(applied(&app), 1, "Changed<FontWeight> fires the union");

    app.world_mut().entity_mut(entity).insert(FontFamily(FontStack(vec![
        FamilyEntry::Named(String::from("Fira Sans")),
    ])));
    app.update();
    assert_eq!(applied(&app), 1, "Changed<FontFamily> fires the union");
}

#[test]
fn writing_mode_resolved_change_resyncs() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "direction-sensitive");
    settle(&mut app);

    app.world_mut().entity_mut(entity).insert(WritingMode {
        direction: Direction::Rtl,
        ..Default::default()
    });
    app.update();

    assert_eq!(
        applied(&app),
        1,
        "WritingModeInherit rewrites the resolved cache (guarded) before TextSync, \
         and the § 5.1 union consumes Changed<WritingModeResolved> the same frame"
    );
}

#[test]
fn fonts_generation_bump_sweeps_every_buffer() {
    let mut app = text_app();
    spawn_text(&mut app, "one");
    spawn_text(&mut app, "two");
    spawn_text(&mut app, "three");
    settle(&mut app);

    app.world_mut().resource_mut::<FontsGeneration>().0 += 1;
    app.update();
    assert_eq!(
        applied(&app),
        3,
        "a font-set change reshapes EVERY TextBuffer once — late fonts never \
         leave stale tofu (architecture § 2.2)"
    );

    app.update();
    assert_eq!(applied(&app), 0, "the sweep is edge-triggered, never latched");
}

/// The deliberate § 5.1 exclusion: scroll moves glyph rects via transforms;
/// layout and shaping are unchanged.
#[test]
fn scroll_offset_change_is_excluded() {
    let mut app = text_app();
    let entity = app
        .world_mut()
        .spawn((
            Node,
            Style::default(),
            Text(String::from("scrolled")),
            ScrollOffset::default(),
        ))
        .id();
    settle(&mut app);

    app.world_mut().get_mut::<ScrollOffset>(entity).unwrap().y = 42.0;
    app.update();
    assert_eq!(applied(&app), 0, "Changed<ScrollOffset> is not a reshape trigger");
}
```

- [x] **Step 2: Run, expect compile FAIL** — `cargo test -p buiy_core --test text_sync` → no `TextSyncAppliedCount` in `buiy_core::text`.

- [x] **Step 3: Implement** — create `crates/buiy_core/src/text/sync.rs`:

```rust
//! `BuiyLayoutStep::TextSync` — the dirty path (measure-and-layout § 4.1;
//! architecture §§ 4.1, 5.1).
//!
//! Creates/updates `TextBuffer` from the authored components via the 0.19
//! LAZY setters — no `FontSystem`, no lock (architecture § 1.2): mutation
//! is recorded, shaping deferred to the next lock-bearing site (T3's
//! measure closure / `TextCommit`). Invalidates the cached intrinsics and
//! (Task 7 / T3 consumers) dirty-marks the Taffy node.
//!
//! **The trigger-union ledger (architecture § 5.1 row 1).** As specced:
//! `Or<(Changed<Text>, Changed<text-style carriers>, Added<TextBuffer>,
//! Changed<WritingModeResolved>)>` ∪ `FontsGeneration` bump ∪ theme
//! font-token swap. Carriers existing in T2: `Text`, `FontFamily`,
//! `FontSize`, `FontWeight`, `WritingModeResolved`, `FontsGeneration`.
//! Members that join with their carriers: line-height / white-space /
//! text-wrap / text-align (**T3**), `TextDirection` (**T5**, with the
//! § 5.4 strong-mark prepend), the theme font-token swap
//! (**buiy-theme-tokens-design**, font-assets § 9).

use bevy::prelude::*;
use cosmic_text::{Attrs, Family, Metrics, Weight, Wrap};

use crate::layout::WritingModeResolved;

use super::components::{
    FamilyEntry, FontFamily, FontSize, FontStack, FontWeight, TEXT_SHAPING, Text, TextBuffer,
    TextStyleDefaults,
};
use super::font_system::FontsGeneration;
use super::whitespace::{CollapseMode, collapse_whitespace};

/// CSS `line-height: normal` stand-in (the common UA factor) until T3 lands
/// the line-height carrier and the measure § 5.1 `Metrics` mapping.
pub(crate) const DEFAULT_LINE_HEIGHT_SCALE: f32 = 1.2;

/// The white-space value table's `normal` row (measure § 5.2): collapse ×
/// `Wrap::Word`. Pinned explicitly — `Buffer::new_empty` defaults to
/// `Wrap::WordOrGlyph` (source-verified), the C-tier `overflow-wrap`
/// behavior, not the CSS initial. T3's white-space/text-wrap carriers
/// drive the full table.
const DEFAULT_WRAP: Wrap = Wrap::Word;

/// CSS `tab-size` initial (measure § 5.2 — "set to 8 … at `TextSync`");
/// the C-tier `tab-size` property later drives the same lazy setter.
const DEFAULT_TAB_WIDTH: u16 = 8;

/// Per-frame count of text entities `text_sync_buffers` applied the lazy
/// setters to — the `SyncStylesIterCount` precedent (layout/systems.rs
/// ~:109). Overwritten (not accumulated) at the top of every invocation.
/// `tests/text_sync.rs` asserts ZERO on a no-change frame (the steady-state
/// half of architecture § 5's contract) and exact counts per § 5.1 trigger.
#[derive(Resource, Default, Debug)]
pub struct TextSyncAppliedCount(pub usize);

/// The § 5.1 row-1 union over the carriers that exist in T2 (the module
/// doc is the ledger for members that join later).
type TextSyncTriggers = Or<(
    Changed<Text>,
    Changed<FontFamily>,
    Changed<FontSize>,
    Changed<FontWeight>,
    Added<TextBuffer>,
    Changed<WritingModeResolved>,
)>;

type SyncedText = (
    &'static Text,
    &'static mut TextBuffer,
    Option<&'static FontFamily>,
    Option<&'static FontSize>,
    Option<&'static FontWeight>,
);

type SyncedTextItem<'w> = (
    &'w Text,
    Mut<'w, TextBuffer>,
    Option<&'w FontFamily>,
    Option<&'w FontSize>,
    Option<&'w FontWeight>,
);

/// The `BuiyLayoutStep::TextSync` body (measure-and-layout § 4.1).
///
/// Registered by `BuiyTextPlugin`; the step set itself is configured
/// (chained) by `LayoutPlugin`'s `configure_pipeline` — standalone
/// `BuiyTextPlugin` apps (the T1 engine tests) run it unordered with empty
/// queries, which is inert.
#[allow(clippy::type_complexity)]
pub fn text_sync_buffers(
    mut commands: Commands,
    defaults: Res<TextStyleDefaults>,
    fonts_generation: Res<FontsGeneration>,
    mut applied: ResMut<TextSyncAppliedCount>,
    mut synced: ParamSet<(
        Query<SyncedText, TextSyncTriggers>,
        Query<SyncedText>,
    )>,
    unsynced: Query<
        (
            Entity,
            &Text,
            Option<&FontFamily>,
            Option<&FontSize>,
            Option<&FontWeight>,
        ),
        Without<TextBuffer>,
    >,
) {
    applied.0 = 0;
    // Explicit reborrows: `Res`/`ResMut` do not deref-coerce in struct
    // field position.
    let mut ctx = SyncContext {
        defaults: &*defaults,
        applied: &mut *applied,
    };

    // Creation: a `Text` entity without a buffer gets one built and FULLY
    // populated this frame (TextSync precedes SyncStyles, so the deferred
    // insert is visible to the same frame's style sync — text never appears
    // a frame late). The insertion tick fires next frame's
    // `Added<TextBuffer>` arm once more: an idempotent lazy re-apply,
    // before any shaping consumer exists (documented; tests `settle()`
    // across it).
    for (entity, text, family, size, weight) in &unsynced {
        let style = AuthoredStyle::resolve(ctx.defaults, family, size, weight);
        let mut buffer = TextBuffer::new(style.metrics());
        apply_authored(&mut buffer, text, &style);
        commands.entity(entity).insert(buffer);
        ctx.applied.0 += 1;
    }

    // A FontsGeneration bump sweeps EVERY buffer — late fonts never leave
    // stale tofu (architecture § 2.2). Otherwise only the trigger set runs.
    // (`is_added` excluded: the plugin-init frame has no buffers to sweep.)
    if fonts_generation.is_changed() && !fonts_generation.is_added() {
        let mut all = synced.p1();
        for item in all.iter_mut() {
            sync_one(item, &mut ctx);
        }
    } else {
        let mut triggered = synced.p0();
        for item in triggered.iter_mut() {
            sync_one(item, &mut ctx);
        }
    }
}

struct SyncContext<'a> {
    defaults: &'a TextStyleDefaults,
    applied: &'a mut TextSyncAppliedCount,
}

fn sync_one(item: SyncedTextItem<'_>, ctx: &mut SyncContext<'_>) {
    let (text, mut buffer, family, size, weight) = item;
    // EVERY in-place TextBuffer write bypasses change detection
    // (measure-and-layout § 7): a sync write is not a damage signal —
    // damage keys on the commit outputs; `Changed<TextBuffer>` is reserved
    // for nothing (tests/text_sync.rs pins it never fires past insertion).
    let buffer = buffer.bypass_change_detection();
    let style = AuthoredStyle::resolve(ctx.defaults, family, size, weight);
    apply_authored(buffer, text, &style);
    ctx.applied.0 += 1;
}

/// The authored style snapshot TextSync lowers into cosmic-text state.
/// Unset components fall back to `TextStyleDefaults` (font-assets § 8).
struct AuthoredStyle<'a> {
    family: &'a FontStack,
    size: f32,
    weight: u16,
}

impl<'a> AuthoredStyle<'a> {
    fn resolve(
        defaults: &'a TextStyleDefaults,
        family: Option<&'a FontFamily>,
        size: Option<&FontSize>,
        weight: Option<&FontWeight>,
    ) -> Self {
        Self {
            family: family.map_or(&defaults.family, |component| &component.0),
            size: size.map_or(defaults.size, |component| component.0),
            weight: weight.map_or(defaults.weight, |component| component.0),
        }
    }

    /// font-size → `Metrics`, with the line-height stand-in (T3 lands the
    /// carrier and the measure § 5.1 mapping).
    fn metrics(&self) -> Metrics {
        Metrics::relative(self.size, DEFAULT_LINE_HEIGHT_SCALE)
    }

    /// Lower to the per-buffer `Attrs`. T2 INTERIM: the stack's FIRST entry
    /// only — the Buiy-owned resolver (fontdb `Query` walk, coverage
    /// span-splitting, `unicode-range`) is T5's (font-assets § 6); until it
    /// lands, misses fall through to cosmic-text's `FontFallbackIter` and
    /// the deterministic `BuiyFallback`.
    fn attrs(&self) -> Attrs<'_> {
        Attrs::new()
            .family(self.first_family())
            .weight(Weight(self.weight))
    }

    fn first_family(&self) -> Family<'_> {
        match self.family.0.first() {
            Some(FamilyEntry::Named(name)) => Family::Name(name),
            Some(FamilyEntry::Generic(generic)) => generic.to_cosmic(),
            // An empty authored stack degrades to the pinned sans-serif
            // generic rather than panicking or skipping the entity.
            None => Family::SansSerif,
        }
    }
}

/// Apply authored content + style through the 0.19 LAZY setters — no
/// FontSystem, no lock; shaping deferred (architecture §§ 1.2, 3.2).
///
/// `alignment: None` = CSS `start` (the § 5.3 table: cosmic-text's
/// unaligned default follows the line's BiDi direction); the align carrier
/// is T3's. The § 5.4 direction strong-mark prepend (T5) slots between the
/// collapse transform and `set_text`, AFTER the trim.
fn apply_authored(buffer: &mut TextBuffer, text: &Text, style: &AuthoredStyle<'_>) {
    // T2 pins the CSS `white-space: normal` initial (collapse mode); T3's
    // carrier selects across the full § 5.2 value table.
    let collapsed = collapse_whitespace(&text.0, CollapseMode::Collapse);
    buffer.buffer.set_metrics(style.metrics());
    buffer.buffer.set_wrap(DEFAULT_WRAP);
    buffer.buffer.set_tab_width(DEFAULT_TAB_WIDTH);
    buffer
        .buffer
        .set_text(&collapsed, &style.attrs(), TEXT_SHAPING, None);
    buffer.invalidate_intrinsics();
}
```

- [x] **Step 4: Wire the plugin.** In `crates/buiy_core/src/text/mod.rs`: add `mod sync;` and `pub use sync::{TextSyncAppliedCount, text_sync_buffers};`. In `BuiyTextPlugin::build`, after the Task-1 registrations:

```rust
        app.init_resource::<TextSyncAppliedCount>();
        // The TextSync step body (measure-and-layout § 4.1). The
        // BuiyLayoutStep::TextSync set is configured by LayoutPlugin's
        // configure_pipeline; without LayoutPlugin (the T1 standalone
        // tests) the system runs unordered with empty queries — inert.
        app.add_systems(
            Update,
            text_sync_buffers.in_set(crate::layout::BuiyLayoutStep::TextSync),
        );
```

Root re-exports: `TextSyncAppliedCount` joins `crates/buiy_core/src/lib.rs`'s `pub use text::{…}` and `crates/buiy/src/lib.rs`'s `text::{…}` group.

- [x] **Step 5: Run the tests, expect PASS** — `cargo test -p buiy_core --test text_sync` → 9 passed. Also re-run `--test text_engine --test text_system_scan --test text_components` (the T1 standalone apps now run `text_sync_buffers`; all must stay green).

- [ ] **Step 6: Run GATE. Commit:** `feat(text): text_sync_buffers — § 5.1 trigger union + FontsGeneration sweep (lock-free lazy setters)`

---

## Task 7 — Taffy dirty-mark + `Text`-removal cleanup

The `mark_dirty` half of § 4.1 ("Taffy caches measure results, so an un-dirtied node serves a stale measurement") and the buffer-lifecycle removal edge.

**Files:**
- Modify: `crates/buiy_core/src/layout/tree.rs`
- Modify: `crates/buiy_core/src/text/sync.rs`
- Test: `crates/buiy_core/tests/text_sync.rs`

- [x] **Step 1: Write the failing tests.** Append to `crates/buiy_core/tests/text_sync.rs` (extend the existing imports with `buiy_core::layout::{BuiyLayoutStep, LayoutTree}`):

```rust
#[derive(Resource, Default)]
struct DirtyProbe(Option<bool>);

/// Reads the text entity's Taffy dirtiness BETWEEN TextSync and SyncStyles
/// (the only window where the mark is observable — TaffyCompute clears it
/// by computing later the same frame).
fn probe_text_node_dirtiness(
    tree: Option<NonSend<LayoutTree>>,
    texts: Query<Entity, With<TextBuffer>>,
    mut out: ResMut<DirtyProbe>,
) {
    out.0 = None;
    let Some(tree) = tree else { return };
    let Ok(entity) = texts.single() else { return };
    let Some(&node) = tree.by_entity().get(&entity) else {
        return;
    };
    out.0 = Some(tree.tree_ref().dirty(node).expect("node is live"));
}

#[test]
fn text_change_marks_the_taffy_node_dirty_before_styles_sync() {
    let mut app = text_app();
    app.init_resource::<DirtyProbe>();
    app.add_systems(
        Update,
        probe_text_node_dirtiness
            .after(BuiyLayoutStep::TextSync)
            .before(BuiyLayoutStep::SyncStyles),
    );
    let entity = spawn_text(&mut app, "measure me");
    settle(&mut app);

    app.update();
    assert_eq!(
        app.world().resource::<DirtyProbe>().0,
        Some(false),
        "steady frame: the node serves Taffy's cache"
    );

    app.world_mut().get_mut::<Text>(entity).unwrap().0 = String::from("longer content now");
    app.update();
    assert_eq!(
        app.world().resource::<DirtyProbe>().0,
        Some(true),
        "content change must invalidate Taffy's leaf cache — the only lever \
         is mark_dirty; set_style is never called for a pure text change \
         (architecture § 4.1)"
    );
}

#[test]
fn removing_text_drops_the_buffer() {
    let mut app = text_app();
    let entity = spawn_text(&mut app, "ephemeral");
    settle(&mut app);
    assert!(app.world().get::<TextBuffer>(entity).is_some());

    app.world_mut().entity_mut(entity).remove::<Text>();
    app.update();
    assert!(
        app.world().get::<TextBuffer>(entity).is_none(),
        "a Text-less entity stops being a text leaf; the buffer dies on the edge"
    );
}
```

- [x] **Step 2: Run, expect FAIL** — `cargo test -p buiy_core --test text_sync` → `text_change_marks_the_taffy_node_dirty_before_styles_sync` fails (probe reads `Some(false)` on the mutation frame: nothing marks dirty yet) and `removing_text_drops_the_buffer` fails (`TextBuffer` still present).

- [x] **Step 3: Implement the tree helper.** In `crates/buiy_core/src/layout/tree.rs`, add to `impl LayoutTree`:

```rust
    /// Mark the Taffy node for `entity` dirty (recursive to ancestors —
    /// taffy_tree.rs:873). Taffy caches measure results; an un-dirtied node
    /// serves a stale measurement (text architecture § 4.1). No-op when the
    /// entity has no node yet — a brand-new text leaf's node is created
    /// fresh by `sync_styles` later the same frame, dirty by construction
    /// (text measure-and-layout § 2.2).
    pub(crate) fn mark_dirty_for_entity(&mut self, entity: Entity) {
        if let Some(&node) = self.by_entity.get(&entity) {
            self.tree
                .mark_dirty(node)
                .expect("LayoutTree: by_entity points at a live Taffy node");
        }
    }
```

- [x] **Step 4: Wire it into the system.** In `crates/buiy_core/src/text/sync.rs`:

Change the imports:

```rust
use crate::layout::{LayoutTree, WritingModeResolved};

use super::components::{
    ComputedTextLayout, FamilyEntry, FontFamily, FontSize, FontStack, FontWeight, TEXT_SHAPING,
    Text, TextBuffer, TextStyleDefaults,
};
```

Grow the query data tuples with the entity id (first position):

```rust
type SyncedText = (
    Entity,
    &'static Text,
    &'static mut TextBuffer,
    Option<&'static FontFamily>,
    Option<&'static FontSize>,
    Option<&'static FontWeight>,
);

type SyncedTextItem<'w> = (
    Entity,
    &'w Text,
    Mut<'w, TextBuffer>,
    Option<&'w FontFamily>,
    Option<&'w FontSize>,
    Option<&'w FontWeight>,
);
```

Grow the system signature with two params (after `applied`):

```rust
    mut tree: Option<NonSendMut<LayoutTree>>,
    mut removed_texts: RemovedComponents<Text>,
```

Grow `SyncContext` and its construction:

```rust
struct SyncContext<'a> {
    defaults: &'a TextStyleDefaults,
    tree: Option<&'a mut LayoutTree>,
    applied: &'a mut TextSyncAppliedCount,
}
```

```rust
    let mut ctx = SyncContext {
        defaults: &*defaults,
        tree: tree.as_deref_mut(),
        applied: &mut *applied,
    };
```

Update `sync_one`:

```rust
fn sync_one(item: SyncedTextItem<'_>, ctx: &mut SyncContext<'_>) {
    let (entity, text, mut buffer, family, size, weight) = item;
    // EVERY in-place TextBuffer write bypasses change detection
    // (measure-and-layout § 7): a sync write is not a damage signal —
    // damage keys on the commit outputs; `Changed<TextBuffer>` is reserved
    // for nothing (tests/text_sync.rs pins it never fires past insertion).
    let buffer = buffer.bypass_change_detection();
    let style = AuthoredStyle::resolve(ctx.defaults, family, size, weight);
    apply_authored(buffer, text, &style);
    if let Some(tree) = ctx.tree.as_deref_mut() {
        // Absent tree (standalone BuiyTextPlugin, no LayoutPlugin): nothing
        // measures, nothing to dirty.
        tree.mark_dirty_for_entity(entity);
    }
    ctx.applied.0 += 1;
}
```

(the two `for` loops need no change — they pass `item` whole and `sync_one` destructures the now-larger tuple). Append the removal arm at the end of `text_sync_buffers`:

```rust
    // `Text` removed while the entity lives: the leaf stops being a text
    // leaf — drop the buffer and the (T3-written) commit output. Despawned
    // entities clean up for free; `get_entity` filters them out here. The
    // Taffy `set_node_context` unregistration on this same edge is T3's
    // (measure § 2.2 — it lands with the TaffyTree<Entity> migration).
    for entity in removed_texts.read() {
        if let Ok(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.remove::<(TextBuffer, ComputedTextLayout)>();
        }
    }
```

- [x] **Step 5: Run the tests, expect PASS** — `cargo test -p buiy_core --test text_sync` → 11 passed. (Deviation: the grown 8-param signature trips `clippy::too_many_arguments` under `-D warnings`; added it to the existing `#[allow(clippy::type_complexity)]` — the `extract.rs`/`layout/systems.rs` multi-param-system precedent.)

- [ ] **Step 6: Run GATE. Commit:** `feat(text): TextSync Taffy dirty-mark + Text-removal buffer cleanup`

---

## Task 8 — Pin the bypass discipline: `Changed<TextBuffer>` never fires

The charter's headline T2 test surface: "Changed-gated reshape (`Changed<TextBuffer>` never fires — the bypass discipline)". A pure test task — if it fails, the bug is in Task 6/7's `bypass_change_detection` placement; fix there, never by loosening the test.

**Files:**
- Test: `crates/buiy_core/tests/text_sync.rs`

- [x] **Step 1: Write the test.** Append to `crates/buiy_core/tests/text_sync.rs` (extend imports with `buiy_core::BuiySet`):

```rust
#[derive(Resource, Default)]
struct BufferTickCount(usize);

fn count_buffer_ticks(
    mut count: ResMut<BufferTickCount>,
    changed: Query<(), Changed<TextBuffer>>,
) {
    count.0 += changed.iter().count();
}

/// measure-and-layout § 7: author intent rides `Changed<Text>` + the style
/// carriers; `Changed<TextBuffer>` is reserved for NOTHING. The only tick
/// ever observed is the insertion tick (the `Added<TextBuffer>` edge the
/// § 5.1 union itself consumes) — every later in-place mutation routes
/// through `Mut::bypass_change_detection`.
#[test]
fn changed_text_buffer_never_fires_after_insertion() {
    let mut app = text_app();
    app.init_resource::<BufferTickCount>();
    app.add_systems(Update, count_buffer_ticks.after(BuiySet::Layout));

    let entity = spawn_text(&mut app, "tick discipline");
    settle(&mut app);
    assert_eq!(
        app.world().resource::<BufferTickCount>().0,
        1,
        "exactly the insertion tick — the frame-2 Added re-apply is bypassed"
    );

    app.world_mut().get_mut::<Text>(entity).unwrap().0 = String::from("rewritten in place");
    app.update();
    app.world_mut().entity_mut(entity).insert(FontSize(32.0));
    app.update();
    app.world_mut().resource_mut::<FontsGeneration>().0 += 1;
    app.update();

    assert_eq!(
        app.world().resource::<BufferTickCount>().0,
        1,
        "content edits, carrier changes, and the generation sweep all mutate \
         the buffer WITHOUT ticking it — the O(0) steady-state contract \
         would die by a thousand downstream filters otherwise (measure § 7)"
    );
}
```

- [x] **Step 2: Run, expect PASS** — `cargo test -p buiy_core --test text_sync changed_text_buffer_never_fires_after_insertion`. If it FAILS: a write path is missing `bypass_change_detection` (or an `insert` of `TextBuffer` happens past creation) — fix the producer in `sync.rs`, re-run. This test exists to catch exactly that regression from any future editor of the sync path.

- [ ] **Step 3: Run GATE. Commit:** `test(text): pin the bypass discipline — Changed<TextBuffer> never fires past insertion`

---

## Task 9 — Docs-with-change + plan self-review

**Files:**
- Modify: `docs/plans/2026-06-09-buiy-text-campaign.md`
- Modify: `docs/README.md`
- Modify: `docs/plans/2026-06-10-buiy-text-t2-component-buffer-lifecycle.md` (this file)

- [x] **Step 1: Campaign flip.** In `docs/plans/2026-06-09-buiy-text-campaign.md`, Phase status table: `| T2 | Text component + Buffer lifecycle | in progress |` → `| T2 | Text component + Buffer lifecycle | landed |`.

- [x] **Step 2: Campaign errata note (only what was actually found).** Following the T1 precedent, append a short "T2 errata for the spec edit pass" list under the campaign's T2 phase entry. Known member from this plan (Decision 14): *architecture § 5.1's TextSync row has no carrier-removal member — removing a style carrier (e.g. `FontSize`) does not resync until the next other trigger; the spec edit pass should either add the `RemovedComponents` arms or pin the exclusion.* Add any further mechanical inaccuracies discovered during implementation; if none beyond this, record just this one.

- [x] **Step 3: Docs index row.** In `docs/README.md` § Text → **Plans**, after the T1 row:

```markdown
- [Buiy text T2 — Text component + Buffer lifecycle](plans/2026-06-10-buiy-text-t2-component-buffer-lifecycle.md) — the retained `TextBuffer` (0.19 lazy setters, `Shaping::Advanced` pin, bypass-change-detection discipline), `Text` + `FontFamily`/`FontSize`/`FontWeight` + `TextStyleDefaults` plugin defaults, `TextColor` (`CurrentColor` default), the `ComputedTextLayout` output type, the white-space collapse pre-pass (§ 5.2 value table), `BuiyLayoutStep::TextSync` (trigger union incl. the `FontsGeneration` sweep, intrinsics invalidation, Taffy `mark_dirty`, `Text`-removal cleanup). Measure/`TextCommit` are T3. `[landed]`
```

- [x] **Step 4: Flip this plan's Status** from `active` to `landed`.

- [x] **Step 5: Implementation-vs-plan review.** Re-read this plan's **Decisions** list against the landed code: every numbered decision must be visible in the code or its doc comments (the union ledger in `sync.rs`'s module doc, the bypass rationale at the write site, the interim-lowering note on `AuthoredStyle::attrs`, the seams table below). Fix drift in code comments, not by rewriting history here.

- [ ] **Step 6: Run GATE** (docs-only change; the gate confirms nothing drifted). **Commit:** `docs(text): T2 component + buffer lifecycle landed — campaign/index flips, errata note`

---

## Done criteria

- [ ] Gate green at every task boundary; **zero** new `#[ignore]` tests; **zero** new dependencies (`Cargo.toml`/`Cargo.lock` dependency sets unchanged); the GPU lane untouched.
- [ ] `Text`, `FontFamily(FontStack)`, `FontSize`, `FontWeight` authored components + `TextStyleDefaults` resource (sans-serif / 16 px / 400, single-sourced from component defaults), reflect-registered by `BuiyTextPlugin`.
- [ ] `TextColor(ColorToken)` in `render/components.rs`, default `CurrentColor`, registered with the other author-set render components.
- [ ] `TextBuffer { pub buffer, intrinsics }` retained component: `Buffer::new_empty` construction (lock-free), `Shaping::Advanced` pinned via `TEXT_SHAPING`, public `intrinsics()` read + crate-internal invalidation; `ComputedTextLayout`/`ComputedTextLine` types with the documented idempotent-write contract (writer + idempotency test = T3).
- [ ] `collapse_whitespace` pure function: all three § 5.2 modes, borrow-through fast path, value-table unit tests (collapse/trim/CRLF/nbsp/preserve/pre-line).
- [ ] `BuiyLayoutStep::TextSync` chained `WritingModeInherit → TextSync → SyncStyles`; `tests/layout_pipeline_order.rs` asserts 10 tracked labels with `text_sync` after `wmi`.
- [ ] `text_sync_buffers`: creation arm (same-frame content), the T2 trigger union (`Text`/`FontFamily`/`FontSize`/`FontWeight`/`Added<TextBuffer>`/`WritingModeResolved`), the `FontsGeneration` sweep, `Wrap::Word` + `set_tab_width(8)` + `alignment: None` pins, intrinsics invalidation, `mark_dirty_for_entity`, `RemovedComponents<Text>` cleanup — all lock-free.
- [ ] Trigger-set tests green per § 5.1 row: steady-state zero, per-carrier counts, sweep count, `ScrollOffset` exclusion, dirty-probe, removal, and the bypass pin (`Changed<TextBuffer>` fires exactly once — insertion).
- [ ] T1's standalone-plugin tests (`text_engine`, `text_system_scan`) still green with the new system registered.
- [ ] Campaign T2 row + docs/README row + this plan's Status flipped to landed; the carrier-removal spec gap recorded as T2 errata.

## Seams named here, built later (do NOT build in T2)

| Seam | Where named | Built in |
|---|---|---|
| `TextBufferAccess` QueryData (display/edit dispatch, measure § 2.3) | `TextBuffer` doc | T3 (measure closure + `TextCommit`; read-only form consumed by T4's producer; the `edit` arm with `TextEditState` in the `buiy-text-editing` campaign) |
| `IntrinsicWidths` computation + the content-version cache key | `TextBuffer.intrinsics` / `IntrinsicWidths` docs | T3 (measure § 3.2) |
| `ComputedTextLayout` writer + idempotency test | `ComputedTextLayout` doc | T3 (`TextCommit`; campaign moved the test there explicitly) |
| `ResolvedBaseline` output component | `ComputedTextLine::line_y` doc | T3 (measure § 6) |
| line-height carrier → `Metrics` (replaces `DEFAULT_LINE_HEIGHT_SCALE`) | `sync.rs` const doc | T3 (measure § 5.1) |
| white-space/text-wrap carriers → the (mode × `Wrap`) table + balance/pretty/stable degrade | `CollapseMode` / `DEFAULT_WRAP` docs | T3 (measure § 5.2) |
| text-align carrier (`set_text`'s `Option<Align>` stays `None` = `start`) | `apply_authored` doc | T3 (measure § 5.3, applied at commit) |
| `set_node_context` registration/unregistration on `Added`/`Removed<Text>` | removal-arm comment; `mark_dirty_for_entity` doc | T3 (`TaffyTree<Entity>` migration, measure § 2.2) |
| `TextDirection` + the § 5.4 strong-mark prepend (slot: after collapse, before `set_text`) | `whitespace.rs` module doc; `apply_authored` doc | T5 (campaign T5 deliverable) |
| The `FontStack` resolver (fontdb `Query` walk, span-splitting, `unicode-range`) | `FontStack` / `AuthoredStyle::attrs` docs | T5 (font-assets § 6) |
| Theme font-token swap member of the TextSync union | `sync.rs` module-doc ledger | `buiy-theme-tokens-design` (font-assets § 9) |
| `TextColor` resolution at extract (straight-alpha into `GlyphAlphaInstance`) | `TextColor` doc | T4 (glyph-pipeline § 7) |
| `debug_assert!` no visible dirty-unshaped buffer at extract | architecture § 3.2 (not referenced in T2 code) | T4 (the producer) |
| `font-size` keyword table (`small`/`medium`/`large`) | `FontSize` doc | with the styling surface (font-assets § 8 names it; no phase pinned) |

## Plan self-review (performed at authoring, 2026-06-10)

1. **Charter coverage.** Every T2 deliverable maps to a task: retained `TextBuffer` (task 3), font trio + defaults (task 1), `TextSync` step + trigger union + collapse pre-pass + intrinsics invalidation + `mark_dirty` (tasks 5–7), `ComputedTextLayout` type (task 3), `TextColor` (task 2); test surface: order test (task 5), § 5.1 trigger tests (tasks 6–7), bypass pin (task 8). The idempotency test is deliberately absent (moved to T3 by the campaign).
2. **Placeholder scan.** No TBDs; every code step carries the real code; the two "grow the re-export" steps name the exact identifiers.
3. **Type consistency.** `TextSyncAppliedCount(pub usize)` used uniformly; `SyncedText`/`SyncedTextItem` change shape exactly once (Task 7 adds `Entity` to both, plus the two loop destructures); `mark_dirty_for_entity` is the single tree-mutation entry point; `TEXT_SHAPING` referenced from `apply_authored` matches the Task 3 definition.
