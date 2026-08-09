#!/usr/bin/env node
/**
 * Renders scripts/og-card.html -> site/assets/og-card.png (1200x630, no
 * device scaling) via a headless Chromium screenshot.
 *
 * scripts/og-card.html is the single source of truth for the share card —
 * never hand-edit the PNG. Run this after changing that file:
 *
 *   node scripts/render-og-card.mjs
 */
import path from 'node:path'
import { fileURLToPath } from 'node:url'
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


const __dirname = path.dirname(fileURLToPath(import.meta.url))
const ROOT = path.resolve(__dirname, '..')
const SRC = path.join(__dirname, 'og-card.html')
const OUT = path.join(ROOT, 'site', 'assets', 'og-card.png')

const browser = await chromium.launch()
const page = await browser.newPage({ viewport: { width: 1200, height: 630 }, deviceScaleFactor: 1 })
await page.goto('file://' + SRC)
await page.waitForTimeout(200) // let @font-face swap in before the capture
await page.screenshot({ path: OUT })
await browser.close()
console.log('wrote', path.relative(ROOT, OUT))
