# Magnetite — Gap Audit (re-audit post F1+F2, 2026-05-30)

> Re-audit performed by the RE-AUDIT partition agent after F1 and F2 fix waves are
> committed. Every claim below was verified by reading the actual .rs and .jsx files;
> line numbers reference the state of the working tree at the time of this re-audit.
>
> **Last updated: 2026-06-03 (INFRA-E2E wave — Agent 5 docs+audit update).** MX1b
> (`def014a`) closed: refunds, content-rating, blocked-routes, analytics time-series,
> email verification enforcement, MediaMTX in docker-compose, wasm-build-runner docs.
> MediaMTX and wasm-runner moved from Bucket D to Closed. Summary counts updated.
>
> **Status legend:** **closed** (code evidence confirms the fix is real), **partial**
> (real but incomplete), **stub** (handler/UI exists but no-op/canned), **mock**
> (fabricated/mock-fallback data), **hardcoded** (literal placeholder), **documented-only**
> (needs external infra/deploy, not wired — Bucket D).

---

## Closed in F1 (fix wave 1)

Evidence: committed code in working tree.

| # | Finding (original) | What was fixed | File evidence |
|---|---|---|---|
| C1 | Marketplace dev-share 0.7% instead of 70% | `/100` divisor removed; `developer_share_pct()` now returns `Decimal::new(70,2)` = 0.70 and is applied directly without further division | `backend/src/services/marketplace.rs:17-22` |
| C2 | Payout fee split wrong (same `/100` bug) | `platform_fee_percent()` and `developer_share_percent()` in payout service fixed; comment on line 14 explicitly warns against re-adding `/100` | `backend/src/services/payout.rs:13-21` |
| C3 | `create_game` hardcodes nil UUID as `developer_id` | Handler uses `Extension(developer_id): Extension<Uuid>` extractor | `backend/src/api/games.rs:81,86,91` |
| C4 | `services/games.rs` — all five functions are `todo!()` panics | Replaced with real sqlx SQL implementations | `backend/src/services/games.rs:128-150` |
| C5 | Admin `analytics_performance` queries non-existent tables | Endpoint rewritten to query `transactions` (24h count) and `voice_participants` (active WS proxy); decision recorded per §4c | `backend/src/api/admin.rs:995-1046` |
| C6 | OAuth CSRF state absent on Discord/GitHub/GitLab | `validate_state_cookie()` helper added; called in all three callbacks | `backend/src/api/oauth.rs:624,657,689,721` |
| C7 | Google ID token — no RS256 signature verification | `decode_google_id_token()` now fetches Google JWKS, matches `kid`, verifies RS256 signature via `jsonwebtoken` | `backend/src/api/oauth.rs:494-570` |
| C8 | `points/award` and `points/season/reset` — no admin guard | `require_admin()` added; all three endpoint groups protected | `backend/src/api/points.rs:71-84,123,249` |
| C9 | `ws/game.rs` router not mounted | `GameWsHandler::router()` merged into Axum app | `backend/src/main.rs:125,135` |
| C10 | Background jobs (payout, subscription renewal) never spawned | Payout batch and subscription renewal jobs spawned on 1-hour intervals; session cleanup and verification cleanup also spawned | `backend/src/main.rs:149-187` |
| C11 | `EmailService::send_email()` is a no-op stub | `EmailProvider` trait + `ResendProvider` (real HTTPS POST to Resend) + `SesProvider` (lettre SMTP to SES); wired into register, forgot-password, reset-password, and payout-notification flows | `backend/src/services/email.rs:1-485` |
| C12 | `useWebSocket` always uses `createMockSocket` | Real `WebSocket` opened; mock only when `VITE_USE_MOCK_WS=true` | `src/hooks/useWebSocket.js:61-70` |
| C13 | `useAuth` fabricates mock JWT on login failure | Fabricated-JWT catch path removed; real error surfaced | `src/hooks/useAuth.js` |
| C14 | ~30 pages/hooks use mock data unconditionally | All pages/hooks fetch real data with loading/error states; mock data gated behind `VITE_USE_MOCKS=true` (default `false`) | `src/pages/admin/Users.jsx:22-31`, `src/pages/admin/Finance.jsx:22-56`, `src/pages/Matchmaking.jsx:15,26`, `src/pages/Wallet.jsx:13`, `src/pages/Earnings.jsx:6-35`, `src/hooks/useWallet.js:12-22`, and ~25 others |
| C15 | Matchmaking WS hardcoded to `ws://localhost:3000` | `getWsBase()` derives URL from `VITE_API_URL` env var | `src/pages/Matchmaking.jsx:19-22` |

