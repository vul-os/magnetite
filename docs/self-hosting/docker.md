# Docker Deployment

Deploy Magnetite using Docker Compose.

## docker-compose.yml Reference

```yaml
version: '3.8'

services:
  api:
    image: magnetite/api:${VERSION:-latest}
    restart: unless-stopped
    ports:
      - "8080:8080"
    environment:
      - DATABASE_URL=${DATABASE_URL}
      - REDIS_URL=${REDIS_URL}
      - JWT_SECRET=${JWT_SECRET}
    depends_on:
      - db
      - redis
    volumes:
      - ./logs:/app/logs

  game-host:
    image: magnetite/game-host:${VERSION:-latest}
    restart: unless-stopped
    environment:
      - DATABASE_URL=${DATABASE_URL}
      - REDIS_URL=${REDIS_URL}
    depends_on:
      - db
      - redis
    deploy:
      replicas: 2

  websocket:
    image: magnetite/websocket:${VERSION:-latest}
    restart: unless-stopped
    ports:
      - "8081:8081"
    environment:
      - REDIS_URL=${REDIS_URL}
    depends_on:
      - redis

  db:
    image: postgres:15-alpine
    restart: unless-stopped
    environment:
      - POSTGRES_DB=${POSTGRES_DB}
      - POSTGRES_USER=${POSTGRES_USER}
      - POSTGRES_PASSWORD=${POSTGRES_PASSWORD}
    volumes:
      - pgdata:/var/lib/postgresql/data
      - ./init.sql:/docker-entrypoint-initdb.d/init.sql

  redis:
    image: redis:7-alpine
    restart: unless-stopped
    volumes:
      - redisdata:/data

volumes:
  pgdata:
  redisdata:
```

## Environment Variables

### Required

```bash
# Database
DATABASE_URL=postgresql://user:pass@localhost:5432/magnetite
POSTGRES_DB=magnetite
POSTGRES_USER=magnetite
POSTGRES_PASSWORD=secure_password_here

# Redis
REDIS_URL=redis://localhost:6379

# Security
JWT_SECRET=your_256_bit_secret_key_here
```

### Optional

```bash
# Server
API_PORT=8080
WS_PORT=8081
LOG_LEVEL=info

# Rate Limiting
RATE_LIMIT_ENABLED=true
RATE_LIMIT_REQUESTS=100

# Storage
S3_BUCKET=magnetite-uploads
S3_REGION=us-east-1
AWS_ACCESS_KEY_ID=your_key
AWS_SECRET_ACCESS_KEY=your_secret

# Email
SMTP_HOST=smtp.example.com
SMTP_PORT=587
SMTP_USER=notifications@example.com
SMTP_PASSWORD=smtp_password

# Payments (for subscriptions)
# Payment providers must be configured for subscription functionality to work.
# Set at least one of:
PAYSTACK_SECRET_KEY=your_paystack_secret_key
CIRCLE_API_KEY=your_circle_api_key
SUBSCRIPTION_WEBHOOK_SECRET=your_webhook_secret
```

## Volume Management

### Default Volumes

| Volume | Host Path | Description |
|--------|-----------|-------------|
| pgdata | postgres_data | Database files |
| redisdata | redis_data | Redis persistence |
| logs | ./logs | Application logs |

### Backup Database

```bash
# Create backup
docker exec magnetite-db-1 pg_dump -U magnetite magnetite > backup.sql

# Restore from backup
docker exec -i magnetite-db-1 psql -U magnetite magnetite < backup.sql
```

### Backup Redis

```bash
# Save RDB snapshot
docker exec magnetite-redis-1 redis-cli BGSAVE

# Copy dump file
docker cp magnetite-redis-1:/data/dump.rdb ./redis_backup.rdb
```

## Updates and Backups

### Update Services

```bash
# Pull latest images
docker compose pull

# Restart services
docker compose up -d

# View logs
docker compose logs -f api
```

### Backup Script

```bash
#!/bin/bash
# backup.sh

DATE=$(date +%Y%m%d_%H%M%S)
BACKUP_DIR="./backups"

mkdir -p $BACKUP_DIR

# Database backup
docker exec magnetite-db-1 pg_dump -U magnetite magnetite > $BACKUP_DIR/db_$DATE.sql

# Redis backup
docker exec magnetite-redis-1 redis-cli BGSAVE
sleep 1
docker cp magnetite-redis-1:/data/dump.rdb $BACKUP_DIR/redis_$DATE.rdb

# Config backup
cp .env $BACKUP_DIR/env_$DATE.bak

echo "Backup complete: $BACKUP_DIR"
```

### Restore

```bash
# Stop services
docker compose down

# Restore database
docker exec -i magnetite-db-1 psql -U magnetite magnetite < backups/db_20240115_103000.sql

# Restore Redis
docker cp backups/redis_20240115_103000.rdb magnetite-redis-1:/data/dump.rdb

# Start services
docker compose up -d
```

## Health Checks

```bash
# API health
curl http://localhost:8080/health

# Game host health
curl http://localhost:8080/health/game-host

# WebSocket health
curl http://localhost:8081/health
```

## Resource Limits

```yaml
services:
  api:
    deploy:
      resources:
        limits:
          cpus: '2'
          memory: 2G
        reservations:
          cpus: '1'
          memory: 1G
```
