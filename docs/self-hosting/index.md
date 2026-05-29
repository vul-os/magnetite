# Self-Hosting Guide

Deploy your own Magnetite instance on any server.

## Overview

Magnetite can be self-hosted using Docker Compose, or deployed to platforms like Fly.io.

```
┌─────────────────────────────────────────────────────────┐
│                    Nginx / Traefik                       │
│                   (Reverse Proxy)                        │
└─────────────────────────┬───────────────────────────────┘
                          │
┌─────────────────────────▼───────────────────────────────┐
│                   Docker Network                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐      │
│  │   Backend   │  │  Frontend   │  │   Redis     │      │
│  │   (Rust)    │  │   (React)   │  │  (Cache)    │      │
│  └──────┬──────┘  └─────────────┘  └─────────────┘      │
│         │                                                   │
│  ┌──────▼──────┐                                           │
│  │ PostgreSQL  │                                           │
│  └─────────────┘                                           │
└─────────────────────────────────────────────────────────┘
```

## Prerequisites

| Component | Version | Notes |
|-----------|---------|-------|
| Docker | 24.0+ | Container runtime |
| Docker Compose | 2.20+ | Orchestration |
| PostgreSQL | 15+ | Database |
| Redis | 7+ | Caching/Sessions (optional) |
| Domain | - | For SSL certificates |

## Guides

| Guide | Description |
|-------|-------------|
| [Quickstart](./quickstart.md) | One-command Docker setup |
| [Docker Deployment](./docker.md) | Complete Docker Compose setup with Dockerfiles |
| [Fly.io](./fly-io.md) | Deploy to Fly.io with autoscaling |
| [Environment Variables](./environment-variables.md) | All configuration options |
| [Database](./database.md) | PostgreSQL setup, migrations, backups |
| [SSL/TLS](./ssl.md) | Let's Encrypt and HTTPS configuration |
| [Updating](./updating.md) | Update procedures and rollback |
| [Monitoring](./monitoring.md) | Logging, error tracking, metrics |

## Quick Start

```bash
# Clone repository
git clone https://github.com/magnetite/platform.git
cd platform

# Copy environment template
cp .env.example .env

# Start services
docker compose up -d

# Verify health
curl http://localhost:8080/health
```

## Common Deployments

### Docker (Recommended)

Full production-ready setup with Docker Compose:

```bash
# Start all services
docker compose up -d

# Check status
docker compose ps

# View logs
docker compose logs -f backend
```

See [Docker Deployment](./docker.md) for complete setup.

### Fly.io

Managed infrastructure with autoscaling:

```bash
fly launch
fly secrets set JWT_SECRET=$(openssl rand -hex 32)
fly deploy
```

See [Fly.io](./fly-io.md) for detailed guide.

### Manual

For development or custom deployments:

```bash
# Build frontend
npm install && npm run build

# Build backend
cd backend && cargo build --release

# Run
./target/release/magnetite-backend
```

## Configuration

Essential environment variables:

```bash
DATABASE_URL=postgresql://user:pass@host:5432/magnetite
JWT_SECRET=your-secure-secret-at-least-32-characters
SERVER_HOST=0.0.0.0
SERVER_PORT=8080
```

See [Environment Variables](./environment-variables.md) for all options.

## Security Checklist

- [ ] Change default passwords
- [ ] Use strong JWT_SECRET (32+ chars)
- [ ] Enable SSL/TLS
- [ ] Configure firewall rules
- [ ] Enable database SSL connections
- [ ] Set up regular backups
- [ ] Review CORS settings
- [ ] Enable rate limiting

## Support

- GitHub Issues: Report deployment problems
- Documentation: Check individual guide files for troubleshooting
