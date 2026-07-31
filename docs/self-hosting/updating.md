# Updating Guide

Procedures for updating Magnetite with minimal downtime and safe rollbacks.

## Pre-Update Checklist

- [ ] Read the [CHANGELOG](../../CHANGELOG.md)
- [ ] Review breaking changes
- [ ] Backup database
- [ ] Test in staging environment
- [ ] Schedule maintenance window
- [ ] Notify users of downtime

## Backup Before Update

### Database Backup

```bash
# Create timestamped backup
BACKUP_DIR="./backups"
DATE=$(date +%Y%m%d_%H%M%S)
DATABASE_URL=${DATABASE_URL:-postgresql://magnetite:password@localhost:5432/magnetite}

mkdir -p "$BACKUP_DIR"
pg_dump "$DATABASE_URL" | gzip > "$BACKUP_DIR/magnetite_pre_update_$DATE.sql.gz"

# Verify backup
zcat "$BACKUP_DIR/magnetite_pre_update_$DATE.sql.gz" | head -5

# List recent backups
ls -la "$BACKUP_DIR" | tail -5
```

### Volume Backup (Docker)

```bash
# Backup all volumes
docker run --rm \
  -v magnetite_postgres_data:/var/lib/postgresql/data \
  -v magnetite_redis_data:/data \
  -v $(pwd)/backups:/backups \
  alpine \
  tar czf /backups/volumes_pre_update_$(date +%Y%m%d_%H%M%S).tar.gz \
  /var/lib/postgresql/data /data
```

### Configuration Backup

```bash
# Backup .env and configs
cp .env .env.backup.$(date +%Y%m%d)
cp docker-compose.yml docker-compose.yml.backup.$(date +%Y%m%d)
cp -r nginx.conf nginx.conf.backup.$(date +%Y%m%d)
```

## Update Procedures

### Docker Update

> **Corrected 2026-07-31.** No `magnetite/backend`/`magnetite/frontend` image
> is published under those names (the one real published image is
> `magnetite/magnetite`/`magnetite/magnetite:vX.Y.Z-frontend`, pushed only on
> a tagged release by `release.yml`'s `docker` job — see
> [docker.md](./docker.md)/[quickstart.md](./quickstart.md)). Compose-driven
> local development still has nothing to `pull` — it builds locally, both
> before and after a release exists. The runtime image also does not contain
> `backend/tools/migrate.sh` — `Dockerfile.backend` only copies the compiled
> `magnetite-backend` binary — and there is no `/app/migrate.sh` inside the
> container to run.

#### 1. Pull latest source and rebuild

```bash
git pull origin main
docker compose up -d --build
```

#### 2. Migrations run automatically

The backend calls `sqlx::migrate!("./migrations").run(pool)` at startup
(`backend/src/db/pool.rs`) — any migration not yet recorded applies before the
server accepts traffic. There is nothing separate to run; if a migration
fails, `backend` exits and `docker compose logs backend` shows why.

#### 3. Verify

```bash
curl http://localhost:8080/health
docker compose logs -f backend
```

### Native Update (Rust Backend)

#### 1. Stop Service

```bash
# Stop current service
sudo systemctl stop magnetite

# Or kill process
pkill -f magnetite-backend
```

#### 2. Download/Build New Version

```bash
cd backend

# Pull latest code
git fetch origin
git checkout tags/vx.x.x -b vx.x.x

# Build
cargo build --release
```

#### 3. Migrations

Migrations run automatically when `magnetite-backend` starts
(`sqlx::migrate!` in `backend/src/db/pool.rs`) — nothing to run separately.
`backend/tools/migrate.sh` (note the path: `backend/tools/`, not
`backend/migrate.sh`) is a standalone operator script for manual
up/down/status/reset if you need finer control than "apply everything on
boot":

```bash
DATABASE_URL=$DATABASE_URL ./backend/tools/migrate.sh status
```

#### 4. Start Service

```bash
sudo systemctl start magnetite
```

### Fly.io Update

```bash
# Pull latest code
git pull origin main

# Deploy
fly deploy

# Check status
fly status
fly logs
```

## Update Commands Reference

### Docker Compose

There is no image to `pull` — rebuild from source (see the correction note
above):

```bash
# Single command update
git pull && docker compose up -d --build

# Force a clean rebuild of one service
docker compose build --no-cache backend
docker compose up -d backend
```

### Kubernetes

`deploy/k8s/` (see [deploy.md](./deploy.md)) references `ghcr.io/magnetite/*`
image names, but **no workflow in this repo builds or pushes them** — that is
a gap in the manifests, not a published registry you can pull from. Build and
push to your own registry first, then point the deployment at it:

