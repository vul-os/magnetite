# Self-Hosting: External Dependencies (Bucket D)

Magnetite's core stack — backend API, frontend, PostgreSQL, Redis — runs entirely on
what is shipped in this repository. However, several platform capabilities require
additional infrastructure or third-party credentials that are **not shipped and not
faked**. This page documents each one honestly: what it is, what you need, and what
happens if it is absent.

---

## Summary

| Capability | Dependency | Absent behaviour |
|------------|-----------|-----------------|
| Live-stream watch (HLS) | MediaMTX media server | `/streams/:id/hls.m3u8` returns HTTP 503 |
| RTMP egress to Twitch/YouTube | MediaMTX + configured runOnPublish | Egress silently does nothing |
| WASM game builds via CI | GitHub CI runner + wasm-pack | Build status stays `building`; artifact never uploaded |
| Dedicated / auto-scaled game servers | Self-hosted or cloud game server fleet | `server_endpoint` on match sessions is `null`; WS connects to the same host as the API |
| Fiat deposits + subscriptions (Paystack) | Paystack account + `PAYSTACK_SECRET_KEY` | Paystack endpoints return HTTP 502 |
| Developer payouts (Wise) | Wise account + `WISE_API_TOKEN` + `WISE_PROFILE_ID` | Payout endpoints return HTTP 502 |
| Transactional email | Resend API key **or** AWS SES SMTP credentials | Email not sent; auth verification links not dispatched |

---

## MediaMTX — Voice / HLS Streaming

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

## GitHub CI Runner + wasm-pack — WASM Game Builds

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

## Dedicated / Auto-Scaled Game Servers

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

## Paystack — Fiat On-Ramp (Deposits + Subscriptions)

**What it does:** Paystack handles fiat card and bank payments for wallet top-ups and
paid subscriptions (all denominated in USD). It is the player-facing on-ramp.

If `PAYSTACK_SECRET_KEY` is not set:
- The Paystack deposit, subscription, and payment-initiation endpoints return HTTP 502
  (`ProviderUnconfigured`).
- Set `PAYMENTS_SANDBOX=true` for local dev sandbox behaviour.

**What you need:**

1. A Paystack account from [paystack.com](https://paystack.com).
2. Set `PAYSTACK_SECRET_KEY` to your Paystack secret key.
3. Configure the Paystack webhook URL to point to
   `https://api.your-domain.com/api/v1/webhooks/paystack`.

---

## Wise — Developer Payouts

**What it does:** Wise (TransferWise) handles outbound payout disbursements to developers.
When a developer requests a payout, the backend creates a Wise quote → transfer → funds it
from the platform's Wise balance. The developer receives their 70 % share to their bank
account or Wise balance.

If `WISE_API_TOKEN` or `WISE_PROFILE_ID` is not set:
- Payout endpoints return HTTP 502 (`ProviderUnconfigured`).
- No money moves and no payout is fabricated.
- Set `WISE_SANDBOX=true` to direct requests to `api.sandbox.transferwise.tech` for local testing.

**What you need:**

1. A Wise business account at [wise.com](https://wise.com).
2. Generate an API token under **Settings → API tokens** (full access required for transfers).
3. Set `WISE_API_TOKEN` and `WISE_PROFILE_ID` in the backend environment.
4. Ensure your Wise account has sufficient balance in USD to fund payouts (or set up auto-funding).

---

## Transactional Email (Resend or AWS SES)

**What it does:** Transactional email is used for account verification links, password
reset, welcome messages, and payout/subscription notifications.

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

The items above are tracked in `TASKS.md` under the **Bucket D** label. Contributions
that integrate dedicated game server allocation (e.g. via Agones), automate the WASM
CI pipeline, or improve the MediaMTX deployment story are welcome — see
[CONTRIBUTING.md](../../CONTRIBUTING.md).
