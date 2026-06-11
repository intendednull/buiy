#!/usr/bin/env bash
# Provenance script for the per-script OFL fixture fonts the multi-script
# shaping corpus + resolver tests pin against (T5 plan decision 12;
# docs/specs/2026-06-09-buiy-text-rendering-design/verification.md § 2.2).
# Regenerating the artifacts is ONLY done by re-running this script — the
# committed fonts are never edited by hand, so the corpus' shaping baseline
# is reproducible and auditable. The `subset_default_font.sh` pattern, once
# per script.
#
# Requirements:
#   - curl, sha256sum
#   - pyftsubset from fonttools, pinned: python3 -m pip install fonttools==4.56.0
#     (fontTools' subset module does not rewrite head.modified by default, so
#     the output is deterministic for a given input + fonttools version.)
set -euo pipefail

# --- pins -------------------------------------------------------------------
# Upstreams, all raw at PINNED refs. Verified 2026-06-10.
#
# notofonts/notofonts.github.io (the Noto release megarepo) at commit
# 129db1d0… — Arabic, Hebrew, Devanagari hinted Regular ttfs. The repo
# carries no per-font OFL, so each family's OFL (correct copyright + RFN
# lines) comes from google/fonts at the commit pinned below.
NOTOFONTS_COMMIT="129db1d0cb9ae8afee1aeadd533f35a1580ad2c0"
NOTOFONTS_BASE="https://raw.githubusercontent.com/notofonts/notofonts.github.io/$NOTOFONTS_COMMIT"
# notofonts/noto-cjk at release tag Sans2.004 — the static SC Regular otf
# (sfnt-wrapped CFF; the loader's sfnt invariant accepts it) + the repo's
# own OFL LICENSE.
NOTO_CJK_TAG="Sans2.004"
NOTO_CJK_BASE="https://raw.githubusercontent.com/notofonts/noto-cjk/$NOTO_CJK_TAG"
# google/fonts at commit 877f8918… — the MONOCHROME variable
# NotoEmoji[wght].ttf (outline glyphs → SwashContent::Mask; the v1 producer
# skips Color content, T5 plan decision 12) + the per-family OFL texts.
# googlefonts/noto-emoji deleted its static fonts/NotoEmoji-Regular.ttf in
# 2022 ("Remove outdated…", 1442f6ac) and the last static build (v1.05,
# 2015) lacks U+200D/U+FE0F in its cmap — the corpus ZWJ sequence would
# never ligate. swash renders the variable font's default instance.
GOOGLE_FONTS_COMMIT="877f8918ee661764418e085766dc0b073260a3ef"
GOOGLE_FONTS_BASE="https://raw.githubusercontent.com/google/fonts/$GOOGLE_FONTS_COMMIT"
FONTTOOLS_PIN="4.56.0"

ARABIC_SHA256="bdff3e5659d67e67def05b33f749683b9376ae819d65d3dd62ac4640b3aaef48"
HEBREW_SHA256="cdefaf8efd47045f6820928eba84db5bed7557539328952b5f828315485e02ee"
DEVANAGARI_SHA256="306b53ecfb182a504dd8a7446093c316387d2fd8dc350d0792ed1753fe0996cd"
CJK_SC_SHA256="2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b"
EMOJI_SHA256="de6c18832938afc99caf132b39d6a30a19bac7f2e812e28db2535b4608d27551"

# The layout features shaping needs, per script (the subset_default_font.sh
# base set + the script-specific shaping features):
BASE_FEATURES="ccmp,kern,liga,clig,calt,locl,mark,mkmk"
ARABIC_FEATURES="$BASE_FEATURES,init,medi,fina,isol,rlig"               # joining
INDIC_FEATURES="$BASE_FEATURES,nukt,akhn,rphf,blwf,half,vatu,pres,abvs,blws,psts,haln,abvm,blwm" # conjunct reordering
EMOJI_FEATURES="ccmp,liga"                                              # ZWJ ligatures

# Subset ranges: the script's block + space + the joining/direction controls
# the corpus strings exercise (the CJK subset is --text=<corpus string> —
# keeps the artifact tiny).
ARABIC_UNICODES="U+0600-06FF,U+0750-077F,U+0020,U+200C-200F"
HEBREW_UNICODES="U+0590-05FF,U+0020,U+200E-200F"
DEVANAGARI_UNICODES="U+0900-097F,U+0020,U+200C-200D"
CJK_TEXT="你好，世界"
EMOJI_UNICODES="U+1F468,U+1F469,U+1F467,U+1F466,U+200D,U+FE0F,U+0020"

# --- paths ------------------------------------------------------------------
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
OUT_DIR="$REPO_ROOT/crates/buiy_core/tests/fixtures/fonts"

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

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT
mkdir -p "$OUT_DIR"

# fetch_verified <url> <sha256> <out-file>
fetch_verified() {
    curl -fsSL "$1" -o "$3"
    echo "$2  $3" | sha256sum -c -
}

