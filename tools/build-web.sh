#!/usr/bin/env bash
# Build a Buiy web example as BOTH a WebGPU and a WebGL2 artifact plus a
# feature-detect `navigator.gpu` loader, into <example>/dist-web/.
#
# The two bevy backend meta-features cannot coexist in one binary (`webgpu` wins),
# so browser reach = two artifacts + a JS switch: load the WebGPU build when a
# usable WebGPU adapter is present, else the WebGL2 build (unflagged in every
# modern browser). See docs/specs/2026-06-30-buiy-browser-reach-widening-design.md § D1.
#
# Usage:  tools/build-web.sh [example-dir]   (default: examples/gallery_web)
#         RELEASE=1 tools/build-web.sh ...    (size-optimized shipping build)
#
# RELEASE=1 builds through the `wasm-release` cargo profile (opt-level="s" + LTO +
# codegen-units=1 + strip) rather than the speed-tuned default `release`, then runs
# a `wasm-opt -Oz` size pass over each artifact (see below). Use it for anything you
# deploy; omit it for a fast dev build.
set -euo pipefail

EX="${1:-examples/gallery_web}"
[ -f "$EX/index.html" ] || { echo "no $EX/index.html — pass a web example dir" >&2; exit 1; }
OUT="$EX/dist-web"
REL="${RELEASE:+--release --cargo-profile wasm-release}"

rm -rf "$OUT"
mkdir -p "$OUT/webgpu" "$OUT/webgl2"

echo "== building WebGPU artifact =="
trunk build "$EX/index.html" --features webgpu $REL --dist "$OUT/webgpu"
echo "== building WebGL2 artifact =="
trunk build "$EX/index.html" --features webgl2 $REL --dist "$OUT/webgl2"

# Discover the wasm-bindgen glue + wasm names trunk emitted in each subdir.
gpu_js=$(cd "$OUT/webgpu" && ls *.js | grep -v -- '-loader' | head -1)
gpu_wasm=$(cd "$OUT/webgpu" && ls *_bg.wasm | head -1)
gl2_js=$(cd "$OUT/webgl2" && ls *.js | grep -v -- '-loader' | head -1)
gl2_wasm=$(cd "$OUT/webgl2" && ls *_bg.wasm | head -1)

# Size pass (release only): wasm-opt -Oz on each artifact, in place. We run it
# here rather than via trunk's `data-wasm-opt` because trunk 0.21's invocation
# omits the wasm-feature flags modern rustc emits (bulk-memory / reference-types /
# …) and so fails outright. `-all` enables those features. Optional + graceful:
# skipped for dev builds and when wasm-opt is absent — the `wasm-release` cargo
# profile already did the bulk of the shrinking, so the bundle is still valid.
if [ -n "${RELEASE:-}" ]; then
  if command -v wasm-opt >/dev/null 2>&1; then
    for w in "$OUT/webgpu/$gpu_wasm" "$OUT/webgl2/$gl2_wasm"; do
      before=$(wc -c < "$w")
      if wasm-opt -Oz -all "$w" -o "$w.opt"; then
        mv "$w.opt" "$w"
        echo "wasm-opt -Oz $(basename "$w"): $((before / 1048576)) MB -> $(( $(wc -c < "$w") / 1048576 )) MB"
      else
        rm -f "$w.opt"
        echo "WARN: wasm-opt failed on $(basename "$w") — keeping the profile-optimized wasm" >&2
      fi
    done
  else
    echo "NOTE: wasm-opt not on PATH — shipping the wasm-release-profile build without the extra -Oz size pass (install binaryen for the smallest artifact)." >&2
  fi
fi

cat > "$OUT/index.html" <<HTML
<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="utf-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1, maximum-scale=1" />
  <title>Buiy — WebGPU / WebGL2 (auto)</title>
  <style>
    html, body { margin: 0; padding: 0; width: 100%; height: 100%; background: #16181d; overflow: hidden; }
    #buiy { display: block; width: 100vw; height: 100vh; }
  </style>
</head>
<body>
  <canvas id="buiy"></canvas>
  <script type="module">
    // Feature-detect a usable WebGPU adapter; load the WebGPU build if present,
    // else the WebGL2 reach build. \`?force=webgpu|webgl2\` overrides (test hook).
    //
    // Resolve each build's assets against THIS page's own directory (not the
    // domain root), so the bundle is relocatable: it serves correctly whether it
    // sits at the site root OR under a project-page subpath like \`…github.io/<repo>/\`.
    const at = (p) => new URL(p, new URL('.', location.href)).href;
    const BUILDS = {
      webgpu: { js: at('webgpu/${gpu_js}'), wasm: at('webgpu/${gpu_wasm}') },
      webgl2: { js: at('webgl2/${gl2_js}'), wasm: at('webgl2/${gl2_wasm}') },
    };
    async function pick() {
      const f = new URLSearchParams(location.search).get('force');
      if (f === 'webgpu' || f === 'webgl2') return f;
      if (!navigator.gpu) return 'webgl2';
      try { return (await navigator.gpu.requestAdapter()) ? 'webgpu' : 'webgl2'; }
      catch { return 'webgl2'; }
    }
    const b = BUILDS[await pick()];
    const { default: init } = await import(b.js);
    await init({ module_or_path: b.wasm });
  </script>
</body>
</html>
HTML

echo "built $OUT (webgpu: $gpu_js / webgl2: $gl2_js + loader)"
