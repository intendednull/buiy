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

mod components;
mod font_system;
mod swash;
mod sync;
mod system_scan;
mod whitespace;

pub use components::{
    ComputedTextLayout, ComputedTextLine, FamilyEntry, FontFamily, FontSize, FontStack, FontWeight,
    GenericFamily, IntrinsicWidths, TEXT_SHAPING, Text, TextBuffer, TextStyleDefaults,
};
pub use font_system::{
    BuiyFallback, DEFAULT_FONT_FAMILY, FontsGeneration, SharedFontSystem, registered_fonts_db,
};
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
            .register_type::<FontWeight>();

        app.init_resource::<TextSyncAppliedCount>();
        // The TextSync step body (measure-and-layout § 4.1). The
        // BuiyLayoutStep::TextSync set is configured by LayoutPlugin's
        // configure_pipeline; without LayoutPlugin (the T1 standalone
        // tests) the system runs unordered with empty queries — inert.
        app.add_systems(
            Update,
            text_sync_buffers.in_set(crate::layout::BuiyLayoutStep::TextSync),
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
