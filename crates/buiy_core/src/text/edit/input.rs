//! E2 input application: the `EditCommand → cosmic Action` lowering
//! (`TextEditState::apply`), the focus-gated `apply_keyboard_edits` system,
//! and the `TextChanged` Message (editing-and-ime §§ 3, 3.3, 11). This file
//! NAMES `Action` (the lowering), so it MUST stay inside the facade — the
//! boundary tripwire (`tests/text_facade_boundary.rs`) enforces it.

use std::time::Duration;

use bevy::prelude::*;
use cosmic_text::{Action, Cursor, Edit, FontSystem, Selection};

use super::clipboard::{ClipboardProvider, MemClipboard};
use super::command::EditCommand;
use super::state::TextEditState;
use super::undo::{GroupKind, UndoUnit};

/// Emitted when an editor's logical value changes (editing-and-ime § 11 row
/// `TextChanged`). Never emitted for caret motion or for preedit (E5). The
/// value is read from the component (`TextEditState::value`), so the payload
/// is just the entity — the § 11 contract.
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextChanged(pub Entity);

/// Emitted when a single-line editor is submitted (editing-and-ime § 11 row
/// `EditSubmitted`, § 3.3). Born from `EditCommand::Submit` — the focused
/// single-line Enter. Payload: the entity (the value is read via the
/// component, per the § 11 contract). This FINALIZES the § 11 taxonomy
/// (the host-facing surface of E2's internal `EditOutcome.submitted` flag).
#[derive(Message, Debug, Clone, Copy, PartialEq, Eq)]
pub struct EditSubmitted(pub Entity);

/// What one `apply` did, so the system can emit the right Messages. `value`
/// changes drive `TextChanged`; `submitted` drives the internal Submit path
/// (E6 turns it into the host-facing `EditSubmitted`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct EditOutcome {
    pub value_changed: bool,
    pub submitted: bool,
    /// The buffer content changed even if the logical value did not (an
    /// Escape that cleared a preedit) — needs the M1 re-measure dirty-mark
    /// but no `TextChanged` (editing-and-ime § 6.2: preedit is not value).
    pub reshaped: bool,
}

/// Everything `apply_tracked` needs beyond the command itself. Carries the
/// policy flags (E2's `single_line`/`read_only`), the virtual-clock instant
/// for undo coalescing (deterministic in tests), and the clipboard provider
/// for cut/copy/paste. A struct (not 4 positional args) keeps the call sites
/// readable and leaves room for E5 to add IME context without churning every
/// caller.
pub struct EditContext<'a> {
    pub single_line: bool,
    pub read_only: bool,
    pub now: Duration,
    pub clipboard: &'a mut dyn ClipboardProvider,
}

impl TextEditState {
    /// E2/E3-compatible apply (the 4-arg signature those test files and any
    /// non-system caller use). Delegates to `apply_tracked` with a zero clock
    /// and an EPHEMERAL in-memory clipboard — so motion/insert/select-all
    /// record-or-seal undo units harmlessly, and a stray Copy/Paste through
    /// this path touches a throwaway clipboard, never the OS. The SYSTEM
    /// (`apply_keyboard_edits`) does NOT use this shim — it builds a real
    /// `EditContext` with the shared `Time<Virtual>` clock + the `Clipboard`
    /// resource. Kept (not renamed) so E4 changes ZERO E2/E3 call sites (M1).
    pub fn apply(
        &mut self,
        font_system: &mut FontSystem,
        command: EditCommand,
        single_line: bool,
        read_only: bool,
    ) -> EditOutcome {
        let mut fallback = MemClipboard::default();
        let mut ctx = EditContext {
            single_line,
            read_only,
            now: Duration::ZERO,
            clipboard: &mut fallback,
        };
        self.apply_tracked(font_system, command, &mut ctx)
    }
}

