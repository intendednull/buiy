//! Headless, adapterless extract integration (the `MainWorld`-swap idiom that
//! `support::extract_harness` and the `render_focus_ring` / `render_border_shadow`
//! node harnesses use — no wgpu adapter, no `RenderApp`) for two Wave-3 render
//! refinements that the pure-function tests cannot observe end to end:
//!
//! 1. **`AnimatedBackgroundColor` auto-composite** (spec § 2 REFINE): a node
//!    carrying a live `AnimatedBackgroundColor` (the resolved color a
//!    `BackgroundColorTween` writes each frame) paints that interpolated color
//!    OVER its static `Background` token — for EVERY node, no widget opt-in.
//!    The pure resolver `resolve_background_color` is unit-tested in
//!    `render_extract.rs`; here is the production `extract_buiy_nodes` wiring
//!    (the `NodePaintQuery.animated_bg` field + the per-node override).
//! 2. **`Changed<BackdropFilter>` damage-gate term** (parity Wave B4): an
//!    ISOLATED backdrop blur-radius edit (the only paint-input change since the
//!    last extract) must re-extract so the captured `backdrop_blur_px` — the
//!    value prepare plans the dual-Kawase pyramid from — never goes stale.
//!    `Changed<EffectGroup>` only fires when the group FORMS/DROPS (the reason
//!    bitset), so a radius edit on an existing former is the one `NodePaintQuery`
//!    paint input with no matching gate term without this. RED before the term
//!    (the gate early-returns; the poisoned carrier stays empty), GREEN after.

use bevy::prelude::*;
use bevy::render::{ExtractSchedule, MainWorld};
use bevy::window::{PrimaryWindow, WindowResolution};

use buiy_core::animation::AnimatedBackgroundColor;
use buiy_core::layout::{Inset, Length, Sizing, Style};
use buiy_core::render::color::{ColorToken, resolve_token};
use buiy_core::render::components::Background;
use buiy_core::render::extract::{
    ExtractedEffectGroups, ExtractedNode, ExtractedNodesView, extract_buiy_nodes,
};
use buiy_core::theme::Theme;
use buiy_core::{BackdropFilter, FilterFn, Node};
use std::borrow::Cow;

/// Adapterless extract harness: swap the live main world into a bare render
/// world's `MainWorld` slot, run an `ExtractSchedule` carrying the production
/// `extract_buiy_nodes`, swap back, and read the carriers. Mirrors the focus-ring
/// / border-shadow node harnesses (TODO follow-up: lift the shared shape into
/// `tests/support`).
struct NodeExtractHarness {
    app: App,
    render: World,
    schedule: Schedule,
}

impl NodeExtractHarness {
    fn new() -> Self {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(buiy_core::theme::ThemePlugin)
            .add_plugins(buiy_core::CorePlugin)
            .add_plugins(buiy_core::layout::LayoutPlugin)
            .add_plugins(bevy::transform::TransformPlugin)
            // BuiyRenderPlugin's MAIN-world half (write_clip_rects, paint-skip,
            // effect groups, forced colors) registers headless — its render half
            // is guarded on a RenderApp that never exists here, so no adapter.
            .add_plugins(buiy_core::render::BuiyRenderPlugin);
        app.world_mut().spawn((
            Window {
                resolution: WindowResolution::new(640, 480),
                ..Default::default()
            },
            PrimaryWindow,
        ));

        let mut render = World::new();
        render.init_resource::<ExtractedNodesView>();
        render.init_resource::<ExtractedEffectGroups>();
        render.init_resource::<MainWorld>();

        let mut schedule = Schedule::new(ExtractSchedule);
        schedule.add_systems(extract_buiy_nodes);

        Self {
            app,
            render,
            schedule,
        }
    }

    fn update(&mut self) {
        self.app.update();
    }

    fn extract(&mut self) {
        {
            let mut main = self.render.resource_mut::<MainWorld>();
            core::mem::swap(&mut **main, self.app.world_mut());
        }
        self.schedule.run(&mut self.render);
        {
            let mut main = self.render.resource_mut::<MainWorld>();
            core::mem::swap(&mut **main, self.app.world_mut());
        }
    }

    fn node_for(&self, entity: Entity) -> Option<ExtractedNode> {
        self.render
            .resource::<ExtractedNodesView>()
            .0
            .nodes
            .iter()
            .find(|n| n.entity == entity)
            .cloned()
    }

    fn backdrop_px(&self, entity: Entity) -> Option<f32> {
        self.render
            .resource::<ExtractedEffectGroups>()
            .0
            .iter()
            .find(|g| g.entity == entity)
            .and_then(|g| g.backdrop_blur_px)
    }

