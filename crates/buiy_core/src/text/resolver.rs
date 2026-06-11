//! The Buiy-owned font-family stack resolver (font-assets § 6): per text
//! run, BEFORE `Attrs` construction, entirely lock-free (T5 plan decision 2
//! — `FontRegistry` + `FontMatchIndex` snapshot only). Two verified API
//! facts force Buiy ownership: `Attrs.family` is a SINGLE `Family` and the
//! `Fallback` trait is constructor-injected + `'static` — per-node stacks
//! cannot live inside cosmic-text. Below the stack, cosmic-text's per-glyph
//! `FontFallbackIter` (engine-internal — shape.rs:307/489/985; public at
//! `cosmic_text::fallback` but never constructed by Buiy) remains the
//! last-resort safety net: it runs only when the resolved face misses a
//! glyph, i.e. when the author's entire stack missed (T5 erratum 4).
//!
//! Per-span authored families/stacks (`<bdi>`, inline `dir` isolates) are
//! the C-tier rich-text seam (font-assets § 8) — the span machinery here is
//! *coverage*-driven only.

use std::ops::Range;
use std::sync::atomic::{AtomicBool, Ordering};

use bevy::log::warn;
use cosmic_text::fontdb;
use unicode_script::{Script, UnicodeScript};

use super::components::{FamilyEntry, FontStack, GenericFamily};
use super::match_index::FontMatchIndex;
use super::registry::{FontDisplay, FontLoadState, FontRegistry};

/// One resolved span's family target. `Named` carries an OWNED `String`
/// (resolution only runs on damage-gated syncs; interning/`SmolStr` is a
/// named perf seam — T5 plan decision 8).
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ResolvedFamily {
    /// A concrete family name the walk matched (or fell back to).
    Named(String),
    /// A terminal generic entry, lowered as the cosmic generic.
    Generic(GenericFamily),
}

/// A resolved byte-range of the (collapsed + direction-marked) string.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ResolvedSpan {
    /// Byte range into the resolver's input string.
    pub range: Range<usize>,
    /// The family this range shapes against.
    pub family: ResolvedFamily,
}

/// Resolution output: spans in source order, tiling the input exactly,
/// plus the `font-display: block` flag (entity-level v1 — decision 9).
#[derive(Clone, PartialEq, Eq, Debug, Default)]
pub struct Resolution {
    /// The resolved spans (empty iff the input is empty).
    pub spans: Vec<ResolvedSpan>,
    /// True when any char's walk passed a `Loading` family declared
    /// `font-display: Block` still inside its 3 s window (Task 7's
    /// `PendingFontBlock` source).
    pub blocked: bool,
}

/// Walk the stack per codepoint (T5 plan decision 7):
///
/// - Chars whose `unicode_script::Script` is `Common`/`Inherited`/`Unknown`
///   never force a span boundary (they join the current span; leading ones
///   attach to the first resolved span) — the HarfBuzz itemization rule,
///   and what keeps the § 5.4 marks + ZWJ/VS16 from fragmenting spans.
/// - Generic entries are TERMINAL (the deterministic `registered_fonts_db`
///   § 4 pins resolve them; no coverage probe — the generic is the
///   author's catch-all).
/// - Named entries: registry gate (`Loading` → skip [Block marking inside
///   its window], `Failed` → skip, declared unicode-range filter), then
///   fontdb `Query` by name + weight (default style/stretch — no carriers,
///   C-tier) on the snapshot; the entry wins iff the matched face COVERS
///   the char.
/// - No winner → the FIRST entry (`FontFallbackIter` patches per-glyph).
///
/// Adjacent equal resolutions merge; the spans tile `text` exactly.
pub fn resolve_spans(
    text: &str,
    stack: &FontStack,
    weight: u16,
    registry: &FontRegistry,
    index: &mut FontMatchIndex,
    now: f64,
) -> Resolution {
    let mut resolution = Resolution::default();
    if text.is_empty() {
        return resolution;
    }
    // (start byte, family) of the span currently being grown.
    let mut current: Option<(usize, ResolvedFamily)> = None;
    for (offset, c) in text.char_indices() {
        if matches!(
            c.script(),
            Script::Common | Script::Inherited | Script::Unknown
        ) {
            continue; // joins the current span (or the first resolved one)
        }
        let family = resolve_char(
            c,
            stack,
            weight,
            registry,
            index,
            now,
            &mut resolution.blocked,
        );
        match &current {
            Some((_, open)) if *open == family => {} // merge
            Some(_) => {
                let (start, open) = current.take().expect("matched Some above");
                resolution.spans.push(ResolvedSpan {
                    range: start..offset,
                    family: open,
                });
                current = Some((offset, family));
            }
            // Leading Common/Inherited chars attach to the first resolved
            // span: start at 0, not at this char's offset.
            None => current = Some((0, family)),
        }
    }
    let tail = match current {
        Some((start, family)) => ResolvedSpan {
            range: start..text.len(),
            family,
        },
        // No boundary-forcing char at all (all Common/Inherited/Unknown):
        // such chars never probe the stack, so the whole text lowers to the
        // first entry — exactly the no-winner rule, span-shaped.
        None => ResolvedSpan {
            range: 0..text.len(),
            family: first_entry(stack),
        },
    };
    resolution.spans.push(tail);
    resolution
}