impl TextEditState {
    /// Lower one `EditCommand` to cosmic `Action`s, apply it, and maintain the
    /// undo history (editing-and-ime §§ 3, 7, 8). This is the ONE place
    /// `EditCommand` meets `Action` AND the one place edits are recorded as
    /// undo units — so E5's IME commit reuses the same `record` seam.
    ///
    /// Mutating commands are wrapped in a `start_change`/`finish_change` pair;
    /// the resulting non-empty `Change` is recorded (grouped by `GroupKind`).
    /// Non-mutating commands (Motion/SelectAll/Escape) SEAL the open run.
    pub fn apply_tracked(
        &mut self,
        font_system: &mut FontSystem,
        command: EditCommand,
        ctx: &mut EditContext<'_>,
    ) -> EditOutcome {
        match command {
            // ── Mutations (gated on !read_only), recorded as undo units ──
            EditCommand::Insert(text) => {
                if ctx.read_only {
                    return EditOutcome::default();
                }
                // Hoist the policy flag so the closure captures a `bool`, not
                // the `&mut EditContext` (n3 — same capture style as Paste).
                let single_line = ctx.single_line;
                self.tracked_edit(font_system, GroupKind::TypingRun, ctx.now, |ed, fs| {
                    for ch in text.chars() {
                        if single_line && (ch == '\n' || ch == '\r') {
                            continue; // § 3.3 single-line newline strip
                        }
                        ed.action(fs, Action::Insert(ch));
                    }
                })
            }
            EditCommand::Backspace => {
                if ctx.read_only {
                    return EditOutcome::default();
                }
                self.tracked_edit(font_system, GroupKind::DeleteRun, ctx.now, |ed, fs| {
                    ed.action(fs, Action::Backspace);
                })
            }
            EditCommand::Delete => {
                if ctx.read_only {
                    return EditOutcome::default();
                }
                self.tracked_edit(font_system, GroupKind::DeleteRun, ctx.now, |ed, fs| {
                    ed.action(fs, Action::Delete);
                })
            }
            EditCommand::Enter => {
                if ctx.single_line {
                    return EditOutcome {
                        value_changed: false,
                        submitted: true,
                        reshaped: false,
                    };
                }
                if ctx.read_only {
                    return EditOutcome::default();
                }
                // A newline is a discrete edit (web parity: undo removes the
                // whole line break as one step, not coalesced with prior typing).
                self.undo.seal();
                self.tracked_edit(font_system, GroupKind::Discrete, ctx.now, |ed, fs| {
                    ed.action(fs, Action::Enter);
                })
            }

            // ── Motion / selection / escape: seal the run, never record ──
            EditCommand::Motion(motion, extend) => {
                self.undo.seal();
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
                self.undo.seal();
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
                self.undo.seal();
                // E5: Escape cancels an active composition (editing-and-ime
                // § 6.2d) — remove the spliced span (no Change) before the
                // editor's own Escape (which clears the selection). The buffer
                // content changed but the logical value did not (the preedit was
                // never in `value()`), so flag `reshaped` for the M1 re-measure
                // without emitting `TextChanged`.
                let cleared = self.has_preedit();
                // `remove_preedit` reverse-applies any compose-over-selection
                // delete (re-inserting the deleted text): that IS a logical
                // value change, so route it to `value_changed` → `TextChanged`.
                // The plain-preedit cancel restores nothing (returns false) and
                // only `reshaped` fires (buffer changed, value did not).
                let restored = if cleared {
                    self.remove_preedit(font_system)
                } else {
                    false
                };
                self.editor.action(font_system, Action::Escape);
                EditOutcome {
                    reshaped: cleared,
                    value_changed: restored,
                    ..Default::default()
                }
            }
            EditCommand::Submit => EditOutcome {
                value_changed: false,
                submitted: true,
                reshaped: false,
            },

            // ── Undo / Redo (§ 8) ────────────────────────────────────────
            EditCommand::Undo => self.apply_undo(),
            EditCommand::Redo => self.apply_redo(),

            // ── Clipboard (§ 7) — plain text only (decision 4) ───────────
            EditCommand::Copy => {
                // copy_selection() is None when there is no selection; a bare
                // caret Copy is a no-op (web parity).
                if let Some(text) = self.editor.copy_selection() {
                    ctx.clipboard.set_text(text);
                }
                EditOutcome::default()
            }
            EditCommand::Cut => {
                if ctx.read_only {
                    return EditOutcome::default(); // ReadOnly: copy yes, cut no
                }
                let Some(text) = self.editor.copy_selection() else {
                    return EditOutcome::default(); // nothing selected
                };
                ctx.clipboard.set_text(text);
                // Delete the selection as one DISCRETE undoable unit (a cut is
                // a deliberate single action — never coalesced with neighbors).
                self.undo.seal();
                self.tracked_edit(font_system, GroupKind::Discrete, ctx.now, |ed, fs| {
                    let _ = fs; // delete_selection needs no FontSystem
                    ed.delete_selection();
                })
            }
            EditCommand::Paste => {
                if ctx.read_only {
                    return EditOutcome::default();
                }
                let Some(text) = ctx.clipboard.get_text() else {
                    return EditOutcome::default(); // empty clipboard
                };
                // A paste is one DISCRETE unit, newline-stripped on single-line
                // (§ 3.3 — the same policy as a single-line Insert).
                self.undo.seal();
                let single_line = ctx.single_line;
                self.tracked_edit(font_system, GroupKind::Discrete, ctx.now, |ed, fs| {
                    for ch in text.chars() {
                        if single_line && (ch == '\n' || ch == '\r') {
                            continue;
                        }
                        ed.action(fs, Action::Insert(ch));
                    }
                })
            }
        }
    }

