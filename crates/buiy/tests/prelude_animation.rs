//! Parity spec § 2 REFINE (prelude promotions): the animation authoring
//! primitives are reachable from the `buiy` prelude in one `use`. This wave
//! promotes `Repeat` (the `Once`/`Loop`/`PingPong` loop control the values
//! table's `infinite` blink/pulse status dots need) alongside the already-
//! promoted `Tween`/`Easing` per-property model. Compile-only: naming the types
//! through the prelude path is the assertion.
#[test]
fn animation_primitives_are_in_the_prelude() {
    use bevy::prelude::Color;
    use buiy::prelude::*;

    // The loop control this wave promotes (§ 2 REFINE) — name every variant so a
    // future rename or a dropped re-export breaks the build here.
    let _once: Repeat = Repeat::Once;
    let _loop: Repeat = Repeat::Loop { count: Some(3) };
    let _ping: Repeat = Repeat::PingPong { count: None };

    // The per-property tween model it rides on (already promoted; named here so
    // the animation prelude surface is documented in one place).
    let _tween: Tween<Color> = Tween::secs(Color::WHITE, Color::BLACK, 1.0, Easing::Linear);
    let _eased: Easing = Easing::DESIGN;
}
