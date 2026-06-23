//! Buiy widgets. Phase 0 shipped a single `Button`; Wave-3 slice-1 adds the
//! Checkbox + Switch toggle widgets, slice-2 adds the Slider value widget (the
//! P1d a11y bundle + the C4 visual layer, bundle-then-pixels in one pass). Full
//! APG widget catalog lives in `buiy-widget-catalog-design`.

use bevy::prelude::*;
use buiy_core::{
    BuiySet,
    a11y::{A11yExpanded, A11yRole, A11yToggled},
};

pub mod button;
pub mod checkbox;
pub mod disclosure;
pub mod scene;
pub mod slider;
pub mod switch;
pub mod text_input;
pub use button::Button;
pub use checkbox::Checkbox;
pub use disclosure::Disclosure;
pub use slider::Slider;
pub use switch::Switch;
// `OnPress` relocated to `buiy_core` (co-drive SC-1) so the in-core P1c action
// router and C3 pointer layer can write the same activation sink. Re-exported
// here for source-compat: `buiy_widgets::OnPress` and the `buiy` prelude keep
// resolving unchanged.
pub use buiy_core::interaction::OnPress;
pub use scene::{
    button, checkbox as checkbox_scene, disclosure as disclosure_scene, slider as slider_scene,
    switch as switch_scene,
};
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

/// The single `OnPress` consumer that **toggles** an expandable widget's
/// `A11yExpanded` (the Disclosure analog of [`advance_toggle_on_press`], Wave-3
/// slice-3). A Disclosure-trigger is `A11yRole::Button` (so its `Click` rides the
/// Button contract → `OnPress`), and it is *expandable* (it carries
/// [`A11yExpanded`]). Pointer click, keyboard activation (Enter/Space via the
/// Button keymap), and an inbound AT `Action::Click` all converge on the one
/// `OnPress` sink — this consumer flips `A11yExpanded` once per activation, so
/// every modality toggles the disclosure identically.
///
/// The explicit AT **set-verbs** `Expand`/`Collapse` take a *different* route: the
/// router honors them generically (action.rs), writing the absolute target state.
/// Together they give the disclosure three converging toggle modalities
/// (pointer/keyboard/AT-`Click`) plus the two absolute AT set-verbs, all over the
/// single `A11yExpanded` source of truth the C4 visual reads.
///
/// Querying `&mut A11yExpanded` (not gated on role) keeps this reusable: any future
/// expandable that activates through `OnPress` toggles by carrying `A11yExpanded`.
/// An entity without it (a Button/Checkbox/Slider) is simply not matched here, so
/// its `OnPress` is inert for this consumer (it flows through
/// `advance_toggle_on_press` or the button callback instead).
pub fn advance_expanded_on_press(
    mut reader: MessageReader<OnPress>,
    mut expandables: Query<&mut A11yExpanded>,
) {
    for OnPress(entity) in reader.read() {
        if let Ok(mut expanded) = expandables.get_mut(*entity) {
            expanded.0 = !expanded.0;
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
            .register_type::<Slider>()
            .register_type::<slider::SliderTrack>()
            .register_type::<slider::SliderThumb>()
            .register_type::<Disclosure>()
            .register_type::<disclosure::DisclosureCaret>()
            .register_type::<disclosure::DisclosurePanel>()
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
        // The Disclosure analog of the toggle consumer (slice-3): pointer/keyboard/
        // AT-`Click` all converge on `OnPress`; this flips `A11yExpanded`. Runs in
        // the same activation stage (`BuiySet::Input`) as the toggle consumer and
        // the producers, so a same-frame activation flips expanded the same frame
        // and the later `BuiySet::A11yUpdate` fold sees it.
        app.add_systems(Update, advance_expanded_on_press.in_set(BuiySet::Input));
        app.add_systems(
            Update,
            (
                checkbox::update_checkbox_visual,
                switch::update_switch_visual,
            )
                .after(advance_toggle_on_press),
        );
        // The Disclosure C4 visual (slice-3) reads `Changed<A11yExpanded>` to rotate
        // the caret + show/hide the panel. `A11yExpanded` is flipped by the
        // `advance_expanded_on_press` consumer (pointer/keyboard/AT-`Click`) and by
        // the router's generic `Expand`/`Collapse` honor (the absolute AT set-verbs),
        // both in `BuiySet::Input`; this visual runs `.after` the consumer so a
        // same-frame toggle is observed the same frame, then settles on the
        // `Changed<A11yExpanded>` gate.
        app.add_systems(
            Update,
            disclosure::update_disclosure_visual.after(advance_expanded_on_press),
        );
        // Wire each disclosure trigger's `A11yRelations.controls = [panel]` once its
        // `children!` exist (the `controls` edge references the panel entity, which
        // does not exist at root-spawn time, so it can't ride the `#[require]` /
        // `Disclosure::new` bundle). Idempotent over the scene-fn path (which
        // authors `controls` directly).
        app.add_systems(Update, disclosure::wire_disclosure_controls);
        // The slider C4 visual (slice-2) reads `Changed<A11yValue>` to reposition
        // the thumb. A slider's value is mutated by the slider contract's `honor`
        // (driven by the APG `slider_keyboard` system / an inbound AT verb, both in
        // `buiy_core`'s `BuiySet::Input`), NOT through the `OnPress` toggle sink —
        // so this visual does not chain after `advance_toggle_on_press`; it runs in
        // `Update` and settles on the `Changed<A11yValue>` gate.
        app.add_systems(Update, slider::update_slider_visual);

        // P1d TextInput a11y sync: mirror the editor's live value into
        // `A11yTextValue` and the `Placeholder` into `A11yPlaceholder` on each
        // `TextInput` root, so the outbound a11y fold (`build_tree`, in
        // `BuiySet::A11yUpdate`) sees the live text. It runs in `BuiySet::Animate`,
        // which the `CorePlugin` set-chain orders strictly BEFORE `A11yUpdate`
        // (`… → Animate → Picking → A11yUpdate → …`), so a value mutated this frame
        // (keyboard edit, or an inbound AT `SetValue` honored in `BuiySet::Input`)
        // is synced into `A11yTextValue` and folded into the a11y tree in the SAME
        // frame. (`build_tree` is `pub(crate)` to `buiy_core`, so cross-crate
        // `.before(build_tree)` is not expressible; the set-chain provides the
        // ordering instead.)
        app.add_systems(
            Update,
            text_input::sync_text_input_a11y.in_set(BuiySet::Animate),
        );
    }
}
