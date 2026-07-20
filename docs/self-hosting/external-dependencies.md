# Self-Hosting: External Dependencies

Magnetite's core stack — backend API, frontend, PostgreSQL, Redis — runs entirely on
what is shipped in this repository. There is no central Magnetite cloud to sign up
for: anyone runs the node. A handful of *optional* capabilities require additional
infrastructure or third-party credentials that are **not shipped and not faked**.
This page documents each one honestly: what it is, what you need, and what happens
if it is absent.

The default configuration — `PAYMENT_RAIL=mock`, `COMMS_PROVIDER=builtin`, no email
provider, no OAuth provider, no media server — needs **zero third-party accounts**.
It is what CI and `magnetite dev` run on.

---

## Required

Exactly two services are required by the backend. Both are in `docker-compose.yml`.

| Service | Env var | Absent behaviour |
|---------|---------|-----------------|
| PostgreSQL | `DATABASE_URL` | Backend fails to start |
| Redis | `REDIS_URL` | Backend fails to start |

See [Database](./database.md) and [Local Infrastructure](./local-infra.md) for setup.

---

## Optional

Everything else is optional. Absent means the specific capability is off — never a
broken node.

| Capability | Dependency | Absent behaviour |
|------------|-----------|-----------------|
| Live-stream watch (HLS) | MediaMTX media server (`media` compose profile) | `/streams/:id/hls.m3u8` returns HTTP 503 |
| RTMP egress to Twitch/YouTube | MediaMTX + configured runOnPublish | Egress silently does nothing |
| External chat / voice / video | Matrix, Jitsi, LiveKit or Owncast deployment | Provider falls back to `builtin` with a warning |
| WASM game builds via CI | GitHub CI runner + wasm-pack | Build status stays `building`; artifact never uploaded |
| Dedicated / auto-scaled game servers | Self-hosted or cloud game server fleet | `server_endpoint` on match sessions is `null`; WS connects to the same host as the API |
| Transactional email | Resend API key **or** AWS SES SMTP credentials | Email not sent; auth verification links not dispatched |
| Social sign-in | Google / Discord / GitHub / GitLab OAuth app | That sign-in button is unavailable; keypair and password login still work |
| Real on-chain settlement | A chain rail — **not implemented yet** | `PAYMENT_RAIL=mock` issues deterministic signed receipts, fully offline |

---

## Payments — No External Dependency

Magnetite holds **no funds**. There is no fiat on-ramp, no custodial balance, no
withdrawal and no payout: buyers pay sellers wallet-to-wallet through the
`PaymentRail` seam, and the signed `Receipt` *is* the entitlement — the node
re-verifies the rail signature on every access, so a database row alone never
grants anything. A refund voids the receipt and revokes the entitlement; no balance
moves because none exists.

The developer receives the whole subtotal. The platform takes only
`PROTOCOL_FEE_BPS`, which defaults to `0` and rides on top of the subtotal.

The default rail, `PAYMENT_RAIL=mock`, produces deterministic signed receipts with
no network access at all — no account, no key, no chain. `CHAIN_RPC_URL`,
`CHAIN_ID` and `STABLECOIN_ADDRESS` exist as placeholders for a future real
on-chain rail and are **unused** by the mock rail; no real chain rail is
implemented yet. `OPERATOR_WALLET_PUBKEY` is only needed if this node sells
hosting or paid tiers.

See [Environment Variables](./environment-variables.md) for the full table.

---

## MediaMTX — Voice / HLS Streaming (optional)

Media is **per-operator**: every operator runs their own media server, and the
backend has no dependency on one. In `docker-compose.yml` MediaMTX sits behind the
`media` compose profile, so it only starts with:

```bash
docker compose --profile media up
```

`MEDIA_SERVER_BASE_URL` is empty by default. The pluggable comms providers
(Jitsi, LiveKit, Owncast) bring their own media plane and need no MediaMTX at all.

**What it does:** MediaMTX is an open-source, self-hosted RTSP/RTMP/HLS/WebRTC media
server. Magnetite uses it as the media plane for:

- **HLS watch**: the backend proxies or redirects `/streams/:id/hls.m3u8` to
  `MEDIA_SERVER_BASE_URL`. Without MediaMTX, all stream-watch requests return HTTP 503.
- **RTMP egress**: to forward a stream to Twitch/YouTube, MediaMTX must be configured
  with a `runOnPublish` hook. The backend stores the RTMP target URL but does not touch
  media bytes.

**What you need:**

