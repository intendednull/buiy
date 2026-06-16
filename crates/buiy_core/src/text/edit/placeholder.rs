//! E6 — placeholder rendering state (editing-and-ime § 10,
//! decoration-and-paint § 7). The placeholder is "just text with a different
//! tint" (the spec decision; the rejected runner-up is a dedicated placeholder
//! paint path). E6 maintains a `PlaceholderActive` marker — present iff the
//! editor's logical value is empty (preedit excluded) AND a non-empty
//! `Placeholder` string exists — and shapes the string into a display-only
//! `PlaceholderBuffer` the glyph producer paints in `color.text.placeholder`.
//! The string never enters the editor buffer or the undo history.
//!
//! **M3 — the placeholder buffer shapes ITSELF.** Unlike the editor buffer
//! (which `TextCommit` reshapes downstream via `TextBufferAccess`), nothing
//! downstream touches a `PlaceholderBuffer` — so `sync_placeholder` must lock
//! `SharedFontSystem` and call `buffer.shape_until_scroll(&mut fs, false)`
//! after `set_text`, or `layout_runs()` stays empty and the placeholder paints
//! nothing. This system takes the lock (correct — it runs in the
//! `BuiyLayoutStep::TextSync` step, the measure-pipeline lock window).
//!
//! This file names only the pure-data `Buffer`/`Metrics`/`Attrs`/`Shaping`
//! cosmic types (no `Editor`/`Edit`/`Action`/`Change`), so it stays inside the
//! `text::edit` facade.

use bevy::prelude::*;
use cosmic_text::{Attrs, Buffer, Metrics, Shaping};

use super::state::{Disabled, Placeholder, TextEditState};
use crate::text::{FontSize, SharedFontSystem};

/// Marker: the placeholder is currently shown (the editor value is empty,
/// preedit excluded, and a non-empty `Placeholder` exists). Drives the
/// producer's DAMAGE gate (a `Changed<PlaceholderActive>` probe member) and the
/// headless test; the producer's PAINT branch keys on `PlaceholderBuffer`
/// presence (M2 — the inner extract tuple is at the 15-cap, so the marker is
/// not in it). Lean derives (not reflect-registered — no authored data).
#[derive(Component, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaceholderActive;

/// The display-only shaped buffer for the placeholder string. NEVER the
/// editor buffer (§ 10: "the placeholder never enters the editor Buffer").
/// `pub buffer` so the run-count test + the producer read it; not
/// reflect-registered (carries a cosmic `Buffer`, the cosmic boundary).
#[derive(Component)]
pub struct PlaceholderBuffer {
    pub buffer: Buffer,
    /// The string the buffer was last shaped from — so we only re-shape on a
    /// `Placeholder` text change, not every frame.
    shaped_from: String,
}

/// Main-world (the `BuiyLayoutStep::TextSync` step): maintain the
/// `PlaceholderActive` marker and the `PlaceholderBuffer` for every editor with
/// a `Placeholder`. Present the marker iff `value().is_empty() &&
/// !has_preedit()` and the placeholder string is non-empty; remove BOTH the
/// marker and the buffer otherwise (so the producer's
/// `PlaceholderBuffer`-presence paint signal is exact). When active, shape the
/// string into the display-only buffer (M3 — its own `shape_until_scroll`,
/// since nothing downstream shapes it).
///
/// Runs in the same step as `text_sync_buffers` (the measure-pipeline lock
/// window); `Without<Disabled>` follows the editor-system discipline.
#[allow(clippy::type_complexity)]
pub fn sync_placeholder(
    mut commands: Commands,
    fonts: Res<SharedFontSystem>,
    mut editors: Query<
        (
            Entity,
            &TextEditState,
            &Placeholder,
            Option<&FontSize>,
            Option<&mut PlaceholderBuffer>,
            Has<PlaceholderActive>,
        ),
        Without<Disabled>,
    >,
) {
    for (entity, state, placeholder, font_size, ph_buffer, was_active) in &mut editors {
        let active = state.value().is_empty() && !state.has_preedit() && !placeholder.0.is_empty();

        if active && !was_active {
            commands.entity(entity).insert(PlaceholderActive);
        } else if !active && was_active {
            // Remove BOTH — the producer paints on PlaceholderBuffer presence.
            commands.entity(entity).remove::<PlaceholderActive>();
            commands.entity(entity).remove::<PlaceholderBuffer>();
        }

        if !active {
            continue;
        }

        // Shape the placeholder string into the display-only buffer. M3: lock
        // and shape OURSELVES — nothing downstream shapes a PlaceholderBuffer
        // (TextCommit only reshapes the editor-owned buffer). Skip when the
        // string is unchanged (already shaped this content).
        let size = font_size.map(|f| f.0).unwrap_or(16.0);
        let metrics = Metrics::new(size, size * 1.2);
        match ph_buffer {
            Some(buf) if buf.shaped_from == placeholder.0 => { /* unchanged */ }
            Some(mut buf) => {
                // `set_metrics` / `set_text` record + dirty (no FontSystem);
                // `shape_until_scroll` does the actual shape under the lock.
                buf.buffer.set_metrics(metrics);
                buf.buffer
                    .set_text(&placeholder.0, &Attrs::new(), Shaping::Advanced, None);
                // M3: shape OURSELVES — shape_until_scroll DOES take the lock.
                buf.buffer.shape_until_scroll(&mut fonts.lock(), false);
                buf.shaped_from = placeholder.0.clone();
            }
            None => {
                // `Buffer::new(&mut fs, metrics)` is the shaped constructor;
                // set_text (no fs, defers) + shape_until_scroll (fs, shapes).
                let mut fs = fonts.lock();
                let mut buffer = Buffer::new(&mut fs, metrics);
                buffer.set_text(&placeholder.0, &Attrs::new(), Shaping::Advanced, None);
                buffer.shape_until_scroll(&mut fs, false);
                commands.entity(entity).insert(PlaceholderBuffer {
                    buffer,
                    shaped_from: placeholder.0.clone(),
                });
            }
        }
    }
}
