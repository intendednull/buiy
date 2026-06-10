//! `BuiyLayoutStep::TextSync` — the dirty path (measure-and-layout § 4.1;
//! architecture §§ 4.1, 5.1).
//!
//! Creates/updates `TextBuffer` from the authored components via the 0.19
//! LAZY setters — no `FontSystem`, no lock (architecture § 1.2): mutation
//! is recorded, shaping deferred to the next lock-bearing site (T3's
//! measure closure / `TextCommit`). Invalidates the cached intrinsics and
//! (Task 7 / T3 consumers) dirty-marks the Taffy node.
//!
//! **The trigger-union ledger (architecture § 5.1 row 1).** As specced:
//! `Or<(Changed<Text>, Changed<text-style carriers>, Added<TextBuffer>,
//! Changed<WritingModeResolved>)>` ∪ `FontsGeneration` bump ∪ theme
//! font-token swap. Carriers existing in T2: `Text`, `FontFamily`,
//! `FontSize`, `FontWeight`, `WritingModeResolved`, `FontsGeneration`.
//! Members that join with their carriers: line-height / white-space /
//! text-wrap / text-align (**T3**), `TextDirection` (**T5**, with the
//! § 5.4 strong-mark prepend), the theme font-token swap
//! (**buiy-theme-tokens-design**, font-assets § 9).

use bevy::prelude::*;
use cosmic_text::{Attrs, Family, Metrics, Weight, Wrap};

use crate::layout::{LayoutTree, WritingModeResolved};

use super::components::{
    ComputedTextLayout, FamilyEntry, FontFamily, FontSize, FontStack, FontWeight, TEXT_SHAPING,
    Text, TextBuffer, TextStyleDefaults,
};
use super::font_system::FontsGeneration;
use super::whitespace::{CollapseMode, collapse_whitespace};

/// CSS `line-height: normal` stand-in (the common UA factor) until T3 lands
/// the line-height carrier and the measure § 5.1 `Metrics` mapping.
pub(crate) const DEFAULT_LINE_HEIGHT_SCALE: f32 = 1.2;

/// The white-space value table's `normal` row (measure § 5.2): collapse ×
/// `Wrap::Word`. Pinned explicitly — `Buffer::new_empty` defaults to
/// `Wrap::WordOrGlyph` (source-verified), the C-tier `overflow-wrap`
/// behavior, not the CSS initial. T3's white-space/text-wrap carriers
/// drive the full table.
const DEFAULT_WRAP: Wrap = Wrap::Word;

/// CSS `tab-size` initial (measure § 5.2 — "set to 8 … at `TextSync`");
/// the C-tier `tab-size` property later drives the same lazy setter.
const DEFAULT_TAB_WIDTH: u16 = 8;

/// Per-frame count of text entities `text_sync_buffers` applied the lazy
/// setters to — the `SyncStylesIterCount` precedent (layout/systems.rs
/// ~:109). Overwritten (not accumulated) at the top of every invocation.
/// `tests/text_sync.rs` asserts ZERO on a no-change frame (the steady-state
/// half of architecture § 5's contract) and exact counts per § 5.1 trigger.
#[derive(Resource, Default, Debug)]
pub struct TextSyncAppliedCount(pub usize);

/// The § 5.1 row-1 union over the carriers that exist in T2 (the module
/// doc is the ledger for members that join later).
type TextSyncTriggers = Or<(
    Changed<Text>,
    Changed<FontFamily>,
    Changed<FontSize>,
    Changed<FontWeight>,
    Added<TextBuffer>,
    Changed<WritingModeResolved>,
)>;

type SyncedText = (
    Entity,
    &'static Text,
    &'static mut TextBuffer,
    Option<&'static FontFamily>,
    Option<&'static FontSize>,
    Option<&'static FontWeight>,
);

type SyncedTextItem<'w> = (
    Entity,
    &'w Text,
    Mut<'w, TextBuffer>,
    Option<&'w FontFamily>,
    Option<&'w FontSize>,
    Option<&'w FontWeight>,
);

/// The `BuiyLayoutStep::TextSync` body (measure-and-layout § 4.1).
///
/// Registered by `BuiyTextPlugin`; the step set itself is configured
/// (chained) by `LayoutPlugin`'s `configure_pipeline` — standalone
/// `BuiyTextPlugin` apps (the T1 engine tests) run it unordered with empty
/// queries, which is inert.
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
pub fn text_sync_buffers(
    mut commands: Commands,
    defaults: Res<TextStyleDefaults>,
    fonts_generation: Res<FontsGeneration>,
    mut applied: ResMut<TextSyncAppliedCount>,
    mut tree: Option<NonSendMut<LayoutTree>>,
    mut removed_texts: RemovedComponents<Text>,
    mut synced: ParamSet<(Query<SyncedText, TextSyncTriggers>, Query<SyncedText>)>,
    unsynced: Query<
        (
            Entity,
            &Text,
            Option<&FontFamily>,
            Option<&FontSize>,
            Option<&FontWeight>,
        ),
        Without<TextBuffer>,
    >,
) {
    applied.0 = 0;
    let mut ctx = SyncContext {
        defaults: &defaults,
        tree: tree.as_deref_mut(),
        applied: &mut applied,
    };

    // Creation: a `Text` entity without a buffer gets one built and FULLY
    // populated this frame (TextSync precedes SyncStyles, so the deferred
    // insert is visible to the same frame's style sync — text never appears
    // a frame late). The insertion tick fires next frame's
    // `Added<TextBuffer>` arm once more: an idempotent lazy re-apply,
    // before any shaping consumer exists (documented; tests `settle()`
    // across it).
    for (entity, text, family, size, weight) in &unsynced {
        let style = AuthoredStyle::resolve(ctx.defaults, family, size, weight);
        let mut buffer = TextBuffer::new(style.metrics());
        apply_authored(&mut buffer, text, &style);
        commands.entity(entity).insert(buffer);
        ctx.applied.0 += 1;
    }

    // A FontsGeneration bump sweeps EVERY buffer — late fonts never leave
    // stale tofu (architecture § 2.2). Otherwise only the trigger set runs.
    // (`is_added` excluded: the plugin-init frame has no buffers to sweep.)
    if fonts_generation.is_changed() && !fonts_generation.is_added() {
        let mut all = synced.p1();
        for item in all.iter_mut() {
            sync_one(item, &mut ctx);
        }
    } else {
        let mut triggered = synced.p0();
        for item in triggered.iter_mut() {
            sync_one(item, &mut ctx);
        }
    }

    // `Text` removed while the entity lives: the leaf stops being a text
    // leaf — drop the buffer and the (T3-written) commit output. Despawned
    // entities clean up for free; `get_entity` filters them out here. The
    // Taffy `set_node_context` unregistration on this same edge is T3's
    // (measure § 2.2 — it lands with the TaffyTree<Entity> migration).
    for entity in removed_texts.read() {
        if let Ok(mut entity_commands) = commands.get_entity(entity) {
            entity_commands.remove::<(TextBuffer, ComputedTextLayout)>();
        }
    }
}

