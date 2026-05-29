# Magnetite — Autonomous Build Decisions & Design System

> Single source of truth for the autonomous multi-wave rebuild. Every agent reads this
> file before working. The orchestrator audits against it every 30 minutes.

Last updated: 2026-05-30 (Wave 0 / setup)

---

## 1. Product Vision (refined)

**Magnetite is the open-source platform for building, distributing, and monetizing
Rust games — that scale from a weekend game jam to a COD-size AAA title.**

- **Rust-first.** Game logic is authored in Rust. Clients compile Bevy → WASM (browser)
  and to native. Servers are server-authoritative Rust, sandboxed.
- **Scales with the game.** A tiny single-file arcade game and a large multiplayer title
  use the same SDK and platform; the platform provides the heavy lifting (hosting,
  matchmaking, real-time netcode, persistence, payments) so developers only write game logic.
- **Distribution built in.** A storefront/marketplace distributes games; players discover,
  play (in-browser via WASM or native), and pay.
- **Open source.** Platform (MIT), SDK (MIT), game template (MIT), docs (CC0).
- **Real money, no middlemen.** USDC payments (Circle), Paystack fiat on-ramp, playtime-based
  developer payouts, 15% platform fee.

The previous "HTML5 games" framing is **deprecated** — all copy/marketing pivots to the
Rust-games-at-any-scale narrative above.

---

## 2. Locked Decisions

| # | Decision | Choice | Rationale |
|---|----------|--------|-----------|
| D1 | Frontend visual direction | **Industrial Magnetite** | Dark, technical, developer-credible. Fits a serious Rust gaming-infra platform that scales small→AAA. |
| D2 | Frontend rebuild strategy | **Re-skin via new design tokens + restyle shared component lib + restyle pages**, keep routing/data architecture | 69 pages + 100 components already exist and build; tearing down architecture wastes effort. Make it *amazing* through a cohesive design system, motion, and polish. |
| D3 | Git | Working branch `feat/redesign-and-harden`; baseline commit; commit after each wave | Recoverable checkpoints during a long unattended run. |
| D4 | Backend stance | "Perfect" = zero warnings, tests pass, docs accurate, vision gaps filled | The backend compiles; harden it rather than rewrite. |
| D5 | sqlx | Upgrade `sqlx 0.7.4 → 0.8.x` to clear future-incompat | Removes the future-incompat rejection warning; small, contained change. |
| D6 | Mock data | Keep mock fallbacks but wire pages to real API where the endpoint exists; mocks become graceful fallback only | Many pages still import `src/data/mock*`. |
| D7 | Orchestration | Waves of up to 5 Sonnet agents via Workflow; 30-min audit loop for ~4 hours | Per user instruction. |

---

## 3. Design System — "Industrial Magnetite"

### Principles
1. **Grounded & precise.** Sharp 1px borders, tight grid, generous negative space. No rounded-blob playfulness.
2. **Magnetic motif.** Subtle field-line/ring/grain textures; restrained, never noisy.
3. **Developer-credible.** Monospace for labels, stats, code, IDs. Sans for prose.
4. **Motion with intent.** Entrance fades/slides, magnetic hover pulls, count-ups for stats. Respect `prefers-reduced-motion`.
5. **Dark-first**, with a real light theme. All colors are CSS variables — never hardcode.

### Color tokens (dark, `:root`)
```
--color-bg-primary:    #07070b;   /* near-black, slight blue */
--color-bg-secondary:  #0f0f16;
--color-bg-card:       #14141d;
--color-bg-elevated:   #1b1b27;
--color-text-primary:  #f4f4f6;
--color-text-secondary:#a8a8b3;
--color-text-muted:    #6b6b78;
--color-border:        #23232e;
--color-border-strong: #33333f;

/* Accent: electric cyan primary + magnetite amber secondary */
--color-accent:        #38e1c8;   /* electric teal/cyan — primary action */
--color-accent-hover:  #19c7ad;
--color-accent-soft:   rgba(56,225,200,0.12);
--color-amber:         #f5a524;   /* secondary / energy / earnings */
--color-amber-soft:    rgba(245,165,36,0.12);

--color-success:#3ddc84; --color-warning:#f5a524; --color-error:#ff5468; --color-info:#5b9dff;

--gradient-primary: linear-gradient(135deg,#38e1c8 0%,#5b9dff 100%);
--gradient-energy:  linear-gradient(135deg,#f5a524 0%,#ff5468 100%);
--gradient-hero:    radial-gradient(ellipse at 50% 0%, #16161f 0%, #07070b 60%);
```
Light theme: invert bg/text, keep accents, soften shadows (define under `[data-theme="light"]`).

