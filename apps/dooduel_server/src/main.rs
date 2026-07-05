//! `dooduel_server` — the authoritative networked Dooduel server (spec §6).
//!
//! W4.1 lands the package + its dependency edges alone (the lockfile commit, house
//! rule); the accept loop, room registry, room actor, and WS wire tasks land in W4.3.

fn main() {
    // Placeholder — the real accept loop lands in W4.3 (spec §6.1).
    eprintln!("dooduel_server: not yet implemented (W4.3)");
    std::process::exit(1);
}
