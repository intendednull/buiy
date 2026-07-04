//! Dooduel persistence (W6) — the player's theme, name, and custom avatar survive
//! a restart, exactly as the design's `localStorage` (`dooduel-proto-theme` /
//! `dooduel-proto-avatar`) does.
//!
//! **What persists:** the [`ThemePref`], the `player_name`, and the avatar CHOICE
//! (default hash / gallery preset / a drawn custom). A custom avatar's 220×220
//! pixels are PNG-encoded (the `image` dep) + base64'd into the blob (the design's
//! `canvas.toDataURL`).
//!
//! **Where it goes.** Native: a JSON file under the platform config dir
//! (`$XDG_CONFIG_HOME/dooduel/state.json`, or `$HOME/.config/…`), overridable with
//! `DOODUEL_STATE_DIR` (so a capture / test writes to a temp dir, never the real
//! profile). Wasm (W7): browser `localStorage`, under the design's keys
//! (`dooduel-proto-theme` = the theme string, `dooduel-proto-avatar` = the avatar
//! JSON; plus a `dooduel-proto-name` the design doesn't persist but the campaign
//! wants round-tripping). The backend is a single [`load_persisted`] /
//! [`save_persisted`] seam — each target owns its own encoding.
//!
//! **How it folds.** [`load_at_boot`] reads the blob ONCE at startup, restores a
//! custom avatar's pixels straight onto the paint surface (a side channel), and
//! enqueues one [`Msg::Restore`] so the theme/name/avatar-choice fold through the
//! funnel like any message. [`persist_on_change`] writes the blob whenever one of
//! the persisted fields (theme / name / avatar kind) changes — NOT every frame
//! (the model ticks every frame; only the persisted subset triggers a write).
//! Both live in [`StoragePlugin`], kept OUT of `install` so the GPU-free probe
//! tests never touch the filesystem.

// `PathBuf` is only used by the native config-file path; wasm persists to
// `localStorage` (no filesystem), so the import would be unused there.
#[cfg(not(target_arch = "wasm32"))]
use std::path::PathBuf;

use base64::Engine;
use bevy::prelude::*;
use buiy_core::mvu::{MvuSet, enqueue};
use serde::{Deserialize, Serialize};

use crate::paint::{AVATAR_H, AVATAR_W, PaintCanvases};
use crate::theme::ThemePref;
use crate::{Dooduel, HumanAvatar, Msg};

/// The persisted blob (design's two `localStorage` keys, folded into one file).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct PersistedState {
    /// `"light"` / `"dark"` (design `dooduel-proto-theme`).
    theme: String,
    /// The display name.
    name: String,
    /// The avatar choice (design `dooduel-proto-avatar`).
    avatar: PersistedAvatar,
}

/// The persisted avatar choice — the design's `{ type: 'preset' | 'custom', … }`
/// (plus a `default` for the name-hashed doodle).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
enum PersistedAvatar {
    Default,
    Preset {
        icon: usize,
        tint: usize,
    },
    /// The drawn avatar, its 220×220 RGBA PNG base64-encoded.
    Custom {
        png: String,
    },
}

/// PNG-encode + base64 the committed custom-avatar pixels (design `toDataURL`).
fn encode_avatar_png(pixels: &[u8]) -> Option<String> {
    let img = image::RgbaImage::from_raw(AVATAR_W as u32, AVATAR_H as u32, pixels.to_vec())?;
    let mut buf = std::io::Cursor::new(Vec::new());
    img.write_to(&mut buf, image::ImageFormat::Png).ok()?;
    Some(base64::engine::general_purpose::STANDARD.encode(buf.into_inner()))
}

/// Decode a base64 PNG back to raw 220×220 RGBA (the boot restore).
fn decode_avatar_png(b64: &str) -> Option<Vec<u8>> {
    let bytes = base64::engine::general_purpose::STANDARD.decode(b64).ok()?;
    let img = image::load_from_memory(&bytes).ok()?;
    Some(img.to_rgba8().into_raw())
}

