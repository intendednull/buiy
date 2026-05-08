# Changelog

All notable changes to Buiy are recorded here.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
once it reaches `0.1.0`. Pre-`0.1.0` releases are pre-alpha; APIs may break in any commit.

## [Unreleased]

Pre-`0.1.0` development. Detailed change tracking begins with the first
tagged release.

### Added
- Render pipeline now produces real pixels for Buiy nodes (instance-buffer
  construction, clip-space conversion, draw call). Closes the Phase 0
  render deferral.
- Per-window AccessKit tree-update bridge. Buiy translates its widget tree
  to `accesskit::TreeUpdate` each frame and pushes it through bevy_winit's
  `ACCESS_KIT_ADAPTERS` so real screen readers attached to a Buiy window
  see the live tree. (Bevy 0.18 owns adapter creation, so Buiy bridges
  rather than owning `Adapter` objects directly.) Closes the Phase 0 a11y
  deferral.
- `bevy_picking` backend. `Hovered` becomes a thin layer over the standard
  `PointerHits` event flow. Closes the Phase 0 picking deferral.
