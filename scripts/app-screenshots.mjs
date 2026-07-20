#!/usr/bin/env node
/**
 * Magnetite — app (React) screenshotter + responsive overflow check.
 *
 * Distinct from scripts/screenshots.mjs, which captures the static marketing
 * site. This one drives the actual SPA with Playwright:
 *
 *   1. builds the app and serves the production bundle
 *   2. screenshots each exemplar route in BOTH themes at 1280px
 *   3. asserts no horizontal overflow at 390 / 768 / 1280
 *
 * Usage:  node scripts/app-screenshots.mjs
 * Output: docs/screenshots/app/
 *
 * Exits non-zero if any route overflows horizontally, so it can gate CI.
 */

import { chromium } from 'playwright'
import { createServer } from 'node:http'
import { readFile, mkdir } from 'node:fs/promises'
import { existsSync } from 'node:fs'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const __dirname = path.dirname(fileURLToPath(import.meta.url))
const ROOT = path.resolve(__dirname, '..')
const DIST = path.join(ROOT, 'dist')
const OUT = path.join(ROOT, 'docs', 'screenshots', 'app')

/* The exemplar pages redesigned in this pass, plus every surface that renders
 * an "unavailable" state. The unavailable routes need a signed-in user and a
 * backend that answers the way the real one does — see API_STUB below. */
const ROUTES = [
  { name: 'server-browser', path: '/servers',   label: 'Dense data — discovery' },
  { name: 'game-detail',    path: '/game/1',    label: 'Game-centric' },
  { name: 'login',          path: '/login',     label: 'Form / auth' },
  { name: 'pricing',        path: '/pricing',   label: 'Editorial' },

  /* Unavailable / honest-failure states. */
  { name: 'points-rewards',   path: '/points',                  label: 'Unavailable — rewards catalogue', auth: true, click: '#points-tab-rewards' },
  { name: 'privacy-settings', path: '/settings/privacy',        label: 'Unavailable — export + delete account', auth: true },
  { name: 'friends',          path: '/friends',                 label: 'Unavailable — game invites', auth: true },
  { name: 'dev-marketplace',  path: '/developers/marketplace',  label: 'Unavailable — store/item deletion', auth: true },
  { name: 'game-deploy',      path: '/developers/deploy',       label: 'Empty — no versions registered', auth: true },
  { name: 'game-deploy-webhook', path: '/developers/deploy',    label: 'Unavailable — webhook secret generation', auth: true, click: 'button:has-text("Webhook Config")' },
  { name: 'matchmaking',      path: '/matchmaking',             label: 'Nav — newly reachable route', auth: true },
]

/**
 * A stub of THIS node's real API surface: 200 for routes the Rust backend
 * actually mounts, 404 for the ones it does not. Screenshotting against a dead
 * origin would only ever show error states and would never exercise the
 * unavailable states, which are the point of this pass.
 */
const MOUNTED = {
  '/api/v1/auth/me':                 { id: 'u1', username: 'operator', email: 'op@example.com', role: 'admin' },
  '/api/v1/points/balance':          { points: 1240, lifetime_points: 8300, rank: 42, season: { name: 'Season 1', tier: 'Silver', next_tier: 'Gold', progress: 40, points_needed: 760, ends_at: '2026-09-01T00:00:00Z' } },
  '/api/v1/points/history':          { history: [] },
  '/api/v1/points/leaderboard':      { entries: [] },
  '/api/v1/friends':                 { friends: [] },
  '/api/v1/friends/pending':         { requests: [] },
  '/api/v1/friends/sent':            { requests: [] },
  '/api/v1/marketplace/my-stores':   { stores: [{ id: 's1', name: 'Cosmetics', game_id: 'g1', game_title: 'Test Game', item_count: 0, revenue_usdc: 0 }] },
  '/api/v1/marketplace/entitlements':{ entitlements: [] },
  '/api/v1/marketplace/stores/s1/items': { items: [] },
  '/api/v1/developer/games':         { games: [] },
  '/api/v1/github/installations':    { installations: [] },
  '/api/v1/games':                   { games: [] },
  '/api/v1/matchmaking/status':      { status: 'not_in_queue' },
}

const THEMES = ['dark', 'light']
const WIDTHS = [390, 768, 1280]

const MIME = {
  '.html': 'text/html', '.js': 'text/javascript', '.css': 'text/css',
  '.json': 'application/json', '.svg': 'image/svg+xml', '.png': 'image/png',
  '.woff2': 'font/woff2', '.webmanifest': 'application/manifest+json',
}

