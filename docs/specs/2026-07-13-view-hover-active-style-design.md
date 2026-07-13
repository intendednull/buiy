<!-- Spec: declarative :hover/:active style API on the buiy_view runtime interaction layer (campaign Track D). -->
<!-- Date: 2026-07-13. Type: feature design (mini-spec). -->

# Buiy view — declarative `:hover`/`:active` style API

- **Date:** 2026-07-13
- **Status:** APPROVED (design-gated) — Track D of the app-author-ergonomics
  campaign (`docs/specs/2026-07-13-app-author-ergonomics-campaign-design.md`
  § "Track D"). Realizes rec 6c's residual.
- **Area:** `crates/buiy_view` — `interaction.rs`, `element.rs`, `reconcile.rs`,
  `app.rs`.
- **Effort:** M. Verified by a read-only architecture investigation (file:line
  cited below) before coding.

## 1. Problem

The runtime interaction layer already tracks transient state **outside the pure
model** (`interaction.rs:48-57` `InteractionState{None,Hover,Press}`, written by
four pointer observers; `PressEffect` + `apply_press_visual` resolve the
press-down depth dip). `InteractionState::Hover` is *tracked* but **no declarative
`:hover`/`:active` style API drives visuals from it** — the residual.

## 2. The crux — `Background` is shared-ownership (why this isn't a copy of press)

`apply_press_visual` gates only on `Changed<InteractionState>` and mutates a
pre-stamped `Translate` in place. That is safe **only because `Translate` has no
author-facing builder** — nothing else ever writes it, so `Changed`-only gating
can't lose a race.

Hover styling must touch **`Background`**, which is **not** exclusively
runtime-owned: `reconcile::<M>` re-derives it from `Element::background` on every
`Changed<M>` frame (`apply_background` unconditionally for containers
`reconcile.rs:619`/`:1071-1088`; `apply_button_style` only when styled
`:1279-1281`). A hover resolver written like `apply_press_visual` (gated on
`Changed<InteractionState>` alone) would be **silently clobbered** whenever a
hovered node's model *also* changes that frame — e.g. any app with a running
`ClockPlugin` tick — producing an intermittent flicker of the hover fill back to
resting. Folding hover into `reconcile` instead is also non-viable: `reconcile`
runs only on `Changed<M>`, so a pure mouse-only hover/leave (the common case, no
msg) would never update — which is exactly why F5 put interaction state outside
the model in the first place.

## 3. Approach (Option A — recommended)

**Builder:** `.hover_bg(Color)` on `Element<Msg>` — one dedicated modifier
mirroring the existing flat `.background()`/`.color()` idiom (`element.rs:354-357`,
module policy `element.rs:1-8`). The *intent* is pure `Element` data (replay-safe,
part of `view`'s pure output); the *resolved* fill is runtime-only. **Background
only in v1**; applies under **`Hover` OR `Press`** (folds `:active` into the same
token — necessary because `InteractionState` priority is `Press > Hover > None`,
so gating on `Hover` alone would flash the fill back to resting during a press).
**Pressable-gated** — `.hover_bg()` is inert on a node without `.on_press(..)` in
v1 (documented on the builder), matching the press-visual's identical install
scope.

**Runtime component + resolver** (`interaction.rs`, mirroring
`PressEffect`/`apply_press_visual` in shape, with a materially different gate):
- `HoverStyle { resting: Option<ColorToken>, hover: ColorToken }` — `resting`
  tracks the author's `.background()` token, or is captured once at install from
  the node's then-current `Background` (the widget's own default, e.g. a
  `button()`'s `SurfaceSecondary`).
- `resolve_hover_background(state, style) -> ColorToken` — pure, unit-testable:
  `Hover|Press => style.hover`, `None => style.resting.unwrap_or(Transparent)`.
- `apply_hover_visual` — `Query<(&InteractionState, &HoverStyle, &mut Background),
  Or<(Changed<InteractionState>, Changed<Background>)>>`, `set_if_neq`, scheduled
  **`.after(reconcile::<M>)`** (`app.rs`, next to `apply_press_visual`). The `Or`
  half `Changed<Background>` means a reconcile write earlier in the *same* frame
  re-trips this system so it re-wins the race that frame; a steady frame with
  neither input changed is a true no-op (empty query).

**Reconciler stamp** (`reconcile.rs`): `update_hover_style` installs/updates/tears
down `HoverStyle` alongside every `update_press_visual` call site (the two
`Kind::Button` arms + `apply_pressable` for `Kind::Column|Row|Raster`), and
guarantees `Background` is present on a `HoverStyle`-bearing entity (so the
resolver's non-`Option` `&mut Background` query always matches).

**Companion fix (required):** `apply_background`'s `None` arm currently *removes*
`Background` unconditionally — for a `HoverStyle`-bearing unstyled **container**
that would strip the fill every `Changed<M>` frame, breaking the resolver's
always-present invariant. Make the `None` arm `HoverStyle`-aware (restore to
`resting` instead of removing). Additive, gated on `HoverStyle`'s presence, so a
node without hover styling takes the exact old path → **no golden/byte-stability
regression**. `apply_button_style` needs no change (it already never touches
`Background` when unstyled).

## 4. Rejected alternatives

- **Option B — `.on_hover(HoverStyle::new()...)` nested-builder bundle:**
  introduces a second parallel mini-DSL against the flat-modifier policy; over-built
  for a v1 with one styleable property.
- **Option C — `.when_interaction(InteractionState, Style)` general combinator:**
  leaks `InteractionState` (a deliberately runtime-only type) into the pure `view`
  surface as an author-written parameter, and is speculative generality with a
  single real consumer — the same anti-over-engineering call the campaign made for
  Track A's 1e.

## 5. Tests

- Unit: `resolve_hover_background` in `interaction.rs`'s `#[cfg(test)]` module
  (mirrors the `transition` tests).
- Live (extend `crates/buiy_view/tests/press_interaction.rs`'s existing
  `move_to`/`press`/`release` harness): `move_to(center)` (no press) → `None→Hover`
  → assert `Background.color == hover_token`; `press()` → assert hover color **and**
  press depth together (composition); `release()` + `move_to(far)` (a real
  `Pointer<Out>`) → assert reversion to resting.
- **Race regression (mandatory):** fire a model msg while hovering and assert the
  hover fill **survives** the same-frame reconcile write. This is the concrete
  guard for the §2 crux; without it the flicker is easy to miss.

## 6. Deferred + risks

**Deferred (v1 minimal):** any property beyond `Background` (text/border/shadow —
same mechanism, new field); a distinct `.active_bg()` (v1 folds `:active` into
`.hover_bg()` + the existing depth dip); hover on non-pressable nodes;
Icon/Checkbox/TextInput/Text hover (those kinds don't install `InteractionState`
today — a pre-existing gap, not this track's); a general `Style`/`when_interaction`
combinator.

**Risks:** (1) the §2 write-race — the `Or` gate + `.after(reconcile::<M>)` is the
fix; the race regression test is not optional. (2) the `apply_background` None-arm
clobber — the companion fix (§3) is the guard; verified via the real call path, not
assumed. (3) MT-safety — `apply_hover_visual` is an ordinary `Query` system (same
shape as the shipped `apply_press_visual`), but it is a *new* always-scheduled
system writing `Background` (a component others write), so run it through the MT
lane before landing. (4) perf — the `Or` gate keeps steady-state at empty-query for
idle nodes/frames (60Hz floor); call out in review that this gate differs from the
press precedent's plain `Changed<InteractionState>`, don't copy-paste it blindly.
