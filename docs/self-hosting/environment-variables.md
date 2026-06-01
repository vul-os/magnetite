# Environment Variables

Copy `.env.example` to `.env` and fill in the required values before starting the stack.
Variables marked **required** will cause the backend to fail at startup if absent.

---

## Required

| Variable | Example | Description |
|----------|---------|-------------|
| `DATABASE_URL` | `postgres://magnetite:pw@postgres:5432/magnetite` | PostgreSQL connection string |
| `JWT_SECRET` | `$(openssl rand -hex 32)` | JWT signing secret — minimum 32 characters, never commit |

---

## PostgreSQL (Docker Compose)

| Variable | Default | Description |
|----------|---------|-------------|
| `POSTGRES_DB` | `magnetite` | Database name |
| `POSTGRES_USER` | `magnetite` | Database user |
| `POSTGRES_PASSWORD` | `magnetite_dev_password` | **Change in production** |
| `POSTGRES_PORT` | `5432` | Host-mapped port |

---

## Redis

| Variable | Default | Description |
|----------|---------|-------------|
| `REDIS_URL` | `redis://redis:6379` | Redis connection URL |
| `REDIS_PORT` | `6379` | Host-mapped port (Docker Compose) |

---

## Server

| Variable | Default | Description |
|----------|---------|-------------|
| `SERVER_HOST` | `0.0.0.0` | Bind address (`127.0.0.1` for localhost-only) |
| `SERVER_PORT` | `8080` | Bind port |
| `FRONTEND_URL` | `http://localhost:5173` | Allowed CORS origin (dev) |
| `CORS_ALLOWED_ORIGINS` | `http://localhost:5173,http://localhost:3000` | Comma-separated allowed CORS origins |
| `RUST_LOG` | `info` | Log level: `error`, `warn`, `info`, `debug`, `trace` |
| `APP_ENV` | `development` | `development` or `production` |
| `APP_URL` | `http://localhost:8080` | Public base URL |

---

## JWT

| Variable | Default | Description |
|----------|---------|-------------|
| `ACCESS_TOKEN_EXPIRY` | `900` | Access token lifetime in seconds (15 min) |
| `REFRESH_TOKEN_EXPIRY` | `604800` | Refresh token lifetime in seconds (7 days) |

---

## OAuth providers

Leave unused providers blank. At least one is required for social login.

| Variable | Description |
|----------|-------------|
| `GOOGLE_CLIENT_ID` | Google OAuth app client ID |
| `GOOGLE_CLIENT_SECRET` | Google OAuth app secret |
| `GOOGLE_REDIRECT_URI` | e.g. `https://api.example.com/api/v1/oauth/google/callback` |
| `DISCORD_CLIENT_ID` | Discord app client ID |
| `DISCORD_CLIENT_SECRET` | Discord app secret |
| `DISCORD_REDIRECT_URI` | Discord OAuth callback URL |
| `GITHUB_CLIENT_ID` | GitHub OAuth app client ID |
| `GITHUB_CLIENT_SECRET` | GitHub OAuth app secret |
| `GITHUB_REDIRECT_URI` | GitHub OAuth callback URL |
| `GITLAB_CLIENT_ID` | GitLab OAuth app client ID |
| `GITLAB_CLIENT_SECRET` | GitLab OAuth app secret |
| `GITLAB_REDIRECT_URI` | GitLab OAuth callback URL |

---

## GitHub App (CI integration)

Required only for the WASM build pipeline (push webhooks + build status reporting).
Leave blank to disable the GitHub App integration in development.

| Variable | Description |
|----------|-------------|
| `GITHUB_APP_ID` | GitHub App numeric ID |
| `GITHUB_APP_PRIVATE_KEY` | PEM-encoded RSA private key (RS256) |
| `GITHUB_WEBHOOK_SECRET` | Secret used to verify `X-Hub-Signature-256` |

---

## Payments

All payment variables are optional. If a provider key is absent, the corresponding
endpoints return HTTP 502 (`ProviderUnconfigured`) instead of fabricating a successful
transfer. Set `PAYMENTS_SANDBOX=true` in local dev to receive labelled sandbox responses
without real credentials.

