//! The lock-in containment tripwire (editing-and-ime § 2.1): the cosmic
//! `Editor`/`Edit`/`Action`/`Change` types are named ONLY inside
//! `crates/buiy_core/src/text/edit/`. A leak anywhere else widens the
//! bridge surface the bevy-cosmic-edit post-mortem warns against — this test
//! fails the build the moment it happens. Greps SOURCE, not the running app:
//! the boundary is a compile-time architectural fact, so a source scan is
//! the right tier (it needs no World, no plugins, no adapter).

use std::path::{Path, PathBuf};

/// The four cosmic editor type identifiers the facade contains. Matched as
/// WHOLE WORDS (word-boundary), NOT substrings — because this codebase uses
/// grouped imports pervasively (`use cosmic_text::{Buffer, Editor};`,
/// e.g. `sync.rs:29`, `extract.rs:19`, `components.rs:10`). A substring
/// match on `"cosmic_text::Editor"` would MISS `cosmic_text::{Buffer,
/// Editor}` entirely — a real leak passing silently. The scan normalizes
/// grouped-import braces first (below), then checks bare identifiers.
///
/// `Edit` is special-cased: as a bare word it collides with the prefix of
/// `Editor`/`EditState`/`TextEditState` and with `edit` in paths, so it is
/// ONLY flagged in a `cosmic_text::`-qualified position (the normalized form
/// makes that exact). The facade subtree (`text/edit/`) is exempt entirely.
const FORBIDDEN: &[&str] = &["Editor", "Edit", "Action", "Change"];

fn src_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// Recursively collect `.rs` files under `dir`, skipping the `text/edit/`
/// facade subtree (the one place these types are allowed).
fn rust_files_outside_facade(dir: &Path, facade: &Path, out: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            if path == facade {
                continue; // the facade is exempt by definition
            }
            rust_files_outside_facade(&path, facade, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

/// Expand grouped `cosmic_text::{A, B, C}` imports into the flat
/// `cosmic_text::A, cosmic_text::B, cosmic_text::C` form, so a single
/// substring rule (`cosmic_text::<Ident>` as a whole word) catches BOTH the
/// single-import (`cosmic_text::Editor`) and grouped-import
/// (`cosmic_text::{Buffer, Editor}`) shapes. Conservative: only rewrites the
/// first `cosmic_text::{ … }` on a line (the codebase never nests two on one
/// line); anything else passes through unchanged.
fn normalize_grouped_imports(line: &str) -> String {
    let Some(brace_start) = line.find("cosmic_text::{") else {
        return line.to_string();
    };
    let inner_start = brace_start + "cosmic_text::{".len();
    let Some(rel_end) = line[inner_start..].find('}') else {
        return line.to_string(); // unterminated (multi-line group) — leave it
    };
    let inner = &line[inner_start..inner_start + rel_end];
    // Each comma-separated entry, re-qualified. Strips `as Alias` and
    // whitespace; an entry like `Edit` becomes `cosmic_text::Edit`.
    let expanded: Vec<String> = inner
        .split(',')
        .map(|e| e.split_whitespace().next().unwrap_or("").trim())
        .filter(|e| !e.is_empty())
        .map(|e| format!("cosmic_text::{e}"))
        .collect();
    // Rebuild: prefix + expanded list + suffix-after-`}`.
    let suffix = &line[inner_start + rel_end + 1..];
    format!("{}{} {}", &line[..brace_start], expanded.join(", "), suffix)
}

/// True if `line` names `cosmic_text::<ident>` where `<ident>` is `needle`
/// as a WHOLE word — the next char after `needle` must not be an
/// identifier char (so `cosmic_text::Editor` does not match needle `Edit`,
/// and `cosmic_text::Edit` does match it; `cosmic_text::Editor` matches
/// needle `Editor`).
fn names_cosmic_type(line: &str, needle: &str) -> bool {
    let pat = format!("cosmic_text::{needle}");
    let mut from = 0;
    while let Some(rel) = line[from..].find(&pat) {
        let after = from + rel + pat.len();
        let boundary = line[after..]
            .chars()
            .next()
            .is_none_or(|c| !c.is_alphanumeric() && c != '_');
        if boundary {
            return true;
        }
        from = after;
    }
    false
}

#[test]
fn no_cosmic_editor_types_outside_the_facade() {
    let src = src_dir();
    let facade = src.join("text").join("edit");
    assert!(facade.is_dir(), "the text::edit facade must exist");
    let mut files = Vec::new();
    rust_files_outside_facade(&src, &facade, &mut files);
    assert!(!files.is_empty(), "scanned at least one source file");

    let mut leaks = Vec::new();
    for file in &files {
        let body = std::fs::read_to_string(file).unwrap();
        for (lineno, line) in body.lines().enumerate() {
            // Ignore comments and doc lines: the boundary is about CODE
            // naming the type, and the codebase documents `TextEditState`
            // wrapping `Editor` in prose all over (e.g. components.rs).
            let trimmed = line.trim_start();
            if trimmed.starts_with("//") || trimmed.starts_with("*") {
                continue;
            }
            let normalized = normalize_grouped_imports(line);
            for needle in FORBIDDEN {
                if names_cosmic_type(&normalized, needle) {
                    leaks.push(format!(
                        "{}:{}: {}",
                        file.display(),
                        lineno + 1,
                        line.trim()
                    ));
                }
            }
        }
    }
    assert!(
        leaks.is_empty(),
        "cosmic editor types leaked outside text::edit (facade boundary, \
         editing-and-ime § 2.1):\n{}",
        leaks.join("\n"),
    );
}
