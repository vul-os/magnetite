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
    // Force-align a selector to the top of the viewport. Unlike
    // scrollToSelector (which uses scrollIntoViewIfNeeded and therefore
    // no-ops when the element already sits just inside the fold), this always
    // scrolls — needed when the interesting content starts right at the
    // bottom edge and would otherwise be cropped.
    if (route.alignTopSelector) {
      try {
        await page.evaluate(({ sel, offset }) => {
          const el = document.querySelector(sel)
          if (!el) return
          const top = el.getBoundingClientRect().top + window.scrollY - (offset || 0)
          window.scrollTo({ top: Math.max(0, top), behavior: 'instant' })
        }, { sel: route.alignTopSelector, offset: route.alignTopOffset ?? 0 })
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

// ── React app routes (src/) — booted with mock data, no backend ───────────
//
// The app boots deterministically under VITE_USE_MOCKS=true: every hook has a
// mock branch (see src/hooks/*.js), so there is no Postgres, no Redis, no wasm
// build and no running backend involved. Auth-gated chrome is satisfied by
// seeding localStorage before the first paint.
//
// NOTE: vite must be started with an explicit `--host 127.0.0.1`. Without it
// vite binds to `::1`/localhost only, the readiness probe below (which polls
// 127.0.0.1) never succeeds, and the whole app capture silently skips. That
// was the original "did not become ready within 15s" bug.
const APP_ROUTES = [
  // Scroll past the hero so the discovered-session table — the actual point of
  // the page — is in frame rather than cut off below the fold.
  { name: 'app-servers', path: '/servers', description: 'App — Server browser (discovered SessionAds, bring-your-own-server)', waitFor: '.session-table, .browser-empty', alignTopSelector: '.browser-filters', alignTopOffset: 150 },
  { name: 'app-wallet', path: '/wallet', description: 'App — Non-custodial wallet (linked address + signed receipts)', waitFor: '.receipts-card' },
  { name: 'app-marketplace', path: '/marketplace', description: 'App — Game catalog (content-addressed games)', waitFor: '.game-grid, .empty-state' },
  { name: 'app-game', path: '/game/1', description: 'App — Game detail', waitFor: 'h1' },
  { name: 'app-developers', path: '/developers', description: 'App — Developer dashboard', waitFor: 'h1' },
  { name: 'app-earnings', path: '/developers/earnings', description: 'App — Developer revenue (receipt-backed)', waitFor: 'h1' },
]

const DEV_PORT = 5183 // avoid colliding with a real dev server on 5174

function startViteDevServer() {
  return new Promise((resolve, reject) => {
    const proc = spawn(
      path.join(ROOT, 'node_modules', '.bin', 'vite'),
      ['--port', String(DEV_PORT), '--strictPort', '--host', '127.0.0.1'],
      {
        cwd: ROOT,
        env: { ...process.env, VITE_USE_MOCKS: 'true' },
        stdio: ['ignore', 'pipe', 'pipe'],
      },
    )
    let stderr = ''
    proc.stdout.on('data', () => {})
    proc.stderr.on('data', (d) => { stderr += String(d) })
    proc.on('error', reject)
    proc.on('exit', (code) => {
      if (code !== 0 && code !== null) reject(new Error(`vite exited ${code}: ${stderr.slice(0, 400)}`))
    })
    resolve(proc)
  })
}

async function waitForDevServer(baseUrl, timeoutMs = 45_000) {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    try {
      const r = await fetch(baseUrl, { signal: AbortSignal.timeout(2_000) })
      if (r.ok) return true
    } catch { /* not up yet */ }
    await new Promise((r) => setTimeout(r, 400))
  }
  return false
}

async function makeAppContext(browser, theme) {
  const ctx = await browser.newContext({
    viewport: VIEWPORT,
    deviceScaleFactor: 2,
    colorScheme: theme,
  })
  // Seed before first paint: the theme the app reads (ThemeContext uses the
  // `theme` key), plus a signed-in user so authenticated chrome renders. The
  // token is inert — VITE_USE_MOCKS means no request is ever made with it.
  await ctx.addInitScript((t) => {
    try {
      localStorage.setItem('theme', t)
      localStorage.setItem('token', 'mock.screenshot.token')
      localStorage.setItem('user', JSON.stringify({
        id: '00000000-0000-4000-8000-000000000001',
        username: 'operator',
        email: 'operator@node.local',
        role: 'developer',
        subscription: { tier: 'pro' },
      }))
      // Suppress one-time overlays that would otherwise cover the screenshot.
      // These key names must match the components exactly — see
      // AnnouncementContext.jsx, CookieConsent.jsx and Marketplace.jsx.
      localStorage.setItem('magnetite_marketplace_tour_done', 'true')
      localStorage.setItem('magnetite_announcement_dismissed', JSON.stringify(true))
      localStorage.setItem('cookie_consent', 'accepted')
    } catch { /* storage unavailable */ }
  }, theme)
  return ctx
}

async function captureReactApp(browser) {
  const results = []
  let viteProc = null
  const DEV_BASE = `http://127.0.0.1:${DEV_PORT}`

  if (!existsSync(path.join(ROOT, 'node_modules', '.bin', 'vite'))) {
    console.log('\n  [app] vite not installed — run `npm install` first')
    return APP_ROUTES.flatMap((r) => ['light', 'dark'].map((theme) => ({
      name: r.name, theme, status: 'failed', error: 'vite not installed',
    })))
  }

  try {
    console.log('\n  [app] booting vite dev server (VITE_USE_MOCKS=true, no backend)…')
    viteProc = await startViteDevServer()

    if (!await waitForDevServer(DEV_BASE)) {
      throw new Error(`dev server did not become ready at ${DEV_BASE}`)
    }
    console.log(`  [app] ready at ${DEV_BASE}`)

    for (const theme of ['light', 'dark']) {
      const ctx = await makeAppContext(browser, theme)
      const page = await ctx.newPage()
      page.on('console', () => {})
      page.on('pageerror', () => {})
      for (const route of APP_ROUTES) {
        results.push(await capture(page, DEV_BASE, route, theme))
      }
      await ctx.close()
    }
  } catch (err) {
    console.warn(`  [app] FAILED: ${err.message}`)
    for (const r of APP_ROUTES) {
      for (const theme of ['light', 'dark']) {
        if (!results.some((x) => x.name === r.name && x.theme === theme)) {
          results.push({ name: r.name, theme, status: 'failed', error: err.message })
        }
      }
    }
  } finally {
    if (viteProc) { try { viteProc.kill('SIGTERM') } catch { /* already gone */ } }
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

  results.push(...await captureReactApp(browser))

  await browser.close()
  server.close()

  // Mirror every generated PNG into site/screenshots/ so the static site
  // (hero-visual + docs.html markdown image rewriting) can reference images
  // without reaching outside site/.
  const ok = results.filter((r) => r.status === 'ok')
  const failed = results.filter((r) => r.status === 'failed')
  for (const r of ok) {
    const fname = `${r.name}-${r.theme}.png`
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

  const allRoutes = [...ROUTES, ...APP_ROUTES]
  const rowsFor = (routes) =>
    results
      .filter((r) => routes.some((rt) => rt.name === r.name))
      .map((r) => {
        const route = allRoutes.find((rt) => rt.name === r.name)
        return `| screenshots/${r.name}-${r.theme}.png | ${route?.description ?? r.name} | ${r.status === 'ok' ? 'populated' : 'needs regeneration'} |`
      })

  const notes = [
    '# docs/screenshots',
    '',
    'Generated by `npm run screenshotter` (alias `npm run screenshots`; script:',
    '`scripts/screenshots.mjs`). Everything is captured in **light and dark** at',
    'retina (1440×900 @2x) with **no backend, database, or wasm build** required.',
    '',
    'Two surfaces are captured:',
    '',
    '1. The static marketing site (`site/index.html`) and docs viewer',
    '   (`site/docs.html`), served from a tiny built-in Node http server.',
    '2. The React app (`src/`), booted on a throwaway `vite` dev server with',
    '   `VITE_USE_MOCKS=true` so every hook serves deterministic mock data.',
    '',
    '## Static site',
    '',
    '| File | Surface | Status |',
    '|------|---------|--------|',
    ...rowsFor(ROUTES),
    '',
    '## App',
    '',
    '| File | Surface | Status |',
    '|------|---------|--------|',
    ...rowsFor(APP_ROUTES),
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