## Closed in F2 (fix wave 2)

| # | Finding (original) | What was fixed | File evidence |
|---|---|---|---|
| C16 | `PaymentService` — Circle/Paystack all fabricated (random addresses, always-succeed) | `PaymentService::from_env()` with real `CIRCLE_API_KEY`/`PAYSTACK_SECRET_KEY`; sandbox mode (`PAYMENTS_SANDBOX=true`) returns clearly-labelled results; unconfigured → HTTP 502 error (not silent success); `create_wallet`, `get_wallet_balance`, `deposit_funds`, `withdraw_funds`, `create_payment` all make real reqwest calls to `api.circle.com` | `backend/src/services/payment.rs:855-960` |
| C17 | Wallet deposit — trusts caller-supplied `payment_id` without verification | `deposit()` calls `payment_svc.verify_paystack_payment()` (or trusts Circle amount for provider='circle') before crediting; rejects payment if Paystack status is not 'success'/'sandbox_success' | `backend/src/api/wallet.rs:93-183` |
| C18 | Wallet withdrawal — no Circle transfer; pending with no processor | `withdraw()` calls `payment_svc.withdraw_funds()` first; DB debit happens only after Circle call succeeds | `backend/src/api/wallet.rs:186-262` |
| C19 | Subscription — hardcoded `payment_provider='stripe'` | Provider determined from caller (circle/paystack); correct currency bound | `backend/src/api/subscriptions.rs:273-292` |
| C20 | Paystack uses fabricated email placeholder | `create_paystack_subscription` now looks up the user's real email from DB | `backend/src/services/payment.rs:243 ("never use a fabricated placeholder")` |
| C21 | `process_single_payout()` marks completed with no transfer | Calls Circle `/v1/transfers`; sandbox-mode logs clearly; marks `failed` on HTTP error | `backend/src/services/payout.rs:250-380` |
| C22 | Payout service `/100` bug (same as C2 but in payout.rs test path) | Fixed in F2 alongside real disbursement; test now passes | `backend/src/services/payout.rs:107-110` |
| C23 | `matchmaking::start_game_session()` — `server_endpoint` always `None` | Sets `server_endpoint` from `GAME_SERVER_WS_BASE` env var (default `ws://localhost:8080`) | `backend/src/services/matchmaking.rs:480-523` |
| C24 | Matchmaking estimated wait time always `Some(30)` hardcoded | `estimate_wait_seconds()` queries actual queue depth; 30s per position, clamped 5-600s | `backend/src/api/matchmaking.rs:17-28` |
| C25 | `matchmaking::filter_by_region()` ignores region entirely | In-memory filter removed; `filter_by_region_db()` added for actual region query; comment clarifies the distinction | `backend/src/services/matchmaking.rs:383-410` |
| C26 | Anti-cheat service — never called from game sessions | `ws/game.rs` now: calls `check_ban()` on connect, velocity check on every input tick, `detect_anomalies()` + `ban_user()` + `store_replay()` on session end | `backend/src/ws/game.rs:4-7,511,662,702,758,783` |
| C27 | ZAR→USDC rate hardcoded at 275.0 | `convert_zar_to_usdc()` reads `ZAR_USDC_RATE` env var; default kept but env-overridable | `backend/src/services/payment.rs:1291-1305` |
| C28 | Session cleanup job never spawned | `session_cleanup::run_cleanup_jobs()` spawned in main.rs | `backend/src/main.rs:183` |
| C29 | Verification token never wired to auth flow | `generate_email_verification_token` + `verify_email_token` called from register/verify/resend-verification handlers; password-reset token wired to forgot/reset handlers | `backend/src/api/auth.rs:124,375,421` |
| C30 | Admin pages — all data from MOCK_* with fake setTimeout actions | All admin pages call real backend; mock data only when `VITE_USE_MOCKS=true` | `src/pages/admin/Users.jsx:59-85`, `src/pages/admin/Finance.jsx:56-95`, `src/pages/admin/AdminDashboard.jsx:75-128` |
| C31 | `GameLobby` — MOCK_PLAYERS + `handleStartGame` console.log | `useGameLobby` uses real `useWebSocket`; `startGame()` sends WS message; no MOCK_PLAYERS | `src/pages/GameLobby.jsx:1-82`, `src/hooks/useGameLobby.js:1-60` |
| C32 | `GameStudio` — GitHub connect does `setTimeout(1500)` | Calls real `GET /api/github/installations`; sets `githubConnected` from real response | `src/pages/GameStudio.jsx:41-59` |
| C33 | `GameDeploy` — MOCK_REPOS/MOCK_BRANCHES/MOCK_DEPLOYMENTS; rollback console.log | All load from real API (gated on `VITE_USE_MOCKS`); rollback shows an honest "not yet implemented" alert | `src/pages/developers/GameDeploy.jsx:26-50,286-291` |
| C34 | `Subscription` — subscribe/cancel are `console.log` stubs | `handleSubscribe` and `handleCancelSubscription` call real API endpoints | `src/pages/Subscription.jsx:120-153` |
| C35 | `Security` — password change/2FA/session revoke all local state; API keys with `Math.random()` | Password change and session revoke call real API; API key creation form is disabled with explicit TODO noting the backend endpoint doesn't exist yet (no `Math.random`) | `src/pages/Security.jsx:84-179,173-178` |
| C36 | Background jobs missing from .env.example + no docs | All new env vars documented in `.env.example` and `docs/self-hosting/environment-variables.md` | `.env.example:39-111`, `docs/self-hosting/environment-variables.md` |
| C37 | No external-dependencies docs | `docs/self-hosting/external-dependencies.md` created with honest absence behaviour for all Bucket D items | `docs/self-hosting/external-dependencies.md:1-200` |

