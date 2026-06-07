//! Warmup: a pre-paint queue of "insert this now" requests, drained before
//! the first paint so golden frames never race a cold atlas (spec § 2.3,
//! gate #2). This spec owns the *mechanism* (the drain); producers
//! (text/icon owners) decide *what* to warm and push requests.

use bevy::prelude::Resource;

use super::{AtlasBitmap, AtlasFormat, AtlasKey};

/// One pre-paint residency request.
pub struct AtlasWarmupRequest {
    pub key: AtlasKey,
    pub format: AtlasFormat,
    pub bitmap: AtlasBitmap,
}

/// Render-world queue of warmup requests, drained pre-paint by
/// `warmup_atlas`. Producers push; the atlas drains.
#[derive(Resource, Default)]
pub struct AtlasWarmupQueue {
    requests: Vec<AtlasWarmupRequest>,
}

impl AtlasWarmupQueue {
    /// Enqueue a residency request.
    pub fn push(&mut self, req: AtlasWarmupRequest) {
        self.requests.push(req);
    }

    /// Pending request count.
    pub fn len(&self) -> usize {
        self.requests.len()
    }

    /// No pending requests.
    pub fn is_empty(&self) -> bool {
        self.requests.is_empty()
    }

    /// Take all pending requests, emptying the queue.
    pub(crate) fn take(&mut self) -> Vec<AtlasWarmupRequest> {
        std::mem::take(&mut self.requests)
    }
}
