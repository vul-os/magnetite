#!/usr/bin/env node
// Verify in a REAL browser that a bundle served by `magnetite-web-host` is
// cross-origin isolated, and therefore that `SharedArrayBuffer` exists.
//
// WHY THIS SCRIPT EXISTS
// ----------------------
// ALIGNMENT.md §5 says to "Test against a real Godot export early". A Godot 4
// web export cannot boot without `SharedArrayBuffer`, which the browser exposes
// only to a cross-origin-isolated document. No amount of asserting on response
// headers in Rust proves the browser agreed to isolate the document — only a
// browser can say that. So this script drives a real Chromium against a real
// `serve-web-bundle` process and reads the three facts out of the page:
//
//   isSecureContext        (http://127.0.0.1 counts; other plain-HTTP hosts do not)
//   crossOriginIsolated    (true only if COOP same-origin AND COEP require-corp)
//   new SharedArrayBuffer  (constructible only when isolated)
//
// WHAT IT DOES NOT PROVE
// ----------------------
// It does not exercise a real Godot 4, Unity or three.js export. No engine
// toolchain was available in the environment this was written in. It verifies the
// *precondition* those engines fail on, which is necessary but not sufficient.
// A real export can still fail for reasons this cannot see. Do not upgrade any
// status claim past "the isolation precondition is verified in Chromium" on the
// strength of this script alone.
//
// The negative control matters as much as the positive one: run 2 serves the same
// bundle with `--no-isolation` and requires `crossOriginIsolated === false`. Without
// it, a pass would be consistent with the browser isolating documents for some
// unrelated reason and the headers doing nothing.
//
// USAGE
//   node scripts/verify-web-bundle-isolation.mjs
//
// Requires: a cargo toolchain, and Playwright's Chromium
// (`npx playwright install chromium`). Exits non-zero on any failed check.

import { spawn } from 'node:child_process';
import { mkdtemp, writeFile, mkdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createRequire } from 'node:module'

// playwright is a devDependency of web/, but this script lives at the repo
// root — and Node resolves bare specifiers by walking up from the *importing
// file*, so scripts/ sees scripts/node_modules and <root>/node_modules and
// never web/node_modules. `import { chromium } from 'playwright'` therefore
// only worked while a stray <root>/node_modules happened to exist, and dies
// with ERR_MODULE_NOT_FOUND once it does not. Resolve it from where it is
// actually declared.
const { chromium } = createRequire(
  path.join(path.dirname(fileURLToPath(import.meta.url)), '..', 'web', 'package.json'),
)('playwright')


const REPO = join(dirname(fileURLToPath(import.meta.url)), '..');
const CRATE = join(REPO, 'magnetite-web-host');

// A fixture in Godot 4 web-export layout. Deliberately the same shape as
// `magnetite-web-host/tests/common/mod.rs`; the `.wasm` is a magic number, not an
// engine.
const FIXTURE = {
  'index.html': `<!doctype html>
<html lang="en">
<head><meta charset="utf-8"><title>magnetite web-bundle fixture</title></head>
<body>
<canvas id="canvas"></canvas>
<pre id="probe">pending</pre>
<script src="index.js"></script>
</body>
</html>
`,
  // Reports exactly the facts a Godot 4 export depends on. Also fetches a
  // subresource, because under COEP a same-origin subresource must still load —
  // if the server got CORP wrong, this is where it shows.
  'index.js': `(function () {
  var state = {
    secureContext: (typeof isSecureContext !== 'undefined') && isSecureContext === true,
    crossOriginIsolated: (typeof crossOriginIsolated !== 'undefined') && crossOriginIsolated === true,
    sharedArrayBufferDefined: (typeof SharedArrayBuffer !== 'undefined'),
    sabConstructs: false,
    pckFetched: null,
    pckStatus: null,
    wasmContentType: null,
    wasmContentEncoding: null,
  };
  try { new SharedArrayBuffer(8); state.sabConstructs = true; } catch (e) { state.sabConstructs = false; }

  function done() {
    window.__magnetiteProbe = state;
    document.getElementById('probe').textContent = JSON.stringify(state);
  }

  // Same-origin subresource under COEP, plus a range request on the pack.
  fetch('index.pck', { headers: { Range: 'bytes=0-3' } })
    .then(function (r) {
      state.pckStatus = r.status;
      return r.arrayBuffer();
    })
    .then(function (b) {
      state.pckFetched = new TextDecoder().decode(b);
      return fetch('index.wasm', { method: 'HEAD' });
    })
    .then(function (r) {
      state.wasmContentType = r.headers.get('content-type');
      state.wasmContentEncoding = r.headers.get('content-encoding');
      done();
    })
    .catch(function (e) {
      state.pckFetched = 'ERROR: ' + e;
      done();
    });
})();
`,
  // wasm magic + version, then filler.
  'index.wasm': Buffer.concat([
    Buffer.from([0x00, 0x61, 0x73, 0x6d, 0x01, 0x00, 0x00, 0x00]),
    Buffer.alloc(512, 7),
  ]),
  // GDPC magic then filler — 'GDPC' is what the range request above reads back.
  'index.pck': Buffer.concat([Buffer.from('GDPC'), Buffer.alloc(4092, 3)]),
  'index.worker.js': '// threads worker\nself.onmessage = function () {};\n',
  'index.audio.worklet.js': '// AudioWorklet processor\n',
};