### Type
- Sans: `Inter` (already used) for body/headings.
- Mono: `JetBrains Mono` / `ui-monospace` for labels, stats, code, kbd, IDs.
- Scale: 12 / 13 / 14 / 16 / 18 / 22 / 28 / 36 / 48 / 64. Headings tight tracking (-0.02em), mono labels wide tracking (0.08em) + uppercase.

### Shape & depth
- Radius: `--radius-sm:6px; --radius:10px; --radius-lg:16px`. Inputs/buttons 6–10px, cards 12–16px.
- Borders: 1px hairlines (`--color-border`); hover lifts to `--color-border-strong` + accent glow.
- Shadows: layered, low-opacity; accent glow `0 0 24px rgba(56,225,200,.18)` on primary/focus.

### Motion
- Durations: `--t-fast:140ms; --t:240ms; --t-slow:420ms`, ease `cubic-bezier(.2,.8,.2,1)`.
- Patterns: section fade-in-up on scroll; card magnetic hover (translateY -2px + glow); stat count-up; skeleton shimmer. Honor reduced-motion.

### Signature elements (use sparingly)
- Magnetic ring/field-line hero backdrop (exists in HeroSection — refine, don't duplicate).
- Faint grid + grain overlay on hero/section backgrounds.
- Mono "kicker" labels above headings (e.g. `// BUILT IN RUST`).

### Accessibility
- WCAG AA contrast. Visible focus rings (accent). Keyboard nav intact. `prefers-reduced-motion` honored. Don't regress existing a11y providers/skip-link.

---

## 4. Work Plan (waves)

- **Wave 1 — Foundation & docs (parallel):**
  (a) Design tokens: rewrite `src/index.css` + add `src/styles/tokens.css` per §3.
  (b) Docs: rewrite `README.md`, `roadmap.md`, `TASKS.md` to reflect reality + vision §1.
  (c) Backend hygiene: clear warnings, `cargo fix`, sqlx upgrade, ensure tests pass.
  (d) Restyle shared component library (`src/components/common/*`) to new tokens.
- **Wave 2 — Frontend pages (parallel, batched by area):** Landing, Marketplace/GameDetail,
  Auth, Wallet/Subscription, Developer portal, Profile/Social/Leaderboard, Admin, Legal/Misc.
- **Wave 3 — Wiring & gaps:** mock→real API; backend vision gaps (game distribution/WASM
  hosting endpoints, SDK polish, game-template). 
- **Wave 4 — Quality:** tests (frontend + e2e), lint clean, build/typecheck, perf, final polish.
- **Wave N — Audit loop:** every 30 min re-check build/test/lint + this plan; dispatch next wave; stop when all green & complete.

## 5. Definition of Done
- `npm run build` clean; `npm run lint` clean; `npm test` green.
- `cargo check` 0 warnings; `cargo test` green; sqlx upgraded.
- Every page restyled to Industrial Magnetite; no leftover old amber-on-`#0a0a0f` look.
- README/roadmap/TASKS accurate to code + vision.
- No console errors on key routes.

## 6. Progress Log
- **Wave 0 (setup):** Reviewed repo (69 pages, 100 components, 27 API modules, 18 services; both build). Confirmed stale docs, 341 backend warnings, HTML5/Rust copy mismatch, mock-data pages. Created branch, gitignore for `target`, this file. Baseline commit next. → launching Wave 1.
