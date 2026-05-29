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

Required for the build pipeline (push webhooks + build status reporting).

| Variable | Description |
|----------|-------------|
| `GITHUB_APP_ID` | GitHub App numeric ID |
| `GITHUB_APP_PRIVATE_KEY` | PEM-encoded RSA private key (RS256) |
| `GITHUB_WEBHOOK_SECRET` | Secret used to verify `X-Hub-Signature-256` |

---

## Payments

| Variable | Description |
|----------|-------------|
| `CIRCLE_API_KEY` | Circle API key for USDC payments |
| `PAYSTACK_SECRET_KEY` | Paystack secret key for fiat on-ramp (Africa) |

---

## Email

Set `EMAIL_PROVIDER` to choose a backend. Leave all three empty to disable outbound email.

| Variable | Default | Description |
|----------|---------|-------------|
| `EMAIL_PROVIDER` | `resend` | `resend`, `smtp`, or `ses` |
| `EMAIL_FROM_ADDRESS` | `noreply@example.com` | Sender address |
| `EMAIL_FROM_NAME` | `Magnetite` | Sender display name |

**Resend:**

| Variable | Description |
|----------|-------------|
| `RESEND_API_KEY` | Resend API key |

**SMTP:**

| Variable | Default | Description |
|----------|---------|-------------|
| `SMTP_HOST` | — | SMTP hostname |
| `SMTP_USERNAME` | — | SMTP user |
| `SMTP_PASSWORD` | — | SMTP password |
| `SMTP_PORT` | `587` | SMTP port |

**AWS SES:**

| Variable | Description |
|----------|-------------|
| `AWS_REGION` | AWS region (e.g. `us-east-1`) |
| `AWS_ACCESS_KEY_ID` | IAM access key |
| `AWS_SECRET_ACCESS_KEY` | IAM secret key |
| `AWS_SES_FROM_ARN` | Verified SES sender ARN |

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
