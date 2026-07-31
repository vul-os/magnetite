# Quickstart Guide

Deploy Magnetite's pre-decentralization backend/frontend in minutes using
Docker Compose.

> **Corrected 2026-07-31.** This page previously referenced `magnetite/app`,
> `magnetite/backend:latest` and `magnetite/frontend:latest` as pre-built
> images to `docker run`/`docker pull`. **No such images are published
> anywhere** — `.github/workflows/deploy.yml` deploys straight to Fly.io via
> `fly deploy`, and no workflow in this repo pushes to Docker Hub or GHCR. The
> only way to run this stack today is to build the images yourself from the
> `Dockerfile.backend` / `Dockerfile.frontend` in this repo, which is exactly
> what the real, tracked [`docker-compose.yml`](../../docker-compose.yml) at
> the repo root already does. This page now matches it.

## Docker-Compose Setup (build from source)

From the repo root, using the tracked `docker-compose.yml` directly:

```bash
git clone https://github.com/vul-os/magnetite.git
cd magnetite
cp .env.example .env   # fill in DATABASE_URL / JWT_SECRET / etc — see environment-variables.md

docker compose up -d --build
```

That builds `backend` from [`Dockerfile.backend`](../../Dockerfile.backend)
and `frontend` from [`Dockerfile.frontend`](../../Dockerfile.frontend)
(`context: .` in both cases — see `docker-compose.yml`), and brings up
`postgres` (`postgres:16-alpine`) and `redis` (`redis:7-alpine`) alongside
them. `mediamtx` is an **opt-in profile**, not part of the default stack —
add `--profile media` if you want it.

If you want a *minimal* compose file of your own rather than the repo's, keep
the same shape — `build:` context, not an `image:` pull:

```yaml
services:
  backend:
    build:
      context: .
      dockerfile: Dockerfile.backend
    restart: unless-stopped
    ports:
      - "8080:8080"
    environment:
      DATABASE_URL: postgresql://magnetite:password@postgres:5432/magnetite
      JWT_SECRET: your-secure-jwt-secret-min-32-chars
      SERVER_HOST: 0.0.0.0
      SERVER_PORT: 8080
      RUST_LOG: info
    depends_on:
      postgres:
        condition: service_healthy
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  frontend:
    build:
      context: .
      dockerfile: Dockerfile.frontend
    restart: unless-stopped
    ports:
      - "80:80"
    depends_on:
      - backend

  postgres:
    image: postgres:16-alpine
    restart: unless-stopped
    environment:
      POSTGRES_USER: magnetite
      POSTGRES_PASSWORD: password
      POSTGRES_DB: magnetite
    volumes:
      - postgres_data:/var/lib/postgresql/data
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U magnetite"]
      interval: 10s
      timeout: 5s
      retries: 5

  redis:
    image: redis:7-alpine
    restart: unless-stopped
    volumes:
      - redis_data:/data
    command: redis-server --appendonly yes

volumes:
  postgres_data:
  redis_data:
```

## Initial Configuration

### 1. Database migrations run automatically

There is no `migrate` subcommand and no manual per-file loop to run: the
backend calls `sqlx::migrate!("./migrations").run(pool)` at startup
(`backend/src/db/pool.rs`), so every `docker compose up` (or plain `cargo run`)
applies any migration not yet recorded, in order, before the server starts
accepting traffic. If it fails, the process exits — check `docker compose
logs backend`.

### 2. First admin user

There is no `create-admin` CLI. `is_admin` is a plain boolean column on
`users`, and the only endpoint that flips it (`PATCH` in
`backend/src/api/admin.rs`) itself requires an existing admin — so the very
first admin has to be set directly against the database:

```bash
# 1. Register a normal account through the frontend or POST /auth/register.
# 2. Promote it directly (one-time, chicken-and-egg bootstrap):
docker compose exec postgres psql -U magnetite -d magnetite \
  -c "UPDATE users SET is_admin = true WHERE email = 'admin@example.com';"
```

### 3. Verify Deployment

- Frontend: http://localhost
- Backend API: http://localhost:8080
- Health check: http://localhost:8080/health

## Next Steps

- [Configure environment variables](./environment-variables.md)
- [Set up SSL certificates](./ssl.md)
- [Configure database backups](./database.md)
- [Set up monitoring](./monitoring.md)

## Troubleshooting

### Container Won't Start

Check logs:
```bash
docker compose logs backend
```

### Database Connection Failed

Ensure PostgreSQL is healthy:
```bash
docker compose ps postgres
docker compose logs postgres
```

### Migration Errors

Migrations are applied automatically on backend startup — a failed migration
shows up in the backend's own logs, not a separate migration container:

```bash
docker compose logs backend | grep -i migrat
```
