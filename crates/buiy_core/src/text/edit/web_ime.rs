//! Wasm-only IME + mobile soft-keyboard DOM bridge (browser-reach widening § D8).
//!
//! winit's web backend emits NO `Ime` events (winit#4424) and — because it attaches
//! `WindowEvent::KeyboardInput` to the **canvas** — a focused hidden `<input>` starves
//! the editor's keyboard entirely (W5 probe). So this bridge, when the editor is
//! focused (`Window.ime_enabled`), focuses a hidden `<input>` (which raises the mobile
//! on-screen keyboard + captures IME composition) and **fully replaces** keyboard for
//! the focused editor: it routes the input's `keydown`/`keyup` → synthesized
//! `bevy::input::keyboard::KeyboardInput` (so bevy's `ButtonInput<KeyCode>` + the editor
//! keymap see modifiers/keys) and its `compositionupdate`/`compositionend` →
//! `bevy::window::Ime` Preedit/Commit (feeding the unchanged E5 engine at `ime.rs`).
//!
//! Non-composing keys are `preventDefault`'d so the `<input>` never accumulates them
//! (bevy owns them); composing keys pass through so the browser drives composition.
//! When `ime_enabled` is false the input is blurred and winit's canvas keyboard resumes.
//! DOM errors are swallowed — the bridge must not crash the app.
//!
//! v1: cross-browser hidden-`<input>` path (EditContext is Chromium-only). Focus-position
//! tracking (`ime_position`) + a touch-only policy are named follow-ups.

use bevy::input::ButtonState;
use bevy::input::keyboard::{Key, KeyCode, KeyboardInput, NativeKey, NativeKeyCode};
use bevy::prelude::*;
use bevy::window::{Ime, PrimaryWindow, Window};
use std::cell::RefCell;
use std::collections::VecDeque;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

const INPUT_ID: &str = "buiy-ime-input";

/// A DOM event queued by a listener, drained into bevy messages on the next frame
/// (listeners run on the JS event loop, not inside a bevy system).
enum Queued {
    Key {
        key: String,
        code: String,
        pressed: bool,
        repeat: bool,
    },
    Preedit {
        value: String,
    },
    Commit {
        value: String,
    },
}

thread_local! {
    static QUEUE: RefCell<VecDeque<Queued>> = RefCell::new(VecDeque::new());
    static SETUP_DONE: RefCell<bool> = const { RefCell::new(false) };
}

/// The wasm-only IME/soft-keyboard bridge plugin (registered by `BuiyTextPlugin` on wasm).
pub struct WebImePlugin;

impl Plugin for WebImePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PreUpdate,
            (ensure_setup, sync_focus_and_drain)
                .chain()
                // Before Bevy consumes keyboard input so the synthesized events land
                // in the same frame's editor pass.
                .before(bevy::input::keyboard::keyboard_input_system),
        );
    }
}

fn document() -> Option<web_sys::Document> {
    web_sys::window()?.document()
}

fn get_input() -> Option<web_sys::HtmlInputElement> {
    document()?
        .get_element_by_id(INPUT_ID)?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()
}

/// Create the hidden `<input>` + attach its keyboard/composition listeners once.
fn ensure_setup(_main: bevy::ecs::system::NonSendMarker) {
    if SETUP_DONE.with(|d| *d.borrow()) {
        return;
    }
    if try_setup().is_some() {
        SETUP_DONE.with(|d| *d.borrow_mut() = true);
    }
}