    /// Run a mutating `edit` closure wrapped in a `start_change`/`finish_change`
    /// pair and record the resulting NON-EMPTY change as an undo unit grouped
    /// by `group`. Captures caret + selection on both sides via the live
    /// `mirror_selection()` read-out (the E3 mirror-direction invariant — never
    /// the stale `selection` field). Returns whether the value changed.
    fn tracked_edit(
        &mut self,
        font_system: &mut FontSystem,
        group: GroupKind,
        now: Duration,
        edit: impl FnOnce(&mut cosmic_text::Editor<'static>, &mut FontSystem),
    ) -> EditOutcome {
        let before = self.value();
        let caret_before = self.editor.cursor();
        let selection_before = self.mirror_selection();

        self.editor.start_change();
        edit(&mut self.editor, font_system);
        let change = self.editor.finish_change().unwrap_or_default();

        let caret_after = self.editor.cursor();
        let selection_after = self.mirror_selection();

        // Empty change ⇒ no-op edit (e.g. Backspace at 0); record_grouped drops
        // it, so the undo stack stays clean and value_changed stays false.
        self.undo.record_grouped(
            UndoUnit {
                change,
                caret_before,
                caret_after,
                selection_before,
                selection_after,
                group,
            },
            now,
        );

        EditOutcome {
            value_changed: self.value() != before,
            submitted: false,
            reshaped: false,
        }
    }

    /// Undo the most recent unit: replay the reversed change, restore the
    /// `_before` caret + selection (§ 8). The reversed change deletes the
    /// inserted text / re-inserts the deleted text. No `FontSystem` needed —
    /// `apply_change` mutates buffer lines + sets redraw; reshape is next
    /// frame's `TextCommit` (the one-frame path). `value_changed` is whether
    /// the text actually moved (always true for a non-empty unit).
    fn apply_undo(&mut self) -> EditOutcome {
        let Some(unit) = self.undo.pop_undo() else {
            return EditOutcome::default();
        };
        let mut reversed = unit.change.clone();
        reversed.reverse();
        let changed = self.editor.apply_change(&reversed);
        self.restore_cursor(unit.caret_before, unit.selection_before);
        EditOutcome {
            value_changed: changed,
            submitted: false,
            reshaped: false,
        }
    }

    /// Redo the most recent undone unit: replay the change forward, restore the
    /// `_after` caret + selection (§ 8).
    fn apply_redo(&mut self) -> EditOutcome {
        let Some(unit) = self.undo.pop_redo() else {
            return EditOutcome::default();
        };
        let changed = self.editor.apply_change(&unit.change);
        self.restore_cursor(unit.caret_after, unit.selection_after);
        EditOutcome {
            value_changed: changed,
            submitted: false,
            reshaped: false,
        }
    }

    /// Restore the caret + the editor's selection after an undo/redo. The
    /// editor's `Selection` is the authoritative one E3 mirrors OUT next pass;
    /// we set both the cursor and (for a non-collapsed range) the anchor.
    pub(crate) fn restore_cursor(
        &mut self,
        caret: Cursor,
        selection: super::selection::TextSelection,
    ) {
        self.editor.set_cursor(caret);
        if selection.is_collapsed() {
            self.editor.set_selection(Selection::None);
        } else {
            self.editor
                .set_selection(Selection::Normal(selection.primary.anchor));
            self.editor.set_cursor(selection.primary.active);
        }
    }

    /// The `GroupKind` of the unit Undo would pop next (the top of the undo
    /// stack), for the `EditUndone` Message payload.
    pub(crate) fn undo_top_group(&self) -> Option<GroupKind> {
        self.undo.undo.last().map(|u| u.group)
    }

    /// The `GroupKind` Redo would pop next (top of the redo stack).
    pub(crate) fn redo_top_group(&self) -> Option<GroupKind> {
        self.undo.redo.last().map(|u| u.group)
    }
}

use bevy::input::keyboard::{Key, KeyboardInput};
use bevy::input::{ButtonInput, ButtonState};

use super::keymap::{KeyKind, Keymap, Modifiers};
use super::state::{Disabled, ReadOnly, SingleLine};
use crate::FocusedEntity;
use crate::layout::LayoutTree;
use crate::text::SharedFontSystem;

use super::clipboard::Clipboard;

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
    time: Res<Time>,
    fonts: Res<SharedFontSystem>,
    mut clipboard: Option<ResMut<Clipboard>>,
    mut tree: Option<NonSendMut<LayoutTree>>,
    mut editors: Query<(&mut TextEditState, Has<SingleLine>, Has<ReadOnly>), Without<Disabled>>,
    mut changed: MessageWriter<TextChanged>,
    mut undone: MessageWriter<super::undo::EditUndone>,
    mut redone: MessageWriter<super::undo::EditRedone>,
    mut submitted: MessageWriter<EditSubmitted>,
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

