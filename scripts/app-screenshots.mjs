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
  /* Single game for the game-detail route (/game/1 → api.games.get('1') →
     /api/games/1). Without it the page renders its "Could not load this game"
     error state. Honest fixture: no stock-photo screenshots (empty gallery,
     never invented art), empty reviews + leaderboard (no fabricated social
     proof), and the determinism proof-state shown as verified — what a real
     replay-verified, signed, playable build looks like, which is the platform's
     whole value proposition. */
  '/api/v1/games/1': {
    id: '1',
    title: 'Voxel Frontier',
    developer: 'Redshift Labs',
    developer_id: 'dev-redshift-labs',
    category: 'action',
    status: 'published',
    description:
      'A deterministic voxel arena — build, raid, and outlast, where every match is reproducible from its input log. Clients send inputs, never state; the simulation is server-authoritative Rust, browser-native via WASM, and identical for everyone in the room.',
    is_free: true,
    fee_per_session: 0,
    rating: 4.6,
    players_min: 1,
    players_max: 16,
    content_rating: 'everyone',
    screenshots: [],
    achievements: [
      { id: 'a1', name: 'First Landing', description: 'Win your first match.' },
      { id: 'a2', name: 'Architect', description: 'Place 1,000 blocks in a single match.' },
      { id: 'a3', name: 'Last Standing', description: 'Take a 16-player free-for-all.' },
    ],
    sessions: [],
    system_requirements: {
      Browser: 'Any WebAssembly + WebGL2 browser',
      Memory: '2 GB RAM',
      Network: 'Broadband for multiplayer; solo play needs no server',
    },
    similar: [],
    live_version: '1.4.2',
    content_hash: '9f2c4a1e8b3d6f70a3f19c4e8b2d5a1c7e0f3b6d9a2c5e8f1b4d7a0c3e6f9b2d5',
    artifact_type: 'wasm32-wasip1',
    github: 'redshift-labs/voxel-frontier',
    created_at: '2026-05-12T00:00:00Z',
    tick_rate: 30,
    replay_verified: true,
    signature_valid: true,
    has_playable_artifact: true,
  },
  '/api/v1/games/1/leaderboard': { entries: [] },
  '/api/v1/games/1/reviews': { reviews: [] },
  /* Discovery session ads for the server-browser (/servers →
     /api/v1/discovery/sessions). Without it the page renders "No tracker
     answered". This mirrors the hook's own canonical MOCK_SESSIONS
     (src/hooks/useDiscovery.js) exactly, including its deliberately honest
     degraded rows — a game this tracker never indexed (null title), and a node
     that declared no operator/region/occupancy — so the provenance framing
     (signed-and-checkable vs self-declared, "null rather than invented") is
     demonstrated in the shot, not hidden behind tidy data. */
  '/api/v1/discovery/sessions': { sessions: [
    { id: '3f2a1b6c-0d4e-4a71-9c83-1e5f7a2b9d04', game: '7f41c0a8e35d92b6104fa7cd8e2059b3746ac1de92f80b5537ea16c4d0938ab1', game_title: 'Cosmic Raiders', game_version: '1.4.2', node: 'nord-fjord-01.operator.net:7777', operator: 'nordfjord', region: 'eu-north', capacity: { cpu_cores: 32, ram_mb: 131072, bandwidth_mbps: 2000, free_slots: 46, max_shards: 24 }, ping_hint: 18, price: { amount: 20, currency: 'USDC', unit: 'per_hour' }, chat_room: 'builtin://room/cosmic-nord-01', voice_room: 'builtin://voice/cosmic-nord-01', node_key: 'a41f6b02c7d95e83104ab7cf2e6d0951b83c4a7e60d29f15caa3b78e4025d6f9', players: 82, max_players: 128, expires_at: 1800000120 },
    { id: '9b7c4d18-5e2f-4c0a-8d61-7b3e9f5a1c26', game: '7f41c0a8e35d92b6104fa7cd8e2059b3746ac1de92f80b5537ea16c4d0938ab1', game_title: 'Cosmic Raiders', game_version: '1.4.2', node: 'home-rack.pareto.dev:7777', operator: 'pareto', region: 'self-hosted', capacity: { cpu_cores: 8, ram_mb: 32768, bandwidth_mbps: 500, free_slots: 11, max_shards: 4 }, ping_hint: 7, price: null, chat_room: 'builtin://room/pareto-lan', voice_room: null, node_key: '5c93e07a1b4d6f28903ac5be7d14f062a97e3b58c026d4f19be7a350c8412e7d', players: 5, max_players: 16, expires_at: 1800000120 },
    { id: 'c15e8a90-2f76-4b3d-9e08-4a1c6d2b7f53', game: '2ad9e6104b73fc85a01d7e29c4b850fa3e6d1927cc4f0b83a71e5d6294fb03c8', game_title: 'Speed Legends', game_version: '2.0.0', node: 'sao-paulo-03.gridhost.io:7777', operator: 'gridhost', region: 'sa-east', capacity: { cpu_cores: 64, ram_mb: 262144, bandwidth_mbps: 5000, free_slots: 0, max_shards: 48 }, ping_hint: 141, price: { amount: 15, currency: 'USDC', unit: 'per_hour' }, chat_room: 'builtin://room/speed-sp-03', voice_room: 'builtin://voice/speed-sp-03', node_key: 'e820b47f5d1a396c02e7a8b34f95d106c73e2a9f480bd51e6ca29738f04b1d6a', players: 240, max_players: 240, expires_at: 1800000120 },
    { id: '7d3b0f24-8c19-4e5a-b076-2f8d1a4c9e35', game: '2ad9e6104b73fc85a01d7e29c4b850fa3e6d1927cc4f0b83a71e5d6294fb03c8', game_title: 'Speed Legends', game_version: '2.0.0', node: 'lan.local:7777', operator: 'you', region: 'lan', capacity: { cpu_cores: 12, ram_mb: 65536, bandwidth_mbps: 1000, free_slots: 14, max_shards: 8 }, ping_hint: 1, price: null, chat_room: null, voice_room: null, node_key: '1a6fptr', players: 2, max_players: 16, expires_at: 1800000120 },
    { id: 'b4e19c67-3a58-4d20-91cf-6e2b8d0a5347', game: 'c184de0a7b29635f10e4a8cd7b3902fe58d64a1cbb730e95d2416af8073c5e29', game_title: null, game_version: null, node: 'frankfurt-11.metalcloud.eu:7777', operator: 'metalcloud', region: 'eu-central', capacity: { cpu_cores: 48, ram_mb: 196608, bandwidth_mbps: 4000, free_slots: 63, max_shards: 36 }, ping_hint: 34, price: { amount: 25, currency: 'USDC', unit: 'per_hour' }, chat_room: 'builtin://room/void-fra-11', voice_room: 'builtin://voice/void-fra-11', node_key: '6b02f9d4718ae35c0d94b7e2a681f350c47d9be208a1f6c35de907b41a2c8e6f', players: 129, max_players: 192, expires_at: 1800000120 },
    { id: 'e0a72d5b-9c34-4f18-86b2-3d7e1a9c4f60', game: 'c184de0a7b29635f10e4a8cd7b3902fe58d64a1cbb730e95d2416af8073c5e29', game_title: null, game_version: null, node: 'tokyo-02.sakuranode.jp:7777', operator: null, region: null, capacity: { cpu_cores: 24, ram_mb: 98304, bandwidth_mbps: 1500, free_slots: 22, max_shards: 16 }, ping_hint: 96, price: { amount: 12, currency: 'USDC', unit: 'per_hour' }, chat_room: null, voice_room: null, node_key: '90c47ea1b6f2d385047ceb91a3f68d20b5e719ca4038f6d2ba81c07e35f9d248', players: null, max_players: null, expires_at: 1800000120 },
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
