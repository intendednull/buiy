//! Buiy text engine: cosmic-text ownership and lifecycle.
//!
//! Spec: `docs/specs/2026-06-09-buiy-text-rendering-design/` — this module is
//! the T1 engine foundation (architecture.md §§ 1–2; font-assets.md §§ 1,
//! 4–5): the shared `FontSystem` resource, the embedded deterministic default
//! font + `BuiyFallback`, the render-world swash cache, and the opt-in
//! background system-font scan with the `FontsGeneration` reshape trigger.
//!
//! T2 (this phase) adds the authored components, `TextBuffer`, and
//! `TextSync`; T3 adds measure + `TextCommit`. Later phases fill the module
//! out further: the `extract_buiy_glyphs` producer (T4), font assets + the
//! `FontStack` resolver + fallback correctness (T5).
//! Campaign: `docs/plans/2026-06-09-buiy-text-campaign.md`.

mod atlas_key;
mod commit;
mod components;
mod decoration;
mod direction;
// pub: `text::edit` is the named editing facade — the boundary every other
// module addresses by path (`crate::text::edit::ReadOnly`, the marker
// registration below; `tests/text_facade_boundary.rs` is the tripwire). Its
// internals (`state`, `access`) stay private behind the re-exports.
pub mod edit;
mod extract;
mod font_asset;
mod font_system;
mod match_index;
// pub(crate): the layout compute sites (taffy_compute + the two cq re-runs)
// call `measure::compute_roots_with_text_measure` directly (measure § 4.3).
pub(crate) mod measure;
mod registry;
mod resolver;
mod stamp;
mod swash;
mod sync;
mod system_scan;
mod visual;
mod whitespace;

