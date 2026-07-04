# Protokit — design system

**Protokit is a scaffold for building working interactive demos**, not a single branded
product. It gives a design agent everything needed to turn a flat mock into something that
*feels* like a real app — persistent + reactive state, a responsive shell that works on
desktop **and** mobile, always-on light/dark theming, and a neutral, re-skinnable component
foundation. You bring the product; Protokit removes the boilerplate.

The visual identity is deliberately **neutral and tool-like** so it reads as a sensible
starting point for almost any product. Swap the accent, the type, and the copy and it
becomes the thing you're demoing.

> Origin: the patterns here were distilled from a polished reference prototype
> ("Adora", a coffee-shop membership demo). Protokit generalizes its plumbing — the
> persistence helpers, theme/layout architecture, app shell, and primitive set — into a
> reusable, unbranded kit. No external Figma or repo is required; everything lives in this
> project.

---

## How a demo is built with Protokit

A demo is a single HTML file (e.g. `ui_kits/demo/index.html`) that wires four layers:

1. **`styles.css`** — the design tokens + base + component CSS (the one file consumers link).
2. **`protokit.js`** — the runtime: `usePersistedState`, `resetState`, `applyTheme`,
   `applyLayout`, `useToasts`, avatar helpers. Plain JS on `window.Protokit`.
3. **`_ds_bundle.js`** — the compiled React component library (auto-generated). Read
   components off `window.ProtokitDesignSystem_47e616`.
4. **Your app code** — `data.js` (mock state) + screens (`*.jsx`) composing the primitives.

The reference implementation of all four is `ui_kits/demo/` — read it first.

### The three things that make it feel real
- **Persistent state.** `usePersistedState(key, initial)` is a drop-in for `useState` that
  mirrors to `localStorage` under a per-surface namespace (`window.PK_NS`). Refresh and the
  app remembers tasks, theme, which screen you were on, even mid-flow.
- **Light/dark, always toggleable.** `[data-theme]` on `<html>` flips the whole ladder.
  `<ThemeToggle>` lives in the app chrome (sidebar foot on desktop, top bar on mobile).
- **One layout, two shapes.** `[data-mode="mobile|desktop"]` reflows the **same** `<AppShell>`
  between a desktop sidebar and a mobile bottom-nav. On a desktop viewport the mobile layout
  is wrapped in a phone-card frame.

---

## Content fundamentals (voice & copy)

Protokit's own chrome is quiet; the demo content shows the intended product voice. Keep copy:

- **Plain and human, second person.** "Add a task," "Sign in to pick up where you left off,"
  "Nothing due — enjoy the calm." Address the user as *you*. Protokit (the tool) never refers
  to itself in the first person.
- **Lowercase-friendly, sentence case.** Buttons and labels are sentence case ("New task",
  "Mark done"), not Title Case or ALL CAPS — except the **mono eyebrow** label, which is
  uppercase and letter-spaced (`Good morning · Friday, June 6`).
- **Short.** Labels are 1–2 words; empty states are one warm line plus one helpful line.
  Toasts are 2–4 words ("Task added", "Nice work — task done").
- **Concrete, lived-in placeholder data.** Real-sounding names (Maya Okafor, Priya Nair),
  real tasks ("Water the fiddle-leaf fig"), believable dates. Never lorem ipsum, never
  "Item 1 / Item 2".
- **No emoji** in UI chrome. Status and meaning come from icons + color, not emoji.
- **Numbers are mono + tabular** so columns and counters never jitter.

---

## Visual foundations

**Palette.** A neutral surface ladder (`--canvas → --surface → --surface-2 → --surface-3`)
and a cool ink ladder (`--ink → --ink-2 → --muted → --faint`), plus hairlines (`--hair`,
`--hair-2`). One **swappable accent** (default indigo-blue `#3a63ee`; brightened to `#5b86f5`
in dark) carries all brand color. Semantics are `--pos` (green), `--warn` (amber), `--danger`
(red), each with a tint. An always-dark `--ink-panel` powers hero/splash panels that stay
dark in *both* themes. Accent is normally set **inline by JS** so it can change independent
of theme; the tint recomputes via `color-mix`.

**Typography.** Geist for everything UI (the `450` "book" weight is the body default —
crisp without shouting), Geist Mono for numbers, codes, timestamps, and the uppercase
eyebrow label. Display/title set in the same family, semibold, tight tracking
(`-0.02em`). `--font-display` is its own var so a separate display face can be promoted later
without touching components. *(Geist + Geist Mono load from Google Fonts — see Caveats.)*

