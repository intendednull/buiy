//! **Whole-UI record/replay** (spec §7).
//!
//! Design + rationale: `docs/specs/2026-06-29-mvu-as-core-design.md` (§7 record/replay —
//! the scoped guarantee §7.3, derived-structure replay §7.4).
//!
//! ## The capability this module delivers
//! The substrate logs the *pieces*: widget Msgs land in
//! [`MsgLog`](crate::mvu::MsgLog); editor commands land in
//! [`EditLog`](crate::text::edit::EditLog). This module **unifies** them so a single recorded
//! session of REAL input replays **byte-identically** — including *widget-internal*
//! state (toggle values, the editor buffer + caret + selection) that an
//! app-boundary-only log provably cannot reconstruct.
//!
//! The unification is one shared switch + one global sequence
//! ([`RecordSession`](crate::mvu::RecordSession)): every recorded entry — widget fold OR
//! editor command — draws its `seq` from the same counter. The two logs keep their
//! natural storage (generic RON vs typed [`RecordedEdit`](crate::text::edit::RecordedEdit))
//! and are merged **by `seq`** into one totally-ordered stream
//! ([`unified_stream`](crate::replay::unified_stream)) at replay time (a read-side view,
//! not a third store). This avoids
//! forcing the editor log — which is not a [`Model`](crate::mvu::Model) — into the
//! widget-Msg shape, while still giving a single interleaved order.
//!
//! ## Replay
//! [`replay_into`](crate::replay::replay_into) walks the merged stream in `seq` order
//! against a FRESH app built from
//! the SAME seed, re-applying each entry to its [`LogicalId`](crate::mvu::LogicalId)
//! target:
//! - **widget entries** re-fold through the registered drain — the entry's RON is turned
//!   back into the concrete `Msg` by the per-type closure
//!   [`ReplayRegistry`](crate::mvu::ReplayRegistry) installed at `add_model`, written to
//!   the inbox, and drained;
//! - **editor entries** re-fold through
//!   [`TextEditState::apply_recorded`](crate::text::edit::TextEditState::apply_recorded)
//!   (the editor is the documented `PureEnv` exemption — not a `Model`).
//!
//! ## What this covers — and what it does NOT (the scoped guarantee, spec §7.3)
//! Whole-UI replay is **complete and byte-identical over the MVU-governed subtree**:
//! every funneled widget fold and every editor command. It is NOT unconditional:
//! - **Structural ops are off-log (spec §7.4).** Spawn/despawn (keyed-reconcile, "add a
//!   todo row") happen in systems *outside* the funnel, so re-folding the log does not
//!   recreate spawned children. Whole-UI replay reconstructs the *state of the entities
//!   present in the seed*, not structural changes made during the session.
//! - **No Subscription primitive (spec §8).** Timer/OS-driven sources that never enter
//!   the funnel as a logged Msg are unreplayable; IME is captured because its events are
//!   tapped at the apply site (the editor record tap), but e.g. caret-blink is not yet
//!   funnel-routed.
//! - **Escape-hatched raw-ECS writes** are by construction not logged Msgs (spec §7.3).

use std::time::Duration;

use bevy::prelude::*;

use crate::mvu::{LogicalId, MsgLog, RecordSession, ReplayRegistry};
use crate::text::SharedFontSystem;
use crate::text::edit::{EditLog, ReadOnly, RecordedEdit, SingleLine, TextEditState};

/// One entry of the unified, totally-ordered whole-UI record stream — a **merge view**
/// over the two per-domain logs keyed by the ONE global `seq`. Borrows from the logs (no
/// copy); the variant says which fold mechanism replay uses.
#[derive(Debug)]
pub enum UnifiedEntry<'a> {
    /// A funneled widget Msg (from [`MsgLog`]). Replays by re-enqueueing the RON-decoded
    /// Msg onto the model inbox (the drain path).
    Widget {
        seq: u64,
        lid: LogicalId,
        /// The `Msg` type path — the [`ReplayRegistry`] key.
        type_path: &'a str,
        /// The `Reflect`-serialized message.
        ron: &'a str,
    },
    /// An editor command (from [`EditLog`]). Replays via
    /// [`TextEditState::apply_recorded`](crate::text::edit::TextEditState::apply_recorded).
    Edit {
        seq: u64,
        lid: LogicalId,
        edit: &'a RecordedEdit,
        now: Duration,
    },
}