/// The JSON file path (native). `DOODUEL_STATE_DIR` overrides the dir (capture /
/// test isolation); otherwise the platform config dir.
#[cfg(not(target_arch = "wasm32"))]
fn state_path() -> Option<PathBuf> {
    if let Ok(dir) = std::env::var("DOODUEL_STATE_DIR")
        && !dir.is_empty()
    {
        return Some(PathBuf::from(dir).join("state.json"));
    }
    let base = std::env::var("XDG_CONFIG_HOME")
        .ok()
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .or_else(|| {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join(".config"))
        })?;
    Some(base.join("dooduel").join("state.json"))
}

/// Load the persisted state (native = the JSON file). `None` when nothing is
/// saved yet (or the blob is unreadable — a clean first-run, not an error).
#[cfg(not(target_arch = "wasm32"))]
fn load_persisted() -> Option<PersistedState> {
    let blob = std::fs::read_to_string(state_path()?).ok()?;
    serde_json::from_str(&blob).ok()
}

/// Save the persisted state (native = the JSON file, creating the dir).
#[cfg(not(target_arch = "wasm32"))]
fn save_persisted(state: &PersistedState) {
    let Some(path) = state_path() else {
        return;
    };
    if let Some(dir) = path.parent() {
        let _ = std::fs::create_dir_all(dir);
    }
    if let Ok(blob) = serde_json::to_string(state) {
        let _ = std::fs::write(path, blob);
    }
}

// ---------------------------------------------------------------------------
// Wasm persistence (W7): browser `localStorage`, under the design's keys. Each
// field is its own key so the theme + avatar match the design's `localStorage`
// exactly (`dooduel-proto-theme` / `dooduel-proto-avatar`); the name rides a
// third key (the design defaults the name every load — the campaign wants it to
// persist, so this is a deliberate, journaled extension).
// ---------------------------------------------------------------------------
#[cfg(target_arch = "wasm32")]
const THEME_KEY: &str = "dooduel-proto-theme";
#[cfg(target_arch = "wasm32")]
const NAME_KEY: &str = "dooduel-proto-name";
#[cfg(target_arch = "wasm32")]
const AVATAR_KEY: &str = "dooduel-proto-avatar";

/// The page's `localStorage`, or `None` if unavailable (private-mode / no window).
/// Every access is best-effort — persistence must never crash the app.
#[cfg(target_arch = "wasm32")]
fn local_storage() -> Option<web_sys::Storage> {
    web_sys::window()?.local_storage().ok().flatten()
}

#[cfg(target_arch = "wasm32")]
fn load_persisted() -> Option<PersistedState> {
    let ls = local_storage()?;
    let theme = ls.get_item(THEME_KEY).ok().flatten();
    let name = ls.get_item(NAME_KEY).ok().flatten();
    let avatar_raw = ls.get_item(AVATAR_KEY).ok().flatten();
    // Nothing saved yet — a clean first run.
    if theme.is_none() && name.is_none() && avatar_raw.is_none() {
        return None;
    }
    // A present-but-unparseable avatar key falls back to the default doodle (the
    // design's `getInitialAvatar` returns null on a bad blob).
    let avatar = avatar_raw
        .and_then(|s| serde_json::from_str::<PersistedAvatar>(&s).ok())
        .unwrap_or(PersistedAvatar::Default);
    Some(PersistedState {
        theme: theme.unwrap_or_else(|| "light".to_string()),
        name: name.unwrap_or_default(),
        avatar,
    })
}

#[cfg(target_arch = "wasm32")]
fn save_persisted(state: &PersistedState) {
    let Some(ls) = local_storage() else {
        return;
    };
    let _ = ls.set_item(THEME_KEY, &state.theme);
    let _ = ls.set_item(NAME_KEY, &state.name);
    // Match the design's `persistAvatar`: a default (name-hashed) avatar is the
    // ABSENCE of the key (`removeItem`); a preset or custom avatar is a JSON blob.
    match &state.avatar {
        PersistedAvatar::Default => {
            let _ = ls.remove_item(AVATAR_KEY);
        }
        other => {
            if let Ok(blob) = serde_json::to_string(other) {
                let _ = ls.set_item(AVATAR_KEY, &blob);
            }
        }
    }
}

