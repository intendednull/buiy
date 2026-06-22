**Date:** 2026-06-18
**Status:** active
**Subject:** Open problems — what browser automation does not cleanly solve, and which gaps are inherent vs. browser-specific (for an in-engine, tree-authoring framework)

# Open problems

What follows is the honest gap list: the things browser automation handles badly,
awkwardly, or not at all. Each item is tagged **[browser-specific]** (an artifact
of automating an HTML/CSS/JS document engine that someone else built) or
**[inherent]** (a difficulty that would follow any UI automation surface,
including an in-engine one like Buiy). The distinction matters: an in-engine,
tree-authoring framework gets to *skip* most browser-specific problems for free,
but inherits the inherent ones and must address them on their own terms.

For how these gaps translate into design lessons, see [lessons.md](lessons.md)
(validates / avoid / borrow). Protocol detail lives in [cdp.md](cdp.md),
[webdriver-bidi.md](webdriver-bidi.md), [playwright-locators.md](playwright-locators.md),
and [actionability.md](actionability.md).

## 1. CDP is Chromium-coupled, sprawling, stateful, and detectable  [browser-specific]

CDP is not a standard. It is versioned with, and coupled to, the Chromium build —
tip-of-tree definitions differ from stable-channel behavior, with no cross-vendor
governance (see [cdp.md](cdp.md)). Four concrete failure modes:

- **Domain sprawl.** Perception and control are scattered across many domains
  (DOM, Accessibility, Input, Page, Overlay, Runtime, Target, CSS, ...). A single
  agent action — find an element, check it is interactable, click it — touches
  several domains, each with its own command/event vocabulary.
