//! A lightweight tween registry (the design rejects `bevy_animation` as
//! over-weight, spec § 3.3 / § 8).
//!
//! A [`Tween<T>`] is a component carrying a `from`/`to` pair of an animatable
//! value `T`, a [`Duration`], elapsed time, an [`Easing`], and an optional
//! [`OnComplete`] marker. One per-property *target* component binds a tween to
//! the concrete component it drives; a per-target system advances `elapsed` by
//! the frame delta, samples the easing, lerps `from`→`to`, and writes the
//! result. On completion the tween writes the exact `to` value and the
//! `Tween<T>`/target pair is removed (the entity keeps its end-state component).
//!
//! # Targets
//!
//! The design animates **transform + opacity only** (spec § 8 tween authoring
//! rule — never animate Taffy-owned layout per frame). The animatable targets
//! the catalog needs:
//!
//! - [`TranslateTween`] — the layout [`Translate`] transform (switch thumb's
//!   `left`, menu/modal/toast entrance translateY). Lerps in px space.
//! - [`RotateTween`] — the layout [`Rotate`] transform (disclosure chevron
//!   `rotate(90deg)`). Slerps the quaternion.
//! - [`ScaleTween`] — the layout [`Scale`] transform (menu/modal entrance
//!   `scale(.98)`→`1`).
//! - [`OpacityTween`] — the render [`Opacity`] group-opacity (every entrance
//!   `opacity:0`→`1`, the blink dot when scripted as a tween).
//! - [`BackgroundColorTween`] — a resolved-color crossfade written to the
//!   companion [`AnimatedBackgroundColor`] component (switch/nav/filter track
//!   `background` transitions). See [`AnimatedBackgroundColor`] for why this is
//!   a separate component and not `Background.color`.
//!
//! All five systems honour [`prefers_reduced_motion`](crate::theme::UserPreferences):
//! when set, a tween snaps to its `to` value on its first tick and completes
//! immediately (spec § 8 / values.md § 5.3 — jump-to-end).

use std::time::Duration;

use bevy::prelude::*;

use super::easing::Easing;
use crate::layout::{Length, Rotate, Scale, Translate};
use crate::render::components::Opacity;

/// Linear interpolation for an animatable value. `lerp(from, to, 0) == from`
/// and `lerp(from, to, 1) == to`.
pub trait Lerp: Clone + Send + Sync + 'static {
    /// Interpolate between `self` (the `from` end) and `to` at fraction `t`
    /// (already eased, in `[0, 1]`).
    fn lerp(&self, to: &Self, t: f32) -> Self;
}

impl Lerp for f32 {
    fn lerp(&self, to: &Self, t: f32) -> Self {
        self + (to - self) * t
    }
}

impl Lerp for Vec3 {
    fn lerp(&self, to: &Self, t: f32) -> Self {
        Vec3::lerp(*self, *to, t)
    }
}

impl Lerp for Quat {
    fn lerp(&self, to: &Self, t: f32) -> Self {
        // `slerp` is the correct rotational interpolation; it degrades to a
        // straight `nlerp` for the small angles UI chevrons use, and never
        // produces the gimbal artifacts a component-wise `lerp` would.
        Quat::slerp(*self, *to, t)
    }
}

impl Lerp for Color {
    fn lerp(&self, to: &Self, t: f32) -> Self {
        // Mix in linear-RGB space (Bevy's `Mix` for `Color` converts to the
        // working space internally) so the crossfade matches how the render
        // pipeline composites — no sRGB gamma banding mid-fade.
        Mix::mix(self, to, t)
    }
}

/// Marker placed on a completed tween's entity by the per-target update systems
/// so a downstream system (or the spawner) can react to the tween finishing.
/// Carried by the `Tween` until it is removed; consumers can also watch for the
/// removal of the target component. The unit payload is the spawner's
/// caller-supplied tag (use `()` when no tag is needed).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct OnComplete;