/// The decision-7 walk for one boundary-forcing char.
fn resolve_char(
    c: char,
    stack: &FontStack,
    weight: u16,
    registry: &FontRegistry,
    index: &mut FontMatchIndex,
    now: f64,
    blocked: &mut bool,
) -> ResolvedFamily {
    for entry in &stack.0 {
        let name = match entry {
            // Terminal: lowered as the cosmic generic, no coverage probe.
            FamilyEntry::Generic(generic) => return ResolvedFamily::Generic(*generic),
            FamilyEntry::Named(name) => name,
        };
        // The registry gate. An UNREGISTERED name passes vacuously — the
        // stack may name faces that entered outside the registry (the
        // embedded default, a system scan).
        match registry.load_state(name) {
            Some(FontLoadState::Loading) => {
                match registry
                    .descriptors(name)
                    .map(|descriptors| descriptors.font_display)
                    .unwrap_or_default()
                {
                    FontDisplay::Block => {
                        // Inside the window: mark; past the deadline Block
                        // degrades to Swap (the § 7 timeout).
                        if registry
                            .block_deadline(name)
                            .is_some_and(|until| now < until)
                        {
                            *blocked = true;
                        }
                    }
                    FontDisplay::Swap => {}
                    // C-tier reserve: parse, degrade to Swap, warn once.
                    FontDisplay::Fallback | FontDisplay::Optional => {
                        warn_once_font_display_degrades();
                    }
                }
                continue;
            }
            Some(FontLoadState::Failed) => continue,
            Some(FontLoadState::Loaded) | None => {}
        }
        // Declared unicode-range: a per-codepoint face filter (§ 6.1);
        // families with no declared range skip the check entirely.
        if let Some(ranges) = registry
            .descriptors(name)
            .and_then(|descriptors| descriptors.unicode_range.as_ref())
            && !ranges.contains(c)
        {
            continue;
        }
        // fontdb's real CSS matcher on the snapshot; the entry wins iff
        // the matched face covers the char.
        let query = fontdb::Query {
            families: &[fontdb::Family::Name(name)],
            weight: fontdb::Weight(weight),
            ..Default::default()
        };
        if let Some(id) = index.query(&query)
            && index.covers(id, c)
        {
            return ResolvedFamily::Named(name.clone());
        }
    }
    first_entry(stack)
}

/// The no-winner resolution: the stack's FIRST entry (shaping's
/// `FontFallbackIter` patches per-glyph below it). An empty authored stack
/// degrades to the pinned sans-serif generic (the sync.rs `first_family`
/// precedent).
fn first_entry(stack: &FontStack) -> ResolvedFamily {
    match stack.0.first() {
        Some(FamilyEntry::Named(name)) => ResolvedFamily::Named(name.clone()),
        Some(FamilyEntry::Generic(generic)) => ResolvedFamily::Generic(*generic),
        None => ResolvedFamily::Generic(GenericFamily::SansSerif),
    }
}

static WARNED_FONT_DISPLAY_DEGRADE: AtomicBool = AtomicBool::new(false);

/// The components.rs warn-once statics precedent.
fn warn_once_font_display_degrades() {
    if !WARNED_FONT_DISPLAY_DEGRADE.swap(true, Ordering::Relaxed) {
        warn!(
            "buiy: font-display fallback/optional are reserved (font-assets § 9); \
             degrading to swap (warned once)"
        );
    }
}
