# Monitoring Guide

Set up comprehensive monitoring for your Magnetite deployment.

## Logging Setup

### Docker Logging

Configure JSON logging for containers:

```yaml
# docker-compose.yml
services:
  backend:
    logging:
      driver: json-file
      options:
        max-size: "100m"
        max-file: "5"
        labels: "service"

  postgres:
    logging:
      driver: json-file
      options:
        max-size: "200m"
        max-file: "10"
```

### Application Logging

Backend logs are sent to stdout by default. Configure in `backend/src/main.rs`:

```rust
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

tracing_subscriber::registry()
    .with(tracing_subscriber::EnvFilter::new(
        std::env::var("RUST_LOG").unwrap_or_else(|_| "info".into()),
    ))
    .with(tracing_subscriber::fmt::layer())
    .init();
```

### Log Aggregation

#### Using Loki (Recommended)

```yaml
services:
  loki:
    image: grafana/loki:2.9.0
    ports:
      - "3100:3100"
    volumes:
      - ./loki-config.yml:/etc/loki/local-config.yaml

  promtail:
    image: grafana/promtail:2.9.0
    volumes:
      - /var/lib/docker/containers:/var/lib/docker/containers
      - ./promtail-config.yml:/etc/promtail/config.yml
    depends_on:
      - loki
```

Create `promtail-config.yml`:

```yaml
server:
  http_listen_port: 9080
  grpc_listen_port: 0

positions:
  filename: /tmp/positions.yaml

clients:
  - url: http://loki:3100/loki/api/v1/push

scrape_configs:
  - job_name: magnetite
    docker_targets:
      - "containers"
    pipeline_stages:
      - json:
          expressions:
            level: level
            msg: msg
            service: service
      - labels:
          level:
          service:
```

### Centralized Logging

#### ELK Stack (Elasticsearch, Logstash, Kibana)

```yaml
services:
  elasticsearch:
    image: docker.elastic.co/elasticsearch/elasticsearch:8.11.0
    environment:
      - discovery.type=single-node
      - "ES_JAVA_OPTS=-Xms512m -Xmx512m"
    volumes:
      - es_data:/usr/share/elasticsearch/data

  logstash:
    image: docker.elastic.co/logstash/logstash:8.11.0
    volumes:
      - ./logstash.conf:/usr/share/logstash/pipeline/logstash.conf

  kibana:
    image: docker.elastic.co/kibana/kibana:8.11.0
    ports:
      - "5601:5601"
    depends_on:
      - elasticsearch

volumes:
  es_data:
```

Create `logstash.conf`:

```
input {
  tcp {
    port => 5000
    codec => json_lines
  }
}

filter {
  if [service] == "magnetite" {
    mutate {
      add_field => { "[@metadata][index]" => "magnetite" }
    }
  }
}

output {
  elasticsearch {
    hosts => ["elasticsearch:9200"]
    index => "%{[@metadata][index]}-%{+YYYY.MM.dd}"
  }
}
```

## Error Tracking (Sentry)

### Backend Setup (Rust)

Add to `Cargo.toml`:

```toml
sentry = { version = "0.32", features = ["actix-web""] }
```

Add to `backend/src/lib.rs`:

```rust
use sentry::{init, ClientOptions};

let _guard = init(ClientOptions::new()
    .set_dsn(std::env::var("SENTRY_DSN").ok())
    .set_environment(std::env::var("RUST_ENV").ok().unwrap_or_default())
    .set_release(env!("CARGO_PKG_VERSION"))
);
```

Add middleware in `backend/src/middleware.rs`:

```rust
use sentry::integrations::tracing::SentryLayer;

pub fn add_sentry_layer() -> SentryLayer {
    SentryLayer::new()
}
```

### Frontend Setup (JavaScript)

```bash
npm install @sentry/react
```

Create `src/sentry.ts`:

```typescript
import * as Sentry from "@sentry/react";

Sentry.init({
  dsn: import.meta.env.VITE_SENTRY_DSN,
  environment: import.meta.env.MODE,
  integrations: [
    Sentry.browserTracingIntegration(),
    Sentry.replayIntegration(),
  ],
  tracesSampleRate: 0.1,
  replaysSessionSampleRate: 0.1,
  replaysOnErrorSampleRate: 1.0,
});

export default Sentry;
```

Wrap your app in `src/main.tsx`:

```typescript
import Sentry from "./sentry";

Sentry.init({
  dsn: import.meta.env.VITE_SENTRY_DSN,
});

ReactDOM.createRoot(document.getElementById("root")!).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>
);
```

### Docker Sentry

```yaml
services:
  sentry:
    image: sentry:23.11.0
    ports:
      - "9000:9000"
    environment:
      SENTRY_SECRET_KEY: ${SENTRY_SECRET_KEY}
      SENTRY_DB_NAME: magnetite
      SENTRY_DB_USER: magnetite
      SENTRY_DB_PASSWORD: ${POSTGRES_PASSWORD}
      SENTRY_POSTGRES_HOST: postgres
      SENTRY_REDIS_HOST: redis
      SENTRY_EMAIL_HOST: ${SMTP_HOST}
      SENTRY_EMAIL_USER: ${SMTP_USERNAME}
      SENTRY_EMAIL_PASSWORD: ${SMTP_PASSWORD}
    depends_on:
      - postgres
      - redis
```

## Uptime Monitoring

### Health Check Endpoint

The backend exposes `/health`:

```json
{
  "status": "healthy",
  "version": "0.1.0",
  "database": "connected",
  "timestamp": "2025-01-19T12:00:00Z"
}
```

### UptimeRobot Setup

