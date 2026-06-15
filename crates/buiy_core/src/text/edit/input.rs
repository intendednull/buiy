//! E2 input application: the `EditCommand → cosmic Action` lowering
//! (`TextEditState::apply`), the focus-gated `apply_keyboard_edits` system,
//! and the `TextChanged` Message (editing-and-ime §§ 3, 3.3, 11). This file
//! NAMES `Action` (the lowering), so it MUST stay inside the facade — the
//! boundary tripwire (`tests/text_facade_boundary.rs`) enforces it.

use bevy::prelude::*;
use cosmic_text::{Action, Edit, FontSystem, Selection};

use super::command::EditCommand;
use super::state::TextEditState;

/// Emitted when an editor's logical value changes (editing-and-ime § 11 row
/// `TextChanged`). Never emitted for caret motion or for preedit (E5). The
/// value is read from the component (`TextEditState::value`), so the payload
/// is just the entity — the § 11 contract.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextChanged(pub Entity);

/// What one `apply` did, so the system can emit the right Messages. `value`
/// changes drive `TextChanged`; `submitted` drives the internal Submit path
/// (E6 turns it into the host-facing `EditSubmitted`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EditOutcome {
    pub value_changed: bool,
    pub submitted: bool,
}

impl TextEditState {
    /// Lower one `EditCommand` to cosmic `Action`s and apply it to the
    /// editor (editing-and-ime § 3). This is the ONE place `EditCommand`
    /// meets `Action`.
    ///
    /// - `single_line` (the `SingleLine` marker): `Enter ⇒ Submit`; inserted
    ///   text is newline-stripped (§ 3.3).
    /// - `read_only` (the `ReadOnly` marker): mutation is refused; motion /
    ///   selection / escape still apply (§ 2.2).
    ///
    /// Returns an [`EditOutcome`] describing what the caller must signal.
    pub fn apply(
        &mut self,
        font_system: &mut FontSystem,
        command: EditCommand,
        single_line: bool,
        read_only: bool,
    ) -> EditOutcome {
        // Value-changed is measured by comparing the buffer text across the
        // mutation (cheap: only mutating commands take this branch, and the
        // buffer is small for F-tier inputs). Motion/Escape never change it,
        // so they skip the snapshot.
        match command {
            // ── Mutations (gated on !read_only) ──────────────────────────
            EditCommand::Insert(text) => {
                if read_only {
                    return EditOutcome::default();
                }
                let before = self.value();
                // Single-line strips newlines (§ 3.3); the editor's
                // Action::Insert self-routes '\n' to Enter, so stripping at
                // the source is the policy.
                for ch in text.chars() {
                    if single_line && (ch == '\n' || ch == '\r') {
                        continue;
                    }
                    self.editor.action(font_system, Action::Insert(ch));
                }
                EditOutcome {
                    value_changed: self.value() != before,
                    submitted: false,
                }
            }
            EditCommand::Backspace => {
                if read_only {
                    return EditOutcome::default();
                }
                let before = self.value();
                self.editor.action(font_system, Action::Backspace);
                EditOutcome {
                    value_changed: self.value() != before,
                    submitted: false,
                }
            }
            EditCommand::Delete => {
                if read_only {
                    return EditOutcome::default();
                }
                let before = self.value();
                self.editor.action(font_system, Action::Delete);
                EditOutcome {
                    value_changed: self.value() != before,
                    submitted: false,
                }
            }
            EditCommand::Enter => {
                if single_line {
                    // § 3.3: single-line Enter submits, never inserts a
                    // newline. No mutation, so read_only is irrelevant.
                    return EditOutcome {
                        value_changed: false,
                        submitted: true,
                    };
                }
                if read_only {
                    return EditOutcome::default();
                }
                let before = self.value();
                self.editor.action(font_system, Action::Enter);
                EditOutcome {
                    value_changed: self.value() != before,
                    submitted: false,
                }
            }

            // ── Motion (allowed under read_only) ─────────────────────────
            EditCommand::Motion(motion, extend) => {
                // Action::Motion does NOT manage the selection anchor
                // (editor.rs:520-528 — the motion arm only updates the
                // cursor). Extend ⇒ ensure an anchor at the current caret
                // before moving; non-extend ⇒ collapse the selection first.
                // The editor moves in VISUAL order (§ 4.1) — we never compute
                // BiDi.
                if extend {
                    if self.editor.selection() == Selection::None {
                        let anchor = self.editor.cursor();
                        self.editor.set_selection(Selection::Normal(anchor));
                    }
                } else {
                    self.editor.set_selection(Selection::None);
                }
                self.editor.action(font_system, Action::Motion(motion));
                EditOutcome::default()
            }

            EditCommand::SelectAll => {
                // Anchor at buffer start, active at buffer end — a single
                // Normal selection spanning everything. Motion-only (no
                // value change), allowed under read_only.
                self.editor.set_selection(Selection::None);
                self.editor.action(
                    font_system,
                    Action::Motion(cosmic_text::Motion::BufferStart),
                );
                let start = self.editor.cursor();
                self.editor.set_selection(Selection::Normal(start));
                self.editor
                    .action(font_system, Action::Motion(cosmic_text::Motion::BufferEnd));
                EditOutcome::default()
            }

            EditCommand::Escape => {
                self.editor.action(font_system, Action::Escape);
                EditOutcome::default()
            }

            // ── Submit (internal) ────────────────────────────────────────
            EditCommand::Submit => EditOutcome {
                value_changed: false,
                submitted: true,
            },

            // ── E4 verbs: recognized, no behavior yet ────────────────────
            // Clipboard (§ 7) and undo (§ 8) land in E4. They must NOT fall
            // through to text insertion; routing them to a no-op here keeps
            // the keymap rows valid from E2 without faking behavior.
            EditCommand::Cut
            | EditCommand::Copy
            | EditCommand::Paste
            | EditCommand::Undo
            | EditCommand::Redo => {
                // TODO(E4): clipboard + undo engine.
                EditOutcome::default()
            }
        }
    }
}

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonInput, ButtonState};

