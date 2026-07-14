//! Pick-occlusion coherence: the **canonical** transparent-top-layer-occluder
//! predicate + a debug-only fail-loud coherence system that upholds it framework-
//! wide.
//!
//! ## The bug class
//!
//! An invisible-but-pickable box that sits topmost and swallows every click
//! beneath it — a transparent `.top_layer()` container with no visible fill and no
//! `Pickable::IGNORE` — has shipped **three times** in project history (the parity
//! campaign's detached modal; the wasm cold-synthetic-click artifact; the
//! prototype's `.fixed().fill().top_layer()` theme toggle). It is invisible to
//! a11y-probe clicks and screenshots (both bypass pick occlusion), so
//! discipline-based gating (remembering to click) kept missing it.
//!
//! ## Two upholders of ONE predicate
//!
//! [`is_transparent_top_layer_occluder`] is the single source of truth for "is
//! this entity the class". It is upheld by two independent nets that MUST agree
//! (so neither can drift):
//!
//! - the **test-time** structural invariant
//!   `buiy_verify::invariant::no_transparent_top_layer_occluder`, which sweeps a
//!   fixture catalog and delegates to this predicate (Tier-3, headless);
//! - the **run-time** `assert_no_transparent_top_layer_occluder` coherence
//!   system (this module), a `#[cfg(debug_assertions)]` `Last`-scheduled check
//!   that DEBUG-PANICS the moment a hand-authored `buiy_core`/`buiy_widgets`/`bsn!`
//!   tree leaves the class in a live world.
//!
//! The run-time net exists because the `buiy_view` reconciler's
//! auto-`Pickable::IGNORE` construction guarantee (`reconcile.rs`) covers ONLY
//! reconciled view trees; a hand-authored retained tree gets no such guard. The
//! coherence system is a **fail-loud panic, deliberately NOT a silent
//! auto-`Pickable::IGNORE`**: silently auto-repairing a hand-authored bug is the
//! exact fail-silent anti-pattern the app-author-ergonomics campaign exists to
//! kill (a corrupted intent is masked, not surfaced). The escape hatches are
//! explicit and visible in the tree: paint a fill (any render-tier fill —
//! `Background`/`BackgroundLayers`/`RasterImage`/`Text`/`Icon`/`Border`/`Outline`/
//! `BoxShadow`), or opt the container out of picking with `Pickable::IGNORE`
//! (the `buiy_view` `.ignore_picking()` lowering, or a direct insert).
//!
//! ## The one intentional case, and its escape hatch
//!
//! An **interactive full-viewport click-catcher** — a transparent `.top_layer()`
//! container carrying a press handler to dismiss-on-outside-click, which the
//! `buiy_view` reconciler keeps pickable (its auto-ignore has an `on_press.is_none()`
//! gate) — is STILL the class by this predicate, deliberately: an invisible
//! full-viewport pick sink has shipped as a bug three times, so the strict
//! definition wins over the reconciler's keep-pickable nuance. Its escape hatch is
//! specifically to **paint a fill** (a dismiss scrim is normally a translucent dim
//! anyway — e.g. dooduel's `scrim()` paints `background(SCRIM)`); `Pickable::IGNORE`
//! is NOT an option for it (that would defeat its click capture). No such case
//! exists in-tree today; this note pre-empts the divergence rather than papering
//! over it.

use bevy::picking::Pickable;
use bevy::prelude::*;

use crate::Node;
use crate::layout::{Stacking, TopLayer};
use crate::render::RasterImage;
use crate::render::components::{
    Background, BackgroundLayers, Border, BoxShadow, ComputedPaintSkip, Icon, Outline,
};
use crate::text::Text;

