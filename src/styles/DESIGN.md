# Magnetite — Design System

**"Cold iron instrumentation."**

This is the rulebook for the app UI. Read it before styling a page. It exists so
that ~67 pages, redesigned by different people at different times, land as one
product.

The marketing site (`site/index.html`) shares this identity. App and site must
feel like one thing — but the app is a **dense tool**, not a landing page. Where
the site has air, the app has data.

---

## 1. The idea

Magnetite is a decentralized, self-hostable Rust game platform: deterministic
authoritative simulation, WASM sandbox, replay verification, bring-any-box
hosting, keypair identity, no cloud.

The UI should read as a **control surface for a verifiable machine** — mission
control, an oscilloscope, a lab notebook on cold metal. It should not read as a
games storefront. We are not competing with Steam on gloss; we are competing on
*legibility of exact facts*.

Consequences that actually change what you build:

- **The hash is the content.** Anything a user might copy, compare, or verify —
  an ID, a public key, a tick count, a duration, a checksum, an amount — is set
  in **mono**. This is the single most recognisable trait of the UI.
- **Density is a feature.** Prefer a table to a grid of cards. Prefer a hairline
  rule to a box. Prefer showing the value to hiding it behind a disclosure.
- **Say what is true.** Every surface is honest about what it knows (see §7).

---

## 2. Colour has meaning

Four semantic accents. They are **not** interchangeable decoration — each is a
claim about how much we can prove.

| Token | Colour | Means |
|---|---|---|
| `--field` | violet | The **verifiable core**: deterministic sim, replay-checked, signed, hash-addressed. "We can prove this." |
| `--boundary` | magenta | The **one honest boundary** where verification stops: untrusted input, third-party rails, unattested clients. |
| `--spec` | amber | Advisory / specification / needs attention. |
| `--live` | green | A running, healthy, currently-true thing. |

**Never use `--boundary` because it looks good.** It is the most striking colour
in the palette and it is reserved for telling the truth about a limit. If you
use magenta for a "featured" badge, you have broken the system.

Each accent has four forms: `--x` (the colour), `--x-text` (a contrast-safe
text variant for use *on a neutral background*), `--x-tint` (a background
wash), and `--on-x` (text to place *on top of* the filled colour).

**`--on-x` flips between themes and you must use it.** Dark-theme accents are
bright fills that need dark ink; light-theme accents are dark fills that need
white. Concretely, on `--field`: white text is 3.48:1 in dark (**fails**) but
6.49:1 in light. So `color: #fff` on an accent button is a real contrast bug in
one theme no matter which literal you pick. Never hardcode `#fff`/`#000` on a
filled accent — use `var(--on-field)`, `var(--on-danger)`, etc.

### Neutrals

`--void` (page) → `--sunk` (recessed) → `--paper` (card) → `--surface-2` →
`--elevated`. Ink runs `--ink` → `--ink-2` → `--ink-3`. Rules are `--line` and
`--line-2`, with `--rule` for the faintest in-table separator.

### Both themes are real

Themes are driven by `data-theme="dark|light"` on `<html>`, set by
`ThemeContext` and pre-applied by an inline script in `index.html` so there is
no flash.

**`src/styles/tokens.css` is the single authority for colour.** Never write
colour values as inline styles from JS — inline styles outrank stylesheet rules
and will silently defeat the entire system. (This bug existed here: the old
`themeConstants.js` inline-wrote a flat neutral ramp over the real palette, and
it is why the app looked generic. Do not reintroduce it.)

Light-theme accents are **darker** than their dark-theme counterparts, because
the dark violet/magenta do not reach 4.5:1 on white. That is a contrast
requirement, not a preference. If you add an accent, add both cuts.

**Rule: no hardcoded hex in any page or component stylesheet.** Only
`tokens.css` defines colour.

---

## 3. Type

Self-hosted woff2, no CDN. All faces **SIL Open Font License 1.1** — see
`public/fonts/OFL.txt`.

