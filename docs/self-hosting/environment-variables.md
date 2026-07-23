# Environment Variables

Copy `.env.example` to `.env` and fill in the required values before starting the stack.
Variables marked **required** will cause the backend to fail at startup if absent.

---

## Required

| Variable | Example | Description |
|----------|---------|-------------|
| `DATABASE_URL` | `postgres://magnetite:pw@postgres:5432/magnetite` | PostgreSQL connection string |
| `REDIS_URL` | `redis://redis:6379` | Redis connection URL |
| `JWT_SECRET` | `$(openssl rand -hex 32)` | JWT signing secret — minimum 32 characters, never commit |

PostgreSQL and Redis are the only external services the backend requires. Everything
below — email, OAuth, media, external comms providers, a real chain rail — is
optional.

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
| `TRUST_PROXY` | `false` | `true` trusts `X-Forwarded-For` when resolving the real client IP behind a reverse proxy — affects rate limiting, analytics, and the superadmin IP allowlist |
| `TRUSTED_PROXY_COUNT` | `1` | Number of trusted reverse-proxy hops in front of the node (used with `TRUST_PROXY`) |

---

## JWT

| Variable | Default | Description |
|----------|---------|-------------|
| `ACCESS_TOKEN_EXPIRY` | `900` | Access token lifetime in seconds (15 min) |
| `REFRESH_TOKEN_EXPIRY` | `604800` | Refresh token lifetime in seconds (7 days) |

---

## Superadmin panel

The operator console at `/superadmin`. It is **disabled unless both
`SUPERADMIN_EMAIL` and a password/hash are set** — with neither configured the
routes are never mounted.

| Variable | Default | Description |
|----------|---------|-------------|
| `SUPERADMIN_EMAIL` | — | Login email; required to enable the panel |
| `SUPERADMIN_PASSWORD_HASH` | — | Argon2 PHC-string credential (**preferred**); takes precedence over `SUPERADMIN_PASSWORD` |
| `SUPERADMIN_PASSWORD` | — | Plaintext credential fallback; logs a warning — use the hash in production |
| `SUPERADMIN_IP_ALLOWLIST` | — (off) | Comma-separated IPs / CIDRs; when set, only these may reach the panel. Unset means no IP restriction |
| `SUPERADMIN_SECURE_COOKIE` | `true` when `APP_ENV=production`, else `false` | Sets the `Secure` flag on the session cookie |
| `SUPERADMIN_SESSION_TTL_SECS` | `7200` | Session lifetime in seconds (2 h) |
| `SUPERADMIN_SESSION_BACKEND` | Redis if reachable, else in-memory | Set to `memory` to force in-memory session + lockout storage (ignores `REDIS_URL`) |

---

## OAuth providers

Optional. Leave unused providers blank — keypair and password login still work.
At least one is required for social login.

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

## Payments (non-custodial crypto)

There is no fiat and no custody. Buyers pay sellers wallet-to-wallet through the
`PaymentRail` seam; the platform holds no funds, so there is nothing to deposit,
withdraw or pay out. The signed receipt *is* the entitlement — the node re-verifies
the rail signature, so a database row alone never grants access. A refund voids the
receipt and revokes the entitlement rather than moving a balance.

All payment variables are optional; the defaults need no external service.

| Variable | Default | Description |
|----------|---------|-------------|
| `PAYMENT_RAIL` | `mock` | Settlement rail: `mock` or `solana`. `mock` issues deterministic signed receipts fully offline — this is what CI and `magnetite dev` use. `solana` selects the real SPL-USDC rail and requires building with `--features solana` (see the **Solana rail** section below); an unknown or not-compiled-in value is fatal at startup |
| `PROTOCOL_FEE_BPS` | `0` | Protocol fee in basis points, taken **on top of** the subtotal. The developer receives the whole subtotal |
| `OPERATOR_WALLET_PUBKEY` | — | Hex Ed25519 pubkey that receives hosting / paid-tier fees. Only needed if this node sells hosting or paid tiers |
| `CHAIN_RPC_URL` | — | Dormant generic-config field; the Solana rail below reads `SOLANA_RPC_URL` instead |
| `CHAIN_ID` | — | Dormant generic-config field, unused by both rails |
| `STABLECOIN_ADDRESS` | — | Dormant generic-config field, unused by both rails |

### Solana rail (`PAYMENT_RAIL=solana`, build with `--features solana`)

A real, non-custodial SPL-USDC settlement rail on Solana
(`magnetite-solana-rail` → `patala-solana`), off by default. When selected, the
node validates every field at startup and refuses to boot on a misconfiguration
(e.g. a mainnet cluster with `PROTOCOL_FEE_BPS > 0` but no fee wallet).