/// Boot-restore the persisted state ONCE: read the blob, restore a custom avatar's
/// pixels onto the paint surface, and enqueue [`Msg::Restore`] so the theme / name
/// / avatar-choice fold through the funnel. `Option`/`Local` so it no-ops without
/// the paint plugin and fires at most once.
fn load_at_boot(
    model: Option<Single<(Entity, &Dooduel)>>,
    canvases: Option<ResMut<PaintCanvases>>,
    mut commands: Commands,
    mut done: Local<bool>,
) {
    if *done {
        return;
    }
    let Some(model) = model else {
        return; // model not spawned yet — try next frame
    };
    // Attempt exactly once (a missing / corrupt blob is a clean no-op, not a retry).
    *done = true;
    let Some(state) = load_persisted() else {
        return;
    };

    let avatar = match state.avatar {
        PersistedAvatar::Default => HumanAvatar::Default,
        PersistedAvatar::Preset { icon, tint } => HumanAvatar::Preset { icon, tint },
        PersistedAvatar::Custom { png } => {
            // Restore the drawn pixels straight onto the committed avatar surface
            // (a side channel — the view can't touch `Assets<Image>`).
            if let (Some(mut canvases), Some(pixels)) = (canvases, decode_avatar_png(&png)) {
                canvases.restore_saved_pixels(pixels);
            }
            HumanAvatar::Custom
        }
    };
    let (entity, _) = *model;
    enqueue::<Dooduel>(
        &mut commands,
        entity,
        Msg::Restore {
            theme: ThemePref::from_stored(&state.theme),
            name: state.name,
            avatar,
        },
    );
}

/// Persist the model's theme / name / avatar-choice whenever one of THEM changes
/// (a `Local` snapshot of the persisted subset — NOT every frame; the model ticks
/// each frame but only this subset triggers a write). A custom avatar re-encodes
/// its current committed pixels.
fn persist_on_change(
    model: Option<Single<&Dooduel>>,
    canvases: Option<Res<PaintCanvases>>,
    mut last: Local<Option<(ThemePref, String, HumanAvatar, u64)>>,
) {
    let Some(model) = model else {
        return;
    };
    // The saved-pixels version is part of the key so a custom-avatar SAVE re-writes
    // AFTER the scratch→saved pixel copy lands (a frame after the `kind` flip), not
    // with the still-blank buffer.
    let pixel_version = canvases.as_ref().map(|c| c.saved_version()).unwrap_or(0);
    let key = (
        model.theme,
        model.player_name.clone(),
        model.avatar.kind,
        pixel_version,
    );
    if last.as_ref() == Some(&key) {
        return;
    }
    // Skip the FIRST observation (the boot value): record it without writing, so a
    // load-then-idle run doesn't immediately rewrite what it just read.
    let first = last.is_none();
    *last = Some(key);
    if first {
        return;
    }

    let avatar = match model.avatar.kind {
        HumanAvatar::Default => PersistedAvatar::Default,
        HumanAvatar::Preset { icon, tint } => PersistedAvatar::Preset { icon, tint },
        HumanAvatar::Custom => {
            let png = canvases
                .as_ref()
                .and_then(|c| encode_avatar_png(c.saved_pixels()))
                .unwrap_or_default();
            PersistedAvatar::Custom { png }
        }
    };
    let state = PersistedState {
        theme: model.theme.as_str().to_string(),
        name: model.player_name.clone(),
        avatar,
    };
    save_persisted(&state);
}

/// Installs W6 persistence: boot-restore + persist-on-change. Kept a distinct
/// plugin (NOT in `install`) so the GPU-free probe tests never touch the disk.
pub struct StoragePlugin;

impl Plugin for StoragePlugin {
    fn build(&self, app: &mut App) {
        // `load_at_boot` enqueues, so it belongs in the enqueue set (like the
        // canvas / clock drivers); `persist_on_change` only reads.
        app.add_systems(Update, load_at_boot.in_set(MvuSet::Enqueue));
        app.add_systems(Update, persist_on_change);
    }
}