/// A generic tween over an animatable value `T: Lerp`.
///
/// Construct via [`Tween::new`]; advance happens in [`BuiySet::Animate`](crate::BuiySet)
/// by the per-target system. The tween is *data only* — the binding to a
/// concrete component is the per-property target component
/// ([`TranslateTween`], [`OpacityTween`], …) that wraps a `Tween<T>`.
#[derive(Clone, Debug, PartialEq)]
pub struct Tween<T: Lerp> {
    /// Start value (written at `elapsed == 0`).
    pub from: T,
    /// End value (written exactly on completion).
    pub to: T,
    /// Total animation duration.
    pub duration: Duration,
    /// Time advanced so far; starts at zero.
    pub elapsed: Duration,
    /// Timing function.
    pub easing: Easing,
    /// When true, the per-target system attaches an [`OnComplete`] marker to the
    /// entity as the tween finishes. Off by default.
    pub mark_on_complete: bool,
}

impl<T: Lerp> Tween<T> {
    /// A tween from `from` to `to` over `duration` with `easing`. `elapsed`
    /// starts at zero and `mark_on_complete` is off.
    pub fn new(from: T, to: T, duration: Duration, easing: Easing) -> Self {
        Self {
            from,
            to,
            duration,
            elapsed: Duration::ZERO,
            easing,
            mark_on_complete: false,
        }
    }

    /// Convenience: duration from seconds.
    pub fn secs(from: T, to: T, secs: f32, easing: Easing) -> Self {
        Self::new(from, to, Duration::from_secs_f32(secs.max(0.0)), easing)
    }

    /// Request an [`OnComplete`] marker be attached on completion.
    pub fn with_on_complete(mut self) -> Self {
        self.mark_on_complete = true;
        self
    }

    /// `true` once `elapsed >= duration` (or the duration is zero).
    pub fn is_complete(&self) -> bool {
        self.elapsed >= self.duration
    }

    /// The current eased fraction in `[0, 1]` (1.0 for a zero-duration tween).
    fn fraction(&self) -> f32 {
        if self.duration.is_zero() {
            return 1.0;
        }
        let raw = (self.elapsed.as_secs_f32() / self.duration.as_secs_f32()).clamp(0.0, 1.0);
        self.easing.sample(raw)
    }

    /// The interpolated value at the current `elapsed`.
    pub fn value(&self) -> T {
        if self.is_complete() {
            return self.to.clone();
        }
        self.from.lerp(&self.to, self.fraction())
    }
}

// ---------------------------------------------------------------------------
// Per-property target components.
//
// Each wraps a `Tween<T>` over the value type that property animates and binds
// it to the concrete component its update system writes. Keeping the binding in
// the component type (rather than a `dyn`/registry) means each property gets a
// fully-typed, allocation-free update system and the gallery spawns exactly the
// target it wants.
// ---------------------------------------------------------------------------

/// Tween the layout [`Translate`] transform (px space). `from`/`to` are the
/// translation in logical pixels; the system writes `Translate(px, px, px)`.
#[derive(Component, Clone, Debug)]
pub struct TranslateTween(pub Tween<Vec3>);

/// Tween the layout [`Rotate`] transform (slerped).
#[derive(Component, Clone, Debug)]
pub struct RotateTween(pub Tween<Quat>);

/// Tween the layout [`Scale`] transform.
#[derive(Component, Clone, Debug)]
pub struct ScaleTween(pub Tween<Vec3>);

/// Tween the render [`Opacity`] group-opacity in `[0, 1]`.
#[derive(Component, Clone, Debug)]
pub struct OpacityTween(pub Tween<f32>);

/// Tween a resolved background color, written to the companion
/// [`AnimatedBackgroundColor`].
#[derive(Component, Clone, Debug)]
pub struct BackgroundColorTween(pub Tween<Color>);

/// The crossfaded background color produced by a [`BackgroundColorTween`].
///
/// **Why a separate component, not `Background.color`** — the parity design
/// locks `Background.color` as **token-only** (spec § 8: "Token-only stays —
/// reject `ColorToken::Literal`"). A color *crossfade* is intrinsically a
/// resolved-`Color` operation (you cannot lerp two named tokens through the
/// token enum), so animating it into `Background.color` would require the
/// rejected `ColorToken::Literal`. Instead the tween resolves both endpoints to
/// `Color` at spawn time (the caller does `resolve_token`) and writes the
/// interpolated `Color` here. A widget that wants the animated fill reads
/// `AnimatedBackgroundColor` for the duration of the tween and falls back to its
/// token `Background` at rest. This keeps the static paint path token-pure while
/// still expressing the design's `background .12s/.15s` transitions.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct AnimatedBackgroundColor(pub Color);

