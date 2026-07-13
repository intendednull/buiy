//! `dooduel_core` must stay Bevy-free (spec §2.1): the only Bevy-family dep is
//! `bevy_reflect` (plus its own transitive family), and no `buiy` crate may
//! appear. This tripwire runs `cargo tree` (metadata only, no build) so it rides
//! the normal workspace nextest gate on all 3 OSes.
//!
//! Matching is done on the **parsed crate name**, not the raw line: the tree's
//! root line embeds this repo's absolute path (which lives under
//! `…/projects/buiy/…`), so a naive `line.contains("buiy")` would false-positive
//! on every path-bearing line. The bevy allow-list is the EXACT transitive set
//! `bevy_reflect` pulls, verified against `cargo tree -p dooduel_core -e normal`
//! at W0 implementation time (the plan's guessed subset omitted
//! `bevy_reflect_derive` + `bevy_macro_utils`).

/// The exact Bevy-family crates `bevy_reflect` drags in (its derive macro, macro
/// utils, platform/ptr/util shims). Anything else starting with `bevy` is a leak.
const ALLOWED_BEVY: &[&str] = &[
    "bevy_reflect",        // the one kept Bevy-family dep (spec §2.1)
    "bevy_reflect_derive", // its `#[derive(Reflect)]` proc-macro
    "bevy_macro_utils",    // used by bevy_reflect_derive
    "bevy_platform",       // bevy_reflect's platform shim
    "bevy_ptr",            // bevy_reflect's pointer utilities
    "bevy_utils",          // bevy_reflect's util crate
];

/// Parse the crate name from a `cargo tree` line: strip the leading tree-drawing
/// glyphs/whitespace, then take the token before the ` v<version>` field. Crate
/// names begin with an ASCII alphanumeric and contain no spaces.
fn crate_name(line: &str) -> &str {
    line.trim_start_matches(|c: char| !c.is_ascii_alphanumeric())
        .split_whitespace()
        .next()
        .unwrap_or("")
}

#[test]
fn dep_tree_is_bevy_free() {
    let out = std::process::Command::new(env!("CARGO"))
        .args(["tree", "-p", "dooduel_core", "-e", "normal", "--locked"])
        .output()
        .expect("cargo tree runs");
    assert!(
        out.status.success(),
        "cargo tree failed: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    let tree = String::from_utf8_lossy(&out.stdout);
    for line in tree.lines() {
        let name = crate_name(line);
        assert!(
            !name.starts_with("buiy"),
            "buiy crate leaked into dooduel_core: {line}"
        );
        if name.starts_with("bevy") {
            assert!(
                ALLOWED_BEVY.contains(&name),
                "non-reflect bevy dep leaked into dooduel_core: {line}"
            );
        }
    }
}
