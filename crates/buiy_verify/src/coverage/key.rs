//! The shared coverage key — `Cell × Fixture` (coverage.md § "The Matrix").
//!
//! A [`CoverageKey`] is the trace identity for one enrolled combination: a
//! fixture (`widget × state`) crossed with one [`Cell`] of
//! the global [`Matrix`](super::matrix::Matrix) (theme × viewport ×
//! forced-colors × dpr), plus the rasterizer [`Backend`]. It is exactly the
//! contract's storage schema and Skia Gold's params/traces identity
//! (`prior-art/skia-gold/lessons.md` §Borrow.2,
//! `(widget, state, theme, viewport, backend, dpr)`).
//!
//! `dpr` is the canonical [`buiy_core::render::golden::Dpr`] (integer
//! milliscale, `Eq + Hash + Ord`) — imported, never redefined — so
//! `CoverageKey` itself derives `Eq + Hash` and the `verify_keys_unique`
//! self-test can collect the keys (not just their stems) into a `HashSet`. The
//! old `dpr: f32` design made this impossible (`f32` is neither `Eq` nor
//! `Hash`); that is the bug this milliscale type unblocks.

use buiy_core::render::golden::Dpr;

use super::fixture::Fixture;
use super::matrix::Cell;

// The golden tier already owns the `Backend` enum (the rasterizer a capture ran
// on). Coverage reuses it verbatim — a key's `backend` is `cpu` for the
// structured CPU tiers (Tiers 1-3) and the rasterizer name for the GPU golden
// tier — so a future cross-backend corpus is a NEW cell, never a corpus-wide
// re-baseline (`prior-art/skia-gold/lessons.md` §Avoid).
pub use crate::golden::Backend;

/// One enrolled combination's identity: the fixture (`widget × state`) crossed
/// with one [`Cell`] of the global matrix, plus the [`Backend`].
///
/// Derives `Eq + Hash` because every field is `Eq + Hash` — crucially `dpr` is
/// the canonical milliscale [`Dpr`], not an `f32`. This lets the keys
/// themselves collect into a `HashSet` for the duplicate-detection self-test.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CoverageKey {
    /// Stable fixture id ([`Fixture::name`]), e.g. `button`.
    pub widget: &'static str,
    /// Per-fixture interaction state ([`Fixture::state`]), e.g. `resting`.
    pub state: &'static str,
    /// Theme axis key (`light` | `forced`), from [`ThemeAxis::key`].
    ///
    /// [`ThemeAxis::key`]: super::matrix::ThemeAxis::key
    pub theme: &'static str,
    /// Named viewport key (`phone` | `tablet` | `desktop`), from
    /// [`Viewport::key`](super::matrix::Viewport::key).
    pub viewport: &'static str,
    /// The forced-colors **mode** axis (`false` | `true`).
    pub forced_colors: bool,
    /// Device-pixel-ratio as canonical milliscale (`Dpr::X1` = 1×, `X2` = 2×).
    pub dpr: Dpr,
    /// The rasterizer the cell targets (`cpu` for Tiers 1-3, the GPU
    /// rasterizer name for the golden tier).
    pub backend: Backend,
}

impl CoverageKey {
    /// Build the key for `fx` crossed with `cell`, captured on `backend`.
    pub fn for_cell(fx: &Fixture, cell: &Cell, backend: Backend) -> Self {
        Self {
            widget: fx.name,
            state: fx.state,
            theme: cell.theme.key(),
            viewport: cell.viewport.key,
            forced_colors: cell.forced_colors,
            dpr: cell.dpr,
            backend,
        }
    }

    /// Canonical filename stem — stable, lossless, ordered. Drives the golden
    /// PNG stem and the `insta` snapshot suffix
    /// (`assert_snapshot!(key.stem(), …)`). Example:
    /// `button.resting.forced.desktop.fc1.dpr2.lavapipe`.
    ///
    /// Lossless + ordered means it round-trips (`from_stem(stem()) == self`) so
    /// a collision in the self-test is a real two-cells-share-a-baseline bug,
    /// not a stem-collision artifact. Retrofitting the field order means
    /// re-baselining everything (`prior-art/skia-gold/lessons.md` §Avoid), so
    /// the order is fixed now.
    pub fn stem(&self) -> String {
        format!(
            "{}.{}.{}.{}.{}.{}.{}",
            self.widget,
            self.state,
            self.theme,
            self.viewport,
            fc_token(self.forced_colors),
            dpr_token(self.dpr),
            backend_token(self.backend),
        )
    }

