# Self-Hosting Guide

Deploy your own Magnetite instance on any Linux server or cloud provider.

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

| Guide | Description |
|-------|-------------|
| [Quickstart](./quickstart.md) | Full stack up in three commands |
| [Docker Deployment](./docker.md) | Complete Compose reference with backups |
| [Environment Variables](./environment-variables.md) | All configuration options |
| [Database](./database.md) | PostgreSQL setup, migrations, backups |
| [SSL/TLS](./ssl.md) | Let's Encrypt and HTTPS configuration |
| [Fly.io](./fly-io.md) | Deploy to Fly.io with autoscaling |
| [Monitoring](./monitoring.md) | Logging, health probes, metrics |
| [Updating](./updating.md) | Upgrade procedures and rollback |

---

## Quickstart (three commands)

```bash
git clone https://github.com/magnetite-platform/magnetite.git
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

See [Environment Variables](./environment-variables.md) for the full list.

---

## Security checklist

- [ ] Set a unique `JWT_SECRET` (at least 32 random bytes)
- [ ] Set a strong `POSTGRES_PASSWORD`
- [ ] Restrict inbound traffic to ports 80 and 443 only
- [ ] Enable TLS — see [SSL/TLS](./ssl.md)
- [ ] Set `CORS_ALLOWED_ORIGINS` to your production domain
- [ ] Remove or firewall pgAdmin and MailHog in production
- [ ] Configure regular database backups — see [Database](./database.md)
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

See [Fly.io](./fly-io.md) for the full guide.

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
