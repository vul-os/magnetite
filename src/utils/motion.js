/*
 * motion.js — the app's live-instrument motion controller.
 *
 * Three responsibilities, all strictly progressive-enhancement:
 *   1. Scroll reveal   — arm `.sr` elements and reveal them as they enter the
 *                        viewport. Only runs when motion is welcome; otherwise
 *                        every `.sr` stays visible (see tokens.css), so a
 *                        static or reduced-motion render shows the settled page.
 *   2. Cursor spotlight — a soft field-violet light that tracks the pointer
 *                        across any `.spot` surface (--spot-x / --spot-y).
 *   3. Field parallax  — `[data-aura]` heroes shift their glow toward the
 *                        pointer, the iron-filings-follow-a-magnet metaphor.
 *
 * Everything is guarded by prefers-reduced-motion and reacts to changes in it.
 * Nothing here is required for the page to function or be legible.
 */

const REDUCE = window.matchMedia('(prefers-reduced-motion: reduce)')

let observer = null
let armed = false

function reveal(el) {
  // rootMargin pulls the trigger slightly before the element is on screen so
  // it is already settling in as it scrolls up; unobserve so it never replays.
  el.classList.add('is-visible')
  if (observer) observer.unobserve(el)
}

function armScrollReveal() {
  if (armed) return
  armed = true
  document.documentElement.classList.add('js-motion')

  observer = new IntersectionObserver(
    (entries) => {
      for (const e of entries) if (e.isIntersecting) reveal(e.target)
    },
    { rootMargin: '0px 0px -8% 0px', threshold: 0.08 },
  )

  const scan = (root) => {
    const nodes = root.querySelectorAll ? root.querySelectorAll('.sr:not(.is-visible)') : []
    nodes.forEach((n) => {
      // Anything already in frame at mount (above the fold) reveals immediately
      // on the next frame rather than waiting for a scroll that may never come.
      observer.observe(n)
    })
  }
  scan(document)

  // SPA route changes inject new `.sr` nodes; pick them up as they arrive.
  const mo = new MutationObserver((mutations) => {
    for (const m of mutations) {
      m.addedNodes.forEach((node) => {
        if (node.nodeType !== 1) return
        if (node.matches && node.matches('.sr:not(.is-visible)')) observer.observe(node)
        scan(node)
      })
    }
  })
  mo.observe(document.body, { childList: true, subtree: true })
}

function disarmScrollReveal() {
  if (!armed) return
  armed = false
  document.documentElement.classList.remove('js-motion')
  if (observer) { observer.disconnect(); observer = null }
  // Leaving js-motion off makes every `.sr` visible again via CSS.
}

/* ── Cursor spotlight + hero parallax (delegated, rAF-throttled) ─────────── */
let pointerRAF = 0
let lastEvent = null

function onPointerMove(e) {
  lastEvent = e
  if (pointerRAF) return
  pointerRAF = requestAnimationFrame(() => {
    pointerRAF = 0
    const ev = lastEvent
    if (!ev) return

    // Spotlight: paint the local coordinates onto the nearest .spot surface.
    const spot = ev.target.closest && ev.target.closest('.spot')
    if (spot) {
      const r = spot.getBoundingClientRect()
      spot.style.setProperty('--spot-x', `${((ev.clientX - r.left) / r.width) * 100}%`)
      spot.style.setProperty('--spot-y', `${((ev.clientY - r.top) / r.height) * 100}%`)
    }

    // Parallax: nudge every [data-aura] field toward the pointer, gently.
    const auras = document.querySelectorAll('[data-aura]')
    if (auras.length) {
      const cx = window.innerWidth / 2
      const cy = window.innerHeight / 2
      const dx = ((ev.clientX - cx) / cx) * 22
      const dy = ((ev.clientY - cy) / cy) * 18
      auras.forEach((a) => {
        a.style.setProperty('--mx', `${dx}px`)
        a.style.setProperty('--my', `${dy}px`)
      })
    }
  })
}

function armPointer() { window.addEventListener('pointermove', onPointerMove, { passive: true }) }
function disarmPointer() { window.removeEventListener('pointermove', onPointerMove) }

function apply() {
  if (REDUCE.matches) {
    disarmScrollReveal()
    disarmPointer()
  } else {
    armScrollReveal()
    armPointer()
  }
}

export function initMotion() {
  if (typeof window === 'undefined' || !('IntersectionObserver' in window)) return
  apply()
  const onChange = () => apply()
  if (REDUCE.addEventListener) REDUCE.addEventListener('change', onChange)
  else if (REDUCE.addListener) REDUCE.addListener(onChange)
}