/// The **canonical** predicate: is `entity` a *transparent top-layer pick
/// occluder* — an invisible box that sits topmost and swallows clicks?
///
/// True iff ALL of:
/// - it is a real layout [`Node`] in the **top layer** (`Stacking.top_layer !=
///   TopLayer::None`) — `Stacking` is `#[require]`d by `Node`, so the explicit
///   `Node` check documents the intent and skips a stray hand-added `Stacking`;
/// - it is **not** paint-skipped ([`ComputedPaintSkip`] absent) — a hidden node
///   (`Display::None` / `CssVisibility::Hidden` / offscreen) is already dropped by
///   the picking backend, so it cannot occlude;
/// - it **paints nothing** the user can see: none of the render-tier fill
///   components — no [`Background`] (solid quad), [`BackgroundLayers`] (gradient /
///   layered fill), [`RasterImage`] (textured quad), [`Text`] (glyphs), [`Icon`]
///   (icon coverage), [`Border`] (border band), [`Outline`] (outline band), or
///   [`BoxShadow`] (drop shadow) — a node carrying any of these is *visibly
///   present*, not the *invisible*-occluder class;
/// - it is **not** [`Pickable::IGNORE`] — a transparent top-layer node is safe
///   ONLY when it neither hit-targets nor occludes; anything else (no `Pickable` ⇒
///   the blocking default, or a blocking `Pickable`) makes it an occluder.
///
/// App-agnostic (it names only `buiy_core` + `bevy` component types), so it runs
/// over any Buiy entity — a reconciled `buiy_view` tree, a hand-authored retained
/// one, or a `bsn!`-authored one.
///
/// The load-bearing shared piece: `buiy_verify`'s Tier-3 invariant and this
/// module's `assert_no_transparent_top_layer_occluder` coherence system both
/// call it, so the two nets can never disagree about what the class is.
pub fn is_transparent_top_layer_occluder(entity: EntityRef) -> bool {
    // A real layout node in the top layer.
    if !entity.contains::<Node>() {
        return false;
    }
    let Some(stacking) = entity.get::<Stacking>() else {
        return false;
    };
    if stacking.top_layer == TopLayer::None {
        return false;
    }
    // A paint-skipped node is not a pick candidate (the backend skips
    // `ComputedPaintSkip`), so it cannot occlude.
    if entity.contains::<ComputedPaintSkip>() {
        return false;
    }
    // Paints something visible ⇒ not the *invisible*-occluder class. This set
    // MUST mirror the render tier producers (extract.rs / icon_producer / raster),
    // so a node painting ANY visible primitive is excluded — otherwise a
    // gradient-only or border-only overlay is a false positive (the two GPU
    // fixtures `bare_gradient_only_top_layer_overlay_occludes_base_glyph` and
    // `single_boundary_v1_scrim_dims_base_band_not_a_fellow_top_layer_band`). The
    // fill set: solid quad (`Background`), gradient / layered fill
    // (`BackgroundLayers`, which carries `LinearGradient` / `RadialGradient` /
    // layered `Solid`), textured quad (`RasterImage`), glyphs (`Text`), icon
    // coverage (`Icon`), border band (`Border`), outline band (`Outline`), and box
    // shadow (`BoxShadow`, which carries the `Shadow` terms). It is a PRESENCE
    // check, matching the long-standing `Background` idiom: attaching a fill
    // component signals visible intent, so a defaulted / transparent fill is out
    // of the *invisible*-occluder class (a value check would risk drifting from
    // the extract's own paint logic).
    if entity.contains::<Background>()
        || entity.contains::<BackgroundLayers>()
        || entity.contains::<RasterImage>()
        || entity.contains::<Text>()
        || entity.contains::<Icon>()
        || entity.contains::<Border>()
        || entity.contains::<Outline>()
        || entity.contains::<BoxShadow>()
    {
        return false;
    }
    // Transparent + top-layer + visible-to-picking. Safe ONLY as
    // `Pickable::IGNORE`; anything else makes it a pick occluder — the bug.
    entity.get::<Pickable>().copied() != Some(Pickable::IGNORE)
}

