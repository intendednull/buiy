//! The clipboard facade (editing-and-ime § 7). `arboard` behind a
//! `ClipboardProvider` Resource trait-object so tests inject a fake and the
//! dependency stays swappable. v1 shipped PLAIN TEXT only; the named follow-up
//! slice adds an HTML flavor (always available) and an image flavor (behind the
//! `clipboard-image` cargo feature, which turns on arboard's `image-data`).
//! This file names NO cosmic type, so the facade-boundary tripwire does not
//! constrain it — it lives in `text::edit` for cohesion (it is editing
//! mechanism), not because it must.

use bevy::prelude::Resource;

/// An owned clipboard image (RGBA8, row-major, `width * height * 4` bytes).
/// Buiy-owned so the [`ClipboardProvider`] trait signature names no arboard
/// `ImageData<'a>` borrowed-lifetime type; conversion to/from arboard happens
/// at the [`ArboardClipboard`] boundary (a one-time byte copy — clipboard
/// payloads are not a hot path). Behind the `clipboard-image` feature because
/// the image flavor pulls arboard's `image-data` deps.
#[cfg(feature = "clipboard-image")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipboardImage {
    pub width: usize,
    pub height: usize,
    pub bytes: Vec<u8>,
}

/// The swappable clipboard backend. Carries a plain-text flavor (always) and an
/// HTML flavor (always); the image flavor is behind the `clipboard-image`
/// feature. All methods take `&mut self`: a real clipboard owns OS handles that
/// mutate on read on some platforms, and the fake owns interior state — `&mut`
/// keeps the trait honest for both. Errors are swallowed to `None` / no-op (a
/// clipboard that is unavailable must never crash an editor; spec § 7 "must not
/// be optional" is about presence, not infallibility).
pub trait ClipboardProvider: Send + Sync + 'static {
    /// The current clipboard text, or `None` if empty / unavailable.
    fn get_text(&mut self) -> Option<String>;
    /// Replace the clipboard text. A failure is a silent no-op.
    fn set_text(&mut self, text: String);
    /// The current clipboard HTML flavor, or `None` if absent / unavailable.
    /// A plain-text editor reads text by preference; the HTML getter is here
    /// for rich-content callers, not for the § 3.3 plain-text Paste path.
    fn get_html(&mut self) -> Option<String>;
    /// Replace the clipboard HTML flavor. A failure is a silent no-op.
    fn set_html(&mut self, html: String);
    /// The current clipboard image, or `None` if absent / unavailable.
    #[cfg(feature = "clipboard-image")]
    fn get_image(&mut self) -> Option<ClipboardImage>;
    /// Replace the clipboard image. A failure is a silent no-op.
    #[cfg(feature = "clipboard-image")]
    fn set_image(&mut self, image: ClipboardImage);
}

/// The active provider, a Resource newtype over the boxed trait object
/// (Bevy resources cannot be bare `dyn`). `BuiyTextPlugin` inserts the
/// arboard-backed one on a real build; tests insert a `MemClipboard`.
#[derive(Resource)]
pub struct Clipboard(pub Box<dyn ClipboardProvider>);

/// The real backend: a lazily-constructed `arboard::Clipboard`. Construction
/// can fail (no display server, Wayland without a clipboard manager); we hold
/// an `Option` and retry on each call, so a headless or transiently-broken
/// clipboard degrades to "empty" rather than panicking at startup.
#[cfg(not(target_arch = "wasm32"))]
#[derive(Default)]
pub struct ArboardClipboard {
    inner: Option<arboard::Clipboard>,
}

#[cfg(not(target_arch = "wasm32"))]
impl ArboardClipboard {
    pub fn new() -> Self {
        Self::default()
    }

    /// Get (or lazily build) the arboard handle. `None` if construction fails.
    fn handle(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.inner.is_none() {
            self.inner = arboard::Clipboard::new().ok();
        }
        self.inner.as_mut()
    }
}

#[cfg(not(target_arch = "wasm32"))]
impl ClipboardProvider for ArboardClipboard {
    fn get_text(&mut self) -> Option<String> {
        self.handle()?.get_text().ok()
    }

    fn set_text(&mut self, text: String) {
        if let Some(h) = self.handle() {
            let _ = h.set_text(text);
        }
    }

    fn get_html(&mut self) -> Option<String> {
        // `get().html()` is on arboard's cross-platform Get builder and is NOT
        // behind `image-data` (verified against arboard 3.6.1).
        self.handle()?.get().html().ok()
    }

    fn set_html(&mut self, html: String) {
        if let Some(h) = self.handle() {
            // No separate alt text: a plain-text editor's html flavor is just
            // escaped text, and the text flavor is set independently.
            let _ = h.set_html(html, None);
        }
    }

    #[cfg(feature = "clipboard-image")]
    fn get_image(&mut self) -> Option<ClipboardImage> {
        let img = self.handle()?.get_image().ok()?;
        Some(ClipboardImage {
            width: img.width,
            height: img.height,
            bytes: img.bytes.into_owned(),
        })
    }

    #[cfg(feature = "clipboard-image")]
    fn set_image(&mut self, image: ClipboardImage) {
        if let Some(h) = self.handle() {
            let _ = h.set_image(arboard::ImageData {
                width: image.width,
                height: image.height,
                bytes: std::borrow::Cow::Owned(image.bytes),
            });
        }
    }
}

/// An in-memory clipboard for tests (PUBLIC so integration-crate tests can
/// use it — `#[cfg(test)]` items are invisible across the crate boundary).
/// Also the right default for a headless app that wants copy/paste WITHIN the
/// app without touching the OS clipboard.
#[derive(Default)]
pub struct MemClipboard {
    text: Option<String>,
    html: Option<String>,
    #[cfg(feature = "clipboard-image")]
    image: Option<ClipboardImage>,
}

impl ClipboardProvider for MemClipboard {
    fn get_text(&mut self) -> Option<String> {
        self.text.clone()
    }

    fn set_text(&mut self, text: String) {
        self.text = Some(text);
    }

    fn get_html(&mut self) -> Option<String> {
        self.html.clone()
    }

    fn set_html(&mut self, html: String) {
        self.html = Some(html);
    }

    #[cfg(feature = "clipboard-image")]
    fn get_image(&mut self) -> Option<ClipboardImage> {
        self.image.clone()
    }

    #[cfg(feature = "clipboard-image")]
    fn set_image(&mut self, image: ClipboardImage) {
        self.image = Some(image);
    }
}
