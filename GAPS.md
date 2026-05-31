# Magnetite — What's Left (current gap state, 2026-06-01)

The original read-only audit found **129** missing/stub/mock/hardcoded/documented-only items. Fix waves
**F1–F3** closed buckets **A (real bugs), B (frontend mocks), C (in-code integrations)**. What remains is
**bucket D** (needs external infrastructure or credentials — not code) plus a short tail of small endpoints
that *could* be coded now. See DECISIONS.md §4c / §6b for the decisions and per-wave detail.

Current verified state: 6 Rust crates `cargo check` 0 warnings + `cargo fmt` clean; frontend build clean,
lint 0 errors (65 intentional warnings), 146 FE tests + 189 backend tests pass.

---

## ✅ Closed in F1–F3 (was mock/stub/bug, now real)
- **Bugs:** marketplace + payout revenue split 0.7%→**70%**; `create_game`/GitHub repo attributed to the
  authed user (not nil UUID); `services/games.rs` `todo!()` → real sqlx; admin analytics rewritten vs real tables.
- **Security:** OAuth `state` CSRF on Discord/GitHub/GitLab; Google ID-token RS256/JWKS verify; admin-role
  guards on admin + `points/award` + `points/season/reset`.
- **Email:** `EmailProvider` trait — **Resend (HTTP)** + **SES (lettre SMTP)**, env-selected; wired into
  register/verify/forgot/reset + payout/subscription/ban notifications; unconfigured → clear error.
- **Payments:** real **Circle + Paystack** HTTP clients, env-gated; explicit error when unconfigured
  (`PAYMENTS_SANDBOX=true` for dev); real wallet deposit/withdraw + subscription + developer payout dispatch;
  Paystack uses real user email; `ZAR_USDC_RATE` env.
- **Realtime/gameplay:** `ws/game` mounted + authoritative loop (JWT auth, room join, input, state broadcast);
  matchmaking session allocation + `server_endpoint` + region filter + queue-depth wait estimate;
  **anti-cheat** velocity check wired into input + DB ban-on-connect + replay store at session end.
- **Jobs:** payout, subscription-renewal, and cleanup jobs spawned.
- **Frontend:** ~40 pages/hooks/contexts de-mocked → real fetch + loading/empty/error; mocks only behind
  `VITE_USE_MOCKS` / `VITE_USE_MOCK_WS` (off by default, never silent fake success); `useAuth` no longer
  fabricates a JWT; real browser WebSocket.
- **Docs/config:** all new env vars in `.env.example`; external-dependency doc.

---

## 🟥 Bucket D — needs external infra / credentials (NOT code)
1. **Media server (MediaMTX)** for voice + streaming — HLS playback, RTMP egress to Twitch/YouTube, WebRTC
   WHIP ingest. Backend manages stream lifecycle/metadata + proxies the manifest; **no media flows without it**
   (`MEDIA_SERVER_BASE_URL`).
2. **Voice SFU** (LiveKit/mediasoup) for rooms larger than ~8 — current voice is WebRTC **mesh** only.
3. **GitHub CI runner executing `wasm-pack`** — backend records build status + ships a build script, but no
   runner actually compiles registered repos to WASM.
4. **Dedicated / auto-scaled game servers** — matchmaking points sessions at `GAME_SERVER_WS_BASE` (the
   platform's own `ws/game`, fine for small/dev); no separate scalable game-server fleet.
5. **Full Bevy WASM builds** for `game-template-fps` / `game-template-motorsport` — verified via
   `cargo check --no-default-features`; the real render/engine path + wasm bundle aren't compiled in CI.
6. **Live FX feed** for ZAR→USDC — currently an env-configurable constant, not a live oracle.
7. **Provider credentials** to actually move money / send mail: `CIRCLE_API_KEY`, `PAYSTACK_SECRET_KEY`,
   `RESEND_API_KEY` / SES SMTP creds. Code is real; needs keys (or sandbox/dev flags).
8. **Circle deposit webhook receiver** — Paystack deposit is verified; the Circle deposit path trusts the
   amount pending a webhook confirmation (inline TODO in `api/wallet.rs`).

## 🟧 Small code leftovers (could be done now, no infra needed)
- **Review helpful/report** endpoints — `GameDetail` `onHelpful`/`onReport` are no-ops; `POST
  /api/games/:id/reviews/:reviewId/{helpful,report}` not implemented.
- **2FA TOTP** setup/verify backend endpoints — `Security.jsx` shows a disabled state + TODO.
- **Contact form** — no backend endpoint (submission goes nowhere).
- **Intentional demo-mode mocks** (acceptable, documented): `GameOverlay` and `GoLivePanel` fall back to a
  visual demo state only when the comms/stream backend is absent — not fabricating live data.
- **65 frontend lint warnings** — experimental `react-hooks` rules deliberately set to `warn` (set-state-in-
  effect, refs, purity). Could be refactored to satisfy them if desired.
- **e2e not executed** — Playwright specs exist + are coherent, but haven't been run against a live stack.

## 🚚 Delivery
- All work is on branch **`feat/redesign-and-harden`** (~16 commits) — **not merged to `main`, not pushed**.