    // A clipboard is required to lower Cut/Copy/Paste; without the resource
    // (a bare BuiyTextPlugin harness that didn't insert one) fall back to an
    // ephemeral in-memory clipboard so the system still runs (the Option-param
    // inert-harness discipline). Clipboard verbs then no-op across frames,
    // which is correct for a harness with no clipboard configured.
    let mut fallback = super::clipboard::MemClipboard::default();
    let clip: &mut dyn ClipboardProvider = match clipboard.as_deref_mut() {
        Some(Clipboard(boxed)) => boxed.as_mut(),
        None => &mut fallback,
    };
    let now = time.elapsed();

    // The one lock hold — the apply burst (E2 erratum 2). Mirrors TextCommit.
    let mut font_system = fonts.lock();
    let mut any_value_change = false;
    let mut any_reshape = false;
    let mut any_submit = false;
    for command in commands {
        // Capture the group BEFORE applying, for the undo/redo Messages.
        let was_undo = command == EditCommand::Undo;
        let was_redo = command == EditCommand::Redo;
        let group_before_undo = state.undo_top_group();
        let group_before_redo = state.redo_top_group();
        let mut ctx = EditContext {
            single_line,
            read_only,
            now,
            // Reborrow each iteration: `&mut` is not `Copy`, so `clipboard: clip`
            // would MOVE it on iteration 1 and fail to compile on iteration 2
            // ("use of moved value"). `&mut *clip` reborrows for this ctx only.
            clipboard: &mut *clip,
        };
        let outcome = state.apply_tracked(&mut font_system, command, &mut ctx);
        any_value_change |= outcome.value_changed;
        any_reshape |= outcome.reshaped;
        any_submit |= outcome.submitted;
        if was_undo
            && outcome.value_changed
            && let Some(g) = group_before_undo
        {
            undone.write(super::undo::EditUndone(entity, g));
        }
        if was_redo
            && outcome.value_changed
            && let Some(g) = group_before_redo
        {
            redone.write(super::undo::EditRedone(entity, g));
        }
    }
    drop(font_system);

    // M1 — the dirty-mark seam. A value-changing edit reshaped the editor's
    // OWNED buffer, but the `Text` component is unchanged, so NONE of
    // TextSyncTriggers fire and `sync_one` (the only `mark_dirty_for_entity`
    // caller) never runs for this entity. Do here exactly what `sync_one`
    // does after a Text change: drop the intrinsics cache and Taffy-dirty the
    // node, so next frame's measure → TextCommit → extract republish (N→N+1).
    // Pure motion reshapes nothing — no dirty. An Escape that cleared a preedit
    // (`any_reshape`) changed the buffer WITHOUT changing the logical value, so
    // it dirty-marks but emits no `TextChanged` (E5, editing-and-ime § 6.2).
    if any_value_change || any_reshape {
        state.invalidate_intrinsics();
        if let Some(tree) = tree.as_deref_mut() {
            // Absent tree (standalone BuiyTextPlugin, no LayoutPlugin):
            // nothing measures, nothing to dirty — `sync_one`'s same guard.
            tree.mark_dirty_for_entity(entity);
        }
    }
    if any_value_change {
        changed.write(TextChanged(entity));
    }
    if any_submit {
        submitted.write(EditSubmitted(entity));
    }
}