async function makeFixture() {
  const dir = await mkdtemp(join(tmpdir(), 'magnetite-bundle-'));
  await mkdir(dir, { recursive: true });
  for (const [name, body] of Object.entries(FIXTURE)) {
    await writeFile(join(dir, name), body);
  }
  return dir;
}

/** Start `serve-web-bundle` and resolve once it prints its listening URL. */
function startServer(dir, extraArgs = []) {
  return new Promise((resolve, reject) => {
    const args = ['run', '-q', '--bin', 'serve-web-bundle', '--', dir, '--addr', '127.0.0.1:0', ...extraArgs];
    const proc = spawn('cargo', args, { cwd: CRATE, stdio: ['ignore', 'pipe', 'pipe'] });
    let out = '';
    let settled = false;
    const timer = setTimeout(() => {
      if (!settled) { settled = true; proc.kill('SIGKILL'); reject(new Error('server did not start in 180s:\n' + out)); }
    }, 180_000);

    proc.stdout.on('data', (d) => {
      out += d.toString();
      const m = out.match(/listening\s+(http:\/\/\S+)/);
      if (m && !settled) {
        settled = true;
        clearTimeout(timer);
        const cookie = out.match(/Cookie:\s*mag_receipt=([0-9a-f]+)/);
        resolve({ proc, url: m[1], receipt: cookie ? cookie[1] : null, log: out });
      }
    });
    proc.stderr.on('data', (d) => { out += d.toString(); });
    proc.on('exit', (code) => {
      if (!settled) { settled = true; clearTimeout(timer); reject(new Error(`server exited ${code}:\n` + out)); }
    });
  });
}

/** Load `url` and return the page's probe object. */
async function probe(browser, url, cookie) {
  const context = await browser.newContext();
  if (cookie) {
    const u = new URL(url);
    await context.addCookies([{
      name: 'mag_receipt', value: cookie,
      domain: u.hostname, path: u.pathname, httpOnly: false, secure: false,
    }]);
  }
  const page = await context.newPage();
  const consoleErrors = [];
  page.on('console', (m) => { if (m.type() === 'error') consoleErrors.push(m.text()); });
  const response = await page.goto(url, { waitUntil: 'load' });
  await page.waitForFunction('window.__magnetiteProbe !== undefined', { timeout: 15_000 });
  const state = await page.evaluate(() => window.__magnetiteProbe);
  const headers = response.headers();
  await context.close();
  return { state, headers, consoleErrors, status: response.status() };
}

let failures = 0;
function check(label, ok, detail = '') {
  console.log(`${ok ? '  PASS' : '  FAIL'}  ${label}${detail ? `  ${detail}` : ''}`);
  if (!ok) failures += 1;
}

