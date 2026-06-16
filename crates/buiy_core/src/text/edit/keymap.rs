//! The data-driven editing keymap (editing-and-ime §§ 3.1, 3.2). One table
//! per platform, selected ONCE at plugin init by a data swap
//! (`default_keymap_for_platform`) — never scattered `cfg` in the hot
//! system. The table is keyed on `(Modifiers, KeyKind)`; `KeyKind` is a
//! small owned discriminant the input system derives from a live
//! `bevy_input::Key`, so the table stays platform-data and the lookup key is
//! `Hash`-clean (a `Key::Character(SmolStr)` payload never enters a map key).
//!
//! This file names NO cosmic `Editor`/`Edit`/`Action`/`Change` type — only
//! the pure-data `Motion` (via `EditCommand`). It is inside the facade
//! regardless (the editing module), but it would pass the boundary scan even
//! outside it.

use std::collections::HashMap;

use bevy::prelude::Resource;

use super::command::EditCommand;

/// The active editing keymap, selected per platform at plugin init
/// (§ 3.2). A `Resource` so the input system reads it; the later
/// user-rebinding hook (§ 3.2, v1 ships fixed tables) replaces this
/// resource's table.
#[derive(Resource)]
pub struct Keymap(pub KeymapTable);

impl Default for Keymap {
    fn default() -> Self {
        Self(default_keymap_for_platform())
    }
}

/// The keyboard modifier state at the moment a key was pressed. `cmd` is the
/// macOS Command key (`Super`); `ctrl`/`alt`/`shift` are uniform. The
/// per-platform tables key the doc/word/clipboard verbs on the platform's
/// modifier (Ctrl on Linux/Windows, Cmd on macOS), so a Linux table never
/// has a `cmd` row and a macOS table never has a `ctrl` clipboard row.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub cmd: bool,
}

impl Modifiers {
    pub const NONE: Modifiers = Modifiers {
        ctrl: false,
        alt: false,
        shift: false,
        cmd: false,
    };
    pub fn ctrl() -> Self {
        Modifiers {
            ctrl: true,
            ..Self::NONE
        }
    }
    pub fn shift() -> Self {
        Modifiers {
            shift: true,
            ..Self::NONE
        }
    }
    pub fn cmd() -> Self {
        Modifiers {
            cmd: true,
            ..Self::NONE
        }
    }
    pub fn alt() -> Self {
        Modifiers {
            alt: true,
            ..Self::NONE
        }
    }
}

/// A platform-neutral key discriminant — the subset of logical keys the
/// editor binds. The input system maps a live `bevy_input::Key` to one of
/// these (and `Char` carries the inserted text for the `Insert` fall-through,
/// handled in the system, not the table). `Char` is intentionally NOT a
/// table key — character insertion is the default when no command matches.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyKind {
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    Home,
    End,
    PageUp,
    PageDown,
    Backspace,
    Delete,
    Enter,
    Escape,
    /// A printable-character key — never a table row; the system inserts the
    /// event `text` when `resolve` returns `None` for it.
    Char,
}

/// The `(Modifiers, KeyKind) → EditCommand` lookup table (§ 3.1).
#[derive(Default)]
pub struct KeymapTable {
    rows: HashMap<(Modifiers, KeyKind), EditCommand>,
}

impl KeymapTable {
    /// Bind a row. Later inserts overwrite — the platform builders insert
    /// once each, so order is irrelevant in v1.
    pub fn insert(&mut self, mods: Modifiers, key: KeyKind, command: EditCommand) {
        self.rows.insert((mods, key), command);
    }

    /// Resolve a key event to its command, if bound. Returns a clone (the
    /// table outlives the lookup; `EditCommand` is cheap to clone — only
    /// `Insert` allocates, and `Insert` is never a table row).
    pub fn resolve(&self, mods: Modifiers, key: KeyKind) -> Option<EditCommand> {
        self.rows.get(&(mods, key)).cloned()
    }
}

/// Select the platform table at init (§ 3.2 — a data swap, evaluated ONCE
/// here, never per-event `cfg`). `cfg!` is a const, so the non-target arms
/// compile out.
pub fn default_keymap_for_platform() -> KeymapTable {
    if cfg!(target_os = "macos") {
        macos_keymap()
    } else {
        linux_windows_keymap()
    }
}

// The per-platform tables. `pub` (not `pub(crate)`) for the same reason
// `KeyKind`, the `keymap` submodule, and `default_keymap_for_platform` are
// `pub`: Task 3's table tests live in the `tests/text_keymap.rs` integration
// crate, which only sees the public surface — it names these builders directly to
// assert the per-platform rows (the host-only `default_keymap_for_platform` cannot
// stand in for the non-host table).

