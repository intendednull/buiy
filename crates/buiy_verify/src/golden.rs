//! Tier 5 — golden persistence + triage (verification-design `goldens.md`).
//!
//! The stored-baseline regression tier for the irreducible rasterization
//! residue Tiers 1–4 provably cannot reach: SDF corner AA, the drop-shadow
//! Gaussian kernel, glyph/color-emoji atlas output, the effect compositor,
//! blend/gamma, and the forced-colors *visual* residual. A `tests/goldens/`
//! corpus is keyed `widget × state × theme × viewport × backend × dpr`, with
//! **set-valued** (multi-positive) baselines so residual GPU AA jitter the
//! determinism pin reduces but cannot fully erase is absorbed by an
//! any-positive-matches semantics.
//!
//! ## What lives here (pure CPU, unit-testable without an adapter)
//!
//! * [`GoldenKey`] — the trace identity, **fixed before any golden is
//!   generated** (retrofitting a key field re-baselines the whole corpus). Its
//!   [`slug`](GoldenKey::slug) drives a stable on-disk path; [`from_slug`]
//!   parses it back.
//! * [`BlessLedger`] / [`Positive`] — the durable, human-diffable accept record
//!   (`<slug>.toml` beside the PNGs), recording, per positive, the blessing
//!   commit, timestamp, per-fixture budget, and reason. This is the explicit
//!   accept ledger reg-suit lacks (Skia-Gold §Borrow 1).
//! * [`check_golden`] / [`assert_golden`] — the comparison entry points
//!   (Phase 3.7).
//! * [`TriageReport`] / [`TriageCard`] — the self-contained offline HTML triage
//!   report (Phase 3.8).
//!
//! Capture (the one GPU-coupled primitive) is delegated to
//! [`buiy_core::render::golden::capture_to_image`]; everything in this module is
//! device-free.
//!
//! [`from_slug`]: GoldenKey::from_slug

use buiy_core::render::golden::Dpr;

mod check;
mod ledger;
mod report;

pub use check::{
    BlessMode, GoldenOutcome, assert_golden, assert_golden_in, check_golden, check_golden_in,
    committed_positives,
};
pub use ledger::{BlessLedger, Positive};
pub use report::{TriageCard, TriageReport};

/// The rasterizer a golden was captured on. One canonical rasterizer is pinned
/// per CI lane today (lavapipe), so a key currently carries a single constant
/// `backend`; the field is part of the trace identity now so a future
/// cross-backend corpus is a *new cell*, never a corpus-wide re-baseline
/// (Skia-Gold "params/traces"; goldens.md §58).
///
/// [`Cpu`](Self::Cpu) is the structured-tier marker: the coverage matrix keys
/// Tiers 1-3 (layout / display-list / invariant snapshots, no GPU) with it, so
/// a [`CoverageKey`](crate::coverage::CoverageKey) and a GPU [`GoldenKey`]
/// share one `Backend` enum (coverage.md §146).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum Backend {
    /// Software Vulkan (Mesa llvmpipe) — the pinned CI rasterizer.
    Lavapipe,
    /// Hardware Vulkan.
    Vulkan,
    /// OpenGL / GLES.
    Gl,
    /// Apple Metal.
    Metal,
    /// Direct3D 12.
    Dx12,
    /// No rasterizer — the structured CPU tiers (coverage Tiers 1-3). Never a
    /// golden capture backend; reserved so the CPU and GPU coverage cells key
    /// off the same enum.
    Cpu,
}