use super::keymap::{KeyKind, Keymap, Modifiers};
use super::state::{Disabled, ReadOnly, SingleLine};
use crate::FocusedEntity;
use crate::layout::LayoutTree;
use crate::text::SharedFontSystem;

/// Map a live logical `Key` to the table's `KeyKind`, plus the inserted text
/// for a character key. Returns `None` for keys the editor does not bind
/// (modifiers, function keys) — those are dropped.
fn classify(key: &Key) -> Option<(KeyKind, Option<String>)> {
    Some(match key {
        Key::ArrowLeft => (KeyKind::ArrowLeft, None),
        Key::ArrowRight => (KeyKind::ArrowRight, None),
        Key::ArrowUp => (KeyKind::ArrowUp, None),
        Key::ArrowDown => (KeyKind::ArrowDown, None),
        Key::Home => (KeyKind::Home, None),
        Key::End => (KeyKind::End, None),
        Key::PageUp => (KeyKind::PageUp, None),
        Key::PageDown => (KeyKind::PageDown, None),
        Key::Backspace => (KeyKind::Backspace, None),
        Key::Delete => (KeyKind::Delete, None),
        Key::Enter => (KeyKind::Enter, None),
        Key::Escape => (KeyKind::Escape, None),
        Key::Space => (KeyKind::Char, Some(String::from(" "))),
        Key::Character(s) => (KeyKind::Char, Some(s.to_string())),
        _ => return None,
    })
}