1. Run MediaMTX (Docker image: `bluenviron/mediamtx:latest`) reachable from the backend.
2. Set `MEDIA_SERVER_BASE_URL=http://<mediamtx-host>:8888` in the backend environment.
3. For RTMP egress, configure MediaMTX `runOnPublish` to forward to the streamer's
   configured target (Twitch/YouTube stream key).

**Resources:** [MediaMTX documentation](https://github.com/bluenviron/mediamtx)

---

## GitHub CI Runner + wasm-pack — WASM Game Builds (optional)

**What it does:** The `trigger_wasm_build` endpoint (called from the GitHub App webhook
on push) currently records a `status='building'` row in the database and logs a message.
It does **not** invoke `cargo` or `wasm-pack` — there is no build worker or subprocess.

For real WASM builds you need:

1. A GitHub Actions runner (GitHub-hosted or self-hosted) with `wasm-pack` installed.
2. A GitHub App registered and installed on developer game repos, with
   `GITHUB_APP_ID`, `GITHUB_APP_PRIVATE_KEY`, and `GITHUB_WEBHOOK_SECRET` set.
3. A CI workflow (`.github/workflows/game-ci.yml`, already in this repo for the arcade
   template) that runs `wasm-pack build` and uploads the artifact to a CDN or storage
   bucket, then calls `PUT /api/v1/distribution/:game_id/artifacts/:artifact_id` to
   set `artifact_url`.

Without this, `artifact_url` stays `null` in the DB and the play manifest has no
playable WASM URL.

**Resources:** [wasm-pack](https://rustwasm.github.io/wasm-pack/), [GitHub Apps](https://docs.github.com/en/apps)

---

## Dedicated / Auto-Scaled Game Servers (optional)

**What it does:** Magnetite's matchmaking service sets a `server_endpoint` on new
game sessions. The `GAME_SERVER_WS_BASE` env var controls what URL is used. In
single-server dev mode this defaults to `ws://localhost:8080` — the game WebSocket
handler (`ws/game.rs`) runs on the same process as the API.

For production multiplayer at scale you need:

1. A fleet of dedicated game server processes (or containers), each running the game
   server binary and exposing a WebSocket endpoint.
2. An orchestration layer (Kubernetes, Agones, Fly Machines, etc.) that provisions and
   scales game server instances on demand.
3. A matchmaking integration that sets `GAME_SERVER_WS_BASE` to the correct host, or
   a dynamic allocation system that updates `server_endpoint` per session.

Without this, all sessions route to the same backend process, which limits concurrency
and means no geographic distribution.

---

## Comms Providers (optional)

Magnetite builds no chat/voice/video/streaming of its own. Everything social sits
behind one adapter seam selected by `COMMS_PROVIDER`:

| Value | Provider | External service |
|-------|----------|------------------|
| `builtin` (default) | The demoted in-house chat/voice/streaming stack | none |
| `matrix` | Text / DMs / presence / spaces | Matrix homeserver (`MATRIX_HOMESERVER`) |
| `jitsi` | Voice + video SFU | Jitsi deployment (`JITSI_DOMAIN`) |
| `livekit` | Voice + video at scale | LiveKit deployment (`LIVEKIT_URL`) |
| `owncast` | Live streaming / VOD | Owncast instance (`OWNCAST_URL`) |

A provider whose service is not configured falls back to `builtin` with a warning —
it never breaks the node. `builtin` requires zero external services and works
offline.

---

## Transactional Email (Resend or AWS SES) (optional)

**What it does:** Transactional email is used for account verification links, password
reset, welcome messages, and subscription notifications.

If email credentials are absent:
- `send_email` returns a clear error (not a silent no-op).
- Registration still succeeds but the verification email is not sent.
- The `/auth/resend-verification` endpoint allows users to request a new link.

**Option A — Resend** (recommended for most self-hosters):

1. Create an account at [resend.com](https://resend.com) and generate an API key.
2. Set `RESEND_API_KEY` and `EMAIL_PROVIDER=resend`.
3. Verify your sending domain in the Resend dashboard.

**Option B — AWS SES via SMTP**:

1. Verify a sending domain in the [AWS SES console](https://aws.amazon.com/ses/).
2. Generate SES SMTP credentials (IAM → SES SMTP credentials — these differ from IAM
   access keys).
3. Set `AWS_SES_SMTP_USER`, `AWS_SES_SMTP_PASSWORD`, `AWS_SES_REGION`, and
   `EMAIL_PROVIDER=ses`.

---

## Roadmap

The items above are tracked in `docs/project/TASKS.md`. Contributions that integrate dedicated
game server allocation (e.g. via Agones), automate the WASM CI pipeline, implement a
real on-chain `PaymentRail`, or improve the media deployment story are welcome — see
[CONTRIBUTING.md](../../CONTRIBUTING.md).