/// Debug-only fail-loud coherence system (F6, spec §2.7 / app-author-ergonomics
/// 4b-scope): DEBUG-PANICS if any live entity is a
/// [`is_transparent_top_layer_occluder`]. Scheduled by `BuiyRenderPlugin` in
/// `Last` — AFTER `write_paint_skip` settles [`ComputedPaintSkip`], so a closed
/// (`Display::None` / `CssVisibility::Hidden`) overlay is correctly excluded.
///
/// **Why a panic, not an auto-fix.** The `buiy_view` reconciler auto-inserts
/// `Pickable::IGNORE` on transparent top-layer containers *at construction* — a
/// legitimate "unwritable by construction" design for the reconciled view surface.
/// This system covers the OTHER surfaces (hand-authored `buiy_core`/`buiy_widgets`/
/// `bsn!` trees), where silently mutating a hand-authored tree would mask the
/// author's mistake instead of surfacing it. Fail-loud is consistent with the
/// `reconcile.rs` `keyed_column` dup-key `debug_assert` and the MVU §7.5 auditor.
///
/// **MT-safety.** Read-only (`Query<EntityRef>` grants shared, whole-entity read
/// access), so it is sound under the `multi_threaded` executor — the scheduler
/// serializes it against any writer in `Last` rather than racing.
///
/// Compiled ONLY in debug builds (`#[cfg(debug_assertions)]`); release builds
/// carry neither the system nor its scheduling (see `BuiyRenderPlugin`).
#[cfg(debug_assertions)]
pub(crate) fn assert_no_transparent_top_layer_occluder(entities: Query<EntityRef>) {
    let offenders: Vec<Entity> = entities
        .iter()
        .filter(|entity| is_transparent_top_layer_occluder(*entity))
        .map(|entity| entity.id())
        .collect();
    assert!(
        offenders.is_empty(),
        "{n} transparent top-layer node(s) occlude picks while painting nothing — the \
         invisible-occluder bug class. A transparent `.top_layer()` container must either paint a \
         visible fill (any of `Background`/`BackgroundLayers`/`RasterImage`/`Text`/`Icon`/`Border`/\
         `Outline`/`BoxShadow`) OR carry `Pickable::IGNORE` (a hand-authored insert, or \
         `.ignore_picking()` in `buiy_view`). Offending entities: {offenders:?}",
        n = offenders.len(),
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::{App, Last};

    /// A `Stacking` promoted to the top layer (the `.top_layer()` lowering).
    fn top_layer() -> Stacking {
        Stacking {
            top_layer: TopLayer::Popover,
            ..Default::default()
        }
    }

    // --- The predicate (the canonical class definition) --------------------

    #[test]
    fn flags_a_transparent_top_layer_node_without_ignore() {
        let mut world = World::new();
        let bug = world.spawn((Node, top_layer())).id();
        assert!(is_transparent_top_layer_occluder(world.entity(bug)));
    }

    #[test]
    fn a_transparent_top_layer_ignore_node_is_safe() {
        let mut world = World::new();
        let safe = world.spawn((Node, top_layer(), Pickable::IGNORE)).id();
        assert!(!is_transparent_top_layer_occluder(world.entity(safe)));
    }

    #[test]
    fn a_painted_top_layer_node_is_not_the_class() {
        let mut world = World::new();
        let painted = world
            .spawn((
                Node,
                top_layer(),
                Background {
                    color: crate::render::color::ColorToken::SurfacePrimary,
                },
            ))
            .id();
        assert!(!is_transparent_top_layer_occluder(world.entity(painted)));
    }

    /// A gradient-only overlay (no solid [`Background`]) still paints a visible
    /// fill, so it is NOT the class — the headless twin of the GPU fixture
    /// `bare_gradient_only_top_layer_overlay_occludes_base_glyph`.
    #[test]
    fn a_gradient_only_top_layer_node_is_not_the_class() {
        use crate::render::color::ColorToken;
        use crate::render::components::{
            BackgroundLayer, BackgroundLayers, ColorStop, LinearGradient,
        };

        let mut world = World::new();
        let gradient = world
            .spawn((
                Node,
                top_layer(),
                BackgroundLayers(vec![BackgroundLayer::Linear(LinearGradient {
                    angle_deg: 90.0,
                    stops: vec![
                        ColorStop {
                            color: ColorToken::SurfacePrimary,
                            position: 0.0,
                        },
                        ColorStop {
                            color: ColorToken::SurfacePrimary,
                            position: 1.0,
                        },
                    ],
                })]),
            ))
            .id();
        assert!(!is_transparent_top_layer_occluder(world.entity(gradient)));
    }

    /// A border-only overlay (a painted band, no solid [`Background`]) is visibly
    /// present, so it is NOT the class — the headless twin of the GPU fixture
    /// `single_boundary_v1_scrim_dims_base_band_not_a_fellow_top_layer_band`.
    #[test]
    fn a_border_only_top_layer_node_is_not_the_class() {
        use crate::render::color::ColorToken;
        use crate::render::components::{Border, BorderSide, Corners, LineStyle};

        let side = BorderSide {
            color: ColorToken::SurfacePrimary,
            style: LineStyle::Solid,
        };
        let mut world = World::new();
        let bordered = world
            .spawn((
                Node,
                top_layer(),
                Border {
                    top: side.clone(),
                    right: side.clone(),
                    bottom: side.clone(),
                    left: side,
                    radius: Corners::ZERO,
                },
            ))
            .id();
        assert!(!is_transparent_top_layer_occluder(world.entity(bordered)));
    }

    #[test]
    fn a_paint_skipped_top_layer_node_is_out_of_scope() {
        let mut world = World::new();
        let hidden = world
            .spawn((
                Node,
                top_layer(),
                ComputedPaintSkip {
                    reason: crate::render::components::SkipReason::CssHidden,
                },
            ))
            .id();
        assert!(!is_transparent_top_layer_occluder(world.entity(hidden)));
    }

    #[test]
    fn a_transparent_in_flow_node_is_out_of_scope() {
        let mut world = World::new();
        let in_flow = world.spawn(Node).id(); // default Stacking ⇒ TopLayer::None
        assert!(!is_transparent_top_layer_occluder(world.entity(in_flow)));
    }

    // --- The coherence system (fires / does not fire) ----------------------

    /// Build a minimal app running only the coherence system in `Last`.
    #[cfg(debug_assertions)]
    fn coherence_app() -> App {
        let mut app = App::new();
        app.add_systems(Last, assert_no_transparent_top_layer_occluder);
        app
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic(expected = "transparent top-layer node")]
    fn debug_panics_on_a_hand_authored_transparent_top_layer_node() {
        let mut app = coherence_app();
        app.world_mut().spawn((Node, top_layer()));
        app.update();
    }

    #[cfg(debug_assertions)]
    #[test]
    fn does_not_panic_for_a_pickable_ignore_node() {
        let mut app = coherence_app();
        app.world_mut().spawn((Node, top_layer(), Pickable::IGNORE));
        app.update(); // no panic
    }

    #[cfg(debug_assertions)]
    #[test]
    fn does_not_panic_for_a_painted_node() {
        let mut app = coherence_app();
        app.world_mut().spawn((
            Node,
            top_layer(),
            Background {
                color: crate::render::color::ColorToken::SurfacePrimary,
            },
        ));
        app.update(); // no panic
    }
}