| Variable | Default | Description |
|----------|---------|-------------|
| `PAYSTACK_SECRET_KEY` | — | Paystack secret key for fiat on-ramp (deposits + subscriptions) |
| `WISE_API_TOKEN` | — | Wise API token for developer payout disbursements |
| `WISE_PROFILE_ID` | — | Wise sending-profile ID (business or personal) used for outbound transfers |
| `WISE_SANDBOX` | `false` | `true` routes Wise requests to `api.sandbox.transferwise.tech`; results are labelled sandbox |
| `PAYMENTS_SANDBOX` | `false` | `true` enables sandbox mode across providers: labelled placeholder results, no real money moves |

---

## Email

Set `EMAIL_PROVIDER` to choose a transport. Leave all credentials blank to disable
outbound email (verification and notification emails will not be sent — a clear error
is returned rather than silent no-op).

| Variable | Default | Description |
|----------|---------|-------------|
| `EMAIL_PROVIDER` | `resend` | `resend` or `ses` |
| `EMAIL_FROM` | `Magnetite <noreply@magnetite.gg>` | Full sender address shown in the From header |

**Resend** (recommended — one API key, no SMTP setup):

| Variable | Description |
|----------|-------------|
| `RESEND_API_KEY` | Resend API key from resend.com |

**AWS SES via SMTP** (`EMAIL_PROVIDER=ses`):

Uses lettre SMTP transport to `email-smtp.<AWS_SES_REGION>.amazonaws.com:587`.
Generate SES SMTP credentials in the AWS console under **IAM → SES SMTP credentials**
(these are different from standard IAM access keys).

| Variable | Default | Description |
|----------|---------|-------------|
| `AWS_SES_SMTP_USER` | — | SES SMTP username |
| `AWS_SES_SMTP_PASSWORD` | — | SES SMTP password |
| `AWS_SES_REGION` | `us-east-1` | AWS region for the SES SMTP endpoint |

---

## Media / Streaming

| Variable | Default | Description |
|----------|---------|-------------|
| `MEDIA_SERVER_BASE_URL` | — | Base URL of the external MediaMTX media server (e.g. `http://mediamtx:8888`). The backend proxies `/streams/:id/hls.m3u8` to this URL. If unset, the watch endpoint returns HTTP 503. **Bucket-D external dependency** — requires a separately deployed MediaMTX instance. |

---

## Game Server WebSocket

| Variable | Default | Description |
|----------|---------|-------------|
| `GAME_SERVER_WS_BASE` | `ws://localhost:8080` | WebSocket base URL used by matchmaking to set `server_endpoint` on new game sessions. In single-server dev mode this defaults to the backend's own host. **Bucket-D**: dedicated or auto-scaled game servers require a separate deployment and this URL. |

---

## Frontend (Vite build-time)

These variables are injected at build time by Vite and available as `import.meta.env.*`
in the frontend bundle. They must be set before running `npm run build`.

| Variable | Default | Description |
|----------|---------|-------------|
| `VITE_API_URL` | `http://localhost:8080` | Backend API base URL (no trailing slash). WebSocket connections are derived from this by replacing `http` with `ws`. |
| `VITE_USE_MOCKS` | `false` | `true` — all hooks fall back to static mock data; useful for UI-only development without a running backend. **Production: must be `false` or absent.** |
| `VITE_USE_MOCK_WS` | `false` | `true` — `useWebSocket` substitutes a local mock socket (no real WebSocket). **Production: must be `false` or absent.** |

---

## Docker Compose extras (development only)

Remove or firewall these services in production.

| Variable | Default | Description |
|----------|---------|-------------|
| `FRONTEND_PORT` | `3000` | Host port for the frontend container |
| `BACKEND_PORT` | `8080` | Host port for the backend container |
| `MAILHOG_SMTP_PORT` | `1025` | MailHog SMTP port (dev email preview) |
| `MAILHOG_UI_PORT` | `8025` | MailHog web UI port |
| `PGADMIN_PORT` | `5050` | pgAdmin web UI port |
| `PGADMIN_EMAIL` | `admin@magnetite.local` | pgAdmin login email |
| `PGADMIN_PASSWORD` | `admin_password` | pgAdmin login password — **change this** |

---

## Variable priority

1. Runtime environment variables (highest)
2. `.env` file
3. Hardcoded defaults in code (lowest)