async function main() {
  const dir = await makeFixture();
  const browser = await chromium.launch();
  const cleanup = [];
  try {
    // --- Run 1: isolation ON, free bundle -----------------------------------
    console.log('\n[1] isolation ENABLED, free bundle');
    const s1 = await startServer(dir);
    cleanup.push(s1.proc);
    console.log(`      ${s1.url}`);
    const r1 = await probe(browser, s1.url);
    console.log(`      probe ${JSON.stringify(r1.state)}`);

    check('document COOP is same-origin', r1.headers['cross-origin-opener-policy'] === 'same-origin', r1.headers['cross-origin-opener-policy'] ?? '(absent)');
    check('document COEP is require-corp', r1.headers['cross-origin-embedder-policy'] === 'require-corp', r1.headers['cross-origin-embedder-policy'] ?? '(absent)');
    check('browser reports a secure context', r1.state.secureContext === true);
    // The two that actually decide whether Godot 4 boots.
    check('browser reports crossOriginIsolated === true', r1.state.crossOriginIsolated === true);
    check('SharedArrayBuffer is defined', r1.state.sharedArrayBufferDefined === true);
    check('new SharedArrayBuffer(8) succeeds', r1.state.sabConstructs === true);
    // Same-origin subresources must still load under COEP.
    check('same-origin subresource loaded under COEP (range request)', r1.state.pckStatus === 206, `status ${r1.state.pckStatus}`);
    check('range request returned the right bytes', r1.state.pckFetched === 'GDPC', JSON.stringify(r1.state.pckFetched));
    check('wasm Content-Type is application/wasm', r1.state.wasmContentType === 'application/wasm', String(r1.state.wasmContentType));
    check('no console errors', r1.consoleErrors.length === 0, r1.consoleErrors.join(' | '));

    // --- Run 2: negative control -------------------------------------------
    // Without this, a pass above would not establish that the headers caused it.
    console.log('\n[2] isolation DISABLED (negative control — Godot 4 would NOT boot here)');
    const s2 = await startServer(dir, ['--no-isolation']);
    cleanup.push(s2.proc);
    const r2 = await probe(browser, s2.url);
    console.log(`      probe ${JSON.stringify(r2.state)}`);
    check('COOP absent', r2.headers['cross-origin-opener-policy'] === undefined);
    check('COEP absent', r2.headers['cross-origin-embedder-policy'] === undefined);
    check('crossOriginIsolated === false without the headers', r2.state.crossOriginIsolated === false);
    check('new SharedArrayBuffer(8) fails without the headers', r2.state.sabConstructs === false);

    // --- Run 3: paid bundle, receipt in a cookie ---------------------------
    // The claim under test is that the BROWSER attaches the cookie to
    // subresource requests it issues itself. A custom header cannot do that, and
    // a paid bundle gated on a header alone would load its document and then
    // 402 on every asset.
    console.log('\n[3] paid bundle — receipt cookie must cover browser-issued subresources');
    const s3 = await startServer(dir, ['--paid']);
    cleanup.push(s3.proc);
    check('server minted a mock receipt', typeof s3.receipt === 'string' && s3.receipt.length > 0);

    // Checked by status, not by "the probe never appeared" — a timeout would
    // also pass if the page were merely slow, which is not the claim.
    const bare = await browser.newContext();
    const barePage = await bare.newPage();
    const bareResp = await barePage.goto(s3.url, { waitUntil: 'commit' });
    check('without a receipt the document is refused with 402', bareResp.status() === 402, `status ${bareResp.status()}`);
    await bare.close();

    const r3 = await probe(browser, s3.url, s3.receipt);
    console.log(`      probe ${JSON.stringify(r3.state)}`);
    check('with the receipt cookie the document loads', r3.status === 200, `status ${r3.status}`);
    check('the cookie also unlocked the browser-issued subresource', r3.state.pckStatus === 206, `status ${r3.state.pckStatus}`);
    check('paid response is not publicly cacheable', (r3.headers['cache-control'] ?? '').includes('private'), r3.headers['cache-control'] ?? '(absent)');
  } finally {
    await browser.close();
    for (const p of cleanup) p.kill('SIGKILL');
    await rm(dir, { recursive: true, force: true });
  }

  console.log(
    failures === 0
      ? '\nAll checks passed. NOTE: this verifies the SharedArrayBuffer precondition in Chromium. It does NOT exercise a real Godot 4 / Unity / three.js export.'
      : `\n${failures} check(s) FAILED.`
  );
  process.exit(failures === 0 ? 0 : 1);
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
