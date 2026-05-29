**Date:** 2026-05-29
**Status:** active
**Subject:** Blink (Chromium) — the 2013 WebKit fork, Google's stewardship within open-source Chromium, the Blink launch process (intent-to-prototype/experiment/ship; blink-dev; API owners), multi-vendor Chromium, the BSD-3-Clause license, and the engine-monoculture concern

# Blink governance

Blink is the browser engine inside Chromium. It is the canonical reference
implementation of the CSS modules Buiy implements a typed-Rust subset of —
which is why its governance, not just its code, is load-bearing prior art. This
file covers *who controls it and how features ship*; [history.md](history.md)
covers *when things landed*; [architecture.md](architecture.md),
[layout.md](layout.md), [stacking-and-paint.md](stacking-and-paint.md),
[containment-and-queries.md](containment-and-queries.md), and [style.md](style.md)
cover *how it works*.

## The WebKit fork (2013)

Google announced Blink on **2013-04-03** as a fork of WebKit's `WebCore`
rendering component (verified against the [TechCrunch announcement](https://techcrunch.com/2013/04/03/google-forks-webkit-and-launches-blink-its-own-rendering-engine-that-will-soon-power-chrome-and-chromeos/)
and the [Blink Wikipedia entry](https://en.wikipedia.org/wiki/Blink_(browser_engine))).
WebKit itself was a fork of KDE's KHTML/KJS (2001), so Blink is two forks deep —
a lineage worth keeping in mind when treating "the browser" as a single canonical
authority. The stated rationale: Chrome's multi-process architecture had forced a
WebKit2-vs-WebKit divergence, and the build carried scaffolding for ports Google
did not ship. The fork let Google delete that scaffolding and move faster; it also
let Apple's WebKit evolve without Google's constraints. The honest read is that
the fork was as much about *organizational control* as technical cleanliness —
two large vendors no longer wanted to coordinate on one codebase.

## Google stewardship inside open-source Chromium

Blink lives in the Chromium source tree (`third_party/blink/`). Chromium is an
open-source project, but its governance is **Google-led**, not foundation-led:
there is no independent Chromium Foundation analogous to the way Rust, Linux, or
even Servo (now under Linux Foundation Europe) are stewarded. Google employs the
overwhelming majority of committers, sets the roadmap, and controls the
trademark-encumbered Chrome release built on top. External contributors and other
vendors participate, but the steering is corporate. This is the structural
opposite of Buiy's substrate: [Taffy](../taffy/), `cosmic-text`, and
[AccessKit](../accesskit/governance.md) are community/company-adjacent Rust crates
under `MIT OR Apache-2.0`, none owned by a single dominant vendor.

## The Blink launch process

Web-exposed feature changes go through the **Blink Intent process**, a public,
mailing-list-driven workflow (verified against the [Launching Features](https://www.chromium.org/blink/launching-features/)
and [Blink Intents](https://developer.chrome.com/docs/web-platform/blink-intents)
docs). The stages:

- **Intent to Prototype** — emailed to the `blink-dev` list; prototype code lands
  behind a runtime flag. No approval gate; it is a notification.
- **Intent to Experiment** — optional; requests an origin trial to gather data
  from real sites. Requires **one LGTM** from an API owner.
- **Intent to Ship** — the gate to enabling a feature by default. Requires
  **three LGTMs** from the **Blink API owners**.

The **API owners** are a small, named group of senior Chromium contributors
(per [Blink API owners](https://www.chromium.org/blink/guidelines/api-owners/))
trusted by the Blink community to weigh interoperability, security, and
specification maturity before a feature ships to the whole web. `ChromeStatus`
(chromestatus.com) tracks each feature's stage. This is a serious, transparent
process — but the API owners are predominantly Google employees, so "three LGTMs"
is not the same as cross-vendor consensus. Cross-vendor signals (Mozilla and
WebKit standards positions, plus W3C TAG review) are *inputs* to the decision, not
vetoes.

## Multi-vendor Chromium

Blink ships in more browsers than Chrome. Microsoft **Edge** moved from its own
EdgeHTML engine to Chromium with **Edge 79, released 2020-01-15** (verified
against the [Microsoft Edge Blog](https://blogs.windows.com/msedgedev/2020/01/15/upgrading-new-microsoft-edge-79-chromium/)),
codenamed "Anaheim." **Brave**, **Opera**, **Vivaldi**, and **Samsung Internet**
also build on Chromium/Blink. Several of these vendors contribute upstream — for
example, the Microsoft Edge team did substantial work re-architecting Chromium's
CSS Grid onto LayoutNG (see [layout.md](layout.md)).

The flip side is the **engine-monoculture concern**. With Edge's 2020 switch, the
only remaining independent major engines are Apple's WebKit (Safari) and Mozilla's
Gecko. Critics — including Mozilla in its [public reaction to the Edge move](https://en.wikipedia.org/wiki/Blink_(browser_engine))
— argue that when one engine becomes the de-facto definition of "the web,"
*shipping in Chromium becomes equivalent to standardizing*, regardless of the
W3C/WHATWG process. A feature that lands in Blink first (as anchor positioning and
the Popover API did — see [history.md](history.md)) sets the interoperability
baseline other engines must then match. This is the reason a CSS-faithful library
like Buiy must cite the **W3C modules** (Display 3, Positioned Layout, CSS
Containment 3, CSS Writing Modes 4, CSS Anchor Positioning 1) as the source of
truth and treat Blink as *an* implementation of them — not as the spec itself.
Where Blink's behavior and the published module diverge, Buiy follows the module
and notes the divergence.

## License

Chromium (and Blink within it) is released predominantly under the
**BSD-3-Clause** license (the "New BSD" license), verified by fetching the
[Chromium `LICENSE` file](https://chromium.googlesource.com/chromium/src/+/main/LICENSE):
it carries the retain-notice, binary-reproduction, and non-endorsement clauses
characteristic of BSD-3-Clause, with Google LLC as the named copyright holder.
Because Blink descends from WebKit, parts of its history carry WebKit's
**LGPL-2.1 / BSD** dual heritage, and individual files still bear those headers;
the project as a whole is governed by the top-level BSD-3-Clause `LICENSE` plus
per-file notices.

For Buiy this is mostly an *attribution* concern rather than a reuse one: Buiy is
`MIT OR Apache-2.0` (see [`../../specs/2026-05-07-buiy-foundation/README.md`](../../specs/2026-05-07-buiy-foundation/README.md))
and does not vendor Blink code. BSD-3-Clause is permissive and compatible in
principle, but Buiy draws on Blink for *semantics and reference behavior*, not
source — the same posture it takes toward the W3C specs.

## Implications for Buiy

- **Treat Blink as a reference, govern like a spec.** Buiy's stacking-context
  trigger union (see [stacking-and-paint.md](stacking-and-paint.md) and the
  Buiy [`stacking-and-top-layer.md`](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md)
  spec, Phase 9) should match the W3C/CSS rules that Blink also implements; where
  Buiy and Blink differ, the divergence is a documented decision, not a bug.
- **No monoculture replication.** Buiy implements a *subset* of CSS on top of
  [Taffy](../taffy/), adding its passes *above* Taffy (never forking it). It does
  not aim to be the canonical web engine, so it is free to omit, simplify, or
  re-shape (e.g. the decomposed public-fielded components vs. Blink's
  `ComputedStyle` megastruct — see [style.md](style.md)).
- **Governance contrast is the lesson.** Blink's three-LGTM gate is a model of
  *transparent change control*; its single-vendor concentration is the
  anti-pattern. Buiy's docs/specs/plans system is the small-project analogue of a
  transparent launch process — design before code, supersede rather than
  contradict.

## Sources

- https://techcrunch.com/2013/04/03/google-forks-webkit-and-launches-blink-its-own-rendering-engine-that-will-soon-power-chrome-and-chromeos/
- https://en.wikipedia.org/wiki/Blink_(browser_engine)
- https://blogs.windows.com/msedgedev/2020/01/15/upgrading-new-microsoft-edge-79-chromium/
- https://www.chromium.org/blink/launching-features/
- https://developer.chrome.com/docs/web-platform/blink-intents
- https://www.chromium.org/blink/guidelines/api-owners/
- https://chromium.googlesource.com/chromium/src/+/main/LICENSE
- [layout.md](layout.md)
- [stacking-and-paint.md](stacking-and-paint.md)
- [style.md](style.md)
- [history.md](history.md)
- [../taffy/](../taffy/)
- [../accesskit/governance.md](../accesskit/governance.md)
- [../../specs/2026-05-07-buiy-foundation/README.md](../../specs/2026-05-07-buiy-foundation/README.md)
- [../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md](../../specs/2026-05-08-buiy-layout-design/stacking-and-top-layer.md)
