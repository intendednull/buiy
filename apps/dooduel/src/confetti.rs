//! Dooduel confetti — a celebratory particle burst on the podium.
//!
//! A **raw-ECS decoupled overlay** (the `paint.rs` pattern): a system reads the MVU
//! model's `screen` field and, on the rising edge into `Podium`, hand-spawns a
//! burst of small colored-quad Buiy nodes, each with a **fall** [`TranslateTween`],
//! a **tumble** [`RotateTween`], and an end-of-life **fade** [`QuadAlphaTween`]; a
//! piece despawns when its fall tween completes ([`OnComplete`]).
//!
//! **The seam.** Confetti is a pure *side effect* of the model, exactly like the
//! paint canvas: the trigger system only READS `Dooduel.screen` (never enqueues or
//! mutates the model), so it doesn't touch the MVU funnel. The pieces are decoupled
//! ECS entities the `buiy_view` reconciler never owns.
//!
//! **The fade is composite-free** (F4b). The design fades each particle near
//! end-of-life. A Buiy `Opacity < 1.0` forms an `EffectGroup` — an OFF-SCREEN
//! composite boundary — so an `OpacityTween` per piece would spin up ~110 off-screen
//! render targets. F4b's per-quad [`QuadAlphaTween`] is an alpha that does NOT
//! promote to a group, so the burst fades AND stays composite-free.
//! `Translate`/`Rotate` likewise form only a cheap stacking context.

use std::time::Duration;

use bevy::prelude::*;

use buiy_core::Node;
use buiy_core::animation::{
    Easing, OnComplete, QuadAlphaTween, RotateTween, TranslateTween, Tween,
};
use buiy_core::layout::{BoxModel, Length, Sizing, Translate};
use buiy_core::render::{Background, Border, ColorToken, Corners, Radius};

/// The design's six confetti colors (`CONFETTI_COLORS`), as sRGB.
const CONFETTI_COLORS: [(u8, u8, u8); 6] = [
    (0x7c, 0x4f, 0xe0), // accent purple
    (0xff, 0xd2, 0x3f), // yellow
    (0xff, 0x6b, 0x4a), // coral
    (0x2f, 0xbf, 0x71), // green
    (0x3b, 0x82, 0xf6), // blue
    (0xff, 0x3e, 0xa5), // pink
];

/// How many pieces one podium burst spawns (design: 130 — kept mid-range for a
/// prototype; the spawn cost is journaled).
const BURST_COUNT: usize = 110;

/// Marks a confetti particle so the trigger can bulk-clear leftovers on podium
/// exit and the despawn system can reap finished pieces.
#[derive(Component)]
pub struct ConfettiPiece;

/// A tiny deterministic splitmix64 PRNG so a burst is reproducible per session
/// (and never pulls in wall-clock randomness — the game-core determinism rule).
#[derive(Resource)]
pub struct ConfettiRng(u64);

impl Default for ConfettiRng {
    fn default() -> Self {
        Self(0x0C0F_FEE1_2345_6789)
    }
}

impl ConfettiRng {
    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }

    /// A uniform float in `[0, 1)`.
    fn f32(&mut self) -> f32 {
        (self.next_u64() >> 40) as f32 / (1u64 << 24) as f32
    }

    /// A uniform index in `0..n`.
    fn index(&mut self, n: usize) -> usize {
        (self.next_u64() as usize) % n.max(1)
    }
}

/// Read the model's screen and, on the rising edge into `Podium`, spawn one
/// confetti burst; on leaving `Podium`, clear any pieces still falling.
fn drive_confetti(
    model: Option<Single<&crate::Dooduel>>,
    windows: Query<&Window>,
    mut rng: ResMut<ConfettiRng>,
    mut commands: Commands,
    pieces: Query<Entity, With<ConfettiPiece>>,
    mut was_podium: Local<bool>,
) {
    let Some(model) = model else {
        return;
    };
    let podium = model.screen == crate::Screen::Podium;
    if podium && !*was_podium {
        let (vw, vh) = windows
            .iter()
            .next()
            .map(|w| (w.width(), w.height()))
            .unwrap_or((1280.0, 800.0));
        burst(&mut commands, &mut rng, vw, vh);
    } else if !podium && *was_podium {
        for e in &pieces {
            commands.entity(e).despawn();
        }
    }
    *was_podium = podium;
}