struct SyncContext<'a> {
    defaults: &'a TextStyleDefaults,
    tree: Option<&'a mut LayoutTree>,
    applied: &'a mut TextSyncAppliedCount,
}

fn sync_one(item: SyncedTextItem<'_>, ctx: &mut SyncContext<'_>) {
    let (entity, text, mut buffer, family, size, weight) = item;
    // EVERY in-place TextBuffer write bypasses change detection
    // (measure-and-layout § 7): a sync write is not a damage signal —
    // damage keys on the commit outputs; `Changed<TextBuffer>` is reserved
    // for nothing (tests/text_sync.rs pins it never fires past insertion).
    let buffer = buffer.bypass_change_detection();
    let style = AuthoredStyle::resolve(ctx.defaults, family, size, weight);
    apply_authored(buffer, text, &style);
    if let Some(tree) = ctx.tree.as_deref_mut() {
        // Absent tree (standalone BuiyTextPlugin, no LayoutPlugin): nothing
        // measures, nothing to dirty.
        tree.mark_dirty_for_entity(entity);
    }
    ctx.applied.0 += 1;
}

/// The authored style snapshot TextSync lowers into cosmic-text state.
/// Unset components fall back to `TextStyleDefaults` (font-assets § 8).
struct AuthoredStyle<'a> {
    family: &'a FontStack,
    size: f32,
    weight: u16,
}

impl<'a> AuthoredStyle<'a> {
    fn resolve(
        defaults: &'a TextStyleDefaults,
        family: Option<&'a FontFamily>,
        size: Option<&FontSize>,
        weight: Option<&FontWeight>,
    ) -> Self {
        Self {
            family: family.map_or(&defaults.family, |component| &component.0),
            size: size.map_or(defaults.size, |component| component.0),
            weight: weight.map_or(defaults.weight, |component| component.0),
        }
    }

    /// font-size → `Metrics`, with the line-height stand-in (T3 lands the
    /// carrier and the measure § 5.1 mapping).
    fn metrics(&self) -> Metrics {
        Metrics::relative(self.size, DEFAULT_LINE_HEIGHT_SCALE)
    }

    /// Lower to the per-buffer `Attrs`. T2 INTERIM: the stack's FIRST entry
    /// only — the Buiy-owned resolver (fontdb `Query` walk, coverage
    /// span-splitting, `unicode-range`) is T5's (font-assets § 6); until it
    /// lands, misses fall through to cosmic-text's `FontFallbackIter` and
    /// the deterministic `BuiyFallback`.
    fn attrs(&self) -> Attrs<'_> {
        Attrs::new()
            .family(self.first_family())
            .weight(Weight(self.weight))
    }

    fn first_family(&self) -> Family<'_> {
        match self.family.0.first() {
            Some(FamilyEntry::Named(name)) => Family::Name(name),
            Some(FamilyEntry::Generic(generic)) => generic.to_cosmic(),
            // An empty authored stack degrades to the pinned sans-serif
            // generic rather than panicking or skipping the entity.
            None => Family::SansSerif,
        }
    }
}

/// Apply authored content + style through the 0.19 LAZY setters — no
/// FontSystem, no lock; shaping deferred (architecture §§ 1.2, 3.2).
///
/// `alignment: None` = CSS `start` (the § 5.3 table: cosmic-text's
/// unaligned default follows the line's BiDi direction); the align carrier
/// is T3's. The § 5.4 direction strong-mark prepend (T5) slots between the
/// collapse transform and `set_text`, AFTER the trim.
fn apply_authored(buffer: &mut TextBuffer, text: &Text, style: &AuthoredStyle<'_>) {
    // T2 pins the CSS `white-space: normal` initial (collapse mode); T3's
    // carrier selects across the full § 5.2 value table.
    let collapsed = collapse_whitespace(&text.0, CollapseMode::Collapse);
    buffer.buffer.set_metrics(style.metrics());
    buffer.buffer.set_wrap(DEFAULT_WRAP);
    buffer.buffer.set_tab_width(DEFAULT_TAB_WIDTH);
    buffer
        .buffer
        .set_text(&collapsed, &style.attrs(), TEXT_SHAPING, None);
    buffer.invalidate_intrinsics();
}
