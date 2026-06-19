//! Task 2.1 — shared dump primitives: `round` + format-version headers.
//! Plain `assert_eq!` (NOT a snapshot) so this meta-test of the snapshot tooling
//! cannot pass vacuously (snapshots.md § Verification #2).

use buiy_verify::snapshot::{DISPLAY_LIST_DUMP_VERSION, LAYOUT_DUMP_VERSION, ROUND_DP, round};

#[test]
fn round_table() {
    // ROUND_DP = 2: round to 2 decimals, then strip trailing zeros / the
    // trailing dot so the dump stays diff-readable and last-ULP-stable.
    // sub-ULP + negative inputs (snapshots.md § Verification #2).
    assert_eq!(ROUND_DP, 2);
    // snapshots.md § Verification #2 lists `round(1.005) == "1.0"`, but that
    // vector is self-inconsistent with `round(50.0) == "50"`: `1.005_f32` is
    // `1.00499…`, which formats to `"1.00"` at 2 dp — byte-identical suffix to
    // `50.0`'s `"50.00"`, so ONE trailing-zero rule cannot strip one to `"1.0"`
    // and the other to `"50"`. We strip ALL trailing zeros (the only
    // self-consistent rule). The vector's INTENT — proving `1.005` rounds DOWN
    // to 1.00, never up to 1.01 — is fully preserved by `"1"`.
    assert_eq!(round(1.005), "1"); // rounds to 1.00 (NOT 1.01), then strips
    assert_eq!(round(50.0), "50"); // integral value drops the ".0"
    assert_eq!(round(-0.001), "0"); // sub-ULP negative collapses to "0" (no "-0")
    assert_eq!(round(0.0), "0");
    assert_eq!(round(-0.0), "0"); // negative zero normalizes to "0"
    assert_eq!(round(50.5), "50.5");
    assert_eq!(round(50.567), "50.57"); // rounds at the 2nd decimal
    assert_eq!(round(-12.34), "-12.34");
    assert_eq!(round(100.0), "100");
}

#[test]
fn version_headers_are_stable_constants() {
    // The format-version tripwire (snapshots.md § Verification #4): a formatter
    // edit that should bump the version but didn't fails the dump header tests
    // (2.2/2.4); these pin the literal strings the dumps emit as line 1.
    assert_eq!(LAYOUT_DUMP_VERSION, "# buiy-layout-dump v1");
    assert_eq!(DISPLAY_LIST_DUMP_VERSION, "# buiy-display-list-dump v1");
}
