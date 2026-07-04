// Headless-browser HiDPI correctness check for a Buiy web build (Dooduel F9).
//
// WHY THIS EXISTS. The prototype's mobile testing reported a "UI renders ~dpr×
// too large and overflows at devicePixelRatio > 1" bug. The F9 investigation
// (docs/reports/2026-07-04-wasm-hidpi-investigation.md) proved that symptom is a
// headless-Chromium EMULATION ARTIFACT, not a real-device bug: winit derives the
// window's LOGICAL size as `physical / scale_factor`, where
//   * scale_factor = window.devicePixelRatio, and
//   * physical     = the DevicePixelContentBox ResizeObserver (true device px).
// On a real device those two dpr signals are ALWAYS consistent, so
// logical == CSS size and Buiy renders at the right size (crisp). Chromium's
// per-context `deviceScaleFactor` fakes devicePixelRatio in JS WITHOUT
// supersampling the device-pixel-content-box, so the two disagree and winit
// computes a wrong logical size — a false "2× too large" (or, with only the
// process `--force-device-scale-factor`, a false "2× too small").
//
// This gate reproduces a REAL device faithfully by setting BOTH signals to the
// same dpr (context deviceScaleFactor + process --force-device-scale-factor) and
// then asserts the sizing invariants below. It is the correct form of the
// HiDPI acceptance gate the spec designates as a manual milestone (§4.4). Run it
// against a served build (both backends) at dsf 2 / 3.
//
// Asserts, at a consistent emulated dpr:
//   (a) canvas backing store == round(CSS × dpr)   — the canvas is HiDPI-crisp,
//   (b) derived logical (backing / dpr) == CSS px   — the app viewport == the CSS
//       viewport (no dpr× mis-scale: the exact thing that overflowed),
//   (c) no horizontal document overflow             — content fits the viewport,
//   (d) the canvas painted (pixel-variance floor).
// Exits non-zero on any failure. Writes a screenshot to SHOT.
//
// Env:
//   SMOKE_URL   served page URL (default http://localhost:8099/)
//   DSF         emulated devicePixelRatio to test (default 2)
//   VW, VH      CSS viewport size (default 390x844, the phone target)
//   WAIT        ms to wait for load+render (default 20000)
//   CHROME_BIN  chromium executable (required)
//   SHOT        screenshot output (default /tmp/hidpi-check.png)
import { chromium } from 'playwright-core';
import fs from 'node:fs';

const URL = process.env.SMOKE_URL || 'http://localhost:8099/';
const DSF = parseFloat(process.env.DSF || '2');
const VW = parseInt(process.env.VW || '390', 10);
const VH = parseInt(process.env.VH || '844', 10);
const WAIT = parseInt(process.env.WAIT || '20000', 10);
const EXEC = process.env.CHROME_BIN;
const SHOT = process.env.SHOT || '/tmp/hidpi-check.png';
if (!EXEC) { console.error('HIDPI CHECK FAIL: set CHROME_BIN'); process.exit(2); }

const fail = (msg) => { console.error('HIDPI CHECK FAIL: ' + msg); process.exitCode = 1; };

const browser = await chromium.launch({
  executablePath: EXEC,
  headless: true,
  args: [
    '--no-sandbox', '--enable-unsafe-swiftshader', '--use-angle=swiftshader', '--ignore-gpu-blocklist',
    // Process-level dpr: makes the compositor's device-pixel-content-box scale
    // by dsf (winit's physical-size source). Paired with the context override
    // below so BOTH dpr signals agree, as on a real device.
    `--force-device-scale-factor=${DSF}`, '--high-dpi-support=1',
  ],
});

try {
  // Context-level dpr: makes window.devicePixelRatio == dsf (winit's scale_factor
  // source). Both signals set => consistent, faithful to a real HiDPI device.
  const ctx = await browser.newContext({ viewport: { width: VW, height: VH }, deviceScaleFactor: DSF });
  const page = await ctx.newPage();
  const consoleErrors = [];
  page.on('console', (m) => {
    const t = m.text();
    if (/panicked|RuntimeError|Validation Error|could not compile|GL_INVALID/i.test(t)) consoleErrors.push(t.split('\n')[0]);
  });
  page.on('pageerror', (e) => consoleErrors.push('PAGEERROR ' + e.message.split('\n')[0]));

  await page.goto(URL, { waitUntil: 'domcontentloaded', timeout: 60000 });
  await page.waitForTimeout(WAIT);

  const m = await page.evaluate(() => {
    const c = document.querySelector('#buiy');
    if (!c) return { error: 'no #buiy canvas' };
    const cs = getComputedStyle(c);
    const d = document.documentElement;
    return {
      dpr: window.devicePixelRatio,
      innerWidth: window.innerWidth,
      innerHeight: window.innerHeight,
      backing: { w: c.width, h: c.height },
      cssW: parseFloat(cs.width),
      cssH: parseFloat(cs.height),
      overflowX: d.scrollWidth - d.clientWidth,
      overflowY: d.scrollHeight - d.clientHeight,
    };
  });

  console.log(`--- HiDPI check: dsf=${DSF} viewport=${VW}x${VH} ---`);
  console.log(JSON.stringify(m));
  if (m.error) { fail(m.error); }
  else {
    // (a) HiDPI-crisp backing store.
    const wantBackW = Math.round(m.cssW * m.dpr);
    if (Math.abs(m.backing.w - wantBackW) > 1) {
      fail(`canvas backing width ${m.backing.w} != round(CSS ${m.cssW} × dpr ${m.dpr}) = ${wantBackW} — not HiDPI-crisp`);
    }
    // (b) logical == CSS (no dpr× mis-scale — the reported overflow class).
    const logical = m.backing.w / m.dpr;
    if (Math.abs(logical - m.cssW) > 1) {
      fail(`derived logical width ${logical.toFixed(1)} != CSS ${m.cssW} — the app viewport is dpr× mis-scaled`);
    }
    // (c) content fits: no horizontal overflow (a few px slack for scrollbars).
    if (m.overflowX > 4) {
      fail(`horizontal overflow ${m.overflowX}px at dsf=${DSF} — UI spills past the viewport`);
    }
    if (!process.exitCode) console.log(`OK: backing ${m.backing.w}×${m.backing.h} = CSS ${m.cssW}×${m.cssH} × dpr ${m.dpr}; logical == CSS; no overflow`);
  }

  const el = await page.$('#buiy');
  if (el) {
    const buf = await el.screenshot();
    fs.writeFileSync(SHOT, buf);
    const seen = new Set();
    for (let i = 0; i < buf.length; i += 7) seen.add(buf[i]);
    if (seen.size < 16) fail(`canvas looks blank (only ${seen.size} distinct sampled bytes)`);
    else console.log(`canvas painted: ${seen.size} distinct sampled bytes`);
    console.log('screenshot -> ' + SHOT);
  } else {
    fail('#buiy canvas not found for screenshot');
  }
  if (consoleErrors.length) { fail(`${consoleErrors.length} console error(s)`); for (const e of [...new Set(consoleErrors)].slice(0, 5)) console.error('  ' + e); }

  console.log(process.exitCode ? 'HIDPI CHECK: FAILED' : 'HIDPI CHECK: PASS');
} finally {
  await browser.close();
}
