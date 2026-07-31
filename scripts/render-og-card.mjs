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
import { chromium } from 'playwright'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

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
