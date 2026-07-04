/* ============================================================
   Protokit · runtime
   The plumbing that makes a static mock feel like a working app:
   persistent + reactive state, a reset, theme + layout-mode appliers,
   and a tiny toast queue. Plain JS (uses the global React) so it loads
   as a normal <script> — no Babel needed. Exposed on window.Protokit.

   Load order in a demo:
     <script src="react.development.js"></script>
     <script src="protokit.js"></script>          <-- this file
     <script src="_ds_bundle.js"></script>          <-- DS components
     <script type="text/babel" src="app.jsx"></script>
   ============================================================ */
(function () {
  var R = window.React;

  /* ---------- namespacing ----------
     Each surface sets window.PK_NS (e.g. "demo.") so multiple prototypes
     in one project keep separate persisted state. */
  function ns() { return (typeof window !== "undefined" && window.PK_NS) || "pk."; }

  /* ---------- usePersistedState ----------
     Drop-in for useState that mirrors the value into localStorage under the
     active namespace. Debounced writes; survives refresh. */
  function usePersistedState(key, initial) {
    var full = ns() + key;
    var ref = R.useState(function () {
      try {
        var raw = localStorage.getItem(full);
        if (raw != null) return JSON.parse(raw);
      } catch (e) { /* ignore */ }
      return typeof initial === "function" ? initial() : initial;
    });
    var val = ref[0], setVal = ref[1];
    R.useEffect(function () {
      var id = setTimeout(function () {
        try { localStorage.setItem(full, JSON.stringify(val)); } catch (e) { /* ignore */ }
      }, 200);
      return function () { clearTimeout(id); };
    }, [full, val]);
    return [val, setVal];
  }

  /* ---------- resetState ----------
     Wipes every key in the active namespace and reloads. Wire this to the
     "Reset app state" tweak. */
  function resetState() {
    try {
      var prefix = ns();
      Object.keys(localStorage)
        .filter(function (k) { return k.indexOf(prefix) === 0; })
        .forEach(function (k) { localStorage.removeItem(k); });
    } catch (e) { /* ignore */ }
    location.reload();
  }

  /* ---------- applyTheme ----------
     Sets [data-theme] on <html> and (optionally) swaps the accent inline so
     it overrides the theme block. Pass {accent, accentPress} to rebrand. */
  function applyTheme(theme, accent) {
    var root = document.documentElement;
    root.setAttribute("data-theme", theme === "light" ? "light" : "dark");
    if (accent) {
      if (accent.accent) root.style.setProperty("--accent", accent.accent);
      if (accent.accentPress) root.style.setProperty("--accent-press", accent.accentPress);
      // let the tint recompute from the new accent in dark; set explicitly in light
      if (theme === "light" && accent.accentTint) root.style.setProperty("--accent-tint", accent.accentTint);
      else root.style.removeProperty("--accent-tint");
    }
  }

  /* ---------- applyLayout ----------
     Sets [data-mode] on <html> from a layout tweak: "mobile" | "desktop" |
     "auto". Auto resolves at a breakpoint and re-resolves on resize. */
  var LAYOUT_BREAKPOINT = 900;
  var _resizeHandler = null;
  function applyLayout(mode) {
    var root = document.documentElement;
    function resolve() {
      var m = (!mode || mode === "auto")
        ? (window.innerWidth >= LAYOUT_BREAKPOINT ? "desktop" : "mobile")
        : mode;
      root.setAttribute("data-mode", m);
    }
    resolve();
    if (_resizeHandler) window.removeEventListener("resize", _resizeHandler);
    if (!mode || mode === "auto") { _resizeHandler = resolve; window.addEventListener("resize", _resizeHandler); }
  }

  /* ---------- useToasts ----------
     Returns [push, node]. Render {node} once near the app root; call
     push("Saved") to fire a toast. Auto-dismisses. */
  function useToasts(timeout) {
    var ref = R.useState([]);
    var toasts = ref[0], setToasts = ref[1];
    var push = R.useCallback(function (msg, opts) {
      var id = Math.random().toString(36).slice(2);
      setToasts(function (t) { return t.concat([{ id: id, msg: msg, icon: (opts && opts.icon) }]); });
      setTimeout(function () {
        setToasts(function (t) { return t.filter(function (x) { return x.id !== id; }); });
      }, timeout || 2600);
    }, []);
    var checkPath = "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18M8.5 12l2.5 2.5 4.5-5";
    var node = R.createElement(
      "div", { className: "pk-toast-wrap" },
      toasts.map(function (t) {
        return R.createElement(
          "div", { key: t.id, className: "pk-toast" },
          R.createElement("svg", { width: 17, height: 17, viewBox: "0 0 24 24", fill: "none" },
            R.createElement("path", {
              d: t.icon || checkPath, stroke: "currentColor", strokeWidth: 1.7,
              strokeLinecap: "round", strokeLinejoin: "round",
            })),
          t.msg
        );
      })
    );
    return [push, node];
  }

  /* ---------- avatar helpers ----------
     Deterministic, legible tint from a name so avatars are stable across
     reloads without storing a color. */
  var TINTS = [
    "#c2410c", "#1f6f54", "#9a5b2b", "#3a6ea5", "#8a5cb0",
    "#b03a4e", "#5a7d3a", "#2f7d5b", "#a8761c", "#7a6cc4",
    "#c45d2e", "#3f7d8a", "#9d4b6f", "#5e6b2f", "#b4791f",
  ];
  function initials(name) {
    return String(name || "?").trim().split(/\s+/).slice(0, 2).map(function (w) { return w[0]; }).join("").toUpperCase();
  }
  function tintFor(name) {
    var h = 0, s = String(name || "");
    for (var i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) >>> 0;
    return TINTS[h % TINTS.length];
  }

  window.Protokit = {
    ns: ns, usePersistedState: usePersistedState, resetState: resetState,
    applyTheme: applyTheme, applyLayout: applyLayout, useToasts: useToasts,
    initials: initials, tintFor: tintFor,
  };
})();