/** Static server with SPA fallback to index.html. */
function serve(dir) {
  return new Promise((resolve) => {
    const server = createServer(async (req, res) => {
      const url = decodeURIComponent(req.url.split('?')[0])
      let file = path.join(dir, url)
      if (!existsSync(file) || url === '/') file = path.join(dir, 'index.html')
      try {
        const stat = await readFile(file)
        res.writeHead(200, { 'Content-Type': MIME[path.extname(file)] || 'application/octet-stream' })
        res.end(stat)
      } catch {
        // SPA fallback — any unknown path renders the app shell.
        const html = await readFile(path.join(dir, 'index.html'))
        res.writeHead(200, { 'Content-Type': 'text/html' })
        res.end(html)
      }
    })
    server.listen(0, '127.0.0.1', () => resolve({ server, port: server.address().port }))
  })
}

const main = async () => {
  if (!existsSync(DIST)) {
    console.error('dist/ not found — run `npm run build` first.')
    process.exit(1)
  }
  await mkdir(OUT, { recursive: true })

  const { server, port } = await serve(DIST)
  const base = `http://127.0.0.1:${port}`
  const browser = await chromium.launch()

  const overflows = []

  for (const theme of THEMES) {
    for (const route of ROUTES) {
      const ctx = await browser.newContext({
        viewport: { width: 1280, height: 900 },
        deviceScaleFactor: 2,
        /* The PWA service worker proxies fetches and would bypass page.route(),
           turning every stubbed API call into a network failure. */
        serviceWorkers: 'block',
      })
      // Set the theme the same way the app does, before first paint.
      await ctx.addInitScript((t) => {
        try { localStorage.setItem('theme', t) } catch (e) { /* storage disabled */ }
      }, theme)

      if (route.auth) {
        await ctx.addInitScript(() => {
          try {
            localStorage.setItem('token', 'screenshot-token')
            localStorage.setItem('magnetite_user', JSON.stringify({
              id: 'u1', username: 'operator', email: 'op@example.com', role: 'admin',
            }))
          } catch (e) { /* storage disabled */ }
        })
      }

      const page = await ctx.newPage()

      /* Answer API calls the way the real node does. */
      await page.route('**/api/**', async (r) => {
        const url = new URL(r.request().url())
        const hit = MOUNTED[url.pathname]
        /* The app calls a different origin (VITE_API_URL), so the stub must
           send CORS headers or the browser rejects the response and every call
           looks like a network failure instead of a 404. */
        const cors = {
          'access-control-allow-origin': '*',
          'access-control-allow-headers': '*',
          'access-control-allow-methods': '*',
        }
        if (r.request().method() === 'OPTIONS') {
          await r.fulfill({ status: 204, headers: cors })
        } else if (hit) {
          await r.fulfill({ status: 200, contentType: 'application/json', headers: cors, body: JSON.stringify(hit) })
        } else {
          await r.fulfill({
            status: 404,
            contentType: 'application/json',
            headers: cors,
            body: JSON.stringify({ message: 'Not Found' }),
          })
        }
      })

      await page.goto(base + route.path, { waitUntil: 'networkidle' }).catch(() => {})
      await page.waitForTimeout(700)

      if (route.click) {
        await page.click(route.click).catch(() => {})
        await page.waitForTimeout(400)
      }

      const file = path.join(OUT, `${route.name}-${theme}.png`)
      await page.screenshot({ path: file, fullPage: true })
      console.log(`  ✓ ${route.name} (${theme})`)

      // Responsive overflow check at each breakpoint.
      for (const w of WIDTHS) {
        await page.setViewportSize({ width: w, height: 900 })
        await page.waitForTimeout(250)
        const res = await page.evaluate(() => ({
          scrollW: document.documentElement.scrollWidth,
          clientW: document.documentElement.clientWidth,
        }))
        // 1px of tolerance for sub-pixel rounding.
        if (res.scrollW > res.clientW + 1) {
          overflows.push(`${route.name} [${theme}] @${w}px: scrollWidth ${res.scrollW} > clientWidth ${res.clientW}`)
        }
      }

      await ctx.close()
    }
  }

  await browser.close()
  server.close()

  console.log('\n── Horizontal overflow ──')
  if (overflows.length === 0) {
    console.log('none at 390 / 768 / 1280 across all routes and both themes')
  } else {
    overflows.forEach((o) => console.log('  ✗ ' + o))
    process.exitCode = 1
  }
  console.log(`\nScreenshots: ${path.relative(ROOT, OUT)}`)
}

main().catch((e) => { console.error(e); process.exit(1) })