// ---------------------------------------------------------------------------
// Update systems. Each advances elapsed, samples + lerps, writes the target,
// and removes the tween (+ target) on completion. Reduced-motion snaps to `to`.
// ---------------------------------------------------------------------------

/// Outcome of one tween tick, shared by every per-target system.
enum Tick<T> {
    /// Still running; write this interpolated value.
    Running(T),
    /// Finished this tick; write the exact `to` value and remove the tween.
    Done(T, bool /* mark_on_complete */),
}

/// Advance one tween by `delta`, honouring reduced motion. Returns the value to
/// write and whether the tween is finished.
fn tick_tween<T: Lerp>(tween: &mut Tween<T>, delta: Duration, reduced_motion: bool) -> Tick<T> {
    if reduced_motion {
        // Jump-to-end: no interpolation, complete immediately (spec § 8).
        return Tick::Done(tween.to.clone(), tween.mark_on_complete);
    }
    tween.elapsed = tween.elapsed.saturating_add(delta);
    if tween.is_complete() {
        Tick::Done(tween.to.clone(), tween.mark_on_complete)
    } else {
        Tick::Running(tween.value())
    }
}

/// Read the active reduced-motion preference (defaulting to "not reduced" when
/// the resource is absent, e.g. a bare test app without `ThemePlugin`).
fn reduced_motion(prefs: Option<&crate::theme::UserPreferences>) -> bool {
    prefs.is_some_and(|p| p.prefers_reduced_motion)
}

/// Drive [`TranslateTween`] → [`Translate`].
pub fn advance_translate_tweens(
    mut commands: Commands,
    time: Res<Time>,
    prefs: Option<Res<crate::theme::UserPreferences>>,
    mut q: Query<(Entity, &mut TranslateTween)>,
) {
    let dt = time.delta();
    let rm = reduced_motion(prefs.as_deref());
    for (entity, mut target) in &mut q {
        match tick_tween(&mut target.0, dt, rm) {
            Tick::Running(v) => {
                commands.entity(entity).insert(Translate(
                    Length::px(v.x),
                    Length::px(v.y),
                    Length::px(v.z),
                ));
            }
            Tick::Done(v, mark) => {
                let mut ec = commands.entity(entity);
                ec.insert(Translate(Length::px(v.x), Length::px(v.y), Length::px(v.z)))
                    .remove::<TranslateTween>();
                if mark {
                    ec.insert(OnComplete);
                }
            }
        }
    }
}

/// Drive [`RotateTween`] → [`Rotate`].
pub fn advance_rotate_tweens(
    mut commands: Commands,
    time: Res<Time>,
    prefs: Option<Res<crate::theme::UserPreferences>>,
    mut q: Query<(Entity, &mut RotateTween)>,
) {
    let dt = time.delta();
    let rm = reduced_motion(prefs.as_deref());
    for (entity, mut target) in &mut q {
        match tick_tween(&mut target.0, dt, rm) {
            Tick::Running(v) => {
                commands.entity(entity).insert(Rotate(v));
            }
            Tick::Done(v, mark) => {
                let mut ec = commands.entity(entity);
                ec.insert(Rotate(v)).remove::<RotateTween>();
                if mark {
                    ec.insert(OnComplete);
                }
            }
        }
    }
}

