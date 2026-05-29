# Database Guide

PostgreSQL setup, migrations, and backup strategies for Magnetite.

## PostgreSQL Setup

### System Requirements

- PostgreSQL 15+ recommended
- Minimum 1GB RAM
- 10GB+ storage for production

### Docker Deployment

```bash
docker run -d \
  --name magnetite-postgres \
  -e POSTGRES_USER=magnetite \
  -e POSTGRES_PASSWORD=your-secure-password \
  -e POSTGRES_DB=magnetite \
  -v postgres_data:/var/lib/postgresql/data \
  -p 5432:5432 \
  postgres:16-alpine
```

### Manual Installation

```bash
# Ubuntu/Debian
sudo apt update
sudo apt install postgresql postgresql-contrib

# macOS
brew install postgresql@16
brew services start postgresql@16
```

### Initial Setup

```bash
# Connect as postgres user
sudo -u postgres psql

# Create user and database
CREATE USER magnetite WITH PASSWORD 'your-secure-password';
CREATE DATABASE magnetite OWNER magnetite;
GRANT ALL PRIVILEGES ON DATABASE magnetite TO magnetite;

# Exit psql
\q
```

## Migrations

### Migration Files Location

Migrations are in `backend/migrations/`:

```
backend/migrations/
├── 20250119_abc_initial_schema.sql
├── 20250120_sessions.sql
└── 20250120_add_admin_fields.sql
```

### Run Migrations

#### Docker

```bash
docker-compose exec postgres psql -U magnetite -d magnetite -f /migrations/20250119_abc_initial_schema.sql
```

#### Native psql

```bash
psql -U magnetite -d magnetite -f backend/migrations/20250119_abc_initial_schema.sql
psql -U magnetite -d magnetite -f backend/migrations/20250120_sessions.sql
psql -U magnetite -d magnetite -f backend/migrations/20250120_add_admin_fields.sql
```

#### Automatic Migration Script

Create `migrate.sh`:

```bash
#!/bin/bash
set -e

DATABASE_URL=${DATABASE_URL:-postgresql://magnetite:password@localhost:5432/magnetite}

for migration in backend/migrations/*.sql; do
    echo "Running migration: $migration"
    psql "$DATABASE_URL" -f "$migration"
done

echo "All migrations completed"
```

### Migration Tracking

Magnetite tracks migrations in the `_migrations` table:

```sql
CREATE TABLE IF NOT EXISTS _migrations (
    id SERIAL PRIMARY KEY,
    name TEXT NOT NULL UNIQUE,
    executed_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
```

## Backup Strategy

### Automated Daily Backups

Create `backup.sh`:

```bash
#!/bin/bash
set -e

BACKUP_DIR="/backups/postgresql"
DATE=$(date +%Y%m%d_%H%M%S)
DATABASE_URL=${DATABASE_URL:-postgresql://magnetite:password@localhost:5432/magnetite}

mkdir -p "$BACKUP_DIR"

# Create backup
pg_dump "$DATABASE_URL" | gzip > "$BACKUP_DIR/magnetite_$DATE.sql.gz"

# Remove backups older than 30 days
find "$BACKUP_DIR" -name "magnetite_*.sql.gz" -mtime +30 -delete

# Keep last 7 daily backups regardless of age
ls -t "$BACKUP_DIR"/magnetite_*.sql.gz | tail -n +8 | xargs -r rm

echo "Backup completed: magnetite_$DATE.sql.gz"
```

Make executable and add to cron:

```bash
chmod +x backup.sh

# Add to crontab
crontab -e

# Add line for daily backup at 2 AM
0 2 * * * /path/to/backup.sh >> /var/log/postgres-backup.log 2>&1
```

### Docker Volume Backup

```bash
# Backup postgres volume
docker run --rm \
  -v magnetite_postgres_data:/var/lib/postgresql/data \
  -v $(pwd)/backups:/backups \
  alpine \
  tar czf /backups/postgres_volume_$(date +%Y%m%d).tar.gz /var/lib/postgresql/data
```

### Verify Backups

```bash
# List backups
ls -la backups/

# Check backup integrity
zcat backups/magnetite_20250119_020000.sql.gz | head -20

# Test restore to different database
pg_restore -d magnetite_test --clean backups/magnetite_latest.sql.gz
```

## Point-in-Time Recovery

### Enable WAL Archiving

Add to `postgresql.conf`:

