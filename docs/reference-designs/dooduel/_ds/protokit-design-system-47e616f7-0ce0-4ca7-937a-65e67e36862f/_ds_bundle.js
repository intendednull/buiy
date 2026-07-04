/* @ds-bundle: {"format":3,"namespace":"ProtokitDesignSystem_47e616","components":[{"name":"Avatar","sourcePath":"components/core/Avatar.jsx"},{"name":"Badge","sourcePath":"components/core/Badge.jsx"},{"name":"Button","sourcePath":"components/core/Button.jsx"},{"name":"Card","sourcePath":"components/core/Card.jsx"},{"name":"ICON_PATHS","sourcePath":"components/core/Icon.jsx"},{"name":"Icon","sourcePath":"components/core/Icon.jsx"},{"name":"IconButton","sourcePath":"components/core/IconButton.jsx"},{"name":"Dialog","sourcePath":"components/feedback/Dialog.jsx"},{"name":"Tooltip","sourcePath":"components/feedback/Tooltip.jsx"},{"name":"Checkbox","sourcePath":"components/forms/Checkbox.jsx"},{"name":"Input","sourcePath":"components/forms/Input.jsx"},{"name":"Segmented","sourcePath":"components/forms/Segmented.jsx"},{"name":"Switch","sourcePath":"components/forms/Switch.jsx"},{"name":"Tabs","sourcePath":"components/forms/Tabs.jsx"},{"name":"AppShell","sourcePath":"components/shell/AppShell.jsx"},{"name":"ThemeToggle","sourcePath":"components/shell/ThemeToggle.jsx"}],"sourceHashes":{"components/core/Avatar.jsx":"41ffa24fcb51","components/core/Badge.jsx":"769d30bc6e81","components/core/Button.jsx":"907aff466950","components/core/Card.jsx":"4bfaec34b3bc","components/core/Icon.jsx":"00d7dbf064f1","components/core/IconButton.jsx":"5ab49bf21cf2","components/feedback/Dialog.jsx":"48124e309c61","components/feedback/Tooltip.jsx":"c6cd0cd4d56d","components/forms/Checkbox.jsx":"85cda9a48eb8","components/forms/Input.jsx":"7d616280ed37","components/forms/Segmented.jsx":"a7f899dc170c","components/forms/Switch.jsx":"398274300cf1","components/forms/Tabs.jsx":"8fa4dace3ccb","components/shell/AppShell.jsx":"1c555eaba94f","components/shell/ThemeToggle.jsx":"0e63f1e32c0c","protokit.js":"a09047a4f1af","ui_kits/demo/app.jsx":"7895c106d117","ui_kits/demo/data.js":"2312342ab380","ui_kits/demo/pk-lib.jsx":"506334d428a4","ui_kits/demo/screens.jsx":"f93a686cbd54","ui_kits/demo/signin.jsx":"e576c86af826","ui_kits/demo/tweaks-panel.jsx":"6591467622ed"},"inlinedExternals":[],"unexposedExports":[]} */

