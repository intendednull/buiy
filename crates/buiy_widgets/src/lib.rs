//! Buiy widgets. Phase 0 shipped a single `Button`; Wave-3 slice-1 adds the
//! Checkbox + Switch toggle widgets, slice-2 adds the Slider value widget (the
//! P1d a11y bundle + the C4 visual layer, bundle-then-pixels in one pass). Full
//! APG widget catalog lives in `buiy-widget-catalog-design`.

use bevy::prelude::*;
use buiy_core::{
    BuiySet,
    a11y::{A11yExpanded, A11yRole, A11yToggled, A11yValue, InlineActionRegistry, Toggled},
    mvu::{
        ControlledLeaf, MvuAppExt, MvuCorePlugin, ToggleLeafSet, ToggleMsg, enqueue,
        register_toggle_leaf,
    },
};

pub mod button;
pub mod checkbox;
pub mod composites;
pub mod dialog;
pub mod disclosure;
pub mod dismiss;
pub mod menu;
pub mod popover;
pub mod scene;
pub mod scroll_area;
pub mod slider;
pub mod switch;
pub mod text_input;
pub mod tooltip;
pub use button::Button;
pub use checkbox::Checkbox;
pub use dialog::Dialog;
pub use disclosure::Disclosure;
pub use dismiss::LightDismiss;
pub use menu::{Menu, MenuButton, MenuItem};
pub use popover::{Popover, PopoverAlign, PopoverPlacement, PopoverSide};
pub use scroll_area::ScrollArea;
pub use slider::Slider;
pub use switch::Switch;
pub use tooltip::TooltipTrigger;
// `OnPress` relocated to `buiy_core` (co-drive SC-1) so the in-core P1c action
// router and C3 pointer layer can write the same activation sink. Re-exported
// here for source-compat: `buiy_widgets::OnPress` and the `buiy` prelude keep
// resolving unchanged. `ValueChange<T>` (Track C / F2) is the typed value-change
// notification the emitters below write; re-exported alongside `OnPress` (and it
// brings the type into this module's scope for the emitters).
pub use buiy_core::interaction::{OnPress, ValueChange};
pub use dialog::dialog_invoker;
pub use scene::{
    button, checkbox as checkbox_scene, dialog as dialog_scene, disclosure as disclosure_scene,
    menu as menu_scene, menu_button as menu_button_scene, menu_item, popover as popover_scene,
    scroll_area, slider as slider_scene, switch as switch_scene, tooltip_trigger,
};
pub use scene::{text_input_multi_line, text_input_single_line};
pub use text_input::TextInput;
// The general composite builders (Wave-5 parity promotion): imperative
// `World`-spawning trees that compose the primitive widgets/render components into
// a recognizable control. Re-exported at the crate root next to the markers +
// scene-fns; also folded into `buiy::prelude` via the `buiy` crate.
pub use composites::{
    CMD_GLYPH_ICON, MeterFill, RowSelBar, TableRow, TableRowData, kbd, kbd_content, meter,
    pulse_blink, search_input, set_meter, set_table_row_selected, status_dot, table_header,
    table_row,
};

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
/// This system reads each `OnPress(entity)`, looks up the entity's role, and — for a
/// **Checkbox** or **Switch** — **enqueues** a [`ToggleMsg::Toggle`] for the pressed
/// entity (the leaf enqueues to ITSELF). A `Button` carries no `A11yToggled`, so its
/// `OnPress` is inert here (the button fires its own callback elsewhere).
///
/// **W2 single-writer reroute (D3).** This system used to mutate `A11yToggled` directly
/// (`advance_checkbox`/`toggle_switch`), which made it one of several writers of that
/// component (the gallery's direct seed/`toggleAll` writes, AT verbs, …) — the proto-1
/// REFINE #5 flicker / multi-writer race. It now only **routes**: the activation Msg
/// enters the MVU funnel and the single ordered drain ([`ToggleLeafSet::Drain`]) is the
/// SOLE writer of `A11yToggled`, committing via `set_if_neq` (so a no-op fold cannot
/// cascade). The role-gate (only Checkbox/Switch toggle on press) stays HERE on the
/// routing side, not in the shared
/// [`toggle_reducer`](buiy_core::mvu::toggle_reducer), which is pure value-folding.
///
/// It runs in [`ToggleLeafSet::Enqueue`] (the toggle leaf's EARLY enqueue-only edge —
/// `.after(BuiySet::Picking)`, where every `OnPress` producer has written), so the
/// pinned `ApplyDeferred` flushes the enqueue and the early drain folds it BEFORE
/// `BuiySet::A11yUpdate` builds the a11y tree — all in the SAME frame (REFINE #1).
pub fn advance_toggle_on_press(
    mut reader: MessageReader<OnPress>,
    // `Without<ControlledLeaf>`: a widget whose `A11yToggled` is owned by an external model
    // (e.g. a `buiy_view` controlled checkbox — design §3 #16) opts OUT of the press-to-toggle
    // leaf, so its model route is the sole source of the fold (no double-fold). The drain stays
    // the sole writer either way.
    toggles: Query<&A11yRole, (With<A11yToggled>, Without<ControlledLeaf>)>,
    mut commands: Commands,
) {
    for OnPress(entity) in reader.read() {
        let Ok(role) = toggles.get(*entity) else {
            // Not a toggle widget (no `A11yToggled`), or a `ControlledLeaf` one an external model
            // owns — inert here either way.
            continue;
        };
        // Only Checkbox/Switch toggle on press; a non-toggle role that nonetheless
        // carries `A11yToggled` (e.g. a toggle Button via aria-pressed) is the Button
        // widget's concern, not this consumer's.
        if matches!(role, A11yRole::Checkbox | A11yRole::Switch) {
            enqueue::<A11yToggled>(&mut commands, *entity, ToggleMsg::Toggle);
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
/// Querying `&mut A11yExpanded` keeps this reusable: any future expandable that
/// activates through `OnPress` toggles by carrying `A11yExpanded`. An entity without
/// it (a Button/Checkbox/Slider) is simply not matched here, so its `OnPress` is inert
/// for this consumer (it flows through `advance_toggle_on_press` or the button callback
/// instead).
///
/// **W6a (MVU-as-core) exclusion:** a [`MenuButton`]'s `A11yExpanded` is now a
/// bind-derived PROJECTION of its menu's `MenuModel` (the machine tier), so the menu
/// button is EXCLUDED here (`Without<MenuButton>`) — flipping `A11yExpanded` directly
/// would be a second writer racing the bind. The menu press is routed into the funnel
/// by [`menu::route_menu_press`] instead. Disclosures (the other `A11yExpanded` user)
/// are unaffected.
pub fn advance_expanded_on_press(
    mut reader: MessageReader<OnPress>,
    mut expandables: Query<&mut A11yExpanded, Without<MenuButton>>,
) {
    for OnPress(entity) in reader.read() {
        if let Ok(mut expanded) = expandables.get_mut(*entity) {
            expanded.0 = !expanded.0;
        }
    }
}

/// Track C / F2 — emit a typed [`ValueChange<bool>`] when a toggle widget's
/// **committed** `A11yToggled` changes (checkbox / switch / aria-pressed button).
/// Runs after [`ToggleLeafSet::Drain`] (the single writer), so the value is
/// settled and this never competes with the writer. `Ref` + `!is_added()` skips
/// the initial spawn insertion — a `ValueChange` is a *change*, not the starting
/// value (read that with a query). `is_final` is always `true` (discrete toggle).
pub fn emit_toggle_value_change(
    changed: Query<(Entity, Ref<A11yToggled>), Changed<A11yToggled>>,
    mut writer: MessageWriter<ValueChange<bool>>,
) {
    for (entity, toggled) in &changed {
        if toggled.is_added() {
            continue;
        }
        writer.write(ValueChange {
            source: entity,
            value: matches!(toggled.0, Toggled::True),
            is_final: true,
        });
    }
}

/// The `Changed<A11yValue>`-on-a-`Slider` filter for [`emit_slider_value_change`]
/// (factored out to keep the query type under clippy's `type_complexity` bar,
/// mirroring `slider::ChangedSlider`).
type ChangedSliderValue = (With<Slider>, Changed<A11yValue>);

/// Track C / F2 — emit a typed [`ValueChange<f64>`] when a [`Slider`]'s committed
/// `A11yValue` changes. Same post-commit `Changed` + `!is_added()` discipline as
/// [`emit_toggle_value_change`]. `is_final` is always `true` today (the slider has
/// no continuous pointer-drag — every change is a discrete keyboard/AT commit).
pub fn emit_slider_value_change(
    changed: Query<(Entity, Ref<A11yValue>), ChangedSliderValue>,
    mut writer: MessageWriter<ValueChange<f64>>,
) {
    for (entity, value) in &changed {
        if value.is_added() {
            continue;
        }
        writer.write(ValueChange {
            source: entity,
            value: value.now,
            is_final: true,
        });
    }
}

pub struct WidgetsPlugin;

impl Plugin for WidgetsPlugin {
    fn build(&self, app: &mut App) {
        // W2 (MVU-as-core, FINAL): the stateful-leaf tier routes `A11yToggled`
        // writes through the MVU funnel, so the widgets now REQUIRE the MVU chain.
        // `MvuCorePlugin` is SEPARATE from `CorePlugin` (the substrate is cheap when
        // absent — W1's "cheap-when-absent" decision); `WidgetsPlugin` is the first
        // consumer that pulls it in. Because every app composing widgets (incl. the
        // gallery) composes `WidgetsPlugin`, a Checkbox/Switch routes through the
        // early `ToggleLeafSet` drain by default — "core, not opt-in." Add it guarded
        // (a test/app may also add it) and register the shared toggle-leaf model +
        // `toggle_reducer` + the early `.after(Picking).before(A11yUpdate)` drain.
        if !app.is_plugin_added::<MvuCorePlugin>() {
            app.add_plugins(MvuCorePlugin);
        }
        register_toggle_leaf(app);

        // Track C / F2: the typed value-change notifications (`buiy_core::interaction`)
        // the emitters below write — `ValueChange<bool>` (checkbox/switch) and
        // `ValueChange<f64>` (slider). The generic type lives in `buiy_core`; its
        // concrete registrations + emitters live here (the widget crate owns the
        // value widgets), mirroring how `OnPress` is core-registered but toggle-routed.
        app.add_message::<ValueChange<bool>>()
            .add_message::<ValueChange<f64>>();

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
            .register_type::<Dialog>()
            .register_type::<dialog::DialogTitle>()
            .register_type::<dialog::DialogBody>()
            .register_type::<dialog::DialogClose>()
            .register_type::<dialog::PendingFocus>()
            .register_type::<TooltipTrigger>()
            .register_type::<tooltip::TooltipNode>()
            .register_type::<scroll_area::ScrollArea>()
            .register_type::<popover::Popover>()
            .register_type::<dismiss::LightDismiss>()
            .register_type::<menu::Menu>()
            .register_type::<menu::MenuButton>()
            .register_type::<menu::MenuItem>()
            .register_type::<text_input::TextInput>();

        // Wave-3 slice-1 + W2 reroute: the single `OnPress` toggle consumer + the C4
        // visual systems.
        //
        // W2 reroute (D3): `advance_toggle_on_press` reads the shared `OnPress` sink
        // and ENQUEUES a `ToggleMsg::Toggle` instead of mutating `A11yToggled`. The
        // early `ToggleLeafSet::Drain` (the SOLE writer) commits `A11yToggled` via
        // `set_if_neq`.
        //
        // REFINE #1 — the EARLY drain. The leaf folds in the activation-stage
        // `ToggleLeafSet` window (`.after(Picking).before(A11yUpdate)`, configured by
        // `register_toggle_leaf`), NOT the late globally-pinned `MvuSet::Drain`. So the
        // enqueue runs in `ToggleLeafSet::Enqueue`; the pinned `ApplyDeferred` flushes
        // it; the early `ToggleLeafSet::Drain` folds it — all BEFORE
        // `BuiySet::A11yUpdate` builds the a11y tree, so an AT-driver click is reflected
        // in the tree the SAME frame. (A late drain ran `.after(A11yUpdate)`, lagging the
        // tree one frame.)
        app.add_systems(
            Update,
            advance_toggle_on_press.in_set(ToggleLeafSet::Enqueue),
        );
        // The Disclosure analog of the toggle consumer (slice-3): pointer/keyboard/
        // AT-`Click` all converge on `OnPress`; this flips `A11yExpanded`. Runs in
        // the same activation stage (`BuiySet::Input`) as the toggle consumer and
        // the producers, so a same-frame activation flips expanded the same frame
        // and the later `BuiySet::A11yUpdate` fold sees it.
        app.add_systems(Update, advance_expanded_on_press.in_set(BuiySet::Input));
        // The C4 visual systems read `Changed<A11yToggled>` to repaint. The WRITE now
        // happens in the early `ToggleLeafSet::Drain` (REFINE #1), so the visuals run
        // `.after(ToggleLeafSet::Drain)` (not `.after(advance_toggle_on_press)`, which
        // only enqueues) — a same-frame toggle is observed by the visual the same frame,
        // settling on the `Changed` gate. (A direct `A11yToggled` write in a test still
        // trips the visual the same way — the visual only cares that the write landed.)
        app.add_systems(
            Update,
            (
                checkbox::update_checkbox_visual,
                switch::update_switch_visual,
                // Track C / F2: emit `ValueChange<bool>` from the SAME committed
                // `Changed<A11yToggled>` the visuals read, after the drain.
                emit_toggle_value_change,
            )
                .after(ToggleLeafSet::Drain),
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
        // Wire each dialog's `A11yRelations.labelled_by = [title]` / `described_by
        // = [body]` once its `children!` exist (the labelling edges reference the
        // title/body child entities, unknown at root-spawn time — the disclosure
        // `controls` precedent). Idempotent over the scene-fn path (which authors
        // the edges directly).
        app.add_systems(Update, dialog::wire_dialog_relations);

        // C5-d (scroll-overlay-modal.md §C.5) — the Dialog open/close/focus-trap/
        // Esc/restore + inert-background overlay state machine.
        //
        // `open_dialog_on_invoker_press` reads the shared `OnPress` sink (the
        // invoker's Click → OnPress via the Button contract) and shows the
        // controlled dialog + queues the deferred focus-into. Runs in
        // `BuiySet::Input` so a same-frame activation opens the same frame.
        //
        // `close_dialog_on_escape` (Escape — WCAG 2.1.2, always escapes) and
        // `close_dialog_on_button` (a `DialogClose` button) hide the top-most /
        // enclosing dialog. Both in `BuiySet::Input`.
        //
        // `apply_dialog_modal_state` reacts to `Changed<CssVisibility>` on a Dialog
        // to mark/clear the inert background (`A11yHidden` on the rest-of-tree) and
        // capture/restore `FocusReturn`. Ordered `.after` the open/close systems so
        // the same-frame visibility flip is reacted to the same frame.
        //
        // `resolve_pending_focus` drains the deferred focus-into the frame after the
        // dialog's children spawn (§B.3a). `.after(apply_dialog_modal_state)` so the
        // inert background is already in place when it picks the first focusable.
        app.add_systems(
            Update,
            (
                dialog::open_dialog_on_invoker_press,
                dialog::close_dialog_on_escape,
                dialog::close_dialog_on_button,
            )
                .in_set(BuiySet::Input),
        );
        app.add_systems(
            Update,
            dialog::apply_dialog_modal_state
                .in_set(BuiySet::Input)
                .after(dialog::open_dialog_on_invoker_press)
                .after(dialog::close_dialog_on_escape)
                .after(dialog::close_dialog_on_button),
        );
        app.add_systems(
            Update,
            dialog::resolve_pending_focus
                .in_set(BuiySet::Input)
                .after(dialog::apply_dialog_modal_state),
        );
        // Wire each tooltip trigger's `A11yRelations.described_by = [tooltip]` once
        // its `children!` exist (the edge references the tooltip child entity,
        // unknown at root-spawn time). This edge is also the source of truth the
        // router's generic `ShowTooltip`/`HideTooltip` honor reads to find which
        // node to show/hide. Idempotent over the scene-fn path.
        app.add_systems(Update, tooltip::wire_tooltip_described_by);
        // The slider C4 visual (slice-2) reads `Changed<A11yValue>` to reposition
        // the thumb. A slider's value is mutated by the slider contract's `honor`
        // (driven by the APG `slider_keyboard` system / an inbound AT verb, both in
        // `buiy_core`'s `BuiySet::Input`), NOT through the `OnPress` toggle sink —
        // so this visual does not chain after `advance_toggle_on_press`; it runs in
        // `Update` and settles on the `Changed<A11yValue>` gate.
        app.add_systems(
            Update,
            // Track C / F2: emit `ValueChange<f64>` from the same committed
            // `Changed<A11yValue>` the visual reads.
            (slider::update_slider_visual, emit_slider_value_change),
        );

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

        // C5-b (scroll-overlay-modal.md §B) — overlay positioning + light-dismiss.
        //
        // `position_popover` lowers each `Popover` onto its required `Anchor`
        // (the placement candidates → an `Anchor.position_try` flip chain). It
        // runs `.before(BuiySet::Layout)` and mutates the `Anchor` IN PLACE, so
        // the same-frame `anchor_resolution` (inside `BuiySet::Layout`) positions
        // the popover with no command-sync frame lag.
        app.add_systems(Update, popover::position_popover.before(BuiySet::Layout));
        // Wire each tooltip node's placement (anchor to its trigger parent +
        // top-layer `Tooltip` stacking + `LightDismiss`) once it gains its parent
        // link (§B.4 — the placement the P1d tooltip slice deferred to C5). The
        // `Anchor` it inserts is consumed by the same-frame `anchor_resolution`.
        app.add_systems(Update, tooltip::position_tooltip);
        // Escape closes the top-most open light-dismiss overlay (§B.5, keyboard
        // channel). Runs in `BuiySet::Input` alongside the other keyboard handlers.
        app.add_systems(Update, dismiss::escape_dismiss.in_set(BuiySet::Input));
        // The pointer light-dismiss observer (§B.5, pointer channel): a primary
        // `Pointer<Press>` outside the top-most open overlay closes it. An
        // observer (not a system) so it rides the C3 `Pointer<E>` capture→bubble
        // layer with the picking-resolved target.
        app.add_observer(dismiss::light_dismiss_on_press);

        // W7 (MVU-as-core, FINAL) — the generic `DismissRegistry` (spec §9, D9) un-inverts
        // the W6a `With<MenuModel>` dismiss stopgap: `dismiss.rs` is now model-agnostic and
        // CONSULTS this widgets-populated registry, mirroring the W6b `InlineActionRegistry`.
        // Register the menu's close-hook — an open `Menu` that light-dismisses / Escapes
        // enqueues `MenuMsg::Close(reason)` through the funnel (the single ordered drain is
        // the sole writer; the early bind projects `open=false` same-frame). A raw overlay
        // (tooltip / popover) registers nothing and keeps the direct `CssVisibility::Hidden`
        // write. Entity-free, never recorded — NOT a per-entity boxed-closure component.
        app.init_resource::<dismiss::DismissRegistry>();
        app.world_mut()
            .resource_mut::<dismiss::DismissRegistry>()
            .register(Box::new(menu::menu_dismiss_hook));

        // C5-c (scroll-overlay-modal.md §B.3) — Menu / MenuButton / MenuItem.
        //
        // Wire each MenuButton↔Menu pair's two-way edges (`controls = [menu]` on
        // the button, `Popover.anchor = button` on the menu) once the button's
        // `children!` exist — the menu child entity is unknown at root-spawn time
        // (the disclosure `wire_disclosure_controls` precedent).
        app.add_systems(Update, menu::wire_menu_button);
        // Attach the click-containment observers to each new Menu so a press/click
        // inside the menu does not bubble up the `ChildOf` chain to the controlling
        // button (which would toggle the menu closed + steal focus). Gated on
        // `Added<Menu>`.
        app.add_systems(Update, menu::guard_menu_clicks);
        // The per-item pointer producer: a primary `Pointer<Click>` on a `MenuItem`
        // writes the shared `OnPress` sink for it + closes the menu (the pointer
        // mirror of the keyboard Enter/Space activate-and-close). `MenuItem` is not
        // in `is_activatable_role`, so the generic `pointer_click_emits_on_press`
        // does not cover it; this dedicated observer does. It fires at the item
        // during the `Pointer<Click>` bubble, before `guard_menu_clicks` stops it at
        // the menu root.
        app.add_observer(menu::menu_item_click_emits_on_press);
        //
        // W6a (MVU-as-core, FINAL) — the MACHINE tier replaces the old two-system
        // open/close lifecycle (`sync_menu_open` button→menu + `sync_menu_dismissed`
        // menu→button reconciliation, both DELETED) with the `MenuModel` funnel:
        //
        //   press / AT-Click → `OnPress` → `route_menu_press` (enqueue `MenuMsg`)
        //   light-dismiss / Escape / keyboard nav / item click → enqueue `MenuMsg`
        //                                   ↓  (the single ordered drain folds)
        //                            `MenuModel` (sole writer = the drain)
        //                                   ↓  `bind_menu_model` projects
        //          CssVisibility + active_descendant + button A11yExpanded + focus
        //
        // THE D4 CORRECTION (spec §4 — the load-bearing fix the prototype did NOT
        // make). The a11y tree reads the button's PROJECTED `A11yExpanded`, written by
        // `bind_menu_model`. The prototype pinned the bind in the LATE `MvuSet::Bind`
        // (`.after(A11yUpdate)`), so a keyboard/pointer-driven open lagged
        // `aria-expanded` by one frame — a REGRESSION vs the base (which writes
        // `A11yExpanded` at `BuiySet::Input`, same-frame-correct). The fix: pin the
        // machine's WHOLE chain — `Enqueue → ApplyDeferred → Drain → Bind` — into an
        // EARLY window `.after(BuiySet::Picking).before(BuiySet::A11yUpdate)`, so the
        // drain AND the bind run before `build_tree`. Legal: nothing in `A11yUpdate`
        // writes `MenuModel`, and reducers are env-free, so there is no cycle.
        //
        // Register the model (Reflect Component + the nested `DismissReason` enum the
        // record log serializes) + its inbox/Msg-reflect/bind-counter/replay-applier
        // (`add_model`), but install the drain in the EARLY `MenuSet::Drain` via
        // `add_reducer_in_set` (NOT the late `mvu_model`/`MvuSet::Drain` default).
        app.register_type::<menu::MenuModel>();
        app.register_type::<menu::DismissReason>();
        app.add_model::<menu::MenuModel>();

        // W6b (MVU-as-core, the AT seam) — register the menu's INLINE AT set-verb hook
        // into the core `InlineActionRegistry` (spec §5.4). This is the POPULATE side of
        // the registry core consults in its generic `Expand`/`Collapse` honor: an AT
        // `Expand`/`Collapse` on a `MenuButton` does the cross-entity hop (button →
        // `controls[0]` → menu) and folds the ABSOLUTE `MenuMsg::Open`/`Close` inline
        // through `menu_reducer` (live-component-synchronous), closing the W5/W6a
        // "advertised but inert" gap. `init_resource` is idempotent (A11yPlugin also inits
        // it); the menu's `Click` activation stays ASYNC via `route_menu_press`
        // (§5.5 Click/Expand reconciliation) and is deliberately NOT routed inline.
        app.init_resource::<InlineActionRegistry>();
        app.world_mut()
            .resource_mut::<InlineActionRegistry>()
            .register(Box::new(menu::menu_inline_action_hook));

        // The early window: `Enqueue → Drain → Bind`, chained
        // `.after(BuiySet::Picking).before(BuiySet::A11yUpdate)` (mirrors
        // `ToggleLeafSet`, plus a `Bind` stage for the projection).
        app.configure_sets(
            Update,
            (
                menu::MenuSet::Enqueue,
                menu::MenuSet::Drain,
                menu::MenuSet::Bind,
            )
                .chain()
                .after(BuiySet::Picking)
                .before(BuiySet::A11yUpdate),
        );
        // Flush the `commands`-deferred enqueues (incl. an `escape_dismiss` /
        // light-dismiss-observer `enqueue::<MenuModel>`, queued no later than
        // `BuiySet::Picking`) into `Messages<Envelope<MenuModel>>` BEFORE the early
        // drain reads the inbox, so they fold the SAME frame (spec §4.3).
        app.add_systems(
            Update,
            ApplyDeferred
                .after(menu::MenuSet::Enqueue)
                .before(menu::MenuSet::Drain),
        );
        // The early ordered drain — `menu_reducer` is the SOLE writer of `MenuModel`.
        app.add_reducer_in_set::<menu::MenuModel, _>(menu::menu_reducer, menu::MenuSet::Drain);

        // The enqueue producers — every menu enqueue producer lives in (or before) the
        // early window or its Msg folds one frame late (spec §4.3). `route_menu_press`
        // (OnPress → Toggle) and `menu_keyboard_nav` (Arrow/Home/End/Enter/Escape →
        // Highlight/Close) both enqueue in `MenuSet::Enqueue`. (The dismiss→Close
        // producers — `escape_dismiss` in `BuiySet::Input`, the `light_dismiss_on_press`
        // observer at `Picking` — enqueue BEFORE this window via the model-agnostic
        // `DismissRegistry` (W7); the pinned `ApplyDeferred` flushes their deferred enqueue
        // before the drain, so the close folds same-frame, §4.3/§4.4.)
        app.add_systems(
            Update,
            menu::route_menu_press.in_set(menu::MenuSet::Enqueue),
        );
        app.add_systems(
            Update,
            menu::menu_keyboard_nav.in_set(menu::MenuSet::Enqueue),
        );
        // The projection bind (the EARLY `MenuSet::Bind`, the D4 correction). It also
        // paints the roving-active item ring (audit N2) inline — folded in rather than
        // a sibling system, because adding ANY system to `Update` perturbs the
        // executor's ordering enough to flip a schedule-fragile hidden-node layout
        // under the MT executor (the tooltip-primitive snapshot fragility).
        app.add_systems(Update, menu::bind_menu_model.in_set(menu::MenuSet::Bind));
    }
}