---

## Remaining — genuine smaller gaps (not Bucket D)

These are real gaps in the current working tree, confirmed by code inspection.

| Impact | Status | Finding | File:line |
|---|---|---|---|
| medium | stub | `api/platform.rs` (platform settings) — module exists with `get_settings`/`update_settings` handlers but is NOT imported or nested in `main.rs` router | `backend/src/api/platform.rs:1` ("not yet wired"); `backend/src/main.rs` (no `platform` import) |
| medium | stub | Admin Settings page — `handleSave` calls `/api/platform/settings` but the route is unmounted; throws a clear error ("Platform settings API is not yet mounted") | `src/pages/admin/Settings.jsx:105-108` |
| medium | stub | `Security.jsx` — API key creation form (`handleCreateApiKey`) is intentionally a no-op with a TODO; backend has no `/api/auth/api-keys` endpoint | `src/pages/Security.jsx:173-179` |
| medium | stub | `Contact.jsx` — form `handleSubmit` only sets `submitted=true`; no API call; all social links are `href='#'` | `src/pages/Contact.jsx:60-62` |
| medium | stub | `GameDetail.jsx` — `onHelpful` and `onReport` review callbacks are `console.log` no-ops; no API call | `src/pages/GameDetail.jsx:309-310` |
| medium | stub | `trigger_wasm_build` (GitHub webhook) — writes `status='building'` DB row and a log entry only; no `cargo`/`wasm-pack` invocation (Bucket D reason: needs CI runner) | `backend/src/api/github.rs:573-598` |
| medium | documented-only | `PaymentService` — file still has `#![allow(dead_code)]` header; methods work when env vars are set but service is constructed per-request; no shared payment state in app | `backend/src/services/payment.rs:2` |
| medium | partial | Circle deposit — `provider='circle'` path in `deposit()` trusts caller-supplied amount without verifying the Circle transfer via `/v1/transfers/{id}`; has a TODO comment noting this | `backend/src/api/wallet.rs:130-136` |
| medium | partial | `PaymentService::process_weekly_payouts()` — returns `Ok(vec![])` and logs "delegated to PayoutService"; the actual delegation to `PayoutService::process_pending_payouts()` happens via the background job spawned in main.rs, not via this method. Method is effectively a no-op shim | `backend/src/services/payment.rs:1365-1371` |
| medium | stub | `services/leaderboard.rs` — Redis-backed score service; `#![allow(dead_code)]`; never instantiated in any route handler | `backend/src/services/leaderboard.rs:1-2` |
| medium | stub | `services/achievements.rs` — unlock tracking service; `#![allow(dead_code)]`; never called from any handler | `backend/src/services/achievements.rs:1-2` |
| medium | stub | `api/tournaments.rs` — bracket management; `#![allow(dead_code)]`; mounted but no real logic | `backend/src/api/tournaments.rs:1` |
| low | partial | Admin analytics — `day_7_retention` and `day_30_retention` always `None`; only `day_1_retention` is queried | `backend/src/api/admin.rs:930-935` |
| low | partial | `jobs/backup.rs` — real `pg_dump` + S3/local storage code exists but is never `tokio::spawn`'d in `main.rs`; no scheduled backup runs | `backend/src/jobs/backup.rs:1`, `backend/src/main.rs` (no backup spawn) |
| low | partial | `services/games.rs` — file comment says "platform surface, not yet wired"; functions now have real SQL but the service layer is still `#![allow(dead_code)]` and not called from any handler (API handlers in `api/games.rs` query the DB directly, bypassing this service) | `backend/src/services/games.rs:1-2` |
| low | partial | `session_cleanup.rs` — `run_cleanup_jobs()` is now spawned in main.rs; however the file header still says "not yet scheduled" and the `#![allow(dead_code)]` attribute remains (stale comment only; function IS wired) | `backend/src/jobs/session_cleanup.rs:1-2` |
| low | hardcoded | README.md line 22 says "15% platform fee" but both the code (`payout.rs`, `marketplace.rs`) and `docs/economy-marketplace.md` implement and document 70/30 (30% platform fee). The README marketing copy was not updated to match the grounded decision | `README.md:22` vs `backend/src/services/payout.rs:16` and `docs/economy-marketplace.md:92-93` |
| low | partial | `Leaderboard.jsx` — imports `mockLeaderboard` from `src/data/mockLeaderboard`; `buildFallbackData()` uses it to generate synthetic entries; shown only when `VITE_USE_MOCKS=true` — correctly gated but the import is unconditional | `src/pages/Leaderboard.jsx:6,118` |
| low | stub | `useVoice` — `joinRoom` catch path returns `mock-voice-token-${roomId}-${Date.now()}`; only triggered when `VITE_USE_MOCKS=true` | `src/hooks/useVoice.js:73-74` |
| low | partial | SDK networking layer (`TickLoop`, `PredictionBuffer`, `StateSyncProtocol`) — real typed implementations with unit tests; no server-side driver calls `TickLoop` on a real async runtime in the backend binary | `backend/magnetite-sdk/src/networking.rs` |
| low | partial | `StreamPlayer` — `if (hlsUrl) { video.src = hlsUrl }` works for native HLS (Safari); comment says "HLS.js would be loaded here dynamically in production" but no HLS.js import exists; `<video>` shows "Awaiting media source" without a real `hlsUrl` | `src/components/streaming/StreamPlayer.jsx:38-41` |
| low | stub | `game-template-fps` `input_map.rs` — `gamepad_button()` always returns `false` (native gilrs integration is a documented future path) | `game-template-fps/src/input_map.rs:307-328` |
| low | stub | `game-template-fps` `bevy_client.rs` — `hud_text()` is `#[allow(dead_code)]` placeholder | `game-template-fps/src/bevy_client.rs:429` |

