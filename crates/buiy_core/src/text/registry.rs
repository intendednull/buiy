//! `FontRegistry` — the `FontFaceSet` analogue (font-assets § 3): strong
//! handles, declared family names, explicit register/unregister. Face
//! ADDITION is in-place (`db_mut().load_font_source` — safe: `db_mut`
//! clears only `font_matches_cache`, and a new face can never make a
//! match-cache entry stale). Face REMOVAL rebuilds the `FontSystem` via
//! `into_locale_and_db` (§ 3.1): the `font_cache` has no purge API — after
//! an in-place remove it would leak the `Arc<Font>` forever AND serve the
//! dead face from `get_font`. The rebuild carries the SAME `Database`, so
//! surviving IDs stay valid (in-lineage — `FontDbLineage` untouched; T5
//! plan Orientation fact 1).
//!
//! Registry methods STAGE ops; [`apply_font_registry`] (one system, before
//! `BuiySet::Layout`) applies them + the `AssetEvent` stream under ONE
//! lock hold and ONE `FontsGeneration` bump per batch — a frame never
//! measures against a half-registered family (§ 3). This is a RARE-EVENT
//! lock site (architecture § 1.2's "exactly three" table is steady-frame
//! scoped — T5 erratum 2).

use std::collections::{HashMap, HashSet};
use std::mem;
use std::ops::RangeInclusive;
use std::sync::Arc;

use bevy::asset::{AssetEvent, AssetId, Assets, Handle};
use bevy::prelude::*;
use cosmic_text::{FontSystem, fontdb};

use super::components::{FamilyEntry, FontStack};
use super::font_asset::BuiyFont;
use super::font_system::{
    BuiyFallback, FontsGeneration, SharedFontSystem, placeholder_font_system,
};
use super::match_index::FontMatchIndex;

/// `font-display` (font-assets § 7): v1 implements Swap (default) + Block;
/// Fallback/Optional parse and degrade to Swap with a warn-once (C-tier
/// reserved — the descriptor shape is the spec's).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum FontDisplay {
    Block,
    #[default]
    Swap,
    Fallback,
    Optional,
}

/// Declared `unicode-range` (font-assets § 6.1): a per-codepoint face
/// filter enforced by the resolver (fontdb has no range concept —
/// verified). Programmatic ranges only; the CSS string syntax is
/// styling-tier (named seam).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnicodeRanges(Vec<RangeInclusive<u32>>);

impl UnicodeRanges {
    pub fn new(ranges: Vec<RangeInclusive<u32>>) -> Self {
        Self(ranges)
    }

    pub fn contains(&self, c: char) -> bool {
        let cp = c as u32;
        self.0.iter().any(|range| range.contains(&cp))
    }
}

/// Registration descriptors (font-assets § 3). Families with NO declared
/// range skip the resolver's range check entirely (§ 6.1's cost gate).
#[derive(Clone, Debug, Default)]
pub struct FontFaceDescriptors {
    pub unicode_range: Option<UnicodeRanges>,
    pub font_display: FontDisplay,
}

/// A registered family's lifecycle. `Failed` = fontdb parsed no faces from
/// the bytes (the bytes path has no loader sniff in front of it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FontLoadState {
    Loading,
    Loaded,
    Failed,
}

/// How the bytes entered: Bevy asset (STRONG handle — pins the asset for
/// the registration's lifetime, the anti-silent-refallback decision § 3)
/// or the `include_bytes!` escape hatch (font-assets § 2's
/// `register_font_bytes`, method form).
enum RegisteredSource {
    Asset(Handle<BuiyFont>),
    Bytes,
}

struct FamilyRecord {
    /// fontdb face IDs, valid for the CURRENT lineage. Re-recorded by the
    /// system-scan swap (fresh db = every ID reissued; Task 5).
    faces: Vec<fontdb::ID>,
    descriptors: FontFaceDescriptors,
    load_state: FontLoadState,
    source: RegisteredSource,
    /// The loaded bytes (Arc clone of `BuiyFont.data` / the bytes argument)
    /// — the system-scan swap's re-add source (T5 decision 6). `None` until
    /// loaded; cleared again if a hot-reload's bytes fail to parse (re-adding
    /// the STALE bytes would resurrect a font the state says Failed).
    data: Option<Arc<Vec<u8>>>,
    /// `Time::elapsed_secs_f64` at registration — the Block deadline base
    /// (the CSS block period starts at load start).
    loading_since: f64,
}

