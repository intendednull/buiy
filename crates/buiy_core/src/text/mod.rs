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

mod commit;
mod components;
mod font_system;
// pub(crate): the layout compute sites (taffy_compute + the two cq re-runs)
// call `measure::compute_roots_with_text_measure` directly (measure § 4.3).
pub(crate) mod measure;
mod swash;
mod sync;
mod system_scan;
mod whitespace;

pub use commit::{TextCommitReshapeCount, text_commit};
pub use components::{
    ComputedTextLayout, ComputedTextLine, FamilyEntry, FontFamily, FontSize, FontStack, FontWeight,
    GenericFamily, IntrinsicWidths, LineHeight, ResolvedBaseline, TEXT_SHAPING, Text, TextAlign,
    TextBuffer, TextStyleDefaults, TextWrap, WhiteSpace, resolve_wrap,
};
pub use font_system::{
    BuiyFallback, DEFAULT_FONT_FAMILY, FontsGeneration, SharedFontSystem, registered_fonts_db,
};
pub use measure::{TextMeasureCallCount, TextMeasureParam};
pub use swash::BuiySwashCache;
pub use sync::{TextSyncAppliedCount, text_sync_buffers};
pub use system_scan::{
    PendingSystemFontScan, apply_system_font_scan, spawn_system_font_scan, swap_font_db,
};
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
            .register_type::<TextAlign>();

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
    render_app.init_resource::<BuiySwashCache>();
}