---

## Closed in Moat N1–N3 (2026-06-01)

These are the four MOAT differentiators previously listed as "Bucket D / remaining" in earlier audits.
All four are now implemented as real, compiling, tested Rust crates. Evidence is the working tree.

| Capability | Status | Crate evidence |
|---|---|---|
| **Scale primitive** — `SingleRoom` / `Dedicated` / `Sharded` topology auto-selection; identical game code across all three | **closed** | `backend/magnetite-sdk/src/authority.rs` — `Topology` enum, `MatchConfig::auto()`, `NativeExecutor<G: AuthoritativeGame>`; `magnetite-runtime/` — `TickScheduler` + `ShardManager` (N1 single-shard seam, handoff hook for N2+); `magnetite-e2e/tests/scale_bench.rs` — throughput bench across SingleRoom→Dedicated |
| **Sandbox** — untrusted game logic runs in Wasmtime with fuel/memory/epoch limits; deterministic (no wall clock, no OS random) | **closed** | `magnetite-sandbox/` — `WasmExecutor` implementing `GameExecutor`; `LimitsConfig` (fuel/memory/epoch); 9 WASI stub imports (clock→ENOSYS, random→ENOSYS); `magnetite-e2e/tests/wasm_end_to_end.rs` — `wasm_sandbox_parity_with_native` proves identical state_hash vs `NativeExecutor` over 30 ticks |
| **Anti-cheat** — server-authoritative by construction; composable `Validator` chain; deterministic replay re-simulation; trust-score escalation | **closed** | `magnetite-anticheat/` — `Anticheat`, `AimbotSnap`, `PositionTeleport`, `FireRateCooldown`, `InputFlood`, `TrustScoreMap`, `ReplayVerifier`; `magnetite-e2e/tests/anticheat.rs` — `anticheat_rejects_speedhack_and_escalates_trust_score` + `anticheat_allows_honest_client`; `magnetite-e2e/tests/convergence.rs` — `verify_replay` returns `Clean` |
| **One-command pipeline** — `magnetite new\|build\|dev\|deploy`; `cargo build --target wasm32-wasip1` → `WasmExecutor` → live server → connect URL | **closed** | `magnetite-cli/` — `magnetite new\|build\|dev\|deploy` binary (clap 4); `game-template-authoritative/src/wasm_abi.rs` — `mag_*` ABI exports (behind `--features wasm`); `scripts/moat-demo.sh` — one-command build→sandbox-parity→convergence→fmt→check pipeline; `magnetite-e2e/tests/wasm_end_to_end.rs` — end-to-end proof |