impl Backend {
    /// The lower-kebab slug component (the inverse of [`from_slug`](Self::from_slug)).
    fn slug(self) -> &'static str {
        match self {
            Backend::Lavapipe => "lavapipe",
            Backend::Vulkan => "vulkan",
            Backend::Gl => "gl",
            Backend::Metal => "metal",
            Backend::Dx12 => "dx12",
            Backend::Cpu => "cpu",
        }
    }

    /// Parse a slug component back to a `Backend` (the inverse of [`slug`](Self::slug)).
    fn from_slug(s: &str) -> Option<Self> {
        Some(match s {
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

/// The trace identity that keys a golden cell (Skia-Gold "params/traces";
/// goldens.md §47). **FIXED before any golden is generated** — adding a field
/// later re-baselines every stored PNG. The ordered fields drive a stable,
/// slug-safe on-disk path and the triage report.
///
/// `dpr` is the canonical [`buiy_core::render::golden::Dpr`] (integer
/// milliscale, `Eq + Hash + Ord`) — imported, never redefined here — so the key
/// compares/sorts/hashes without float pitfalls.
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub struct GoldenKey {
    /// Catalog fixture id (the BSN gallery entry — e.g. `button`).
    pub widget: String,
    /// Interaction state: `default | hover | focus | pressed | disabled`.
    pub state: String,
    /// `light | dark | high-contrast | forced-*`.
    pub theme: String,
    /// Named viewport (e.g. `sm` = 360×640).
    pub viewport: String,
    /// The rasterizer the golden was captured on (one pinned lane today).
    pub backend: Backend,
    /// Device-pixel-ratio as canonical milliscale (`Dpr::X1` = 1×, `X2` = 2×).
    pub dpr: Dpr,
}

/// The slug separator between the directory part (`widget/state/theme`) and the
/// flat key tail (`viewport__backend__dpr`). `__` is chosen so a single-`_`
/// inside a slug-safe component never splits a field.
const FIELD_SEP: &str = "__";

impl GoldenKey {
    /// `widget/state/theme__viewport__backend__dpr` — a directory per
    /// `widget/state/theme` keeps a fixture's whole row of cells together for
    /// review. Deterministic, lower-kebab, slug-safe (no raw `Debug`):
    /// components are lowercased and every run of non-`[a-z0-9]` collapses to a
    /// single `-`. The DPR renders as `dpr<milli/1000-ish>` via `dpr_slug`.
    pub fn slug(&self) -> String {
        format!(
            "{}/{}/{}{FIELD_SEP}{}{FIELD_SEP}{}{FIELD_SEP}{}",
            slug_component(&self.widget),
            slug_component(&self.state),
            slug_component(&self.theme),
            slug_component(&self.viewport),
            self.backend.slug(),
            dpr_slug(self.dpr),
        )
    }

    /// Parse a [`slug`](Self::slug) back into a key. `None` if the shape is
    /// wrong (not exactly `a/b/c` where `c` is `d__e__f__g`), the backend is
    /// unknown, or the dpr token is malformed. Round-trips any key whose
    /// components are already slug-safe (lower-kebab); display-name
    /// normalization (uppercasing/spaces) is lossy by design and not expected to
    /// round-trip.
    pub fn from_slug(slug: &str) -> Option<Self> {
        let mut dirs = slug.split('/');
        let widget = dirs.next()?.to_string();
        let state = dirs.next()?.to_string();
        let tail = dirs.next()?;
        if dirs.next().is_some() {
            return None; // too many `/` segments
        }
        let mut fields = tail.split(FIELD_SEP);
        let theme = fields.next()?.to_string();
        let viewport = fields.next()?.to_string();
        let backend = Backend::from_slug(fields.next()?)?;
        let dpr = dpr_from_slug(fields.next()?)?;
        if fields.next().is_some() {
            return None; // too many `__` fields
        }
        // Reject empty components — a valid key never has an empty field.
        if widget.is_empty() || state.is_empty() || theme.is_empty() || viewport.is_empty() {
            return None;
        }
        Some(GoldenKey {
            widget,
            state,
            theme,
            viewport,
            backend,
            dpr,
        })
    }

    /// The corpus directory holding `<slug-stem>.<n>.png` (n = positive index)
    /// plus the `<slug-stem>.toml` ledger. `root.join(self.slug())` — the slug
    /// IS a relative path (`widget/state/theme__…`).
    pub fn dir(&self, root: &std::path::Path) -> std::path::PathBuf {
        root.join(self.slug())
    }
}

/// Lowercase + collapse every run of non-`[a-z0-9]` to a single `-`, trimming
/// leading/trailing `-`. Makes a display name slug-safe; idempotent on
/// already-slug-safe input (so `slug()`→`from_slug` round-trips).
fn slug_component(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_dash = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            prev_dash = false;
        } else if !prev_dash {
            out.push('-');
            prev_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

/// Render a `Dpr` as a slug token: the common 1×/2× become `dpr1`/`dpr2`; any
/// other milliscale becomes `dprm<milli>` so it round-trips exactly (e.g.
/// `Dpr(1500)` → `dprm1500`). `dpr_from_slug` is the inverse.
fn dpr_slug(dpr: Dpr) -> String {
    let milli = dpr.0;
    if milli.is_multiple_of(1000) {
        format!("dpr{}", milli / 1000)
    } else {
        format!("dprm{milli}")
    }
}

/// Parse a `dpr_slug` token back to a `Dpr`. Accepts `dpr<n>` (= `n×1000`
/// milliscale) and `dprm<milli>` (raw milliscale).
fn dpr_from_slug(tok: &str) -> Option<Dpr> {
    if let Some(rest) = tok.strip_prefix("dprm") {
        Some(Dpr(rest.parse().ok()?))
    } else if let Some(rest) = tok.strip_prefix("dpr") {
        Some(Dpr(rest.parse::<u32>().ok()?.checked_mul(1000)?))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpr_slug_round_trips_common_and_fractional() {
        for d in [Dpr::X1, Dpr::X2, Dpr(1500), Dpr(1235), Dpr(3000)] {
            assert_eq!(dpr_from_slug(&dpr_slug(d)), Some(d), "round-trip for {d:?}");
        }
        assert_eq!(dpr_slug(Dpr::X1), "dpr1");
        assert_eq!(dpr_slug(Dpr::X2), "dpr2");
        assert_eq!(dpr_slug(Dpr(1500)), "dprm1500");
    }

    #[test]
    fn slug_component_is_slug_safe_and_idempotent() {
        assert_eq!(slug_component("Focus Ring"), "focus-ring");
        assert_eq!(slug_component("high-contrast"), "high-contrast"); // idempotent
        assert_eq!(slug_component("  weird__name  "), "weird-name");
    }
}