enum RegistryOp {
    RegisterAsset {
        family: String,
        handle: Handle<BuiyFont>,
        descriptors: FontFaceDescriptors,
    },
    RegisterBytes {
        family: String,
        bytes: Arc<Vec<u8>>,
        descriptors: FontFaceDescriptors,
    },
    /// Hot-reload via the bytes path (tests; advanced apps).
    ReregisterBytes {
        family: String,
        bytes: Arc<Vec<u8>>,
    },
    Unregister {
        family: String,
    },
}

/// The main-world registry resource. Methods only STAGE ops (decision 5:
/// engine mutation scattered across user call sites would make the
/// one-bump-per-batch contract unenforceable); [`apply_font_registry`]
/// drains them once per frame.
#[derive(Resource, Default)]
pub struct FontRegistry {
    families: HashMap<String, FamilyRecord>,
    ops: Vec<RegistryOp>,
    /// Families already warned for a declared-vs-internal name mismatch
    /// (decision 4: warn loudly, once per family — re-warns on every
    /// hot-reload cycle would be noise).
    warned_mismatch: HashSet<String>,
}

impl FontRegistry {
    /// Register `family` backed by a `BuiyFont` asset (the `@font-face`
    /// model — the DECLARED name keys the record and is what stacks
    /// reference, decision 4). The handle is held STRONG for the
    /// registration's lifetime. A not-yet-loaded handle records `Loading`;
    /// the asset's arrival completes it. Re-registering an existing family
    /// replaces it (CSS: last declaration wins).
    pub fn register_asset(
        &mut self,
        family: impl Into<String>,
        handle: Handle<BuiyFont>,
        descriptors: FontFaceDescriptors,
    ) {
        self.ops.push(RegistryOp::RegisterAsset {
            family: family.into(),
            handle,
            descriptors,
        });
    }

    /// Register `family` from raw sfnt bytes — the `include_bytes!` escape
    /// hatch (font-assets § 2). Needs no asset machinery (decision 13).
    pub fn register_bytes(
        &mut self,
        family: impl Into<String>,
        bytes: Arc<Vec<u8>>,
        descriptors: FontFaceDescriptors,
    ) {
        self.ops.push(RegistryOp::RegisterBytes {
            family: family.into(),
            bytes,
            descriptors,
        });
    }

    /// Hot-reload `family`'s bytes: remove + re-add composed under the
    /// batch's single lock hold and single bump (the bytes-path analogue
    /// of `AssetEvent::Modified`). Descriptors are kept.
    pub fn reregister_bytes(&mut self, family: impl Into<String>, bytes: Arc<Vec<u8>>) {
        self.ops.push(RegistryOp::ReregisterBytes {
            family: family.into(),
            bytes,
        });
    }

    /// Unregister `family`: its faces leave via the § 3.1 rebuild and its
    /// strong handle drops with the record.
    pub fn unregister_family(&mut self, family: impl Into<String>) {
        self.ops.push(RegistryOp::Unregister {
            family: family.into(),
        });
    }

    /// The family's lifecycle state; `None` when not registered.
    pub fn load_state(&self, family: &str) -> Option<FontLoadState> {
        self.families.get(family).map(|record| record.load_state)
    }

    /// The family's current fontdb face IDs (empty when not registered or
    /// not yet loaded). Valid for the current lineage only — never
    /// persisted (font-assets § 3.2).
    pub fn faces(&self, family: &str) -> &[fontdb::ID] {
        self.families
            .get(family)
            .map_or(&[], |record| &record.faces)
    }

    /// The descriptors declared at registration.
    pub fn descriptors(&self, family: &str) -> Option<&FontFaceDescriptors> {
        self.families.get(family).map(|record| &record.descriptors)
    }

    /// The `font-display: block` deadline for a still-Loading family
    /// (`loading_since` + [`FONT_BLOCK_TIMEOUT_SECS`]); `None` once loaded
    /// or failed. The resolver (Task 6) and block expiry (Task 7) consume.
    pub fn block_deadline(&self, family: &str) -> Option<f64> {
        let record = self.families.get(family)?;
        (record.load_state == FontLoadState::Loading)
            .then_some(record.loading_since + FONT_BLOCK_TIMEOUT_SECS)
    }

