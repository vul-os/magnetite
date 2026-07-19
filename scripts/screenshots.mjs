#!/usr/bin/env node
/**
 * Magnetite — Playwright screenshotter
 *
 * Captures the static marketing site (site/index.html) and docs viewer
 * (site/docs.html) at 1440×900 @2x (retina), in both light and dark, into
 * docs/screenshots/. This needs nothing but a browser: no Postgres, no
 * Redis, no wasm build, no running backend.
 *
 * Usage:
 *   npm install                          # installs the playwright devDep
 *   npx playwright install chromium      # one-time chromium download
 *   npm run screenshotter                # (alias: npm run screenshots)
 *
 * The static site is served from ./site via a tiny built-in Node http
 * server (no extra dependency) on an ephemeral local port.
 */

import { chromium } from 'playwright'
import { createServer } from 'node:http'
import { readFile, mkdir, writeFile, copyFile } from 'node:fs/promises'
import { existsSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'
import { spawn } from 'node:child_process'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const ROOT = path.resolve(__dirname, '..')
const SITE_DIR = path.join(ROOT, 'site')
const OUT = path.join(ROOT, 'docs', 'screenshots')
const SITE_SCREENSHOTS_DIR = path.join(SITE_DIR, 'screenshots')

const VIEWPORT = { width: 1440, height: 900 }

// ── Static routes to capture (site/index.html + site/docs.html#slug) ───────
const ROUTES = [
  { name: 'landing', path: '/index.html', description: 'Landing page — hero, features, seams, quick start' },
  { name: 'docs-overview', path: '/docs.html#overview', description: 'Docs — Overview', waitFor: '.markdown h1' },
  { name: 'architecture', path: '/docs.html#architecture', description: 'Docs — Architecture (node / discovery / payment / comms diagram)', waitFor: '.markdown .mermaid svg', settleMs: 1200, scrollToSelector: '.markdown .mermaid' },
  { name: 'hosting-a-server', path: '/docs.html#hosting-a-server', description: 'Docs — Hosting a server', waitFor: '.markdown h1' },
  { name: 'payments', path: '/docs.html#payments', description: 'Docs — Payments', waitFor: '.markdown h1' },
  { name: 'comms', path: '/docs.html#comms', description: 'Docs — Comms', waitFor: '.markdown h1' },
]

// ── Tiny static file server for ./site (no extra dependency) ───────────────

const MIME = {
  '.html': 'text/html; charset=utf-8',
  '.css': 'text/css; charset=utf-8',
  '.js': 'text/javascript; charset=utf-8',
  '.mjs': 'text/javascript; charset=utf-8',
  '.svg': 'image/svg+xml',
  '.png': 'image/png',
  '.md': 'text/markdown; charset=utf-8',
  '.txt': 'text/plain; charset=utf-8',
  '.json': 'application/json; charset=utf-8',
}

function startStaticServer(rootDir) {
  return new Promise((resolve, reject) => {
    const server = createServer(async (req, res) => {
      try {
        const urlPath = decodeURIComponent((req.url || '/').split('?')[0].split('#')[0])
        let filePath = path.join(rootDir, urlPath === '/' ? '/index.html' : urlPath)
        if (!filePath.startsWith(rootDir)) { res.writeHead(403); res.end(); return }
        const data = await readFile(filePath)
        const ext = path.extname(filePath)
        res.writeHead(200, { 'Content-Type': MIME[ext] || 'application/octet-stream' })
        res.end(data)
      } catch (err) {
        res.writeHead(404, { 'Content-Type': 'text/plain' })
        res.end('Not found: ' + err.message)
      }
    })
    server.listen(0, '127.0.0.1', () => {
      const { port } = server.address()
      resolve({ server, baseUrl: `http://127.0.0.1:${port}` })
    })
    server.on('error', reject)
  })
}

// ── Capture ──────────────────────────────────────────────────────────────

async function makeThemeContext(browser, theme) {
  const ctx = await browser.newContext({
    viewport: VIEWPORT,
    deviceScaleFactor: 2,
    colorScheme: theme,
  })
  // The site has no explicit light/dark toggle (it's dark-only by design, like
  // magnetite's graphite theme), but we still drive both Playwright colorScheme
  // contexts and a matching localStorage key in case a toggle is added later.
  await ctx.addInitScript((t) => {
    try { localStorage.setItem('magnetite.theme', t) } catch {}
  }, theme)
  return ctx
}

async function capture(page, baseUrl, route, theme) {
  const url = `${baseUrl}${route.path}`
  console.log(`  → [${theme}] ${route.description}`)
  try {
    await page.goto(url, { waitUntil: 'domcontentloaded', timeout: 20_000 })
    if (route.waitFor) {
      try {
        await page.waitForSelector(route.waitFor, { timeout: 10_000 })
      } catch {
        await page.waitForTimeout(2_000)
      }
    } else {
      try {
        await page.waitForLoadState('networkidle', { timeout: 8_000 })
      } catch {
        await page.waitForTimeout(1_500)
      }
    }
    if (route.scrollToSelector) {
      try {
        await page.locator(route.scrollToSelector).first().scrollIntoViewIfNeeded({ timeout: 3_000 })
        // Nudge back up a bit so the section heading is visible above the diagram.
        await page.evaluate(() => window.scrollBy(0, -120))
      } catch { /* best effort */ }
    }
    await page.waitForTimeout(route.settleMs || 600)
    const outPath = path.join(OUT, `${route.name}-${theme}.png`)
    await page.screenshot({ path: outPath, fullPage: false })
    console.log(`     saved ${path.relative(ROOT, outPath)}`)
    return { name: route.name, theme, status: 'ok' }
  } catch (err) {
    console.warn(`     FAILED: ${err.message}`)
    return { name: route.name, theme, status: 'failed', error: err.message }
  }
}

// ── Optional bonus: try the React app in src/ if it boots trivially ────────
//
// TODO(later wave): the React app under src/ is the pre-decentralization
// marketplace frontend (fiat wallet, communities, marketplace UI) and does
// not yet reflect the seams/architecture described in DECENTRALIZATION.md.
// Once the frontend is rebuilt against magnetite-seams, replace this
// best-effort probe with real route captures (landing, marketplace, game
// lobby, dev dashboard, etc.), following the ROUTES pattern above.
//
// This block is intentionally non-blocking: any failure is caught and
// logged, and the script's exit code is never affected by it. It only
// attempts a fast `vite` dev boot (no production build) with mock data
// (VITE_USE_MOCKS=true) and a short timeout.
async function tryCaptureReactApp(browser) {
  const results = []
  let viteProc = null
  const DEV_PORT = 5183 // avoid colliding with a real dev server on 5173
  const DEV_BASE = `http://127.0.0.1:${DEV_PORT}`

  try {
    if (!existsSync(path.join(ROOT, 'node_modules', '.bin', 'vite'))) {
      console.log('\n  [react-app] vite not installed — skipping (npm install first)')
      return results
    }

    console.log('\n  [react-app] attempting a best-effort vite dev boot (VITE_USE_MOCKS=true)…')
    viteProc = spawn(
      path.join(ROOT, 'node_modules', '.bin', 'vite'),
      ['--port', String(DEV_PORT), '--strictPort'],
      {
        cwd: ROOT,
        env: { ...process.env, VITE_USE_MOCKS: 'true' },
        stdio: ['ignore', 'pipe', 'pipe'],
      },
    )
    viteProc.stdout.on('data', () => {})
    viteProc.stderr.on('data', () => {})

    const deadline = Date.now() + 15_000
    let ready = false
    while (Date.now() < deadline) {
      try {
        const r = await fetch(DEV_BASE, { signal: AbortSignal.timeout(1_000) })
        if (r.ok) { ready = true; break }
      } catch { /* not yet */ }
      await new Promise((r) => setTimeout(r, 500))
    }

    if (!ready) {
      console.log('  [react-app] did not become ready within 15s — skipping (deferred to a later wave)')
      return results
    }

    const ctx = await browser.newContext({ viewport: VIEWPORT, deviceScaleFactor: 2 })
    const page = await ctx.newPage()
    page.on('pageerror', () => {})
    page.on('console', () => {})
    await page.goto(DEV_BASE, { waitUntil: 'domcontentloaded', timeout: 10_000 })
    await page.waitForTimeout(2_000)
    // If the app rendered something substantive (not a blank error boundary),
    // capture it as a bonus image. This is a soft heuristic, not a hard gate.
    const bodyLen = await page.evaluate(() => document.body.innerText.length).catch(() => 0)
    if (bodyLen > 40) {
      const outPath = path.join(OUT, 'app-landing.png')
      await page.screenshot({ path: outPath, fullPage: false })
      console.log(`  [react-app] saved bonus ${path.relative(ROOT, outPath)}`)
      results.push({ name: 'app-landing', theme: 'default', status: 'ok' })
    } else {
      console.log('  [react-app] rendered an empty page — likely needs a live backend; skipping')
    }
    await ctx.close()
  } catch (err) {
    console.log(`  [react-app] skipped (${err.message}) — deferred to a later wave, no backend/DB assumed`)
  } finally {
    if (viteProc) { try { viteProc.kill() } catch {} }
  }
  return results
}

// ── Main ─────────────────────────────────────────────────────────────────

async function main() {
  await mkdir(OUT, { recursive: true })
  await mkdir(SITE_SCREENSHOTS_DIR, { recursive: true })

  console.log('\nMagnetite screenshotter')
  console.log(`  site        : ${path.relative(ROOT, SITE_DIR)}/`)
  console.log(`  output      : ${path.relative(ROOT, OUT)}/`)
  console.log(`  viewport    : 1440×900 @2x (retina), light + dark`)

  const { server, baseUrl } = await startStaticServer(SITE_DIR)
  console.log(`  serving     : ${baseUrl}`)

  const browser = await chromium.launch({ headless: true })

  const results = []
  for (const theme of ['light', 'dark']) {
    const context = await makeThemeContext(browser, theme)
    const page = await context.newPage()
    page.on('console', () => {})
    page.on('pageerror', () => {})
    for (const route of ROUTES) {
      results.push(await capture(page, baseUrl, route, theme))
    }
    await context.close()
  }

  const bonus = await tryCaptureReactApp(browser)
  results.push(...bonus)

  await browser.close()
  server.close()

  // Mirror every generated PNG into site/screenshots/ so the static site
  // (hero-visual + docs.html markdown image rewriting) can reference images
  // without reaching outside site/.
  const ok = results.filter((r) => r.status === 'ok')
  const failed = results.filter((r) => r.status === 'failed')
  for (const r of ok) {
    const fname = `${r.name}-${r.theme === 'default' ? '' : r.theme}`.replace(/-$/, '') + '.png'
    const src = path.join(OUT, fname)
    if (existsSync(src)) {
      await copyFile(src, path.join(SITE_SCREENSHOTS_DIR, fname))
    }
  }

  console.log(`\nDone — ${ok.length} captured, ${failed.length} failed`)
  if (failed.length > 0) {
    console.log('\nFailed routes:')
    for (const r of failed) console.log(`  ${r.name}-${r.theme}: ${r.error}`)
  }

  const galleryRows = results
    .filter((r) => r.name !== 'app-landing')
    .map((r) => {
      const route = ROUTES.find((rt) => rt.name === r.name)
      return `| screenshots/${r.name}-${r.theme}.png | ${route?.description ?? r.name} | ${r.status === 'ok' ? 'populated' : 'needs regeneration'} |`
    })

  const notes = [
    '# docs/screenshots',
    '',
    'Generated by `npm run screenshotter` (alias `npm run screenshots`; script:',
    '`scripts/screenshots.mjs`). Captures the static marketing site',
    '(`site/index.html`) and docs viewer (`site/docs.html`) in **light and',
    'dark** at retina (1440×900 @2x) — no backend, database, or wasm build',
    'required.',
    '',
    '| File | Surface | Status |',
    '|------|---------|--------|',
    ...galleryRows,
    '',
    ok.some((r) => r.name === 'app-landing')
      ? '| screenshots/app-landing.png | React app (`src/`) — best-effort bonus capture | populated |'
      : '| _app-landing.png_ | React app (`src/`) — deferred | not captured this wave (needs a live backend; see TODO in `scripts/screenshots.mjs`) |',
    '',
    'To regenerate: `npm run screenshotter`',
  ].join('\n')

  await writeFile(path.join(OUT, 'README.md'), notes + '\n')
  console.log('  wrote docs/screenshots/README.md\n')

  if (failed.length > 0) process.exit(1)
}

main().catch((err) => {
  console.error('Fatal:', err)
  process.exit(1)
})
