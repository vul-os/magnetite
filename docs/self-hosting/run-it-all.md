# Run it all — local end-to-end runbook

> Last updated: 2026-06-03 (INFRA-E2E wave)

This is the single authoritative runbook for spinning up the complete local
Magnetite stack from a fresh clone, building a game, running the authoritative
runtime, opening the Studio, playing in the browser, and going live with
MediaMTX. Every command is accurate to the scripts and docker-compose files
in this repository.

---

## Architecture at a glance

```
localhost
  ├── :3000   Frontend   — React SPA (nginx, hot-reload on :5173 in dev mode)
  ├── :8080   Backend    — Axum REST + WebSocket (auth, matchmaking, economy, …)
  ├── :9000   Runtime    — magnetite-runtime authoritative game-server (WS)
  ├── :8888   MediaMTX   — HLS watch + REST API
  ├── :1935   MediaMTX   — RTMP ingest (OBS / FFmpeg)
  ├── :8889   MediaMTX   — WebRTC / WHIP ingest
  ├── :5432   PostgreSQL
  ├── :6379   Redis
  ├── :1025   MailHog    — SMTP capture (dev only)
  └── :8025   MailHog    — Web UI (browse sent emails)
```

The backend connects to MediaMTX via `http://mediamtx:8888` (Docker Compose
internal network). Matchmaking routes players to the runtime via
`GAME_SERVER_WS_BASE` (pre-wired in `docker-compose.override.yml`).

---

## Prerequisites

