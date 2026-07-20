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

/* The exemplar pages redesigned in this pass. */
const ROUTES = [
  { name: 'server-browser', path: '/servers',   label: 'Dense data — discovery' },
  { name: 'game-detail',    path: '/game/1',    label: 'Game-centric' },
  { name: 'login',          path: '/login',     label: 'Form / auth' },
  { name: 'pricing',        path: '/pricing',   label: 'Editorial' },
]

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
      })
      // Set the theme the same way the app does, before first paint.
      await ctx.addInitScript((t) => {
        try { localStorage.setItem('theme', t) } catch (e) { /* storage disabled */ }
      }, theme)

      const page = await ctx.newPage()
      await page.goto(base + route.path, { waitUntil: 'networkidle' }).catch(() => {})
      await page.waitForTimeout(700)

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
