//! E3 — mouse selection (editing-and-ime § 4, mouse gestures). The pure mapping
//! (`pointer_to_cursor`), the click-count state machine (`ClickTracker` /
//! `PointerGesture`), and the gesture→`Action` application on `TextEditState`.
//! The windowed wiring (reading `PointerLocation`/`Hovered`, setting
//! `FocusedEntity`) is `pointer_selection`, registered in `BuiySet::Input`.
//! This file NAMES `Action`/`Edit` (the lowering) ⇒ inside the facade.

use std::time::Duration;

use bevy::input::ButtonInput;
use bevy::math::Vec2;
use bevy::picking::pointer::PointerLocation;
use bevy::prelude::*;
use cosmic_text::{Action, Buffer, Cursor, Edit, FontSystem};

use super::state::{Disabled, TextEditState};
use crate::FocusedEntity;
use crate::picking::Hovered;
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

/// The focus-gated mouse-selection system (editing-and-ime § 4), `BuiySet::Input`
/// (alongside `apply_keyboard_edits`). On a left press over an editable, non-
/// `Disabled` entity it sets `FocusedEntity` (focus-on-click — CORE mechanism,
/// since the caret is core; widget focus POLICY is E6) and applies a classified
/// Click/Double/Triple. While the button stays held and the pointer moves it
/// applies `Drag`. All `Option<...>` params so a headless harness without
/// picking/input infra runs it inertly (the apply_keyboard_edits precedent).
///
/// **One-frame `Hovered` lag (accepted).** `Hovered` is written by
/// `update_hovered` in `BuiySet::Picking`, which runs AFTER `BuiySet::Input`
/// (lib.rs ordering) — so a press this frame hit-tests against LAST frame's
/// `Hovered`. Functionally correct and consistent with the OQ#1 one-frame
/// latency posture (a pointer that moved between frames is a sub-frame
/// nicety); not a bug. Note: `origin` folds `GlobalTransform + content_offset`
/// only — it does NOT fold `ScrollOffset`, correct for E3 (auto-scroll-into-view
/// is E6; until then the buffer is laid out at full size at the node origin).
#[allow(clippy::too_many_arguments, clippy::type_complexity)]
pub fn pointer_selection(
    time: Res<Time>,
    mouse: Option<Res<ButtonInput<MouseButton>>>,
    hovered: Option<Res<Hovered>>,
    pointers: Query<&PointerLocation>,
    fonts: Res<SharedFontSystem>,
    mut focused: Option<ResMut<FocusedEntity>>,
    mut tracker: Local<ClickTracker>,
    mut editors: Query<
        (&mut TextEditState, &GlobalTransform, &ComputedTextLayout),
        Without<Disabled>,
    >,
) {
    let (Some(mouse), Some(hovered), Some(focused)) = (mouse, hovered, focused.as_mut()) else {
        return;
    };
    // The active pointer's window-space position (logical px).
    let Some(pointer_pos) = pointers
        .iter()
        .find_map(|p| p.location.as_ref().map(|l| l.position))
    else {
        return;
    };

    let pressed = mouse.just_pressed(MouseButton::Left);
    let held = mouse.pressed(MouseButton::Left);

    if pressed {
        // Focus-on-click: only when the press lands on an editable entity.
        let Some(hit) = hovered.0 else { return };
        if editors.get(hit).is_err() {
            return; // clicked a non-editor — leave focus to other handlers
        }
        focused.0 = Some(hit);
        let gesture = tracker.classify(pointer_pos, time.elapsed());
        if let Ok((mut state, gt, layout)) = editors.get_mut(hit) {
            let origin = gt.translation().truncate() + layout.content_offset;
            let mut fs = fonts.lock();
            state.apply_pointer_gesture(&mut fs, gesture, pointer_pos, origin);
        }
    } else if held {
        // Drag-extend on the focused editor (the press already focused it).
        let Some(entity) = focused.0 else { return };
        if let Ok((mut state, gt, layout)) = editors.get_mut(entity) {
            let origin = gt.translation().truncate() + layout.content_offset;
            let mut fs = fonts.lock();
            state.apply_pointer_gesture(&mut fs, PointerGesture::Drag, pointer_pos, origin);
        }
    }
}