```bash
docker build -t your-registry.example/magnetite-backend:x.x.x -f Dockerfile.backend .
docker push your-registry.example/magnetite-backend:x.x.x
kubectl set image deployment/magnetite-backend backend=your-registry.example/magnetite-backend:x.x.x
kubectl rollout status deployment/magnetite-backend
```

## Rollback Procedure

### Docker Rollback

There is no previous image tag to fall back to (nothing is published) — roll
back the source and rebuild:

```bash
git checkout tags/vPREVIOUS
docker compose up -d --build
```

Restore the database separately if the rolled-back version's schema differs:

```bash
docker compose stop backend frontend
gunzip < backups/magnetite_pre_update_20250119_120000.sql.gz | docker compose exec -T postgres psql -U postgres -d magnetite
docker compose up -d
```

### Kubernetes Rollback

Only meaningful once you are pushing your own tags (see above — this repo
does not publish any):

```bash
kubectl rollout undo deployment/magnetite-backend
kubectl rollout undo deployment/magnetite-frontend
kubectl rollout status deployment/magnetite-backend
```

### Fly.io Rollback

```bash
# List releases
fly releases

# Rollback to specific version
fly deploy --image <previous-image>

# Or rollback to previous release
fly releases undo
```

## Database Rollback

### Point-in-Time Recovery

If you need to restore to a specific point:

```bash
# Stop database writes
docker-compose stop backend

# Restore to specific timestamp
docker-compose exec postgres psql -U magnetite -d magnetite -c "
  SELECT pg_restore_to_point('2025-01-19 12:00:00 UTC');
"
```

### Restore Specific Tables

```bash
# Export specific table before update
pg_dump --table=users --data-only $DATABASE_URL > users_data.sql

# If needed after update, restore
psql $DATABASE_URL < users_data.sql
```

## Verifying Update Success

### Health Check

```bash
# Backend API
curl -f http://localhost:8080/health

# Frontend
curl -f http://localhost/health
```

### Functional Tests

```bash
# Test login
curl -X POST http://localhost:8080/api/v1/auth/login \
  -H "Content-Type: application/json" \
  -d '{"username":"testuser","password":"testpassword"}'

# Test database connection
docker-compose exec backend psql "$DATABASE_URL" -c "SELECT 1"
```

### Log Verification

```bash
# Check for errors
docker-compose logs --tail=100 backend | grep -i error

# Check for warnings
docker-compose logs --tail=100 backend | grep -i warn
```

## Post-Update Tasks

### 1. Verify Migrations

```bash
docker-compose exec postgres psql -U magnetite -d magnetite -c "SELECT * FROM _migrations ORDER BY executed_at DESC LIMIT 5;"
```

### 2. Clear Caches

```bash
# Redis cache clear
docker-compose exec redis redis-cli FLUSHALL

# Or restart cache service
docker-compose restart redis
```

### 3. Update Dependencies

```bash
# Frontend
npm install

# Rust dependencies
cd backend && cargo update
```

### 4. Monitor Error Rates

Watch logs for 30 minutes after update:

```bash
docker-compose logs -f backend | grep -i error
```

## Troubleshooting Update Issues

### Container Stuck in Restart Loop

```bash
# Check logs
docker-compose logs backend

# View exit code
docker-compose ps

# Shell into container
docker-compose exec backend /bin/sh
```

### Migration Fails

```bash
# Check migration status
docker-compose exec postgres psql -U magnetite -d magnetite -c "SELECT * FROM _migrations;"

# Run failed migration manually
docker-compose exec postgres psql -U magnetite -d magnetite -f /migrations/failed_migration.sql
```

### Database Connection Lost

```bash
# Check PostgreSQL status
docker-compose ps postgres
docker-compose logs postgres

# Verify connection string
docker-compose exec backend env | grep DATABASE
```

## Version-Specific Updates

### v0.1.x to v0.2.x

```bash
# Breaking changes in v0.2.0:
# - New JWT_SECRET format (must be 32+ chars)
# - DATABASE_URL now requires sslmode

# Update .env
echo "DATABASE_URL=postgresql://user:pass@host:5432/db?sslmode=require" >> .env

# Run migration
docker-compose exec backend /app/migrate.sh
```

### v0.2.x to v0.3.x

```bash
# Breaking changes in v0.3.0:
# - REDIS_URL now required
# - New environment variables added

# Add to .env
echo "REDIS_URL=redis://redis:6379" >> .env

# Restart services
docker-compose restart
```

## Maintenance Windows

For major updates, schedule maintenance:

```bash
# Create maintenance page
cat > public/maintenance.html << 'EOF'
<!DOCTYPE html>
<html>
<head>
    <title>Maintenance</title>
</head>
<body>
    <h1>Scheduled Maintenance</h1>
    <p>We'll be back shortly.</p>
</body>
</html>
EOF

# Deploy maintenance page
docker-compose exec frontend cp /app/maintenance.html /usr/share/nginx/html/index.html
```