| Tool | Minimum version | Install |
|------|-----------------|---------|
| Docker + Docker Compose | v24+ | [docker.com/get-started](https://www.docker.com/get-started/) |
| Rust + cargo | stable 1.75+ | `curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \| sh` |
| rustup `wasm32-wasip1` target | — | `rustup target add wasm32-wasip1` |
| curl, jq, git | any recent | OS package manager |

wasm-opt (optional, for smaller `.wasm` artifacts):

```bash
brew install binaryen   # macOS
apt install binaryen    # Debian/Ubuntu
```

---

## Step 1 — Clone and configure

```bash
git clone https://github.com/magnetite-platform/magnetite.git
cd magnetite
cp .env.example .env
```

The default `.env` works for local development with no edits. All external
credentials (Paystack, Wise, Resend) are empty by default — the backend
returns clear HTTP 502 errors for the affected endpoints rather than silent
failures.

To enable sandbox payment simulation add:

```dotenv
PAYMENTS_SANDBOX=true
WISE_SANDBOX=true
```

---

## Step 2 — Start the stack with Docker Compose

```bash
docker compose up -d
```

This starts: PostgreSQL, Redis, MediaMTX, the Backend API, the Frontend, and
MailHog. The `magnetite-runtime` process runs separately (step 5).

Wait for all services to be healthy:

```bash
docker compose ps
```

Verify each service responds:

```bash
# Backend API
curl -s http://localhost:8080/health
# → {"status":"success","data":{...}}

# Frontend
curl -s -o /dev/null -w "%{http_code}" http://localhost:3000
# → 200

# MediaMTX REST API
curl -s http://localhost:8888/v3/config/global/get | jq '.loglevel'
# → "info"

# MailHog Web UI
curl -s -o /dev/null -w "%{http_code}" http://localhost:8025
# → 200
```

---

## Step 3 — Run database migrations (first run only)

```bash
docker compose exec backend sqlx migrate run
```

On success you will see `Applied N migration(s)`. Subsequent runs are
idempotent — sqlx skips already-applied migrations.

To seed a development admin account (optional):

```bash
# Replace <TOKEN> with the value of ADMIN_SEED_TOKEN in .env (default: dev-seed-token)
curl -s -X POST http://localhost:8080/api/v1/admin/seed \
  -H "Authorization: Bearer dev-seed-token"
# Seeds admin@magnetite.local / admin123 and developer@magnetite.local / developer123
```

---

## Step 4 — Register a game and build the WASM artifact

Install the `magnetite` CLI from the workspace:

```bash
cargo install --path magnetite-cli
```

Register your game with the platform (uses `game-template-authoritative` as
the reference game):

```bash
magnetite register \
  --api http://localhost:8080 \
  --repo magnetite-platform/game-template-authoritative \
  --name "Arena Shooter Demo"
```

The command prints a `game-uuid`. Use it in the next step.

Build the authoritative WASM artifact and upload it to the local API:

```bash
export MAGNETITE_API_URL=http://localhost:8080
export BUILD_RUNNER_TOKEN=<token-from-register-output>

./scripts/wasm-build-runner.sh \
  --game-id <game-uuid> \
  --path ./game-template-authoritative
```

What this script does:

1. Runs `cargo build --target wasm32-wasip1 --release --features wasm` inside
   the game crate.
2. Optionally runs `wasm-opt -Oz` on the output (if binaryen is installed).
3. Reports the artifact to `POST /api/v1/distribution/<game-uuid>/builds/report`
   — the backend stores the `artifact_url` and sets `build_status = 'success'`.

**Dry-run** (offline test, no API call):

```bash
./scripts/wasm-build-runner.sh \
  --game-id any-uuid \
  --path ./game-template-authoritative \
  --dry-run
```

---

## Step 5 — Start the magnetite-runtime authoritative server

The runtime is a separate process (not part of `docker compose up`). It hosts
WASM game modules inside the sandbox and speaks the `ClientNet`/`ServerNet`
WebSocket protocol.

```bash
MAGNETITE_API_URL=http://localhost:8080 \
./scripts/run-runtime.sh
```

Expected output:

```
[magnetite-runtime] Starting magnetite-runtime authoritative game-server host
[magnetite-runtime]   Listen:       ws://127.0.0.1:9000
[magnetite-runtime]   Workers:      0 (0 = auto)
[magnetite-runtime]   API:          http://localhost:8080
```

The backend's `GAME_SERVER_WS_BASE=ws://localhost:9000` is pre-set in
`docker-compose.override.yml` when running the backend via Docker Compose.
If you run the backend natively, set it in your shell:

```bash
export GAME_SERVER_WS_BASE=ws://localhost:9000
```

### Alternative — via Docker Compose

The `docker-compose.override.yml` includes a `magnetite-runtime` service
definition. Build and start it with:

```bash
docker compose build magnetite-runtime
docker compose up -d magnetite-runtime
```

This automatically sets `GAME_SERVER_WS_BASE` in the backend container.

---

## Step 6 — Open the Studio and create/play a game

1. Open [http://localhost:3000](http://localhost:3000) in your browser.
2. Log in with `admin@magnetite.local` / `admin123` (or register a new account).
3. Navigate to **Developer Portal → Studio**.
4. Click **New Game** to scaffold a project (or pick the game you registered
   in step 4).
5. Connect your GitHub repository when prompted, or skip for a local build.

### Play in the browser

1. Navigate to the game detail page (Discover → your game).
2. Click **Play**.
3. The frontend fetches the play manifest from
   `GET /api/v1/distribution/<game-id>/play`, which returns:
   ```json
   {
     "ws_url": "ws://localhost:9000",
     "wasm_url": "file:///tmp/magnetite-wasip1-builds/…/game.wasm",
     "game_id": "…",
     "version_id": "…"
   }
   ```
4. The `magnetite-web-client` opens a WebSocket to `ws_url` and sends
   `ClientNet::InputFrame` frames. The runtime loads `game.wasm` via
   `WasmExecutor`, starts the tick loop, and broadcasts
   `ServerNet::{Welcome, Snapshot, Delta, Ack, Reject}`.
5. Use keyboard and mouse to play. The canvas renders the arena shooter view.

### Alternative — one-command local dev

If you just want to run and play your own game crate locally without the
full stack:

```bash
cd my-game-crate
magnetite dev
```

This builds the WASM artifact, loads it into the sandbox, starts a
`SingleRoom` runtime, and prints a connect URL you can open directly in the
browser.

---

## Step 7 — Go live with MediaMTX

MediaMTX is already running as part of `docker compose up`. To go live:

### Stream from OBS

1. Open OBS → Settings → Stream → Custom.
2. Set **Server** to `rtmp://localhost:1935/live`.
3. Set **Stream key** to any identifier, for example `my-stream-key`.
4. Click **Start Streaming**.

### Register the stream in the Magnetite UI

In the Magnetite frontend, click **Go Live** on your game page and enter the
same RTMP key you used in OBS. The backend stores the stream record and
constructs the HLS manifest URL:

```
http://localhost:8888/live/my-stream-key/index.m3u8
```

### Watch via HLS

Other users navigate to the stream page. The frontend fetches the HLS URL
from `GET /api/v1/streams/<id>/hls`, which 302-redirects to MediaMTX. HLS.js
handles adaptive playback in the browser.

Verify MediaMTX is receiving the stream:

```bash
curl -s http://localhost:8888/v3/paths/list | jq '.items[].name'
# → "my-stream-key"
```

---

## Verify the full end-to-end pipeline

Run the MOAT demo script to confirm build → sandbox → convergence → replay in
one command:

```bash
./scripts/moat-demo.sh
```

This:
1. Compiles `game-template-authoritative` to `wasm32-wasip1`.
2. Loads the `.wasm` via `WasmExecutor` (Wasmtime, fuel-metered).
3. Runs the same game via `NativeExecutor` in parallel.
4. Asserts `state_hash` is identical on every tick (determinism parity).
5. Asserts `verify_replay` returns `Clean` (anti-cheat tamper detection).
6. Launches a live `GameServer` on an ephemeral port and prints the WebSocket
   connect URL.

Results are written to `/tmp/demo.txt`.

---

## Port reference

| Service | Port | Protocol | Notes |
|---------|------|----------|-------|
| Frontend | 3000 | HTTP | React SPA (nginx); hot-reload on 5173 in dev |
| Backend API | 8080 | HTTP / WS | Axum REST + WebSocket |
| magnetite-runtime | 9000 | WS | Authoritative game server |
| MediaMTX HLS + API | 8888 | HTTP | HLS `.m3u8` + MediaMTX REST API |
| MediaMTX RTMP | 1935 | TCP | OBS / FFmpeg ingest |
| MediaMTX WebRTC | 8889 | TCP+UDP | WHIP ingest (browser camera) |
| PostgreSQL | 5432 | TCP | |
| Redis | 6379 | TCP | |
| MailHog SMTP | 1025 | TCP | Dev email capture |
| MailHog Web UI | 8025 | HTTP | Browse captured emails |
| pgAdmin | 5050 | HTTP | DB browser |

All ports can be overridden in `.env` using the corresponding `_PORT`
variable (e.g. `MEDIAMTX_HLS_PORT`, `RUNTIME_PORT`, `BACKEND_PORT`).

---

## Troubleshooting

### `docker compose up` fails on the backend health check

The backend waits for PostgreSQL and Redis to be healthy before starting.
Check their status:

```bash
docker compose logs postgres redis
```

If PostgreSQL fails to start, check that port 5432 is not already in use:

```bash
lsof -i :5432
```

### `wasm-build-runner.sh` — `No Cargo.toml found`

Ensure you pass the path to the crate root (the directory containing
`Cargo.toml`), not the workspace root:

```bash
./scripts/wasm-build-runner.sh -g <uuid> -p ./game-template-authoritative
```

### `run-runtime.sh` — `cargo run` takes a long time on first run

The runtime depends on Wasmtime, which has a large build graph. Subsequent
builds use the cache. Set `RUNTIME_BIN` to point to a pre-built binary to
skip the build step.

### Backend returns HTTP 503 for `/streams/:id/hls`

`MEDIA_SERVER_BASE_URL` is empty or MediaMTX is not running. Check:

```bash
docker compose ps mediamtx
curl -s http://localhost:8888/v3/config/global/get | jq '.loglevel'
```

If MediaMTX is unhealthy, restart it:

```bash
docker compose restart mediamtx
```

### Play manifest returns `wasm_url: null`

The game has no registered WASM artifact. Run the build step (step 4) and
ensure `wasm-build-runner.sh` reports `outcome: success` for the game UUID.

---

## Further reading

- [local-infra.md](local-infra.md) — detailed notes on each local stand-in
  service (MediaMTX, magnetite-runtime, wasm-build-runner)
- [streaming.md](streaming.md) — MediaMTX configuration, RTMP egress to
  Twitch/YouTube, HLS latency tuning
- [environment-variables.md](environment-variables.md) — all env vars
- [external-dependencies.md](external-dependencies.md) — what each Bucket-D
  service enables and its honest absent behaviour
- [../moat/play-in-browser.md](../moat/play-in-browser.md) — browser play
  flow deep-dive (web client protocol, renderer extension)
- [../moat/quickstart.md](../moat/quickstart.md) — CLI developer walkthrough