| Role | Face | Use for |
|---|---|---|
| Display | **Instrument Serif** (400) | The one page title. One editorial line. Nothing else. |
| Sans | **IBM Plex Sans** (var 100–700) | All UI prose, labels, buttons, body. |
| Mono | **IBM Plex Mono** (400/600) | Data, IDs, hashes, stats, micro-labels. |

Instrument Serif ships **400 only** — never set a heavier weight on it or the
browser synthesises a fake bold that looks broken. `.font-extrabold` and
`.font-black` are clamped to 700 for the same reason.

`h1`/`h2` are display serif. `h3`–`h6` are **sans** — a serif at small sizes in
a dense tool is noise.

**Micro-labels are the connective tissue.** Wide-tracked uppercase mono, via
`.m-xs` / `.m-sm` / `.m-md`. Use them for field labels, table headers, stat
captions and section eyebrows (`.kicker`). If a label looks like a label, it
should be one of these.

Scale is deliberately denser than the marketing site: base is 15px, and the
small end (11/12/13px) carries most of the UI.

---

## 4. Space, shape, motion

- **Spacing: a 4px grid.** `--space-1` (4px) … `--space-24` (96px). Legacy
  `--spacing-N` aliases exist for the migration tail; use `--space-N` in new
  code.
- **Radius is tight** — machined, not pillowy. `--radius-sm` (5px) for controls,
  `--radius-lg` (12px) for panels. Nothing exceeds 16px except pills
  (`--radius-full`).
- **Elevation**: prefer a hairline (`1px solid var(--line)`) over a shadow.
  Shadows (`--shadow-sm/md/lg/xl`) are for genuinely floating things — modals,
  dropdowns, toasts.
- **Motion is fast and scarce**: `--t-fast` 120ms, `--t` 200ms, `--t-slow`
  380ms, all on `--ease-out`. One orchestrated page-load stagger (`.reveal` +
  `.reveal-1..8`) beats scattered micro-animations.
- **`prefers-reduced-motion` is honoured at the token level** (durations
  collapse to zero) *and* explicitly for the reveal/grain/shimmer animations.
  If you add an animation, drive its duration from a token so it is covered.

---

## 5. The signature detail

The **field-line motif**: a 2px coloured rule on the leading edge of a block,
marking its epistemic status.

```html
<div class="edge-field">    <!-- provable -->
<div class="edge-boundary"> <!-- verification stops here -->
<div class="edge-spec">     <!-- advisory -->
<div class="edge-live">     <!-- currently running -->
```

This is the thing people should remember about the UI. It is only memorable if
it is *meaningful*, so apply it where it is literally true and nowhere else.

---

## 6. Components

Global primitives live in `src/index.css`. Compose these before writing new CSS.

- **Buttons** `.btn` + `.btn-primary` / `.btn-secondary` / `.btn-ghost` /
  `.btn-danger` / `.btn-boundary`; sizes `.btn-sm` / `.btn-lg` / `.btn-block`.
  Solid fills, never gradients — the violet→magenta sweep means "field →
  boundary" and a button is not a boundary. Min height 40px for touch.
- **Forms** `.form-group`, `.form-label`, `.form-input`, `.form-error`,
  `.form-hint`, `.form-row`, `.form-actions`. Add `.mono` to any input holding
  a key/hash/ID. `.form-row` collapses to one column below 640px.
- **Tables** wrap in `.table-wrap`, use `table.data`, with `td.lead` (primary
  cell), `td.num` (right-aligned tabular mono), `td.key` (identifier, mono
  violet). **The wrapper is what stops a wide table from scrolling the page.**
- **Surfaces** `.panel`, `.panel-sunk`, `.rule`.
- **Status** `.st` + `.st-live` / `.st-field` / `.st-boundary` / `.st-spec` /
  `.st-off`. Renders a dot **plus text** — never encode status by colour alone.
- **States** `.state` + `.state-empty` / `.state-error` / `.state-unavailable`,
  with `.state-title`, `.state-body`, `.state-actions`.
