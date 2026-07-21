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
  { name: 'home',           path: '/home',      label: 'Marketing surface — /home' },
  { name: 'server-browser', path: '/servers',   label: 'Dense data — discovery' },
  { name: 'game-detail',    path: '/game/1',    label: 'Game-centric' },
  { name: 'login',          path: '/login',     label: 'Form / auth' },
  { name: 'marketplace',    path: '/',          label: 'Editorial — catalogue' },
  { name: 'game-lobby',     path: '/lobby/1',   label: 'Pre-game surface', auth: true },
  { name: 'matchmaking',    path: '/matchmaking', label: 'Pre-game surface — queue' },
  { name: 'game-analytics', path: '/developers/analytics/1', label: 'Dataviz — developer analytics', auth: true },

  /* Unavailable / honest-failure states. */
  { name: 'points-rewards',   path: '/points',                  label: 'Unavailable — rewards catalogue', auth: true, click: '#points-tab-rewards' },
  { name: 'privacy-settings', path: '/settings/privacy',        label: 'Unavailable — export + delete account', auth: true },
  { name: 'friends',          path: '/friends',                 label: 'Unavailable — game invites', auth: true },
  { name: 'dev-marketplace',  path: '/developers/marketplace',  label: 'Unavailable — store/item deletion', auth: true },
  { name: 'game-deploy',      path: '/developers/deploy',       label: 'Empty — no versions registered', auth: true },
  { name: 'game-deploy-webhook', path: '/developers/deploy',    label: 'Unavailable — webhook secret generation', auth: true, click: 'button:has-text("Webhook Config")' },
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
  /* A small, deterministic catalogue so the marketplace shows its grid rather
     than the empty state. Fixture data (not live), and it honours GameCard's
     own honesty rules: no thumbnails (the branded fallback tile renders — never
     a stock photo), one game left unrated (rating null, not zero), and free
     games carry no price rather than "0 USDC". */
  '/api/v1/games':                   { games: [
    /* players_online > 100 earns a "Popular" badge and is_new earns a "New"
       one; a game is realistically never both at once (a brand-new game has
       not drawn a crowd yet), which also keeps the two badges from stacking. */
    { id: 'g1', title: 'Voxel Frontier',  developer: 'Redshift Labs',   category: 'action',   is_free: true,  fee_per_session: 0,     players_online: 1240, rating: 4.6, is_new: false },
    { id: 'g2', title: 'Nebula Drift',    developer: 'Orbital Studio',  category: 'racing',   is_free: false, fee_per_session: 0.05,  players_online: 830,  rating: 4.3, is_new: false },
    { id: 'g3', title: 'Rune & Ruin',     developer: 'Hollow Forge',    category: 'rpg',      is_free: false, fee_per_session: 0.10,  players_online: 2100, rating: 4.8, is_new: false },
    { id: 'g4', title: 'Grid Tactics',    developer: 'Iron Meridian',   category: 'strategy', is_free: true,  fee_per_session: 0,     players_online: 88,   rating: null, is_new: false },
    { id: 'g5', title: 'Pixel Panic',     developer: 'Arcade Kernel',   category: 'arcade',   is_free: true,  fee_per_session: 0,     players_online: 3400, rating: 4.1, is_new: false },
    { id: 'g6', title: 'Cipher Cascade',  developer: 'Latch & Key',     category: 'puzzle',   is_free: false, fee_per_session: 0.02,  players_online: 64,   rating: 4.5, is_new: true },
  ] },
  '/api/v1/matchmaking/status':      { status: 'not_in_queue' },
  /* Fixed (non-random) 14-day series so the analytics dataviz screenshot is
     reproducible — not live data, just a deterministic capture fixture. */
  '/api/v1/developer/games/1/analytics': {
    game_id: '1',
    game_title: 'Cosmic Raiders',
    summary: { total_revenue: 6420, active_players: 2310, total_sessions: 15840 },
    daily_revenue: [
      { date: '2026-06-18', revenue: 380 }, { date: '2026-06-19', revenue: 410 },
      { date: '2026-06-20', revenue: 395 }, { date: '2026-06-21', revenue: 460 },
      { date: '2026-06-22', revenue: 505 }, { date: '2026-06-23', revenue: 470 },
      { date: '2026-06-24', revenue: 430 }, { date: '2026-06-25', revenue: 445 },
      { date: '2026-06-26', revenue: 480 }, { date: '2026-06-27', revenue: 520 },
      { date: '2026-06-28', revenue: 495 }, { date: '2026-06-29', revenue: 455 },
      { date: '2026-06-30', revenue: 470 }, { date: '2026-07-01', revenue: 505 },
    ],
    daily_playtime: [
      { date: '2026-06-18', minutes: 9800 },  { date: '2026-06-19', minutes: 10200 },
      { date: '2026-06-20', minutes: 9950 },  { date: '2026-06-21', minutes: 11100 },
      { date: '2026-06-22', minutes: 11800 }, { date: '2026-06-23', minutes: 11400 },
      { date: '2026-06-24', minutes: 10600 }, { date: '2026-06-25', minutes: 10850 },
      { date: '2026-06-26', minutes: 11300 }, { date: '2026-06-27', minutes: 12100 },
      { date: '2026-06-28', minutes: 11700 }, { date: '2026-06-29', minutes: 11050 },
      { date: '2026-06-30', minutes: 11250 }, { date: '2026-07-01', minutes: 11900 },
    ],
  },
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
        /* Resolve the staggered `.reveal` entrance animations to their final
           state immediately. They run for up to ~900ms (600ms delay + duration)
           and the capture waited only 700ms, so hero content was photographed
           mid-animation — faded and mid-translate, which read as an overlapping,
           greyed-out header. The app's own `prefers-reduced-motion` rules snap
           `.reveal` to opacity:1 / transform:none, which is the settled look a
           real visitor sees a second after load. */
        reducedMotion: 'reduce',
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
