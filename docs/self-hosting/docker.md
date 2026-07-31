# Docker Deployment

Deploy Magnetite's pre-decentralization backend/frontend using Docker Compose.

> **Corrected 2026-07-31.** This page previously described three services
> that do not exist in this repo (`api`, `game-host`, `websocket` as
> separately-pulled `magnetite/api`, `magnetite/game-host`,
> `magnetite/websocket` images), env vars this codebase never reads
> (`PROTOCOL_FEE_BPS` — deleted, see [payments.md](../payments.md);
> `S3_BUCKET`/AWS SES creds under the wrong names; `RATE_LIMIT_*`), and backup
> commands against container names Compose never assigns (`magnetite-db-1`
> — the real service is named `postgres`, not `db`). None of those specific
> names is ever published or built by any workflow in `.github/workflows/` —
> the one real published image is `magnetite/magnetite` (backend) /
> `magnetite/magnetite:vX.Y.Z-frontend`, pushed by `release.yml`'s `docker`
> job only on a tagged release; see [quickstart.md](./quickstart.md) for the
> exact conditions. This page now matches the actual, tracked
> [`docker-compose.yml`](../../docker-compose.yml)
> at the repo root.

## docker-compose.yml Reference

The real compose file builds two images from source (`Dockerfile.backend`,
`Dockerfile.frontend`) rather than pulling published ones, and has exactly
four required services plus one opt-in profile:

```yaml
services:
  backend:
    build: { context: ., dockerfile: Dockerfile.backend }
    ports: ["${BACKEND_PORT:-8080}:8080"]
    environment:
      DATABASE_URL: postgres://postgres:postgres@postgres:5432/magnetite
      REDIS_URL: redis://redis:6379
      JWT_SECRET: ${JWT_SECRET}
    depends_on: [postgres, redis]

  frontend:
    build: { context: ., dockerfile: Dockerfile.frontend }
    ports: ["${FRONTEND_PORT:-3000}:80"]
    depends_on: [backend]

  postgres:
    image: postgres:16-alpine
    environment:
      POSTGRES_DB: magnetite
      POSTGRES_USER: postgres
      POSTGRES_PASSWORD: postgres
    volumes: ["postgres_data:/var/lib/postgresql/data"]

  redis:
    image: redis:7-alpine
    command: redis-server --appendonly yes
    volumes: ["redis_data:/data"]

  # Opt-in only — `docker compose --profile media up`. Not a dependency of
  # anything above.
  mediamtx:
    image: bluenviron/mediamtx:latest
    profiles: [media]

volumes:
  postgres_data:
  redis_data:
```

This is a condensed view — see the tracked `docker-compose.yml` for the exact,
current version (health checks, port variables, volumes).

## Environment Variables

The full reference lives in [environment-variables.md](./environment-variables.md)
and [`.env.example`](../../.env.example) at the repo root — copy the latter to
`.env` and fill it in; `docker compose` reads it automatically. The
payment-related variables that actually exist are:

```bash
# Non-custodial crypto. `mock` (default) is fully offline and holds no funds.
PAYMENT_RAIL=mock
# Only meaningful for the (unwired-into-this-repo) real chain rails:
CHAIN_RPC_URL=
CHAIN_ID=
STABLECOIN_ADDRESS=
# Only if this node sells hosting or paid tiers:
OPERATOR_WALLET_PUBKEY=
```

There is **no** `PROTOCOL_FEE_BPS` — it was deleted along with the platform-fee
model; see [Payments](../payments.md#there-is-no-protocol-fee). There is no
`S3_BUCKET`/`RATE_LIMIT_*` in this codebase today.

## Backup and Restore

Service names in the real compose file are `postgres` and `redis`, not `db`
and container-suffixed names — use `docker compose exec <service>`, not
`docker exec <project>-<service>-1` (the latter is fragile across Compose
versions and project-name overrides):

```bash
# Database backup
docker compose exec postgres pg_dump -U postgres magnetite > backup.sql

# Database restore
docker compose exec -T postgres psql -U postgres magnetite < backup.sql

# Redis snapshot
docker compose exec redis redis-cli BGSAVE
docker compose cp redis:/data/dump.rdb ./redis_backup.rdb
```

```bash
#!/bin/bash
# backup.sh
DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="./backups"
mkdir -p "$BACKUP_DIR"

docker compose exec postgres pg_dump -U postgres magnetite > "$BACKUP_DIR/db_$DATE.sql"
docker compose exec redis redis-cli BGSAVE
cp .env "$BACKUP_DIR/env_$DATE.bak"
echo "Backup complete: $BACKUP_DIR"
```

## Updating

There is no published image to `docker compose pull` — rebuild from source:

```bash
git pull
docker compose up -d --build
docker compose logs -f backend
```

## Health Checks

```bash
curl http://localhost:8080/health
```

There is no separate `game-host` or `websocket` service or health endpoint in
this stack — WebSocket traffic is served by the same `backend` process on the
same port. The standalone `magnetite-runtime` node (the decentralized-redesign
authoritative server, not part of this compose file) is documented separately
in [Hosting a server](../hosting-a-server.md).

## Resource Limits

```yaml
services:
  backend:
    deploy:
      resources:
        limits: { cpus: '2', memory: 2G }
        reservations: { cpus: '1', memory: 1G }
```