```ini
wal_level = replica
max_wal_senders = 3
archive_mode = on
archive_command = 'cp %p /var/lib/postgresql/wal_archive/%f'
```

Restart PostgreSQL after changes.

### Point-in-Time Recovery Steps

#### 1. Identify Recovery Target

```bash
# Find the timestamp to recover to
psql -U magnetite -d magnetite -c "SELECT * FROM transactions ORDER BY created_at DESC LIMIT 10;"
```

#### 2. Create Recovery Config

Create `recovery.conf`:

```ini
restore_command = 'cp /var/lib/postgresql/wal_archive/%f %p'
recovery_target_time = '2025-01-20 15:30:00 UTC'
recovery_target_action = 'promote'
```

#### 3. Stop PostgreSQL

```bash
# Docker
docker-compose stop postgres

# Native
sudo systemctl stop postgresql
```

#### 4. Copy Data Directory

```bash
cp -r /var/lib/postgresql/data /var/lib/postgresql/data_old
```

#### 5. Initialize PITR

```bash
# Docker: create new container with volume mount
docker run -d \
  --name magnetite-postgres-pitr \
  -e POSTGRES_USER=magnetite \
  -e POSTGRES_PASSWORD=password \
  -e POSTGRES_DB=magnetite \
  -v postgres_pitr_data:/var/lib/postgresql/data \
  postgres:16-alpine
```

#### 6. Monitor Recovery

```bash
# Check logs
docker logs magnetite-postgres-pitr -f

# Verify data
psql -U magnetite -d magnetite -c "SELECT COUNT(*) FROM users;"
```

## Maintenance

### VACUUM and ANALYZE

Run regularly for performance:

```bash
# Analyze all tables
psql -U magnetite -d magnetite -c "ANALYZE;"

# Vacuum to reclaim space
psql -U magnetite -d magnetite -c "VACUUM FULL;"

# Vacuum with analyze
psql -U magnetite -d magnetite -c "VACUUM (ANALYZE);"
```

### Check for Bloat

```sql
SELECT tablename,
       pg_size_pretty(pg_total_relation_size(tablename::regclass)) AS size,
       pg_total_relation_size(tablename::regclass) - pg_relation_size(tablename::regclass) AS bloat
FROM pg_tables
WHERE schemaname = 'public'
ORDER BY bloat DESC;
```

### Index Maintenance

```sql
-- Check unused indexes
SELECT indexrelname, idx_scan
FROM pg_stat_user_indexes
WHERE idx_scan = 0
ORDER BY indexrelname;
```

### Connection Management

```sql
-- Active connections
SELECT * FROM pg_stat_activity WHERE state = 'active';

-- Kill idle connections
SELECT pg_terminate_backend(pid)
FROM pg_stat_activity
WHERE state = 'idle'
AND query_start < now() - interval '10 minutes';
```

## Performance Tuning

### Recommended Settings for Production

Add to `postgresql.conf`:

```ini
# Memory
shared_buffers = 256MB
effective_cache_size = 1GB
work_mem = 16MB
maintenance_work_mem = 128MB

# Write Ahead Log
wal_buffers = 16MB
min_wal_size = 1GB
max_wal_size = 4GB

# Parallel queries
max_worker_processes = 4
max_parallel_workers_per_gather = 2
max_parallel_workers = 4

# Connection limits
max_connections = 100

# Logging
log_min_duration_statement = 1000
log_connections = on
log_disconnections = on
```

Reload configuration:

```bash
# Docker
docker-compose exec postgres pg_ctl reload -D /var/lib/postgresql/data

# Native
sudo systemctl reload postgresql
```

## Monitoring

### Key Queries

```sql
-- Database size
SELECT pg_database.datname,
       pg_size_pretty(pg_database_size(pg_database.datname))
FROM pg_database;

-- Table sizes
SELECT relname, pg_size_pretty(pg_total_relation_size(relid))
FROM pg_catalog.pg_statio_user_tables
ORDER BY pg_total_relation_size(relid) DESC
LIMIT 10;

-- Slow queries
SELECT query, calls, mean_time, total_time
FROM pg_stat_statements
ORDER BY mean_time DESC
LIMIT 10;
```

### Enable pg_stat_statements

```sql
CREATE EXTENSION IF NOT EXISTS pg_stat_statements;

-- View slow queries
SELECT query, calls, mean_time, total_time
FROM pg_stat_statements
ORDER BY mean_time DESC
LIMIT 10;
```
