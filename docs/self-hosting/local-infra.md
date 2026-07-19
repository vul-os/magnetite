# Local Infrastructure Stand-ins

This page documents how to run every optional external service locally so that
the full create-build-play loop works on a single developer machine without any
cloud accounts.

Only PostgreSQL and Redis are actually required. The default configuration —
`PAYMENT_RAIL=mock`, `COMMS_PROVIDER=builtin`, no media server — needs no
third-party account and works fully offline.

---

## Architecture overview

```
localhost
  │
  ├── :3000  Frontend (React SPA, nginx)
  ├── :8080  Backend API (Axum)
  ├── :9000  magnetite-runtime (authoritative game-server)
  ├── :8888  MediaMTX (HLS / RTMP / WebRTC)   ← optional, `media` profile
  ├── :1935  MediaMTX RTMP ingest             ← optional, `media` profile
  ├── :5432  PostgreSQL
  ├── :6379  Redis
  ├── :1025  MailHog SMTP  (dev email preview)
  └── :8025  MailHog Web UI
```

The backend connects to the runtime at `ws://magnetite-runtime:9000`, pre-wired in
`docker-compose.override.yml`. It has **no** dependency on MediaMTX:
`MEDIA_SERVER_BASE_URL` is empty by default and you set it yourself if you start
the `media` profile.

---

## Quick start: everything in one command

```bash
git clone https://github.com/magnetite-platform/magnetite.git
cd magnetite
cp .env.example .env      # no edits required for local dev
docker compose up -d
```

This starts:
- PostgreSQL + Redis (data stores)
- Backend API (port 8080)
- Frontend with nginx (port 3000)
- magnetite-runtime authoritative game-server (port 9000)
- MailHog email preview (ports 1025, 8025)

MediaMTX is **not** included — it is optional and behind the `media` profile:

```bash
docker compose --profile media up -d      # adds ports 8888, 1935, 8189/udp
```

Verify the stack is healthy:

```bash
# Backend API
curl http://localhost:8080/health/ready
# → {"status":"success","data":{"database":"ok","redis":"ok"}}

# Frontend
curl -s http://localhost:3000 | head -5

# MediaMTX API
curl -s http://localhost:8888/v3/config/global/get | jq '.loglevel'

# Runtime (WebSocket — check it listens)
curl --include --no-buffer \
  -H "Connection: Upgrade" -H "Upgrade: websocket" \
  -H "Sec-WebSocket-Version: 13" \
  -H "Sec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==" \
  http://localhost:9000/ 2>&1 | head -5
```

---

## Service: MediaMTX (HLS / RTMP / WHIP) — optional

MediaMTX is **one optional** media plane for live-stream watch (HLS) and RTMP
egress to Twitch or YouTube. Media is per-operator: every operator runs their own,
and the backend depends on none. Start it with `docker compose --profile media up`.

If you use an external comms provider instead (`COMMS_PROVIDER=jitsi`, `livekit`
or `owncast`), that provider brings its own media plane and you do not need
MediaMTX at all.

### What it enables

| Feature | Without MediaMTX | With MediaMTX |
|---------|-----------------|---------------|
| HLS stream watch | `/streams/:id/hls` returns HTTP 503 | Returns `.m3u8` playlist |
| RTMP egress | Stored target URL, no bytes forwarded | OBS/FFmpeg can push to `:1935` |
| WebRTC ingest (WHIP) | Not available | Available on `:8189` (UDP) |

### Environment variables

```bash
# Empty by default. Set in .env only if you started the `media` profile.
MEDIA_SERVER_BASE_URL=http://mediamtx:8888

# Optional overrides
MEDIAMTX_HLS_PORT=8888
MEDIAMTX_RTMP_PORT=1935
MEDIAMTX_WEBRTC_PORT=8189
MTX_LOGLEVEL=info          # debug | info | warn | error
MTX_HLSSEGMENTDURATION=2s  # lower = lower latency
```

### Streaming from OBS (local dev)