    /// The earliest still-open `font-display: block` deadline across
    /// `stack`'s walkable entries — Named entries up to the first Generic,
    /// which is terminal (the resolver's decision-7 walk never probes past
    /// it). This is exactly the per-family window the resolver consulted
    /// when it reported `blocked`; `TextSync` stamps it into
    /// [`PendingFontBlock::until`] (decision 9). Already-expired windows
    /// are excluded: an expired Block family degrades to Swap and must not
    /// shorten a still-open sibling's deadline.
    pub fn earliest_block_deadline(&self, stack: &FontStack, now: f64) -> Option<f64> {
        stack
            .0
            .iter()
            .map_while(|entry| match entry {
                FamilyEntry::Named(name) => Some(name.as_str()),
                FamilyEntry::Generic(_) => None,
            })
            .filter(|name| {
                self.descriptors(name)
                    .is_some_and(|descriptors| descriptors.font_display == FontDisplay::Block)
            })
            .filter_map(|name| self.block_deadline(name))
            .filter(|&until| now < until)
            .min_by(f64::total_cmp)
    }

    /// Iterate every loaded family's bytes — the system-scan swap's re-add
    /// source (T5 decision 6: re-added on the main thread at apply time,
    /// under the swap's own lock hold — an in-task re-add would lose fonts
    /// registered DURING the scan).
    pub(crate) fn loaded_sources(&self) -> impl Iterator<Item = (&str, &Arc<Vec<u8>>)> {
        self.families
            .iter()
            .filter_map(|(family, record)| Some((family.as_str(), record.data.as_ref()?)))
    }

    /// Re-record `family`'s faces after a fresh-db swap (every fontdb ID is
    /// reissued; `swap_font_db` calls this per re-added source).
    pub(crate) fn record_faces(&mut self, family: &str, faces: Vec<fontdb::ID>) {
        if let Some(record) = self.families.get_mut(family) {
            record.faces = faces;
        }
    }

    /// The declared families of every record backed by asset `id`. Plural:
    /// one handle may legitimately back several families (aliasing one font
    /// file as e.g. "Body" and "Heading"), and each `AssetEvent` must apply
    /// to ALL of them — completing only one would strand the rest in
    /// `Loading` forever (the event is consumed once).
    fn families_of_asset(&self, id: AssetId<BuiyFont>) -> Vec<String> {
        self.families
            .iter()
            .filter_map(|(family, record)| match &record.source {
                RegisteredSource::Asset(handle) if handle.id() == id => Some(family.clone()),
                _ => None,
            })
            .collect()
    }
}

/// CSS `font-display` block period (web default 3 s). Configurability is a
/// named seam (T5 plan honesty pins), not a resource knob.
pub const FONT_BLOCK_TIMEOUT_SECS: f64 = 3.0;

/// `font-display: block`'s paint-side marker (Task 7 consumes): inserted by
/// `TextSync` while a blocking family is loading inside its window; the
/// producer emits the entity's glyphs with zero alpha (identical fallback
/// LAYOUT, invisible paint — font-assets § 7). `until` = `loading_since`
/// + [`FONT_BLOCK_TIMEOUT_SECS`].
#[derive(Component, Clone, Copy, PartialEq, Debug)]
pub struct PendingFontBlock {
    pub until: f64,
}

/// `font-display: block`'s timeout (font-assets § 7: "until load or a
/// configurable timeout (web default: 3 s), then swap"): removing the
/// marker IS the swap-to-visible — the producer's `Changed`/Removed probes
/// repaint the entity's instances at full alpha, and layout never moves
/// (it has been the fallback family's all along). A load that beats the
/// deadline removes the marker through `TextSync` instead (the generation
/// sweep re-resolves to not-blocked).
pub fn expire_font_block(
    mut commands: Commands,
    time: Res<Time>,
    pending: Query<(Entity, &PendingFontBlock)>,
) {
    let now = time.elapsed_secs_f64();
    for (entity, block) in &pending {
        if now >= block.until {
            commands.entity(entity).remove::<PendingFontBlock>();
        }
    }
}