- **Enable/disable statefulness.** Most domains must be explicitly enabled before
  they emit events, and that enabling is itself a fragile operation. Practitioners
  report `Page.enable` hanging on discarded/frozen tabs (Chrome Memory Saver), and
  recommend wrapping enable calls in a 2–3 s timeout and skipping or recreating the
  tab on no-response ([agent-browser #1036](https://github.com/vercel-labs/agent-browser/issues/1036)).
  The protocol carries hidden session state the caller must manage.
- **Enabling perturbs the page (and fingerprints the automation).** Enabling
  `Runtime` is itself observable: sites detect automation by watching for the side
  effects of `Runtime.enable`
  ([Rebrowser](https://rebrowser.net/blog/how-to-fix-runtime-enable-cdp-detection-of-puppeteer-playwright-and-other-automation-libraries)).
  The act of attaching the perception surface changes — and reveals — the thing
  being perceived. This is the *attach-layer* detection that coexists with CDP
  input being **trusted** at the *input layer* (`isTrusted = true`, indistinguishable
  from hardware — see [cdp.md](cdp.md)): the automation is undetectable when it
  *acts* but detectable when it *attaches*. The whole anti-bot / automation-fingerprinting
  arms race lives in this gap.

An in-engine framework that exposes its own tree over its own channel has **no
external protocol to version-couple, no domains to enable, no observation side
effects, and no attach to fingerprint** — the tree is already in memory. This
whole class is browser-specific.

## 2. The two-tree (DOM + accessibility) join is a browser artifact  [browser-specific]

In a browser there are *two* trees. The DOM is authored; the accessibility (AX)
tree is a **derived** simplification of it. Chrome's own docs: "The accessibility
tree is a derivative of the DOM tree. Its structure is roughly the same, but
simplified to remove nodes with no semantic content"
([Chrome for Developers](https://developer.chrome.com/blog/full-accessibility-tree)).
Automation that wants semantics (role/name/state) but action targets (coordinates,
DOM nodes) must continually **join** the two — AX nodes carry a back-reference to
their originating DOM node, and tools hop between `Accessibility.getFullAXTree`
and `DOM.querySelector`/`getBoxModel` to turn a semantic match into a clickable
rectangle. That join is pure overhead created by the document engine maintaining
two representations.

Buiy authors **one** tree (the AccessKit semantic tree) that already carries
role+name+state+actions, and the same node owns its layout box. There is no second
tree to reconcile. This problem disappears entirely for a single-tree framework.

## 3. The accessibility tree is reverse-engineered, hence lossy  [browser-specific]

This is the sharpest contrast for Buiy. In a browser the AX tree is **computed**
from DOM + ARIA + CSS by the Accessible Name and Description Computation algorithm
(aria-labelledby, aria-label, native naming rules, text content), run inside
Blink's renderer ([Chrome for Developers](https://developer.chrome.com/blog/full-accessibility-tree),
[Max Design](https://www.maxdesign.com.au/articles/axtree.html)). The role and
name an agent sees are an *inference* about authored markup, not a statement of
intent. When the markup is sloppy — a `<div onclick>` with no role, an icon button
with no label, a name assembled from stray text content — the inference is wrong or
empty, and the agent perceives garbage. The accessibility community's own lesson:
"assistive technology will work much more reliably when information flows directly
to it rather than be reverse engineered" ([Max Design](https://www.maxdesign.com.au/articles/end-to-end-event-architecture.html)).

A tree-authoring framework **inverts** this: Buiy's widgets *declare* role+name+state
at construction, so the semantic tree is the source of truth, not a guess recovered
from presentation. The lossiness here is browser-specific — it is the cost of
recovering semantics that were never explicitly authored. (Inherent residue: a
*framework* can still under-author — ship a button with no accessible name — but
that is a fixable authoring bug, not an unrecoverable inference, and it is
detectable at the source.)

## 4. Shadow DOM, iframes, and realms leak through abstractions  [browser-specific]

Encapsulation boundaries the document platform introduced for *other* reasons leak
into automation. Playwright's locator engine pierces open shadow roots, but auto-wait
"operates within a single frame": at a cross-origin iframe edge the auto-wait
boundary stops, and the most common flake is interacting with iframe content before
the iframe's own scripts/network have settled ([Augment Code](https://www.augmentcode.com/open-source/microsoft/playwright)
summary; cf. [actionability.md](actionability.md)). Closed shadow roots are opaque
by design. Separate JS *realms* (per-frame execution contexts) mean injected scripts
and element handles are not portable across boundaries. None of this is about the UI —
it is about the document engine's composition and isolation model bleeding into the
control surface. A single-process, single-tree engine has no cross-origin realms or
closed-root opacity to leak.

## 5. Flakiness persists even with auto-waiting  [inherent]

Playwright's actionability model (visible, stable, receives-events, enabled; see
[actionability.md](actionability.md)) removes the biggest flake source — `sleep()` —
but does not eliminate flakiness. Documented residual causes:

- **Data vs. element readiness.** Auto-wait waits for the *element* to appear, not
  for the *data* it needs; a list renders empty before its API response lands
  ([Augment Code](https://www.augmentcode.com/open-source/microsoft/playwright)).
- **Animation/transition.** "Actionable" does not account for CSS animation — an
  element can be clickable while still sliding into place, so the click lands off
  target ([Augment Code](https://www.augmentcode.com/open-source/microsoft/playwright)).
- **Stability is a heuristic.** "Two consecutive frames at the same position" is a
  sampling proxy for "settled," and proxies have false positives and negatives.

Part of this is browser-specific (animations, async network the engine hides), but
the **core is inherent**: "is this UI ready for interaction?" is a question about
application state that no actionability heuristic can fully answer from the outside.
An in-engine framework has a real advantage — it can observe its own layout/dirty
flags and async state directly instead of sampling pixels — but "ready" still
requires the *application* to signal intent. Buiy can make readiness *observable*;
it cannot make it free. Whether Buiy's `ResolvedLayout` / layout-dirty machinery
already exposes a *settled* bit that would let it do better than frame-to-frame
bounds sampling is an open question the consuming spec must answer against the layout
internals (teed up in [actionability.md](actionability.md)). See [lessons.md](lessons.md).

## 6. WebDriver BiDi is still maturing and partial across vendors  [browser-specific]

BiDi is the cross-vendor convergence of CDP and WebDriver classic, but as of
2026-06-01 it is a **W3C Working Draft, not a Recommendation** (status line:
"W3C Working Draft, 1 June 2026"; see [webdriver-bidi.md](webdriver-bidi.md)).
Production readiness landed first in Firefox and Chrome / Puppeteer, with critical
gaps still being closed after launch; Playwright kept BiDi at prototype stage,
driving its own suite to find the remaining work; Cypress is mid-transition off the
deprecated CDP backend ([Chrome for Developers](https://developer.chrome.com/blog/firefox-support-in-puppeteer-with-webdriver-bidi)).
Safari support remains the weak corner — Playwright drives WebKit (the engine), not
actual Safari, so true Safari parity lags. The standard exists; uniform behavior
across every engine does not yet. This is the slow tax of multi-vendor convergence —
a problem an in-engine framework, which *is* its own single runtime, never pays.

## 7. Screenshot / pixel-diff verification is brittle  [partly inherent]

Pixel comparison is the fallback when semantic checks are not enough, and it is
notoriously flaky. Two screenshots of the same page seconds apart differ from
anti-aliasing, subpixel font rendering, cursor-blink state, and animation frames;
font hinting differs across OSes; GPU/driver differences flag diffs the human eye
would not see ([DEV / Dennis](https://dev.to/dennis-ddev/screenshot-diffing-pixel-level-comparison-techniques-18k),
[Playwright #7548](https://github.com/microsoft/playwright/issues/7548)). The
industry response — tolerance thresholds, SSIM/perceptual hashing, AI "would a human
notice?" diffing ([Applitools](https://applitools.com/blog/visual-regression-testing/))
— concedes that raw pixel equality is the wrong assertion.

The **inherent** part: rasterization is genuinely nondeterministic across GPUs and
drivers, so any framework verifying at the pixel level inherits this. Buiy's own
verification design confronts exactly this — it pushes assertions down to the lowest
tier that can observe a bug (layout snapshot → display-list snapshot → invariant →
reftest), treating goldens as a last resort for the rasterization residue only
(project CLAUDE.md, `buiy_verify`). That is the same lesson browser visual testing
learned the hard way: **assert on structure/semantics, not pixels, wherever the bug
is observable there.** The browser-specific aggravator is cross-*browser* rendering
divergence on top of cross-GPU divergence.

## 8. Multi-driver / human-vs-agent input arbitration is unaddressed  [inherent]

Browser automation almost always assumes **one driver**: a single WebDriver/CDP
client owns the session, and there is no human simultaneously typing into the same
page. So the corpus offers essentially *no* answer to the question an in-engine
surface hits immediately: if an agent dispatches an action while a human is also
driving the same widget — or two agents act at once — what happens? Browser tools
sidestep this by construction (the automation owns the tab); Buiy cannot, because the
agent surface and the live human UI share one process, one input pipeline, and one
frame loop. An `ActionRequest` arriving through `bevy_winit` interleaves with real
keyboard/pointer input on the same channel. Arbitration (who wins, whether agent input
is queued / rejected / merged, whether focus is contested, whether a human keystroke
mid-agent-action aborts it) is a genuinely **inherent** design question that this
prior-art corpus does *not* answer — flagged here so a consuming spec does not assume
the browser literature covers it. See [lessons.md](lessons.md).

## Summary table

| Gap | Tag | Does an in-engine, tree-authoring framework inherit it? |
|---|---|---|
| Protocol coupling + domain sprawl + enable/disable state + attach-detection | browser-specific | No — no external protocol, no attach |
| Two-tree DOM↔AX join | browser-specific | No — single authored tree |
| AX tree reverse-engineered / lossy | browser-specific | No — semantics authored at source |
| Shadow DOM / iframe / realm leakage | browser-specific | No — single process, single tree |
| Flakiness despite auto-waiting | **inherent** | Partly — readiness is reducible but not free |
| BiDi maturity / cross-vendor parity | browser-specific | No — single runtime |
| Screenshot / pixel-diff brittleness | **partly inherent** | Partly — pixel nondeterminism is real; assert lower |
| Multi-driver / human-vs-agent arbitration | **inherent** | **Yes — and the corpus does not answer it** |

The throughline: **the browser-specific gaps are all symptoms of automating a
document engine you did not author and recovering semantics it never explicitly
declared.** An AccessKit-first framework that authors its semantic tree directly,
in-process, sidesteps that entire column — and is left with the genuinely hard,
inherent problems (interaction-readiness, pixel-level verification, and — uniquely
to an in-engine surface — multi-driver arbitration), the first two of which it is
better positioned to attack from the inside than any external driver is, and the
third of which it must answer for the first time because browsers never had to. The
design consequences are drawn out in [lessons.md](lessons.md).

## Sources

- https://github.com/vercel-labs/agent-browser/issues/1036
- https://rebrowser.net/blog/how-to-fix-runtime-enable-cdp-detection-of-puppeteer-playwright-and-other-automation-libraries
- https://developer.chrome.com/blog/full-accessibility-tree
- https://www.maxdesign.com.au/articles/axtree.html
- https://www.maxdesign.com.au/articles/end-to-end-event-architecture.html
- https://www.augmentcode.com/open-source/microsoft/playwright
- https://developer.chrome.com/blog/firefox-support-in-puppeteer-with-webdriver-bidi
- https://dev.to/dennis-ddev/screenshot-diffing-pixel-level-comparison-techniques-18k
- https://github.com/microsoft/playwright/issues/7548
- https://applitools.com/blog/visual-regression-testing/
- https://www.w3.org/TR/webdriver-bidi/