| Variable | Default | Description |
|----------|---------|-------------|
| `SOLANA_RPC_URL` | — | JSON-RPC endpoint (required); must be an `http(s)://` URL |
| `SOLANA_CLUSTER` | — | `mainnet-beta` \| `devnet` \| `testnet` \| `localnet` (required) |
| `SOLANA_COMMITMENT` | `finalized` | `confirmed` \| `finalized` |
| `SOLANA_USDC_MINT` | canonical mint for the cluster | base58 USDC mint address |
| `SOLANA_FEE_WALLET` | — | base58; **required when `PROTOCOL_FEE_BPS > 0`** |
| `SOLANA_KEYPAIR_PATH` / `SOLANA_KEYPAIR` | — | Optional signer (`chmod 600`); absent ⇒ the rail is verify-only |

---

## Comms

Magnetite builds no chat/voice/video/streaming of its own; `COMMS_PROVIDER` selects
which system serves it. A provider whose service is not configured falls back to
`builtin` with a warning.

| Variable | Default | Description |
|----------|---------|-------------|
| `COMMS_PROVIDER` | `builtin` | `builtin` \| `matrix` \| `jitsi` \| `livekit` \| `owncast`. `builtin` is the in-house stack and requires no external service |
| `NODE_SIGNING_SEED` | — | 64 hex chars (32 bytes). The node mints comms join credentials with this key. Unset — an ephemeral key is generated at boot and credentials do not survive a restart |
| `MATRIX_HOMESERVER` | — | Set to enable the Matrix provider |
| `MATRIX_SERVER_NAME` | — | Matrix server name |
| `MATRIX_ALIAS_PREFIX` | `magnetite` | Room alias prefix |
| `MATRIX_SHARED_SECRET` | — | Only needed if the homeserver trusts this node as a JWT/SSO identity provider |
| `JITSI_DOMAIN` | — | Set to enable the Jitsi provider |
| `JITSI_APP_ID` | `magnetite` | Jitsi JWT app ID (optional — an open deployment needs neither) |
| `JITSI_JWT_SECRET` | — | Jitsi JWT secret (optional) |
| `LIVEKIT_URL` | — | Set to enable the LiveKit provider |
| `LIVEKIT_API_KEY` | — | Without the key/secret pair no access token is minted |
| `LIVEKIT_API_SECRET` | — | See above |
| `OWNCAST_URL` | — | Set to enable the Owncast provider |
| `OWNCAST_STREAM_KEY` | — | Owncast stream key |

---

## Email

Set `EMAIL_PROVIDER` to choose a transport. Leave all credentials blank to disable
outbound email — it is optional (verification and notification emails will not be sent — a clear error
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
| `MEDIA_SERVER_BASE_URL` | — | Base URL of an external MediaMTX (or equivalent) media server (e.g. `http://mediamtx:8888`). The backend proxies `/streams/:id/hls.m3u8` to this URL. Empty by default; if unset, the watch endpoint returns HTTP 503. Optional — the backend has no dependency on a media server, and in Docker Compose MediaMTX sits behind the `media` profile. Media is per-operator: a stream/room record carries its own `media_host`, which always wins. |

---

## Game Server WebSocket

| Variable | Default | Description |
|----------|---------|-------------|
| `GAME_SERVER_WS_BASE` | `ws://localhost:8080` | WebSocket base URL used by matchmaking to set `server_endpoint` on new game sessions. In single-server dev mode this defaults to the backend's own host. **Bucket-D**: dedicated or auto-scaled game servers require a separate deployment and this URL. |

---

## Database backups

The backup job dumps the database on a schedule and stores each dump either on
local disk or in S3, selected by `BACKUP_STORAGE_TYPE` (an unknown value is a
hard error at backup time).

| Variable | Default | Description |
|----------|---------|-------------|
| `BACKUP_STORAGE_TYPE` | `local` | `local` or `s3` |
| `BACKUP_LOCAL_DIR` | `/var/lib/magnetite/backups` | Directory for `local` backups |
| `BACKUP_S3_BUCKET` | — | Destination bucket; **required** when `BACKUP_STORAGE_TYPE=s3` |
| `BACKUP_S3_REGION` | `us-east-1` | Bucket region for `s3` backups |

For `s3`, AWS credentials come from the standard AWS provider chain —
`AWS_ACCESS_KEY_ID` / `AWS_SECRET_ACCESS_KEY` in the environment, or an attached
IAM role. The bucket region is taken from `BACKUP_S3_REGION` (not `AWS_REGION`).

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