fn try_setup() -> Option<()> {
    let document = document()?;
    let body = document.body()?;
    let input = document
        .create_element("input")
        .ok()?
        .dyn_into::<web_sys::HtmlInputElement>()
        .ok()?;
    input.set_id(INPUT_ID);
    input.set_type("text");
    // Off-screen but focusable (NOT display:none — a hidden input can't take focus /
    // raise the OSK). Disable the browser's own autocorrect/capitalize.
    let _ = input.set_attribute(
        "style",
        "position:absolute;opacity:0;left:0;top:0;width:1px;height:1px;\
         border:0;padding:0;pointer-events:none;",
    );
    let _ = input.set_attribute("autocomplete", "off");
    let _ = input.set_attribute("autocorrect", "off");
    let _ = input.set_attribute("autocapitalize", "off");
    let _ = input.set_attribute("aria-hidden", "true");
    body.append_child(&input).ok()?;

    // keydown: non-composing keys → queue a KeyboardInput(Pressed) + preventDefault so
    // the input never inserts them (bevy owns them). Composing keys pass through.
    let on_keydown =
        Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
            if e.is_composing() || e.key() == "Process" {
                return; // let composition* drive it
            }
            e.prevent_default();
            QUEUE.with(|q| {
                q.borrow_mut().push_back(Queued::Key {
                    key: e.key(),
                    code: e.code(),
                    pressed: true,
                    repeat: e.repeat(),
                });
            });
        });
    let on_keyup =
        Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
            if e.is_composing() {
                return;
            }
            QUEUE.with(|q| {
                q.borrow_mut().push_back(Queued::Key {
                    key: e.key(),
                    code: e.code(),
                    pressed: false,
                    repeat: false,
                });
            });
        });
    let on_comp_update = Closure::<dyn FnMut(web_sys::CompositionEvent)>::new(
        move |e: web_sys::CompositionEvent| {
            QUEUE.with(|q| {
                q.borrow_mut().push_back(Queued::Preedit {
                    value: e.data().unwrap_or_default(),
                });
            });
        },
    );
    let on_comp_end = Closure::<dyn FnMut(web_sys::CompositionEvent)>::new(
        move |e: web_sys::CompositionEvent| {
            let value = e.data().unwrap_or_default();
            if let Some(input) = get_input() {
                input.set_value("");
            }
            QUEUE.with(|q| {
                let mut q = q.borrow_mut();
                // A committed composition ends the preedit then inserts the text.
                q.push_back(Queued::Preedit {
                    value: String::new(),
                });
                q.push_back(Queued::Commit { value });
            });
        },
    );

    let target: &web_sys::EventTarget = input.as_ref();
    let _ = target.add_event_listener_with_callback("keydown", on_keydown.as_ref().unchecked_ref());
    let _ = target.add_event_listener_with_callback("keyup", on_keyup.as_ref().unchecked_ref());
    let _ = target.add_event_listener_with_callback(
        "compositionupdate",
        on_comp_update.as_ref().unchecked_ref(),
    );
    let _ = target
        .add_event_listener_with_callback("compositionend", on_comp_end.as_ref().unchecked_ref());
    // The listeners live for the app's lifetime — leak the closures (v1).
    on_keydown.forget();
    on_keyup.forget();
    on_comp_update.forget();
    on_comp_end.forget();
    Some(())
}

/// Focus/blur the hidden input to match `Window.ime_enabled` (raising/dismissing the
/// OSK), and drain the DOM event queue into bevy `KeyboardInput`/`Ime` messages.
fn sync_focus_and_drain(
    _main: bevy::ecs::system::NonSendMarker,
    window: Query<(Entity, &Window), With<PrimaryWindow>>,
    mut keys: MessageWriter<KeyboardInput>,
    mut ime: MessageWriter<Ime>,
    mut last_enabled: Local<Option<bool>>,
) {
    let Some((win_entity, win)) = window.iter().next() else {
        return;
    };
    let enabled = win.ime_enabled;
    if *last_enabled != Some(enabled) {
        *last_enabled = Some(enabled);
        if let Some(input) = get_input() {
            if enabled {
                let _ = input.focus();
                ime.write(Ime::Enabled { window: win_entity });
            } else {
                let _ = input.blur();
                ime.write(Ime::Disabled { window: win_entity });
            }
        }
    }

    QUEUE.with(|q| {
        for item in q.borrow_mut().drain(..) {
            match item {
                Queued::Key {
                    key,
                    code,
                    pressed,
                    repeat,
                } => {
                    // A single-character key contributes its char as composed text.
                    let text = (key.chars().count() == 1).then(|| key.as_str().into());
                    keys.write(KeyboardInput {
                        key_code: dom_code_to_keycode(&code),
                        logical_key: dom_key_to_logical(&key),
                        text,
                        state: if pressed {
                            ButtonState::Pressed
                        } else {
                            ButtonState::Released
                        },
                        repeat,
                        window: win_entity,
                    });
                }
                Queued::Preedit { value } => {
                    let cursor = (!value.is_empty()).then(|| {
                        let n = value.chars().count();
                        (n, n)
                    });
                    ime.write(Ime::Preedit {
                        window: win_entity,
                        value,
                        cursor,
                    });
                }
                Queued::Commit { value } => {
                    ime.write(Ime::Commit {
                        window: win_entity,
                        value,
                    });
                }
            }
        }
    });
}