    /// Parse a [`stem`](Self::stem) back into a key (the inverse). `None` if the
    /// shape is wrong, the forced-colors / dpr / backend token is malformed, or
    /// any field is empty.
    ///
    /// The `widget`/`state`/`theme`/`viewport`/`backend` fields are `'static`
    /// strings in the live type but parse out of an owned `String` here; the
    /// round-trip self-test therefore compares the **stems** (lossless), not the
    /// borrowed keys — `from_stem(k.stem()).stem() == k.stem()`. That is the
    /// identity the duplicate-baseline guard needs (two cells collide iff their
    /// stems collide).
    pub fn from_stem(stem: &str) -> Option<ParsedStem> {
        let mut parts = stem.split('.');
        let widget = nonempty(parts.next()?)?;
        let state = nonempty(parts.next()?)?;
        let theme = nonempty(parts.next()?)?;
        let viewport = nonempty(parts.next()?)?;
        let forced_colors = fc_from_token(parts.next()?)?;
        let dpr = dpr_from_token(parts.next()?)?;
        let backend = Backend::from_stem_token(parts.next()?)?;
        if parts.next().is_some() {
            return None; // too many `.` segments
        }
        Some(ParsedStem {
            widget: widget.to_string(),
            state: state.to_string(),
            theme: theme.to_string(),
            viewport: viewport.to_string(),
            forced_colors,
            dpr,
            backend,
        })
    }
}

/// The owned-string twin of [`CoverageKey`] produced by
/// [`CoverageKey::from_stem`]. Distinct from `CoverageKey` because the live key
/// borrows `'static` fixture/axis identifiers while a parsed stem owns its
/// components. [`stem`](Self::stem) recomputes the canonical form, so a
/// round-trip is asserted on the stems (lossless), not the borrowed type.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct ParsedStem {
    pub widget: String,
    pub state: String,
    pub theme: String,
    pub viewport: String,
    pub forced_colors: bool,
    pub dpr: Dpr,
    pub backend: Backend,
}

impl ParsedStem {
    /// Recompute the canonical stem from the parsed components (the inverse of
    /// the inverse — used to assert `from_stem` round-trips losslessly).
    pub fn stem(&self) -> String {
        format!(
            "{}.{}.{}.{}.{}.{}.{}",
            self.widget,
            self.state,
            self.theme,
            self.viewport,
            fc_token(self.forced_colors),
            dpr_token(self.dpr),
            backend_token(self.backend),
        )
    }
}

/// `fc0` / `fc1` — the forced-colors mode token (Chromatic-style: each mode
/// gets its own baseline, so it is part of the stem, not collapsed away).
fn fc_token(fc: bool) -> &'static str {
    if fc { "fc1" } else { "fc0" }
}

fn fc_from_token(tok: &str) -> Option<bool> {
    match tok {
        "fc1" => Some(true),
        "fc0" => Some(false),
        _ => None,
    }
}

/// `dpr1` / `dpr2` for the common integer ratios; `dprm<milli>` otherwise so any
/// milliscale round-trips exactly (e.g. `Dpr(1500)` → `dprm1500`). Mirrors the
/// golden slug's `dpr_slug` so the two key schemas agree on the DPR token.
fn dpr_token(dpr: Dpr) -> String {
    let milli = dpr.0;
    if milli.is_multiple_of(1000) {
        format!("dpr{}", milli / 1000)
    } else {
        format!("dprm{milli}")
    }
}

