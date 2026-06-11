//! The `@font-face` byte source: `BuiyFont` + `BuiyFontLoader`
//! (font-assets § 2). The loader's invariant is
//! **loader-output-is-always-sfnt** — whatever it accepts, the bytes
//! handed to fontdb are sfnt. That invariant IS the named woff2 seam:
//! adding woff2 later means a magic sniff + decompression pre-pass inside
//! `load`, touching neither the registry nor the `FontSystem`. **C (seam
//! named, font-assets § 9.)**

use std::sync::Arc;

use bevy::asset::{Asset, AssetLoader, LoadContext, io::Reader};
use bevy::reflect::TypePath;

/// Raw sfnt bytes (ttf/otf/ttc/otc). `Arc` so registration hands fontdb a
/// zero-copy `Source::Binary(Arc<dyn AsRef<[u8]> + Send + Sync>)`
/// (font-assets § 2; `Arc<Vec<u8>>` satisfies the bound).
#[derive(Asset, TypePath)]
pub struct BuiyFont {
    /// The validated sfnt bytes.
    pub data: Arc<Vec<u8>>,
}

/// `AssetLoader` for fontdb's native formats (verified: fontdb 0.23 "Will
/// load ttf, otf, ttc and otc fonts"; no WOFF/WOFF2).
#[derive(Default, TypePath)]
pub struct BuiyFontLoader;

/// Loader failure: IO, or bytes that are not sfnt (the woff2 seam's
/// honest error). Hand-written impls — `thiserror` is not a buiy_core
/// dependency.
#[derive(Debug)]
pub enum BuiyFontLoaderError {
    /// Reading the asset bytes failed.
    Io(std::io::Error),
    /// The bytes are not sfnt — the rejection that names the woff2 seam.
    NotSfnt,
}

impl std::fmt::Display for BuiyFontLoaderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(err) => write!(f, "failed to read font bytes: {err}"),
            Self::NotSfnt => f.write_str(
                "not an sfnt font (ttf/otf/ttc/otc); woff2 needs the \
                 font-assets § 9 decompression seam",
            ),
        }
    }
}

impl std::error::Error for BuiyFontLoaderError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(err) => Some(err),
            Self::NotSfnt => None,
        }
    }
}

impl From<std::io::Error> for BuiyFontLoaderError {
    fn from(err: std::io::Error) -> Self {
        Self::Io(err)
    }
}

impl AssetLoader for BuiyFontLoader {
    type Asset = BuiyFont;
    type Settings = ();
    type Error = BuiyFontLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut LoadContext<'_>,
    ) -> Result<BuiyFont, Self::Error> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        if !sniff_sfnt(&bytes) {
            return Err(BuiyFontLoaderError::NotSfnt);
        }
        Ok(BuiyFont {
            data: Arc::new(bytes),
        })
    }

    fn extensions(&self) -> &[&str] {
        &["ttf", "otf", "ttc", "otc"]
    }
}

/// The sfnt magic sniff — the loader-output-is-always-sfnt gate: TrueType
/// (0x00010000), CFF (`OTTO`), collection (`ttcf`), legacy Apple TrueType
/// (`true`). Everything else (wOF2 included) is rejected with the seam
/// named in the error.
pub fn sniff_sfnt(bytes: &[u8]) -> bool {
    matches!(
        bytes.get(..4),
        Some([0x00, 0x01, 0x00, 0x00] | b"OTTO" | b"ttcf" | b"true")
    )
}