1. In OBS → Settings → Stream → Custom:
   - Server: `rtmp://localhost:1935/live`
   - Stream key: `my-stream-key` (any string)
2. Start streaming in OBS.
3. Watch via HLS: `http://localhost:8888/live/my-stream-key/index.m3u8`
4. The backend HLS endpoint proxies through MediaMTX when
   `MEDIA_SERVER_BASE_URL` is set.

### RTMP egress to Twitch/YouTube

Mount a custom `mediamtx.yml` to configure `runOnPublish` forwarding:

```yaml
# config/mediamtx.yml
paths:
  live:
    runOnPublish: >
      ffmpeg -i rtsp://127.0.0.1:8554/$MTX_PATH
      -c copy -f flv rtmp://live.twitch.tv/app/<YOUR_STREAM_KEY>
```

```yaml
# docker-compose.override.yml (add to mediamtx service)
volumes:
  - ./config/mediamtx.yml:/mediamtx.yml
```

### Advanced configuration

See the [MediaMTX documentation](https://github.com/bluenviron/mediamtx) for
SRT ingest, WebRTC WHIP/WHEP, path access control, and more.

---

## Service: magnetite-runtime (authoritative game server)

The magnetite-runtime process hosts WASM game modules inside the
magnetite-sandbox, executing the mag_* ABI tick loop and speaking the
ClientNet/ServerNet protocol to browser and native clients.

### What it enables

| Feature | Without runtime | With runtime |
|---------|----------------|-------------|
| Web play (WASM games) | Falls back to legacy `ws/game` path | Full ClientNet/ServerNet play |
| Authoritative simulation | None | Deterministic tick loop + delta broadcast |
| Anti-cheat | Velocity check in ws/game only | Full `magnetite-anticheat` pipeline |
| Replay logging | None | `ReplayLog` + `verify_replay` |

### Start the runtime (outside Docker)

```bash
# First start: builds from source (takes ~1 min)
./scripts/run-runtime.sh

# With custom port
RUNTIME_PORT=9001 ./scripts/run-runtime.sh

# Point at a running Magnetite API
MAGNETITE_API_URL=http://localhost:8080 \
RUNTIME_PORT=9000 \
./scripts/run-runtime.sh
```

The script searches for a pre-built binary at
`magnetite-runtime/target/release/magnetite-runtime`, then falls back to
`cargo run --release`. Set `RUNTIME_BIN=/path/to/binary` to use a specific
location.

### Start the runtime via Docker Compose

```bash
docker compose up magnetite-runtime
```

The `docker-compose.override.yml` pre-configures:
- `GAME_SERVER_WS_BASE=ws://magnetite-runtime:9000` in the backend service
- `COMMS_PROVIDER=builtin` in the backend service
- Port mapping `:9000 → 9000`

It does **not** set `MEDIA_SERVER_BASE_URL` — set that yourself if you run the
`media` profile.

### End-to-end play loop

```
Developer          Platform              Browser player
─────────          ────────              ──────────────
magnetite register ──► backend DB ◄──── fetches play manifest
  (repo/game)          (game record)    GET /api/v1/distribution/:id/play

wasm-build-runner ──► artifact_url ──► Playground.jsx loads wasm_url
  (game.wasm)          stored in DB     (wasm-bindgen JS glue)

                       GAME_SERVER_WS_BASE
backend matchmaking ──► ws://magnetite-runtime:9000
                       runtime loads game.wasm from artifact_url
                       starts tick loop

browser ──► ws://localhost:9000 ──► ClientNet::InputFrame {seq, tick, input}
        ◄── ServerNet::{Welcome, Snapshot, Delta, Ack, Reject}
```

---

## Service: WASM Build Runner (wasm32-wasip1)

The `scripts/wasm-build-runner.sh` script compiles Rust game crates for the
MOAT authoritative runtime target (`wasm32-wasip1` + mag_* ABI) and uploads
the resulting `game.wasm` to the distribution API.

### Difference from `run-wasm-build.sh`

| Script | Target | Output | Used for |
|--------|--------|--------|---------|
| `run-wasm-build.sh` | `wasm32-unknown-unknown` | `game_bg.wasm` + JS glue | Browser wasm-bindgen / Bevy WASM |
| `wasm-build-runner.sh` | `wasm32-wasip1` | `game.wasm` (bare WASI) | magnetite-runtime / mag_* ABI |

### Prerequisites

```bash
# Add the WASI target
rustup target add wasm32-wasip1

# Optional: wasm-opt for smaller binaries
brew install binaryen     # macOS
apt install binaryen      # Debian/Ubuntu

# Required tools
brew install curl jq git  # macOS
```

### Single-shot build (local game crate)

```bash
export MAGNETITE_API_URL=http://localhost:8080
export BUILD_RUNNER_TOKEN=tok_...   # generate with: magnetite token create --scope build-runner

./scripts/wasm-build-runner.sh \
  --game-id <game-uuid> \
  --path /path/to/my-game-crate
```

The script:
1. Runs `cargo build --target wasm32-wasip1 --release --features wasm`
2. Optionally runs `wasm-opt -Oz` on the output
3. Uploads `game.wasm` to S3 (if `ARTIFACT_BUCKET` is set) or reports the
   local `file://` path
4. POSTs the result to `POST /api/v1/distribution/<game-id>/builds/report`

### Daemon mode (polls for queued builds)

```bash
export MAGNETITE_API_URL=http://localhost:8080
export BUILD_RUNNER_TOKEN=tok_...
export POLL_INTERVAL=30

./scripts/wasm-build-runner.sh   # runs forever
```

The API endpoint `GET /api/v1/distribution/builds/pending` returns queued
build jobs. The runner processes each one, clones the registered repository at
the given commit SHA, compiles, and reports the result.

### Dry run (offline test)

```bash
./scripts/wasm-build-runner.sh \
  --game-id any-uuid \
  --path ./game-template-authoritative \
  --dry-run
```

`--dry-run` skips the API report calls — useful for verifying the build
pipeline without a running backend.

### S3 artifact storage

```bash
export ARTIFACT_BUCKET=my-magnetite-artifacts
export CDN_BASE_URL=https://cdn.magnetite.gg   # optional
export ARTIFACT_PREFIX=wasm                    # default: artifacts

./scripts/wasm-build-runner.sh --game-id <id> --path /path/to/crate
```

If `ARTIFACT_BUCKET` is unset, the artifact is kept locally and the
`artifact_url` reported to the API is a `file://` path — useful for testing
the full loop on one machine.

---

## nginx security headers

Both nginx configurations ship production-grade security headers:

### `nginx.conf` (Docker Compose / single-server)

| Header | Value |
|--------|-------|
| `Content-Security-Policy` | `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline' fonts.googleapis.com; font-src 'self' fonts.gstatic.com; connect-src 'self' wss: https:; img-src 'self' data: https:; media-src 'self' blob:; worker-src 'self' blob:; frame-ancestors 'none'; object-src 'none'; base-uri 'self'` |
| `X-Frame-Options` | `DENY` |
| `X-Content-Type-Options` | `nosniff` |
| `X-XSS-Protection` | `1; mode=block` |
| `Referrer-Policy` | `strict-origin-when-cross-origin` |

### `frontend/nginx.fly.conf` (Fly.io edge)

All headers from above, plus:

| Header | Value |
|--------|-------|
| `Strict-Transport-Security` | `max-age=63072000; includeSubDomains` |

HSTS is safe on Fly.io because the edge always terminates TLS. Do not add HSTS
to `nginx.conf` unless your self-hosted instance uses HTTPS end-to-end (see
[SSL/TLS](./ssl.md)).

### Tightening the CSP for production

The `unsafe-inline` in `style-src` is required by Vite's CSS injection at
build time. To remove it:

1. Build the frontend with `--reportCompressedSize false --cssCodeSplit true`.
2. Replace `'unsafe-inline'` with a nonce injected by the backend's response
   headers, or switch to CSS Modules / Tailwind (no inline styles).
3. Add `style-src-attr 'none'` to block inline `style=` attributes.

---

## Complete local dev walkthrough

### 1. Clone and start

```bash
git clone https://github.com/magnetite-platform/magnetite.git
cd magnetite
cp .env.example .env
docker compose up -d
```

### 2. Run migrations and seed (first run)

```bash
docker compose exec backend sqlx migrate run
# Optional: seed test data
curl -X POST http://localhost:8080/api/v1/admin/seed \
  -H "Authorization: Bearer $ADMIN_TOKEN"
```

### 3. Register a game and build WASM

```bash
# Install the Magnetite CLI
cargo install --path magnetite-cli

# Register your game repo with the platform
magnetite register \
  --api http://localhost:8080 \
  --repo owner/my-game \
  --name "My Game"
# → game UUID printed

# Build game.wasm and upload to the local API
BUILD_RUNNER_TOKEN=<your-token> \
MAGNETITE_API_URL=http://localhost:8080 \
./scripts/wasm-build-runner.sh \
  --game-id <game-uuid> \
  --path ./game-template-authoritative
```

### 4. Start the runtime (if not using Docker Compose)

```bash
MAGNETITE_API_URL=http://localhost:8080 \
./scripts/run-runtime.sh
# → Listening on ws://127.0.0.1:9000
```

Set `GAME_SERVER_WS_BASE=ws://localhost:9000` in the backend `.env` so
matchmaking routes sessions to the runtime.

### 5. Play the game

1. Open `http://localhost:3000` in a browser.
2. Log in (use the seeded `admin@magnetite.local` / `admin123` account in dev).
3. Navigate to the game page.
4. Click **Play** — the frontend fetches the play manifest, loads `game.wasm`,
   opens a WebSocket to the runtime at `ws://localhost:9000`, and starts the
   tick loop.

### 6. Stream (optional)

1. Start OBS → Settings → Stream → Custom → Server: `rtmp://localhost:1935/live`
2. Click **Start Streaming** in OBS.
3. In the Magnetite UI, click **Go Live** and set your RTMP key to `live`.
4. Other users can watch at `http://localhost:3000/streams`.

---

## Port reference

| Service | Port | Notes |
|---------|------|-------|
| Frontend | 3000 | nginx SPA; proxies /api/ and /ws to backend |
| Backend API | 8080 | Axum REST + WebSocket |
| magnetite-runtime | 9000 | Authoritative WS game server |
| MediaMTX HLS/API | 8888 | HLS manifest + MediaMTX REST API |
| MediaMTX RTMP | 1935 | OBS/FFmpeg ingest |
| MediaMTX WebRTC | 8189/udp | WHIP ingest |
| PostgreSQL | 5432 | |
| Redis | 6379 | |
| MailHog SMTP | 1025 | Dev email capture |
| MailHog Web UI | 8025 | Browse outbound emails |
| pgAdmin | 5050 | DB browser |

All ports can be overridden via `.env` using the corresponding `_PORT` variable
(e.g. `MEDIAMTX_HLS_PORT`, `RUNTIME_PORT`).

---

## Production notes

- Expose only ports 80/443 externally. Keep all other ports on an internal
  network or behind a firewall.
- Replace MailHog with a real email provider (Resend or SES) — see
  [External Dependencies](./external-dependencies.md).
- For production streaming at scale, run MediaMTX on a dedicated machine or
  use a managed SFU (LiveKit, mediasoup) — see
  [External Dependencies](./external-dependencies.md).
- Enable TLS — see [SSL/TLS](./ssl.md). HSTS is active on Fly.io deployments
  automatically via `nginx.fly.conf`.
- The magnetite-runtime Dockerfile (`magnetite-runtime/Dockerfile`) is
  referenced by `docker-compose.override.yml`. Build it once with
  `docker compose build magnetite-runtime`.