---

## Closed in Backlog B1 (2026-06-01)

The "Remaining — genuine smaller gaps" list above (lines 65-94) is now **largely closed** by Backlog
wave B1 (commit `b90248f`). All verified: backend 0 warnings + tests compile; frontend build clean,
lint 0 errors, 157 tests; game-template-fps + sdk clean.

| Item | Closed how (evidence) |
|---|---|
| `api/platform.rs` settings unmounted | mounted at `/api/v1/platform`; stale comment removed (`backend/src/main.rs:115`) |
| `api/tournaments.rs` allow-only shell | real bracket logic + mounted at `/api/v1/tournaments` (`backend/src/main.rs:117`) |
| `services/leaderboard.rs` never called | wired into `submit_score` (Redis primary + Postgres fallback) |
| `services/achievements.rs` never called | wired into achievement progress (`check_achievements`) |
| `jobs/backup.rs` never scheduled | spawned on a 6-hour interval in `main.rs` |
| admin day_7/day_30 retention always None | real CTE queries on `transactions` (`backend/src/api/admin.rs`) |
| API-key creation missing | `POST/GET/DELETE /api/auth/api-keys` (hashed, one-time secret) + migration; Security.jsx wired |
| 2FA TOTP missing | `POST /api/auth/2fa/{setup,verify,disable}` (inline RFC-6238) + migration; Security.jsx wired |
| review helpful/report missing | `POST /api/games/:id/reviews/:rid/{helpful,report}` + migration + trigger; GameDetail.jsx wired |
| Contact form went nowhere | `POST /api/contact` persisted (+ optional email); Contact.jsx wired |
| `reviews.rs` never mounted (latent) | declared in `mod.rs` + merged into `/games` nest; **4 hidden compile bugs fixed** (u32→i64) |
| StreamPlayer native-HLS only | dynamic `hls.js` import for non-Safari |
| game-template-fps `gamepad_button()` always false | real implementation via SDK gamepad input |
| README "15% platform fee" | corrected to 30% (70/30) |
| stale "not yet wired"/dead-code comments | swept in `session_cleanup`, `payment`, `networking`, `games` |

**Accepted (not gaps):** `services/games.rs` kept as a typed library surface (api/games queries the DB
directly — decision GC-R5); `process_weekly_payouts()` is an intentional shim (real work runs in the
spawned payout job); `useVoice`/`Leaderboard` mock fallbacks are gated behind `VITE_USE_MOCKS` (off by
default, never silent). None are silent-mock-successes.

**Remaining after B1 = Bucket D only** (external infra/credentials — see below). The codeable backlog is closed.

---

## Closed in MX1b (2026-06-03)

