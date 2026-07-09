//! The optional `dooduel_server.toml` configuration — the room/game knobs.
//!
//! Path resolution: `--config <path>` > `DOODUEL_CONFIG` > `dooduel_server.toml` in the
//! working directory. The file is **optional and non-fatal**: a missing default file, an
//! unreadable explicit file, or a parse error all fall back to [`Config::default()`] — the
//! server never fails to start over its config. Every field is optional too (serde
//! `default`), each defaulting to the matching `Config::default()` value, so a file may
//! set only the knobs it cares about.
//!
//! The knobs exist because the spec leaves the phase durations / round count / bots a
//! `Config` knob (`dooduel_core::game::Config` doc): the M1 multi-agent acceptance run
//! widens the timers for slow file-protocol agents and turns the built-in bot guessers OFF
//! so every seat is agent/human-driven.

use std::path::PathBuf;

use dooduel_core::game::Config;
use serde::Deserialize;

/// The TOML document root. Only a `[room]` table today; `deny_unknown_fields` turns a typo
/// (a stray top-level key) into an error rather than a silently-ignored setting.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ServerConfig {
    pub room: RoomConfig,
}

/// The `[room]` table — the game [`Config`] knobs, each optional (omitted ⇒ the default).
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct RoomConfig {
    /// Rounds per match (each seated player draws once per round).
    pub rounds: u32,
    /// Seconds the drawer has to draw before the turn auto-ends.
    pub draw_seconds: u64,
    /// Seconds to pick a word before one is auto-chosen (the safety timeout).
    pub pick_seconds: u64,
    /// Seconds the turn-end reveal card stays up before the next turn.
    pub reveal_seconds: u64,
    /// Letters progressively revealed as hints during a drawing turn.
    pub hints: usize,
    /// The built-in bot guessers. `false` ⇒ every seat is agent/human-driven (the
    /// acceptance run); the auto-pick safety timeout still runs.
    pub bots: bool,
}

impl Default for RoomConfig {
    fn default() -> Self {
        // Mirror `Config::default()` so an omitted field keeps the game's own default.
        let c = Config::default();
        RoomConfig {
            rounds: c.total_rounds,
            draw_seconds: c.draw_seconds,
            pick_seconds: c.pick_seconds,
            reveal_seconds: c.reveal_seconds,
            hints: c.hint_count,
            bots: c.bots_enabled,
        }
    }
}

impl RoomConfig {
    /// Lower the TOML room knobs into the game [`Config`] the room actor runs on.
    pub fn to_game_config(&self) -> Config {
        Config {
            total_rounds: self.rounds,
            draw_seconds: self.draw_seconds,
            pick_seconds: self.pick_seconds,
            reveal_seconds: self.reveal_seconds,
            hint_count: self.hints,
            bots_enabled: self.bots,
        }
    }
}

/// Resolve the config path: `--config <path>` > `DOODUEL_CONFIG` > `dooduel_server.toml`.
/// The bool is whether the path was chosen EXPLICITLY, so a read failure on it warns
/// rather than silently falling back (a missing *default* file is normal and stays quiet).
fn resolve_path() -> (PathBuf, bool) {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--config")
        && let Some(p) = args.get(i + 1)
    {
        return (PathBuf::from(p), true);
    }
    if let Ok(p) = std::env::var("DOODUEL_CONFIG") {
        return (PathBuf::from(p), true);
    }
    (PathBuf::from("dooduel_server.toml"), false)
}

/// The effective room [`Config`]: the resolved TOML file's `[room]` knobs over
/// `Config::default()`, with graceful fallback (missing / unreadable / malformed ⇒ log and
/// use defaults, never stop the server).
pub fn load() -> Config {
    let (path, explicit) = resolve_path();
    match std::fs::read_to_string(&path) {
        Ok(text) => match toml::from_str::<ServerConfig>(&text) {
            Ok(cfg) => {
                eprintln!("dooduel_server: loaded config {}", path.display());
                cfg.room.to_game_config()
            }
            Err(e) => {
                eprintln!(
                    "dooduel_server: config {} parse error ({e}); using defaults",
                    path.display()
                );
                Config::default()
            }
        },
        Err(e) => {
            if explicit {
                eprintln!(
                    "dooduel_server: config {} unreadable ({e}); using defaults",
                    path.display()
                );
            }
            Config::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_empty_document_is_all_defaults() {
        let cfg: ServerConfig = toml::from_str("").expect("empty is valid");
        assert_eq!(cfg.room.to_game_config(), Config::default());
    }

    #[test]
    fn a_partial_room_table_overrides_only_named_knobs() {
        let cfg: ServerConfig = toml::from_str("[room]\nrounds = 1\nbots = false\n")
            .expect("partial room table parses");
        let game = cfg.room.to_game_config();
        let def = Config::default();
        // The named knobs change…
        assert_eq!(game.total_rounds, 1);
        assert!(!game.bots_enabled);
        // …and everything omitted keeps the game default.
        assert_eq!(game.draw_seconds, def.draw_seconds);
        assert_eq!(game.pick_seconds, def.pick_seconds);
        assert_eq!(game.reveal_seconds, def.reveal_seconds);
        assert_eq!(game.hint_count, def.hint_count);
    }

    #[test]
    fn all_knobs_lower_into_the_game_config() {
        let cfg: ServerConfig = toml::from_str(
            "[room]\nrounds = 3\ndraw_seconds = 120\npick_seconds = 30\n\
             reveal_seconds = 12\nhints = 4\nbots = false\n",
        )
        .expect("full room table parses");
        assert_eq!(
            cfg.room.to_game_config(),
            Config {
                total_rounds: 3,
                draw_seconds: 120,
                pick_seconds: 30,
                reveal_seconds: 12,
                hint_count: 4,
                bots_enabled: false,
            }
        );
    }

    #[test]
    fn an_unknown_field_is_rejected() {
        // `deny_unknown_fields` catches a typo instead of silently ignoring it.
        assert!(toml::from_str::<ServerConfig>("[room]\ndraw_secs = 99\n").is_err());
    }
}