/// Read modifier state from the physical-key input (logical `Key` modifiers
/// are side-agnostic, but physical codes give the same booleans and are
/// already maintained). `cmd` is `Super`.
fn read_modifiers(keys: &ButtonInput<KeyCode>) -> Modifiers {
    Modifiers {
        ctrl: keys.pressed(KeyCode::ControlLeft) || keys.pressed(KeyCode::ControlRight),
        alt: keys.pressed(KeyCode::AltLeft) || keys.pressed(KeyCode::AltRight),
        shift: keys.pressed(KeyCode::ShiftLeft) || keys.pressed(KeyCode::ShiftRight),
        cmd: keys.pressed(KeyCode::SuperLeft) || keys.pressed(KeyCode::SuperRight),
    }
}

/// The platform's command modifier (Ctrl on Linux/Windows, Cmd on macOS) —
/// the one the letter-commands (A/X/C/V/Z/Y) key on. Evaluated as a const
/// `cfg!`, not a per-event branch.
fn command_modifier_held(mods: &Modifiers) -> bool {
    if cfg!(target_os = "macos") {
        mods.cmd
    } else {
        mods.ctrl
    }
}

/// The letter-command lookup the table cannot model (`KeyKind::Char` carries
/// no letter). Resolves Ctrl/Cmd-{A,X,C,V,Z,Y} and Ctrl/Cmd-Shift-Z (redo)
/// from the typed character (§ 3.1). Returns `None` when it is not a
/// command — the caller then inserts the character.
fn letter_command(text: &str, mods: &Modifiers) -> Option<EditCommand> {
    if !command_modifier_held(mods) {
        return None;
    }
    // Lowercase so Shift+letter (uppercase text) still matches.
    let ch = text.chars().next()?.to_ascii_lowercase();
    Some(match ch {
        'a' => EditCommand::SelectAll,
        'x' => EditCommand::Cut,
        'c' => EditCommand::Copy,
        'v' => EditCommand::Paste,
        'z' if mods.shift => EditCommand::Redo, // Ctrl/Cmd-Shift-Z
        'z' => EditCommand::Undo,
        'y' if !cfg!(target_os = "macos") => EditCommand::Redo, // Ctrl-Y (not macOS)
        _ => return None,
    })
}