pub use atlas_key::{FontKeyInterner, GLYPH_KEY_LEN, glyph_atlas_key};
pub use commit::{TextCommitReshapeCount, text_commit};
pub use components::{
    CaretVisual, ComputedTextLayout, ComputedTextLine, DecorationLineStyle, DecorationLines,
    FamilyEntry, FontFamily, FontSize, FontStack, FontWeight, GenericFamily, IntrinsicWidths,
    LineHeight, ResolvedBaseline, SelectionVisual, TEXT_SHAPING, Text, TextAlign, TextBuffer,
    TextDecorations, TextDirection, TextStyleDefaults, TextWrap, WhiteSpace, resolve_wrap,
};
pub use decoration::{
    DecorationKind, DecorationRect, snap_thickness, snap_y, span_decoration_rects, span_x_extent,
};
pub use direction::prepend_strong_marks;
pub use edit::{
    CaretBlink, CaretMoved, ClickTracker, Disabled, EditCommand, Keymap, Placeholder,
    PointerGesture, ReadOnly, SelectionChanged, SelectionRange, SingleLine, TextBufferAccess,
    TextChanged, TextEditState, TextSelection, apply_keyboard_edits, pointer_selection,
    pointer_to_cursor, write_caret_and_selection,
};
pub use extract::{
    GlyphBearing, GlyphMetaCache, ResidentTextKeys, extract_buiy_glyphs, glyph_rect_logical,
    pack_clip, physical_offset,
};
pub use font_asset::{BuiyFont, BuiyFontLoader, BuiyFontLoaderError, sniff_sfnt};
pub use font_system::{
    BuiyFallback, DEFAULT_FONT_FAMILY, FontDbLineage, FontsGeneration, SharedFontSystem,
    registered_fonts_db,
};
pub use match_index::FontMatchIndex;
pub use measure::{TextMeasureCallCount, TextMeasureParam};
pub use registry::{
    FONT_BLOCK_TIMEOUT_SECS, FontDisplay, FontFaceDescriptors, FontLoadState, FontRegistry,
    PendingFontBlock, UnicodeRanges, apply_font_registry, expire_font_block,
};
pub use resolver::{Resolution, ResolvedFamily, ResolvedSpan, resolve_spans};
pub use stamp::{solid_stamp_bitmap, solid_stamp_key, solid_stamp_warmup_request, stamp_uv};
pub use swash::BuiySwashCache;
pub use sync::{TextSyncAppliedCount, text_sync_buffers};
pub use system_scan::{
    PendingSystemFontScan, apply_system_font_scan, spawn_system_font_scan, swap_font_db,
};
pub use visual::{CaretBlinkInterval, blink_phase, caret_stamp_rect, write_caret_blink};
pub use whitespace::{CollapseMode, collapse_whitespace};

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
        // T5: the fresh-database lineage counter the render-world
        // FontKeyInterner synchronizes against (font-assets § 3.2).
        app.init_resource::<FontDbLineage>();

        // T5: the `@font-face` byte source (font-assets § 2). The asset
        // half is gated: init_asset/register_asset_loader PANIC without an
        // AssetServer (bevy_asset lib.rs:590/637), and headless text
        // fixtures carry no AssetPlugin. The bytes registration path (T5
        // registry) needs no asset machinery at all.
        if app.world().contains_resource::<bevy::asset::AssetServer>() {
            use bevy::asset::AssetApp;
            app.init_asset::<BuiyFont>()
                .register_asset_loader(BuiyFontLoader);
        }

        // T2: the authoring-surface defaults (font-assets § 8) and the
        // author-set component registrations (reflection / BSN / inspectors —
        // the layout convention). The computed text state (TextBuffer,
        // ComputedTextLayout) is deliberately NOT registered, matching the
        // render components.rs convention for computed components.
        app.init_resource::<TextStyleDefaults>();
        app.register_type::<Text>()
            .register_type::<FontFamily>()
            .register_type::<FontSize>()
            .register_type::<FontWeight>()
            .register_type::<LineHeight>()
            .register_type::<WhiteSpace>()
            .register_type::<TextWrap>()
            .register_type::<TextAlign>()
            // T5: the § 5.4 direction carrier (absent = Auto).
            .register_type::<TextDirection>()
            // E1: the four decomposed policy markers (editing-and-ime § 2.2)
            // — authoring-surface components, reflect-registered like `Text`.
            .register_type::<crate::text::edit::ReadOnly>()
            .register_type::<crate::text::edit::Disabled>()
            .register_type::<crate::text::edit::SingleLine>()
            .register_type::<crate::text::edit::Placeholder>()
            // T6: the decoration carrier (decoration-and-paint § 2.2).
            // DecorationLines rides its impl_reflect_opaque! registration
            // (the ContainFlags precedent).
            .register_type::<TextDecorations>()
            .register_type::<DecorationLines>()
            .register_type::<DecorationLineStyle>();

        app.init_resource::<TextSyncAppliedCount>();
        // T3: the per-frame measure instrument (measure § 7). Incremented
        // by the measure closure inside the layout compute sites; reset by
        // `taffy_compute` (the LayoutTaffyComputeCount pattern).
        app.init_resource::<TextMeasureCallCount>();
        // T3: the per-frame commit instrument (spec § 8 item 4).
        app.init_resource::<TextCommitReshapeCount>();
        // The TextSync step body (measure-and-layout § 4.1). The
        // BuiyLayoutStep sets are configured by LayoutPlugin's
        // configure_pipeline; without LayoutPlugin (the T1 standalone
        // tests) the systems run unordered with empty queries — inert.
        app.add_systems(
            Update,
            (
                text_sync_buffers.in_set(crate::layout::BuiyLayoutStep::TextSync),
                // The new FINAL layout step (architecture § 4.2). Inert
                // without LayoutPlugin (Option params return early).
                text_commit.in_set(crate::layout::BuiyLayoutStep::TextCommit),
            ),
        );

        // T7 (decoration-and-paint § 6.3): the caret-blink render-prep
        // writer — the same Animate→Picking window as write_clip_rects /
        // write_paint_skip, so extract reads a settled CaretVisual.
        // Main-world, headless-safe (no RenderApp dependency).
        app.init_resource::<CaretBlinkInterval>();
        app.add_systems(
            Update,
            visual::write_caret_blink
                .after(crate::BuiySet::Animate)
                .before(crate::BuiySet::Picking),
        );

        // E3 (editing-and-ime §§ 4.1, 4.3, 5, 11): the caret + selection
        // geometry writer — mirrors editor state into the T7 paint seats,
        // resets the per-entity blink, emits CaretMoved/SelectionChanged. Runs
        // in the render-prep window, BEFORE write_caret_blink (which reads the
        // CaretBlink origin this system resets). Net order:
        // Input < write_caret_and_selection < write_caret_blink < Picking.
        app.add_message::<crate::text::edit::CaretMoved>();
        app.add_message::<crate::text::edit::SelectionChanged>();
        app.add_systems(
            Update,
            crate::text::edit::write_caret_and_selection
                .after(crate::BuiySet::Input)
                .before(crate::text::visual::write_caret_blink),
        );

        // E2 (editing-and-ime §§ 3, 11): the per-platform keymap (selected
        // once at init by a data swap) and the focus-gated input system.
        // Runs in BuiySet::Input — the `handle_tab` precedent (focus.rs:56),
        // two sets after Layout, so an edit publishes N→N+1 (OQ#1: accepted
        // one-frame latency). The TextChanged Message is registered so
        // consumers (the a11y layer, the widget catalog) can subscribe.
        app.init_resource::<crate::text::edit::Keymap>();
        app.add_message::<crate::text::edit::TextChanged>();
        app.add_systems(
            Update,
            crate::text::edit::apply_keyboard_edits.in_set(crate::BuiySet::Input),
        );

        // E3 (editing-and-ime § 4): the focus-gated mouse-selection system —
        // window→buffer-local mapping → cosmic Click/DoubleClick/TripleClick/
        // Drag, setting FocusedEntity on press. BuiySet::Input, alongside
        // apply_keyboard_edits; inert headless (Option params no-op without
        // mouse/picking infra).
        app.add_systems(
            Update,
            crate::text::edit::pointer_selection.in_set(crate::BuiySet::Input),
        );

        // The poll/swap system is registered UNCONDITIONALLY: it is inert
        // without a PendingSystemFontScan resource (zero steady-state cost),
        // and tests / apps may inject a scan task without the startup flag.
        // Before Layout, so a completed swap (and its FontsGeneration bump)
        // is visible to the same frame's TextSync (T2+).
        app.add_systems(
            Update,
            apply_system_font_scan.before(crate::BuiySet::Layout),
        );

        // T5: the FontRegistry (font-assets § 3) — methods stage ops; ONE
        // applier drains them + the AssetEvent stream per frame, after a
        // possible scan swap and before Layout, so a frame never measures
        // against a half-registered family. The message registration is
        // unconditional (idempotent — bevy_asset lib.rs:656): the reader
        // works without AssetPlugin; the bytes path needs no asset
        // machinery at all.
        app.init_resource::<FontRegistry>();
        // T5: the resolver's lock-free match substrate (decision 2) — a db
        // snapshot every engine-mutation site re-takes under its own lock
        // hold (apply_font_registry, apply_system_font_scan).
        app.insert_resource(FontMatchIndex::new(fonts.lock().db().clone()));
        app.add_message::<bevy::asset::AssetEvent<BuiyFont>>();
        app.add_systems(
            Update,
            (
                apply_font_registry.after(apply_system_font_scan),
                // T5: the font-display Block timeout (font-assets § 7) —
                // removing the expired marker IS the swap-to-visible.
                expire_font_block,
            )
                .before(crate::BuiySet::Layout),
        );
        if self.system_fonts {
            app.add_systems(Startup, spawn_system_font_scan);
        }

        if let Some(render_app) = app.get_sub_app_mut(RenderApp) {
            register_render_world(render_app, &fonts);
        }
    }
}