- **Loading** `.sk` skeletons (`.sk-text`, `.sk-title`, `.sk-row`), sized like
  the content they replace.
- **Atmosphere** `.bg-atmosphere` / `.bg-grain` / `.bg-grid`, `.glow-accent`.
  Near-invisible by design: texture on metal, never a pattern competing with
  data. Do not stack them.

---

## 7. Honesty rules (non-negotiable)

This codebase has a documented history of fabricated statistics — invented
player counts, a fake `aggregateRating` in a sibling repo, mock data leaking
into production paths. Treat this as a live hazard.

1. **Never invent data.** No placeholder player counts, ratings, revenue
   figures, download numbers, testimonials or "trending" stats. If the API does
   not supply it, do not display it.
2. **Distinguish the three failure states.** `.state-empty` = there is genuinely
   nothing yet. `.state-error` = the request failed. `.state-unavailable` = this
   node has no backend for this feature — nothing is broken, the capability is
   absent. Many pages in this app need the third one.
3. **Never silently substitute mock data for a failed request.** Mocks are
   gated on `import.meta.env.VITE_USE_MOCKS === 'true'` — the **strict**
   comparison. The bare-truthiness form (`import.meta.env.VITE_USE_MOCKS ?`) is
   a bug: the string `"false"` is truthy, so it *enables* mocks. This was fixed
   across 43 files; do not reintroduce it.
4. **Don't render a claim the backend cannot support.** The product is
   non-custodial: no balances we hold, no fiat, no payouts, no withdrawals. A UI
   that shows a platform-held balance is not a design bug, it is a false claim.

---

## 8. Accessibility (not optional)

- Contrast ≥ 4.5:1 for text in **both** themes. Use `--x-text` variants on
  tinted backgrounds.
- **Visible focus everywhere.** `tokens.css` sets a global
  `:focus-visible` outline. Never remove it without replacing it.
- Semantic markup: real `<button>`, `<a>`, `<table>` with `<th scope>`, `<nav>`,
  `<main>`, headings in order, no skipped levels.
- Icon-only controls need an accessible name.
- Form errors: `role="alert"` + `aria-describedby` + `aria-invalid`.
- Never encode meaning in colour alone — pair it with text or a shape.
- Keyboard paths for everything, including tabs, filters and galleries.
- A `.skip-link` should be the first tab stop.
- `*.a11y.test.jsx` files exist and must keep passing.

---

## 9. Responsive

Must hold at **390 / 768 / 1280px**, with **no horizontal page overflow**.

The usual culprits, and their fixes:
- Wide tables → `.table-wrap` (scrolls inside itself).
- Long hashes/pubkeys → `.break-key`, or put them in a `.table-wrap`.
- Two-column form rows → `.form-row` already collapses at 640px.
- Fixed-width grids → use `repeat(auto-fit, minmax(min(100%, 280px), 1fr))`;
  the `min(100%, …)` is what saves you at 390px.

---

## 10. How to apply this to a remaining page

1. **Check the page is honest first.** Consult the page census. If it renders
   data whose backend does not exist, fix the claim before the styling — add a
   `.state-unavailable`, don't redesign a lie.
2. Read this file and `tokens.css`.
3. Delete hardcoded colours from the page's CSS; replace with tokens.
4. Replace bespoke buttons/inputs/tables/cards with the §6 primitives. Most
   page CSS should *shrink*.
5. Set the page title in display serif (`h1` or `.display-hero`); make every
   label a `.m-sm`/`.m-md` micro-label; move all data to `--font-mono`.
6. Apply `.edge-*` only where the epistemic claim is true.
7. Add the three states (empty / error / unavailable) and a skeleton.
8. Walk the keyboard path. Check focus rings in both themes.
9. Check 390 / 768 / 1280 for overflow.
10. Run `npm run test:run` and `npm run test:a11y`.

**Exemplars to copy from:** `ServerBrowser` (dense data), `GameDetail`
(game-centric), `Login` + `auth.css` (form/auth), `Pricing` (editorial).