**Shape & depth.** Radii ramp from `--r-sm` (8) through `--r-md` (11, the workhorse) to
`--r-full` pills; buttons and badges are fully rounded. Cards are `--surface` + 1px `--hair`
+ `--sh-sm`, radius `--r-lg`. Shadows are soft and cool in light, opacity-driven in dark
(`--sh-sm` cards → `--sh-md` menus → `--sh-lg` modals → `--sh-pop` popovers).

**Backgrounds.** Mostly flat `--canvas`. The only gradients are (a) a subtle accent
radial glow behind the phone-frame and the auth hero panel, and (b) chart fills. No
textures, no full-bleed photography in the core kit (the reference includes a tasteful
duotone `Photo` placeholder pattern if you need image stand-ins). Avoid decorative gradient
backgrounds.

**Motion.** One easing curve does almost all the work: `--ease cubic-bezier(.32,.72,.32,1)`
— confident, slightly settling. Durations are short and functional: `.12s` press, `.18s`
state, `.28s` popovers/toasts, `.36s` sheets. Entrances are small (fade-up 10px, pop-in
scale .94, sheet-up from bottom). **Press states** shrink/translate (`translateY(1px)`).
No infinite decorative loops in product UI. Everything respects `prefers-reduced-motion`.

**Hover & press.** Hover lightens/darkens one surface step (`--surface-2`) or shifts border
to `--faint`; primary buttons go to `--accent-press`. Press translates down 1px. Focus rings
are a 3px `--ring` (accent tint) halo plus an accent border.

**Layout.** Content caps at `--screen-max` (1180px) and centers. The desktop sidebar is
260px. Tap targets stay ≥ 44px on mobile (`--control` = 42, `--control-lg` = 52). Everything
snaps to the 4px spacing scale (`--sp-1 … --sp-10`).

---

## Iconography

One inline component, **`Icon`**, draws from a single `ICON_PATHS` map (~60 icons) in
`components/core/Icon.jsx`. All icons are **24×24, stroke-based (width 1.7), round caps and
joins, `currentColor`** — so they inherit text color and size via props. There is **no icon
font and no external icon dependency**; to add an icon, drop a new 24×24 stroke path into the
map and it's available everywhere by name.

Usage: `<Icon name="check" size={18} />`. Coverage spans nav (home, list, calendar, inbox),
status (check, circle, flag, star, clock), arrows/chevrons, actions (edit, trash, copy,
filter), and theme (sun, moon). **Never substitute emoji or unicode glyphs for icons.** If a
needed icon is missing, add a matching stroke path rather than reaching for an emoji.

---

## Index / manifest

**Root**
- `styles.css` — entry point; `@import` manifest only (tokens → base → components).
- `protokit.js` — runtime (`window.Protokit`): state persistence, reset, theme/layout, toasts.
- `base.css` — reset, text helpers (`.display/.title/.eyebrow/.mono/.num`), utilities, keyframes.
- `components.css` — component classes (`.pk-btn`, `.pk-card`, `.pk-input`, …).
- `readme.md` — this guide. `SKILL.md` — Agent-Skills-compatible entry.

**`tokens/`** — `colors.css` (light + dark), `typography.css`, `fonts.css`, `spacing.css`,
`elevation.css`, `motion.css`. All `@import`ed by `styles.css`.

**`components/`** — React primitives (each `*.jsx` + `*.d.ts` + `*.prompt.md`, with one
`@dsCard` HTML per directory). Read off `window.ProtokitDesignSystem_47e616`.
- `core/` — Icon, Button, IconButton, Badge, Avatar, Card
- `forms/` — Input, Checkbox, Switch, Segmented, Tabs
- `feedback/` — Dialog (modal/sheet), Tooltip *(toasts via `Protokit.useToasts`)*
- `shell/` — AppShell (responsive chrome) + `shell.css`, ThemeToggle

**`ui_kits/demo/`** — **Protokit Tasks**, the full interactive demo and the canonical
reference for wiring a Protokit app: `index.html`, `data.js`, `app.jsx` (phase machine +
state + the two tweaks), `screens.jsx`, `signin.jsx`, `demo.css`, `tweaks-panel.jsx`.

**`guidelines/`** — foundation specimen cards (Colors, Type, Spacing, Motion) shown in the
Design System tab.

**Tweaks (in the demo):** *Reset app state* (wipes the namespace + reloads) and *Layout*
(Auto / Mobile / Desktop). Light/dark is **not** a tweak — it's an always-visible in-app
toggle. Extend with more tweaks per real-app needs.