Wave MX1b (`def014a`) closed the following items from "Remaining — genuine smaller gaps" and
from the Bucket D list. All verified grep-on-disk per DECISIONS.md §6.

| Item | How closed | Evidence |
|------|-----------|---------|
| Refunds/chargebacks missing | `POST /api/v1/admin/transactions/:id/refund` + Finance.jsx wired | `backend/src/api/admin.rs` |
| Content rating absent | `content_rating` column + enum on `games`; displayed on cards; age-gate on M/AO play page | `backend/src/api/games.rs` |
| Blocked-user routes not routed | `GET /friends/blocked` + `DELETE /friends/block/:id` + `FriendService::unblock` | `backend/src/api/social.rs` |
| Developer analytics no time-series | `daily_revenue_chart` added to `GameAnalytics` | `backend/src/api/developer.rs` |
| Email verification not enforced at login | `email_verified` checked; restricted token for unverified | `backend/src/api/auth.rs` |
| MediaMTX — HLS 503 without separately deployed MediaMTX | MediaMTX added as a service to `docker-compose.yml`; `config/mediamtx.yml`; `MEDIA_SERVER_BASE_URL` wired in backend service | `docker-compose.yml`; `docker-compose.override.yml`; `docs/self-hosting/streaming.md` |
| MediaMTX — RTMP egress to Twitch/YouTube | `config/mediamtx.yml` ships `runOnPublish` template; documented in `docs/self-hosting/streaming.md` | `config/mediamtx.yml`; `docs/self-hosting/streaming.md:170-190` |
| WASM build runner not documented | `scripts/wasm-build-runner.sh` created; `docs/self-hosting/local-infra.md` updated | `scripts/wasm-build-runner.sh`; `docs/self-hosting/local-infra.md` |

---

## Remaining — Bucket D (needs external infra/credentials)

These cannot be resolved without external infrastructure or third-party accounts. They are honestly documented and not faked. The MOAT items previously listed here have been closed (see section above).

| Capability | Status | What is missing | File evidence / docs |
|---|---|---|---|
| **MediaMTX media server — HLS watch (local docker compose)** | **closed** (MX1b) | MediaMTX is now a Docker Compose service. `docker compose up` starts it. `/streams/:id/hls` returns 302 to `http://mediamtx:8888/live/<key>/index.m3u8`. | `docker-compose.yml`; `docs/self-hosting/streaming.md` |
| **MediaMTX — RTMP egress to Twitch/YouTube (local docker compose)** | **closed** (MX1b) | `config/mediamtx.yml` ships a `runOnPublish` template for FFmpeg forwarding to Twitch/YouTube; documented in streaming.md. | `config/mediamtx.yml`; `docs/self-hosting/streaming.md:170-190` |
| **Voice SFU (LiveKit/mediasoup) for large rooms** | documented-only | Current WebRTC mesh relay works up to ~8 participants; SFU is the scale path, not implemented | `backend/src/ws/voice.rs:5-17`; `docs/comms/realtime.md` |
| **GitHub CI runner executing `wasm-pack` for the platform store** | documented-only | `trigger_wasm_build()` in the storefront API writes a `status='building'` DB row; no subprocess invoked; WASM artifact never uploaded to CDN | `backend/src/api/github.rs:573-598`; `docs/self-hosting/external-dependencies.md:47-75` |
| **game-template-fps and game-template-motorsport — full WASM CI builds** | documented-only | `game-ci.yml` covers only `game-template` (arcade); FPS and motorsport have no CI WASM build (they pass `cargo check --no-default-features` only) | `.github/workflows/game-ci.yml:21,59` |
| **Multi-node sharding / distributed shard coordination** | documented-only | `ShardManager` always assigns `ShardId::LOCAL` in N1; multi-process shard handoff and distributed coordination require a separate N3+ pass | `magnetite-runtime/src/shard.rs`; DECISIONS.md §10 crossroad R5 |
| **Cloud auto-scaled runner fleet** | documented-only | `magnetite deploy` registers the artifact and requests a runtime instance via the backend distribution API; no auto-scaled cloud fleet provisioned; self-hosted runner only | `magnetite-cli/src/main.rs`; `backend/src/api/distribution.rs` |
| **Production container orchestration** | documented-only | No Kubernetes/Nomad manifests for the magnetite-runtime process; Dockerfile.fly covers the monolith only | `Dockerfile.fly`; `fly.toml` |
| **Wise live credentials (developer payouts)** | documented-only | Wise payout dispatch works when `WISE_API_TOKEN` + `WISE_PROFILE_ID` are set (or `WISE_SANDBOX=true` for dev); absent → HTTP 502 "payouts not configured" (no silent success) | `backend/src/services/wise.rs`; `.env.example` (WISE_*) |
| **Paystack live credentials (fiat on-ramp)** | documented-only | `PAYSTACK_SECRET_KEY` required for player deposits + paid subscriptions; absent → HTTP 502 | `backend/src/services/payment.rs`; `.env.example` |
| **Transactional email credentials** | documented-only | `RESEND_API_KEY` (Resend) or `AWS_SES_SMTP_USER/PASSWORD` (SES) required; absent → Err returned, no silent success | `backend/src/services/email.rs:41-50,111-130`; `.env.example:57-87` |
| *(removed)* ~~Circle deposit webhook~~ | n/a | **Obsolete after the 2026-06-01 payments pivot** — crypto/USDC/Circle removed entirely; deposits are Paystack-verified (implemented), payouts are Wise. | — |
| **SDK platform clients — no transport** | documented-only | All five SDK platform clients (`comms`, `points`, `marketplace`, `cloud_save`, `streaming`) are I/O-free message-builder types; caller must supply WebSocket send/receive | `backend/magnetite-sdk/src/platform/comms.rs:546` |