1. Create account at https://uptimerobot.com/
2. Add monitor:
   - Type: HTTP(s)
   - Friendly name: Magnetite API
   - URL: `https://api.magnetite.example.com/health`
3. Set check interval (5 minutes recommended)
4. Configure alerting preferences

### Grafana Uptime Panel

```yaml
- expr: up{job="magnetite-backend"}
  legendFormat: "Backend"
  - expr: up{job="magnetite-frontend"}
  legendFormat: "Frontend"
  - expr: up{job="magnetite-postgres"}
  legendFormat: "PostgreSQL"
```

### Cron-based Monitoring

Create `monitor.sh`:

```bash
#!/bin/bash

API_URL="http://localhost:8080/health"
EMAIL="admin@example.com"
TIMEOUT=10

response=$(curl -s -o /dev/null -w "%{http_code}" --max-time $TIMEOUT $API_URL)

if [ "$response" != "200" ]; then
    echo "Magnetite is down! HTTP Status: $response" | mail -s "ALERT: Magnetite Down" $EMAIL
fi
```

Add to crontab:

```bash
*/5 * * * * /path/to/monitor.sh
```

## Performance Metrics

### Prometheus Setup

```yaml
services:
  prometheus:
    image: prom/prometheus:v2.48.0
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus_data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'

volumes:
  prometheus_data:
```

Create `prometheus.yml`:

```yaml
global:
  scrape_interval: 15s

scrape_configs:
  - job_name: 'magnetite-backend'
    static_configs:
      - targets: ['backend:8080']
    metrics_path: /metrics

  - job_name: 'postgres'
    static_configs:
      - targets: ['postgres-exporter:9187']

  - job_name: 'redis'
    static_configs:
      - targets: ['redis-exporter:9121']
```

### Key Metrics to Track

#### Backend Metrics

| Metric | Description | Alert Threshold |
|--------|-------------|-----------------|
| `http_requests_total` | Total HTTP requests | N/A |
| `http_request_duration_seconds` | Request latency | p99 > 2s |
| `db_pool_connections` | DB connection pool | > 80% max |
| `active_websockets` | Active WebSocket connections | > 1000 |

#### Database Metrics

| Metric | Description | Alert Threshold |
|--------|-------------|-----------------|
| `pg_stat_database_numbackends` | Active connections | > 80 |
| `pg_stat_database_tup_returned` | Rows returned | N/A |
| `pg_stat_database_tup_fetched` | Rows fetched | N/A |
| `pg_stat_bgwriter_buffers_clean` | Buffer writes | N/A |

#### Redis Metrics

| Metric | Description | Alert Threshold |
|--------|-------------|-----------------|
| `redis_connected_clients` | Connected clients | > 1000 |
| `redis_memory_used_bytes` | Memory usage | > 256mb |
| `redis_keyspace_hits_total` | Cache hits | < 95% |

### Grafana Dashboard

Create `grafana-dashboard.json`:

```json
{
  "dashboard": {
    "title": "Magnetite Overview",
    "panels": [
      {
        "title": "Request Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(http_requests_total[5m])",
            "legendFormat": "{{ method }} {{ path }}"
          }
        ]
      },
      {
        "title": "Error Rate",
        "type": "graph",
        "targets": [
          {
            "expr": "rate(http_requests_total{status=~\"5..\"}[5m])",
            "legendFormat": "5xx errors"
          }
        ]
      },
      {
        "title": "Database Connections",
        "type": "graph",
        "targets": [
          {
            "expr": "db_pool_connections",
            "legendFormat": "Active"
          }
        ]
      }
    ]
  }
}
```

### Alerting Rules

Create `alerts.yml`:

```yaml
groups:
  - name: magnetite
    rules:
      - alert: HighErrorRate
        expr: rate(http_requests_total{status=~"5.."}[5m]) > 0.01
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "High error rate on {{ $labels.instance }}"

      - alert: HighLatency
        expr: histogram_quantile(0.99, rate(http_request_duration_seconds_bucket[5m])) > 2
        for: 5m
        labels:
          severity: warning
        annotations:
          summary: "High latency on {{ $labels.instance }}"

      - alert: DatabaseDown
        expr: up{job="postgres"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "Database is down"
```

## Docker Monitoring Stack

Complete monitoring stack with Grafana, Prometheus, and Loki:

```yaml
version: '3.8'

services:
  prometheus:
    image: prom/prometheus:v2.48.0
    ports:
      - "9090:9090"
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus_data:/prometheus

  grafana:
    image: grafana/grafana:10.2.0
    ports:
      - "3000:3000"
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=${GRAFANA_PASSWORD}
    volumes:
      - grafana_data:/var/lib/grafana
      - ./dashboards:/etc/grafana/provisioning/dashboards

  loki:
    image: grafana/loki:2.9.0
    ports:
      - "3100:3100"
    volumes:
      - ./loki-config.yml:/etc/loki/local-config.yaml

  promtail:
    image: grafana/promtail:2.9.0
    volumes:
      - /var/lib/docker/containers:/var/lib/docker/containers
      - ./promtail-config.yml:/etc/promtail/config.yml
    depends_on:
      - loki

volumes:
  prometheus_data:
  grafana_data:
```

## Monitoring Checklist

- [ ] Health check endpoint configured
- [ ] Logs aggregated centrally
- [ ] Error tracking (Sentry) enabled
- [ ] Uptime monitoring configured
- [ ] Performance metrics collected
- [ ] Alerting rules defined
- [ ] Alert notifications configured
- [ ] Dashboard created in Grafana
- [ ] Runbook documented for alerts