    fn theme(&self) -> Theme {
        self.app.world().resource::<Theme>().clone()
    }

    /// Overwrite the render-world carriers with the empty default. A steady-state
    /// early-return does NOT touch them (the gate `return`s before any
    /// `insert_resource`), so the empty sentinel survives an extract that
    /// early-returns and is overwritten by one that rebuilds — the detector for
    /// "did the gate re-run?".
    fn poison(&mut self) {
        self.render
            .insert_resource(ExtractedEffectGroups::default());
        self.render.insert_resource(ExtractedNodesView::default());
    }
}

/// Settle layout + transform across a few frames (the bounded spawn-settle the
/// sibling node harnesses use).
fn settle(h: &mut NodeExtractHarness) {
    for _ in 0..4 {
        h.update();
    }
}

/// Spawn one absolutely-positioned, laid-out leaf under a root context, with the
/// given extra bundle. Returns the leaf entity.
fn spawn_leaf(app: &mut App, extra: impl Bundle) -> Entity {
    let child = app
        .world_mut()
        .spawn((
            Node,
            Style::default()
                .absolute()
                .inset(Inset {
                    top: Sizing::Length(Length::px(20.0)),
                    left: Sizing::Length(Length::px(20.0)),
                    ..default()
                })
                .width_px(80.0)
                .height_px(30.0),
            extra,
        ))
        .id();
    app.world_mut()
        .spawn((Node, Style::default()))
        .add_children(&[child]);
    child
}

const SURFACE_TOKEN: &str = "color.surface.primary";

// --- 1. AnimatedBackgroundColor auto-composite ------------------------------

#[test]
fn animated_background_color_composites_over_the_token() {
    let mut h = NodeExtractHarness::new();
    // A distinct animated color the live crossfade would write (not equal to the
    // token's resolved color, asserted below).
    let animated = Color::srgb(0.12, 0.34, 0.56);
    let e = spawn_leaf(
        &mut h.app,
        (
            Background {
                color: ColorToken::Token(Cow::Borrowed(SURFACE_TOKEN)),
            },
            AnimatedBackgroundColor(animated),
        ),
    );
    settle(&mut h);
    h.extract();

    let node = h.node_for(e).expect("the leaf reaches the display list");
    let token_color = resolve_token(&ColorToken::Token(Cow::Borrowed(SURFACE_TOKEN)), &h.theme());
    assert_ne!(
        animated, token_color,
        "test setup: the animated color must differ from the token so the override is observable"
    );
    assert_eq!(
        node.color, animated,
        "a node with AnimatedBackgroundColor paints the interpolated color, not its Background token"
    );
}

#[test]
fn background_token_resolves_when_no_animation_present() {
    let mut h = NodeExtractHarness::new();
    let e = spawn_leaf(
        &mut h.app,
        Background {
            color: ColorToken::Token(Cow::Borrowed(SURFACE_TOKEN)),
        },
    );
    settle(&mut h);
    h.extract();

    let node = h.node_for(e).expect("the leaf reaches the display list");
    let token_color = resolve_token(&ColorToken::Token(Cow::Borrowed(SURFACE_TOKEN)), &h.theme());
    assert_eq!(
        node.color, token_color,
        "with no AnimatedBackgroundColor the node falls back to its resolved Background token"
    );
}

// --- 2. Changed<BackdropFilter> damage-gate term ----------------------------

#[test]
fn isolated_backdrop_filter_edit_re_extracts() {
    let mut h = NodeExtractHarness::new();
    let e = spawn_leaf(
        &mut h.app,
        BackdropFilter(vec![FilterFn::Blur(Length::px(6.0))]),
    );
    settle(&mut h);
    h.extract();
    assert_eq!(
        h.backdrop_px(e),
        Some(6.0),
        "baseline: the backdrop-filter former captures its 6px blur"
    );

    // Poison the carriers, then edit ONLY the BackdropFilter (no `app.update()`,
    // so the main-world render-prep passes do NOT re-run and re-insert their
    // markers — the BackdropFilter component is the sole paint-input change since
    // the last extract). The second extract therefore re-runs IFF the damage
    // gate's Or-set carries a `Changed<BackdropFilter>` term. RED (None) before
    // the term, GREEN (Some(12)) after.
    h.poison();
    h.app
        .world_mut()
        .entity_mut(e)
        .insert(BackdropFilter(vec![FilterFn::Blur(Length::px(12.0))]));
    h.extract();
    assert_eq!(
        h.backdrop_px(e),
        Some(12.0),
        "an isolated backdrop blur-radius edit must re-extract so backdrop_blur_px is not stale"
    );
}
