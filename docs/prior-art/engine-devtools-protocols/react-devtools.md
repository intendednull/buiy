**Date:** 2026-06-18
**Status:** active
**Subject:** React DevTools — the transport-agnostic Bridge/Wall protocol (incremental tree + lazy dehydrated detail + overrideValueAtPath drive), plus Testing Library getByRole as the a11y-locator analog

# React DevTools

Maintainer: **Meta (Facebook)**. **License: MIT.** Lives in the React monorepo
(`packages/react-devtools`, `react-devtools-core`, `react-devtools-shared`,
`react-devtools-inline`). Repo: https://github.com/facebook/react. Standalone
npm packages `react-devtools` / `react-devtools-core` are on the **7.x** line; the
latest published version is **7.0.1** (MIT, published 2025-10-20 — confirmed via the
npm registry endpoint `registry.npmjs.org/react-devtools/latest`).

Why this belongs in an engine-devtools-protocols folder: React DevTools is the
clearest worked example of the folder's thesis — **same substrate, different
client.** A human inspector (the Components/Profiler panel) attaches to the same
live component tree an automated client could. The instrumentation is
**transport-agnostic by construction**, and the drive path is a small, unified
mutation API. That is structurally the shape Buiy wants for its AccessKit tree
(see [lessons.md](lessons.md)).

## Layering: Backend → Agent → Bridge → Wall → Frontend

DevTools is split into a stack of layers so the **transport** is swappable:

- **Backend** — runs *inside the application's JS context*, alongside React. It
  instruments the renderer (via the `__REACT_DEVTOOLS_GLOBAL_HOOK__`), watches
  every relevant commit, and emits events. The only layer that touches fibers.
- **Agent** — receives backend events and is the backend-side party that talks to
  the frontend; it owns selection state and forwards inspect/override requests
  down to the renderer interface.
- **Bridge** — a message abstraction (`BackendBridge` / `FrontendBridge`) that
  serializes named events (`bridge.send(event, payload)`) in both directions.
  Versioned (see handshake below).
- **Wall** — the raw channel the Bridge wraps. A `Wall` is just an object with
  `listen()` and `send()`; whatever moves bytes between the two contexts plugs in
  here. This is the seam that makes the whole thing transport-agnostic.
- **Frontend** — the visible DevTools UI (Components tree + Profiler). It never
  sees a fiber; it only consumes Bridge events and renders its own mirror tree.

**Transport-agnostic in practice.** In the browser extension the Wall is
`window.postMessage` (content script ↔ page). For **React Native** and the
**standalone** app, the Wall is a **WebSocket** relay: `connectToDevTools()` in
`react-devtools-core` builds a `BackendBridge` over a WebSocket whose `host`
defaults to `localhost` and `port` defaults to **8097** (a custom `WebSocket` can
be injected to override host/port for bespoke integrations). The frontend renders
via `react-devtools-inline` or the standalone shell. Same protocol either way —
only the Wall differs.

Contrast the game engines in this folder: Godot/Unreal/Unity each bake a
*specific* socket into the engine ([godot.md](godot.md), [unreal.md](unreal.md),
[unity.md](unity.md)); React factors the transport out behind the Wall so the
*same* backend serves a browser panel and a WebSocket client unchanged.

## Inspect: incremental tree + lazy dehydrated detail

The protocol is deliberately split into a cheap, always-on **structure** stream
and an expensive, on-demand **detail** fetch.

**Structure — compact incremental operations.** Every relevant React commit emits
a single `"operations"` message encoded as a **typed array** (not JSON objects),
so steady-state tree mutations are tiny. Layout:

- entries `[0]`,`[1]`: renderer id + root id;
- a **string table** (encoded display names + keys, de-duplicated);
- then a run of operations, each led by a constant:
  - **add node** (`1`): fiber id, element type, **parent id**, **owner id**,
    display-name string-table id, key string-table id;
  - **remove node** (`2`): count, then fiber ids;
  - **re-order children** (`3`): fiber id, child count, ordered child ids;
  - plus update-tree-base-duration (`4`), update-error/warning-counts (`5`),
    remove-root (`6`), set-subtree-mode (`7`).

So the frontend learns *ids / types / parent / owner / key* incrementally and
rebuilds the tree shape from a delta stream — never a full re-serialization. From
the OVERVIEW: when an element mounts, DevTools sends "a minimal amount of
information … its display name, type, and key — but does **not** include things
like props or state."

**Detail — lazy, dehydrated, per-selection.** Props / state / hooks / context are
fetched only when a node is **selected**, via an `"inspectElement"` request
answered by `"inspectedElement"`. The payload is **dehydrated**: only a shallow
copy crosses the Bridge; nested objects/arrays are sent as placeholders and
**filled in on demand** as the user expands them. Values that can't be serialized
(functions, symbols, class instances) become **"Unserializable"** markers that
preserve the type/name without the data.