impl UnifiedEntry<'_> {
    /// The global sequence number (the merge key).
    pub fn seq(&self) -> u64 {
        match self {
            UnifiedEntry::Widget { seq, .. } | UnifiedEntry::Edit { seq, .. } => *seq,
        }
    }

    /// The logical target of this entry.
    pub fn lid(&self) -> LogicalId {
        match self {
            UnifiedEntry::Widget { lid, .. } | UnifiedEntry::Edit { lid, .. } => *lid,
        }
    }

    /// Whether this is a widget fold (vs an editor command).
    pub fn is_widget(&self) -> bool {
        matches!(self, UnifiedEntry::Widget { .. })
    }
}

/// A replay entry whose [`LogicalId`] resolved to **no entity** in the fresh app — a
/// genuine miss, surfaced LOUD + TYPED (spec §7.4 / D14) instead of a silent
/// `continue`.
///
/// [`replay_into`] `warn!`s each miss as it happens AND returns the full list, so a caller
/// — a test, a tool, or the §8 structural-ops work — can assert "zero dead letters" or
/// inspect which targets went missing. For a same-seed whole-UI replay the expected count
/// is **zero**: a non-empty list means an entry targeted a `LogicalId` absent from the seed
/// scene (the structural-ops gap, spec §7.4, or a mismatched seed).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DeadLetter {
    /// The unresolved logical target.
    pub lid: LogicalId,
    /// The global `seq` of the missed entry (its position in the unified stream).
    pub seq: u64,
    /// Whether the missed entry was a widget fold (`true`) or an editor command (`false`).
    pub is_widget: bool,
}

/// Merge the two per-domain logs into one stream **totally ordered by the global
/// `seq`**. Each log is already individually sorted (every append took the next global
/// seq), so this is a stable sort of the concatenation — `O((n+m) log(n+m))`, trivially a
/// two-pointer merge if it ever matters. The result borrows both logs.
pub fn unified_stream<'a>(msg_log: &'a MsgLog, edit_log: &'a EditLog) -> Vec<UnifiedEntry<'a>> {
    let mut out: Vec<UnifiedEntry<'a>> =
        Vec::with_capacity(msg_log.entries.len() + edit_log.entries.len());
    out.extend(msg_log.entries.iter().map(|e| UnifiedEntry::Widget {
        seq: e.seq,
        lid: e.lid,
        type_path: &e.type_path,
        ron: &e.ron,
    }));
    out.extend(edit_log.entries.iter().map(|e| UnifiedEntry::Edit {
        seq: e.seq,
        lid: e.lid,
        edit: &e.edit,
        now: e.now,
    }));
    out.sort_by_key(UnifiedEntry::seq);
    out
}

