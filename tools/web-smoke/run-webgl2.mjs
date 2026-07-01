// Headless-browser WebGL2 smoke for Buiy — the reach-backend sibling of run.mjs
// (the WebGPU/Tint gate). WebGL2's advantage over WebGPU for CI: software WebGL2
// (SwiftShader/ANGLE) IS available on a GPU-less runner, so — unlike the WebGPU
// smoke, which SKIPS its shader/paint check with no adapter — this gate is
// FULLY ENFORCED in CI (spec 2026-06-30 § D4).
//
// Asserts against a served Buiy WebGL2 build:
//   (a) a WebGL2 context exists (backend init),
//   (b) zero GLSL-ES shader COMPILE/LINK errors (naga WGSL->GLSL ES), captured by
//       hooking compileShader/linkProgram (the WebGL2 analogue of Tint's
//       getCompilationInfo),
//   (c) zero wgpu/render/panic console errors (non-render 404s are ignored),
//   (d) the canvas painted (pixel variance above a floor).
// Exits non-zero on any failure (CI gate). Writes a screenshot to SHOT_PATH.
//
// Env:
//   SMOKE_URL           served page URL (default http://localhost:8091/)
//   SMOKE_WAIT          ms to wait for load+render (default 30000)
//   CHROME_BIN          chromium executable (required)
//   SHOT_PATH           screenshot output (default /tmp/webgl2-shot.png)
//   WEBGL2_CHROME_ARGS  comma-separated Chrome flags. Default forces software
//                       WebGL2 (SwiftShader) so it works on a GPU-less CI runner;
//                       a dev GPU host can pass e.g.
//                       "--no-sandbox,--use-angle=vulkan,--ignore-gpu-blocklist".
import { chromium } from 'playwright-core';
import fs from 'node:fs';

const URL = process.env.SMOKE_URL || 'http://localhost:8091/';
const WAIT = parseInt(process.env.SMOKE_WAIT || '30000', 10);
const EXEC = process.env.CHROME_BIN;
const SHOT = process.env.SHOT_PATH || '/tmp/webgl2-shot.png';
const ARGS = (process.env.WEBGL2_CHROME_ARGS ||
  '--no-sandbox,--enable-unsafe-swiftshader,--use-angle=swiftshader,--ignore-gpu-blocklist')
  .split(',')
  .map((s) => s.trim())
  .filter(Boolean);
if (!EXEC) { console.error('WEBGL2 SMOKE FAIL: set CHROME_BIN'); process.exit(2); }

const browser = await chromium.launch({ executablePath: EXEC, headless: true, args: ARGS });

try {
  const page = await browser.newPage({ viewport: { width: 1024, height: 720 } });
  const consoleErrors = [];
  page.on('console', (m) => {
    const t = m.text();
    if (/404|Failed to load resource/i.test(t)) return; // non-render asset misses
    if (m.type() === 'error' ||
        /panicked|RuntimeError|Validation Error|Failed to create|shader translation|link error|GL_INVALID|could not compile/i.test(t)) {
      consoleErrors.push(`[${m.type()}] ${t.split('\n')[0]}`);
    }
  });
  page.on('pageerror', (e) => consoleErrors.push(`PAGEERROR ${e.message.split('\n')[0]}`));
  page.on('crash', () => { console.error('page crashed'); process.exitCode = 1; });

  // Capture GLSL-ES compile/link failures directly (bevy logs only the cascade).
  await page.addInitScript(() => {
    window.__glShaderErrors = [];
    const g = self.WebGL2RenderingContext && self.WebGL2RenderingContext.prototype;
    if (g && g.compileShader) {
      const oc = g.compileShader;
      g.compileShader = function (sh) {
        oc.call(this, sh);
        try {
          if (!this.getShaderParameter(sh, this.COMPILE_STATUS))
            window.__glShaderErrors.push('COMPILE: ' + (this.getShaderInfoLog(sh) || '').trim().split('\n')[0]);
        } catch (_) {}
      };
      const ol = g.linkProgram;
      g.linkProgram = function (pr) {
        ol.call(this, pr);
        try {
          if (!this.getProgramParameter(pr, this.LINK_STATUS))
            window.__glShaderErrors.push('LINK: ' + (this.getProgramInfoLog(pr) || '').trim().split('\n')[0]);
        } catch (_) {}
      };
    }
  });

  await page.goto(URL, { waitUntil: 'domcontentloaded', timeout: 60000 });
  await page.waitForTimeout(WAIT);

  const ctx = await page.evaluate(() => {
    const c = document.querySelector('#buiy');
    if (!c) return { canvas: false };
    const gl2 = c.getContext('webgl2');
    return { canvas: true, webgl2: !!gl2, version: gl2 ? gl2.getParameter(gl2.VERSION) : null };
  });
  console.log('context: ' + JSON.stringify(ctx));
  if (!ctx.canvas) { console.error('WEBGL2 SMOKE FAIL: #buiy canvas not found'); process.exitCode = 1; }
  if (!ctx.webgl2) { console.error('WEBGL2 SMOKE FAIL: no WebGL2 context (backend init failed)'); process.exitCode = 1; }

  const glErrors = await page.evaluate(() => window.__glShaderErrors || []);
  if (glErrors.length) {
    process.exitCode = 1;
    console.error(`${glErrors.length} GLSL-ES compile/link error(s):`);
    for (const e of glErrors.slice(0, 20)) console.error('  ' + e);
  }
  if (consoleErrors.length) {
    process.exitCode = 1;
    console.error(`${consoleErrors.length} console error(s):`);
    for (const e of [...new Set(consoleErrors)].slice(0, 20)) console.error('  ' + e);
  }

  const el = await page.$('#buiy');
  if (el) {
    const buf = await el.screenshot();
    fs.writeFileSync(SHOT, buf);
    const seen = new Set();
    for (let i = 0; i < buf.length; i += 7) seen.add(buf[i]);
    if (seen.size < 16) { process.exitCode = 1; console.error(`canvas looks blank (only ${seen.size} distinct sampled bytes)`); }
    else console.log(`canvas painted: ${seen.size} distinct sampled bytes`);
    console.log('screenshot -> ' + SHOT);
  }

  console.log(process.exitCode ? 'WEBGL2 SMOKE: FAILED' : 'WEBGL2 SMOKE: PASS (0 shader/pipeline errors, canvas painted)');
} finally {
  await browser.close();
}
