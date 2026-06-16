//! E6 — focus lifecycle (editing-and-ime § 10). One render-prep system,
//! `focus_lifecycle`, detects the `FocusedEntity` transition (gain / loss)
//! via a `Local<Option<Entity>>` previous-value compare (no transition
//! detector existed before E6) and runs the spec § 10 edges:
//!
//!   - on GAIN of a non-`Disabled` editor: the blink phase resets (the user
//!     just acted — the focused caret is solid-on for one half-period, web
//!     parity);
//!   - on LOSS of an editor: the open undo group seals (`seal()`), and any live
//!     preedit is removed (wiring E5's deferred focus-loss removal — E5's
//!     `apply_ime` only removes on `Ime::Disabled`, never on a bare focus
//!     change). The **selection / buffer is RETAINED** (we never touch
//!     `SelectionVisual` — re-focus restores it, web parity).
//!
//! **Caret visibility is NOT touched here (M1).** `write_caret_blink` is the
//! single focus-aware owner of `CaretVisual.visible` (it forces a non-focused
//! editor caret hidden and blinks the focused one). `focus_lifecycle` only
//! resets the blink ORIGIN on gain.
//!
//! `ime_enabled` is also NOT handled here: E5's `write_ime_window` already
//! decides it from focus + markers alone every frame (`ime.rs` enable_q).
//!
//! **The M1 dirty-mark.** `remove_preedit` does direct BufferLine surgery and
//! does NOT invalidate intrinsics or Taffy-dirty the node (verified `ime.rs` —
//! the removal there relies on `apply_ime`'s own dirty-mark). On a bare focus
//! change there is no `apply_ime` pass, so `focus_lifecycle` MUST do the same
//! dirty-mark itself after removing the preedit, or the orphaned preedit glyphs
//! persist a frame: `invalidate_intrinsics()` +
//! `tree.mark_dirty_for_entity(entity)` (the `apply_keyboard_edits` /
//! `apply_ime` seam).
//!
//! It names only the pure-data cosmic types via the facade accessors
//! (`remove_preedit` locks the `SharedFontSystem`; `seal` names none), so it
//! stays inside the `text::edit` facade.

use bevy::prelude::*;

use super::state::{Disabled, TextEditState};
use crate::FocusedEntity;
use crate::layout::LayoutTree;
use crate::text::SharedFontSystem;

/// Render-prep: react to focus gain / loss for editor entities (§ 10). Runs in
/// the `.after(BuiySet::Input).before(write_caret_blink)` window alongside the
/// E3 caret writer and E5 IME window writer.
///
/// `Local<Option<Entity>>` holds last frame's focused entity — the canonical
/// transition detector E6 introduces (the codebase had none). Option params
/// (`focused`, `tree`) follow the inert-harness discipline — a bare
/// `BuiyTextPlugin` without `FocusPlugin` / `LayoutPlugin` no-ops instead of
/// panicking at param validation (the `apply_ime` precedent).
#[allow(clippy::type_complexity)]
pub fn focus_lifecycle(
    time: Res<Time>,
    focused: Option<Res<FocusedEntity>>,
    fonts: Res<SharedFontSystem>,
    mut tree: Option<NonSendMut<LayoutTree>>,
    mut prev: Local<Option<Entity>>,
    mut editors: Query<&mut TextEditState, Without<Disabled>>,
) {
    let Some(focused) = focused.as_ref() else {
        return;
    };
    let now = time.elapsed();
    let current = focused.0;
    if current == *prev {
        return; // no transition — the common case
    }
    let lost = *prev;
    let gained = current;
    *prev = current;

    // --- LOSS: seal undo, remove preedit (+ M1 dirty-mark), RETAIN selection --
    if let Some(old) = lost
        && let Ok(mut state) = editors.get_mut(old)
    {
        state.seal_undo_for_lifecycle();
        if state.has_preedit() {
            {
                let mut fs = fonts.lock();
                state.remove_preedit(&mut fs);
            }
            // The M1 dirty-mark: remove_preedit changed the buffer but tripped
            // no TextSyncTrigger and did not invalidate — do it here so next
            // frame's measure → TextCommit → extract republishes (the
            // apply_ime / apply_keyboard_edits seam).
            state.invalidate_intrinsics();
            if let Some(tree) = tree.as_deref_mut() {
                tree.mark_dirty_for_entity(old);
            }
        }
    }

    // --- GAIN: reset the blink origin (caret visibility is the blink writer's) -
    if let Some(new) = gained
        && let Ok(mut state) = editors.get_mut(new)
    {
        state.blink.reset(now);
    }
}
