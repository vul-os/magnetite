# Production Deploy — Kubernetes & Nomad Manifests

> **Honest status:** The manifests in `deploy/k8s/` and `deploy/nomad/` are
> real, coherent, and reflect the actual stack. The **design** (cloud-runner
> orchestration, fleet scaling) is described below with a clear distinction
> between what is **implemented** and what is a **design/roadmap** item.

---

## Table of contents

1. [Stack overview](#1-stack-overview)
2. [Kubernetes quickstart](#2-kubernetes-quickstart)
3. [Nomad quickstart](#3-nomad-quickstart)
4. [Cloud-runner orchestration design](#4-cloud-runner-orchestration-design)
5. [Scaling: single-node to fleet](#5-scaling-single-node-to-fleet)
6. [Bare-metal notes](#6-bare-metal-notes)
7. [Secrets management](#7-secrets-management)
8. [Image build references](#8-image-build-references)

---

## 1. Stack overview

| Service | Image | Port | Notes |
|---------|-------|------|-------|
| backend | `ghcr.io/magnetite/backend` | 8080 | Axum REST + WebSocket; `/api/v1/*`, `/ws/*` |
| frontend | `ghcr.io/magnetite/frontend` | 80 | nginx SPA; reverse-proxies `/api/`, `/ws` → backend |
| magnetite-runtime | `ghcr.io/magnetite/runtime` | 9000 | Authoritative WASM game-server; WebSocket only |
| mediamtx | `bluenviron/mediamtx:latest` | 8888/1935/8889/8554 | HLS, RTMP, WebRTC/WHIP, RTSP |
| postgres | `postgres:16-alpine` | 5432 | Stateful; replace with managed RDS/Neon for scale |
| redis | `redis:7-alpine` | 6379 | Stateful; replace with ElastiCache/Upstash for scale |

**External dependencies** (managed services are the recommended path for scale):

- PostgreSQL → AWS RDS, Neon, Supabase, or the bundled StatefulSet.
- Redis → AWS ElastiCache, Upstash, or the bundled StatefulSet.
- Email → Resend (default) or AWS SES via SMTP. Set `EMAIL_PROVIDER` + `RESEND_API_KEY`.
- Payments → Paystack (`PAYSTACK_SECRET_KEY`), Wise payouts (`WISE_API_TOKEN`).
- TLS → cert-manager + Let's Encrypt (Kubernetes) or Caddy/Traefik (Nomad).
- Artifact storage → S3 (or any S3-compatible store) for compiled `game.wasm` artifacts.

---

## 2. Kubernetes quickstart

### 2.1 Prerequisites

```bash
# Ingress controller (nginx)
kubectl apply -f https://raw.githubusercontent.com/kubernetes/ingress-nginx/controller-v1.10.1/deploy/static/provider/cloud/deploy.yaml

# cert-manager (TLS)
kubectl apply -f https://github.com/cert-manager/cert-manager/releases/download/v1.14.4/cert-manager.yaml

# metrics-server (required for HPA)
kubectl apply -f https://github.com/kubernetes-sigs/metrics-server/releases/latest/download/components.yaml

# Create a ClusterIssuer for Let's Encrypt
kubectl apply -f - <<EOF
apiVersion: cert-manager.io/v1
kind: ClusterIssuer
metadata:
  name: letsencrypt-prod
spec:
  acme:
    server: https://acme-v02.api.letsencrypt.org/directory
    email: ops@magnetite.gg
    privateKeySecretRef:
      name: letsencrypt-prod
    solvers:
      - http01:
          ingress:
            class: nginx
EOF
```

### 2.2 Populate secrets

Edit `deploy/k8s/02-secret.yaml` — replace every `CHANGE_ME_*` placeholder
with a real `base64`-encoded value:

```bash
echo -n "your-32+-char-jwt-secret-here" | base64
echo -n "postgres://magnetite:password@postgres:5432/magnetite" | base64
# ... etc.
```

**Never commit real secrets.** Use Sealed Secrets, Vault + External Secrets
Operator, or AWS Secrets Manager instead of editing the file in-place.

### 2.3 Apply all manifests

```bash
kubectl apply -f deploy/k8s/
# Verify
kubectl -n magnetite get pods,svc,ingress
```

### 2.4 Run database migrations

```bash
kubectl -n magnetite exec deploy/backend -- \
  sh -c 'sqlx migrate run'
```

Or use an init Job (not included — add if you need automated migration on deploy).

### 2.5 Verify

```bash
# Backend health
kubectl -n magnetite exec deploy/backend -- wget -qO- http://localhost:8080/health

# Frontend
kubectl -n magnetite exec deploy/frontend -- wget -qO- http://localhost/health

# Runtime (TCP — no HTTP health endpoint)
kubectl -n magnetite exec deploy/magnetite-runtime -- \
  sh -c "nc -z localhost 9000 && echo 'runtime port open'"
```

---

## 3. Nomad quickstart

### 3.1 Prerequisites

- Nomad cluster with the `docker` driver enabled.
- Consul for service discovery (used by all job specs).
- Host volumes `postgres_data` and `redis_data` configured on data-class nodes:

```hcl
# In the Nomad agent config (client.hcl):
host_volume "postgres_data" {
  path      = "/data/magnetite/postgres"
  read_only = false
}
host_volume "redis_data" {
  path      = "/data/magnetite/redis"
  read_only = false
}
```

### 3.2 Store secrets in Nomad Variables

```bash
nomad var put magnetite/secrets \
  DATABASE_URL="postgres://magnetite:password@postgres.service.consul:5432/magnetite" \
  JWT_SECRET="your-32+-char-secret" \
  PAYSTACK_SECRET_KEY="sk_live_..." \
  WISE_API_TOKEN="T-live-..." \
  WISE_PROFILE_ID="1234567" \
  RESEND_API_KEY="re_..." \
  BUILD_RUNNER_TOKEN="tok_..."
```

Alternatively, use Vault with the `vault {}` block in each job spec (see the
commented-out blocks in `backend.nomad`).

### 3.3 Submit jobs

```bash
# Deploy in dependency order.
nomad job run deploy/nomad/postgres.nomad
nomad job run deploy/nomad/redis.nomad
nomad job run deploy/nomad/mediamtx.nomad
nomad job run deploy/nomad/backend.nomad
nomad job run deploy/nomad/frontend.nomad
nomad job run deploy/nomad/runtime.nomad

# Verify all jobs are running
nomad job status -namespace magnetite
```

### 3.4 Expose services

Add Fabio or Traefik as a Nomad load balancer job to route:
- `magnetite.gg` → `frontend` service (port 80)
- `api.magnetite.gg` → `backend` service (port 8080)
- `runtime.magnetite.gg:9000` → `magnetite-runtime` service (port 9000, TCP pass-through)

---

## 4. Cloud-runner orchestration design

This section describes how `scripts/wasm-build-runner.sh` and
`magnetite-runtime` instances are provisioned and scaled per game. It
distinguishes what is **implemented** from what is a **design/roadmap** item.

### 4.1 WASM build runner

**Implemented:**

The build runner is a shell script (`scripts/wasm-build-runner.sh`) that:

1. Polls `GET /api/v1/distribution/builds/pending` (bearer-authed with
   `BUILD_RUNNER_TOKEN`) to fetch queued build jobs.
2. Clones the registered game repository at the committed SHA.
3. Runs `cargo build --target wasm32-wasip1 --release --features wasm`.
4. Optionally runs `wasm-opt -Oz`.
5. Uploads the resulting `game.wasm` to S3 (if `ARTIFACT_BUCKET` is set).
6. Reports the result to `POST /api/v1/distribution/:game_id/builds/report`
   with `outcome`, `artifact_url`, `sha256_hash`, `file_size_bytes`.

The backend correctly keeps `build_status = 'queued'` until the runner reports
back — there is no fabricated success.

**To deploy on Kubernetes:**

```yaml
# Example Job spec (not included in deploy/k8s/ — add as needed).
apiVersion: batch/v1
kind: Job
metadata:
  name: wasm-build-runner
  namespace: magnetite
spec:
  template:
    spec:
      containers:
        - name: runner
          image: ghcr.io/magnetite/build-runner:latest
          # The runner script is entrypoint; it runs in daemon mode.
          env:
            - name: MAGNETITE_API_URL
              value: "http://backend:8080"
            - name: BUILD_RUNNER_TOKEN
              valueFrom:
                secretKeyRef:
                  name: magnetite-secrets
                  key: BUILD_RUNNER_TOKEN
            - name: ARTIFACT_BUCKET
              value: "magnetite-wasm-artifacts"
            - name: POLL_INTERVAL
              value: "30"
      restartPolicy: OnFailure
```

For a multi-runner fleet (parallel builds), run the same Job as a
Deployment with `replicas: N` — each runner polls independently, and the
backend's `build_status` column ensures a job is only claimed once (the
runner should mark the job `in_progress` before building — **this is a
Bucket-D roadmap item**: the current API only has `queued` → `success/failed`,
not an atomic claim step).

**Roadmap (Bucket D):**

- Atomic job claim (`build_status = 'in_progress'`) to prevent duplicate builds
  when multiple runners poll concurrently.
- GitHub Actions integration: instead of a self-hosted runner, trigger builds
  from the GitHub App webhook (registered games already have a GitHub App
  integration; the build-dispatch step is not yet wired).
- CDN invalidation after a successful upload.

### 4.2 Runtime instance provisioning

**Implemented:**

The provisioning API (`backend/src/api/provisioning.rs`) provides:

```
POST   /api/v1/provisioning/:game_id/instances    — create a pending instance
GET    /api/v1/provisioning/:game_id/instances    — list instances for a game
GET    /api/v1/provisioning/:game_id/instances/:id — single instance
PATCH  /api/v1/provisioning/:game_id/instances/:id — runner reports status/endpoint
DELETE /api/v1/provisioning/:game_id/instances/:id — stop
GET    /api/v1/provisioning/pending               — runner poll
```

The flow:
1. A player clicks **Play** → the frontend calls `POST /instances`.
2. The backend writes a `runtime_instances` row with `status = 'pending'`.
3. A running `magnetite-runtime` Pod polls `GET /provisioning/pending`, picks up
   the row, loads the `artifact_url` (the compiled `game.wasm`), starts a
   Wasmtime executor, and binds a WebSocket listener.
4. The runtime calls `PATCH /instances/:id` with `status = 'running'` and
   `ws_endpoint = "ws://<pod-ip>:9000"`.
5. The frontend polls `GET /instances/:id` until `ws_endpoint` is set, then
   connects via WebSocket.

**Current limitation:** All instances share a single `magnetite-runtime`
Deployment. The runtime process can host multiple `SharedRoom` sessions
concurrently, but `Dedicated` topology (one process per game session) requires
spawning a new Pod per session — not yet automated.

**Roadmap (Bucket D — auto-provisioning):**

The following steps would implement fully automatic per-session Pod
provisioning on Kubernetes:

```
Backend POST /instances
   ↓
Provisioning controller (new — does not exist yet)
   ↓  creates a Kubernetes Job or Pod via the k8s API
magnetite-runtime Pod starts with --wasm <artifact_url>
   ↓
Pod calls PATCH /instances/:id { status: "running", ws_endpoint: "ws://..." }
   ↓
Frontend connects to ws_endpoint
```

A minimal provisioning controller could be a sidecar or a separate Go/Rust
binary that watches the `runtime_instances` table (via polling or LISTEN/NOTIFY)
and calls the Kubernetes API to create a Pod. Alternatively, use the Nomad API
to dispatch a parameterized job per session.

**For the current implementation** (single shared runtime process, multiple
sessions in-process), scale the `magnetite-runtime` Deployment horizontally
via the HPA and accept that sessions on different Pods are independent.
The frontend polls `ws_endpoint` and connects to whichever Pod responded.

---

## 5. Scaling: single-node to fleet

### Stage 1 — Single node (Docker Compose)

Run the entire stack on one machine:

```bash
docker compose up -d
```

All services share one host. Suitable for development, small communities,
and game jams with up to ~50 concurrent players.

### Stage 2 — Single Kubernetes node or small Nomad cluster

Deploy the manifests in this directory to a single-node cluster (e.g. k3s on
a bare-metal server or a single cloud VM):

```bash
# k3s single node
curl -sfL https://get.k3s.io | sh -
kubectl apply -f deploy/k8s/
```

Replace Postgres and Redis with managed services (Neon, Upstash) to avoid
data-loss risk on node failure. Suitable for a live game with ~200–500 concurrent
players.

### Stage 3 — Multi-node cluster

Scale out by:

1. Adding more backend replicas — the HPA (`09-hpa.yaml`) handles this automatically.
2. Adding more `magnetite-runtime` replicas for more concurrent game sessions.
3. Moving Postgres to RDS/Neon and Redis to ElastiCache/Upstash.
4. Moving MediaMTX to a dedicated VM with high egress bandwidth (streaming is bandwidth-heavy).
5. Pointing `GAME_SERVER_WS_BASE` at a load balancer that distributes WebSocket
   connections across runtime Pods (e.g. an NGINX TCP proxy or AWS NLB).

**Session stickiness:** WebSocket connections to `magnetite-runtime` must be
sticky to a Pod for the duration of a game session. Use session affinity on the
LoadBalancer Service, or expose each Pod via its own IP (Headless Service +
client-side DNS). The provisioned `ws_endpoint` already contains the direct
Pod IP, so clients bypass the Service VIP for the game connection.

### Stage 4 — Auto-provisioned game servers (Bucket D roadmap)

Full per-session auto-provisioning:

- A provisioning controller watches `runtime_instances` and creates/destroys
  Kubernetes Jobs (one Job = one game session) automatically.
- The WASM build runner fleet scales via a Deployment with KEDA (event-driven
  autoscaling on `pending_builds` queue depth).
- MediaMTX scales horizontally with a CDN (e.g. Cloudflare Stream, AWS CloudFront)
  in front for HLS delivery.
- The backend scales to many replicas with a read replica for the DB.

This stage requires the provisioning controller (not yet implemented) and the
atomic job-claim step for the build runner.

---

## 6. Bare-metal notes

If your cluster does not have a cloud LoadBalancer controller (e.g. bare-metal
k3s), change the `magnetite-runtime` Service type to `NodePort` and forward
port 9000 via an external nginx or HAProxy:

```nginx
stream {
  upstream runtime_ws {
    server <node-ip>:<nodePort>;
  }
  server {
    listen 9000;
    proxy_pass runtime_ws;
  }
}
```

For the Ingress, use MetalLB to assign a real IP to the nginx Ingress
LoadBalancer on bare-metal:

```bash
kubectl apply -f https://raw.githubusercontent.com/metallb/metallb/v0.14.5/config/manifests/metallb-native.yaml
```

---

## 7. Secrets management

**Never commit real secrets.** Options in order of recommendation:

1. **Vault + External Secrets Operator** (Kubernetes) — stores secrets in Vault;
   ESO syncs them to Kubernetes Secrets. Best for teams.
2. **Sealed Secrets** (Kubernetes) — encrypt secrets in-repo using a cluster key;
   safe to commit.
3. **AWS Secrets Manager + External Secrets Operator** — managed, no Vault to run.
4. **Nomad Variables + Vault** (Nomad) — built-in; see commented `vault {}` blocks
   in the Nomad job specs.
5. **Manual** (`kubectl create secret generic ...`) — fine for a single operator,
   but not reproducible.

### Required secrets

| Secret key | Description | Config field |
|-----------|-------------|--------------|
| `DATABASE_URL` | PostgreSQL connection string | `Config::database_url` |
| `JWT_SECRET` | HMAC signing key (min 32 bytes in production) | `Config::jwt_secret` |
| `PAYSTACK_SECRET_KEY` | Paystack live key (`sk_live_...`) | `Config::paystack_secret_key` |
| `WISE_API_TOKEN` | Wise API token (`T-live-...`) | `Config::wise_api_token` |
| `WISE_PROFILE_ID` | Wise sending profile ID | `Config::wise_profile_id` |
| `RESEND_API_KEY` | Resend transactional email key | `Config::resend_api_key` |
| `BUILD_RUNNER_TOKEN` | Bearer token for the WASM build runner | provisioning auth |

OAuth keys (GOOGLE, DISCORD, GITHUB, GITLAB client IDs + secrets) are optional —
leave empty if those providers are not used.

---

## 8. Image build references

| Image | Dockerfile | Notes |
|-------|-----------|-------|
| `ghcr.io/magnetite/backend:latest` | `Dockerfile.backend` (target: `runtime`) | Axum binary; runs as non-root user 1000 |
| `ghcr.io/magnetite/frontend:latest` | `Dockerfile.frontend` | nginx + built React SPA |
| `ghcr.io/magnetite/runtime:latest` | `magnetite-runtime/Dockerfile` | `magnetite-serve` binary; Wasmtime runtime; Debian bookworm-slim base |

Build and push (example for GitHub Container Registry):

```bash
# Backend
docker build -f Dockerfile.backend --target runtime \
  -t ghcr.io/magnetite/backend:latest .
docker push ghcr.io/magnetite/backend:latest

# Frontend (pass API URL at build time)
docker build -f Dockerfile.frontend \
  -t ghcr.io/magnetite/frontend:latest .
docker push ghcr.io/magnetite/frontend:latest

# Runtime (build context must be the repo root — Dockerfile copies 4 crates)
docker build -f magnetite-runtime/Dockerfile \
  -t ghcr.io/magnetite/runtime:latest .
docker push ghcr.io/magnetite/runtime:latest
```

The `.github/workflows/release.yml` workflow builds and pushes all three images
on a tagged release.