/// The Linux/Windows table (§§ 3.1, 3.2): Ctrl is the doc/word/clipboard
/// modifier. Motion rows are emitted for both the plain and Shift (extend)
/// modifier, so a single `motion` helper writes the pair.
pub fn linux_windows_keymap() -> KeymapTable {
    use KeyKind::*;
    let mut t = KeymapTable::default();

    // Plain + Shift(extend) for every arrow/Home/End/Page motion.
    let mut motion = |mods: Modifiers, key: KeyKind, m: cosmic_text::Motion| {
        t.insert(mods, key, EditCommand::Motion(m, false));
        t.insert(
            Modifiers {
                shift: true,
                ..mods
            },
            key,
            EditCommand::Motion(m, true),
        );
    };
    motion(Modifiers::NONE, ArrowLeft, cosmic_text::Motion::Left);
    motion(Modifiers::NONE, ArrowRight, cosmic_text::Motion::Right);
    motion(Modifiers::NONE, ArrowUp, cosmic_text::Motion::Up);
    motion(Modifiers::NONE, ArrowDown, cosmic_text::Motion::Down);
    motion(Modifiers::NONE, Home, cosmic_text::Motion::Home);
    motion(Modifiers::NONE, End, cosmic_text::Motion::End);
    motion(Modifiers::NONE, PageUp, cosmic_text::Motion::PageUp);
    motion(Modifiers::NONE, PageDown, cosmic_text::Motion::PageDown);
    // Ctrl+arrow ⇒ visual word-nav; Ctrl+Home/End ⇒ doc start/end.
    motion(Modifiers::ctrl(), ArrowLeft, cosmic_text::Motion::LeftWord);
    motion(
        Modifiers::ctrl(),
        ArrowRight,
        cosmic_text::Motion::RightWord,
    );
    motion(Modifiers::ctrl(), Home, cosmic_text::Motion::BufferStart);
    motion(Modifiers::ctrl(), End, cosmic_text::Motion::BufferEnd);

    // Discrete keys (no extend pairing).
    t.insert(Modifiers::NONE, Backspace, EditCommand::Backspace);
    t.insert(Modifiers::NONE, Delete, EditCommand::Delete);
    t.insert(Modifiers::NONE, Enter, EditCommand::Enter);
    t.insert(Modifiers::NONE, Escape, EditCommand::Escape);
    t
}

/// The macOS table (§ 3.2): Cmd is the doc/clipboard/undo/select-all
/// modifier; Option is word-nav; Cmd+Left/Right are line-ends; Cmd+Up/Down
/// are buffer start/end.
pub fn macos_keymap() -> KeymapTable {
    use KeyKind::*;
    let mut t = KeymapTable::default();

    let mut motion = |mods: Modifiers, key: KeyKind, m: cosmic_text::Motion| {
        t.insert(mods, key, EditCommand::Motion(m, false));
        t.insert(
            Modifiers {
                shift: true,
                ..mods
            },
            key,
            EditCommand::Motion(m, true),
        );
    };
    // Plain arrows.
    motion(Modifiers::NONE, ArrowLeft, cosmic_text::Motion::Left);
    motion(Modifiers::NONE, ArrowRight, cosmic_text::Motion::Right);
    motion(Modifiers::NONE, ArrowUp, cosmic_text::Motion::Up);
    motion(Modifiers::NONE, ArrowDown, cosmic_text::Motion::Down);
    motion(Modifiers::NONE, Home, cosmic_text::Motion::Home);
    motion(Modifiers::NONE, End, cosmic_text::Motion::End);
    motion(Modifiers::NONE, PageUp, cosmic_text::Motion::PageUp);
    motion(Modifiers::NONE, PageDown, cosmic_text::Motion::PageDown);
    // Option(Alt)+Left/Right ⇒ word-nav.
    motion(Modifiers::alt(), ArrowLeft, cosmic_text::Motion::LeftWord);
    motion(Modifiers::alt(), ArrowRight, cosmic_text::Motion::RightWord);
    // Cmd+Left/Right ⇒ line-ends; Cmd+Up/Down ⇒ buffer ends.
    motion(Modifiers::cmd(), ArrowLeft, cosmic_text::Motion::Home);
    motion(Modifiers::cmd(), ArrowRight, cosmic_text::Motion::End);
    motion(Modifiers::cmd(), ArrowUp, cosmic_text::Motion::BufferStart);
    motion(Modifiers::cmd(), ArrowDown, cosmic_text::Motion::BufferEnd);

    t.insert(Modifiers::NONE, Backspace, EditCommand::Backspace);
    t.insert(Modifiers::NONE, Delete, EditCommand::Delete);
    t.insert(Modifiers::NONE, Enter, EditCommand::Enter);
    t.insert(Modifiers::NONE, Escape, EditCommand::Escape);
    t
}
