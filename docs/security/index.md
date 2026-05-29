# Security & Sandboxing

This page documents Magnetite's security model: authentication, game sandboxing,
anti-cheat, and hardening recommendations for production deployments.

---

## Authentication model

Magnetite uses a two-token JWT scheme:

| Token | Lifetime | Storage | Purpose |
|-------|----------|---------|---------|
| Access token | 15 min (configurable via `ACCESS_TOKEN_EXPIRY`) | Memory / `Authorization` header | API request authentication |
| Refresh token | 7 days (configurable via `REFRESH_TOKEN_EXPIRY`) | HttpOnly cookie or secure storage | Obtain new access token |

**Token issuance** (`POST /api/v1/auth/login`, `/register`):

```json
{
  "access_token": "eyJ…",
  "refresh_token": "eyJ…",
  "expires_at": "2026-05-30T14:15:00Z",
  "user_id": "…"
}
```

**Refresh** (`POST /api/v1/auth/refresh`):

Clients exchange a valid refresh token for a new token pair. Old refresh tokens are
rotated (single-use). All sessions for a user can be revoked with
`DELETE /api/v1/auth/logout-all`.

### Password storage

Passwords are hashed with **bcrypt** (configured cost factor) in `services/auth.rs`.
Plaintext passwords are never stored or logged.

### Role-based access

| Role | Capabilities |
|------|-------------|
| `user` | Play games, manage own wallet/profile/social |
| `developer` | All of `user` + manage own games, view earnings, request payouts |
| `admin` | All routes including `admin::*`, user moderation, financial operations |

Admin routes enforce `middleware::admin_middleware` (role check after JWT validation).

---

## Game sandboxing

**Server-authoritative design.** Game logic runs server-side only. Clients send inputs
(key states, mouse deltas) — never position or score updates directly. The server calls
`GameLogic::tick` and broadcasts the authoritative `GameState` to all clients.

This means:

- Clients cannot set their own positions or scores.
- All physics/collision/scoring happens in Rust server-side code.
- WASM artifacts loaded in the browser are the rendering and input layer only.

### WASM artifact integrity

When a game version is promoted to live, its `sha256_hash` is stored in the
`distribution` table. The browser can verify the downloaded `_bg.wasm` against this
hash before instantiation:

```javascript
const resp = await fetch(manifest.wasm_url);
const buf = await resp.arrayBuffer();
const digest = await crypto.subtle.digest('SHA-256', buf);
// compare hex(digest) to manifest.sha256_hash
```

### Anti-cheat service (`services/anticheat.rs`)

The anti-cheat service analyses submitted session data for anomalies:

| Check | What it detects |
|-------|----------------|
| `VelocityViolation` | Player position delta exceeds physics limits |
| `ScoreAnomaly` | Score increase inconsistent with session duration or game rules |
| `InputFrequency` | Input rate far exceeds human limits (bot detection) |
| `SessionIntegrity` | Session data hash mismatch (replay tampering) |

Anomalies are recorded with `severity` (low / medium / high / critical) and
`session_id`. High and critical anomalies trigger platform review and may result in
session invalidation or account suspension.

> **Note:** The anti-cheat service is implemented and its types are wired to the session
> data model, but deep integration with the live session websocket loop is planned for
> a future release.

---

## Network security

### Rate limiting

All API routes go through a Redis-backed sliding-window rate limiter
(`middleware/rate_limit.rs`). Configure the limit via `RateLimitConfig` (default: 100
requests / 60 s per IP). HTTP 429 is returned with a `Retry-After` header.

### CORS

The `cors_layer()` middleware restricts requests to origins in `CORS_ALLOWED_ORIGINS`.
In production set this to your exact frontend domain — never `*`.

### Webhook signature verification

**GitHub** webhooks are verified with `HMAC-SHA256` against `GITHUB_WEBHOOK_SECRET`
using the `X-Hub-Signature-256` header.

**Paystack / Circle** webhooks are verified with provider-specific HMAC signatures.
Do not process webhook events without verifying the signature first.

---

## Production hardening checklist

**Secrets**

- [ ] `JWT_SECRET` is at least 32 random bytes (`openssl rand -hex 32`)
- [ ] `POSTGRES_PASSWORD` is unique and not the default
- [ ] GitHub App private key is stored as an environment variable, not a file in the repo
- [ ] Payment API keys are in environment variables only — never committed

**Network**

- [ ] PostgreSQL and Redis are not exposed on public interfaces
- [ ] pgAdmin and MailHog are disabled or firewalled in production
- [ ] TLS 1.2+ is enforced on all public endpoints (see [SSL/TLS](../self-hosting/ssl.md))
- [ ] `CORS_ALLOWED_ORIGINS` is set to your exact production domain

**Container**

- [ ] Backend container runs as a non-root user (see `Dockerfile.backend`)
- [ ] Container filesystem is read-only where possible
- [ ] Images pin to specific digest tags, not `latest`

**Database**

- [ ] Application DB user has only `SELECT / INSERT / UPDATE / DELETE` on application tables
- [ ] Database SSL connections enabled (`?sslmode=require` in `DATABASE_URL`)
- [ ] Automated backups configured (see [Database](../self-hosting/database.md))

**Monitoring**

- [ ] `RUST_LOG=info` in production (not `debug`)
- [ ] Health probes configured: `GET /health/ready`, `GET /health/live`
- [ ] Prometheus scraping `GET /metrics`
- [ ] Alerts on high error rate (`error_rate > 5%`) and slow responses (`p99 > 2 s`)

---

## Secret rotation

```bash
# Rotate JWT secret (causes all sessions to expire — users must re-login)
fly secrets set JWT_SECRET=$(openssl rand -hex 32)
# or update .env and restart containers

# Rotate database password
docker exec magnetite-postgres-1 psql -U postgres \
  -c "ALTER USER magnetite WITH PASSWORD 'new_password';"
# Update DATABASE_URL in .env / secrets manager, restart backend
```

---

## Database-level access control

Create a least-privilege application user:

```sql
CREATE USER magnetite_app WITH PASSWORD 'secure_password';
GRANT CONNECT ON DATABASE magnetite TO magnetite_app;
GRANT USAGE ON SCHEMA public TO magnetite_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO magnetite_app;
GRANT USAGE, SELECT ON ALL SEQUENCES IN SCHEMA public TO magnetite_app;
-- Revoke DDL:
REVOKE CREATE ON SCHEMA public FROM magnetite_app;
```

---

## TLS configuration (Nginx)

```nginx
server {
    listen 443 ssl http2;
    ssl_certificate     /etc/ssl/certs/magnetite.crt;
    ssl_certificate_key /etc/ssl/private/magnetite.key;
    ssl_protocols       TLSv1.2 TLSv1.3;
    ssl_ciphers         ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384;
    ssl_prefer_server_ciphers on;
    add_header Strict-Transport-Security "max-age=63072000" always;
}
```

See [SSL/TLS](../self-hosting/ssl.md) for Let's Encrypt automation.

---

## Responsible disclosure

Found a security issue? Email **security@magnetite.gg** (placeholder — update with your
actual address). Please do not open a public GitHub issue for security vulnerabilities.