(() => {

const __ds_ns = (window.ProtokitDesignSystem_47e616 = window.ProtokitDesignSystem_47e616 || {});

const __ds_scope = {};

(__ds_ns.__errors = __ds_ns.__errors || []);

// components/core/Avatar.jsx
try { (() => {
/* Avatar — initials chip with a deterministic tint derived from the name
   (stable across reloads). Pass src to show a real image instead. */
function tintFor(name) {
  const TINTS = ["#c2410c", "#1f6f54", "#9a5b2b", "#3a6ea5", "#8a5cb0", "#b03a4e", "#5a7d3a", "#2f7d5b", "#a8761c", "#7a6cc4"];
  let h = 0,
    s = String(name || "");
  for (let i = 0; i < s.length; i++) h = h * 31 + s.charCodeAt(i) >>> 0;
  return TINTS[h % TINTS.length];
}
function Avatar({
  name,
  initials,
  tint,
  src,
  size = 38,
  className = "",
  style
}) {
  const ini = initials || String(name || "?").trim().split(/\s+/).slice(0, 2).map(w => w[0]).join("").toUpperCase();
  const bg = tint || tintFor(name || ini);
  return /*#__PURE__*/React.createElement("span", {
    className: "pk-avatar " + className,
    style: {
      width: size,
      height: size,
      background: src ? "var(--surface-3)" : bg,
      fontSize: size * 0.4,
      ...style
    }
  }, src ? /*#__PURE__*/React.createElement("img", {
    src: src,
    alt: name || "",
    style: {
      width: "100%",
      height: "100%",
      objectFit: "cover"
    }
  }) : ini);
}
Object.assign(__ds_scope, { Avatar });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Avatar.jsx", error: String((e && e.message) || e) }); }

// components/core/Badge.jsx
try { (() => {
/* Badge — a small status / category pill.
   tone: neutral (default) | accent | pos | warn | danger.
   mono renders it as an uppercase mono label; dot adds a leading dot. */
function Badge({
  tone = "neutral",
  mono,
  dot,
  children,
  className = "",
  style
}) {
  const cls = ["pk-badge"];
  if (tone && tone !== "neutral") cls.push(`pk-badge--${tone}`);
  if (mono) cls.push("pk-badge--mono");
  if (dot) cls.push("pk-badge--dot");
  if (className) cls.push(className);
  return /*#__PURE__*/React.createElement("span", {
    className: cls.join(" "),
    style: style
  }, children);
}
Object.assign(__ds_scope, { Badge });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Badge.jsx", error: String((e && e.message) || e) }); }

// components/core/Card.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/* Card — the base surface. pad adds standard padding; tone="2" uses the
   subtle surface; flat drops the shadow. Compose freely. */
function Card({
  pad,
  tone,
  flat,
  as = "div",
  children,
  className = "",
  style,
  ...rest
}) {
  const cls = ["pk-card"];
  if (pad) cls.push("pk-card--pad");
  if (tone === "2") cls.push("pk-card--2");
  if (flat) cls.push("pk-card--flat");
  if (className) cls.push(className);
  const El = as;
  return /*#__PURE__*/React.createElement(El, _extends({
    className: cls.join(" "),
    style: style
  }, rest), children);
}
Object.assign(__ds_scope, { Card });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Card.jsx", error: String((e && e.message) || e) }); }

// components/core/Icon.jsx
try { (() => {
/* Protokit icon set — 24×24, stroke 1.7, round caps/joins, currentColor.
   One component, one path map. No emoji, no icon-font dependency. Add an
   entry here and it's instantly available by name everywhere. */
const ICON_PATHS = {
  // nav / app
  home: "M3 10.5 12 3l9 7.5M5 9.5V20h5v-6h4v6h5V9.5",
  inbox: "M3 13h4l1.5 3h7L17 13h4M5 5h14l2 8v5a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-5z",
  list: "M8 6h13M8 12h13M8 18h13M3.5 6h.01M3.5 12h.01M3.5 18h.01",
  grid: "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z",
  calendar: "M4 6h16v15H4zM4 10h16M8 3v4M16 3v4",
  chart: "M4 20V4M4 20h16M8 16v-5M12 16V7M16 16v-8M20 16v-3",
  members: "M16 19v-1.5a3.5 3.5 0 0 0-3.5-3.5h-5A3.5 3.5 0 0 0 4 17.5V19M9.5 11a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7M20 19v-1.5a3.5 3.5 0 0 0-2.6-3.4M15 4.2a3.5 3.5 0 0 1 0 6.6",
  user: "M16 19v-1.5a3.5 3.5 0 0 0-3.5-3.5h-1A3.5 3.5 0 0 0 8 17.5V19M12 11a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7",
  settings: "M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7M19.4 12a7.6 7.6 0 0 0-.1-1.3l2-1.6-2-3.4-2.4 1a7.6 7.6 0 0 0-2.2-1.3l-.4-2.5H10l-.4 2.5a7.6 7.6 0 0 0-2.2 1.3l-2.4-1-2 3.4 2 1.6a7.6 7.6 0 0 0 0 2.6l-2 1.6 2 3.4 2.4-1a7.6 7.6 0 0 0 2.2 1.3l.4 2.5h4l.4-2.5a7.6 7.6 0 0 0 2.2-1.3l2.4 1 2-3.4-2-1.6c.07-.43.1-.86.1-1.3",
  bell: "M18 8.5a6 6 0 1 0-12 0c0 6-2.5 7.5-2.5 7.5h17S18 14.5 18 8.5M10 20a2 2 0 0 0 4 0",
  search: "M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14M20 20l-4-4",
  // task / status
  circle: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18",
  check: "M4 12.5 9 17.5 20 6.5",
  checkCircle: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18M8.5 12l2.5 2.5 4.5-5",
  flag: "M5 21V4M5 4h11l-2 4 2 4H5",
  star: "M12 3.5l2.6 5.3 5.9.86-4.25 4.14 1 5.85L12 16.9l-5.25 2.75 1-5.85L3.5 9.66l5.9-.86z",
  clock: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18M12 7v5l3.5 2",
  tag: "M3.5 12.5 11 5h6.5V11.5L10 19a1.5 1.5 0 0 1-2 0l-4.5-4.5a1.5 1.5 0 0 1 0-2M15 8.5h.01",
  spark: "M12 3v4M12 17v4M5 12H3M21 12h-2M6.3 6.3 4.9 4.9M19.1 19.1l-1.4-1.4M6.3 17.7l-1.4 1.4M19.1 4.9l-1.4 1.4M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6",
  sparkles: "M12 4l1.6 4.4L18 10l-4.4 1.6L12 16l-1.6-4.4L6 10l4.4-1.6zM18 15l.8 2.2L21 18l-2.2.8L18 21l-.8-2.2L15 18l2.2-.8z",
  bolt: "M13 3 5 13h6l-1 8 8-10h-6z",
  // arrows / chevrons
  plus: "M12 5v14M5 12h14",
  minus: "M5 12h14",
  chevR: "M9 5l7 7-7 7",
  chevL: "M15 5l-7 7 7 7",
  chevD: "M6 9l6 6 6-6",
  chevU: "M6 15l6-6 6 6",
  arrowR: "M5 12h14M13 6l6 6-6 6",
  arrowUp: "M12 19V5M6 11l6-6 6 6",
  back: "M19 12H5M11 18l-6-6 6-6",
  x: "M6 6l12 12M18 6 6 18",
  external: "M14 4h6v6M20 4l-9 9M18 13v6a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h6",
  // actions
  edit: "M4 20h4L19 9a2 2 0 0 0-3-3L5 17zM14 7l3 3",
  trash: "M4 7h16M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2M6 7l1 13h10l1-13",
  copy: "M9 9h10a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H9a1 1 0 0 1-1-1V10a1 1 0 0 1 1-1M5 15H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v1",
  filter: "M3 5h18l-7 8v6l-4 2v-8z",
  sort: "M7 4v16M7 20l-3-3M7 4l3 3M17 20V4M17 4l3 3M17 20l-3-3",
  more: "M12 6h.01M12 12h.01M12 18h.01",
  pin: "M12 21s7-5.5 7-11a7 7 0 1 0-14 0c0 5.5 7 11 7 11M12 12.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5",
  link: "M9 15l6-6M10.5 6.5 12 5a4 4 0 0 1 6 6l-1.5 1.5M13.5 17.5 12 19a4 4 0 0 1-6-6l1.5-1.5",
  // misc
  info: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18M12 11v5M12 8h.01",
  alert: "M12 3 2 20h20zM12 10v4M12 17h.01",
  lock: "M6 11h12v9H6zM8 11V8a4 4 0 0 1 8 0v3",
  mail: "M3 7a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2zM3.5 7.5l8.5 6 8.5-6",
  logout: "M9 4H6a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h3M16 17l5-5-5-5M21 12H9",
  heart: "M12 20s-7-4.7-7-10a4 4 0 0 1 7-2.6A4 4 0 0 1 19 10c0 5.3-7 10-7 10",
  dollar: "M12 3v18M16 7.5c0-1.7-1.8-3-4-3s-4 1.3-4 3 1.8 3 4 3 4 1.3 4 3-1.8 3-4 3-4-1.3-4-3",
  folder: "M3 7a2 2 0 0 1 2-2h4l2 2h6a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
  doc: "M6 3h8l4 4v14H6zM14 3v4h4",
  play: "M7 5l12 7-12 7z",
  pause: "M9 5v14M15 5v14",
  // theme
  sun: "M12 17a5 5 0 1 0 0-10 5 5 0 0 0 0 10M12 1.5v2.5M12 20v2.5M4 4l1.8 1.8M18.2 18.2 20 20M1.5 12H4M20 12h2.5M4 20l1.8-1.8M18.2 5.8 20 4",
  moon: "M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"
};
function Icon({
  name,
  size = 18,
  stroke = 1.7,
  fill = false,
  className,
  style
}) {
  const d = ICON_PATHS[name];
  if (!d) return null;
  return /*#__PURE__*/React.createElement("svg", {
    width: size,
    height: size,
    viewBox: "0 0 24 24",
    fill: "none",
    className: className,
    style: style,
    "aria-hidden": "true"
  }, /*#__PURE__*/React.createElement("path", {
    d: d,
    stroke: "currentColor",
    strokeWidth: stroke,
    strokeLinecap: "round",
    strokeLinejoin: "round",
    fill: fill ? "currentColor" : "none"
  }));
}
Object.assign(__ds_scope, { ICON_PATHS, Icon });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Icon.jsx", error: String((e && e.message) || e) }); }

// components/core/Button.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/* Button — the primary action primitive.
   Variants map to brand intents; size + icon + block are orthogonal. */
function Button({
  variant = "ghost",
  size,
  icon,
  iconRight,
  block,
  children,
  className = "",
  ...rest
}) {
  const cls = ["pk-btn", `pk-btn--${variant}`];
  if (size === "lg") cls.push("pk-btn--lg");
  if (size === "sm") cls.push("pk-btn--sm");
  if (block) cls.push("pk-btn--block");
  if (className) cls.push(className);
  const iconSize = size === "lg" ? 19 : size === "sm" ? 15 : 17;
  return /*#__PURE__*/React.createElement("button", _extends({
    className: cls.join(" ")
  }, rest), icon && /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: icon,
    size: iconSize
  }), children, iconRight && /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: iconRight,
    size: iconSize
  }));
}
Object.assign(__ds_scope, { Button });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/Button.jsx", error: String((e && e.message) || e) }); }

// components/core/IconButton.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/* IconButton — a square/round button holding a single icon.
   variant: "default" (bordered) | "bare" (no chrome until hover). */
function IconButton({
  icon,
  size = "md",
  variant = "default",
  title,
  children,
  className = "",
  ...rest
}) {
  const cls = ["pk-iconbtn"];
  if (size === "sm") cls.push("pk-iconbtn--sm");
  if (variant === "bare") cls.push("pk-iconbtn--bare");
  if (className) cls.push(className);
  return /*#__PURE__*/React.createElement("button", _extends({
    className: cls.join(" "),
    title: title,
    "aria-label": title
  }, rest), icon ? /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: icon,
    size: size === "sm" ? 16 : 18
  }) : children);
}
Object.assign(__ds_scope, { IconButton });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/core/IconButton.jsx", error: String((e && e.message) || e) }); }

// components/feedback/Dialog.jsx
try { (() => {
/* Dialog — a modal overlay. align="center" (default) renders a centered
   card; align="sheet" renders a bottom sheet (great on mobile). Esc and
   scrim-click close. Compose header/body/footer inside as children. */
function Dialog({
  open = true,
  onClose,
  align = "center",
  maxW = 460,
  children
}) {
  React.useEffect(() => {
    if (!open) return undefined;
    const onKey = e => {
      if (e.key === "Escape") onClose && onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);
  if (!open) return null;
  const sheet = align === "sheet";
  return /*#__PURE__*/React.createElement("div", {
    onClick: onClose,
    style: {
      position: "fixed",
      inset: 0,
      zIndex: 500,
      background: "rgba(8,10,14,.46)",
      backdropFilter: "blur(3px)",
      WebkitBackdropFilter: "blur(3px)",
      display: "flex",
      alignItems: sheet ? "flex-end" : "center",
      justifyContent: "center",
      padding: sheet ? 0 : 20,
      animation: "scrim-in .2s ease"
    }
  }, /*#__PURE__*/React.createElement("div", {
    onClick: e => e.stopPropagation(),
    className: "pk-card scroll",
    style: {
      width: "100%",
      maxWidth: sheet ? 520 : maxW,
      maxHeight: "92vh",
      overflowY: "auto",
      borderRadius: sheet ? "var(--r-xl) var(--r-xl) 0 0" : "var(--r-xl)",
      boxShadow: "var(--sh-lg)",
      animation: sheet ? "sheet-up var(--dur-4) var(--ease)" : "pop-in var(--dur-3) var(--ease)"
    }
  }, children));
}
Object.assign(__ds_scope, { Dialog });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/feedback/Dialog.jsx", error: String((e && e.message) || e) }); }

// components/feedback/Tooltip.jsx
try { (() => {
/* Tooltip — wraps a trigger; shows a label on hover/focus above it. */
function Tooltip({
  label,
  children,
  className = ""
}) {
  const [show, setShow] = React.useState(false);
  return /*#__PURE__*/React.createElement("span", {
    className: "pk-tip-wrap " + className,
    onMouseEnter: () => setShow(true),
    onMouseLeave: () => setShow(false),
    onFocus: () => setShow(true),
    onBlur: () => setShow(false)
  }, children, show && /*#__PURE__*/React.createElement("span", {
    className: "pk-tip",
    role: "tooltip"
  }, label));
}
Object.assign(__ds_scope, { Tooltip });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/feedback/Tooltip.jsx", error: String((e && e.message) || e) }); }

// components/forms/Checkbox.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/* Checkbox — a labelled checkbox. Controlled via checked + onChange. */
function Checkbox({
  checked,
  onChange,
  label,
  className = "",
  style,
  ...rest
}) {
  const box = /*#__PURE__*/React.createElement("input", _extends({
    type: "checkbox",
    className: "pk-check",
    checked: !!checked,
    onChange: e => onChange && onChange(e.target.checked)
  }, rest));
  if (!label) return box;
  return /*#__PURE__*/React.createElement("label", {
    className: "row gap-10 " + className,
    style: {
      cursor: "pointer",
      fontSize: "var(--text-base)",
      color: "var(--ink)",
      ...style
    }
  }, box, /*#__PURE__*/React.createElement("span", null, label));
}
Object.assign(__ds_scope, { Checkbox });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Checkbox.jsx", error: String((e && e.message) || e) }); }

// components/forms/Input.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/* Input — a labelled text field. Renders a <label>+<input> with optional
   hint. Pass multiline for a textarea, or prefix for a leading affix. */
function Input({
  label,
  hint,
  prefix,
  multiline,
  className = "",
  style,
  id,
  ...rest
}) {
  const inputId = id || (label ? "in-" + label.replace(/\W+/g, "-").toLowerCase() : undefined);
  const control = multiline ? /*#__PURE__*/React.createElement("textarea", _extends({
    id: inputId,
    className: "pk-textarea"
  }, rest)) : prefix ? /*#__PURE__*/React.createElement("div", {
    className: "pk-input",
    style: {
      display: "flex",
      alignItems: "center",
      padding: "0 0 0 14px",
      gap: 4
    }
  }, /*#__PURE__*/React.createElement("span", {
    className: "mono",
    style: {
      color: "var(--muted)"
    }
  }, prefix), /*#__PURE__*/React.createElement("input", _extends({
    id: inputId
  }, rest, {
    style: {
      flex: 1,
      minWidth: 0,
      height: 44,
      border: 0,
      outline: "none",
      background: "transparent",
      color: "var(--ink)",
      font: "inherit",
      padding: "0 14px 0 4px"
    }
  }))) : /*#__PURE__*/React.createElement("input", _extends({
    id: inputId,
    className: "pk-input"
  }, rest));
  if (!label && !hint) return prefix ? control : React.cloneElement(control, {
    className: (control.props.className || "") + " " + className,
    style
  });
  return /*#__PURE__*/React.createElement("div", {
    className: "pk-field " + className,
    style: style
  }, label && /*#__PURE__*/React.createElement("label", {
    htmlFor: inputId
  }, label), control, hint && /*#__PURE__*/React.createElement("span", {
    className: "pk-hint"
  }, hint));
}
Object.assign(__ds_scope, { Input });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Input.jsx", error: String((e && e.message) || e) }); }

// components/forms/Segmented.jsx
try { (() => {
/* Segmented — a compact pill segmented control for 2–4 short options.
   options: string[] or { value, label }[]. Controlled via value+onChange. */
function Segmented({
  value,
  onChange,
  options,
  className = "",
  style
}) {
  return /*#__PURE__*/React.createElement("div", {
    className: "pk-seg " + className,
    style: style,
    role: "radiogroup"
  }, options.map(o => {
    const v = typeof o === "object" ? o.value : o;
    const l = typeof o === "object" ? o.label : o;
    return /*#__PURE__*/React.createElement("button", {
      key: v,
      type: "button",
      role: "radio",
      "aria-checked": v === value,
      "data-on": v === value ? "1" : "0",
      onClick: () => onChange && onChange(v)
    }, l);
  }));
}
Object.assign(__ds_scope, { Segmented });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Segmented.jsx", error: String((e && e.message) || e) }); }

// components/forms/Switch.jsx
try { (() => {
/* Switch — an on/off toggle. Controlled via on + onChange(next:boolean). */
function Switch({
  on,
  onChange,
  label,
  className = "",
  style
}) {
  const toggle = /*#__PURE__*/React.createElement("button", {
    type: "button",
    className: "pk-switch",
    "data-on": on ? "1" : "0",
    role: "switch",
    "aria-checked": !!on,
    onClick: () => onChange && onChange(!on)
  }, /*#__PURE__*/React.createElement("i", null));
  if (!label) return toggle;
  return /*#__PURE__*/React.createElement("label", {
    className: "row between gap-12 " + className,
    style: {
      cursor: "pointer",
      ...style
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      fontSize: "var(--text-base)",
      color: "var(--ink)"
    }
  }, label), toggle);
}
Object.assign(__ds_scope, { Switch });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Switch.jsx", error: String((e && e.message) || e) }); }

// components/forms/Tabs.jsx
try { (() => {
/* Tabs — an underline tab bar. options: string[] or { value, label, badge }[].
   Controlled via value + onChange. */
function Tabs({
  value,
  onChange,
  options,
  className = "",
  style
}) {
  return /*#__PURE__*/React.createElement("div", {
    className: "pk-tabs " + className,
    style: style,
    role: "tablist"
  }, options.map(o => {
    const v = typeof o === "object" ? o.value : o;
    const l = typeof o === "object" ? o.label : o;
    const badge = typeof o === "object" ? o.badge : null;
    return /*#__PURE__*/React.createElement("button", {
      key: v,
      type: "button",
      role: "tab",
      "aria-selected": v === value,
      className: "pk-tab",
      "data-on": v === value ? "1" : "0",
      onClick: () => onChange && onChange(v)
    }, l, badge != null && /*#__PURE__*/React.createElement("span", {
      className: "num",
      style: {
        marginLeft: 7,
        fontSize: "var(--text-xs)",
        color: "var(--muted)"
      }
    }, badge));
  }));
}
Object.assign(__ds_scope, { Tabs });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/forms/Tabs.jsx", error: String((e && e.message) || e) }); }

// components/shell/ThemeToggle.jsx
try { (() => {
/* ThemeToggle — a sun/moon segmented switch that lives in the app chrome,
   always visible. Controlled: theme ("light"|"dark") + onChange. */
function ThemeToggle({
  theme = "dark",
  onChange,
  className = ""
}) {
  const dark = theme === "dark";
  return /*#__PURE__*/React.createElement("div", {
    className: "pk-theme-toggle " + className,
    role: "group",
    "aria-label": "Color theme"
  }, /*#__PURE__*/React.createElement("button", {
    type: "button",
    "data-on": !dark ? "1" : "0",
    "aria-pressed": !dark,
    title: "Light",
    "aria-label": "Light theme",
    onClick: () => onChange && onChange("light")
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "sun",
    size: 15
  })), /*#__PURE__*/React.createElement("button", {
    type: "button",
    "data-on": dark ? "1" : "0",
    "aria-pressed": dark,
    title: "Dark",
    "aria-label": "Dark theme",
    onClick: () => onChange && onChange("dark")
  }, /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: "moon",
    size: 15
  })));
}
Object.assign(__ds_scope, { ThemeToggle });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/shell/ThemeToggle.jsx", error: String((e && e.message) || e) }); }

// components/shell/AppShell.jsx
try { (() => {
/* AppShell — the responsive product chrome. On desktop it's a fixed left
   sidebar (brand + nav + theme toggle in the footer); on mobile the same
   nav reflows into a top bar + bottom tab bar. Driven by [data-mode] on
   <html> (set by Protokit.applyLayout). The phone-card framing for
   mobile-on-desktop is handled by shell.css.

   Props:
     brand      node rendered in the sidebar head / top bar (logo, name)
     nav        [{ id, label, icon, badge }]
     active     id of the current nav item
     onNavigate (id) => void
     theme, onSetTheme   wired to ThemeToggle (always visible)
     headerRight  optional node for the top-bar right slot (mobile)
     sidebarFoot  optional node above the theme toggle (e.g. account row)
     children   the screen content (placed in a scroll region) */
function AppShell({
  brand,
  nav = [],
  active,
  onNavigate,
  theme = "dark",
  onSetTheme,
  headerRight,
  sidebarFoot,
  children
}) {
  const go = id => onNavigate && onNavigate(id);
  const NavItem = ({
    item
  }) => /*#__PURE__*/React.createElement("button", {
    type: "button",
    className: "pk-nav-item",
    "data-on": item.id === active ? "1" : "0",
    onClick: () => go(item.id)
  }, item.icon && /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: item.icon,
    size: 19
  }), /*#__PURE__*/React.createElement("span", {
    className: "grow"
  }, item.label), item.badge != null && /*#__PURE__*/React.createElement("span", {
    className: "pk-nav-badge"
  }, item.badge));
  // bottom bar shows up to 5 destinations
  const bottomNav = nav.slice(0, 5);
  return /*#__PURE__*/React.createElement("div", {
    className: "pk-app"
  }, /*#__PURE__*/React.createElement("aside", {
    className: "pk-sidebar"
  }, /*#__PURE__*/React.createElement("div", {
    className: "pk-sidebar-head"
  }, brand), /*#__PURE__*/React.createElement("nav", {
    className: "pk-nav"
  }, nav.map(item => /*#__PURE__*/React.createElement(NavItem, {
    key: item.id,
    item: item
  }))), /*#__PURE__*/React.createElement("div", {
    className: "pk-sidebar-foot"
  }, sidebarFoot || /*#__PURE__*/React.createElement("span", null), /*#__PURE__*/React.createElement(__ds_scope.ThemeToggle, {
    theme: theme,
    onChange: onSetTheme
  }))), /*#__PURE__*/React.createElement("div", {
    className: "pk-main"
  }, /*#__PURE__*/React.createElement("header", {
    className: "pk-topbar"
  }, brand, /*#__PURE__*/React.createElement("div", {
    className: "row gap-8"
  }, headerRight, /*#__PURE__*/React.createElement(__ds_scope.ThemeToggle, {
    theme: theme,
    onChange: onSetTheme
  }))), /*#__PURE__*/React.createElement("main", {
    className: "pk-screen scroll"
  }, /*#__PURE__*/React.createElement("div", {
    className: "pk-screen-inner"
  }, children)), /*#__PURE__*/React.createElement("nav", {
    className: "pk-bottom"
  }, bottomNav.map(item => /*#__PURE__*/React.createElement("button", {
    key: item.id,
    type: "button",
    "data-on": item.id === active ? "1" : "0",
    onClick: () => go(item.id)
  }, item.icon && /*#__PURE__*/React.createElement(__ds_scope.Icon, {
    name: item.icon,
    size: 21
  }), /*#__PURE__*/React.createElement("span", null, item.label))))));
}
Object.assign(__ds_scope, { AppShell });
})(); } catch (e) { __ds_ns.__errors.push({ path: "components/shell/AppShell.jsx", error: String((e && e.message) || e) }); }

// protokit.js
try { (() => {
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
  function ns() {
    return typeof window !== "undefined" && window.PK_NS || "pk.";
  }

  /* ---------- usePersistedState ----------
     Drop-in for useState that mirrors the value into localStorage under the
     active namespace. Debounced writes; survives refresh. */
  function usePersistedState(key, initial) {
    var full = ns() + key;
    var ref = R.useState(function () {
      try {
        var raw = localStorage.getItem(full);
        if (raw != null) return JSON.parse(raw);
      } catch (e) {/* ignore */}
      return typeof initial === "function" ? initial() : initial;
    });
    var val = ref[0],
      setVal = ref[1];
    R.useEffect(function () {
      var id = setTimeout(function () {
        try {
          localStorage.setItem(full, JSON.stringify(val));
        } catch (e) {/* ignore */}
      }, 200);
      return function () {
        clearTimeout(id);
      };
    }, [full, val]);
    return [val, setVal];
  }

  /* ---------- resetState ----------
     Wipes every key in the active namespace and reloads. Wire this to the
     "Reset app state" tweak. */
  function resetState() {
    try {
      var prefix = ns();
      Object.keys(localStorage).filter(function (k) {
        return k.indexOf(prefix) === 0;
      }).forEach(function (k) {
        localStorage.removeItem(k);
      });
    } catch (e) {/* ignore */}
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
      if (theme === "light" && accent.accentTint) root.style.setProperty("--accent-tint", accent.accentTint);else root.style.removeProperty("--accent-tint");
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
      var m = !mode || mode === "auto" ? window.innerWidth >= LAYOUT_BREAKPOINT ? "desktop" : "mobile" : mode;
      root.setAttribute("data-mode", m);
    }
    resolve();
    if (_resizeHandler) window.removeEventListener("resize", _resizeHandler);
    if (!mode || mode === "auto") {
      _resizeHandler = resolve;
      window.addEventListener("resize", _resizeHandler);
    }
  }

  /* ---------- useToasts ----------
     Returns [push, node]. Render {node} once near the app root; call
     push("Saved") to fire a toast. Auto-dismisses. */
  function useToasts(timeout) {
    var ref = R.useState([]);
    var toasts = ref[0],
      setToasts = ref[1];
    var push = R.useCallback(function (msg, opts) {
      var id = Math.random().toString(36).slice(2);
      setToasts(function (t) {
        return t.concat([{
          id: id,
          msg: msg,
          icon: opts && opts.icon
        }]);
      });
      setTimeout(function () {
        setToasts(function (t) {
          return t.filter(function (x) {
            return x.id !== id;
          });
        });
      }, timeout || 2600);
    }, []);
    var checkPath = "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18M8.5 12l2.5 2.5 4.5-5";
    var node = R.createElement("div", {
      className: "pk-toast-wrap"
    }, toasts.map(function (t) {
      return R.createElement("div", {
        key: t.id,
        className: "pk-toast"
      }, R.createElement("svg", {
        width: 17,
        height: 17,
        viewBox: "0 0 24 24",
        fill: "none"
      }, R.createElement("path", {
        d: t.icon || checkPath,
        stroke: "currentColor",
        strokeWidth: 1.7,
        strokeLinecap: "round",
        strokeLinejoin: "round"
      })), t.msg);
    }));
    return [push, node];
  }

  /* ---------- avatar helpers ----------
     Deterministic, legible tint from a name so avatars are stable across
     reloads without storing a color. */
  var TINTS = ["#c2410c", "#1f6f54", "#9a5b2b", "#3a6ea5", "#8a5cb0", "#b03a4e", "#5a7d3a", "#2f7d5b", "#a8761c", "#7a6cc4", "#c45d2e", "#3f7d8a", "#9d4b6f", "#5e6b2f", "#b4791f"];
  function initials(name) {
    return String(name || "?").trim().split(/\s+/).slice(0, 2).map(function (w) {
      return w[0];
    }).join("").toUpperCase();
  }
  function tintFor(name) {
    var h = 0,
      s = String(name || "");
    for (var i = 0; i < s.length; i++) h = h * 31 + s.charCodeAt(i) >>> 0;
    return TINTS[h % TINTS.length];
  }
  window.Protokit = {
    ns: ns,
    usePersistedState: usePersistedState,
    resetState: resetState,
    applyTheme: applyTheme,
    applyLayout: applyLayout,
    useToasts: useToasts,
    initials: initials,
    tintFor: tintFor
  };
})();
})(); } catch (e) { __ds_ns.__errors.push({ path: "protokit.js", error: String((e && e.message) || e) }); }

// ui_kits/demo/app.jsx
try { (() => {
/* ============================================================
   Protokit demo · app root
   Persisted phase machine (signin → app), persisted task state, the
   responsive AppShell, and the two required tweaks (Reset · Layout).
   The light/dark toggle lives in the app chrome, always visible.
   ============================================================ */
const APP = window.ProtokitDesignSystem_47e616;
const {
  AppShell,
  Button,
  IconButton,
  Icon,
  Avatar,
  Badge
} = APP;
const {
  usePersistedState,
  useToasts,
  applyTheme,
  applyLayout,
  resetState
} = window.Protokit;
const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
  "layoutMode": "auto"
} /*EDITMODE-END*/;
const NAV = [{
  id: "today",
  label: "Today",
  icon: "home"
}, {
  id: "all",
  label: "All tasks",
  icon: "list"
}, {
  id: "upcoming",
  label: "Upcoming",
  icon: "calendar"
}, {
  id: "done",
  label: "Done",
  icon: "checkCircle"
}];
function greeting() {
  const h = new Date().getHours();
  return h < 12 ? "Good morning" : h < 18 ? "Good afternoon" : "Good evening";
}
function fullDate() {
  return new Date().toLocaleDateString("en-US", {
    weekday: "long",
    month: "long",
    day: "numeric"
  });
}
function Root() {
  const [t, setTweak] = useTweaks(TWEAK_DEFAULTS);
  const [theme, setTheme] = usePersistedState("theme", "dark");
  const [phase, setPhase] = usePersistedState("phase", "signin");
  const [tasks, setTasks] = usePersistedState("tasks", () => window.DEMO.seedTasks());
  const [tab, setTab] = usePersistedState("tab", "today");
  const [openId, setOpenId] = React.useState(null);
  const [push, toastNode] = useToasts();
  React.useEffect(() => {
    applyTheme(theme);
  }, [theme]);
  React.useEffect(() => {
    applyLayout(t.layoutMode);
  }, [t.layoutMode]);
  const setThemePersist = v => {
    setTheme(v);
    applyTheme(v);
  };

  /* ---- task ops ---- */
  const addTask = ({
    title,
    project
  }) => {
    const task = {
      id: "t" + Date.now(),
      title,
      done: false,
      project: project || "inbox",
      due: null,
      priority: "none",
      notes: "",
      created: Date.now()
    };
    setTasks(all => [task, ...all]);
    push("Task added");
  };
  const toggleTask = id => {
    setTasks(all => all.map(x => x.id === id ? {
      ...x,
      done: !x.done
    } : x));
    const wasDone = tasks.find(x => x.id === id)?.done;
    if (!wasDone) push("Nice work — task done", {
      icon: "checkCircle"
    });
  };
  const changeTask = (id, patch) => setTasks(all => all.map(x => x.id === id ? {
    ...x,
    ...patch
  } : x));
  const deleteTask = id => {
    setTasks(all => all.filter(x => x.id !== id));
    setOpenId(null);
    push("Task deleted", {
      icon: "trash"
    });
  };

  /* ---- derived ---- */
  const active = tasks.filter(x => !x.done);
  const counts = {
    today: active.filter(x => window.DEMO.bucket(x) === "today").length,
    all: active.length,
    upcoming: active.filter(x => ["upcoming", "someday"].includes(window.DEMO.bucket(x))).length,
    done: tasks.filter(x => x.done).length
  };
  const navItems = NAV.map(n => ({
    ...n,
    badge: counts[n.id] ? String(counts[n.id]) : null
  }));
  const openTask = tasks.find(x => x.id === openId) || null;
  if (phase === "signin") {
    return /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement(SignIn, {
      theme: theme,
      onSetTheme: setThemePersist,
      onAuthed: () => {
        setPhase("app");
        setTab("today");
      }
    }), /*#__PURE__*/React.createElement(DemoTweaks, {
      t: t,
      setTweak: setTweak
    }), toastNode);
  }
  const brand = /*#__PURE__*/React.createElement("span", {
    className: "brand-mark"
  }, /*#__PURE__*/React.createElement("span", {
    className: "brand-glyph"
  }, "P"), " Protokit");
  const sidebarFoot = /*#__PURE__*/React.createElement("div", {
    className: "acct"
  }, /*#__PURE__*/React.createElement(Avatar, {
    name: window.DEMO.user.name,
    size: 32
  }), /*#__PURE__*/React.createElement("div", {
    className: "col",
    style: {
      minWidth: 0
    }
  }, /*#__PURE__*/React.createElement("span", {
    className: "acct-name"
  }, window.DEMO.user.name), /*#__PURE__*/React.createElement("span", {
    className: "acct-mail"
  }, window.DEMO.user.email)));
  return /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement(AppShell, {
    brand: brand,
    nav: navItems,
    active: tab,
    onNavigate: setTab,
    theme: theme,
    onSetTheme: setThemePersist,
    sidebarFoot: sidebarFoot,
    headerRight: /*#__PURE__*/React.createElement(IconButton, {
      icon: "logout",
      title: "Sign out",
      variant: "bare",
      onClick: () => setPhase("signin")
    })
  }, /*#__PURE__*/React.createElement(Screen, {
    tab: tab,
    tasks: tasks,
    counts: counts,
    onAdd: addTask,
    onToggle: toggleTask,
    onOpen: x => setOpenId(x.id),
    onSignOut: () => setPhase("signin")
  })), /*#__PURE__*/React.createElement(TaskDetail, {
    task: openTask,
    onClose: () => setOpenId(null),
    onChange: changeTask,
    onToggle: toggleTask,
    onDelete: deleteTask
  }), /*#__PURE__*/React.createElement(DemoTweaks, {
    t: t,
    setTweak: setTweak
  }), toastNode);
}

/* ---- per-tab screen ---- */
function Screen({
  tab,
  tasks,
  counts,
  onAdd,
  onToggle,
  onOpen,
  onSignOut
}) {
  const active = tasks.filter(x => !x.done);
  const byCreated = (a, b) => b.created - a.created;
  const byDue = (a, b) => (a.due || "9999").localeCompare(b.due || "9999");
  if (tab === "today") {
    const list = active.filter(x => window.DEMO.bucket(x) === "today").sort(byDue);
    return /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement("div", {
      className: "screen-head"
    }, /*#__PURE__*/React.createElement("div", {
      className: "greeting eyebrow"
    }, greeting(), ", ", window.DEMO.user.name.split(" ")[0], " \xB7 ", fullDate()), /*#__PURE__*/React.createElement("h1", {
      className: "display",
      style: {
        fontSize: 34
      }
    }, "Today")), /*#__PURE__*/React.createElement("div", {
      className: "pk-grid-stats",
      style: {
        marginBottom: 22
      }
    }, /*#__PURE__*/React.createElement(Stat, {
      label: "Due today",
      value: counts.today,
      tone: "var(--accent)"
    }), /*#__PURE__*/React.createElement(Stat, {
      label: "Upcoming",
      value: counts.upcoming
    }), /*#__PURE__*/React.createElement(Stat, {
      label: "Completed",
      value: counts.done,
      tone: "var(--pos)"
    }), /*#__PURE__*/React.createElement(Stat, {
      label: "All active",
      value: counts.all
    })), /*#__PURE__*/React.createElement(Composer, {
      onAdd: onAdd,
      project: "inbox"
    }), list.length ? /*#__PURE__*/React.createElement(TaskList, {
      tasks: list,
      onToggle: onToggle,
      onOpen: onOpen,
      group: true
    }) : /*#__PURE__*/React.createElement(EmptyState, {
      icon: "checkCircle",
      title: "Inbox zero for today",
      sub: "Nothing due. Add something above or enjoy the calm."
    }));
  }
  if (tab === "all") {
    const list = active.sort(byCreated);
    return /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement(ScreenHead, {
      title: "All tasks",
      count: counts.all,
      sub: "Everything active, newest first"
    }), /*#__PURE__*/React.createElement(Composer, {
      onAdd: onAdd,
      project: "inbox"
    }), list.length ? /*#__PURE__*/React.createElement(TaskList, {
      tasks: list,
      onToggle: onToggle,
      onOpen: onOpen
    }) : /*#__PURE__*/React.createElement(EmptyState, {
      title: "No active tasks",
      sub: "You're all caught up."
    }));
  }
  if (tab === "upcoming") {
    const list = active.filter(x => ["upcoming", "someday"].includes(window.DEMO.bucket(x))).sort(byDue);
    const dated = list.filter(x => x.due);
    const someday = list.filter(x => !x.due);
    return /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement(ScreenHead, {
      title: "Upcoming",
      count: counts.upcoming,
      sub: "Scheduled ahead, plus someday/maybe"
    }), dated.length > 0 && /*#__PURE__*/React.createElement(TaskList, {
      tasks: dated,
      onToggle: onToggle,
      onOpen: onOpen
    }), someday.length > 0 && /*#__PURE__*/React.createElement("div", {
      style: {
        marginTop: 16
      }
    }, /*#__PURE__*/React.createElement("div", {
      className: "list-sect"
    }, "Someday"), /*#__PURE__*/React.createElement(TaskList, {
      tasks: someday,
      onToggle: onToggle,
      onOpen: onOpen
    })), !list.length && /*#__PURE__*/React.createElement(EmptyState, {
      icon: "calendar",
      title: "Nothing on the horizon",
      sub: "Schedule a task to see it here."
    }));
  }

  // done
  const list = tasks.filter(x => x.done).sort(byCreated);
  return /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement(ScreenHead, {
    title: "Done",
    count: counts.done,
    sub: "Completed tasks \u2014 tap to reopen"
  }), list.length ? /*#__PURE__*/React.createElement(TaskList, {
    tasks: list,
    onToggle: onToggle,
    onOpen: onOpen
  }) : /*#__PURE__*/React.createElement(EmptyState, {
    title: "Nothing done yet",
    sub: "Check something off and it lands here."
  }));
}
function ScreenHead({
  title,
  count,
  sub
}) {
  return /*#__PURE__*/React.createElement("div", {
    className: "screen-head"
  }, /*#__PURE__*/React.createElement("div", {
    className: "row gap-10",
    style: {
      alignItems: "baseline"
    }
  }, /*#__PURE__*/React.createElement("h1", {
    className: "display",
    style: {
      fontSize: 34
    }
  }, title), count != null && /*#__PURE__*/React.createElement("span", {
    className: "num",
    style: {
      fontSize: 18,
      color: "var(--muted)"
    }
  }, count)), sub && /*#__PURE__*/React.createElement("div", {
    className: "muted",
    style: {
      fontSize: 14,
      marginTop: 2
    }
  }, sub));
}

/* ---- tweaks: the two required controls ---- */
function DemoTweaks({
  t,
  setTweak
}) {
  return /*#__PURE__*/React.createElement(TweaksPanel, {
    title: "Tweaks"
  }, /*#__PURE__*/React.createElement(TweakSection, {
    label: "Device"
  }, /*#__PURE__*/React.createElement(TweakRadio, {
    label: "Layout",
    value: t.layoutMode || "auto",
    options: [{
      value: "auto",
      label: "Auto"
    }, {
      value: "mobile",
      label: "Mobile"
    }, {
      value: "desktop",
      label: "Desktop"
    }],
    onChange: v => setTweak("layoutMode", v)
  })), /*#__PURE__*/React.createElement(TweakSection, {
    label: "Prototype state"
  }, /*#__PURE__*/React.createElement(TweakButton, {
    label: "Reset app state",
    secondary: true,
    onClick: resetState
  })));
}
ReactDOM.createRoot(document.getElementById("root")).render(/*#__PURE__*/React.createElement(Root, null));
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/demo/app.jsx", error: String((e && e.message) || e) }); }

// ui_kits/demo/data.js
try { (() => {
/* ============================================================
   Protokit demo · mock data layer
   A small, deterministic seed for "Protokit Tasks". Exposed on
   window.DEMO. Dates are computed relative to today at seed time so
   the Today / Upcoming / Overdue filters always have something to show.
   ============================================================ */
(function () {
  function startOfDay(d) {
    const x = new Date(d);
    x.setHours(0, 0, 0, 0);
    return x;
  }
  function iso(d) {
    return startOfDay(d).toISOString().slice(0, 10);
  }
  function addDays(n) {
    const d = new Date();
    d.setDate(d.getDate() + n);
    return iso(d);
  }
  const PROJECTS = [{
    id: "inbox",
    label: "Inbox",
    color: "#6f7783"
  }, {
    id: "work",
    label: "Work",
    color: "#3a63ee"
  }, {
    id: "home",
    label: "Home",
    color: "#1c8a52"
  }, {
    id: "reading",
    label: "Reading",
    color: "#8a5cb0"
  }];

  // seed tasks (offset = days from today; null = no date)
  const SEED = [{
    title: "Email the roaster about the spring blend",
    project: "work",
    offset: 0,
    priority: "high"
  }, {
    title: "Review Q3 deck before standup",
    project: "work",
    offset: 0,
    priority: "high"
  }, {
    title: "Water the fiddle-leaf fig",
    project: "home",
    offset: 0,
    priority: "low"
  }, {
    title: "Reply to Priya about the venue",
    project: "work",
    offset: -1,
    priority: "high"
  }, {
    title: "Pick up dry cleaning",
    project: "home",
    offset: -2,
    priority: "low"
  }, {
    title: "Draft the onboarding checklist",
    project: "work",
    offset: 1,
    priority: "none"
  }, {
    title: "Book dentist appointment",
    project: "home",
    offset: 2,
    priority: "none"
  }, {
    title: "Finish 'The Overstory' (pg 240)",
    project: "reading",
    offset: 3,
    priority: "low"
  }, {
    title: "Plan weekend hike",
    project: "home",
    offset: 4,
    priority: "none"
  }, {
    title: "Renew library books",
    project: "reading",
    offset: 6,
    priority: "none"
  }, {
    title: "Sketch logo options for side project",
    project: "work",
    offset: null,
    priority: "none"
  }, {
    title: "Try the new pour-over recipe",
    project: "home",
    offset: null,
    priority: "low"
  }, {
    title: "Archive last quarter's invoices",
    project: "work",
    offset: -3,
    priority: "none",
    done: true
  }, {
    title: "Send thank-you notes",
    project: "home",
    offset: -4,
    priority: "low",
    done: true
  }, {
    title: "Read 'Pragmatic Programmer' ch. 4",
    project: "reading",
    offset: -2,
    priority: "none",
    done: true
  }];
  function seedTasks() {
    return SEED.map((t, i) => ({
      id: "t" + (i + 1),
      title: t.title,
      done: !!t.done,
      project: t.project,
      due: t.offset == null ? null : addDays(t.offset),
      priority: t.priority || "none",
      notes: "",
      created: Date.now() - (SEED.length - i) * 3600_000
    }));
  }

  // due-date formatting + bucketing
  function fmtDue(due) {
    if (!due) return null;
    const today = iso(new Date());
    const t = startOfDay(due),
      n = startOfDay(today);
    const days = Math.round((t - n) / 86400000);
    if (days < 0) return {
      label: days === -1 ? "Yesterday" : Math.abs(days) + "d overdue",
      tone: "danger"
    };
    if (days === 0) return {
      label: "Today",
      tone: "accent"
    };
    if (days === 1) return {
      label: "Tomorrow",
      tone: "neutral"
    };
    if (days < 7) return {
      label: new Date(due + "T00:00").toLocaleDateString("en-US", {
        weekday: "short"
      }),
      tone: "neutral"
    };
    return {
      label: new Date(due + "T00:00").toLocaleDateString("en-US", {
        month: "short",
        day: "numeric"
      }),
      tone: "neutral"
    };
  }
  function bucket(task) {
    if (task.done) return "done";
    if (!task.due) return "someday";
    const days = Math.round((startOfDay(task.due) - startOfDay(new Date())) / 86400000);
    if (days <= 0) return "today"; // today + overdue land in Today
    return "upcoming";
  }
  window.DEMO = {
    PROJECTS,
    project: id => PROJECTS.find(p => p.id === id) || PROJECTS[0],
    seedTasks,
    fmtDue,
    bucket,
    iso,
    addDays,
    user: {
      name: "Maya Okafor",
      email: "maya@beanandbranch.co"
    },
    quickAdds: ["Reply to email", "Stand-up notes", "Grocery run", "Call the bank"]
  };
})();
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/demo/data.js", error: String((e && e.message) || e) }); }

// ui_kits/demo/pk-lib.jsx
try { (() => {
function _extends() { return _extends = Object.assign ? Object.assign.bind() : function (n) { for (var e = 1; e < arguments.length; e++) { var t = arguments[e]; for (var r in t) ({}).hasOwnProperty.call(t, r) && (n[r] = t[r]); } return n; }, _extends.apply(null, arguments); }
/* ============================================================
   Protokit demo · component lib (AUTO-DERIVED from /components/*.jsx)
   Do NOT hand-edit — regenerate from the canonical sources. The compiled
   _ds_bundle.js is the source of truth for consuming projects; this mirror
   exists so the demo renders standalone (the generated bundle isn't on the
   plain serve origin). Exposed under window.ProtokitDesignSystem_47e616.
   ============================================================ */

/* ---- components/core/Icon.jsx ---- */
/* Protokit icon set — 24×24, stroke 1.7, round caps/joins, currentColor.
   One component, one path map. No emoji, no icon-font dependency. Add an
   entry here and it's instantly available by name everywhere. */
const ICON_PATHS = {
  // nav / app
  home: "M3 10.5 12 3l9 7.5M5 9.5V20h5v-6h4v6h5V9.5",
  inbox: "M3 13h4l1.5 3h7L17 13h4M5 5h14l2 8v5a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1v-5z",
  list: "M8 6h13M8 12h13M8 18h13M3.5 6h.01M3.5 12h.01M3.5 18h.01",
  grid: "M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z",
  calendar: "M4 6h16v15H4zM4 10h16M8 3v4M16 3v4",
  chart: "M4 20V4M4 20h16M8 16v-5M12 16V7M16 16v-8M20 16v-3",
  members: "M16 19v-1.5a3.5 3.5 0 0 0-3.5-3.5h-5A3.5 3.5 0 0 0 4 17.5V19M9.5 11a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7M20 19v-1.5a3.5 3.5 0 0 0-2.6-3.4M15 4.2a3.5 3.5 0 0 1 0 6.6",
  user: "M16 19v-1.5a3.5 3.5 0 0 0-3.5-3.5h-1A3.5 3.5 0 0 0 8 17.5V19M12 11a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7",
  settings: "M12 15.5a3.5 3.5 0 1 0 0-7 3.5 3.5 0 0 0 0 7M19.4 12a7.6 7.6 0 0 0-.1-1.3l2-1.6-2-3.4-2.4 1a7.6 7.6 0 0 0-2.2-1.3l-.4-2.5H10l-.4 2.5a7.6 7.6 0 0 0-2.2 1.3l-2.4-1-2 3.4 2 1.6a7.6 7.6 0 0 0 0 2.6l-2 1.6 2 3.4 2.4-1a7.6 7.6 0 0 0 2.2 1.3l.4 2.5h4l.4-2.5a7.6 7.6 0 0 0 2.2-1.3l2.4 1 2-3.4-2-1.6c.07-.43.1-.86.1-1.3",
  bell: "M18 8.5a6 6 0 1 0-12 0c0 6-2.5 7.5-2.5 7.5h17S18 14.5 18 8.5M10 20a2 2 0 0 0 4 0",
  search: "M11 18a7 7 0 1 0 0-14 7 7 0 0 0 0 14M20 20l-4-4",
  // task / status
  circle: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18",
  check: "M4 12.5 9 17.5 20 6.5",
  checkCircle: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18M8.5 12l2.5 2.5 4.5-5",
  flag: "M5 21V4M5 4h11l-2 4 2 4H5",
  star: "M12 3.5l2.6 5.3 5.9.86-4.25 4.14 1 5.85L12 16.9l-5.25 2.75 1-5.85L3.5 9.66l5.9-.86z",
  clock: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18M12 7v5l3.5 2",
  tag: "M3.5 12.5 11 5h6.5V11.5L10 19a1.5 1.5 0 0 1-2 0l-4.5-4.5a1.5 1.5 0 0 1 0-2M15 8.5h.01",
  spark: "M12 3v4M12 17v4M5 12H3M21 12h-2M6.3 6.3 4.9 4.9M19.1 19.1l-1.4-1.4M6.3 17.7l-1.4 1.4M19.1 4.9l-1.4 1.4M12 9a3 3 0 1 0 0 6 3 3 0 0 0 0-6",
  sparkles: "M12 4l1.6 4.4L18 10l-4.4 1.6L12 16l-1.6-4.4L6 10l4.4-1.6zM18 15l.8 2.2L21 18l-2.2.8L18 21l-.8-2.2L15 18l2.2-.8z",
  bolt: "M13 3 5 13h6l-1 8 8-10h-6z",
  // arrows / chevrons
  plus: "M12 5v14M5 12h14",
  minus: "M5 12h14",
  chevR: "M9 5l7 7-7 7",
  chevL: "M15 5l-7 7 7 7",
  chevD: "M6 9l6 6 6-6",
  chevU: "M6 15l6-6 6 6",
  arrowR: "M5 12h14M13 6l6 6-6 6",
  arrowUp: "M12 19V5M6 11l6-6 6 6",
  back: "M19 12H5M11 18l-6-6 6-6",
  x: "M6 6l12 12M18 6 6 18",
  external: "M14 4h6v6M20 4l-9 9M18 13v6a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h6",
  // actions
  edit: "M4 20h4L19 9a2 2 0 0 0-3-3L5 17zM14 7l3 3",
  trash: "M4 7h16M9 7V5a1 1 0 0 1 1-1h4a1 1 0 0 1 1 1v2M6 7l1 13h10l1-13",
  copy: "M9 9h10a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H9a1 1 0 0 1-1-1V10a1 1 0 0 1 1-1M5 15H4a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h10a1 1 0 0 1 1 1v1",
  filter: "M3 5h18l-7 8v6l-4 2v-8z",
  sort: "M7 4v16M7 20l-3-3M7 4l3 3M17 20V4M17 4l3 3M17 20l-3-3",
  more: "M12 6h.01M12 12h.01M12 18h.01",
  pin: "M12 21s7-5.5 7-11a7 7 0 1 0-14 0c0 5.5 7 11 7 11M12 12.5a2.5 2.5 0 1 0 0-5 2.5 2.5 0 0 0 0 5",
  link: "M9 15l6-6M10.5 6.5 12 5a4 4 0 0 1 6 6l-1.5 1.5M13.5 17.5 12 19a4 4 0 0 1-6-6l1.5-1.5",
  // misc
  info: "M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18M12 11v5M12 8h.01",
  alert: "M12 3 2 20h20zM12 10v4M12 17h.01",
  lock: "M6 11h12v9H6zM8 11V8a4 4 0 0 1 8 0v3",
  mail: "M3 7a2 2 0 0 1 2-2h14a2 2 0 0 1 2 2v10a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2zM3.5 7.5l8.5 6 8.5-6",
  logout: "M9 4H6a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h3M16 17l5-5-5-5M21 12H9",
  heart: "M12 20s-7-4.7-7-10a4 4 0 0 1 7-2.6A4 4 0 0 1 19 10c0 5.3-7 10-7 10",
  dollar: "M12 3v18M16 7.5c0-1.7-1.8-3-4-3s-4 1.3-4 3 1.8 3 4 3 4 1.3 4 3-1.8 3-4 3-4-1.3-4-3",
  folder: "M3 7a2 2 0 0 1 2-2h4l2 2h6a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
  doc: "M6 3h8l4 4v14H6zM14 3v4h4",
  play: "M7 5l12 7-12 7z",
  pause: "M9 5v14M15 5v14",
  // theme
  sun: "M12 17a5 5 0 1 0 0-10 5 5 0 0 0 0 10M12 1.5v2.5M12 20v2.5M4 4l1.8 1.8M18.2 18.2 20 20M1.5 12H4M20 12h2.5M4 20l1.8-1.8M18.2 5.8 20 4",
  moon: "M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8z"
};
function Icon({
  name,
  size = 18,
  stroke = 1.7,
  fill = false,
  className,
  style
}) {
  const d = ICON_PATHS[name];
  if (!d) return null;
  return /*#__PURE__*/React.createElement("svg", {
    width: size,
    height: size,
    viewBox: "0 0 24 24",
    fill: "none",
    className: className,
    style: style,
    "aria-hidden": "true"
  }, /*#__PURE__*/React.createElement("path", {
    d: d,
    stroke: "currentColor",
    strokeWidth: stroke,
    strokeLinecap: "round",
    strokeLinejoin: "round",
    fill: fill ? "currentColor" : "none"
  }));
}

/* ---- components/core/Button.jsx ---- */
/* Button — the primary action primitive.
   Variants map to brand intents; size + icon + block are orthogonal. */
function Button({
  variant = "ghost",
  size,
  icon,
  iconRight,
  block,
  children,
  className = "",
  ...rest
}) {
  const cls = ["pk-btn", `pk-btn--${variant}`];
  if (size === "lg") cls.push("pk-btn--lg");
  if (size === "sm") cls.push("pk-btn--sm");
  if (block) cls.push("pk-btn--block");
  if (className) cls.push(className);
  const iconSize = size === "lg" ? 19 : size === "sm" ? 15 : 17;
  return /*#__PURE__*/React.createElement("button", _extends({
    className: cls.join(" ")
  }, rest), icon && /*#__PURE__*/React.createElement(Icon, {
    name: icon,
    size: iconSize
  }), children, iconRight && /*#__PURE__*/React.createElement(Icon, {
    name: iconRight,
    size: iconSize
  }));
}

/* ---- components/core/IconButton.jsx ---- */
/* IconButton — a square/round button holding a single icon.
   variant: "default" (bordered) | "bare" (no chrome until hover). */
function IconButton({
  icon,
  size = "md",
  variant = "default",
  title,
  children,
  className = "",
  ...rest
}) {
  const cls = ["pk-iconbtn"];
  if (size === "sm") cls.push("pk-iconbtn--sm");
  if (variant === "bare") cls.push("pk-iconbtn--bare");
  if (className) cls.push(className);
  return /*#__PURE__*/React.createElement("button", _extends({
    className: cls.join(" "),
    title: title,
    "aria-label": title
  }, rest), icon ? /*#__PURE__*/React.createElement(Icon, {
    name: icon,
    size: size === "sm" ? 16 : 18
  }) : children);
}

/* ---- components/core/Badge.jsx ---- */
/* Badge — a small status / category pill.
   tone: neutral (default) | accent | pos | warn | danger.
   mono renders it as an uppercase mono label; dot adds a leading dot. */
function Badge({
  tone = "neutral",
  mono,
  dot,
  children,
  className = "",
  style
}) {
  const cls = ["pk-badge"];
  if (tone && tone !== "neutral") cls.push(`pk-badge--${tone}`);
  if (mono) cls.push("pk-badge--mono");
  if (dot) cls.push("pk-badge--dot");
  if (className) cls.push(className);
  return /*#__PURE__*/React.createElement("span", {
    className: cls.join(" "),
    style: style
  }, children);
}

/* ---- components/core/Avatar.jsx ---- */
/* Avatar — initials chip with a deterministic tint derived from the name
   (stable across reloads). Pass src to show a real image instead. */
function tintFor(name) {
  const TINTS = ["#c2410c", "#1f6f54", "#9a5b2b", "#3a6ea5", "#8a5cb0", "#b03a4e", "#5a7d3a", "#2f7d5b", "#a8761c", "#7a6cc4"];
  let h = 0,
    s = String(name || "");
  for (let i = 0; i < s.length; i++) h = h * 31 + s.charCodeAt(i) >>> 0;
  return TINTS[h % TINTS.length];
}
function Avatar({
  name,
  initials,
  tint,
  src,
  size = 38,
  className = "",
  style
}) {
  const ini = initials || String(name || "?").trim().split(/\s+/).slice(0, 2).map(w => w[0]).join("").toUpperCase();
  const bg = tint || tintFor(name || ini);
  return /*#__PURE__*/React.createElement("span", {
    className: "pk-avatar " + className,
    style: {
      width: size,
      height: size,
      background: src ? "var(--surface-3)" : bg,
      fontSize: size * 0.4,
      ...style
    }
  }, src ? /*#__PURE__*/React.createElement("img", {
    src: src,
    alt: name || "",
    style: {
      width: "100%",
      height: "100%",
      objectFit: "cover"
    }
  }) : ini);
}

/* ---- components/core/Card.jsx ---- */
/* Card — the base surface. pad adds standard padding; tone="2" uses the
   subtle surface; flat drops the shadow. Compose freely. */
function Card({
  pad,
  tone,
  flat,
  as = "div",
  children,
  className = "",
  style,
  ...rest
}) {
  const cls = ["pk-card"];
  if (pad) cls.push("pk-card--pad");
  if (tone === "2") cls.push("pk-card--2");
  if (flat) cls.push("pk-card--flat");
  if (className) cls.push(className);
  const El = as;
  return /*#__PURE__*/React.createElement(El, _extends({
    className: cls.join(" "),
    style: style
  }, rest), children);
}

/* ---- components/forms/Input.jsx ---- */
/* Input — a labelled text field. Renders a <label>+<input> with optional
   hint. Pass multiline for a textarea, or prefix for a leading affix. */
function Input({
  label,
  hint,
  prefix,
  multiline,
  className = "",
  style,
  id,
  ...rest
}) {
  const inputId = id || (label ? "in-" + label.replace(/\W+/g, "-").toLowerCase() : undefined);
  const control = multiline ? /*#__PURE__*/React.createElement("textarea", _extends({
    id: inputId,
    className: "pk-textarea"
  }, rest)) : prefix ? /*#__PURE__*/React.createElement("div", {
    className: "pk-input",
    style: {
      display: "flex",
      alignItems: "center",
      padding: "0 0 0 14px",
      gap: 4
    }
  }, /*#__PURE__*/React.createElement("span", {
    className: "mono",
    style: {
      color: "var(--muted)"
    }
  }, prefix), /*#__PURE__*/React.createElement("input", _extends({
    id: inputId
  }, rest, {
    style: {
      flex: 1,
      minWidth: 0,
      height: 44,
      border: 0,
      outline: "none",
      background: "transparent",
      color: "var(--ink)",
      font: "inherit",
      padding: "0 14px 0 4px"
    }
  }))) : /*#__PURE__*/React.createElement("input", _extends({
    id: inputId,
    className: "pk-input"
  }, rest));
  if (!label && !hint) return prefix ? control : React.cloneElement(control, {
    className: (control.props.className || "") + " " + className,
    style
  });
  return /*#__PURE__*/React.createElement("div", {
    className: "pk-field " + className,
    style: style
  }, label && /*#__PURE__*/React.createElement("label", {
    htmlFor: inputId
  }, label), control, hint && /*#__PURE__*/React.createElement("span", {
    className: "pk-hint"
  }, hint));
}

/* ---- components/forms/Checkbox.jsx ---- */
/* Checkbox — a labelled checkbox. Controlled via checked + onChange. */
function Checkbox({
  checked,
  onChange,
  label,
  className = "",
  style,
  ...rest
}) {
  const box = /*#__PURE__*/React.createElement("input", _extends({
    type: "checkbox",
    className: "pk-check",
    checked: !!checked,
    onChange: e => onChange && onChange(e.target.checked)
  }, rest));
  if (!label) return box;
  return /*#__PURE__*/React.createElement("label", {
    className: "row gap-10 " + className,
    style: {
      cursor: "pointer",
      fontSize: "var(--text-base)",
      color: "var(--ink)",
      ...style
    }
  }, box, /*#__PURE__*/React.createElement("span", null, label));
}

/* ---- components/forms/Switch.jsx ---- */
/* Switch — an on/off toggle. Controlled via on + onChange(next:boolean). */
function Switch({
  on,
  onChange,
  label,
  className = "",
  style
}) {
  const toggle = /*#__PURE__*/React.createElement("button", {
    type: "button",
    className: "pk-switch",
    "data-on": on ? "1" : "0",
    role: "switch",
    "aria-checked": !!on,
    onClick: () => onChange && onChange(!on)
  }, /*#__PURE__*/React.createElement("i", null));
  if (!label) return toggle;
  return /*#__PURE__*/React.createElement("label", {
    className: "row between gap-12 " + className,
    style: {
      cursor: "pointer",
      ...style
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      fontSize: "var(--text-base)",
      color: "var(--ink)"
    }
  }, label), toggle);
}

/* ---- components/forms/Segmented.jsx ---- */
/* Segmented — a compact pill segmented control for 2–4 short options.
   options: string[] or { value, label }[]. Controlled via value+onChange. */
function Segmented({
  value,
  onChange,
  options,
  className = "",
  style
}) {
  return /*#__PURE__*/React.createElement("div", {
    className: "pk-seg " + className,
    style: style,
    role: "radiogroup"
  }, options.map(o => {
    const v = typeof o === "object" ? o.value : o;
    const l = typeof o === "object" ? o.label : o;
    return /*#__PURE__*/React.createElement("button", {
      key: v,
      type: "button",
      role: "radio",
      "aria-checked": v === value,
      "data-on": v === value ? "1" : "0",
      onClick: () => onChange && onChange(v)
    }, l);
  }));
}

/* ---- components/forms/Tabs.jsx ---- */
/* Tabs — an underline tab bar. options: string[] or { value, label, badge }[].
   Controlled via value + onChange. */
function Tabs({
  value,
  onChange,
  options,
  className = "",
  style
}) {
  return /*#__PURE__*/React.createElement("div", {
    className: "pk-tabs " + className,
    style: style,
    role: "tablist"
  }, options.map(o => {
    const v = typeof o === "object" ? o.value : o;
    const l = typeof o === "object" ? o.label : o;
    const badge = typeof o === "object" ? o.badge : null;
    return /*#__PURE__*/React.createElement("button", {
      key: v,
      type: "button",
      role: "tab",
      "aria-selected": v === value,
      className: "pk-tab",
      "data-on": v === value ? "1" : "0",
      onClick: () => onChange && onChange(v)
    }, l, badge != null && /*#__PURE__*/React.createElement("span", {
      className: "num",
      style: {
        marginLeft: 7,
        fontSize: "var(--text-xs)",
        color: "var(--muted)"
      }
    }, badge));
  }));
}

/* ---- components/feedback/Dialog.jsx ---- */
/* Dialog — a modal overlay. align="center" (default) renders a centered
   card; align="sheet" renders a bottom sheet (great on mobile). Esc and
   scrim-click close. Compose header/body/footer inside as children. */
function Dialog({
  open = true,
  onClose,
  align = "center",
  maxW = 460,
  children
}) {
  React.useEffect(() => {
    if (!open) return undefined;
    const onKey = e => {
      if (e.key === "Escape") onClose && onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open, onClose]);
  if (!open) return null;
  const sheet = align === "sheet";
  return /*#__PURE__*/React.createElement("div", {
    onClick: onClose,
    style: {
      position: "fixed",
      inset: 0,
      zIndex: 500,
      background: "rgba(8,10,14,.46)",
      backdropFilter: "blur(3px)",
      WebkitBackdropFilter: "blur(3px)",
      display: "flex",
      alignItems: sheet ? "flex-end" : "center",
      justifyContent: "center",
      padding: sheet ? 0 : 20,
      animation: "scrim-in .2s ease"
    }
  }, /*#__PURE__*/React.createElement("div", {
    onClick: e => e.stopPropagation(),
    className: "pk-card scroll",
    style: {
      width: "100%",
      maxWidth: sheet ? 520 : maxW,
      maxHeight: "92vh",
      overflowY: "auto",
      borderRadius: sheet ? "var(--r-xl) var(--r-xl) 0 0" : "var(--r-xl)",
      boxShadow: "var(--sh-lg)",
      animation: sheet ? "sheet-up var(--dur-4) var(--ease)" : "pop-in var(--dur-3) var(--ease)"
    }
  }, children));
}

/* ---- components/feedback/Tooltip.jsx ---- */
/* Tooltip — wraps a trigger; shows a label on hover/focus above it. */
function Tooltip({
  label,
  children,
  className = ""
}) {
  const [show, setShow] = React.useState(false);
  return /*#__PURE__*/React.createElement("span", {
    className: "pk-tip-wrap " + className,
    onMouseEnter: () => setShow(true),
    onMouseLeave: () => setShow(false),
    onFocus: () => setShow(true),
    onBlur: () => setShow(false)
  }, children, show && /*#__PURE__*/React.createElement("span", {
    className: "pk-tip",
    role: "tooltip"
  }, label));
}

/* ---- components/shell/ThemeToggle.jsx ---- */
/* ThemeToggle — a sun/moon segmented switch that lives in the app chrome,
   always visible. Controlled: theme ("light"|"dark") + onChange. */
function ThemeToggle({
  theme = "dark",
  onChange,
  className = ""
}) {
  const dark = theme === "dark";
  return /*#__PURE__*/React.createElement("div", {
    className: "pk-theme-toggle " + className,
    role: "group",
    "aria-label": "Color theme"
  }, /*#__PURE__*/React.createElement("button", {
    type: "button",
    "data-on": !dark ? "1" : "0",
    "aria-pressed": !dark,
    title: "Light",
    "aria-label": "Light theme",
    onClick: () => onChange && onChange("light")
  }, /*#__PURE__*/React.createElement(Icon, {
    name: "sun",
    size: 15
  })), /*#__PURE__*/React.createElement("button", {
    type: "button",
    "data-on": dark ? "1" : "0",
    "aria-pressed": dark,
    title: "Dark",
    "aria-label": "Dark theme",
    onClick: () => onChange && onChange("dark")
  }, /*#__PURE__*/React.createElement(Icon, {
    name: "moon",
    size: 15
  })));
}

/* ---- components/shell/AppShell.jsx ---- */
/* AppShell — the responsive product chrome. On desktop it's a fixed left
   sidebar (brand + nav + theme toggle in the footer); on mobile the same
   nav reflows into a top bar + bottom tab bar. Driven by [data-mode] on
   <html> (set by Protokit.applyLayout). The phone-card framing for
   mobile-on-desktop is handled by shell.css.

   Props:
     brand      node rendered in the sidebar head / top bar (logo, name)
     nav        [{ id, label, icon, badge }]
     active     id of the current nav item
     onNavigate (id) => void
     theme, onSetTheme   wired to ThemeToggle (always visible)
     headerRight  optional node for the top-bar right slot (mobile)
     sidebarFoot  optional node above the theme toggle (e.g. account row)
     children   the screen content (placed in a scroll region) */
function AppShell({
  brand,
  nav = [],
  active,
  onNavigate,
  theme = "dark",
  onSetTheme,
  headerRight,
  sidebarFoot,
  children
}) {
  const go = id => onNavigate && onNavigate(id);
  const NavItem = ({
    item
  }) => /*#__PURE__*/React.createElement("button", {
    type: "button",
    className: "pk-nav-item",
    "data-on": item.id === active ? "1" : "0",
    onClick: () => go(item.id)
  }, item.icon && /*#__PURE__*/React.createElement(Icon, {
    name: item.icon,
    size: 19
  }), /*#__PURE__*/React.createElement("span", {
    className: "grow"
  }, item.label), item.badge != null && /*#__PURE__*/React.createElement("span", {
    className: "pk-nav-badge"
  }, item.badge));
  // bottom bar shows up to 5 destinations
  const bottomNav = nav.slice(0, 5);
  return /*#__PURE__*/React.createElement("div", {
    className: "pk-app"
  }, /*#__PURE__*/React.createElement("aside", {
    className: "pk-sidebar"
  }, /*#__PURE__*/React.createElement("div", {
    className: "pk-sidebar-head"
  }, brand), /*#__PURE__*/React.createElement("nav", {
    className: "pk-nav"
  }, nav.map(item => /*#__PURE__*/React.createElement(NavItem, {
    key: item.id,
    item: item
  }))), /*#__PURE__*/React.createElement("div", {
    className: "pk-sidebar-foot"
  }, sidebarFoot || /*#__PURE__*/React.createElement("span", null), /*#__PURE__*/React.createElement(ThemeToggle, {
    theme: theme,
    onChange: onSetTheme
  }))), /*#__PURE__*/React.createElement("div", {
    className: "pk-main"
  }, /*#__PURE__*/React.createElement("header", {
    className: "pk-topbar"
  }, brand, /*#__PURE__*/React.createElement("div", {
    className: "row gap-8"
  }, headerRight, /*#__PURE__*/React.createElement(ThemeToggle, {
    theme: theme,
    onChange: onSetTheme
  }))), /*#__PURE__*/React.createElement("main", {
    className: "pk-screen scroll"
  }, /*#__PURE__*/React.createElement("div", {
    className: "pk-screen-inner"
  }, children)), /*#__PURE__*/React.createElement("nav", {
    className: "pk-bottom"
  }, bottomNav.map(item => /*#__PURE__*/React.createElement("button", {
    key: item.id,
    type: "button",
    "data-on": item.id === active ? "1" : "0",
    onClick: () => go(item.id)
  }, item.icon && /*#__PURE__*/React.createElement(Icon, {
    name: item.icon,
    size: 21
  }), /*#__PURE__*/React.createElement("span", null, item.label))))));
}
window.ProtokitDesignSystem_47e616 = {
  Icon,
  ICON_PATHS,
  Button,
  IconButton,
  Badge,
  Avatar,
  Card,
  Input,
  Checkbox,
  Switch,
  Segmented,
  Tabs,
  Dialog,
  Tooltip,
  ThemeToggle,
  AppShell
};
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/demo/pk-lib.jsx", error: String((e && e.message) || e) }); }

// ui_kits/demo/screens.jsx
try { (() => {
/* ============================================================
   Protokit demo · screens & task components
   Built entirely from DS primitives (window.ProtokitDesignSystem_*) +
   the Protokit runtime. Shared to window for app.jsx.
   ============================================================ */
const PK = window.ProtokitDesignSystem_47e616;
const {
  Button,
  IconButton,
  Icon,
  Badge,
  Avatar,
  Card,
  Input,
  Checkbox,
  Switch,
  Segmented,
  Tabs,
  Dialog
} = PK;

/* ---------- small bits ---------- */
function ProjectDot({
  id,
  size = 8
}) {
  const p = window.DEMO.project(id);
  return /*#__PURE__*/React.createElement("span", {
    style: {
      width: size,
      height: size,
      borderRadius: 999,
      background: p.color,
      flex: "none",
      display: "inline-block"
    }
  });
}
function Stat({
  label,
  value,
  tone
}) {
  return /*#__PURE__*/React.createElement(Card, {
    pad: true,
    style: {
      padding: "16px 18px"
    }
  }, /*#__PURE__*/React.createElement("div", {
    className: "num",
    style: {
      fontSize: 28,
      fontWeight: 600,
      color: tone || "var(--ink)"
    }
  }, value), /*#__PURE__*/React.createElement("div", {
    className: "muted",
    style: {
      fontSize: 13,
      marginTop: 2
    }
  }, label));
}
function EmptyState({
  icon = "checkCircle",
  title,
  sub
}) {
  return /*#__PURE__*/React.createElement("div", {
    className: "col center",
    style: {
      padding: "54px 20px",
      textAlign: "center",
      gap: 10
    }
  }, /*#__PURE__*/React.createElement("span", {
    style: {
      color: "var(--faint)"
    }
  }, /*#__PURE__*/React.createElement(Icon, {
    name: icon,
    size: 40,
    stroke: 1.4
  })), /*#__PURE__*/React.createElement("div", {
    className: "subtitle",
    style: {
      color: "var(--ink-2)"
    }
  }, title), sub && /*#__PURE__*/React.createElement("div", {
    className: "muted",
    style: {
      fontSize: 14,
      maxWidth: 280
    }
  }, sub));
}

/* ---------- task row ---------- */
function TaskRow({
  task,
  onToggle,
  onOpen
}) {
  const due = window.DEMO.fmtDue(task.due);
  const proj = window.DEMO.project(task.project);
  return /*#__PURE__*/React.createElement("div", {
    className: "task-row",
    onClick: () => onOpen(task)
  }, /*#__PURE__*/React.createElement("span", {
    onClick: e => {
      e.stopPropagation();
      onToggle(task.id);
    },
    style: {
      display: "inline-flex"
    }
  }, /*#__PURE__*/React.createElement(Checkbox, {
    checked: task.done,
    onChange: () => onToggle(task.id)
  })), /*#__PURE__*/React.createElement("span", {
    className: "task-title",
    "data-done": task.done ? "1" : "0"
  }, task.title), /*#__PURE__*/React.createElement("span", {
    className: "task-meta"
  }, task.priority === "high" && /*#__PURE__*/React.createElement("span", {
    style: {
      color: "var(--danger)",
      display: "inline-flex"
    },
    title: "High priority"
  }, /*#__PURE__*/React.createElement(Icon, {
    name: "flag",
    size: 15,
    fill: true
  })), task.priority === "low" && /*#__PURE__*/React.createElement("span", {
    style: {
      color: "var(--faint)",
      display: "inline-flex"
    },
    title: "Low priority"
  }, /*#__PURE__*/React.createElement(Icon, {
    name: "flag",
    size: 15
  })), due && /*#__PURE__*/React.createElement(Badge, {
    tone: due.tone
  }, due.label), /*#__PURE__*/React.createElement("span", {
    className: "task-proj"
  }, /*#__PURE__*/React.createElement(ProjectDot, {
    id: proj.id
  }), " ", proj.label)));
}

/* ---------- inline composer ---------- */
function Composer({
  onAdd,
  project = "inbox"
}) {
  const [val, setVal] = React.useState("");
  const submit = () => {
    const v = val.trim();
    if (!v) return;
    onAdd({
      title: v,
      project
    });
    setVal("");
  };
  return /*#__PURE__*/React.createElement("div", {
    className: "composer"
  }, /*#__PURE__*/React.createElement("span", {
    className: "composer-plus"
  }, /*#__PURE__*/React.createElement(Icon, {
    name: "plus",
    size: 18
  })), /*#__PURE__*/React.createElement("input", {
    className: "composer-input",
    placeholder: "Add a task\u2026  (press Enter)",
    value: val,
    onChange: e => setVal(e.target.value),
    onKeyDown: e => {
      if (e.key === "Enter") submit();
    }
  }), val.trim() && /*#__PURE__*/React.createElement(Button, {
    variant: "primary",
    size: "sm",
    onClick: submit
  }, "Add"));
}

/* ---------- task list with optional section grouping ---------- */
function TaskList({
  tasks,
  onToggle,
  onOpen,
  group
}) {
  if (!tasks.length) return null;
  if (!group) {
    return /*#__PURE__*/React.createElement("div", {
      className: "task-list"
    }, tasks.map(t => /*#__PURE__*/React.createElement(TaskRow, {
      key: t.id,
      task: t,
      onToggle: onToggle,
      onOpen: onOpen
    })));
  }
  // group by overdue / today for the Today screen
  const today = window.DEMO.iso(new Date());
  const overdue = tasks.filter(t => t.due && t.due < today);
  const rest = tasks.filter(t => !t.due || t.due >= today);
  return /*#__PURE__*/React.createElement("div", {
    className: "col gap-16"
  }, overdue.length > 0 && /*#__PURE__*/React.createElement("div", null, /*#__PURE__*/React.createElement("div", {
    className: "list-sect",
    style: {
      color: "var(--danger)"
    }
  }, "Overdue \xB7 ", overdue.length), /*#__PURE__*/React.createElement("div", {
    className: "task-list"
  }, overdue.map(t => /*#__PURE__*/React.createElement(TaskRow, {
    key: t.id,
    task: t,
    onToggle: onToggle,
    onOpen: onOpen
  })))), rest.length > 0 && /*#__PURE__*/React.createElement("div", null, overdue.length > 0 && /*#__PURE__*/React.createElement("div", {
    className: "list-sect"
  }, "Today"), /*#__PURE__*/React.createElement("div", {
    className: "task-list"
  }, rest.map(t => /*#__PURE__*/React.createElement(TaskRow, {
    key: t.id,
    task: t,
    onToggle: onToggle,
    onOpen: onOpen
  })))));
}

/* ---------- task detail sheet ---------- */
function TaskDetail({
  task,
  onClose,
  onChange,
  onToggle,
  onDelete
}) {
  if (!task) return null;
  const set = patch => onChange(task.id, patch);
  return /*#__PURE__*/React.createElement(Dialog, {
    align: "sheet",
    onClose: onClose
  }, /*#__PURE__*/React.createElement("div", {
    className: "detail"
  }, /*#__PURE__*/React.createElement("div", {
    className: "row between",
    style: {
      marginBottom: 4
    }
  }, /*#__PURE__*/React.createElement(Button, {
    variant: task.done ? "pos" : "soft",
    size: "sm",
    icon: task.done ? "checkCircle" : "circle",
    onClick: () => onToggle(task.id)
  }, task.done ? "Completed" : "Mark done"), /*#__PURE__*/React.createElement(IconButton, {
    icon: "x",
    title: "Close",
    variant: "bare",
    onClick: onClose
  })), /*#__PURE__*/React.createElement("textarea", {
    className: "detail-title",
    rows: 2,
    value: task.title,
    onChange: e => set({
      title: e.target.value
    })
  }), /*#__PURE__*/React.createElement("div", {
    className: "detail-fields"
  }, /*#__PURE__*/React.createElement("label", {
    className: "detail-field"
  }, /*#__PURE__*/React.createElement("span", {
    className: "detail-lbl"
  }, "Project"), /*#__PURE__*/React.createElement(Segmented, {
    value: task.project,
    onChange: v => set({
      project: v
    }),
    options: window.DEMO.PROJECTS.map(p => ({
      value: p.id,
      label: p.label
    }))
  })), /*#__PURE__*/React.createElement("label", {
    className: "detail-field"
  }, /*#__PURE__*/React.createElement("span", {
    className: "detail-lbl"
  }, "Priority"), /*#__PURE__*/React.createElement(Segmented, {
    value: task.priority,
    onChange: v => set({
      priority: v
    }),
    options: [{
      value: "none",
      label: "None"
    }, {
      value: "low",
      label: "Low"
    }, {
      value: "high",
      label: "High"
    }]
  })), /*#__PURE__*/React.createElement("label", {
    className: "detail-field"
  }, /*#__PURE__*/React.createElement("span", {
    className: "detail-lbl"
  }, "Due date"), /*#__PURE__*/React.createElement("input", {
    type: "date",
    className: "pk-input",
    style: {
      maxWidth: 220
    },
    value: task.due || "",
    onChange: e => set({
      due: e.target.value || null
    })
  })), /*#__PURE__*/React.createElement("label", {
    className: "detail-field"
  }, /*#__PURE__*/React.createElement("span", {
    className: "detail-lbl"
  }, "Notes"), /*#__PURE__*/React.createElement("textarea", {
    className: "pk-textarea",
    rows: 3,
    placeholder: "Add detail\u2026",
    value: task.notes,
    onChange: e => set({
      notes: e.target.value
    })
  }))), /*#__PURE__*/React.createElement("div", {
    className: "row between",
    style: {
      marginTop: 4
    }
  }, /*#__PURE__*/React.createElement(Button, {
    variant: "quiet",
    icon: "trash",
    onClick: () => onDelete(task.id),
    style: {
      color: "var(--danger)"
    }
  }, "Delete task"), /*#__PURE__*/React.createElement(Button, {
    variant: "primary",
    onClick: onClose
  }, "Done"))));
}
Object.assign(window, {
  ProjectDot,
  Stat,
  EmptyState,
  TaskRow,
  Composer,
  TaskList,
  TaskDetail
});
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/demo/screens.jsx", error: String((e && e.message) || e) }); }

// ui_kits/demo/signin.jsx
try { (() => {
/* ============================================================
   Protokit demo · sign-in screen
   Exercises the persisted phase machine (signin → app) and shows the
   always-dark hero panel pattern. Theme toggle stays visible.
   ============================================================ */
const PKauth = window.ProtokitDesignSystem_47e616;
function SignIn({
  onAuthed,
  theme,
  onSetTheme
}) {
  const {
    Button,
    Input,
    Icon,
    ThemeToggle
  } = PKauth;
  const [email, setEmail] = React.useState(window.DEMO.user.email);
  const [busy, setBusy] = React.useState(false);
  const go = () => {
    if (!email.trim()) return;
    setBusy(true);
    setTimeout(() => onAuthed(email), 520); // fake the round-trip
  };
  return /*#__PURE__*/React.createElement("div", {
    className: "auth"
  }, /*#__PURE__*/React.createElement("aside", {
    className: "auth-brand"
  }, /*#__PURE__*/React.createElement("div", {
    className: "auth-brand-top"
  }, /*#__PURE__*/React.createElement("span", {
    className: "brand-mark on-dark"
  }, /*#__PURE__*/React.createElement("span", {
    className: "brand-glyph"
  }, "P"), " Protokit")), /*#__PURE__*/React.createElement("div", {
    className: "auth-brand-body"
  }, /*#__PURE__*/React.createElement("h1", {
    className: "auth-brand-h"
  }, "Your day, in one tidy list."), /*#__PURE__*/React.createElement("p", {
    className: "auth-brand-sub"
  }, "Protokit Tasks keeps work, home, and reading in a single calm place \u2014 with everything that matters today right up top."), /*#__PURE__*/React.createElement("div", {
    className: "auth-points"
  }, /*#__PURE__*/React.createElement("span", {
    className: "auth-chip"
  }, /*#__PURE__*/React.createElement(Icon, {
    name: "checkCircle",
    size: 17
  }), " Today, upcoming & overdue at a glance"), /*#__PURE__*/React.createElement("span", {
    className: "auth-chip"
  }, /*#__PURE__*/React.createElement(Icon, {
    name: "bolt",
    size: 17
  }), " Add a task in one keystroke"), /*#__PURE__*/React.createElement("span", {
    className: "auth-chip"
  }, /*#__PURE__*/React.createElement(Icon, {
    name: "moon",
    size: 17
  }), " Light or dark, your call"))), /*#__PURE__*/React.createElement("p", {
    className: "auth-brand-foot"
  }, "A Protokit demo \xB7 persistent state")), /*#__PURE__*/React.createElement("div", {
    className: "auth-main"
  }, /*#__PURE__*/React.createElement("div", {
    className: "auth-top"
  }, /*#__PURE__*/React.createElement("span", {
    className: "brand-mark"
  }, /*#__PURE__*/React.createElement("span", {
    className: "brand-glyph"
  }, "P"), " Protokit"), /*#__PURE__*/React.createElement(ThemeToggle, {
    theme: theme,
    onChange: onSetTheme
  })), /*#__PURE__*/React.createElement("div", {
    className: "auth-form-wrap"
  }, /*#__PURE__*/React.createElement("div", {
    className: "auth-form"
  }, /*#__PURE__*/React.createElement("h2", {
    className: "title",
    style: {
      fontSize: 26,
      marginBottom: 6
    }
  }, "Welcome back"), /*#__PURE__*/React.createElement("p", {
    className: "muted",
    style: {
      margin: "0 0 22px",
      fontSize: 15
    }
  }, "Sign in to pick up where you left off."), /*#__PURE__*/React.createElement("div", {
    className: "col gap-14",
    style: {
      marginBottom: 18
    }
  }, /*#__PURE__*/React.createElement(Input, {
    label: "Email",
    type: "email",
    value: email,
    onChange: e => setEmail(e.target.value),
    placeholder: "you@example.com",
    onKeyDown: e => {
      if (e.key === "Enter") go();
    }
  }), /*#__PURE__*/React.createElement(Input, {
    label: "Password",
    type: "password",
    defaultValue: "\xB7\xB7\xB7\xB7\xB7\xB7\xB7\xB7",
    onKeyDown: e => {
      if (e.key === "Enter") go();
    }
  })), /*#__PURE__*/React.createElement(Button, {
    variant: "primary",
    block: true,
    size: "lg",
    onClick: go,
    disabled: busy
  }, busy ? /*#__PURE__*/React.createElement("span", {
    className: "pk-spinner"
  }) : "Continue"), /*#__PURE__*/React.createElement("p", {
    className: "auth-demo"
  }, /*#__PURE__*/React.createElement(Icon, {
    name: "info",
    size: 14
  }), " Demo \u2014 any details work. State persists across reloads.")))));
}
Object.assign(window, {
  SignIn
});
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/demo/signin.jsx", error: String((e && e.message) || e) }); }

// ui_kits/demo/tweaks-panel.jsx
try { (() => {
// @ds-adherence-ignore -- omelette starter scaffold (raw elements/hex/px by design)

/* BEGIN USAGE */
// tweaks-panel.jsx
// Reusable Tweaks shell + form-control helpers.
// Exports (to window): useTweaks, TweaksPanel, TweakSection, TweakRow, TweakSlider,
//   TweakToggle, TweakRadio, TweakSelect, TweakText, TweakNumber, TweakColor, TweakButton.
//
// Owns the host protocol (listens for __activate_edit_mode / __deactivate_edit_mode,
// posts __edit_mode_available / __edit_mode_set_keys / __edit_mode_dismissed) so
// individual prototypes don't re-roll it. Ships a consistent set of controls so you
// don't hand-draw <input type="range">, segmented radios, steppers, etc.
//
// Usage (in an HTML file that loads React + Babel):
//
//   const TWEAK_DEFAULTS = /*EDITMODE-BEGIN*/{
//     "primaryColor": "#D97757",
//     "palette": ["#D97757", "#29261b", "#f6f4ef"],
//     "fontSize": 16,
//     "density": "regular",
//     "dark": false
//   }/*EDITMODE-END*/;
//
//   function App() {
//     const [t, setTweak] = useTweaks(TWEAK_DEFAULTS);
//     return (
//       <div style={{ fontSize: t.fontSize, color: t.primaryColor }}>
//         Hello
//         <TweaksPanel>
//           <TweakSection label="Typography" />
//           <TweakSlider label="Font size" value={t.fontSize} min={10} max={32} unit="px"
//                        onChange={(v) => setTweak('fontSize', v)} />
//           <TweakRadio  label="Density" value={t.density}
//                        options={['compact', 'regular', 'comfy']}
//                        onChange={(v) => setTweak('density', v)} />
//           <TweakSection label="Theme" />
//           <TweakColor  label="Primary" value={t.primaryColor}
//                        options={['#D97757', '#2A6FDB', '#1F8A5B', '#7A5AE0']}
//                        onChange={(v) => setTweak('primaryColor', v)} />
//           <TweakColor  label="Palette" value={t.palette}
//                        options={[['#D97757', '#29261b', '#f6f4ef'],
//                                  ['#475569', '#0f172a', '#f1f5f9']]}
//                        onChange={(v) => setTweak('palette', v)} />
//           <TweakToggle label="Dark mode" value={t.dark}
//                        onChange={(v) => setTweak('dark', v)} />
//         </TweaksPanel>
//       </div>
//     );
//   }
//
// TweakRadio is the segmented control for 2–3 short options (auto-falls-back to
// TweakSelect past ~16/~10 chars per label); reach for TweakSelect directly when
// options are many or long. For color tweaks always curate 3-4 options rather than
// a free picker; an option can also be a whole 2–5 color palette (the stored value
// is the array). The Tweak* controls are a floor, not a ceiling — build custom
// controls inside the panel if a tweak calls for UI they don't cover.
/* END USAGE */
// ─────────────────────────────────────────────────────────────────────────────

const __TWEAKS_STYLE = `
  .twk-panel{position:fixed;right:16px;bottom:16px;z-index:2147483646;width:280px;
    max-height:calc(100vh - 32px);display:flex;flex-direction:column;
    transform:scale(var(--dc-inv-zoom,1));transform-origin:bottom right;
    background:rgba(250,249,247,.78);color:#29261b;
    -webkit-backdrop-filter:blur(24px) saturate(160%);backdrop-filter:blur(24px) saturate(160%);
    border:.5px solid rgba(255,255,255,.6);border-radius:14px;
    box-shadow:0 1px 0 rgba(255,255,255,.5) inset,0 12px 40px rgba(0,0,0,.18);
    font:11.5px/1.4 ui-sans-serif,system-ui,-apple-system,sans-serif;overflow:hidden}
  .twk-hd{display:flex;align-items:center;justify-content:space-between;
    padding:10px 8px 10px 14px;cursor:move;user-select:none}
  .twk-hd b{font-size:12px;font-weight:600;letter-spacing:.01em}
  .twk-x{appearance:none;border:0;background:transparent;color:rgba(41,38,27,.55);
    width:22px;height:22px;border-radius:6px;cursor:default;font-size:13px;line-height:1}
  .twk-x:hover{background:rgba(0,0,0,.06);color:#29261b}
  .twk-body{padding:2px 14px 14px;display:flex;flex-direction:column;gap:10px;
    overflow-y:auto;overflow-x:hidden;min-height:0;
    scrollbar-width:thin;scrollbar-color:rgba(0,0,0,.15) transparent}
  .twk-body::-webkit-scrollbar{width:8px}
  .twk-body::-webkit-scrollbar-track{background:transparent;margin:2px}
  .twk-body::-webkit-scrollbar-thumb{background:rgba(0,0,0,.15);border-radius:4px;
    border:2px solid transparent;background-clip:content-box}
  .twk-body::-webkit-scrollbar-thumb:hover{background:rgba(0,0,0,.25);
    border:2px solid transparent;background-clip:content-box}
  .twk-row{display:flex;flex-direction:column;gap:5px}
  .twk-row-h{flex-direction:row;align-items:center;justify-content:space-between;gap:10px}
  .twk-lbl{display:flex;justify-content:space-between;align-items:baseline;
    color:rgba(41,38,27,.72)}
  .twk-lbl>span:first-child{font-weight:500}
  .twk-val{color:rgba(41,38,27,.5);font-variant-numeric:tabular-nums}

  .twk-sect{font-size:10px;font-weight:600;letter-spacing:.06em;text-transform:uppercase;
    color:rgba(41,38,27,.45);padding:10px 0 0}
  .twk-sect:first-child{padding-top:0}

  .twk-field{appearance:none;box-sizing:border-box;width:100%;min-width:0;height:26px;padding:0 8px;
    border:.5px solid rgba(0,0,0,.1);border-radius:7px;
    background:rgba(255,255,255,.6);color:inherit;font:inherit;outline:none}
  .twk-field:focus{border-color:rgba(0,0,0,.25);background:rgba(255,255,255,.85)}
  select.twk-field{padding-right:22px;
    background-image:url("data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' width='10' height='6' viewBox='0 0 10 6'><path fill='rgba(0,0,0,.5)' d='M0 0h10L5 6z'/></svg>");
    background-repeat:no-repeat;background-position:right 8px center}

  .twk-slider{appearance:none;-webkit-appearance:none;width:100%;height:4px;margin:6px 0;
    border-radius:999px;background:rgba(0,0,0,.12);outline:none}
  .twk-slider::-webkit-slider-thumb{-webkit-appearance:none;appearance:none;
    width:14px;height:14px;border-radius:50%;background:#fff;
    border:.5px solid rgba(0,0,0,.12);box-shadow:0 1px 3px rgba(0,0,0,.2);cursor:default}
  .twk-slider::-moz-range-thumb{width:14px;height:14px;border-radius:50%;
    background:#fff;border:.5px solid rgba(0,0,0,.12);box-shadow:0 1px 3px rgba(0,0,0,.2);cursor:default}

  .twk-seg{position:relative;display:flex;padding:2px;border-radius:8px;
    background:rgba(0,0,0,.06);user-select:none}
  .twk-seg-thumb{position:absolute;top:2px;bottom:2px;border-radius:6px;
    background:rgba(255,255,255,.9);box-shadow:0 1px 2px rgba(0,0,0,.12);
    transition:left .15s cubic-bezier(.3,.7,.4,1),width .15s}
  .twk-seg.dragging .twk-seg-thumb{transition:none}
  .twk-seg button{appearance:none;position:relative;z-index:1;flex:1;border:0;
    background:transparent;color:inherit;font:inherit;font-weight:500;min-height:22px;
    border-radius:6px;cursor:default;padding:4px 6px;line-height:1.2;
    overflow-wrap:anywhere}

  .twk-toggle{position:relative;width:32px;height:18px;border:0;border-radius:999px;
    background:rgba(0,0,0,.15);transition:background .15s;cursor:default;padding:0}
  .twk-toggle[data-on="1"]{background:#34c759}
  .twk-toggle i{position:absolute;top:2px;left:2px;width:14px;height:14px;border-radius:50%;
    background:#fff;box-shadow:0 1px 2px rgba(0,0,0,.25);transition:transform .15s}
  .twk-toggle[data-on="1"] i{transform:translateX(14px)}

  .twk-num{display:flex;align-items:center;box-sizing:border-box;min-width:0;height:26px;padding:0 0 0 8px;
    border:.5px solid rgba(0,0,0,.1);border-radius:7px;background:rgba(255,255,255,.6)}
  .twk-num-lbl{font-weight:500;color:rgba(41,38,27,.6);cursor:ew-resize;
    user-select:none;padding-right:8px}
  .twk-num input{flex:1;min-width:0;height:100%;border:0;background:transparent;
    font:inherit;font-variant-numeric:tabular-nums;text-align:right;padding:0 8px 0 0;
    outline:none;color:inherit;-moz-appearance:textfield}
  .twk-num input::-webkit-inner-spin-button,.twk-num input::-webkit-outer-spin-button{
    -webkit-appearance:none;margin:0}
  .twk-num-unit{padding-right:8px;color:rgba(41,38,27,.45)}

  .twk-btn{appearance:none;height:26px;padding:0 12px;border:0;border-radius:7px;
    background:rgba(0,0,0,.78);color:#fff;font:inherit;font-weight:500;cursor:default}
  .twk-btn:hover{background:rgba(0,0,0,.88)}
  .twk-btn.secondary{background:rgba(0,0,0,.06);color:inherit}
  .twk-btn.secondary:hover{background:rgba(0,0,0,.1)}

  .twk-swatch{appearance:none;-webkit-appearance:none;width:56px;height:22px;
    border:.5px solid rgba(0,0,0,.1);border-radius:6px;padding:0;cursor:default;
    background:transparent;flex-shrink:0}
  .twk-swatch::-webkit-color-swatch-wrapper{padding:0}
  .twk-swatch::-webkit-color-swatch{border:0;border-radius:5.5px}
  .twk-swatch::-moz-color-swatch{border:0;border-radius:5.5px}

  .twk-chips{display:flex;gap:6px}
  .twk-chip{position:relative;appearance:none;flex:1;min-width:0;height:46px;
    padding:0;border:0;border-radius:6px;overflow:hidden;cursor:default;
    box-shadow:0 0 0 .5px rgba(0,0,0,.12),0 1px 2px rgba(0,0,0,.06);
    transition:transform .12s cubic-bezier(.3,.7,.4,1),box-shadow .12s}
  .twk-chip:hover{transform:translateY(-1px);
    box-shadow:0 0 0 .5px rgba(0,0,0,.18),0 4px 10px rgba(0,0,0,.12)}
  .twk-chip[data-on="1"]{box-shadow:0 0 0 1.5px rgba(0,0,0,.85),
    0 2px 6px rgba(0,0,0,.15)}
  .twk-chip>span{position:absolute;top:0;bottom:0;right:0;width:34%;
    display:flex;flex-direction:column;box-shadow:-1px 0 0 rgba(0,0,0,.1)}
  .twk-chip>span>i{flex:1;box-shadow:0 -1px 0 rgba(0,0,0,.1)}
  .twk-chip>span>i:first-child{box-shadow:none}
  .twk-chip svg{position:absolute;top:6px;left:6px;width:13px;height:13px;
    filter:drop-shadow(0 1px 1px rgba(0,0,0,.3))}
`;

// ── useTweaks ───────────────────────────────────────────────────────────────
// Single source of truth for tweak values. setTweak persists via the host
// (__edit_mode_set_keys → host rewrites the EDITMODE block on disk).
function useTweaks(defaults) {
  const [values, setValues] = React.useState(defaults);
  // Accepts either setTweak('key', value) or setTweak({ key: value, ... }) so a
  // useState-style call doesn't write a "[object Object]" key into the persisted
  // JSON block.
  const setTweak = React.useCallback((keyOrEdits, val) => {
    const edits = typeof keyOrEdits === 'object' && keyOrEdits !== null ? keyOrEdits : {
      [keyOrEdits]: val
    };
    setValues(prev => ({
      ...prev,
      ...edits
    }));
    window.parent.postMessage({
      type: '__edit_mode_set_keys',
      edits
    }, '*');
    // Same-window signal so in-page listeners (deck-stage rail thumbnails)
    // can react — the parent message only reaches the host, not peers.
    window.dispatchEvent(new CustomEvent('tweakchange', {
      detail: edits
    }));
  }, []);
  return [values, setTweak];
}

// ── TweaksPanel ─────────────────────────────────────────────────────────────
// Floating shell. Registers the protocol listener BEFORE announcing
// availability — if the announce ran first, the host's activate could land
// before our handler exists and the toolbar toggle would silently no-op.
// The close button posts __edit_mode_dismissed so the host's toolbar toggle
// flips off in lockstep; the host echoes __deactivate_edit_mode back which
// is what actually hides the panel.
function TweaksPanel({
  title = 'Tweaks',
  children
}) {
  const [open, setOpen] = React.useState(false);
  const dragRef = React.useRef(null);
  const offsetRef = React.useRef({
    x: 16,
    y: 16
  });
  const PAD = 16;
  const clampToViewport = React.useCallback(() => {
    const panel = dragRef.current;
    if (!panel) return;
    const w = panel.offsetWidth,
      h = panel.offsetHeight;
    const maxRight = Math.max(PAD, window.innerWidth - w - PAD);
    const maxBottom = Math.max(PAD, window.innerHeight - h - PAD);
    offsetRef.current = {
      x: Math.min(maxRight, Math.max(PAD, offsetRef.current.x)),
      y: Math.min(maxBottom, Math.max(PAD, offsetRef.current.y))
    };
    panel.style.right = offsetRef.current.x + 'px';
    panel.style.bottom = offsetRef.current.y + 'px';
  }, []);
  React.useEffect(() => {
    if (!open) return;
    clampToViewport();
    if (typeof ResizeObserver === 'undefined') {
      window.addEventListener('resize', clampToViewport);
      return () => window.removeEventListener('resize', clampToViewport);
    }
    const ro = new ResizeObserver(clampToViewport);
    ro.observe(document.documentElement);
    return () => ro.disconnect();
  }, [open, clampToViewport]);
  React.useEffect(() => {
    const onMsg = e => {
      const t = e?.data?.type;
      if (t === '__activate_edit_mode') setOpen(true);else if (t === '__deactivate_edit_mode') setOpen(false);
    };
    window.addEventListener('message', onMsg);
    window.parent.postMessage({
      type: '__edit_mode_available'
    }, '*');
    return () => window.removeEventListener('message', onMsg);
  }, []);
  const dismiss = () => {
    setOpen(false);
    window.parent.postMessage({
      type: '__edit_mode_dismissed'
    }, '*');
  };
  const onDragStart = e => {
    const panel = dragRef.current;
    if (!panel) return;
    const r = panel.getBoundingClientRect();
    const sx = e.clientX,
      sy = e.clientY;
    const startRight = window.innerWidth - r.right;
    const startBottom = window.innerHeight - r.bottom;
    const move = ev => {
      offsetRef.current = {
        x: startRight - (ev.clientX - sx),
        y: startBottom - (ev.clientY - sy)
      };
      clampToViewport();
    };
    const up = () => {
      window.removeEventListener('mousemove', move);
      window.removeEventListener('mouseup', up);
    };
    window.addEventListener('mousemove', move);
    window.addEventListener('mouseup', up);
  };
  if (!open) return null;
  return /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement("style", null, __TWEAKS_STYLE), /*#__PURE__*/React.createElement("div", {
    ref: dragRef,
    className: "twk-panel",
    "data-omelette-chrome": "",
    style: {
      right: offsetRef.current.x,
      bottom: offsetRef.current.y
    }
  }, /*#__PURE__*/React.createElement("div", {
    className: "twk-hd",
    onMouseDown: onDragStart
  }, /*#__PURE__*/React.createElement("b", null, title), /*#__PURE__*/React.createElement("button", {
    className: "twk-x",
    "aria-label": "Close tweaks",
    onMouseDown: e => e.stopPropagation(),
    onClick: dismiss
  }, "\u2715")), /*#__PURE__*/React.createElement("div", {
    className: "twk-body"
  }, children)));
}

// ── Layout helpers ──────────────────────────────────────────────────────────

function TweakSection({
  label,
  children
}) {
  return /*#__PURE__*/React.createElement(React.Fragment, null, /*#__PURE__*/React.createElement("div", {
    className: "twk-sect"
  }, label), children);
}
function TweakRow({
  label,
  value,
  children,
  inline = false
}) {
  return /*#__PURE__*/React.createElement("div", {
    className: inline ? 'twk-row twk-row-h' : 'twk-row'
  }, /*#__PURE__*/React.createElement("div", {
    className: "twk-lbl"
  }, /*#__PURE__*/React.createElement("span", null, label), value != null && /*#__PURE__*/React.createElement("span", {
    className: "twk-val"
  }, value)), children);
}

// ── Controls ────────────────────────────────────────────────────────────────

function TweakSlider({
  label,
  value,
  min = 0,
  max = 100,
  step = 1,
  unit = '',
  onChange
}) {
  return /*#__PURE__*/React.createElement(TweakRow, {
    label: label,
    value: `${value}${unit}`
  }, /*#__PURE__*/React.createElement("input", {
    type: "range",
    className: "twk-slider",
    min: min,
    max: max,
    step: step,
    value: value,
    onChange: e => onChange(Number(e.target.value))
  }));
}
function TweakToggle({
  label,
  value,
  onChange
}) {
  return /*#__PURE__*/React.createElement("div", {
    className: "twk-row twk-row-h"
  }, /*#__PURE__*/React.createElement("div", {
    className: "twk-lbl"
  }, /*#__PURE__*/React.createElement("span", null, label)), /*#__PURE__*/React.createElement("button", {
    type: "button",
    className: "twk-toggle",
    "data-on": value ? '1' : '0',
    role: "switch",
    "aria-checked": !!value,
    onClick: () => onChange(!value)
  }, /*#__PURE__*/React.createElement("i", null)));
}
function TweakRadio({
  label,
  value,
  options,
  onChange
}) {
  const trackRef = React.useRef(null);
  const [dragging, setDragging] = React.useState(false);
  // The active value is read by pointer-move handlers attached for the lifetime
  // of a drag — ref it so a stale closure doesn't fire onChange for every move.
  const valueRef = React.useRef(value);
  valueRef.current = value;

  // Segments wrap mid-word once per-segment width runs out. The track is
  // ~248px (280 panel − 28 body pad − 4 seg pad), each button loses 12px
  // to its own padding, and 11.5px system-ui averages ~6.3px/char — so 2
  // options fit ~16 chars each, 3 fit ~10. Past that (or >3 options), fall
  // back to a dropdown rather than wrap.
  const labelLen = o => String(typeof o === 'object' ? o.label : o).length;
  const maxLen = options.reduce((m, o) => Math.max(m, labelLen(o)), 0);
  const fitsAsSegments = maxLen <= ({
    2: 16,
    3: 10
  }[options.length] ?? 0);
  if (!fitsAsSegments) {
    // <select> emits strings — map back to the original option value so the
    // fallback stays type-preserving (numbers, booleans) like the segment path.
    const resolve = s => {
      const m = options.find(o => String(typeof o === 'object' ? o.value : o) === s);
      return m === undefined ? s : typeof m === 'object' ? m.value : m;
    };
    return /*#__PURE__*/React.createElement(TweakSelect, {
      label: label,
      value: value,
      options: options,
      onChange: s => onChange(resolve(s))
    });
  }
  const opts = options.map(o => typeof o === 'object' ? o : {
    value: o,
    label: o
  });
  const idx = Math.max(0, opts.findIndex(o => o.value === value));
  const n = opts.length;
  const segAt = clientX => {
    const r = trackRef.current.getBoundingClientRect();
    const inner = r.width - 4;
    const i = Math.floor((clientX - r.left - 2) / inner * n);
    return opts[Math.max(0, Math.min(n - 1, i))].value;
  };
  const onPointerDown = e => {
    setDragging(true);
    const v0 = segAt(e.clientX);
    if (v0 !== valueRef.current) onChange(v0);
    const move = ev => {
      if (!trackRef.current) return;
      const v = segAt(ev.clientX);
      if (v !== valueRef.current) onChange(v);
    };
    const up = () => {
      setDragging(false);
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  };
  return /*#__PURE__*/React.createElement(TweakRow, {
    label: label
  }, /*#__PURE__*/React.createElement("div", {
    ref: trackRef,
    role: "radiogroup",
    onPointerDown: onPointerDown,
    className: dragging ? 'twk-seg dragging' : 'twk-seg'
  }, /*#__PURE__*/React.createElement("div", {
    className: "twk-seg-thumb",
    style: {
      left: `calc(2px + ${idx} * (100% - 4px) / ${n})`,
      width: `calc((100% - 4px) / ${n})`
    }
  }), opts.map(o => /*#__PURE__*/React.createElement("button", {
    key: o.value,
    type: "button",
    role: "radio",
    "aria-checked": o.value === value
  }, o.label))));
}
function TweakSelect({
  label,
  value,
  options,
  onChange
}) {
  return /*#__PURE__*/React.createElement(TweakRow, {
    label: label
  }, /*#__PURE__*/React.createElement("select", {
    className: "twk-field",
    value: value,
    onChange: e => onChange(e.target.value)
  }, options.map(o => {
    const v = typeof o === 'object' ? o.value : o;
    const l = typeof o === 'object' ? o.label : o;
    return /*#__PURE__*/React.createElement("option", {
      key: v,
      value: v
    }, l);
  })));
}
function TweakText({
  label,
  value,
  placeholder,
  onChange
}) {
  return /*#__PURE__*/React.createElement(TweakRow, {
    label: label
  }, /*#__PURE__*/React.createElement("input", {
    className: "twk-field",
    type: "text",
    value: value,
    placeholder: placeholder,
    onChange: e => onChange(e.target.value)
  }));
}
function TweakNumber({
  label,
  value,
  min,
  max,
  step = 1,
  unit = '',
  onChange
}) {
  const clamp = n => {
    if (min != null && n < min) return min;
    if (max != null && n > max) return max;
    return n;
  };
  const startRef = React.useRef({
    x: 0,
    val: 0
  });
  const onScrubStart = e => {
    e.preventDefault();
    startRef.current = {
      x: e.clientX,
      val: value
    };
    const decimals = (String(step).split('.')[1] || '').length;
    const move = ev => {
      const dx = ev.clientX - startRef.current.x;
      const raw = startRef.current.val + dx * step;
      const snapped = Math.round(raw / step) * step;
      onChange(clamp(Number(snapped.toFixed(decimals))));
    };
    const up = () => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
    };
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  };
  return /*#__PURE__*/React.createElement("div", {
    className: "twk-num"
  }, /*#__PURE__*/React.createElement("span", {
    className: "twk-num-lbl",
    onPointerDown: onScrubStart
  }, label), /*#__PURE__*/React.createElement("input", {
    type: "number",
    value: value,
    min: min,
    max: max,
    step: step,
    onChange: e => onChange(clamp(Number(e.target.value)))
  }), unit && /*#__PURE__*/React.createElement("span", {
    className: "twk-num-unit"
  }, unit));
}

// Relative-luminance contrast pick — checkmarks drawn over a swatch need to
// read on both #111 and #fafafa without per-option configuration. Hex input
// only (#rgb / #rrggbb); named or rgb()/hsl() colors fall through to "light".
function __twkIsLight(hex) {
  const h = String(hex).replace('#', '');
  const x = h.length === 3 ? h.replace(/./g, c => c + c) : h.padEnd(6, '0');
  const n = parseInt(x.slice(0, 6), 16);
  if (Number.isNaN(n)) return true;
  const r = n >> 16 & 255,
    g = n >> 8 & 255,
    b = n & 255;
  return r * 299 + g * 587 + b * 114 > 148000;
}
const __TwkCheck = ({
  light
}) => /*#__PURE__*/React.createElement("svg", {
  viewBox: "0 0 14 14",
  "aria-hidden": "true"
}, /*#__PURE__*/React.createElement("path", {
  d: "M3 7.2 5.8 10 11 4.2",
  fill: "none",
  strokeWidth: "2.2",
  strokeLinecap: "round",
  strokeLinejoin: "round",
  stroke: light ? 'rgba(0,0,0,.78)' : '#fff'
}));

// TweakColor — curated color/palette picker. Each option is either a single
// hex string or an array of 1-5 hex strings; the card adapts — a lone color
// renders solid, a palette renders colors[0] as the hero (left ~2/3) with the
// rest stacked in a sharp column on the right. onChange emits the
// option in the shape it was passed (string stays string, array stays array).
// Without options it falls back to the native color input for back-compat.
function TweakColor({
  label,
  value,
  options,
  onChange
}) {
  if (!options || !options.length) {
    return /*#__PURE__*/React.createElement("div", {
      className: "twk-row twk-row-h"
    }, /*#__PURE__*/React.createElement("div", {
      className: "twk-lbl"
    }, /*#__PURE__*/React.createElement("span", null, label)), /*#__PURE__*/React.createElement("input", {
      type: "color",
      className: "twk-swatch",
      value: value,
      onChange: e => onChange(e.target.value)
    }));
  }
  // Native <input type=color> emits lowercase hex per the HTML spec, so
  // compare case-insensitively. String() guards JSON.stringify(undefined),
  // which returns the primitive undefined (no .toLowerCase).
  const key = o => String(JSON.stringify(o)).toLowerCase();
  const cur = key(value);
  return /*#__PURE__*/React.createElement(TweakRow, {
    label: label
  }, /*#__PURE__*/React.createElement("div", {
    className: "twk-chips",
    role: "radiogroup"
  }, options.map((o, i) => {
    const colors = Array.isArray(o) ? o : [o];
    const [hero, ...rest] = colors;
    const sup = rest.slice(0, 4);
    const on = key(o) === cur;
    return /*#__PURE__*/React.createElement("button", {
      key: i,
      type: "button",
      className: "twk-chip",
      role: "radio",
      "aria-checked": on,
      "data-on": on ? '1' : '0',
      "aria-label": colors.join(', '),
      title: colors.join(' · '),
      style: {
        background: hero
      },
      onClick: () => onChange(o)
    }, sup.length > 0 && /*#__PURE__*/React.createElement("span", null, sup.map((c, j) => /*#__PURE__*/React.createElement("i", {
      key: j,
      style: {
        background: c
      }
    }))), on && /*#__PURE__*/React.createElement(__TwkCheck, {
      light: __twkIsLight(hero)
    }));
  })));
}
function TweakButton({
  label,
  onClick,
  secondary = false
}) {
  return /*#__PURE__*/React.createElement("button", {
    type: "button",
    className: secondary ? 'twk-btn secondary' : 'twk-btn',
    onClick: onClick
  }, label);
}
Object.assign(window, {
  useTweaks,
  TweaksPanel,
  TweakSection,
  TweakRow,
  TweakSlider,
  TweakToggle,
  TweakRadio,
  TweakSelect,
  TweakText,
  TweakNumber,
  TweakColor,
  TweakButton
});
})(); } catch (e) { __ds_ns.__errors.push({ path: "ui_kits/demo/tweaks-panel.jsx", error: String((e && e.message) || e) }); }

__ds_ns.Avatar = __ds_scope.Avatar;

__ds_ns.Badge = __ds_scope.Badge;

__ds_ns.Button = __ds_scope.Button;

__ds_ns.Card = __ds_scope.Card;

__ds_ns.ICON_PATHS = __ds_scope.ICON_PATHS;

__ds_ns.Icon = __ds_scope.Icon;

__ds_ns.IconButton = __ds_scope.IconButton;

__ds_ns.Dialog = __ds_scope.Dialog;

__ds_ns.Tooltip = __ds_scope.Tooltip;

__ds_ns.Checkbox = __ds_scope.Checkbox;

__ds_ns.Input = __ds_scope.Input;

__ds_ns.Segmented = __ds_scope.Segmented;

__ds_ns.Switch = __ds_scope.Switch;

__ds_ns.Tabs = __ds_scope.Tabs;

__ds_ns.AppShell = __ds_scope.AppShell;

__ds_ns.ThemeToggle = __ds_scope.ThemeToggle;

})();