---

## Closed vs Remaining — Summary

**F1 fixed:** 15 findings  
**F2 fixed:** 22 findings  
**F3 fixed:** final de-mock + anti-cheat DB wiring  
**Total closed (F1–F3):** 37+ findings  
**Closed in Moat N1–N3 (2026-06-01):** 4 MOAT differentiators (scale primitive, sandbox, anti-cheat, one-command pipeline)  
**Closed in Backlog B1 (2026-06-01):** 14 smaller gaps (tournaments, leaderboard, achievements, backup job, etc.)  
**Closed in MX1b (2026-06-03):** 8 items (refunds, content rating, blocked routes, analytics time-series, email verification, MediaMTX in compose, wasm-build-runner docs)

**Remaining — smaller gaps (not Bucket D):** ~14 genuinely open medium/low items (see AUDIT.md "Still genuinely open" table)  
**Remaining — Bucket D (external infra/creds):** 9 entries (MediaMTX local compose closed; multi-node sharding, cloud runner fleet, Voice SFU, GitHub CI wasm-pack runner, FPS/motorsport WASM CI builds, Wise/Paystack/email live credentials remain)

Of the remaining non-Bucket-D gaps: all show honest errors or clearly-labelled absent states — none are silent mock successes.

---

## Crossroads recorded per §4c

| Crossroad | Decision | Rationale |
|---|---|---|
| Email provider abstraction | `ResendProvider` (reqwest HTTPS) + `SesProvider` (lettre SMTP to SES endpoint); `lettre` was already a Cargo.toml dep, avoiding new dep bloat | Resend works immediately with `RESEND_API_KEY`; SES via SMTP is live once AWS SMTP creds are set |
| Unconfigured payment behavior | Return HTTP 502 "payments not configured" (not fabricated success); `PAYMENTS_SANDBOX=true` returns clearly-labelled sandbox results | Fabricated success hides misconfiguration; explicit error forces operator to supply real keys |
| Admin analytics tables (`api_request_logs`, `websocket_connections`) | Rewrote endpoint against existing `transactions` and `voice_participants` tables; no new migration for tables with no writers yet | A hollow migration would create tables that always return 0, adding no value |
| Email templates | Rendered inline in Rust using string formatting; did not add Tera or Handlebars engine | Avoids a new crate; the existing `templates/emails/` directory uses Jinja2-style syntax incompatible with Handlebars |
| Payout fee split (30%/70% in code vs 15%/85% in DECISIONS.md §1 vision) | Code and economy docs settled at 70/30; README line 22 ("15% platform fee") is a stale marketing copy not updated during F1/F2 | 70/30 is what was grounded in DECISIONS.md §4c and implemented across marketplace + payout; README line 22 should be updated to "30% platform fee" in a future doc pass |
