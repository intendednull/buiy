//! E3 — mouse selection (editing-and-ime § 4, mouse gestures). The pure mapping
//! (`pointer_to_cursor`), the click-count state machine (`ClickTracker` /
//! `PointerGesture`), and the gesture→`Action` application on `TextEditState`.
//! The windowed wiring (the `Pointer<Press>` / `Pointer<Drag>` observers that
//! source the gesture and set `FocusedEntity`) is
//! [`editor_pointer_press`]/[`editor_pointer_drag`], registered as observers by
//! `BuiyTextPlugin`. This file NAMES `Action`/`Edit` (the lowering) ⇒ inside the
//! facade.

use std::time::Duration;

use bevy::math::Vec2;
use bevy::picking::events::{Drag, Pointer, Press};
use bevy::picking::pointer::PointerButton;
use bevy::prelude::*;
use cosmic_text::{Action, Buffer, Cursor, Edit, FontSystem};

use super::state::{Disabled, TextEditState};
use crate::FocusedEntity;
use crate::text::{ComputedTextLayout, SharedFontSystem};

/// The classified pointer gesture (one mouse-down).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerGesture {
    Click,
    DoubleClick,
    TripleClick,
    /// A move while the button is held (extends the selection from the anchor).
    Drag,
}

/// The multi-click window (wall-clock) and the adjacency radius (logical px).
const MULTI_CLICK_WINDOW: Duration = Duration::from_millis(450);
const MULTI_CLICK_RADIUS: f32 = 4.0;

/// Tracks consecutive clicks to classify single/double/triple (no platform API
/// gives this — cosmic only consumes the already-classified `Action`).
#[derive(Debug, Clone, Copy, Default)]
pub struct ClickTracker {
    last_pos: Vec2,
    last_time: Option<Duration>,
    streak: u8, // 0 ⇒ none; 1 single; 2 double; 3 triple
}

impl ClickTracker {
    /// Classify a press at `pos` / `now`. Increments the streak when the press
    /// is within the time window AND adjacency radius of the previous press;
    /// caps at triple, then rolls back to single.
    pub fn classify(&mut self, pos: Vec2, now: Duration) -> PointerGesture {
        let within = self
            .last_time
            .map(|t| now.saturating_sub(t) <= MULTI_CLICK_WINDOW)
            .unwrap_or(false)
            && pos.distance(self.last_pos) <= MULTI_CLICK_RADIUS;
        self.streak = if within && self.streak < 3 {
            self.streak + 1
        } else {
            1
        };
        self.last_pos = pos;
        self.last_time = Some(now);
        match self.streak {
            2 => PointerGesture::DoubleClick,
            3 => PointerGesture::TripleClick,
            _ => PointerGesture::Click,
        }
    }
}

/// Map a window-space `pointer` (logical px) to a buffer `Cursor` via the
/// buffer-local hit (`pointer − origin`, then `Buffer::hit`). `origin` is the
/// content-box top-left in window space (the caller folds
/// `GlobalTransform.translation().xy() + ComputedTextLayout.content_offset`,
/// the `extract.rs:398` term). `None` if the buffer has no run at that y.
pub fn pointer_to_cursor(buffer: &Buffer, pointer: Vec2, origin: Vec2) -> Option<Cursor> {
    let local = pointer - origin;
    buffer.hit(local.x, local.y)
}

impl TextEditState {
    /// Apply a classified pointer gesture at `pointer` (window space) to the
    /// editor via the matching cosmic `Action` (§ 4). `Click`/`Drag` use
    /// `Action::Click`/`Drag`; `Double`/`Triple` use word/line granularity.
    /// cosmic's `Action::{Click,...}` take `i32` window-LOCAL pixel coords, so
    /// we fold `origin` and round.
    pub fn apply_pointer_gesture(
        &mut self,
        font_system: &mut FontSystem,
        gesture: PointerGesture,
        pointer: Vec2,
        origin: Vec2,
    ) {
        let local = pointer - origin;
        let (x, y) = (local.x.round() as i32, local.y.round() as i32);
        let action = match gesture {
            PointerGesture::Click => Action::Click { x, y },
            PointerGesture::DoubleClick => Action::DoubleClick { x, y },
            PointerGesture::TripleClick => Action::TripleClick { x, y },
            PointerGesture::Drag => Action::Drag { x, y },
        };
        self.editor.action(font_system, action);
    }

    /// Test/inspection: the editor has no selection (a bare caret).
    pub fn editor_selection_is_none(&self) -> bool {
        self.editor.selection() == cosmic_text::Selection::None
    }

