//! The bless ledger — the durable, human-diffable accept record (`goldens.md`
//! § "The bless ledger"). One `<slug-stem>.toml` lives beside each key's
//! `<slug-stem>.<n>.png` positives, recording *why* each positive was accepted:
//! the blessing commit, an RFC3339 timestamp, the per-fixture budget, and a
//! one-line reason. This is the explicit, reviewable accept ledger reg-suit
//! lacks (Skia-Gold §Borrow 1) — a real regression is caught in the PR diff of
//! this file, not buried in git history.

use super::GoldenKey;
use crate::metric::FuzzBudget;

/// The `<slug-stem>.toml` accept ledger for one [`GoldenKey`]: the key itself
/// (so the file is self-describing) plus its set of accepted positives. Index
/// `i` in `positives` corresponds on disk to `<slug-stem>.i.png`.
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct BlessLedger {
    /// The trace identity this ledger records positives for.
    pub key: GoldenKey,
    /// The accepted baselines, in bless order. `positives[i]` ⇒ `<stem>.i.png`.
    pub positives: Vec<Positive>,
}

/// One accepted baseline. Records the provenance a reviewer needs to judge
/// whether a positive is still legitimate (the stale-positive guard,
/// goldens.md § "Stale-positive guard").
#[derive(Clone, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Positive {
    /// PNG filename relative to the ledger (`<slug-stem>.<n>.png`).
    pub file: String,
    /// `git rev-parse HEAD` at bless time — pins the source state that produced
    /// this pixel set.
    pub blessed_commit: String,
    /// RFC3339 timestamp the positive was blessed.
    pub blessed_at: String,
    /// The budget this positive is asserted against — `(0,0)` after the
    /// determinism pin, widened per-fixture with a documented [`reason`](Self::reason).
    pub budget: FuzzBudget,
    /// Why this positive exists (or why its budget was widened).
    pub reason: String,
}

impl BlessLedger {
    /// An empty ledger for `key` (no positives yet). The first bless pushes
    /// `<stem>.0.png`.
    pub fn empty(key: GoldenKey) -> Self {
        Self {
            key,
            positives: Vec::new(),
        }
    }

    /// Load the ledger from `path`, or return an [`empty`](Self::empty) one for
    /// `key` if the file does not exist. Propagates a real read/parse error (a
    /// corrupt ledger must surface loudly, never silently reset the corpus).
    pub fn load_or_empty(path: &std::path::Path, key: &GoldenKey) -> std::io::Result<Self> {
        match std::fs::read_to_string(path) {
            Ok(s) => toml::from_str(&s)
                .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::empty(key.clone())),
            Err(e) => Err(e),
        }
    }

    /// Serialize to human-diffable TOML and write to `path` (creating parent
    /// directories). The written file is what a reviewer reads in the PR diff.
    pub fn save(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let body = toml::to_string_pretty(self)
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::InvalidData, e))?;
        std::fs::write(path, body)
    }
}