# fetch_ofl <url> <out-name> — the license must be the OFL (the artifacts'
# name tables keep the copyright records via --name-IDs='*' below). The
# noto-cjk LICENSE wraps the phrase across a line break, so match
# newline-tolerantly.
fetch_ofl() {
    curl -fsSL "$1" -o "$TMP_DIR/$2"
    tr '\n' ' ' <"$TMP_DIR/$2" | grep -q "SIL Open Font License, Version 1.1"
    cp "$TMP_DIR/$2" "$OUT_DIR/$2"
}

# subset <in> <out> <features> [pyftsubset selector args…]
# --name-IDs='*' / --name-languages='*' / --name-legacy keep the full name
# table — the OFL requires the copyright + license records survive the
# subset, and fontdb/the registry match by the declared family name.
# --notdef-outline keeps a visible tofu box.
subset() {
    local in="$1" out="$2" features="$3"
    shift 3
    pyftsubset "$TMP_DIR/$in" \
        --output-file="$OUT_DIR/$out" \
        --layout-features="$features" \
        --name-IDs='*' \
        --name-languages='*' \
        --name-legacy \
        --notdef-outline \
        "$@"
}

# assert_size <file> <max-bytes> — a bloated subset means the ranges are wrong.
assert_size() {
    local size
    size="$(stat -c%s "$OUT_DIR/$1")"
    if [ "$size" -gt "$2" ]; then
        echo "error: $1 is $size bytes (> $2) — the subset ranges are wrong" >&2
        exit 1
    fi
    echo "wrote $OUT_DIR/$1 ($size bytes)"
}

# --- fetch + verify ----------------------------------------------------------
fetch_verified "$NOTOFONTS_BASE/fonts/NotoSansArabic/hinted/ttf/NotoSansArabic-Regular.ttf" \
    "$ARABIC_SHA256" "$TMP_DIR/NotoSansArabic-Regular.ttf"
fetch_verified "$NOTOFONTS_BASE/fonts/NotoSansHebrew/hinted/ttf/NotoSansHebrew-Regular.ttf" \
    "$HEBREW_SHA256" "$TMP_DIR/NotoSansHebrew-Regular.ttf"
fetch_verified "$NOTOFONTS_BASE/fonts/NotoSansDevanagari/hinted/ttf/NotoSansDevanagari-Regular.ttf" \
    "$DEVANAGARI_SHA256" "$TMP_DIR/NotoSansDevanagari-Regular.ttf"
fetch_verified "$NOTO_CJK_BASE/Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf" \
    "$CJK_SC_SHA256" "$TMP_DIR/NotoSansCJKsc-Regular.otf"
fetch_verified "$GOOGLE_FONTS_BASE/ofl/notoemoji/NotoEmoji%5Bwght%5D.ttf" \
    "$EMOJI_SHA256" "$TMP_DIR/NotoEmoji-wght.ttf"

fetch_ofl "$GOOGLE_FONTS_BASE/ofl/notosansarabic/OFL.txt" "OFL-NotoSansArabic.txt"
fetch_ofl "$GOOGLE_FONTS_BASE/ofl/notosanshebrew/OFL.txt" "OFL-NotoSansHebrew.txt"
fetch_ofl "$GOOGLE_FONTS_BASE/ofl/notosansdevanagari/OFL.txt" "OFL-NotoSansDevanagari.txt"
fetch_ofl "$NOTO_CJK_BASE/LICENSE" "OFL-NotoSansCJKsc.txt"
fetch_ofl "$GOOGLE_FONTS_BASE/ofl/notoemoji/OFL.txt" "OFL-NotoEmoji.txt"

# --- subset -------------------------------------------------------------------
subset NotoSansArabic-Regular.ttf NotoSansArabic-arabic.ttf \
    "$ARABIC_FEATURES" --unicodes="$ARABIC_UNICODES"
subset NotoSansHebrew-Regular.ttf NotoSansHebrew-hebrew.ttf \
    "$BASE_FEATURES" --unicodes="$HEBREW_UNICODES"
subset NotoSansDevanagari-Regular.ttf NotoSansDevanagari-devanagari.ttf \
    "$INDIC_FEATURES" --unicodes="$DEVANAGARI_UNICODES"
subset NotoSansCJKsc-Regular.otf NotoSansCJKsc-han.otf \
    "$BASE_FEATURES" --text="$CJK_TEXT"
subset NotoEmoji-wght.ttf NotoEmoji-emoji.ttf \
    "$EMOJI_FEATURES" --unicodes="$EMOJI_UNICODES"

assert_size NotoSansArabic-arabic.ttf $((200 * 1024))
assert_size NotoSansHebrew-hebrew.ttf $((200 * 1024))
assert_size NotoSansDevanagari-devanagari.ttf $((200 * 1024))
assert_size NotoSansCJKsc-han.otf $((1024 * 1024))
assert_size NotoEmoji-emoji.ttf $((200 * 1024))

for f in "$OUT_DIR"/*.ttf "$OUT_DIR"/*.otf; do
    echo "sha256: $(sha256sum "$f" | cut -d' ' -f1)  $(basename "$f")"
done