**Poll cadence.** The frontend re-requests the *selected* element's detail on a
**~1/sec** poll; if the element hasn't re-rendered, the backend returns a no-op.
(The OVERVIEW states "about once a second"; treat the precise interval as
**(unverified)** against current `react-devtools-shared` if it is load-bearing.)
This is the same observe-loop pattern an agent runtime would want over the
AccessKit tree: cheap incremental structure, fetch-on-focus detail.

## Drive: the unified mutation path

DevTools can *mutate* live dev-mode values. The drive surface was refactored
(facebook/react #19774) from four parallel methods —
**`overrideProps` / `overrideState` / `overrideHookState` / `overrideContext`** —
into **three** methods that take a `type` **discriminator** (`"props"` /
`"state"` / `"hooks"` / `"context"`):

- **`overrideValueAtPath(type, fiber, path, value)`** — set the value at a path;
- **`deletePath(type, fiber, path)`** — remove a key;
- **`renamePath(type, fiber, oldPath, newPath)`** — rename a key.

The old method names are forwarded for backward compatibility. Under the hood
these map to DEV-only reconciler hooks injected into the renderer interface
(`overridePropsDeletePath`, `overridePropsRenamePath`, `overrideHookStateDeletePath`,
`overrideHookStateRenamePath`, etc.). A separate **force-rerender** request lets
the inspector re-render a component on demand.

**Scope of the drive.** This mutates *existing instrumented values* — it cannot
arbitrarily create or destroy components. Comparable to Godot's Remote-tab
property edits and Unreal's property writes ([godot.md](godot.md),
[unreal.md](unreal.md)); weaker than Unreal's *function invoke*. The point for
Buiy: the entire drive vocabulary is a handful of `(type, path, value)`
operations against an *already-published* tree — small, total, and auditable.

## Versioned, handshake-checked protocol

The Bridge protocol is **versioned and checked when backend and frontend
connect**. A mismatch surfaces as **"Unsupported DevTools backend version"** (the
backend is older/newer than the frontend expects) or **"Unsupported Bridge
operation"** (an op the peer doesn't understand). The version negotiation gates
the session rather than silently mis-decoding the typed-array operations stream —
a deliberate fail-loud handshake. (See [open-problems.md](open-problems.md) on
protocol versioning as a cross-cutting devtools concern.)

## `getByRole({ name })` — the a11y-locator analog

Distinct from anything *inside* DevTools: **Testing Library**'s
`getByRole(role, { name })` (dom-testing-library, author **Kent C. Dodds**,
**MIT**, https://github.com/testing-library/dom-testing-library). It addresses a
live DOM node by **ARIA role + accessible name** — "find the `button` named
'Submit'" — then acts on it, rather than by a CSS selector or an internal node
id. `logRoles(container)` enumerates the available roles/names of a tree.

This matters as a contrast axis. DevTools addresses nodes by **internal fiber
ids** minted by the instrumentation — opaque, renderer-private, meaningless
outside the session. `getByRole` addresses nodes by their **semantic identity**
(role + name), which is exactly the address space a screen reader — or an LLM
agent — already reasons in. The two patterns are complementary: incremental-tree
+ override (DevTools) is the *protocol*; role+name (Testing Library / AccessKit)
is the *addressing scheme*. Buiy's bet is that the AccessKit tree gives it the
`getByRole`-style semantic address **and** the live tree to run the
DevTools-style observe/override loop over — one surface, both jobs. The role +
name + state + action model itself is documented in the sibling
[../accesskit/](../accesskit/) folder; the same role-based-locator move was made
by Playwright/WebDriver on top of the browser accessibility tree, for the same
reason (this folder does not yet have a browser-automation file — treat that as
prose, not a link).

## Implications for Buiy (pointer)

The load-bearing borrow — Wall/Bridge transport-agnosticism, incremental tree +
lazy dehydrated detail, and a small unified `(type, path, value)` override
vocabulary — is recorded as `validates` / `borrow` entries in
[lessons.md](lessons.md), not here. This file stays evidence-only.

## Sources

- React DevTools architecture overview (Backend/Bridge/Wall, operations typed-array format, dehydration, ~1/sec poll): https://github.com/facebook/react/blob/main/packages/react-devtools/OVERVIEW.md
- `react-devtools-core` backend / `connectToDevTools` (WebSocket Wall, host/port 8097, BackendBridge): https://github.com/facebook/react/blob/main/packages/react-devtools-core/src/backend.js
- `connectToDevTools` config (host=localhost, port=8097, custom WebSocket): https://snyk.io/advisor/npm-package/react-devtools-core/functions/react-devtools-core.connectToDevTools
- Unify editing methods into `overrideValueAtPath` / `deletePath` / `renamePath` with a `type` discriminator (PR #19774): https://github.com/facebook/react/commit/50d9451
- Standalone package `react-devtools` 7.0.1 (latest, MIT, published 2025-10-20): https://registry.npmjs.org/react-devtools/latest and https://www.npmjs.com/package/react-devtools
- "Unsupported backend version" handshake error: https://gist.github.com/bvaughn/4bc90775530873fdf8e7ade4a039e579
- Testing Library `getByRole`: https://testing-library.com/docs/queries/byrole/ and https://github.com/testing-library/dom-testing-library