    /// Test/inspection: the editor's selection bounds (ordered), if any.
    pub fn editor_selection_bounds(&self) -> Option<(Cursor, Cursor)> {
        self.editor.selection_bounds()
    }
}

/// The focus-gated mouse-selection press observer (editing-and-ime § 4),
/// registered on the `Pointer<Press>` stream by `BuiyTextPlugin`. C3c migrated
/// the gesture *source* off the legacy `Hovered` resource onto the bevy_picking
/// `Pointer<E>` layer: the press observer fires for the **picked entity**
/// directly (capture→target→bubble), so the hit target is `press.entity` (no
/// `Hovered` read) and the window-space cursor is `press.pointer_location.position`.
///
/// On a primary press over an editable, non-`Disabled` entity it sets
/// `FocusedEntity` (focus-on-click — CORE mechanism, since the caret is core;
/// widget focus POLICY is E6 / the C3d relocation) and applies a classified
/// Click/Double/Triple via the editor's own `ClickTracker` (the multi-click
/// window+radius bevy_picking does not provide — § 2.8 keeps the classifier).
/// `focused` is `Option<ResMut<FocusedEntity>>` — `FocusedEntity` is init by
/// `FocusPlugin`, not `BuiyTextPlugin`, so a text-only harness has none and the
/// observer still applies the gesture, just without moving focus (the codebase
/// convention across the editor systems).
///
/// Note: `origin` folds `GlobalTransform + content_offset` only — it does NOT
/// fold `ScrollOffset`, correct for E3 (auto-scroll-into-view is E6; until then
/// the buffer is laid out at full size at the node origin). The one-frame
/// `GlobalTransform` lag `emit_picks` documents (§ 3.3) is inherited: the press
/// hit-tests against last frame's transform, an accepted sub-frame nicety.
///
/// Observers fire only when the picking pipeline (the meta-crate's
/// `PointerInputPlugin` + the hover stage) is present, so a headless harness
/// without that infra is inert by construction — the same robustness the old
/// `Option<...>`-param system had, achieved structurally.
pub fn editor_pointer_press(
    press: On<Pointer<Press>>,
    time: Res<Time>,
    fonts: Res<SharedFontSystem>,
    mut focused: Option<ResMut<FocusedEntity>>,
    mut tracker: Local<ClickTracker>,
    mut editors: Query<
        (&mut TextEditState, &GlobalTransform, &ComputedTextLayout),
        Without<Disabled>,
    >,
) {
    if press.event.button != PointerButton::Primary {
        return;
    }
    // The picked target IS the hit (no `Hovered` round-trip). A press that
    // bubbles up to a non-editor ancestor classifies against that ancestor; the
    // editable check below drops it, exactly as the old hovered-hit check did.
    let hit = press.entity;
    let Ok((mut state, gt, layout)) = editors.get_mut(hit) else {
        return; // pressed a non-editor — leave focus to other handlers
    };
    let pointer_pos = press.pointer_location.position;
    if let Some(focused) = focused.as_mut() {
        focused.0 = Some(hit);
    }
    let gesture = tracker.classify(pointer_pos, time.elapsed());
    let origin = gt.translation().truncate() + layout.content_offset;
    let mut fs = fonts.lock();
    state.apply_pointer_gesture(&mut fs, gesture, pointer_pos, origin);
}

/// The drag-extend observer (editing-and-ime § 4), registered on the
/// `Pointer<Drag>` stream by `BuiyTextPlugin`. bevy_picking emits `Pointer<Drag>`
/// over the press target while a button is held and the pointer moves, so this
/// replaces the old `held`-branch poll: it applies a `Drag` gesture to the
/// dragged editor, extending the selection from the press anchor
/// (`apply_pointer_gesture(Drag)` keeps the anchor fixed and moves the active
/// endpoint — see `text_mouse_selection.rs`). Drag fires on the press target, so
/// `drag.entity` is the editor the press focused; the `FocusedEntity` is no
/// longer needed to route the drag.
pub fn editor_pointer_drag(
    drag: On<Pointer<Drag>>,
    fonts: Res<SharedFontSystem>,
    mut editors: Query<
        (&mut TextEditState, &GlobalTransform, &ComputedTextLayout),
        Without<Disabled>,
    >,
) {
    if drag.event.button != PointerButton::Primary {
        return;
    }
    let Ok((mut state, gt, layout)) = editors.get_mut(drag.entity) else {
        return;
    };
    let pointer_pos = drag.pointer_location.position;
    let origin = gt.translation().truncate() + layout.content_offset;
    let mut fs = fonts.lock();
    state.apply_pointer_gesture(&mut fs, PointerGesture::Drag, pointer_pos, origin);
}
