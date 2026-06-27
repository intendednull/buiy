// Headless-browser WebGPU smoke for Buiy — the only gate that exercises the
// real Tint compiler, so the only one that catches the WGSL-uniformity class
// (D2) that the native GPU lane cannot see (naga is lenient by design).
//
// Asserts, against a served Buiy WebGPU build:
//   (a) zero WGSL shader-module compilation errors (Tint),
//   (b) zero `create_render_pipeline` validation errors,
//   (c) the canvas actually painted (non-blank: pixel variance above a floor).
// Exits non-zero on any failure (CI gate).
//
// Env:
//   SMOKE_URL   served page URL (default http://localhost:8090/)
//   CHROME_BIN  chromium executable (default: Playwright's resolved browser)
//   SMOKE_WAIT  ms to wait for load+render (default 45000)
import { chromium } from 'playwright-core';

const URL = process.env.SMOKE_URL || 'http://localhost:8090/';
const WAIT = parseInt(process.env.SMOKE_WAIT || '45000', 10);
const EXEC = process.env.CHROME_BIN; // a recent (WebGPU-capable) Chrome/Chromium
if (!EXEC) { console.error('SMOKE FAIL: set CHROME_BIN to a WebGPU-capable Chrome/Chromium executable'); process.exit(2); }

const fail = (msg) => { console.error(`SMOKE FAIL: ${msg}`); process.exitCode = 1; };

const browser = await chromium.launch({
  executablePath: EXEC,
  headless: true,
  args: [
    '--no-sandbox',
    // Use a real GPU adapter when one is present (Chrome's WebGPU is Dawn-on-
    // Vulkan): these flags got the hardware adapter locally. On a GPU-LESS runner
    // no WebGPU adapter is exposed — Dawn supports neither lavapipe nor, in
    // practice, headless SwiftShader here (verified: every software-WebGPU flag
    // combination yields requestAdapter() -> null). The runner detects "no
    // adapter" and SKIPS the shader/paint checks rather than failing on the
    // environment (see below). The wasm BUILD is gated regardless; shader
    // conformance is enforced wherever a WebGPU adapter exists (dev GPU hosts; a
    // future WebGPU-capable CI runner — follow-ups.md).
    '--enable-features=Vulkan',
    '--use-angle=vulkan',
    '--ignore-gpu-blocklist',
    '--enable-unsafe-swiftshader',
  ],
});

try {
  const page = await browser.newPage({ viewport: { width: 800, height: 600 } });
  const consoleErrors = [];
  page.on('console', (m) => {
    const t = m.text();
    if (/Invalid ShaderModule|must only be called from uniform|create_render_pipeline|Validation Error|RuntimeError|panicked/i.test(t)) {
      consoleErrors.push(t.split('\n')[0]);
    }
  });
  page.on('pageerror', (e) => consoleErrors.push(`PAGEERROR ${e.message}`));
  page.on('crash', () => fail('page crashed'));

  // Capture Tint shader-compilation errors directly (bevy only logs the cascade).
  await page.addInitScript(() => {
    window.__shaderErrors = [];
    const proto = self.GPUDevice && self.GPUDevice.prototype;
    if (proto && proto.createShaderModule) {
      const orig = proto.createShaderModule;
      proto.createShaderModule = function (desc) {
        const mod = orig.call(this, desc);
        if (mod.getCompilationInfo) {
          mod.getCompilationInfo().then((info) => {
            for (const m of info.messages || []) {
              if (m.type === 'error') window.__shaderErrors.push(`L${m.lineNum}:${m.linePos} ${m.message}`);
            }
          }).catch(() => {});
        }
        return mod;
      };
    }
  });

  await page.goto(URL, { waitUntil: 'domcontentloaded', timeout: 60000 });

  // Query WebGPU on the SERVED page (a secure context), not about:blank.
  const gpu = await page.evaluate(async () => {
    if (!navigator.gpu) return { hasGpu: false };
    const a = await navigator.gpu.requestAdapter().catch(() => null);
    return { hasGpu: true, adapter: !!a };
  });
  console.log(`webgpu: ${JSON.stringify(gpu)}`);
  if (!gpu.hasGpu || !gpu.adapter) {
    // No WebGPU adapter in this environment (e.g. a GPU-less CI runner). The
    // shader-conformance + paint checks below need a real Tint compile, which
    // needs an adapter — so SKIP them (exit 0) rather than fail on the
    // environment. The wasm BUILD is still gated by the CI job. Do NOT inspect
    // the console errors: bevy's renderer-init panic is downstream of the missing
    // adapter, not a shader bug.
    console.log(
      'SMOKE: SKIP — no WebGPU adapter here, so shader-conformance + paint are NOT enforced ' +
      '(the wasm BUILD is gated by CI). Run on a WebGPU-capable host to enforce shader conformance.',
    );
    await browser.close();
    process.exit(0);
  }

  await page.waitForTimeout(WAIT);

  const shaderErrors = await page.evaluate(() => window.__shaderErrors || []);
  if (shaderErrors.length) {
    fail(`${shaderErrors.length} WGSL shader-compilation error(s) (Tint):`);
    for (const e of shaderErrors.slice(0, 10)) console.error(`  ${e}`);
  }
  if (consoleErrors.length) {
    fail(`${consoleErrors.length} render/pipeline error(s) in console:`);
    for (const e of [...new Set(consoleErrors)].slice(0, 10)) console.error(`  ${e}`);
  }

  // (c) the canvas painted: screenshot it and require pixel variance above a floor.
  const el = await page.$('#buiy');
  if (!el) { fail('#buiy canvas not found'); }
  else {
    const buf = await el.screenshot();
    // Cheap variance check on the raw PNG bytes: a blank canvas compresses to a
    // near-uniform buffer; require a spread of distinct byte values.
    const seen = new Set();
    for (let i = 0; i < buf.length; i += 7) seen.add(buf[i]);
    if (seen.size < 16) fail(`canvas looks blank (only ${seen.size} distinct sampled bytes)`);
    else console.log(`canvas painted: ${seen.size} distinct sampled bytes`);
  }

  if (process.exitCode) console.error('SMOKE: FAILED');
  else console.log('SMOKE: PASS (0 shader/pipeline errors, canvas painted)');
} finally {
  await browser.close();
}
