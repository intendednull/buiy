//! The clipboard facade (editing-and-ime § 7). `arboard` behind a
//! `ClipboardProvider` Resource trait-object so tests inject a fake and the
//! dependency stays swappable. v1 is PLAIN TEXT only (HTML/image deferred —
//! pre-campaign decision 4). This file names NO cosmic type, so the
//! facade-boundary tripwire does not constrain it — it lives in `text::edit`
//! for cohesion (it is editing mechanism), not because it must.

use bevy::prelude::Resource;

/// The swappable clipboard backend. Plain text only in v1. Both methods take
/// `&mut self`: a real clipboard owns OS handles that mutate on read on some
/// platforms, and the fake owns interior state — `&mut` keeps the trait
/// honest for both. Errors are swallowed to `None` / no-op (a clipboard that
/// is unavailable must never crash an editor; spec § 7 "must not be optional"
/// is about presence, not infallibility).
pub trait ClipboardProvider: Send + Sync + 'static {
    /// The current clipboard text, or `None` if empty / unavailable.
    fn get_text(&mut self) -> Option<String>;
    /// Replace the clipboard text. A failure is a silent no-op.
    fn set_text(&mut self, text: String);
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
#[derive(Default)]
pub struct ArboardClipboard {
    inner: Option<arboard::Clipboard>,
}

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

impl ClipboardProvider for ArboardClipboard {
    fn get_text(&mut self) -> Option<String> {
        self.handle()?.get_text().ok()
    }

    fn set_text(&mut self, text: String) {
        if let Some(h) = self.handle() {
            let _ = h.set_text(text);
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
}

impl ClipboardProvider for MemClipboard {
    fn get_text(&mut self) -> Option<String> {
        self.text.clone()
    }

    fn set_text(&mut self, text: String) {
        self.text = Some(text);
    }
}
