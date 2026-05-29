# Security Documentation

Security best practices for Magnetite deployments.

## Best Practices

### Container Security

```dockerfile
# Use specific versions, not 'latest'
FROM rust:1.75-slim

# Create non-root user
RUN adduser --disabled-password --gecos '' magnetite
USER magnetite

# Read-only filesystem
READONLY=true
```

### Network Isolation

```yaml
services:
  api:
    networks:
      - internal
    expose:
      - "8080"

networks:
  internal:
    driver: bridge
    internal: true
```

## Environment Secrets

### Secret Management

| Secret | Description | Rotation |
|--------|-------------|----------|
| JWT_SECRET | Token signing key | 90 days |
| DATABASE_PASSWORD | DB credentials | 30 days |
| API_KEYS | Third-party API keys | As needed |

### Generating Secrets

```bash
# Generate JWT secret
openssl rand -hex 32

# Generate database password
openssl rand -base64 32

# Store in .env (never commit)
echo "JWT_SECRET=$(openssl rand -hex 32)" >> .env
```

### Secret Rotation

```bash
# Rotate JWT secret
magnetite secret rotate jwt --new-secret

# Rotate database password
magnetite secret rotate db --user magnetite
```

## Database Security

### PostgreSQL

```sql
-- Create dedicated app user
CREATE USER magnetite_app WITH PASSWORD 'secure_password';

-- Grant minimal privileges
GRANT CONNECT ON DATABASE magnetite TO magnetite_app;
GRANT USAGE ON SCHEMA public TO magnetite_app;
GRANT SELECT, INSERT, UPDATE, DELETE ON ALL TABLES IN SCHEMA public TO magnetite_app;

-- Row-level security
ALTER TABLE users ENABLE ROW LEVEL SECURITY;

CREATE POLICY "Users can view own row" ON users
    FOR SELECT USING (auth.uid() = user_id);
```

### Connection Pooling

```yaml
# Use PgBouncer for connection pooling
services:
  pgbouncer:
    image: edoburu/pgbouncer:latest
    environment:
      - DATABASE_URL=${DATABASE_URL}
      - POOL_MODE=transaction
      - MAX_CLIENT_CONN=100
      - DEFAULT_POOL_SIZE=20
```

## Monitoring

### Health Endpoints

```bash
# System health
curl http://localhost:8080/health

# Detailed status
curl http://localhost:8080/health/detailed

# Metrics
curl http://localhost:8080/metrics
```

### Logging

```yaml
# Structured logging format
LOG_FORMAT=json
LOG_LEVEL=info
LOG_FIELDS=timestamp,level,message,request_id,user_id
```

### Alerting

| Alert | Condition | Severity |
|-------|-----------|----------|
| High Error Rate | error_rate > 5% | critical |
| Slow Response | p99_latency > 2s | warning |
| Disk Usage | disk > 80% | warning |
| Memory Usage | memory > 90% | critical |

### Audit Logs

```sql
-- Audit table
CREATE TABLE audit_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id UUID,
    action VARCHAR(100),
    resource VARCHAR(200),
    details JSONB,
    ip_address INET,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

-- Log all mutations
CREATE OR REPLACE FUNCTION audit_trigger()
RETURNS TRIGGER AS $$
BEGIN
    INSERT INTO audit_log (action, resource, details)
    VALUES (TG_OP, TG_TABLE_NAME, to_jsonb(NEW));
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
```

## SSL/TLS

### Certificate Requirements

| Type | Requirement |
|------|-------------|
| Protocol | TLS 1.2+ |
| Ciphers | AES-256-GCM, ChaCha20 |
| Certificate | 2048-bit RSA or 256-bit ECC |

### Nginx Configuration

```nginx
server {
    listen 443 ssl http2;
    ssl_certificate /etc/ssl/certs/magnetite.crt;
    ssl_certificate_key /etc/ssl/private/magnetite.key;
    ssl_protocols TLSv1.2 TLSv1.3;
    ssl_ciphers ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384;
}
```