/// Spawn one burst of [`BURST_COUNT`] falling pieces across the top of the
/// viewport.
fn burst(commands: &mut Commands, rng: &mut ConfettiRng, vw: f32, vh: f32) {
    // Gravity-like accelerating fall: a strong ease-in (slow start, fast end).
    let fall = Easing::CubicBezier(0.35, 0.0, 1.0, 1.0);
    for _ in 0..BURST_COUNT {
        let (r, g, b) = CONFETTI_COLORS[rng.index(CONFETTI_COLORS.len())];
        let size = 6.0 + rng.f32() * 6.0; // 6..12 px
        let start_x = vw * 0.5 + (rng.f32() - 0.5) * vw * 0.7;
        let start_y = -20.0 - rng.f32() * 60.0;
        let end_x = start_x + (rng.f32() - 0.5) * 160.0; // horizontal drift
        let end_y = vh + 60.0; // off the bottom
        let dur = 1.3 + rng.f32() * 1.1; // 1.3..2.4 s
        let rot0 = rng.f32() * std::f32::consts::TAU;
        let spin = (rng.f32() - 0.5) * 12.0; // total tumble, radians
        let duration = Duration::from_secs_f32(dur);

        commands.spawn((
            Node,
            ConfettiPiece,
            BoxModel {
                width: Sizing::Length(Length::px(size)),
                height: Sizing::Length(Length::px(size * 0.66)),
                ..default()
            },
            Background {
                color: ColorToken::Custom(Color::srgb_u8(r, g, b)),
            },
            // Borderless-rounded: a small radius softens the rects (no border
            // side paints, so the fill itself rounds).
            Border {
                radius: Corners::all(Radius::circular((size * 0.2).max(1.0))),
                ..default()
            },
            Translate(Length::px(start_x), Length::px(start_y), Length::px(0.0)),
            TranslateTween(
                Tween::new(
                    Vec3::new(start_x, start_y, 0.0),
                    Vec3::new(end_x, end_y, 0.0),
                    duration,
                    fall,
                )
                .with_on_complete(),
            ),
            RotateTween(Tween::new(
                Quat::from_rotation_z(rot0),
                Quat::from_rotation_z(rot0 + spin),
                duration,
                Easing::Linear,
            )),
            // End-of-life fade (F4b, composite-free): hold ~opaque for most of the
            // fall then fade out near the bottom (the ease-in keeps alpha high early
            // and drops it late).
            QuadAlphaTween(Tween::new(
                1.0,
                0.0,
                duration,
                Easing::CubicBezier(0.8, 0.0, 1.0, 1.0),
            )),
        ));
    }
}

/// Reap pieces whose fall tween finished (the tween system tags them `OnComplete`
/// as it removes the tween).
fn despawn_finished(
    mut commands: Commands,
    done: Query<Entity, (With<ConfettiPiece>, With<OnComplete>)>,
) {
    for e in &done {
        commands.entity(e).despawn();
    }
}

/// Installs the podium confetti. Decoupled from `dooduel::install` (like
/// `CanvasPlugin`) — the overlay is a pure side effect of the model's screen.
///
/// The pieces animate through `buiy_core`'s tween-advance systems
/// (`BuiySet::Animate`), which the full `BuiyPlugin` includes but the *headless*
/// `BuiyHeadlessPlugin` deliberately omits — so we add `AnimationPlugin` here if
/// it is not already present (idempotent across the windowed + capture bins).
pub struct ConfettiPlugin;

impl Plugin for ConfettiPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<buiy_core::animation::AnimationPlugin>() {
            app.add_plugins(buiy_core::animation::AnimationPlugin);
        }
        app.init_resource::<ConfettiRng>();
        app.add_systems(Update, (drive_confetti, despawn_finished));
    }
}
