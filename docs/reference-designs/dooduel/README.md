# Dooduel — design target bundle

Archived design bundle (immutable input to specs, not a spec) for the **Dooduel campaign**
(the skribbl.io-clone game app; see `../../prototypes/2026-07-01-scribbl-campaign-charter.md`,
Amendment 2026-07-02). **Match this target exactly.**

**Provenance:** Claude Design project "Scribbl.io clone design", DesignSync projectId
`19e829e9-b6ac-4c4a-af97-e70782a0da67`, fetched 2026-07-02 via the DesignSync tool (each file
retrieved with `get_file`, un-truncated, byte-faithful; original project-relative paths
preserved so the HTML's relative `_ds/...` links resolve locally).

## Contents

- `Dooduel Prototype.dc.html` — **the parity target**: the full interactive prototype
  (screens, game logic, layout, both desktop + mobile variants).
- `Dooduel - Game Spec.dc.html` — the written game spec (rules, phases, scoring, hints).
- `DoodleAvatar.dc.html` — the avatar component: 22 hand-drawn doodle icons (cat…heart) as
  stroked SVG primitives (paths/lines/circles/ellipses/dots, white 2.6-stroke on a tinted
  circular badge), icon + tint deterministically hashed from the player name.
- `support.js` — the dc-runtime shim (generated; renders `.dc.html` files via React). Not
  game logic — archived so the bundle can render locally.
- `_ds/protokit-design-system-47e616f7…/` — the **Protokit design system** the prototype
  links: `readme.md` (the design-language guide — read it), `styles.css` (@import manifest),
  `base.css` (reset/text-helpers/keyframes), `components.css` (pk-* component classes),
  `components/shell/shell.css` (responsive app shell: desktop sidebar ↔ mobile bottom-nav +
  phone-card framing), `tokens/{colors,typography,fonts,spacing,elevation,motion}.css`
  (the token ladder: light `:root` default + `[data-theme="dark"]`; Geist/Geist Mono;
  4px spacing scale; shadow ramp; one easing + 4 durations), `protokit.js` (runtime:
  persisted state, applyTheme/applyLayout, toasts, avatar tint hash), `_ds_bundle.js`
  (compiled React component library), `_ds_manifest.json` (component + token manifest),
  `_adherence.oxlintrc.json` (design-adherence lint rules — closed prop/variant enums).
- `screenshots/violet-theme.png`, `screenshots/mobile-check.png` — **CAUTION: these render
  the pre-rebrand "Scribbl" snapshot** (letter-initial avatars, flat buttons, Geist type,
  blue-reading accent) and are NOT visual ground truth for Dooduel — the HTML/CSS/JS is.
  Kept for provenance only. (JPEG data despite the `.png` names, as served; 924×540.)

## Not archived

- `screenshots/01|02|03-spec-top.png` — page captures of the spec document top; redundant
  with `Dooduel - Game Spec.dc.html` itself. Re-fetchable from the project if wanted.
- `.thumbnail` — design-tool internal.

## Notes for consumers

- The prototype additionally loads Google Fonts **Caveat** (600/700) and **Shantell Sans**
  (400–700) — the hand-drawn display faces — on top of Protokit's Geist/Geist Mono.
- `REQUIREMENTS-DELTA.md` (beside this file) records what changed vs the previously audited
  Scribbl snapshot (see `../../reports/2026-07-01-scribbl-app-capability-gap-audit.md` §1).
- This bundle is immutable; a re-sync from the design project should land as a new dated
  bundle, not edits here.