fn dpr_from_token(tok: &str) -> Option<Dpr> {
    if let Some(rest) = tok.strip_prefix("dprm") {
        Some(Dpr(rest.parse().ok()?))
    } else if let Some(rest) = tok.strip_prefix("dpr") {
        Some(Dpr(rest.parse::<u32>().ok()?.checked_mul(1000)?))
    } else {
        None
    }
}

/// The lower-kebab backend token, mirroring `golden::Backend::slug`.
fn backend_token(b: Backend) -> &'static str {
    match b {
        Backend::Lavapipe => "lavapipe",
        Backend::Vulkan => "vulkan",
        Backend::Gl => "gl",
        Backend::Metal => "metal",
        Backend::Dx12 => "dx12",
        Backend::Cpu => "cpu",
    }
}

/// Parse a backend stem token, the inverse of [`backend_token`].
trait BackendStem {
    fn from_stem_token(tok: &str) -> Option<Backend>;
}
impl BackendStem for Backend {
    fn from_stem_token(tok: &str) -> Option<Backend> {
        Some(match tok {
            "lavapipe" => Backend::Lavapipe,
            "vulkan" => Backend::Vulkan,
            "gl" => Backend::Gl,
            "metal" => Backend::Metal,
            "dx12" => Backend::Dx12,
            "cpu" => Backend::Cpu,
            _ => return None,
        })
    }
}

fn nonempty(s: &str) -> Option<&str> {
    (!s.is_empty()).then_some(s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::coverage::matrix::{ThemeAxis, Viewport};

    fn sample_cell(fc: bool, dpr: Dpr) -> Cell {
        Cell {
            theme: ThemeAxis::ForcedColors,
            viewport: Viewport {
                w: 1280,
                h: 800,
                key: "desktop",
            },
            forced_colors: fc,
            dpr,
        }
    }

    fn sample_fixture() -> Fixture {
        Fixture {
            name: "button",
            state: "resting",
            spawn: |_| {},
        }
    }

    #[test]
    fn stem_matches_documented_example() {
        let key = CoverageKey::for_cell(
            &sample_fixture(),
            &sample_cell(true, Dpr::X2),
            Backend::Lavapipe,
        );
        assert_eq!(
            key.stem(),
            "button.resting.forced.desktop.fc1.dpr2.lavapipe"
        );
    }

    #[test]
    fn stem_round_trips_through_from_stem() {
        for fc in [false, true] {
            for dpr in [Dpr::X1, Dpr::X2, Dpr(1500)] {
                for backend in [Backend::Cpu, Backend::Lavapipe] {
                    let key =
                        CoverageKey::for_cell(&sample_fixture(), &sample_cell(fc, dpr), backend);
                    let stem = key.stem();
                    let parsed = CoverageKey::from_stem(&stem)
                        .unwrap_or_else(|| panic!("from_stem failed for {stem}"));
                    assert_eq!(parsed.stem(), stem, "stem must round-trip for {stem}");
                }
            }
        }
    }

    #[test]
    fn from_stem_rejects_malformed() {
        assert!(CoverageKey::from_stem("too.few.parts").is_none());
        assert!(CoverageKey::from_stem("a.b.c.d.fcX.dpr1.cpu").is_none()); // bad fc
        assert!(CoverageKey::from_stem("a.b.c.d.fc0.nope.cpu").is_none()); // bad dpr
        assert!(CoverageKey::from_stem("a.b.c.d.fc0.dpr1.bogus").is_none()); // bad backend
        assert!(CoverageKey::from_stem("a..c.d.fc0.dpr1.cpu").is_none()); // empty field
    }

    #[test]
    fn key_is_eq_hash_collectible() {
        // The milliscale payoff: keys (not just stems) collect into a HashSet.
        use std::collections::HashSet;
        let k1 = CoverageKey::for_cell(
            &sample_fixture(),
            &sample_cell(false, Dpr::X1),
            Backend::Cpu,
        );
        let k2 = CoverageKey::for_cell(
            &sample_fixture(),
            &sample_cell(false, Dpr::X2),
            Backend::Cpu,
        );
        let set: HashSet<CoverageKey> = [k1, k2].into_iter().collect();
        assert_eq!(set.len(), 2, "distinct dpr → distinct keys");
    }
}
