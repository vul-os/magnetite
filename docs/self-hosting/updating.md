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

#### 1. Pull Latest Images

```bash
docker-compose pull
```

#### 2. Review Image Tags

```bash
# Backend
docker images | grep magnetite/backend

# Frontend
docker images | grep magnetite/frontend
```

#### 3. Run Migrations

```bash
# Check for new migrations
ls -la backend/migrations/

# Run migrations before updating containers
docker-compose run --rm backend /app/migrate.sh
```

#### 4. Stop Services Gracefully

```bash
docker-compose stop backend
```

#### 5. Update and Start

```bash
docker-compose up -d
```

#### 6. Verify

```bash
# Check health
curl http://localhost:8080/health

# Check logs
docker-compose logs -f backend
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

#### 3. Run Migrations

```bash
DATABASE_URL=$DATABASE_URL ./migrate.sh
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

```bash
# Single command update
docker-compose pull && docker-compose up -d

# Update specific service
docker-compose pull backend && docker-compose up -d backend

# Force rebuild
docker-compose build --no-cache backend
docker-compose up -d backend
```

### Kubernetes

```bash
# Update images
kubectl set image deployment/magnetite-backend backend=magnetite/backend:x.x.x
kubectl set image deployment/magnetite-frontend frontend=magnetite/frontend:x.x.x

# Check rollout
kubectl rollout status deployment/magnetite-backend
```

## Rollback Procedure

### Docker Rollback

#### 1. Identify Previous Image

```bash
# List available images
docker images | grep magnetite

# Use specific previous tag
docker-compose pull backend
docker tag magnetite/backend:previous magnetite/backend:latest
```

#### 2. Restore Database

```bash
# Stop services
docker-compose stop backend frontend

# Restore database
gunzip < backups/magnetite_pre_update_20250119_120000.sql.gz | docker-compose exec -T postgres psql -U magnetite -d magnetite
```

#### 3. Restart Services

```bash
docker-compose up -d
```

### Kubernetes Rollback

```bash
# Rollback to previous revision
kubectl rollout undo deployment/magnetite-backend
kubectl rollout undo deployment/magnetite-frontend

# Check status
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
curl -X POST http://localhost:8080/api/auth/login \
  -H "Content-Type: application/json" \
  -d '{"email":"test@example.com","password":"testpassword"}'

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