/// Drive [`ScaleTween`] → [`Scale`].
pub fn advance_scale_tweens(
    mut commands: Commands,
    time: Res<Time>,
    prefs: Option<Res<crate::theme::UserPreferences>>,
    mut q: Query<(Entity, &mut ScaleTween)>,
) {
    let dt = time.delta();
    let rm = reduced_motion(prefs.as_deref());
    for (entity, mut target) in &mut q {
        match tick_tween(&mut target.0, dt, rm) {
            Tick::Running(v) => {
                commands.entity(entity).insert(Scale(v.x, v.y, v.z));
            }
            Tick::Done(v, mark) => {
                let mut ec = commands.entity(entity);
                ec.insert(Scale(v.x, v.y, v.z)).remove::<ScaleTween>();
                if mark {
                    ec.insert(OnComplete);
                }
            }
        }
    }
}

/// Drive [`OpacityTween`] → [`Opacity`].
pub fn advance_opacity_tweens(
    mut commands: Commands,
    time: Res<Time>,
    prefs: Option<Res<crate::theme::UserPreferences>>,
    mut q: Query<(Entity, &mut OpacityTween)>,
) {
    let dt = time.delta();
    let rm = reduced_motion(prefs.as_deref());
    for (entity, mut target) in &mut q {
        match tick_tween(&mut target.0, dt, rm) {
            Tick::Running(v) => {
                commands.entity(entity).insert(Opacity(v));
            }
            Tick::Done(v, mark) => {
                let mut ec = commands.entity(entity);
                ec.insert(Opacity(v)).remove::<OpacityTween>();
                if mark {
                    ec.insert(OnComplete);
                }
            }
        }
    }
}

/// Drive [`BackgroundColorTween`] → [`AnimatedBackgroundColor`].
pub fn advance_background_color_tweens(
    mut commands: Commands,
    time: Res<Time>,
    prefs: Option<Res<crate::theme::UserPreferences>>,
    mut q: Query<(Entity, &mut BackgroundColorTween)>,
) {
    let dt = time.delta();
    let rm = reduced_motion(prefs.as_deref());
    for (entity, mut target) in &mut q {
        match tick_tween(&mut target.0, dt, rm) {
            Tick::Running(v) => {
                commands.entity(entity).insert(AnimatedBackgroundColor(v));
            }
            Tick::Done(v, mark) => {
                let mut ec = commands.entity(entity);
                ec.insert(AnimatedBackgroundColor(v))
                    .remove::<BackgroundColorTween>();
                if mark {
                    ec.insert(OnComplete);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f32_lerp_endpoints() {
        // Fully-qualified so the trait impl is exercised (not any inherent
        // `lerp` that may shadow the method name).
        assert_eq!(Lerp::lerp(&0.0_f32, &20.0, 0.0), 0.0);
        assert_eq!(Lerp::lerp(&0.0_f32, &20.0, 1.0), 20.0);
        assert!((Lerp::lerp(&0.0_f32, &20.0, 0.5) - 10.0).abs() < 1e-6);
    }

    #[test]
    fn vec3_lerp_endpoints() {
        let a = Vec3::ZERO;
        let b = Vec3::new(20.0, 0.0, 0.0);
        assert_eq!(Lerp::lerp(&a, &b, 0.0), a);
        assert_eq!(Lerp::lerp(&a, &b, 1.0), b);
    }

    #[test]
    fn tween_zero_duration_is_complete_at_to() {
        let t = Tween::new(0.0_f32, 5.0, Duration::ZERO, Easing::Linear);
        assert!(t.is_complete());
        assert_eq!(t.value(), 5.0);
    }

    #[test]
    fn tween_value_tracks_elapsed_linear() {
        let mut t = Tween::secs(0.0_f32, 10.0, 1.0, Easing::Linear);
        assert_eq!(t.value(), 0.0);
        t.elapsed = Duration::from_secs_f32(0.5);
        assert!((t.value() - 5.0).abs() < 1e-5);
        t.elapsed = Duration::from_secs_f32(1.0);
        assert_eq!(t.value(), 10.0);
    }

    #[test]
    fn tick_reduced_motion_snaps_to_end() {
        let mut t = Tween::secs(0.0_f32, 20.0, 0.15, Easing::DESIGN);
        match tick_tween(&mut t, Duration::from_millis(1), true) {
            Tick::Done(v, _) => assert_eq!(v, 20.0),
            Tick::Running(_) => panic!("reduced motion must complete immediately"),
        }
    }
}
