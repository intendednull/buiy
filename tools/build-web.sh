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
#         RELEASE=1 tools/build-web.sh ...    (size-optimized build)
set -euo pipefail

EX="${1:-examples/gallery_web}"
[ -f "$EX/index.html" ] || { echo "no $EX/index.html — pass a web example dir" >&2; exit 1; }
OUT="$EX/dist-web"
REL="${RELEASE:+--release}"

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
    const BUILDS = {
      webgpu: { js: '/webgpu/${gpu_js}', wasm: '/webgpu/${gpu_wasm}' },
      webgl2: { js: '/webgl2/${gl2_js}', wasm: '/webgl2/${gl2_wasm}' },
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
