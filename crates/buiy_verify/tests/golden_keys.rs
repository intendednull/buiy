//! Tier-5 golden key schema self-tests (Phase 3.6, verification-design
//! `goldens.md` § Verification #6). Pure-CPU, headless — no GPU adapter.
//!
//! The `GoldenKey` trace identity is **fixed before any golden is generated**
//! (Skia-Gold lesson — retrofitting a key field re-baselines the whole corpus).
//! These tests pin that the key:
//!   * slugs deterministically (lower-kebab, stable field order),
//!   * round-trips through `slug()` → parse,
//!   * never collides two distinct keys onto one slug, and
//!   * the bless ledger serializes to human-diffable TOML.

use buiy_core::render::golden::Dpr;
use buiy_verify::golden::{Backend, BlessLedger, GoldenKey, Positive};
use buiy_verify::metric::FuzzBudget;
use proptest::prelude::*;

#[allow(clippy::too_many_arguments)]
fn key(
    widget: &str,
    state: &str,
    theme: &str,
    viewport: &str,
    forced_colors: bool,
    backend: Backend,
    dpr: Dpr,
) -> GoldenKey {
    GoldenKey {
        widget: widget.into(),
        state: state.into(),
        theme: theme.into(),
        viewport: viewport.into(),
        forced_colors,
        backend,
        dpr,
    }
}

#[test]
fn slug_is_deterministic_lower_kebab() {
    let k = key("button", "hover", "dark", "sm", false, Backend::Lavapipe, Dpr::X2);
    // Stable schema: `widget/state/theme__viewport__fc__backend__dpr`.
    assert_eq!(k.slug(), "button/hover/dark__sm__fc0__lavapipe__dpr2");
    // Deterministic: the same key slugs identically every call.
    assert_eq!(k.slug(), k.slug());
}

#[test]
fn forced_colors_mode_is_a_distinct_baseline() {
    // The forced-colors axis is the trap the coverage matrix exists to cover:
    // the same theme renders differently with forced-colors on, so the two
    // modes MUST get separate slugs (else a regression in one passes against
    // the other's baseline).
    let off = key("button", "default", "forced", "md", false, Backend::Lavapipe, Dpr::X1);
    let on = key("button", "default", "forced", "md", true, Backend::Lavapipe, Dpr::X1);
    assert_ne!(off, on);
    assert_ne!(
        off.slug(),
        on.slug(),
        "fc0 and fc1 must get separate slugs — dropping the axis collapses two captures"
    );
    assert!(off.slug().contains("fc0") && on.slug().contains("fc1"));
}

#[test]
fn slug_lowercases_and_kebabs_mixed_case_input() {
    let k = key(
        "ToggleSwitch",
        "Focus Ring",
        "High Contrast",
        "Large XL",
        false,
        Backend::Vulkan,
        Dpr::X1,
    );
    let slug = k.slug();
    assert_eq!(
        slug, "toggleswitch/focus-ring/high-contrast__large-xl__fc0__vulkan__dpr1",
        "slug must be lower-kebab + slug-safe (no spaces, no raw Debug)"
    );
    // Slug-safe: no whitespace, no uppercase.
    assert!(!slug.chars().any(|c| c.is_whitespace()));
    assert!(!slug.chars().any(|c| c.is_ascii_uppercase()));
}

#[test]
fn dir_places_corpus_under_widget_directory() {
    let root = std::path::Path::new("/tmp/goldens");
    let k = key(
        "button",
        "default",
        "light",
        "md",
        false,
        Backend::Lavapipe,
        Dpr::X1,
    );
    let dir = k.dir(root);
    // The whole row of a fixture's cells lives under one directory per widget
    // (Skia-Gold review ergonomics).
    assert!(dir.starts_with(root));
    assert!(
        dir.ends_with("button/default/light__md__fc0__lavapipe__dpr1"),
        "dir = root.join(slug); got {dir:?}"
    );
}

#[test]
fn ledger_round_trips_through_toml() {
    let k = key("button", "hover", "dark", "sm", false, Backend::Lavapipe, Dpr::X2);
    let ledger = BlessLedger {
        key: k.clone(),
        positives: vec![Positive {
            file: "button/hover/dark__sm__fc0__lavapipe__dpr2.0.png".into(),
            blessed_commit: "deadbeef".into(),
            blessed_at: "2026-06-15T00:00:00Z".into(),
            budget: FuzzBudget::EXACT,
            reason: "initial bless".into(),
        }],
    };
    let serialized = toml::to_string(&ledger).expect("ledger serializes to TOML");
    // Human-diffable: a reviewer reads the commit/reason in the PR diff.
    assert!(serialized.contains("deadbeef"));
    assert!(serialized.contains("initial bless"));
    let parsed: BlessLedger = toml::from_str(&serialized).expect("ledger round-trips");
    assert_eq!(parsed.key, k);
    assert_eq!(parsed.positives.len(), 1);
    assert_eq!(parsed.positives[0].budget, FuzzBudget::EXACT);
}

// ---------------------------------------------------------------------------
// goldens.md § Verification #6: a GoldenKey round-trips through slug()→parse,
// and two distinct keys never collide on a slug.
// ---------------------------------------------------------------------------

// A canonical (already slug-safe) component: lower-alnum runs joined by single
// dashes, no leading/trailing/double dash. The round-trip contract holds for
// canonical components — `slug_component` is idempotent on them and `from_slug`
// is its exact inverse. Non-canonical display names (spaces, mixed case,
// trailing dashes) are a lossy normalization concern, covered by the
// lower-kebab unit test above, not by the round-trip property.
fn arb_component() -> impl Strategy<Value = String> {
    prop::collection::vec("[a-z0-9]{1,5}", 1..=3).prop_map(|parts| parts.join("-"))
}

prop_compose! {
    fn arb_key()(
        widget in arb_component(),
        state in arb_component(),
        theme in arb_component(),
        viewport in arb_component(),
        forced_colors in prop::bool::ANY,
        backend in prop::sample::select(vec![
            Backend::Lavapipe, Backend::Vulkan, Backend::Gl, Backend::Metal, Backend::Dx12,
        ]),
        dpr_milli in 1u32..=4000,
    ) -> GoldenKey {
        key(&widget, &state, &theme, &viewport, forced_colors, backend, Dpr(dpr_milli))
    }
}

proptest! {
    #[test]
    fn key_slug_round_trips(k in arb_key()) {
        let slug = k.slug();
        let parsed = GoldenKey::from_slug(&slug)
            .unwrap_or_else(|| panic!("slug `{slug}` failed to parse back"));
        prop_assert_eq!(parsed, k);
    }

    #[test]
    fn distinct_keys_never_collide(a in arb_key(), b in arb_key()) {
        if a != b {
            prop_assert_ne!(a.slug(), b.slug(), "distinct keys collided on a slug");
        }
    }
}
