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

/// How a [`Tween`] behaves when it reaches the end of one `from`→`to` pass.
///
/// The design eases most motion ONCE (entrance translateY, the thumb slide), but
/// the menu's "last action" / "ready" status dots PULSE forever (CSS
/// `blink 1.6s infinite`, opacity `1`→`.25`→`1`). A one-shot tween cannot express
/// that, so `Repeat` lets a tween loop (sawtooth restart) or ping-pong (reverse
/// each pass), a fixed number of cycles or forever (`None` count).
///
/// Under reduced motion a repeating tween snaps to its steady "rest" state and
/// completes immediately (it never oscillates): the `to` end for [`Repeat::Once`]
/// / [`Repeat::Loop`], the `from` end (bright/on) for [`Repeat::PingPong`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Repeat {
    /// Run `from`→`to` once, then complete + remove (the default — the entrance /
    /// thumb-slide / chevron behaviour).
    #[default]
    Once,
    /// On reaching `to`, restart at `from` (a sawtooth). `count` is the number of
    /// passes remaining; `None` repeats forever. A `Some(0)`/`Some(1)` pass left
    /// completes like [`Repeat::Once`].
    Loop { count: Option<u32> },
    /// On reaching `to`, reverse direction so the value oscillates
    /// `from`→`to`→`from`→… (the blink/pulse dot). `count` is the number of
    /// one-way passes remaining; `None` oscillates forever. The "rest" value a
    /// reduced-motion snap lands on is `from` (the dot's bright/on state).
    PingPong { count: Option<u32> },
}

impl Repeat {
    /// Whether this mode keeps the tween alive past one pass (loop or ping-pong
    /// with passes still to run). A `count` of 0 or 1 has no further pass.
    fn repeats(&self) -> bool {
        match self {
            Repeat::Once => false,
            Repeat::Loop { count } | Repeat::PingPong { count } => count.is_none_or(|c| c > 1),
        }
    }

    /// Decrement the remaining pass count (saturating at 0); infinite stays
    /// infinite. Called once per completed pass.
    fn decremented(self) -> Self {
        match self {
            Repeat::Once => Repeat::Once,
            Repeat::Loop { count } => Repeat::Loop {
                count: count.map(|c| c.saturating_sub(1)),
            },
            Repeat::PingPong { count } => Repeat::PingPong {
                count: count.map(|c| c.saturating_sub(1)),
            },
        }
    }
}

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
    /// How the tween behaves at the end of one `from`→`to` pass — once (default),
    /// looping, or ping-pong. See [`Repeat`].
    pub repeat: Repeat,
}

impl<T: Lerp> Tween<T> {
    /// A tween from `from` to `to` over `duration` with `easing`. `elapsed`
    /// starts at zero, `mark_on_complete` is off, and it runs once.
    pub fn new(from: T, to: T, duration: Duration, easing: Easing) -> Self {
        Self {
            from,
            to,
            duration,
            elapsed: Duration::ZERO,
            easing,
            mark_on_complete: false,
            repeat: Repeat::Once,
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

    /// Set the [`Repeat`] mode (looping / ping-pong). Builder form so the blink
    /// dot reads `Tween::secs(1.0, 0.25, 0.8, …).with_repeat(Repeat::PingPong {
    /// count: None })`.
    pub fn with_repeat(mut self, repeat: Repeat) -> Self {
        self.repeat = repeat;
        self
    }

    /// The steady "rest" value a reduced-motion snap lands on. For a one-shot or
    /// looping tween that is the `to` end (its natural completion); for a
    /// ping-pong (a blink/pulse) it is the `from` end — the dot's bright/on
    /// state, so reduced motion shows a steady-lit dot, never a frozen dim one.
    fn rest_value(&self) -> T {
        match self.repeat {
            Repeat::PingPong { .. } => self.from.clone(),
            Repeat::Once | Repeat::Loop { .. } => self.to.clone(),
        }
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
///
/// A one-shot tween completes when `elapsed >= duration`. A repeating tween
/// (loop / ping-pong) instead WRAPS at the pass boundary — it subtracts one
/// `duration` from `elapsed` (carrying the remainder so the cadence never
/// drifts), decrements its pass count, and for ping-pong swaps `from`/`to` so
/// the value reverses. It only completes once no passes remain. Under reduced
/// motion EVERY tween (one-shot or repeating) snaps to its steady rest value and
/// completes immediately — a blink/pulse never oscillates (spec § 8 /
/// values.md § 5.3 jump-to-end).
fn tick_tween<T: Lerp>(tween: &mut Tween<T>, delta: Duration, reduced_motion: bool) -> Tick<T> {
    if reduced_motion {
        // Jump-to-rest: no interpolation, complete immediately (spec § 8). The
        // rest value is `to` for once/loop, `from` for ping-pong (steady-lit dot).
        return Tick::Done(tween.rest_value(), tween.mark_on_complete);
    }
    tween.elapsed = tween.elapsed.saturating_add(delta);
    if !tween.is_complete() {
        return Tick::Running(tween.value());
    }
    // Reached the end of this pass. A non-repeating tween (or one with no passes
    // left) completes at `to`; a repeating one wraps and keeps running.
    if !tween.repeat.repeats() {
        return Tick::Done(tween.to.clone(), tween.mark_on_complete);
    }
    // Carry the overshoot past `duration` into the next pass so the cadence is
    // drift-free (a long frame does not lose time). A zero-duration repeating
    // tween cannot make progress per pass, so it degrades to a single completion.
    if tween.duration.is_zero() {
        return Tick::Done(tween.to.clone(), tween.mark_on_complete);
    }
    tween.elapsed = tween.elapsed.saturating_sub(tween.duration);
    if matches!(tween.repeat, Repeat::PingPong { .. }) {
        std::mem::swap(&mut tween.from, &mut tween.to);
    }
    tween.repeat = tween.repeat.decremented();
    Tick::Running(tween.value())
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