/// The render-world half of text registration (mirrors `atlas::register`):
/// the `SharedFontSystem` Arc clone — one engine, two worlds (architecture
/// § 1.1; fontdb IDs are stable only within one engine, so a second instance
/// would mis-key every glyph) — plus the T4 glyph producer and its retained
/// state, and the T6 warmup-pinned solid-stamp push (decoration-and-paint
/// § 4.3). `.after(maintain_atlas)` so inserts/touches use the just-advanced
/// atlas frame clock (glyph-pipeline § 6.1; ordering against an absent
/// system set is vacuously satisfied, so a bare `SubApp` without the atlas
/// systems still registers cleanly). Public so the headless `SubApp`
/// registration test (and any external render setup) can drive it without a
/// live `RenderApp`; the live wiring is exercised on the GPU lane from T4.
pub fn register_render_world(render_app: &mut SubApp, fonts: &SharedFontSystem) {
    render_app.insert_resource(fonts.clone());
    render_app.init_resource::<BuiySwashCache>();
    // T6 (decoration-and-paint § 4.3): the warmup-pinned solid stamp — the
    // one committed AtlasWarmupQueue push of the text campaign. This runs
    // inside the live-RenderApp guard (the render architecture § 1.1
    // finish-ordering seam: BuiyPlugin adds this plugin after
    // DefaultPlugins, so the sub-app exists). init_resource is insert-if-
    // absent, so plugin order vs atlas::register is irrelevant.
    render_app.init_resource::<crate::render::atlas::AtlasWarmupQueue>();
    render_app
        .world_mut()
        .resource_mut::<crate::render::atlas::AtlasWarmupQueue>()
        .push(stamp::solid_stamp_warmup_request());
    render_app
        .init_resource::<FontKeyInterner>()
        .init_resource::<ResidentTextKeys>()
        .init_resource::<GlyphMetaCache>()
        .add_systems(
            bevy::render::ExtractSchedule,
            extract::extract_buiy_glyphs.after(crate::render::atlas::maintain_atlas),
        );
}
