//! Buiy widgets. Phase 0 shipped a single `Button`; Wave-3 slice-1 adds the
//! Checkbox + Switch toggle widgets (the P1d a11y bundle + the C4 visual layer,
//! bundle-then-pixels in one pass). Full APG widget catalog lives in
//! `buiy-widget-catalog-design`.

use bevy::prelude::*;
use buiy_core::{
    BuiySet,
    a11y::{A11yRole, A11yToggled},
};

pub mod button;
pub mod checkbox;
pub mod scene;
pub mod switch;
pub mod text_input;
pub use button::Button;
pub use checkbox::Checkbox;
pub use switch::Switch;
// `OnPress` relocated to `buiy_core` (co-drive SC-1) so the in-core P1c action
// router and C3 pointer layer can write the same activation sink. Re-exported
// here for source-compat: `buiy_widgets::OnPress` and the `buiy` prelude keep
// resolving unchanged.
pub use buiy_core::interaction::OnPress;
pub use scene::{button, checkbox as checkbox_scene, switch as switch_scene};
pub use scene::{text_input_multi_line, text_input_single_line};
pub use text_input::TextInput;

/// The single `OnPress` consumer that advances a toggle widget's `A11yToggled`
/// (co-drive SC-1 — "one sink, consumers read `OnPress`"). EVERY activation
/// modality converges here:
///
/// - the pointer producer (`pointer_click_emits_on_press`) writes `OnPress` for an
///   activatable role on a `Pointer<Click>`,
/// - the keyboard keymap (`keyboard_activation`) writes `OnPress` on the role's
///   APG activation keys (Checkbox = Space only; Switch = Space + Enter),
/// - the inbound AT router's `honor(Click)` writes `OnPress`.
///
/// This system reads each `OnPress(entity)`, looks up the entity's role + its
/// `A11yToggled`, and advances it: a **Checkbox** advances tri-state
/// (`False → True → False`, `Mixed → False`); a **Switch** flips binary
/// (`False ↔ True`). A `Button` carries no `A11yToggled`, so its `OnPress` is
/// inert here (the button fires its own callback elsewhere). Because all three
/// modalities feed this one consumer, a toggle advances **exactly once** per
/// activation regardless of source — and the C4 visual systems' `Changed<…>`
/// gates then repaint once.
pub fn advance_toggle_on_press(
    mut reader: MessageReader<OnPress>,
    mut toggles: Query<(&A11yRole, &mut A11yToggled)>,
) {
    for OnPress(entity) in reader.read() {
        let Ok((role, mut toggled)) = toggles.get_mut(*entity) else {
            // Not a toggle widget (no `A11yToggled`) — e.g. a Button. Inert.
            continue;
        };
        match role {
            A11yRole::Checkbox => toggled.advance_checkbox(),
            A11yRole::Switch => toggled.toggle_switch(),
            // A non-toggle role that nonetheless carries `A11yToggled` (e.g. a
            // toggle Button via aria-pressed) is the Button widget's concern, not
            // this consumer's — leave it untouched here.
            _ => {}
        }
    }
}

pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        // `Messages<OnPress>` is registered by `CorePlugin`
        // (`InteractionPlugin`, co-drive SC-1), not here — the shared
        // activation sink lives in `buiy_core` so in-core producers can write
        // it. `WidgetsPlugin` is always composed after `CorePlugin`.
        //
        // C3c retired the Phase-0 `Hovered`-polling input systems
        // (input-event-model.md § 2.8): Button activation now lowers through
        // `buiy_core`'s C3b `Pointer<Click>` → `OnPress` producer (registered by
        // `PickingPlugin`). C3d (§ 2.7) then consolidated focus-on-click into
        // `buiy_core`'s single `focus::focus_on_click` observer (registered by
        // `FocusPlugin`) over every `Focusable`, so the widget crate carries no
        // focus observer either — `TextInput` `#[require]`s `Focusable` and is
        // focused through that shared path.
        app.register_type::<Button>()
            .register_type::<Checkbox>()
            .register_type::<checkbox::CheckboxMark>()
            .register_type::<Switch>()
            .register_type::<switch::SwitchThumb>()
            .register_type::<text_input::TextInput>();

        // Wave-3 slice-1: the single `OnPress` toggle consumer + the C4 visual
        // systems.
        //
        // `advance_toggle_on_press` reads the shared `OnPress` sink and advances
        // `A11yToggled`. It runs in `BuiySet::Input` (the activation stage) — the
        // pointer/keyboard producers write `OnPress` in the same stage, and a
        // Message written this frame is readable the same frame, so the toggle
        // advances within the frame and the (later) `BuiySet::A11yUpdate`
        // outbound fold sees the new state. (For the headless AT driver, whose
        // `dispatch_action_request` writes `OnPress` synchronously without
        // ticking, the consumer runs on the next `app.update()` — the documented
        // `perform`-then-`update` contract.)
        //
        // The C4 visual systems read `Changed<A11yToggled>` to repaint; they run
        // in `Update` AFTER the consumer (`.after`) so a same-frame toggle is
        // observed by the visual the same frame, then settle on the `Changed`
        // gate.
        app.add_systems(Update, advance_toggle_on_press.in_set(BuiySet::Input));
        app.add_systems(
            Update,
            (
                checkbox::update_checkbox_visual,
                switch::update_switch_visual,
            )
                .after(advance_toggle_on_press),
        );
    }
}
