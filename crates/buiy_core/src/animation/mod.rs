//! Animation: a lightweight tween registry wired into [`BuiySet::Animate`].
//!
//! The Widget Catalog parity work needs exactly two timing functions and a
//! handful of animated properties (transform + opacity + a background-color
//! crossfade); `bevy_animation` is over-weight for that (spec § 3.3 / § 8), so
//! this module ships a small typed tween instead.
//!
//! - [`easing::Easing`] — `Linear` / `CubicBezier`, with the design constants
//!   [`Easing::EASE`](easing::Easing::EASE) and
//!   [`Easing::DESIGN`](easing::Easing::DESIGN).
//! - [`tween::Tween`] — `from`/`to`/`duration`/`elapsed`/`easing` over any
//!   [`tween::Lerp`] value, plus the per-property target components
//!   ([`tween::TranslateTween`], [`tween::RotateTween`], [`tween::ScaleTween`],
//!   [`tween::OpacityTween`], [`tween::BackgroundColorTween`]).
//! - [`AnimationPlugin`] — registers the five per-target update systems in
//!   [`BuiySet::Animate`].
//!
//! All systems honour
//! [`UserPreferences::prefers_reduced_motion`](crate::theme::UserPreferences):
//! when set, a tween jumps to its end value on the first tick (values.md § 5.3).
//!
//! Headless-testable: `Time` can be advanced manually
//! (`Time::advance_by`) so a test can step the tween without a render loop —
//! see the integration tests at the bottom of this file.
//!
//! Spec: docs/specs/2026-06-25-widget-catalog-parity-design.md § 3.3 / § 8.

pub mod easing;
pub mod tween;

pub use easing::Easing;
pub use tween::{
    AnimatedBackgroundColor, BackgroundColorTween, Lerp, OnComplete, OpacityTween, QuadAlphaTween,
    Repeat, RotateTween, ScaleTween, TranslateTween, Tween, advance_background_color_tweens,
    advance_opacity_tweens, advance_quad_alpha_tweens, advance_rotate_tweens, advance_scale_tweens,
    advance_translate_tweens,
};

use bevy::prelude::*;

use crate::BuiySet;

/// Registers the tween-update systems in [`BuiySet::Animate`].
///
/// Aggregated into the umbrella `BuiyPlugin` (`crates/buiy/src/lib.rs`) next to
/// the other core sub-plugins. `BuiySet::Animate` is configured (ordered after
/// `Input`, before `Picking`) by `CorePlugin`; this plugin only adds systems to
/// it, matching the codebase correction in spec § 8 ("the set already exists —
/// wire systems into it").
pub struct AnimationPlugin;

