#!/usr/bin/env bash
# Provenance script for Buiy's embedded deterministic default font
# (docs/specs/2026-06-09-buiy-text-rendering-design/font-assets.md § 4,
# normative). Regenerating the artifact is ONLY done by re-running this
# script — the committed ttf is never edited by hand, so the embedded bytes
# are reproducible and the goldens' shaping baseline is auditable.
#
# Requirements:
#   - curl, sha256sum
#   - pyftsubset from fonttools, pinned: python3 -m pip install fonttools==4.56.0
#     (fontTools' subset module does not rewrite head.modified by default, so
#     the output is deterministic for a given input + fonttools version.)
set -euo pipefail

# --- pins -------------------------------------------------------------------
# Upstream: the mozilla/Fira foundry repo, release tag 4.202
# (commit 48a8d0a0354e933c0d1cfcf9feb07ccb00eb6fa9). Verified 2026-06-09.
UPSTREAM_FONT_URL="https://raw.githubusercontent.com/mozilla/Fira/4.202/ttf/FiraSans-Regular.ttf"
UPSTREAM_FONT_SHA256="a389cef71891df1232370fcebd7cfde5f74e741967070399adc91fd069b2094b"
UPSTREAM_LICENSE_URL="https://raw.githubusercontent.com/mozilla/Fira/4.202/LICENSE"
FONTTOOLS_PIN="4.56.0"

# The latin web-subset ranges (plan decision 7; font-assets § 4 pins "the
# latin unicode ranges" without enumerating — this list is the enumeration):
LATIN_UNICODES="U+0000-00FF,U+0131,U+0152-0153,U+2013-2014,U+2018-201A,U+201C-201E,U+2026,U+2039-203A"
# The layout features shaping needs (kerning, ligatures, composition, marks):
LAYOUT_FEATURES="ccmp,kern,liga,clig,calt,locl,mark,mkmk"

# --- paths ------------------------------------------------------------------
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="$REPO_ROOT/crates/buiy_core/assets/fonts"
OUT_FONT="$OUT_DIR/FiraSans-Regular-latin.ttf"
OUT_LICENSE="$OUT_DIR/OFL-FiraSans.txt"

# --- preflight ---------------------------------------------------------------
command -v pyftsubset >/dev/null || {
    echo "error: pyftsubset not found — python3 -m pip install fonttools==$FONTTOOLS_PIN" >&2
    exit 1
}
python3 - "$FONTTOOLS_PIN" <<'EOF'
import sys
import fontTools
want = sys.argv[1]
if fontTools.version != want:
    sys.exit(f"error: fontTools {fontTools.version} found, {want} required "
             f"(python3 -m pip install fonttools=={want})")
EOF

# --- fetch + verify ----------------------------------------------------------
TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT
curl -fsSL "$UPSTREAM_FONT_URL" -o "$TMP_DIR/FiraSans-Regular.ttf"
echo "$UPSTREAM_FONT_SHA256  $TMP_DIR/FiraSans-Regular.ttf" | sha256sum -c -
curl -fsSL "$UPSTREAM_LICENSE_URL" -o "$TMP_DIR/LICENSE"
grep -q "SIL Open Font License, Version 1.1" "$TMP_DIR/LICENSE"

# --- subset -------------------------------------------------------------------
mkdir -p "$OUT_DIR"
# --name-IDs='*' / --name-languages='*' / --name-legacy keep the full name
# table — the OFL requires the copyright + license records survive the subset
# (font-assets § 4). --notdef-outline keeps a visible tofu box.
pyftsubset "$TMP_DIR/FiraSans-Regular.ttf" \
    --output-file="$OUT_FONT" \
    --unicodes="$LATIN_UNICODES" \
    --layout-features="$LAYOUT_FEATURES" \
    --name-IDs='*' \
    --name-languages='*' \
    --name-legacy \
    --notdef-outline
cp "$TMP_DIR/LICENSE" "$OUT_LICENSE"

echo "wrote $OUT_FONT ($(stat -c%s "$OUT_FONT") bytes)"
echo "sha256: $(sha256sum "$OUT_FONT" | cut -d' ' -f1)"
echo "wrote $OUT_LICENSE"