/// DOM `KeyboardEvent.key` → bevy logical [`Key`]. Single-character keys become
/// `Key::Character`; the named keys the editor's keymap classifies are mapped
/// explicitly; everything else is `Unidentified` (harmless — unbound).
fn dom_key_to_logical(key: &str) -> Key {
    match key {
        "ArrowLeft" => Key::ArrowLeft,
        "ArrowRight" => Key::ArrowRight,
        "ArrowUp" => Key::ArrowUp,
        "ArrowDown" => Key::ArrowDown,
        "Home" => Key::Home,
        "End" => Key::End,
        "PageUp" => Key::PageUp,
        "PageDown" => Key::PageDown,
        "Backspace" => Key::Backspace,
        "Delete" => Key::Delete,
        "Enter" => Key::Enter,
        "Escape" => Key::Escape,
        "Tab" => Key::Tab,
        " " => Key::Space,
        "Shift" => Key::Shift,
        "Control" => Key::Control,
        "Alt" => Key::Alt,
        "Meta" => Key::Super,
        _ => {
            if key.chars().count() == 1 {
                Key::Character(key.into())
            } else {
                Key::Unidentified(NativeKey::Unidentified)
            }
        }
    }
}

/// DOM `KeyboardEvent.code` → bevy physical [`KeyCode`]. Mainly so `ButtonInput<KeyCode>`
/// reflects modifiers (the editor reads chords via `Modifiers` from there); the letter/
/// digit/named codes match the bevy variant names 1:1.
fn dom_code_to_keycode(code: &str) -> KeyCode {
    match code {
        "ControlLeft" => KeyCode::ControlLeft,
        "ControlRight" => KeyCode::ControlRight,
        "ShiftLeft" => KeyCode::ShiftLeft,
        "ShiftRight" => KeyCode::ShiftRight,
        "AltLeft" => KeyCode::AltLeft,
        "AltRight" => KeyCode::AltRight,
        "MetaLeft" => KeyCode::SuperLeft,
        "MetaRight" => KeyCode::SuperRight,
        "ArrowLeft" => KeyCode::ArrowLeft,
        "ArrowRight" => KeyCode::ArrowRight,
        "ArrowUp" => KeyCode::ArrowUp,
        "ArrowDown" => KeyCode::ArrowDown,
        "Enter" => KeyCode::Enter,
        "Backspace" => KeyCode::Backspace,
        "Delete" => KeyCode::Delete,
        "Space" => KeyCode::Space,
        "Tab" => KeyCode::Tab,
        "Escape" => KeyCode::Escape,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        _ => {
            letter_or_digit_code(code).unwrap_or(KeyCode::Unidentified(NativeKeyCode::Unidentified))
        }
    }
}

/// `KeyA`..`KeyZ` / `Digit0`..`Digit9` → the matching bevy `KeyCode` (names align 1:1).
fn letter_or_digit_code(code: &str) -> Option<KeyCode> {
    match code {
        "KeyA" => Some(KeyCode::KeyA),
        "KeyB" => Some(KeyCode::KeyB),
        "KeyC" => Some(KeyCode::KeyC),
        "KeyD" => Some(KeyCode::KeyD),
        "KeyE" => Some(KeyCode::KeyE),
        "KeyF" => Some(KeyCode::KeyF),
        "KeyG" => Some(KeyCode::KeyG),
        "KeyH" => Some(KeyCode::KeyH),
        "KeyI" => Some(KeyCode::KeyI),
        "KeyJ" => Some(KeyCode::KeyJ),
        "KeyK" => Some(KeyCode::KeyK),
        "KeyL" => Some(KeyCode::KeyL),
        "KeyM" => Some(KeyCode::KeyM),
        "KeyN" => Some(KeyCode::KeyN),
        "KeyO" => Some(KeyCode::KeyO),
        "KeyP" => Some(KeyCode::KeyP),
        "KeyQ" => Some(KeyCode::KeyQ),
        "KeyR" => Some(KeyCode::KeyR),
        "KeyS" => Some(KeyCode::KeyS),
        "KeyT" => Some(KeyCode::KeyT),
        "KeyU" => Some(KeyCode::KeyU),
        "KeyV" => Some(KeyCode::KeyV),
        "KeyW" => Some(KeyCode::KeyW),
        "KeyX" => Some(KeyCode::KeyX),
        "KeyY" => Some(KeyCode::KeyY),
        "KeyZ" => Some(KeyCode::KeyZ),
        "Digit0" => Some(KeyCode::Digit0),
        "Digit1" => Some(KeyCode::Digit1),
        "Digit2" => Some(KeyCode::Digit2),
        "Digit3" => Some(KeyCode::Digit3),
        "Digit4" => Some(KeyCode::Digit4),
        "Digit5" => Some(KeyCode::Digit5),
        "Digit6" => Some(KeyCode::Digit6),
        "Digit7" => Some(KeyCode::Digit7),
        "Digit8" => Some(KeyCode::Digit8),
        "Digit9" => Some(KeyCode::Digit9),
        _ => None,
    }
}