impl Plugin for AnimationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                advance_translate_tweens,
                advance_rotate_tweens,
                advance_scale_tweens,
                advance_opacity_tweens,
                advance_quad_alpha_tweens,
                advance_background_color_tweens,
            )
                .in_set(BuiySet::Animate),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use bevy::prelude::*;

    use super::*;
    use crate::layout::{Length, Translate};
    use crate::render::components::Opacity;
    use crate::theme::UserPreferences;

    /// A minimal headless app: manual `Time`, the animation systems, and an
    /// optional `UserPreferences`. No render loop, no `ThemePlugin`.
    fn test_app(prefs: UserPreferences) -> App {
        let mut app = App::new();
        app.init_resource::<Time>();
        app.insert_resource(prefs);
        app.add_plugins(AnimationPlugin);
        // `AnimationPlugin` schedules into `BuiySet::Animate`, which is only
        // *configured* by `CorePlugin`. Configure the bare set here so the
        // systems run in this minimal app (no full `CorePlugin` needed).
        app.configure_sets(Update, BuiySet::Animate);
        app
    }

    fn px(t: &Translate) -> f32 {
        match t.0 {
            Length::Px(v) => v,
            other => panic!("expected px translate, got {other:?}"),
        }
    }

    /// Advance virtual `Time` by `ms` and run one `Update`.
    fn step(app: &mut App, ms: u64) {
        let mut time = app.world_mut().resource_mut::<Time>();
        time.advance_by(Duration::from_millis(ms));
        app.update();
    }

    /// A Translate tween 0→20 over 150 ms with the design cubic-bezier: lands
    /// within epsilon of 20 at t≥150 ms and is partway (and front-loaded by the
    /// ease-out curve) at 75 ms.
    #[test]
    fn translate_tween_lands_on_end_and_is_partway_midway() {
        let mut app = test_app(UserPreferences::default());
        let e = app
            .world_mut()
            .spawn(TranslateTween(Tween::new(
                Vec3::ZERO,
                Vec3::new(20.0, 0.0, 0.0),
                Duration::from_millis(150),
                Easing::DESIGN,
            )))
            .id();

        // 75 ms in — half the duration. The design curve is strongly
        // front-loaded (ease-out): y(0.5) ≈ 0.94608, so x ≈ 18.9, clearly
        // partway and well past the linear-midpoint 10.
        step(&mut app, 75);
        let mid = px(app.world().entity(e).get::<Translate>().unwrap());
        assert!(
            mid > 10.0 && mid < 20.0,
            "expected front-loaded partway value in (10,20), got {mid}"
        );
        // The tween must still be live at the midpoint.
        assert!(
            app.world().entity(e).get::<TranslateTween>().is_some(),
            "tween should still be running at 75ms"
        );

        // Finish: another 75 ms reaches exactly 150 ms total.
        step(&mut app, 75);
        let end = px(app.world().entity(e).get::<Translate>().unwrap());
        assert!((end - 20.0).abs() < 1e-4, "expected ~20 at end, got {end}");
        // Completed tween is removed; the end-state Translate persists.
        assert!(
            app.world().entity(e).get::<TranslateTween>().is_none(),
            "completed tween must be removed"
        );
    }

    /// Reduced motion snaps a tween to its end value on the very first tick
    /// (no interpolation), and removes the tween.
    #[test]
    fn reduced_motion_snaps_to_end_on_first_tick() {
        let mut app = test_app(UserPreferences {
            prefers_reduced_motion: true,
            ..Default::default()
        });
        let e = app
            .world_mut()
            .spawn(TranslateTween(Tween::new(
                Vec3::ZERO,
                Vec3::new(20.0, 0.0, 0.0),
                Duration::from_millis(150),
                Easing::DESIGN,
            )))
            .id();

        // One tiny tick (1 ms) — far short of 150 ms — must already be at 20.
        step(&mut app, 1);
        let v = px(app.world().entity(e).get::<Translate>().unwrap());
        assert!(
            (v - 20.0).abs() < 1e-4,
            "reduced motion must snap to 20 immediately, got {v}"
        );
        assert!(
            app.world().entity(e).get::<TranslateTween>().is_none(),
            "reduced-motion tween must complete on first tick"
        );
    }

    /// The opacity target writes `Opacity` and an entrance fade 0→1 completes.
    #[test]
    fn opacity_tween_completes_at_one() {
        let mut app = test_app(UserPreferences::default());
        let e = app
            .world_mut()
            .spawn(OpacityTween(Tween::secs(0.0, 1.0, 0.12, Easing::EASE)))
            .id();

        step(&mut app, 60);
        let mid = app.world().entity(e).get::<Opacity>().unwrap().0;
        assert!(mid > 0.0 && mid < 1.0, "partway opacity, got {mid}");

        step(&mut app, 60);
        let end = app.world().entity(e).get::<Opacity>().unwrap().0;
        assert!(
            (end - 1.0).abs() < 1e-4,
            "opacity should reach 1, got {end}"
        );
        assert!(app.world().entity(e).get::<OpacityTween>().is_none());
    }

    /// A ping-pong `OpacityTween` (the menu blink dot — opacity `1`→`.25`→`1`,
    /// infinite) OSCILLATES: it dims toward `.25` over the first pass, brightens
    /// back toward `1` over the second, and NEVER completes (the tween stays
    /// resident for the live pulse). This is the M3 looping-tween behaviour.
    #[test]
    fn ping_pong_opacity_tween_oscillates_and_never_completes() {
        let mut app = test_app(UserPreferences::default());
        // 1.0 → 0.25 over 100 ms each way, forever (the blink dot at a fast
        // cadence so the test steps land cleanly inside each pass).
        let e = app
            .world_mut()
            .spawn(OpacityTween(
                Tween::secs(1.0, 0.25, 0.1, Easing::Linear)
                    .with_repeat(Repeat::PingPong { count: None }),
            ))
            .id();

        let opacity = |app: &App| app.world().entity(e).get::<Opacity>().unwrap().0;

        // 50 ms into the first (dimming) pass: partway between 1.0 and 0.25.
        step(&mut app, 50);
        let dimming = opacity(&app);
        assert!(
            dimming < 1.0 && dimming > 0.25,
            "first pass dims toward 0.25, got {dimming}"
        );

        // Cross the pass boundary (another 60 ms → 110 ms total → 10 ms into the
        // SECOND, brightening pass). The value reverses direction: it is now
        // brighter than the dim end, climbing back toward 1.0.
        step(&mut app, 60);
        let brightening = opacity(&app);
        assert!(
            brightening > 0.25 && brightening < 1.0,
            "second pass brightens back toward 1.0, got {brightening}"
        );

        // The tween is STILL resident — an infinite ping-pong never completes.
        assert!(
            app.world().entity(e).get::<OpacityTween>().is_some(),
            "an infinite ping-pong tween must never be removed"
        );

        // Step many full cycles; it is still live and still in range — proving the
        // sustained pulse (and that the carried-remainder wrap does not drift off).
        for _ in 0..50 {
            step(&mut app, 37);
        }
        let v = opacity(&app);
        assert!(
            (0.25..=1.0).contains(&v),
            "opacity stays within [0.25, 1.0] across many cycles, got {v}"
        );
        assert!(
            app.world().entity(e).get::<OpacityTween>().is_some(),
            "ping-pong tween still resident after many cycles"
        );
    }

    /// A finite `Repeat::Loop` runs the requested number of passes then completes
    /// and is removed (the looping mode is not only-infinite).
    #[test]
    fn finite_loop_completes_after_its_passes() {
        let mut app = test_app(UserPreferences::default());
        let e = app
            .world_mut()
            .spawn(OpacityTween(
                Tween::secs(0.0, 1.0, 0.05, Easing::Linear)
                    .with_repeat(Repeat::Loop { count: Some(2) }),
            ))
            .id();
        // First pass (0→1 over 50 ms): still running at 30 ms.
        step(&mut app, 30);
        assert!(app.world().entity(e).get::<OpacityTween>().is_some());
        // Cross into the second (last) pass, then finish it: total ~110 ms covers
        // both 50 ms passes → completed and removed, end-state Opacity at 1.0.
        step(&mut app, 40); // 70 ms → 20 ms into pass 2
        step(&mut app, 40); // 110 ms → past pass 2 end
        assert!(
            app.world().entity(e).get::<OpacityTween>().is_none(),
            "a 2-pass loop must complete after its passes"
        );
        assert!((app.world().entity(e).get::<Opacity>().unwrap().0 - 1.0).abs() < 1e-4);
    }

    /// Reduced motion snaps a ping-pong (blink) tween to its STEADY rest state —
    /// the `from`/bright end (a steady-lit dot), not a frozen dim frame — and
    /// completes immediately, so a pulse never oscillates under reduced motion.
    #[test]
    fn reduced_motion_snaps_blink_to_steady_bright() {
        let mut app = test_app(UserPreferences {
            prefers_reduced_motion: true,
            ..Default::default()
        });
        let e = app
            .world_mut()
            .spawn(OpacityTween(
                Tween::secs(1.0, 0.25, 0.8, Easing::Linear)
                    .with_repeat(Repeat::PingPong { count: None }),
            ))
            .id();
        step(&mut app, 1);
        let v = app.world().entity(e).get::<Opacity>().unwrap().0;
        assert!(
            (v - 1.0).abs() < 1e-4,
            "reduced motion snaps the blink to the bright rest state 1.0, got {v}"
        );
        assert!(
            app.world().entity(e).get::<OpacityTween>().is_none(),
            "reduced-motion blink completes immediately (no pulse)"
        );
    }

    /// `OnComplete` is attached only when requested.
    #[test]
    fn on_complete_marker_is_opt_in() {
        let mut app = test_app(UserPreferences::default());
        let marked = app
            .world_mut()
            .spawn(OpacityTween(
                Tween::secs(0.0, 1.0, 0.05, Easing::Linear).with_on_complete(),
            ))
            .id();
        let unmarked = app
            .world_mut()
            .spawn(OpacityTween(Tween::secs(0.0, 1.0, 0.05, Easing::Linear)))
            .id();

        step(&mut app, 60); // past 50 ms — both complete
        assert!(
            app.world().entity(marked).get::<OnComplete>().is_some(),
            "opt-in tween should be marked complete"
        );
        assert!(
            app.world().entity(unmarked).get::<OnComplete>().is_none(),
            "default tween should not be marked"
        );
    }
}