/// Apply staged registry ops + `AssetEvent<BuiyFont>` messages: in-place
/// adds, § 3.1 rebuild-removals (`Modified` = remove+re-add composed;
/// `Removed`/`Unused` = forced unregister), then ONE [`FontMatchIndex`]
/// re-snapshot + ONE `FontsGeneration` bump if the font set actually
/// changed. The lock is taken once per batch, only when there is work —
/// zero steady-state cost. `FontDbLineage` is NOT touched: every mutation
/// here is in-lineage by construction (the db is carried, never replaced).
pub fn apply_font_registry(
    mut registry: ResMut<FontRegistry>,
    fonts: Res<SharedFontSystem>,
    mut index: ResMut<FontMatchIndex>,
    mut generation: ResMut<FontsGeneration>,
    assets: Option<Res<Assets<BuiyFont>>>,
    mut events: MessageReader<AssetEvent<BuiyFont>>,
    time: Res<Time>,
) {
    if registry.ops.is_empty() && events.is_empty() {
        return;
    }
    let now = time.elapsed_secs_f64();
    // Faces leaving this batch (ONE rebuild covers them all) and sources
    // entering it (family → bytes; at most one entry per family).
    let mut dead: Vec<fontdb::ID> = Vec::new();
    let mut additions: Vec<(String, Arc<Vec<u8>>)> = Vec::new();

    // 1. Fold the AssetEvent stream (decision 5). Events can only carry
    //    work when the asset machinery exists; the reader stays registered
    //    headless via the plugin's unconditional add_message.
    if let Some(assets) = assets.as_deref() {
        for event in events.read() {
            match *event {
                AssetEvent::Added { id } | AssetEvent::LoadedWithDependencies { id } => {
                    // Complete every pending load backed by this asset. The
                    // Loading guard also swallows the echo of a load
                    // completed at op-drain time (the asset existed when
                    // RegisterAsset drained).
                    let Some(font) = assets.get(id) else { continue };
                    for family in registry.families_of_asset(id) {
                        if registry.families[&family].load_state != FontLoadState::Loading {
                            continue;
                        }
                        stage_addition(&mut additions, family, font.data.clone());
                    }
                }
                AssetEvent::Modified { id } => {
                    // Hot-reload every family backed by this asset:
                    // remove + re-add composed (font-assets § 2). A
                    // still-Loading record degenerates to plain completion
                    // (no faces to remove yet).
                    let Some(font) = assets.get(id) else { continue };
                    for family in registry.families_of_asset(id) {
                        let record = registry
                            .families
                            .get_mut(&family)
                            .expect("families_of_asset hit");
                        dead.append(&mut record.faces);
                        stage_addition(&mut additions, family, font.data.clone());
                    }
                }
                AssetEvent::Removed { id } | AssetEvent::Unused { id } => {
                    // The deliberate-unload arm (font-assets § 3): the
                    // registry pins its assets strong, so this only fires
                    // for a forced remove — honor it as an unregister of
                    // every family the asset backed.
                    for family in registry.families_of_asset(id) {
                        warn!(
                            "font family \"{family}\": its BuiyFont asset was removed — \
                             forcing unregister"
                        );
                        if let Some(record) = registry.families.remove(&family) {
                            dead.extend(record.faces);
                        }
                        additions.retain(|(staged, _)| *staged != family);
                    }
                }
            }
        }
    }

    // 2. Drain staged ops, in call order.
    for op in mem::take(&mut registry.ops) {
        match op {
            RegistryOp::RegisterAsset {
                family,
                handle,
                descriptors,
            } => {
                let bytes = assets
                    .as_deref()
                    .and_then(|assets| assets.get(&handle))
                    .map(|font| font.data.clone());
                replace_record(&mut registry, &mut dead, &mut additions, family.clone());
                registry.families.insert(
                    family.clone(),
                    FamilyRecord {
                        faces: Vec::new(),
                        descriptors,
                        load_state: FontLoadState::Loading,
                        source: RegisteredSource::Asset(handle),
                        data: None,
                        loading_since: now,
                    },
                );
                // Already loaded? Complete in this same batch (the
                // AssetEvent::Added echo is swallowed by the Loading
                // guard above next frame).
                if let Some(bytes) = bytes {
                    stage_addition(&mut additions, family, bytes);
                }
            }
            RegistryOp::RegisterBytes {
                family,
                bytes,
                descriptors,
            } => {
                replace_record(&mut registry, &mut dead, &mut additions, family.clone());
                registry.families.insert(
                    family.clone(),
                    FamilyRecord {
                        faces: Vec::new(),
                        descriptors,
                        // Flips to Loaded (or Failed) at the addition pass
                        // below — within this same batch.
                        load_state: FontLoadState::Loading,
                        source: RegisteredSource::Bytes,
                        data: None,
                        loading_since: now,
                    },
                );
                stage_addition(&mut additions, family, bytes);
            }
            RegistryOp::ReregisterBytes { family, bytes } => {
                let Some(record) = registry.families.get_mut(&family) else {
                    warn!("reregister_bytes(\"{family}\"): family is not registered — ignored");
                    continue;
                };
                dead.append(&mut record.faces);
                stage_addition(&mut additions, family, bytes);
            }
            RegistryOp::Unregister { family } => {
                match registry.families.remove(&family) {
                    Some(record) => dead.extend(record.faces),
                    None => {
                        warn!("unregister_family(\"{family}\"): family is not registered — ignored")
                    }
                }
                // A same-batch registration that this unregister revokes
                // must not enter the db.
                additions.retain(|(staged, _)| *staged != family);
            }
        }
    }

    if dead.is_empty() && additions.is_empty() {
        return;
    }

    // 3. ONE lock hold for the whole batch — a frame never measures
    //    against a half-registered family (font-assets § 3).
    let mut guard = fonts.lock();
    let mut db_changed = !dead.is_empty();
    if !dead.is_empty() {
        // The § 3.1 rebuild: the font_cache has no purge API — a fresh
        // FontSystem is the only way dead IDs stop resolving and their
        // Arc<Font>s drop. into_locale_and_db carries the SAME Database:
        // surviving IDs stay valid (in-lineage, FontDbLineage untouched).
        let old = mem::replace(&mut *guard, placeholder_font_system());
        let (locale, mut db) = old.into_locale_and_db();
        for id in &dead {
            db.remove_face(*id);
        }
        *guard = FontSystem::new_with_locale_and_db_and_fallback(locale, db, BuiyFallback);
    }
    for (family, bytes) in additions {
        let registry = &mut *registry;
        if !registry.families.contains_key(&family) {
            // Unregistered later in the same batch — the bytes never
            // enter the db.
            continue;
        }
        // In-place addition is safe: db_mut clears only the
        // font_matches_cache, and a new face can never make a match-cache
        // entry stale (Orientation).
        let ids = guard
            .db_mut()
            .load_font_source(fontdb::Source::Binary(bytes.clone()));
        db_changed |= !ids.is_empty();
        if ids.is_empty() {
            let record = registry.families.get_mut(&family).expect("checked above");
            record.load_state = FontLoadState::Failed;
            record.data = None;
            warn!(
                "font family \"{family}\": fontdb parsed no faces from the registered \
                 bytes — marked Failed"
            );
            continue;
        }
        // Decision 4: validate the declared name against the faces'
        // internal family names. The resolver queries by name, so a
        // mismatched registration will not match until the C-tier alias
        // seam (push_face_info, font-assets § 9) lands.
        let declared_matches = ids.iter().any(|id| {
            guard
                .db()
                .face(*id)
                .is_some_and(|face| face.families.iter().any(|(name, _)| name == &family))
        });
        if !declared_matches && registry.warned_mismatch.insert(family.clone()) {
            warn!(
                "font family \"{family}\": no loaded face declares that family name — \
                 stacks referencing \"{family}\" will NOT match it (the font-assets § 9 \
                 family-alias seam is the fix)"
            );
        }
        let record = registry.families.get_mut(&family).expect("checked above");
        record.faces = ids.to_vec();
        record.load_state = FontLoadState::Loaded;
        record.data = Some(bytes);
    }
    if db_changed {
        // T5 decision 2: the resolver's lock-free snapshot follows the
        // engine under the SAME batch (and the same lock hold).
        index.reset_in_lineage(guard.db().clone());
    }
    drop(guard);
    if db_changed {
        generation.0 += 1; // exactly once per batch
    }
}

/// Stage `family`'s bytes for the batch's addition pass, replacing any
/// earlier staged addition for the same family (last wins — fontdb never
/// dedups sources, so a double load would leak duplicate faces).
fn stage_addition(
    additions: &mut Vec<(String, Arc<Vec<u8>>)>,
    family: String,
    bytes: Arc<Vec<u8>>,
) {
    additions.retain(|(staged, _)| *staged != family);
    additions.push((family, bytes));
}

/// Re-registration replaces (CSS `@font-face`: last declaration wins): the
/// old record's faces leave with the batch's rebuild, and any addition the
/// replaced registration staged earlier in this batch is revoked.
fn replace_record(
    registry: &mut FontRegistry,
    dead: &mut Vec<fontdb::ID>,
    additions: &mut Vec<(String, Arc<Vec<u8>>)>,
    family: String,
) {
    if let Some(old) = registry.families.remove(&family) {
        dead.extend(old.faces);
        additions.retain(|(staged, _)| *staged != family);
    }
}
