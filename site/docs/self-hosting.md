<style>
/* magnetite type: the docs shell exposes --doc-font/--doc-display-font from the
   manifest but not the mono stack, so the product's mono is set here — it drives
   code blocks, inline code and every figure label. */
.dv{--doc-mono:'IBM Plex Mono',ui-monospace,SFMono-Regular,'SF Mono',Menlo,Consolas,monospace;
     --mg-bnd:#C4006B;--mg-live:#17803D;--mg-spec:#A45B00}
:root[data-theme="dark"] .dv{--mg-bnd:#FF74B2;--mg-live:#6EE79B;--mg-spec:#FFC24D}
</style>

# Self-hosting the legacy backend

> **This is a different binary from the `magnetite` game node.** To host a
> deterministic, replay-verified game with no database at all, see
> [Getting started](#getting-started) (`magnetite dev`) and
> [Hosting a server](#hosting-a-server) (`magnetite serve` / `magnetite
> node`) — that path needs nothing below. This page covers the **pre-redesign
> REST backend and React marketplace frontend** (accounts, social features,
> the developer marketplace) that still lives in `backend/` and `src/` and is
> deployable with the included `docker-compose.yml` — it is the legacy
> surface the redesign is moving away from, not the decentralized node.

Deploy the legacy Magnetite backend + frontend stack on any Linux server or
cloud provider you already have. There is no central Magnetite cloud and
nothing to sign up for — this is a self-hosted instance, same as any other.

The backend requires PostgreSQL and Redis. Everything else — email, OAuth
providers, a media server, external comms providers, a real chain rail — is
optional, and the default configuration needs no third-party account at all.

## Architecture

```
Internet
    │  HTTPS 443
    ▼
Nginx reverse proxy (nginx.conf)
    │
    ├─► Frontend  (React SPA, port 80 internal)
    │
    └─► Backend   (Axum, port 8080 internal)
            │
            ├─► PostgreSQL 16  (port 5432)
            └─► Redis 7        (port 6379)
```

All services are containerised. The included `docker-compose.yml` runs everything
including dev-only extras (MailHog email preview, pgAdmin).

---

## Prerequisites

| Component | Version | Notes |
|-----------|---------|-------|
| Docker | 24.0+ | Container runtime |
| Docker Compose | 2.20+ | Orchestration |
| A domain name | — | Required for TLS in production |
| 2 GB RAM min | — | Recommended 4 GB for comfortable headroom |

---

## Guides

Extended guides for each of these live under `docs/self-hosting/` in the
repository checkout (not reproduced here — this page covers the essentials):

| Guide | Repo path | Description |
|-------|-----------|-------------|
| Quickstart | `docs/self-hosting/quickstart.md` | Full stack up in three commands |
| Docker Deployment | `docs/self-hosting/docker.md` | Complete Compose reference with backups |
| Environment Variables | `docs/self-hosting/environment-variables.md` | All configuration options |
| External Dependencies | `docs/self-hosting/external-dependencies.md` | What is required (Postgres, Redis) vs optional (email, OAuth, MediaMTX, external comms providers) |
| Database | `docs/self-hosting/database.md` | PostgreSQL setup, migrations, backups |
| SSL/TLS | `docs/self-hosting/ssl.md` | Let's Encrypt and HTTPS configuration |
| Fly.io | `docs/self-hosting/fly-io.md` | Deploy to Fly.io with autoscaling |
| Monitoring | `docs/self-hosting/monitoring.md` | Logging, health probes, metrics |
| Updating | `docs/self-hosting/updating.md` | Upgrade procedures and rollback |

---

## Quickstart (three commands)

```bash
git clone https://github.com/vul-os/magnetite.git
cd magnetite
cp .env.example .env        # edit JWT_SECRET and database passwords
docker compose up -d
```

Verify the stack is healthy:

```bash
curl http://localhost:8080/health/ready
# → {"status":"success","data":{"database":"ok","redis":"ok"}}

curl http://localhost:3000
# → HTML page (frontend)
```

---

## Service ports (default)

| Service | External port | Environment variable |
|---------|--------------|---------------------|
| Backend API | `8080` | `BACKEND_PORT` |
| Frontend | `3000` | `FRONTEND_PORT` |
| PostgreSQL | `5432` | `POSTGRES_PORT` |
| Redis | `6379` | `REDIS_PORT` |
| MailHog SMTP | `1025` | `MAILHOG_SMTP_PORT` |
| MailHog UI | `8025` | `MAILHOG_UI_PORT` |
| pgAdmin | `5050` | `PGADMIN_PORT` |

In production, expose only ports 80/443 through a reverse proxy. Keep PostgreSQL,
Redis, MailHog, and pgAdmin on internal networks only.

---

## Minimal required environment variables

```bash
# .env
DATABASE_URL=postgres://magnetite:CHANGE_ME@postgres:5432/magnetite
POSTGRES_PASSWORD=CHANGE_ME
JWT_SECRET=<openssl rand -hex 32>
```

See `docs/self-hosting/environment-variables.md` in the repo for the full list.

---

## Security checklist

- [ ] Set a unique `JWT_SECRET` (at least 32 random bytes)
- [ ] Set a strong `POSTGRES_PASSWORD`
- [ ] Restrict inbound traffic to ports 80 and 443 only
- [ ] Enable TLS — see `docs/self-hosting/ssl.md`
- [ ] Set `CORS_ALLOWED_ORIGINS` to your production domain
- [ ] Remove or firewall pgAdmin and MailHog in production
- [ ] Configure regular database backups — see `docs/self-hosting/database.md`
- [ ] Set `RUST_LOG=info` (not `debug`) in production to reduce log volume

---

## Fly.io (managed)

```bash
fly launch                              # creates fly.toml (already present in repo)
fly secrets set JWT_SECRET=$(openssl rand -hex 32)
fly postgres create                     # attach managed Postgres
fly redis create                        # attach Upstash Redis
fly deploy
```

See `docs/self-hosting/fly-io.md` for the full guide.

---

## Manual (no Docker)

Build the frontend and backend separately:

```bash
# Frontend
npm install && npm run build
# Output: dist/

# Backend
cd backend && cargo build --release
# Output: target/release/magnetite-backend

# Run migrations
cd backend && sqlx migrate run

# Start server (requires DATABASE_URL and JWT_SECRET in env)
./backend/target/release/magnetite-backend
```

Serve `dist/` with any static file server or reverse proxy.
