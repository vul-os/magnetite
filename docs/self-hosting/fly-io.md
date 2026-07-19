# Fly.io Deployment Guide

Deploy Magnetite on Fly.io for automatic scaling and HTTPS.

## Prerequisites

- [Fly CLI](https://fly.io/docs/flyctl/install/) installed
- Fly.io account created
- Docker installed locally

## Quick Start

```bash
# Make scripts executable
chmod +x scripts/fly-setup.sh scripts/fly-scale.sh

# Run setup (creates app, secrets, database, and deploys)
./scripts/fly-setup.sh

# Scale the app
./scripts/fly-scale.sh
```

## Configuration Files

### Root fly.toml (Backend API)

```toml
app = "magnetite"
primary_region = "jnb"

[build]
dockerfile = "Dockerfile.fly"

[deploy]
release_command = "/app/migrate.sh up"

[env]
PORT = "8080"
SERVER_HOST = "0.0.0.0"
RUST_LOG = "info"

[http_service]
internal_port = 8080
force_https = true
auto_stop_machines = false
auto_start_machines = true
min_machines_running = 1

[health_check]
  port = 8080
  path = "/health"
  interval = "10s"
  timeout = "5s"
  retries = 3

[[vm]]
memory = "512mb"
cpu_kind = "shared"
cpus = 1

[autoscaling]
min_machines_running = 1
max_machines_running = 5
```

### Frontend fly.toml

Located at `frontend/fly.toml`:

```toml
app = "magnetite-frontend"
primary_region = "jnb"

[build]
dockerfile = "Dockerfile.fly.frontend"

[http_service]
internal_port = 80
force_https = true
auto_stop_machines = true
auto_start_machines = true
min_machines_running = 0

[[vm]]
memory = "256mb"
cpu_kind = "shared"
cpus = 1

[autoscaling]
min_machines_running = 0
max_machines_running = 3
```

## Optimized Dockerfile

The `Dockerfile.fly` creates smaller images using multi-stage builds:

```dockerfile
FROM rust:1.75-slim as builder
WORKDIR /app
RUN apt-get update && apt-get install -y pkg-config libssl-dev curl && rm -rf /var/lib/apt/lists/*
COPY backend/Cargo.toml backend/Cargo.lock ./
RUN mkdir -p src && echo "fn main() {}" > src/main.rs && cargo build --release && rm -rf src
COPY backend/src ./src
COPY backend/migrations ./migrations
COPY backend/tools/migrate.sh ./migrate.sh
RUN chmod +x migrate.sh && touch src/main.rs && cargo build --release

FROM debian:bookworm-slim
RUN apt-get update && apt-get install -y ca-certificates curl && rm -rf /var/lib/apt/lists/*
WORKDIR /app
COPY --from=builder /app/target/release/magnetite-backend /app/magnetite
COPY --from=builder /app/backend/migrations ./migrations
COPY --from=builder /app/migrate.sh /app/migrate.sh
RUN chmod +x /app/migrate.sh && useradd -m -u 1000 appuser && chown -R appuser:appuser /app
USER appuser
EXPOSE 8080
ENV SERVER_HOST=0.0.0.0 SERVER_PORT=8080
ENTRYPOINT ["/app/magnetite"]
```

## Setup Script

`scripts/fly-setup.sh` performs initial deployment:

```bash
#!/bin/bash
# Creates app, sets secrets, creates database and volumes, deploys
fly apps create magnetite
fly secrets set DATABASE_URL=$DATABASE_URL
fly secrets set JWT_SECRET=$JWT_SECRET
fly postgres create --name magnetite-db --region jnb
fly postgres attach --app magnetite magnetite-db
fly volumes create pg_data --size 10
fly deploy
```

## Scaling Script

`scripts/fly-scale.sh` configures autoscaling:

```bash
#!/bin/bash
fly scale count 2 --region jnb
fly scale memory 512mb
```

## Manual Deployment

### 1. Initialize the App

```bash
fly launch --name magnetite --no-deploy
```

### 2. Create PostgreSQL Database

```bash
fly postgres create --name magnetite-db --region jnb
fly postgres attach --app magnetite magnetite-db
```

### 3. Add Secrets

```bash
fly secrets set JWT_SECRET=$(openssl rand -hex 32)
fly secrets set DATABASE_URL="postgres://..."
fly secrets set GOOGLE_CLIENT_ID=your_google_client_id
fly secrets set GOOGLE_CLIENT_SECRET=your_google_client_secret
fly secrets set DISCORD_CLIENT_ID=your_discord_client_id
fly secrets set DISCORD_CLIENT_SECRET=your_discord_client_secret
fly secrets set GITHUB_CLIENT_ID=your_github_client_id
fly secrets set GITHUB_CLIENT_SECRET=your_github_client_secret
```

Payments need no secrets: the default `PAYMENT_RAIL=mock` issues deterministic
signed receipts offline, and there is no fiat provider to configure. Set
`OPERATOR_WALLET_PUBKEY` only if this node sells hosting or paid tiers:

```bash
fly secrets set OPERATOR_WALLET_PUBKEY=<hex ed25519 pubkey>
```

### 4. Deploy

```bash
fly deploy
```

### 5. Verify

```bash
fly status
fly logs
fly scale show
```

## Scaling

### Vertical Scaling

```bash
fly scale memory 1024mb
fly scale vm shared-cpu-1x
```

### Horizontal Scaling

```bash
fly scale count 2 --region jnb
fly scale count 3 --region sjc
```

### Autoscaling

Configured in fly.toml:

```toml
[autoscaling]
min_machines_running = 1
max_machines_running = 5
```

## Certificates

Fly.io automatically provisions Let's Encrypt certificates.

### Custom Domain

```bash
fly certs create api.yourdomain.com
# Add DNS A record to your Fly.io app IP
fly certs show api.yourdomain.com
```

## Health Checks

The `/health` endpoint is checked every 10s. Configure in fly.toml:

```toml
[health_check]
  port = 8080
  path = "/health"
  interval = "10s"
  timeout = "5s"
  retries = 3
```

## Database Access

```bash
fly postgres connect --app magnetite-db
fly ssh console -C "/app/migrate.sh"
```

## Monitoring

```bash
fly logs
fly metrics
fly status
fly machines list
```

## Troubleshooting

### Deployment Fails

```bash
fly deploy --verbose
fly logs -n 100
```

### Database Connection

```bash
fly secrets list
fly postgres connect --app magnetite-db
```

### High Memory

```bash
fly machines list
fly machines restart <machine-id>
```