/// The focus-gated editing input system (editing-and-ime §§ 3, 3.3). Runs in
/// `BuiySet::Input` (the `handle_tab` precedent), main-thread (NonSend — it
/// touches `LayoutTree`). Reads every `KeyboardInput` press for the focused,
/// non-`Disabled` editor, resolves it to an `EditCommand`, applies it, and on
/// a value-changing edit (M1): emits `TextChanged`, invalidates the editor's
/// intrinsics cache, and Taffy-dirties the node so the **existing** measure →
/// commit → extract path republishes next frame (`sync_one`'s pair —
/// `sync.rs:330,335`). The `SharedFontSystem` lock is held ONLY when a
/// command is actually applied (collect-then-lock keeps a no-key frame
/// lock-free — architecture § 1.2 / E2 erratum 2).
///
/// `focused` is `Option<Res<FocusedEntity>>` (M2): `FocusedEntity` is init by
/// `FocusPlugin`, NOT `CorePlugin`, so a `BuiyTextPlugin` harness without
/// `FocusPlugin` has no such resource — the system no-ops rather than
/// panicking at param validation. `tree` is `Option<NonSendMut<LayoutTree>>`
/// (init by `LayoutPlugin`) for the same reason, mirroring `text_sync_buffers`
/// (`sync.rs:147`).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn apply_keyboard_edits(
    events: Option<MessageReader<KeyboardInput>>,
    focused: Option<Res<FocusedEntity>>,
    keymap: Res<Keymap>,
    keys: Option<Res<ButtonInput<KeyCode>>>,
    fonts: Res<SharedFontSystem>,
    mut tree: Option<NonSendMut<LayoutTree>>,
    mut editors: Query<(&mut TextEditState, Has<SingleLine>, Has<ReadOnly>), Without<Disabled>>,
    mut changed: MessageWriter<TextChanged>,
) {
    // No input infrastructure (no `InputPlugin` and no manual seed) ⇒ the
    // `KeyboardInput` Message and the `ButtonInput<KeyCode>` resource are
    // absent. Like `focused`/`tree` (M2), these are `Option` so a bare
    // `BuiyTextPlugin` harness (no input registered) runs the system inertly
    // instead of panicking at param validation. There is nothing to read and
    // nothing to drain — return.
    let (Some(mut events), Some(keys)) = (events, keys) else {
        return;
    };
    // No focus infrastructure (no FocusPlugin) ⇒ nothing to edit. Drain the
    // queue so events don't accumulate across frames.
    let Some(focused) = focused else {
        events.clear();
        return;
    };
    let Some(entity) = focused.0 else {
        events.clear(); // nothing focused this frame
        return;
    };
    let Ok((mut state, single_line, read_only)) = editors.get_mut(entity) else {
        events.clear(); // focus is on a non-editor / Disabled entity
        return;
    };

    let mods = read_modifiers(&keys);
    // Collect this frame's commands BEFORE locking, so the lock is held for
    // the apply burst only (and not at all when nothing resolves).
    let mut commands: Vec<EditCommand> = Vec::new();
    for ev in events.read() {
        if ev.state != ButtonState::Pressed {
            continue; // key-up never edits
        }
        let Some((kind, text)) = classify(&ev.logical_key) else {
            continue; // an unbound logical key (function keys, bare modifiers)
        };
        // Honor key-repeat for motions/deletes (§ 3); a repeated character
        // also re-inserts (the OS sends repeats with `repeat = true` and the
        // resolved text, so no special-casing is needed — we simply process
        // every Pressed event, repeats included).
        let _ = ev.repeat;

        let command = match kind {
            KeyKind::Char => {
                let text = text.unwrap_or_default();
                // A command modifier turns a letter into a verb; otherwise
                // it is literal insertion.
                letter_command(&text, &mods).unwrap_or(EditCommand::Insert(text))
            }
            // A positional key with no row for THIS modifier set is dropped
            // (m4 — drop unmapped keys, do not invent a fallback command). An
            // unbound combo like Ctrl+Backspace simply does nothing in v1;
            // the user-rebinding hook (§ 3.2) is where such bindings are added.
            other => {
                let Some(command) = keymap.0.resolve(mods, other) else {
                    continue;
                };
                command
            }
        };
        commands.push(command);
    }

    if commands.is_empty() {
        return; // NO lock on an idle / non-editing frame.
    }

    // The one lock hold — the apply burst (E2 erratum 2). Mirrors TextCommit.
    let mut font_system = fonts.lock();
    let mut any_value_change = false;
    for command in commands {
        let outcome = state.apply(&mut font_system, command, single_line, read_only);
        any_value_change |= outcome.value_changed;
        // `outcome.submitted` is consumed internally in E2 (the host-facing
        // EditSubmitted is E6). A submit does not change the value.
    }
    drop(font_system);

    // M1 — the dirty-mark seam. A value-changing edit reshaped the editor's
    // OWNED buffer, but the `Text` component is unchanged, so NONE of
    // TextSyncTriggers fire and `sync_one` (the only `mark_dirty_for_entity`
    // caller) never runs for this entity. Do here exactly what `sync_one`
    // does after a Text change: drop the intrinsics cache and Taffy-dirty the
    // node, so next frame's measure → TextCommit → extract republish (N→N+1).
    // Pure motion (any_value_change == false) reshapes nothing — no dirty.
    if any_value_change {
        state.invalidate_intrinsics();
        if let Some(tree) = tree.as_deref_mut() {
            // Absent tree (standalone BuiyTextPlugin, no LayoutPlugin):
            // nothing measures, nothing to dirty — `sync_one`'s same guard.
            tree.mark_dirty_for_entity(entity);
        }
        changed.write(TextChanged(entity));
    }
}