/// **Whole-UI replay**: re-apply a recorded session — the two logs merged by global
/// `seq` — into `app`, a FRESH app built from the SAME seed. Recording is forced OFF for
/// the duration so replay does not re-log.
///
/// Walks the unified stream in order, resolving each entry's [`LogicalId`] to the matching
/// entity in the fresh app and re-folding it through the appropriate path (drain for widget
/// folds, [`apply_recorded`](crate::text::edit::TextEditState::apply_recorded) for editor
/// commands).
///
/// **Resolution is LIVE, per entry** (spec §7.4): each entry queries the world for its
/// `LogicalId` *at the moment it is applied*, not via a resolver snapshotted once before the
/// loop. So an entity spawned *during* replay (by a prior entry's fold) is found rather than
/// dead-lettered — the once-before index could not see it. Replay is offline, so the
/// per-entry query is not a hot path.
///
/// **A genuine miss is LOUD + TYPED** (spec §7.4 / D14): an entry whose `LogicalId` has no
/// entity is collected into a [`DeadLetter`], `warn!`-logged, and returned — never a
/// silent `continue`. The returned `Vec` is empty for a clean same-seed replay;
/// a non-empty list flags the structural-ops gap (spec §7.4) or a seed mismatch.
///
/// `msg_log`/`edit_log` come from the *recorded* app (or a deserialized file); `app` is a
/// *different* app, so borrowing the logs while mutating the replay app is sound.
pub fn replay_into(app: &mut App, msg_log: &MsgLog, edit_log: &EditLog) -> Vec<DeadLetter> {
    // Force recording OFF (so re-folds do not re-enter the logs) AND set the replay guard: a
    // re-folded `Cmd::Task` is SUPPRESSED for the duration — the recorded `Origin::Command`
    // result already in the log is what re-drives the model, so the effect (network/clock/RNG)
    // is not re-run (buiy_view design §3 #15). The guard is orthogonal to record-mode: it must
    // be an explicit flag because `RecordMode::Off` is also the normal production hot path,
    // where tasks MUST launch.
    if let Some(mut session) = app.world_mut().get_resource_mut::<RecordSession>() {
        session.stop();
        session.set_replaying(true);
    }

    let stream = unified_stream(msg_log, edit_log);
    let mut dead_letters = Vec::new();

    for entry in stream {
        // Resolve LIVE, this iteration — so an entity spawned during replay is found.
        let Some(target) = resolve_lid(app, entry.lid()) else {
            // A genuine miss: surface it LOUD + TYPED (never a silent `continue`).
            let dead = DeadLetter {
                lid: entry.lid(),
                seq: entry.seq(),
                is_widget: entry.is_widget(),
            };
            warn!(
                "replay dead-letter: no entity for {:?} at seq {} (is_widget={}) — the \
                 entry's logical target is absent from the fresh app (structural-ops gap \
                 H8(i), or a seed mismatch)",
                dead.lid, dead.seq, dead.is_widget
            );
            dead_letters.push(dead);
            continue;
        };
        match entry {
            UnifiedEntry::Widget { type_path, ron, .. } => {
                // Re-enqueue the logged Msg via its per-type applier, then drain one
                // frame so the SAME registered reducer re-folds it (the "drain path").
                // `resource_scope` lifts the registry out so the applier gets `&mut World`.
                app.world_mut()
                    .resource_scope::<ReplayRegistry, _>(|world, registry| {
                        if let Some(applier) = registry.applier(type_path) {
                            applier(world, target, ron);
                        }
                    });
                app.update();
            }
            UnifiedEntry::Edit { edit, now, .. } => {
                apply_recorded_edit(app, target, edit, now);
            }
        }
    }
    dead_letters
}

/// Resolve a [`LogicalId`] to its entity in `app` **right now** — queried live so an entity
/// spawned during replay is found (the per-iteration resolution §7.4 mandates over a
/// once-before-the-loop index). `None` for a genuine miss (→ a [`DeadLetter`]).
fn resolve_lid(app: &mut App, lid: LogicalId) -> Option<Entity> {
    let mut q = app.world_mut().query::<(Entity, &LogicalId)>();
    q.iter(app.world())
        .find(|(_, l)| **l == lid)
        .map(|(e, _)| e)
}

/// Re-apply one editor command to its target editor in the replay app, through the SAME
/// fold the live editor uses (`apply_recorded` → `apply_tracked` / the IME primitives),
/// locking the replay app's own `FontSystem` (the determinism boundary).
fn apply_recorded_edit(app: &mut App, target: Entity, edit: &RecordedEdit, now: Duration) {
    let fonts = app.world().resource::<SharedFontSystem>().clone();
    let single_line = app.world().get::<SingleLine>(target).is_some();
    let read_only = app.world().get::<ReadOnly>(target).is_some();
    let mut fs = fonts.lock();
    if let Some(mut state) = app.world_mut().get_mut::<TextEditState>(target) {
        state.apply_recorded(&mut fs, edit, single_line, read_only, now);
    }
}
