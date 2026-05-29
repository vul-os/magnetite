# Quickstart Guide

Deploy Magnetite in minutes using Docker.

## One-Command Setup (Docker)

The fastest way to get Magnetite running:

```bash
docker run -d \
  --name magnetite \
  -p 80:80 \
  -p 8080:8080 \
  -e DATABASE_URL=postgresql://user:password@host:5432/magnetite \
  -e JWT_SECRET=your-secure-secret-here \
  -e SERVER_HOST=0.0.0.0 \
  -e SERVER_PORT=8080 \
  magnetite/app
```

## Docker-Compose Setup

Create a `docker-compose.yml` file:

```yaml
version: '3.8'

services:
  backend:
    image: magnetite/backend:latest
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
    volumes:
      - ./data/backend:/data
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8080/health"]
      interval: 30s
      timeout: 10s
      retries: 3

  frontend:
    image: magnetite/frontend:latest
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
      - ./migrations:/docker-entrypoint-initdb.d
    healthcheck:
      test: ["CMD-SHELL", "pg_isready -U magnetite"]
      interval: 10s
      timeout: 5s
      retries: 5

  redis:
    image: redis:7-alpine
    restart: unless-stopped
    ports:
      - "6379:6379"
    volumes:
      - redis_data:/data
    command: redis-server --appendonly yes

volumes:
  postgres_data:
  redis_data:
```

Start all services:

```bash
docker-compose up -d
```

## Initial Configuration

### 1. Run Database Migrations

```bash
docker-compose exec backend migrate
```

Or manually:

```bash
for migration in backend/migrations/*.sql; do
  docker-compose exec -T postgres psql -U magnetite -d magnetite -f "$migration"
done
```

### 2. Create Admin User

```bash
docker-compose exec backend create-admin \
  --email admin@example.com \
  --username admin \
  --password your-secure-password
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
docker-compose logs backend
```

### Database Connection Failed

Ensure PostgreSQL is healthy:
```bash
docker-compose ps postgres
docker-compose logs postgres
```

### Migration Errors

Verify migrations ran in order:
```bash
docker-compose exec postgres psql -U magnetite -d magnetite -c "SELECT * FROM _migrations;"
```
